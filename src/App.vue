<script setup lang="ts">
/**
 * Four screens. Whether a file is open decides between the drop target and the studio; the
 * queue and settings are the ones the user navigates to. No router — there is nothing to route,
 * and a URL the user never sees would be a strange thing to maintain.
 *
 * Dropping a G-code is handled here rather than on the drop screen, so it works from anywhere:
 * once a file is open the drop target is gone, and having to close the app to look at a second
 * print would be a strange thing to ask.
 */
import { computed, onBeforeUnmount, onMounted } from 'vue';

import EmptyState from './screens/EmptyState.vue';
import Studio from './screens/Studio.vue';
import Queue from './screens/Queue.vue';
import Settings from './screens/Settings.vue';
import { openPath, project } from './stores/project';
import { goTo, ui } from './stores/ui';

const hasFile = computed(() => project.status === 'ready');

let unlisten: (() => void) | null = null;

onMounted(async () => {
  // The payload is a real path on disk, which is why this is the webview's drag-drop event and
  // not the DOM's — a browser File handle would be no use to the Rust parser.
  try {
    const { getCurrentWebview } = await import('@tauri-apps/api/webview');
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === 'over') {
        ui.fileDragging = true;
      } else if (event.payload.type === 'leave') {
        ui.fileDragging = false;
      } else if (event.payload.type === 'drop') {
        ui.fileDragging = false;
        const first = event.payload.paths[0];
        if (!first) return;
        goTo('studio');
        void openPath(first);
      }
    });
  } catch {
    // A plain browser tab; the file picker still works.
  }
});

onBeforeUnmount(() => unlisten?.());
</script>

<template>
  <Queue v-if="ui.screen === 'queue'" />
  <Settings v-else-if="ui.screen === 'settings'" />
  <Studio v-else-if="hasFile" />
  <EmptyState v-else />
</template>
