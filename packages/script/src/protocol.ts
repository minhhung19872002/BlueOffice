/** Messages between the editor and the BlueScript worker (CORE-SCRIPT-001). */
import type { LogLevel, RunResult, ScriptApp, ScriptLimits } from './sandbox';

export type ToWorker =
  | {
      type: 'run';
      code: string;
      app: ScriptApp;
      limits: Partial<ScriptLimits>;
      /** URL of QuickJS' `emscripten-module.wasm`. */
      wasmUrl?: string;
    }
  | { type: 'reply'; id: number; ok: true; value: unknown }
  | { type: 'reply'; id: number; ok: false; error: string };

export type FromWorker =
  | { type: 'call'; id: number; method: string; args: unknown[] }
  | { type: 'log'; level: LogLevel; text: string }
  | { type: 'done'; result: RunResult };
