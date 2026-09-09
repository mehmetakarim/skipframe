<script setup lang="ts">
/**
 * A colour swatch and its hex, opening the OS colour picker.
 *
 * The design's own picker (a popover with a guide palette, RGB fields and a surface texture
 * choice) is its own piece of work; this is the field it will eventually open from, and until
 * then the system picker does the job without pretending to be that panel.
 */
import { computed } from 'vue';

withDefaults(defineProps<{ label?: string; disabled?: boolean }>(), { disabled: false });

const model = defineModel<string>({ required: true });

const hex = computed(() => model.value.toUpperCase());
</script>

<template>
  <div :class="['sf-color-field', { disabled }]">
    <span v-if="label" class="t-overline">{{ label }}</span>

    <label class="field">
      <span class="swatch" :style="{ background: model }" />
      <span class="hex">{{ hex }}</span>
      <input v-model="model" type="color" :disabled="disabled" :aria-label="label" />
    </label>
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
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 11px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  cursor: pointer;
  transition:
    border-color 90ms ease,
    background 90ms ease;
}

.field:hover {
  border-color: var(--border-strong);
  background: var(--bg-raised);
}

.field:has(input:focus-visible) {
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

/* The native swatch is invisible; the styled row above is the control. */
input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  padding: 0;
  border: 0;
  opacity: 0;
  cursor: inherit;
}
</style>
