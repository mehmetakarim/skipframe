<script setup lang="ts">
/**
 * A 3px rail with a gold fill and a 12px knob, per the design. Built on a native
 * `input[type=range]` so keyboard, screen readers and drag behaviour come for free — the
 * design's five states are painted onto it rather than reimplemented.
 */
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    /** Optional label shown above the rail; the formatted value sits opposite it. */
    label?: string;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    /** How the current value should read. Defaults to the raw number. */
    format?: (value: number) => string;
  }>(),
  { min: 0, max: 100, step: 1, disabled: false },
);

const model = defineModel<number>({ required: true });

const percent = computed(() => {
  const span = props.max - props.min;
  if (span <= 0) return 0;
  return ((model.value - props.min) / span) * 100;
});

const display = computed(() => (props.format ? props.format(model.value) : String(model.value)));
</script>

<template>
  <div :class="['sf-slider', { disabled }]">
    <div v-if="label" class="head">
      <span class="label">{{ label }}</span>
      <span class="value">{{ display }}</span>
    </div>

    <div class="rail" :style="{ '--fill': `${percent}%` }">
      <input
        v-model.number="model"
        type="range"
        :min="min"
        :max="max"
        :step="step"
        :disabled="disabled"
        :aria-label="label"
      />
    </div>

    <span v-if="!label" class="value standalone">{{ display }}</span>
  </div>
</template>

<style scoped>
.sf-slider {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.label {
  font-size: 12px;
  color: var(--text-muted);
}

.value {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-primary);
}

.value.standalone {
  font-size: var(--type-overline-size);
  color: var(--text-faint);
}

.rail {
  position: relative;
  height: 3px;
  border-radius: 2px;
  background: var(--border);
}

/* The filled portion is drawn by the rail itself so the native input stays a bare hit target. */
.rail::before {
  content: '';
  position: absolute;
  inset: 0 auto 0 0;
  width: var(--fill);
  border-radius: 2px;
  background: var(--gold);
  transition: background 90ms ease;
}

.rail:hover::before {
  background: var(--gold-hover);
}

.rail:active::before {
  background: var(--gold-pressed);
}

.disabled .rail::before {
  background: var(--border);
}

input {
  position: absolute;
  inset: -8px 0;
  width: 100%;
  margin: 0;
  background: transparent;
  appearance: none;
  cursor: pointer;
}

input:disabled {
  cursor: not-allowed;
}

input::-webkit-slider-thumb {
  appearance: none;
  width: 12px;
  height: 12px;
  border: 0;
  border-radius: 50%;
  background: var(--text-primary);
  cursor: grab;
  transition:
    width 90ms ease,
    height 90ms ease;
}

.rail:hover input::-webkit-slider-thumb {
  width: 15px;
  height: 15px;
}

input:active::-webkit-slider-thumb {
  cursor: grabbing;
  box-shadow: 0 0 0 4px var(--gold-tint);
}

input:focus-visible::-webkit-slider-thumb {
  box-shadow: var(--focus-ring);
}

input:disabled::-webkit-slider-thumb {
  background: var(--border-strong);
  width: 12px;
  height: 12px;
  box-shadow: none;
}

/* Focus lands on the thumb, not on the whole control. */
input:focus-visible {
  box-shadow: none;
}

.disabled .label,
.disabled .value {
  color: var(--border-strong);
}
</style>
