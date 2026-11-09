import type {
  CollabChange,
  DocInfo,
  EditResult,
  EngineHost,
  FontSource,
  GlyphOutline,
  PageDisplayList,
  PeerRects,
  ShowVideoInfo,
  ShowVideoOptions,
  SignatureLineSignRequest,
  SignatureReport,
  SlideBox,
  SlideCommand,
  SlideEditResult,
  TimedCommand,
} from '../types';
import type { SlideExportOptions } from '../slideExport';
import type { EngineRequest, EngineResponse } from './protocol';
import type { RevocationRequest, SignatureBackend } from './signing';

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };
// Distributive Omit so each union member keeps its own fields.
type RequestBody = EngineRequest extends infer R
  ? R extends EngineRequest
    ? Omit<R, 'id'>
    : never
  : never;

/** Engine running as WASM inside a dedicated Web Worker. */
export class WebEngineHost implements EngineHost, SignatureBackend {
  readonly backend = 'wasm' as const;
  private worker: Worker | null = null;
  private seq = 0;
  private pending = new Map<number, Pending>();
  private ready: Promise<void> | null = null;

  constructor(
    private readonly fonts: FontSource,
    private readonly createWorker: () => Worker = () =>
      new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' }),
    /** Spelling dictionaries (`.aff`, `.dic`) loaded in the background after start-up. */
    private readonly dictionaries?: () => Promise<{ aff: Uint8Array; dic: Uint8Array }[]>,
  ) {}

  private call<T>(body: RequestBody, transfer: Transferable[] = []): Promise<T> {
    if (!this.worker) {
      this.worker = this.createWorker();
      this.worker.onmessage = (ev: MessageEvent<EngineResponse>) => {
        const r = ev.data;
        const p = this.pending.get(r.id);
        if (!p) return;
        this.pending.delete(r.id);
        if (r.ok) p.resolve(r.result);
        else p.reject(new Error(r.error));
      };
      this.worker.onerror = (ev) => {
        const err = new Error(ev.message || 'engine.workerCrashed');
        for (const p of this.pending.values()) p.reject(err);
        this.pending.clear();
      };
    }
    const id = ++this.seq;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
      this.worker!.postMessage({ ...body, id }, transfer);
    });
  }

  init(): Promise<void> {
    this.ready ??= (async () => {
      const list = await this.fonts.list();
      const fonts = await Promise.all(list.map((f) => this.fonts.load(f.file)));
      await this.call<number>(
        { method: 'init', fonts },
        fonts.map((f) => f.buffer as ArrayBuffer),
      );
      // English spelling is optional: Vietnamese works without data, a missing file is ignored.
      void this.dictionaries?.()
        .then((list) =>
          Promise.all(list.map((d) => this.call<null>({ method: 'loadDictionary', ...d }))),
        )
        .catch(() => {});
    })();
    return this.ready;
  }

  async open(bytes: Uint8Array, name: string, password?: string): Promise<DocInfo> {
    await this.init();
    return this.call<DocInfo>({ method: 'open', bytes, name, password });
  }

  async getPage(index: number): Promise<PageDisplayList> {
    return this.call<PageDisplayList>({ method: 'page', index });
  }

  async getGlyphOutlines(face: number, glyphs: number[]): Promise<GlyphOutline[]> {
    return this.call<GlyphOutline[]>({ method: 'glyphs', face, glyphs });
  }

  async exportPdf(): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'pdf' });
  }

  async newDocument(name: string): Promise<DocInfo> {
    await this.init();
    return this.call<DocInfo>({ method: 'new', name });
  }

  command(cmd: TimedCommand): Promise<EditResult> {
    return this.call<EditResult>({ method: 'command', json: JSON.stringify(cmd) });
  }

  insertImage(bytes: Uint8Array, name: string): Promise<EditResult> {
    return this.call<EditResult>({ method: 'insertImage', bytes, name });
  }

  getMedia(name: string): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'media', name });
  }

  // Digital signatures (two-phase, ADR-0007).
  signPrepare(
    bytes: Uint8Array,
    certsPem: string,
    options: {
      timeSecs: number;
      name?: string;
      reason?: string;
      location?: string;
      signatureLine?: SignatureLineSignRequest;
    },
  ): Promise<{ id: number; digest: Uint8Array }> {
    return this.call({ method: 'signPrepare', bytes, certsPem, options: JSON.stringify(options) });
  }
  signTimestampRequest(id: number, signature: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    return this.call({ method: 'signTimestampRequest', session: id, signature, nonce });
  }
  signFinish(id: number, signature: Uint8Array, tsResponse?: Uint8Array): Promise<Uint8Array> {
    return this.call({ method: 'signFinish', session: id, signature, tsResponse });
  }
  verifySignatures(
    bytes: Uint8Array,
    anchorsPem: string,
    nowSecs: number,
    revocation: string[] = [],
  ): Promise<SignatureReport> {
    return this.call({ method: 'verifySignatures', bytes, anchorsPem, nowSecs, revocation });
  }
  revocationRequests(
    bytes: Uint8Array,
    anchorsPem: string,
    nowSecs: number,
  ): Promise<RevocationRequest[]> {
    return this.call({ method: 'revocationRequests', bytes, anchorsPem, nowSecs });
  }

  saveDocx(): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'saveDocx' });
  }

  getSlideNotes(index: number): Promise<string> {
    return this.call<string>({ method: 'notes', index });
  }

  savePptx(): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'savePptx' });
  }

  exportSlides(options: SlideExportOptions): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'exportSlides', json: JSON.stringify(options) });
  }

  slideCommand(cmd: SlideCommand): Promise<SlideEditResult> {
    return this.call<SlideEditResult>({ method: 'slideCommand', json: JSON.stringify(cmd) });
  }

  slideInsertPicture(bytes: Uint8Array, name: string): Promise<SlideEditResult> {
    return this.call<SlideEditResult>({ method: 'slideInsertPicture', bytes, name });
  }

  async markSaved(): Promise<void> {
    await this.call<null>({ method: 'markSaved' });
  }

  collabStart(seed: boolean, clientId?: number): Promise<CollabChange> {
    return this.call<CollabChange>({ method: 'collabStart', seed, clientId });
  }

  collabApply(change: string): Promise<EditResult> {
    return this.call<EditResult>({ method: 'collabApply', json: change });
  }

  collabPeers(selections: [number, number][]): Promise<PeerRects[]> {
    return this.call<PeerRects[]>({ method: 'collabPeers', json: JSON.stringify(selections) });
  }

  async collabStop(): Promise<void> {
    await this.call<null>({ method: 'collabStop' });
  }

  slideCollabStart(seed: boolean, clientId: number): Promise<CollabChange> {
    return this.call<CollabChange>({ method: 'slideCollabStart', seed, clientId });
  }

  slideCollabApply(change: string): Promise<SlideEditResult> {
    return this.call<SlideEditResult>({ method: 'slideCollabApply', json: change });
  }

  slideCollabPeers(keys: string[]): Promise<({ slide: number; rect: SlideBox } | null)[]> {
    return this.call({ method: 'slideCollabPeers', json: JSON.stringify(keys) });
  }

  videoBegin(opts: ShowVideoOptions): Promise<ShowVideoInfo> {
    return this.call<ShowVideoInfo>({ method: 'videoBegin', json: JSON.stringify(opts) });
  }
  videoFrame(index: number, png = false): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'videoFrame', index, png });
  }
  videoGifAdd(index: number): Promise<void> {
    return this.call<void>({ method: 'videoGifAdd', index });
  }
  videoGifFinish(): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'videoGifFinish' });
  }
  videoEnd(): Promise<void> {
    return this.call<void>({ method: 'videoEnd' });
  }

  dispose(): void {
    this.worker?.terminate();
    this.worker = null;
    this.ready = null;
  }
}
