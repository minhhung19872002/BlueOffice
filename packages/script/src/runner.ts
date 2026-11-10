/**
 * Running a BlueScript (CORE-SCRIPT-001). The default runner starts a dedicated
 * Web Worker per run that hosts QuickJS: a runaway script never blocks the UI and
 * Stop terminates the worker outright. Where workers are unavailable (tests,
 * very old web views) the inline runner runs QuickJS on this thread with the
 * same limits.
 *
 * Host calls are answered one at a time, in the order the script made them, and
 * the run is bracketed by `host.begin()` / `host.end()` (one undo step).
 */
import quickjsWasmUrl from '@jitl/quickjs-wasmfile-release-sync/wasm?url';
import type { ScriptHost } from './host';
import {
  DEFAULT_LIMITS,
  loadQuickJS,
  runSandboxed,
  type LogLevel,
  type RunResult,
  type ScriptCall,
  type ScriptLimits,
} from './sandbox';
import type { FromWorker, ToWorker } from './protocol';

export interface ScriptRunOptions extends Partial<ScriptLimits> {
  log?: (level: LogLevel, text: string) => void;
}

export interface ScriptRun {
  readonly done: Promise<RunResult>;
  /** Cancel the run (the result reports `cancelled`). */
  stop(): void;
}

export interface ScriptRunner {
  run(code: string, host: ScriptHost, opts?: ScriptRunOptions): ScriptRun;
}

/** Answers the script's calls one at a time, in order. */
function serial(call: ScriptCall): ScriptCall {
  let tail: Promise<unknown> = Promise.resolve();
  return (method, args) => {
    const p = tail.then(() => call(method, args));
    tail = p.catch(() => undefined);
    return p;
  };
}

const failed = (message: string, kind: 'error' | 'cancelled' | 'timeout' = 'error'): RunResult => ({
  ok: false,
  error: { kind, message },
  calls: 0,
  ms: 0,
});

async function bracket(host: ScriptHost, body: () => Promise<RunResult>): Promise<RunResult> {
  try {
    await host.begin?.();
  } catch (e) {
    return failed(e instanceof Error ? e.message : String(e));
  }
  try {
    return await body();
  } finally {
    await host.end?.().catch(() => undefined);
  }
}

/** QuickJS on the calling thread. */
export function inlineRunner(wasmLocation?: string): ScriptRunner {
  return {
    run(code, host, opts = {}) {
      const ac = new AbortController();
      const done = bracket(host, async () => {
        const module = await loadQuickJS(wasmLocation);
        const { log, ...limits } = opts;
        return runSandboxed(module, code, {
          ...limits,
          ...(log ? { log } : {}),
          app: host.app,
          call: serial(host.call),
          signal: ac.signal,
        });
      });
      return { done, stop: () => ac.abort() };
    },
  };
}

/** Extra time the worker gets past its own deadline before it is terminated. */
const KILL_GRACE_MS = 2_000;

/** QuickJS in a dedicated worker per run (terminated on Stop or when it overruns). */
export function workerRunner(
  createWorker: () => Worker = () =>
    new Worker(new URL('./script.worker.ts', import.meta.url), { type: 'module' }),
  wasmUrl: string = quickjsWasmUrl,
): ScriptRunner {
  return {
    run(code, host, opts = {}) {
      let stop: () => void = () => {};
      const done = bracket(
        host,
        () =>
          new Promise<RunResult>((resolve) => {
            const { log, ...limits } = opts;
            const timeoutMs = limits.timeoutMs ?? DEFAULT_LIMITS.timeoutMs;
            const call = serial(host.call);
            let worker: Worker;
            try {
              worker = createWorker();
            } catch (e) {
              resolve(failed(e instanceof Error ? e.message : String(e)));
              return;
            }
            let settled = false;
            const end = (r: RunResult) => {
              if (settled) return;
              settled = true;
              clearTimeout(kill);
              worker.terminate();
              resolve(r);
            };
            const kill = setTimeout(
              () => end(failed('timeout', 'timeout')),
              timeoutMs + KILL_GRACE_MS,
            );
            stop = () => end(failed('cancelled', 'cancelled'));
            const post = (m: ToWorker) => worker.postMessage(m);
            worker.onmessage = (e: MessageEvent<FromWorker>) => {
              const m = e.data;
              if (m.type === 'log') log?.(m.level, m.text);
              else if (m.type === 'done') end(m.result);
              else if (m.type === 'call') {
                call(m.method, m.args).then(
                  (value) => !settled && post({ type: 'reply', id: m.id, ok: true, value }),
                  (err: unknown) =>
                    !settled &&
                    post({
                      type: 'reply',
                      id: m.id,
                      ok: false,
                      error: err instanceof Error ? err.message : String(err),
                    }),
                );
              }
            };
            worker.onerror = (e) => {
              e.preventDefault();
              end(failed(e.message || 'worker error'));
            };
            post({ type: 'run', code, app: host.app, limits, wasmUrl });
          }),
      );
      return { done, stop: () => stop() };
    },
  };
}

/** Worker runner where workers exist, else inline. */
export function defaultRunner(): ScriptRunner {
  return typeof Worker === 'function' ? workerRunner() : inlineRunner();
}
