import { PrintScene } from '../render/PrintScene';

/**
 * The renderer the batch queue draws into.
 *
 * It owns a canvas that is not in the document. That is the whole point: a queue run must
 * survive the user walking back to the studio, and a canvas owned by a component dies with the
 * component. The studio's own renderer is left alone, so a queue job never disturbs what is on
 * screen — and only one of the two holds a print's geometry at a time, because the queue's is
 * released as soon as the run ends.
 */

let canvas: HTMLCanvasElement | null = null;
let printScene: PrintScene | null = null;

export interface QueueRenderer {
  scene: PrintScene;
  canvas: HTMLCanvasElement;
}

export function acquireQueueRenderer(): QueueRenderer {
  if (!canvas) {
    canvas = document.createElement('canvas');
    // A starting size; runExport sets the real one per job.
    canvas.width = 1080;
    canvas.height = 1920;
  }
  if (!printScene) {
    printScene = new PrintScene(canvas, { maxPixelRatio: 1 });
  }
  return { scene: printScene, canvas };
}

/** Drop the WebGL context and the print's buffers once the run is over. */
export function releaseQueueRenderer(): void {
  printScene?.dispose();
  printScene = null;
  canvas = null;
}
