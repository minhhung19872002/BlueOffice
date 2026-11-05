import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react()],
  worker: { format: 'es' },
  server: { port: 5173, strictPort: true },
  build: { target: 'es2022', sourcemap: true },
});
