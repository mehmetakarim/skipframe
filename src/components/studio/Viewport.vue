<script setup lang="ts">
/**
 * The print viewport: layer counter, aspect selector, and the WebGL canvas framed to the export
 * aspect ratio so what you see is what gets rendered.
 *
 * This is where the studio meets the renderer built in phase 0. The scene is fed by layer
 * index, which the transport derives from the frame index — never from a clock.
 */
import { computed, onBeforeUnmount, onMounted, useTemplateRef, watch } from 'vue';

import SfTabs from '../ui/SfTabs.vue';
import { PrintScene } from '../../render/PrintScene';
import { setActiveScene } from '../../render/activeScene';
import { applySceneTo, applyViewTo } from '../../render/applyScene';
import { ir } from '../../stores/project';
import { exportState, running as exporting } from '../../stores/exportJob';
import { ASPECTS, currentLayer, currentView, scene } from '../../stores/scene';
import { integer } from '../../lib/format';

const canvasRef = useTemplateRef<HTMLCanvasElement>('canvas');
const frameRef = useTemplateRef<HTMLDivElement>('frame');

let printScene: PrintScene | null = null;
let observer: ResizeObserver | null = null;
let dragging: { x: number; y: number } | null = null;

const aspectRatio = computed(() => {
  const [w, h] = ASPECTS.find((a) => a.value === scene.aspect)?.size ?? [1080, 1920];
  return `${w} / ${h}`;
});

/** Dragging writes to the store, which is why the camera section's numbers move as you drag. */
function applyScene() {
  if (printScene) applySceneTo(printScene);
}

function applyView() {
  if (printScene) applyViewTo(printScene);
}

function draw() {
  printScene?.render();
}

function resize() {
  const frame = frameRef.value;
  if (!frame || !printScene) return;
  const { width, height } = frame.getBoundingClientRect();
  if (width < 1 || height < 1) return;
  printScene.resize(width, height);
  draw();
}

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;
  printScene = new PrintScene(canvas);
  // Export drives this same renderer rather than building a second copy of the geometry.
  setActiveScene({ scene: printScene, canvas });

  observer = new ResizeObserver(resize);
  if (frameRef.value) observer.observe(frameRef.value);

  if (ir.value) printScene.setIr(ir.value);
  applyScene();
  resize();
});

onBeforeUnmount(() => {
  setActiveScene(null);
  observer?.disconnect();
  printScene?.dispose();
  printScene = null;
});

watch(ir, (next) => {
  if (!printScene || !next) return;
  printScene.setIr(next);
  applyScene();
  resize();
});

// The frame index moves far more often than anything else, so it gets its own narrow watcher.
watch(currentLayer, (layer) => {
  printScene?.setLayer(layer);
  draw();
});

watch(currentView, () => {
  applyView();
  draw();
});

watch(
  () => [
    scene.plate,
    scene.background,
    scene.filamentIndex,
    scene.filaments,
    scene.hideTravel,
    scene.highlightCurrentLayer,
    scene.camera.fovDeg,
  ],
  () => {
    applyScene();
    draw();
  },
  { deep: true },
);

watch(() => scene.aspect, resize);

// Dragging and the wheel write to the store, not to the renderer, so the camera section and the
// viewport can never disagree about where the camera is.
function onPointerDown(e: PointerEvent) {
  dragging = { x: e.clientX, y: e.clientY };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!dragging) return;
  const degrees = 0.45;
  scene.camera.azimuthDeg = wrap(scene.camera.azimuthDeg - (e.clientX - dragging.x) * degrees);
  scene.camera.elevationDeg = clamp(
    scene.camera.elevationDeg + (e.clientY - dragging.y) * degrees,
    -80,
    80,
  );
  dragging = { x: e.clientX, y: e.clientY };
}

function onPointerUp(e: PointerEvent) {
  dragging = null;
  (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  scene.camera.zoom = clamp(scene.camera.zoom * (e.deltaY > 0 ? 1 / 1.08 : 1.08), 0.3, 8);
}

function wrap(deg: number): number {
  return ((deg % 360) + 360) % 360;
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}
</script>

<template>
  <div class="viewport">
    <header class="head">
      <span class="t-counter">{{ integer(currentLayer + 1) }}</span>
      <span class="total">/ {{ integer(ir?.layerCount ?? 0) }}</span>
      <span class="t-overline">Katman</span>

      <div class="spacer" />

      <SfTabs
        v-model="scene.aspect"
        :options="ASPECTS.map((a) => ({ value: a.value, label: a.label }))"
        aria-label="Oran"
      />
    </header>

    <div class="stage">
      <div ref="frame" class="frame" :style="{ aspectRatio }">
        <canvas
          ref="canvas"
          @pointerdown="onPointerDown"
          @pointermove="onPointerMove"
          @pointerup="onPointerUp"
          @pointercancel="onPointerUp"
          @wheel="onWheel"
        ></canvas>

        <!-- The export drives this same canvas at the output resolution, so while it runs the
             viewport is showing frames that are not the user's. Cover it rather than let it
             flicker through a render they did not ask to watch. -->
        <div v-if="exporting" class="exporting">
          <span class="t-overline">Render sürüyor</span>
          <span class="exporting-count">
            {{ integer(exportState.progress.frame) }} /
            {{ integer(exportState.progress.frameCount) }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.viewport {
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: var(--bg-base);
}

.head {
  height: var(--viewport-header-height);
  flex: none;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 var(--space-5);
  border-bottom: 1px solid var(--border);
}

.total {
  font-family: var(--font-mono);
  font-size: var(--type-body-size);
  color: var(--text-faint);
}

.spacer {
  flex: 1;
}

.stage {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
}

.frame {
  position: relative;
  max-width: 100%;
  max-height: 100%;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.exporting {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  background: var(--bg-base);
}

.exporting-count {
  font-family: var(--font-mono);
  font-size: var(--type-counter-size);
  font-weight: 700;
  color: var(--gold);
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
  cursor: grab;
  touch-action: none;
}

canvas:active {
  cursor: grabbing;
}
</style>
