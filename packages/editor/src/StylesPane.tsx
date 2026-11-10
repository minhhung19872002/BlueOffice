import type {
  DocStyleEntry,
  DocStyleSpec,
  EditCommand,
  EditResult,
  OpenedFile,
} from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { useEffect, useMemo, useState } from 'react';
import { emptyStyleSpec, StyleDialog } from './StyleDialog';
import { StyleOrganizer } from './StyleOrganizer';
import styles from './StylesPane.module.css';

/** Which styles the pane lists. */
export type StylesShow = 'recommended' | 'inUse' | 'all';

/** Style management hooks (DOC-STYLE-001); without them the pane only applies styles. */
export interface StylesPaneManage {
  run: (c: EditCommand) => Promise<EditResult | null>;
  query: (c: EditCommand) => Promise<EditResult | null>;
  openFile: (accept: string[]) => Promise<OpenedFile | null>;
  saveFile: (bytes: Uint8Array, name: string, mime: string) => Promise<string | null>;
  /** Open the Style Inspector or Reveal Formatting pane. */
  openPane: (pane: 'styleInspector' | 'revealFormatting') => void;
}

/** A dialog the host asks the pane to open (ribbon / command search). */
export interface StylesPaneRequest {
  kind: 'newStyle' | 'organizer';
  /** Bumped for every request. */
  n: number;
}

export interface StylesPaneProps {
  /** Paragraph and character styles of the document (`styleList`, ECMA-376 Part 1, §17.7.4.17). */
  load: () => Promise<DocStyleEntry[] | null>;
  /** Style of the paragraph at the caret (`null`: Normal). */
  current: string | null;
  /** Changes when the list may have changed (document opened, style applied). */
  refreshKey: string;
  apply: (id: string) => void;
  manage?: StylesPaneManage;
  request?: StylesPaneRequest | null;
  /** The requested dialog closed (the host forgets the request). */
  onRequestDone?: () => void;
}

/** Preview size in the list (points, kept readable). */
const previewSize = (pt: number) => Math.min(18, Math.max(10, pt));

/** Entries shown for `show`: recommended ones by §17.7.4.19 uiPriority, then list order. */
export function visibleStyles(list: DocStyleEntry[], show: StylesShow, filter: string) {
  const q = filter.trim().toLocaleLowerCase();
  const shown = list.filter(
    (s) =>
      (!q || s.name.toLocaleLowerCase().includes(q)) &&
      (show === 'all' || (show === 'inUse' ? s.inUse === true : s.recommended !== false)),
  );
  if (show !== 'recommended') return shown;
  return shown
    .map((s, i) => ({ s, i }))
    .sort((a, b) => (a.s.uiPriority ?? 99) - (b.s.uiPriority ?? 99) || a.i - b.i)
    .map((x) => x.s);
}

type DialogState =
  | { kind: 'modify'; id: string; spec: DocStyleSpec; error: string | null }
  | { kind: 'new'; spec: DocStyleSpec; error: string | null }
  | { kind: 'organizer' };

/**
 * Styles task pane (CORE-UI-006, DOC-STYLE-001): the document's paragraph and
 * character styles — recommended, in use or all — drawn in their own formatting;
 * click applies one. Each style has a menu (modify, select all instances, update to
 * match the selection, delete); the toolbar opens New Style, the Style Inspector,
 * Reveal Formatting and the Organizer.
 */
export function StylesPane({
  load,
  current,
  refreshKey,
  apply,
  manage,
  request,
  onRequestDone,
}: StylesPaneProps) {
  const { t } = useTranslation();
  const [list, setList] = useState<DocStyleEntry[] | null>(null);
  const [filter, setFilter] = useState('');
  const [preview, setPreview] = useState(true);
  const [show, setShow] = useState<StylesShow>('recommended');
  const [menu, setMenu] = useState<string | null>(null);
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let live = true;
    void load().then((l) => {
      if (live) setList(l ?? []);
    });
    return () => {
      live = false;
    };
  }, [load, refreshKey, tick]);

  const shown = useMemo(() => visibleStyles(list ?? [], show, filter), [list, show, filter]);
  const active = current ?? 'Normal';

  /** Run an edit; refresh the list from its `styles`. */
  const edit = async (c: EditCommand) => {
    if (!manage) return null;
    const r = await manage.run(c);
    if (r?.styles) setList(r.styles);
    else setTick((n) => n + 1);
    return r;
  };

  const openModify = async (id: string) => {
    setMenu(null);
    const r = await manage?.query({ cmd: 'styleDetails', id });
    const spec = r?.styleMgr?.spec;
    if (spec) setDialog({ kind: 'modify', id, spec, error: null });
  };
  const openNew = () => {
    setDialog({ kind: 'new', spec: emptyStyleSpec(current ?? 'Normal'), error: null });
  };

  // A new request opens its dialog (state derived during render, not in an effect).
  const [handled, setHandled] = useState(0);
  if (manage && request && request.n !== handled) {
    setHandled(request.n);
    setDialog(
      request.kind === 'organizer'
        ? { kind: 'organizer' }
        : { kind: 'new', spec: emptyStyleSpec(current ?? 'Normal'), error: null },
    );
  }
  const close = () => {
    setDialog(null);
    onRequestDone?.();
  };

  const action = async (id: string, what: 'selectAll' | 'updateToMatch' | 'delete') => {
    setMenu(null);
    setNote(null);
    if (what === 'delete') {
      // Undoable: no confirmation (the menu item's tooltip says what happens).
      const r = await edit({ cmd: 'styleDelete', id });
      if (r?.error)
        setNote(t(`docStyles.errors.${r.error}`, { defaultValue: t('docStyles.errors.failed') }));
    } else if (what === 'selectAll') {
      const r = await edit({ cmd: 'styleSelectAll', id });
      setNote(t('taskPane.styles.instances', { count: r?.matches?.length ?? 0 }));
    } else {
      await edit({ cmd: 'styleUpdateToMatch', id });
    }
  };

  const submit = async (spec: DocStyleSpec, fromSelection: boolean) => {
    if (!dialog || dialog.kind === 'organizer') return;
    const r =
      dialog.kind === 'modify'
        ? await edit({ cmd: 'styleModify', id: dialog.id, spec })
        : await edit({ cmd: 'styleNew', spec, fromSelection });
    if (r?.error) setDialog({ ...dialog, spec, error: r.error });
    else close();
  };

  return (
    <div className={styles.pane} data-testid="styles-pane">
      <div className={styles.tools}>
        <input
          type="search"
          className={styles.filter}
          value={filter}
          placeholder={t('taskPane.styles.filter')}
          aria-label={t('taskPane.styles.filter')}
          data-testid="styles-filter"
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => e.stopPropagation()}
        />
        <label className={styles.toggle}>
          {t('taskPane.styles.show')}
          <select
            className={styles.filter}
            data-testid="styles-show"
            value={show}
            onChange={(e) => setShow(e.target.value as StylesShow)}
          >
            <option value="recommended">{t('taskPane.styles.showRecommended')}</option>
            <option value="inUse">{t('taskPane.styles.showInUse')}</option>
            <option value="all">{t('taskPane.styles.showAll')}</option>
          </select>
        </label>
        <label className={styles.toggle}>
          <input type="checkbox" checked={preview} onChange={(e) => setPreview(e.target.checked)} />
          {t('taskPane.styles.preview')}
        </label>
        {manage && (
          <div className={styles.actions}>
            <button
              type="button"
              className={styles.action}
              data-testid="styles-new"
              onClick={openNew}
            >
              {t('taskPane.styles.newStyle')}
            </button>
            <button
              type="button"
              className={styles.action}
              data-testid="styles-inspector"
              onClick={() => manage.openPane('styleInspector')}
            >
              {t('taskPane.styles.inspector')}
            </button>
            <button
              type="button"
              className={styles.action}
              data-testid="styles-reveal"
              onClick={() => manage.openPane('revealFormatting')}
            >
              {t('taskPane.styles.reveal')}
            </button>
            <button
              type="button"
              className={styles.action}
              data-testid="styles-organizer"
              onClick={() => setDialog({ kind: 'organizer' })}
            >
              {t('taskPane.styles.organizer')}
            </button>
          </div>
        )}
      </div>
      {note && (
        <p className={styles.note} role="status" data-testid="styles-note">
          {note}
        </p>
      )}
      {list === null ? (
        <p className={styles.note}>{t('taskPane.styles.loading')}</p>
      ) : shown.length === 0 ? (
        <p className={styles.note}>{t('taskPane.styles.empty')}</p>
      ) : (
        <ul className={styles.list} aria-label={t('taskPane.styles.title')}>
          {shown.map((s) => (
            <li key={s.id} className={styles.row}>
              <button
                type="button"
                className={styles.item}
                aria-pressed={s.id === active}
                data-testid="style-item"
                data-style-id={s.id}
                title={t(
                  s.linked
                    ? 'taskPane.styles.linked'
                    : s.kind === 'paragraph'
                      ? 'taskPane.styles.paragraph'
                      : 'taskPane.styles.character',
                )}
                onClick={() => apply(s.id)}
              >
                <span
                  className={styles.name}
                  style={
                    preview
                      ? {
                          fontFamily: `"${s.preview.font}", sans-serif`,
                          fontSize: `${previewSize(s.preview.size)}pt`,
                          fontWeight: s.preview.bold ? 700 : 400,
                          fontStyle: s.preview.italic ? 'italic' : 'normal',
                          textDecoration: s.preview.underline ? 'underline' : 'none',
                          color: `#${s.preview.color}`,
                        }
                      : undefined
                  }
                >
                  {s.name}
                </span>
                <span className={styles.kind} aria-hidden>
                  {s.linked ? '¶a' : s.kind === 'paragraph' ? '¶' : 'a'}
                </span>
              </button>
              {manage && (
                <button
                  type="button"
                  className={styles.more}
                  aria-label={t('taskPane.styles.options', { name: s.name })}
                  aria-expanded={menu === s.id}
                  data-testid="style-menu"
                  data-style-id={s.id}
                  onClick={() => setMenu(menu === s.id ? null : s.id)}
                >
                  ⋯
                </button>
              )}
              {menu === s.id && (
                <div className={styles.menu} role="menu" data-testid="style-menu-list">
                  <button
                    type="button"
                    role="menuitem"
                    data-testid="style-modify"
                    onClick={() => void openModify(s.id)}
                  >
                    {t('taskPane.styles.modify')}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    data-testid="style-select-all"
                    onClick={() => void action(s.id, 'selectAll')}
                  >
                    {t('taskPane.styles.selectAll')}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    data-testid="style-update"
                    onClick={() => void action(s.id, 'updateToMatch')}
                  >
                    {t('taskPane.styles.updateToMatch')}
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    data-testid="style-delete"
                    title={t('taskPane.styles.confirmDelete', { name: s.name })}
                    disabled={s.isDefault === true}
                    onClick={() => void action(s.id, 'delete')}
                  >
                    {t('taskPane.styles.delete')}
                  </button>
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
      {manage && dialog && dialog.kind !== 'organizer' && (
        <StyleDialog
          key={dialog.kind === 'modify' ? dialog.id : 'new'}
          mode={dialog.kind}
          {...(dialog.kind === 'modify' ? { styleId: dialog.id } : {})}
          initial={dialog.spec}
          styles={list ?? []}
          error={dialog.error}
          onCancel={close}
          onOk={(spec, fromSel) => void submit(spec, fromSel)}
        />
      )}
      {manage && dialog?.kind === 'organizer' && (
        <StyleOrganizer
          run={async (c) => {
            const r = await manage.run(c);
            setTick((n) => n + 1);
            return r;
          }}
          query={manage.query}
          openFile={manage.openFile}
          saveFile={manage.saveFile}
          onClose={close}
        />
      )}
    </div>
  );
}
