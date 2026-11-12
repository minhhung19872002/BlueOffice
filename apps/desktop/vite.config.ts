import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Tauri serves this build from its own protocol; the dev server runs on a fixed port.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  // No source maps in the shipped bundle (they would be embedded in the exe).
  build: { target: 'es2022', outDir: 'dist', sourcemap: false },
});
