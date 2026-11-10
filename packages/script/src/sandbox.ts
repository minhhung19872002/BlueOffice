/**
 * BlueScript sandbox (CORE-SCRIPT-001): runs a user script in a fresh QuickJS
 * runtime compiled to WebAssembly (quickjs-emscripten, MIT; ADR-0012).
 *
 * The script sees only ECMAScript built-ins plus the object API defined by the
 * prelude (`doc`, `workbook` / `sheet`, `presentation`, `console`). There is no
 * DOM, `fetch`, `postMessage`, timers, storage or platform API inside QuickJS:
 * the only way out is `__bo_call(method, json)`, which the host answers through
 * {@link SandboxOptions.call} (an allow-listed API dispatcher). Values cross the
 * boundary as JSON strings, so no host object is ever reachable from the script.
 *
 * Limits: CPU/wall time (interrupt handler + host timer), heap (`setMemoryLimit`),
 * stack, number of API calls and console output; `signal` cancels a run.
 */
import releaseSync from '@jitl/quickjs-wasmfile-release-sync';
import {
  newQuickJSWASMModuleFromVariant,
  newVariant,
  type QuickJSContext,
  type QuickJSDeferredPromise,
  type QuickJSHandle,
  type QuickJSRuntime,
  type QuickJSWASMModule,
} from 'quickjs-emscripten-core';
import { errorText } from './host';
import { PRELUDE } from './prelude';
import { wrapScript } from './wrap';

/** Default limits of a run. */
export const DEFAULT_LIMITS = {
  /** Wall time of the whole run, host calls included. */
  timeoutMs: 30_000,
  /** QuickJS heap. */
  memoryBytes: 64 * 1024 * 1024,
  /** Native stack of the interpreter. */
  stackBytes: 512 * 1024,
  /** API calls to the host. */
  maxCalls: 200_000,
  /** Console lines kept. */
  maxLogLines: 2_000,
} as const;

export type ScriptLimits = { -readonly [K in keyof typeof DEFAULT_LIMITS]: number };

export type LogLevel = 'log' | 'info' | 'warn' | 'error';

/** Why a run did not finish normally. */
export type ScriptErrorKind = 'syntax' | 'error' | 'timeout' | 'memory' | 'cancelled' | 'limit';

export interface ScriptError {
  kind: ScriptErrorKind;
  message: string;
  /** Script stack trace, when QuickJS has one. */
  stack?: string;
}

export interface RunResult {
  ok: boolean;
  /** JSON value the script returned (`return …` at top level). */
  value?: unknown;
  error?: ScriptError;
  /** API calls made. */
  calls: number;
  ms: number;
}

/** Host side of the bridge: one API call (method name + JSON arguments). */
export type ScriptCall = (method: string, args: unknown[]) => Promise<unknown>;

export interface SandboxOptions extends Partial<ScriptLimits> {
  call: ScriptCall;
  log?: (level: LogLevel, text: string) => void;
  signal?: AbortSignal;
  /** Which application globals the prelude defines. */
  app: ScriptApp;
}

export type ScriptApp = 'doc' | 'sheet' | 'slide';

let modulePromise: Promise<QuickJSWASMModule> | null = null;

/**
 * The QuickJS WebAssembly module (one instance, reused by runs). `wasmLocation`:
 * URL of `emscripten-module.wasm` (bundlers; default: next to the loader).
 */
export function loadQuickJS(wasmLocation?: string): Promise<QuickJSWASMModule> {
  modulePromise ??= newQuickJSWASMModuleFromVariant(
    wasmLocation ? newVariant(releaseSync, { wasmLocation }) : releaseSync,
  ).catch((e: unknown) => {
    modulePromise = null;
    throw e;
  });
  return modulePromise;
}

/** Forget the module (after a run could not be cleaned up): the next run gets a fresh instance. */
export function resetQuickJS(): void {
  modulePromise = null;
}

export { WRAP_LINES, wrapScript } from './wrap';

/**
 * Runs `code` in a fresh QuickJS runtime of `module`. Never throws: every failure
 * (syntax, exception, limit, cancellation) comes back in {@link RunResult.error}.
 */
export async function runSandboxed(
  module: QuickJSWASMModule,
  code: string,
  opts: SandboxOptions,
): Promise<RunResult> {
  const limits: ScriptLimits = { ...DEFAULT_LIMITS };
  for (const k of Object.keys(DEFAULT_LIMITS) as (keyof ScriptLimits)[]) {
    const v = opts[k];
    if (typeof v === 'number' && v > 0) limits[k] = v;
  }
  const start = Date.now();
  const deadline = start + limits.timeoutMs;
  let stopped: 'timeout' | 'cancelled' | 'limit' | null = null;
  let calls = 0;
  let logLines = 0;
  const log = (level: LogLevel, text: string) => {
    if (logLines >= limits.maxLogLines) return;
    logLines++;
    opts.log?.(level, logLines === limits.maxLogLines ? '…' : text.slice(0, 10_000));
  };
  if (opts.signal?.aborted) {
    return { ok: false, error: { kind: 'cancelled', message: 'cancelled' }, calls, ms: 0 };
  }

  let runtime: QuickJSRuntime | null = null;
  let vm: QuickJSContext | null = null;
  const pending = new Set<QuickJSDeferredPromise>();
  let alive = true;
  // Wakes the waiting loop below when the host stops the run.
  let wake: () => void = () => {};
  const stopWait = new Promise<void>((r) => (wake = r));
  const stop = (why: 'timeout' | 'cancelled' | 'limit') => {
    stopped ??= why;
    wake();
  };
  const timer = setTimeout(() => stop('timeout'), limits.timeoutMs);
  const onAbort = () => stop('cancelled');
  opts.signal?.addEventListener('abort', onAbort);

  const finish = (r: Omit<RunResult, 'calls' | 'ms'>): RunResult => ({
    ...r,
    calls,
    ms: Date.now() - start,
  });
  const stoppedError = (): ScriptError => ({
    kind: stopped ?? 'error',
    message: stopped ?? 'stopped',
  });

  try {
    runtime = module.newRuntime();
    runtime.setMemoryLimit(limits.memoryBytes);
    runtime.setMaxStackSize(limits.stackBytes);
    runtime.setInterruptHandler(() => {
      if (opts.signal?.aborted) stopped ??= 'cancelled';
      else if (Date.now() > deadline) stopped ??= 'timeout';
      return stopped !== null;
    });
    const ctx = runtime.newContext();
    vm = ctx;
    const rt = runtime;
    const pump = () => {
      if (!alive) return;
      const r = rt.executePendingJobs();
      if (r.error) r.error.dispose();
    };

    const callFn = ctx.newFunction('__bo_call', (mH, aH) => {
      const method = ctx.getString(mH);
      const json = ctx.getString(aH);
      const d = ctx.newPromise();
      pending.add(d);
      calls++;
      const settle = (ok: boolean, payload: string) => {
        if (!alive || !pending.has(d)) return;
        pending.delete(d);
        if (ok) {
          const h = ctx.newString(payload);
          d.resolve(h);
          h.dispose();
        } else {
          const h = ctx.newError(payload);
          d.reject(h);
          h.dispose();
        }
        d.dispose();
        pump();
      };
      if (calls > limits.maxCalls) {
        stop('limit');
        queueMicrotask(() => settle(false, 'too many API calls'));
      } else {
        let args: unknown[];
        try {
          const parsed: unknown = JSON.parse(json);
          args = Array.isArray(parsed) ? parsed : [parsed];
        } catch {
          args = [];
        }
        Promise.resolve()
          .then(() => opts.call(method, args))
          .then(
            (v) => settle(true, JSON.stringify(v ?? null)),
            (e: unknown) => settle(false, errorText(e)),
          );
      }
      return d.handle;
    });
    ctx.setProp(ctx.global, '__bo_call', callFn);
    callFn.dispose();
    const logFn = ctx.newFunction('__bo_log', (lH, tH) => {
      const level = ctx.getString(lH);
      log(
        level === 'warn' || level === 'error' || level === 'info' ? level : 'log',
        ctx.getString(tH),
      );
    });
    ctx.setProp(ctx.global, '__bo_log', logFn);
    logFn.dispose();
    const appH = ctx.newString(opts.app);
    ctx.setProp(ctx.global, '__bo_app', appH);
    appH.dispose();

    const pre = ctx.evalCode(PRELUDE, 'bluescript-prelude.js');
    if (pre.error) {
      const e = ctx.dump(pre.error) as unknown;
      pre.error.dispose();
      return finish({ ok: false, error: { kind: 'error', message: `prelude: ${errorText(e)}` } });
    }
    pre.value.dispose();

    const ev = ctx.evalCode(wrapScript(code), 'script.js');
    if (ev.error) {
      const e = dumpError(ctx, ev.error);
      ev.error.dispose();
      if (stopped) return finish({ ok: false, error: stoppedError() });
      return finish({ ok: false, error: { ...e, kind: e.kind === 'error' ? 'syntax' : e.kind } });
    }
    const done = ctx.resolvePromise(ev.value);
    ev.value.dispose();
    pump();
    const outcome = await Promise.race([done, stopWait.then(() => null)]);
    if (outcome === null || stopped) {
      if (outcome && 'value' in outcome && outcome.value) outcome.value.dispose();
      if (outcome && 'error' in outcome && outcome.error) outcome.error.dispose();
      return finish({ ok: false, error: stoppedError() });
    }
    if (outcome.error) {
      const e = dumpError(ctx, outcome.error);
      outcome.error.dispose();
      return finish({ ok: false, error: e });
    }
    const value = ctx.dump(outcome.value) as unknown;
    outcome.value.dispose();
    let plain: unknown;
    try {
      plain = value === undefined ? undefined : (JSON.parse(JSON.stringify(value)) as unknown);
    } catch {
      plain = String(value);
    }
    return finish({ ok: true, value: plain });
  } catch (e) {
    const message = errorText(e);
    if (stopped) return finish({ ok: false, error: stoppedError() });
    return finish({
      ok: false,
      error: { kind: /out of memory/i.test(message) ? 'memory' : 'error', message },
    });
  } finally {
    clearTimeout(timer);
    opts.signal?.removeEventListener('abort', onAbort);
    alive = false;
    for (const d of pending) {
      try {
        d.dispose();
      } catch {
        // already gone with the runtime
      }
    }
    pending.clear();
    try {
      vm?.dispose();
      runtime?.dispose();
    } catch {
      // A runtime that cannot be freed (stopped mid-call) poisons the module.
      resetQuickJS();
    }
  }
}

function dumpError(ctx: QuickJSContext, h: QuickJSHandle): ScriptError {
  const v = ctx.dump(h) as unknown;
  let message: string;
  let stack: string | undefined;
  let name = '';
  if (v && typeof v === 'object') {
    const o = v as { name?: unknown; message?: unknown; stack?: unknown };
    name = typeof o.name === 'string' ? o.name : '';
    message = typeof o.message === 'string' ? o.message : JSON.stringify(v);
    if (typeof o.stack === 'string' && o.stack) stack = o.stack;
  } else {
    message = String(v);
  }
  const text = name && name !== 'Error' ? `${name}: ${message}` : message;
  const kind: ScriptErrorKind = /out of memory/i.test(message)
    ? 'memory'
    : name === 'SyntaxError'
      ? 'syntax'
      : 'error';
  return stack ? { kind, message: text, stack } : { kind, message: text };
}
