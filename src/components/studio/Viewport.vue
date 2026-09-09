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
import { ir } from '../../stores/project';
import { ASPECTS, currentLayer, scene } from '../../stores/scene';
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

function resize() {
  const frame = frameRef.value;
  if (!frame || !printScene) return;
  const { width, height } = frame.getBoundingClientRect();
  if (width < 1 || height < 1) return;
  printScene.resize(width, height);
  printScene.render();
}

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;
  printScene = new PrintScene(canvas);

  observer = new ResizeObserver(resize);
  if (frameRef.value) observer.observe(frameRef.value);

  if (ir.value) {
    printScene.setIr(ir.value);
    printScene.setShowTravel(!scene.hideTravel);
    // setIr leaves the whole print visible; the playhead decides what is actually shown.
    printScene.setLayer(currentLayer.value);
  }
  resize();
});

onBeforeUnmount(() => {
  observer?.disconnect();
  printScene?.dispose();
  printScene = null;
});

watch(ir, (next) => {
  if (!printScene || !next) return;
  printScene.setIr(next);
  printScene.setShowTravel(!scene.hideTravel);
  printScene.setLayer(currentLayer.value);
  resize();
});

watch(currentLayer, (layer) => {
  printScene?.setLayer(layer);
  printScene?.render();
});

watch(
  () => scene.hideTravel,
  (hidden) => {
    printScene?.setShowTravel(!hidden);
    printScene?.render();
  },
);

watch(() => scene.aspect, resize);

// Orbit and zoom, so the viewport is inspectable while the scene controls are still being built.
function onPointerDown(e: PointerEvent) {
  dragging = { x: e.clientX, y: e.clientY };
  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
  if (!dragging || !printScene) return;
  printScene.orbitBy((e.clientX - dragging.x) * -0.008, (e.clientY - dragging.y) * 0.008);
  printScene.render();
  dragging = { x: e.clientX, y: e.clientY };
}

function onPointerUp(e: PointerEvent) {
  dragging = null;
  (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  printScene?.zoomBy(e.deltaY > 0 ? 1.08 : 1 / 1.08);
  printScene?.render();
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
  max-width: 100%;
  max-height: 100%;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  overflow: hidden;
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
