/// <reference lib="webworker" />
/**
 * App-shell service worker of the web editors (prompt §3.4: the web app runs
 * as a PWA and edits offline). Built as `sw.js` next to `index.html`
 * (`vite.config.ts`), which replaces `__BO_PRECACHE__` with the list of every
 * built file (shell, JS chunks, workers, the WASM engine, fonts, the sample)
 * and its version hash — so each build changes `sw.js` itself and the browser
 * installs the new worker (Service Workers §4.4 update: byte-for-byte check).
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
/** Replaced at build time (`vite.config.ts`). */
declare const __BO_PRECACHE__: Precache;

const SHELL = 'bo-shell-';
const SERVER_ROUTES = ['api/', 'collab/', 'hosting/', 'jobs/', 'auth/callback'];

interface Precache {
  version: string;
  files: string[];
}

/** Files and version of this build. */
const PRECACHE: Precache = __BO_PRECACHE__;
const CACHE = SHELL + PRECACHE.version;

const scope = () => new URL(self.registration.scope);

self.addEventListener('install', (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(CACHE);
      await cache.addAll(PRECACHE.files.map((f) => new URL(f, scope()).toString()));
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys()) {
        if (key.startsWith(SHELL) && key !== CACHE) {
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
        const shell = await caches.match(new URL('index.html', scope()).toString(), {
          cacheName: CACHE,
        });
        return shell ?? Response.error();
      }),
    );
    return;
  }
  event.respondWith(
    (async () => {
      // `crossorigin` module scripts send `Origin`; the precached copies were
      // fetched without it, so a `Vary: Origin` response must still match.
      const hit = await caches.match(req, { cacheName: CACHE, ignoreVary: true });
      return hit ?? fetch(req);
    })(),
  );
});
