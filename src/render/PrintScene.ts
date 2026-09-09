import * as THREE from 'three';

import type { Ir } from '../ir/types';
import {
  BY_FEATURE,
  CURRENT_LAYER,
  MONOCHROME,
  PALETTE_SIZE,
  paletteToTextureData,
  type Palette,
} from './palette';

/**
 * The print viewport.
 *
 * One merged `LineSegments` geometry holds the entire print. Animation is a single
 * `setDrawRange` call — no per-layer meshes, no geometry rebuilds, one draw call per frame.
 *
 * The scene is driven by a **layer index**, never by elapsed time. Export walks the same
 * `setLayer` path frame by frame, which is what makes a rendered video reproducible.
 */

// GLSL ES 3.00. Three supplies `position`, `modelViewMatrix` and `projectionMatrix`; everything
// else, including the fragment output, has to be declared here.
const VERTEX_SHADER = /* glsl */ `
  in float aFeature;

  uniform sampler2D uPalette;
  uniform float uShowTravel;
  uniform float uCurrentLayerStart;   // first vertex of the layer being printed
  uniform float uCurrentLayerEnd;     // one past its last vertex
  uniform vec3 uCurrentLayerColor;
  uniform float uHighlightCurrent;

  out vec3 vColor;

  void main() {
    float vertexId = float(gl_VertexID);
    vec3 base = texture(uPalette, vec2((aFeature + 0.5) / ${PALETTE_SIZE}.0, 0.5)).rgb;

    bool isTravel = aFeature < 0.5;
    bool isCurrent = uHighlightCurrent > 0.5
      && vertexId >= uCurrentLayerStart
      && vertexId < uCurrentLayerEnd
      && !isTravel;

    vColor = isCurrent ? uCurrentLayerColor : base;

    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);

    // Travels are hidden by collapsing them outside the clip volume rather than by splitting
    // the geometry, which would cost a second draw call and break the single draw range.
    if (isTravel && uShowTravel < 0.5) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    }
  }
`;

const FRAGMENT_SHADER = /* glsl */ `
  in vec3 vColor;

  out vec4 fragColor;

  void main() {
    fragColor = vec4(vColor, 1.0);
  }
`;

export interface PrintSceneOptions {
  /** Device pixel ratio cap. Export overrides this to 1 and drives the size itself. */
  maxPixelRatio?: number;
}

export class PrintScene {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = new THREE.Scene();
  readonly camera: THREE.PerspectiveCamera;

  private readonly root = new THREE.Group();
  private readonly bed = new THREE.Group();
  private material: THREE.ShaderMaterial | null = null;
  private geometry: THREE.BufferGeometry | null = null;
  private lines: THREE.LineSegments | null = null;
  private paletteTexture: THREE.DataTexture;

  private ir: Ir | null = null;
  private layer = 0;
  private maxPixelRatio: number;

  /** Orbit state, kept here so export can set an exact camera without a controls dependency. */
  private orbit = { azimuth: Math.PI * 0.25, elevation: Math.PI * 0.22, distance: 400 };
  private target = new THREE.Vector3();

  constructor(canvas: HTMLCanvasElement, options: PrintSceneOptions = {}) {
    this.maxPixelRatio = options.maxPixelRatio ?? 2;

    this.renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: true,
      // The export path reads frames straight off this canvas, so it must keep its contents.
      preserveDrawingBuffer: true,
      powerPreference: 'high-performance',
    });
    this.renderer.setClearColor(0x0b0b0b, 1);

    this.camera = new THREE.PerspectiveCamera(38, 1, 0.5, 5000);

    // G-code is Z up; Three.js is Y up. Rotating the container costs nothing and leaves the
    // position buffer exactly as the parser wrote it.
    this.root.rotation.x = -Math.PI / 2;
    this.scene.add(this.root);
    this.scene.add(this.bed);

    this.paletteTexture = new THREE.DataTexture(
      paletteToTextureData(MONOCHROME),
      PALETTE_SIZE,
      1,
      THREE.RGBAFormat,
    );
    this.paletteTexture.magFilter = THREE.NearestFilter;
    this.paletteTexture.minFilter = THREE.NearestFilter;
    this.paletteTexture.needsUpdate = true;
  }

  // -- data -----------------------------------------------------------------------------

  setIr(ir: Ir): void {
    this.disposeGeometry();
    this.ir = ir;

    const geometry = new THREE.BufferGeometry();
    // No copy: the attribute points straight at the buffer the parser produced.
    geometry.setAttribute('position', new THREE.BufferAttribute(ir.positions, 3));
    geometry.setAttribute('aFeature', new THREE.Uint8BufferAttribute(expandFeature(ir), 1));
    geometry.setDrawRange(0, 0);

    const material = new THREE.ShaderMaterial({
      // GLSL 3 is what makes `gl_VertexID` available, which is how the current layer is
      // highlighted without a six-megabyte per-vertex index attribute. WebGL2 is present in
      // both WebView2 and WKWebView on the platforms we ship.
      glslVersion: THREE.GLSL3,
      vertexShader: VERTEX_SHADER,
      fragmentShader: FRAGMENT_SHADER,
      uniforms: {
        uPalette: { value: this.paletteTexture },
        uShowTravel: { value: 0 },
        uCurrentLayerStart: { value: 0 },
        uCurrentLayerEnd: { value: 0 },
        uCurrentLayerColor: { value: new THREE.Color().setRGB(...normalised(CURRENT_LAYER)) },
        uHighlightCurrent: { value: 1 },
      },
    });

    this.geometry = geometry;
    this.material = material;
    this.lines = new THREE.LineSegments(geometry, material);
    // The bounding sphere would otherwise be computed from a 750k-segment buffer on every
    // frustum test; the camera always looks at the print, so culling buys nothing here.
    this.lines.frustumCulled = false;
    this.root.add(this.lines);

    this.buildBed(ir);
    this.frameToPrint(ir);
    this.setLayer(ir.layerCount - 1);
  }

  setPalette(palette: Palette): void {
    const data = this.paletteTexture.image.data as Uint8Array | null;
    if (!data) return;
    data.set(paletteToTextureData(palette));
    this.paletteTexture.needsUpdate = true;
  }

  setFeatureColouring(on: boolean): void {
    this.setPalette(on ? BY_FEATURE : MONOCHROME);
  }

  setShowTravel(on: boolean): void {
    if (this.material) this.material.uniforms.uShowTravel!.value = on ? 1 : 0;
  }

  setHighlightCurrentLayer(on: boolean): void {
    if (this.material) this.material.uniforms.uHighlightCurrent!.value = on ? 1 : 0;
  }

  // -- animation ------------------------------------------------------------------------

  /** Show layers `0 .. layer` inclusive. This is the only thing the animation ever changes. */
  setLayer(layer: number): void {
    const ir = this.ir;
    if (!ir || !this.geometry || !this.material) return;

    const clamped = Math.max(0, Math.min(Math.floor(layer), ir.layerCount - 1));
    this.layer = clamped;

    const start = (ir.layerStart[clamped] ?? 0) * 2;
    const end = (ir.layerStart[clamped + 1] ?? ir.segmentCount) * 2;

    this.geometry.setDrawRange(0, end);
    this.material.uniforms.uCurrentLayerStart!.value = start;
    this.material.uniforms.uCurrentLayerEnd!.value = end;
  }

  get currentLayer(): number {
    return this.layer;
  }

  // -- camera ---------------------------------------------------------------------------

  orbitBy(deltaAzimuth: number, deltaElevation: number): void {
    this.orbit.azimuth += deltaAzimuth;
    this.orbit.elevation = clamp(this.orbit.elevation + deltaElevation, -1.4, 1.4);
    this.applyCamera();
  }

  zoomBy(factor: number): void {
    this.orbit.distance = clamp(this.orbit.distance * factor, 20, 4000);
    this.applyCamera();
  }

  /** Exact camera placement, for deterministic export and for saved viewpoints. */
  setCamera(azimuth: number, elevation: number, distance: number): void {
    this.orbit = { azimuth, elevation, distance };
    this.applyCamera();
  }

  private applyCamera(): void {
    const { azimuth, elevation, distance } = this.orbit;
    const cosE = Math.cos(elevation);
    this.camera.position.set(
      this.target.x + distance * cosE * Math.sin(azimuth),
      this.target.y + distance * Math.sin(elevation),
      this.target.z + distance * cosE * Math.cos(azimuth),
    );
    this.camera.lookAt(this.target);
  }

  private frameToPrint(ir: Ir): void {
    const b = ir.meta.bounds;
    const bed = ir.meta.bedSize ?? [220, 220];
    // Bounds are in G-code space (Z up); the root group rotates them into Y up.
    const centreX = (b[0] + b[3]) / 2;
    const centreY = (b[1] + b[4]) / 2;
    const height = Math.max(b[5] - b[2], 1);
    const footprint = Math.max(b[3] - b[0], b[4] - b[1], bed[0] * 0.5, 1);

    this.target.set(centreX - bed[0] / 2, height / 2, -(centreY - bed[1] / 2));
    this.orbit.distance = Math.max(footprint, height) * 2.4;
    this.applyCamera();
  }

  // -- bed ------------------------------------------------------------------------------

  private buildBed(ir: Ir): void {
    this.bed.clear();
    const [w, d] = ir.meta.bedSize ?? [220, 220];

    const grid = new THREE.GridHelper(Math.max(w, d), Math.round(Math.max(w, d) / 10));
    const gridMaterial = grid.material as THREE.LineBasicMaterial;
    gridMaterial.color = new THREE.Color(0x2a2a2a);
    gridMaterial.transparent = true;
    gridMaterial.opacity = 0.6;
    this.bed.add(grid);

    const outline = new THREE.LineSegments(
      new THREE.EdgesGeometry(new THREE.PlaneGeometry(w, d)),
      new THREE.LineBasicMaterial({ color: 0x3f4441 }),
    );
    outline.rotation.x = -Math.PI / 2;
    this.bed.add(outline);

    // The print is modelled with 0,0 at the bed's front-left corner; the bed is centred.
    this.root.position.set(-w / 2, 0, d / 2);
  }

  // -- frame ----------------------------------------------------------------------------

  resize(width: number, height: number, pixelRatio = window.devicePixelRatio): void {
    this.renderer.setPixelRatio(Math.min(pixelRatio, this.maxPixelRatio));
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / Math.max(height, 1);
    this.camera.updateProjectionMatrix();
  }

  render(): void {
    this.renderer.render(this.scene, this.camera);
  }

  // -- teardown -------------------------------------------------------------------------

  private disposeGeometry(): void {
    if (this.lines) this.root.remove(this.lines);
    this.geometry?.dispose();
    this.material?.dispose();
    this.geometry = null;
    this.material = null;
    this.lines = null;
  }

  dispose(): void {
    this.disposeGeometry();
    this.paletteTexture.dispose();
    this.renderer.dispose();
  }
}

/**
 * Feature type is stored once per segment; the GPU wants one value per vertex. This is the only
 * per-vertex expansion in the pipeline, and it runs once per file.
 */
function expandFeature(ir: Ir): Uint8Array {
  const out = new Uint8Array(ir.segmentCount * 2);
  for (let i = 0; i < ir.segmentCount; i++) {
    const f = ir.featureType[i]!;
    out[i * 2] = f;
    out[i * 2 + 1] = f;
  }
  return out;
}

function normalised(rgb: [number, number, number]): [number, number, number] {
  return [rgb[0] / 255, rgb[1] / 255, rgb[2] / 255];
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}
