import { computed, reactive } from 'vue';

import { errorCode, errorText } from '../lib/messages';
import { notify, notifyError } from './notices';

/**
 * The StepperSkip account.
 *
 * All of the work — the browser sign-in, the tokens, the credential store, the HTTP — happens
 * in Rust (`src-tauri/src/stepperskip`). This module only holds what the interface shows, and
 * never sees a token: none crosses the IPC boundary in either direction.
 *
 * Sharing is for users with an active StepperSkip company profile. That rule is StepperSkip's to
 * enforce and it does, on every upload and publish. The interface reads `companies` to explain
 * the situation up front; it does not make the decision itself.
 */

export interface StepperSkipUser {
  id: number;
  username: string;
  displayName: string;
  avatarUrl: string | null;
  profileUrl: string | null;
}

export interface StepperSkipCompany {
  id: number;
  name: string;
  slug: string;
  logoUrl: string | null;
  profileUrl: string | null;
  isActive: boolean;
  canPublish: boolean;
}

export interface UploadLimits {
  maxFileSizeBytes: number;
  maxVideoDurationSeconds: number;
  preferredChunkSizeBytes: number;
  allowedMimeTypes: string[];
}

interface AccountSnapshot {
  user: StepperSkipUser;
  companies: StepperSkipCompany[];
  limits: UploadLimits;
  capabilities: { resumableUpload: boolean; companyVideoPublish: boolean };
}

export type AccountStatus =
  /** Not asked yet. */
  | 'unknown'
  /** This build has no StepperSkip server, or this is a browser tab. */
  | 'unconfigured'
  | 'signed-out'
  /** Waiting on the system browser. */
  | 'signing-in'
  /** A session is stored; fetching who it belongs to. */
  | 'loading'
  | 'signed-in'
  /** A session is stored but StepperSkip could not be reached. Nothing has been forgotten. */
  | 'unreachable';

export const account = reactive({
  status: 'unknown' as AccountStatus,
  user: null as StepperSkipUser | null,
  companies: [] as StepperSkipCompany[],
  selectedCompanyId: null as number | null,
  limits: null as UploadLimits | null,
  /** The last failure to reach StepperSkip, already in Turkish, for the card to show. */
  problem: null as string | null,
});

export const selectedCompany = computed(
  () => account.companies.find((c) => c.id === account.selectedCompanyId) ?? null,
);

/** "Mert Kaya" -> "MK". Turkish casing, so "ilker" becomes "İ", not "I". */
export function initialsOf(user: StepperSkipUser | null): string {
  const name = user?.displayName.trim() || user?.username || '';
  const letters = name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word[0] ?? '')
    .join('');
  return letters ? letters.toLocaleUpperCase('tr-TR') : '—';
}

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(command, args);
}

/**
 * Work out where the account stands. Reads the credential store first — no network — and only
 * talks to StepperSkip if a session is actually stored.
 */
export async function refreshAccount(): Promise<void> {
  if (!inTauri) {
    account.status = 'unconfigured';
    return;
  }
  if (account.status === 'signing-in') return;

  const status = await call<{ configured: boolean; hasCredential: boolean }>('ss_status').catch(
    (e) => {
      notifyError('StepperSkip hesabı okunamadı', e);
      return null;
    },
  );
  if (!status) return;
  if (!status.configured) {
    account.status = 'unconfigured';
    return;
  }
  if (!status.hasCredential) {
    forget();
    return;
  }

  account.status = 'loading';
  try {
    adopt(await call<AccountSnapshot>('ss_account'));
  } catch (e) {
    handleSessionFailure(e);
  }
}

/**
 * Whether this build has StepperSkip, and whether a session is stored — without talking to the
 * server. Enough to decide whether to offer sharing at all; reading the account itself waits
 * until someone actually goes to share.
 */
export async function loadAccountStatus(): Promise<void> {
  if (!inTauri) {
    account.status = 'unconfigured';
    return;
  }
  if (account.status !== 'unknown') return;
  try {
    const status = await call<{ configured: boolean; hasCredential: boolean }>('ss_status');
    if (!status.configured) account.status = 'unconfigured';
    else if (!status.hasCredential) forget();
  } catch {
    // Left as unknown; opening the share screen reads the account properly.
  }
}

export async function signIn(): Promise<void> {
  if (!inTauri || account.status === 'signing-in') return;
  account.status = 'signing-in';
  account.problem = null;
  try {
    adopt(await call<AccountSnapshot>('ss_sign_in'));
  } catch (e) {
    forget();
    // Pressing "Vazgeç" is not a failure worth a notice.
    if (errorCode(e) !== 'sign_in_cancelled') notifyError('StepperSkip girişi tamamlanmadı', e);
  }
}

export function cancelSignIn(): void {
  if (inTauri) void call('ss_cancel_sign_in');
}

/** `quiet` when signing out is only the first half of switching accounts. */
export async function signOut(options: { quiet?: boolean } = {}): Promise<void> {
  if (!inTauri) return;
  try {
    await call('ss_sign_out');
    forget();
    if (!options.quiet) notify({ title: 'StepperSkip hesabı kaldırıldı' });
  } catch (e) {
    notifyError('StepperSkip hesabı kaldırılamadı', e);
  }
}

/** The page where a user without a company profile opens one. */
export async function openCompanySetup(): Promise<void> {
  await call('ss_open_page', { page: 'company-setup' }).catch((e) =>
    notifyError('StepperSkip açılamadı', e),
  );
}

/** A profile or company page StepperSkip itself returned. */
export async function openStepperSkipUrl(url: string): Promise<void> {
  await call('ss_open_page', { page: { url } }).catch((e) =>
    notifyError('StepperSkip açılamadı', e),
  );
}

function adopt(snapshot: AccountSnapshot): void {
  account.user = snapshot.user;
  account.companies = snapshot.companies;
  account.limits = snapshot.limits;
  account.problem = null;

  // One company — the only case StepperSkip allows today — is chosen for the user. With more,
  // keep a previous choice if it still exists, and otherwise make them pick.
  const usable = snapshot.companies.filter((c) => c.canPublish);
  if (usable.length === 1) {
    account.selectedCompanyId = usable[0]!.id;
  } else if (!usable.some((c) => c.id === account.selectedCompanyId)) {
    account.selectedCompanyId = null;
  }

  account.status = 'signed-in';
}

function forget(): void {
  account.status = 'signed-out';
  account.user = null;
  account.companies = [];
  account.selectedCompanyId = null;
  account.limits = null;
}

function handleSessionFailure(e: unknown): void {
  const code = errorCode(e);
  if (code === 'session_expired' || code === 'not_signed_in') {
    forget();
    notify({
      kind: 'warning',
      title: 'StepperSkip oturumunun süresi doldu',
      detail: 'Paylaşmak için yeniden giriş yapman gerekiyor.',
    });
    return;
  }
  // The session is still stored. Say what went wrong and let the user try again.
  account.status = 'unreachable';
  account.problem = errorText(e);
}
