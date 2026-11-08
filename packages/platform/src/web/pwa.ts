/**
 * Progressive Web App (prompt §3.4 `offline`: "web = IndexedDB + Service
 * Worker"): registers the app-shell service worker built by `apps/web`
 * (`sw.js`), which caches the shell, the WASM engine, workers and fonts so the
 * editors start and open local files without a network.
 */

/** Registers `url` for `scope`; resolves `null` when service workers are unavailable (or refused). */
export async function registerServiceWorker(
  url: string,
  scope: string,
): Promise<ServiceWorkerRegistration | null> {
  if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return null;
  try {
    return await navigator.serviceWorker.register(url, { scope, type: 'module' });
  } catch {
    // Private mode, insecure origin, …: the app still works online.
    return null;
  }
}
