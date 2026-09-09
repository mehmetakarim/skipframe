<script setup lang="ts">
/**
 * Scaffold shell. This screen exists to prove the whole chain end to end — file dialog,
 * Rust parse, raw-byte IPC, TypedArray views — and is replaced by the studio UI in step 3.
 */
import { ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';

import { parseFile } from './ir/parseFile';
import type { Ir } from './ir/types';

const ir = ref<Ir | null>(null);
const elapsedMs = ref(0);
const busy = ref(false);
const error = ref<string | null>(null);

async function pickFile() {
  const selected = await open({
    multiple: false,
    filters: [{ name: 'G-code', extensions: ['gcode', 'gco', 'g', '3mf'] }],
  });
  if (typeof selected !== 'string') return;

  busy.value = true;
  error.value = null;
  try {
    const result = await parseFile(selected);
    ir.value = result.ir;
    elapsedMs.value = result.elapsedMs;
  } catch (e) {
    error.value = String(e);
    ir.value = null;
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <main class="shell">
    <header class="head">
      <h1>SkipFrame</h1>
      <p class="dim">Scaffold build — studio UI lands in step 3.</p>
    </header>

    <button class="pick" :disabled="busy" @click="pickFile">
      {{ busy ? 'Parsing…' : 'Open a G-code file' }}
    </button>

    <p v-if="error" class="err mono">{{ error }}</p>

    <dl v-if="ir" class="stats mono">
      <div>
        <dt>file</dt>
        <dd>{{ ir.meta.sourceName }}</dd>
      </div>
      <div>
        <dt>slicer</dt>
        <dd>{{ ir.meta.dialect }} {{ ir.meta.slicerVersion ?? '' }}</dd>
      </div>
      <div>
        <dt>printer</dt>
        <dd>{{ ir.meta.printerModel ?? '—' }}</dd>
      </div>
      <div>
        <dt>bed</dt>
        <dd>{{ ir.meta.bedSize?.join(' × ') ?? '—' }} mm</dd>
      </div>
      <div>
        <dt>layers</dt>
        <dd>{{ ir.layerCount }}</dd>
      </div>
      <div>
        <dt>segments</dt>
        <dd>{{ ir.segmentCount.toLocaleString() }}</dd>
      </div>
      <div>
        <dt>buffer</dt>
        <dd>{{ (ir.buffer.byteLength / 1e6).toFixed(1) }} MB</dd>
      </div>
      <div>
        <dt>round trip</dt>
        <dd>{{ elapsedMs.toFixed(0) }} ms</dd>
      </div>
    </dl>

    <ul v-if="ir?.meta.warnings.length" class="warn">
      <li v-for="w in ir.meta.warnings" :key="w">{{ w }}</li>
    </ul>
  </main>
</template>

<style scoped>
.shell {
  display: flex;
  flex-direction: column;
  gap: var(--sf-space-4);
  align-items: flex-start;
  padding: var(--sf-space-6);
}

.head h1 {
  margin: 0;
  font-size: var(--sf-text-xl);
  letter-spacing: -0.01em;
}

.dim {
  margin: var(--sf-space-1) 0 0;
  color: var(--sf-text-dim);
  font-size: var(--sf-text-sm);
}

.pick {
  padding: var(--sf-space-2) var(--sf-space-4);
  border: 0;
  border-radius: var(--sf-radius);
  background: var(--sf-accent);
  color: var(--sf-ink);
  font: inherit;
  font-weight: 600;
  cursor: pointer;
}

.pick:disabled {
  opacity: 0.5;
  cursor: progress;
}

.err {
  color: var(--sf-danger);
  font-size: var(--sf-text-sm);
}

.stats {
  display: grid;
  grid-template-columns: max-content max-content;
  gap: var(--sf-space-1) var(--sf-space-5);
  margin: 0;
  font-size: var(--sf-text-sm);
}

.stats > div {
  display: contents;
}

.stats dt {
  color: var(--sf-text-faint);
}

.stats dd {
  margin: 0;
}

.warn {
  margin: 0;
  padding-left: var(--sf-space-4);
  color: var(--sf-text-dim);
  font-size: var(--sf-text-sm);
  max-width: 60ch;
}
</style>
