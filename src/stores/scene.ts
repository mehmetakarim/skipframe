import { computed, reactive } from 'vue';

import { layerCount } from './project';

/**
 * Everything the studio's controls change.
 *
 * The animation is expressed as a **frame index**, never as elapsed time. `frame` is the source
 * of truth; the layer shown and the timecode displayed are both derived from it. That is what
 * makes the preview and a rendered export agree frame for frame.
 */

export type Aspect = '9:16' | '16:9' | '1:1';
export type PlateStyle = 'grid' | 'solid' | 'none';
export type BackgroundStyle = 'solid' | 'gradient';
export type Easing = 'linear' | 'ease-in-out';

export const PLATE_STYLES = [
  { value: 'grid', label: 'Izgara' },
  { value: 'solid', label: 'Düz' },
  { value: 'none', label: 'Yok' },
];

export const BACKGROUND_STYLES = [
  { value: 'solid', label: 'Düz' },
  { value: 'gradient', label: 'Geçişli' },
];

export const EASINGS = [
  { value: 'linear', label: 'Sabit' },
  { value: 'ease-in-out', label: 'Yumuşak' },
];
export type PresetId = 'plain-studio' | 'desktop' | 'showcase' | 'raw' | 'night';
export type SectionKey =
  'source' | 'filament' | 'bed' | 'background' | 'light' | 'camera' | 'motion' | 'timing';

export const PRESETS: { id: PresetId; label: string }[] = [
  { id: 'plain-studio', label: 'Sade stüdyo' },
  { id: 'desktop', label: 'Masa üstü' },
  { id: 'showcase', label: 'Ürün vitrini' },
  { id: 'raw', label: 'Ham baskı' },
  { id: 'night', label: 'Gece' },
];

export const ASPECTS: { value: Aspect; label: string; size: [number, number] }[] = [
  { value: '9:16', label: '9:16', size: [1080, 1920] },
  { value: '16:9', label: '16:9', size: [1920, 1080] },
  { value: '1:1', label: '1:1', size: [1080, 1080] },
];

export const SURFACES = [
  { value: 'matte-pla', label: 'Mat PLA' },
  { value: 'glossy-pla', label: 'Parlak PLA' },
  { value: 'silk', label: 'İpek' },
  { value: 'metallic', label: 'Metalik' },
];

export const scene = reactive({
  preset: 'plain-studio' as PresetId,
  presetDirty: false,

  /** Which left-rail sections are open. The design opens the first two. */
  sections: {
    source: true,
    filament: true,
    bed: false,
    background: false,
    light: false,
    camera: false,
    motion: false,
    timing: false,
  } satisfies Record<SectionKey, boolean>,

  // 01 Source
  /** Render every Nth layer. 1 shows them all. */
  layerSkip: 1,
  hideTravel: true,

  // 02 Filament
  filaments: ['#c9ccc6', '#101010', '#ebb60e', '#3f4441', '#ffffff'],
  filamentIndex: 0,
  surface: 'matte-pla',
  /** Colour the layer being laid down differently from the material behind it. */
  highlightCurrentLayer: true,

  // 03 Build plate
  plate: {
    style: 'grid' as PlateStyle,
    /** Grid spacing in millimetres. */
    spacing: 10,
    showOutline: true,
    showOrigin: false,
  },

  // 04 Background
  background: {
    style: 'solid' as BackgroundStyle,
    top: '#0b0b0b',
    bottom: '#171717',
    /** 0 is off, 1 is a hard corner falloff. */
    vignette: 0,
  },

  // 06 Camera
  camera: {
    azimuthDeg: 45,
    elevationDeg: 13,
    fovDeg: 38,
    /** 1.0 is the automatic fit; larger moves closer. */
    zoom: 1,
  },

  // 07 Motion — all of it a pure function of the frame index, never of the clock
  motion: {
    /** Degrees of orbit swept across the whole clip. 0 holds still. */
    orbitDeg: 0,
    /** Degrees the camera climbs across the clip. */
    riseDeg: 0,
    /** Zoom multiplier reached at the last frame. 1 holds. */
    zoomTo: 1,
    easing: 'ease-in-out' as Easing,
  },

  // 08 Timing
  timing: {
    /** Frames held on the empty plate before the print starts. */
    holdStart: 0,
    /** Frames held on the finished print at the end. */
    holdEnd: 0,
    easing: 'linear' as Easing,
  },

  // Output
  aspect: '9:16' as Aspect,
  fps: 30,
  durationS: 12,

  // Transport
  frame: 0,
  playing: false,
  speed: 1,
  loop: false,
});

export const frameCount = computed(() => Math.max(1, Math.round(scene.durationS * scene.fps)));

/** Smoothstep. The only easing offered, because a second one would need a reason. */
export function ease(t: number, kind: Easing): number {
  const c = Math.min(1, Math.max(0, t));
  return kind === 'ease-in-out' ? c * c * (3 - 2 * c) : c;
}

/** Everything about the shape of the animation that is not the frame index itself. */
export interface LayerTiming {
  holdStart: number;
  holdEnd: number;
  easing: Easing;
  layerSkip: number;
}

export function layerTiming(): LayerTiming {
  return { ...scene.timing, layerSkip: scene.layerSkip };
}

/**
 * Which layer a given frame shows, or -1 for an empty plate.
 *
 * The preview and the exporter both call this and nothing else. If they computed the mapping
 * separately the rendered video could differ from what the user scrubbed through, and the whole
 * frame-index design would buy nothing.
 */
export function layerForFrame(
  frame: number,
  frames: number,
  layers: number,
  timing: LayerTiming,
): number {
  if (layers <= 0) return -1;

  // The holds are frames, not seconds, so they survive a change of frame rate unchanged.
  const hold = Math.max(0, Math.floor(timing.holdStart));
  if (frame < hold) return -1;

  const active = Math.max(1, frames - hold - Math.max(0, Math.floor(timing.holdEnd)));
  const step = Math.min(Math.max(0, frame - hold), active - 1);
  const t = active <= 1 ? 1 : step / (active - 1);

  const raw = Math.min(layers - 1, Math.floor(ease(t, timing.easing) * layers));

  // Layer skip makes the animation step in chunks; it never hides finished material, because
  // the draw range only ever runs from the first segment.
  const skip = Math.max(1, Math.floor(timing.layerSkip));
  if (skip === 1) return raw;
  return Math.min(layers - 1, Math.floor(raw / skip) * skip);
}

/** Camera placement for a frame. Pure, so export reproduces the preview's move exactly. */
export interface View {
  azimuth: number;
  elevation: number;
  zoom: number;
}

export function viewForFrame(frame: number, frames: number): View {
  const t = frames <= 1 ? 0 : frame / (frames - 1);
  const e = ease(t, scene.motion.easing);
  const rad = (deg: number) => (deg * Math.PI) / 180;
  return {
    azimuth: rad(scene.camera.azimuthDeg + scene.motion.orbitDeg * e),
    elevation: rad(scene.camera.elevationDeg + scene.motion.riseDeg * e),
    zoom: scene.camera.zoom * (1 + (scene.motion.zoomTo - 1) * e),
  };
}

/** The layer the current frame shows. Frame index drives the layer, never the other way round. */
export const currentLayer = computed(() =>
  layerForFrame(scene.frame, frameCount.value, layerCount.value, layerTiming()),
);

/** The camera the current frame implies. */
export const currentView = computed(() => viewForFrame(scene.frame, frameCount.value));

export const resolution = computed(
  () => ASPECTS.find((a) => a.value === scene.aspect)?.size ?? [1080, 1920],
);

export const timecode = computed(() => formatTimecode(scene.frame / scene.fps));
export const duration = computed(() => formatTimecode(scene.durationS));

export function seekToFrame(frame: number): void {
  scene.frame = Math.max(0, Math.min(Math.round(frame), frameCount.value - 1));
}

/**
 * What each preset actually sets.
 *
 * A preset is a starting point, not a mode: it writes these values into the scene and then gets
 * out of the way, which is why touching any control afterwards marks it dirty rather than
 * switching the preset off.
 */
const PRESET_VALUES: Record<PresetId, () => void> = {
  'plain-studio': () => {
    scene.plate = { style: 'grid', spacing: 10, showOutline: true, showOrigin: false };
    scene.background = { style: 'solid', top: '#0b0b0b', bottom: '#0b0b0b', vignette: 0 };
    scene.camera = { azimuthDeg: 45, elevationDeg: 13, fovDeg: 38, zoom: 1 };
    scene.motion = { orbitDeg: 0, riseDeg: 0, zoomTo: 1, easing: 'ease-in-out' };
    scene.timing = { holdStart: 0, holdEnd: 0, easing: 'linear' };
    scene.hideTravel = true;
  },
  desktop: () => {
    scene.plate = { style: 'solid', spacing: 10, showOutline: true, showOrigin: false };
    scene.background = { style: 'gradient', top: '#171717', bottom: '#0b0b0b', vignette: 0.25 };
    scene.camera = { azimuthDeg: 35, elevationDeg: 22, fovDeg: 34, zoom: 1 };
    scene.motion = { orbitDeg: 25, riseDeg: 0, zoomTo: 1, easing: 'ease-in-out' };
    scene.timing = { holdStart: 0, holdEnd: 12, easing: 'linear' };
    scene.hideTravel = true;
  },
  showcase: () => {
    scene.plate = { style: 'none', spacing: 10, showOutline: false, showOrigin: false };
    scene.background = { style: 'gradient', top: '#1f1f1f', bottom: '#0b0b0b', vignette: 0.6 };
    scene.camera = { azimuthDeg: 40, elevationDeg: 16, fovDeg: 30, zoom: 1.05 };
    scene.motion = { orbitDeg: 360, riseDeg: 8, zoomTo: 1.15, easing: 'ease-in-out' };
    scene.timing = { holdStart: 6, holdEnd: 18, easing: 'ease-in-out' };
    scene.hideTravel = true;
  },
  raw: () => {
    scene.plate = { style: 'grid', spacing: 10, showOutline: true, showOrigin: true };
    scene.background = { style: 'solid', top: '#101010', bottom: '#101010', vignette: 0 };
    scene.camera = { azimuthDeg: 0, elevationDeg: 35, fovDeg: 45, zoom: 1 };
    scene.motion = { orbitDeg: 0, riseDeg: 0, zoomTo: 1, easing: 'linear' };
    scene.timing = { holdStart: 0, holdEnd: 0, easing: 'linear' };
    // The one preset that shows what the machine actually does, travels included.
    scene.hideTravel = false;
  },
  night: () => {
    scene.plate = { style: 'grid', spacing: 20, showOutline: true, showOrigin: false };
    scene.background = { style: 'gradient', top: '#101010', bottom: '#000000', vignette: 0.8 };
    scene.camera = { azimuthDeg: 55, elevationDeg: 10, fovDeg: 32, zoom: 1 };
    scene.motion = { orbitDeg: 45, riseDeg: 12, zoomTo: 1.1, easing: 'ease-in-out' };
    scene.timing = { holdStart: 8, holdEnd: 20, easing: 'ease-in-out' };
    scene.hideTravel = true;
  },
};

export function applyPreset(id: PresetId): void {
  PRESET_VALUES[id]();
  scene.preset = id;
  scene.presetDirty = false;
}

export function markPresetDirty(): void {
  scene.presetDirty = true;
}

/** `00:09.06` — minutes, seconds, then hundredths, as the design's timecode role specifies. */
export function formatTimecode(seconds: number): string {
  const safe = Math.max(0, seconds);
  const m = Math.floor(safe / 60);
  const s = Math.floor(safe % 60);
  const cs = Math.floor((safe % 1) * 100);
  return `${pad(m)}:${pad(s)}.${pad(cs)}`;
}

function pad(n: number): string {
  return String(n).padStart(2, '0');
}
