import { BlueEditor } from '@blueoffice/editor';
import { connectHost } from '@blueoffice/editor-sdk';
import type { Platform } from '@blueoffice/platform';
import { createWebPlatform } from '@blueoffice/platform/web';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { appForFile, commandHandler, embedPlatform, openSession, type FetchLike } from './embed';
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
      const hosted = embedPlatform(platform, { api, session, file, app, conn, fetchImpl });
      conn.onCommand(commandHandler(hosted, app, file.name));
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
