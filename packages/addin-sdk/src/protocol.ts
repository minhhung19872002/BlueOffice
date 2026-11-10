/**
 * Wire protocol between an add-in page (sandboxed iframe) and the BlueOffice
 * editor (CORE-ADDIN-001, ADR-0013): JSON-RPC 2.0 messages over `postMessage`,
 * inside an envelope that names the protocol and its version. Both sides treat
 * every incoming message as untrusted and check it with the parsers below.
 */
import type { AddinApp } from './manifest';

/** Envelope tag. */
export const ADDIN_TAG = 'bo-addin';
/** Protocol versions this library speaks (newest first). */
export const PROTOCOL_VERSIONS = [1] as const;
export type ProtocolVersion = (typeof PROTOCOL_VERSIONS)[number];

/** Largest message (JSON characters) either side accepts. */
export const MAX_MESSAGE_CHARS = 2_000_000;

/** Error codes (JSON-RPC 2.0 reserved ones, then BlueOffice's -32001…). */
export const ERROR_CODES = {
  invalidRequest: -32600,
  methodNotFound: -32601,
  invalidParams: -32602,
  internal: -32603,
  permissionDenied: -32001,
  rateLimited: -32002,
  apiError: -32003,
  unsupportedVersion: -32004,
  noDocument: -32005,
  notReady: -32006,
} as const;

export type ErrorCode = (typeof ERROR_CODES)[keyof typeof ERROR_CODES];

interface Envelope {
  tag: typeof ADDIN_TAG;
  v: ProtocolVersion;
  jsonrpc: '2.0';
}

/** Add-in → host. */
export interface RpcRequest extends Envelope {
  id: number | string;
  method: string;
  params: unknown[];
}

export interface RpcError {
  code: number;
  message: string;
  /** Missing permissions of a `permissionDenied` error. */
  data?: { missing?: string[] };
}

/** Host → add-in, answer to a request. */
export type RpcResponse = Envelope & { id: number | string | null } & (
    { result: unknown; error?: undefined } | { error: RpcError; result?: undefined }
  );

/** Events an add-in can subscribe to. They carry no document content. */
export type AddinEvent =
  | { type: 'selectionChanged' }
  | { type: 'documentChanged' }
  /** A ribbon command of the manifest with `action: "event"` was clicked. */
  | { type: 'command'; command: string };

export type AddinEventType = AddinEvent['type'];

/** Host → add-in notification. */
export interface RpcNotification extends Envelope {
  method: 'event';
  params: [AddinEvent];
}

/** Result of `addin.hello`. */
export interface HelloResult {
  protocol: ProtocolVersion;
  app: AddinApp;
  addinId: string;
  version: string;
  /** Permissions the user granted. */
  permissions: string[];
  /** UI language of the editor (`vi-VN`, `en-US`). */
  language: string;
  /** Methods this add-in may call in this application. */
  methods: string[];
  /** A document is open. */
  hasDocument: boolean;
}

const METHOD_RE = /^[a-z][A-Za-z]{0,30}\.[a-z][A-Za-z]{0,40}$/;

const isObject = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v);

const validId = (id: unknown): id is number | string =>
  (typeof id === 'number' && Number.isSafeInteger(id)) ||
  (typeof id === 'string' && id.length > 0 && id.length <= 64);

/**
 * A copy of `v` that is plain JSON (no functions, cycles, class instances,
 * binary data), or `undefined` when it cannot be expressed as JSON or is too
 * large.
 */
export function plainJson(v: unknown, max = MAX_MESSAGE_CHARS): unknown {
  let text: string | undefined;
  try {
    text = JSON.stringify(v, (_k, x: unknown) => {
      // JSON would quietly turn these into null or drop them: refuse instead.
      if (typeof x === 'function' || typeof x === 'symbol') throw new TypeError('not JSON');
      return x;
    });
  } catch {
    return undefined;
  }
  if (text === undefined || text.length > max) return undefined;
  return JSON.parse(text) as unknown;
}

const envelopeOk = (m: Record<string, unknown>) =>
  m.tag === ADDIN_TAG &&
  (PROTOCOL_VERSIONS as readonly unknown[]).includes(m.v) &&
  m.jsonrpc === '2.0';

/** Why a message is not a request: `null` means not ours (ignore), else the error to answer. */
export type ParsedRequest =
  | { ok: true; request: RpcRequest }
  | { ok: false; id: number | string | null; error: RpcError }
  | null;

/** Checks a message received by the host. */
export function parseRequest(data: unknown): ParsedRequest {
  if (!isObject(data) || data.tag !== ADDIN_TAG) return null;
  const id = validId(data.id) ? data.id : null;
  const bad = (code: number, message: string): ParsedRequest => ({
    ok: false,
    id,
    error: { code, message },
  });
  if (!(PROTOCOL_VERSIONS as readonly unknown[]).includes(data.v)) {
    return bad(ERROR_CODES.unsupportedVersion, `unsupported protocol version ${String(data.v)}`);
  }
  if (data.jsonrpc !== '2.0' || id === null) return bad(ERROR_CODES.invalidRequest, 'bad request');
  if (typeof data.method !== 'string' || !METHOD_RE.test(data.method)) {
    return bad(ERROR_CODES.invalidRequest, 'bad method name');
  }
  const raw = data.params === undefined ? [] : data.params;
  if (!Array.isArray(raw) || raw.length > 16) {
    return bad(ERROR_CODES.invalidParams, 'params must be a list');
  }
  const params = plainJson(raw);
  if (!Array.isArray(params)) return bad(ERROR_CODES.invalidParams, 'params must be plain JSON');
  return {
    ok: true,
    request: {
      tag: ADDIN_TAG,
      v: data.v as ProtocolVersion,
      jsonrpc: '2.0',
      id,
      method: data.method,
      params,
    },
  };
}

/** Checks a message received by the add-in (a response or an event); null: ignore. */
export function parseHostMessage(
  data: unknown,
): { kind: 'response'; response: RpcResponse } | { kind: 'event'; event: AddinEvent } | null {
  if (!isObject(data) || !envelopeOk(data)) return null;
  if (data.method === 'event') {
    const p = Array.isArray(data.params) ? (data.params[0] as unknown) : undefined;
    if (!isObject(p)) return null;
    if (p.type === 'selectionChanged' || p.type === 'documentChanged') {
      return { kind: 'event', event: { type: p.type } };
    }
    if (p.type === 'command' && typeof p.command === 'string') {
      return { kind: 'event', event: { type: 'command', command: p.command } };
    }
    return null;
  }
  if (!validId(data.id)) return null;
  if (isObject(data.error)) {
    const e = data.error;
    if (typeof e.code !== 'number' || typeof e.message !== 'string') return null;
    const missing =
      isObject(e.data) && Array.isArray(e.data.missing)
        ? e.data.missing.filter((x): x is string => typeof x === 'string')
        : undefined;
    return {
      kind: 'response',
      response: {
        tag: ADDIN_TAG,
        v: 1,
        jsonrpc: '2.0',
        id: data.id,
        error: { code: e.code, message: e.message, ...(missing ? { data: { missing } } : {}) },
      },
    };
  }
  if (!('result' in data)) return null;
  return {
    kind: 'response',
    response: { tag: ADDIN_TAG, v: 1, jsonrpc: '2.0', id: data.id, result: data.result },
  };
}

export const request = (id: number | string, method: string, params: unknown[]): RpcRequest => ({
  tag: ADDIN_TAG,
  v: 1,
  jsonrpc: '2.0',
  id,
  method,
  params,
});

export const response = (id: number | string, result: unknown): RpcResponse => ({
  tag: ADDIN_TAG,
  v: 1,
  jsonrpc: '2.0',
  id,
  result: result === undefined ? null : result,
});

export const errorResponse = (id: number | string | null, error: RpcError): RpcResponse => ({
  tag: ADDIN_TAG,
  v: 1,
  jsonrpc: '2.0',
  id,
  error,
});

export const notification = (event: AddinEvent): RpcNotification => ({
  tag: ADDIN_TAG,
  v: 1,
  jsonrpc: '2.0',
  method: 'event',
  params: [event],
});

/** Minimal window surface (lets tests run without a DOM). */
export interface PostTarget {
  postMessage(message: unknown, targetOrigin: string): void;
}

export interface MessageEventLike {
  data: unknown;
  source?: unknown;
  origin?: string;
}

export interface MessageSourceLike {
  addEventListener(type: 'message', listener: (ev: MessageEventLike) => void): void;
  removeEventListener(type: 'message', listener: (ev: MessageEventLike) => void): void;
}
