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
  applyAppearanceTo(printScene);
  applyViewTo(printScene);
  printScene.setLayer(currentLayer.value);
}

/**
 * Everything except the camera and the layer — the settings that change when the user edits a
 * rail control, not once per frame.
 *
 * The viewport re-runs this inside a `watchEffect`, so every store field read here becomes a
 * dependency automatically. That is the whole point of it being a function: a hand-written watch
 * list drifted from this one the moment tool colours were added, and editing an extruder's
 * colour stopped reaching the renderer.
 */
export function applyAppearanceTo(printScene: PrintScene): void {
  printScene.setPlate(scene.plate);
  printScene.setBackground(scene.background);
  printScene.setToolColours(scene.toolColours);
  printScene.setColourByFeature(scene.colourByFeature);
  printScene.setLight(scene.light);
  printScene.setSurface(scene.surface);
  printScene.setShowTravel(!scene.hideTravel);
  printScene.setHighlightCurrentLayer(scene.highlightCurrentLayer);
  printScene.setFov(scene.camera.fovDeg);
}

/** Just the camera, which moves far more often than the rest. */
export function applyViewTo(printScene: PrintScene): void {
  const { azimuth, elevation, zoom } = currentView.value;
  printScene.setView(azimuth, elevation, zoom);
}
