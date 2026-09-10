<script setup lang="ts">
/**
 * Preferences.
 *
 * One scrolling page with a rail that jumps between its sections, which is how the design draws
 * it — the rail marks where you are rather than swapping panels.
 */
import { onMounted, ref, useTemplateRef } from 'vue';

import TitleBar from '../components/studio/TitleBar.vue';
import SfButton from '../components/ui/SfButton.vue';
import SfSwitch from '../components/ui/SfSwitch.vue';
import {
  checkForUpdate,
  clearFfmpegPath,
  decorateStem,
  pickFfmpeg,
  pickOutputDir,
  pickWatchDir,
  setWatchEnabled,
  settings,
  settingsState,
  sinceLastCheck,
} from '../stores/settings';

const SECTIONS = [
  { id: 'output', label: 'Çıktı' },
  { id: 'watch', label: 'İzlenen klasör' },
  { id: 'advanced', label: 'Gelişmiş' },
  { id: 'account', label: 'Hesap' },
  { id: 'update', label: 'Güncelleme' },
];

const active = ref('output');
const pageRef = useTemplateRef<HTMLDivElement>('page');
const version = ref('0.1.0');

function jumpTo(id: string) {
  active.value = id;
  document.getElementById(`set-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

/** The rail follows the scroll rather than only responding to clicks. */
function onScroll() {
  const page = pageRef.value;
  if (!page) return;
  const top = page.getBoundingClientRect().top;
  for (const section of [...SECTIONS].reverse()) {
    const el = document.getElementById(`set-${section.id}`);
    if (el && el.getBoundingClientRect().top - top <= 24) {
      active.value = section.id;
      return;
    }
  }
  active.value = SECTIONS[0]!.id;
}

onMounted(async () => {
  try {
    const { getVersion } = await import('@tauri-apps/api/app');
    version.value = await getVersion();
  } catch {
    // A browser tab; the fallback above is fine.
  }
});
</script>

<template>
  <div class="screen">
    <TitleBar>
      <template #trailing>
        <span class="where">Ayarlar</span>
      </template>
    </TitleBar>

    <div class="body">
      <!-- rail ------------------------------------------------------------------------- -->
      <nav class="rail">
        <button
          v-for="section in SECTIONS"
          :key="section.id"
          type="button"
          :class="['nav', { active: active === section.id }]"
          @click="jumpTo(section.id)"
        >
          {{ section.label }}
        </button>

        <div class="spacer" />

        <div class="version">
          <span class="t-overline">Sürüm</span>
          <span class="version-number">{{ version }}</span>
        </div>
      </nav>

      <!-- page ------------------------------------------------------------------------- -->
      <div ref="page" class="page" @scroll="onScroll">
        <!-- Çıktı -->
        <section id="set-output" class="group">
          <span class="t-overline">Çıktı klasörü</span>
          <div class="path-row">
            <div class="path">{{ settings.outputDir ?? 'Videolar klasörü · SkipFrame' }}</div>
            <SfButton variant="outline" @click="pickOutputDir">Değiştir</SfButton>
          </div>

          <div class="setting">
            <div class="label">
              <span class="title">Dosya adına tarih ekle</span>
              <span class="hint mono">{{ decorateStem('benchy') }}.mp4</span>
            </div>
            <SfSwitch v-model="settings.dateInFilename" />
          </div>
        </section>

        <div class="divider" />

        <!-- İzlenen klasör -->
        <section id="set-watch" class="group">
          <span class="t-overline">İzlenen klasör</span>
          <p class="lede">
            Slicer’ın çıktı dizinini izle. Yeni bir G-code belirdiğinde SkipFrame onu kuyruğa alır
            ve seçili preset ile render eder.
          </p>

          <div class="path-row">
            <div class="path">{{ settings.watchDir ?? 'Seçilmedi' }}</div>
            <SfButton variant="outline" @click="pickWatchDir">Değiştir</SfButton>
          </div>

          <div class="setting">
            <div class="label">
              <span class="title">Klasör izlemeyi aç</span>
              <span class="hint">
                Klasör iki saniyede bir taranır; dosya yazılmayı bitirene kadar kuyruğa alınmaz.
              </span>
            </div>
            <SfSwitch
              :model-value="settings.watchEnabled"
              :disabled="!settings.watchDir"
              @update:model-value="setWatchEnabled"
            />
          </div>

          <p v-if="settingsState.watchError" class="error">{{ settingsState.watchError }}</p>
        </section>

        <div class="divider" />

        <!-- Gelişmiş -->
        <section id="set-advanced" class="group">
          <span class="t-overline">Harici FFmpeg</span>
          <p class="lede">
            İsteğe bağlı ve hiçbir zaman paketlenmiyor. Kurulu bir FFmpeg bulunursa ProRes ve WebM
            çıktıları açılacak — o çıktılar henüz yazılmadı, burada yalnızca bulunup bulunmadığı
            görünüyor.
          </p>

          <div class="path-row">
            <div class="path">
              {{ settingsState.ffmpeg?.path ?? settings.ffmpegPath ?? 'PATH üzerinde aranıyor' }}
            </div>
            <SfButton variant="outline" @click="pickFfmpeg">Yolu seç</SfButton>
            <SfButton v-if="settings.ffmpegPath" variant="ghost" @click="clearFfmpegPath">
              Sıfırla
            </SfButton>
          </div>

          <span v-if="settingsState.ffmpeg" class="found">
            bulundu · {{ settingsState.ffmpeg.version }}
          </span>
          <span v-else-if="settingsState.ffmpegChecked" class="missing">bulunamadı</span>
        </section>

        <div class="divider" />

        <!-- Hesap -->
        <section id="set-account" class="group">
          <span class="t-overline">StepperSkip hesabı</span>
          <div class="card">
            <div class="avatar">—</div>
            <div class="card-body">
              <span class="title">Hesap bağlama henüz yok</span>
              <span class="hint">
                SkipFrame’in çalışması için hesap gerekmiyor; dosyalar zaten cihazdan çıkmıyor. Bu
                bölüm paylaşım akışıyla birlikte gelecek.
              </span>
            </div>
          </div>
        </section>

        <div class="divider" />

        <!-- Güncelleme -->
        <section id="set-update" class="group">
          <span class="t-overline">Güncelleme</span>
          <p class="lede">
            Uygulama imzasız dağıtılıyor, bu yüzden güncellemeler uygulamanın içinden geliyor —
            ikinci bir kurulum sürtünmesi olmasın diye.
          </p>

          <div class="setting">
            <span class="title">Otomatik güncelle</span>
            <SfSwitch v-model="settings.autoUpdate" />
          </div>

          <div class="update-row">
            <SfButton
              variant="outline"
              :disabled="settingsState.checkingUpdate"
              @click="checkForUpdate"
            >
              {{ settingsState.checkingUpdate ? 'Bakılıyor…' : 'Güncelleme ara' }}
            </SfButton>
            <span class="hint">{{ settingsState.updateMessage ?? sinceLastCheck() }}</span>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.screen {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-surface);
}

.where {
  font-size: var(--type-label-size);
  color: var(--text-primary);
}

.body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 212px 1fr;
}

.body > * {
  min-width: 0;
  min-height: 0;
}

/* -- rail ----------------------------------------------------------------------------- */

.rail {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-4) var(--space-3);
  border-right: 1px solid var(--border);
}

.nav {
  padding: 9px var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius);
  background: none;
  color: var(--text-secondary);
  font-size: var(--type-label-size);
  text-align: left;
  cursor: pointer;
}

.nav:hover:not(.active) {
  background: var(--bg-raised);
  color: var(--text-primary);
}

.nav.active {
  border-color: var(--gold);
  background: var(--gold-tint);
  color: var(--text-primary);
  font-weight: 700;
}

.spacer {
  flex: 1;
}

.version {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  border-top: 1px solid var(--border);
}

.version-number {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
}

/* -- page ----------------------------------------------------------------------------- */

.page {
  display: flex;
  flex-direction: column;
  gap: 26px;
  padding: 30px 34px;
  overflow-y: auto;
}

.group {
  display: flex;
  flex-direction: column;
  gap: 14px;
  scroll-margin-top: 30px;
}

.divider {
  height: 1px;
  background: var(--border);
}

.lede {
  margin: 0;
  max-width: 64ch;
  font-size: var(--type-body-size);
  line-height: 1.6;
  color: var(--text-muted);
}

.path-row {
  display: flex;
  align-items: center;
  gap: 10px;
  max-width: 720px;
}

.path {
  flex: 1;
  min-width: 0;
  padding: 11px 13px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-5);
  max-width: 720px;
}

.label {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.title {
  font-size: var(--type-body-size);
  color: var(--text-primary);
}

.hint {
  max-width: 60ch;
  font-size: 10.5px;
  line-height: 1.6;
  color: var(--text-faint);
}

.hint.mono {
  font-family: var(--font-mono);
}

.found {
  font-size: 10.5px;
  color: var(--text-secondary);
}

.missing {
  font-size: 10.5px;
  color: var(--text-faint);
}

.error {
  margin: 0;
  font-size: var(--type-label-size);
  color: var(--danger);
}

.card {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  max-width: 720px;
  padding: 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-base);
}

.avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  flex: none;
  border-radius: 50%;
  background: var(--border);
  color: var(--text-faint);
  font-size: 12px;
  font-weight: 700;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.update-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
</style>
