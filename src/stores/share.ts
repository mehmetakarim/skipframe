import { computed, reactive } from 'vue';

import type { Ir } from '../ir/types';
import { draftCaption } from '../lib/captionDraft';
import { decimal, megabytes } from '../lib/format';
import { errorCode, errorText } from '../lib/messages';
import { account, refreshAccount, selectedCompany } from './account';
import { notify, notifyError } from './notices';

/**
 * Sharing a finished video to a StepperSkip company profile — the share screen's state.
 *
 * The upload itself runs in Rust (`src-tauri/src/stepperskip/share.rs`), which resumes from the
 * server's progress, verifies the SHA-256 and publishes. This module decides whether sharing can
 * start, carries the progress events to the screen, and keeps going if the screen is closed.
 */

/** What was exported, as the share screen describes and checks it. */
export interface ShareSource {
  path: string;
  format: 'mp4' | 'frames';
  bytes: number;
  durationS: number;
  width: number;
  height: number;
  fps: number;
}

export type ShareStage = 'preparing' | 'uploading' | 'reconnecting' | 'restarting' | 'publishing';

export const SHARE_STAGE_LABELS: Record<ShareStage, string> = {
  preparing: 'Video hazırlanıyor',
  uploading: 'Yükleniyor',
  reconnecting: 'Bağlantı yeniden kuruluyor',
  restarting: 'Yükleme baştan başlatılıyor',
  publishing: 'Yayımlanıyor',
};

export interface SharedPost {
  id: number;
  slug: string;
  companyId: number;
  title: string;
  postUrl: string;
  mediaUrl: string | null;
  published: boolean;
  publishedAt: string | null;
}

/**
 * StepperSkip's publish validation (`Skipframe_publish_service`). Unlike size and duration these
 * are not in `/limits`, so they are written down here — and the server still checks them.
 */
export const TITLE_LIMIT = 255;
export const DESCRIPTION_LIMIT = 5000;

export type SharePhase = 'form' | 'sharing' | 'done' | 'failed';

export const share = reactive({
  open: false,
  source: null as ShareSource | null,
  title: '',
  description: '',
  phase: 'form' as SharePhase,
  progress: { stage: 'preparing' as ShareStage, sentBytes: 0, totalBytes: 0 },
  post: null as SharedPost | null,
  error: null as string | null,
  errorCode: null as string | null,
  linkCopied: false,
});

export interface Blocker {
  reason: string;
  /** Something the screen can offer to do about it. */
  action?: 'sign-in' | 'retry-account' | 'company-setup';
}

const characters = (s: string) => [...s].length;

/**
 * Why sharing cannot start, in the order they would have to be fixed. Empty means it can.
 *
 * Every one of these is also enforced by StepperSkip. Checking here is about saying so before a
 * 50 MB upload rather than after it.
 */
export const blockers = computed<Blocker[]>(() => {
  const list: Blocker[] = [];
  const source = share.source;

  if (source && source.format !== 'mp4') {
    list.push({ reason: 'Yalnızca MP4 çıktılar paylaşılabilir; kare sekansı paylaşılamaz.' });
  }

  switch (account.status) {
    case 'unconfigured':
      list.push({ reason: 'Bu sürümde StepperSkip bağlantısı yok.' });
      return list;
    case 'unknown':
    case 'loading':
      list.push({ reason: 'StepperSkip hesabı okunuyor…' });
      return list;
    case 'signing-in':
      list.push({ reason: 'Tarayıcıda StepperSkip girişi bekleniyor.' });
      return list;
    case 'signed-out':
      list.push({ reason: 'Paylaşmak için StepperSkip’e giriş yap.', action: 'sign-in' });
      return list;
    case 'unreachable':
      list.push({
        reason: account.problem ?? 'StepperSkip’e ulaşılamadı.',
        action: 'retry-account',
      });
      return list;
  }

  const publishable = account.companies.filter((c) => c.canPublish);
  if (publishable.length === 0) {
    list.push({
      reason: 'Paylaşım için StepperSkip’te aktif bir firma profili gerekli.',
      action: 'company-setup',
    });
  } else if (!selectedCompany.value) {
    list.push({ reason: 'Paylaşılacak firma profilini seç.' });
  }

  const limits = account.limits;
  if (source && limits) {
    if (source.bytes > limits.maxFileSizeBytes) {
      list.push({
        reason:
          `Video ${megabytes(source.bytes)}; StepperSkip en fazla ` +
          `${megabytes(limits.maxFileSizeBytes)} kabul ediyor. Kare hızını, render ölçeğini ya da ` +
          'süreyi düşürüp yeniden render al.',
      });
    }
    // Exports are a whole number of frames, so their length is exact; the tolerance only absorbs
    // floating-point noise, never a real extra frame.
    if (source.durationS > limits.maxVideoDurationSeconds + 1e-6) {
      list.push({
        reason:
          `Video ${decimal(source.durationS, 1)} sn; StepperSkip en fazla ` +
          `${decimal(limits.maxVideoDurationSeconds, 0)} sn kabul ediyor.`,
      });
    }
  }

  if (share.title.trim() === '') {
    list.push({ reason: 'Başlık gerekli.' });
  } else if (characters(share.title.trim()) > TITLE_LIMIT) {
    list.push({ reason: `Başlık en fazla ${TITLE_LIMIT} karakter olabilir.` });
  }
  if (characters(share.description.trim()) > DESCRIPTION_LIMIT) {
    list.push({ reason: `Açıklama en fazla ${DESCRIPTION_LIMIT} karakter olabilir.` });
  }

  return list;
});

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/**
 * Open the share screen for an export. A different file than last time starts a fresh draft; the
 * same file keeps what was typed, and a share that is still running is simply shown again.
 */
export function openShareDialog(source: ShareSource, model: Ir | null): void {
  if (share.phase !== 'sharing' && share.source?.path !== source.path) {
    const draft = model ? draftCaption(model) : { title: '', description: '' };
    share.source = source;
    share.title = draft.title;
    share.description = draft.description;
    share.phase = 'form';
    share.post = null;
    share.error = null;
    share.errorCode = null;
    share.linkCopied = false;
  }
  share.open = true;

  // A signed-in account already has its companies and limits; anything else needs reading.
  if (account.status !== 'signed-in' && account.status !== 'signing-in') void refreshAccount();
}

export function closeShareDialog(): void {
  share.open = false;
  // A finished share has nothing left to show; the next export starts clean.
  if (share.phase === 'done') share.source = null;
}

export async function startShare(): Promise<void> {
  const source = share.source;
  const company = selectedCompany.value;
  if (!inTauri || !source || !company || share.phase === 'sharing' || blockers.value.length > 0) {
    return;
  }

  share.phase = 'sharing';
  share.error = null;
  share.errorCode = null;
  share.progress = { stage: 'preparing', sentBytes: 0, totalBytes: source.bytes };

  const [{ invoke }, { listen }] = await Promise.all([
    import('@tauri-apps/api/core'),
    import('@tauri-apps/api/event'),
  ]);
  const unlisten = await listen<typeof share.progress>('skipframe://share-progress', (event) => {
    share.progress = event.payload;
  });

  try {
    const post = await invoke<SharedPost>('ss_share', {
      request: {
        path: source.path,
        companyId: company.id,
        title: share.title.trim(),
        description: share.description.trim(),
      },
    });
    share.post = post;
    share.phase = 'done';
    // The design promises the link is on the clipboard when the upload finishes.
    share.linkCopied = await copyText(post.postUrl);

    if (!share.open) {
      notify({
        title: 'Video StepperSkip’te paylaşıldı',
        detail: post.postUrl,
        action: { label: 'Göster', run: () => (share.open = true) },
      });
    }
  } catch (e) {
    const code = errorCode(e);
    if (code === 'share_cancelled') {
      share.phase = 'form';
      return;
    }
    share.phase = 'failed';
    share.error = errorText(e);
    share.errorCode = code;

    // The Rust side has already forgotten a dead session; bring the account card in line.
    if (code === 'session_expired' || code === 'not_signed_in') void refreshAccount();

    if (!share.open) {
      notifyError('Video paylaşılamadı', e, {
        label: 'Göster',
        run: () => (share.open = true),
      });
    }
  } finally {
    unlisten();
  }
}

export function cancelShare(): void {
  if (!inTauri || share.phase !== 'sharing') return;
  void import('@tauri-apps/api/core').then(({ invoke }) => invoke('ss_cancel_share'));
}

export async function copyPostLink(): Promise<void> {
  if (!share.post) return;
  share.linkCopied = await copyText(share.post.postUrl);
  if (!share.linkCopied) notify({ kind: 'warning', title: 'Bağlantı panoya kopyalanamadı' });
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
