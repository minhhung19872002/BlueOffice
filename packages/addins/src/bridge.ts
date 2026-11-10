/**
 * Host end of one add-in frame (CORE-ADDIN-001, ADR-0013). Every message from
 * the frame is untrusted: it is schema-checked ({@link parseRequest}), matched
 * against the permissions the user granted, rate-limited, and only then turned
 * into a BlueScript API call ({@link ScriptHost}, ADR-0012) — the same
 * allow-listed dispatcher and command bridge the editor's own scripts use.
 * A call that changes the document is one undo step.
 */
import {
  ERROR_CODES,
  METHOD_PERMISSIONS,
  PROTOCOL_VERSIONS,
  errorResponse,
  isWriteMethod,
  methodApp,
  missingPermissions,
  notification,
  parseRequest,
  plainJson,
  response,
  type AddinApp,
  type AddinEvent,
  type AddinManifest,
  type HelloResult,
  type MessageEventLike,
  type PostTarget,
  type RpcError,
  type RpcRequest,
} from '@blueoffice/addin-sdk';
import type { ScriptHost } from '@blueoffice/script';
import { AddinStorageError, type AddinStorage } from './storage';

export interface RateLimit {
  /** Calls refilled per second. */
  perSecond: number;
  /** Calls allowed in a burst. */
  burst: number;
}

export const DEFAULT_RATE_LIMIT: RateLimit = { perSecond: 20, burst: 60 };

/** Events kept while the add-in has not said hello yet. */
const MAX_QUEUED_EVENTS = 20;

export interface AddinBridgeOptions {
  manifest: AddinManifest;
  app: AddinApp;
  /** Permissions the user consented to (intersected with the manifest's). */
  granted: readonly string[];
  /** The frame's window: answers go there, and only messages from it are handled. */
  frame: PostTarget;
  /** The editor's UI language. */
  language: () => string;
  /** BlueScript host of the open document; null while none is open. */
  host: () => ScriptHost | null;
  /** `addin.selection`: the selection with its content. */
  selection?: () => Promise<unknown>;
  storage: AddinStorage;
  rateLimit?: RateLimit;
  /** Clock in ms (tests). */
  now?: () => number;
  /** Every handled call (audit, tests): method and outcome code (0 = ok). */
  onCall?: (method: string, code: number) => void;
}

const ok = 0;

export class AddinBridge {
  private readonly granted: string[];
  private tokens: number;
  private last: number;
  private queue: Promise<void> = Promise.resolve();
  private ready = false;
  private pendingEvents: AddinEvent[] = [];
  private disposed = false;

  constructor(private readonly o: AddinBridgeOptions) {
    this.granted = o.granted.filter((p) => (o.manifest.permissions as string[]).includes(p));
    const rl = o.rateLimit ?? DEFAULT_RATE_LIMIT;
    this.tokens = rl.burst;
    this.last = this.now();
  }

  private now(): number {
    return this.o.now?.() ?? Date.now();
  }

  /** Granted permissions in effect. */
  get permissions(): readonly string[] {
    return this.granted;
  }

  /** The add-in has completed the handshake. */
  get connected(): boolean {
    return this.ready;
  }

  /** Methods this add-in may call here. */
  allowedMethods(): string[] {
    return Object.keys(METHOD_PERMISSIONS).filter((m) => {
      const a = methodApp(m);
      return (a === null || a === this.o.app) && missingPermissions(m, this.granted)?.length === 0;
    });
  }

  /** Handles a `message` event of the editor window (ignores other sources). */
  handle(e: MessageEventLike): void {
    if (this.disposed || e.source !== this.o.frame) return;
    const parsed = parseRequest(e.data);
    if (!parsed) return;
    if (!parsed.ok) {
      this.post(errorResponse(parsed.id, parsed.error));
      return;
    }
    const req = parsed.request;
    if (!this.take()) {
      this.reply(req, null, { code: ERROR_CODES.rateLimited, message: 'too many calls' });
      return;
    }
    // One call at a time, in the order the add-in made them.
    this.queue = this.queue.then(() => this.dispatch(req)).catch(() => undefined);
  }

  /** Sends an event if the add-in may see it (queued until its handshake). */
  emit(event: AddinEvent): void {
    if (this.disposed) return;
    const need =
      event.type === 'selectionChanged'
        ? 'selection'
        : event.type === 'documentChanged'
          ? 'read-document'
          : 'ui:ribbon';
    if (!this.granted.includes(need)) return;
    if (!this.ready) {
      if (this.pendingEvents.length < MAX_QUEUED_EVENTS) this.pendingEvents.push(event);
      return;
    }
    this.post(notification(event));
  }

  /** Stops answering (the frame was closed or the add-in uninstalled). */
  dispose(): void {
    this.disposed = true;
    this.pendingEvents = [];
  }

  /** Waits for the calls received so far (tests). */
  idle(): Promise<void> {
    return this.queue;
  }

  private take(): boolean {
    const rl = this.o.rateLimit ?? DEFAULT_RATE_LIMIT;
    const t = this.now();
    this.tokens = Math.min(rl.burst, this.tokens + ((t - this.last) / 1000) * rl.perSecond);
    this.last = t;
    if (this.tokens < 1) return false;
    this.tokens -= 1;
    return true;
  }

  private post(msg: unknown): void {
    if (this.disposed) return;
    try {
      // The frame's origin is opaque (sandbox without allow-same-origin): it cannot be named.
      this.o.frame.postMessage(msg, '*');
    } catch {
      // The frame went away.
    }
  }

  private reply(req: RpcRequest, result: unknown, error?: RpcError): void {
    this.o.onCall?.(req.method, error?.code ?? ok);
    this.post(error ? errorResponse(req.id, error) : response(req.id, result));
  }

  private async dispatch(req: RpcRequest): Promise<void> {
    if (this.disposed) return;
    const { method, params } = req;
    const app = methodApp(method);
    const missing = missingPermissions(method, this.granted);
    if (missing === null || (app !== null && app !== this.o.app)) {
      this.reply(req, null, {
        code: ERROR_CODES.methodNotFound,
        message: `unknown method ${method}`,
      });
      return;
    }
    if (missing.length) {
      this.reply(req, null, {
        code: ERROR_CODES.permissionDenied,
        message: `${method} needs the ${missing.join(', ')} permission`,
        data: { missing },
      });
      return;
    }
    if (method !== 'addin.hello' && !this.ready) {
      this.reply(req, null, { code: ERROR_CODES.notReady, message: 'call addin.hello first' });
      return;
    }
    try {
      const result = await this.run(method, params);
      const json = plainJson(result);
      this.reply(req, json === undefined ? null : json);
      if (method === 'addin.hello') this.flushEvents();
    } catch (e) {
      const error: RpcError =
        e instanceof BridgeError
          ? { code: e.code, message: e.message }
          : { code: ERROR_CODES.apiError, message: e instanceof Error ? e.message : String(e) };
      this.reply(req, null, error);
    }
  }

  private flushEvents(): void {
    const events = this.pendingEvents;
    this.pendingEvents = [];
    for (const ev of events) this.post(notification(ev));
  }

  private async run(method: string, params: unknown[]): Promise<unknown> {
    switch (method) {
      case 'addin.hello':
        return this.hello(params[0]);
      case 'addin.selection': {
        if (!this.o.selection)
          throw new BridgeError(ERROR_CODES.methodNotFound, 'no selection here');
        if (!this.o.host()) throw new BridgeError(ERROR_CODES.noDocument, 'no document is open');
        return this.o.selection();
      }
      case 'storage.get':
        return this.storage(() => this.o.storage.get(params[0] as string));
      case 'storage.set':
        return this.storage(() => this.o.storage.set(params[0] as string, params[1]));
      case 'storage.remove':
        return this.storage(() => this.o.storage.remove(params[0] as string));
      case 'storage.keys':
        return this.storage(() => this.o.storage.keys());
      default:
        return this.document(method, params);
    }
  }

  private hello(arg: unknown): HelloResult {
    const offered =
      arg && typeof arg === 'object' && Array.isArray((arg as { protocols?: unknown }).protocols)
        ? ((arg as { protocols: unknown[] }).protocols as unknown[])
        : [1];
    const protocol = PROTOCOL_VERSIONS.find((v) => offered.includes(v));
    if (protocol === undefined) {
      throw new BridgeError(ERROR_CODES.unsupportedVersion, 'no common protocol version');
    }
    this.ready = true;
    return {
      protocol,
      app: this.o.app,
      addinId: this.o.manifest.id,
      version: this.o.manifest.version,
      permissions: [...this.granted],
      language: this.o.language(),
      methods: this.allowedMethods(),
      hasDocument: this.o.host() !== null,
    };
  }

  private storage(f: () => unknown): unknown {
    try {
      return f();
    } catch (e) {
      if (e instanceof AddinStorageError)
        throw new BridgeError(ERROR_CODES.invalidParams, e.message);
      throw e;
    }
  }

  private async document(method: string, params: unknown[]): Promise<unknown> {
    const host = this.o.host();
    if (!host) throw new BridgeError(ERROR_CODES.noDocument, 'no document is open');
    if (!isWriteMethod(method)) return host.call(method, params);
    // One undo step per call, like one BlueScript run.
    await host.begin?.();
    try {
      return await host.call(method, params);
    } finally {
      await host.end?.();
    }
  }
}

class BridgeError extends Error {
  constructor(
    readonly code: number,
    message: string,
  ) {
    super(message);
  }
}
