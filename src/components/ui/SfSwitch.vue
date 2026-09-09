<script setup lang="ts">
/** 34x19 track, 14px knob. Gold when on — one of the four places gold is allowed. */
withDefaults(defineProps<{ label?: string; disabled?: boolean }>(), { disabled: false });

const model = defineModel<boolean>({ required: true });
</script>

<template>
  <label :class="['sf-switch', { disabled }]">
    <span v-if="label" class="label">{{ label }}</span>
    <input v-model="model" type="checkbox" role="switch" :disabled="disabled" :aria-label="label" />
    <span class="track"><span class="knob" /></span>
  </label>
</template>

<style scoped>
.sf-switch {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  cursor: pointer;
}

.sf-switch.disabled {
  cursor: not-allowed;
}

.label {
  font-size: 12px;
  color: var(--text-muted);
}

.disabled .label {
  color: var(--border-strong);
}

input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}

.track {
  position: relative;
  flex: none;
  width: 34px;
  height: 19px;
  border-radius: 10px;
  background: var(--border);
  transition: background 110ms ease;
}

.knob {
  position: absolute;
  top: 2.5px;
  left: 2.5px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--text-muted);
  transition:
    transform 110ms ease,
    background 110ms ease;
}

.sf-switch:hover .track {
  background: var(--border-strong);
}

.sf-switch:hover .knob {
  background: var(--text-secondary);
}

input:checked ~ .track {
  background: var(--gold);
}

input:checked ~ .track .knob {
  transform: translateX(15px);
  background: var(--bg-surface);
}

.sf-switch:hover input:checked ~ .track {
  background: var(--gold-hover);
}

.sf-switch:active input:checked ~ .track {
  background: var(--gold-pressed);
}

input:focus-visible ~ .track {
  box-shadow: var(--focus-ring);
}

input:disabled ~ .track,
.disabled:hover .track {
  background: var(--border);
}

input:disabled ~ .track .knob,
.disabled:hover .knob {
  background: var(--border-strong);
}
</style>
