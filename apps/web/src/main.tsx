import { BlueEditor } from '@blueoffice/editor';
import { connectHost, type EditorEvent, type InitPayload } from '@blueoffice/editor-sdk';
import {
  HostedBanner,
  HostedChrome,
  HostedClosed,
  OfflineError,
  OfflineSaves,
  appForFile,
  fetchAllowedOrigins,
  hostedText,
  joinWindowSession,
  offlinePlatform,
  openHosted,
  serverBase,
  wantsChrome,
  type DirtyState,
  type FetchLike,
  type HostedApp,
} from '@blueoffice/hosted';
import { WINDOW_PARAM, type OidcSession, type Platform } from '@blueoffice/platform';
import {
  AUTH_CALLBACK_PATH,
  createWebAuth,
  createWebPlatform,
  registerServiceWorker,
} from '@blueoffice/platform/web';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor, RemotePage, WatchView } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import type { ThemeSetting } from '@blueoffice/ui';
import { StrictMode, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { AccountChip, authText } from './Account';
import { presentRoute, resolveRoute, type AppId } from './routes';

// The web app is only a shell: it builds the WebPlatform and embeds the editor.
const base = import.meta.env.BASE_URL;
const api = serverBase((import.meta.env.VITE_API_BASE as string | undefined) ?? '');
const fetchImpl = window.fetch.bind(window) as FetchLike;
const params = new URLSearchParams(window.location.search);
let language: 'vi-VN' | 'en-US' = params.get('lang') === 'en-US' ? 'en-US' : 'vi-VN';
const root = createRoot(document.getElementById('root')!);
const relPath = window.location.pathname.replace(base, '/');
const isEmbed = relPath.startsWith('/edit');

// PWA: the app shell, the WASM engine and the fonts stay available offline (`src/sw.ts`).
if (import.meta.env.PROD) void registerServiceWorker(`${base}sw.js`, base);

function Editor({
  app,
  platform,
  theme,
}: {
  app: AppId;
  platform: Platform;
  theme?: ThemeSetting;
}) {
  return app === 'sheet' ? (
    <BlueSheetEditor platform={platform} language={language} {...(theme ? { theme } : {})} />
  ) : app === 'slide' ? (
    <BlueSlideEditor
      platform={platform}
      language={language}
      audience={params.get('audience') ?? undefined}
      {...(theme ? { theme } : {})}
    />
  ) : (
    <BlueEditor
      platform={platform}
      app={app}
      language={language}
      {...(theme ? { theme } : {})}
      sampleUrl={`${base}samples/phase1-sample.docx`}
    />
  );
}

/** One-line notice over the editor (offline saves and syncs of host files, sign-in errors). */
function Notice({ text }: { text: string }) {
  return (
    <div
      data-testid="hosted-notice"
      role="status"
      style={{
        position: 'fixed',
        bottom: 30,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
        maxWidth: 'calc(100vw - 32px)',
        padding: '4px 12px',
        borderRadius: 6,
        background: '#e0f2fe',
        color: '#0c4a6e',
        font: '500 13px/20px system-ui, sans-serif',
        boxShadow: '0 1px 4px rgba(0,0,0,.2)',
      }}
    >
      {text}
    </div>
  );
}

const APP_NAMES: Record<HostedApp, string> = {
  doc: 'BlueDoc',
  sheet: 'BlueSheet',
  slide: 'BlueSlide',
};

/** Editor below the host chrome (embed). */
function Framed({
  init,
  app,
  fileName,
  dirty,
  onClose,
  children,
}: {
  init: InitPayload;
  app: HostedApp;
  fileName: string;
  dirty?: DirtyState;
  onClose: (reason: 'close' | 'back') => void;
  children: ReactNode;
}) {
  const ui = init.ui ?? {};
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {wantsChrome(ui) && (
        <HostedChrome
          ui={ui}
          appName={APP_NAMES[app]}
          fileName={fileName}
          {...(dirty ? { dirty } : {})}
          language={language}
          theme={init.theme ?? 'auto'}
          onBack={() => onClose('back')}
          onShare={() => events?.('share-requested', { fileId: init.fileId })}
          onClose={() => onClose('close')}
        />
      )}
      <div style={{ flex: 1, minHeight: 0 }}>{children}</div>
    </div>
  );
}

let events: ((e: EditorEvent, payload?: unknown) => void) | null = null;

/**
 * Hosted in an iframe (BlueDrive, …, prompt §6A.2): the token arrives via
 * postMessage, the file comes from the server session, saves become host
 * versions and the session's collaboration room is joined
 * (`@blueoffice/hosted`). Theme, host chrome (brand, Back / Share / Close)
 * and the requested mode come from the host's `BlueOffice.open` options;
 * saving while offline keeps a local copy (IndexedDB) that is synced through
 * `…/sync` the next time the file is opened.
 */
async function embed(platform: Platform): Promise<void> {
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
  events = (e, payload) => conn.emit(e, payload);
  const fail = (code: string, e: unknown) =>
    conn.emit('error', { code, message: e instanceof Error ? e.message : String(e) });
  let notice: string | undefined;
  let content: ReactNode = null;
  const render = () =>
    root.render(
      <StrictMode>
        {notice ? <Notice text={notice} /> : null}
        {content}
      </StrictMode>,
    );
  const say = (text: string | undefined) => {
    notice = text;
    render();
  };
  const init = await conn.ready;
  if (init.lang === 'en-US' || init.lang === 'vi-VN') language = init.lang;
  const theme = init.theme ?? 'auto';
  const saves = new OfflineSaves(platform.offline);
  let closed = false;
  const showClosed = (reason: 'close' | 'back' | 'command', saved: boolean) => {
    closed = true;
    notice = undefined;
    content = <HostedClosed language={language} theme={theme} />;
    render();
    conn.emit('closed', { reason, saved });
    return { saved };
  };
  try {
    let synced: string | undefined;
    const h = await openHosted({
      platform,
      api,
      base: api || window.location.href,
      ref: init,
      requestedMode: init.mode,
      fetchImpl,
      offline: saves,
      onSaved: (r) => conn.emit('saved', r),
      onSaveError: (e) => fail('save-failed', e),
      onSavedOffline: () => say(hostedText(language, 'savedOffline')),
      onOfflineSync: (r) => {
        synced =
          r.outcome === 'saved'
            ? hostedText(language, 'syncedOffline', { version: r.version ?? '' })
            : hostedText(language, 'conflictCopy', { name: r.conflictCopy?.name ?? '' });
        conn.emit('saved', { ...r, offline: true });
      },
      onCollaborators: (c) => conn.emit('collaborators-changed', c),
      onEnded: (r) => {
        // After a restore the host content is authoritative: start over.
        if (r === 'reload') window.location.reload();
        else if (r === 'revoked' || r === 'expired') fail(r, `session.${r}`);
      },
    });
    h.dirty.subscribe((dirty) => conn.emit('dirty', { dirty }));
    const close = async (reason: 'close' | 'back' | 'command') => {
      if (closed) return { saved: false };
      const { saved } = await h.close();
      return showClosed(reason, saved);
    };
    conn.onCommand((cmd) => (cmd === 'close' ? close('command') : h.command(cmd)));
    content = (
      <Framed
        init={init}
        app={h.app}
        fileName={h.file.name}
        dirty={h.dirty}
        onClose={(r) => void close(r)}
      >
        <HostedBanner mode={h.mode} status={h.status} language={language} />
        <Editor app={h.app} platform={h.platform} theme={theme} />
      </Framed>
    );
    say(synced);
    conn.emit('ready', { mode: h.mode.value });
  } catch (e: unknown) {
    // No server: edit the last downloaded version (or the pending offline save).
    const copy = e instanceof OfflineError ? await saves.getCopy(init.host, init.fileId) : null;
    if (!copy) return fail('open-failed', e);
    const p = offlinePlatform(platform, saves, copy, () =>
      say(hostedText(language, 'savedOffline')),
    );
    conn.onCommand(async (cmd) => {
      if (cmd === 'close') return closed ? { saved: false } : showClosed('command', false);
      throw new Error(`session.offline (${cmd})`);
    });
    const app = appForFile(copy.meta.name);
    content = (
      <Framed
        init={init}
        app={app}
        fileName={copy.meta.name}
        onClose={(r) => void showClosed(r, false)}
      >
        <Editor app={app} platform={p} theme={theme} />
      </Framed>
    );
    say(hostedText(language, 'offlineCopy'));
    conn.emit('ready', { mode: 'edit', offline: true });
  }
}

/** Standalone web app: local files, optional OIDC sign-in (server `Auth:WebClient`). */
async function standalone(): Promise<void> {
  const auth: OidcSession | null = await createWebAuth({ api, base }).catch(() => null);
  let notice: string | undefined;
  if (auth && relPath === `/${AUTH_CALLBACK_PATH}`) {
    try {
      const back = await auth.handleRedirect(window.location.href);
      window.history.replaceState(null, '', back.startsWith('/') ? back : base);
    } catch (e) {
      notice = authText(language, 'signInFailed', {
        message: e instanceof Error ? e.message : String(e),
      });
      window.history.replaceState(null, '', `${base}doc`);
    }
  }
  // An expired token is refreshed (or the session dropped) before the editor starts.
  await auth?.getAccessToken().catch(() => null);
  const { app, redirect } = resolveRoute(window.location.pathname, base);
  if (redirect) {
    // Old routes (/write, /docs, /sheets, /slides) keep working.
    window.history.replaceState(null, '', redirect + window.location.search + window.location.hash);
  }
  let platform = createWebPlatform({
    storage: (files) => new LocalStorageProvider(files),
    ...(auth ? { auth } : {}),
  });
  // A window of View › New Window: the same document, edits shared with the other window.
  const windowId = params.get(WINDOW_PARAM);
  if (windowId) {
    const w = await joinWindowSession({ platform, app, id: windowId }).catch(() => null);
    if (w) platform = w.platform;
  }
  root.render(
    <StrictMode>
      {notice ? <Notice text={notice} /> : null}
      {auth ? (
        <AccountChip
          auth={auth}
          language={language}
          returnTo={window.location.pathname + window.location.search}
        />
      ) : null}
      <Editor app={app} platform={platform} />
    </StrictMode>,
  );
}

/**
 * The phone remote and the online presentation viewer (SLIDE-REMOTE-001): no sign-in, the
 * token in the link is all they have (`docs/deploy/security.md`).
 */
function present(route: { kind: 'remote' | 'watch'; token: string }): void {
  const options = { baseUrl: api };
  // Vietnamese unless the link asks for English (?lang=en-US), like the editors.
  const lang = language;
  document.documentElement.lang = lang;
  root.render(
    <StrictMode>
      {route.kind === 'remote' ? (
        <RemotePage token={route.token} options={options} language={lang} />
      ) : (
        <WatchView
          platform={createWebPlatform({ storage: (files) => new LocalStorageProvider(files) })}
          token={route.token}
          options={options}
          language={lang}
        />
      )}
    </StrictMode>,
  );
}

const presenting = presentRoute(window.location.pathname, base);
if (presenting) {
  present(presenting);
} else if (isEmbed) {
  void embed(createWebPlatform({ storage: (files) => new LocalStorageProvider(files) }));
} else {
  void standalone();
}
