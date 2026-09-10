import { reactive } from 'vue';

/**
 * Which screen is showing.
 *
 * Three of them, so no router: a router would add a dependency, a build step's worth of code
 * splitting and a URL the user never sees, to replace one string.
 */
export type Screen = 'studio' | 'queue' | 'settings';

export const ui = reactive({
  screen: 'studio' as Screen,
});

export function goTo(screen: Screen): void {
  ui.screen = screen;
}
