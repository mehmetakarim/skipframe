<script setup lang="ts">
/**
 * The embedded title bar. The design draws it twice — macOS with the system traffic lights on
 * the left, Windows with our own controls on the right — and the two share everything between
 * the mark and the trailing links.
 */
import { computed } from 'vue';

import SfMark from '../SfMark.vue';
import { goTo, ui } from '../../stores/ui';
import { queue } from '../../stores/queue';

defineProps<{ fileName?: string | null }>();

const isMac = computed(() => /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent));

async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const w = getCurrentWindow();
  if (action === 'minimize') await w.minimize();
  else if (action === 'toggleMaximize') await w.toggleMaximize();
  else await w.close();
}
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <!-- On macOS the system draws the buttons over this space; we only reserve it. -->
    <div v-if="isMac" class="traffic" aria-hidden="true"><span /><span /><span /></div>

    <div class="brand">
      <SfMark :size="20" />
      <span class="name">SkipFrame</span>
      <span class="by">BY STEPPERSKIP</span>
    </div>

    <template v-if="fileName">
      <div class="divider" />
      <span class="file">{{ fileName }}</span>
    </template>

    <slot name="trailing" />

    <div class="spacer" data-tauri-drag-region />

    <nav class="links">
      <button type="button" :class="{ active: ui.screen === 'studio' }" @click="goTo('studio')">
        Stüdyo
      </button>
      <button type="button" :class="{ active: ui.screen === 'queue' }" @click="goTo('queue')">
        Kuyruk
        <span v-if="queue.jobs.length" class="badge" :class="{ running: queue.running }">
          {{ queue.jobs.length }}
        </span>
      </button>
      <button type="button" disabled title="Henüz yok">Ayarlar</button>
    </nav>

    <div v-if="!isMac" class="controls">
      <button type="button" aria-label="Küçült" @click="windowAction('minimize')">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <path d="M0 5h10" stroke="currentColor" />
        </svg>
      </button>
      <button type="button" aria-label="Büyüt" @click="windowAction('toggleMaximize')">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <rect x="0.5" y="0.5" width="9" height="9" fill="none" stroke="currentColor" />
        </svg>
      </button>
      <button type="button" class="close" aria-label="Kapat" @click="windowAction('close')">
        <svg width="10" height="10" viewBox="0 0 10 10">
          <path d="M0 0l10 10M10 0L0 10" stroke="currentColor" />
        </svg>
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  height: var(--titlebar-height);
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: 0 var(--space-4);
  background: var(--bg-raised);
  border-bottom: 1px solid var(--border);
}

.traffic {
  display: flex;
  gap: var(--space-2);
}

.traffic span {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--border-strong);
}

.brand {
  display: flex;
  align-items: center;
  gap: 9px;
  margin-left: var(--space-2);
}

.name {
  font-size: var(--type-body-size);
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.01em;
}

.by {
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.14em;
  color: var(--text-faint);
}

.divider {
  width: 1px;
  height: 16px;
  background: var(--border);
  margin: 0 var(--space-1);
}

.file {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spacer {
  flex: 1;
  align-self: stretch;
}

.links {
  display: flex;
  gap: 18px;
}

.links button {
  border: 0;
  background: none;
  padding: 0;
  font-size: var(--type-label-size);
  color: var(--text-muted);
  cursor: pointer;
}

.links button {
  display: flex;
  align-items: center;
  gap: 6px;
}

.links button:hover:not(:disabled) {
  color: var(--text-primary);
}

.links button.active {
  color: var(--text-primary);
}

.links button:disabled {
  color: var(--border-strong);
  cursor: not-allowed;
}

.badge {
  padding: 1px 5px;
  border-radius: 999px;
  background: var(--border);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 9px;
}

.badge.running {
  background: var(--gold);
  color: var(--bg-surface);
}

.controls {
  display: flex;
  gap: var(--space-1);
  margin-right: calc(var(--space-4) * -1);
}

.controls button {
  width: 40px;
  height: var(--titlebar-height);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 0;
  background: none;
  color: var(--text-muted);
  cursor: pointer;
}

.controls button:hover {
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.controls .close:hover {
  background: var(--danger);
  color: var(--text-primary);
}
</style>
