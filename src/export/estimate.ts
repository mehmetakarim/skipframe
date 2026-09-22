import { defaultBitrate } from './h264';
import type { ExportFormat } from './types';

/**
 * Roughly how many bytes a render will write.
 *
 * Shown next to the export button, and checked against the free space on the destination before
 * a render starts.
 */
export function estimateOutputBytes(
  width: number,
  height: number,
  fps: number,
  frameCount: number,
  format: ExportFormat,
): number {
  if (format === 'frames') {
    // PNG of a mostly flat render compresses hard; roughly a third of a byte per pixel.
    return width * height * 0.33 * frameCount;
  }
  return (defaultBitrate(width, height, fps) / 8) * (frameCount / fps);
}
