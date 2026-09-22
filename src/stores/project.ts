import { computed, reactive, readonly, shallowRef } from 'vue';

import { parseFile } from '../ir/parseFile';
import { openFailureAlert, unknownDialectAlert } from '../lib/openAlerts';
import { showAlert } from './alerts';
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
  /** Round trip for the last open, milliseconds. */
  elapsedMs: 0,
  /** Size on disk in bytes, from the parser's meta. */
  bytes: null as number | null,
});

export const project = readonly(state);

export const layerCount = computed(() => ir.value?.layerCount ?? 0);
export const warnings = computed(() => ir.value?.meta.warnings ?? []);

/**
 * Files already warned about an unrecognised slicer this session. Reopening the same print, or
 * switching plates in it, is not news.
 */
const dialectWarned = new Set<string>();

export async function openPath(path: string, options: { plate?: number } = {}): Promise<void> {
  // What to fall back to if this file turns out not to be readable. A bad second file used to
  // empty the studio and drop the user back on the drop screen with the print they were working
  // on gone — a file that cannot be opened must cost them nothing but the attempt.
  const previous = { status: state.status, path: state.path, bytes: state.bytes };
  const hadFile = ir.value !== null;

  state.status = 'loading';
  state.path = path;
  try {
    const result = await parseFile(path, options);
    ir.value = result.ir;
    state.bytes = result.ir.meta.sourceBytes;
    state.elapsedMs = result.elapsedMs;
    state.status = 'ready';

    if (result.ir.meta.warnings.includes('unknown_dialect') && !dialectWarned.has(path)) {
      dialectWarned.add(path);
      showAlert(unknownDialectAlert(path, result.ir.layerCount));
    }
  } catch (e) {
    if (hadFile) {
      state.status = previous.status;
      state.path = previous.path;
      state.bytes = previous.bytes;
    } else {
      ir.value = null;
      state.status = 'error';
    }
    // The user just chose this file and is waiting on it, so this stops them rather than
    // sliding a notice into the corner. The file is not opened, whatever the failure: a
    // truncated print rendered as far as it goes would look like a finished one.
    showAlert(
      openFailureAlert(path, e, {
        retry: () => openPath(path, options),
        pickAnother: () => void pickAndOpen(),
      }),
    );
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
  state.elapsedMs = 0;
  state.status = 'ready';
}

export function closeProject(): void {
  ir.value = null;
  state.status = 'empty';
  state.path = null;
  state.bytes = null;
}
