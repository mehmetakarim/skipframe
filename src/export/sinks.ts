import { ArrayBufferTarget, Muxer } from 'mp4-muxer';

import { pickH264Config } from './h264';
import { writeExportFile } from './writeFile';

/**
 * Where rendered frames go.
 *
 * Two implementations, and the difference between them is the whole point of having the
 * interface: the MP4 sink depends on the operating system having an H.264 encoder, and the frame
 * sequence sink depends on nothing at all. When WebCodecs is missing or refuses the resolution,
 * the second one still produces something the user can edit.
 */
export interface FrameSink {
  /** Human-readable description of what is being produced, for the progress panel. */
  readonly label: string;
  /** Called once before the first frame. */
  open(): Promise<void>;
  /** Called for every frame, in order, with the canvas already drawn. */
  write(canvas: HTMLCanvasElement, frame: number): Promise<void>;
  /** Finish and write to disk. Returns the number of bytes produced. */
  close(): Promise<number>;
  /** Give up without writing anything. */
  abort(): void;
}

export class UnsupportedCodecError extends Error {}

/** How long the encoder may sit on a full queue without handing back a single chunk. */
const STALL_TIMEOUT_MS = 15_000;

/** The encoder accepted frames and then stopped producing output, with no error of its own. */
export class EncoderStalledError extends UnsupportedCodecError {
  constructor(frame: number) {
    super(
      `H.264 kodlayıcı ${frame + 1}. karede yanıt vermeyi bıraktı. ` +
        'Kare sekansı olarak dışa aktarabilirsin.',
    );
  }
}

/**
 * H.264 in MP4, encoded by the operating system through `VideoEncoder`.
 *
 * Frames are taken with `new VideoFrame(canvas, { timestamp })` — no `readPixels`, no IPC round
 * trip per frame. Timestamps come from the frame index, so the muxed timeline is exact and the
 * same input always produces the same output.
 */
export class Mp4Sink implements FrameSink {
  label = 'MP4 · H.264';

  private muxer: Muxer<ArrayBufferTarget> | null = null;
  private encoder: VideoEncoder | null = null;
  private error: string | null = null;
  private frames = 0;
  private readonly path: string;
  private readonly width: number;
  private readonly height: number;
  private readonly fps: number;

  constructor(path: string, width: number, height: number, fps: number) {
    this.path = path;
    this.width = width;
    this.height = height;
    this.fps = fps;
  }

  async open(): Promise<void> {
    const choice = await pickH264Config(this.width, this.height, this.fps);
    if (!choice) {
      throw new UnsupportedCodecError(
        `Bu sistem ${this.width}×${this.height} boyutunda H.264 kodlamayı kabul etmiyor. ` +
          'Kare sekansı olarak dışa aktarabilirsin.',
      );
    }
    this.label = `MP4 · ${choice.label}`;

    this.muxer = new Muxer({
      target: new ArrayBufferTarget(),
      video: { codec: 'avc', width: this.width, height: this.height, frameRate: this.fps },
      // Puts the index at the front of the file, which is what makes an upload start playing
      // before it has finished transferring.
      fastStart: 'in-memory',
    });

    this.encoder = new VideoEncoder({
      // An exception thrown here is swallowed by the encoder, and the muxer then fails at
      // finalize with an error that names neither the chunk nor the cause. Keep the real one.
      output: (chunk, meta) => {
        try {
          this.muxer?.addVideoChunk(chunk, bt709Limited(meta));
        } catch (e) {
          this.error ??= String(e);
        }
      },
      error: (e) => {
        this.error = String(e);
      },
    });
    this.encoder.configure(choice.config);
  }

  async write(canvas: HTMLCanvasElement, frame: number): Promise<void> {
    if (this.error) throw new Error(this.error);
    const encoder = this.encoder;
    if (!encoder) throw new Error('encoder is not open');

    // The duration is stated, not left to the encoder: Chromium fills it in on the way out, but
    // WebKit hands back chunks with `duration: null`, which the muxer refuses outright.
    const frameObject = new VideoFrame(canvas, {
      timestamp: Math.round((frame * 1e6) / this.fps),
      duration: Math.round(1e6 / this.fps),
    });
    // A keyframe every two seconds: enough for scrubbing, not enough to bloat the file.
    encoder.encode(frameObject, { keyFrame: frame % (this.fps * 2) === 0 });
    this.frames = frame + 1;
    frameObject.close();

    // Let the encoder drain rather than queueing every frame of a long export at once. An
    // encoder that stops draining — or dies, which WebKit reports by closing it — must end the
    // export with an error rather than leave the progress panel waiting forever.
    if (encoder.encodeQueueSize > 8) {
      await new Promise<void>((resolve, reject) => {
        let lastSize = encoder.encodeQueueSize;
        let lastProgress = performance.now();
        const wait = () => {
          if (this.error) return reject(new Error(this.error));
          if (encoder.state === 'closed') return reject(new Error('encoder closed unexpectedly'));
          const size = encoder.encodeQueueSize;
          if (size <= 4) return resolve();
          const now = performance.now();
          if (size < lastSize) {
            lastSize = size;
            lastProgress = now;
          } else if (now - lastProgress > STALL_TIMEOUT_MS) {
            return reject(new EncoderStalledError(frame));
          }
          setTimeout(wait, 1);
        };
        wait();
      });
    }
  }

  async close(): Promise<number> {
    if (!this.encoder || !this.muxer) return 0;
    // The same stall can hide here on a short export that never filled the queue.
    const flushed = await Promise.race([
      this.encoder.flush().then(() => true),
      new Promise<false>((resolve) => setTimeout(() => resolve(false), STALL_TIMEOUT_MS)),
    ]);
    if (!flushed) {
      this.abort();
      throw new EncoderStalledError(Math.max(0, this.frames - 1));
    }
    this.encoder.close();
    this.muxer.finalize();
    if (this.error) throw new Error(this.error);

    const buffer = this.muxer.target.buffer;
    if (!buffer) throw new Error('muxer produced no output');
    await writeExportFile(this.path, new Uint8Array(buffer));
    return buffer.byteLength;
  }

  abort(): void {
    if (this.encoder && this.encoder.state !== 'closed') this.encoder.close();
    this.encoder = null;
    this.muxer = null;
  }
}

/**
 * What the muxer writes into the MP4's `colr` box: limited-range BT.709, stated rather than
 * repeated back from the encoder.
 *
 * The H.264 these encoders produce carries no colour description of its own — no
 * `video_signal_type` in the SPS, checked on every file this app has written — so the `colr` box
 * is the only thing that says how to read the picture, and both platforms get it wrong in their
 * own way.
 *
 * **Range.** Every encoder behind WebCodecs here writes limited-range video: luma 16–235.
 * WebView2 reports that honestly. WKWebView reports `fullRange: true` for the very same kind of
 * stream — measured on macOS 27 by encoding a black-to-white ramp and reading 16..235 back — and
 * a player that trusts the container then stretches 16–235 as if it were 0–255: greyed blacks,
 * dimmed whites, a flat, washed-out video.
 *
 * **Transfer.** WebView2 labelled the same render `bt709` in September and `iec61966-2-1` (sRGB)
 * after a runtime update, with nothing changed here. The two curves part company in exactly the
 * tones this app renders most: at code 8 the sRGB reading is a third of the BT.709 one, at 32 it
 * is half, and by 200 they agree. A colour-managed player — macOS honours the tag, most Windows
 * players ignore it — therefore showed the same video differently on the two platforms.
 *
 * So the box is written, not echoed: BT.709 primaries, transfer and matrix, limited range. The
 * frames come from an sRGB canvas and nothing converts them, which makes `sRGB` the more literal
 * label; BT.709 is what SDR video means by convention and what every platform assumes of an
 * upload, and one answer on both platforms is worth more here than a distinction players
 * disagree about.
 */
function bt709Limited(
  meta: EncodedVideoChunkMetadata | undefined,
): EncodedVideoChunkMetadata | undefined {
  const config = meta?.decoderConfig;
  if (!config) return meta;
  return {
    ...meta,
    decoderConfig: {
      ...config,
      colorSpace: {
        primaries: 'bt709',
        transfer: 'bt709',
        matrix: 'bt709',
        fullRange: false,
      },
    },
  };
}

/**
 * A numbered PNG per frame. Needs no codec, so it works everywhere and is the answer whenever
 * the MP4 path is unavailable — and it is what an editor wants anyway.
 */
export class FrameSequenceSink implements FrameSink {
  readonly label = 'PNG kare sekansı';

  private bytes = 0;
  private cancelled = false;
  private readonly dir: string;
  private readonly stem: string;
  private readonly digits: number;

  constructor(dir: string, stem: string, frameCount: number) {
    this.dir = dir.replace(/[/\\]+$/, '');
    this.stem = stem;
    this.digits = Math.max(4, String(frameCount).length);
  }

  async open(): Promise<void> {}

  async write(canvas: HTMLCanvasElement, frame: number): Promise<void> {
    if (this.cancelled) return;
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
    if (!blob) throw new Error(`frame ${frame} could not be encoded as PNG`);

    const bytes = new Uint8Array(await blob.arrayBuffer());
    const name = `${this.stem}_${String(frame + 1).padStart(this.digits, '0')}.png`;
    await writeExportFile(`${this.dir}/${name}`, bytes);
    this.bytes += bytes.byteLength;
  }

  async close(): Promise<number> {
    return this.bytes;
  }

  abort(): void {
    this.cancelled = true;
  }
}
