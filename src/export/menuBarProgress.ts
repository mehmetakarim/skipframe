import { invoke } from '@tauri-apps/api/core';

/**
 * The render counter in the macOS menu bar.
 *
 * A render takes minutes and the window is usually behind something else by then, so the count
 * and the time left go up next to the clock. Rust decides whether there is a menu bar to put
 * them in; on Windows these calls land on a no-op rather than being guarded here.
 *
 * Sent at most a few times a second: the renderer reports every frame, and 30 IPC calls a second
 * to move a number by one would be a strange use of the time.
 */

const MIN_GAP_MS = 400;

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
let lastSentAt = 0;

export function reportRenderProgress(frame: number, frameCount: number, etaS: number | null): void {
  if (!inTauri) return;
  const now = performance.now();
  if (now - lastSentAt < MIN_GAP_MS) return;
  lastSentAt = now;
  void invoke('render_progress', { progress: { frame, frameCount, etaS } }).catch(() => {
    // The counter is a convenience; a render must not fail because a menu bar would not take it.
  });
}

/** Take the counter down — the render finished, failed or was cancelled. */
export function clearRenderProgress(): void {
  if (!inTauri) return;
  lastSentAt = 0;
  void invoke('render_progress', { progress: null }).catch(() => {});
}
