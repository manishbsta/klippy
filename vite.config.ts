import { defineConfig } from 'vite';
import solidPlugin from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solidPlugin()],
  resolve: {
    alias: {
      'solid-js/web': 'solid-js/web/dist/web.js',
    },
    conditions: ['browser'],
  },
  build: {
    target: 'esnext',
  },
});
