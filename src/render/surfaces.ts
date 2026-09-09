/**
 * Surface finishes, as the four numbers a shader needs.
 *
 * These are the design's four filament finishes. They are not physically based — the aim is that
 * matte PLA reads as matte and silk reads as silk in a nine-second clip, which is a smaller
 * problem than being right about microfacets.
 */

export interface SurfaceFinish {
  /** Strength of the specular highlight. */
  specular: number;
  /** Blinn-Phong exponent: high is a tight highlight, low is a broad sheen. */
  shininess: number;
  /** Edge brightening, which is most of what makes silk and metal read as themselves. */
  rim: number;
  /**
   * How much the highlight takes the filament's own colour. Plastics keep a white highlight;
   * metals tint theirs, which is the main thing that separates the two.
   */
  tint: number;
}

export const SURFACE_FINISHES: Record<string, SurfaceFinish> = {
  'matte-pla': { specular: 0.06, shininess: 8, rim: 0.06, tint: 0 },
  'glossy-pla': { specular: 0.45, shininess: 46, rim: 0.16, tint: 0 },
  silk: { specular: 0.6, shininess: 14, rim: 0.34, tint: 0.35 },
  metallic: { specular: 0.85, shininess: 64, rim: 0.5, tint: 1 },
};

export function finishFor(surface: string): SurfaceFinish {
  return SURFACE_FINISHES[surface] ?? SURFACE_FINISHES['matte-pla']!;
}

export interface LightRig {
  /** Key light direction, degrees around the print and above the plate. */
  azimuthDeg: number;
  elevationDeg: number;
  intensity: number;
  /** Flat light from everywhere, so nothing goes fully black. */
  ambient: number;
  /** A dimmer light from the opposite side, which keeps the shadow side readable. */
  fill: number;
}

/** Turn azimuth and elevation into a unit vector in the scene's Y-up world. */
export function lightDirection(azimuthDeg: number, elevationDeg: number): [number, number, number] {
  const a = (azimuthDeg * Math.PI) / 180;
  const e = (elevationDeg * Math.PI) / 180;
  const cosE = Math.cos(e);
  return [cosE * Math.sin(a), Math.sin(e), cosE * Math.cos(a)];
}
