/// <reference lib="webworker" />
/**
 * Web Worker hosting the WASM engine so layout never blocks the UI thread
 * (prompt §5.7).
 */
import init, { WasmEngine } from '@blueoffice/engine-wasm';
import wasmUrl from '@blueoffice/engine-wasm/wasm?url';
import type { EngineRequest, EngineResponse } from './protocol';

declare const self: DedicatedWorkerGlobalScope;

let engine: WasmEngine | null = null;

async function handle(req: EngineRequest): Promise<{ result: unknown; transfer?: Transferable[] }> {
  if (req.method === 'init') {
    if (!engine) {
      await init({ module_or_path: wasmUrl });
      engine = new WasmEngine();
      for (const f of req.fonts) engine.addFont(f);
    }
    return { result: engine.faceCount() };
  }
  if (!engine) throw new Error('engine.notInitialized');
  switch (req.method) {
    case 'open':
      return { result: JSON.parse(engine.open(req.bytes, req.name)) };
    case 'page':
      return { result: JSON.parse(engine.pageJson(req.index)) };
    case 'glyphs':
      return { result: JSON.parse(engine.glyphOutlines(req.face, Uint16Array.from(req.glyphs))) };
    case 'pdf': {
      const pdf = engine.exportPdf();
      return { result: pdf, transfer: [pdf.buffer] };
    }
  }
}

self.onmessage = async (ev: MessageEvent<EngineRequest>) => {
  const req = ev.data;
  try {
    const { result, transfer } = await handle(req);
    const msg: EngineResponse = { id: req.id, ok: true, result };
    self.postMessage(msg, transfer ?? []);
  } catch (e) {
    const msg: EngineResponse = {
      id: req.id,
      ok: false,
      error: e instanceof Error ? e.message : String(e),
    };
    self.postMessage(msg);
  }
};
