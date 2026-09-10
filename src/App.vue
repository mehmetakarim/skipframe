<script setup lang="ts">
/**
 * Three screens. Whether a file is open decides between the drop target and the studio; the
 * queue is the one the user navigates to. No router — there is nothing to route, and a URL the
 * user never sees would be a strange thing to maintain.
 */
import { computed } from 'vue';

import EmptyState from './screens/EmptyState.vue';
import Studio from './screens/Studio.vue';
import Queue from './screens/Queue.vue';
import { project } from './stores/project';
import { ui } from './stores/ui';

const hasFile = computed(() => project.status === 'ready');
</script>

<template>
  <Queue v-if="ui.screen === 'queue'" />
  <Studio v-else-if="hasFile" />
  <EmptyState v-else />
</template>
