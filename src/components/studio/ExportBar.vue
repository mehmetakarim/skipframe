<script setup lang="ts">
/**
 * The export bar states what will be produced before anything is produced: resolution, length,
 * frame rate, codec. "Videoyu çıkart" opens the export dialog; the queue is not built yet.
 */
import { computed } from 'vue';

import SfButton from '../ui/SfButton.vue';
import { project } from '../../stores/project';
import { duration, resolution, scene } from '../../stores/scene';
import { openExportDialog } from '../../stores/exportJob';

const ready = computed(() => project.status === 'ready');
const size = computed(() => `${resolution.value[0]} × ${resolution.value[1]}`);
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

    <SfButton variant="outline" :disabled="!ready">Kuyruğa ekle</SfButton>
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
