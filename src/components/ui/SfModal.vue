<script setup lang="ts">
/**
 * A centred panel over a dimmed studio.
 *
 * Escape and a click on the backdrop close it, unless `persistent` is set — a running export
 * must not be dismissed by a stray click.
 */
import { onBeforeUnmount, onMounted, useTemplateRef } from 'vue';

const props = withDefaults(defineProps<{ title: string; width?: number; persistent?: boolean }>(), {
  width: 560,
  persistent: false,
});

const emit = defineEmits<{ close: [] }>();
const panelRef = useTemplateRef<HTMLDivElement>('panel');

function requestClose() {
  if (!props.persistent) emit('close');
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') requestClose();
}

onMounted(() => {
  window.addEventListener('keydown', onKey);
  panelRef.value?.focus();
});

onBeforeUnmount(() => window.removeEventListener('keydown', onKey));
</script>

<template>
  <div class="backdrop" @mousedown.self="requestClose">
    <div
      ref="panel"
      class="panel"
      role="dialog"
      aria-modal="true"
      :aria-label="title"
      tabindex="-1"
      :style="{ width: `${width}px` }"
    >
      <header class="head">
        <span class="title">{{ title }}</span>
        <slot name="status" />
        <button
          v-if="!persistent"
          class="close"
          type="button"
          aria-label="Kapat"
          @click="emit('close')"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" stroke-width="1.4" />
          </svg>
        </button>
      </header>

      <div class="body"><slot /></div>

      <footer v-if="$slots.footer" class="foot"><slot name="footer" /></footer>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgb(0 0 0 / 0.55);
}

.panel {
  max-width: 100%;
  max-height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--bg-raised);
  overflow: hidden;
}

.head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-5) 24px;
  border-bottom: 1px solid var(--border);
}

.title {
  flex: 1;
  font-size: var(--type-heading-size);
  font-weight: 700;
  color: var(--text-primary);
}

.close {
  display: flex;
  padding: var(--space-1);
  border: 0;
  border-radius: var(--radius-xs);
  background: none;
  color: var(--text-faint);
  cursor: pointer;
}

.close:hover {
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.body {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 22px;
  overflow-y: auto;
}

.foot {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 18px 24px;
  border-top: 1px solid var(--border);
  background: var(--bg-raised);
}
</style>
