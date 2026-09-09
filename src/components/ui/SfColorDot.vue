<script setup lang="ts">
/**
 * A 22px filament swatch. Selection is a gold ring held off the swatch by a gap in the panel
 * colour, so it reads on a light swatch and a dark one alike.
 *
 * `add` turns it into the dashed "+" that opens the colour picker.
 */
withDefaults(
  defineProps<{
    /** Any CSS colour. Ignored when `add` is set. */
    color?: string;
    selected?: boolean;
    add?: boolean;
    label?: string;
  }>(),
  { color: 'var(--text-secondary)', selected: false, add: false },
);
</script>

<template>
  <button
    :class="['sf-color-dot', { selected, add }]"
    type="button"
    :style="add ? undefined : { background: color }"
    :aria-label="label ?? (add ? 'Add a colour' : color)"
    :aria-pressed="add ? undefined : selected"
  >
    <span v-if="add" aria-hidden="true">+</span>
  </button>
</template>

<style scoped>
.sf-color-dot {
  flex: none;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  cursor: pointer;
  transition: box-shadow 90ms ease;
}

.sf-color-dot:hover:not(.selected) {
  box-shadow:
    0 0 0 2px var(--bg-surface),
    0 0 0 3.5px var(--border-strong);
}

.selected {
  box-shadow:
    0 0 0 2px var(--bg-surface),
    0 0 0 3.5px var(--gold);
}

.selected:hover {
  box-shadow:
    0 0 0 2px var(--bg-surface),
    0 0 0 3.5px var(--gold-hover);
}

.sf-color-dot:focus-visible {
  box-shadow: var(--focus-ring);
}

.add {
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--border-strong);
  background: transparent;
  color: var(--text-faint);
  font-size: 12px;
  line-height: 1;
}

.add:hover {
  border-color: var(--text-muted);
  color: var(--text-secondary);
  box-shadow: none;
}
</style>
