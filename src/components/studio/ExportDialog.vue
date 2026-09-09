<script setup lang="ts">
/**
 * The export panel: settings before a run, progress during one.
 *
 * It is the same modal in both states, because it is the same job — the settings collapse into a
 * summary and the buttons change. Closing it during a render does not stop the render.
 */
import { computed } from 'vue';

import SfModal from '../ui/SfModal.vue';
import SfButton from '../ui/SfButton.vue';
import SfTabs from '../ui/SfTabs.vue';
import SfSlider from '../ui/SfSlider.vue';
import SfSwitch from '../ui/SfSwitch.vue';
import {
  cancelExport,
  closeExportDialog,
  estimatedBytes,
  exportFrameCount,
  exportResolution,
  exportState,
  running,
  startExport,
} from '../../stores/exportJob';
import { ASPECTS } from '../../stores/scene';
import { STAGE_LABELS, type ExportFormat } from '../../export/types';
import { decimal, integer } from '../../lib/format';

const FORMATS: { value: ExportFormat; title: string; note: string }[] = [
  { value: 'mp4', title: 'MP4', note: 'H.264 · sosyal medya' },
  { value: 'frames', title: 'Kare sekansı', note: 'PNG · düzenleme için' },
];

const FPS_OPTIONS = [
  { value: '24', label: '24' },
  { value: '30', label: '30' },
  { value: '60', label: '60' },
];

const size = computed(() => `${exportResolution.value[0]} × ${exportResolution.value[1]}`);

const percent = computed(() => {
  const { frame, frameCount } = exportState.progress;
  return frameCount > 0 ? Math.round((frame / frameCount) * 100) : 0;
});

const megabytes = computed(() => `${decimal(estimatedBytes.value / 1e6, 0)} MB`);

function formatClock(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '—';
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

const fpsModel = computed({
  get: () => String(exportState.settings.fps),
  set: (v: string) => (exportState.settings.fps = Number(v)),
});

const finished = computed(() =>
  ['done', 'cancelled', 'failed'].includes(exportState.progress.stage),
);
</script>

<template>
  <SfModal
    :title="running ? 'Render sürüyor' : 'Dışa aktar'"
    :persistent="false"
    @close="closeExportDialog"
  >
    <template #status>
      <span v-if="running" class="t-overline running">Çalışıyor</span>
    </template>

    <!-- ---------------------------------------------------------------- settings -->
    <template v-if="!running && !finished">
      <section class="group">
        <span class="t-overline">Format</span>
        <div class="formats">
          <button
            v-for="format in FORMATS"
            :key="format.value"
            type="button"
            :class="['format', { selected: exportState.settings.format === format.value }]"
            :aria-pressed="exportState.settings.format === format.value"
            @click="exportState.settings.format = format.value"
          >
            <span class="format-title">{{ format.title }}</span>
            <span class="format-note">{{ format.note }}</span>
          </button>
        </div>
      </section>

      <div class="pair">
        <section class="group">
          <span class="t-overline">Çözünürlük</span>
          <div class="readout">{{ size }}</div>
        </section>

        <section class="group">
          <span class="t-overline">Oran</span>
          <SfTabs
            v-model="exportState.settings.aspect"
            class="wide"
            :options="ASPECTS.map((a) => ({ value: a.value, label: a.label }))"
            aria-label="Oran"
          />
        </section>
      </div>

      <section class="group">
        <div class="group-head">
          <span class="t-overline">Kare hızı</span>
          <span class="value">{{ exportState.settings.fps }} fps</span>
        </div>
        <SfTabs v-model="fpsModel" class="wide" :options="FPS_OPTIONS" aria-label="Kare hızı" />
      </section>

      <section class="group">
        <SfSlider
          v-model="exportState.settings.scale"
          label="Render ölçeği"
          :min="0.5"
          :max="2"
          :step="0.1"
          :format="(v) => `${decimal(v, 1)}×`"
        />
        <span class="hint">0,5× hızlı deneme · 2,0× yavaş ve keskin</span>
      </section>

      <section class="group bordered">
        <SfSwitch v-model="exportState.settings.openWhenDone" label="Bitince klasörü aç" />
      </section>
    </template>

    <!-- ---------------------------------------------------------------- progress -->
    <template v-else>
      <div class="progress">
        <div class="numbers">
          <span class="percent">{{ percent }}<span class="unit">%</span></span>
          <span class="frames">
            {{ integer(exportState.progress.frame) }} /
            {{ integer(exportState.progress.frameCount) }} kare
          </span>
        </div>
        <div class="bar"><div class="fill" :style="{ width: `${percent}%` }" /></div>
      </div>

      <dl class="rows">
        <div>
          <dt>Kalan süre</dt>
          <dd>≈ {{ formatClock(exportState.progress.etaS) }}</dd>
        </div>
        <div>
          <dt>Hız</dt>
          <dd>{{ decimal(exportState.progress.rate, 1) }} kare/s</dd>
        </div>
        <div>
          <dt>Aşama</dt>
          <dd class="word">{{ STAGE_LABELS[exportState.progress.stage] }}</dd>
        </div>
        <div>
          <dt>Çıktı</dt>
          <dd>
            {{ exportState.settings.format === 'mp4' ? 'MP4' : 'PNG' }} · {{ size
            }}{{
              exportState.progress.bytes
                ? ` · ${decimal(exportState.progress.bytes / 1e6, 1)} MB`
                : ''
            }}
          </dd>
        </div>
      </dl>

      <p v-if="exportState.progress.outputPath" class="path">
        {{ exportState.progress.outputPath }}
      </p>

      <p v-if="exportState.progress.error" class="error">{{ exportState.progress.error }}</p>
    </template>

    <!-- ---------------------------------------------------------------- footer -->
    <template #footer>
      <template v-if="running">
        <SfButton variant="outline" @click="cancelExport">İptal et</SfButton>
        <div class="spacer" />
        <span class="hint">Bu pencereyi kapatabilirsin, render arka planda sürer.</span>
      </template>

      <template v-else-if="finished">
        <SfButton variant="primary" @click="closeExportDialog">Kapat</SfButton>
        <div class="spacer" />
        <span class="hint">
          {{
            exportState.progress.stage === 'done'
              ? `${decimal(exportState.progress.bytes / 1e6, 1)} MB yazıldı.`
              : STAGE_LABELS[exportState.progress.stage]
          }}
        </span>
      </template>

      <template v-else>
        <SfButton variant="primary" @click="startExport">Render’ı başlat</SfButton>
        <SfButton variant="ghost" @click="closeExportDialog">Vazgeç</SfButton>
        <div class="spacer" />
        <span class="estimate"> ~ {{ integer(exportFrameCount) }} kare · ≈ {{ megabytes }} </span>
      </template>
    </template>
  </SfModal>
</template>

<style scoped>
.group {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.group-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.group.bordered {
  padding-top: var(--space-1);
  border-top: 1px solid var(--border);
}

.pair {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
}

.formats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.format {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 13px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.format:hover:not(.selected) {
  border-color: var(--border-strong);
  background: var(--bg-overlay);
}

.format.selected {
  border-color: var(--gold);
  background: var(--gold-tint);
}

.format-title {
  font-size: var(--type-body-size);
  font-weight: 700;
  color: var(--text-secondary);
}

.format.selected .format-title {
  color: var(--text-primary);
}

.format-note {
  font-size: 10.5px;
  color: var(--text-faint);
}

.format.selected .format-note {
  color: var(--text-muted);
}

.readout {
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-primary);
}

.value {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-primary);
}

.hint {
  font-size: 10.5px;
  color: var(--text-faint);
}

.wide {
  display: flex;
}

.wide :deep(.tab) {
  flex: 1;
  text-align: center;
}

/* -- progress ------------------------------------------------------------------------- */

.progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.numbers {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
}

.percent {
  font-family: var(--font-mono);
  font-size: 22px;
  font-weight: 700;
  color: var(--text-primary);
}

.unit {
  font-size: 14px;
  color: var(--text-faint);
}

.frames {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.bar {
  height: 5px;
  border-radius: 3px;
  background: var(--border);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--gold);
  transition: width 120ms linear;
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 7px;
  margin: 0;
}

.rows > div {
  display: flex;
  justify-content: space-between;
  gap: var(--space-3);
}

.rows dt {
  font-size: 12px;
  color: var(--text-muted);
}

.rows dd {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-primary);
}

.rows dd.word {
  font-family: var(--font-sans);
}

.path {
  margin: 0;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  font-family: var(--font-mono);
  font-size: 10.5px;
  line-height: 1.7;
  color: var(--text-faint);
  word-break: break-all;
}

.error {
  margin: 0;
  font-size: var(--type-label-size);
  line-height: 1.6;
  color: var(--danger);
}

.running {
  color: var(--gold);
}

.spacer {
  flex: 1;
}

.estimate {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-faint);
}
</style>
