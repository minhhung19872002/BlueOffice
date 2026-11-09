import { useTranslation } from '@blueoffice/i18n';
import type {
  Alignment,
  ControlInfo,
  ControlKind,
  EditCommand,
  EditResult,
  EquationInfo,
  FormatState,
  NoteInfo,
  NotePos,
  NoteRestart,
  PageSetupSpec,
  DocPasteMode,
  SectionState,
  StoryState,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState } from 'react';
import styles from './Editor.module.css';
import { setupOf } from './Ruler';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Paper sizes (twips, portrait; ECMA-376 Part 1, §17.6.13 pgSz). */
export const PAPER: Record<string, [number, number]> = {
  a4: [11906, 16838],
  a5: [8391, 11906],
  letter: [12240, 15840],
  legal: [12240, 20160],
};

/** Margin presets (twips: top, bottom, left, right). */
export const MARGINS: Record<string, [number, number, number, number]> = {
  // Nghị định 30/2020/NĐ-CP, Phụ lục I: top/bottom 20–25 mm, left 30–35 mm, right 15–20 mm.
  nd30: [1134, 1134, 1701, 851],
  normal: [1440, 1440, 1440, 1440],
  narrow: [720, 720, 720, 720],
  wide: [1440, 1440, 2880, 2880],
};

function unitOf(language: string) {
  return language === 'en-US' ? { per: 1440, label: 'in' } : { per: 567, label: 'cm' };
}

function paperOf(w: number, h: number): string {
  const [a, b] = w < h ? [w, h] : [h, w];
  return (
    Object.entries(PAPER).find(
      ([, [pw, ph]]) => Math.abs(pw - a) < 20 && Math.abs(ph - b) < 20,
    )?.[0] ?? 'custom'
  );
}

function marginsOf(s: PageSetupSpec): string {
  return (
    Object.entries(MARGINS).find(([, m]) =>
      [s.marginTop, s.marginBottom, s.marginLeft, s.marginRight].every(
        (v, i) => Math.abs(v - m[i]!) < 3,
      ),
    )?.[0] ?? 'custom'
  );
}

/** Number input in cm / inches bound to a twips value. */
function Length({
  label,
  twips,
  onChange,
  language,
  testId,
}: {
  label: string;
  twips: number;
  onChange: (tw: number) => void;
  language: string;
  testId?: string;
}) {
  const u = unitOf(language);
  // Text being typed; the bound value otherwise.
  const [edit, setEdit] = useState<string | null>(null);
  const text = edit ?? (twips / u.per).toFixed(2);
  return (
    <label className={styles.field}>
      <span>{label}</span>
      <input
        type="number"
        step="0.01"
        min="0"
        data-testid={testId}
        value={text}
        onBlur={() => setEdit(null)}
        onChange={(e) => {
          setEdit(e.target.value);
          const v = Number(e.target.value);
          if (Number.isFinite(v) && v >= 0) onChange(Math.round(v * u.per));
        }}
      />
      <span className={styles.unit}>{u.label}</span>
    </label>
  );
}

/** Page setup: paper, orientation, margins (incl. Nghị định 30), columns, header/footer distance. */
export function PageSetupDialog({
  format,
  run,
  language,
  onClose,
}: {
  format: FormatState | null | undefined;
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const initial = setupOf(format);
  const [s, setS] = useState<PageSetupSpec | null>(initial);
  const [scope, setScope] = useState<'section' | 'document'>(
    (format?.section?.count ?? 1) > 1 ? 'section' : 'document',
  );
  if (!s) return null;
  const landscape = s.width > s.height;
  const paper = paperOf(s.width, s.height);
  const preset = marginsOf(s);
  const set = (patch: Partial<PageSetupSpec>) => setS({ ...s, ...patch });
  const apply = async () => {
    await run({ cmd: 'setPageSetup', setup: s, scope });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('pageSetup.title')}
      width={640}
      testId="page-setup"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="page-setup-ok"
            onClick={() => void apply()}
          >
            {t('pageSetup.ok')}
          </button>
        </>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('pageSetup.paper')}</span>
          <select
            data-testid="page-paper"
            value={paper}
            onChange={(e) => {
              const p = PAPER[e.target.value];
              if (!p) return;
              const [w, h] = landscape ? [p[1], p[0]] : p;
              set({ width: w, height: h });
            }}
          >
            {Object.keys(PAPER).map((k) => (
              <option key={k} value={k}>
                {t(`pageSetup.papers.${k}`)}
              </option>
            ))}
            <option value="custom">{t('pageSetup.papers.custom')}</option>
          </select>
        </label>
        <fieldset className={styles.fieldset}>
          <legend>{t('pageSetup.orientation')}</legend>
          {(['portrait', 'landscape'] as const).map((o) => (
            <label key={o}>
              <input
                type="radio"
                name="orientation"
                data-testid={`page-${o}`}
                checked={(o === 'landscape') === landscape}
                onChange={() => {
                  if ((o === 'landscape') !== landscape) set({ width: s.height, height: s.width });
                }}
              />{' '}
              {t(`pageSetup.${o}`)}
            </label>
          ))}
        </fieldset>
        {paper === 'custom' && (
          <>
            <Length
              label={t('pageSetup.width')}
              twips={s.width}
              language={language}
              onChange={(width) => set({ width })}
            />
            <Length
              label={t('pageSetup.height')}
              twips={s.height}
              language={language}
              onChange={(height) => set({ height })}
            />
          </>
        )}
        <label className={styles.field}>
          <span>{t('pageSetup.margins')}</span>
          <select
            data-testid="page-margins"
            value={preset}
            onChange={(e) => {
              const m = MARGINS[e.target.value];
              if (m)
                set({ marginTop: m[0], marginBottom: m[1], marginLeft: m[2], marginRight: m[3] });
            }}
          >
            {Object.keys(MARGINS).map((k) => (
              <option key={k} value={k}>
                {t(`pageSetup.presets.${k}`)}
              </option>
            ))}
            <option value="custom">{t('pageSetup.presets.custom')}</option>
          </select>
        </label>
        <div className={styles.hint}>{t('pageSetup.nd30Hint')}</div>
        <Length
          label={t('pageSetup.top')}
          twips={s.marginTop}
          language={language}
          testId="margin-top"
          onChange={(marginTop) => set({ marginTop })}
        />
        <Length
          label={t('pageSetup.bottom')}
          twips={s.marginBottom}
          language={language}
          testId="margin-bottom"
          onChange={(marginBottom) => set({ marginBottom })}
        />
        <Length
          label={t('pageSetup.left')}
          twips={s.marginLeft}
          language={language}
          testId="margin-left"
          onChange={(marginLeft) => set({ marginLeft })}
        />
        <Length
          label={t('pageSetup.right')}
          twips={s.marginRight}
          language={language}
          testId="margin-right"
          onChange={(marginRight) => set({ marginRight })}
        />
        <label className={styles.field}>
          <span>{t('pageSetup.columns')}</span>
          <input
            type="number"
            min="1"
            max="6"
            data-testid="page-columns"
            value={s.columns}
            onChange={(e) =>
              set({ columns: Math.max(1, Math.min(6, Number(e.target.value) || 1)) })
            }
          />
        </label>
        <Length
          label={t('pageSetup.columnSpace')}
          twips={s.columnSpace}
          language={language}
          onChange={(columnSpace) => set({ columnSpace })}
        />
        <Length
          label={t('pageSetup.header')}
          twips={s.header}
          language={language}
          onChange={(header) => set({ header })}
        />
        <Length
          label={t('pageSetup.footer')}
          twips={s.footer}
          language={language}
          onChange={(footer) => set({ footer })}
        />
        <label className={styles.field}>
          <span>{t('pageSetup.applyTo')}</span>
          <select
            data-testid="page-scope"
            value={scope}
            onChange={(e) => setScope(e.target.value as 'section' | 'document')}
          >
            <option value="section">{t('pageSetup.thisSection')}</option>
            <option value="document">{t('pageSetup.wholeDocument')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

/** Paste Special: formatted (keep / merge) or unformatted text. */
export function PasteSpecialDialog({
  available,
  onPaste,
  onClose,
}: {
  /** What the clipboard holds. */
  available: { html: boolean; text: boolean };
  onPaste: (mode: DocPasteMode) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [as, setAs] = useState<'html' | 'text'>(available.html ? 'html' : 'text');
  const [keep, setKeep] = useState<'keep' | 'merge'>('keep');
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('paste.title')}
      testId="paste-special"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="paste-special-ok"
            disabled={!available.html && !available.text}
            onClick={() => onPaste(as === 'text' ? 'text' : keep)}
          >
            {t('paste.ok')}
          </button>
        </>
      }
    >
      <fieldset className={styles.fieldset}>
        <legend>{t('paste.as')}</legend>
        <label>
          <input
            type="radio"
            name="as"
            disabled={!available.html}
            checked={as === 'html'}
            onChange={() => setAs('html')}
          />{' '}
          {t('paste.html')}
        </label>
        <label>
          <input
            type="radio"
            name="as"
            data-testid="paste-as-text"
            checked={as === 'text'}
            onChange={() => setAs('text')}
          />{' '}
          {t('paste.text')}
        </label>
      </fieldset>
      <fieldset className={styles.fieldset} disabled={as === 'text'}>
        <legend>{t('paste.formatting')}</legend>
        <label>
          <input
            type="radio"
            name="keep"
            checked={keep === 'keep'}
            onChange={() => setKeep('keep')}
          />{' '}
          {t('paste.keep')}
        </label>
        <label>
          <input
            type="radio"
            name="keep"
            data-testid="paste-merge"
            checked={keep === 'merge'}
            onChange={() => setKeep('merge')}
          />{' '}
          {t('paste.merge')}
        </label>
      </fieldset>
      {!available.html && !available.text && <div className={styles.hint}>{t('paste.empty')}</div>}
    </Dialog>
  );
}

/** Position of the floating picture at the selection (§20.4.2.10 positionH,
 * §20.4.2.11 positionV posOffset); a tracked move when track changes is on. */
export function PicturePositionDialog({
  format,
  run,
  language,
  onClose,
}: {
  format: FormatState | null | undefined;
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const pic = format?.picture;
  // Points ↔ twips for the length fields.
  const [x, setX] = useState(Math.round((pic?.x ?? 0) * 20));
  const [y, setY] = useState(Math.round((pic?.y ?? 0) * 20));
  const apply = async () => {
    await run({ cmd: 'setPicturePosition', x: x / 20, y: y / 20 });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('picture.position')}
      testId="picture-position"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="picture-position-ok"
            onClick={() => void apply()}
          >
            {t('pageSetup.ok')}
          </button>
        </>
      }
    >
      <div className={styles.formGrid}>
        <Length
          label={t('picture.horizontal')}
          twips={x}
          language={language}
          onChange={setX}
          testId="picture-x"
        />
        <Length
          label={t('picture.vertical')}
          twips={y}
          language={language}
          onChange={setY}
          testId="picture-y"
        />
      </div>
    </Dialog>
  );
}

/** Table width and alignment (§17.4.63 tblW, §17.4.29 jc). */
export function TablePropertiesDialog({
  format,
  run,
  language,
  onClose,
}: {
  format: FormatState | null | undefined;
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const ts = format?.table;
  const [mode, setMode] = useState<'auto' | 'pct' | 'fixed'>(
    ts?.widthPct != null ? 'pct' : ts?.width ? 'fixed' : 'auto',
  );
  const [pct, setPct] = useState(ts?.widthPct ?? 100);
  const [width, setWidth] = useState(ts?.width || 9000);
  const [align, setAlign] = useState<Alignment>(ts?.align ?? 'Left');
  const apply = async () => {
    await run({
      cmd: 'tableEdit',
      edit: {
        kind: 'properties',
        widthPct: mode === 'pct' ? pct : null,
        width: mode === 'fixed' ? width : null,
        align,
      },
    });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('table.properties')}
      testId="table-properties"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="table-properties-ok"
            onClick={() => void apply()}
          >
            {t('pageSetup.ok')}
          </button>
        </>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('table.width')}</span>
          <select value={mode} onChange={(e) => setMode(e.target.value as typeof mode)}>
            <option value="auto">{t('table.widthAuto')}</option>
            <option value="pct">{t('table.widthPct')}</option>
            <option value="fixed">{t('table.widthFixed')}</option>
          </select>
        </label>
        {mode === 'pct' && (
          <label className={styles.field}>
            <span>%</span>
            <input
              type="number"
              min="1"
              max="500"
              value={pct}
              onChange={(e) => setPct(Math.max(1, Number(e.target.value) || 100))}
            />
          </label>
        )}
        {mode === 'fixed' && (
          <Length label={t('table.width')} twips={width} language={language} onChange={setWidth} />
        )}
        <label className={styles.field}>
          <span>{t('table.align')}</span>
          <select value={align} onChange={(e) => setAlign(e.target.value as Alignment)}>
            <option value="Left">{t('commands.format.alignLeft')}</option>
            <option value="Center">{t('commands.format.alignCenter')}</option>
            <option value="Right">{t('commands.format.alignRight')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

/** Built-in templates (generated by the engine, `bo_engine::templates`). */
export const TEMPLATES = ['nd30'] as const;

/** "File → New from template". */
export function TemplateDialog({
  onCreate,
  onClose,
}: {
  onCreate: (id: string, name: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('templates.title')}
      testId="templates"
      width={480}
      footer={
        <button type="button" className={styles.dialogButton} onClick={onClose}>
          {t('security.cancel')}
        </button>
      }
    >
      {TEMPLATES.map((id) => (
        <button
          key={id}
          type="button"
          className={styles.templateCard}
          data-testid={`template-${id}`}
          onClick={() => onCreate(id, t(`templates.${id}.file`))}
        >
          <strong>{t(`templates.${id}.name`)}</strong>
          <span>{t(`templates.${id}.description`)}</span>
        </button>
      ))}
    </Dialog>
  );
}

/** Header/footer editing bar (§17.10): options, page number fields, close. */
export function HeaderFooterBar({ story, run }: { story: StoryState; run: Run }) {
  const { t } = useTranslation();
  return (
    <div
      className={styles.hfBar}
      role="toolbar"
      data-testid="hf-bar"
      aria-label={t(`hf.${story.kind}`)}
    >
      <strong data-testid="hf-kind">
        {t(`hf.${story.kind}`)} · {t('hf.section', { n: story.section + 1 })} ·{' '}
        {t(`hf.variants.${story.variant}`)}
      </strong>
      <label>
        <input
          type="checkbox"
          data-testid="hf-first"
          checked={story.titlePage}
          onChange={(e) => void run({ cmd: 'setHeaderOptions', titlePage: e.target.checked })}
        />{' '}
        {t('hf.differentFirst')}
      </label>
      <label>
        <input
          type="checkbox"
          data-testid="hf-odd-even"
          checked={story.evenAndOdd}
          onChange={(e) => void run({ cmd: 'setHeaderOptions', evenAndOdd: e.target.checked })}
        />{' '}
        {t('hf.differentOddEven')}
      </label>
      <label>
        <input
          type="checkbox"
          data-testid="hf-link"
          checked={story.linked}
          disabled={!story.canLink}
          onChange={(e) => void run({ cmd: 'setHeaderLink', linked: e.target.checked })}
        />{' '}
        {t('hf.linkToPrevious')}
      </label>
      <button
        type="button"
        data-testid="hf-page-number"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => void run({ cmd: 'insertField', instr: 'PAGE' })}
      >
        {t('hf.pageNumber')}
      </button>
      <button
        type="button"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => void run({ cmd: 'insertField', instr: 'NUMPAGES' })}
      >
        {t('hf.pageCount')}
      </button>
      <button
        type="button"
        data-testid="hf-close"
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => void run({ cmd: 'closeHeaderFooter' })}
      >
        {t('hf.close')}
      </button>
    </div>
  );
}

/**
 * Footnotes and endnotes pane (optional list): insert, jump to a note to edit it in
 * place on the page, footnote numbering/placement of the section (§17.11).
 */
export function NotesPanel({
  run,
  onClose,
  section,
}: {
  run: Run;
  onClose: () => void;
  section?: SectionState;
}) {
  const { t } = useTranslation();
  const [notes, setNotes] = useState<NoteInfo[]>([]);
  useEffect(() => {
    void run({ cmd: 'notes' }).then((r) => setNotes(r?.notes ?? []));
  }, [run]);
  const key = (n: NoteInfo) => `${n.endnote ? 'e' : 'f'}${n.id}`;
  const insert = async (endnote: boolean) => {
    const r = await run({ cmd: 'insertNote', endnote, text: '' });
    if (r?.notes) setNotes(r.notes);
  };
  const options = (restart: NoteRestart, pos: NotePos) =>
    void run({ cmd: 'setNoteOptions', endnote: false, options: { restart, pos } });
  const restart = section?.footnoteRestart ?? 'continuous';
  const pos = section?.footnotePos ?? 'pageBottom';
  return (
    <aside className={styles.reviewPane} aria-label={t('notes.title')} data-testid="notes-pane">
      <div className={styles.reviewHeader}>
        <strong>{t('notes.title')}</strong>
        <button type="button" aria-label={t('host.close')} onClick={onClose}>
          ×
        </button>
      </div>
      <div className={styles.reviewActions}>
        <button type="button" data-testid="insert-footnote" onClick={() => void insert(false)}>
          {t('notes.insertFootnote')}
        </button>
        <button type="button" onClick={() => void insert(true)}>
          {t('notes.insertEndnote')}
        </button>
      </div>
      <label className={styles.reviewMeta}>
        {t('notes.restart')}{' '}
        <select
          data-testid="note-restart"
          value={restart}
          onChange={(e) => options(e.target.value as NoteRestart, pos)}
        >
          {(['continuous', 'eachSect', 'eachPage'] as const).map((v) => (
            <option key={v} value={v}>
              {t(`notes.restarts.${v}`)}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.reviewMeta}>
        {t('notes.position')}{' '}
        <select
          data-testid="note-position"
          value={pos}
          onChange={(e) => options(restart, e.target.value as NotePos)}
        >
          {(['pageBottom', 'beneathText'] as const).map((v) => (
            <option key={v} value={v}>
              {t(`notes.positions.${v}`)}
            </option>
          ))}
        </select>
      </label>
      {notes.length === 0 && <div className={styles.reviewMeta}>{t('notes.none')}</div>}
      <ul className={styles.reviewList}>
        {notes.map((n, i) => (
          <li key={key(n)} className={styles.noteItem}>
            <button
              type="button"
              data-testid="note-item"
              title={t('notes.goTo')}
              onClick={() => void run({ cmd: 'editNote', endnote: n.endnote, id: n.id })}
            >
              <span className={styles.reviewKind}>
                {t(n.endnote ? 'notes.endnote' : 'notes.footnote', { n: i + 1 })}
              </span>{' '}
              {n.text}
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}

/** Type an equation in the UnicodeMath linear format or LaTeX (an equation of the document is edited in
 * place when the selection covers one). */
export function EquationDialog({
  initial,
  onInsert,
  onClose,
}: {
  initial: EquationInfo | null;
  onInsert: (text: string, latex: boolean, display: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState(initial?.linear ?? '');
  const [latex, setLatex] = useState(false);
  const [display, setDisplay] = useState(initial?.display ?? false);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(initial ? 'equation.editTitle' : 'equation.title')}
      testId="equation-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="equation-ok"
            disabled={!text.trim()}
            onClick={() => onInsert(text, latex, display)}
          >
            {t(initial ? 'equation.update' : 'equation.insert')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        {t('equation.input')}
        <textarea
          data-testid="equation-input"
          rows={3}
          value={text}
          spellCheck={false}
          onChange={(e) => setText(e.target.value)}
          style={{ font: '14px monospace', width: '100%' }}
        />
      </label>
      <fieldset className={styles.fieldset}>
        <legend>{t('equation.syntax')}</legend>
        <label>
          <input type="radio" name="syntax" checked={!latex} onChange={() => setLatex(false)} />{' '}
          {t('equation.unicodeMath')}
        </label>
        <label>
          <input
            type="radio"
            name="syntax"
            data-testid="equation-latex"
            checked={latex}
            onChange={() => setLatex(true)}
          />{' '}
          LaTeX
        </label>
      </fieldset>
      <label>
        <input
          type="checkbox"
          data-testid="equation-display"
          checked={display}
          onChange={(e) => setDisplay(e.target.checked)}
        />{' '}
        {t('equation.display')}
      </label>
      <div className={styles.hint}>{t(latex ? 'equation.hintLatex' : 'equation.hintUnicode')}</div>
    </Dialog>
  );
}

/** Field kinds of the Insert Field dialog by category (ECMA-376 Part 1, §17.16.5). */
export const FIELD_KINDS: { group: string; kinds: string[] }[] = [
  { group: 'date', kinds: ['DATE', 'TIME', 'CREATEDATE', 'SAVEDATE', 'PRINTDATE', 'EDITTIME'] },
  {
    group: 'info',
    kinds: [
      'AUTHOR',
      'TITLE',
      'SUBJECT',
      'KEYWORDS',
      'COMMENTS',
      'LASTSAVEDBY',
      'TEMPLATE',
      'FILENAME',
      'FILESIZE',
      'REVNUM',
      'NUMPAGES',
      'NUMWORDS',
      'NUMCHARS',
      'DOCPROPERTY',
      'DOCVARIABLE',
    ],
  },
  { group: 'user', kinds: ['USERNAME', 'USERINITIALS', 'USERADDRESS'] },
  { group: 'numbering', kinds: ['PAGE', 'SECTION', 'SECTIONPAGES', 'SEQ', 'AUTONUM', 'LISTNUM'] },
  { group: 'links', kinds: ['REF', 'PAGEREF', 'STYLEREF', 'NOTEREF', 'HYPERLINK', 'INCLUDETEXT'] },
  { group: 'formula', kinds: ['=', 'IF', 'COMPARE', 'SET', 'ASK', 'FILLIN', 'QUOTE', 'SYMBOL'] },
  { group: 'index', kinds: ['TOC', 'TC', 'XE', 'INDEX', 'MACROBUTTON', 'GOTOBUTTON'] },
];

const NUMBER_FORMATS = [
  '',
  'Arabic',
  'ROMAN',
  'roman',
  'ALPHABETIC',
  'alphabetic',
  'Ordinal',
  'CardText',
  'OrdText',
  'Hex',
];
const TEXT_FORMATS = ['', 'Upper', 'Lower', 'FirstCap', 'Caps'];
const DATE_PICTURES = ['dd/MM/yyyy', 'd MMMM yyyy', 'dddd, d MMMM yyyy', 'HH:mm', 'h:mm am/pm'];
const DATE_KINDS = new Set(['DATE', 'TIME', 'CREATEDATE', 'SAVEDATE', 'PRINTDATE']);
const NUMBER_KINDS = new Set([
  'PAGE',
  'NUMPAGES',
  'NUMWORDS',
  'NUMCHARS',
  'SECTION',
  'SECTIONPAGES',
  'SEQ',
  'REVNUM',
  'EDITTIME',
  'FILESIZE',
  '=',
  'COMPARE',
]);

const HINTS = new Set([
  'formula',
  'IF',
  'REF',
  'PAGEREF',
  'SEQ',
  'DOCPROPERTY',
  'DOCVARIABLE',
  'SET',
  'ASK',
  'SYMBOL',
  'STYLEREF',
  'QUOTE',
]);

/** The instruction of a field from the dialog's choices (§17.16.1 syntax). */
export function fieldInstr(kind: string, args: string, picture: string, format: string): string {
  const quote = (s: string) => (/\s/.test(s) && !s.startsWith('"') ? `"${s}"` : s);
  let instr = kind === '=' ? `= ${args.trim()}` : `${kind}${args.trim() ? ` ${args.trim()}` : ''}`;
  if (picture.trim()) {
    instr += DATE_KINDS.has(kind)
      ? ` \\@ ${quote(picture.trim())}`
      : ` \\# ${quote(picture.trim())}`;
  }
  if (format) instr += ` \\* ${format}`;
  return instr;
}

/** Insert Field: kind, arguments, format switches (§17.16.4) and the resulting instruction. */
export function FieldDialog({
  onInsert,
  onClose,
}: {
  onInsert: (instr: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [kind, setKind] = useState('DATE');
  const [args, setArgs] = useState('');
  const [picture, setPicture] = useState('dd/MM/yyyy');
  const [format, setFormat] = useState('');
  const isDate = DATE_KINDS.has(kind);
  const isNumber = NUMBER_KINDS.has(kind);
  const instr = fieldInstr(kind, args, isDate || isNumber ? picture : '', format);
  const hint = kind === '=' ? 'formula' : kind;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('fieldDialog.title')}
      testId="field-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="field-ok"
            onClick={() => onInsert(instr)}
          >
            {t('fieldDialog.insert')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        {t('fieldDialog.kind')}
        <select
          data-testid="field-kind"
          value={kind}
          onChange={(e) => {
            const k = e.target.value;
            setKind(k);
            setArgs('');
            setFormat('');
            setPicture(DATE_KINDS.has(k) ? (k === 'TIME' ? 'HH:mm' : 'dd/MM/yyyy') : '');
          }}
        >
          {FIELD_KINDS.map((g) => (
            <optgroup key={g.group} label={t(`fieldDialog.groups.${g.group}`)}>
              {g.kinds.map((k) => (
                <option key={k} value={k}>
                  {k === '=' ? t('fieldDialog.formula') : k}
                </option>
              ))}
            </optgroup>
          ))}
        </select>
      </label>
      <label className={styles.field}>
        {t('fieldDialog.args')}
        <input
          data-testid="field-args"
          value={args}
          spellCheck={false}
          placeholder={HINTS.has(hint) ? t(`fieldDialog.hint.${hint}`) : undefined}
          onChange={(e) => setArgs(e.target.value)}
        />
      </label>
      {(isDate || isNumber) && (
        <label className={styles.field}>
          {t(isDate ? 'fieldDialog.datePicture' : 'fieldDialog.numberPicture')}
          <input
            data-testid="field-picture"
            list={isDate ? 'field-date-pictures' : undefined}
            value={picture}
            spellCheck={false}
            onChange={(e) => setPicture(e.target.value)}
          />
          {isDate && (
            <datalist id="field-date-pictures">
              {DATE_PICTURES.map((p) => (
                <option key={p} value={p} />
              ))}
            </datalist>
          )}
        </label>
      )}
      <label className={styles.field}>
        {t('fieldDialog.format')}
        <select
          data-testid="field-format"
          value={format}
          onChange={(e) => setFormat(e.target.value)}
        >
          {(isNumber ? NUMBER_FORMATS : TEXT_FORMATS).map((f) => (
            <option key={f} value={f}>
              {f ? t(`fieldDialog.formats.${f}`) : t('fieldDialog.formats.none')}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.field}>
        {t('fieldDialog.code')}
        <code data-testid="field-code">{instr}</code>
      </label>
    </Dialog>
  );
}

const CONTROL_KINDS: ControlKind[] = [
  'text',
  'richText',
  'checkbox',
  'dropDown',
  'comboBox',
  'date',
];

/** Insert a content control (§17.5.2 sdt): its kind, placeholder and list items. */
export function ControlDialog({
  onInsert,
  onClose,
}: {
  onInsert: (cmd: EditCommand) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [kind, setKind] = useState<ControlKind>('text');
  const [placeholder, setPlaceholder] = useState(t('controlDialog.defaultPlaceholder'));
  const [items, setItems] = useState('');
  const list = kind === 'dropDown' || kind === 'comboBox';
  const pick = (k: ControlKind) => {
    setKind(k);
    setPlaceholder(
      k === 'dropDown' || k === 'comboBox'
        ? t('controlDialog.defaultItem')
        : k === 'date'
          ? t('controlDialog.defaultDate')
          : t('controlDialog.defaultPlaceholder'),
    );
  };
  const insert = () =>
    onInsert({
      cmd: 'insertControl',
      kind,
      placeholder,
      items: list
        ? items
            .split('\n')
            .map((i) => i.trim())
            .filter((i) => i !== '')
        : [],
    });
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('controlDialog.title')}
      testId="control-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="control-ok"
            onClick={insert}
          >
            {t('controlDialog.insert')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        {t('controlDialog.kind')}
        <select
          data-testid="control-kind"
          value={kind}
          onChange={(e) => pick(e.target.value as ControlKind)}
        >
          {CONTROL_KINDS.map((k) => (
            <option key={k} value={k}>
              {t(`controlDialog.kinds.${k}`)}
            </option>
          ))}
        </select>
      </label>
      {kind !== 'checkbox' && (
        <label className={styles.field}>
          {t('controlDialog.placeholder')}
          <input
            data-testid="control-placeholder"
            value={placeholder}
            onChange={(e) => setPlaceholder(e.target.value)}
          />
        </label>
      )}
      {list && (
        <label className={styles.field}>
          {t('controlDialog.items')}
          <textarea
            data-testid="control-items"
            rows={4}
            value={items}
            onChange={(e) => setItems(e.target.value)}
          />
        </label>
      )}
    </Dialog>
  );
}

/** The content control around the caret: toggle, pick an item or a date. */
export function ControlBar({ control, run }: { control: ControlInfo; run: Run }) {
  const { t } = useTranslation();
  const [value, setValue] = useState('');
  const known = (CONTROL_KINDS as string[]).includes(control.kind);
  const set = (v: string) => void run({ cmd: 'setControlValue', value: v });
  return (
    <div
      className={`${styles.hfBar} ${styles.ccBar}`}
      role="toolbar"
      data-testid="control-bar"
      aria-label={t('controlBar.label')}
    >
      <strong data-testid="control-kind-label">
        {known ? t(`controlDialog.kinds.${control.kind}`) : t('controlBar.label')}
        {control.title ? ` · ${control.title}` : ''}
      </strong>
      {control.locked && <span>{t('controlBar.locked')}</span>}
      {!control.locked && control.kind === 'checkbox' && (
        <button
          type="button"
          data-testid="control-toggle"
          aria-pressed={!!control.checked}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => set('')}
        >
          {control.checked ? '☒' : '☐'} {t('controlBar.toggle')}
        </button>
      )}
      {!control.locked && (control.kind === 'dropDown' || control.kind === 'comboBox') && (
        <select
          data-testid="control-choose"
          aria-label={t('controlBar.choose')}
          value=""
          onChange={(e) => e.target.value !== '' && set(e.target.value)}
        >
          <option value="">{t('controlBar.choose')}</option>
          {control.items.map((i) => (
            <option key={i} value={i}>
              {i}
            </option>
          ))}
        </select>
      )}
      {!control.locked && control.kind === 'comboBox' && (
        <>
          <input
            data-testid="control-value"
            aria-label={t('controlBar.value')}
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
          <button
            type="button"
            data-testid="control-apply"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => value !== '' && set(value)}
          >
            {t('controlBar.apply')}
          </button>
        </>
      )}
      {!control.locked && control.kind === 'date' && (
        <input
          type="date"
          data-testid="control-date"
          aria-label={t('controlBar.date')}
          value={control.date?.slice(0, 10) ?? ''}
          onChange={(e) => e.target.value !== '' && set(e.target.value)}
        />
      )}
    </div>
  );
}
