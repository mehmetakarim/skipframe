<script setup lang="ts">
/**
 * A colour swatch and its hex, opening SkipFrame's own picker.
 *
 * It used to open the operating system's colour dialog, which meant a Windows dialog on Windows
 * and a Mac one on macOS, neither of them knowing anything about filament or about the colours
 * this app keeps. The picker is a slot-through, so a caller can put its own control inside —
 * the surface texture, for a single filament.
 */
import { computed, ref, useTemplateRef } from 'vue';

import SfColorPicker from './SfColorPicker.vue';

const props = withDefaults(
  defineProps<{ label?: string; disabled?: boolean; pickerTitle?: string }>(),
  { disabled: false },
);

const model = defineModel<string>({ required: true });

const hex = computed(() => model.value.toUpperCase());
const open = ref(false);
const fieldRef = useTemplateRef<HTMLButtonElement>('field');

function toggle() {
  if (!props.disabled) open.value = !open.value;
}
</script>

<template>
  <div :class="['sf-color-field', { disabled }]">
    <span v-if="label" class="t-overline">{{ label }}</span>

    <button
      ref="field"
      class="field"
      type="button"
      :disabled="disabled"
      :aria-label="label ? `${label}: ${hex}` : hex"
      :aria-expanded="open"
      aria-haspopup="dialog"
      @click="toggle"
    >
      <span class="swatch" :style="{ background: model }" />
      <span class="hex">{{ hex }}</span>
    </button>

    <SfColorPicker
      v-if="open"
      v-model="model"
      :anchor="fieldRef"
      :title="pickerTitle ?? label ?? 'Renk'"
      @close="open = false"
    >
      <slot />
    </SfColorPicker>
  </div>
</template>

<style scoped>
.sf-color-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.field {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 11px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  cursor: pointer;
  text-align: left;
  transition:
    border-color 90ms ease,
    background 90ms ease;
}

.field:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--bg-raised);
}

.field[aria-expanded='true'] {
  border-color: var(--gold);
}

.field:focus-visible {
  outline: none;
  border-color: var(--gold);
  box-shadow: var(--focus-ring);
}

.disabled .field {
  cursor: not-allowed;
  border-color: var(--border);
  background: var(--bg-base);
}

.swatch {
  flex: none;
  width: 16px;
  height: 16px;
  border-radius: var(--radius-xs);
  /* A ring, so a swatch the colour of the field is still visible. */
  box-shadow: inset 0 0 0 1px var(--border-strong);
}

.hex {
  flex: 1;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-primary);
}

.disabled .hex {
  color: var(--border-strong);
}
</style>
