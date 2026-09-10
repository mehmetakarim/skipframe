<script setup lang="ts">
/**
 * The export bar states what will be produced before anything is produced: resolution, length,
 * frame rate, codec. "Videoyu çıkart" renders this file now; "Kuyruğa ekle" hands it and the
 * current settings to the batch queue instead.
 */
import { computed } from 'vue';

import SfButton from '../ui/SfButton.vue';
import { project } from '../../stores/project';
import { duration, resolution, scene } from '../../stores/scene';
import { openExportDialog } from '../../stores/exportJob';
import { addPaths, queue } from '../../stores/queue';
import { goTo } from '../../stores/ui';

const ready = computed(() => project.status === 'ready');
const size = computed(() => `${resolution.value[0]} × ${resolution.value[1]}`);

const alreadyQueued = computed(
  () => project.path !== null && queue.jobs.some((j) => j.path === project.path),
);

/**
 * Send the open file to the queue, carrying the studio's current output settings with it.
 * The scene preset goes too, because the queue applies one preset to everything it renders.
 */
async function addToQueue() {
  if (!project.path) return;
  queue.settings.preset = scene.preset;
  queue.settings.aspect = scene.aspect;
  queue.settings.fps = scene.fps;
  queue.settings.durationS = scene.durationS;
  await addPaths([project.path]);
  goTo('queue');
}
</script>

<template>
  <footer class="exportbar">
    <SfButton variant="primary" :disabled="!ready" @click="openExportDialog">
      Videoyu çıkart
    </SfButton>

    <div class="spec">
      <span>{{ size }}</span>
      <span class="sep">·</span>
      <span>{{ duration.slice(0, 5) }}</span>
      <span class="sep">·</span>
      <span>{{ scene.fps }} fps</span>
      <span class="sep">·</span>
      <span>MP4 H.264</span>
    </div>

    <div class="spacer" />

    <SfButton variant="outline" :disabled="!ready || alreadyQueued" @click="addToQueue">
      {{ alreadyQueued ? 'Kuyrukta' : 'Kuyruğa ekle' }}
    </SfButton>
  </footer>
</template>

<style scoped>
.exportbar {
  height: var(--exportbar-height);
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-5);
  padding: 0 var(--space-5);
  background: var(--bg-raised);
  border-top: 1px solid var(--border);
}

.spec {
  display: flex;
  gap: 18px;
  font-family: var(--font-mono);
  font-size: var(--type-data-size);
  color: var(--text-secondary);
}

.sep {
  color: var(--border-strong);
}

.spacer {
  flex: 1;
}
</style>
