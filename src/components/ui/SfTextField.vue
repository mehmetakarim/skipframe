<script setup lang="ts">
/**
 * A text field, one line or several, with the same ground, edge and gold focus as the other
 * fields on the component sheet.
 *
 * The counter is shown when a limit is given. The limit is not enforced with `maxlength`: a
 * pasted description that runs over should be visibly over and trimmed by the person who wrote
 * it, not silently cut mid-sentence by the browser.
 */
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    label?: string;
    multiline?: boolean;
    rows?: number;
    limit?: number;
    placeholder?: string;
    disabled?: boolean;
    required?: boolean;
  }>(),
  { multiline: false, rows: 5, disabled: false, required: false },
);

const model = defineModel<string>({ required: true });

/** Characters as a person counts them — and as StepperSkip's `mb_strlen` does — not UTF-16 units. */
const length = computed(() => [...model.value].length);
const over = computed(() => props.limit !== undefined && length.value > props.limit);
const empty = computed(() => props.required && model.value.trim() === '');
</script>

<template>
  <div :class="['sf-text-field', { over, disabled }]">
    <span v-if="label" class="t-overline">{{ label }}</span>

    <textarea
      v-if="multiline"
      v-model="model"
      class="field"
      :rows="rows"
      :placeholder="placeholder"
      :disabled="disabled"
      :aria-label="label"
      :aria-invalid="over || empty"
    />
    <input
      v-else
      v-model="model"
      class="field"
      type="text"
      :placeholder="placeholder"
      :disabled="disabled"
      :aria-label="label"
      :aria-invalid="over || empty"
    />

    <span v-if="limit !== undefined" class="counter" aria-live="polite">
      {{ length }} / {{ limit }}
    </span>
  </div>
</template>

<style scoped>
.sf-text-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.field {
  width: 100%;
  padding: 11px 13px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: var(--type-body-size);
  line-height: 1.55;
  resize: none;
  transition:
    border-color 90ms ease,
    background 90ms ease;
}

.field::placeholder {
  color: var(--text-faint);
}

.field:hover:not(:disabled) {
  border-color: var(--border-strong);
}

.field:focus-visible {
  outline: none;
  border-color: var(--gold);
  box-shadow: var(--focus-ring);
}

.field:disabled {
  color: var(--text-faint);
  cursor: not-allowed;
}

.over .field {
  border-color: var(--danger);
}

.counter {
  align-self: flex-end;
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-faint);
}

.over .counter {
  color: var(--danger);
}
</style>
