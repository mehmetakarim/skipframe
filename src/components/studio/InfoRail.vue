<script setup lang="ts">
/**
 * The read-only right rail: what the file says, what the selected layer contains, and how the
 * scene is currently set. Everything here comes from the parser's meta or from the IR itself —
 * nothing is entered by the user.
 */
import { computed } from 'vue';

import { ir } from '../../stores/project';
import { currentLayer, scene } from '../../stores/scene';
import { FEATURE_LABELS, FeatureType } from '../../ir/types';
import { boundsSize, integer, metres, millimetres, printDuration } from '../../lib/format';

const meta = computed(() => ir.value?.meta ?? null);

const layerHeight = computed(() => {
  const zs = meta.value?.layerZ ?? [];
  if (zs.length < 2) return null;
  return (zs[1] ?? 0) - (zs[0] ?? 0);
});

const layerZ = computed(() => meta.value?.layerZ[currentLayer.value] ?? null);

/** Segment counts and extruded length for the layer under the playhead. */
const layerStats = computed(() => {
  const model = ir.value;
  if (!model) return null;
  const from = model.layerStart[currentLayer.value] ?? 0;
  const to = model.layerStart[currentLayer.value + 1] ?? model.segmentCount;

  let moves = 0;
  let extrudedMm = 0;
  for (let s = from; s < to; s++) {
    moves += 1;
    if (model.featureType[s] === FeatureType.Travel) continue;
    const o = s * 6;
    const dx = (model.positions[o + 3] ?? 0) - (model.positions[o] ?? 0);
    const dy = (model.positions[o + 4] ?? 0) - (model.positions[o + 1] ?? 0);
    const dz = (model.positions[o + 5] ?? 0) - (model.positions[o + 2] ?? 0);
    extrudedMm += Math.sqrt(dx * dx + dy * dy + dz * dz);
  }
  return { moves, extrudedMm };
});

const cameraLabel = computed(
  () =>
    `Yörünge ${Math.round(scene.camera.azimuthDeg)}° · ${Math.round(scene.camera.elevationDeg)}°`,
);

const motionLabel = computed(() => {
  const { orbitDeg, riseDeg, zoomTo } = scene.motion;
  const parts: string[] = [];
  if (orbitDeg !== 0) parts.push(`dönüş ${Math.round(orbitDeg)}°`);
  if (riseDeg !== 0) parts.push(`yükselme ${Math.round(riseDeg)}°`);
  if (zoomTo !== 1) parts.push(`yakınlık ${zoomTo.toFixed(2)}×`);
  return parts.length ? parts.join(' · ') : 'Sabit';
});

const backgroundLabel = computed(() =>
  scene.background.style === 'gradient'
    ? `Geçişli ${scene.background.top.toUpperCase()}`
    : `Düz ${scene.background.top.toUpperCase()}`,
);

const plateLabel = computed(() => {
  const size = ir.value?.meta.bedSize;
  const styles: Record<string, string> = { grid: 'Izgara', solid: 'Düz', none: 'Yok' };
  const style = styles[scene.plate.style] ?? '—';
  return size ? `${style} · ${Math.round(size[0])}×${Math.round(size[1])}` : style;
});

const featureSummary = computed(() => {
  const model = ir.value;
  if (!model || !model.meta.hasFeatureTypes) return null;
  const seen = new Set<number>();
  const from = model.layerStart[currentLayer.value] ?? 0;
  const to = model.layerStart[currentLayer.value + 1] ?? model.segmentCount;
  for (let s = from; s < to; s++) {
    const f = model.featureType[s];
    if (f !== undefined && f !== FeatureType.Travel) seen.add(f);
  }
  return [...seen]
    .sort((a, b) => a - b)
    .map((f) => FEATURE_LABELS[f] ?? '—')
    .join(', ');
});
</script>

<template>
  <aside class="rail">
    <section class="group">
      <span class="t-overline">Dosya</span>
      <div class="row">
        <span class="key">Slicer</span>
        <span class="val word">{{ meta?.dialect ?? '—' }}</span>
      </div>
      <div class="row">
        <span class="key">Katman</span>
        <span class="val">{{ layerHeight ? millimetres(layerHeight) : '—' }}</span>
      </div>
      <div class="row">
        <span class="key">Boyut</span>
        <span class="val">{{ meta ? boundsSize(meta.bounds) : '—' }}</span>
      </div>
      <div class="row">
        <span class="key">Baskı süresi</span>
        <span class="val">{{
          meta?.estimatedTimeS ? printDuration(meta.estimatedTimeS) : '—'
        }}</span>
      </div>
    </section>

    <div class="divider" />

    <section class="group">
      <span class="t-overline">Seçili katman</span>
      <div class="row">
        <span class="key">Yükseklik</span>
        <span class="val">{{ layerZ !== null ? millimetres(layerZ) : '—' }}</span>
      </div>
      <div class="row">
        <span class="key">Hareket</span>
        <span class="val">{{ layerStats ? integer(layerStats.moves) : '—' }}</span>
      </div>
      <div class="row">
        <span class="key">Ekstrüzyon</span>
        <span class="val">{{ layerStats ? metres(layerStats.extrudedMm) : '—' }}</span>
      </div>
      <div v-if="featureSummary" class="row wrap">
        <span class="key">Bölümler</span>
        <span class="val word">{{ featureSummary }}</span>
      </div>
    </section>

    <div class="divider" />

    <section class="group">
      <span class="t-overline">Sahne</span>
      <div class="row">
        <span class="key">Yazıcı</span>
        <span class="val word">{{ meta?.printerModel ?? 'Jenerik tabla' }}</span>
      </div>
      <div class="row">
        <span class="key">Kamera</span>
        <span class="val word">{{ cameraLabel }}</span>
      </div>
      <div class="row">
        <span class="key">Hareket</span>
        <span class="val word">{{ motionLabel }}</span>
      </div>
      <div class="row">
        <span class="key">Arka plan</span>
        <span class="val word">{{ backgroundLabel }}</span>
      </div>
      <div class="row">
        <span class="key">Tabla</span>
        <span class="val word">{{ plateLabel }}</span>
      </div>
      <div class="row">
        <span class="key">Hareketler</span>
        <span class="val word">{{ scene.hideTravel ? 'Gizli' : 'Görünür' }}</span>
      </div>
    </section>

    <ul v-if="meta?.warnings.length" class="warnings">
      <li v-for="warning in meta.warnings" :key="warning">{{ warning }}</li>
    </ul>

    <div v-else-if="scene.presetDirty" class="note">Sahne değişiklikleri presete kaydedilmedi.</div>
  </aside>
</template>

<style scoped>
.rail {
  width: var(--rail-right-width);
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding: 18px var(--space-4);
  border-left: 1px solid var(--border);
  background: var(--bg-surface);
  overflow-y: auto;
}

.group {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.row {
  display: flex;
  justify-content: space-between;
  gap: var(--space-2);
}

.row.wrap {
  flex-direction: column;
  gap: var(--space-1);
}

.key {
  flex: none;
  font-size: 12px;
  color: var(--text-muted);
}

.val {
  font-family: var(--font-mono);
  font-size: var(--type-data-size);
  color: var(--text-primary);
  text-align: right;
  min-width: 0;
}

.val.word {
  font-family: var(--font-sans);
}

.row.wrap .val {
  text-align: left;
  line-height: 1.5;
}

.divider {
  height: 1px;
  background: var(--border);
}

.note,
.warnings {
  margin: auto 0 0;
  padding: 11px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  font-size: 12px;
  color: var(--text-faint);
  line-height: 1.55;
}

.warnings {
  padding-left: 26px;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
</style>
