<script setup lang="ts">
/**
 * The segmented control: a bordered tray with a gold pill on the selected option. Used for
 * aspect ratio, frame rate and format — anything with two to four mutually exclusive choices.
 */
export interface SfTabOption {
  value: string;
  label: string;
}

withDefaults(
  defineProps<{
    options: SfTabOption[];
    /** Numbers and ratios are mono; word labels are not. */
    mono?: boolean;
    disabled?: boolean;
    ariaLabel?: string;
  }>(),
  { mono: true, disabled: false },
);

const model = defineModel<string>({ required: true });
</script>

<template>
  <div :class="['sf-tabs', { mono, disabled }]" role="tablist" :aria-label="ariaLabel">
    <button
      v-for="option in options"
      :key="option.value"
      class="tab"
      type="button"
      role="tab"
      :aria-selected="model === option.value"
      :class="{ selected: model === option.value }"
      :disabled="disabled"
      @click="model = option.value"
    >
      {{ option.label }}
    </button>
  </div>
</template>

<style scoped>
.sf-tabs {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-surface);
}

.tab {
  padding: 5px 11px;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-muted);
  font-size: 11px;
  cursor: pointer;
  transition:
    background 90ms ease,
    color 90ms ease;
}

.mono .tab {
  font-family: var(--font-mono);
}

.tab:hover:not(:disabled):not(.selected) {
  background: var(--bg-raised);
  color: var(--text-primary);
}

.tab:active:not(:disabled):not(.selected) {
  background: var(--bg-base);
}

.tab.selected {
  background: var(--gold);
  color: var(--bg-surface);
  font-weight: 700;
}

.tab.selected:hover:not(:disabled) {
  background: var(--gold-hover);
}

.tab:disabled {
  color: var(--border-strong);
  cursor: not-allowed;
}

.disabled .tab.selected {
  background: var(--border);
  color: var(--text-faint);
}
</style>
