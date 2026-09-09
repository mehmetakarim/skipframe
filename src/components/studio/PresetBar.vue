<script setup lang="ts">
/** Scene presets. One is always active; the active one is the only gold thing in this bar. */
import SfButton from '../ui/SfButton.vue';
import { PRESETS, scene, type PresetId } from '../../stores/scene';

function choose(id: PresetId) {
  scene.preset = id;
  scene.presetDirty = false;
}
</script>

<template>
  <div class="presetbar">
    <span class="t-overline">Sahne preset</span>

    <div class="list">
      <button
        v-for="preset in PRESETS"
        :key="preset.id"
        type="button"
        :class="['preset', { active: scene.preset === preset.id }]"
        :aria-pressed="scene.preset === preset.id"
        @click="choose(preset.id)"
      >
        {{ preset.label }}
      </button>
    </div>

    <div class="spacer" />

    <SfButton variant="ghost" :disabled="!scene.presetDirty">Preset kaydet</SfButton>
  </div>
</template>

<style scoped>
.presetbar {
  height: var(--presetbar-height);
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 var(--space-5);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border);
}

.t-overline {
  margin-right: var(--space-2);
}

.list {
  display: flex;
  gap: var(--space-2);
}

.preset {
  padding: 7px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--type-label-size);
  cursor: pointer;
  transition:
    background 90ms ease,
    border-color 90ms ease,
    color 90ms ease;
}

.preset:hover:not(.active) {
  border-color: var(--border-strong);
  background: var(--bg-raised);
  color: var(--text-primary);
}

.preset.active {
  border-color: var(--gold);
  background: var(--gold);
  color: var(--bg-surface);
  font-weight: 700;
}

.preset.active:hover {
  border-color: var(--gold-hover);
  background: var(--gold-hover);
}

.spacer {
  flex: 1;
}
</style>
