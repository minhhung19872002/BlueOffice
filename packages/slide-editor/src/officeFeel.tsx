import type { SlideFormatState, SlideHfSpec, SlideTextFormat } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from 'react';
import styles from './officeFeel.module.css';

// Everyday dialogs and pickers of BlueSlide that work the way presentation programs of the
// OOXML family do (docs/parity/office-behaviour-slide.md, pass 2): the table size grid, the
// font dialog, paste special, header & footer and the rulers. Wording and look are BlueOffice's.

/** Superscript / subscript baseline shifts (1/1000 %), as the reference writes them. */
export const SUPERSCRIPT = 30000;
export const SUBSCRIPT = -25000;

// ---------------------------------------------------------------- table size grid

const GRID_COLS = 10;
const GRID_ROWS = 8;

/**
 * Insert › Table: a grid of cells under the button; pointing at a cell shows "columns × rows",
 * a click inserts that table. Arrow keys move, Enter inserts, Esc closes. Below: the number of
 * columns and rows typed in (the "Insert table…" dialog of the reference).
 */
export function TableSizePicker({
  anchor,
  onPick,
  onClose,
}: {
  anchor: { x: number; y: number } | null;
  onPick: (rows: number, cols: number) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [at, setAt] = useState<[number, number]>([1, 1]);
  const [typed, setTyped] = useState({ rows: 2, cols: 5 });
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    ref.current?.querySelector<HTMLElement>('[data-cell="1x1"]')?.focus();
  }, []);
  useEffect(() => {
    const down = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    window.addEventListener('pointerdown', down, true);
    return () => window.removeEventListener('pointerdown', down, true);
  }, [onClose]);
  const [cols, rows] = at;
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const move: Record<string, [number, number]> = {
      ArrowRight: [1, 0],
      ArrowLeft: [-1, 0],
      ArrowDown: [0, 1],
      ArrowUp: [0, -1],
    };
    const m = move[e.key];
    if (m && (e.target as HTMLElement).dataset.cell) {
      e.preventDefault();
      const next: [number, number] = [
        Math.max(1, Math.min(GRID_COLS, cols + m[0])),
        Math.max(1, Math.min(GRID_ROWS, rows + m[1])),
      ];
      setAt(next);
      ref.current?.querySelector<HTMLElement>(`[data-cell="${next[0]}x${next[1]}"]`)?.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  };
  return (
    <div
      ref={ref}
      className={styles.tablePicker}
      role="dialog"
      aria-label={t('slideFeel.table.title')}
      data-testid="table-size-picker"
      style={anchor ? { left: anchor.x, top: anchor.y } : undefined}
      onKeyDown={onKey}
    >
      <div className={styles.tableLabel} data-testid="table-size-label" aria-live="polite">
        {t('slideFeel.table.size', { cols, rows })}
      </div>
      <div className={styles.tableGrid} role="grid" aria-label={t('slideFeel.table.title')}>
        {Array.from({ length: GRID_ROWS }, (_, r) => (
          <div key={r} role="row" className={styles.tableRow}>
            {Array.from({ length: GRID_COLS }, (_, c) => {
              const on = c < cols && r < rows;
              return (
                <button
                  key={c}
                  type="button"
                  role="gridcell"
                  tabIndex={c + 1 === cols && r + 1 === rows ? 0 : -1}
                  data-cell={`${c + 1}x${r + 1}`}
                  data-on={on ? 'true' : undefined}
                  className={styles.tableCell}
                  aria-label={t('slideFeel.table.size', { cols: c + 1, rows: r + 1 })}
                  onPointerEnter={() => setAt([c + 1, r + 1])}
                  onFocus={() => setAt([c + 1, r + 1])}
                  onClick={() => onPick(r + 1, c + 1)}
                />
              );
            })}
          </div>
        ))}
      </div>
      <div className={styles.tableTyped}>
        <label>
          {t('slideFeel.table.cols')}
          <input
            type="number"
            min={1}
            max={75}
            value={typed.cols}
            data-testid="table-cols"
            onChange={(e) => setTyped((x) => ({ ...x, cols: Number(e.target.value) || 1 }))}
          />
        </label>
        <label>
          {t('slideFeel.table.rows')}
          <input
            type="number"
            min={1}
            max={75}
            value={typed.rows}
            data-testid="table-rows"
            onChange={(e) => setTyped((x) => ({ ...x, rows: Number(e.target.value) || 1 }))}
          />
        </label>
        <button
          type="button"
          data-testid="table-insert-typed"
          onClick={() =>
            onPick(Math.max(1, Math.min(75, typed.rows)), Math.max(1, Math.min(75, typed.cols)))
          }
        >
          {t('slideFeel.table.insert')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- font dialog

export interface FontChoice {
  font: string;
  bold: boolean;
  italic: boolean;
  size: number | null;
  color: string | null;
  underline: boolean;
  strike: boolean;
  doubleStrike: boolean;
  /** Baseline shift (1/1000 %): > 0 superscript, < 0 subscript. */
  baseline: number;
  caps: 'small' | 'all' | null;
  spacing: number;
}

/** The dialog's starting values from the format state of the selection. */
export function fontChoiceOf(f: SlideFormatState | undefined): FontChoice {
  return {
    font: f?.font ?? '',
    bold: !!f?.bold,
    italic: !!f?.italic,
    size: f?.size ?? null,
    color: f?.color ?? null,
    underline: !!f?.underline,
    strike: !!f?.strike && !f?.doubleStrike,
    doubleStrike: !!f?.doubleStrike,
    baseline: f?.baseline ?? 0,
    caps: f?.caps ?? null,
    spacing: f?.spacing ?? 0,
  };
}

/** The formats to apply for what the dialog changed (unchanged fields stay mixed / as they are). */
export function fontChanges(from: FontChoice, to: FontChoice): SlideTextFormat[] {
  const out: SlideTextFormat[] = [];
  if (to.font && to.font !== from.font) out.push({ kind: 'font', typeface: to.font });
  if (to.bold !== from.bold) out.push({ kind: 'bold', value: to.bold });
  if (to.italic !== from.italic) out.push({ kind: 'italic', value: to.italic });
  if (to.size && to.size !== from.size) out.push({ kind: 'size', points: to.size });
  if (to.color && to.color !== from.color) out.push({ kind: 'color', hex: to.color });
  if (to.underline !== from.underline) out.push({ kind: 'underline', value: to.underline });
  if (to.doubleStrike !== from.doubleStrike && to.doubleStrike) {
    out.push({ kind: 'doubleStrike', value: true });
  } else if (to.strike !== from.strike || (from.doubleStrike && !to.doubleStrike)) {
    out.push({ kind: 'strike', value: to.strike });
  }
  if (to.baseline !== from.baseline) out.push({ kind: 'baseline', value: to.baseline });
  if (to.caps !== from.caps) out.push({ kind: 'caps', value: to.caps });
  if (to.spacing !== from.spacing) out.push({ kind: 'spacing', points: to.spacing });
  return out;
}

/** Format › Font (Ctrl+T, Ctrl+Shift+F): font, style, size, colour, effects, spacing. */
export function FontDialog({
  initial,
  fonts,
  onApply,
  onClose,
}: {
  initial: FontChoice;
  fonts: readonly string[];
  onApply: (changes: SlideTextFormat[]) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [v, setV] = useState(initial);
  const set = (p: Partial<FontChoice>) => setV((x) => ({ ...x, ...p }));
  const style = v.bold ? (v.italic ? 'boldItalic' : 'bold') : v.italic ? 'italic' : 'regular';
  const offset = Math.round(v.baseline / 1000);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideFeel.font.title')}
      testId="font-dialog"
      width={480}
      footer={
        <>
          <button
            type="button"
            data-testid="font-dialog-ok"
            onClick={() => onApply(fontChanges(initial, v))}
          >
            {t('slideFeel.ok')}
          </button>
          <button type="button" onClick={onClose}>
            {t('slideFeel.cancel')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('slideFeel.font.face')}
          <select
            data-testid="font-dialog-face"
            value={v.font}
            onChange={(e) => set({ font: e.target.value })}
          >
            {[...new Set([v.font, ...fonts])].map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('slideFeel.font.style')}
          <select
            data-testid="font-dialog-style"
            value={style}
            onChange={(e) => {
              const s = e.target.value;
              set({
                bold: s === 'bold' || s === 'boldItalic',
                italic: s.toLowerCase().includes('italic'),
              });
            }}
          >
            {(['regular', 'italic', 'bold', 'boldItalic'] as const).map((s) => (
              <option key={s} value={s}>
                {t(`slideFeel.font.styles.${s}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('slideFeel.font.size')}
          <input
            type="number"
            min={1}
            max={4000}
            step={0.5}
            data-testid="font-dialog-size"
            value={v.size ?? ''}
            onChange={(e) => set({ size: Number(e.target.value) || null })}
          />
        </label>
        <label>
          {t('slideFeel.font.color')}
          <input
            type="color"
            data-testid="font-dialog-color"
            value={`#${(v.color ?? '000000').toLowerCase()}`}
            onChange={(e) => set({ color: e.target.value.slice(1).toUpperCase() })}
          />
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-underline"
            checked={v.underline}
            onChange={(e) => set({ underline: e.target.checked })}
          />
          {t('slideFeel.font.underline')}
        </label>
      </div>
      <fieldset className={styles.effects}>
        <legend>{t('slideFeel.font.effects')}</legend>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-strike"
            checked={v.strike}
            onChange={(e) => set({ strike: e.target.checked, doubleStrike: false })}
          />
          {t('slideFeel.font.strike')}
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-double-strike"
            checked={v.doubleStrike}
            onChange={(e) => set({ doubleStrike: e.target.checked, strike: false })}
          />
          {t('slideFeel.font.doubleStrike')}
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-superscript"
            checked={v.baseline > 0}
            onChange={(e) => set({ baseline: e.target.checked ? SUPERSCRIPT : 0 })}
          />
          {t('slideFeel.font.superscript')}
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-subscript"
            checked={v.baseline < 0}
            onChange={(e) => set({ baseline: e.target.checked ? SUBSCRIPT : 0 })}
          />
          {t('slideFeel.font.subscript')}
        </label>
        <label>
          {t('slideFeel.font.offset')}
          <input
            type="number"
            min={-100}
            max={100}
            data-testid="font-dialog-offset"
            value={offset}
            disabled={v.baseline === 0}
            onChange={(e) => set({ baseline: (Number(e.target.value) || 0) * 1000 })}
          />
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-small-caps"
            checked={v.caps === 'small'}
            onChange={(e) => set({ caps: e.target.checked ? 'small' : null })}
          />
          {t('slideFeel.font.smallCaps')}
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="font-dialog-all-caps"
            checked={v.caps === 'all'}
            onChange={(e) => set({ caps: e.target.checked ? 'all' : null })}
          />
          {t('slideFeel.font.allCaps')}
        </label>
      </fieldset>
      <label className={styles.form}>
        {t('slideFeel.font.spacing')}
        <input
          type="number"
          step={0.1}
          min={-100}
          max={1584}
          data-testid="font-dialog-spacing"
          value={v.spacing}
          onChange={(e) => set({ spacing: Number(e.target.value) || 0 })}
        />
      </label>
    </Dialog>
  );
}

// ---------------------------------------------------------------- paste special

export type PasteSpecialChoice = 'theme' | 'keep' | 'picture' | 'text';

/** Edit › Paste special (Ctrl+Alt+V): the forms the clipboard can be pasted as. */
export function PasteSpecialDialog({
  choices,
  onPaste,
  onClose,
}: {
  choices: PasteSpecialChoice[];
  onPaste: (choice: PasteSpecialChoice) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [pick, setPick] = useState<PasteSpecialChoice | null>(choices[0] ?? null);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideFeel.pasteSpecial.title')}
      testId="paste-special-dialog"
      footer={
        <>
          <button
            type="button"
            data-testid="paste-special-ok"
            disabled={!pick}
            onClick={() => pick && onPaste(pick)}
          >
            {t('slideFeel.ok')}
          </button>
          <button type="button" onClick={onClose}>
            {t('slideFeel.cancel')}
          </button>
        </>
      }
    >
      {choices.length === 0 ? (
        <p>{t('slideFeel.pasteSpecial.empty')}</p>
      ) : (
        <div
          role="radiogroup"
          aria-label={t('slideFeel.pasteSpecial.as')}
          className={styles.radios}
        >
          {choices.map((c) => (
            <label key={c} className={styles.check}>
              <input
                type="radio"
                name="paste-special"
                value={c}
                data-testid={`paste-special-${c}`}
                checked={pick === c}
                onChange={() => setPick(c)}
              />
              {t(`pasteOptions.slide.${c}`)}
            </label>
          ))}
        </div>
      )}
    </Dialog>
  );
}

// ---------------------------------------------------------------- header & footer

/**
 * Insert › Header & Footer: date and time (updated automatically or fixed), slide number,
 * footer text, "not on the title slide"; applied to the selected slides or to all of them.
 */
export function HeaderFooterDialog({
  initial,
  today,
  onApply,
  onClose,
}: {
  initial: SlideHfSpec;
  /** The current date as the user's language writes it. */
  today: string;
  onApply: (spec: SlideHfSpec, all: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [date, setDate] = useState(!!initial.date);
  const [auto, setAuto] = useState(initial.date?.auto ?? true);
  const [fixed, setFixed] = useState(
    initial.date && !initial.date.auto ? initial.date.text : today,
  );
  const [number, setNumber] = useState(initial.slideNumber);
  const [footer, setFooter] = useState(initial.footer !== undefined);
  const [text, setText] = useState(initial.footer ?? '');
  const [notOnTitle, setNotOnTitle] = useState(initial.notOnTitle);
  const spec = (): SlideHfSpec => ({
    ...(date ? { date: { text: auto ? today : fixed, auto } } : {}),
    slideNumber: number,
    ...(footer ? { footer: text } : {}),
    notOnTitle,
  });
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideFeel.hf.title')}
      testId="header-footer-dialog"
      width={460}
      footer={
        <>
          <button type="button" data-testid="hf-apply" onClick={() => onApply(spec(), false)}>
            {t('slideFeel.hf.apply')}
          </button>
          <button type="button" data-testid="hf-apply-all" onClick={() => onApply(spec(), true)}>
            {t('slideFeel.hf.applyAll')}
          </button>
          <button type="button" onClick={onClose}>
            {t('slideFeel.cancel')}
          </button>
        </>
      }
    >
      <fieldset className={styles.effects}>
        <legend>{t('slideFeel.hf.include')}</legend>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="hf-date"
            checked={date}
            onChange={(e) => setDate(e.target.checked)}
          />
          {t('slideFeel.hf.date')}
        </label>
        <div className={styles.indent} role="radiogroup" aria-label={t('slideFeel.hf.date')}>
          <label className={styles.check}>
            <input
              type="radio"
              name="hf-date"
              data-testid="hf-date-auto"
              disabled={!date}
              checked={auto}
              onChange={() => setAuto(true)}
            />
            {t('slideFeel.hf.auto', { date: today })}
          </label>
          <label className={styles.check}>
            <input
              type="radio"
              name="hf-date"
              data-testid="hf-date-fixed"
              disabled={!date}
              checked={!auto}
              onChange={() => setAuto(false)}
            />
            {t('slideFeel.hf.fixed')}
          </label>
          <input
            aria-label={t('slideFeel.hf.fixed')}
            data-testid="hf-date-text"
            disabled={!date || auto}
            value={fixed}
            onChange={(e) => setFixed(e.target.value)}
          />
        </div>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="hf-number"
            checked={number}
            onChange={(e) => setNumber(e.target.checked)}
          />
          {t('slideFeel.hf.number')}
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="hf-footer"
            checked={footer}
            onChange={(e) => setFooter(e.target.checked)}
          />
          {t('slideFeel.hf.footer')}
        </label>
        <input
          className={styles.indent}
          aria-label={t('slideFeel.hf.footerText')}
          data-testid="hf-footer-text"
          disabled={!footer}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
      </fieldset>
      <label className={styles.check}>
        <input
          type="checkbox"
          data-testid="hf-not-title"
          checked={notOnTitle}
          onChange={(e) => setNotOnTitle(e.target.checked)}
        />
        {t('slideFeel.hf.notOnTitle')}
      </label>
    </Dialog>
  );
}

// ---------------------------------------------------------------- rulers

/** Points per ruler unit: centimetres for Vietnamese, inches otherwise. */
export function rulerUnit(lang: string): { pt: number; minor: number } {
  return lang.toLowerCase().startsWith('en') ? { pt: 72, minor: 8 } : { pt: 72 / 2.54, minor: 2 };
}

/**
 * Horizontal or vertical ruler along the slide (View › Ruler): zero at the centre of the
 * slide, a number every unit, minor ticks between.
 */
export function SlideRuler({
  axis,
  length,
  points,
  lang,
}: {
  axis: 'h' | 'v';
  /** Length on screen, CSS pixels. */
  length: number;
  /** Length of the slide side, points. */
  points: number;
  lang: string;
}) {
  const { pt, minor } = rulerUnit(lang);
  const scale = points > 0 ? length / points : 1;
  const half = points / 2;
  const n = Math.floor(half / pt);
  const ticks: { at: number; label?: number; major: boolean }[] = [];
  for (let u = -n; u <= n; u++) {
    ticks.push({ at: (half + u * pt) * scale, label: Math.abs(u), major: true });
    for (let k = 1; k < minor; k++) {
      const p = half + (u + k / minor) * pt;
      if (p <= points && u < n) ticks.push({ at: p * scale, major: false });
    }
  }
  const h = axis === 'h';
  return (
    <svg
      className={h ? styles.rulerH : styles.rulerV}
      data-testid={h ? 'ruler-h' : 'ruler-v'}
      aria-hidden="true"
      width={h ? length : 18}
      height={h ? 18 : length}
    >
      {ticks.map((tk, i) => {
        const len = tk.major ? 7 : 3;
        return (
          <g key={i}>
            <line
              x1={h ? tk.at : 18 - len}
              x2={h ? tk.at : 18}
              y1={h ? 18 - len : tk.at}
              y2={h ? 18 : tk.at}
            />
            {tk.major && tk.label !== undefined && (
              <text
                x={h ? tk.at : 8}
                y={h ? 9 : tk.at + 3}
                textAnchor="middle"
                transform={h ? undefined : `rotate(-90 8 ${tk.at})`}
              >
                {tk.label}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}
