import { computed, reactive, readonly, shallowRef } from 'vue';

import { parseFile } from '../ir/parseFile';
import type { Ir } from '../ir/types';

/**
 * The open file and what the parser made of it.
 *
 * A plain reactive module rather than a store library: there is one of each of these for the
 * lifetime of the app, and nothing here needs devtools time travel or module registration.
 *
 * The IR is deliberately **not** part of the reactive object. `reactive`/`readonly` would wrap
 * it in a proxy, and every one of the millions of TypedArray reads the renderer makes would go
 * through that proxy. It lives in a `shallowRef`, which makes the whole IR one atomic value:
 * replacing it is reactive, reading into it is not.
 */

export type ProjectStatus = 'empty' | 'loading' | 'ready' | 'error';

/** The parsed file, or null. Swap the whole value; never mutate through it. */
export const ir = shallowRef<Ir | null>(null);

const state = reactive({
  status: 'empty' as ProjectStatus,
  /** Absolute path on disk. The UI shows the file name from `ir.meta` instead. */
  path: null as string | null,
  error: null as string | null,
  /** Round trip for the last open, milliseconds. */
  elapsedMs: 0,
  /** Size on disk in bytes, from the parser's meta. */
  bytes: null as number | null,
});

export const project = readonly(state);

export const layerCount = computed(() => ir.value?.layerCount ?? 0);
export const warnings = computed(() => ir.value?.meta.warnings ?? []);

export async function openPath(path: string, options: { plate?: number } = {}): Promise<void> {
  state.status = 'loading';
  state.error = null;
  state.path = path;
  try {
    const result = await parseFile(path, options);
    ir.value = result.ir;
    state.bytes = result.ir.meta.sourceBytes;
    state.elapsedMs = result.elapsedMs;
    state.status = 'ready';
  } catch (e) {
    ir.value = null;
    state.error = messageOf(e);
    state.status = 'error';
  }
}

/** Ask the OS for a file, then open it. Resolves to false when the user cancelled. */
export async function pickAndOpen(): Promise<boolean> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const selected = await open({
    multiple: false,
    filters: [{ name: 'G-code', extensions: ['gcode', 'gco', 'g', '3mf'] }],
  });
  if (typeof selected !== 'string') return false;
  await openPath(selected);
  return true;
}

/**
 * Install an IR that did not come from `openPath` — the parse cache and the batch queue will
 * both need this, and the development preview entry point uses it today.
 */
export function adoptIr(model: Ir, path: string | null = null): void {
  ir.value = model;
  state.path = path;
  state.bytes = model.meta.sourceBytes;
  state.error = null;
  state.elapsedMs = 0;
  state.status = 'ready';
}

export function closeProject(): void {
  ir.value = null;
  state.status = 'empty';
  state.path = null;
  state.error = null;
  state.bytes = null;
}

function messageOf(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
