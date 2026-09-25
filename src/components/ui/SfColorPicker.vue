<script setup lang="ts">
/**
 * FRAME 09's colour picker: a guide palette, a hex field, R/G/B, and whatever the caller puts in
 * the slot — the surface texture, for a single filament.
 *
 * It replaces the operating system's colour dialog, which looked like a different application on
 * each platform and knew nothing about filament. A change applies to the render as it is made:
 * the point of picking a colour here is watching the print take it. There is no cancel, which is
 * why the design's only buttons are "Uygula" and "Kütüphaneye kaydet".
 *
 * Anchored to the field that opened it and drawn in a portal, because the left rail scrolls and
 * clips, and a popover that scrolls away from its own field is worse than no popover.
 */
import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue';

import { forgetColour, saveColour, settings } from '../../stores/settings';

const props = defineProps<{
  /** The field this belongs to; the popover sits under it. */
  anchor: HTMLElement | null;
  title?: string;
}>();

const emit = defineEmits<{ close: [] }>();
const model = defineModel<string>({ required: true });

/**
 * The design's palette, in its order: filament greys and white, then the accents. These are the
 * colours a print is usually made of, not a general-purpose spectrum.
 */
const GUIDE = [
  '#c9ccc6',
  '#ffffff',
  '#101010',
  '#3f4441',
  '#ebb60e',
  '#9a2a1e',
  '#1e6b33',
  '#e7e8e4',
  '#7c817b',
  '#2a2a2a',
  '#8a6a1e',
  '#1c3f5a',
  '#5a1c48',
];

const panelRef = useTemplateRef<HTMLDivElement>('panel');
const hexRef = useTemplateRef<HTMLInputElement>('hex');
/** What the user is typing. Only a complete, valid colour reaches the model. */
const typed = ref(model.value.toUpperCase());
const removing = ref<string | null>(null);

const rgb = computed(() => toRgb(model.value));
const saved = computed(() => settings.colourLibrary);
const isSaved = computed(() =>
  saved.value.some((c) => c.toLowerCase() === model.value.toLowerCase()),
);

function toRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  const full =
    h.length === 3
      ? h
          .split('')
          .map((c) => c + c)
          .join('')
      : h;
  return [
    parseInt(full.slice(0, 2), 16) || 0,
    parseInt(full.slice(2, 4), 16) || 0,
    parseInt(full.slice(4, 6), 16) || 0,
  ];
}

function toHex(r: number, g: number, b: number): string {
  const clamp = (n: number) => Math.max(0, Math.min(255, Math.round(n || 0)));
  return `#${[r, g, b].map((n) => clamp(n).toString(16).padStart(2, '0')).join('')}`;
}

function pick(colour: string) {
  model.value = colour;
  typed.value = colour.toUpperCase();
  removing.value = null;
}

/** Accept `#abc`, `abc`, `#aabbcc` and `aabbcc`; ignore anything else until it is complete. */
function onHexInput(value: string) {
  typed.value = value;
  const h = value.trim().replace(/^#/, '');
  if (/^[0-9a-f]{3}$/i.test(h) || /^[0-9a-f]{6}$/i.test(h)) {
    model.value = `#${h.length === 3 ? [...h].map((c) => c + c).join('') : h}`.toLowerCase();
  }
}

function onHexBlur() {
  // Put back the colour that is actually in use, so a half-typed value cannot stay on screen.
  typed.value = model.value.toUpperCase();
}

function onChannel(index: 0 | 1 | 2, value: string) {
  const next = [...rgb.value] as [number, number, number];
  next[index] = Number(value);
  pick(toHex(...next));
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.stopPropagation();
    emit('close');
  }
}

function onPointerDown(e: PointerEvent) {
  const target = e.target as Node;
  if (panelRef.value?.contains(target) || props.anchor?.contains(target)) return;
  emit('close');
}

/** Where the popover sits: under its field, pushed back inside the window when it would not fit. */
const position = ref({ left: 0, top: 0 });
const WIDTH = 316;
const MARGIN = 12;

function place() {
  const rect = props.anchor?.getBoundingClientRect();
  if (!rect) return;
  const height = panelRef.value?.offsetHeight ?? 420;
  const left = Math.min(Math.max(MARGIN, rect.left), window.innerWidth - WIDTH - MARGIN);
  const below = rect.bottom + 8;
  const top = below + height > window.innerHeight - MARGIN ? rect.top - height - 8 : below;
  position.value = { left, top: Math.max(MARGIN, top) };
}

onMounted(async () => {
  place();
  window.addEventListener('keydown', onKey, true);
  window.addEventListener('pointerdown', onPointerDown, true);
  window.addEventListener('resize', place);
  // The rail scrolls under the popover; following it would be worse than closing with it.
  window.addEventListener('scroll', place, true);
  await Promise.resolve();
  place();
  hexRef.value?.focus();
  hexRef.value?.select();
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey, true);
  window.removeEventListener('pointerdown', onPointerDown, true);
  window.removeEventListener('resize', place);
  window.removeEventListener('scroll', place, true);
});
</script>

<template>
  <Teleport to="body">
    <div
      ref="panel"
      class="picker"
      role="dialog"
      aria-modal="false"
      :aria-label="title ?? 'Renk'"
      :style="{ left: `${position.left}px`, top: `${position.top}px` }"
    >
      <header class="head">
        <span class="title">{{ title ?? 'Renk' }}</span>
        <button class="close" type="button" aria-label="Kapat" @click="emit('close')">
          <svg width="11" height="11" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" stroke-width="1.4" />
          </svg>
        </button>
      </header>

      <div class="body">
        <section class="group">
          <span class="t-overline">Kılavuz</span>
          <div class="grid">
            <button
              v-for="colour in GUIDE"
              :key="colour"
              type="button"
              :class="['chip', { selected: colour === model.toLowerCase() }]"
              :style="{ background: colour }"
              :title="colour.toUpperCase()"
              :aria-label="colour.toUpperCase()"
              :aria-pressed="colour === model.toLowerCase()"
              @click="pick(colour)"
            />
          </div>
        </section>

        <section v-if="saved.length > 0" class="group">
          <span class="t-overline">Kayıtlı</span>
          <div class="grid">
            <div v-for="colour in saved" :key="colour" class="slot">
              <button
                type="button"
                :class="['chip', { selected: colour === model.toLowerCase() }]"
                :style="{ background: colour }"
                :title="`${colour.toUpperCase()} · sağ tık: kaldır`"
                :aria-label="colour.toUpperCase()"
                :aria-pressed="colour === model.toLowerCase()"
                @click="pick(colour)"
                @contextmenu.prevent="removing = colour"
              />
              <button
                v-if="removing === colour"
                type="button"
                class="remove"
                @click="(forgetColour(colour), (removing = null))"
                @blur="removing = null"
              >
                Kaldır
              </button>
            </div>
          </div>
        </section>

        <section class="group">
          <span class="t-overline">Özel renk</span>
          <div class="custom">
            <span class="preview" :style="{ background: model }" aria-hidden="true" />
            <input
              ref="hex"
              class="hex"
              type="text"
              spellcheck="false"
              maxlength="7"
              aria-label="HEX"
              :value="typed"
              @input="onHexInput(($event.target as HTMLInputElement).value)"
              @blur="onHexBlur"
            />
          </div>
          <div class="channels">
            <label v-for="(name, i) in ['R', 'G', 'B']" :key="name" class="channel">
              <span class="channel-name">{{ name }}</span>
              <input
                type="number"
                min="0"
                max="255"
                :value="rgb[i]"
                :aria-label="name"
                @input="onChannel(i as 0 | 1 | 2, ($event.target as HTMLInputElement).value)"
              />
            </label>
          </div>
        </section>

        <slot />
      </div>

      <footer class="foot">
        <button class="apply" type="button" @click="emit('close')">Uygula</button>
        <button class="save" type="button" :disabled="isSaved" @click="saveColour(model)">
          {{ isSaved ? 'Kütüphanede' : 'Kütüphaneye kaydet' }}
        </button>
      </footer>
    </div>
  </Teleport>
</template>

<style scoped>
.picker {
  position: fixed;
  z-index: 60;
  width: 316px;
  max-height: calc(100vh - 24px);
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--bg-raised);
  box-shadow: 0 24px 48px -12px var(--bg-base);
  overflow: hidden;
}

.head {
  display: flex;
  align-items: center;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border);
}

.title {
  flex: 1;
  font-size: var(--type-body-size);
  font-weight: 700;
  color: var(--text-primary);
}

.close {
  display: flex;
  padding: var(--space-1);
  border: 0;
  border-radius: var(--radius-xs);
  background: none;
  color: var(--text-faint);
  cursor: pointer;
}

.close:hover {
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.body {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: var(--space-4);
  /* A full library on a short window: the head and the buttons stay, the palette scrolls. */
  overflow-y: auto;
}

.group {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 9px;
}

.slot {
  position: relative;
  display: flex;
}

.chip {
  width: 100%;
  aspect-ratio: 1;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 50%;
  cursor: pointer;
}

.chip.selected {
  border-color: transparent;
  box-shadow:
    0 0 0 2px var(--bg-raised),
    0 0 0 3.5px var(--gold);
}

.chip:focus-visible {
  outline: none;
  box-shadow:
    0 0 0 2px var(--bg-raised),
    0 0 0 4px var(--gold);
}

/* Sits over the neighbouring swatches on purpose: the grid cell is 30px and the word is not. */
.remove {
  position: absolute;
  top: calc(100% + 4px);
  left: 50%;
  transform: translateX(-50%);
  z-index: 1;
  padding: 4px 8px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  background: var(--bg-overlay);
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: 11px;
  white-space: nowrap;
  cursor: pointer;
}

.custom {
  display: flex;
  align-items: center;
  gap: 9px;
}

.preview {
  flex: none;
  width: 34px;
  height: 34px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius);
}

.hex {
  flex: 1;
  min-width: 0;
  padding: 9px 11px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  color: var(--text-primary);
  font-family: var(--font-sans);
  font-size: 12px;
  text-transform: uppercase;
}

.hex:focus {
  outline: none;
  border-color: var(--gold);
}

.channels {
  display: flex;
  gap: var(--space-2);
}

.channel {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 7px 9px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
}

.channel:focus-within {
  border-color: var(--gold);
}

.channel-name {
  font-family: var(--font-sans);
  font-size: 9px;
  color: var(--text-faint);
}

.channel input {
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--type-data-size);
}

.channel input:focus {
  outline: none;
}

/* The spinners would not survive the design's 9px-wide box, and the value is typed anyway. */
.channel input::-webkit-inner-spin-button {
  appearance: none;
}

.foot {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  border-top: 1px solid var(--border);
}

.apply,
.save {
  border-radius: var(--radius);
  font-family: var(--font-sans);
  font-size: var(--type-label-size);
  cursor: pointer;
}

.apply {
  padding: 9px 16px;
  border: 0;
  background: var(--gold);
  color: var(--bg-surface);
  font-weight: 700;
}

.apply:hover {
  background: var(--gold-hover);
}

.save {
  padding: 8px 14px;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--text-secondary);
}

.save:hover:not(:disabled) {
  border-color: var(--border-strong);
  background: var(--bg-overlay);
  color: var(--text-primary);
}

.save:disabled {
  color: var(--text-faint);
  cursor: default;
}

.apply:focus-visible,
.save:focus-visible,
.close:focus-visible,
.remove:focus-visible {
  outline: none;
  box-shadow: var(--focus-ring);
}
</style>
