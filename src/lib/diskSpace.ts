import { invoke } from '@tauri-apps/api/core';

import { decimal } from './format';
import type { Alert } from '../stores/alerts';

/**
 * The free space check made before a render starts.
 *
 * The estimate is a bitrate times a duration, and an encoder can overshoot it; the margin keeps
 * a render that would only just fit from failing on its last write. It is deliberately small —
 * refusing a render that would have fitted is the worse mistake, since the alert offers no way
 * past it except a smaller render or another folder.
 */
export const DISK_MARGIN = 1.15;

/** Free bytes on the volume that would hold `path`, or null when the system will not say. */
export async function freeSpace(path: string): Promise<number | null> {
  try {
    return await invoke<number>('free_space', { path });
  } catch {
    // A network share, a path that went away: no reason to stop the render over a check.
    return null;
  }
}

/** `6,4 GB`, or `840 MB` below a gigabyte. */
export function diskSize(bytes: number): string {
  return bytes >= 1e9 ? `${decimal(bytes / 1e9, 1)} GB` : `${decimal(bytes / 1e6, 0)} MB`;
}

export function diskSpaceAlert(
  needed: number,
  free: number,
  actions: { changeFolder: () => void | Promise<void>; halveScale?: () => void | Promise<void> },
): Alert {
  return {
    kind: 'warning',
    title: 'Disk alanı yetersiz',
    body: 'Bu render için gereken alan hedef diskte yok. Çıktı klasörünü değiştir ya da ölçeği düşür.',
    figures: [
      { label: 'gerekli', value: diskSize(needed) },
      { label: 'boş', value: diskSize(free) },
    ],
    primary: { label: 'Klasörü değiştir', run: actions.changeFolder },
    secondary: actions.halveScale
      ? { label: 'Ölçeği 0,5× yap', run: actions.halveScale }
      : undefined,
  };
}
