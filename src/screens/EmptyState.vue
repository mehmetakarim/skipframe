<script setup lang="ts">
/**
 * First launch. A drop target on the left, slicer export instructions on the right.
 *
 * The design's hint line reads ".gcode · .gco · .g · maks. 500 MB". The size limit is dropped
 * here on purpose: the parser streams and has no file-size ceiling, so advertising one would
 * turn people away from files the app handles fine. `.gcode.3mf` is added because Bambu and
 * Orca users have that container by default.
 */
import { onBeforeUnmount, onMounted, ref } from 'vue';

import SfButton from '../components/ui/SfButton.vue';
import SfMark from '../components/SfMark.vue';
import TitleBar from '../components/studio/TitleBar.vue';
import { openPath, pickAndOpen, project } from '../stores/project';

const dragging = ref(false);
let unlisten: (() => void) | null = null;

const GUIDES = [
  {
    slicer: 'PrusaSlicer · OrcaSlicer',
    body: 'Dilimledikten sonra Dışa Aktar düğmesinin yanındaki oku aç.',
    path: 'Export G-code → .gcode',
  },
  {
    slicer: 'Ultimaker Cura',
    body: 'Yazıcıya gönderme yerine diske kaydet seçeneğini kullan.',
    path: 'File → Save Project as… → .gcode',
  },
  {
    slicer: 'Bambu Studio',
    body: 'Plaka dosyasını değil, düz G-code’u dışa aktar.',
    path: 'Export plate sliced file → .gcode',
  },
];

onMounted(async () => {
  // Drag and drop is delivered by the webview, not by DOM drag events, because the payload is a
  // real path on disk rather than a browser File handle.
  try {
    const { getCurrentWebview } = await import('@tauri-apps/api/webview');
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'over') dragging.value = true;
      else if (event.payload.type === 'leave') dragging.value = false;
      else if (event.payload.type === 'drop') {
        dragging.value = false;
        const first = event.payload.paths[0];
        if (first) void openPath(first);
      }
    });
  } catch {
    // Running in a plain browser tab; the file picker still works.
  }
});

onBeforeUnmount(() => unlisten?.());
</script>

<template>
  <div class="screen">
    <TitleBar />

    <div class="body">
      <section class="main">
        <div :class="['dropzone', { dragging, busy: project.status === 'loading' }]">
          <SfMark :size="96" />

          <div class="copy">
            <h1>G-code dosyasını buraya bırak</h1>
            <p>
              SkipFrame katmanları okur ve baskının kendi animasyonunu kurar. Sahneyi sonra
              ayarlarsın.
            </p>
          </div>

          <div class="action">
            <SfButton
              variant="primary"
              :disabled="project.status === 'loading'"
              @click="pickAndOpen"
            >
              {{ project.status === 'loading' ? 'Okunuyor…' : 'Dosya seç' }}
            </SfButton>
            <span class="hint">.gcode · .gco · .g · .gcode.3mf</span>
          </div>
        </div>

        <p v-if="project.status === 'error'" class="error">
          {{ project.error }}
        </p>

        <div class="privacy">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <rect
              x="4"
              y="10.5"
              width="16"
              height="10"
              rx="2"
              stroke="currentColor"
              stroke-width="1.8"
            />
            <path d="M8 10.5V7.5a4 4 0 0 1 8 0v3" stroke="currentColor" stroke-width="1.8" />
          </svg>
          <span>Dosyalarınız cihazınızdan çıkmıyor. Bütün işleme yerel olarak yapılır.</span>
        </div>
      </section>

      <aside class="guides">
        <span class="t-overline">Slicer’dan dışa aktarma</span>

        <div class="cards">
          <article v-for="guide in GUIDES" :key="guide.slicer" class="card">
            <h2>{{ guide.slicer }}</h2>
            <p>{{ guide.body }}</p>
            <code>{{ guide.path }}</code>
          </article>
        </div>

        <p class="fallback">
          Slicer’ını göremiyorsan yine de bırakabilirsin — tanınmayan lehçeler için yedek okuyucu
          var.
        </p>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.screen {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-surface);
}

.body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 1fr 380px;
}

.main {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-6);
  padding: 56px;
}

.dropzone {
  width: 100%;
  max-width: 620px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 22px;
  padding: 54px 40px;
  border: 1.5px dashed var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--bg-base);
  text-align: center;
  transition:
    border-color 120ms ease,
    background 120ms ease;
}

.dropzone.dragging {
  border-color: var(--gold);
  background: var(--gold-tint);
}

.dropzone.busy {
  border-color: var(--border);
}

.copy {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.015em;
}

.copy p {
  margin: 0;
  max-width: 42ch;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-muted);
}

.action {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-top: 2px;
}

.hint {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.06em;
  color: var(--text-faint);
}

.error {
  margin: 0;
  max-width: 620px;
  font-size: var(--type-label-size);
  color: var(--danger);
  text-align: center;
}

.privacy {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--bg-base);
  color: var(--text-muted);
  font-size: var(--type-label-size);
}

.guides {
  display: flex;
  flex-direction: column;
  gap: 22px;
  padding: var(--space-7) 30px;
  border-left: 1px solid var(--border);
  background: var(--bg-base);
  overflow-y: auto;
}

.cards {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.card {
  padding: var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
}

.card h2 {
  margin: 0 0 var(--space-2);
  font-size: var(--type-body-size);
  font-weight: 700;
  color: var(--text-primary);
}

.card p {
  margin: 0 0 10px;
  font-size: var(--type-label-size);
  line-height: 1.65;
  color: var(--text-muted);
}

.card code {
  display: block;
  padding: var(--space-2) 10px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
}

.fallback {
  margin: auto 0 0;
  font-size: var(--type-label-size);
  line-height: 1.6;
  color: var(--text-faint);
}
</style>
