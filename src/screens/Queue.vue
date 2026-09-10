<script setup lang="ts">
/**
 * The batch queue.
 *
 * One toolbar of settings that apply to every job, a table of jobs, and a summary. Jobs run
 * sequentially into a renderer that belongs to the queue rather than to this component, so
 * walking back to the studio does not stop the run.
 */
import { computed, onMounted } from 'vue';

import TitleBar from '../components/studio/TitleBar.vue';
import SfButton from '../components/ui/SfButton.vue';
import SfSelect from '../components/ui/SfSelect.vue';
import SfTabs from '../components/ui/SfTabs.vue';
import SfSlider from '../components/ui/SfSlider.vue';
import {
  STATUS_LABELS,
  cancelQueue,
  clearFinished,
  defaultOutputDir,
  doneCount,
  failedCount,
  frameCountFor,
  pendingCount,
  pickAndAddFiles,
  pickOutputDir,
  queue,
  removeJob,
  resolutionFor,
  startQueue,
  totalEtaS,
  warningCount,
  type QueueJob,
} from '../stores/queue';
import { ASPECTS, PRESETS } from '../stores/scene';
import { revealFile } from '../export/writeFile';
import { integer, megabytes, shortPath } from '../lib/format';

const FPS_OPTIONS = [
  { value: '24', label: '24' },
  { value: '30', label: '30' },
  { value: '60', label: '60' },
];

const FORMAT_OPTIONS = [
  { value: 'mp4', label: 'MP4' },
  { value: 'frames', label: 'PNG' },
];

const fpsModel = computed({
  get: () => String(queue.settings.fps),
  set: (v: string) => (queue.settings.fps = Number(v)),
});

const canStart = computed(() => pendingCount.value > 0 && !queue.running);

const headline = computed(() => {
  const total = queue.jobs.length;
  if (total === 0) return 'Kuyruk boş';
  return queue.running ? `${total} iş · 1 çalışıyor` : `${total} iş`;
});

const summary = computed(() => {
  const parts: string[] = [];
  if (doneCount.value) parts.push(`${doneCount.value} bitti`);
  if (pendingCount.value) parts.push(`${pendingCount.value} sırada`);
  if (warningCount.value) parts.push(`${warningCount.value} uyarı`);
  if (failedCount.value) parts.push(`${failedCount.value} hata`);
  return parts.join(' · ');
});

function clock(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '—';
  const m = Math.floor(Math.max(0, seconds) / 60);
  const s = Math.round(Math.max(0, seconds) % 60);
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

/** The line under the progress bar says something different in each state. */
function progressNote(job: QueueJob): string {
  if (job.status === 'done') {
    return `${integer(job.frameCount)} / ${integer(job.frameCount)} · ${clock(job.tookS)}'de bitti`;
  }
  if (job.status === 'failed') return job.error ?? 'Hata';
  if (job.status === 'rendering') {
    return `${integer(job.frame)} / ${integer(job.frameCount)} · ≈ ${clock(job.etaS)}`;
  }
  if (job.status === 'parsing') return 'Dosya okunuyor';
  if (job.status === 'cancelled') return 'İptal edildi';
  return job.warnings.length ? 'yine de render edilecek' : 'bekliyor';
}

function progressPercent(job: QueueJob): number {
  if (job.status === 'done') return 100;
  if (job.frameCount <= 0) return 0;
  return Math.round((job.frame / job.frameCount) * 100);
}

/** The second line of the file cell: what we know about the file, or why it is a problem. */
function fileNote(job: QueueJob): string {
  if (job.status === 'failed') return job.error ?? 'Okunamadı';
  if (job.warnings.length) return `${job.dialect ?? 'Bilinmeyen'} · ${job.warnings.length} uyarı`;
  const bits: string[] = [];
  if (job.layers !== null) bits.push(`${integer(job.layers)} katman`);
  if (job.bytes !== null) bits.push(megabytes(job.bytes));
  return bits.join(' · ') || 'okunuyor…';
}

function statusClass(job: QueueJob): string {
  if (job.status === 'rendering' || job.status === 'parsing') return 'active';
  if (job.status === 'failed') return 'failed';
  if (job.status === 'pending' && job.warnings.length) return 'warned';
  return '';
}

function statusText(job: QueueJob): string {
  if (job.status === 'pending' && job.warnings.length) return 'Uyarı';
  return STATUS_LABELS[job.status];
}

async function openOutputFolder() {
  const dir = queue.outputDir ?? (await defaultOutputDir());
  await revealFile(dir).catch(() => {});
}

onMounted(async () => {
  queue.outputDir ??= await defaultOutputDir();
});
</script>

<template>
  <div class="screen">
    <TitleBar>
      <template #trailing>
        <span class="count">{{ headline }}</span>
      </template>
    </TitleBar>

    <!-- toolbar ------------------------------------------------------------------------ -->
    <div class="toolbar">
      <SfButton v-if="!queue.running" variant="primary" :disabled="!canStart" @click="startQueue">
        Kuyruğu başlat
      </SfButton>
      <SfButton v-else variant="outline" @click="cancelQueue">Kuyruğu durdur</SfButton>

      <SfButton variant="outline" :disabled="queue.running" @click="pickAndAddFiles">
        Dosya ekle
      </SfButton>

      <div class="divider" />

      <span class="t-overline">Tümüne preset</span>

      <SfSelect
        v-model="queue.settings.preset"
        class="preset"
        :options="PRESETS.map((p) => ({ value: p.id, label: p.label }))"
        :disabled="queue.running"
      />

      <SfTabs
        v-model="queue.settings.aspect"
        :options="ASPECTS.map((a) => ({ value: a.value, label: a.label }))"
        :disabled="queue.running"
        aria-label="Oran"
      />

      <SfTabs
        v-model="fpsModel"
        :options="FPS_OPTIONS"
        :disabled="queue.running"
        aria-label="Kare hızı"
      />

      <SfTabs
        v-model="queue.settings.format"
        :options="FORMAT_OPTIONS"
        :mono="false"
        :disabled="queue.running"
        aria-label="Format"
      />

      <div class="spacer" />

      <SfSlider
        v-model="queue.settings.durationS"
        class="duration"
        label="Süre"
        :min="2"
        :max="60"
        :step="1"
        :disabled="queue.running"
        :format="(v) => `${integer(v)} sn`"
      />
    </div>

    <!-- table -------------------------------------------------------------------------- -->
    <div class="table">
      <div class="row head">
        <span>#</span>
        <span>Dosya</span>
        <span>Durum</span>
        <span>İlerleme</span>
        <span>Çıktı yolu</span>
        <span />
      </div>

      <div v-if="queue.jobs.length === 0" class="empty">
        <p>Kuyruk boş.</p>
        <p class="dim">
          Buraya birden çok G-code ekleyip hepsini aynı sahneyle arka arkaya render edebilirsin.
        </p>
        <SfButton variant="primary" @click="pickAndAddFiles">Dosya ekle</SfButton>
      </div>

      <div
        v-for="(job, i) in queue.jobs"
        :key="job.id"
        class="row"
        :class="{ current: job.status === 'rendering' || job.status === 'parsing' }"
      >
        <span class="index" :class="statusClass(job)">{{ String(i + 1).padStart(2, '0') }}</span>

        <div class="file">
          <span class="file-name">{{ job.name }}</span>
          <span class="file-note">{{ fileNote(job) }}</span>
        </div>

        <span class="status" :class="statusClass(job)">{{ statusText(job) }}</span>

        <div class="progress">
          <div class="bar">
            <div
              class="fill"
              :class="{ finished: job.status === 'done' }"
              :style="{ width: `${progressPercent(job)}%` }"
            />
          </div>
          <span class="note">{{ progressNote(job) }}</span>
        </div>

        <span class="path" :title="job.outputPath ?? ''">
          {{ job.outputPath ? shortPath(job.outputPath) : '—' }}
        </span>

        <button
          class="remove"
          type="button"
          aria-label="Kuyruktan çıkar"
          :disabled="job.status === 'rendering'"
          @click="removeJob(job.id)"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" stroke-width="1.4" />
          </svg>
        </button>
      </div>
    </div>

    <!-- footer ------------------------------------------------------------------------- -->
    <footer class="foot">
      <span v-if="queue.running" class="total">Toplam ≈ {{ clock(totalEtaS) }} kaldı</span>
      <span v-else class="total">
        {{ integer(frameCountFor) }} kare · {{ resolutionFor[0] }} × {{ resolutionFor[1] }}
      </span>
      <span class="summary">{{ summary }}</span>

      <div class="spacer" />

      <span class="dir" :title="queue.outputDir ?? ''">
        {{ queue.outputDir ? shortPath(queue.outputDir) : '…' }}
      </span>
      <SfButton variant="outline" :disabled="queue.running" @click="pickOutputDir">
        Klasörü değiştir
      </SfButton>
      <SfButton variant="outline" :disabled="!doneCount" @click="clearFinished">
        Bitenleri temizle
      </SfButton>
      <SfButton variant="outline" @click="openOutputFolder">Klasörü aç</SfButton>
    </footer>
  </div>
</template>

<style scoped>
.screen {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-surface);
}

.count {
  font-size: 11px;
  color: var(--text-faint);
}

/* -- toolbar ------------------------------------------------------------------------- */

.toolbar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: var(--space-3) 24px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-surface);
  flex-wrap: wrap;
}

.divider {
  width: 1px;
  height: 22px;
  background: var(--border);
}

.preset {
  min-width: 150px;
}

.duration {
  width: 160px;
}

.spacer {
  flex: 1;
}

/* -- table --------------------------------------------------------------------------- */

.table {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.row {
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr) 110px 210px minmax(0, 1fr) 40px;
  gap: var(--space-4);
  align-items: center;
  padding: var(--space-4) 24px;
  border-bottom: 1px solid var(--border);
}

.row.current {
  background: var(--bg-raised);
}

.row.head {
  padding: var(--space-3) 24px;
  font-family: var(--font-mono);
  font-size: var(--type-overline-size);
  letter-spacing: 0.12em;
  color: var(--text-faint);
  text-transform: uppercase;
  position: sticky;
  top: 0;
  background: var(--bg-surface);
  z-index: 1;
}

.index {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-faint);
}

.index.active {
  color: var(--gold);
}

.file {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.file-name {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-note {
  font-family: var(--font-mono);
  font-size: var(--type-overline-size);
  color: var(--text-faint);
}

.status {
  font-size: 11px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.status.active {
  color: var(--gold);
}

.status.failed {
  color: var(--danger);
}

.status.warned {
  color: var(--text-secondary);
}

.progress {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bar {
  height: 4px;
  border-radius: 2px;
  background: var(--border);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--gold);
  transition: width 140ms linear;
}

.fill.finished {
  background: var(--border-strong);
}

.progress .note {
  font-family: var(--font-mono);
  font-size: var(--type-overline-size);
  color: var(--text-muted);
}

.path {
  font-family: var(--font-mono);
  font-size: var(--type-path-size);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remove {
  display: flex;
  padding: var(--space-1);
  border: 0;
  border-radius: var(--radius-xs);
  background: none;
  color: var(--text-faint);
  cursor: pointer;
}

.remove:hover:not(:disabled) {
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.remove:disabled {
  color: var(--border-strong);
  cursor: not-allowed;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: 80px 24px;
  text-align: center;
}

.empty p {
  margin: 0;
  font-size: 14px;
  color: var(--text-secondary);
}

.empty .dim {
  max-width: 46ch;
  font-size: var(--type-label-size);
  line-height: 1.6;
  color: var(--text-muted);
}

/* -- footer -------------------------------------------------------------------------- */

.foot {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--space-5);
  padding: 0 24px;
  height: 64px;
  border-top: 1px solid var(--border);
  background: var(--bg-raised);
}

.total {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.summary {
  font-size: 11.5px;
  color: var(--text-faint);
}

.dir {
  max-width: 280px;
  font-family: var(--font-mono);
  font-size: var(--type-path-size);
  color: var(--text-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
