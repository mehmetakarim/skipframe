<script setup lang="ts">
/**
 * FRAME 08 — sharing a finished video to a StepperSkip company profile.
 *
 * One modal through the whole job, as the export panel is: the form, then the upload, then the
 * published post or what went wrong. Closing it does not stop an upload; the notice stack says
 * when it finishes.
 *
 * The design's tag row is gone. StepperSkip's company posts have no tags, and a field that sends
 * nothing anywhere would be worse than no field.
 */
import { computed, ref, watch } from 'vue';
import { convertFileSrc } from '@tauri-apps/api/core';

import SfModal from '../ui/SfModal.vue';
import SfButton from '../ui/SfButton.vue';
import SfSelect from '../ui/SfSelect.vue';
import SfTextField from '../ui/SfTextField.vue';
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
} from '../../stores/account';
import {
  DESCRIPTION_LIMIT,
  SHARE_STAGE_LABELS,
  TITLE_LIMIT,
  blockers,
  cancelShare,
  closeShareDialog,
  copyPostLink,
  durationOf,
  share,
  startShare,
  type Blocker,
} from '../../stores/share';
import { revealFile } from '../../export/writeFile';
import { decimal, megabytes, shortPath } from '../../lib/format';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/**
 * The exported MP4, played from disk. The Rust side granted the asset protocol access to exactly
 * this file when it wrote it; nothing else on disk is reachable this way.
 */
const videoSrc = computed(() => {
  const video = share.video;
  if (!inTauri || !video || video.format !== 'mp4') return null;
  return convertFileSrc(video.outputPath);
});

/**
 * A video shared later may have been moved or deleted since it was written. Say so in the preview
 * rather than show an empty frame; sharing it would fail on the same missing file.
 */
const previewMissing = ref(false);
watch(videoSrc, () => (previewMissing.value = false));

const aspect = computed(() => {
  const v = share.video;
  return v ? `${v.width} / ${v.height}` : '9 / 16';
});

const percent = computed(() => {
  const { sentBytes, totalBytes } = share.progress;
  if (share.progress.stage === 'publishing') return 100;
  return totalBytes > 0 ? Math.min(100, Math.round((sentBytes / totalBytes) * 100)) : 0;
});

const usableCompanies = computed(() => account.companies.filter((c) => c.canPublish));

const companyModel = computed({
  get: () => (account.selectedCompanyId === null ? '' : String(account.selectedCompanyId)),
  set: (v: string) => {
    account.selectedCompanyId = v ? Number(v) : null;
  },
});

const editable = computed(() => share.phase === 'form' || share.phase === 'failed');

async function switchAccount() {
  await signOut({ quiet: true });
  await signIn();
}

function act(blocker: Blocker) {
  if (blocker.action === 'sign-in') void signIn();
  else if (blocker.action === 'retry-account') void refreshAccount();
  else if (blocker.action === 'company-setup') void openCompanySetup();
}

const ACTION_LABELS: Record<NonNullable<Blocker['action']>, string> = {
  'sign-in': 'Giriş yap',
  'retry-account': 'Tekrar dene',
  'company-setup': 'Firma profili aç',
};
</script>

<template>
  <SfModal
    :title="share.phase === 'done' ? 'Paylaşıldı' : 'StepperSkip’te paylaş'"
    :width="920"
    @close="closeShareDialog"
  >
    <template #status>
      <span v-if="share.phase === 'sharing'" class="t-overline running">Yükleniyor</span>
    </template>

    <div class="layout">
      <!-- ------------------------------------------------------------- preview -->
      <aside class="preview">
        <div class="frame" :style="{ aspectRatio: aspect }">
          <video
            v-if="videoSrc && !previewMissing"
            :src="videoSrc"
            controls
            playsinline
            preload="metadata"
            @error="previewMissing = true"
          />
          <div v-else class="no-video">
            <span class="t-overline">{{
              previewMissing ? 'Video dosyası bulunamadı' : 'Önizleme yok'
            }}</span>
          </div>
        </div>
        <p v-if="share.video" class="facts">
          {{ share.video.width }}×{{ share.video.height }} · {{ share.video.fps }} fps ·
          {{ decimal(durationOf(share.video), 0) }} sn · {{ megabytes(share.video.bytes) }}
        </p>
        <p v-if="share.video" class="file" :title="share.video.outputPath">
          {{ shortPath(share.video.outputPath) }}
        </p>
      </aside>

      <!-- ------------------------------------------------------------- right -->
      <section class="main">
        <!-- account -->
        <div class="account">
          <div :class="['avatar', { filled: account.status === 'signed-in' }]">
            {{ account.status === 'signed-in' ? initialsOf(account.user) : '—' }}
          </div>
          <div class="who">
            <template v-if="account.status === 'signed-in' && account.user">
              <span class="name">{{ account.user.displayName }}</span>
              <span class="sub">
                {{
                  selectedCompany
                    ? `${selectedCompany.name} firma profiline`
                    : 'Firma profili seçilmedi'
                }}
              </span>
            </template>
            <template v-else-if="account.status === 'signing-in'">
              <span class="name">Tarayıcıda devam et</span>
              <span class="sub">StepperSkip girişi sistem tarayıcında açıldı.</span>
            </template>
            <template v-else-if="account.status === 'loading' || account.status === 'unknown'">
              <span class="name">Hesap okunuyor…</span>
            </template>
            <template v-else>
              <span class="name">StepperSkip hesabı bağlı değil</span>
              <span class="sub"
                >Giriş sistem tarayıcısında yapılır; SkipFrame parolanı görmez.</span
              >
            </template>
          </div>
          <div class="spacer" />
          <SfButton
            v-if="account.status === 'signed-in' && editable"
            variant="outline"
            @click="switchAccount"
          >
            Hesabı değiştir
          </SfButton>
          <SfButton
            v-else-if="account.status === 'signing-in'"
            variant="ghost"
            @click="cancelSignIn"
          >
            Vazgeç
          </SfButton>
        </div>

        <SfSelect
          v-if="account.status === 'signed-in' && usableCompanies.length > 1 && editable"
          v-model="companyModel"
          label="Paylaşılacak firma"
          :options="usableCompanies.map((c) => ({ value: String(c.id), label: c.name }))"
        />

        <!-- form ------------------------------------------------------------- -->
        <template v-if="editable">
          <SfTextField
            v-model="share.title"
            label="Başlık"
            :limit="TITLE_LIMIT"
            required
            placeholder="Videonun başlığı"
          />
          <SfTextField
            v-model="share.description"
            label="Açıklama"
            multiline
            :rows="6"
            :limit="DESCRIPTION_LIMIT"
            placeholder="Filament, sıcaklık, hız — izleyenin bilmek isteyecekleri"
          />

          <p v-if="share.phase === 'failed' && share.error" class="error">{{ share.error }}</p>

          <ul v-if="blockers.length" class="blockers">
            <li v-for="blocker in blockers" :key="blocker.reason">
              <span>{{ blocker.reason }}</span>
              <button
                v-if="blocker.action"
                type="button"
                class="blocker-action"
                @click="act(blocker)"
              >
                {{ ACTION_LABELS[blocker.action] }}
              </button>
            </li>
          </ul>
        </template>

        <!-- sharing ---------------------------------------------------------- -->
        <div v-else-if="share.phase === 'sharing'" class="progress">
          <div class="numbers">
            <span class="percent">{{ percent }}<span class="unit">%</span></span>
            <span class="bytes">
              {{ megabytes(share.progress.sentBytes) }} / {{ megabytes(share.progress.totalBytes) }}
            </span>
          </div>
          <div class="bar"><div class="fill" :style="{ width: `${percent}%` }" /></div>
          <span class="stage">{{ SHARE_STAGE_LABELS[share.progress.stage] }}</span>
          <dl class="rows">
            <div>
              <dt>Firma</dt>
              <dd>{{ selectedCompany?.name ?? '—' }}</dd>
            </div>
            <div>
              <dt>Başlık</dt>
              <dd>{{ share.title }}</dd>
            </div>
          </dl>
        </div>

        <!-- done ------------------------------------------------------------- -->
        <div v-else-if="share.phase === 'done' && share.post" class="done">
          <div class="done-head">
            <span class="badge" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="18" height="18">
                <path
                  d="M5 12.5l4.5 4.5L19 7.5"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.4"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </span>
            <div>
              <span class="done-title">Başarıyla paylaşıldı</span>
              <span class="sub">{{ selectedCompany?.name ?? '' }} · {{ share.post.title }}</span>
            </div>
          </div>
          <p class="link">{{ share.post.postUrl }}</p>
          <span class="sub">
            {{
              share.linkCopied
                ? 'Bağlantı panoya kopyalandı.'
                : 'Bağlantı kopyalanamadı; aşağıdan tekrar dene.'
            }}
          </span>
        </div>
      </section>
    </div>

    <!-- --------------------------------------------------------------- footer -->
    <template #footer>
      <template v-if="share.phase === 'sharing'">
        <SfButton variant="outline" @click="cancelShare">İptal et</SfButton>
        <div class="spacer" />
        <span class="hint">Pencereyi kapatabilirsin; yükleme sürer, bitince haber verilir.</span>
      </template>

      <template v-else-if="share.phase === 'done' && share.post">
        <SfButton variant="primary" @click="openStepperSkipUrl(share.post.postUrl)">
          StepperSkip’te aç
        </SfButton>
        <SfButton variant="outline" @click="copyPostLink">Bağlantıyı kopyala</SfButton>
        <div class="spacer" />
        <SfButton variant="ghost" @click="closeShareDialog">Kapat</SfButton>
      </template>

      <template v-else>
        <SfButton variant="primary" :disabled="blockers.length > 0" @click="startShare">
          {{ share.phase === 'failed' ? 'Tekrar dene' : 'StepperSkip’te paylaş' }}
        </SfButton>
        <SfButton v-if="share.video" variant="outline" @click="revealFile(share.video.outputPath)">
          Klasörde göster
        </SfButton>
        <div class="spacer" />
        <span class="hint">Yükleme bittiğinde bağlantı panoya kopyalanır.</span>
      </template>
    </template>
  </SfModal>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  gap: 28px;
  min-height: 0;
}

/* -- preview ---------------------------------------------------------------------------- */

.preview {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
}

.frame {
  width: 100%;
  max-height: 480px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-base);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
}

video {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  background: var(--bg-base);
}

.no-video {
  color: var(--text-faint);
  text-align: center;
  padding: 0 var(--space-4);
}

.file {
  margin: -6px 0 0;
  max-width: 100%;
  font-family: var(--font-mono);
  font-size: var(--type-path-size);
  color: var(--text-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.facts {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-muted);
  text-align: center;
}

/* -- right column ----------------------------------------------------------------------- */

.main {
  display: flex;
  flex-direction: column;
  gap: 20px;
  min-width: 0;
}

.account {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.avatar {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  flex: none;
  border-radius: 50%;
  background: var(--border);
  color: var(--text-faint);
  font-size: 12px;
}

.avatar.filled {
  background: var(--bg-overlay);
  color: var(--text-secondary);
  font-weight: 700;
}

.who {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.name {
  font-size: var(--type-body-size);
  font-weight: 700;
  color: var(--text-primary);
}

.sub {
  display: block;
  font-size: 11.5px;
  color: var(--text-muted);
  overflow-wrap: anywhere;
}

.spacer {
  flex: 1;
}

.error {
  margin: 0;
  font-size: var(--type-label-size);
  line-height: 1.6;
  color: var(--danger);
}

.blockers {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding: 12px 14px;
  list-style: none;
  border: 1px solid var(--border);
  border-left: 2px solid var(--border-strong);
  border-radius: var(--radius);
  background: var(--bg-base);
}

.blockers li {
  display: flex;
  align-items: baseline;
  gap: var(--space-3);
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary);
}

.blockers li span {
  flex: 1;
}

.blocker-action {
  flex: none;
  padding: 0;
  border: 0;
  background: none;
  font-family: inherit;
  font-size: 12px;
  font-weight: 700;
  color: var(--gold);
  cursor: pointer;
}

.blocker-action:hover {
  color: var(--gold-hover);
  text-decoration: underline;
}

/* -- progress --------------------------------------------------------------------------- */

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

.bytes {
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
  transition: width 160ms linear;
}

.stage {
  font-size: 12px;
  color: var(--text-muted);
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 7px;
  margin: var(--space-3) 0 0;
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
  font-size: 12px;
  color: var(--text-primary);
  text-align: right;
  overflow-wrap: anywhere;
}

/* -- done ------------------------------------------------------------------------------- */

.done {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.done-head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.done-title {
  display: block;
  font-size: var(--type-heading-size);
  font-weight: 700;
  color: var(--text-primary);
}

.link {
  margin: 0;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--text-primary);
  word-break: break-all;
  user-select: text;
}

.running {
  color: var(--gold);
}

.hint {
  font-size: 10.5px;
  color: var(--text-faint);
}
</style>
