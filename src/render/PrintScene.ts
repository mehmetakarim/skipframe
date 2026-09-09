import * as THREE from 'three';

import type { Ir } from '../ir/types';
import { buildBeadGeometry, layerHeightOf } from './beadGeometry';
import { finishFor, lightDirection, type LightRig } from './surfaces';
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
 * The whole print is one instanced geometry: an extrusion bead uploaded once, drawn once per
 * segment. Animation is a single `instanceCount` assignment — no per-layer meshes, no geometry
 * rebuilds, one draw call per frame. (`setDrawRange` counts indices inside one instance, so it
 * cannot express "the first n segments"; `instanceCount` is its equivalent here and everything
 * that rule protects is unchanged.)
 *
 * The scene is driven by a **layer index**, never by elapsed time. Export walks the same
 * `setLayer` path frame by frame, which is what makes a rendered video reproducible.
 */

// GLSL ES 3.00. Three supplies `position`, `normal`, the matrices and `cameraPosition`;
// everything else, including the fragment output, has to be declared here.
//
// `position` is the cross-section coordinate (u, v, t) of the instanced bead prism and `normal`
// is the direction around that cross-section — see beadGeometry.ts. The prism is built here, in
// the vertex shader, from the two endpoints of the segment.
const VERTEX_SHADER = /* glsl */ `
  in vec3 aStart;
  in vec3 aEnd;
  in float aWidth;
  in float aFeature;

  uniform sampler2D uPalette;
  uniform float uShowTravel;
  uniform float uLayerHeight;
  uniform float uTravelWidth;
  uniform float uCurrentStart;      // first segment of the layer being printed
  uniform float uCurrentEnd;        // one past its last segment
  uniform vec3 uCurrentLayerColor;
  uniform float uHighlightCurrent;

  out vec3 vColor;
  out vec3 vNormal;
  out vec3 vWorld;

  void main() {
    bool isTravel = aFeature < 0.5;

    vec3 seg = aEnd - aStart;
    float len = length(seg);
    vec3 dir = len > 1e-6 ? seg / len : vec3(1.0, 0.0, 0.0);

    // Segments are almost always in the layer plane; a near-vertical one would make the usual
    // reference axis degenerate, so it gets a different one.
    vec3 up = abs(dir.z) > 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, 1.0);
    vec3 side = normalize(cross(up, dir));
    vec3 vert = cross(dir, side);

    float w = isTravel ? uTravelWidth : max(aWidth, 0.05);
    float h = isTravel ? uTravelWidth : uLayerHeight;

    // Both ends run half a bead past the endpoint so consecutive prisms overlap; without it
    // every corner in the toolpath would show a wedge of daylight.
    vec3 centre = mix(aStart, aEnd, position.z) + dir * ((position.z - 0.5) * w);
    vec3 local = centre + side * (position.x * w) + vert * (position.y * h);

    // The cross-section is an ellipse, not a circle, so its normal has to be corrected for the
    // bead's own width-to-height ratio — which is per segment and cannot be baked into the mesh.
    vec2 n2 = normalize(vec2(normal.x / max(w, 1e-4), normal.y / max(h, 1e-4)));
    vec3 n = normalize(side * n2.x + vert * n2.y);

    vec4 world = modelMatrix * vec4(local, 1.0);
    vWorld = world.xyz;
    vNormal = normalize(mat3(modelMatrix) * n);

    float id = float(gl_InstanceID);
    bool isCurrent = uHighlightCurrent > 0.5 && id >= uCurrentStart && id < uCurrentEnd && !isTravel;
    vec3 base = texture(uPalette, vec2((aFeature + 0.5) / ${PALETTE_SIZE}.0, 0.5)).rgb;
    vColor = isCurrent ? uCurrentLayerColor : base;

    gl_Position = projectionMatrix * viewMatrix * world;

    // Travels are hidden by collapsing them outside the clip volume rather than by splitting the
    // geometry, which would cost a second draw call and break the single instance count.
    if (isTravel && uShowTravel < 0.5) {
      gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    }
  }
`;

const FRAGMENT_SHADER = /* glsl */ `
  in vec3 vColor;
  in vec3 vNormal;
  in vec3 vWorld;

  uniform vec3 uKeyDir;
  uniform float uKey;
  uniform float uFill;
  uniform float uAmbient;
  uniform float uSpecular;
  uniform float uShininess;
  uniform float uRim;
  uniform float uTint;

  out vec4 fragColor;

  void main() {
    vec3 n = normalize(vNormal);
    vec3 view = normalize(cameraPosition - vWorld);

    // Two lights and an ambient term: a key, and a dimmer fill from the opposite side so the
    // shadow half of the print stays readable without a second shadow map.
    float key = max(dot(n, uKeyDir), 0.0) * uKey;
    float fill = max(dot(n, -uKeyDir), 0.0) * uFill;
    vec3 diffuse = vColor * (uAmbient + key + fill);

    vec3 halfVec = normalize(uKeyDir + view);
    float spec = pow(max(dot(n, halfVec), 0.0), uShininess) * uSpecular;
    // Plastics keep a white highlight; a metal takes its own colour, which is most of what
    // separates the two finishes.
    vec3 specColour = mix(vec3(1.0), vColor, uTint);

    float rim = pow(1.0 - max(dot(n, view), 0.0), 3.0) * uRim;

    fragColor = vec4(diffuse + spec * specColour + rim * vColor, 1.0);
  }
`;

/**
 * The background, drawn as a full-screen triangle before anything else.
 *
 * It has to live in the canvas rather than in CSS, because the exporter captures the canvas and
 * a page background would not be in the video.
 */
const BACKGROUND_VERTEX = /* glsl */ `
  out vec2 vUv;

  void main() {
    vUv = uv;
    gl_Position = vec4(position.xy, 0.0, 1.0);
  }
`;

const BACKGROUND_FRAGMENT = /* glsl */ `
  in vec2 vUv;

  uniform vec3 uTop;
  uniform vec3 uBottom;
  uniform float uVignette;

  out vec4 fragColor;

  void main() {
    vec3 colour = mix(uBottom, uTop, vUv.y);

    if (uVignette > 0.001) {
      vec2 d = vUv - 0.5;
      float falloff = smoothstep(0.75, 0.15, length(d));
      colour *= mix(1.0, falloff, uVignette);
    }

    fragColor = vec4(colour, 1.0);
  }
`;

/** A little air around the print so it never touches the frame edge. */
const FRAME_MARGIN = 1.12;

export interface PlateOptions {
  style: 'grid' | 'solid' | 'none';
  /** Grid spacing in millimetres. */
  spacing: number;
  showOutline: boolean;
  showOrigin: boolean;
}

export interface BackgroundOptions {
  style: 'solid' | 'gradient';
  top: string;
  bottom: string;
  vignette: number;
}

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
  private geometry: THREE.InstancedBufferGeometry | null = null;
  private beads: THREE.Mesh | null = null;
  private paletteTexture: THREE.DataTexture;

  private ir: Ir | null = null;
  private layer = 0;
  private maxPixelRatio: number;

  /** What the studio last told us to look at. The renderer holds no other view state. */
  private view = { azimuth: Math.PI * 0.25, elevation: 0.23, zoom: 1 };
  private target = new THREE.Vector3();
  /** Radius of the sphere the framing has to keep on screen. Zero until a file is loaded. */
  private frameRadius = 0;

  private plateOptions: PlateOptions = {
    style: 'grid',
    spacing: 10,
    showOutline: true,
    showOrigin: false,
  };
  private bedSize: [number, number] | null = null;
  private bedOrigin: [number, number] = [0, 0];

  /** Background is its own scene so it can be drawn behind everything without a depth trick. */
  private readonly backgroundScene = new THREE.Scene();
  private readonly backgroundCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  private readonly backgroundMaterial: THREE.ShaderMaterial;

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

    this.backgroundMaterial = new THREE.ShaderMaterial({
      glslVersion: THREE.GLSL3,
      vertexShader: BACKGROUND_VERTEX,
      fragmentShader: BACKGROUND_FRAGMENT,
      depthTest: false,
      depthWrite: false,
      uniforms: {
        uTop: { value: new THREE.Color(0x0b0b0b) },
        uBottom: { value: new THREE.Color(0x0b0b0b) },
        uVignette: { value: 0 },
      },
    });
    this.backgroundScene.add(
      new THREE.Mesh(new THREE.PlaneGeometry(2, 2), this.backgroundMaterial),
    );
  }

  // -- 04 background ----------------------------------------------------------------------

  setBackground(options: BackgroundOptions): void {
    const u = this.backgroundMaterial.uniforms;
    (u.uTop!.value as THREE.Color).set(options.top);
    (u.uBottom!.value as THREE.Color).set(
      options.style === 'gradient' ? options.bottom : options.top,
    );
    u.uVignette!.value = Math.min(1, Math.max(0, options.vignette));
  }

  // -- 02 filament ------------------------------------------------------------------------

  /**
   * Paint every extrusion in the filament's colour, leaving travels in their own grey.
   *
   * Feature colouring and filament colour are the same mechanism — a 16-entry palette texture —
   * so switching between them costs a 64-byte upload rather than touching the vertex buffer.
   */
  setFilamentColour(hex: string): void {
    const colour = new THREE.Color(hex);
    const rgb: [number, number, number] = [
      Math.round(colour.r * 255),
      Math.round(colour.g * 255),
      Math.round(colour.b * 255),
    ];
    const palette: Palette = { ...MONOCHROME };
    for (let feature = 1; feature < PALETTE_SIZE; feature++) palette[feature] = rgb;
    this.setPalette(palette);
  }

  // -- data -----------------------------------------------------------------------------

  setIr(ir: Ir): void {
    this.disposeGeometry();
    this.ir = ir;

    // Endpoints, width and feature are bound as views onto the parser's own buffer; nothing
    // about the print is copied to build this.
    const geometry = buildBeadGeometry(ir);

    const material = new THREE.ShaderMaterial({
      // GLSL 3 is what makes `gl_InstanceID` available, which is how the layer being printed is
      // told apart from the rest without a per-segment index attribute. WebGL2 is present in
      // both WebView2 and WKWebView on the platforms we ship.
      glslVersion: THREE.GLSL3,
      vertexShader: VERTEX_SHADER,
      fragmentShader: FRAGMENT_SHADER,
      uniforms: {
        uPalette: { value: this.paletteTexture },
        uShowTravel: { value: 0 },
        uLayerHeight: { value: layerHeightOf(ir) },
        uTravelWidth: { value: Math.max(0.08, layerHeightOf(ir) * 0.4) },
        uCurrentStart: { value: 0 },
        uCurrentEnd: { value: 0 },
        uCurrentLayerColor: { value: new THREE.Color().setRGB(...normalised(CURRENT_LAYER)) },
        uHighlightCurrent: { value: 1 },
        uKeyDir: { value: new THREE.Vector3(...lightDirection(135, 45)) },
        uKey: { value: 0.75 },
        uFill: { value: 0.18 },
        uAmbient: { value: 0.28 },
        uSpecular: { value: 0.06 },
        uShininess: { value: 8 },
        uRim: { value: 0.06 },
        uTint: { value: 0 },
      },
    });

    this.geometry = geometry;
    this.material = material;
    this.beads = new THREE.Mesh(geometry, material);
    // The bounding sphere would otherwise be computed from the instance buffer on every frustum
    // test; the camera always looks at the print, so culling buys nothing here.
    this.beads.frustumCulled = false;
    this.root.add(this.beads);

    this.buildBed(ir.meta.bedSize ?? [220, 220], ir.meta.bedOrigin ?? [0, 0]);
    this.frameToPrint(ir);
    this.setLayer(ir.layerCount - 1);
  }

  // -- 05 light and 02 surface --------------------------------------------------------------

  setLight(rig: LightRig): void {
    if (!this.material) return;
    const u = this.material.uniforms;
    (u.uKeyDir!.value as THREE.Vector3).set(...lightDirection(rig.azimuthDeg, rig.elevationDeg));
    u.uKey!.value = rig.intensity;
    u.uFill!.value = rig.fill;
    u.uAmbient!.value = rig.ambient;
  }

  setSurface(surface: string): void {
    if (!this.material) return;
    const finish = finishFor(surface);
    const u = this.material.uniforms;
    u.uSpecular!.value = finish.specular;
    u.uShininess!.value = finish.shininess;
    u.uRim!.value = finish.rim;
    u.uTint!.value = finish.tint;
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

  /**
   * Show layers `0 .. layer` inclusive, or nothing at all for -1.
   *
   * This is the only thing the animation ever changes. -1 is what a hold on the empty plate at
   * the start of a clip looks like.
   */
  setLayer(layer: number): void {
    const ir = this.ir;
    if (!ir || !this.geometry || !this.material) return;

    const clamped = Math.max(-1, Math.min(Math.floor(layer), ir.layerCount - 1));
    this.layer = clamped;

    if (clamped < 0) {
      this.geometry.instanceCount = 0;
      return;
    }

    const start = ir.layerStart[clamped] ?? 0;
    const end = ir.layerStart[clamped + 1] ?? ir.segmentCount;

    // The instanced equivalent of setDrawRange: one scalar, no rebuild, no per-layer meshes.
    this.geometry.instanceCount = end;
    this.material.uniforms.uCurrentStart!.value = start;
    this.material.uniforms.uCurrentEnd!.value = end;
  }

  get currentLayer(): number {
    return this.layer;
  }

  // -- camera ---------------------------------------------------------------------------

  /**
   * Place the camera.
   *
   * `zoom` multiplies the automatic fit rather than being a distance in millimetres, so the same
   * view survives a change of print, of aspect ratio or of field of view. The studio's camera
   * and motion sections are the only source of these values; the renderer holds no view state
   * of its own beyond what it was last told.
   */
  setView(azimuth: number, elevation: number, zoom = 1): void {
    this.view = {
      azimuth,
      elevation: clamp(elevation, -1.4, 1.4),
      zoom: clamp(zoom, 0.2, 20),
    };
    this.applyCamera();
  }

  setFov(degrees: number): void {
    this.camera.fov = clamp(degrees, 10, 90);
    this.camera.updateProjectionMatrix();
    this.applyCamera();
  }

  private applyCamera(): void {
    const { azimuth, elevation, zoom } = this.view;
    const distance = this.fitDistance() / zoom;
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
    const [ox, oy] = ir.meta.bedOrigin ?? [0, 0];
    const [bw, bd] = ir.meta.bedSize ?? [220, 220];

    // Bounds are in G-code space (Z up); the root group rotates them into Y up and shifts the
    // bed's centre to the world origin, so the target has to make the same journey.
    const centreX = (b[0] + b[3]) / 2;
    const centreY = (b[1] + b[4]) / 2;
    const height = Math.max(b[5] - b[2], 1);

    this.target.set(centreX - (ox + bw / 2), height / 2, -centreY + (oy + bd / 2));

    // The radius that has to fit on screen: half the diagonal of the print's box, so no
    // orbit angle can push a corner out of frame.
    const dx = Math.max(b[3] - b[0], 1);
    const dy = Math.max(b[4] - b[1], 1);
    this.frameRadius = 0.5 * Math.sqrt(dx * dx + dy * dy + height * height);

    this.applyCamera();
  }

  /**
   * Distance at which the print's bounding sphere fills the frame.
   *
   * A 9:16 canvas is the binding case: its horizontal field of view is far narrower than the
   * vertical one, so fitting on the vertical alone crops the print. Recomputed on every use
   * rather than cached, because it depends on the aspect ratio and the field of view, both of
   * which change from under it.
   */
  private fitDistance(): number {
    if (this.frameRadius <= 0) return 400;
    const vFov = (this.camera.fov * Math.PI) / 180;
    const hFov = 2 * Math.atan(Math.tan(vFov / 2) * this.camera.aspect);
    const tightest = Math.min(vFov, hFov);
    return (this.frameRadius / Math.sin(tightest / 2)) * FRAME_MARGIN;
  }

  // -- bed ------------------------------------------------------------------------------

  /** 03 Build plate. Rebuilt rather than toggled, because it is a handful of lines. */
  setPlate(options: PlateOptions): void {
    this.plateOptions = { ...this.plateOptions, ...options };
    if (this.bedSize) this.buildBed(this.bedSize, this.bedOrigin);
  }

  private buildBed(size: [number, number], origin: [number, number]): void {
    this.bed.clear();
    this.bedSize = size;
    this.bedOrigin = origin;
    const [w, d] = size;
    const { style, spacing, showOutline, showOrigin } = this.plateOptions;

    if (style === 'grid') {
      const span = Math.max(w, d);
      const divisions = Math.max(1, Math.round(span / Math.max(1, spacing)));
      const grid = new THREE.GridHelper(span, divisions, 0x2a2a2a, 0x2a2a2a);
      const gridMaterial = grid.material as THREE.LineBasicMaterial;
      gridMaterial.transparent = true;
      gridMaterial.opacity = 0.6;
      this.bed.add(grid);
    } else if (style === 'solid') {
      const plate = new THREE.Mesh(
        new THREE.PlaneGeometry(w, d),
        new THREE.MeshBasicMaterial({ color: 0x171717 }),
      );
      plate.rotation.x = -Math.PI / 2;
      // Just below zero so the first layer is never in a depth fight with the plate.
      plate.position.y = -0.05;
      this.bed.add(plate);
    }

    if (showOutline && style !== 'none') {
      const outline = new THREE.LineSegments(
        new THREE.EdgesGeometry(new THREE.PlaneGeometry(w, d)),
        new THREE.LineBasicMaterial({ color: 0x3f4441 }),
      );
      outline.rotation.x = -Math.PI / 2;
      this.bed.add(outline);
    }

    if (showOrigin) {
      // A cross at the machine's own 0,0, which is not the middle of the plate on every printer.
      const [ox, oy] = origin;
      const x = ox + w / 2;
      const z = -(oy + d / 2);
      const arm = Math.min(w, d) * 0.06;
      const points = new Float32Array([
        -arm + x,
        0,
        z,
        arm + x,
        0,
        z,
        x,
        0,
        -arm + z,
        x,
        0,
        arm + z,
      ]);
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(points, 3));
      this.bed.add(
        new THREE.LineSegments(geometry, new THREE.LineBasicMaterial({ color: 0x7c817b })),
      );
    }

    // The bed is drawn centred on the world origin, so the print is shifted by wherever the
    // bed's own origin sits. Most printers put 0,0 at the front-left corner, but a
    // centre-origin machine reports bed_origin = -w/2,-d/2 and would otherwise print into a
    // corner of its own plate.
    this.root.position.set(-(origin[0] + w / 2), 0, origin[1] + d / 2);
  }

  // -- frame ----------------------------------------------------------------------------

  resize(width: number, height: number, pixelRatio = window.devicePixelRatio): void {
    this.renderer.setPixelRatio(Math.min(pixelRatio, this.maxPixelRatio));
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / Math.max(height, 1);
    this.camera.updateProjectionMatrix();
    // Switching from 16:9 to 9:16 changes which field of view is the binding one, so the
    // camera has to be placed again rather than left where it was.
    this.applyCamera();
  }

  render(): void {
    // Background first, into a cleared buffer; then the scene on top without clearing again.
    this.renderer.autoClear = false;
    this.renderer.clear();
    this.renderer.render(this.backgroundScene, this.backgroundCamera);
    this.renderer.render(this.scene, this.camera);
  }

  // -- teardown -------------------------------------------------------------------------

  private disposeGeometry(): void {
    if (this.beads) this.root.remove(this.beads);
    this.geometry?.dispose();
    this.material?.dispose();
    this.geometry = null;
    this.material = null;
    this.beads = null;
  }

  dispose(): void {
    this.disposeGeometry();
    this.paletteTexture.dispose();
    this.backgroundMaterial.dispose();
    this.renderer.dispose();
  }
}

function normalised(rgb: [number, number, number]): [number, number, number] {
  return [rgb[0] / 255, rgb[1] / 255, rgb[2] / 255];
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}
