<script setup lang="ts">
/**
 * A native `select` wearing the design's field styling: `--bg-base` ground, `--border` edge,
 * gold border while open. Native keeps keyboard handling, and the OS popup is the one place
 * the app cannot restyle anyway.
 */
export interface SfSelectOption {
  value: string;
  label: string;
}

withDefaults(
  defineProps<{
    options: SfSelectOption[];
    label?: string;
    disabled?: boolean;
  }>(),
  { disabled: false },
);

const model = defineModel<string>({ required: true });
</script>

<template>
  <div :class="['sf-select', { disabled }]">
    <span v-if="label" class="t-overline">{{ label }}</span>

    <div class="field">
      <select v-model="model" :disabled="disabled" :aria-label="label">
        <option v-for="option in options" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>

      <svg
        class="chevron"
        width="10"
        height="10"
        viewBox="0 0 12 12"
        fill="none"
        aria-hidden="true"
      >
        <path d="M2 4.5 6 8.5 10 4.5" stroke="currentColor" stroke-width="1.6" />
      </svg>
    </div>
  </div>
</template>

<style scoped>
.sf-select {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.field {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  color: var(--text-faint);
  transition:
    border-color 90ms ease,
    background 90ms ease;
}

.field:hover {
  border-color: var(--border-strong);
  background: var(--bg-raised);
  color: var(--text-secondary);
}

.field:has(select:focus-visible) {
  border-color: var(--gold);
  box-shadow: var(--focus-ring);
}

.disabled .field,
.disabled .field:hover {
  border-color: var(--border);
  background: var(--bg-base);
  color: var(--border-strong);
}

select {
  flex: 1;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-size: var(--type-label-size);
  appearance: none;
  cursor: pointer;
  /* The chevron is ours; the field is the click target. */
  outline: none;
}

select:disabled {
  color: var(--border-strong);
  cursor: not-allowed;
}

/* The popup is drawn by the OS and inherits nothing, so its two colours are set here. */
option {
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.chevron {
  flex: none;
  display: block;
  pointer-events: none;
}
</style>
