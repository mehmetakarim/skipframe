import type { PrintScene } from '../render/PrintScene';
import { layerForFrame, viewForFrame, type LayerTiming } from '../stores/scene';
import type { FrameSink } from './sinks';
import type { ExportProgress, ExportTarget } from './types';

/**
 * The render loop that produces a video.
 *
 * It walks the frame index one step at a time with no clock involved: frame `n` shows exactly
 * the layer `layerForFrame` says it shows, which is the same function the preview scrubber uses.
 * A slow machine produces the same file as a fast one, only later.
 *
 * The viewport's own renderer does the drawing, resized to the export resolution for the
 * duration. Building a second scene would mean a second copy of the geometry in video memory —
 * tens of megabytes for a large print — to save a resize.
 */

export interface RunExportOptions {
  scene: PrintScene;
  canvas: HTMLCanvasElement;
  sink: FrameSink;
  target: ExportTarget;
  frameCount: number;
  layerCount: number;
  /** Holds, easing and layer skip. Passed in rather than read, so the caller owns the shape. */
  timing: LayerTiming;
  signal: AbortSignal;
  onProgress: (progress: Partial<ExportProgress>) => void;
}

/** Yield to the browser often enough that the progress panel keeps painting and Cancel works. */
const YIELD_EVERY = 4;

export async function runExport(options: RunExportOptions): Promise<number> {
  const { scene, canvas, sink, target, frameCount, layerCount, timing, signal, onProgress } =
    options;

  // Remember the viewport's own size so the studio looks untouched afterwards.
  const restoreWidth = canvas.clientWidth;
  const restoreHeight = canvas.clientHeight;
  const restoreLayer = scene.currentLayer;

  onProgress({ stage: 'preparing', frame: 0, frameCount });

  // Pixel ratio 1: the export resolution is stated in pixels, not in CSS units.
  scene.resize(target.width, target.height, 1);

  try {
    await sink.open();
    onProgress({ stage: 'rendering' });

    const startedAt = performance.now();
    let smoothedRate = 0;

    for (let frame = 0; frame < frameCount; frame++) {
      if (signal.aborted) {
        sink.abort();
        onProgress({ stage: 'cancelled' });
        return 0;
      }

      // Both the layer and the camera are pure functions of the frame index — the same two the
      // preview calls — so the run is reproducible and the camera move is baked in.
      const view = viewForFrame(frame, frameCount);
      scene.setView(view.azimuth, view.elevation, view.zoom);
      scene.setLayer(layerForFrame(frame, frameCount, layerCount, timing));
      scene.render();
      await sink.write(canvas, frame);

      const done = frame + 1;
      const elapsed = (performance.now() - startedAt) / 1000;
      const rate = done / Math.max(elapsed, 1e-6);
      // Exponential smoothing, so the first slow frame does not dominate the estimate.
      smoothedRate = smoothedRate === 0 ? rate : smoothedRate * 0.8 + rate * 0.2;

      onProgress({
        frame: done,
        rate: smoothedRate,
        etaS: smoothedRate > 0 ? (frameCount - done) / smoothedRate : null,
      });

      if (frame % YIELD_EVERY === 0) await nextTick();
    }

    onProgress({ stage: 'finishing' });
    const bytes = await sink.close();
    onProgress({ stage: 'writing', bytes });
    return bytes;
  } finally {
    scene.resize(restoreWidth, restoreHeight);
    scene.setLayer(restoreLayer);
    scene.render();
  }
}

function nextTick(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}
