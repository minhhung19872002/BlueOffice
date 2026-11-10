/**
 * Get & Transform data in BlueSheet (SHEET-PQ-001): Data ▸ Get & Transform (from CSV, JSON,
 * XML, a workbook, a folder, the web), the query editor (preview grid, applied steps, column
 * header menus, Close & Load) and the Queries pane (refresh, refresh all, edit, delete).
 *
 * Files and web pages are read through `Platform` (`files`, `dataSources`); the engine gets
 * their bytes (`querySource`) and does the rest (`bo_engine::sheet::query_cmd`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  Platform,
  QueryAggregateOp,
  QueryColType,
  QueryCondition,
  QueryDef,
  QueryInspectResult,
  QueryJoinKind,
  QueryListEntry,
  QueryPreview,
  QueryRefreshResult,
  QuerySaveResult,
  QuerySource,
  QueryStep,
  QueryStepOp,
  SheetHost,
} from '@blueoffice/platform';
import { Dialog, type TaskPaneDef } from '@blueoffice/ui';
import { ArrowDown, ArrowUp, Database, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import {
  COL_TYPES,
  CONDITIONS,
  SOURCE_ACCEPT,
  baseName,
  columnStep,
  errorText,
  guessFolderKind,
  initialSource,
  insertStep,
  moveStep,
  refineSource,
  removeStep,
  sourceLabel,
  stepLabel,
  toBase64,
  uniqueName,
  type ColumnAction,
  type QuerySourceKind,
} from './queryModel';
import styles from './SheetEditor.module.css';
import q from './queries.module.css';

/** Where Close & Load puts the result. */
export type QueryLoadMode = 'newSheet' | 'existing' | 'none';

interface Draft {
  def: QueryDef;
  inspect: QueryInspectResult;
  /** Name of the saved query being edited. */
  original?: string;
}

/** A source the user must provide again before a refresh. */
interface Reload {
  key: string;
  access: 'file' | 'folder' | 'web';
  /** The engine still has the bytes read earlier. */
  cached: boolean;
}

/** Options of {@link useSheetQueries}. */
export interface SheetQueriesOptions {
  host: SheetHost | null | undefined;
  platform: Platform;
  sheet: number;
  /** Active cell (the "existing sheet" load target). */
  cell: { row: number; col: number };
  /** Redraw after the workbook changed. */
  onChanged: () => Promise<void>;
  /** Show a sheet. */
  goToSheet: (sheet: number) => void;
  fail: (e: unknown) => void;
}

/** Get & Transform state of the editor: ribbon actions, the editor dialog and the Queries pane. */
export function useSheetQueries(o: SheetQueriesOptions) {
  const { t } = useTranslation();
  const { host, platform, fail, onChanged, goToSheet, sheet, cell } = o;
  const [draft, setDraft] = useState<Draft | null>(null);
  const [paneOpen, setPaneOpen] = useState(false);
  const [list, setList] = useState<QueryListEntry[]>([]);
  const [webPrompt, setWebPrompt] = useState(false);
  const [reload, setReload] = useState<{ items: Reload[]; names?: string[] } | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [mashup, setMashup] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!host) return [];
    const r = await host.exec<{ queries: QueryListEntry[] }>({ cmd: 'queryList' });
    setList(r.queries);
    // M formulas of a query written by other software ([MS-QDEFF]), shown read only.
    const m = await host
      .exec<{ present: boolean; formulas?: string }>({ cmd: 'queryMashup' })
      .catch(() => null);
    setMashup(m?.present ? (m.formulas ?? '') : null);
    return r.queries;
  }, [host]);

  const provide = useCallback(
    async (
      key: string,
      access: 'file' | 'folder' | 'web',
      files: { name: string; bytes: Uint8Array }[],
    ) => {
      if (!host) return;
      await host.exec({
        cmd: 'querySource',
        key,
        access,
        files: files.map((f) => ({ name: f.name, data: toBase64(f.bytes) })),
      });
    },
    [host],
  );

  const inspect = useCallback(
    async (source: QuerySource) => {
      if (!host) return {};
      return host.exec<QueryInspectResult>({ cmd: 'queryInspect', source });
    },
    [host],
  );

  /** Open the editor on a new query of `source` (navigator choice and proposed steps). */
  const startEditor = useCallback(
    async (source: QuerySource, name: string) => {
      let info = await inspect(source);
      const refined = refineSource(source, info);
      if (JSON.stringify(refined) !== JSON.stringify(source)) info = await inspect(refined);
      const taken = (await load()).map((x) => x.name);
      setDraft({
        def: { name: uniqueName(name, taken), source: refined, steps: info.suggested ?? [] },
        inspect: info,
      });
    },
    [inspect, load],
  );

  const fetchWeb = useCallback(
    async (url: string) => {
      const fetcher = platform.dataSources?.fetchWeb;
      if (!fetcher) throw new Error('unavailable');
      const bytes = await fetcher(url);
      await provide(url, 'web', [{ name: url, bytes }]);
    },
    [platform, provide],
  );

  const webError = useCallback(
    (e: unknown) => {
      const code = e instanceof Error ? e.message : 'network';
      setMessage(
        t(
          `sheetQuery.web.errors.${['unavailable', 'status', 'tooLarge'].includes(code) ? code : 'network'}`,
        ),
      );
    },
    [t],
  );

  /** Data ▸ Get & Transform ▸ From …. */
  const startFrom = useCallback(
    async (kind: QuerySourceKind) => {
      if (!host) return;
      try {
        if (kind === 'web') {
          if (!platform.dataSources?.fetchWeb) {
            setMessage(t('sheetQuery.web.errors.unavailable'));
            return;
          }
          setWebPrompt(true);
          return;
        }
        if (kind === 'folder') {
          const pick = platform.dataSources?.pickFolder;
          if (!pick) {
            setMessage(t('sheetQuery.folder.unavailable'));
            return;
          }
          const f = await pick();
          if (!f) return;
          const key = f.path ?? f.name;
          await provide(key, 'folder', f.files);
          const fileKind = guessFolderKind(f.files.map((x) => x.name));
          await startEditor(initialSource('folder', key, fileKind), baseName(f.name));
          return;
        }
        const f = await platform.files.openFile(SOURCE_ACCEPT[kind]);
        if (!f) return;
        const key = f.path ?? f.name;
        await provide(key, 'file', [{ name: f.name, bytes: f.bytes }]);
        await startEditor(initialSource(kind, key), baseName(f.name));
      } catch (e) {
        fail(e);
      }
    },
    [host, platform, provide, startEditor, t, fail],
  );

  const onWebUrl = useCallback(
    async (url: string) => {
      setWebPrompt(false);
      setBusy(true);
      try {
        await fetchWeb(url);
        let host_ = url;
        try {
          host_ = new URL(url).hostname;
        } catch {
          // keep the URL
        }
        await startEditor(initialSource('web', url), host_);
      } catch (e) {
        webError(e);
      } finally {
        setBusy(false);
      }
    },
    [fetchWeb, startEditor, webError],
  );

  /** Edit a saved query. */
  const edit = useCallback(
    async (name: string) => {
      if (!host) return;
      try {
        const def = await host.exec<QueryDef>({ cmd: 'queryGet', name });
        const info = await inspect(def.source);
        setDraft({ def, inspect: info, original: def.name });
      } catch (e) {
        fail(e);
      }
    },
    [host, inspect, fail],
  );

  /** Re-read sources the platform can read again silently (desktop paths, web pages). */
  const reread = useCallback(
    async (entries: QueryListEntry[]): Promise<Reload[]> => {
      const ask: Reload[] = [];
      const seen = new Set<string>();
      for (const e of entries) {
        if (!e.key || seen.has(e.key) || e.access === 'query') continue;
        seen.add(e.key);
        try {
          if (e.access === 'web') {
            await fetchWeb(e.key);
            continue;
          }
          const looksLikePath = /^([a-zA-Z]:[\\/]|\/)/.test(e.key);
          if (e.access === 'file' && looksLikePath && platform.files.openPath) {
            const f = await platform.files.openPath(e.key);
            await provide(e.key, 'file', [{ name: f.name, bytes: f.bytes }]);
            continue;
          }
          if (e.access === 'folder' && looksLikePath && platform.dataSources?.readFolder) {
            const f = await platform.dataSources.readFolder(e.key);
            await provide(e.key, 'folder', f.files);
            continue;
          }
        } catch {
          // Fall back to asking (or to the copy read earlier).
        }
        ask.push({ key: e.key, access: e.access, cached: e.available });
      }
      return ask;
    },
    [fetchWeb, platform, provide],
  );

  const runRefresh = useCallback(
    async (names?: string[]) => {
      if (!host) return;
      setBusy(true);
      try {
        const r = await host.exec<QueryRefreshResult>({
          cmd: 'queryRefresh',
          ...(names ? { names } : {}),
        });
        await onChanged();
        await load();
        if (r.errors.length > 0) {
          setMessage(r.errors.map((e) => `${e.name}: ${errorText(e, t)}`).join('\n'));
        } else {
          setMessage(null);
        }
      } catch (e) {
        fail(e);
      } finally {
        setBusy(false);
      }
    },
    [host, load, t, onChanged, fail],
  );

  /** Refresh queries (all when `names` is absent), asking for the sources the browser cannot read again. */
  const refresh = useCallback(
    async (names?: string[]) => {
      if (!host) return;
      try {
        const all = await load();
        const chosen = all.filter(
          (e) => !names || names.some((n) => n.toLowerCase() === e.name.toLowerCase()),
        );
        const ask = await reread(chosen);
        if (ask.length > 0) {
          setReload({ items: ask, ...(names ? { names } : {}) });
          return;
        }
        await runRefresh(names);
      } catch (e) {
        fail(e);
      }
    },
    [host, load, reread, runRefresh, fail],
  );

  /** The reload dialog: pick the file (or folder) again, or use the copy read earlier. */
  const pickAgain = useCallback(
    async (item: Reload) => {
      try {
        if (item.access === 'folder') {
          const f = await platform.dataSources?.pickFolder?.();
          if (!f) return false;
          await provide(item.key, 'folder', f.files);
        } else {
          const ext = item.key.toLowerCase().split('.').pop() ?? '';
          const f = await platform.files.openFile(ext ? [`.${ext}`] : []);
          if (!f) return false;
          await provide(item.key, 'file', [{ name: f.name, bytes: f.bytes }]);
        }
        return true;
      } catch (e) {
        fail(e);
        return false;
      }
    },
    [platform, provide, fail],
  );

  const remove = useCallback(
    async (name: string) => {
      if (!host) return;
      try {
        await host.exec({ cmd: 'queryDelete', name });
        await onChanged();
        await load();
      } catch (e) {
        fail(e);
      }
    },
    [host, load, onChanged, fail],
  );

  const closeAndLoad = useCallback(
    async (def: QueryDef, mode: QueryLoadMode, original?: string) => {
      if (!host) return false;
      const r = await host.exec<QuerySaveResult>({
        cmd: 'querySave',
        query: def,
        load: mode,
        ...(original ? { original } : {}),
        sheet: sheet,
        row: cell.row,
        col: cell.col,
      });
      setDraft(null);
      await onChanged();
      await load();
      setPaneOpen(true);
      if (r.sheet !== null && r.sheet !== undefined) goToSheet(r.sheet);
      return true;
    },
    [host, load, onChanged, goToSheet, sheet, cell.row, cell.col],
  );

  // The pane shows the queries of the open workbook.
  const togglePane = useCallback(() => {
    if (!paneOpen) void load().catch(() => {});
    setPaneOpen(!paneOpen);
  }, [paneOpen, load]);

  const pane: TaskPaneDef | null = paneOpen
    ? {
        id: 'queries',
        title: t('sheetQuery.pane.title'),
        icon: <Database size={14} aria-hidden />,
        content: (
          <QueriesPane
            list={list}
            mashup={mashup}
            busy={busy}
            onRefresh={(n) => void refresh([n])}
            onRefreshAll={() => void refresh()}
            onEdit={(n) => void edit(n)}
            onDelete={(n) => void remove(n)}
            onGo={goToSheet}
          />
        ),
      }
    : null;

  const element = (
    <>
      {draft && host && (
        <QueryEditorDialog
          host={host}
          draft={draft}
          others={list.map((x) => x.name).filter((n) => n !== draft.original)}
          onClose={() => setDraft(null)}
          onLoad={closeAndLoad}
          fail={o.fail}
        />
      )}
      {webPrompt && (
        <WebUrlDialog onClose={() => setWebPrompt(false)} onOk={(u) => void onWebUrl(u)} />
      )}
      {reload && (
        <ReloadDialog
          items={reload.items}
          onPick={pickAgain}
          onClose={() => setReload(null)}
          onDone={() => {
            const names = reload.names;
            setReload(null);
            void runRefresh(names);
          }}
        />
      )}
      {message !== null && (
        <Dialog
          open
          onOpenChange={(v) => !v && setMessage(null)}
          title={t('sheetQuery.title')}
          testId="query-message"
          footer={
            <button type="button" className={styles.btnPrimary} onClick={() => setMessage(null)}>
              {t('sheetEditor.dlg.ok')}
            </button>
          }
        >
          <p className={q.message}>{message}</p>
        </Dialog>
      )}
    </>
  );

  return {
    startFrom,
    refresh,
    edit,
    paneOpen,
    togglePane,
    closePane: () => setPaneOpen(false),
    pane,
    element,
  };
}

// ---------------------------------------------------------------- Queries pane

function QueriesPane({
  list,
  mashup,
  busy,
  onRefresh,
  onRefreshAll,
  onEdit,
  onDelete,
  onGo,
}: {
  list: QueryListEntry[];
  busy: boolean;
  onRefresh: (name: string) => void;
  onRefreshAll: () => void;
  onEdit: (name: string) => void;
  onDelete: (name: string) => void;
  onGo: (sheet: number) => void;
  mashup: string | null;
}) {
  const { t } = useTranslation();
  return (
    <div className={q.pane} data-testid="queries-pane">
      <div className={q.paneBar}>
        <button
          type="button"
          className={styles.btn}
          data-testid="queries-refresh-all"
          disabled={busy || list.length === 0}
          onClick={onRefreshAll}
        >
          <RefreshCw size={14} aria-hidden /> {t('sheetQuery.pane.refreshAll')}
        </button>
      </div>
      {list.length === 0 && <p className={q.empty}>{t('sheetQuery.pane.empty')}</p>}
      <ul className={q.items}>
        {list.map((e, i) => (
          <li key={e.name} className={q.item} data-testid={`query-item-${i}`}>
            <button
              type="button"
              className={q.itemName}
              disabled={e.sheet === null}
              onClick={() => e.sheet !== null && onGo(e.sheet)}
            >
              {e.name}
            </button>
            <span className={q.itemInfo} data-testid={`query-info-${i}`}>
              {e.table
                ? t('sheetQuery.pane.loaded', { count: e.rows ?? 0, sheet: e.sheetName ?? '' })
                : t('sheetQuery.pane.connectionOnly')}
              {' · '}
              {t(`sheetQuery.kinds.${e.kind}`)}
            </span>
            <span className={q.itemActions}>
              <button
                type="button"
                className={q.icon}
                title={t('sheetQuery.pane.refresh')}
                aria-label={t('sheetQuery.pane.refresh')}
                data-testid={`query-refresh-${i}`}
                disabled={busy || !e.table}
                onClick={() => onRefresh(e.name)}
              >
                <RefreshCw size={14} aria-hidden />
              </button>
              <button
                type="button"
                className={q.icon}
                title={t('sheetQuery.pane.edit')}
                aria-label={t('sheetQuery.pane.edit')}
                data-testid={`query-edit-${i}`}
                onClick={() => onEdit(e.name)}
              >
                <Pencil size={14} aria-hidden />
              </button>
              <button
                type="button"
                className={q.icon}
                title={t('sheetQuery.pane.delete')}
                aria-label={t('sheetQuery.pane.delete')}
                data-testid={`query-delete-${i}`}
                onClick={() => onDelete(e.name)}
              >
                <Trash2 size={14} aria-hidden />
              </button>
            </span>
          </li>
        ))}
      </ul>
      {mashup !== null && (
        <details className={q.mashup} data-testid="query-mashup">
          <summary>{t('sheetQuery.pane.foreign')}</summary>
          <p className={q.hint}>{t('sheetQuery.pane.foreignHint')}</p>
          <pre>{mashup}</pre>
        </details>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- Small dialogs

function WebUrlDialog({ onClose, onOk }: { onClose: () => void; onOk: (url: string) => void }) {
  const { t } = useTranslation();
  const [url, setUrl] = useState('https://');
  const valid = /^https?:\/\/[^\s/]+/i.test(url.trim());
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('sheetQuery.web.title')}
      width={460}
      testId="query-web-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="query-web-ok"
            disabled={!valid}
            onClick={() => onOk(url.trim())}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetQuery.web.url')}
          <input
            data-testid="query-web-url"
            value={url}
            autoFocus
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && valid && onOk(url.trim())}
          />
        </label>
        <p className={q.hint}>{t('sheetQuery.web.hint')}</p>
      </div>
    </Dialog>
  );
}

function ReloadDialog({
  items,
  onPick,
  onClose,
  onDone,
}: {
  items: Reload[];
  onPick: (item: Reload) => Promise<boolean>;
  onClose: () => void;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  const [ready, setReady] = useState<Set<string>>(new Set());
  const allReady = items.every((i) => ready.has(i.key) || i.cached);
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('sheetQuery.reload.title')}
      width={480}
      testId="query-reload"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="query-reload-ok"
            disabled={!allReady}
            onClick={onDone}
          >
            {t('sheetQuery.reload.refresh')}
          </button>
        </>
      }
    >
      <p className={q.hint}>{t('sheetQuery.reload.hint')}</p>
      <ul className={q.items}>
        {items.map((it, i) => (
          <li key={it.key} className={q.item}>
            <span className={q.itemName}>{it.key}</span>
            <span className={q.itemInfo}>
              {ready.has(it.key)
                ? t('sheetQuery.reload.picked')
                : it.cached
                  ? t('sheetQuery.reload.cached')
                  : t('sheetQuery.reload.needed')}
            </span>
            <button
              type="button"
              className={styles.btn}
              data-testid={`query-reload-pick-${i}`}
              onClick={() =>
                void onPick(it).then((ok) => ok && setReady((s) => new Set(s).add(it.key)))
              }
            >
              {it.access === 'folder'
                ? t('sheetQuery.reload.pickFolder')
                : t('sheetQuery.reload.pickFile')}
            </button>
          </li>
        ))}
      </ul>
    </Dialog>
  );
}

// ---------------------------------------------------------------- Query editor

/** Step kinds of the "add step" menu. */
const ADD_STEPS: QueryStepOp[] = [
  'promoteHeaders',
  'demoteHeaders',
  'changeType',
  'filterRows',
  'sort',
  'removeColumns',
  'keepColumns',
  'renameColumns',
  'reorderColumns',
  'splitColumn',
  'replaceValues',
  'groupBy',
  'addColumn',
  'removeDuplicates',
  'removeBlankRows',
  'keepTop',
  'removeTop',
  'unpivot',
  'merge',
  'append',
];

/** Steps that need no settings. */
const NO_FORM: QueryStepOp[] = ['promoteHeaders', 'demoteHeaders', 'removeBlankRows'];

const AGGREGATES: QueryAggregateOp[] = [
  'sum',
  'average',
  'min',
  'max',
  'count',
  'countDistinct',
  'median',
];
const JOINS: QueryJoinKind[] = [
  'leftOuter',
  'rightOuter',
  'fullOuter',
  'inner',
  'leftAnti',
  'rightAnti',
];

function QueryEditorDialog({
  host,
  draft,
  others,
  onClose,
  onLoad,
  fail,
}: {
  host: SheetHost;
  draft: Draft;
  others: string[];
  onClose: () => void;
  onLoad: (def: QueryDef, mode: QueryLoadMode, original?: string) => Promise<boolean>;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [def, setDef] = useState<QueryDef>(draft.def);
  const [info, setInfo] = useState<QueryInspectResult>(draft.inspect);
  /** Selected step (-1: the source). */
  const [selected, setSelected] = useState(draft.def.steps.length - 1);
  const [preview, setPreview] = useState<QueryPreview | null>(null);
  const [colMenu, setColMenu] = useState<{ col: number; x: number; y: number } | null>(null);
  const [form, setForm] = useState<{ op: QueryStepOp; index?: number; column?: string } | null>(
    null,
  );
  const [addOpen, setAddOpen] = useState(false);
  const [mode, setMode] = useState<QueryLoadMode>(draft.original ? 'newSheet' : 'newSheet');
  const [error, setError] = useState('');
  const seq = useRef(0);
  const stepsTouched = useRef(false);

  // Preview of the table after the selected step.
  useEffect(() => {
    const n = ++seq.current;
    const timer = setTimeout(() => {
      host
        .exec<QueryPreview>({ cmd: 'queryPreview', query: def, upto: selected + 1, limit: 100 })
        .then((p) => n === seq.current && setPreview(p))
        .catch((e: unknown) => n === seq.current && fail(e));
    }, 60);
    return () => clearTimeout(timer);
  }, [host, def, selected, fail]);

  const columns = preview?.columns ?? [];

  const addStep = (step: QueryStep) => {
    stepsTouched.current = true;
    const r = insertStep(def.steps, selected, step);
    setDef({ ...def, steps: r.steps });
    setSelected(r.index);
  };

  const replaceStep = (i: number, step: QueryStep) => {
    stepsTouched.current = true;
    const steps = [...def.steps];
    steps[i] = step;
    setDef({ ...def, steps });
    setSelected(i);
  };

  const setSource = async (source: QuerySource) => {
    try {
      const next = await host.exec<QueryInspectResult>({ cmd: 'queryInspect', source });
      setInfo((old) => ({ ...old, ...next }));
      // A new query follows its source's proposals until the user edits the steps.
      if (!draft.original && !stepsTouched.current) {
        const steps = next.suggested ?? [];
        setDef({ ...def, source, steps });
        setSelected(steps.length - 1);
      } else {
        setDef({ ...def, source });
      }
    } catch (e) {
      fail(e);
    }
  };

  const onColumnAction = (
    action: ColumnAction | 'filter' | 'rename' | 'split' | 'replace' | 'groupBy',
  ) => {
    const col = colMenu ? columns[colMenu.col] : undefined;
    setColMenu(null);
    if (col === undefined) return;
    switch (action) {
      case 'filter':
        setForm({ op: 'filterRows', column: col });
        return;
      case 'rename':
        setForm({ op: 'renameColumns', column: col });
        return;
      case 'split':
        setForm({ op: 'splitColumn', column: col });
        return;
      case 'replace':
        setForm({ op: 'replaceValues', column: col });
        return;
      case 'groupBy':
        setForm({ op: 'groupBy', column: col });
        return;
      default:
        addStep(columnStep(action, col, columns));
    }
  };

  const save = async () => {
    const name = def.name.trim();
    if (!name) {
      setError(t('sheetQuery.editor.nameRequired'));
      return;
    }
    if (others.some((n) => n.toLowerCase() === name.toLowerCase())) {
      setError(t('sheetQuery.editor.nameTaken'));
      return;
    }
    try {
      await onLoad({ ...def, name }, mode, draft.original);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg.replace(/^.*?query: /, ''));
    }
  };

  const fmt = (v: unknown) => (v === null || v === undefined ? '' : String(v));

  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={draft.original ? t('sheetQuery.editor.editTitle') : t('sheetQuery.editor.title')}
      width={980}
      testId="query-editor"
      footer={
        <>
          <fieldset className={q.loadModes}>
            <legend>{t('sheetQuery.editor.loadTo')}</legend>
            {(['newSheet', 'existing', 'none'] as QueryLoadMode[]).map((m) => (
              <label key={m} className={styles.check}>
                <input
                  type="radio"
                  name="query-load"
                  data-testid={`query-load-mode-${m}`}
                  checked={mode === m}
                  onChange={() => setMode(m)}
                />
                {t(`sheetQuery.editor.load.${m}`)}
              </label>
            ))}
          </fieldset>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="query-load"
            onClick={() => void save()}
          >
            {t('sheetQuery.editor.closeAndLoad')}
          </button>
        </>
      }
    >
      <div className={q.editor}>
        <div className={q.top}>
          <label className={q.nameField}>
            {t('sheetQuery.editor.name')}
            <input
              data-testid="query-name"
              value={def.name}
              onChange={(e) => setDef({ ...def, name: e.target.value })}
            />
          </label>
          <SourceOptions
            source={def.source}
            info={info}
            others={others}
            onChange={(s) => void setSource(s)}
          />
        </div>
        {error && (
          <p className={q.error} role="alert" data-testid="query-editor-error">
            {error}
          </p>
        )}
        <div className={q.body}>
          <div className={q.steps}>
            <div className={q.stepsHead}>
              <span>{t('sheetQuery.editor.steps')}</span>
              <span className={q.addWrap}>
                <button
                  type="button"
                  className={q.icon}
                  aria-label={t('sheetQuery.editor.addStep')}
                  title={t('sheetQuery.editor.addStep')}
                  data-testid="query-add-step"
                  onClick={() => setAddOpen((v) => !v)}
                >
                  <Plus size={14} aria-hidden />
                </button>
                {addOpen && (
                  <ul className={q.menu} role="menu" data-testid="query-add-menu">
                    {ADD_STEPS.filter((op) =>
                      op === 'merge' || op === 'append' ? others.length > 0 : true,
                    ).map((op) => (
                      <li key={op}>
                        <button
                          type="button"
                          role="menuitem"
                          data-testid={`query-add-${op}`}
                          onClick={() => {
                            setAddOpen(false);
                            if (NO_FORM.includes(op)) addStep({ op } as QueryStep);
                            else setForm({ op });
                          }}
                        >
                          {t(`sheetQuery.steps.${op}`)}
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </span>
            </div>
            <ol className={q.stepList} data-testid="query-steps">
              <li className={selected === -1 ? q.stepOn : q.step}>
                <button
                  type="button"
                  className={q.stepText}
                  data-testid="query-step-source"
                  onClick={() => setSelected(-1)}
                >
                  {sourceLabel(def.source, t)}
                </button>
              </li>
              {def.steps.map((s, i) => (
                <li
                  key={i}
                  className={`${selected === i ? q.stepOn : q.step} ${preview?.error?.step === i ? q.stepBad : ''}`}
                  data-testid={`query-step-${i}`}
                >
                  <button
                    type="button"
                    className={q.stepText}
                    title={stepLabel(s, t)}
                    onClick={() => setSelected(i)}
                  >
                    {stepLabel(s, t)}
                  </button>
                  <span className={q.stepActions}>
                    {!NO_FORM.includes(s.op) && (
                      <button
                        type="button"
                        className={q.icon}
                        aria-label={t('sheetQuery.editor.editStep')}
                        data-testid={`query-step-edit-${i}`}
                        onClick={() => {
                          // The form offers the columns entering the step.
                          setSelected(i - 1);
                          setForm({ op: s.op, index: i });
                        }}
                      >
                        <Pencil size={12} aria-hidden />
                      </button>
                    )}
                    <button
                      type="button"
                      className={q.icon}
                      aria-label={t('sheetQuery.editor.moveUp')}
                      data-testid={`query-step-up-${i}`}
                      disabled={i === 0}
                      onClick={() => {
                        stepsTouched.current = true;
                        setDef({ ...def, steps: moveStep(def.steps, i, -1) });
                        setSelected(i - 1);
                      }}
                    >
                      <ArrowUp size={12} aria-hidden />
                    </button>
                    <button
                      type="button"
                      className={q.icon}
                      aria-label={t('sheetQuery.editor.moveDown')}
                      data-testid={`query-step-down-${i}`}
                      disabled={i === def.steps.length - 1}
                      onClick={() => {
                        stepsTouched.current = true;
                        setDef({ ...def, steps: moveStep(def.steps, i, 1) });
                        setSelected(i + 1);
                      }}
                    >
                      <ArrowDown size={12} aria-hidden />
                    </button>
                    <button
                      type="button"
                      className={q.icon}
                      aria-label={t('sheetQuery.editor.deleteStep')}
                      data-testid={`query-step-delete-${i}`}
                      onClick={() => {
                        stepsTouched.current = true;
                        setDef({ ...def, steps: removeStep(def.steps, i) });
                        setSelected(Math.min(selected, def.steps.length - 2));
                      }}
                    >
                      <Trash2 size={12} aria-hidden />
                    </button>
                  </span>
                </li>
              ))}
            </ol>
          </div>
          <div className={q.previewWrap}>
            {preview?.error && (
              <p className={q.error} role="alert" data-testid="query-error">
                {errorText(preview.error, t)}
              </p>
            )}
            <div className={q.gridScroll}>
              <table className={q.grid} data-testid="query-preview">
                <thead>
                  <tr>
                    {columns.map((c, i) => (
                      <th key={`${i}-${c}`}>
                        <button
                          type="button"
                          className={q.colHead}
                          data-testid={`query-col-${i}`}
                          aria-haspopup="menu"
                          onClick={(e) => {
                            const r = e.currentTarget.getBoundingClientRect();
                            setColMenu({ col: i, x: r.left, y: r.bottom });
                          }}
                        >
                          <span className={q.colType}>
                            {t(`sheetQuery.typeBadges.${preview?.types[i] ?? 'any'}`)}
                          </span>
                          {c}
                        </button>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {(preview?.rows ?? []).map((r, ri) => (
                    <tr key={ri}>
                      {r.map((v, ci) => (
                        <td
                          key={ci}
                          className={v === null ? q.null : typeof v === 'number' ? q.num : ''}
                        >
                          {fmt(v)}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className={q.status} data-testid="query-row-count">
              {t('sheetQuery.editor.rows', {
                count: preview?.rowCount ?? 0,
                columns: columns.length,
              })}
            </p>
          </div>
        </div>
      </div>
      {colMenu && (
        <ColumnMenu
          x={colMenu.x}
          y={colMenu.y}
          onAction={onColumnAction}
          onClose={() => setColMenu(null)}
        />
      )}
      {form && (
        <StepForm
          op={form.op}
          initial={form.index !== undefined ? def.steps[form.index] : undefined}
          column={form.column}
          columns={columns}
          others={others}
          onClose={() => setForm(null)}
          onOk={(step) => {
            if (form.index !== undefined) replaceStep(form.index, step);
            else addStep(step);
            setForm(null);
          }}
        />
      )}
    </Dialog>
  );
}

/** The source's options (navigator choices). */
function SourceOptions({
  source,
  info,
  others,
  onChange,
}: {
  source: QuerySource;
  info: QueryInspectResult;
  others: string[];
  onChange: (s: QuerySource) => void;
}) {
  const { t } = useTranslation();
  const field = (label: string, control: ReactNode) => (
    <label className={q.sourceField}>
      {label}
      {control}
    </label>
  );
  switch (source.kind) {
    case 'csv':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.delimiter'),
            <select
              data-testid="query-source-delimiter"
              value={source.delimiter ?? info.csv?.delimiter ?? ','}
              onChange={(e) => onChange({ ...source, delimiter: e.target.value })}
            >
              {[',', ';', '\t', '|', ' '].map((d) => (
                <option key={d} value={d}>
                  {t(
                    `sheetQuery.delimiters.${{ ',': 'comma', ';': 'semicolon', '\t': 'tab', '|': 'pipe', ' ': 'space' }[d]}`,
                  )}
                </option>
              ))}
            </select>,
          )}
          {field(
            t('sheetQuery.source.encoding'),
            <select
              data-testid="query-source-encoding"
              value={source.encoding ?? info.csv?.encoding ?? 'utf-8'}
              onChange={(e) => onChange({ ...source, encoding: e.target.value })}
            >
              {['utf-8', 'utf-16le', 'utf-16be', 'windows-1252', 'tcvn3', 'vni'].map((enc) => (
                <option key={enc} value={enc}>
                  {t(`sheetQuery.encodings.${enc}`)}
                </option>
              ))}
            </select>,
          )}
        </div>
      );
    case 'json':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.jsonPath'),
            <select
              data-testid="query-source-path"
              value={source.path ?? '$'}
              onChange={(e) => onChange({ ...source, path: e.target.value })}
            >
              {(info.jsonPaths?.length ? info.jsonPaths : ['$']).map((p) => (
                <option key={p} value={p}>
                  {p}
                </option>
              ))}
            </select>,
          )}
        </div>
      );
    case 'xml':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.element'),
            <select
              data-testid="query-source-element"
              value={source.element ?? ''}
              onChange={(e) => onChange({ ...source, element: e.target.value })}
            >
              {(info.xmlElements ?? []).map((el) => (
                <option key={el.name} value={el.name}>
                  {`${el.name} (${el.count})`}
                </option>
              ))}
            </select>,
          )}
        </div>
      );
    case 'workbook':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.item'),
            <select
              data-testid="query-source-item"
              value={`${source.itemKind ?? 'sheet'}:${source.item}`}
              onChange={(e) => {
                const [kind, ...rest] = e.target.value.split(':');
                onChange({ ...source, item: rest.join(':'), itemKind: kind as 'sheet' | 'table' });
              }}
            >
              {(info.items ?? []).map((it) => (
                <option key={`${it.kind}:${it.name}`} value={`${it.kind}:${it.name}`}>
                  {t(`sheetQuery.source.${it.kind}`, { name: it.name })}
                </option>
              ))}
            </select>,
          )}
        </div>
      );
    case 'web':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.webTableLabel'),
            <select
              data-testid="query-source-table"
              value={source.table ?? 0}
              onChange={(e) => onChange({ ...source, table: Number(e.target.value) })}
            >
              {(info.tables ?? []).map((tb) => (
                <option key={tb.index} value={tb.index}>
                  {t('sheetQuery.source.webTable', {
                    n: tb.index + 1,
                    caption: tb.caption || (tb.preview[0] ?? []).slice(0, 3).join(' · '),
                    rows: tb.rows,
                    cols: tb.cols,
                  })}
                </option>
              ))}
            </select>,
          )}
        </div>
      );
    case 'folder':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.fileKind'),
            <select
              data-testid="query-source-filekind"
              value={source.fileKind}
              onChange={(e) =>
                onChange({
                  ...source,
                  fileKind: e.target.value as 'csv' | 'json' | 'xml' | 'workbook',
                })
              }
            >
              {(['csv', 'json', 'xml', 'workbook'] as const).map((k) => (
                <option key={k} value={k}>
                  {t(`sheetQuery.kinds.${k}`)}
                </option>
              ))}
            </select>,
          )}
          <span className={q.hint}>
            {t('sheetQuery.source.files', { count: info.files?.length ?? 0 })}
          </span>
        </div>
      );
    case 'query':
      return (
        <div className={q.source} data-testid="query-source">
          {field(
            t('sheetQuery.source.query'),
            <select
              value={source.query}
              onChange={(e) => onChange({ kind: 'query', query: e.target.value })}
            >
              {others.map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>,
          )}
        </div>
      );
  }
}

function ColumnMenu({
  x,
  y,
  onAction,
  onClose,
}: {
  x: number;
  y: number;
  onAction: (a: ColumnAction | 'filter' | 'rename' | 'split' | 'replace' | 'groupBy') => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const ref = useRef<HTMLUListElement>(null);
  useEffect(() => {
    ref.current?.querySelector('button')?.focus();
    const down = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    document.addEventListener('mousedown', down);
    document.addEventListener('keydown', key);
    return () => {
      document.removeEventListener('mousedown', down);
      document.removeEventListener('keydown', key);
    };
  }, [onClose]);
  const item = (
    a: ColumnAction | 'filter' | 'rename' | 'split' | 'replace' | 'groupBy',
    label: string,
  ) => (
    <li key={a}>
      <button
        type="button"
        role="menuitem"
        data-testid={`query-colmenu-${a.replace(':', '-')}`}
        onClick={() => onAction(a)}
      >
        {label}
      </button>
    </li>
  );
  return (
    <ul
      ref={ref}
      className={q.colMenu}
      role="menu"
      data-testid="query-colmenu"
      style={{ left: x, top: y }}
    >
      {item('sortAsc', t('sheetQuery.col.sortAsc'))}
      {item('sortDesc', t('sheetQuery.col.sortDesc'))}
      {item('filter', t('sheetQuery.col.filter'))}
      {item('removeBlank', t('sheetQuery.col.removeBlank'))}
      <li className={q.sep} role="separator" />
      {COL_TYPES.map((ty) =>
        item(`type:${ty}`, t('sheetQuery.col.type', { type: t(`sheetQuery.types.${ty}`) })),
      )}
      <li className={q.sep} role="separator" />
      {item('rename', t('sheetQuery.col.rename'))}
      {item('remove', t('sheetQuery.col.remove'))}
      {item('removeOthers', t('sheetQuery.col.removeOthers'))}
      {item('split', t('sheetQuery.col.split'))}
      {item('replace', t('sheetQuery.col.replace'))}
      {item('removeDuplicates', t('sheetQuery.col.removeDuplicates'))}
      {item('groupBy', t('sheetQuery.col.groupBy'))}
      {item('unpivot', t('sheetQuery.col.unpivot'))}
      {item('unpivotOthers', t('sheetQuery.col.unpivotOthers'))}
    </ul>
  );
}

/** Settings of one step (new or edited). */
export function StepForm({
  op,
  initial,
  column,
  columns,
  others,
  onClose,
  onOk,
}: {
  op: QueryStepOp;
  initial?: QueryStep | undefined;
  column?: string | undefined;
  columns: string[];
  others: string[];
  onClose: () => void;
  onOk: (step: QueryStep) => void;
}) {
  const { t } = useTranslation();
  const first = column ?? columns[0] ?? '';
  const init = (initial ?? {}) as Record<string, unknown>;
  const str = (k: string, d = '') => (typeof init[k] === 'string' ? (init[k] as string) : d);
  const [col, setCol] = useState(str('column', first) || first);
  const [cond, setCond] = useState<QueryCondition>((init.condition as QueryCondition) ?? 'equals');
  const [value, setValue] = useState(str('value'));
  const [ty, setTy] = useState<QueryColType>(
    initial?.op === 'changeType' ? (initial.columns[0]?.type ?? 'text') : 'text',
  );
  const [picked, setPicked] = useState<Set<string>>(() => {
    if (initial && 'columns' in initial && Array.isArray(initial.columns)) {
      return new Set(
        (initial.columns as unknown[]).filter((c): c is string => typeof c === 'string'),
      );
    }
    if (initial?.op === 'groupBy') return new Set(initial.keys);
    return new Set(column ? [column] : []);
  });
  const [to, setTo] = useState(
    initial?.op === 'renameColumns' ? (initial.renames[0]?.to ?? '') : (column ?? ''),
  );
  const [from, setFrom] = useState(
    initial?.op === 'renameColumns' ? (initial.renames[0]?.from ?? first) : first,
  );
  const [delimiter, setDelimiter] = useState(str('delimiter', ','));
  const [find, setFind] = useState(str('find'));
  const [replace, setReplace] = useState(str('replace'));
  const [entire, setEntire] = useState(init.matchEntire === true);
  const [aggOp, setAggOp] = useState<QueryAggregateOp>(
    initial?.op === 'groupBy' ? (initial.aggregates[0]?.op ?? 'count') : 'count',
  );
  const [aggCol, setAggCol] = useState(
    initial?.op === 'groupBy' ? (initial.aggregates[0]?.column ?? first) : first,
  );
  const [aggName, setAggName] = useState(
    initial?.op === 'groupBy'
      ? (initial.aggregates[0]?.name ?? '')
      : t('sheetQuery.form.countName'),
  );
  const [desc, setDesc] = useState(initial?.op === 'sort' ? !!initial.keys[0]?.descending : false);
  const [count, setCount] = useState(typeof init.count === 'number' ? (init.count as number) : 10);
  const [name, setName] = useState(str('name', t('sheetQuery.form.newColumn')));
  const [formula, setFormula] = useState(str('formula', '='));
  const [other, setOther] = useState(str('query', others[0] ?? ''));
  const [leftKey, setLeftKey] = useState(
    initial?.op === 'merge' ? (initial.leftKeys[0] ?? first) : first,
  );
  const [rightKey, setRightKey] = useState(
    initial?.op === 'merge' ? (initial.rightKeys[0] ?? '') : '',
  );
  const [rightCols, setRightCols] = useState(
    initial?.op === 'merge' ? (initial.columns ?? []).join(', ') : '',
  );
  const [join, setJoin] = useState<QueryJoinKind>(
    initial?.op === 'merge' ? (initial.kind ?? 'leftOuter') : 'leftOuter',
  );
  const [unpivotOther, setUnpivotOther] = useState(
    initial?.op === 'unpivot' ? !!initial.other : false,
  );
  const [error, setError] = useState('');

  const colSelect = (v: string, set: (s: string) => void, testId: string) => (
    <select data-testid={testId} value={v} onChange={(e) => set(e.target.value)}>
      {columns.map((c) => (
        <option key={c} value={c}>
          {c}
        </option>
      ))}
    </select>
  );
  const row = (label: string, control: ReactNode) => (
    <label>
      {label}
      {control}
    </label>
  );
  const checks = (
    <div className={q.checks} data-testid="query-form-columns">
      {columns.map((c) => (
        <label key={c} className={styles.check}>
          <input
            type="checkbox"
            checked={picked.has(c)}
            onChange={(e) =>
              setPicked((s) => {
                const n = new Set(s);
                if (e.target.checked) n.add(c);
                else n.delete(c);
                return n;
              })
            }
          />
          {c}
        </label>
      ))}
    </div>
  );
  const chosen = () => columns.filter((c) => picked.has(c));

  const build = (): QueryStep | string => {
    switch (op) {
      case 'filterRows':
        return { op, column: col, condition: cond, value };
      case 'changeType':
        return { op, columns: [{ column: col, type: ty }] };
      case 'removeColumns':
      case 'keepColumns':
      case 'reorderColumns':
        return chosen().length ? { op, columns: chosen() } : t('sheetQuery.form.pickColumns');
      case 'removeDuplicates':
        return { op, columns: chosen() };
      case 'renameColumns':
        return to.trim()
          ? { op, renames: [{ from, to: to.trim() }] }
          : t('sheetQuery.form.nameRequired');
      case 'splitColumn':
        return delimiter ? { op, column: col, delimiter } : t('sheetQuery.form.delimiterRequired');
      case 'replaceValues':
        return { op, columns: [col], find, replace, matchEntire: entire };
      case 'groupBy':
        return chosen().length && aggName.trim()
          ? {
              op,
              keys: chosen(),
              aggregates: [
                {
                  name: aggName.trim(),
                  op: aggOp,
                  ...(aggOp === 'count' ? {} : { column: aggCol }),
                },
              ],
            }
          : t('sheetQuery.form.pickColumns');
      case 'sort':
        return { op, keys: [{ column: col, descending: desc }] };
      case 'keepTop':
      case 'removeTop':
        return count >= 0 ? { op, count } : t('sheetQuery.form.countRequired');
      case 'addColumn':
        return name.trim() && formula.trim().length > 1
          ? { op, name: name.trim(), formula: formula.trim() }
          : t('sheetQuery.form.formulaRequired');
      case 'merge':
        return other && rightKey.trim()
          ? {
              op,
              query: other,
              leftKeys: [leftKey],
              rightKeys: [rightKey.trim()],
              kind: join,
              columns: rightCols
                .split(',')
                .map((s) => s.trim())
                .filter(Boolean),
            }
          : t('sheetQuery.form.keyRequired');
      case 'append':
        return other ? { op, queries: [other] } : t('sheetQuery.form.queryRequired');
      case 'unpivot':
        return chosen().length
          ? { op, columns: chosen(), other: unpivotOther }
          : t('sheetQuery.form.pickColumns');
      default:
        return { op } as QueryStep;
    }
  };

  const fields = (): ReactNode => {
    switch (op) {
      case 'filterRows':
        return (
          <>
            {row(t('sheetQuery.form.column'), colSelect(col, setCol, 'query-form-column'))}
            {row(
              t('sheetQuery.form.condition'),
              <select
                data-testid="query-form-condition"
                value={cond}
                onChange={(e) => setCond(e.target.value as QueryCondition)}
              >
                {CONDITIONS.map((c) => (
                  <option key={c} value={c}>
                    {t(`sheetQuery.conditions.${c}`)}
                  </option>
                ))}
              </select>,
            )}
            {cond !== 'isBlank' &&
              cond !== 'isNotBlank' &&
              row(
                t('sheetQuery.form.value'),
                <input
                  data-testid="query-form-value"
                  value={value}
                  onChange={(e) => setValue(e.target.value)}
                />,
              )}
          </>
        );
      case 'changeType':
        return (
          <>
            {row(t('sheetQuery.form.column'), colSelect(col, setCol, 'query-form-column'))}
            {row(
              t('sheetQuery.form.type'),
              <select
                data-testid="query-form-type"
                value={ty}
                onChange={(e) => setTy(e.target.value as QueryColType)}
              >
                {COL_TYPES.map((c) => (
                  <option key={c} value={c}>
                    {t(`sheetQuery.types.${c}`)}
                  </option>
                ))}
              </select>,
            )}
          </>
        );
      case 'removeColumns':
      case 'keepColumns':
      case 'reorderColumns':
      case 'removeDuplicates':
        return checks;
      case 'renameColumns':
        return (
          <>
            {row(t('sheetQuery.form.column'), colSelect(from, setFrom, 'query-form-column'))}
            {row(
              t('sheetQuery.form.newName'),
              <input
                data-testid="query-form-name"
                value={to}
                autoFocus
                onChange={(e) => setTo(e.target.value)}
              />,
            )}
          </>
        );
      case 'splitColumn':
        return (
          <>
            {row(t('sheetQuery.form.column'), colSelect(col, setCol, 'query-form-column'))}
            {row(
              t('sheetQuery.form.delimiter'),
              <input
                data-testid="query-form-delimiter"
                value={delimiter}
                onChange={(e) => setDelimiter(e.target.value)}
              />,
            )}
          </>
        );
      case 'replaceValues':
        return (
          <>
            {row(t('sheetQuery.form.column'), colSelect(col, setCol, 'query-form-column'))}
            {row(
              t('sheetQuery.form.find'),
              <input
                data-testid="query-form-find"
                value={find}
                onChange={(e) => setFind(e.target.value)}
              />,
            )}
            {row(
              t('sheetQuery.form.replace'),
              <input
                data-testid="query-form-replace"
                value={replace}
                onChange={(e) => setReplace(e.target.value)}
              />,
            )}
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={entire}
                onChange={(e) => setEntire(e.target.checked)}
              />
              {t('sheetQuery.form.matchEntire')}
            </label>
          </>
        );
      case 'groupBy':
        return (
          <>
            <span>{t('sheetQuery.form.groupKeys')}</span>
            {checks}
            {row(
              t('sheetQuery.form.aggregate'),
              <select
                data-testid="query-form-aggregate"
                value={aggOp}
                onChange={(e) => {
                  const v = e.target.value as QueryAggregateOp;
                  setAggOp(v);
                  setAggName(
                    v === 'count'
                      ? t('sheetQuery.form.countName')
                      : `${t(`sheetQuery.aggregates.${v}`)} ${aggCol}`,
                  );
                }}
              >
                {AGGREGATES.map((a) => (
                  <option key={a} value={a}>
                    {t(`sheetQuery.aggregates.${a}`)}
                  </option>
                ))}
              </select>,
            )}
            {aggOp !== 'count' &&
              row(
                t('sheetQuery.form.column'),
                colSelect(aggCol, setAggCol, 'query-form-aggcolumn'),
              )}
            {row(
              t('sheetQuery.form.newName'),
              <input
                data-testid="query-form-name"
                value={aggName}
                onChange={(e) => setAggName(e.target.value)}
              />,
            )}
          </>
        );
      case 'sort':
        return (
          <>
            {row(t('sheetQuery.form.column'), colSelect(col, setCol, 'query-form-column'))}
            <label className={styles.check}>
              <input type="checkbox" checked={desc} onChange={(e) => setDesc(e.target.checked)} />
              {t('sheetQuery.form.descending')}
            </label>
          </>
        );
      case 'keepTop':
      case 'removeTop':
        return row(
          t('sheetQuery.form.count'),
          <input
            type="number"
            min={0}
            data-testid="query-form-count"
            value={count}
            onChange={(e) => setCount(Math.max(0, Math.floor(Number(e.target.value) || 0)))}
          />,
        );
      case 'addColumn':
        return (
          <>
            {row(
              t('sheetQuery.form.newName'),
              <input
                data-testid="query-form-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />,
            )}
            {row(
              t('sheetQuery.form.formula'),
              <input
                data-testid="query-form-formula"
                value={formula}
                spellCheck={false}
                onChange={(e) => setFormula(e.target.value)}
              />,
            )}
            <p className={q.hint}>{t('sheetQuery.form.formulaHint')}</p>
            <div className={q.chips}>
              {columns.map((c) => (
                <button
                  key={c}
                  type="button"
                  className={q.chip}
                  onClick={() => setFormula((f) => `${f}[@[${c.replace(/[[\]#']/g, "'$&")}]]`)}
                >
                  {c}
                </button>
              ))}
            </div>
          </>
        );
      case 'merge':
      case 'append':
        return (
          <>
            {row(
              t('sheetQuery.form.otherQuery'),
              <select
                data-testid="query-form-query"
                value={other}
                onChange={(e) => setOther(e.target.value)}
              >
                {others.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>,
            )}
            {op === 'merge' && (
              <>
                {row(
                  t('sheetQuery.form.leftKey'),
                  colSelect(leftKey, setLeftKey, 'query-form-leftkey'),
                )}
                {row(
                  t('sheetQuery.form.rightKey'),
                  <input
                    data-testid="query-form-rightkey"
                    value={rightKey}
                    onChange={(e) => setRightKey(e.target.value)}
                  />,
                )}
                {row(
                  t('sheetQuery.form.join'),
                  <select
                    data-testid="query-form-join"
                    value={join}
                    onChange={(e) => setJoin(e.target.value as QueryJoinKind)}
                  >
                    {JOINS.map((j) => (
                      <option key={j} value={j}>
                        {t(`sheetQuery.joins.${j}`)}
                      </option>
                    ))}
                  </select>,
                )}
                {row(
                  t('sheetQuery.form.rightColumns'),
                  <input
                    data-testid="query-form-rightcolumns"
                    value={rightCols}
                    placeholder={t('sheetQuery.form.allColumnsHint')}
                    onChange={(e) => setRightCols(e.target.value)}
                  />,
                )}
              </>
            )}
          </>
        );
      case 'unpivot':
        return (
          <>
            {checks}
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={unpivotOther}
                onChange={(e) => setUnpivotOther(e.target.checked)}
              />
              {t('sheetQuery.form.unpivotOther')}
            </label>
          </>
        );
      default:
        return null;
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t(`sheetQuery.steps.${op}`)}
      width={440}
      testId="query-form"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="query-form-ok"
            onClick={() => {
              const s = build();
              if (typeof s === 'string') setError(s);
              else onOk(s);
            }}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        {fields()}
        {error && (
          <p className={q.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
