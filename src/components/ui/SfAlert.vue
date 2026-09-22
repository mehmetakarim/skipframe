<script setup lang="ts">
/**
 * FRAME 10's error and warning window, 520 px, mounted once for the whole app.
 *
 * No header bar and no close button, unlike `SfModal`: the design draws these as a single card
 * whose buttons are the way out. Escape and a click outside still dismiss it — an alert that
 * can only be left through one of its actions would be a trap.
 */
import { nextTick, onBeforeUnmount, ref, useTemplateRef, watch } from 'vue';

import { alerts, closeAlert, type AlertAction } from '../../stores/alerts';

const panelRef = useTemplateRef<HTMLDivElement>('panel');
/** The action currently showing its `doneLabel`. */
const done = ref<AlertAction | null>(null);
let doneTimer: ReturnType<typeof setTimeout> | undefined;

async function act(action: AlertAction) {
  if (action.doneLabel) {
    await action.run();
    done.value = action;
    clearTimeout(doneTimer);
    doneTimer = setTimeout(() => (done.value = null), 1800);
    return;
  }
  closeAlert();
  await action.run();
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && alerts.current) closeAlert();
}

watch(
  () => alerts.current,
  async (alert) => {
    done.value = null;
    if (alert) {
      window.addEventListener('keydown', onKey);
      await nextTick();
      panelRef.value?.focus();
    } else {
      window.removeEventListener('keydown', onKey);
    }
  },
);

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  clearTimeout(doneTimer);
});
</script>

<template>
  <div v-if="alerts.current" class="backdrop" @mousedown.self="closeAlert">
    <div
      ref="panel"
      :class="['panel', alerts.current.kind]"
      :role="alerts.current.kind === 'error' ? 'alertdialog' : 'dialog'"
      aria-modal="true"
      :aria-label="alerts.current.title"
      tabindex="-1"
    >
      <div class="content">
        <div class="badge" aria-hidden="true">
          <svg
            v-if="alerts.current.kind === 'error'"
            width="12"
            height="12"
            viewBox="0 0 12 12"
            stroke="currentColor"
            stroke-width="1.6"
            fill="none"
          >
            <path d="M1.5 1.5l9 9M10.5 1.5l-9 9" />
          </svg>
          <svg v-else width="4" height="14" viewBox="0 0 4 14" fill="currentColor">
            <rect width="4" height="9" rx="2" />
            <rect y="11" width="4" height="3" rx="1.5" />
          </svg>
        </div>

        <div class="text">
          <span class="title">{{ alerts.current.title }}</span>
          <span class="body">{{ alerts.current.body }}</span>
          <span
            v-if="alerts.current.chip"
            :class="['chip', alerts.current.chipFont === 'sans' ? 'sans' : 'mono']"
          >
            {{ alerts.current.chip }}
          </span>
          <div v-if="alerts.current.figures" class="figures">
            <span v-for="figure in alerts.current.figures" :key="figure.label">
              {{ figure.label }} <b>{{ figure.value }}</b>
            </span>
          </div>
        </div>
      </div>

      <footer class="foot">
        <button type="button" class="primary" @click="act(alerts.current.primary)">
          {{ alerts.current.primary.label }}
        </button>
        <button
          v-if="alerts.current.secondary"
          type="button"
          class="secondary"
          @click="act(alerts.current.secondary)"
        >
          {{
            done === alerts.current.secondary
              ? alerts.current.secondary.doneLabel
              : alerts.current.secondary.label
          }}
        </button>
        <template v-else-if="alerts.current.hint">
          <span class="spacer" />
          <span class="hint">{{ alerts.current.hint }}</span>
        </template>
      </footer>
    </div>
  </div>
</template>

<style scoped>
/* Above the export panel, which is where the disk space check is made from; below notices. */
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 45;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgb(0 0 0 / 0.55);
}

.panel {
  width: 520px;
  max-width: 100%;
  max-height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--bg-raised);
  overflow: hidden;
  outline: none;
}

.content {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  padding: var(--space-5) 22px;
  overflow-y: auto;
}

.badge {
  flex: none;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1.5px solid var(--border-strong);
  border-radius: 50%;
  color: var(--text-secondary);
}

.error .badge {
  border-color: var(--danger);
}

.text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.title {
  font-size: var(--type-heading-size);
  font-weight: 700;
  color: var(--text-primary);
}

.body {
  font-size: var(--type-body-size);
  line-height: 1.6;
  color: var(--text-muted);
}

.chip {
  margin-top: 3px;
  padding: var(--space-2) 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  font-size: 11px;
  color: var(--text-secondary);
  overflow-wrap: anywhere;
  user-select: text;
}

.chip.mono {
  font-family: var(--font-mono);
}

.chip.sans {
  font-family: var(--font-sans);
}

.figures {
  display: flex;
  gap: var(--space-5);
  margin-top: 3px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-faint);
}

.figures b {
  font-weight: 400;
  color: var(--text-primary);
}

.foot {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: var(--space-4) 22px;
  border-top: 1px solid var(--border);
  background: var(--bg-raised);
}

.primary,
.secondary {
  border-radius: var(--radius);
  font-family: var(--font-sans);
  font-size: var(--type-label-size);
  cursor: pointer;
  white-space: nowrap;
}

.primary {
  padding: 10px var(--space-4);
  border: 0;
  background: var(--gold);
  color: var(--bg-surface);
  font-weight: 700;
}

.primary:hover {
  background: var(--gold-hover);
}

.primary:active {
  background: var(--gold-pressed);
}

.secondary {
  padding: 9px 14px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-secondary);
}

.secondary:hover {
  border-color: var(--border-strong);
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.primary:focus-visible,
.secondary:focus-visible {
  outline: none;
  box-shadow: var(--focus-ring);
}

.spacer {
  flex: 1;
}

.hint {
  font-size: 12px;
  color: var(--text-faint);
}
</style>
