import { ArrayBufferTarget, Muxer } from 'mp4-muxer';

/**
 * Phase-0 risk (c), the one that decides the export architecture: does `VideoEncoder` really
 * give us a hardware H.264 path inside the shipping WebView?
 *
 * Windows (WebView2/Chromium) is expected to work. macOS (WKWebView) is the unknown, and the
 * fallback if it does not is a thin VideoToolbox binding in Rust — also Apple's own encoder,
 * also zero licence surface. This harness is what decides that, and it is written to run
 * unchanged on both platforms.
 *
 * Frames are taken straight off the canvas with `new VideoFrame(canvas, { timestamp })`. There
 * is deliberately no `gl.readPixels` and no IPC round trip in this path.
 */

export interface CodecProbe {
  label: string;
  codec: string;
  hardwareAcceleration: HardwareAcceleration;
  supported: boolean;
  /** What the browser said it would actually give us, when it answered. */
  resolved?: string;
  error?: string;
}

export interface EncodeBenchResult {
  hasVideoEncoder: boolean;
  hasVideoFrame: boolean;
  probes: CodecProbe[];
  /** The configuration the timed run used, if any probe passed. */
  used?: CodecProbe;
  frames: number;
  width: number;
  height: number;
  fps: number;
  /** Frames encoded per second of wall-clock time. */
  encodeFps: number;
  encodeMs: number;
  muxMs: number;
  bytes: number;
  keyFrames: number;
  /** The muxed MP4, so the caller can write it out and prove it actually plays. */
  data?: Uint8Array;
  /** Populated when the run failed; the report is still returned. */
  error?: string;
}

type HardwareAcceleration = 'prefer-hardware' | 'prefer-software' | 'no-preference';

/**
 * Reels and Shorts are 1080x1920, and 1080p60 needs level 4.2 — the third byte of the codec
 * string, 0x2a. (0x28 would be level 4.0, which caps out below 1080p60.)
 */
const CANDIDATES: { label: string; codec: string; acceleration: HardwareAcceleration }[] = [
  { label: 'H.264 High 4.2, hardware', codec: 'avc1.64002a', acceleration: 'prefer-hardware' },
  { label: 'H.264 High 4.2, any', codec: 'avc1.64002a', acceleration: 'no-preference' },
  { label: 'H.264 Main 4.2, hardware', codec: 'avc1.4d002a', acceleration: 'prefer-hardware' },
  { label: 'H.264 Baseline 4.2, any', codec: 'avc1.42002a', acceleration: 'no-preference' },
];

export interface EncodeBenchOptions {
  width?: number;
  height?: number;
  fps?: number;
  frames?: number;
  bitrate?: number;
  /** Called before each frame is captured, so the scene can advance by frame index. */
  onFrame?: (frameIndex: number) => void;
}

export async function runEncodeBench(
  canvas: HTMLCanvasElement,
  options: EncodeBenchOptions = {},
): Promise<EncodeBenchResult> {
  const width = options.width ?? 1080;
  const height = options.height ?? 1920;
  const fps = options.fps ?? 60;
  const frames = options.frames ?? 120;
  const bitrate = options.bitrate ?? 14_000_000;

  const hasVideoEncoder = typeof globalThis.VideoEncoder !== 'undefined';
  const hasVideoFrame = typeof globalThis.VideoFrame !== 'undefined';

  const result: EncodeBenchResult = {
    hasVideoEncoder,
    hasVideoFrame,
    probes: [],
    frames: 0,
    width,
    height,
    fps,
    encodeFps: 0,
    encodeMs: 0,
    muxMs: 0,
    bytes: 0,
    keyFrames: 0,
  };

  if (!hasVideoEncoder || !hasVideoFrame) {
    result.error = 'WebCodecs is not available in this WebView.';
    return result;
  }

  for (const candidate of CANDIDATES) {
    const config: VideoEncoderConfig = {
      codec: candidate.codec,
      width,
      height,
      bitrate,
      framerate: fps,
      hardwareAcceleration: candidate.acceleration,
      avc: { format: 'avc' },
    };
    try {
      const support = await VideoEncoder.isConfigSupported(config);
      result.probes.push({
        label: candidate.label,
        codec: candidate.codec,
        hardwareAcceleration: candidate.acceleration,
        supported: !!support.supported,
        resolved: support.config ? JSON.stringify(support.config) : undefined,
      });
    } catch (e) {
      result.probes.push({
        label: candidate.label,
        codec: candidate.codec,
        hardwareAcceleration: candidate.acceleration,
        supported: false,
        error: String(e),
      });
    }
  }

  const chosen = result.probes.find((p) => p.supported);
  if (!chosen) {
    result.error = 'No H.264 encoder configuration was accepted.';
    return result;
  }
  result.used = chosen;

  const muxer = new Muxer({
    target: new ArrayBufferTarget(),
    video: { codec: 'avc', width, height, frameRate: fps },
    fastStart: 'in-memory',
  });

  let encoderError: string | undefined;
  let keyFrames = 0;
  let muxMs = 0;

  const encoder = new VideoEncoder({
    output: (chunk, meta) => {
      const t = performance.now();
      muxer.addVideoChunk(chunk, meta);
      muxMs += performance.now() - t;
      if (chunk.type === 'key') keyFrames += 1;
    },
    error: (e) => {
      encoderError = String(e);
    },
  });

  encoder.configure({
    codec: chosen.codec,
    width,
    height,
    bitrate,
    framerate: fps,
    hardwareAcceleration: chosen.hardwareAcceleration,
    avc: { format: 'avc' },
  });

  const started = performance.now();
  try {
    for (let i = 0; i < frames; i++) {
      options.onFrame?.(i);

      // Timestamps come from the frame index, never from the clock. This is what makes an
      // export reproducible and what keeps the muxed timeline gap-free.
      const frame = new VideoFrame(canvas, { timestamp: Math.round((i * 1e6) / fps) });
      encoder.encode(frame, { keyFrame: i % (fps * 2) === 0 });
      frame.close();

      // Do not let the encoder queue grow without bound on slower hardware.
      if (encoder.encodeQueueSize > 8) {
        await new Promise<void>((r) => {
          const wait = () => (encoder.encodeQueueSize > 4 ? setTimeout(wait, 1) : r());
          wait();
        });
      }
      if (encoderError) break;
    }

    await encoder.flush();
    muxer.finalize();
  } catch (e) {
    result.error = String(e);
  } finally {
    if (encoder.state !== 'closed') encoder.close();
  }

  result.encodeMs = performance.now() - started;
  result.muxMs = muxMs;
  result.frames = frames;
  result.keyFrames = keyFrames;
  const output = muxer.target.buffer;
  result.bytes = output?.byteLength ?? 0;
  if (output) result.data = new Uint8Array(output);
  result.encodeFps = frames / (result.encodeMs / 1000);
  if (encoderError && !result.error) result.error = encoderError;

  return result;
}
