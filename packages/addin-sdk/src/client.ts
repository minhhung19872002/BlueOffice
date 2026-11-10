/**
 * Client library for add-in pages (CORE-ADDIN-001): typed promises over the
 * postMessage JSON-RPC protocol, plus the `selectionChanged` /
 * `documentChanged` / `command` events. The document API mirrors BlueScript
 * (`packages/script/bluescript.d.ts`): each function is one host method.
 *
 * ```js
 * const bo = await BlueOfficeAddin.connect();
 * const sel = await bo.selection.get();
 * if (bo.app === 'doc') await bo.doc.insertText('Xin chào');
 * ```
 */
import {
  ERROR_CODES,
  PROTOCOL_VERSIONS,
  parseHostMessage,
  plainJson,
  request,
  type AddinEvent,
  type AddinEventType,
  type HelloResult,
  type MessageEventLike,
  type MessageSourceLike,
  type PostTarget,
  type RpcError,
} from './protocol';

/** Error of a host call. `code` is one of {@link ERROR_CODES}. */
export class AddinError extends Error {
  override name = 'AddinError';
  constructor(
    readonly code: number,
    message: string,
    /** Permissions the call needed and the add-in was not granted. */
    readonly missing: string[] = [],
  ) {
    super(message);
  }
}

export interface ConnectOptions {
  /** Where requests go (default `window.parent`). */
  target?: PostTarget;
  /** Where answers come from (default `window`). */
  source?: MessageSourceLike;
  /**
   * Only messages whose `source` is this object are accepted (default: the
   * `target`, i.e. the editor window).
   */
  expectSource?: unknown;
  /** `targetOrigin` of posted messages (default `*`: the editor's origin is not known in advance). */
  targetOrigin?: string;
  /** Per-call timeout (default 30 s). */
  timeoutMs?: number;
}

/** A position in a BlueDoc paragraph (UTF-16 offset in its text). */
export interface DocPosition {
  paragraph: number;
  offset: number;
}

/** What `selection.get()` answers, per application. */
export type AddinSelection =
  | { app: 'doc'; anchor: DocPosition; focus: DocPosition; text: string }
  | {
      app: 'sheet';
      sheet: number;
      sheetName: string;
      /** A1 address of the selected range. */
      address: string;
      /** Address of the active cell. */
      active: string;
      values: (string | number | boolean | null)[][];
    }
  | { app: 'slide'; slide: number; shapes: number[] };

type Handler = (event: AddinEvent) => void;

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: AddinError) => void;
  timer: ReturnType<typeof setTimeout>;
}

/** A connection to the editor. */
export class AddinClient {
  private nextId = 1;
  private pending = new Map<number, Pending>();
  private handlers = new Map<AddinEventType, Set<Handler>>();
  private readonly listener = (e: MessageEventLike) => this.onMessage(e);
  private closed = false;
  /** Filled by {@link connect}. */
  info!: HelloResult;

  constructor(
    private readonly target: PostTarget,
    private readonly source: MessageSourceLike,
    private readonly expectSource: unknown,
    private readonly targetOrigin: string,
    private readonly timeoutMs: number,
  ) {
    source.addEventListener('message', this.listener);
  }

  /** `doc`, `sheet` or `slide`. */
  get app() {
    return this.info.app;
  }

  /** Whether the user granted `permission`. */
  has(permission: string): boolean {
    return this.info.permissions.includes(permission);
  }

  /** One host method (see the API reference); rejects with {@link AddinError}. */
  call<T = unknown>(method: string, ...params: unknown[]): Promise<T> {
    if (this.closed) return Promise.reject(new AddinError(ERROR_CODES.internal, 'closed'));
    const json = plainJson(params);
    if (!Array.isArray(json)) {
      return Promise.reject(
        new AddinError(ERROR_CODES.invalidParams, 'parameters must be plain JSON'),
      );
    }
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new AddinError(ERROR_CODES.internal, `${method}: no answer`));
      }, this.timeoutMs);
      this.pending.set(id, { resolve: resolve as (v: unknown) => void, reject, timer });
      this.target.postMessage(request(id, method, json), this.targetOrigin);
    });
  }

  /** Subscribes to an event; returns the unsubscribe function. */
  on(type: AddinEventType, handler: Handler): () => void {
    let set = this.handlers.get(type);
    if (!set) this.handlers.set(type, (set = new Set()));
    set.add(handler);
    return () => set.delete(handler);
  }

  /** Stops listening; pending calls reject. */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.source.removeEventListener('message', this.listener);
    for (const p of this.pending.values()) {
      clearTimeout(p.timer);
      p.reject(new AddinError(ERROR_CODES.internal, 'closed'));
    }
    this.pending.clear();
  }

  private onMessage(e: MessageEventLike): void {
    if (this.expectSource !== undefined && e.source !== this.expectSource) return;
    const m = parseHostMessage(e.data);
    if (!m) return;
    if (m.kind === 'event') {
      for (const h of this.handlers.get(m.event.type) ?? []) {
        try {
          h(m.event);
        } catch {
          // A failing handler does not stop the others.
        }
      }
      return;
    }
    const r = m.response;
    if (typeof r.id !== 'number') return;
    const p = this.pending.get(r.id);
    if (!p) return;
    this.pending.delete(r.id);
    clearTimeout(p.timer);
    if (r.error) {
      const e2: RpcError = r.error;
      p.reject(new AddinError(e2.code, e2.message, e2.data?.missing ?? []));
    } else p.resolve(r.result);
  }

  // ------------------------------------------------------------- services

  readonly selection = {
    /** The current selection with its content (`selection` permission). */
    get: () => this.call<AddinSelection>('addin.selection'),
  };

  /** Key/value storage of this add-in for this user (`storage` permission). */
  readonly storage = {
    get: <T = unknown>(key: string) => this.call<T | null>('storage.get', key),
    set: (key: string, value: unknown) => this.call<void>('storage.set', key, value),
    remove: (key: string) => this.call<void>('storage.remove', key),
    keys: () => this.call<string[]>('storage.keys'),
  };

  // ------------------------------------------------------------- BlueDoc

  readonly doc = {
    info: () => this.call('doc.info'),
    text: () => this.call<string>('doc.text'),
    paragraphs: () => this.call<{ index: number; text: string }[]>('doc.paragraphs'),
    paragraphStyle: (index: number) => this.call<string>('doc.paragraphStyle', index),
    setParagraphText: (index: number, text: string) =>
      this.call<void>('doc.setParagraphText', index, text),
    setParagraphStyle: (index: number, style: string) =>
      this.call<void>('doc.setParagraphStyle', index, style),
    formatParagraph: (index: number, format: object) =>
      this.call<void>('doc.formatParagraph', index, format),
    setParagraphFormat: (index: number, format: object) =>
      this.call<void>('doc.setParagraphFormat', index, format),
    insertParagraphAfter: (index: number, text: string, style?: string) =>
      this.call<number>('doc.insertParagraphAfter', index, text, style ?? null),
    deleteParagraph: (index: number) => this.call<void>('doc.deleteParagraph', index),
    appendParagraph: (text: string, style?: string) =>
      this.call<number>('doc.appendParagraph', text, style ?? null),
    insertText: (text: string) => this.call<void>('doc.insertText', text),
    select: (paragraph: number, start = 0, end?: number) =>
      this.call<void>('doc.select', paragraph, start, end ?? null),
    selectAll: () => this.call<void>('doc.selectAll'),
    selection: () => this.call<{ anchor: DocPosition; focus: DocPosition }>('doc.selection'),
    format: (format: object) => this.call<void>('doc.format', format),
    find: (query: string, options?: object) => this.call('doc.find', query, options ?? {}),
    replace: (query: string, replacement: string, options?: object) =>
      this.call<number>('doc.replace', query, replacement, options ?? {}),
    insertTable: (rows: number, cols: number, values?: unknown[][]) =>
      this.call<void>('doc.insertTable', rows, cols, values ?? null),
    insertPageBreak: () => this.call<void>('doc.insertPageBreak'),
  };

  // ------------------------------------------------------------- BlueSheet

  /** `sheet` is a 0-based index, or -1 for the active sheet. */
  readonly workbook = {
    sheets: () => this.call<{ index: number; name: string }[]>('sheet.list'),
    activeSheet: () => this.call<{ index: number; name: string }>('sheet.active'),
    addSheet: (name?: string) =>
      this.call<{ index: number; name: string }>('sheet.add', name ?? null),
    renameSheet: (sheet: number, name: string) => this.call<void>('sheet.rename', sheet, name),
    getValues: (sheet: number, address: string) =>
      this.call<(string | number | boolean | null)[][]>('sheet.getValues', sheet, address),
    setValues: (sheet: number, address: string, values: unknown[][]) =>
      this.call<void>('sheet.setValues', sheet, address, values),
    getFormulas: (sheet: number, address: string) =>
      this.call<string[][]>('sheet.getFormulas', sheet, address),
    setFormulas: (sheet: number, address: string, formulas: string[][]) =>
      this.call<void>('sheet.setFormulas', sheet, address, formulas),
    fill: (sheet: number, address: string, value: unknown) =>
      this.call<void>('sheet.fill', sheet, address, value),
    format: (sheet: number, address: string, patch: object) =>
      this.call<void>('sheet.format', sheet, address, patch),
    clear: (sheet: number, address: string, what: 'all' | 'contents' | 'formats' = 'all') =>
      this.call<void>('sheet.clear', sheet, address, what),
  };

  // ------------------------------------------------------------- BlueSlide

  readonly presentation = {
    info: () => this.call<{ slideCount: number }>('slide.info'),
    shapes: (slide: number) => this.call('slide.shapes', slide),
    notes: (slide: number) => this.call<string>('slide.getNotes', slide),
    setNotes: (slide: number, text: string) => this.call<void>('slide.setNotes', slide, text),
    addSlide: (after?: number, layout?: string) =>
      this.call('slide.addSlide', after ?? null, layout ?? null),
    addTextBox: (slide: number, text: string, rect?: object) =>
      this.call('slide.addTextBox', slide, text, rect ?? null),
    setText: (slide: number, shape: number, text: string) =>
      this.call<void>('slide.setText', slide, shape, text),
  };
}

const browserWindow = (): (PostTarget & MessageSourceLike & { parent: PostTarget }) | null =>
  typeof window === 'undefined'
    ? null
    : (window as unknown as PostTarget & MessageSourceLike & { parent: PostTarget });

/**
 * Connects to the editor that loaded this page: sends `addin.hello` and
 * resolves once the editor answers (rejects when it does not within the timeout).
 */
export async function connect(opts: ConnectOptions = {}): Promise<AddinClient> {
  const w = browserWindow();
  const target = opts.target ?? w?.parent;
  const source = opts.source ?? w;
  if (!target || !source) throw new AddinError(ERROR_CODES.internal, 'no editor window');
  const client = new AddinClient(
    target,
    source,
    'expectSource' in opts ? opts.expectSource : target,
    opts.targetOrigin ?? '*',
    opts.timeoutMs ?? 30_000,
  );
  try {
    client.info = await client.call<HelloResult>('addin.hello', {
      protocols: [...PROTOCOL_VERSIONS],
    });
  } catch (e) {
    client.close();
    throw e;
  }
  return client;
}
