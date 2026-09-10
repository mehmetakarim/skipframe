import { reactive, watch } from 'vue';

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
});

export const settingsState = reactive({
  loaded: false,
  /** What `probe_ffmpeg` last found, or null if there is no FFmpeg to be had. */
  ffmpeg: null as FfmpegInfo | null,
  ffmpegChecked: false,
  /** Last thing the updater said, for the line under the check button. */
  updateMessage: null as string | null,
  checkingUpdate: false,
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
}

function scheduleSave() {
  if (!inTauri || suppressSave || !settingsState.loaded) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('save_settings', { settings: { ...settings } }).catch(() => {});
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

export async function checkForUpdate(): Promise<void> {
  if (!inTauri || settingsState.checkingUpdate) return;
  settingsState.checkingUpdate = true;
  settingsState.updateMessage = null;
  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    const update = await check();
    settingsState.updateMessage = update
      ? `Sürüm ${update.version} hazır`
      : 'Güncel sürümü kullanıyorsun';
  } catch (e) {
    settingsState.updateMessage = `Kontrol edilemedi — ${String(e)}`;
  } finally {
    settings.lastUpdateCheck = Math.floor(Date.now() / 1000);
    settingsState.checkingUpdate = false;
  }
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
