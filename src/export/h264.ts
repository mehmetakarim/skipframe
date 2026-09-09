/**
 * Choosing an H.264 configuration the WebView will actually accept.
 *
 * The codec string's third byte is the level, and the level caps both frame size and macroblock
 * rate. A 2x render scale on a 9:16 canvas is 2160x3840, which no 4.x level allows, so the level
 * has to follow the resolution rather than being a constant.
 *
 * Profiles are tried High first — it is what every social platform expects and what gives the
 * best quality per bit — then Main, then Baseline. Hardware is preferred but never required:
 * `no-preference` lets the WebView fall back to its software encoder rather than failing.
 */

export interface H264Choice {
  config: VideoEncoderConfig;
  /** For the UI and the logs: "High 5.1, hardware". */
  label: string;
}

const PROFILES: { name: string; prefix: string }[] = [
  { name: 'High', prefix: '6400' },
  { name: 'Main', prefix: '4d00' },
  { name: 'Baseline', prefix: '4200' },
];

/** level_idc as it appears in the codec string, paired with the frame size it covers. */
const LEVELS: { name: string; hex: string; maxPixels: number }[] = [
  { name: '4.2', hex: '2a', maxPixels: 1920 * 1088 },
  { name: '5.0', hex: '32', maxPixels: 2560 * 1600 },
  { name: '5.1', hex: '33', maxPixels: 3840 * 2160 },
  { name: '5.2', hex: '34', maxPixels: 4096 * 2304 },
];

/**
 * Bits per pixel per frame. 0.15 puts a 1080x1920 clip at roughly 9 Mbit/s at 30 fps, which is
 * where Reels and Shorts stop re-compressing aggressively.
 */
const BITS_PER_PIXEL = 0.15;
const MAX_BITRATE = 40_000_000;

export function defaultBitrate(width: number, height: number, fps: number): number {
  return Math.min(MAX_BITRATE, Math.round(width * height * fps * BITS_PER_PIXEL));
}

/**
 * Ask the WebView which configuration it will take, cheapest-to-decode last.
 * Returns null when it will not encode H.264 at this size at all.
 */
export async function pickH264Config(
  width: number,
  height: number,
  fps: number,
  bitrate = defaultBitrate(width, height, fps),
): Promise<H264Choice | null> {
  if (typeof globalThis.VideoEncoder === 'undefined') return null;

  const pixels = width * height;
  const levels = LEVELS.filter((l) => pixels <= l.maxPixels);
  if (levels.length === 0) return null;

  for (const acceleration of ['prefer-hardware', 'no-preference'] as const) {
    for (const level of levels) {
      for (const profile of PROFILES) {
        const config: VideoEncoderConfig = {
          codec: `avc1.${profile.prefix}${level.hex}`,
          width,
          height,
          bitrate,
          framerate: fps,
          hardwareAcceleration: acceleration,
          avc: { format: 'avc' },
        };
        try {
          const support = await VideoEncoder.isConfigSupported(config);
          if (support.supported) {
            const how = acceleration === 'prefer-hardware' ? 'donanım' : 'yazılım';
            return { config, label: `H.264 ${profile.name} ${level.name}, ${how}` };
          }
        } catch {
          // An unsupported string throws rather than answering; try the next one.
        }
      }
    }
  }
  return null;
}
