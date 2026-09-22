/**
 * Turkish for the codes the Rust side sends — from the parser and from StepperSkip.
 *
 * The Rust crate speaks English: it is an MIT library with a CLI, and that is the right
 * language for both. What crosses the IPC boundary is therefore a stable code —
 * `skipframe_gcode::Error::code` and `Warning::code` — with the English text alongside it. This
 * file is the only place that turns one into something the user reads, and the only file a
 * second language would need.
 *
 * StepperSkip follows the same rule: its errors carry a code, and those codes are passed through
 * unchanged. The ones listed here were read from StepperSkip's source, not its written handoff.
 *
 * Every lookup falls back to the message that came with the code. For the parser that is
 * English — an unknown code means a build mismatch or an old cache entry, and an English
 * sentence still beats a bare identifier. StepperSkip writes its own messages in Turkish, so a
 * code this file has not heard of yet still reads correctly.
 */

const ERRORS: Record<string, string> = {
  no_moves: 'Dosyada basılabilir hareket yok. Bu bir G-code çıktısı değil ya da boş.',
  archive: 'Arşiv okunamadı. Dosya bozuk ya da geçerli bir .3mf değil.',
  no_gcode_in_archive: 'Arşivin içinde plaka G-code’u bulunamadı.',
  unsupported_container: 'Bu dosya biçimi desteklenmiyor.',
  model_file: 'Bu bir 3D model, dilimlenmiş G-code değil. Önce slicer’da dilimle.',
  truncated: 'Dosya baskı bitmeden kesilmiş. Aktarma yarıda kalmış olabilir.',
  io: 'Dosya okunamadı',
  worker: 'Okuma işlemi tamamlanamadı.',

  // -- StepperSkip: reaching it -------------------------------------------------------------
  stepperskip_unconfigured: 'Bu sürümde StepperSkip bağlantısı yapılandırılmamış.',
  network: 'StepperSkip’e ulaşılamadı. Bağlantını ya da sunucunun çalıştığını kontrol et.',
  timeout: 'StepperSkip zamanında yanıt vermedi.',
  bad_response: 'StepperSkip’ten beklenmeyen bir yanıt geldi.',
  rate_limited: 'StepperSkip’e çok sık istek gönderildi. Biraz bekleyip tekrar dene.',
  server_error: 'StepperSkip tarafında beklenmeyen bir hata oluştu.',
  method_not_allowed: 'StepperSkip’ten beklenmeyen bir yanıt geldi.',

  // -- StepperSkip: signing in --------------------------------------------------------------
  sign_in_in_progress: 'Tarayıcıda zaten bekleyen bir giriş var.',
  sign_in_cancelled: 'Giriş iptal edildi.',
  sign_in_timeout: 'Tarayıcıdan dönülmedi; giriş zaman aşımına uğradı.',
  state_mismatch: 'Giriş yanıtı bu isteğe ait değil, güvenlik için durduruldu. Tekrar dene.',
  invalid_callback: 'Tarayıcıdan eksik bir giriş yanıtı geldi. Tekrar dene.',
  browser_open_failed: 'Sistem tarayıcısı açılamadı.',
  loopback: 'Giriş dönüşü için yerel bağlantı açılamadı.',
  random: 'Güvenli rastgele değer üretilemedi.',
  access_denied: 'StepperSkip’te SkipFrame’e izin verilmedi.',
  invalid_client: 'StepperSkip bu SkipFrame sürümünü tanımıyor.',
  invalid_scope: 'StepperSkip istenen izinleri vermedi.',
  unsupported_response_type: 'StepperSkip giriş isteğini geçersiz buldu.',
  invalid_request: 'StepperSkip isteği geçersiz buldu.',
  invalid_grant: 'Giriş kodu geçersiz ya da süresi dolmuş. Tekrar dene.',

  // -- StepperSkip: the session -------------------------------------------------------------
  credential_store:
    'Oturum bilgisi işletim sisteminin anahtar deposuna yazılamadı ya da okunamadı.',
  not_signed_in: 'StepperSkip hesabına giriş yapılmamış.',
  session_expired: 'StepperSkip oturumunun süresi doldu. Yeniden giriş yap.',
  invalid_token: 'StepperSkip oturumu geçersiz. Yeniden giriş yap.',
  insufficient_scope: 'StepperSkip oturumunun bu işlem için izni yok. Yeniden giriş yap.',

  // -- StepperSkip: company profile ---------------------------------------------------------
  company_required: 'Paylaşım için StepperSkip’te aktif bir firma profili gerekli.',
  company_forbidden: 'Bu firma profilinde paylaşım yetkin yok ya da profil aktif değil.',

  // -- Sharing: before anything is sent -----------------------------------------------------
  not_mp4: 'Yalnızca MP4 videolar paylaşılabilir.',
  file_read: 'Video dosyası okunamadı. Taşınmış ya da silinmiş olabilir.',
  share_in_progress: 'Zaten paylaşılmakta olan bir video var.',
  share_cancelled: 'Paylaşım iptal edildi.',

  // -- Sharing: the upload --------------------------------------------------------------------
  upload_too_large: 'Video, StepperSkip’in kabul ettiği boyuttan büyük.',
  invalid_media_type: 'StepperSkip yalnızca MP4 video kabul ediyor.',
  quota_exceeded: 'Yarım kalmış çok fazla yüklemen var. Biraz sonra tekrar dene.',
  upload_not_found: 'Yükleme oturumu bulunamadı.',
  upload_expired: 'Yükleme oturumunun süresi doldu. Paylaşımı yeniden başlat.',
  upload_failed: 'Yükleme tamamlanamadı. Paylaşımı yeniden başlat.',
  upload_completed: 'Bu yükleme zaten tamamlanmış.',
  upload_offset_conflict: 'Yükleme sırası karıştı ve toparlanamadı. Tekrar dene.',
  upload_state_mismatch: 'StepperSkip yüklemenin bir kısmını kaybetti. Tekrar dene.',
  missing_content_range: 'Yükleme isteği eksik gönderildi.',
  invalid_content_range: 'Yükleme isteği geçersiz bir aralık içeriyordu.',
  chunk_too_large: 'Yükleme parçası StepperSkip’in sınırını aştı.',
  size_mismatch: 'StepperSkip’e ulaşan video boyutu beklenenle uyuşmadı.',
  write_incomplete: 'Video parçası StepperSkip’e eksik ulaştı.',
  checksum_mismatch:
    'StepperSkip’e ulaşan video diskteki dosyayla aynı değil, bu yüzden yayımlanmadı. Tekrar dene.',

  // -- Sharing: StepperSkip's checks on the finished video ------------------------------------
  invalid_mp4_container: 'StepperSkip videoyu geçerli bir MP4 olarak tanımadı.',
  video_too_long: 'Video, StepperSkip’in izin verdiği süreden uzun.',
  duration_validation_failed: 'StepperSkip videonun süresini doğrulayamadı.',
  media_validation_unavailable:
    'StepperSkip şu an videoları doğrulayamıyor. Daha sonra tekrar dene.',

  // -- Sharing: publishing --------------------------------------------------------------------
  upload_not_completed: 'Yükleme bitmeden yayımlanamaz.',
  upload_forbidden: 'Bu yüklemeyi yayımlama yetkin yok.',
  upload_invalid: 'Yüklenen video yayımlanabilir durumda değil.',
  media_missing: 'Yüklenen video StepperSkip’te bulunamadı.',
  upload_already_published: 'Bu video başka bir firma profilinde zaten yayımlanmış.',
  publication_failed: 'Gönderi oluşturulamadı. Tekrar dene.',
  post_not_found: 'Gönderi bulunamadı.',
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
  /** Particulars beyond the code — the line a truncated file stops at. Absent when none. */
  detail?: Record<string, unknown>;
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
/** The code of a failed command, if it carried one. */
export function errorCode(e: unknown): string | null {
  return isIpcError(e) ? e.code : null;
}

/** The English message of a failed command, or whatever else was thrown, as a string. */
export function errorMessage(e: unknown): string {
  if (isIpcError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** One number from a failed command's detail, if it is there. */
export function errorDetailNumber(e: unknown, key: string): number | null {
  const value = isIpcError(e) ? e.detail?.[key] : undefined;
  return typeof value === 'number' ? value : null;
}

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
