/**
 * Host side of the SDK: runs in BlueDrive (or any host) and controls an
 * editor iframe. The access token is never placed in the iframe URL; it is
 * handed over via `postMessage` to the whitelisted editor origin only after
 * the editor says `hello`.
 */
import {
  envelope,
  isEnvelope,
  originAllowed,
  type EditorCommand,
  type EditorEvent,
  type EditorLang,
  type EditorMode,
  type EditorTheme,
  type EditorUiOptions,
  type IncomingMessage,
  type InitPayload,
  type MessageSource,
  type MessageTarget,
} from './protocol';

export interface OpenOptions {
  fileId: string;
  accessToken: string;
  host: string;
  mode?: EditorMode;
  lang?: EditorLang;
  theme?: EditorTheme;
  ui?: EditorUiOptions;
  /** Base URL of the editor, e.g. `https://office.example.vn`. */
  editorUrl: string;
  /** Command timeout in ms (default 30 s). */
  timeoutMs?: number;
}

export type EventHandler = (payload: unknown) => void;

export interface EditorHandle {
  on(event: EditorEvent, handler: EventHandler): () => void;
  command(cmd: EditorCommand): Promise<unknown>;
  destroy(): void;
}

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

/** Path of the editor's embed entry (served by `apps/web`). */
export function editorFrameUrl(
  o: Pick<OpenOptions, 'editorUrl' | 'fileId' | 'host' | 'mode'>,
): string {
  const u = new URL('/edit', o.editorUrl);
  u.searchParams.set('file', o.fileId);
  u.searchParams.set('host', o.host);
  u.searchParams.set('mode', o.mode ?? 'edit');
  u.searchParams.set('embed', '1');
  return u.toString();
}

/**
 * Transport-level connection, independent of the DOM. `target` is the
 * iframe's `contentWindow`, `listenOn` the host `window`.
 */
export function connectEditor(
  target: MessageTarget,
  listenOn: MessageSource,
  options: OpenOptions,
  onDestroy?: () => void,
): EditorHandle {
  const editorOrigin = new URL(options.editorUrl).origin;
  const handlers = new Map<EditorEvent, Set<EventHandler>>();
  const pending = new Map<string, Pending>();
  const timeoutMs = options.timeoutMs ?? 30_000;
  let seq = 0;
  let destroyed = false;

  const init: InitPayload = {
    fileId: options.fileId,
    accessToken: options.accessToken,
    host: options.host,
    mode: options.mode ?? 'edit',
    lang: options.lang ?? 'vi-VN',
    theme: options.theme ?? 'auto',
    ui: options.ui ?? {},
  };

  const emit = (event: EditorEvent, payload: unknown): void => {
    handlers.get(event)?.forEach((h) => h(payload));
  };

  const listener = (ev: IncomingMessage): void => {
    if (!originAllowed(ev.origin, [editorOrigin]) || !isEnvelope(ev.data)) return;
    const msg = ev.data;
    switch (msg.type) {
      case 'hello':
        target.postMessage(envelope('init', 'init', { payload: init }), editorOrigin);
        break;
      case 'event':
        emit(msg.name as EditorEvent, msg.payload);
        break;
      case 'response': {
        const p = msg.requestId === undefined ? undefined : pending.get(msg.requestId);
        if (!p || msg.requestId === undefined) return;
        clearTimeout(p.timer);
        pending.delete(msg.requestId);
        if (msg.error !== undefined) p.reject(new Error(msg.error));
        else p.resolve(msg.payload);
        break;
      }
      default:
        break;
    }
  };
  listenOn.addEventListener('message', listener);

  return {
    on(event, handler) {
      let set = handlers.get(event);
      if (!set) {
        set = new Set();
        handlers.set(event, set);
      }
      set.add(handler);
      return () => set.delete(handler);
    },
    command(cmd) {
      if (destroyed) return Promise.reject(new Error('editor destroyed'));
      seq += 1;
      const requestId = `c${seq}`;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(requestId);
          reject(new Error(`command '${cmd}' timed out`));
        }, timeoutMs);
        pending.set(requestId, { resolve, reject, timer });
        target.postMessage(envelope('command', cmd, { requestId }), editorOrigin);
      });
    },
    destroy() {
      if (destroyed) return;
      destroyed = true;
      listenOn.removeEventListener('message', listener);
      pending.forEach((p) => {
        clearTimeout(p.timer);
        p.reject(new Error('editor destroyed'));
      });
      pending.clear();
      handlers.clear();
      onDestroy?.();
    },
  };
}

export const BlueOffice = {
  /** Creates the editor iframe inside `container` and connects to it. */
  open(container: HTMLElement, options: OpenOptions): EditorHandle {
    const frame = container.ownerDocument.createElement('iframe');
    frame.src = editorFrameUrl(options);
    frame.style.border = '0';
    frame.style.width = '100%';
    frame.style.height = '100%';
    frame.allow = 'clipboard-read; clipboard-write; fullscreen';
    container.appendChild(frame);
    const win = frame.contentWindow;
    const hostWin = container.ownerDocument.defaultView;
    if (!win || !hostWin) throw new Error('@blueoffice/editor-sdk: iframe window unavailable');
    return connectEditor(win, hostWin as unknown as MessageSource, options, () => frame.remove());
  },
};
