import { reactive, watch } from 'vue';

import { notify, notifyError } from './notices';

/**
 * Preferences, loaded once at boot and written back whenever they change.
 *
 * The write is debounced because these are edited with sliders and switches, and a settings
 * file rewritten on every keystroke would be a strange thing to do to someone's disk.
 */

export interface Settings {
  outputDir: string | null;
  dateInFilename: boolean;
  watchDir: string | null;
  watchEnabled: boolean;
  ffmpegPath: string | null;
  autoUpdate: boolean;
  lastUpdateCheck: number | null;
  /** Colours saved from the colour picker, newest first. */
  colourLibrary: string[];
}

export interface FfmpegInfo {
  path: string;
  version: string;
}

export const settings = reactive<Settings>({
  outputDir: null,
  dateInFilename: false,
  watchDir: null,
  watchEnabled: false,
  ffmpegPath: null,
  autoUpdate: true,
  lastUpdateCheck: null,
  colourLibrary: [],
});

export const settingsState = reactive({
  loaded: false,
  /** What `probe_ffmpeg` last found, or null if there is no FFmpeg to be had. */
  ffmpeg: null as FfmpegInfo | null,
  ffmpegChecked: false,
  /** Last thing the updater said, for the line under the check button. */
  updateMessage: null as string | null,
  checkingUpdate: false,
  /** The version waiting to be installed, once a check has found one. */
  updateReady: null as string | null,
  /** 0 to 1 while it downloads, null when nothing is downloading. */
  updateProgress: null as number | null,
  watchError: null as string | null,
});

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

let saveTimer: ReturnType<typeof setTimeout> | null = null;
let suppressSave = false;

export async function loadSettings(): Promise<void> {
  if (!inTauri) {
    settingsState.loaded = true;
    return;
  }
  const { invoke } = await import('@tauri-apps/api/core');
  const stored = await invoke<Settings>('load_settings');

  suppressSave = true;
  Object.assign(settings, stored);
  suppressSave = false;
  settingsState.loaded = true;

  await probeFfmpeg();
  if (settings.watchEnabled && settings.watchDir) await startWatching();

  // After the rest of the boot, and never in the way of it: an update is not urgent.
  setTimeout(() => void checkForUpdateInBackground(), 4000);
}

function scheduleSave() {
  if (!inTauri || suppressSave || !settingsState.loaded) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    try {
      await invoke('save_settings', { settings: { ...settings } });
    } catch (e) {
      // Swallowing this meant a read-only config directory looked exactly like a working one
      // until the app was restarted and every preference had gone back to its default.
      notifyError('Tercihler kaydedilemedi', e);
    }
  }, 400);
}

watch(settings, scheduleSave, { deep: true });

// -- output ---------------------------------------------------------------------------------

export async function pickOutputDir(): Promise<void> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const dir = await open({ directory: true, multiple: false, title: 'Çıktı klasörü' });
  if (typeof dir !== 'string') return;
  settings.outputDir = dir;
}

/** `benchy` becomes `benchy_2026-09-10` when the date suffix is on. */
export function decorateStem(stem: string): string {
  if (!settings.dateInFilename) return stem;
  const now = new Date();
  const date = [
    now.getFullYear(),
    String(now.getMonth() + 1).padStart(2, '0'),
    String(now.getDate()).padStart(2, '0'),
  ].join('-');
  return `${stem}_${date}`;
}

// -- FFmpeg ---------------------------------------------------------------------------------

export async function probeFfmpeg(): Promise<void> {
  if (!inTauri) return;
  const { invoke } = await import('@tauri-apps/api/core');
  settingsState.ffmpeg = await invoke<FfmpegInfo | null>('probe_ffmpeg', {
    path: settings.ffmpegPath,
  }).catch(() => null);
  settingsState.ffmpegChecked = true;
}

export async function pickFfmpeg(): Promise<void> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const file = await open({ multiple: false, title: 'FFmpeg' });
  if (typeof file !== 'string') return;
  settings.ffmpegPath = file;
  await probeFfmpeg();
}

export async function clearFfmpegPath(): Promise<void> {
  settings.ffmpegPath = null;
  await probeFfmpeg();
}

// -- watched folder ---------------------------------------------------------------------------

export async function pickWatchDir(): Promise<void> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const dir = await open({ directory: true, multiple: false, title: 'İzlenecek klasör' });
  if (typeof dir !== 'string') return;
  settings.watchDir = dir;
  if (settings.watchEnabled) await startWatching();
}

export async function setWatchEnabled(on: boolean): Promise<void> {
  settings.watchEnabled = on;
  if (on) await startWatching();
  else await stopWatching();
}

async function startWatching(): Promise<void> {
  if (!inTauri || !settings.watchDir) return;
  const { invoke } = await import('@tauri-apps/api/core');

  settingsState.watchError = null;
  try {
    await invoke('start_watch', { path: settings.watchDir });
  } catch (e) {
    settingsState.watchError = String(e);
    settings.watchEnabled = false;
  }
  // What happens to a file that appears is watchBridge's business, not this module's.
}

async function stopWatching(): Promise<void> {
  if (!inTauri) return;
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('stop_watch').catch(() => {});
}

// -- updates -----------------------------------------------------------------------------------

/**
 * What a check found, kept so that installing it does not have to ask the server again — the
 * plugin's own handle carries the download URL and the signature it will verify against.
 */
let pending: Awaited<ReturnType<typeof checkUpdate>> | null = null;

async function checkUpdate() {
  const { check } = await import('@tauri-apps/plugin-updater');
  return check();
}

/** `quiet` is the check made at startup: it says nothing when there is nothing to say. */
export async function checkForUpdate(options: { quiet?: boolean } = {}): Promise<void> {
  if (!inTauri || settingsState.checkingUpdate || settingsState.updateProgress !== null) return;
  settingsState.checkingUpdate = true;
  if (!options.quiet) settingsState.updateMessage = null;
  try {
    const update = await checkUpdate();
    pending = update;
    settingsState.updateReady = update?.version ?? null;
    if (update) {
      settingsState.updateMessage = `Sürüm ${update.version} hazır`;
    } else if (!options.quiet) {
      settingsState.updateMessage = 'Güncel sürümü kullanıyorsun';
    }
  } catch (e) {
    if (!options.quiet) settingsState.updateMessage = `Kontrol edilemedi — ${String(e)}`;
  } finally {
    settings.lastUpdateCheck = Math.floor(Date.now() / 1000);
    settingsState.checkingUpdate = false;
  }
}

/**
 * Download the update the last check found, install it, and restart into it.
 *
 * Never on its own: installing replaces the running program and takes the window with it, so a
 * render or an upload in progress would be thrown away. The user presses the button, and even
 * then a busy app says no rather than doing it anyway.
 */
export async function installUpdate(): Promise<void> {
  if (!inTauri || !pending || settingsState.updateProgress !== null) return;

  const busy = await busyReason();
  if (busy) {
    settingsState.updateMessage = busy;
    return;
  }

  settingsState.updateProgress = 0;
  settingsState.updateMessage = 'İndiriliyor…';
  let downloaded = 0;
  let total = 0;
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        total = event.data.contentLength ?? 0;
      } else if (event.event === 'Progress') {
        downloaded += event.data.chunkLength;
        settingsState.updateProgress = total > 0 ? Math.min(1, downloaded / total) : null;
      } else if (event.event === 'Finished') {
        settingsState.updateProgress = 1;
        settingsState.updateMessage = 'Kuruluyor…';
      }
    });
    settingsState.updateMessage = 'Yeniden başlatılıyor…';
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
  } catch (e) {
    settingsState.updateProgress = null;
    settingsState.updateMessage = `Güncellenemedi — ${String(e)}`;
    notifyError('Güncelleme tamamlanamadı', e);
  }
}

/** Why now is a bad moment, or null when it is not. */
async function busyReason(): Promise<string | null> {
  const [{ running }, { queue }, { share }] = await Promise.all([
    import('./exportJob'),
    import('./queue'),
    import('./share'),
  ]);
  if (running.value) return 'Render sürerken güncellenemez; bitince tekrar dene.';
  if (queue.running) return 'Kuyruk çalışırken güncellenemez; bitince tekrar dene.';
  if (share.phase === 'sharing') return 'Paylaşım sürerken güncellenemez; bitince tekrar dene.';
  return null;
}

/**
 * The quiet check at startup, for "Otomatik güncelle".
 *
 * It downloads nothing and installs nothing: it puts a notice in the corner with a way to the
 * button. An app that restarted itself while someone was working would be a worse app than one
 * that is a version behind.
 */
export async function checkForUpdateInBackground(): Promise<void> {
  if (!inTauri || !settings.autoUpdate) return;
  await checkForUpdate({ quiet: true });
  if (!settingsState.updateReady) return;
  notify({
    kind: 'info',
    title: `SkipFrame ${settingsState.updateReady} hazır`,
    detail: 'Ayarlar → Güncelleme bölümünden kurabilirsin.',
    action: { label: 'Ayarlar’a git', run: () => void goToSettings() },
  });
}

async function goToSettings(): Promise<void> {
  const { goTo } = await import('./ui');
  goTo('settings');
}

/** "2 sa önce", for the line beside the check button. */
export function sinceLastCheck(): string {
  if (!settings.lastUpdateCheck) return 'hiç kontrol edilmedi';
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - settings.lastUpdateCheck);
  if (seconds < 60) return 'az önce kontrol edildi';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `son kontrol ${minutes} dk önce`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `son kontrol ${hours} sa önce`;
  return `son kontrol ${Math.floor(hours / 24)} gün önce`;
}

// -- saved colours --------------------------------------------------------------------------

/** How many saved colours the picker keeps. Two rows of its seven-wide grid. */
const LIBRARY_LIMIT = 14;

/**
 * Save a colour, newest first.
 *
 * Saving one that is already there moves it to the front rather than adding it twice — pressing
 * the button again on a colour you already kept should not quietly do nothing.
 */
export function saveColour(hex: string): void {
  const colour = hex.toLowerCase();
  settings.colourLibrary = [
    colour,
    ...settings.colourLibrary.filter((c) => c.toLowerCase() !== colour),
  ].slice(0, LIBRARY_LIMIT);
}

export function forgetColour(hex: string): void {
  const colour = hex.toLowerCase();
  settings.colourLibrary = settings.colourLibrary.filter((c) => c.toLowerCase() !== colour);
}
