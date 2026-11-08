/// <reference lib="webworker" />
/**
 * App-shell service worker of the web editors (prompt §3.4: the web app runs
 * as a PWA and edits offline). Built as `sw.js` next to `index.html`
 * (`vite.config.ts`), with `precache.json` listing every built file (shell,
 * JS chunks, workers, the WASM engine, fonts, the sample).
 *
 * - install: caches the whole list under `bo-shell-<version>`;
 * - activate: drops shell caches of older versions;
 * - navigations: network first, the cached `index.html` when offline (every
 *   route — `/doc`, `/sheet`, `/slide`, `/edit` — is the same single page);
 * - other same-origin GETs: cache first, else the network;
 * - server routes (`/api`, `/collab`, `/hosting`, `/jobs`) are never cached.
 */
export {};

declare const self: ServiceWorkerGlobalScope;

const SHELL = 'bo-shell-';
const SERVER_ROUTES = ['api/', 'collab/', 'hosting/', 'jobs/', 'auth/callback'];

interface Precache {
  version: string;
  files: string[];
}

const scope = () => new URL(self.registration.scope);

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const res = await fetch(new URL('precache.json', scope()), { cache: 'no-store' });
      if (!res.ok) throw new Error(`precache.json ${res.status}`);
      const list = (await res.json()) as Precache;
      const cache = await caches.open(SHELL + list.version);
      await cache.addAll(list.files.map((f) => new URL(f, scope()).toString()));
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      const res = await caches.match(new URL('precache.json', scope()).toString());
      const current = res ? ((await res.json()) as Precache).version : null;
      for (const key of await caches.keys()) {
        if (key.startsWith(SHELL) && current !== null && key !== SHELL + current) {
          await caches.delete(key);
        }
      }
      await self.clients.claim();
    })(),
  );
});

function isServerRoute(url: URL): boolean {
  const base = scope().pathname;
  const path = url.pathname.startsWith(base) ? url.pathname.slice(base.length) : url.pathname;
  return SERVER_ROUTES.some((r) => path.startsWith(r));
}

self.addEventListener('fetch', (event) => {
  const req = event.request;
  const url = new URL(req.url);
  if (req.method !== 'GET' || url.origin !== self.location.origin || isServerRoute(url)) return;
  if (req.mode === 'navigate') {
    event.respondWith(
      fetch(req).catch(async () => {
        const shell = await caches.match(new URL('index.html', scope()).toString());
        return shell ?? Response.error();
      }),
    );
    return;
  }
  event.respondWith(
    (async () => {
      const hit = await caches.match(req);
      return hit ?? fetch(req);
    })(),
  );
});
