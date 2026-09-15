import { computed, reactive } from 'vue';

import type { ExportedVideo } from '../export/types';
import { parseFile } from '../ir/parseFile';
import { draftCaption, readableName, type CaptionDraft } from '../lib/captionDraft';
import { decimal, megabytes } from '../lib/format';
import { errorCode, errorText } from '../lib/messages';
import { account, refreshAccount, selectedCompany } from './account';
import { notify, notifyError } from './notices';
import { ir, project } from './project';

/**
 * Sharing a finished video to a StepperSkip company profile — the share screen's state.
 *
 * The upload itself runs in Rust (`src-tauri/src/stepperskip/share.rs`), which resumes from the
 * server's progress, verifies the SHA-256 and publishes. This module decides whether sharing can
 * start, carries the progress events to the screen, and keeps going if the screen is closed.
 */

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
  video: null as ExportedVideo | null,
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
  const video = share.video;

  if (video && video.format !== 'mp4') {
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
  if (video && limits) {
    if (video.bytes > limits.maxFileSizeBytes) {
      list.push({
        reason:
          `Video ${megabytes(video.bytes)}; StepperSkip en fazla ` +
          `${megabytes(limits.maxFileSizeBytes)} kabul ediyor. Kare hızını, render ölçeğini ya da ` +
          'süreyi düşürüp yeniden render al.',
      });
    }
    // Exports are a whole number of frames, so their length is exact; the tolerance only absorbs
    // floating-point noise, never a real extra frame.
    if (durationOf(video) > limits.maxVideoDurationSeconds + 1e-6) {
      list.push({
        reason:
          `Video ${decimal(durationOf(video), 1)} sn; StepperSkip en fazla ` +
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

/** Length in seconds: exports are a whole number of frames at a fixed rate. */
export function durationOf(video: ExportedVideo): number {
  return video.frameCount / video.fps;
}

/** Guards against a slow draft for one video landing on the form of another. */
let draftFor: string | null = null;

/**
 * Open the share screen for a video SkipFrame wrote — just now in the export panel, earlier from
 * the export bar, or from a finished row in the queue.
 *
 * A different video than last time starts a fresh form; the same one keeps what was typed; and a
 * share still running is simply shown again rather than replaced.
 */
export function openShareDialog(video: ExportedVideo): void {
  if (share.phase !== 'sharing' && share.video?.outputPath !== video.outputPath) {
    share.video = video;
    share.title = '';
    share.description = '';
    share.phase = 'form';
    share.post = null;
    share.error = null;
    share.errorCode = null;
    share.linkCopied = false;

    draftFor = video.outputPath;
    void draftCaptionFor(video).then((draft) => {
      // Only fill a form that is still for this video and that nobody has typed into.
      if (draftFor !== video.outputPath || share.video?.outputPath !== video.outputPath) return;
      if (share.title === '') share.title = draft.title;
      if (share.description === '') share.description = draft.description;
    });
  }
  share.open = true;

  // A signed-in account already has its companies and limits; anything else needs reading.
  if (account.status !== 'signed-in' && account.status !== 'signing-in') void refreshAccount();
}

/**
 * The draft comes from the print the video was rendered from. That is the open file when it is
 * the same one; otherwise it is read again — a parse-cache hit, since the queue parsed it to
 * render it. A G-code that has since moved leaves only its name to go on.
 */
async function draftCaptionFor(video: ExportedVideo): Promise<CaptionDraft> {
  if (video.sourcePath && video.sourcePath === project.path && ir.value) {
    return draftCaption(ir.value.meta);
  }
  if (video.sourcePath) {
    try {
      return draftCaption((await parseFile(video.sourcePath)).ir.meta);
    } catch {
      // Fall through to the name.
    }
  }
  return { title: readableName(video.sourceName), description: '' };
}

export function closeShareDialog(): void {
  share.open = false;
  // A finished share has nothing left to show; the next one starts clean.
  if (share.phase === 'done') share.video = null;
}

export async function startShare(): Promise<void> {
  const video = share.video;
  const company = selectedCompany.value;
  if (!inTauri || !video || !company || share.phase === 'sharing' || blockers.value.length > 0) {
    return;
  }

  share.phase = 'sharing';
  share.error = null;
  share.errorCode = null;
  share.progress = { stage: 'preparing', sentBytes: 0, totalBytes: video.bytes };

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
        path: video.outputPath,
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
