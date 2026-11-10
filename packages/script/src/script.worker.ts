/// <reference lib="webworker" />
/**
 * BlueScript worker (CORE-SCRIPT-001): one run per worker. The user script runs
 * inside QuickJS, which has no access to this worker's globals (`fetch`,
 * `postMessage`, `importScripts`, IndexedDB …); API calls are relayed to the
 * editor as messages and answered there.
 */
import type { FromWorker, ToWorker } from './protocol';
import { loadQuickJS, runSandboxed } from './sandbox';

const scope = self as unknown as DedicatedWorkerGlobalScope;
const post = (m: FromWorker) => scope.postMessage(m);
const waiting = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
let next = 1;
let started = false;

scope.onmessage = (e: MessageEvent<ToWorker>) => {
  const m = e.data;
  if (m.type === 'reply') {
    const w = waiting.get(m.id);
    waiting.delete(m.id);
    if (!w) return;
    if (m.ok) w.resolve(m.value);
    else w.reject(new Error(m.error));
    return;
  }
  if (started) return;
  started = true;
  void (async () => {
    try {
      const module = await loadQuickJS(m.wasmUrl);
      const result = await runSandboxed(module, m.code, {
        ...m.limits,
        app: m.app,
        log: (level, text) => post({ type: 'log', level, text }),
        call: (method, args) =>
          new Promise((resolve, reject) => {
            const id = next++;
            waiting.set(id, { resolve, reject });
            post({ type: 'call', id, method, args });
          }),
      });
      post({ type: 'done', result });
    } catch (err) {
      post({
        type: 'done',
        result: {
          ok: false,
          error: { kind: 'error', message: err instanceof Error ? err.message : String(err) },
          calls: 0,
          ms: 0,
        },
      });
    }
  })();
};
