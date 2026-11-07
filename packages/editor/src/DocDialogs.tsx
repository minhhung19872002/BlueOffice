import { useTranslation } from '@blueoffice/i18n';
import type {
  Alignment,
  EditCommand,
  EditResult,
  FormatState,
  NoteInfo,
  PageSetupSpec,
  DocPasteMode,
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

/** Footnotes and endnotes pane: insert, edit the text of each note. */
export function NotesPanel({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [notes, setNotes] = useState<NoteInfo[]>([]);
  const [draft, setDraft] = useState<Record<string, string>>({});
  useEffect(() => {
    void run({ cmd: 'notes' }).then((r) => setNotes(r?.notes ?? []));
  }, [run]);
  const key = (n: NoteInfo) => `${n.endnote ? 'e' : 'f'}${n.id}`;
  const insert = async (endnote: boolean) => {
    const r = await run({ cmd: 'insertNote', endnote, text: '' });
    if (r?.notes) setNotes(r.notes);
  };
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
      {notes.length === 0 && <div className={styles.reviewMeta}>{t('notes.none')}</div>}
      <ul className={styles.reviewList}>
        {notes.map((n, i) => (
          <li key={key(n)} className={styles.noteItem}>
            <span className={styles.reviewKind}>
              {t(n.endnote ? 'notes.endnote' : 'notes.footnote', { n: i + 1 })}
            </span>
            <textarea
              data-testid="note-text"
              aria-label={t(n.endnote ? 'notes.endnote' : 'notes.footnote', { n: i + 1 })}
              value={draft[key(n)] ?? n.text}
              rows={2}
              onChange={(e) => setDraft({ ...draft, [key(n)]: e.target.value })}
            />
            <div className={styles.reviewActions}>
              <button
                type="button"
                data-testid="note-save"
                disabled={draft[key(n)] === undefined || draft[key(n)] === n.text}
                onClick={async () => {
                  const r = await run({
                    cmd: 'setNoteText',
                    endnote: n.endnote,
                    id: n.id,
                    text: draft[key(n)] ?? n.text,
                  });
                  if (r?.notes) setNotes(r.notes);
                  const next = { ...draft };
                  delete next[key(n)];
                  setDraft(next);
                }}
              >
                {t('notes.save')}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </aside>
  );
}
