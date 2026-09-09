import { computed, reactive } from 'vue';

import { getActiveScene } from '../render/activeScene';
import { runExport } from '../export/runExport';
import { FrameSequenceSink, Mp4Sink, UnsupportedCodecError, type FrameSink } from '../export/sinks';
import { defaultBitrate } from '../export/h264';
import { revealFile, stemOf } from '../export/writeFile';
import type { ExportProgress, ExportSettings } from '../export/types';
import { ir } from './project';
import { ASPECTS, scene } from './scene';

/** The export dialog's own state: the settings being chosen and the job running, if any. */

const idleProgress = (): ExportProgress => ({
  stage: 'idle',
  frame: 0,
  frameCount: 0,
  rate: 0,
  etaS: null,
  bytes: 0,
  outputPath: null,
  error: null,
});

export const exportState = reactive({
  open: false,
  settings: {
    format: 'mp4',
    aspect: scene.aspect,
    fps: scene.fps,
    scale: 1,
    durationS: scene.durationS,
    openWhenDone: false,
  } as ExportSettings,
  progress: idleProgress(),
});

let controller: AbortController | null = null;

export const running = computed(() =>
  ['preparing', 'rendering', 'finishing', 'writing'].includes(exportState.progress.stage),
);

export const exportFrameCount = computed(() =>
  Math.max(1, Math.round(exportState.settings.durationS * exportState.settings.fps)),
);

export const exportResolution = computed<[number, number]>(() => {
  const base = ASPECTS.find((a) => a.value === exportState.settings.aspect)?.size ?? [1080, 1920];
  // H.264 wants even dimensions; a 0.5x scale of an odd number would otherwise be rejected.
  const even = (n: number) => Math.max(2, Math.round((n * exportState.settings.scale) / 2) * 2);
  return [even(base[0]), even(base[1])];
});

/** The size estimate shown next to the start button. */
export const estimatedBytes = computed(() => {
  const [w, h] = exportResolution.value;
  const { fps, format } = exportState.settings;
  if (format === 'frames') {
    // PNG of a mostly flat render compresses hard; roughly a third of a byte per pixel.
    return w * h * 0.33 * exportFrameCount.value;
  }
  return (defaultBitrate(w, h, fps) / 8) * (exportFrameCount.value / fps);
});

export function openExportDialog(): void {
  if (running.value) {
    exportState.open = true;
    return;
  }
  // The dialog opens on what the studio is currently showing.
  exportState.settings.aspect = scene.aspect;
  exportState.settings.fps = scene.fps;
  exportState.settings.durationS = scene.durationS;
  exportState.progress = idleProgress();
  exportState.open = true;
}

export function closeExportDialog(): void {
  exportState.open = false;
}

export function cancelExport(): void {
  controller?.abort();
}

export async function startExport(): Promise<void> {
  if (running.value) return;

  const active = getActiveScene();
  const model = ir.value;
  if (!active || !model) return;

  const [width, height] = exportResolution.value;
  const frameCount = exportFrameCount.value;
  const stem = stemOf(model.meta.sourceName || 'skipframe');

  let sink: FrameSink;
  let outputPath: string;
  try {
    const chosen = await chooseDestination(stem);
    if (!chosen) return;
    outputPath = chosen;
    sink =
      exportState.settings.format === 'mp4'
        ? new Mp4Sink(outputPath, width, height, exportState.settings.fps)
        : new FrameSequenceSink(outputPath, stem, frameCount);
  } catch (e) {
    exportState.progress = { ...idleProgress(), stage: 'failed', error: messageOf(e) };
    return;
  }

  exportState.progress = { ...idleProgress(), frameCount, outputPath };
  controller = new AbortController();

  try {
    const bytes = await runExport({
      scene: active.scene,
      canvas: active.canvas,
      sink,
      target: { width, height },
      frameCount,
      layerCount: model.layerCount,
      signal: controller.signal,
      onProgress: (patch) => Object.assign(exportState.progress, patch),
    });

    if (exportState.progress.stage === 'cancelled') return;

    exportState.progress.bytes = bytes;
    exportState.progress.stage = 'done';
    if (exportState.settings.openWhenDone) await revealFile(outputPath);
  } catch (e) {
    sink.abort();
    exportState.progress.stage = 'failed';
    exportState.progress.error = e instanceof UnsupportedCodecError ? e.message : messageOf(e);
  } finally {
    controller = null;
  }
}

/** MP4 asks for a file, a frame sequence asks for a folder. */
async function chooseDestination(stem: string): Promise<string | null> {
  const { open, save } = await import('@tauri-apps/plugin-dialog');

  if (exportState.settings.format === 'frames') {
    const dir = await open({ directory: true, multiple: false, title: 'Kare sekansı klasörü' });
    return typeof dir === 'string' ? dir : null;
  }

  const { videoDir, join } = await import('@tauri-apps/api/path');
  let defaultPath = `${stem}.mp4`;
  try {
    defaultPath = await join(await videoDir(), 'SkipFrame', `${stem}.mp4`);
  } catch {
    // No videos folder on this machine; the dialog's own default is fine.
  }

  const file = await save({
    defaultPath,
    filters: [{ name: 'MP4', extensions: ['mp4'] }],
  });
  return typeof file === 'string' ? file : null;
}

function messageOf(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
