<script setup lang="ts">
/**
 * The framed brand mark, at the two levels of detail the logo kit defines.
 *
 * The kit is explicit that this framed drawing is for the lockup and the brand indicator only —
 * the app icon uses the unframed mark on a squircle, and the two are never mixed.
 */
import { computed } from 'vue';

const props = withDefaults(defineProps<{ size?: number }>(), { size: 20 });

/** Below 32px the full drawing closes up, so the kit's simplified level takes over. */
const simplified = computed(() => props.size < 32);
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 100 100"
    class="sf-mark"
    role="img"
    aria-label="SkipFrame"
  >
    <template v-if="simplified">
      <rect
        x="12"
        y="12"
        width="76"
        height="76"
        rx="8"
        fill="none"
        stroke="currentColor"
        stroke-width="14"
      />
      <rect x="30" y="56" width="40" height="18" fill="currentColor" />
      <rect x="38" y="31" width="24" height="18" class="accent" />
    </template>
    <template v-else>
      <rect
        x="10"
        y="10"
        width="80"
        height="80"
        rx="10"
        fill="none"
        stroke="currentColor"
        stroke-width="10"
      />
      <g fill="currentColor">
        <rect x="25" y="72" width="50" height="10" />
        <rect x="30" y="57" width="40" height="10" />
        <rect x="35" y="42" width="30" height="10" />
      </g>
      <rect x="40" y="27" width="20" height="10" class="accent" />
    </template>
  </svg>
</template>

<style scoped>
.sf-mark {
  display: block;
  color: var(--text-primary);
}

.accent {
  fill: var(--gold);
}
</style>
