/**
 * Desktop Start screen (prompt §6A.3): new / local / recent files, a
 * "BlueDrive" tab (SSO or token sign-in, folders, recent, shared with me —
 * through the server proxy of docs/bohp.md §A2) and the settings (BlueOffice
 * server, BlueDrive sync folders). Everything platform specific (opening
 * files, the system browser, stored settings) comes in through props.
 */
import type { Language } from '@blueoffice/i18n';
import {
  updateNotice,
  type RecentFile,
  type UpdateService,
  type UpdateStatus,
} from '@blueoffice/platform';
import { useCallback, useEffect, useState, type CSSProperties, type ReactNode } from 'react';
import {
  DriveClient,
  DriveError,
  listHosts,
  type DriveHostInfo,
  type DriveItem,
  type DriveListing,
} from '../drive';
import type { FetchLike, HostedApp } from '../session';
import type { SyncFolder } from '../syncFolders';
import { startText, updatesText, type StartKey, type UpdatesKey } from './text';

/** A signed-in host user (the token stays in memory). */
export interface DriveSession {
  host: string;
  token: string;
  name: string;
}

export interface StartScreenProps {
  language: Language;
  /** Window caption above the page (desktop title bar without a system frame, ADR-0016). */
  caption?: ReactNode;
  fetchImpl: FetchLike;
  /** Normalised server URL, or null. */
  serverUrl: string | null;
  /** The server comes from `BLUEOFFICE_SERVER` (the setting is ignored then). */
  serverFromEnv?: boolean;
  onServerUrlChange: (url: string | null) => Promise<void> | void;
  syncFolders: SyncFolder[];
  onSyncFoldersChange: (folders: SyncFolder[]) => Promise<void> | void;
  recent: RecentFile[];
  onNew: (app: HostedApp) => void;
  onOpenLocal: () => void;
  onOpenRecent: (file: RecentFile) => void;
  /** Signed-in user of the BlueDrive tab (lifted so the shell can resolve sync folders). */
  drive: DriveSession | null;
  onDriveChange: (s: DriveSession | null) => void;
  onOpenDriveFile: (host: string, item: DriveItem, token: string) => void;
  /** System-browser sign-in (OAuth code + PKCE); resolves the host token. */
  signInWithBrowser?: (host: DriveHostInfo) => Promise<string>;
  /** Initial tab. */
  tab?: 'home' | 'drive' | 'settings';
  /** "Check for updates" (desktop auto-update). */
  updates?: UpdateService;
}

const C = {
  bg: '#f6f8fb',
  panel: '#ffffff',
  line: '#d9e1ec',
  text: '#16233a',
  muted: '#5b6b82',
  accent: '#0b5cad',
  accentSoft: '#e6f0fb',
  warn: '#8a4b00',
};

const page: CSSProperties = {
  position: 'fixed',
  inset: 0,
  display: 'flex',
  flexDirection: 'column',
  background: C.bg,
  color: C.text,
  font: '14px/1.45 system-ui, "Segoe UI", sans-serif',
  overflow: 'auto',
  zIndex: 900,
};
const button: CSSProperties = {
  border: `1px solid ${C.line}`,
  background: C.panel,
  color: C.text,
  borderRadius: 6,
  padding: '6px 12px',
  font: 'inherit',
  cursor: 'pointer',
};
const primary: CSSProperties = {
  ...button,
  background: C.accent,
  borderColor: C.accent,
  color: '#fff',
};
const input: CSSProperties = {
  border: `1px solid ${C.line}`,
  borderRadius: 6,
  padding: '6px 8px',
  font: 'inherit',
  minWidth: 0,
};

function Tab(props: { id: string; active: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      role="tab"
      data-testid={`start-tab-${props.id}`}
      aria-selected={props.active}
      onClick={props.onClick}
      style={{
        ...button,
        border: 'none',
        borderBottom: `2px solid ${props.active ? C.accent : 'transparent'}`,
        borderRadius: 0,
        background: 'transparent',
        color: props.active ? C.accent : C.text,
        fontWeight: props.active ? 600 : 400,
      }}
    >
      {props.children}
    </button>
  );
}

const fileIcon = (name: string): string => {
  const ext = name.toLowerCase().split('.').pop() ?? '';
  if (['xlsx', 'xlsm', 'xls', 'xlsb', 'ods', 'csv', 'slk', 'dif', 'prn', 'dbf'].includes(ext))
    return '▦';
  if (['pptx', 'ppt', 'odp'].includes(ext)) return '▭';
  return '≡';
};

function ItemRow(props: { item: DriveItem; language: Language; onOpen: () => void }) {
  const { item, language } = props;
  const when = item.openedAt ?? item.lastModified;
  return (
    <li>
      <button
        type="button"
        data-testid="drive-item"
        data-kind={item.kind}
        data-name={item.name}
        onClick={props.onOpen}
        style={{
          ...button,
          border: 'none',
          width: '100%',
          display: 'flex',
          gap: 10,
          textAlign: 'left',
          alignItems: 'baseline',
        }}
      >
        <span aria-hidden style={{ width: 16, color: C.accent }}>
          {item.kind === 'folder' ? '▸' : fileIcon(item.name)}
        </span>
        <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.name}</span>
        {when ? (
          <span style={{ color: C.muted, fontSize: 12 }}>
            {startText(language, 'modified', {
              date: new Date(when).toLocaleString(language),
            })}
          </span>
        ) : null}
      </button>
    </li>
  );
}

type DriveView = 'files' | 'recent' | 'shared';

function DriveTab(p: StartScreenProps) {
  const t = (k: StartKey, v?: Record<string, string>) => startText(p.language, k, v);
  const [hosts, setHosts] = useState<DriveHostInfo[] | null>(null);
  const [hostName, setHostName] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [token, setToken] = useState('');
  const [waiting, setWaiting] = useState(false);
  const [view, setView] = useState<DriveView>('files');
  const [folderId, setFolderId] = useState('root');
  const [result, setResult] = useState<{
    key: string;
    listing: DriveListing | null;
    items: DriveItem[];
  } | null>(null);
  const { serverUrl, fetchImpl, drive, onDriveChange } = p;

  useEffect(() => {
    if (!serverUrl) return;
    let live = true;
    listHosts(serverUrl, fetchImpl)
      .then((list) => {
        if (!live) return;
        const browsable = list.filter((h) => h.browse);
        setHosts(browsable);
        setHostName((h) => h ?? drive?.host ?? browsable[0]?.name ?? null);
      })
      .catch((e: unknown) => {
        if (live) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      live = false;
    };
  }, [serverUrl, fetchImpl, drive?.host]);

  const host = hosts?.find((h) => h.name === hostName) ?? null;
  const client =
    serverUrl && drive && host && drive.host === host.name
      ? new DriveClient(serverUrl, host.name, drive.token, fetchImpl)
      : null;
  const clientKey = client ? `${client.host}:${client.token}` : '';
  const loadKey = `${clientKey}|${view}|${folderId}`;
  const current = result?.key === loadKey ? result : null;
  const listing = current?.listing ?? null;
  const items = current?.items ?? null;

  const fail = useCallback(
    (e: unknown) => {
      if (e instanceof DriveError && e.status === 401) {
        onDriveChange(null);
        setError(startText(p.language, 'sessionExpired'));
      } else {
        setError(
          startText(p.language, 'loadFailed', {
            message: e instanceof Error ? e.message : String(e),
          }),
        );
      }
    },
    [onDriveChange, p.language],
  );

  useEffect(() => {
    if (!client) return;
    let live = true;
    const load: Promise<{ listing: DriveListing | null; items: DriveItem[] }> =
      view === 'files'
        ? client.children(folderId).then((l) => ({ listing: l, items: l.items }))
        : (view === 'recent' ? client.recent() : client.shared()).then((list) => ({
            listing: null,
            items: list,
          }));
    load.then(
      (r) => {
        if (!live) return;
        setError(null);
        setResult({ key: loadKey, ...r });
      },
      (e: unknown) => {
        if (live) fail(e);
      },
    );
    return () => {
      live = false;
    };
    // `client` is rebuilt every render; its identity is `clientKey`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadKey, fail]);

  const signIn = async (accessToken: string) => {
    if (!serverUrl || !host) return;
    setError(null);
    try {
      const me = await new DriveClient(serverUrl, host.name, accessToken, fetchImpl).me();
      setToken('');
      setFolderId('root');
      onDriveChange({ host: host.name, token: accessToken, name: me.name || me.id });
    } catch (e) {
      setError(t('signInFailed', { message: e instanceof Error ? e.message : String(e) }));
    }
  };

  if (!serverUrl) return <p data-testid="drive-no-server">{t('driveNoServer')}</p>;
  if (hosts && hosts.length === 0) return <p>{t('driveNoHost')}</p>;
  if (!host) return <p>{error ?? t('loading')}</p>;

  const errorLine = error ? (
    <p role="alert" data-testid="drive-error" style={{ color: C.warn }}>
      {error}
    </p>
  ) : null;

  if (!client) {
    return (
      <section data-testid="drive-signin" style={{ display: 'grid', gap: 12, maxWidth: 520 }}>
        {hosts && hosts.length > 1 ? (
          <select
            aria-label={t('tabDrive')}
            value={host.name}
            onChange={(e) => setHostName(e.target.value)}
            style={input}
          >
            {hosts.map((h) => (
              <option key={h.name} value={h.name}>
                {h.displayName}
              </option>
            ))}
          </select>
        ) : null}
        {host.signIn && p.signInWithBrowser ? (
          <button
            type="button"
            style={primary}
            disabled={waiting}
            data-testid="drive-signin-sso"
            onClick={() => {
              const sso = p.signInWithBrowser;
              if (!sso) return;
              setWaiting(true);
              sso(host)
                .then(signIn)
                .catch((e: unknown) =>
                  setError(
                    t('signInFailed', { message: e instanceof Error ? e.message : String(e) }),
                  ),
                )
                .finally(() => setWaiting(false));
            }}
          >
            {waiting ? t('signInWaiting') : t('signInSso')}
          </button>
        ) : null}
        <label style={{ display: 'grid', gap: 4 }}>
          <span style={{ color: C.muted }}>{t('tokenLabel')}</span>
          <textarea
            data-testid="drive-token"
            rows={3}
            value={token}
            onChange={(e) => setToken(e.target.value)}
            style={{ ...input, fontFamily: 'ui-monospace, monospace', fontSize: 12 }}
          />
        </label>
        <div>
          <button
            type="button"
            style={button}
            data-testid="drive-signin-token"
            disabled={!token.trim()}
            onClick={() => void signIn(token.trim())}
          >
            {t('signInToken')}
          </button>
        </div>
        {errorLine}
      </section>
    );
  }

  const open = (item: DriveItem) => {
    if (item.kind === 'folder') {
      setView('files');
      setFolderId(item.id);
    } else p.onOpenDriveFile(client.host, item, client.token);
  };

  return (
    <section data-testid="drive-browser" style={{ display: 'grid', gap: 12 }}>
      <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
        <span data-testid="drive-user" style={{ color: C.muted }}>
          {t('signedInAs', { name: drive?.name ?? '' })}
        </span>
        <button type="button" style={button} onClick={() => onDriveChange(null)}>
          {t('signOut')}
        </button>
      </div>
      <div role="tablist" style={{ display: 'flex', gap: 4, borderBottom: `1px solid ${C.line}` }}>
        <Tab id="drive-files" active={view === 'files'} onClick={() => setView('files')}>
          {t('myFiles')}
        </Tab>
        <Tab id="drive-recent" active={view === 'recent'} onClick={() => setView('recent')}>
          {t('recentDrive')}
        </Tab>
        <Tab id="drive-shared" active={view === 'shared'} onClick={() => setView('shared')}>
          {t('shared')}
        </Tab>
      </div>
      {view === 'files' && listing ? (
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          {listing.folder.parentId ? (
            <button
              type="button"
              style={button}
              data-testid="drive-up"
              onClick={() => setFolderId(listing.folder.parentId ?? 'root')}
            >
              ↑ {t('up')}
            </button>
          ) : null}
          <strong data-testid="drive-folder">{listing.folder.name}</strong>
        </div>
      ) : null}
      {errorLine}
      {items === null ? (
        <p>{t('loading')}</p>
      ) : items.length === 0 ? (
        <p style={{ color: C.muted }}>{t('empty')}</p>
      ) : (
        <ul
          data-testid={`drive-list-${view}`}
          style={{
            listStyle: 'none',
            margin: 0,
            padding: 0,
            background: C.panel,
            border: `1px solid ${C.line}`,
            borderRadius: 8,
          }}
        >
          {items.map((item) => (
            <ItemRow
              key={`${item.kind}:${item.id}`}
              item={item}
              language={p.language}
              onOpen={() => open(item)}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

/** `http(s)://authority[/path]` without query, fragment or credentials (mirrors the desktop's check). */
export function normalizeServerUrl(value: string): string | null {
  const v = value.trim().replace(/\/+$/, '');
  const m = /^https?:\/\/([^/?#@\s]+)(\/[^?#@\s]*)?$/.exec(v);
  return m && /^[A-Za-z0-9.\-:[\]]+$/.test(m[1] ?? '') ? v : null;
}

function SettingsTab(p: StartScreenProps) {
  const t = (k: StartKey) => startText(p.language, k);
  const [server, setServer] = useState(p.serverUrl ?? '');
  const [note, setNote] = useState<string | null>(null);
  const [path, setPath] = useState('');
  const [hosts, setHosts] = useState<string[]>([]);
  const [host, setHost] = useState('bluedrive');
  useEffect(() => {
    if (!p.serverUrl) return;
    let live = true;
    listHosts(p.serverUrl, p.fetchImpl)
      .then((l) => {
        if (!live) return;
        const names = l.filter((h) => h.browse).map((h) => h.name);
        setHosts(names);
        if (names[0]) setHost(names[0]);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [p.serverUrl, p.fetchImpl]);
  const save = async () => {
    const trimmed = server.trim();
    if (!trimmed) {
      await p.onServerUrlChange(null);
      setNote(t('serverSaved'));
      return;
    }
    const url = normalizeServerUrl(trimmed);
    if (!url) return setNote(t('serverInvalid'));
    await p.onServerUrlChange(url);
    setNote(t('serverSaved'));
  };
  return (
    <section style={{ display: 'grid', gap: 20, maxWidth: 640 }}>
      <div style={{ display: 'grid', gap: 6 }}>
        <label htmlFor="bo-start-server" style={{ fontWeight: 600 }}>
          {t('serverLabel')}
        </label>
        <div style={{ display: 'flex', gap: 8 }}>
          <input
            id="bo-start-server"
            data-testid="settings-server"
            style={{ ...input, flex: 1 }}
            placeholder={t('serverHint')}
            value={server}
            disabled={p.serverFromEnv}
            onChange={(e) => {
              setServer(e.target.value);
              setNote(null);
            }}
          />
          <button
            type="button"
            style={primary}
            data-testid="settings-server-save"
            disabled={p.serverFromEnv}
            onClick={() => void save()}
          >
            {t('serverSave')}
          </button>
        </div>
        {p.serverFromEnv ? <span style={{ color: C.muted }}>{t('serverFromEnv')}</span> : null}
        {note ? (
          <span role="status" data-testid="settings-note" style={{ color: C.muted }}>
            {note}
          </span>
        ) : null}
      </div>
      <div style={{ display: 'grid', gap: 6 }}>
        <strong>{t('syncFolders')}</strong>
        <span style={{ color: C.muted }}>{t('syncFoldersHint')}</span>
        {p.syncFolders.length === 0 ? (
          <span style={{ color: C.muted }}>{t('syncNone')}</span>
        ) : null}
        <ul data-testid="settings-sync-list" style={{ listStyle: 'none', margin: 0, padding: 0 }}>
          {p.syncFolders.map((f, i) => (
            <li
              key={`${f.host}:${f.path}`}
              style={{ display: 'flex', gap: 8, alignItems: 'center' }}
            >
              <code style={{ flex: 1 }}>{f.path}</code>
              <span style={{ color: C.muted }}>{f.host}</span>
              <button
                type="button"
                style={button}
                onClick={() => void p.onSyncFoldersChange(p.syncFolders.filter((_, j) => j !== i))}
              >
                {t('syncRemove')}
              </button>
            </li>
          ))}
        </ul>
        <div style={{ display: 'flex', gap: 8 }}>
          <input
            data-testid="settings-sync-path"
            aria-label={t('syncFolderPath')}
            placeholder={t('syncFolderPath')}
            style={{ ...input, flex: 1 }}
            value={path}
            onChange={(e) => setPath(e.target.value)}
          />
          {hosts.length > 1 ? (
            <select value={host} onChange={(e) => setHost(e.target.value)} style={input}>
              {hosts.map((h) => (
                <option key={h}>{h}</option>
              ))}
            </select>
          ) : null}
          <button
            type="button"
            style={button}
            data-testid="settings-sync-add"
            disabled={!path.trim()}
            onClick={() => {
              const next = [
                ...p.syncFolders.filter((f) => f.path !== path.trim()),
                { host, path: path.trim() },
              ];
              setPath('');
              void p.onSyncFoldersChange(next);
            }}
          >
            {t('syncAdd')}
          </button>
        </div>
      </div>
      {p.updates ? <UpdatesSection language={p.language} updates={p.updates} /> : null}
    </section>
  );
}

/** Settings → Updates: check, then install and restart when a newer version exists. */
export function UpdatesSection({
  language,
  updates,
}: {
  language: Language;
  updates: UpdateService;
}) {
  const t = (k: UpdatesKey) => updatesText(language, k);
  const [busy, setBusy] = useState<'check' | 'install' | null>(null);
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const check = async () => {
    setBusy('check');
    setError(null);
    try {
      setStatus(await updates.check());
    } finally {
      setBusy(null);
    }
  };
  const install = async () => {
    setBusy('install');
    setError(null);
    try {
      await updates.install();
    } catch (e) {
      setError(
        updatesText(language, 'installFailed', {
          message: e instanceof Error ? e.message : String(e),
        }),
      );
    } finally {
      setBusy(null);
    }
  };
  const notice = status ? updateNotice(status) : null;
  return (
    <div style={{ display: 'grid', gap: 6 }} data-testid="settings-updates">
      <strong>{t('title')}</strong>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
        <button
          type="button"
          style={button}
          data-testid="settings-update-check"
          disabled={busy !== null}
          onClick={() => void check()}
        >
          {t('check')}
        </button>
        {status?.state === 'available' ? (
          <button
            type="button"
            style={primary}
            data-testid="settings-update-install"
            disabled={busy !== null}
            onClick={() => void install()}
          >
            {t('install')}
          </button>
        ) : null}
      </div>
      <span
        role="status"
        data-testid="settings-update-status"
        data-state={busy ?? status?.state ?? ''}
        style={{ color: C.muted }}
      >
        {busy === 'check'
          ? t('checking')
          : busy === 'install'
            ? t('installing')
            : (error ??
              (notice ? updatesText(language, notice.key as UpdatesKey, notice.params) : ''))}
      </span>
    </div>
  );
}

function HomeTab(p: StartScreenProps) {
  const t = (k: StartKey) => startText(p.language, k);
  const tile = (app: HostedApp, key: StartKey, mark: string) => (
    <button
      type="button"
      data-testid={`start-new-${app}`}
      onClick={() => p.onNew(app)}
      style={{ ...button, width: 168, height: 96, display: 'grid', placeItems: 'center', gap: 4 }}
    >
      <span aria-hidden style={{ fontSize: 24, color: C.accent }}>
        {mark}
      </span>
      {t(key)}
    </button>
  );
  return (
    <section style={{ display: 'grid', gap: 20 }}>
      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        {tile('doc', 'newDoc', '≡')}
        {tile('sheet', 'newSheet', '▦')}
        {tile('slide', 'newSlide', '▭')}
      </div>
      <div>
        <button type="button" style={button} data-testid="start-open-local" onClick={p.onOpenLocal}>
          {t('openLocal')}
        </button>
      </div>
      <div style={{ display: 'grid', gap: 6 }}>
        <strong>{t('recentLocal')}</strong>
        {p.recent.length === 0 ? (
          <span style={{ color: C.muted }}>{t('noRecent')}</span>
        ) : (
          <ul data-testid="start-recent" style={{ listStyle: 'none', margin: 0, padding: 0 }}>
            {p.recent.map((f) => (
              <li key={f.path}>
                <button
                  type="button"
                  style={{ ...button, border: 'none', width: '100%', textAlign: 'left' }}
                  title={f.path}
                  onClick={() => p.onOpenRecent(f)}
                >
                  {fileIcon(f.name)} {f.name}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

export function StartScreen(p: StartScreenProps) {
  const [tab, setTab] = useState(p.tab ?? 'home');
  const t = (k: StartKey) => startText(p.language, k);
  return (
    <div data-testid="start-screen" style={page}>
      {p.caption && (
        <div style={{ position: 'sticky', top: 0, zIndex: 1, flex: 'none' }}>{p.caption}</div>
      )}
      <header
        style={{
          display: 'flex',
          alignItems: 'flex-end',
          gap: 24,
          padding: '16px 32px 0',
          background: C.panel,
          borderBottom: `1px solid ${C.line}`,
        }}
      >
        <h1 style={{ margin: '0 0 10px', fontSize: 20, color: C.accent }}>BlueOffice</h1>
        <nav role="tablist" style={{ display: 'flex', gap: 4 }}>
          <Tab id="home" active={tab === 'home'} onClick={() => setTab('home')}>
            {t('tabHome')}
          </Tab>
          <Tab id="drive" active={tab === 'drive'} onClick={() => setTab('drive')}>
            {t('tabDrive')}
          </Tab>
          <Tab id="settings" active={tab === 'settings'} onClick={() => setTab('settings')}>
            {t('tabSettings')}
          </Tab>
        </nav>
      </header>
      <main style={{ padding: 32 }}>
        {tab === 'home' ? (
          <HomeTab {...p} />
        ) : tab === 'drive' ? (
          <DriveTab {...p} />
        ) : (
          <SettingsTab {...p} />
        )}
      </main>
    </div>
  );
}
