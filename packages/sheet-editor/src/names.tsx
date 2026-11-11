/**
 * Defined names of BlueSheet (Formulas ▸ Defined names): the Name Manager (Ctrl+F3) with
 * New / Edit / Delete, the New name dialog, and Create from Selection (Ctrl+Shift+F3).
 * The engine (`nameList`, `defineName`, `deleteName`, `createNames`) keeps the names in the
 * workbook (§18.2.5 `definedName`); each change is one undo step.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { DefinedNameInfo, SheetHost, SheetRange } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';
import { colName } from './grid';
import styles from './SheetEditor.module.css';

interface Common {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}

/** `=Sheet1!$A$1:$B$5` (`='Bảng 1'!$A$1` when the sheet name needs quotes). */
export function absoluteRef(sheetName: string, r: SheetRange): string {
  const simple = /^[A-Za-z_][A-Za-z0-9_.]*$/.test(sheetName);
  const s = simple ? sheetName : `'${sheetName.replace(/'/g, "''")}'`;
  const cell = (row: number, col: number) => `$${colName(col)}$${row + 1}`;
  const body =
    r.r0 === r.r1 && r.c0 === r.c1 ? cell(r.r0, r.c0) : `${cell(r.r0, r.c0)}:${cell(r.r1, r.c1)}`;
  return `=${s}!${body}`;
}

/** The engine's refusal of a name, as the message shown under the fields. */
function nameError(t: (k: string) => string, e: unknown): string {
  const m = e instanceof Error ? e.message : String(e);
  if (m.includes('invalid name')) return t('sheetNames.invalidName');
  if (m.includes('name exists')) return t('sheetNames.exists');
  if (m.includes('invalid formula')) return t('sheetNames.invalidRef');
  return m;
}

/** New name / Edit name: name, scope, comment and what it refers to. */
export function NameEditDialog({
  host,
  onClose,
  onDone,
  sheets,
  initial,
  refersTo,
}: Omit<Common, 'sheet' | 'fail'> & {
  sheets: string[];
  /** The name being edited (Edit), or nothing (New). */
  initial?: DefinedNameInfo;
  /** New: the selection as an absolute reference (`=Sheet1!$A$1:$A$5`). */
  refersTo: string;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(initial?.name ?? '');
  const [scope, setScope] = useState<number | null>(initial?.scope ?? null);
  const [comment, setComment] = useState(initial?.comment ?? '');
  const [ref, setRef] = useState(initial?.refersTo ?? refersTo);
  const [error, setError] = useState('');
  const ok = async () => {
    try {
      await host.exec({
        cmd: 'defineName',
        name,
        refersTo: ref,
        scope,
        comment,
        ...(initial ? { old: { name: initial.name, scope: initial.scope } } : {}),
      });
      await onDone();
      onClose();
    } catch (e) {
      setError(nameError(t, e));
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={initial ? t('sheetNames.editTitle') : t('sheetNames.newTitle')}
      testId="name-edit-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="name-edit-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetNames.name')}
          <input
            autoFocus
            data-testid="name-edit-name"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setError('');
            }}
            onKeyDown={(e) => e.key === 'Enter' && void ok()}
          />
        </label>
        <label>
          {t('sheetNames.scope')}
          <select
            data-testid="name-edit-scope"
            value={scope === null ? '' : String(scope)}
            onChange={(e) => setScope(e.target.value === '' ? null : Number(e.target.value))}
          >
            <option value="">{t('sheetNames.workbook')}</option>
            {sheets.map((s, i) => (
              <option key={i} value={i}>
                {s}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('sheetNames.comment')}
          <input
            data-testid="name-edit-comment"
            value={comment}
            onChange={(e) => setComment(e.target.value)}
          />
        </label>
        <label>
          {t('sheetNames.refersTo')}
          <input
            data-testid="name-edit-ref"
            value={ref}
            onChange={(e) => {
              setRef(e.target.value);
              setError('');
            }}
            onKeyDown={(e) => e.key === 'Enter' && void ok()}
          />
        </label>
      </div>
      {error && (
        <p className={styles.error} role="alert" data-testid="name-edit-error">
          {error}
        </p>
      )}
    </Dialog>
  );
}

/** The Name Manager: every name with what it refers to, its scope and comment. */
export function NameManagerDialog({
  host,
  onClose,
  onDone,
  fail,
  sheets,
  refersTo,
}: Omit<Common, 'sheet'> & { sheets: string[]; refersTo: string }) {
  const { t } = useTranslation();
  const [names, setNames] = useState<DefinedNameInfo[]>([]);
  const [picked, setPicked] = useState<number>(-1);
  const [editing, setEditing] = useState<DefinedNameInfo | 'new' | null>(null);
  const load = useCallback(
    () =>
      host
        .exec<DefinedNameInfo[]>({ cmd: 'nameList' })
        .then((n) => setNames(n ?? []))
        .catch(fail),
    [host, fail],
  );
  useEffect(() => {
    void load();
  }, [load]);
  const current = names[picked];
  const scopeLabel = (s: number | null) =>
    s === null ? t('sheetNames.workbook') : (sheets[s] ?? String(s));
  return (
    <>
      <Dialog
        open={editing === null}
        onOpenChange={(o) => !o && onClose()}
        title={t('sheetNames.manager')}
        testId="name-manager"
        footer={
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="name-manager-close"
            onClick={onClose}
          >
            {t('sheetNames.close')}
          </button>
        }
      >
        <div className={styles.row}>
          <button
            type="button"
            className={styles.btn}
            data-testid="name-manager-new"
            onClick={() => setEditing('new')}
          >
            {t('sheetNames.newButton')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="name-manager-edit"
            disabled={!current}
            onClick={() => current && setEditing(current)}
          >
            {t('sheetNames.editButton')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="name-manager-delete"
            disabled={!current}
            onClick={() => {
              if (!current) return;
              void host
                .exec({ cmd: 'deleteName', name: current.name, scope: current.scope })
                .then(async () => {
                  setPicked(-1);
                  await onDone();
                  await load();
                })
                .catch(fail);
            }}
          >
            {t('sheetNames.deleteButton')}
          </button>
        </div>
        <table className={styles.nameTable} data-testid="name-manager-list">
          <thead>
            <tr>
              <th>{t('sheetNames.name')}</th>
              <th>{t('sheetNames.refersTo')}</th>
              <th>{t('sheetNames.scope')}</th>
              <th>{t('sheetNames.comment')}</th>
            </tr>
          </thead>
          <tbody>
            {names.map((n, i) => (
              <tr
                key={`${n.name}|${n.scope ?? ''}`}
                data-name={n.name}
                aria-selected={i === picked}
                className={i === picked ? styles.nameRowPicked : undefined}
                onClick={() => setPicked(i)}
                onDoubleClick={() => setEditing(n)}
              >
                <td>{n.name}</td>
                <td>{n.refersTo}</td>
                <td>{scopeLabel(n.scope)}</td>
                <td>{n.comment ?? ''}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {names.length === 0 && <p className={styles.hint}>{t('sheetNames.none')}</p>}
      </Dialog>
      {editing !== null && (
        <NameEditDialog
          host={host}
          sheets={sheets}
          refersTo={refersTo}
          {...(editing === 'new' ? {} : { initial: editing })}
          onClose={() => setEditing(null)}
          onDone={async () => {
            await onDone();
            await load();
          }}
        />
      )}
    </>
  );
}

/** Create Names from Selection: which sides of the range hold the labels. */
export function CreateNamesDialog({
  host,
  sheet,
  range,
  guess,
  onClose,
  onDone,
  onCreated,
  fail,
}: Common & {
  range: SheetRange;
  /** Sides ticked at first: the top row / left column when they hold text. */
  guess: { top: boolean; left: boolean };
  onCreated: (names: string[]) => void;
}) {
  const { t } = useTranslation();
  const [sides, setSides] = useState({ ...guess, bottom: false, right: false });
  const ok = async () => {
    try {
      const r = await host.exec<{ created: string[] }>({
        cmd: 'createNames',
        sheet,
        range,
        ...sides,
      });
      await onDone();
      onCreated(r?.created ?? []);
      onClose();
    } catch (e) {
      fail(e);
    }
  };
  const box = (key: keyof typeof sides) => (
    <label className={styles.check}>
      <input
        type="checkbox"
        data-testid={`create-names-${key}`}
        checked={sides[key]}
        onChange={(e) => setSides({ ...sides, [key]: e.target.checked })}
      />
      {t(`sheetNames.side.${key}`)}
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetNames.createTitle')}
      testId="create-names"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="create-names-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <p className={styles.hint}>{t('sheetNames.createHint')}</p>
      {box('top')}
      {box('left')}
      {box('bottom')}
      {box('right')}
    </Dialog>
  );
}
