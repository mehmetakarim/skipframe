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

/**
 * Which layer a given frame shows.
 *
 * The preview and the exporter both call this and nothing else. If they computed the mapping
 * separately the rendered video could differ from what the user scrubbed through, and the whole
 * frame-index design would buy nothing.
 */
export function layerForFrame(frame: number, frames: number, layers: number): number {
  if (layers <= 0) return 0;
  const t = frames <= 1 ? 0 : frame / (frames - 1);
  return Math.min(layers - 1, Math.max(0, Math.floor(t * layers)));
}

/** The layer the current frame shows. Frame index drives the layer, never the other way round. */
export const currentLayer = computed(() =>
  layerForFrame(scene.frame, frameCount.value, layerCount.value),
);

export const resolution = computed(
  () => ASPECTS.find((a) => a.value === scene.aspect)?.size ?? [1080, 1920],
);

export const timecode = computed(() => formatTimecode(scene.frame / scene.fps));
export const duration = computed(() => formatTimecode(scene.durationS));

/** Jump to the frame that first shows a given layer — what the layer scrubber writes to. */
export function seekToLayer(layer: number): void {
  const layers = layerCount.value;
  if (layers <= 0) return;
  const t = layers <= 1 ? 0 : layer / (layers - 1);
  scene.frame = Math.round(t * (frameCount.value - 1));
}

export function seekToFrame(frame: number): void {
  scene.frame = Math.max(0, Math.min(Math.round(frame), frameCount.value - 1));
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
