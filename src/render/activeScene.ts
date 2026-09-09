import type { PrintScene } from './PrintScene';

/**
 * The one scene the studio is showing.
 *
 * Export needs the renderer and its canvas, but it is not a child of the viewport and should not
 * have to be. The viewport registers itself here on mount and clears the registration on
 * unmount; everything else asks.
 */

export interface ActiveScene {
  scene: PrintScene;
  canvas: HTMLCanvasElement;
}

let active: ActiveScene | null = null;

export function setActiveScene(next: ActiveScene | null): void {
  active = next;
}

export function getActiveScene(): ActiveScene | null {
  return active;
}
