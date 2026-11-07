import type {
  DocInfo,
  EditResult,
  EngineHost,
  FontSource,
  GlyphOutline,
  PageDisplayList,
  TimedCommand,
} from '../types';
import type { EngineRequest, EngineResponse } from './protocol';

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };
// Distributive Omit so each union member keeps its own fields.
type RequestBody = EngineRequest extends infer R
  ? R extends EngineRequest
    ? Omit<R, 'id'>
    : never
  : never;

/** Engine running as WASM inside a dedicated Web Worker. */
export class WebEngineHost implements EngineHost {
  readonly backend = 'wasm' as const;
  private worker: Worker | null = null;
  private seq = 0;
  private pending = new Map<number, Pending>();
  private ready: Promise<void> | null = null;

  constructor(
    private readonly fonts: FontSource,
    private readonly createWorker: () => Worker = () =>
      new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' }),
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

  saveDocx(): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'saveDocx' });
  }

  getSlideNotes(index: number): Promise<string> {
    return this.call<string>({ method: 'notes', index });
  }

  savePptx(): Promise<Uint8Array> {
    return this.call<Uint8Array>({ method: 'savePptx' });
  }

  async markSaved(): Promise<void> {
    await this.call<null>({ method: 'markSaved' });
  }

  dispose(): void {
    this.worker?.terminate();
    this.worker = null;
    this.ready = null;
  }
}
