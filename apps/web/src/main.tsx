import { BlueEditor } from '@blueoffice/editor';
import { connectHost } from '@blueoffice/editor-sdk';
import type { EngineHost, Platform, SheetHost } from '@blueoffice/platform';
import { createWebPlatform } from '@blueoffice/platform/web';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import {
  CollabClient,
  collabDocHost,
  collabSheetHost,
  collabSlideHost,
  colorFor,
  wasmEngine,
  type CollabEngine,
} from '@blueoffice/collab';
import {
  appForFile,
  commandHandler,
  embedPlatform,
  openSession,
  type EmbedSession,
  type FetchLike,
} from './embed';
import { resolveRoute, type AppId } from './routes';

// The web app is only a shell: it builds the WebPlatform and embeds the editor.
const platform = createWebPlatform({ storage: (files) => new LocalStorageProvider(files) });
const base = import.meta.env.BASE_URL;
const { app, redirect } = resolveRoute(window.location.pathname, base);
if (redirect) {
  // Old routes (/write, /docs, /sheets, /slides) keep working.
  window.history.replaceState(null, '', redirect + window.location.search + window.location.hash);
}
const params = new URLSearchParams(window.location.search);
const language = params.get('lang') === 'en-US' ? 'en-US' : 'vi-VN';

function Editor({ app, platform }: { app: AppId; platform: Platform }) {
  return app === 'sheet' ? (
    <BlueSheetEditor platform={platform} language={language} />
  ) : app === 'slide' ? (
    <BlueSlideEditor platform={platform} language={language} />
  ) : (
    <BlueEditor
      platform={platform}
      app={app}
      language={language}
      sampleUrl={`${base}samples/phase1-sample.docx`}
    />
  );
}

const root = createRoot(document.getElementById('root')!);
const isEmbed = window.location.pathname.replace(base, '/').startsWith('/edit');

/**
 * Joins the session's collaboration room (`@blueoffice/collab`): BlueDoc text,
 * BlueSlide objects and BlueSheet cell inputs sync live with presence; every app
 * answers the server's autosave requests.
 */
async function startCollab(
  hosted: Platform,
  app: AppId,
  session: EmbedSession,
  saveNow: () => void,
): Promise<Platform> {
  const wasm = await import('@blueoffice/engine-wasm');
  const wasmUrl = (await import('@blueoffice/engine-wasm/wasm?url')).default;
  await wasm.default({ module_or_path: wasmUrl });
  const replica = new wasm.WasmCollab(crypto.getRandomValues(new Uint32Array(1))[0] ?? 1);
  const url = new URL(session.collab!.url, window.location.href);
  url.protocol =
    url.protocol === 'https:' ? 'wss:' : url.protocol === 'http:' ? 'ws:' : url.protocol;
  let client: CollabClient | null = null;
  let shared: { remoteApplied(): Promise<void>; seed(): Promise<void> } | null = null;
  let engine: CollabEngine = wasmEngine(replica);
  const flush = () => client?.flush();
  const id = replica.clientId();
  const user = {
    userId: hosted.auth.user?.id ?? `guest-${id}`,
    name: hosted.auth.user?.name ?? '',
    color: colorFor(id),
  };
  if (app === 'sheet' && hosted.sheet) {
    const sheet = collabSheetHost<SheetHost>(hosted.sheet, replica, flush);
    shared = sheet;
    hosted = { ...hosted, sheet };
  } else if (app === 'slide') {
    // BlueSlide: slides and objects (last writer wins per object), selected object.
    const live = collabSlideHost<EngineHost>(hosted.engine, replica, flush, user);
    shared = live;
    engine = live.engine;
    hosted = { ...hosted, engine: live };
  } else {
    // BlueDoc: the body text with formatting, collaborators' carets.
    const live = collabDocHost<EngineHost>(hosted.engine, replica, flush, user);
    shared = live;
    engine = live.engine;
    hosted = { ...hosted, engine: live };
  }
  client = new CollabClient({
    url: url.toString(),
    sessionId: session.sessionId,
    sessionToken: session.sessionToken,
    engine,
  });
  // BlueDoc / BlueSlide pull remote changes through their frame gate.
  if (app === 'sheet') client.on('remote', () => void shared?.remoteApplied());
  client.on('control', (m) => {
    if (m.type === 'seed') void shared?.seed();
    else if (m.type === 'save-request') saveNow();
  });
  client.connect();
  return hosted;
}

if (isEmbed) {
  // Hosted in an iframe (BlueDrive, …): token via postMessage, file from the session.
  const api = (import.meta.env.VITE_API_BASE as string | undefined) ?? '';
  const allowedOrigins = String(import.meta.env.VITE_EMBED_ORIGINS ?? '')
    .split(',')
    .map((o) => o.trim())
    .filter(Boolean);
  const conn = connectHost({
    parent: window.parent,
    self: window as unknown as Parameters<typeof connectHost>[0]['self'],
    allowedOrigins,
  });
  const fetchImpl = window.fetch.bind(window) as FetchLike;
  void conn.ready
    .then(async (init) => {
      const { session, file } = await openSession(api, init, fetchImpl);
      const app = appForFile(file.name);
      let hosted = embedPlatform(platform, { api, session, file, app, conn, fetchImpl });
      const save = commandHandler(hosted, app, file.name);
      conn.onCommand(save);
      if (session.collab?.url) {
        hosted = await startCollab(hosted, app, session, () => void save('save'));
      }
      root.render(
        <StrictMode>
          <Editor app={app} platform={hosted} />
        </StrictMode>,
      );
      conn.emit('ready', { mode: session.mode });
    })
    .catch((e: unknown) =>
      conn.emit('error', { message: e instanceof Error ? e.message : String(e) }),
    );
} else {
  root.render(
    <StrictMode>
      <Editor app={app} platform={platform} />
    </StrictMode>,
  );
}
