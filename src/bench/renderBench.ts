import type { PrintScene } from '../render/PrintScene';
import type { Ir } from '../ir/types';

/**
 * Phase-0 risk (b): can one merged geometry hold 60 fps while the layer scrub sweeps the whole
 * print?
 *
 * Two numbers are taken, because they answer different questions:
 *
 * * **Frame interval** under `requestAnimationFrame` is what the user feels. It is capped by
 *   vsync, so a healthy result looks like a tight cluster at the display's refresh interval.
 * * **Render cost** is the same sweep with vsync taken out of the picture: how long one
 *   `renderer.render()` actually takes. This is the number with headroom in it.
 *
 * When `EXT_disjoint_timer_query_webgl2` is available the GPU side is measured too.
 */

export interface RenderBenchResult {
  frames: number;
  layers: number;
  segments: number;
  canvas: { width: number; height: number; pixelRatio: number };

  /** requestAnimationFrame interval, milliseconds. */
  interval: Stats;
  /** Wall-clock cost of `renderer.render()`, milliseconds. */
  cpu: Stats;
  /** GPU time from a timer query, milliseconds. Null when the extension is missing. */
  gpu: Stats | null;

  /** Share of frames that arrived within a 60 Hz budget. */
  under60HzBudget: number;
  meanFps: number;
  drawCalls: number;
  webglVersion: 1 | 2;
  renderer: string;
}

export interface Stats {
  p50: number;
  p95: number;
  p99: number;
  max: number;
  mean: number;
}

const SIXTY_HZ_BUDGET_MS = 16.7;

export async function runRenderBench(
  scene: PrintScene,
  ir: Ir,
  options: { width?: number; height?: number; pixelRatio?: number; frames?: number } = {},
): Promise<RenderBenchResult> {
  const width = options.width ?? 1080;
  const height = options.height ?? 1920;
  const pixelRatio = options.pixelRatio ?? 1;
  const frames = options.frames ?? Math.max(ir.layerCount, 300);

  scene.resize(width, height, pixelRatio);

  const gl = scene.renderer.getContext();
  const timer = createGpuTimer(gl);

  // A warm-up sweep so shader compilation and the first buffer upload do not land in the sample.
  for (let i = 0; i < 30; i++) {
    scene.setLayer(Math.floor((i / 30) * (ir.layerCount - 1)));
    scene.render();
  }
  const intervals: number[] = [];
  const cpu: number[] = [];
  const calls: number[] = [];
  let last = performance.now();

  await new Promise<void>((resolve) => {
    let frame = 0;
    const step = () => {
      const now = performance.now();
      if (frame > 0) intervals.push(now - last);
      last = now;

      scene.setLayer(Math.floor((frame / (frames - 1)) * (ir.layerCount - 1)));

      timer?.begin();
      const t0 = performance.now();
      scene.render();
      cpu.push(performance.now() - t0);
      timer?.end();
      // Three resets `info` at the start of every render, so this has to be read per frame.
      calls.push(scene.renderer.info.render.calls);

      frame += 1;
      if (frame >= frames) {
        resolve();
        return;
      }
      requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  });

  const gpu = timer ? await timer.collect() : null;
  const drawCalls = mean(calls);

  const debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
  const rendererName = debugInfo
    ? String(gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL))
    : String(gl.getParameter(gl.RENDERER));

  return {
    frames,
    layers: ir.layerCount,
    segments: ir.segmentCount,
    canvas: { width, height, pixelRatio },
    interval: stats(intervals),
    cpu: stats(cpu),
    gpu: gpu && gpu.length ? stats(gpu) : null,
    under60HzBudget:
      intervals.filter((v) => v <= SIXTY_HZ_BUDGET_MS + 0.5).length / Math.max(intervals.length, 1),
    meanFps: 1000 / (mean(intervals) || 1),
    drawCalls,
    webglVersion: scene.renderer.capabilities.isWebGL2 ? 2 : 1,
    renderer: rendererName,
  };
}

function stats(values: number[]): Stats {
  if (values.length === 0) return { p50: 0, p95: 0, p99: 0, max: 0, mean: 0 };
  const sorted = [...values].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))]!;
  return {
    p50: at(0.5),
    p95: at(0.95),
    p99: at(0.99),
    max: sorted[sorted.length - 1]!,
    mean: mean(values),
  };
}

function mean(values: number[]): number {
  return values.length ? values.reduce((a, b) => a + b, 0) / values.length : 0;
}

interface GpuTimer {
  begin(): void;
  end(): void;
  collect(): Promise<number[]>;
}

/**
 * `EXT_disjoint_timer_query_webgl2` gives real GPU milliseconds. It is absent on plenty of
 * drivers (and on Safari), so every caller has to cope with `null`.
 */
function createGpuTimer(gl: WebGLRenderingContext | WebGL2RenderingContext): GpuTimer | null {
  if (!('createQuery' in gl)) return null;
  const gl2 = gl as WebGL2RenderingContext;
  const ext = gl2.getExtension('EXT_disjoint_timer_query_webgl2');
  if (!ext) return null;

  const target = (ext as unknown as { TIME_ELAPSED_EXT: number }).TIME_ELAPSED_EXT;
  const queries: WebGLQuery[] = [];
  let active: WebGLQuery | null = null;

  return {
    begin() {
      const q = gl2.createQuery();
      if (!q) return;
      gl2.beginQuery(target, q);
      active = q;
    },
    end() {
      if (!active) return;
      gl2.endQuery(target);
      queries.push(active);
      active = null;
    },
    async collect() {
      // Results are not available until the GPU has drained; a few frames of slack is enough.
      await new Promise((r) => setTimeout(r, 200));
      const out: number[] = [];
      for (const q of queries) {
        const available = gl2.getQueryParameter(q, gl2.QUERY_RESULT_AVAILABLE);
        const disjoint = gl2.getParameter(
          (ext as unknown as { GPU_DISJOINT_EXT: number }).GPU_DISJOINT_EXT,
        );
        if (available && !disjoint) {
          out.push(Number(gl2.getQueryParameter(q, gl2.QUERY_RESULT)) / 1e6);
        }
        gl2.deleteQuery(q);
      }
      return out;
    },
  };
}
