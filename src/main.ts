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
  createApp(App).mount('#app');
}
