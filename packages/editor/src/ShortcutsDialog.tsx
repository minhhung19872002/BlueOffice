import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type { FileAccess } from '@blueoffice/platform';
import { Dialog, normalize, type SettingStore } from '@blueoffice/ui';
import type { CommandRegistry } from './commands';
import {
  assignShortcut,
  exportShortcuts,
  importShortcuts,
  loadShortcuts,
  saveShortcuts,
  shortcutFromKey,
  validShortcut,
  type ShortcutApp,
  type ShortcutMap,
} from './shortcuts';

export interface ShortcutCommand {
  id: string;
  label: string;
  /** Where it lives ("Tab › Group"). */
  path?: string;
}

export interface ShortcutsDialogProps<S> {
  open: boolean;
  onClose: () => void;
  registry: CommandRegistry<S>;
  /** Commands to list (labelled); the ones the registry lacks are left out. */
  commands: readonly ShortcutCommand[];
  /** The user's changes (applied to the registry by the caller). */
  value: ShortcutMap;
  onChange: (map: ShortcutMap) => void;
  onExport: () => void;
  onImport: () => void;
  /** Message after an import / export (shown under the list). */
  notice?: string | null;
}

type Pending = { id: string; combo: string | null; taken: string[]; invalid: boolean };

/**
 * Customise keyboard shortcuts (CORE-UI-004): every command with its shortcut; change it by
 * pressing the new keys, remove it, or reset it. A combo already used elsewhere is shown
 * with the command holding it and assigned only when confirmed (the other one loses it).
 */
export function ShortcutsDialog<S>({
  open,
  onClose,
  registry,
  commands,
  value,
  onChange,
  onExport,
  onImport,
  notice,
}: ShortcutsDialogProps<S>) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [pending, setPending] = useState<Pending | null>(null);
  const labelOf = useMemo(() => {
    const m = new Map(commands.map((c) => [c.id, c.label]));
    return (id: string) => m.get(id) ?? id;
  }, [commands]);
  const rows = useMemo(() => {
    const q = normalize(query);
    const seen = new Set<string>();
    return commands.filter((c) => {
      if (seen.has(c.id) || !registry.has(c.id)) return false;
      seen.add(c.id);
      if (!q) return true;
      const s = registry.shortcut(c.id) ?? '';
      return `${normalize(c.label)} ${normalize(c.path ?? '')} ${c.id} ${s.toLowerCase()}`.includes(
        q,
      );
    });
  }, [commands, query, registry, value]); // eslint-disable-line react-hooks/exhaustive-deps

  const commit = (id: string, combo: string | null, taken: readonly string[]) => {
    onChange(assignShortcut(value, id, combo, taken, (x) => registry.defaultShortcut(x)));
    setPending(null);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) {
          setPending(null);
          onClose();
        }
      }}
      title={t('shortcuts.title')}
      width={640}
      testId="shortcuts-dialog"
      footer={
        <>
          <button type="button" data-testid="shortcuts-export" onClick={onExport}>
            {t('shortcuts.export')}
          </button>
          <button type="button" data-testid="shortcuts-import" onClick={onImport}>
            {t('shortcuts.import')}
          </button>
          <button
            type="button"
            data-testid="shortcuts-reset-all"
            disabled={Object.keys(value).length === 0}
            onClick={() => onChange({})}
          >
            {t('shortcuts.resetAll')}
          </button>
          <button type="button" onClick={onClose}>
            {t('shortcuts.close')}
          </button>
        </>
      }
    >
      <input
        type="search"
        aria-label={t('shortcuts.search')}
        placeholder={t('shortcuts.search')}
        value={query}
        data-testid="shortcuts-search"
        onChange={(e) => setQuery(e.target.value)}
        style={{ width: '100%', boxSizing: 'border-box', marginBottom: 8 }}
      />
      <div style={{ maxHeight: 360, overflow: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr style={{ textAlign: 'left' }}>
              <th>{t('shortcuts.command')}</th>
              <th>{t('shortcuts.shortcut')}</th>
              <th>
                <span style={{ position: 'absolute', width: 1, height: 1, overflow: 'hidden' }}>
                  {t('shortcuts.actions')}
                </span>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((c) => {
              const current = registry.shortcut(c.id);
              const changed = c.id in value;
              const p = pending?.id === c.id ? pending : null;
              return (
                <tr key={c.id} data-shortcut-row={c.id}>
                  <td style={{ padding: '3px 4px' }}>
                    {c.label}
                    {c.path && <div style={{ fontSize: 11, opacity: 0.7 }}>{c.path}</div>}
                  </td>
                  <td style={{ padding: '3px 4px' }}>
                    {p ? (
                      <input
                        autoFocus
                        readOnly
                        aria-label={t('shortcuts.pressKeys', { command: c.label })}
                        placeholder={t('shortcuts.pressKeys', { command: c.label })}
                        value={p.combo ?? ''}
                        data-testid="shortcut-capture"
                        onKeyDown={(e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey) {
                            setPending(null);
                            return;
                          }
                          const combo = shortcutFromKey(e);
                          if (!combo) return;
                          const invalid = !validShortcut(combo);
                          setPending({
                            id: c.id,
                            combo,
                            invalid,
                            taken: invalid ? [] : registry.conflicts(combo, c.id),
                          });
                        }}
                      />
                    ) : (
                      <kbd data-testid="shortcut-value">{current ?? '—'}</kbd>
                    )}
                    {changed && !p && (
                      <span style={{ marginLeft: 6, fontSize: 11, opacity: 0.7 }}>
                        {t('shortcuts.custom')}
                      </span>
                    )}
                    {p?.invalid && (
                      <div role="alert" data-testid="shortcut-invalid" style={{ fontSize: 12 }}>
                        {t('shortcuts.invalid')}
                      </div>
                    )}
                    {p && !p.invalid && p.taken.length > 0 && (
                      <div role="alert" data-testid="shortcut-conflict" style={{ fontSize: 12 }}>
                        {t('shortcuts.conflict', {
                          commands: p.taken.map(labelOf).join(', '),
                        })}
                      </div>
                    )}
                  </td>
                  <td style={{ padding: '3px 4px', whiteSpace: 'nowrap', textAlign: 'right' }}>
                    {p ? (
                      <>
                        <button
                          type="button"
                          data-testid="shortcut-assign"
                          disabled={!p.combo || p.invalid}
                          onClick={() => p.combo && commit(c.id, p.combo, p.taken)}
                        >
                          {p.taken.length ? t('shortcuts.reassign') : t('shortcuts.assign')}
                        </button>{' '}
                        <button type="button" onClick={() => setPending(null)}>
                          {t('shortcuts.cancel')}
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          type="button"
                          data-testid="shortcut-change"
                          aria-label={`${t('shortcuts.change')}: ${c.label}`}
                          onClick={() =>
                            setPending({ id: c.id, combo: null, taken: [], invalid: false })
                          }
                        >
                          {t('shortcuts.change')}
                        </button>{' '}
                        <button
                          type="button"
                          data-testid="shortcut-remove"
                          aria-label={`${t('shortcuts.remove')}: ${c.label}`}
                          disabled={!current}
                          onClick={() => commit(c.id, null, [])}
                        >
                          {t('shortcuts.remove')}
                        </button>{' '}
                        <button
                          type="button"
                          data-testid="shortcut-reset"
                          aria-label={`${t('shortcuts.reset')}: ${c.label}`}
                          disabled={!changed}
                          onClick={() => {
                            const next = { ...value };
                            delete next[c.id];
                            onChange(next);
                          }}
                        >
                          {t('shortcuts.reset')}
                        </button>
                      </>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {notice && (
        <p role="status" data-testid="shortcuts-notice" style={{ fontSize: 12 }}>
          {notice}
        </p>
      )}
    </Dialog>
  );
}

export interface CustomShortcutsOptions<S> {
  app: ShortcutApp;
  registry: CommandRegistry<S>;
  /** Where the user's shortcuts are kept (`platform.shell`). */
  store: SettingStore;
  files: Pick<FileAccess, 'openFile' | 'saveFile'>;
  /** Commands listed by the dialog (labelled). */
  commands: () => readonly ShortcutCommand[];
  /** The dialog is shown (owned by the editor: its `options.shortcuts` command opens it). */
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * The user's shortcuts of an app: loaded and applied to the registry on start, changed through
 * the dialog (saved at once). Returns the dialog element to render; `version`
 * changes whenever the shortcuts do (ribbon tooltips re-read them).
 */
export function useCustomShortcuts<S>({
  app,
  registry,
  store,
  files,
  commands,
  open,
  onOpenChange,
}: CustomShortcutsOptions<S>) {
  const { t } = useTranslation();
  const [map, setMap] = useState<ShortcutMap>({});
  const [notice, setNotice] = useState<string | null>(null);
  const [version, setVersion] = useState(0);
  const storeRef = useRef(store);
  useEffect(() => {
    storeRef.current = store;
  }, [store]);
  // Loaded once per app; applied to every registry the editor builds (it may rebuild it).
  useEffect(() => {
    let live = true;
    void loadShortcuts(storeRef.current, app).then((m) => {
      if (!live) return;
      setMap(m);
      setVersion((v) => v + 1);
    });
    return () => {
      live = false;
    };
  }, [app]);
  useLayoutEffect(() => {
    registry.setShortcutOverrides(map);
  }, [registry, map]);
  const apply = useCallback(
    (m: ShortcutMap) => {
      registry.setShortcutOverrides(m);
      setMap(m);
      setVersion((v) => v + 1);
      void saveShortcuts(storeRef.current, app, m).catch(() => {});
    },
    [registry, app],
  );
  const element = open ? (
    <ShortcutsDialog
      open
      onClose={() => {
        onOpenChange(false);
        setNotice(null);
      }}
      registry={registry}
      commands={commands()}
      value={map}
      onChange={apply}
      notice={notice}
      onExport={() => {
        const bytes = new TextEncoder().encode(exportShortcuts(app, map));
        void files
          .saveFile(bytes, `blueoffice-shortcuts-${app}.json`, 'application/json')
          .then((saved) => saved !== null && setNotice(t('shortcuts.exported')))
          .catch(() => setNotice(t('shortcuts.importInvalid')));
      }}
      onImport={() => {
        void files
          .openFile(['.json'])
          .then((f) => {
            if (!f) return;
            const r = importShortcuts(new TextDecoder().decode(f.bytes), app);
            if (r.ok) {
              apply(r.map);
              setNotice(t('shortcuts.imported', { count: Object.keys(r.map).length }));
            } else
              setNotice(
                r.error === 'wrongApp'
                  ? t('shortcuts.importWrongApp')
                  : t('shortcuts.importInvalid'),
              );
          })
          .catch(() => setNotice(t('shortcuts.importInvalid')));
      }}
    />
  ) : null;
  return { element, version, map };
}
