import { reactive } from 'vue';

/**
 * The error and warning window — FRAME 10.
 *
 * The rule for which mechanism reports what: something that goes wrong in answer to what the
 * user just did — dropping a file, pressing export — stops them here, because the next step
 * depends on it. Something that goes wrong in the background — a queued job, a watched folder —
 * is a notice, because nobody is waiting on it.
 *
 * One alert at a time. A second foreground failure replaces the first; both describe the
 * action the user just took, and the newer one is the one they are looking at.
 */

export interface AlertAction {
  label: string;
  run: () => void | Promise<void>;
  /**
   * Shown in place of the label for a moment after it ran, for an action that stays on the
   * alert — "Kopyalandı" after copying. Actions without it close the alert first.
   */
  doneLabel?: string;
}

export interface AlertFigure {
  label: string;
  value: string;
}

export interface Alert {
  kind: 'error' | 'warning';
  title: string;
  body: string;
  /** The one fact the alert is about, in a box: a file name, a line number. */
  chip?: string;
  /** Mono, the default, for names and numbers; sans for a chip that is mostly words. */
  chipFont?: 'mono' | 'sans';
  /** Label and value pairs in place of a chip — needed and free space. */
  figures?: AlertFigure[];
  primary: AlertAction;
  secondary?: AlertAction;
  /** Faint text on the right of the footer, where a secondary action would be. */
  hint?: string;
}

export const alerts = reactive({
  current: null as Alert | null,
});

export function showAlert(alert: Alert): void {
  alerts.current = alert;
}

export function closeAlert(): void {
  alerts.current = null;
}
