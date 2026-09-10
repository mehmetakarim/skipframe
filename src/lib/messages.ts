/**
 * Turkish for the codes the parser sends.
 *
 * The Rust crate speaks English: it is an MIT library with a CLI, and that is the right
 * language for both. What crosses the IPC boundary is therefore a stable code —
 * `skipframe_gcode::Error::code` and `Warning::code` — with the English text alongside it. This
 * file is the only place that turns one into something the user reads, and the only file a
 * second language would need.
 *
 * Every lookup falls back to the English the parser sent. An unknown code means either a build
 * mismatch or a cache entry written before codes existed, and in both cases the user is better
 * served by an English sentence than by a bare identifier.
 */

const ERRORS: Record<string, string> = {
  no_moves: 'Dosyada basılabilir hareket yok. Bu bir G-code çıktısı değil ya da boş.',
  archive: 'Arşiv okunamadı. Dosya bozuk ya da geçerli bir .3mf değil.',
  no_gcode_in_archive: 'Arşivin içinde plaka G-code’u bulunamadı.',
  unsupported_container: 'Bu dosya biçimi desteklenmiyor.',
  io: 'Dosya okunamadı',
  worker: 'Okuma işlemi tamamlanamadı.',
};

/**
 * Codes whose original message says something the Turkish sentence cannot.
 *
 * Only the OS errors qualify. Windows distinguishes "not found" from "access denied" and
 * writes both in the user's own language already, so replacing that with a flat "could not be
 * read" would be throwing away the useful half. A zip library's reason, by contrast, means
 * nothing to anyone opening a print.
 */
const WITH_ORIGINAL = new Set(['io']);

const WARNINGS: Record<string, string> = {
  unknown_dialect:
    'Dilimleyici tanınamadı. Geometri birebir doğru; katman ve bölüm renklendirmesi yaklaşık.',
  no_layer_markers: 'Katman değişimi yorumları yok. Katmanlar Z hareketinden çıkarıldı.',
  no_feature_markers: 'Bölüm türü yorumları yok. Duvar, dolgu ve destek aynı çiziliyor.',
  derived_width:
    'Bu dilimleyici ekstrüzyon genişliğini bildirmiyor. Genişlik E hareketi ve katman ' +
    'yüksekliğinden hesaplandı.',
  no_printer_profile: 'Yazıcı modeli tanınamadı. Genel bir tabla gösteriliyor.',
  arc_without_offsets: 'I/J değeri olmayan bir yay hareketi düz çizgi olarak çizildi.',
};

/** The shape a failed Tauri command answers with. */
export interface IpcError {
  code: string;
  message: string;
}

function isIpcError(e: unknown): e is IpcError {
  return (
    typeof e === 'object' &&
    e !== null &&
    typeof (e as IpcError).code === 'string' &&
    typeof (e as IpcError).message === 'string'
  );
}

/**
 * What to show the user for something that was thrown.
 *
 * Anything at all can arrive here — a coded parser failure, a plain `Error` from the front end,
 * a string from a Tauri plugin — and all of them have to come out as one readable line.
 */
export function errorText(e: unknown): string {
  if (isIpcError(e)) {
    const turkish = ERRORS[e.code];
    if (turkish === undefined) return e.message;
    return WITH_ORIGINAL.has(e.code) ? `${turkish}: ${e.message}` : turkish;
  }
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Turkish for one `meta.warnings` entry, or the entry itself if it is not a code we know. */
export function warningText(code: string): string {
  return WARNINGS[code] ?? code;
}
