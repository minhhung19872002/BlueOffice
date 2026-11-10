/**
 * Entry of the headless bundle (CORE-SCRIPT-004), built by
 * `scripts/build-headless.mjs` into `crates/bo-script/js/bluescript-headless.js`
 * and evaluated by the native runtime before the prelude. It publishes one
 * configurable global, `__bo_headless`, which the runtime reads and deletes
 * before any user code runs.
 */
import { headlessCall, headlessScriptHost, type EngineBridge } from './headless';
import { PRELUDE } from './prelude';
import { WRAP_LINES, wrapScript } from './wrap';
import type { ScriptApp } from './sandbox';

Object.defineProperty(globalThis, '__bo_headless', {
  configurable: true,
  enumerable: false,
  writable: false,
  value: Object.freeze({
    prelude: PRELUDE,
    wrap: wrapScript,
    wrapLines: WRAP_LINES,
    create: (app: ScriptApp, engine: EngineBridge, decimal: ',' | '.', activeSheet: number) =>
      headlessCall(headlessScriptHost(app, engine, { decimal, activeSheet })),
  }),
});
