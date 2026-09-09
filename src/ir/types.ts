/**
 * The SkipFrame intermediate representation, as it exists on the JavaScript side.
 *
 * Every array here is a view onto the single ArrayBuffer that came back from the Rust parser.
 * Nothing is copied and nothing is re-serialised. The canonical description of the binary
 * layout lives in `crates/skipframe-gcode/src/ir.rs`; the two must be changed together.
 */

export const FeatureType = {
  Travel: 0,
  OuterWall: 1,
  InnerWall: 2,
  SolidInfill: 3,
  SparseInfill: 4,
  Support: 5,
  SkirtBrim: 6,
  Bridge: 7,
  TopSurface: 8,
  Unknown: 9,
} as const;

export type FeatureTypeValue = (typeof FeatureType)[keyof typeof FeatureType];

export const FEATURE_LABELS: Record<number, string> = {
  [FeatureType.Travel]: 'Travel',
  [FeatureType.OuterWall]: 'Outer wall',
  [FeatureType.InnerWall]: 'Inner wall',
  [FeatureType.SolidInfill]: 'Solid infill',
  [FeatureType.SparseInfill]: 'Sparse infill',
  [FeatureType.Support]: 'Support',
  [FeatureType.SkirtBrim]: 'Skirt / brim',
  [FeatureType.Bridge]: 'Bridge',
  [FeatureType.TopSurface]: 'Top surface',
  [FeatureType.Unknown]: 'Other',
};

/** Mirrors `skipframe_gcode::ir::Meta`. */
export interface IrMeta {
  dialect: string;
  slicerVersion: string | null;
  printerModel: string | null;
  printerProfile: string | null;
  bedSize: [number, number] | null;
  bedOrigin: [number, number] | null;
  estimatedTimeS: number | null;
  filamentGrams: number | null;
  filamentMm: number | null;
  layerCount: number;
  segmentCount: number;
  layerZ: number[];
  bounds: [number, number, number, number, number, number];
  toolCount: number;
  nozzleDiameter: number | null;
  hasFeatureTypes: boolean;
  hasDeclaredWidth: boolean;
  warnings: string[];
  sourceName: string;
  plate: number | null;
}

/**
 * Structure of arrays. `positions` is per vertex (two per segment, so it uploads straight into a
 * BufferAttribute); `featureType`, `toolIndex` and `width` are per segment.
 *
 * `layerStart[i]` is the segment index where layer `i` begins, and the array carries a trailing
 * sentinel equal to `segmentCount` — so layer `i` is `layerStart[i] .. layerStart[i + 1]`.
 */
export interface Ir {
  /** `segmentCount * 6` floats: x,y,z of the segment start then of its end. Z-up, millimetres. */
  positions: Float32Array;
  /** `layerCount + 1` segment offsets. */
  layerStart: Uint32Array;
  /** `segmentCount` entries. */
  featureType: Uint8Array;
  /** `segmentCount` entries. Populated but not yet visualised. */
  toolIndex: Uint8Array;
  /** `segmentCount` entries, millimetres. Populated but not yet visualised. */
  width: Float32Array;
  meta: IrMeta;
  /** The backing buffer, kept alive so the views stay valid. */
  buffer: ArrayBuffer;
  segmentCount: number;
  layerCount: number;
}
