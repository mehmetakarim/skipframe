<script setup lang="ts">
/**
 * A two-column grid of exclusive choices, for a set small enough to show at once and worded too
 * long for a tab strip — the surface textures inside the colour picker.
 */
defineProps<{ label?: string; options: { value: string; label: string }[] }>();

const model = defineModel<string>({ required: true });
</script>

<template>
  <div class="sf-option-grid">
    <span v-if="label" class="t-overline">{{ label }}</span>
    <div class="grid" role="radiogroup" :aria-label="label">
      <button
        v-for="option in options"
        :key="option.value"
        type="button"
        role="radio"
        :aria-checked="model === option.value"
        :class="['option', { selected: model === option.value }]"
        @click="model = option.value"
      >
        {{ option.label }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.sf-option-grid {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-2);
}

.option {
  padding: 9px 11px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: transparent;
  color: var(--text-secondary);
  font-family: var(--font-sans);
  font-size: var(--type-label-size);
  text-align: left;
  cursor: pointer;
  transition:
    border-color 90ms ease,
    background 90ms ease,
    color 90ms ease;
}

.option:hover:not(.selected) {
  border-color: var(--border-strong);
  color: var(--text-primary);
}

.option.selected {
  border-color: var(--gold);
  background: var(--gold-tint);
  color: var(--text-primary);
}

.option:focus-visible {
  outline: none;
  box-shadow: var(--focus-ring);
}
</style>
