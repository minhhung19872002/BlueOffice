/// <reference lib="webworker" />
/**
 * Web Worker hosting the WASM engine so layout never blocks the UI thread
 * (prompt §5.7).
 */
import init, {
  revocationRequests,
  signPrepare,
  verifySignatures,
  WasmEngine,
  type WasmPreparedSignature,
} from '@blueoffice/engine-wasm';
import wasmUrl from '@blueoffice/engine-wasm/wasm?url';
import type { EngineRequest, EngineResponse } from './protocol';

declare const self: DedicatedWorkerGlobalScope;

let engine: WasmEngine | null = null;
/** Signatures waiting for their value (two-phase signing, ADR-0007). */
const prepared = new Map<number, WasmPreparedSignature>();
let nextSession = 0;

function session(id: number): WasmPreparedSignature {
  const p = prepared.get(id);
  if (!p) throw new Error('sign.noSession');
  return p;
}

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
      return { result: JSON.parse(engine.open(req.bytes, req.name, req.password)) };
    case 'loadDictionary':
      engine.loadDictionary(req.aff, req.dic);
      return { result: null };
    case 'page':
      return { result: JSON.parse(engine.pageJson(req.index)) };
    case 'glyphs':
      return { result: JSON.parse(engine.glyphOutlines(req.face, Uint16Array.from(req.glyphs))) };
    case 'pdf': {
      const pdf = engine.exportPdf();
      return { result: pdf, transfer: [pdf.buffer] };
    }
    case 'new':
      return { result: JSON.parse(engine.newDocument(req.name)) };
    case 'command':
      return { result: JSON.parse(engine.command(req.json)) };
    case 'insertImage':
      return { result: JSON.parse(engine.insertImage(req.bytes, req.name)) };
    case 'media': {
      const bytes = engine.media(req.name);
      return { result: bytes, transfer: [bytes.buffer] };
    }
    case 'saveDocx': {
      const bytes = engine.saveDocx();
      return { result: bytes, transfer: [bytes.buffer] };
    }
    case 'notes':
      return { result: engine.slideNotes(req.index) };
    case 'savePptx': {
      const bytes = engine.savePptx();
      return { result: bytes, transfer: [bytes.buffer] };
    }
    case 'slideCommand':
      return { result: JSON.parse(engine.slideCommand(req.json)) };
    case 'slideInsertPicture':
      return { result: JSON.parse(engine.slideInsertPicture(req.bytes, req.name)) };
    case 'markSaved':
      engine.markSaved();
      return { result: null };
    case 'collabStart':
      return { result: JSON.parse(engine.collabStart(req.seed, req.clientId)) };
    case 'collabApply':
      return { result: JSON.parse(engine.collabApply(req.json)) };
    case 'collabPeers':
      return { result: JSON.parse(engine.collabPeers(req.json)) };
    case 'collabStop':
      engine.collabStop();
      return { result: null };
    case 'slideCollabStart':
      return { result: JSON.parse(engine.slideCollabStart(req.seed, req.clientId)) };
    case 'slideCollabApply':
      return { result: JSON.parse(engine.slideCollabApply(req.json)) };
    case 'slideCollabPeers':
      return { result: JSON.parse(engine.slideCollabPeers(req.json)) };
    case 'videoBegin':
      return { result: JSON.parse(engine.videoBegin(req.json)) };
    case 'videoFrame': {
      const bytes = engine.videoFrame(req.index, req.png);
      return { result: bytes, transfer: [bytes.buffer] };
    }
    case 'videoGifAdd':
      engine.videoGifAdd(req.index);
      return { result: null };
    case 'videoGifFinish': {
      const bytes = engine.videoGifFinish();
      return { result: bytes, transfer: [bytes.buffer] };
    }
    case 'videoEnd':
      engine.videoEnd();
      return { result: null };
    case 'signPrepare': {
      const p = signPrepare(req.bytes, req.certsPem, req.options);
      const id = ++nextSession;
      prepared.set(id, p);
      return { result: { id, digest: p.digest() } };
    }
    case 'signTimestampRequest':
      return { result: session(req.session).timestampRequest(req.signature, req.nonce) };
    case 'signFinish': {
      const p = session(req.session);
      prepared.delete(req.session);
      try {
        const bytes = p.finish(req.signature, req.tsResponse);
        return { result: bytes, transfer: [bytes.buffer] };
      } finally {
        p.free();
      }
    }
    case 'verifySignatures':
      return {
        result: JSON.parse(
          verifySignatures(req.bytes, req.anchorsPem, req.nowSecs, JSON.stringify(req.revocation)),
        ),
      };
    case 'revocationRequests':
      return { result: JSON.parse(revocationRequests(req.bytes, req.anchorsPem, req.nowSecs)) };
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
