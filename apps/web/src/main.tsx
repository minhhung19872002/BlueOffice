import { BlueEditor } from '@blueoffice/editor';
import { connectHost } from '@blueoffice/editor-sdk';
import {
  HostedBanner,
  HostedStatus,
  SessionMode,
  appForFile,
  commandHandler,
  fetchAllowedOrigins,
  guardPlatform,
  hostedPlatform,
  loadWasmReplica,
  openSession,
  parseMode,
  serverBase,
  startCollab,
  type FetchLike,
  type HostCommand,
} from '@blueoffice/hosted';
import type { Platform } from '@blueoffice/platform';
import { createWebPlatform } from '@blueoffice/platform/web';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
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
let language: 'vi-VN' | 'en-US' = params.get('lang') === 'en-US' ? 'en-US' : 'vi-VN';

function Editor({ app, platform }: { app: AppId; platform: Platform }) {
  return app === 'sheet' ? (
    <BlueSheetEditor platform={platform} language={language} />
  ) : app === 'slide' ? (
    <BlueSlideEditor
      platform={platform}
      language={language}
      audience={params.get('audience') ?? undefined}
    />
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
 * Hosted in an iframe (BlueDrive, …, prompt §6A.2): the token arrives via
 * postMessage, the file comes from the server session, saves become host
 * versions and the session's collaboration room is joined
 * (`@blueoffice/hosted`).
 */
async function embed(): Promise<void> {
  const api = serverBase((import.meta.env.VITE_API_BASE as string | undefined) ?? '');
  const fetchImpl = window.fetch.bind(window) as FetchLike;
  // Hosts allowed to embed: fixed at build time, or the server's configured hosts.
  const built = String(import.meta.env.VITE_EMBED_ORIGINS ?? '')
    .split(',')
    .map((o) => o.trim())
    .filter(Boolean);
  const allowedOrigins = built.length
    ? built
    : await fetchAllowedOrigins(api, fetchImpl).catch(() => []);
  const conn = connectHost({
    parent: window.parent,
    self: window as unknown as Parameters<typeof connectHost>[0]['self'],
    allowedOrigins,
  });
  try {
    const init = await conn.ready;
    if (init.lang === 'en-US' || init.lang === 'vi-VN') language = init.lang;
    const { session, file } = await openSession(api, init, fetchImpl);
    const app = appForFile(file.name);
    const mode = new SessionMode(parseMode(session.mode));
    const status = new HostedStatus();
    let hosted = hostedPlatform(platform, {
      api,
      session,
      file,
      app,
      mode,
      fetchImpl,
      onSaved: (r) => conn.emit('saved', r),
    });
    // Commands run against the final platform (collaborative + view-mode guard).
    let final: Platform = hosted;
    const run = (cmd: HostCommand) => commandHandler(final, app, file.name)(cmd);
    conn.onCommand(run);
    if (session.collab?.url) {
      const h = startCollab({
        platform: hosted,
        app,
        session,
        base: api || window.location.href,
        replica: await loadWasmReplica(),
        mode,
        saveNow: () => void run('save').catch(() => {}),
        onStatus: (s) => status.setStatus(s),
        onEnded: (r) => {
          status.setEnded(r);
          // After a restore the host content is authoritative: start over.
          if (r === 'reload') window.location.reload();
        },
      });
      h.client.on('control', (m) => {
        if (m.type === 'collaborators-changed') {
          conn.emit('collaborators-changed', m.collaborators);
        }
      });
      hosted = h.platform;
    }
    final = guardPlatform(hosted, mode);
    root.render(
      <StrictMode>
        <HostedBanner mode={mode} status={status} language={language} />
        <Editor app={app} platform={final} />
      </StrictMode>,
    );
    conn.emit('ready', { mode: session.mode });
  } catch (e: unknown) {
    conn.emit('error', { message: e instanceof Error ? e.message : String(e) });
  }
}

if (isEmbed) {
  void embed();
} else {
  root.render(
    <StrictMode>
      <Editor app={app} platform={platform} />
    </StrictMode>,
  );
}
