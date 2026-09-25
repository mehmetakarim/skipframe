import '@fontsource/space-grotesk/700.css';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/700.css';
import './about.css';

import { getVersion, getTauriVersion } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import { openUrl } from '@tauri-apps/plugin-opener';

/**
 * The about window, without Vue: it is four lines of text, two buttons and a file.
 *
 * The licence list is 1.3 MB of other people's text, bundled with the app so it can be read on a
 * plane. It opens in this same window rather than a third one — the window grows, the panel is
 * swapped, and "Geri" puts it back.
 */

const PANEL = { width: 420, height: 400 } as const;
const LICENCES = { width: 680, height: 640 } as const;

const about = document.getElementById('about');
const licenceView = document.getElementById('licence-view');
const licenceText = document.getElementById('licence-text');

async function show(which: 'about' | 'licences') {
  const licences = which === 'licences';
  if (about) about.hidden = licences;
  if (licenceView) licenceView.hidden = !licences;
  const size = licences ? LICENCES : PANEL;
  try {
    const window = getCurrentWindow();
    await window.setSize(new LogicalSize(size.width, size.height));
    await window.center();
  } catch {
    // Not in a window that can resize itself; the panel still swaps.
  }
}

document.getElementById('licences')?.addEventListener('click', () => {
  void show('licences');
  if (licenceText && licenceText.dataset.loaded !== 'yes') {
    void invoke<string>('third_party_licenses')
      .then((text) => {
        licenceText.textContent = text;
        licenceText.dataset.loaded = 'yes';
      })
      .catch((e: unknown) => {
        licenceText.textContent = `Lisans listesi okunamadı.\n\n${String(e)}`;
      });
  }
});

document.getElementById('back')?.addEventListener('click', () => void show('about'));

document.getElementById('notes')?.addEventListener('click', () => {
  void openUrl('https://github.com/mehmetakarim/skipframe/releases');
});

window.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  if (licenceView && !licenceView.hidden) void show('about');
  else void getCurrentWindow().close();
});

async function fillFacts() {
  const build = document.getElementById('build');
  const stack = document.getElementById('stack');
  const [version, tauri, buildNumber] = await Promise.all([
    getVersion().catch(() => null),
    getTauriVersion().catch(() => null),
    invoke<string>('build_number').catch(() => null),
  ]);
  if (build) {
    build.textContent = version
      ? `Sürüm ${version}${buildNumber ? ` · build ${buildNumber}` : ''}`
      : 'Sürüm bilinmiyor';
  }
  if (stack && tauri) stack.textContent = `Tauri ${tauri} · Vue 3`;
}

void fillFacts();
