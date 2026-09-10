import { reactive } from 'vue';

import { errorText } from '../lib/messages';

/**
 * What the app tells the user when something happened away from where they are looking.
 *
 * Every failure in SkipFrame used to be reported by the screen that owned it: a queue job's
 * error appeared in its row, a parse error appeared on the drop screen. That works right up to
 * the moment the user is somewhere else — a job failing while the studio is open, or the
 * watched folder failing while nobody is on the settings screen, said nothing at all.
 *
 * So notices are deliberately global and deliberately few. This is not a log: the screens keep
 * owning the detail, and a notice only says that something happened and offers the one step
 * that leads to it.
 */

export type NoticeKind = 'error' | 'warning' | 'info';

export interface NoticeAction {
  label: string;
  run: () => void;
}

export interface Notice {
  id: number;
  kind: NoticeKind;
  title: string;
  /** The machine's own words — a parser message, an OS error. Shown smaller, and selectable. */
  detail: string | null;
  action: NoticeAction | null;
  /** How many times this same notice has arrived. Shown from two upwards. */
  count: number;
}

/** More than a few at once is a wall of text, and the newest are the ones being read. */
const MAX_VISIBLE = 4;

/**
 * How long a notice that carries no problem stays up.
 *
 * Only plain information goes by itself. A warning is still something that went differently
 * from what the user asked for — taking it off the screen before it has been read is the same
 * mistake as never showing it.
 */
const INFO_TIMEOUT_MS = 6000;

export const notices = reactive({ items: [] as Notice[] });

const timers = new Map<number, ReturnType<typeof setTimeout>>();
let nextId = 1;

export interface NoticeInput {
  kind?: NoticeKind;
  title: string;
  detail?: string | null;
  action?: NoticeAction | null;
}

/**
 * Raise a notice, or bump the one that is already saying this.
 *
 * The repeat case is not a nicety: a watched folder that cannot write to its output directory
 * fails once per file that lands in it, and four identical notices tell the user nothing the
 * first one did not.
 */
export function notify(input: NoticeInput): number {
  const kind = input.kind ?? 'info';
  const detail = input.detail ?? null;

  const existing = notices.items.find(
    (n) => n.kind === kind && n.title === input.title && n.detail === detail,
  );
  if (existing) {
    existing.count += 1;
    arm(existing);
    return existing.id;
  }

  const notice: Notice = {
    id: nextId++,
    kind,
    title: input.title,
    detail,
    action: input.action ?? null,
    count: 1,
  };
  notices.items.push(notice);

  // Drop from the front, and prefer to drop something that would have gone by itself: an error
  // pushed off the end by a run of progress messages is exactly what must not happen. The
  // arrival is never the one dropped — a notice nobody saw is not a notice.
  while (notices.items.length > MAX_VISIBLE) {
    const older = notices.items.filter((n) => n.id !== notice.id);
    const victim =
      older.find((n) => n.kind === 'info') ?? older.find((n) => n.kind === 'warning') ?? older[0];
    if (!victim) break;
    dismiss(victim.id);
  }

  arm(notice);
  return notice.id;
}

/** An error, with what went wrong underneath it. */
export function notifyError(title: string, e: unknown, action?: NoticeAction): number {
  return notify({ kind: 'error', title, detail: errorText(e), action: action ?? null });
}

export function dismiss(id: number): void {
  const timer = timers.get(id);
  if (timer) {
    clearTimeout(timer);
    timers.delete(id);
  }
  const i = notices.items.findIndex((n) => n.id === id);
  if (i >= 0) notices.items.splice(i, 1);
}

export function dismissAll(): void {
  for (const id of [...notices.items.map((n) => n.id)]) dismiss(id);
}

/** Start, or restart, the countdown for a notice that dismisses itself. */
function arm(notice: Notice): void {
  const existing = timers.get(notice.id);
  if (existing) clearTimeout(existing);
  timers.delete(notice.id);
  if (notice.kind !== 'info') return;
  timers.set(
    notice.id,
    setTimeout(() => dismiss(notice.id), INFO_TIMEOUT_MS),
  );
}
