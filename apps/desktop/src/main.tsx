import { BlueEditor } from '@blueoffice/editor';
import {
  AccountChip,
  DriveClient,
  HostedBanner,
  OfflineError,
  OfflineSaves,
  StartScreen,
  SyncPrompt,
  appForFile,
  appForFileContent,
  authText,
  beginSignIn,
  completeSignIn,
  hostedText,
  offlinePlatform,
  openHosted,
  parseSyncFolders,
  serverBase,
  syncFolderOf,
  updatesText,
  type DriveHostInfo,
  type DriveSession,
  type FetchLike,
  type HostFileRef,
  type HostedApp,
  type HostedEditor,
  type SyncFolder,
} from '@blueoffice/hosted';
import {
  TABULAR_OPEN_EXTENSIONS,
  openExtensions,
  webOpenExtensions,
  shouldCheckForUpdates,
  updateNotice,
  type HostFileLink,
  type OpenedFile,
  type Platform,
  type RecentFile,
} from '@blueoffice/platform';
import { createDesktopPlatform, isOidcCallback } from '@blueoffice/platform/desktop';
import { BlueSheetEditor } from '@blueoffice/sheet-editor';
import { BlueSlideEditor } from '@blueoffice/slide-editor';
import { LocalStorageProvider } from '@blueoffice/storage-providers';
import { StrictMode, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';

// The desktop app is only a shell: same editor, DesktopPlatform (native engine + real disk).
// One shell, three products: the window URL selects the app (see tauri.*.conf.json).
const query = new URLSearchParams(window.location.search);
const requested = query.get('app');
const shellApp: HostedApp = requested === 'sheet' || requested === 'slide' ? requested : 'doc';
const scheme = shellApp === 'sheet' ? 'bluesheet' : shellApp === 'slide' ? 'blueslide' : 'bluedoc';
const base = createDesktopPlatform({
  storage: (files) => new LocalStorageProvider(files),
  scheme,
});
const language = 'vi-VN';
const fetchImpl = window.fetch.bind(window) as FetchLike;
const saves = new OfflineSaves(base.offline);
const root = createRoot(document.getElementById('root')!);
const SYNC_KEY = 'syncFolders';
const UPDATE_CHECK_KEY = 'lastUpdateCheck';

/** Platform whose editor opens `file` once (the OS open requests are routed by this shell). */
function withFile(file: OpenedFile | null): Platform {
  const p = platform;
  return {
    ...p,
    shell: {
      recentFiles: p.shell.recentFiles.bind(p.shell),
      addRecentFile: p.shell.addRecentFile.bind(p.shell),
      onOpenRequest: (handler) => {
        if (file) queueMicrotask(() => handler(file));
        return () => {};
      },
    },
  };
}

function Editor({ app, platform }: { app: HostedApp; platform: Platform }) {
  return app === 'sheet' ? (
    <BlueSheetEditor platform={platform} />
  ) : app === 'slide' ? (
    <BlueSlideEditor platform={platform} audience={query.get('audience') ?? undefined} />
  ) : (
    <BlueEditor
      platform={platform}
      app={app}
      sampleUrl={`${import.meta.env.BASE_URL}samples/phase1-sample.docx`}
    />
  );
}

/** One-line notice over the editor (opening / errors / offline sync of host files). */
function Notice({ text }: { text: string }) {
  return (
    <div
      data-testid="hosted-notice"
      role="status"
      style={{
        position: 'fixed',
        top: 8,
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

// ---------------------------------------------------------------- shell state

let view: 'start' | 'editor' | 'blank' = 'blank';
let editor: ReactNode = null;
let notice: string | undefined;
let prompt: { name: string; online: () => void; local: () => void } | null = null;
let server: string | null = null;
let serverFromEnv = false;
let syncFolders: SyncFolder[] = [];
let recent: RecentFile[] = [];
let drive: DriveSession | null = null;
let current: HostedEditor | null = null;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

function render(): void {
  root.render(
    <StrictMode>
      {notice ? <Notice text={notice} /> : null}
      {base.auth.enabled ? (
        <AccountChip
          auth={base.auth}
          language={language}
          onError={(e) =>
            say(
              authText(language, 'signInFailed', {
                message: e instanceof Error ? e.message : String(e),
              }),
              true,
            )
          }
        />
      ) : null}
      {prompt ? (
        <SyncPrompt
          language={language}
          name={prompt.name}
          onOnline={prompt.online}
          onLocal={prompt.local}
        />
      ) : null}
      {view === 'start' ? (
        <StartScreen
          language={language}
          fetchImpl={fetchImpl}
          serverUrl={server}
          serverFromEnv={serverFromEnv}
          onServerUrlChange={async (url) => {
            await base.shell.setServerUrl?.(url);
            await loadSettings();
          }}
          syncFolders={syncFolders}
          onSyncFoldersChange={async (folders) => {
            await base.shell.setSetting?.(SYNC_KEY, JSON.stringify(folders));
            syncFolders = folders;
            render();
          }}
          recent={recent}
          onNew={(app) => showEditor(app, null)}
          onOpenLocal={() => void pickLocal()}
          onOpenRecent={(f) => void openRecent(f)}
          drive={drive}
          onDriveChange={(s) => {
            drive = s;
            render();
          }}
          onOpenDriveFile={(host, item, token) =>
            void openHostFile({ host, fileId: item.id, accessToken: token }, item.name)
          }
          signInWithBrowser={signInWithBrowser}
          updates={base.updates}
        />
      ) : view === 'editor' ? (
        editor
      ) : null}
    </StrictMode>,
  );
}

/** Shows a notice; transient ones disappear after a while. */
function say(text: string | undefined, transient = false): void {
  clearTimeout(noticeTimer);
  notice = text;
  if (text && transient) {
    noticeTimer = setTimeout(() => {
      notice = undefined;
      render();
    }, 8000);
  }
  render();
}

async function loadSettings(): Promise<void> {
  server = serverBase((await base.shell.serverUrl?.()) ?? '') || null;
  serverFromEnv = (await base.shell.serverFromEnvironment?.()) ?? false;
  syncFolders = parseSyncFolders((await base.shell.getSetting?.(SYNC_KEY)) ?? null);
  recent = await base.shell.recentFiles();
  // BlueOffice user sign-in (OIDC) when the server has a client for the desktop apps.
  await base.auth.configure(server).catch(() => false);
  render();
}

/** Daily update check at start-up; a newer version is announced in the notice line. */
async function autoCheckUpdates(): Promise<void> {
  const last = Number((await base.shell.getSetting?.(UPDATE_CHECK_KEY)) ?? NaN);
  if (!shouldCheckForUpdates(Number.isFinite(last) ? last : null, Date.now())) return;
  const status = await base.updates.check();
  if (status.state === 'disabled' || status.state === 'notConfigured') return;
  await base.shell.setSetting?.(UPDATE_CHECK_KEY, String(Date.now()));
  if (status.state === 'available') {
    const n = updateNotice(status);
    say(updatesText(language, n.key as `updates.available`, n.params), true);
  }
}

function showStart(): void {
  void current?.close();
  current = null;
  view = 'start';
  void loadSettings();
}

function showEditor(app: HostedApp, file: OpenedFile | null, p?: Platform): void {
  view = 'editor';
  editor = <Editor key={`${app}:${Date.now()}`} app={app} platform={p ?? withFile(file)} />;
  render();
}

// ---------------------------------------------------------------- local files + sync folders

/** A local file inside a BlueDrive sync folder: ask whether to collaborate online instead. */
async function syncedHostFile(file: OpenedFile): Promise<HostFileRef | null> {
  if (!file.path || !server) return null;
  const hit = syncFolderOf(file.path, syncFolders);
  if (!hit || !drive || drive.host !== hit.folder.host) return null;
  const found = await new DriveClient(server, drive.host, drive.token, fetchImpl)
    .resolve(hit.relative)
    .catch(() => null);
  if (!found) return null;
  const ref = { host: drive.host, fileId: found.id, accessToken: drive.token };
  return new Promise((resolve) => {
    const done = (r: HostFileRef | null) => {
      prompt = null;
      render();
      resolve(r);
    };
    prompt = { name: file.name, online: () => done(ref), local: () => done(null) };
    render();
  });
}

async function openLocal(file: OpenedFile): Promise<void> {
  const ref = await syncedHostFile(file);
  if (ref) return openHostFile(ref, file.name);
  if (file.path) {
    void base.shell.addRecentFile({ name: file.name, path: file.path, openedAt: Date.now() });
  }
  showEditor(appForFileContent(file.name, file.bytes), file);
}

async function pickLocal(): Promise<void> {
  // Documents, workbooks and decks with their templates, macro-enabled variants and
  // slide shows (CORE-FMT-001).
  const f = await base.files.openFile([
    ...openExtensions('doc'),
    '.doc',
    '.odt',
    ...openExtensions('sheet'),
    '.xls',
    '.ods',
    '.csv',
    ...openExtensions('slide'),
    '.ppt',
    '.odp',
    // Web pages, Flat OPC and XML Spreadsheet 2003 (CORE-FMT-003).
    ...webOpenExtensions('doc'),
    // SYLK, DIF, formatted text and dBASE tables (CORE-FMT-004).
    ...TABULAR_OPEN_EXTENSIONS,
    // PDF opens as a document of its text, best effort (CORE-FMT-004).
    '.pdf',
  ]);
  if (f) await openLocal(f);
}

async function openRecent(f: RecentFile): Promise<void> {
  const host = /^([A-Za-z0-9._-]+):(.+)$/.exec(f.path);
  if (host && !/^[A-Za-z]$/.test(host[1] ?? '') && drive && drive.host === host[1]) {
    // A host file opened earlier (`host:fileId`).
    return openHostFile(
      { host: drive.host, fileId: host[2] ?? '', accessToken: drive.token },
      f.name,
    );
  }
  try {
    const file = await base.files.openPath?.(f.path);
    if (file) await openLocal(file);
  } catch (e) {
    say(
      hostedText(language, 'openFailed', { message: e instanceof Error ? e.message : String(e) }),
      true,
    );
  }
}

// ---------------------------------------------------------------- host files

/**
 * Opens a host file through a BlueOffice server session (prompt §6A.3): the
 * file is edited live with the other users of the same file and saved back as
 * host versions. While the server is unreachable saves stay in the local
 * cache; offline copies are synced (or become a conflict copy) on the next open.
 */
async function openHostFile(ref: HostFileRef, name: string): Promise<void> {
  const srv = server ?? (serverBase((await base.shell.serverUrl?.()) ?? '') || null);
  if (!srv) return say(hostedText(language, 'noServer'));
  say(hostedText(language, 'opening', { name }));
  // Result of syncing an earlier offline save, shown once the file is open.
  let synced: string | undefined;
  try {
    void current?.close();
    const h = await openHosted({
      platform,
      api: srv,
      base: srv,
      ref,
      fetchImpl,
      offline: saves,
      onSavedOffline: () => say(hostedText(language, 'savedOffline'), true),
      onOfflineSync: (r) => {
        synced =
          r.outcome === 'saved'
            ? hostedText(language, 'syncedOffline', { version: r.version ?? '' })
            : hostedText(language, 'conflictCopy', { name: r.conflictCopy?.name ?? '' });
      },
      onEnded: (r) => {
        // After a restore the host content is authoritative: open it again.
        if (r === 'reload' && current === h) void openHostFile(ref, name);
      },
    });
    current = h;
    void base.shell.addRecentFile({
      name: h.file.name,
      path: `${ref.host}:${ref.fileId}`,
      openedAt: Date.now(),
    });
    view = 'editor';
    editor = (
      <>
        <HostedBanner mode={h.mode} status={h.status} language={language} />
        <Editor key={h.session.sessionId} app={h.app} platform={h.platform} />
      </>
    );
    say(synced);
  } catch (e: unknown) {
    const copy = e instanceof OfflineError ? await saves.getCopy(ref.host, ref.fileId) : null;
    if (copy) {
      // No server: edit the last downloaded version (or the pending offline save).
      showEditor(
        appForFile(copy.meta.name),
        null,
        offlinePlatform(platform, saves, copy, () =>
          say(hostedText(language, 'savedOffline'), true),
        ),
      );
      return say(hostedText(language, 'offlineCopy'));
    }
    const msg = e instanceof Error ? e.message : String(e);
    say(
      /session\.create 40[13]/.test(msg)
        ? hostedText(language, 'signInAgain')
        : hostedText(language, 'openFailed', { message: msg }),
    );
  }
}

/** System-browser sign-in (OAuth 2.0 code + PKCE) with the app's deep link as redirect. */
async function signInWithBrowser(host: DriveHostInfo): Promise<string> {
  const cfg = host.signIn;
  if (!cfg || !base.shell.openExternal || !base.shell.onAuthCallback)
    throw new Error('signIn.unsupported');
  const pending = await beginSignIn(cfg, `${scheme}://auth`);
  const callback = new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => {
      off();
      reject(new Error('signIn.timeout'));
    }, 5 * 60_000);
    const off = base.shell.onAuthCallback!((url) => {
      // BlueOffice user sign-in redirects belong to DesktopAuth.
      if (isOidcCallback(url)) return;
      clearTimeout(timer);
      off();
      resolve(url);
    });
  });
  await base.shell.openExternal(pending.url);
  return completeSignIn(cfg, pending, await callback, fetchImpl);
}

// ---------------------------------------------------------------- wiring

/**
 * The platform handed to editors: Ctrl+O on a file of a sync folder may
 * switch to the hosted session instead (the editor then gets nothing).
 */
const platform: Platform = {
  ...base,
  files: {
    saveFile: base.files.saveFile.bind(base.files),
    openFile: async (accept) => {
      const f = await base.files.openFile(accept);
      if (!f) return null;
      const ref = await syncedHostFile(f);
      if (!ref) return f;
      void openHostFile(ref, f.name);
      return null;
    },
    ...(base.files.openPath ? { openPath: base.files.openPath.bind(base.files) } : {}),
  },
};

// OS requests: deep links (`bluedoc://open?…`, `bluesheet://…`, `blueslide://…`) and local files.
base.shell.onOpenLink?.((link: HostFileLink) => {
  if (!link.token) return say(hostedText(language, 'openFailed', { message: 'token' }));
  void openHostFile({ host: link.host, fileId: link.fileId, accessToken: link.token }, link.fileId);
});
// `?start=1` (Start screen of a new window, tests): the launch file is not reopened here.
const forceStart = query.get('start') === '1';
if (!forceStart) base.shell.onOpenRequest((f) => void openLocal(f));

base.auth.onError((e) => say(authText(language, 'signInFailed', { message: e.message }), true));
base.auth.onChange(() => render());

// Alt+Home: back to the Start screen.
window.addEventListener('keydown', (e) => {
  if (e.altKey && e.key === 'Home') {
    e.preventDefault();
    showStart();
  }
});

void (async () => {
  await loadSettings();
  // Started for a file or a deep link: the editor appears when it is opened; else the Start screen.
  if (view === 'blank' && (forceStart || !(await base.shell.hasLaunchRequest?.()))) showStart();
  void autoCheckUpdates().catch(() => {});
})();
