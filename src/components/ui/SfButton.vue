<script setup lang="ts">
/**
 * Three variants, five states each, exactly as the design's component sheet specifies.
 *
 * Gold is reserved: it appears on the primary action and nowhere else in this component.
 */
withDefaults(
  defineProps<{
    variant?: 'primary' | 'outline' | 'ghost';
    disabled?: boolean;
    type?: 'button' | 'submit';
  }>(),
  { variant: 'outline', disabled: false, type: 'button' },
);
</script>

<template>
  <button :class="['sf-button', variant]" :type="type" :disabled="disabled">
    <slot />
  </button>
</template>

<style scoped>
.sf-button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  border: 1px solid transparent;
  border-radius: var(--radius);
  background: transparent;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 90ms ease,
    border-color 90ms ease,
    color 90ms ease;
}

.sf-button:disabled {
  cursor: not-allowed;
}

/* -- primary -------------------------------------------------------------------------- */

.primary {
  padding: 11px 20px;
  font-size: var(--type-body-size);
  font-weight: 700;
  background: var(--gold);
  color: var(--bg-surface);
}

.primary:hover:not(:disabled) {
  background: var(--gold-hover);
}

.primary:active:not(:disabled) {
  background: var(--gold-pressed);
  transform: translateY(1px);
}

.primary:disabled {
  background: var(--border);
  color: var(--text-faint);
}

/* -- outline -------------------------------------------------------------------------- */

.outline {
  padding: 10px 16px;
  font-size: var(--type-label-size);
  border-color: var(--border);
  color: var(--text-secondary);
}

.outline:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--bg-raised);
  color: var(--text-primary);
}

.outline:active:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--bg-base);
  color: var(--text-primary);
  transform: translateY(1px);
}

.outline:disabled {
  color: var(--border-strong);
}

/* -- ghost ---------------------------------------------------------------------------- */

.ghost {
  padding: 10px 12px;
  font-size: var(--type-label-size);
  color: var(--text-muted);
}

.ghost:hover:not(:disabled) {
  background: var(--bg-raised);
  color: var(--text-primary);
}

.ghost:active:not(:disabled) {
  background: var(--bg-base);
  color: var(--text-primary);
  transform: translateY(1px);
}

.ghost:disabled {
  color: var(--border-strong);
}
</style>
