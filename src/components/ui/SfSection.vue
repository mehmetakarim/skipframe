<script setup lang="ts">
/**
 * A numbered, collapsible section of the left rail.
 *
 * The number is the design's way of showing where you are in the sequence: it turns gold on the
 * open section and the section's ground rises to `--bg-raised`. Closed sections keep a
 * right-pointing chevron and a quieter title.
 */
withDefaults(
  defineProps<{
    /** Two-digit index, e.g. `"03"`. */
    index: string;
    title: string;
    disabled?: boolean;
  }>(),
  { disabled: false },
);

const open = defineModel<boolean>('open', { required: true });
</script>

<template>
  <section :class="['sf-section', { open, disabled }]">
    <button
      class="header"
      type="button"
      :disabled="disabled"
      :aria-expanded="open"
      @click="open = !open"
    >
      <span class="index">{{ index }}</span>
      <span class="title">{{ title }}</span>
      <svg
        class="chevron"
        width="10"
        height="10"
        viewBox="0 0 12 12"
        fill="none"
        aria-hidden="true"
      >
        <path
          :d="open ? 'M2 4.5 6 8.5 10 4.5' : 'M4.5 2 8.5 6 4.5 10'"
          stroke="currentColor"
          stroke-width="1.6"
        />
      </svg>
    </button>

    <div v-if="open" class="body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.sf-section {
  border-bottom: 1px solid var(--border);
}

.sf-section.open {
  background: var(--bg-raised);
}

.header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
  padding: 13px var(--space-4);
  border: 0;
  background: transparent;
  text-align: left;
  cursor: pointer;
}

.header:disabled {
  cursor: not-allowed;
}

.index {
  font-family: var(--font-mono);
  font-size: var(--type-overline-size);
  color: var(--text-faint);
  transition: color 90ms ease;
}

.open .index {
  color: var(--gold);
}

.title {
  flex: 1;
  font-size: var(--type-body-size);
  font-weight: 700;
  color: var(--text-secondary);
}

.open .title {
  color: var(--text-primary);
}

.header:hover:not(:disabled) .title {
  color: var(--text-primary);
}

.chevron {
  display: block;
  color: var(--border-strong);
}

.open .chevron {
  color: var(--text-faint);
}

.disabled .index,
.disabled .title,
.disabled .chevron {
  color: var(--border-strong);
}

.body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: 0 var(--space-4) var(--space-4);
}
</style>
