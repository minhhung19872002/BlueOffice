/**
 * References tab dialogs of BlueDoc: table of figures (`TOC \c` / `\a`, ECMA-376 Part 1,
 * §17.16.5.68), index entries and index (§17.16.5.72 XE, §17.16.5.29 INDEX), citations and
 * table of authorities (§17.16.5.62 TA, §17.16.5.67 TOA) and footnote ↔ endnote
 * conversion (§17.11). Each dialog sends one engine command (one undo step); the field
 * instructions are built by the pure helpers below.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { EditCommand, EditResult, RefTarget } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState, type ReactNode } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Dialogs of this file (values of the editor's `dialog` state). */
export const REFERENCE_DIALOGS = [
  'tableOfFigures',
  'markEntry',
  'insertIndex',
  'markCitation',
  'insertToa',
  'convertNotes',
] as const;
export type ReferenceDialogId = (typeof REFERENCE_DIALOGS)[number];

export function isReferenceDialog(d: string | null): d is ReferenceDialogId {
  return (REFERENCE_DIALOGS as readonly string[]).includes(d ?? '');
}

/** Leader of right-aligned page numbers (§17.18.84 ST_TabTlc). */
export type Leader = 'dot' | 'hyphen' | 'underscore' | 'none';
const LEADERS: Leader[] = ['dot', 'hyphen', 'underscore', 'none'];

/** Quotes cannot appear inside a field argument. */
const quoteFree = (s: string) => s.trim().replace(/"/g, "'");

export interface FiguresOptions {
  label: string;
  includeLabel: boolean;
  pageNumbers: boolean;
  rightAlign: boolean;
  hyperlinks: boolean;
}

/** `TOC \c "label"` (with label and number) or `\a` (caption text only); `\n` without page
 * numbers, `\p " "` page number right after the text, `\h` links. */
export function figuresInstr(o: FiguresOptions): string {
  const parts = ['TOC'];
  if (o.hyperlinks) parts.push('\\h', '\\z');
  parts.push(`${o.includeLabel ? '\\c' : '\\a'} "${quoteFree(o.label)}"`);
  if (!o.pageNumbers) parts.push('\\n');
  else if (!o.rightAlign) parts.push('\\p " "');
  return parts.join(' ');
}

export interface IndexOptions {
  columns: number;
  runIn: boolean;
  rightAlign: boolean;
  headings: boolean;
  /** Sort language: Vietnamese (LCID 1066) or English (1033). */
  language: 'vi' | 'en';
}

/** `INDEX`: `\h "A"` letter headings, `\c` columns, `\r` run-in, `\e` tab (right-aligned page
 * numbers), `\z` language. */
export function indexInstr(o: IndexOptions): string {
  const parts = ['INDEX'];
  if (o.headings) parts.push('\\h "A"');
  const cols = Math.max(1, Math.min(4, Math.round(o.columns)));
  parts.push(`\\c "${cols}"`);
  if (o.runIn) parts.push('\\r');
  if (o.rightAlign && !o.runIn) parts.push('\\e "\t"');
  parts.push(`\\z "${o.language === 'vi' ? 1066 : 1033}"`);
  return parts.join(' ');
}

export interface ToaOptions {
  /** 0 = every category. */
  category: number;
  passim: boolean;
  headings: boolean;
  rightAlign: boolean;
}

/** `TOA`: `\h` category headings, `\c` category, `\p` passim, `\e` tab. */
export function toaInstr(o: ToaOptions): string {
  const parts = ['TOA'];
  if (o.headings) parts.push('\\h');
  parts.push(`\\c "${Math.max(0, Math.min(16, Math.round(o.category)))}"`);
  if (o.passim) parts.push('\\p');
  if (o.rightAlign) parts.push('\\e "\t"');
  return parts.join(' ');
}

/** Name of citation category `n` (1–16). */
export function categoryName(t: (k: string, o?: Record<string, string>) => string, n: number) {
  return n >= 1 && n <= 7 ? t(`refDialogs.cat${n}`) : t('refDialogs.catN', { n: String(n) });
}

/** The selected text (first line) to start a mark dialog with. */
function useSelectedText(run: Run, onText: (t: string) => void) {
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'copy' }).then((r) => {
      const text = r?.clip?.text?.split(/\r?\n/)[0]?.trim() ?? '';
      if (alive && text) onText(text);
    });
    return () => {
      alive = false;
    };
    // Seeds the form once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run]);
}

function Buttons({ onClose, children }: { onClose: () => void; children: ReactNode }) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.dialogButton} onClick={onClose}>
        {t('refDialogs.cancel')}
      </button>
      {children}
    </>
  );
}

function Ok({ testId, onClick, label }: { testId: string; onClick: () => void; label?: string }) {
  const { t } = useTranslation();
  return (
    <button type="button" className={styles.dialogPrimary} data-testid={testId} onClick={onClick}>
      {label ?? t('refDialogs.ok')}
    </button>
  );
}

function Check({
  label,
  checked,
  onChange,
  testId,
  disabled,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  testId: string;
  disabled?: boolean;
}) {
  return (
    <label className={styles.field}>
      <input
        type="checkbox"
        data-testid={testId}
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

function LeaderSelect({
  value,
  onChange,
  disabled,
  testId,
}: {
  value: Leader;
  onChange: (l: Leader) => void;
  disabled?: boolean;
  testId: string;
}) {
  const { t } = useTranslation();
  const names: Record<Leader, string> = {
    dot: t('refDialogs.leaderDot'),
    hyphen: t('refDialogs.leaderHyphen'),
    underscore: t('refDialogs.leaderUnderscore'),
    none: t('refDialogs.leaderNone'),
  };
  return (
    <label className={styles.field}>
      <span>{t('refDialogs.leader')}</span>
      <select
        data-testid={testId}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as Leader)}
      >
        {LEADERS.map((l) => (
          <option key={l} value={l}>
            {names[l]}
          </option>
        ))}
      </select>
    </label>
  );
}

/** Insert a table of figures for one caption label. */
export function TableOfFiguresDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const labels = [t('references.figure'), t('references.table'), t('references.equation')];
  const [o, setO] = useState<FiguresOptions>({
    label: labels[0]!,
    includeLabel: true,
    pageNumbers: true,
    rightAlign: true,
    hyperlinks: true,
  });
  const [leader, setLeader] = useState<Leader>('dot');
  const apply = async () => {
    onClose();
    await run({
      cmd: 'insertReferenceTable',
      instr: figuresInstr(o),
      leader: o.pageNumbers && o.rightAlign ? leader : undefined,
    });
  };
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('refDialogs.figuresTitle')}
      width={460}
      testId="figures-dialog"
      footer={
        <Buttons onClose={onClose}>
          <Ok testId="figures-ok" onClick={() => void apply()} />
        </Buttons>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('refDialogs.captionLabel')}</span>
          <input
            data-testid="figures-label"
            list="figures-labels"
            value={o.label}
            onChange={(e) => setO({ ...o, label: e.target.value })}
          />
          <datalist id="figures-labels">
            {labels.map((l) => (
              <option key={l} value={l} />
            ))}
          </datalist>
        </label>
        <Check
          label={t('refDialogs.includeLabel')}
          checked={o.includeLabel}
          testId="figures-include-label"
          onChange={(includeLabel) => setO({ ...o, includeLabel })}
        />
        <Check
          label={t('refDialogs.pageNumbers')}
          checked={o.pageNumbers}
          testId="figures-pages"
          onChange={(pageNumbers) => setO({ ...o, pageNumbers })}
        />
        <Check
          label={t('refDialogs.rightAlign')}
          checked={o.rightAlign}
          disabled={!o.pageNumbers}
          testId="figures-right"
          onChange={(rightAlign) => setO({ ...o, rightAlign })}
        />
        <LeaderSelect
          value={leader}
          onChange={setLeader}
          disabled={!o.pageNumbers || !o.rightAlign}
          testId="figures-leader"
        />
        <Check
          label={t('refDialogs.hyperlinks')}
          checked={o.hyperlinks}
          testId="figures-links"
          onChange={(hyperlinks) => setO({ ...o, hyperlinks })}
        />
      </div>
    </Dialog>
  );
}

type EntryKind = 'page' | 'crossRef' | 'range';

/** Mark the selection as an index entry (or every occurrence of it). */
export function MarkEntryDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [main, setMain] = useState('');
  const [sub, setSub] = useState('');
  const [kind, setKind] = useState<EntryKind>('page');
  const [see, setSee] = useState(t('refDialogs.seeDefault'));
  const [bookmark, setBookmark] = useState('');
  const [bookmarks, setBookmarks] = useState<RefTarget[]>([]);
  const [bold, setBold] = useState(false);
  const [italic, setItalic] = useState(false);
  useSelectedText(run, setMain);
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'crossReferenceTargets' }).then((r) => {
      const list = (r?.targets ?? []).filter((x) => !x.id.startsWith('caption:'));
      if (!alive) return;
      setBookmarks(list);
      if (list[0]) setBookmark(list[0].id);
    });
    return () => {
      alive = false;
    };
  }, [run]);
  const mark = async (all: boolean) => {
    if (!main.trim()) return;
    onClose();
    await run({
      cmd: 'markIndexEntry',
      mark: {
        main: main.trim(),
        sub: sub.trim(),
        crossRef: kind === 'crossRef' ? see.trim() : '',
        bookmark: kind === 'range' ? bookmark : '',
        bold,
        italic,
      },
      all,
    });
  };
  const radio = (k: EntryKind, label: string) => (
    <label className={styles.field}>
      <input
        type="radio"
        name="entry-kind"
        data-testid={`entry-kind-${k}`}
        checked={kind === k}
        disabled={k === 'range' && bookmarks.length === 0}
        onChange={() => setKind(k)}
      />
      <span>{label}</span>
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('refDialogs.markEntryTitle')}
      width={480}
      testId="mark-entry-dialog"
      footer={
        <Buttons onClose={onClose}>
          <Ok
            testId="mark-entry-all"
            label={t('refDialogs.markAll')}
            onClick={() => void mark(true)}
          />
          <Ok
            testId="mark-entry-ok"
            label={t('refDialogs.mark')}
            onClick={() => void mark(false)}
          />
        </Buttons>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('refDialogs.main')}</span>
          <input data-testid="entry-main" value={main} onChange={(e) => setMain(e.target.value)} />
        </label>
        <label className={styles.field}>
          <span>{t('refDialogs.sub')}</span>
          <input data-testid="entry-sub" value={sub} onChange={(e) => setSub(e.target.value)} />
        </label>
        {radio('page', t('refDialogs.currentPage'))}
        {radio('crossRef', t('refDialogs.crossRef'))}
        <label className={styles.field}>
          <span>{t('refDialogs.crossRef')}</span>
          <input
            data-testid="entry-see"
            value={see}
            disabled={kind !== 'crossRef'}
            onChange={(e) => setSee(e.target.value)}
          />
        </label>
        {radio('range', t('refDialogs.pageRange'))}
        <label className={styles.field}>
          <span>{t('refDialogs.bookmark')}</span>
          <select
            data-testid="entry-bookmark"
            value={bookmark}
            disabled={kind !== 'range'}
            onChange={(e) => setBookmark(e.target.value)}
          >
            {bookmarks.map((b) => (
              <option key={b.id} value={b.id}>
                {b.id}
              </option>
            ))}
          </select>
        </label>
        <Check label={t('refDialogs.bold')} checked={bold} testId="entry-bold" onChange={setBold} />
        <Check
          label={t('refDialogs.italic')}
          checked={italic}
          testId="entry-italic"
          onChange={setItalic}
        />
      </div>
    </Dialog>
  );
}

/** Insert an index of the marked entries. */
export function IndexDialog({
  run,
  language,
  onClose,
}: {
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [o, setO] = useState<IndexOptions>({
    columns: 2,
    runIn: false,
    rightAlign: false,
    headings: true,
    language: language.startsWith('vi') ? 'vi' : 'en',
  });
  const [leader, setLeader] = useState<Leader>('dot');
  const apply = async () => {
    onClose();
    const cols = Math.max(1, Math.min(4, Math.round(o.columns)));
    await run({
      cmd: 'insertReferenceTable',
      instr: indexInstr(o),
      leader: o.rightAlign && !o.runIn ? leader : undefined,
      columns: cols,
    });
  };
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('refDialogs.indexTitle')}
      width={460}
      testId="index-dialog"
      footer={
        <Buttons onClose={onClose}>
          <Ok testId="index-ok" onClick={() => void apply()} />
        </Buttons>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('refDialogs.columns')}</span>
          <input
            type="number"
            min={1}
            max={4}
            data-testid="index-columns"
            value={o.columns}
            onChange={(e) => setO({ ...o, columns: Number(e.target.value) || 1 })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('refDialogs.layout')}</span>
          <select
            data-testid="index-type"
            value={o.runIn ? 'runIn' : 'indented'}
            onChange={(e) => setO({ ...o, runIn: e.target.value === 'runIn' })}
          >
            <option value="indented">{t('refDialogs.indented')}</option>
            <option value="runIn">{t('refDialogs.runIn')}</option>
          </select>
        </label>
        <Check
          label={t('refDialogs.rightAlign')}
          checked={o.rightAlign}
          disabled={o.runIn}
          testId="index-right"
          onChange={(rightAlign) => setO({ ...o, rightAlign })}
        />
        <LeaderSelect
          value={leader}
          onChange={setLeader}
          disabled={!o.rightAlign || o.runIn}
          testId="index-leader"
        />
        <Check
          label={t('refDialogs.headings')}
          checked={o.headings}
          testId="index-headings"
          onChange={(headings) => setO({ ...o, headings })}
        />
        <label className={styles.field}>
          <span>{t('refDialogs.language')}</span>
          <select
            data-testid="index-language"
            value={o.language}
            onChange={(e) => setO({ ...o, language: e.target.value === 'vi' ? 'vi' : 'en' })}
          >
            <option value="vi">{t('refDialogs.vietnamese')}</option>
            <option value="en">{t('refDialogs.english')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

function CategorySelect({
  value,
  onChange,
  all,
  testId,
}: {
  value: number;
  onChange: (n: number) => void;
  all?: boolean;
  testId: string;
}) {
  const { t } = useTranslation();
  const cats = Array.from({ length: 16 }, (_, i) => i + 1);
  return (
    <label className={styles.field}>
      <span>{t('refDialogs.category')}</span>
      <select data-testid={testId} value={value} onChange={(e) => onChange(Number(e.target.value))}>
        {all && <option value={0}>{t('refDialogs.allCategories')}</option>}
        {cats.map((n) => (
          <option key={n} value={n}>
            {categoryName(t, n)}
          </option>
        ))}
      </select>
    </label>
  );
}

/** Mark the selection as a citation (or every mention of it). */
export function MarkCitationDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [long, setLong] = useState('');
  const [short, setShort] = useState('');
  const [category, setCategory] = useState(1);
  useSelectedText(run, (text) => {
    setLong(text);
    setShort(text);
  });
  const mark = async (all: boolean) => {
    if (!long.trim()) return;
    onClose();
    await run({
      cmd: 'markCitation',
      mark: { long: long.trim(), short: short.trim(), category },
      all,
    });
  };
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('refDialogs.markCitationTitle')}
      width={480}
      testId="mark-citation-dialog"
      footer={
        <Buttons onClose={onClose}>
          <Ok
            testId="mark-citation-all"
            label={t('refDialogs.markAll')}
            onClick={() => void mark(true)}
          />
          <Ok
            testId="mark-citation-ok"
            label={t('refDialogs.mark')}
            onClick={() => void mark(false)}
          />
        </Buttons>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('refDialogs.long')}</span>
          <input
            data-testid="citation-long"
            value={long}
            onChange={(e) => setLong(e.target.value)}
          />
        </label>
        <label className={styles.field}>
          <span>{t('refDialogs.short')}</span>
          <input
            data-testid="citation-short"
            value={short}
            onChange={(e) => setShort(e.target.value)}
          />
        </label>
        <CategorySelect value={category} onChange={setCategory} testId="citation-category" />
      </div>
    </Dialog>
  );
}

/** Insert a table of authorities. */
export function AuthoritiesDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [o, setO] = useState<ToaOptions>({
    category: 0,
    passim: true,
    headings: true,
    rightAlign: true,
  });
  const [leader, setLeader] = useState<Leader>('dot');
  const apply = async () => {
    onClose();
    await run({
      cmd: 'insertReferenceTable',
      instr: toaInstr(o),
      leader: o.rightAlign ? leader : undefined,
    });
  };
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('refDialogs.toaTitle')}
      width={460}
      testId="toa-dialog"
      footer={
        <Buttons onClose={onClose}>
          <Ok testId="toa-ok" onClick={() => void apply()} />
        </Buttons>
      }
    >
      <div className={styles.formGrid}>
        <CategorySelect
          value={o.category}
          all
          onChange={(category) => setO({ ...o, category })}
          testId="toa-category"
        />
        <Check
          label={t('refDialogs.passim')}
          checked={o.passim}
          testId="toa-passim"
          onChange={(passim) => setO({ ...o, passim })}
        />
        <Check
          label={t('refDialogs.categoryHeadings')}
          checked={o.headings}
          testId="toa-headings"
          onChange={(headings) => setO({ ...o, headings })}
        />
        <Check
          label={t('refDialogs.rightAlign')}
          checked={o.rightAlign}
          testId="toa-right"
          onChange={(rightAlign) => setO({ ...o, rightAlign })}
        />
        <LeaderSelect
          value={leader}
          onChange={setLeader}
          disabled={!o.rightAlign}
          testId="toa-leader"
        />
      </div>
    </Dialog>
  );
}

type Conversion = 'toEndnotes' | 'toFootnotes' | 'swap';

/** Convert footnotes ↔ endnotes, all or those in the selection. */
export function ConvertNotesDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Conversion>('toEndnotes');
  const [selection, setSelection] = useState(false);
  const apply = async () => {
    onClose();
    await run({ cmd: 'convertNotes', mode, selection });
  };
  const radio = (m: Conversion, label: string) => (
    <label className={styles.field}>
      <input
        type="radio"
        name="convert-mode"
        data-testid={`convert-${m}`}
        checked={mode === m}
        onChange={() => setMode(m)}
      />
      <span>{label}</span>
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('refDialogs.convertTitle')}
      width={460}
      testId="convert-notes-dialog"
      footer={
        <Buttons onClose={onClose}>
          <Ok testId="convert-ok" onClick={() => void apply()} />
        </Buttons>
      }
    >
      <div className={styles.formGrid}>
        {radio('toEndnotes', t('refDialogs.toEndnotes'))}
        {radio('toFootnotes', t('refDialogs.toFootnotes'))}
        {radio('swap', t('refDialogs.swap'))}
        <label className={styles.field}>
          <span>{t('refDialogs.scope')}</span>
          <select
            data-testid="convert-scope"
            value={selection ? 'selection' : 'all'}
            onChange={(e) => setSelection(e.target.value === 'selection')}
          >
            <option value="all">{t('refDialogs.scopeAll')}</option>
            <option value="selection">{t('refDialogs.scopeSelection')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

/** The dialog `which` of this file. */
export function ReferenceDialog({
  which,
  run,
  language,
  onClose,
}: {
  which: ReferenceDialogId;
  run: Run;
  language: string;
  onClose: () => void;
}) {
  switch (which) {
    case 'tableOfFigures':
      return <TableOfFiguresDialog run={run} onClose={onClose} />;
    case 'markEntry':
      return <MarkEntryDialog run={run} onClose={onClose} />;
    case 'insertIndex':
      return <IndexDialog run={run} language={language} onClose={onClose} />;
    case 'markCitation':
      return <MarkCitationDialog run={run} onClose={onClose} />;
    case 'insertToa':
      return <AuthoritiesDialog run={run} onClose={onClose} />;
    case 'convertNotes':
      return <ConvertNotesDialog run={run} onClose={onClose} />;
  }
}
