/// <reference lib="webworker" />
/**
 * Web Worker hosting the BlueSheet WASM engine (`WasmSheet`), separate from
 * the document engine worker so a large recalculation never blocks either.
 */
import init, { WasmSheet } from '@blueoffice/engine-wasm';
import wasmUrl from '@blueoffice/engine-wasm/wasm?url';
import type { SheetMethod } from '../sheet';

declare const self: DedicatedWorkerGlobalScope;

export type SheetWorkerRequest =
  | { id: number; method: 'init'; fonts: Uint8Array[]; locale: string }
  | { id: number; method: SheetMethod; args: unknown[] };

let engine: WasmSheet | null = null;

const n = (x: unknown): number => Number(x);
const s = (x: unknown): string => String(x);
const parse = (json: string): unknown => JSON.parse(json);

function dispatch(e: WasmSheet, method: SheetMethod, a: unknown[]): unknown {
  switch (method) {
    case 'open':
      return parse(e.open(a[0] as Uint8Array, s(a[1])));
    case 'newWorkbook':
      return parse(e.newWorkbook(s(a[0])));
    case 'info':
      return parse(e.info());
    case 'render':
      return parse(e.render(n(a[0]), n(a[1]), n(a[2]), n(a[3]), n(a[4])));
    case 'hitTest':
      return parse(e.hitTest(n(a[0]), n(a[1]), n(a[2]), n(a[3]), n(a[4]), n(a[5]), n(a[6])));
    case 'rangeRect':
      return parse(
        e.rangeRect(
          n(a[0]),
          n(a[1]),
          n(a[2]),
          n(a[3]),
          n(a[4]),
          n(a[5]),
          n(a[6]),
          n(a[7]),
          n(a[8]),
        ),
      );
    case 'contentSize': {
      const r = e.contentSize(n(a[0]));
      return [r[0] ?? 0, r[1] ?? 0];
    }
    case 'cell':
      return parse(e.cell(n(a[0]), n(a[1]), n(a[2])));
    case 'setInputs':
      return e.setInputs(n(a[0]), s(a[1]));
    case 'structural':
      return e.structural(n(a[0]), Boolean(a[1]), n(a[2]), n(a[3]), Boolean(a[4]));
    case 'undo':
      return e.undo();
    case 'redo':
      return e.redo();
    case 'addSheet':
      return e.addSheet(a[0] == null ? undefined : s(a[0]));
    case 'renameSheet':
      return e.renameSheet(n(a[0]), s(a[1]));
    case 'deleteSheet':
      return e.deleteSheet(n(a[0]));
    case 'save':
      return e.save(s(a[0]), n(a[1]));
    case 'exportPdf':
      return e.exportPdf();
    case 'exec':
      return parse(e.exec(s(a[0])));
    case 'collabStart':
      return parse(e.collabStart(Boolean(a[0]), n(a[1])));
    case 'collabLocal':
      return parse(e.collabLocal());
    case 'collabApply':
      return parse(e.collabApply(s(a[0])));
    case 'collabStop':
      e.collabStop();
      return null;
    case 'glyphs':
      return parse(e.glyphOutlines(n(a[0]), Uint16Array.from(a[1] as number[])));
  }
}

self.onmessage = async (ev: MessageEvent<SheetWorkerRequest>) => {
  const req = ev.data;
  try {
    let result: unknown;
    if (req.method === 'init') {
      if (!engine) {
        await init({ module_or_path: wasmUrl });
        engine = new WasmSheet();
        engine.setLocale(req.locale);
        for (const f of req.fonts) engine.addFont(f);
      }
      result = null;
    } else {
      if (!engine) throw new Error('engine.notInitialized');
      engine.setNow(Date.now());
      result = dispatch(engine, req.method, req.args);
    }
    const transfer = result instanceof Uint8Array ? [result.buffer as ArrayBuffer] : [];
    self.postMessage({ id: req.id, ok: true, result }, transfer);
  } catch (e) {
    self.postMessage({ id: req.id, ok: false, error: e instanceof Error ? e.message : String(e) });
  }
};
