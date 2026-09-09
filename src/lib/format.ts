/**
 * Number formatting for the interface.
 *
 * The design writes numbers Turkish-style — `14,8 MB`, `0,20 mm`, `1 284` — so the locale is
 * fixed here rather than taken from the host, which would otherwise change the look of the app
 * depending on whose machine it runs on.
 */

const LOCALE = 'tr-TR';

export function decimal(value: number, digits = 1): string {
  return new Intl.NumberFormat(LOCALE, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(value);
}

export function integer(value: number): string {
  return new Intl.NumberFormat(LOCALE).format(Math.round(value));
}

export function megabytes(bytes: number): string {
  return `${decimal(bytes / 1e6, 1)} MB`;
}

export function millimetres(mm: number, digits = 2): string {
  return `${decimal(mm, digits)} mm`;
}

export function metres(mm: number): string {
  return `${decimal(mm / 1000, 2)} m`;
}

export function grams(g: number): string {
  return `${decimal(g, 1)} g`;
}

/** `1s 46dk` — the design's shorthand for a print time. */
export function printDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.round((total % 3600) / 60);
  if (h === 0) return `${m}dk`;
  return `${h}s ${m}dk`;
}

/** `60×31×48` — the print's bounding box, rounded to whole millimetres. */
export function boundsSize(bounds: readonly number[]): string {
  const x = Math.round((bounds[3] ?? 0) - (bounds[0] ?? 0));
  const y = Math.round((bounds[4] ?? 0) - (bounds[1] ?? 0));
  const z = Math.round((bounds[5] ?? 0) - (bounds[2] ?? 0));
  return `${x}×${y}×${z}`;
}
