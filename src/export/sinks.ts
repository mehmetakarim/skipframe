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
      output: (chunk, meta) => this.muxer?.addVideoChunk(chunk, meta),
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

    const frameObject = new VideoFrame(canvas, {
      timestamp: Math.round((frame * 1e6) / this.fps),
    });
    // A keyframe every two seconds: enough for scrubbing, not enough to bloat the file.
    encoder.encode(frameObject, { keyFrame: frame % (this.fps * 2) === 0 });
    frameObject.close();

    // Let the encoder drain rather than queueing every frame of a long export at once.
    if (encoder.encodeQueueSize > 8) {
      await new Promise<void>((resolve) => {
        const wait = () => (encoder.encodeQueueSize > 4 ? setTimeout(wait, 1) : resolve());
        wait();
      });
    }
  }

  async close(): Promise<number> {
    if (!this.encoder || !this.muxer) return 0;
    await this.encoder.flush();
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
