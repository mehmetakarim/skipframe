<script setup lang="ts">
/**
 * Phase-0 harness. Mounted instead of the studio when the page is opened at `#bench`.
 *
 * It runs unchanged in a plain browser tab and inside the app, which is the point: the macOS
 * WebCodecs answer has to come from the same code that produced the Windows one.
 */
import { onMounted, ref, useTemplateRef } from 'vue';

import { PrintScene } from '../render/PrintScene';
import { runRenderBench, type RenderBenchResult } from './renderBench';
import { runEncodeBench, type EncodeBenchResult } from './encodeBench';
import { runExport } from '../export/runExport';
import { applyPreset, layerTiming } from '../stores/scene';
import { applySceneTo } from '../render/applyScene';
import { FrameSequenceSink, Mp4Sink } from '../export/sinks';
import { makeSyntheticIr } from './syntheticIr';
import type { Ir } from '../ir/types';

const canvasRef = useTemplateRef<HTMLCanvasElement>('canvas');
const log = ref<string[]>([]);
const render = ref<RenderBenchResult | null>(null);
const encode = ref<EncodeBenchResult | null>(null);
const source = ref('');
const running = ref(false);

const inTauri = '__TAURI_INTERNALS__' in window;

function say(line: string) {
  log.value.push(line);
  // Under `tauri dev` this is the only way the numbers reach a terminal.
  if (inTauri) {
    import('@tauri-apps/api/core').then(({ invoke }) => {
      invoke('bench_log', { line }).catch(() => {});
    });
  } else {
    console.log('[bench]', line);
  }
}

async function loadIr(): Promise<Ir> {
  const file =
    new URLSearchParams(location.hash.split('?')[1] ?? '').get('file') ??
    (import.meta.env.VITE_BENCH_FILE as string | undefined) ??
    null;
  if (inTauri && file) {
    const { parseFile } = await import('../ir/parseFile');
    const { ir, elapsedMs } = await parseFile(file, { useCache: false });
    source.value = `${ir.meta.sourceName} — ${ir.meta.dialect}, parsed + IPC in ${elapsedMs.toFixed(0)} ms`;
    say(`ir: parsed ${file} in ${elapsedMs.toFixed(0)} ms (round trip incl. IPC)`);
    return ir;
  }
  const ir = makeSyntheticIr(570, 1316);
  source.value = 'synthetic — 570 layers x 1316 paths';
  return ir;
}

async function run() {
  const canvas = canvasRef.value;
  if (!canvas || running.value) return;
  running.value = true;

  try {
    const ir = await loadIr();
    say(`ir: ${ir.layerCount} layers, ${ir.segmentCount.toLocaleString()} segments`);

    const scene = new PrintScene(canvas, { maxPixelRatio: 1 });
    scene.setIr(ir);
    scene.setShowTravel(false);

    // --- risk (b) ---------------------------------------------------------------------
    const r = await runRenderBench(scene, ir, { width: 1080, height: 1920, pixelRatio: 1 });
    render.value = r;
    say(
      `render: ${r.frames} frames at ${r.canvas.width}x${r.canvas.height} — ` +
        `interval p50 ${r.interval.p50.toFixed(2)} ms, p95 ${r.interval.p95.toFixed(2)} ms, ` +
        `max ${r.interval.max.toFixed(2)} ms; ${(r.under60HzBudget * 100).toFixed(1)}% within 60 Hz; ` +
        `mean ${r.meanFps.toFixed(1)} fps`,
    );
    say(
      `render cost: cpu p50 ${r.cpu.p50.toFixed(2)} ms / p95 ${r.cpu.p95.toFixed(2)} ms` +
        (r.gpu
          ? `, gpu p50 ${r.gpu.p50.toFixed(2)} ms / p95 ${r.gpu.p95.toFixed(2)} ms`
          : ', gpu n/a') +
        `, ${r.drawCalls.toFixed(1)} draw call(s)/frame, WebGL${r.webglVersion}, ${r.renderer}`,
    );

    // --- risk (c) ---------------------------------------------------------------------
    const e = await runEncodeBench(canvas, {
      width: 1080,
      height: 1920,
      fps: 60,
      frames: 180,
      onFrame: (i) => {
        scene.setLayer(Math.floor((i / 179) * (ir.layerCount - 1)));
        scene.render();
      },
    });
    encode.value = e;

    say(`encode: VideoEncoder ${e.hasVideoEncoder ? 'present' : 'MISSING'}`);
    for (const p of e.probes) {
      say(`  probe ${p.supported ? 'OK  ' : 'FAIL'} ${p.label}${p.error ? ` — ${p.error}` : ''}`);
    }
    if (e.error) {
      say(`encode: FAILED — ${e.error}`);
    } else {
      say(
        `encode: ${e.frames} frames of ${e.width}x${e.height}@${e.fps} via "${e.used?.label}" — ` +
          `${e.encodeFps.toFixed(1)} fps, ${(e.encodeMs / 1000).toFixed(2)} s total, ` +
          `mux ${e.muxMs.toFixed(0)} ms, ${(e.bytes / 1e6).toFixed(2)} MB, ${e.keyFrames} keyframes`,
      );
    }
    if (e.data && e.data.byteLength > 0 && inTauri) {
      const { writeFile, BaseDirectory } = await import('@tauri-apps/plugin-fs');
      await writeFile('skipframe-bench.mp4', e.data, { baseDir: BaseDirectory.Download });
      say('encode: wrote skipframe-bench.mp4 to the Downloads folder for playback checking');
    }

    // --- the export pipeline end to end ------------------------------------------------
    // Not a measurement: a check that the shipping path — runExport, the sink, the raw-body
    // IPC write — produces a file on disk, since the harness is the only way to drive it
    // without a save dialog.
    const out = import.meta.env.VITE_BENCH_OUT as string | undefined;
    if (inTauri && out) {
      // A preset with a camera move and holds at both ends, so the export check covers the
      // scene sections too and not just the encoder.
      applyPreset('showcase');
      applySceneTo(scene);
      const frames = 90;
      const t0 = performance.now();
      const sink = new Mp4Sink(out, 1080, 1920, 30);
      const bytes = await runExport({
        scene,
        canvas,
        sink,
        target: { width: 1080, height: 1920 },
        frameCount: frames,
        layerCount: ir.layerCount,
        timing: layerTiming(),
        signal: new AbortController().signal,
        onProgress: () => {},
      });
      const ms = performance.now() - t0;
      say(
        `export: ${frames} frames via "${sink.label}" in ${(ms / 1000).toFixed(2)} s — ` +
          `${(bytes / 1e6).toFixed(2)} MB written to ${out}`,
      );

      // The codec-free path, on the same journey. It is slower by construction — one file and
      // one IPC write per frame — so it is checked over a short run.
      const cut = Math.max(out.lastIndexOf('/'), out.lastIndexOf('\\'));
      const dir = `${out.slice(0, cut)}/frames-check`;
      const seqFrames = 12;
      const t1 = performance.now();
      const seqSink = new FrameSequenceSink(dir, 'check', seqFrames);
      const seqBytes = await runExport({
        scene,
        canvas,
        sink: seqSink,
        target: { width: 1080, height: 1920 },
        frameCount: seqFrames,
        layerCount: ir.layerCount,
        timing: { holdStart: 0, holdEnd: 0, easing: 'linear', layerSkip: 1 },
        signal: new AbortController().signal,
        onProgress: () => {},
      });
      const seqMs = performance.now() - t1;
      say(
        `export: ${seqFrames} PNG frames in ${(seqMs / 1000).toFixed(2)} s ` +
          `(${(seqMs / seqFrames).toFixed(0)} ms/frame) — ` +
          `${(seqBytes / 1e6).toFixed(2)} MB written to ${dir}`,
      );
    }

    // --- the batch queue, end to end ---------------------------------------------------
    // Driven through the store rather than the dialogs, which is the only way to exercise it
    // without a person clicking.
    const queueDir = import.meta.env.VITE_BENCH_QUEUE as string | undefined;
    if (inTauri && queueDir) {
      const { addPaths, queue: q, startQueue } = await import('../stores/queue');
      const paths = queueDir.split('|').slice(1);
      q.outputDir = queueDir.split('|')[0]!;
      q.settings.format = 'mp4';
      q.settings.fps = 30;
      q.settings.durationS = 2;
      q.settings.preset = 'showcase';

      const t2 = performance.now();
      await addPaths(paths);
      say(
        `queue: added ${q.jobs.length} job(s) in ${((performance.now() - t2) / 1000).toFixed(2)} s`,
      );
      for (const job of q.jobs) {
        say(
          `  ${job.name} — ${job.status}, ${job.layers ?? '?'} layers, ${job.warnings.length} warning(s)`,
        );
      }

      const t3 = performance.now();
      await startQueue();
      say(`queue: ran in ${((performance.now() - t3) / 1000).toFixed(2)} s`);
      for (const job of q.jobs) {
        say(
          `  ${job.name} — ${job.status}${job.error ? ` — ${job.error}` : ''} -> ${job.outputPath}`,
        );
      }
    }

    say('done');
  } catch (err) {
    say(`FAILED — ${String(err)}`);
  } finally {
    running.value = false;
  }
}

onMounted(run);
</script>

<template>
  <main class="bench">
    <header>
      <h1>Phase 0</h1>
      <p class="dim">{{ source || 'loading…' }}</p>
    </header>

    <div class="cols">
      <canvas ref="canvas" class="stage"></canvas>
      <pre class="log mono">{{ log.join('\n') }}</pre>
    </div>

    <button :disabled="running" @click="run">{{ running ? 'Running…' : 'Run again' }}</button>
  </main>
</template>

<style scoped>
.bench {
  display: flex;
  flex-direction: column;
  gap: var(--sf-space-4);
  padding: var(--sf-space-5);
  height: 100%;
  overflow: auto;
}

h1 {
  margin: 0;
  font-size: var(--sf-text-lg);
}

.dim {
  margin: var(--sf-space-1) 0 0;
  color: var(--sf-text-dim);
  font-size: var(--sf-text-sm);
}

.cols {
  display: flex;
  gap: var(--sf-space-5);
  align-items: flex-start;
}

.stage {
  width: 216px;
  height: 384px;
  border: 1px solid var(--sf-line);
  border-radius: var(--sf-radius);
  background: #0b0b0b;
  flex: none;
}

.log {
  flex: 1;
  margin: 0;
  padding: var(--sf-space-3);
  border: 1px solid var(--sf-line);
  border-radius: var(--sf-radius);
  background: var(--sf-surface);
  font-size: var(--sf-text-sm);
  line-height: 1.6;
  white-space: pre-wrap;
  user-select: text;
}

button {
  align-self: flex-start;
  padding: var(--sf-space-2) var(--sf-space-4);
  border: 1px solid var(--sf-line);
  border-radius: var(--sf-radius);
  background: var(--sf-surface-raised);
  color: var(--sf-text);
  font: inherit;
  cursor: pointer;
}
</style>
