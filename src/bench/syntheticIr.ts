import type { Ir } from '../ir/types';

/**
 * An IR of the right shape and size, built in the browser.
 *
 * The render bench needs a body to draw; where it came from does not change what is being
 * measured. Using this instead of a parsed file lets the bench run in a plain browser tab as
 * well as inside the app, which is how the same harness runs on a Mac that has no fixtures.
 *
 * This is not an SFIR decoder and must never become one — the binary layout has exactly one
 * reader, `decodeIr`.
 */
export function makeSyntheticIr(layers: number, segmentsPerLayer: number): Ir {
  const segmentCount = layers * segmentsPerLayer;
  const positions = new Float32Array(segmentCount * 6);
  const featureType = new Uint8Array(segmentCount);
  const toolIndex = new Uint8Array(segmentCount);
  const width = new Float32Array(segmentCount);
  const layerStart = new Uint32Array(layers + 1);
  const layerZ: number[] = [];

  let s = 0;
  let px = 125;
  let py = 105;
  for (let l = 0; l < layers; l++) {
    layerStart[l] = s;
    const z = 0.2 + l * 0.2;
    layerZ.push(z);
    for (let i = 0; i < segmentsPerLayer; i++) {
      const a = i * 0.05 + l * 0.01;
      const r = 20 + ((i * 0.01) % 40);
      const x = 125 + r * Math.cos(a);
      const y = 105 + r * Math.sin(a);
      const o = s * 6;
      positions[o] = px;
      positions[o + 1] = py;
      positions[o + 2] = z;
      positions[o + 3] = x;
      positions[o + 4] = y;
      positions[o + 5] = z;
      px = x;
      py = y;
      // A travel at the start of the layer, then feature bands that change every 20 paths.
      featureType[s] = i === 0 ? 0 : 1 + ((((i / 20) | 0) % 4) as number);
      toolIndex[s] = 0;
      width[s] = featureType[s] === 0 ? 0 : 0.45;
      s += 1;
    }
  }
  layerStart[layers] = segmentCount;

  return {
    positions,
    layerStart,
    featureType,
    toolIndex,
    width,
    buffer: positions.buffer as ArrayBuffer,
    segmentCount,
    layerCount: layers,
    meta: {
      dialect: 'Synthetic',
      slicerVersion: null,
      printerModel: 'Prusa MK4',
      printerProfile: 'prusa-mk4',
      bedSize: [250, 210],
      bedOrigin: [0, 0],
      estimatedTimeS: null,
      filamentGrams: null,
      filamentMm: null,
      layerCount: layers,
      segmentCount,
      layerZ,
      bounds: [65, 45, 0.2, 185, 165, 0.2 + layers * 0.2],
      toolCount: 1,
      nozzleDiameter: 0.4,
      hasFeatureTypes: true,
      hasDeclaredWidth: true,
      warnings: [],
      sourceName: 'synthetic',
      sourceBytes: null,
      plate: null,
    },
  };
}
