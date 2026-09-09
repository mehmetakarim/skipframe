import * as THREE from 'three';

import type { Ir } from '../ir/types';

/**
 * The extrusion bead, as instanced geometry.
 *
 * Every segment becomes a short prism with a flat-topped hexagonal cross-section — the shape a
 * bead of plastic actually makes when it is squashed onto the layer below. That is what gives
 * the print a surface, and a surface is what light needs.
 *
 * # Why instancing
 *
 * A prism per segment as merged geometry would be twelve vertices and thirty-six indices each:
 * at 785 000 segments that is 9.4 million vertices and about 220 MB of buffers. As instanced
 * geometry the prism is uploaded **once** and each segment contributes only its endpoints, its
 * width and its feature — and all three of those are already in the IR, so they are bound as
 * views onto the parser's own buffer with no copy at all. The geometry the GPU holds is
 * therefore the same 23 MB it held when the print was drawn as lines.
 *
 * # What this changes about the animation
 *
 * The locked decision was one merged geometry animated by `setDrawRange`. `setDrawRange` counts
 * indices within a single instance, so it cannot express "the first n segments"; the equivalent
 * for instanced geometry is `instanceCount`. Everything the rule was protecting still holds —
 * one geometry, one draw call, one scalar advancing the animation, no per-layer meshes and no
 * rebuilds. Only the name of the scalar changed.
 */

/**
 * Cross-section, counter-clockwise, on a unit bead: `u` runs across the extrusion width and `v`
 * across the layer height, both in -0.5..0.5. Six points give a flat top and rounded sides;
 * four would put a ridge down the middle of every bead and eight buys nothing you can see.
 */
const CROSS_SECTION: [number, number][] = [
  [0.5, 0],
  [0.25, 0.433],
  [-0.25, 0.433],
  [-0.5, 0],
  [-0.25, -0.433],
  [0.25, -0.433],
];

const SIDES = CROSS_SECTION.length;

/**
 * Build the instanced geometry for a print.
 *
 * `position` carries the cross-section coordinate as `(u, v, t)` where `t` picks the near or far
 * end of the segment, and `normal` carries the cross-section direction as `(cos, sin, 0)`. Both
 * are Three's own attribute names so its shader prelude declares them for us.
 */
export function buildBeadGeometry(ir: Ir): THREE.InstancedBufferGeometry {
  const geometry = new THREE.InstancedBufferGeometry();

  const corners = new Float32Array(SIDES * 2 * 3);
  const normals = new Float32Array(SIDES * 2 * 3);
  for (let end = 0; end < 2; end++) {
    for (let i = 0; i < SIDES; i++) {
      const [u, v] = CROSS_SECTION[i]!;
      const o = (end * SIDES + i) * 3;
      corners[o] = u;
      corners[o + 1] = v;
      corners[o + 2] = end;
      // The direction around the cross-section; the shader corrects it for the bead's real
      // width-to-height ratio, which is per segment and so cannot be baked in here.
      const angle = Math.atan2(v, u);
      normals[o] = Math.cos(angle);
      normals[o + 1] = Math.sin(angle);
      normals[o + 2] = 0;
    }
  }

  const indices: number[] = [];
  for (let i = 0; i < SIDES; i++) {
    const next = (i + 1) % SIDES;
    // Counter-clockwise seen from outside, so back-face culling keeps the outside.
    indices.push(i, next, SIDES + i);
    indices.push(next, SIDES + next, SIDES + i);
  }

  geometry.setAttribute('position', new THREE.Float32BufferAttribute(corners, 3));
  geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3));
  geometry.setIndex(indices);

  // Per segment, all three straight off the IR buffer.
  const endpoints = new THREE.InstancedInterleavedBuffer(ir.positions, 6, 1);
  geometry.setAttribute('aStart', new THREE.InterleavedBufferAttribute(endpoints, 3, 0));
  geometry.setAttribute('aEnd', new THREE.InterleavedBufferAttribute(endpoints, 3, 3));
  geometry.setAttribute('aWidth', new THREE.InstancedBufferAttribute(ir.width, 1));
  geometry.setAttribute('aFeature', new THREE.InstancedBufferAttribute(ir.featureType, 1));

  geometry.instanceCount = 0;
  return geometry;
}

/** Nominal layer height, from the gaps between layer Z values. Falls back to a sane default. */
export function layerHeightOf(ir: Ir): number {
  const zs = ir.meta.layerZ;
  if (zs.length < 2) return 0.2;
  // The median gap, so one odd first layer or one variable-height stretch cannot skew it.
  const gaps: number[] = [];
  for (let i = 1; i < zs.length; i++) {
    const gap = (zs[i] ?? 0) - (zs[i - 1] ?? 0);
    if (gap > 1e-4 && gap < 2) gaps.push(gap);
  }
  if (gaps.length === 0) return 0.2;
  gaps.sort((a, b) => a - b);
  return gaps[gaps.length >> 1] ?? 0.2;
}
