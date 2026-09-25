/**
 * What the splash window is told while the app starts.
 *
 * The main window is hidden until this says it is ready, so every path out of here has to end in
 * `bootDone` — including the ones that failed. A window that never appears is worse than one
 * that appears with an error in it.
 */

/** The splash is up for at least this long; a 200 ms flash of a logo reads as a glitch. */
const MINIMUM_MS = 700;

const startedAt = performance.now();
const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function bootStage(progress: number, stage: string): Promise<void> {
  if (!inTauri) return;
  try {
    const { emit } = await import('@tauri-apps/api/event');
    await emit('skipframe://boot', { progress, stage });
  } catch {
    // The splash is decoration; nothing about the app depends on it hearing this.
  }
}

/** Show the main window and close the splash. */
export async function bootDone(): Promise<void> {
  if (!inTauri) return;
  await bootStage(1, 'HAZIR');
  const left = MINIMUM_MS - (performance.now() - startedAt);
  if (left > 0) await new Promise((resolve) => setTimeout(resolve, left));
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('app_ready');
  } catch {
    // Rust shows the window on a timer as well, so a failure here costs a short wait, not the
    // whole app.
  }
}
