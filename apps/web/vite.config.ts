import react from '@vitejs/plugin-react';
import { createHash } from 'node:crypto';
import { readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { defineConfig, type Plugin, type ProxyOptions } from 'vite';

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

/** Every file below `dir`, as `/`-separated paths relative to it. */
function listFiles(dir: string, root = dir): string[] {
  let out: string[] = [];
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out = out.concat(listFiles(p, root));
    else out.push(relative(root, p).split(sep).join('/'));
  }
  return out;
}

/**
 * PWA (prompt §3.4): builds `src/sw.ts` as `sw.js` and writes `precache.json`
 * (every emitted file + the public files: fonts, sample, icons) with a
 * version hash, read by the service worker at install.
 */
function serviceWorker(): Plugin {
  let publicDir = '';
  return {
    name: 'blueoffice-service-worker',
    apply: 'build',
    configResolved(c) {
      publicDir = c.publicDir;
    },
    buildStart() {
      this.emitFile({ type: 'chunk', id: 'src/sw.ts', fileName: 'sw.js' });
    },
    generateBundle(_options, bundle) {
      const built = Object.keys(bundle).filter((f) => f !== 'sw.js' && !f.endsWith('.map'));
      const files = [...built, ...(publicDir ? listFiles(publicDir) : [])].sort();
      const hash = createHash('sha256');
      for (const f of files) {
        const item = bundle[f];
        hash.update(f);
        if (item?.type === 'chunk') hash.update(item.code);
        else if (item?.type === 'asset') hash.update(item.source);
        else hash.update(String(statSync(join(publicDir, f)).size));
      }
      this.emitFile({
        type: 'asset',
        fileName: 'precache.json',
        source: JSON.stringify({ version: hash.digest('hex').slice(0, 16), files }),
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), serviceWorker()],
  worker: { format: 'es' },
  server: { port: 5173, strictPort: true, ...(proxy ? { proxy } : {}) },
  preview: proxy ? { proxy } : {},
  build: { target: 'es2022', sourcemap: true },
});
