import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { Dialog } from './Dialog';
import styles from './ObjectTools.module.css';
import { toBase64 } from './usePictureTools';

/**
 * Embedded and linked BlueSheet workbooks in BlueDoc and BlueSlide (CORE-EMBED-001): the
 * "Insert object" dialog (new table, or a workbook file embedded or linked), the editing
 * grid of an embedded workbook (double-click the object) and "Update links". The dialogs
 * talk to the engine through the host's command channel (`ole*` commands, structurally the
 * platform's `OleCommand`); the registry commands (`object.*`) open them.
 */

/** Object command (structurally the platform's `OleCommand`). */
export type ObjectToolsCommand =
  | { cmd: 'oleInfo'; cells?: boolean; data?: boolean }
  | {
      cmd: 'oleInsert';
      data?: string;
      range?: string;
      cells?: string[][];
      link?: string;
      autoUpdate?: boolean;
    }
  | { cmd: 'oleEdit'; cells: string[][] }
  | { cmd: 'oleSetData'; data: string }
  | { cmd: 'oleLinks' }
  | { cmd: 'oleUpdateLinks'; sources: { source: string; data: string }[] };

/** What the tools read from a command result. */
export interface ObjectToolsInfo {
  workbook: boolean;
  linked: boolean;
  source?: string;
  sheet?: string;
  range?: string;
  cells?: string[][];
  data?: string;
}

export interface ObjectToolsResult {
  ole?: ObjectToolsInfo | null;
  links?: { source: string; path: string; item: string; count: number }[] | null;
  count?: number | null;
  error?: string | null;
}

export interface ObjectToolsLabels {
  insertTitle: string;
  editTitle: string;
  newTable: string;
  fromFile: string;
  browse: string;
  noFile: string;
  link: string;
  linkHint: string;
  autoUpdate: string;
  range: string;
  rangeHint: string;
  addRow: string;
  addColumn: string;
  removeRow: string;
  removeColumn: string;
  ok: string;
  cancel: string;
  /** Accessible name of a grid cell. */
  cell: (p: { address: string }) => string;
}

/** Registry command ids of the object tools. */
export const OBJECT_COMMANDS = [
  'object.insert',
  'object.edit',
  'object.updateLinks',
  'object.saveCopy',
] as const;

export type ObjectActions = Record<(typeof OBJECT_COMMANDS)[number], () => void>;

export interface ObjectToolsHost {
  labels: ObjectToolsLabels;
  run: (cmd: ObjectToolsCommand) => Promise<ObjectToolsResult | null>;
  /** Ask the user for a workbook file (`path` on desktop). */
  pickFile: () => Promise<{ bytes: Uint8Array; name: string; path?: string } | null>;
  /** Read a file by path (desktop); absent where linked files cannot be read. */
  readPath?: (path: string) => Promise<Uint8Array>;
  /** Save a copy of the embedded workbook. */
  saveFile?: (bytes: Uint8Array, name: string) => Promise<unknown>;
  /** A refused command's error code (`noLinks`, `linksUnavailable`, `notWorkbook`, …). */
  onError?: (code: string) => void;
  /** "Update links" finished: objects updated. */
  onUpdated?: (count: number) => void;
  /** Receives the actions (after each render). */
  hub?: ObjectActionsHub;
}

/** Hands the actions of the mounted tools to the host's registry commands. */
export class ObjectActionsHub {
  private actions: ObjectActions | null = null;
  private opener: (() => Promise<boolean>) | null = null;

  /** Open the object at the selection, if any (double-click); resolves whether there was one. */
  activate(): Promise<boolean> {
    return this.opener?.() ?? Promise.resolve(false);
  }

  setActivate(f: (() => Promise<boolean>) | null): void {
    this.opener = f;
  }

  set(a: ObjectActions | null): void {
    this.actions = a;
  }

  get(): ObjectActions | null {
    return this.actions;
  }
}

/** Column letters of a 0-based column (A … Z, AA …). */
export function columnName(c: number): string {
  let s = '';
  let n = c + 1;
  while (n > 0) {
    const m = (n - 1) % 26;
    s = String.fromCharCode(65 + m) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
}

/** Top-left cell (0-based row, column) of an `A1:C3` area. */
export function areaOrigin(range: string | undefined): { row: number; col: number } {
  const m = /^\$?([A-Za-z]+)\$?(\d+)/.exec((range ?? '').split('!').pop() ?? '');
  const [, letters, digits] = m ?? [];
  if (!letters || !digits) return { row: 0, col: 0 };
  let col = 0;
  for (const ch of letters.toUpperCase()) col = col * 26 + (ch.charCodeAt(0) - 64);
  return { row: Number(digits) - 1, col: col - 1 };
}

/** A rectangular copy of `cells` (`rows` × `cols`, empty strings added / cut). */
export function resizeGrid(cells: string[][], rows: number, cols: number): string[][] {
  return Array.from({ length: Math.max(1, rows) }, (_, r) =>
    Array.from({ length: Math.max(1, cols) }, (_, c) => cells[r]?.[c] ?? ''),
  );
}

function base64ToBytes(b64: string): Uint8Array {
  const s = atob(b64);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

/** Insert object dialog. */
export function InsertObjectDialog({
  labels: l,
  canLink,
  onPick,
  onInsert,
  onClose,
}: {
  labels: ObjectToolsLabels;
  /** Linking needs a file path (desktop). */
  canLink: boolean;
  onPick: () => Promise<{ bytes: Uint8Array; name: string; path?: string } | null>;
  onInsert: (cmd: Extract<ObjectToolsCommand, { cmd: 'oleInsert' }>) => void;
  onClose: () => void;
}) {
  const [from, setFrom] = useState<'new' | 'file'>('new');
  const [file, setFile] = useState<{ bytes: Uint8Array; name: string; path?: string } | null>(null);
  const [link, setLink] = useState(false);
  const [auto, setAuto] = useState(false);
  const [range, setRange] = useState('');
  const linkable = canLink && !!file?.path;
  const ok = () => {
    if (from === 'new') {
      onInsert({ cmd: 'oleInsert' });
      return;
    }
    if (!file) return;
    onInsert({
      cmd: 'oleInsert',
      data: toBase64(file.bytes),
      range: range.trim() || undefined,
      link: link && linkable ? file.path : undefined,
      autoUpdate: link && linkable ? auto : undefined,
    });
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={l.insertTitle}
      testId="object-insert-dialog"
      footer={
        <>
          <button
            type="button"
            data-testid="object-insert-ok"
            disabled={from === 'file' && !file}
            onClick={ok}
          >
            {l.ok}
          </button>
          <button type="button" onClick={onClose}>
            {l.cancel}
          </button>
        </>
      }
    >
      <fieldset className={styles.fieldset}>
        <label className={styles.radio}>
          <input
            type="radio"
            name="object-from"
            checked={from === 'new'}
            data-testid="object-from-new"
            onChange={() => setFrom('new')}
          />
          {l.newTable}
        </label>
        <label className={styles.radio}>
          <input
            type="radio"
            name="object-from"
            checked={from === 'file'}
            data-testid="object-from-file"
            onChange={() => setFrom('file')}
          />
          {l.fromFile}
        </label>
      </fieldset>
      {from === 'file' && (
        <div className={styles.file}>
          <div className={styles.row}>
            <button
              type="button"
              data-testid="object-browse"
              onClick={() => void onPick().then((f) => f && setFile(f))}
            >
              {l.browse}
            </button>
            <span className={styles.name} data-testid="object-file-name">
              {file?.name ?? l.noFile}
            </span>
          </div>
          <label className={styles.field}>
            {l.range}
            <input
              type="text"
              value={range}
              placeholder={l.rangeHint}
              data-testid="object-range"
              onChange={(e) => setRange(e.target.value)}
            />
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={link && linkable}
              disabled={!linkable}
              data-testid="object-link"
              onChange={(e) => setLink(e.target.checked)}
            />
            {l.link}
          </label>
          {!canLink && <p className={styles.note}>{l.linkHint}</p>}
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={auto}
              disabled={!(link && linkable)}
              onChange={(e) => setAuto(e.target.checked)}
            />
            {l.autoUpdate}
          </label>
        </div>
      )}
    </Dialog>
  );
}

/** Editing grid of an embedded workbook's shown area. */
export function EditObjectDialog({
  labels: l,
  cells,
  range,
  onApply,
  onClose,
}: {
  labels: ObjectToolsLabels;
  cells: string[][];
  /** Shown area (`A1:C3`): the grid's cell names start at its top-left. */
  range?: string;
  onApply: (cells: string[][]) => void;
  onClose: () => void;
}) {
  const [grid, setGrid] = useState(() =>
    resizeGrid(cells, cells.length, Math.max(1, ...cells.map((r) => r.length))),
  );
  const rows = grid.length;
  const cols = grid[0]?.length ?? 1;
  const o = areaOrigin(range);
  const set = (r: number, c: number, v: string) =>
    setGrid((g) => g.map((row, i) => (i === r ? row.map((x, j) => (j === c ? v : x)) : row)));
  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={l.editTitle}
      width={640}
      testId="object-edit-dialog"
      footer={
        <>
          <button type="button" data-testid="object-edit-ok" onClick={() => onApply(grid)}>
            {l.ok}
          </button>
          <button type="button" onClick={onClose}>
            {l.cancel}
          </button>
        </>
      }
    >
      <div className={styles.row}>
        <button
          type="button"
          data-testid="object-add-row"
          onClick={() => setGrid(resizeGrid(grid, rows + 1, cols))}
        >
          {l.addRow}
        </button>
        <button
          type="button"
          data-testid="object-add-column"
          onClick={() => setGrid(resizeGrid(grid, rows, cols + 1))}
        >
          {l.addColumn}
        </button>
        <button
          type="button"
          disabled={rows <= 1}
          onClick={() => setGrid(resizeGrid(grid, rows - 1, cols))}
        >
          {l.removeRow}
        </button>
        <button
          type="button"
          disabled={cols <= 1}
          onClick={() => setGrid(resizeGrid(grid, rows, cols - 1))}
        >
          {l.removeColumn}
        </button>
      </div>
      <div className={styles.gridWrap}>
        <table className={styles.grid} data-testid="object-grid">
          <thead>
            <tr>
              <th />
              {grid[0]?.map((_, c) => (
                <th key={c} scope="col">
                  {columnName(o.col + c)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {grid.map((row, r) => (
              <tr key={r}>
                <th scope="row">{o.row + r + 1}</th>
                {row.map((v, c) => (
                  <td key={c}>
                    <input
                      type="text"
                      value={v}
                      aria-label={l.cell({ address: `${columnName(o.col + c)}${o.row + r + 1}` })}
                      data-testid={`object-cell-${r}-${c}`}
                      onChange={(e) => set(r, c, e.target.value)}
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Dialog>
  );
}

type Open = { kind: 'insert' } | { kind: 'edit'; info: ObjectToolsInfo } | null;

/** The object dialogs and the actions of the registry commands. */
export function useObjectTools(host: ObjectToolsHost): {
  dialogs: ReactNode;
  actions: ObjectActions;
  /** Open the editing grid for the object at the selection (double-click). */
  activate: () => Promise<boolean>;
} {
  const [open, setOpen] = useState<Open>(null);
  const hostRef = useRef(host);
  useEffect(() => {
    hostRef.current = host;
  });
  const send = useCallback(async (cmd: ObjectToolsCommand) => {
    const r = await hostRef.current.run(cmd);
    if (r?.error) hostRef.current.onError?.(r.error);
    return r;
  }, []);

  const activate = useCallback(async () => {
    const r = await hostRef.current.run({ cmd: 'oleInfo', cells: true });
    const info = r?.ole;
    if (!info) return false;
    if (info.linked || !info.workbook || !info.cells) {
      hostRef.current.onError?.(info.linked ? 'linkedObject' : 'notWorkbook');
      return true;
    }
    setOpen({ kind: 'edit', info });
    return true;
  }, []);

  const actions: ObjectActions = {
    'object.insert': () => setOpen({ kind: 'insert' }),
    'object.edit': () => {
      void activate().then((found) => {
        if (!found) hostRef.current.onError?.('noObject');
      });
    },
    'object.updateLinks': () => {
      void (async () => {
        const h = hostRef.current;
        const r = await h.run({ cmd: 'oleLinks' });
        const links = r?.links ?? [];
        if (links.length === 0) {
          h.onError?.('noLinks');
          return;
        }
        if (!h.readPath) {
          h.onError?.('linksUnavailable');
          return;
        }
        const sources: { source: string; data: string }[] = [];
        for (const l of links) {
          try {
            sources.push({ source: l.source, data: toBase64(await h.readPath(l.path)) });
          } catch {
            h.onError?.('linkMissing');
          }
        }
        if (sources.length === 0) return;
        const u = await send({ cmd: 'oleUpdateLinks', sources });
        if (u && !u.error) h.onUpdated?.(u.count ?? 0);
      })();
    },
    'object.saveCopy': () => {
      void (async () => {
        const h = hostRef.current;
        const r = await h.run({ cmd: 'oleInfo', data: true });
        if (!r?.ole?.data) {
          h.onError?.('noObject');
          return;
        }
        await h.saveFile?.(base64ToBytes(r.ole.data), 'object.xlsx');
      })();
    },
  };

  useEffect(() => {
    host.hub?.set(actions);
    host.hub?.setActivate(activate);
  });

  const close = () => setOpen(null);
  const dialogs = (
    <>
      {open?.kind === 'insert' && (
        <InsertObjectDialog
          labels={host.labels}
          canLink={!!host.readPath}
          onPick={() => hostRef.current.pickFile()}
          onClose={close}
          onInsert={(cmd) => {
            close();
            void send(cmd);
          }}
        />
      )}
      {open?.kind === 'edit' && (
        <EditObjectDialog
          labels={host.labels}
          cells={open.info.cells ?? [['']]}
          range={open.info.range}
          onClose={close}
          onApply={(cells) => {
            close();
            void send({ cmd: 'oleEdit', cells });
          }}
        />
      )}
    </>
  );
  return { dialogs, actions, activate };
}
