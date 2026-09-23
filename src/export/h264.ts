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
 *
 * `isConfigSupported` is not the last word. WKWebView answers `true` for High and Main and then
 * holds on to every frame: no chunk comes back, `encodeQueueSize` climbs, `flush()` never settles
 * on a long run, and no error is raised. Measured on macOS 27 (WebKit 605.1.15); Baseline works
 * there, and so does `latencyMode: 'realtime'`, but realtime is allowed to drop frames — it
 * dropped 7 of 120 in the same run — which would break the frame-for-frame determinism export
 * promises. So a candidate is only taken once it has handed back output while encoding, and on
 * macOS that lands on Baseline.
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

  const key = `${width}x${height}@${fps}/${bitrate}`;
  const known = verified.get(key);
  if (known) return known;

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
          if (support.supported && (await encodesForReal(config))) {
            const how = acceleration === 'prefer-hardware' ? 'donanım' : 'yazılım';
            const choice = { config, label: `H.264 ${profile.name} ${level.name}, ${how}` };
            verified.set(key, choice);
            return choice;
          }
        } catch {
          // An unsupported string throws rather than answering; try the next one.
        }
      }
    }
  }
  return null;
}

/**
 * What a size has already been proven to encode at, for the life of the page. The trial encode
 * below costs a couple of seconds on WebKit for every profile that stalls, and a batch queue
 * would otherwise pay it once per job.
 */
const verified = new Map<string, H264Choice>();

/**
 * Frames pushed through a candidate before trusting it. A working encoder hands chunks back as it
 * goes; the stalled one holds every frame and only a `flush()` shakes the first few loose, which
 * is why the test waits for output without flushing.
 */
const TRIAL_FRAMES = 10;
/** How long a candidate gets to hand back its first chunk. Baseline answers in milliseconds. */
const TRIAL_TIMEOUT_MS = 1500;

/** Encode a few blank frames and report whether anything came out without being flushed. */
async function encodesForReal(config: VideoEncoderConfig): Promise<boolean> {
  const canvas = document.createElement('canvas');
  canvas.width = config.width;
  canvas.height = config.height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return false;

  let chunks = 0;
  let failed = false;
  const encoder = new VideoEncoder({
    output: () => {
      chunks += 1;
    },
    error: () => {
      failed = true;
    },
  });

  try {
    encoder.configure(config);
    const fps = config.framerate ?? 30;
    for (let i = 0; i < TRIAL_FRAMES; i++) {
      ctx.fillStyle = i % 2 ? '#202020' : '#101010';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      const frame = new VideoFrame(canvas, { timestamp: Math.round((i * 1e6) / fps) });
      encoder.encode(frame, { keyFrame: i === 0 });
      frame.close();
    }
    const deadline = performance.now() + TRIAL_TIMEOUT_MS;
    while (chunks === 0 && !failed && performance.now() < deadline) {
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    return chunks > 0 && !failed;
  } catch {
    return false;
  } finally {
    if (encoder.state !== 'closed') encoder.close();
  }
}
