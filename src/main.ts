import { createApp } from 'vue';

import App from './App.vue';
import './styles/base.css';

// The phase-0 harness is a separate entry point so it can be opened in a plain browser tab as
// well as inside the app, and so it never ships as part of the studio's component tree.
// VITE_BENCH lets `tauri dev` open straight into the harness, since the Tauri window is
// loaded without a hash.
const isBench = location.hash.startsWith('#bench') || import.meta.env.VITE_BENCH === '1';

if (isBench) {
  import('./bench/BenchApp.vue').then(({ default: BenchApp }) => {
    createApp(BenchApp).mount('#app');
  });
} else {
  // `#preview` opens the studio against a synthetic model, so the screens can be worked on in a
  // browser tab without the desktop shell. Development only; it is compiled out of a build.
  if (import.meta.env.DEV && location.hash.startsWith('#preview')) {
    void Promise.all([
      import('./bench/syntheticIr'),
      import('./stores/project'),
      import('./stores/queue'),
      import('./bench/previewJobs'),
    ]).then(([{ makeSyntheticIr }, { adoptIr }, { queue }, { seedPreviewJobs }]) => {
      adoptIr(makeSyntheticIr(570, 400));
      queue.outputDir = '/Users/mk/Movies/SkipFrame';
      seedPreviewJobs();
    });
  }
  // Preferences decide where renders land and whether a folder is being watched, so they are
  // read before anything can act on their defaults.
  void import('./stores/settings').then(({ loadSettings }) => loadSettings());
  void import('./queue/watchBridge').then(({ installWatchBridge }) => installWatchBridge());
  createApp(App).mount('#app');
}
