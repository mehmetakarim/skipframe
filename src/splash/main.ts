import '@fontsource/space-grotesk/700.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/700.css';
import './splash.css';

import { listen } from '@tauri-apps/api/event';
import { getVersion } from '@tauri-apps/api/app';

/**
 * The splash window's own script: no Vue, no store, no renderer.
 *
 * Everything it shows comes from the main window, which reports what it is loading as it loads
 * it — the bar is the real state of the boot, not an animation that happens to take as long.
 */

export interface BootProgress {
  /** 0 to 1. */
  progress: number;
  /** Shown as written, in the design's uppercase mono. */
  stage: string;
}

const bar = document.getElementById('bar');
const stage = document.getElementById('stage');
const version = document.getElementById('version');

void listen<BootProgress>('skipframe://boot', ({ payload }) => {
  if (bar) bar.style.width = `${Math.round(Math.min(1, Math.max(0, payload.progress)) * 100)}%`;
  if (stage) stage.textContent = payload.stage;
});

void getVersion()
  .then((v) => {
    if (version) version.textContent = v;
  })
  .catch(() => {
    // The window is up either way; a missing version line is not worth an error.
  });
