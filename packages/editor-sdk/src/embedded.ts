/**
 * Editor side of the SDK: runs inside the iframe (`/edit?embed=1`). Announces
 * itself to the parent, receives {@link InitPayload} (with the access token)
 * from a whitelisted host origin, emits events and answers commands.
 */
import {
  EDITOR_COMMANDS,
  envelope,
  isEnvelope,
  originAllowed,
  type EditorCommand,
  type EditorEvent,
  type IncomingMessage,
  type InitPayload,
  type MessageSource,
  type MessageTarget,
} from './protocol';

export type CommandHandler = (cmd: EditorCommand) => Promise<unknown> | unknown;

export interface HostConnection {
  /** Resolves with the init payload once the host has answered `hello`. */
  readonly ready: Promise<InitPayload>;
  emit(event: EditorEvent, payload?: unknown): void;
  onCommand(handler: CommandHandler): void;
  close(): void;
}

export interface ConnectHostOptions {
  /** Parent window (`window.parent`). */
  parent: MessageTarget;
  /** Own window, to listen on. */
  self: MessageSource;
  /** Whitelisted host origins (server configuration). */
  allowedOrigins: readonly string[];
}

export function connectHost(o: ConnectHostOptions): HostConnection {
  let hostOrigin: string | undefined;
  let handler: CommandHandler | undefined;
  let resolveReady: (p: InitPayload) => void = () => undefined;
  const ready = new Promise<InitPayload>((r) => {
    resolveReady = r;
  });

  const post = (msg: ReturnType<typeof envelope>): void => {
    if (hostOrigin !== undefined) o.parent.postMessage(msg, hostOrigin);
  };

  const listener = (ev: IncomingMessage): void => {
    if (!originAllowed(ev.origin, o.allowedOrigins) || !isEnvelope(ev.data)) return;
    // Once bound, only the first host origin may talk to us.
    if (hostOrigin !== undefined && ev.origin !== hostOrigin) return;
    const msg = ev.data;
    if (msg.type === 'init' && hostOrigin === undefined) {
      hostOrigin = ev.origin;
      resolveReady(msg.payload as InitPayload);
      return;
    }
    if (msg.type === 'command' && msg.requestId !== undefined) {
      const requestId = msg.requestId;
      const cmd = msg.name as EditorCommand;
      if (!EDITOR_COMMANDS.includes(cmd) || !handler) {
        post(
          envelope('response', msg.name, { requestId, error: `unsupported command '${msg.name}'` }),
        );
        return;
      }
      Promise.resolve()
        .then(() => handler?.(cmd))
        .then(
          (payload) => post(envelope('response', cmd, { requestId, payload })),
          (e: unknown) =>
            post(
              envelope('response', cmd, {
                requestId,
                error: e instanceof Error ? e.message : String(e),
              }),
            ),
        );
    }
  };
  o.self.addEventListener('message', listener);

  // The host origin is unknown before init, so `hello` carries no data and
  // is broadcast to each whitelisted origin individually (never to '*').
  for (const origin of o.allowedOrigins) o.parent.postMessage(envelope('hello', 'hello'), origin);

  return {
    ready,
    emit(event, payload) {
      post(envelope('event', event, { payload }));
    },
    onCommand(h) {
      handler = h;
    },
    close() {
      o.self.removeEventListener('message', listener);
    },
  };
}
