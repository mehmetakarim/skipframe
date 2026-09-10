import { computed, reactive } from 'vue';

import { parseFile } from '../ir/parseFile';
import { runExport } from '../export/runExport';
import { FrameSequenceSink, Mp4Sink, UnsupportedCodecError, type FrameSink } from '../export/sinks';
import { stemOf } from '../export/writeFile';
import { applySceneTo } from '../render/applyScene';
import { acquireQueueRenderer, releaseQueueRenderer } from '../queue/queueRenderer';
import { ASPECTS, applyPreset, layerTiming, scene, type Aspect, type PresetId } from './scene';
import { decorateStem, settings } from './settings';
import type { ExportFormat } from '../export/types';

/**
 * The batch queue.
 *
 * Jobs run one at a time, into a renderer of their own, with one set of settings shared by all
 * of them — which is what the design's "TÜMÜNE PRESET" means. Per-job overrides would need a
 * per-job settings panel, and the design does not have one.
 *
 * A file is parsed twice: once when it is added, to fill in its row, and once when it is
 * rendered. The second parse is a cache hit, and the alternative — holding every job's IR in
 * memory from the moment it is queued — would be tens of megabytes per row.
 */

export type JobStatus = 'pending' | 'parsing' | 'rendering' | 'done' | 'failed' | 'cancelled';

export const STATUS_LABELS: Record<JobStatus, string> = {
  pending: 'Sırada',
  parsing: 'Okunuyor',
  rendering: 'Render',
  done: 'Bitti',
  failed: 'Hata',
  cancelled: 'İptal',
};

export interface QueueJob {
  id: string;
  path: string;
  name: string;
  /** Filled in by the parse that happens when the job is added. */
  layers: number | null;
  bytes: number | null;
  dialect: string | null;
  /** Non-fatal parser warnings. A job with these still renders. */
  warnings: string[];

  status: JobStatus;
  frame: number;
  frameCount: number;
  /** Seconds left on this job, or null before there is anything to estimate from. */
  etaS: number | null;
  outputPath: string | null;
  error: string | null;
  /** Wall-clock seconds the finished job took. */
  tookS: number | null;
}

export const queue = reactive({
  jobs: [] as QueueJob[],
  running: false,
  /** Where every output lands. Chosen once, not per job. */
  outputDir: null as string | null,
  settings: {
    preset: 'showcase' as PresetId,
    format: 'mp4' as ExportFormat,
    aspect: '9:16' as Aspect,
    fps: 30,
    durationS: 12,
    scale: 1,
  },
});

let controller: AbortController | null = null;
let nextId = 1;

export const pendingCount = computed(() => queue.jobs.filter((j) => j.status === 'pending').length);
export const doneCount = computed(() => queue.jobs.filter((j) => j.status === 'done').length);
export const warningCount = computed(() => queue.jobs.filter((j) => j.warnings.length > 0).length);
export const failedCount = computed(() => queue.jobs.filter((j) => j.status === 'failed').length);

export const activeJob = computed(
  () => queue.jobs.find((j) => j.status === 'rendering' || j.status === 'parsing') ?? null,
);

/** Rough seconds left across the whole queue: this job's estimate plus a flat rate for the rest. */
export const totalEtaS = computed(() => {
  const active = activeJob.value;
  const perJob = active && active.frame > 0 && active.etaS !== null ? secondsPerJob(active) : null;
  const remaining = pendingCount.value;
  const head = active?.etaS ?? 0;
  if (perJob === null) return remaining > 0 ? null : head;
  return head + remaining * perJob;
});

function secondsPerJob(job: QueueJob): number {
  if (job.frame <= 0 || job.etaS === null) return 0;
  const elapsedPerFrame = (job.etaS ?? 0) / Math.max(job.frameCount - job.frame, 1);
  return elapsedPerFrame * job.frameCount;
}

export const frameCountFor = computed(() =>
  Math.max(1, Math.round(queue.settings.durationS * queue.settings.fps)),
);

export const resolutionFor = computed<[number, number]>(() => {
  const base = ASPECTS.find((a) => a.value === queue.settings.aspect)?.size ?? [1080, 1920];
  const even = (n: number) => Math.max(2, Math.round((n * queue.settings.scale) / 2) * 2);
  return [even(base[0]), even(base[1])];
});

// -- building the queue -------------------------------------------------------------------

/** Add files, reading each one so its row can show what it is before the run starts. */
export async function addPaths(paths: string[]): Promise<void> {
  for (const path of paths) {
    if (queue.jobs.some((j) => j.path === path)) continue;

    const job: QueueJob = {
      id: `job-${nextId++}`,
      path,
      name: path.split(/[\\/]/).pop() ?? path,
      layers: null,
      bytes: null,
      dialect: null,
      warnings: [],
      status: 'parsing',
      frame: 0,
      frameCount: 0,
      etaS: null,
      outputPath: null,
      error: null,
      tookS: null,
    };
    queue.jobs.push(job);
    // `push` stores the object as it is; the reactive array hands back a proxy on read. Writing
    // to the original changes the value but notifies nothing, so everything computed from it —
    // the pending count, and with it whether "start" is enabled — would go quietly stale.
    const tracked = queue.jobs[queue.jobs.length - 1]!;

    try {
      const { ir } = await parseFile(path);
      tracked.name = ir.meta.sourceName || tracked.name;
      tracked.layers = ir.layerCount;
      tracked.bytes = ir.meta.sourceBytes;
      tracked.dialect = ir.meta.dialect;
      tracked.warnings = [...ir.meta.warnings];
      tracked.status = 'pending';
      // The IR itself is deliberately not kept: the run re-reads it from the parse cache.
    } catch (e) {
      tracked.status = 'failed';
      tracked.error = messageOf(e);
    }
  }
}

export async function pickAndAddFiles(): Promise<void> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({
    multiple: true,
    filters: [{ name: 'G-code', extensions: ['gcode', 'gco', 'g', '3mf'] }],
  });
  if (!selected) return;
  await addPaths(Array.isArray(selected) ? selected : [selected]);
}

export function removeJob(id: string): void {
  const i = queue.jobs.findIndex((j) => j.id === id);
  if (i >= 0 && queue.jobs[i]?.status !== 'rendering') queue.jobs.splice(i, 1);
}

export function clearFinished(): void {
  queue.jobs = queue.jobs.filter((j) => j.status !== 'done' && j.status !== 'cancelled');
}

// -- output folder ------------------------------------------------------------------------

/** Whatever settings says, else `~/Videos/SkipFrame`. */
export async function defaultOutputDir(): Promise<string> {
  if (settings.outputDir) return settings.outputDir;
  const { videoDir, join } = await import('@tauri-apps/api/path');
  try {
    return await join(await videoDir(), 'SkipFrame');
  } catch {
    const { downloadDir } = await import('@tauri-apps/api/path');
    return downloadDir();
  }
}

export async function pickOutputDir(): Promise<void> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const dir = await open({ directory: true, multiple: false, title: 'Çıktı klasörü' });
  if (typeof dir === 'string') queue.outputDir = dir;
}

// -- running --------------------------------------------------------------------------------

export function cancelQueue(): void {
  controller?.abort();
}

export async function startQueue(): Promise<void> {
  if (queue.running) return;
  const todo = queue.jobs.filter((j) => j.status === 'pending');
  if (todo.length === 0) return;

  queue.outputDir ??= await defaultOutputDir();
  const dir = queue.outputDir.replace(/[\\/]+$/, '');

  // One preset for the whole run, written into the scene the renderer reads from.
  applyPreset(queue.settings.preset);
  scene.aspect = queue.settings.aspect;
  scene.fps = queue.settings.fps;
  scene.durationS = queue.settings.durationS;

  queue.running = true;
  controller = new AbortController();
  const { scene: printScene, canvas } = acquireQueueRenderer();
  const [width, height] = resolutionFor.value;
  const frameCount = frameCountFor.value;

  try {
    for (const job of todo) {
      if (controller.signal.aborted) {
        job.status = 'cancelled';
        continue;
      }

      const startedAt = performance.now();
      job.status = 'parsing';
      job.frame = 0;
      job.frameCount = frameCount;
      job.error = null;

      let sink: FrameSink | null = null;
      try {
        const { ir } = await parseFile(job.path);
        job.layers = ir.layerCount;
        job.bytes = ir.meta.sourceBytes;
        job.warnings = [...ir.meta.warnings];

        const stem = decorateStem(stemOf(ir.meta.sourceName || job.name));
        job.outputPath = queue.settings.format === 'mp4' ? `${dir}/${stem}.mp4` : `${dir}/${stem}`;

        printScene.setIr(ir);
        applySceneTo(printScene);

        sink =
          queue.settings.format === 'mp4'
            ? new Mp4Sink(job.outputPath, width, height, queue.settings.fps)
            : new FrameSequenceSink(job.outputPath, stem, frameCount);

        job.status = 'rendering';
        await runExport({
          scene: printScene,
          canvas,
          sink,
          target: { width, height },
          frameCount,
          layerCount: ir.layerCount,
          timing: layerTiming(),
          signal: controller.signal,
          onProgress: (patch) => {
            if (patch.frame !== undefined) job.frame = patch.frame;
            if (patch.etaS !== undefined) job.etaS = patch.etaS;
          },
        });

        if (controller.signal.aborted) {
          job.status = 'cancelled';
        } else {
          job.status = 'done';
          job.tookS = (performance.now() - startedAt) / 1000;
          job.etaS = null;
        }
      } catch (e) {
        sink?.abort();
        job.status = 'failed';
        job.error = e instanceof UnsupportedCodecError ? e.message : messageOf(e);
      }
    }
  } finally {
    queue.running = false;
    controller = null;
    // Hand the print's buffers back rather than holding them until the next run.
    releaseQueueRenderer();
  }
}

function messageOf(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
