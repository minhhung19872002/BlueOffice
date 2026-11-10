/**
 * Headless BlueScript host (CORE-SCRIPT-004): the same API hosts as the editor
 * (`hosts/doc.ts`, `hosts/sheet.ts`, `hosts/slide.ts`) driven by a synchronous
 * engine bridge instead of the editor's command path. The native runtime
 * (`crates/bo-script`, QuickJS inside `bo-cli`) evaluates the bundle built from
 * `headless-entry.ts`, so browser and server translate every API call into the
 * same engine commands with the same argument checks (ADR-0012 §10).
 */
import type {
  EditCommand,
  EditResult,
  SheetCommand,
  SlideCommand,
  SlideEditResult,
  WorkbookInfo,
  CellDetails,
} from '@blueoffice/platform';
import { errorText, type ScriptHost } from './host';
import { docScriptHost } from './hosts/doc';
import { sheetScriptHost } from './hosts/sheet';
import { slideScriptHost } from './hosts/slide';
import type { ScriptApp } from './sandbox';

/**
 * One synchronous engine call: `op` with JSON array arguments, JSON result
 * (empty string = no value). Ops: `command` (BlueDoc `EditCommand`),
 * `slideCommand` (`SlideCommand`), and for BlueSheet `info`, `cell`, `exec`,
 * `addSheet`, `renameSheet` (the `SheetHost` methods of the same names).
 * Throws with the engine's error message.
 */
export type EngineBridge = (op: string, argsJson: string) => string;

export interface HeadlessOptions {
  /** Decimal separator of BlueSheet input (the engine locale's). */
  decimal?: ',' | '.';
  /** The sheet `sheet` refers to. */
  activeSheet?: number;
}

/** The API host of `app` over `engine`. */
export function headlessScriptHost(
  app: ScriptApp,
  engine: EngineBridge,
  options: HeadlessOptions = {},
): ScriptHost {
  const call = <T>(op: string, ...args: unknown[]): T => {
    const out = engine(op, JSON.stringify(args));
    return (out === '' ? undefined : JSON.parse(out)) as T;
  };
  if (app === 'doc') {
    return docScriptHost({ run: async (cmd: EditCommand) => call<EditResult>('command', cmd) });
  }
  if (app === 'slide') {
    return slideScriptHost({
      run: async (cmd: SlideCommand) => call<SlideEditResult>('slideCommand', cmd),
    });
  }
  const active = options.activeSheet ?? 0;
  return sheetScriptHost({
    host: {
      info: async () => call<WorkbookInfo>('info'),
      cell: async (s: number, r: number, c: number) => call<CellDetails>('cell', s, r, c),
      exec: async <T>(cmd: SheetCommand) => call<T>('exec', cmd),
      addSheet: async (name?: string) => call<number>('addSheet', name ?? null),
      renameSheet: async (s: number, name: string) => {
        call('renameSheet', s, name);
      },
    },
    activeSheet: () => active,
    decimal: options.decimal ?? ',',
  });
}

/**
 * The script-facing bridge `__bo_call(method, argsJson) → Promise<resultJson>`
 * over `host`, with the same contract as the browser sandbox: bad JSON means no
 * arguments, results are JSON (`null` for none) and failures reject with a
 * plain `Error` carrying the message only.
 */
export function headlessCall(host: ScriptHost): (method: string, json: string) => Promise<string> {
  return async (method, json) => {
    let args: unknown[];
    try {
      const parsed: unknown = JSON.parse(json);
      args = Array.isArray(parsed) ? parsed : [parsed];
    } catch {
      args = [];
    }
    let value: unknown;
    try {
      value = await host.call(method, args);
    } catch (e) {
      throw new Error(errorText(e), { cause: e });
    }
    return JSON.stringify(value ?? null);
  };
}
