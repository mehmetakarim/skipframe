import { fileURLToPath, URL } from 'node:url';
import { resolve } from 'node:path';
import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// Tauri sets these when it spawns the dev server.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  // Tauri expects a fixed port and does not tolerate a fallback.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**', '**/target/**', '**/design/**'] },
  },
  build: {
    // WebView2 (Chromium) on Windows, WKWebView (Safari 16.4+) on macOS.
    target: ['chrome105', 'safari16'],
    minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    // The bundle is loaded from disk inside the app, not downloaded, so the default warning
    // about a 500 kB chunk is measuring something that does not apply here. Three.js is most
    // of it and splitting it would only add a round trip to the first frame.
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      // The splash is its own page so it can draw before the studio's bundle is parsed.
      input: {
        main: resolve(fileURLToPath(new URL('.', import.meta.url)), 'index.html'),
        splash: resolve(fileURLToPath(new URL('.', import.meta.url)), 'splash.html'),
        about: resolve(fileURLToPath(new URL('.', import.meta.url)), 'about.html'),
      },
    },
  },
});
