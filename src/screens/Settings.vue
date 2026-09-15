<script setup lang="ts">
/**
 * Preferences.
 *
 * One scrolling page with a rail that jumps between its sections, which is how the design draws
 * it — the rail marks where you are rather than swapping panels.
 */
import { computed, onMounted, ref, useTemplateRef } from 'vue';

import TitleBar from '../components/studio/TitleBar.vue';
import SfButton from '../components/ui/SfButton.vue';
import SfSwitch from '../components/ui/SfSwitch.vue';
import SfSelect from '../components/ui/SfSelect.vue';
import {
  account,
  cancelSignIn,
  initialsOf,
  openCompanySetup,
  openStepperSkipUrl,
  refreshAccount,
  selectedCompany,
  signIn,
  signOut,
} from '../stores/account';
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

const initials = computed(() => initialsOf(account.user));

const usableCompanies = computed(() => account.companies.filter((c) => c.canPublish));

const companyOptions = computed(() =>
  usableCompanies.value.map((c) => ({ value: String(c.id), label: c.name })),
);

const companyModel = computed({
  get: () => (account.selectedCompanyId === null ? '' : String(account.selectedCompanyId)),
  set: (v: string) => {
    account.selectedCompanyId = v ? Number(v) : null;
  },
});

onMounted(async () => {
  // Only now, and only here: reading the account refreshes its token, and there is no reason to
  // rotate a refresh token on every launch for a user who never opens this screen or shares.
  void refreshAccount();

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
          <p class="lede">
            Bitmiş bir videoyu StepperSkip’teki firma profilinde paylaşmak için. SkipFrame’in geri
            kalanı hesapsız çalışır ve G-code dosyaların hiçbir durumda cihazdan çıkmaz. Giriş
            sistem tarayıcısında yapılır; SkipFrame parolanı görmez.
          </p>

          <div class="card">
            <div :class="['avatar', { filled: account.status === 'signed-in' }]">
              {{ account.status === 'signed-in' ? initials : '—' }}
            </div>

            <div class="card-body">
              <template v-if="account.status === 'signed-in' && account.user">
                <span class="title">{{ account.user.displayName }}</span>
                <span class="hint mono">@{{ account.user.username }}</span>
              </template>
              <template v-else-if="account.status === 'signing-in'">
                <span class="title">Tarayıcıda devam et</span>
                <span class="hint">
                  StepperSkip giriş sayfası sistem tarayıcında açıldı. İzin verdiğinde buraya
                  kendiliğinden dönülür.
                </span>
              </template>
              <template v-else-if="account.status === 'loading' || account.status === 'unknown'">
                <span class="title">Hesap okunuyor…</span>
              </template>
              <template v-else-if="account.status === 'unreachable'">
                <!-- The reason is in the hint, in StepperSkip's or our own words; the title only
                     says the account is still there. -->
                <span class="title">Hesap şu an okunamadı</span>
                <span class="hint">{{ account.problem }} Oturum silinmedi.</span>
              </template>
              <template v-else-if="account.status === 'unconfigured'">
                <span class="title">Bu sürümde StepperSkip bağlantısı yok</span>
                <span class="hint"
                  >Paylaşım, yapılandırılmış bir StepperSkip sunucusu gerektiriyor.</span
                >
              </template>
              <template v-else>
                <span class="title">Hesap bağlı değil</span>
                <span class="hint">Paylaşmak istediğinde giriş yapman yeterli.</span>
              </template>
            </div>

            <div class="card-actions">
              <SfButton v-if="account.status === 'signed-out'" variant="outline" @click="signIn">
                StepperSkip ile giriş yap
              </SfButton>
              <SfButton
                v-if="account.status === 'signing-in'"
                variant="ghost"
                @click="cancelSignIn"
              >
                Vazgeç
              </SfButton>
              <SfButton
                v-if="account.status === 'unreachable'"
                variant="outline"
                @click="refreshAccount"
              >
                Tekrar dene
              </SfButton>
              <SfButton
                v-if="account.status === 'signed-in' || account.status === 'unreachable'"
                variant="ghost"
                @click="signOut"
              >
                Hesabı kaldır
              </SfButton>
            </div>
          </div>

          <!-- Paylaşım firma profiline yapılır. Kural StepperSkip'in; burada yalnızca anlatılıyor. -->
          <template v-if="account.status === 'signed-in'">
            <div v-if="usableCompanies.length === 0" class="company-missing">
              <span class="title">Firma profili yok</span>
              <span class="hint">
                Paylaşım StepperSkip’teki firma profillerine yapılır. Koşulları karşılıyorsan
                StepperSkip’te bir firma profili açıp buraya dönebilirsin.
              </span>
              <div class="update-row">
                <SfButton variant="outline" @click="openCompanySetup">
                  StepperSkip’te firma profili aç
                </SfButton>
                <SfButton variant="ghost" @click="refreshAccount">Yeniden kontrol et</SfButton>
              </div>
            </div>

            <div v-else-if="usableCompanies.length === 1 && selectedCompany" class="setting">
              <div class="card-body">
                <span class="t-overline">Paylaşılacak firma</span>
                <span class="title">{{ selectedCompany.name }}</span>
              </div>
              <SfButton
                v-if="selectedCompany.profileUrl"
                variant="ghost"
                @click="openStepperSkipUrl(selectedCompany.profileUrl)"
              >
                Profili aç
              </SfButton>
            </div>

            <SfSelect
              v-else
              v-model="companyModel"
              class="company-select"
              label="Paylaşılacak firma"
              :options="companyOptions"
            />
          </template>
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

.avatar.filled {
  background: var(--bg-overlay);
  color: var(--text-secondary);
  font-weight: 700;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
  flex: 1;
}

.card-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: none;
}

.company-missing {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  max-width: 720px;
}

.company-select {
  max-width: 360px;
}

.update-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
</style>
