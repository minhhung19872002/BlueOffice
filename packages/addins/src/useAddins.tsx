/**
 * Add-ins in an editor (CORE-ADDIN-001, ADR-0014): loads the organisation
 * catalogue (`Platform.addins`), registers the add-ins' ribbon commands in the
 * editor's command registry, shows the catalogue pane (install with consent,
 * uninstall, open) and one task pane per open add-in, whose sandboxed frame
 * talks to the document only through an {@link AddinBridge}.
 */
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Blocks, Puzzle } from 'lucide-react';
import {
  localize,
  networkOrigins,
  ribbonCommands,
  validateManifest,
  type AddinApp,
  type AddinEvent,
  type AddinManifest,
  type AddinSelection,
} from '@blueoffice/addin-sdk';
import { useTranslation } from '@blueoffice/i18n';
import type { AddinCatalog } from '@blueoffice/platform';
import type { ScriptHost } from '@blueoffice/script';
import { Dialog, type RibbonItem, type RibbonTab, type TaskPaneDef } from '@blueoffice/ui';
import { AddinBridge } from './bridge';
import { frameAttributes } from './frame';
import { ADDINS_TAB, mergeAddinTabs, type AddinGroup } from './ribbon';
import { addinStorage, defaultBacking, type KeyValueBacking } from './storage';
import styles from './AddinsPane.module.css';

/** Pane of the catalogue. */
export const CATALOGUE_PANE = 'addins';
/** Pane id of add-in `id`. */
export const addinPaneId = (id: string) => `addin:${id}`;
export const MANAGE_COMMAND = 'addins.manage';
/** Command id of ribbon command `cmd` of add-in `id`. */
export const addinCommandId = (id: string, cmd: string) => `addin.${id}.${cmd}`;

/** A catalogue entry whose manifest passed validation. */
export interface AddinEntry {
  manifest: AddinManifest;
  trusted: boolean;
  installed: boolean;
  granted: string[];
}

/** What the hook needs from the editor's `CommandRegistry`. */
export interface AddinCommandRegistry {
  register(cmd: {
    id: string;
    run: () => void | Promise<void>;
    isEnabled?: () => boolean;
  }): () => void;
}

export interface UseAddinsOptions {
  app: AddinApp;
  catalog: AddinCatalog | undefined;
  registry: AddinCommandRegistry;
  /** BlueScript host of the open document; null while none is open. */
  host: () => ScriptHost | null;
  /** The selection with its content (`addin.selection`). */
  selection: (host: ScriptHost) => Promise<AddinSelection>;
  /** Changes whenever the selection changes (a string or number). */
  selectionKey: unknown;
  /** Changes whenever the document changes. */
  documentKey: unknown;
  /** Local storage of the add-ins (default: the browser's). */
  backing?: KeyValueBacking;
  /** Origin serving `/addin-sdk/v1.js` (default: this page's). */
  editorOrigin?: string;
}

export interface Addins {
  /** The catalogue answered (feature on). */
  enabled: boolean;
  entries: AddinEntry[];
  /** Ribbon tabs with the add-in groups and the "Add-in" tab (unchanged while off). */
  applyTabs(tabs: RibbonTab[]): RibbonTab[];
  panes: TaskPaneDef[];
  /** Closes pane `id` when it is an add-in pane; false otherwise. */
  close(id: string): boolean;
  /** Consent dialog. */
  element: ReactNode;
}

const PERM_KEYS: Record<string, string> = {
  'read-document': 'addins.perm.readDocument',
  'write-document': 'addins.perm.writeDocument',
  selection: 'addins.perm.selection',
  'ui:ribbon': 'addins.perm.ribbon',
  storage: 'addins.perm.storage',
};

function PermissionList({ permissions }: { permissions: readonly string[] }) {
  const { t } = useTranslation();
  const network = networkOrigins(permissions);
  const base = permissions.filter((p) => PERM_KEYS[p]);
  if (!base.length && !network.length) return <p>{t('addins.consent.none')}</p>;
  return (
    <ul className={styles.perms} data-testid="addin-permissions">
      {base.map((p) => (
        <li key={p} data-permission={p}>
          {t(PERM_KEYS[p]!)}
        </li>
      ))}
      {network.map((o) => (
        <li key={o} data-permission={`network:${o}`}>
          {t('addins.perm.network', { origin: o })}
        </li>
      ))}
    </ul>
  );
}

interface FrameProps {
  entry: AddinEntry;
  app: AddinApp;
  editorOrigin: string;
  language: string;
  bridgeOptions: () => Omit<
    ConstructorParameters<typeof AddinBridge>[0],
    'frame' | 'manifest' | 'app' | 'granted' | 'language'
  >;
  onBridge: (id: string, bridge: AddinBridge | null) => void;
}

/** The sandboxed frame of one add-in and its bridge. */
function AddinFrame({ entry, app, editorOrigin, language, bridgeOptions, onBridge }: FrameProps) {
  const ref = useRef<HTMLIFrameElement>(null);
  const { t } = useTranslation();
  const { manifest, trusted } = entry;
  const attrs = useMemo(
    () => frameAttributes(manifest, trusted, editorOrigin),
    [manifest, trusted, editorOrigin],
  );
  const lang = useRef(language);
  const options = useRef(bridgeOptions);
  useEffect(() => {
    lang.current = language;
    options.current = bridgeOptions;
  });
  const granted = entry.granted.join(' ');
  useEffect(() => {
    const win = ref.current?.contentWindow;
    if (!win) return;
    const bridge = new AddinBridge({
      ...options.current(),
      manifest,
      app,
      granted: granted ? granted.split(' ') : [],
      frame: win,
      language: () => lang.current,
    });
    const onMessage = (e: MessageEvent) => bridge.handle(e);
    window.addEventListener('message', onMessage);
    onBridge(manifest.id, bridge);
    return () => {
      window.removeEventListener('message', onMessage);
      bridge.dispose();
      onBridge(manifest.id, null);
    };
  }, [manifest, app, granted, onBridge]);
  const name = localize(manifest.name, language);
  return (
    <iframe
      ref={ref}
      className={styles.frame}
      title={t('addins.frameTitle', { name })}
      data-testid="addin-frame"
      data-addin={manifest.id}
      src={attrs.src}
      sandbox={attrs.sandbox}
      allow={attrs.allow}
      referrerPolicy={attrs.referrerPolicy}
      {...{ csp: attrs.csp }}
    />
  );
}

export function useAddins(o: UseAddinsOptions): Addins {
  const { t, i18n } = useTranslation();
  const language = i18n.language;
  const { app, catalog, registry } = o;
  const [entries, setEntries] = useState<AddinEntry[] | null>(null);
  const [open, setOpen] = useState<string[]>([]);
  const [consent, setConsent] = useState<AddinEntry | null>(null);
  const [error, setError] = useState<string | null>(null);
  const bridges = useRef(new Map<string, AddinBridge>());
  /** Ribbon command events waiting for the add-in's frame. */
  const waiting = useRef(new Map<string, AddinEvent[]>());
  const latest = useRef(o);
  useEffect(() => {
    latest.current = o;
  });
  const backing = useMemo(() => o.backing ?? defaultBacking(), [o.backing]);
  const editorOrigin = o.editorOrigin ?? (typeof location !== 'undefined' ? location.origin : '');

  /** The catalogue entries usable here; null when the feature is off. */
  const load = useCallback(async (): Promise<AddinEntry[] | null> => {
    if (!catalog) return null;
    const list = await catalog.list(app);
    if (!list) return null;
    const valid: AddinEntry[] = [];
    for (const l of list) {
      const r = validateManifest(l.manifest);
      if (!r.ok || !r.manifest.apps.includes(app)) continue;
      // Uninstalled elsewhere: its data on this device goes too.
      if (!l.installed) addinStorage(r.manifest.id, backing).clear();
      valid.push({
        manifest: r.manifest,
        trusted: l.trusted,
        installed: l.installed,
        granted: l.granted,
      });
    }
    return valid;
  }, [catalog, app, backing]);
  const reload = useCallback(async () => setEntries(await load()), [load]);

  useEffect(() => {
    let live = true;
    load().then(
      (v) => live && setEntries(v),
      () => live && setEntries(null),
    );
    return () => {
      live = false;
    };
  }, [load]);

  const installed = useMemo(() => (entries ?? []).filter((e) => e.installed), [entries]);

  const openPane = useCallback((id: string) => {
    setOpen((l) => (l.includes(id) ? l : [...l, id]));
  }, []);
  const closePane = useCallback((id: string) => {
    setOpen((l) => l.filter((x) => x !== id));
  }, []);

  const send = useCallback(
    (id: string, ev: AddinEvent) => {
      const b = bridges.current.get(id);
      if (b) b.emit(ev);
      else waiting.current.set(id, [...(waiting.current.get(id) ?? []), ev].slice(-10));
      openPane(addinPaneId(id));
    },
    [openPane],
  );

  const onBridge = useCallback((id: string, bridge: AddinBridge | null) => {
    if (!bridge) {
      bridges.current.delete(id);
      return;
    }
    bridges.current.set(id, bridge);
    for (const ev of waiting.current.get(id) ?? []) bridge.emit(ev);
    waiting.current.delete(id);
  }, []);

  // Commands: "Manage add-ins" and each ribbon command of an installed add-in.
  useEffect(() => {
    if (!entries) return;
    const offs: (() => void)[] = [];
    const add = (cmd: Parameters<AddinCommandRegistry['register']>[0]) => {
      try {
        offs.push(registry.register(cmd));
      } catch {
        // Already registered (another editor instance of the same registry): keep the first.
      }
    };
    add({ id: MANAGE_COMMAND, run: () => openPane(CATALOGUE_PANE) });
    for (const e of installed) {
      for (const c of ribbonCommands(e.manifest, app)) {
        const id = addinCommandId(e.manifest.id, c.id);
        add({
          id,
          run: () =>
            c.action === 'event'
              ? send(e.manifest.id, { type: 'command', command: c.id })
              : openPane(addinPaneId(e.manifest.id)),
        });
      }
    }
    return () => offs.forEach((off) => off());
  }, [entries, installed, registry, app, openPane, send]);

  // Events: selection / document changes reach the open add-ins (their bridges filter by permission).
  useEffect(() => {
    const h = setTimeout(() => {
      for (const b of bridges.current.values()) b.emit({ type: 'selectionChanged' });
    }, 30);
    return () => clearTimeout(h);
  }, [o.selectionKey]);
  useEffect(() => {
    const h = setTimeout(() => {
      for (const b of bridges.current.values()) b.emit({ type: 'documentChanged' });
    }, 30);
    return () => clearTimeout(h);
  }, [o.documentKey]);

  const run = useCallback(
    async (f: () => Promise<void>) => {
      setError(null);
      try {
        await f();
      } catch (e) {
        setError(t('addins.failed', { message: e instanceof Error ? e.message : String(e) }));
      }
    },
    [t],
  );

  const install = useCallback(
    (e: AddinEntry) =>
      run(async () => {
        if (!catalog) return;
        await catalog.install(e.manifest.id, [...e.manifest.permissions]);
        setConsent(null);
        await reload();
        openPane(addinPaneId(e.manifest.id));
      }),
    [catalog, reload, openPane, run],
  );
  const uninstall = useCallback(
    (e: AddinEntry) =>
      run(async () => {
        if (!catalog) return;
        await catalog.uninstall(e.manifest.id);
        bridges.current.get(e.manifest.id)?.dispose();
        closePane(addinPaneId(e.manifest.id));
        addinStorage(e.manifest.id, backing).clear();
        await reload();
      }),
    [catalog, reload, closePane, backing, run],
  );

  const bridgeOptions = useCallback(
    (id: string) => () => ({
      host: () => latest.current.host(),
      selection: () => {
        const h = latest.current.host();
        return h ? latest.current.selection(h) : Promise.reject(new Error('no document is open'));
      },
      storage: addinStorage(id, backing),
    }),
    [backing],
  );

  const enabled = entries !== null;

  const applyTabs = useCallback(
    (tabs: RibbonTab[]): RibbonTab[] => {
      if (!enabled) return tabs;
      const groups: AddinGroup[] = [];
      for (const e of installed) {
        const cmds = ribbonCommands(e.manifest, app);
        if (!cmds.length) continue;
        const items: RibbonItem[] = cmds.map((c) => {
          const label = localize(c.label, language);
          return {
            id: addinCommandId(e.manifest.id, c.id),
            label,
            icon: <Puzzle size={20} aria-hidden />,
            size: 'large',
            tooltip: c.tooltip ? localize(c.tooltip, language) : label,
            onSelect: () =>
              c.action === 'event'
                ? send(e.manifest.id, { type: 'command', command: c.id })
                : openPane(addinPaneId(e.manifest.id)),
          };
        });
        groups.push({
          ...(e.manifest.ribbon?.tab ? { tab: e.manifest.ribbon.tab } : {}),
          group: {
            id: `addin-${e.manifest.id}`,
            label: e.manifest.ribbon?.group
              ? localize(e.manifest.ribbon.group, language)
              : localize(e.manifest.name, language),
            items,
          },
        });
      }
      const addinTab: RibbonTab = {
        id: ADDINS_TAB,
        label: t('addins.tab'),
        groups: [
          {
            id: 'addinsManage',
            label: t('addins.group'),
            items: [
              {
                id: MANAGE_COMMAND,
                label: t('addins.manage'),
                icon: <Blocks size={20} aria-hidden />,
                size: 'large',
                onSelect: () => openPane(CATALOGUE_PANE),
              },
            ],
          },
        ],
      };
      return mergeAddinTabs(tabs, groups, addinTab);
    },
    [enabled, installed, app, language, t, send, openPane],
  );

  const panes: TaskPaneDef[] = [];
  if (enabled) {
    for (const id of open) {
      if (id === CATALOGUE_PANE) {
        panes.push({
          id,
          title: t('addins.paneTitle'),
          icon: <Blocks size={14} aria-hidden />,
          content: (
            <div className={styles.pane} data-testid="addins-pane">
              {error && (
                <div className={styles.error} role="alert" data-testid="addins-error">
                  {error}
                </div>
              )}
              {entries && entries.length === 0 && <p>{t('addins.empty')}</p>}
              {(entries ?? []).map((e) => (
                <div
                  key={e.manifest.id}
                  className={styles.item}
                  data-testid="addin-item"
                  data-addin={e.manifest.id}
                  data-installed={e.installed}
                >
                  <div className={styles.head}>
                    {e.manifest.icon ? (
                      <img
                        className={styles.icon}
                        src={e.manifest.icon}
                        alt=""
                        referrerPolicy="no-referrer"
                      />
                    ) : (
                      <Puzzle size={20} aria-hidden />
                    )}
                    <div>
                      <div className={styles.name}>{localize(e.manifest.name, language)}</div>
                      <div className={styles.meta}>
                        {t('addins.publisher', {
                          publisher: e.manifest.publisher,
                          version: e.manifest.version,
                        })}
                        {e.installed ? ` · ${t('addins.installed')}` : ''}
                      </div>
                    </div>
                  </div>
                  {e.manifest.description && (
                    <div>{localize(e.manifest.description, language)}</div>
                  )}
                  {e.trusted && <div className={styles.meta}>{t('addins.trusted')}</div>}
                  <div className={styles.row}>
                    {e.installed ? (
                      <>
                        <button
                          type="button"
                          data-testid="addin-open"
                          onClick={() => openPane(addinPaneId(e.manifest.id))}
                        >
                          {t('addins.open')}
                        </button>
                        <button
                          type="button"
                          data-testid="addin-uninstall"
                          onClick={() => void uninstall(e)}
                        >
                          {t('addins.uninstall')}
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        data-testid="addin-install"
                        onClick={() => setConsent(e)}
                      >
                        {t('addins.install')}
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ),
        });
        continue;
      }
      const e = installed.find((x) => addinPaneId(x.manifest.id) === id);
      if (!e) continue;
      panes.push({
        id,
        title: localize(e.manifest.name, language),
        icon: <Puzzle size={14} aria-hidden />,
        content: (
          <AddinFrame
            entry={e}
            app={app}
            editorOrigin={editorOrigin}
            language={language}
            bridgeOptions={bridgeOptions(e.manifest.id)}
            onBridge={onBridge}
          />
        ),
      });
    }
  }

  const element = consent ? (
    <Dialog
      open
      onOpenChange={(v) => !v && setConsent(null)}
      title={t('addins.consent.title', { name: localize(consent.manifest.name, language) })}
      testId="addin-consent"
      footer={
        <div className={styles.footer}>
          <button type="button" onClick={() => setConsent(null)} data-testid="addin-consent-cancel">
            {t('addins.consent.cancel')}
          </button>
          <button
            type="button"
            className={styles.primary}
            data-testid="addin-consent-accept"
            onClick={() => void install(consent)}
          >
            {t('addins.consent.accept')}
          </button>
        </div>
      }
    >
      <p>
        {t('addins.consent.intro', {
          publisher: consent.manifest.publisher,
          origin: new URL(consent.manifest.startUrl).origin,
        })}
      </p>
      <PermissionList permissions={consent.manifest.permissions} />
      <p className={styles.meta}>{t('addins.consent.note')}</p>
      {error && (
        <div className={styles.error} role="alert">
          {error}
        </div>
      )}
    </Dialog>
  ) : null;

  return {
    enabled,
    entries: entries ?? [],
    applyTabs,
    panes,
    close: (id: string) => {
      if (id !== CATALOGUE_PANE && !id.startsWith('addin:')) return false;
      closePane(id);
      return true;
    },
    element,
  };
}
