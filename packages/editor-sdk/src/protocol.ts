/**
 * Wire protocol between a host page (BlueDrive, …) and the editor iframe
 * (prompt §6A.2). Every message is a {@link BohpMessage} envelope; messages
 * whose `origin` is not whitelisted, or that are not envelopes, are ignored.
 */

export type EditorMode = 'edit' | 'view' | 'comment';
export type EditorTheme = 'light' | 'dark' | 'auto';
export type EditorLang = 'vi-VN' | 'en-US';

export interface EditorUiOptions {
  showShare?: boolean;
  showClose?: boolean;
  brandName?: string;
}

export type EditorEvent =
  'ready' | 'saved' | 'dirty' | 'closed' | 'share-requested' | 'error' | 'collaborators-changed';

export type EditorCommand = 'save' | 'print' | 'export-pdf' | 'close';

/** Sent by the host once the editor announces `hello`; carries the token. */
export interface InitPayload {
  fileId: string;
  accessToken: string;
  host: string;
  mode: EditorMode;
  lang: EditorLang;
  theme: EditorTheme;
  ui: EditorUiOptions;
}

/** Message envelope exchanged between host and editor iframe. */
export interface BohpMessage {
  source: 'blueoffice';
  version: 1;
  type: 'hello' | 'init' | 'event' | 'command' | 'response';
  name: string;
  payload?: unknown;
  requestId?: string;
  error?: string;
}

/** Minimal subset of `Window` used by the SDK (lets tests run without a DOM). */
export interface MessageTarget {
  postMessage(message: unknown, targetOrigin: string): void;
}

export interface MessageSource {
  addEventListener(type: 'message', listener: (ev: IncomingMessage) => void): void;
  removeEventListener(type: 'message', listener: (ev: IncomingMessage) => void): void;
}

export interface IncomingMessage {
  data: unknown;
  origin: string;
  source?: unknown;
}

export const EDITOR_EVENTS: readonly EditorEvent[] = [
  'ready',
  'saved',
  'dirty',
  'closed',
  'share-requested',
  'error',
  'collaborators-changed',
];

export const EDITOR_COMMANDS: readonly EditorCommand[] = ['save', 'print', 'export-pdf', 'close'];

export function envelope(
  type: BohpMessage['type'],
  name: string,
  extra: Partial<Pick<BohpMessage, 'payload' | 'requestId' | 'error'>> = {},
): BohpMessage {
  return { source: 'blueoffice', version: 1, type, name, ...extra };
}

export function isEnvelope(data: unknown): data is BohpMessage {
  if (typeof data !== 'object' || data === null) return false;
  const m = data as Record<string, unknown>;
  return (
    m.source === 'blueoffice' &&
    m.version === 1 &&
    typeof m.type === 'string' &&
    typeof m.name === 'string'
  );
}

/** Origin check against a whitelist; `'*'` is never accepted. */
export function originAllowed(origin: string, allowed: readonly string[]): boolean {
  return origin !== '*' && origin !== 'null' && allowed.includes(origin);
}
