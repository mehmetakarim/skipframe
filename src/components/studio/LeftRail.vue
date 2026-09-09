<script setup lang="ts">
/**
 * The eight numbered scene sections.
 *
 * All eight drive the render. Light and surface finish became possible once the print stopped
 * being lines and started being extrusion beads with real normals.
 */
import { computed } from 'vue';

import SfSection from '../ui/SfSection.vue';
import SfSlider from '../ui/SfSlider.vue';
import SfSwitch from '../ui/SfSwitch.vue';
import SfSelect from '../ui/SfSelect.vue';
import SfTabs from '../ui/SfTabs.vue';
import SfColorDot from '../ui/SfColorDot.vue';
import SfColorField from '../ui/SfColorField.vue';
import SfButton from '../ui/SfButton.vue';
import { ir, project } from '../../stores/project';
import {
  BACKGROUND_STYLES,
  EASINGS,
  PLATE_STYLES,
  SURFACES,
  markPresetDirty,
  scene,
} from '../../stores/scene';
import { decimal, integer, megabytes } from '../../lib/format';

const FPS_OPTIONS = [
  { value: '24', label: '24' },
  { value: '30', label: '30' },
  { value: '60', label: '60' },
];

const fpsModel = computed({
  get: () => String(scene.fps),
  set: (v: string) => {
    scene.fps = Number(v);
    markPresetDirty();
  },
});

/** Custom filament colours are appended, so the guide swatches stay where the eye left them. */
const customColour = computed({
  get: () => scene.filaments[scene.filamentIndex] ?? '#c9ccc6',
  set: (v: string) => {
    scene.filaments[scene.filamentIndex] = v;
    markPresetDirty();
  },
});

const bedLabel = computed(() => {
  const size = ir.value?.meta.bedSize;
  return size ? `${integer(size[0])} × ${integer(size[1])} mm` : 'Bilinmiyor';
});

function touched() {
  markPresetDirty();
}

function fitCamera() {
  scene.camera.zoom = 1;
  touched();
}
</script>

<template>
  <aside class="rail">
    <!-- 01 --------------------------------------------------------------------------- -->
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
        :max="20"
        :step="1"
        @update:model-value="touched"
      />

      <SfSwitch
        v-model="scene.hideTravel"
        label="Hareketleri gizle"
        @update:model-value="touched"
      />
      <SfSwitch
        v-model="scene.highlightCurrentLayer"
        label="Şu anki katmanı vurgula"
        @update:model-value="touched"
      />
    </SfSection>

    <!-- 02 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.filament" index="02" title="Filament">
      <div class="swatches">
        <SfColorDot
          v-for="(colour, i) in scene.filaments"
          :key="i"
          :color="colour"
          :selected="scene.filamentIndex === i"
          @click="((scene.filamentIndex = i), touched())"
        />
        <SfColorDot add @click="scene.filaments.push('#e7e8e4')" />
      </div>

      <SfColorField v-model="customColour" label="Renk" />

      <SfSelect
        v-model="scene.surface"
        label="Yüzey"
        :options="SURFACES"
        @update:model-value="touched"
      />
    </SfSection>

    <!-- 03 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.bed" index="03" title="Yapı tablası">
      <div class="field">
        <span class="t-overline">Biçim</span>
        <SfTabs
          v-model="scene.plate.style"
          class="wide"
          :mono="false"
          :options="PLATE_STYLES"
          aria-label="Tabla biçimi"
          @update:model-value="touched"
        />
      </div>

      <SfSlider
        v-model="scene.plate.spacing"
        label="Izgara aralığı"
        :min="5"
        :max="50"
        :step="5"
        :disabled="scene.plate.style !== 'grid'"
        :format="(v) => `${integer(v)} mm`"
        @update:model-value="touched"
      />

      <SfSwitch
        v-model="scene.plate.showOutline"
        label="Tabla kenarı"
        :disabled="scene.plate.style === 'none'"
        @update:model-value="touched"
      />
      <SfSwitch
        v-model="scene.plate.showOrigin"
        label="Sıfır noktası"
        @update:model-value="touched"
      />

      <p class="note">Tabla ölçüsü dosyadan geliyor: {{ bedLabel }}</p>
    </SfSection>

    <!-- 04 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.background" index="04" title="Arka plan">
      <div class="field">
        <span class="t-overline">Biçim</span>
        <SfTabs
          v-model="scene.background.style"
          class="wide"
          :mono="false"
          :options="BACKGROUND_STYLES"
          aria-label="Arka plan biçimi"
          @update:model-value="touched"
        />
      </div>

      <SfColorField
        v-model="scene.background.top"
        :label="scene.background.style === 'gradient' ? 'Üst' : 'Renk'"
      />
      <SfColorField
        v-if="scene.background.style === 'gradient'"
        v-model="scene.background.bottom"
        label="Alt"
      />

      <SfSlider
        v-model="scene.background.vignette"
        label="Vinyet"
        :min="0"
        :max="1"
        :step="0.05"
        :format="(v) => `${integer(v * 100)}%`"
        @update:model-value="touched"
      />
    </SfSection>

    <!-- 05 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.light" index="05" title="Işık">
      <SfSlider
        v-model="scene.light.azimuthDeg"
        label="Yön"
        :min="0"
        :max="360"
        :step="5"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.light.elevationDeg"
        label="Yükseklik"
        :min="0"
        :max="90"
        :step="1"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.light.intensity"
        label="Şiddet"
        :min="0"
        :max="1.5"
        :step="0.05"
        :format="(v) => `${decimal(v, 2)}`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.light.fill"
        label="Dolgu"
        :min="0"
        :max="0.6"
        :step="0.02"
        :format="(v) => `${decimal(v, 2)}`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.light.ambient"
        label="Ortam"
        :min="0"
        :max="0.8"
        :step="0.02"
        :format="(v) => `${decimal(v, 2)}`"
        @update:model-value="touched"
      />

      <p class="note">
        Anahtar ışık, karşıdan bir dolgu ve ortam. Gölge yok — kare bütçesi ona yetmez.
      </p>
    </SfSection>

    <!-- 06 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.camera" index="06" title="Kamera">
      <SfSlider
        v-model="scene.camera.azimuthDeg"
        label="Yörünge"
        :min="0"
        :max="360"
        :step="1"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.camera.elevationDeg"
        label="Yükseklik"
        :min="-80"
        :max="80"
        :step="1"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.camera.fovDeg"
        label="Görüş açısı"
        :min="14"
        :max="70"
        :step="1"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.camera.zoom"
        label="Yakınlık"
        :min="0.3"
        :max="4"
        :step="0.05"
        :format="(v) => `${decimal(v, 2)}×`"
        @update:model-value="touched"
      />

      <SfButton variant="ghost" @click="fitCamera">Sığdır</SfButton>
    </SfSection>

    <!-- 07 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.motion" index="07" title="Hareket">
      <SfSlider
        v-model="scene.motion.orbitDeg"
        label="Dönüş"
        :min="-720"
        :max="720"
        :step="15"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.motion.riseDeg"
        label="Yükselme"
        :min="-60"
        :max="60"
        :step="1"
        :format="(v) => `${integer(v)}°`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.motion.zoomTo"
        label="Bitişte yakınlık"
        :min="0.5"
        :max="3"
        :step="0.05"
        :format="(v) => `${decimal(v, 2)}×`"
        @update:model-value="touched"
      />

      <div class="field">
        <span class="t-overline">Geçiş</span>
        <SfTabs
          v-model="scene.motion.easing"
          class="wide"
          :mono="false"
          :options="EASINGS"
          aria-label="Hareket geçişi"
          @update:model-value="touched"
        />
      </div>

      <p class="note">Hareket kare indeksine bağlı; dışa aktarmada birebir aynısı çıkar.</p>
    </SfSection>

    <!-- 08 --------------------------------------------------------------------------- -->
    <SfSection v-model:open="scene.sections.timing" index="08" title="Zamanlama">
      <SfSlider
        v-model="scene.durationS"
        label="Süre"
        :min="2"
        :max="60"
        :step="1"
        :format="(v) => `${integer(v)} sn`"
        @update:model-value="touched"
      />

      <div class="field">
        <span class="t-overline">Kare hızı</span>
        <SfTabs v-model="fpsModel" class="wide" :options="FPS_OPTIONS" aria-label="Kare hızı" />
      </div>

      <SfSlider
        v-model="scene.timing.holdStart"
        label="Baştaki bekleme"
        :min="0"
        :max="90"
        :step="1"
        :format="(v) => `${integer(v)} kare`"
        @update:model-value="touched"
      />
      <SfSlider
        v-model="scene.timing.holdEnd"
        label="Sondaki bekleme"
        :min="0"
        :max="90"
        :step="1"
        :format="(v) => `${integer(v)} kare`"
        @update:model-value="touched"
      />

      <div class="field">
        <span class="t-overline">Geçiş</span>
        <SfTabs
          v-model="scene.timing.easing"
          class="wide"
          :mono="false"
          :options="EASINGS"
          aria-label="Katman geçişi"
          @update:model-value="touched"
        />
      </div>
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

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.wide {
  display: flex;
}

.wide :deep(.tab) {
  flex: 1;
  text-align: center;
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

.note {
  margin: 0;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--text-faint);
}
</style>
