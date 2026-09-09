import { FeatureType } from '../ir/types';

/**
 * Feature colours live in a 16x1 RGB texture that the vertex shader samples with the segment's
 * feature id. Keeping the palette out of the vertex buffer means a colour change costs a 48-byte
 * texture upload instead of rewriting tens of megabytes of attributes.
 */
export const PALETTE_SIZE = 16;

export type Palette = Record<number, [number, number, number]>;

const hex = (h: string): [number, number, number] => [
  parseInt(h.slice(1, 3), 16),
  parseInt(h.slice(3, 5), 16),
  parseInt(h.slice(5, 7), 16),
];

/**
 * The default look follows the design: printed material is a single quiet grey, and colour is
 * spent only where it carries meaning. Feature colouring is opt-in.
 */
export const MONOCHROME: Palette = {
  [FeatureType.Travel]: hex('#3f4441'),
  [FeatureType.OuterWall]: hex('#c9ccc6'),
  [FeatureType.InnerWall]: hex('#c9ccc6'),
  [FeatureType.SolidInfill]: hex('#c9ccc6'),
  [FeatureType.SparseInfill]: hex('#c9ccc6'),
  [FeatureType.Support]: hex('#c9ccc6'),
  [FeatureType.SkirtBrim]: hex('#c9ccc6'),
  [FeatureType.Bridge]: hex('#c9ccc6'),
  [FeatureType.TopSurface]: hex('#c9ccc6'),
  [FeatureType.Unknown]: hex('#c9ccc6'),
};

/** Feature colouring, for when the user wants to read the print rather than watch it. */
export const BY_FEATURE: Palette = {
  [FeatureType.Travel]: hex('#3f4441'),
  [FeatureType.OuterWall]: hex('#ebb60e'),
  [FeatureType.InnerWall]: hex('#8a6a1e'),
  [FeatureType.SolidInfill]: hex('#c9ccc6'),
  [FeatureType.SparseInfill]: hex('#7c817b'),
  [FeatureType.Support]: hex('#5a1c48'),
  [FeatureType.SkirtBrim]: hex('#1c3f5a'),
  [FeatureType.Bridge]: hex('#1e6b33'),
  [FeatureType.TopSurface]: hex('#e7e8e4'),
  [FeatureType.Unknown]: hex('#8a8f88'),
};

/** Colour of the layer currently being laid down. */
export const CURRENT_LAYER = hex('#ebb60e');

export function paletteToTextureData(palette: Palette): Uint8Array {
  const data = new Uint8Array(PALETTE_SIZE * 4);
  for (let i = 0; i < PALETTE_SIZE; i++) {
    const rgb = palette[i] ?? [255, 0, 255];
    data[i * 4 + 0] = rgb[0];
    data[i * 4 + 1] = rgb[1];
    data[i * 4 + 2] = rgb[2];
    data[i * 4 + 3] = 255;
  }
  return data;
}
