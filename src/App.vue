<script setup lang="ts">
/**
 * Two screens so far. Which one shows is decided by whether a file is open — there is no router
 * because there is nothing to route: the studio *is* the app once a file exists.
 */
import { computed } from 'vue';

import EmptyState from './screens/EmptyState.vue';
import Studio from './screens/Studio.vue';
import { project } from './stores/project';

const hasFile = computed(() => project.status === 'ready');
</script>

<template>
  <Studio v-if="hasFile" />
  <EmptyState v-else />
</template>
