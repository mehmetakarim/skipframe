<script setup lang="ts">
/**
 * The studio: title bar, preset bar, three columns, timeline, export bar.
 *
 * The design keeps the two side rails at a fixed width and gives every pixel a wider window
 * gains to the viewport, which is what the grid below does.
 */
import TitleBar from '../components/studio/TitleBar.vue';
import PresetBar from '../components/studio/PresetBar.vue';
import LeftRail from '../components/studio/LeftRail.vue';
import Viewport from '../components/studio/Viewport.vue';
import Timeline from '../components/studio/Timeline.vue';
import InfoRail from '../components/studio/InfoRail.vue';
import ExportBar from '../components/studio/ExportBar.vue';
import { ir } from '../stores/project';
</script>

<template>
  <div class="screen">
    <TitleBar :file-name="ir?.meta.sourceName" />
    <PresetBar />

    <div class="columns">
      <LeftRail />

      <div class="centre">
        <Viewport />
        <Timeline />
      </div>

      <InfoRail />
    </div>

    <ExportBar />
  </div>
</template>

<style scoped>
.screen {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-surface);
}

.columns {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: var(--rail-left-width) 1fr var(--rail-right-width);
}

.centre {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

/* The viewport takes the height the timeline does not need. */
.centre > :first-child {
  flex: 1;
  min-height: 0;
}
</style>
