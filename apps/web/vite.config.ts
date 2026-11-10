import react from '@vitejs/plugin-react';
import { createHash } from 'node:crypto';
import { readdirSync, statSync } from 'node:fs';
import { join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, defineConfig, type Plugin, type ProxyOptions } from 'vite';

// `BLUEOFFICE_API_PROXY=http://localhost:5000 pnpm dev|preview`: the web shell
// and the BlueOffice server on one origin, like deploy/web/nginx.conf
// (`/api` with the `/api/present` WebSockets, `/hosting`, `/collab` WebSocket); used by the
// collaboration E2E.
const api = process.env.BLUEOFFICE_API_PROXY;
const proxy: Record<string, ProxyOptions> | undefined = api
  ? {
      '/api': { target: api, changeOrigin: true, ws: true },
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
 * PWA (prompt §3.4): builds `src/sw.ts` as `sw.js` and writes into it the
 * list of every emitted file + the public files (fonts, sample, icons) with a
 * version hash (`__BO_PRECACHE__`), cached by the service worker at install.
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
    // After Vite's own plugins have emitted `index.html` and the assets.
    generateBundle: {
      order: 'post',
      handler(_options, bundle) {
        const sw = bundle['sw.js'];
        if (sw?.type !== 'chunk') throw new Error('sw.js was not built');
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
        const precache = JSON.stringify({ version: hash.digest('hex').slice(0, 16), files });
        if (!sw.code.includes('__BO_PRECACHE__')) throw new Error('sw.js lost __BO_PRECACHE__');
        sw.code = sw.code.replaceAll('__BO_PRECACHE__', precache);
      },
    },
  };
}

/**
 * Add-in SDK for add-in pages (CORE-ADDIN-001): `packages/addin-sdk/src/browser.ts`
 * built on its own as a classic script, `addin-sdk/v1.js` (global
 * `BlueOfficeAddin`), so pages on other origins load it with a plain
 * `<script src>` (no CORS, no bundler).
 */
function addinSdk(): Plugin {
  let outDir = '';
  return {
    name: 'blueoffice-addin-sdk',
    apply: 'build',
    configResolved(c) {
      outDir = resolve(c.root, c.build.outDir);
    },
    async closeBundle() {
      await build({
        configFile: false,
        publicDir: false,
        logLevel: 'warn',
        build: {
          outDir: join(outDir, 'addin-sdk'),
          emptyOutDir: false,
          target: 'es2020',
          sourcemap: false,
          lib: {
            entry: fileURLToPath(
              new URL('../../packages/addin-sdk/src/browser.ts', import.meta.url),
            ),
            formats: ['iife'],
            name: 'BlueOfficeAddin',
            fileName: () => 'v1.js',
          },
        },
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), serviceWorker(), addinSdk()],
  worker: { format: 'es' },
  server: { port: 5173, strictPort: true, ...(proxy ? { proxy } : {}) },
  preview: proxy ? { proxy } : {},
  build: { target: 'es2022', sourcemap: true },
});
