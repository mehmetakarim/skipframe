import type { PrintScene } from './PrintScene';
import { currentLayer, currentView, scene } from '../stores/scene';

/**
 * Push the whole scene state at a renderer.
 *
 * The store is the single source of truth and the renderer holds no settings of its own, so
 * there has to be exactly one place that copies one into the other. Having the viewport own it
 * privately meant the export harness drove a renderer with default plate and background while
 * the store said otherwise — the sort of drift this file exists to prevent.
 */
export function applySceneTo(printScene: PrintScene): void {
  printScene.setPlate(scene.plate);
  printScene.setBackground(scene.background);
  printScene.setFilamentColour(scene.filaments[scene.filamentIndex] ?? '#c9ccc6');
  printScene.setLight(scene.light);
  printScene.setSurface(scene.surface);
  printScene.setShowTravel(!scene.hideTravel);
  printScene.setHighlightCurrentLayer(scene.highlightCurrentLayer);
  printScene.setFov(scene.camera.fovDeg);
  applyViewTo(printScene);
  printScene.setLayer(currentLayer.value);
}

/** Just the camera, which moves far more often than the rest. */
export function applyViewTo(printScene: PrintScene): void {
  const { azimuth, elevation, zoom } = currentView.value;
  printScene.setView(azimuth, elevation, zoom);
}
