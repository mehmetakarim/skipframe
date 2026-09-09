<script setup lang="ts">
/**
 * The eight numbered scene sections.
 *
 * Sections 01 and 02 carry real controls in this build. 03 to 08 are present, numbered and
 * collapsible so the sequence reads correctly, but their bodies land with the scene work — the
 * design's own ordering is the specification for what goes in each.
 */
import SfSection from '../ui/SfSection.vue';
import SfSlider from '../ui/SfSlider.vue';
import SfSwitch from '../ui/SfSwitch.vue';
import SfSelect from '../ui/SfSelect.vue';
import SfColorDot from '../ui/SfColorDot.vue';
import { ir, project } from '../../stores/project';
import { SURFACES, markPresetDirty, scene, type SectionKey } from '../../stores/scene';
import { integer, megabytes } from '../../lib/format';

const PENDING: { key: SectionKey; index: string; title: string }[] = [
  { key: 'bed', index: '03', title: 'Yapı tablası' },
  { key: 'background', index: '04', title: 'Arka plan' },
  { key: 'light', index: '05', title: 'Işık' },
  { key: 'camera', index: '06', title: 'Kamera' },
  { key: 'motion', index: '07', title: 'Hareket' },
  { key: 'timing', index: '08', title: 'Zamanlama' },
];
</script>

<template>
  <aside class="rail">
    <SfSection v-model:open="scene.sections.source" index="01" title="Kaynak">
      <div class="file-card">
        <span class="file-name">{{ ir?.meta.sourceName ?? '—' }}</span>
        <span class="file-meta">
          {{ project.bytes ? megabytes(project.bytes) + ' · ' : ''
          }}{{ integer(ir?.layerCount ?? 0) }} katman
        </span>
      </div>

      <SfSlider
        v-model="scene.layerSkip"
        label="Katman atlama"
        :min="1"
        :max="10"
        :step="1"
        @update:model-value="markPresetDirty"
      />

      <SfSwitch
        v-model="scene.hideTravel"
        label="Hareketleri gizle"
        @update:model-value="markPresetDirty"
      />
    </SfSection>

    <SfSection v-model:open="scene.sections.filament" index="02" title="Filament">
      <div class="swatches">
        <SfColorDot
          v-for="(colour, i) in scene.filaments"
          :key="colour + i"
          :color="colour"
          :selected="scene.filamentIndex === i"
          @click="((scene.filamentIndex = i), markPresetDirty())"
        />
        <SfColorDot add />
      </div>

      <SfSelect
        v-model="scene.surface"
        label="Yüzey"
        :options="SURFACES"
        @update:model-value="markPresetDirty"
      />
    </SfSection>

    <SfSection
      v-for="section in PENDING"
      :key="section.key"
      v-model:open="scene.sections[section.key]"
      :index="section.index"
      :title="section.title"
    >
      <p class="pending t-label">Bu bölüm sahne çalışmasıyla birlikte gelecek.</p>
    </SfSection>
  </aside>
</template>

<style scoped>
.rail {
  width: var(--rail-left-width);
  border-right: 1px solid var(--border);
  background: var(--bg-surface);
  overflow-y: auto;
  overflow-x: hidden;
}

.file-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  min-width: 0;
}

.file-name {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-meta {
  font-family: var(--font-mono);
  font-size: var(--type-overline-size);
  color: var(--text-faint);
}

.swatches {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.pending {
  margin: 0;
  line-height: 1.6;
  color: var(--text-faint);
}
</style>
