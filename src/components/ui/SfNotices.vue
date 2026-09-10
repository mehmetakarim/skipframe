<script setup lang="ts">
/**
 * The notice stack, mounted once for the whole window.
 *
 * Bottom right, above everything including a modal: an error is the one thing that must not be
 * hidden by what was already on screen. Newest sits nearest the corner, which is where the eye
 * already is after pressing a button.
 */
import { dismiss, notices, type Notice } from '../../stores/notices';

const ICONS: Record<Notice['kind'], string> = {
  // A cross, a bang and an i, drawn rather than typed so they line up with the design's stroke.
  error: 'M5 5l6 6M11 5l-6 6',
  warning: 'M8 4v5M8 11.4v.2',
  info: 'M8 7.2v4.8M8 4.4v.2',
};

function act(notice: Notice) {
  notice.action?.run();
  dismiss(notice.id);
}
</script>

<template>
  <div class="stack" role="status" aria-live="polite">
    <TransitionGroup name="notice">
      <article v-for="notice in notices.items" :key="notice.id" :class="['notice', notice.kind]">
        <svg class="icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="8" cy="8" r="7" fill="none" stroke="currentColor" stroke-width="1.3" />
          <path
            :d="ICONS[notice.kind]"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>

        <div class="text">
          <p class="title">
            {{ notice.title }}
            <span v-if="notice.count > 1" class="count">×{{ notice.count }}</span>
          </p>
          <p v-if="notice.detail" class="detail">{{ notice.detail }}</p>
          <button v-if="notice.action" class="action" type="button" @click="act(notice)">
            {{ notice.action.label }}
          </button>
        </div>

        <button class="close" type="button" aria-label="Kapat" @click="dismiss(notice.id)">
          <svg width="10" height="10" viewBox="0 0 12 12" fill="none" aria-hidden="true">
            <path d="M1 1l10 10M11 1L1 11" stroke="currentColor" stroke-width="1.4" />
          </svg>
        </button>
      </article>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.stack {
  position: fixed;
  right: var(--space-5);
  bottom: var(--space-5);
  z-index: 50;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 380px;
  max-width: calc(100vw - var(--space-6));
  /* The column is only a rail for the cards; clicks pass through the gaps between them. */
  pointer-events: none;
}

.notice {
  pointer-events: auto;
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
  border: 1px solid var(--border);
  border-left: 2px solid var(--border-strong);
  border-radius: var(--radius-md);
  background: var(--bg-overlay);
  box-shadow: 0 8px 24px rgb(0 0 0 / 0.45);
}

.notice.error {
  border-left-color: var(--danger);
}

.notice.warning {
  border-left-color: var(--gold);
}

.icon {
  flex: none;
  margin-top: 1px;
  color: var(--text-faint);
}

.error .icon {
  color: var(--danger);
}

.warning .icon {
  color: var(--gold);
}

.text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.title {
  margin: 0;
  font-size: var(--type-label-size);
  font-weight: 700;
  color: var(--text-primary);
}

.count {
  margin-left: var(--space-1);
  font-family: var(--font-mono);
  font-size: var(--type-data-size);
  font-weight: 400;
  color: var(--text-faint);
}

/*
 * The machine's own words. Selectable on purpose: the first thing anyone does with a parser
 * error is paste it somewhere. Wrapped rather than truncated for the same reason, and capped so
 * one enormous message cannot fill the window.
 */
.detail {
  margin: 0;
  max-height: 84px;
  overflow-y: auto;
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--text-muted);
  /* A run that failed on several files lists them one per line. */
  white-space: pre-line;
  overflow-wrap: anywhere;
  user-select: text;
  cursor: text;
}

.action {
  align-self: flex-start;
  margin-top: var(--space-1);
  padding: 0;
  border: 0;
  background: none;
  font-family: inherit;
  font-size: var(--type-label-size);
  font-weight: 700;
  color: var(--gold);
  cursor: pointer;
}

.action:hover {
  color: var(--gold-hover);
  text-decoration: underline;
}

.action:focus-visible,
.close:focus-visible {
  outline: none;
  box-shadow: var(--focus-ring);
  border-radius: var(--radius-xs);
}

.close {
  flex: none;
  display: flex;
  padding: var(--space-1);
  border: 0;
  border-radius: var(--radius-xs);
  background: none;
  color: var(--text-faint);
  cursor: pointer;
}

.close:hover {
  background: var(--bg-raised);
  color: var(--text-primary);
}

/*
 * In from the right, and nothing about the entrance touches opacity.
 *
 * Vue's enter classes set opacity to 0 and take it away one frame later; a keyframe holds its
 * first frame until the animation is scheduled. Both mean the same thing for a window that is
 * minimised or occluded when a queue job fails: an error notice sitting there invisible. A
 * stalled slide is a card twelve pixels to the right, which is a card.
 */
.notice {
  animation: notice-in 140ms ease;
}

@keyframes notice-in {
  from {
    transform: translateX(12px);
  }
}

.notice-leave-active {
  transition:
    opacity 140ms ease,
    transform 140ms ease;
}

.notice-leave-to {
  opacity: 0;
  transform: translateX(12px);
}

.notice-move {
  transition: transform 140ms ease;
}

@media (prefers-reduced-motion: reduce) {
  .notice,
  .notice-leave-active,
  .notice-move {
    animation: none;
    transition: none;
  }
}
</style>
