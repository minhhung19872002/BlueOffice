import react from '@vitejs/plugin-react';
import { defineConfig, type ProxyOptions } from 'vite';

// `BLUEOFFICE_API_PROXY=http://localhost:5000 pnpm dev|preview`: the web shell
// and the BlueOffice server on one origin, like deploy/web/nginx.conf
// (`/api`, `/hosting`, `/collab` WebSocket); used by the collaboration E2E.
const api = process.env.BLUEOFFICE_API_PROXY;
const proxy: Record<string, ProxyOptions> | undefined = api
  ? {
      '/api': { target: api, changeOrigin: true },
      '/hosting': { target: api, changeOrigin: true },
      '/collab': { target: api, changeOrigin: true, ws: true },
    }
  : undefined;

export default defineConfig({
  plugins: [react()],
  worker: { format: 'es' },
  server: { port: 5173, strictPort: true, ...(proxy ? { proxy } : {}) },
  preview: proxy ? { proxy } : {},
  build: { target: 'es2022', sourcemap: true },
});
