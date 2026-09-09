<script setup lang="ts">
/**
 * Transport and scrubber.
 *
 * The frame index is the single source of truth for what is on screen — that is what makes an
 * export reproducible. Preview and export differ only in how that index is *advanced*:
 *
 * * **Preview** is paced by the clock, so a twelve-second clip plays in twelve seconds. On a
 *   120 Hz display it lands on the same frames as on a 60 Hz one, and on a slow machine it
 *   drops frames rather than playing in slow motion.
 * * **Export** will walk the index one frame at a time with no clock involved, so it produces
 *   the same video every run regardless of how long each frame took.
 *
 * Advancing one index per animation frame would have made preview speed depend on the display's
 * refresh rate, which is exactly the coupling the frame-index rule exists to avoid.
 */
import { onBeforeUnmount, computed } from 'vue';

import { duration, frameCount, scene, seekToFrame, timecode } from '../../stores/scene';
import { project } from '../../stores/project';

const ready = computed(() => project.status === 'ready');
const progress = computed(() => (scene.frame / Math.max(frameCount.value - 1, 1)) * 100);

let raf = 0;
let startedAt = 0;
let startFrame = 0;

function tick(now: number) {
  if (!scene.playing) return;

  const elapsed = (now - startedAt) / 1000;
  const target = startFrame + elapsed * scene.fps * scene.speed;

  if (target >= frameCount.value) {
    if (scene.loop) {
      startedAt = now;
      startFrame = 0;
      scene.frame = 0;
    } else {
      scene.frame = frameCount.value - 1;
      scene.playing = false;
      return;
    }
  } else {
    scene.frame = Math.floor(target);
  }
  raf = requestAnimationFrame(tick);
}

function play() {
  if (scene.frame >= frameCount.value - 1) scene.frame = 0;
  startedAt = performance.now();
  startFrame = scene.frame;
  raf = requestAnimationFrame(tick);
}

function stop() {
  scene.playing = false;
  cancelAnimationFrame(raf);
}

function togglePlay() {
  if (!ready.value) return;
  scene.playing = !scene.playing;
  if (scene.playing) play();
  else cancelAnimationFrame(raf);
}

function step(by: number) {
  stop();
  seekToFrame(scene.frame + by);
}

function scrub(e: MouseEvent) {
  if (!ready.value) return;
  const rail = e.currentTarget as HTMLElement;
  const { left, width } = rail.getBoundingClientRect();
  const t = Math.min(1, Math.max(0, (e.clientX - left) / width));
  stop();
  seekToFrame(t * (frameCount.value - 1));
}

function onDrag(e: MouseEvent) {
  if (e.buttons === 1) scrub(e);
}

onBeforeUnmount(() => cancelAnimationFrame(raf));
</script>

<template>
  <div class="timeline">
    <div class="row">
      <div class="transport">
        <button
          type="button"
          class="step"
          aria-label="Başa sar"
          :disabled="!ready"
          @click="step(-scene.fps)"
        >
          <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
          >
            <path d="M11 5 4 12l7 7M20 5l-7 7 7 7" />
          </svg>
        </button>

        <button
          type="button"
          class="play"
          :aria-label="scene.playing ? 'Duraklat' : 'Oynat'"
          :disabled="!ready"
          @click="togglePlay"
        >
          <svg v-if="scene.playing" width="11" height="11" viewBox="0 0 12 12" fill="currentColor">
            <rect x="1.5" y="1" width="3" height="10" rx="1" />
            <rect x="7.5" y="1" width="3" height="10" rx="1" />
          </svg>
          <svg v-else width="11" height="11" viewBox="0 0 12 12" fill="currentColor">
            <path d="M2.5 1.2 10.5 6l-8 4.8z" />
          </svg>
        </button>

        <button
          type="button"
          class="step"
          aria-label="İleri sar"
          :disabled="!ready"
          @click="step(scene.fps)"
        >
          <svg
            width="15"
            height="15"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
          >
            <path d="M13 5l7 7-7 7M4 5l7 7-7 7" />
          </svg>
        </button>
      </div>

      <span class="now">{{ timecode }}</span>
      <span class="total">/ {{ duration }}</span>

      <div class="spacer" />

      <span class="mode">{{ scene.speed }}× hız · döngü {{ scene.loop ? 'açık' : 'kapalı' }}</span>
    </div>

    <div
      class="rail"
      role="slider"
      :aria-valuenow="scene.frame"
      :aria-valuemin="0"
      :aria-valuemax="frameCount - 1"
      aria-label="Zaman çizelgesi"
      tabindex="0"
      @mousedown="scrub"
      @mousemove="onDrag"
      @keydown.left.prevent="step(-1)"
      @keydown.right.prevent="step(1)"
    >
      <div class="ticks" aria-hidden="true">
        <span v-for="i in 8" :key="i" />
      </div>
      <div class="played" :style="{ width: `${progress}%` }" />
      <div class="playhead" :style="{ left: `${progress}%` }" />
    </div>
  </div>
</template>

<style scoped>
.timeline {
  height: var(--timeline-height);
  flex: none;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: 14px var(--space-5);
  border-top: 1px solid var(--border);
  background: var(--bg-surface);
}

.row {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.transport {
  display: flex;
  align-items: center;
  gap: 10px;
}

.step {
  display: flex;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
}

.step:hover:not(:disabled) {
  color: var(--text-primary);
}

.play {
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 50%;
  background: var(--gold);
  color: var(--bg-surface);
  cursor: pointer;
}

.play:hover:not(:disabled) {
  background: var(--gold-hover);
}

.play:active:not(:disabled) {
  background: var(--gold-pressed);
}

button:disabled {
  color: var(--border-strong);
  cursor: not-allowed;
}

.play:disabled {
  background: var(--border);
  color: var(--text-faint);
}

.now,
.total {
  font-family: var(--font-mono);
  font-size: 12px;
}

.now {
  color: var(--text-primary);
}

.total {
  color: var(--text-faint);
}

.spacer {
  flex: 1;
}

.mode {
  font-size: 11px;
  color: var(--text-faint);
}

.rail {
  position: relative;
  height: 22px;
  border: 1px solid var(--border);
  border-radius: var(--radius-xs);
  background: var(--bg-raised);
  overflow: hidden;
  cursor: pointer;
}

.ticks {
  position: absolute;
  inset: 1px;
  display: flex;
  gap: 1px;
}

.ticks span {
  flex: 1;
  background: var(--bg-raised);
}

.played {
  position: absolute;
  inset: 0 auto 0 0;
  background: var(--gold);
  opacity: 0.16;
}

.playhead {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--gold);
}
</style>
