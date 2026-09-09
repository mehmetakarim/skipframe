import type { Ir, IrMeta } from './types';

/** `SFIR`, little endian. */
const MAGIC = 0x52494653;
const VERSION = 1;
const HEADER_LEN = 64;

export class IrFormatError extends Error {}

/**
 * Wrap the raw parser output in TypedArray views. No copying, no JSON round trip — the buffer
 * that arrives over IPC is the buffer the renderer uploads to the GPU.
 */
export function decodeIr(buffer: ArrayBuffer): Ir {
  if (buffer.byteLength < HEADER_LEN) {
    throw new IrFormatError(`IR buffer is ${buffer.byteLength} bytes, too short to hold a header`);
  }
  const head = new DataView(buffer);

  if (head.getUint32(0, true) !== MAGIC) {
    throw new IrFormatError('IR buffer does not start with the SFIR magic');
  }
  const version = head.getUint16(4, true);
  if (version !== VERSION) {
    throw new IrFormatError(
      `IR format v${version} is not supported by this build (expected v${VERSION})`,
    );
  }

  const segmentCount = head.getUint32(8, true);
  const layerCount = head.getUint32(12, true);
  const positionsOff = head.getUint32(16, true);
  const layerStartOff = head.getUint32(20, true);
  const featureOff = head.getUint32(24, true);
  const toolOff = head.getUint32(28, true);
  const widthOff = head.getUint32(32, true);
  const metaOff = head.getUint32(36, true);
  const metaLen = head.getUint32(40, true);
  const totalLen = head.getUint32(44, true);

  if (totalLen !== buffer.byteLength) {
    throw new IrFormatError(`IR buffer claims ${totalLen} bytes but is ${buffer.byteLength}`);
  }

  const meta = JSON.parse(
    new TextDecoder().decode(new Uint8Array(buffer, metaOff, metaLen)),
  ) as IrMeta;

  return {
    positions: new Float32Array(buffer, positionsOff, segmentCount * 6),
    layerStart: new Uint32Array(buffer, layerStartOff, layerCount + 1),
    featureType: new Uint8Array(buffer, featureOff, segmentCount),
    toolIndex: new Uint8Array(buffer, toolOff, segmentCount),
    width: new Float32Array(buffer, widthOff, segmentCount),
    meta,
    buffer,
    segmentCount,
    layerCount,
  };
}

/** Vertex draw count for showing layers `0 .. layer` inclusive. Drives `setDrawRange`. */
export function vertexCountThroughLayer(ir: Ir, layer: number): number {
  const clamped = Math.max(0, Math.min(layer, ir.layerCount - 1));
  return (ir.layerStart[clamped + 1] ?? ir.segmentCount) * 2;
}

/** First and last vertex of one layer, for highlighting the layer currently being printed. */
export function layerVertexRange(ir: Ir, layer: number): [number, number] {
  const clamped = Math.max(0, Math.min(layer, ir.layerCount - 1));
  const start = (ir.layerStart[clamped] ?? 0) * 2;
  const end = (ir.layerStart[clamped + 1] ?? ir.segmentCount) * 2;
  return [start, end];
}
