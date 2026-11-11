/**
 * Data tools dialogs (Data ▸ Data tools / Outline): Text to Columns, Remove Duplicates,
 * Consolidate and Subtotal, plus the circles drawn around invalid data. The dialogs only
 * collect options; the engine commands (`textToColumns`, `removeDuplicates`, `consolidate`,
 * `subtotal`, …) do the work, each as one undo step.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  CONSOLIDATE_FUNCTIONS,
  type CellAt,
  type ConsolidateFunction,
  type DateOrder,
  type DedupeResult,
  type SheetHost,
  type SheetRange,
  type SubtotalResult,
  type TextColumnFormat,
  type TextSplitMode,
  type TextSplitPreview,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState } from 'react';
import { colName, parseA1 } from './grid';
import styles from './SheetEditor.module.css';
import dt from './dataTools.module.css';

interface Common {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}

/** A cell typed as `B3`, `$B$3` or `Sheet2!B3`; `undefined` when it is not one. */
export function parseCellAt(text: string, sheets: string[], sheet: number): CellAt | undefined {
  const s = text.trim();
  const bang = s.lastIndexOf('!');
  let target = sheet;
  if (bang >= 0) {
    const name = s.slice(0, bang).replace(/^'|'$/g, '').replace(/''/g, "'");
    target = sheets.findIndex((x) => x.toLowerCase() === name.toLowerCase());
    if (target < 0) return undefined;
  }
  const sel = parseA1((bang >= 0 ? s.slice(bang + 1) : s).replace(/\$/g, ''));
  return sel ? { sheet: target, row: sel.anchor.row, col: sel.anchor.col } : undefined;
}

/** Positions typed as `4, 12` (fixed-width break lines); `null` when one is not a number. */
export function parseBreaks(text: string): number[] | null {
  const parts = text
    .split(/[,;\s]+/)
    .map((p) => p.trim())
    .filter((p) => p !== '');
  const out = parts.map(Number);
  return out.every((n) => Number.isInteger(n) && n > 0) ? out : null;
}

/** Engine error code after `prefix: ` (e.g. `subtotal: noData` → `noData`). */
function errorCode(e: unknown, prefix: string): string | null {
  const m = new RegExp(`${prefix}: (\\w+)`).exec(String(e instanceof Error ? e.message : e));
  return m ? (m[1] ?? null) : null;
}

const DATE_ORDERS: DateOrder[] = ['dmy', 'mdy', 'ymd', 'dym', 'myd', 'ydm'];

type FormatChoice = 'general' | 'text' | 'skip' | DateOrder;

const toFormat = (c: FormatChoice): TextColumnFormat =>
  c === 'general' || c === 'text' || c === 'skip' ? c : { date: c };

function Footer({
  onClose,
  onOk,
  testId,
}: {
  onClose: () => void;
  onOk: () => void;
  testId: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.btn} onClick={onClose}>
        {t('sheetEditor.dlg.cancel')}
      </button>
      <button type="button" className={styles.btnPrimary} data-testid={testId} onClick={onOk}>
        {t('sheetEditor.dlg.ok')}
      </button>
    </>
  );
}

// ---------------------------------------------------------------- Text to Columns

/**
 * Text to Columns as the three-step wizard spreadsheet users know: 1 how the text is
 * divided, 2 the separators (or break positions), 3 the format of each column and where
 * the result goes. Back / Next move between the steps, Finish splits from any step.
 */
export function TextToColumnsDialog({
  host,
  sheet,
  sheets,
  range,
  onClose,
  onDone,
  fail,
}: Common & { sheets: string[]; range: SheetRange }) {
  const { t } = useTranslation();
  const tl = (k: string, o?: Record<string, unknown>) => t(`sheetEditor.dataTools.split.${k}`, o);
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [mode, setMode] = useState<'delimited' | 'fixed'>('delimited');
  const [delims, setDelims] = useState({ tab: true, semicolon: false, comma: false, space: false });
  const [other, setOther] = useState('');
  const [consecutive, setConsecutive] = useState(false);
  const [qualifier, setQualifier] = useState<'"' | "'" | ''>('"');
  const [breaks, setBreaks] = useState('');
  const [formats, setFormats] = useState<FormatChoice[]>([]);
  const [selected, setSelected] = useState(0);
  const [dateOrder, setDateOrder] = useState<DateOrder>('dmy');
  const [dest, setDest] = useState(`${colName(range.c0)}${range.r0 + 1}`);
  const [preview, setPreview] = useState<TextSplitPreview | null>(null);
  const [error, setError] = useState('');

  const split = (): TextSplitMode | null => {
    if (mode === 'fixed') {
      const b = parseBreaks(breaks);
      return b ? { mode: 'fixed', breaks: b } : null;
    }
    const d: string[] = [];
    if (delims.tab) d.push('\t');
    if (delims.semicolon) d.push(';');
    if (delims.comma) d.push(',');
    if (delims.space) d.push(' ');
    const o = [...other][0];
    if (o) d.push(o);
    return { mode: 'delimited', delimiters: d, consecutive, qualifier: qualifier || null };
  };
  // Step 1 previews the text as it is; the later steps the split.
  const spec: TextSplitMode | null =
    step === 1
      ? { mode: 'delimited', delimiters: [], consecutive: false, qualifier: null }
      : split();
  const key = JSON.stringify(spec);

  useEffect(() => {
    if (!spec) return;
    let cancelled = false;
    host
      .exec<TextSplitPreview>({
        cmd: 'textToColumns',
        sheet,
        range,
        split: spec,
        preview: true,
      })
      .then((p) => {
        if (!cancelled) setPreview(p);
      })
      .catch(() => {
        if (!cancelled) setPreview(null);
      });
    return () => {
      cancelled = true;
    };
    // `key` stands for `spec`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [host, sheet, range, key]);

  const columns = preview?.columns ?? 0;
  const formatOf = (i: number): FormatChoice => formats[i] ?? 'general';
  const setFormat = (i: number, f: FormatChoice) =>
    setFormats((list) => {
      const out = Array.from({ length: Math.max(columns, i + 1) }, (_, j) => list[j] ?? 'general');
      out[i] = f;
      return out;
    });
  const isDate = (f: FormatChoice): f is DateOrder =>
    f !== 'general' && f !== 'text' && f !== 'skip';
  const formatName = (f: FormatChoice) =>
    isDate(f) ? tl('date', { order: f.toUpperCase() }) : tl(f);
  const current = formatOf(selected);

  const next = () => {
    if (step === 2 && !split()) {
      setError(tl('badBreaks'));
      return;
    }
    setError('');
    setStep((s) => (s === 1 ? 2 : 3));
  };

  const finish = async () => {
    const s = split();
    if (!s) {
      setError(tl('badBreaks'));
      return;
    }
    const at = parseCellAt(dest, sheets, sheet);
    if (!at) {
      setError(tl('badDest'));
      return;
    }
    try {
      // The column count of the split as finished (step 1 previews the raw text).
      const p =
        step === 3 && preview
          ? preview
          : await host.exec<TextSplitPreview>({
              cmd: 'textToColumns',
              sheet,
              range,
              split: s,
              preview: true,
            });
      const n = p?.columns ?? 0;
      await host.exec({
        cmd: 'textToColumns',
        sheet,
        range,
        split: s,
        formats: Array.from({ length: n }, (_, i) => toFormat(formatOf(i))),
        dest: at,
      });
      await onDone();
      onClose();
    } catch (e) {
      const code = errorCode(e, 'textToColumns');
      if (code) setError(tl(`errors.${code}`));
      else fail(e);
    }
  };

  const check = (k: keyof typeof delims) => (
    <label className={styles.check} key={k}>
      <input
        type="checkbox"
        data-testid={`split-${k}`}
        checked={delims[k]}
        onChange={(e) => setDelims((d) => ({ ...d, [k]: e.target.checked }))}
      />{' '}
      {tl(k)}
    </label>
  );

  const radio = (value: 'general' | 'text' | 'date' | 'skip') => (
    <label className={styles.check} key={value}>
      <input
        type="radio"
        name="split-format"
        data-testid={`split-fmt-${value}`}
        checked={value === 'date' ? isDate(current) : current === value}
        onChange={() => setFormat(selected, value === 'date' ? dateOrder : value)}
      />{' '}
      {value === 'date' ? tl('dateKind') : tl(value)}
    </label>
  );

  const footer = (
    <>
      <button type="button" className={styles.btn} onClick={onClose}>
        {t('sheetEditor.dlg.cancel')}
      </button>
      <button
        type="button"
        className={styles.btn}
        data-testid="split-back"
        disabled={step === 1}
        onClick={() => {
          setError('');
          setStep((s) => (s === 3 ? 2 : 1));
        }}
      >
        {tl('back')}
      </button>
      <button
        type="button"
        className={styles.btn}
        data-testid="split-next"
        disabled={step === 3}
        onClick={next}
      >
        {tl('next')}
      </button>
      <button
        type="button"
        className={styles.btnPrimary}
        data-testid="split-ok"
        onClick={() => void finish()}
      >
        {tl('finish')}
      </button>
    </>
  );

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={tl('stepTitle', { step })}
      width={600}
      testId="split-dialog"
      footer={footer}
    >
      <div className={styles.form} data-step={step}>
        {step === 1 && (
          <fieldset>
            <legend>{tl('kind')}</legend>
            <label className={styles.check}>
              <input
                type="radio"
                name="split-mode"
                data-testid="split-delimited"
                checked={mode === 'delimited'}
                onChange={() => setMode('delimited')}
              />{' '}
              {tl('delimited')}
            </label>
            <label className={styles.check}>
              <input
                type="radio"
                name="split-mode"
                data-testid="split-fixed"
                checked={mode === 'fixed'}
                onChange={() => setMode('fixed')}
              />{' '}
              {tl('fixed')}
            </label>
          </fieldset>
        )}
        {step === 2 && mode === 'delimited' && (
          <fieldset>
            <legend>{tl('delimiters')}</legend>
            {check('tab')}
            {check('semicolon')}
            {check('comma')}
            {check('space')}
            <label>
              {tl('other')}
              <input
                data-testid="split-other"
                maxLength={1}
                value={other}
                onChange={(e) => setOther(e.target.value)}
              />
            </label>
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="split-consecutive"
                checked={consecutive}
                onChange={(e) => setConsecutive(e.target.checked)}
              />{' '}
              {tl('consecutive')}
            </label>
            <label>
              {tl('qualifier')}
              <select
                data-testid="split-qualifier"
                value={qualifier}
                onChange={(e) => setQualifier(e.target.value as '"' | "'" | '')}
              >
                <option value={'"'}>&quot;</option>
                <option value="'">&apos;</option>
                <option value="">{tl('none')}</option>
              </select>
            </label>
          </fieldset>
        )}
        {step === 2 && mode === 'fixed' && (
          <label>
            {tl('breaks')}
            <input
              data-testid="split-breaks"
              placeholder="4, 12"
              value={breaks}
              onChange={(e) => {
                setBreaks(e.target.value);
                setError('');
              }}
            />
          </label>
        )}
        {step === 3 && (
          <>
            <fieldset>
              <legend>{tl('columnFormat')}</legend>
              {radio('general')}
              {radio('text')}
              <div className={dt.toolbar}>
                {radio('date')}
                <select
                  aria-label={tl('dateOrder')}
                  data-testid="split-date-order"
                  value={dateOrder}
                  onChange={(e) => {
                    const o = e.target.value as DateOrder;
                    setDateOrder(o);
                    if (isDate(current)) setFormat(selected, o);
                  }}
                >
                  {DATE_ORDERS.map((o) => (
                    <option key={o} value={o}>
                      {o.toUpperCase()}
                    </option>
                  ))}
                </select>
              </div>
              {radio('skip')}
            </fieldset>
            <label>
              {tl('dest')}
              <input
                data-testid="split-dest"
                value={dest}
                onChange={(e) => {
                  setDest(e.target.value);
                  setError('');
                }}
              />
            </label>
          </>
        )}
        {preview && columns > 0 && (
          <div className={dt.previewBox}>
            <table className={dt.preview} data-testid="split-preview">
              {step === 3 && (
                <thead>
                  <tr>
                    {Array.from({ length: columns }, (_, i) => (
                      <th key={i} className={i === selected ? dt.selectedCol : undefined}>
                        <button
                          type="button"
                          className={dt.colHead}
                          data-testid={`split-col-${i}`}
                          aria-pressed={i === selected}
                          onClick={() => setSelected(i)}
                        >
                          {formatName(formatOf(i))}
                        </button>
                      </th>
                    ))}
                  </tr>
                </thead>
              )}
              <tbody>
                {preview.rows.map((row, r) => (
                  <tr key={r}>
                    {Array.from({ length: columns }, (_, i) => (
                      <td
                        key={i}
                        className={
                          step === 3 && formatOf(i) === 'skip'
                            ? dt.skipped
                            : step === 3 && i === selected
                              ? dt.selectedCol
                              : undefined
                        }
                      >
                        {row[i] ?? ''}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {error && (
          <p role="alert" data-testid="split-error">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- list helpers

/** The list around `range` (engine `region`) and the captions of its columns. */
function useList(host: SheetHost, sheet: number, range: SheetRange) {
  const [list, setList] = useState<SheetRange | null>(null);
  const [heads, setHeads] = useState<string[]>([]);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const r = await host.exec<SheetRange>({ cmd: 'region', sheet, range });
      const cells = await Promise.all(
        Array.from({ length: Math.min(r.c1 - r.c0 + 1, 256) }, (_, i) =>
          host.cell(sheet, r.r0, r.c0 + i),
        ),
      );
      if (cancelled) return;
      setList(r);
      setHeads(cells.map((c) => c.display));
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, range]);
  return { list, heads };
}

// ---------------------------------------------------------------- Remove Duplicates

export function RemoveDuplicatesDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
  onResult,
}: Common & { range: SheetRange; onResult: (r: DedupeResult) => void }) {
  const { t } = useTranslation();
  const tl = (k: string) => t(`sheetEditor.dataTools.dedupe.${k}`);
  const { list, heads } = useList(host, sheet, range);
  const [header, setHeader] = useState(true);
  const [off, setOff] = useState<Set<number>>(new Set());
  const [error, setError] = useState('');
  const cols = list ? Array.from({ length: heads.length }, (_, i) => list.c0 + i) : [];
  const label = (c: number, i: number) =>
    header && heads[i] ? heads[i] : t('sheetEditor.dataTools.columnN', { name: colName(c) });

  const ok = async () => {
    if (!list) return;
    const chosen = cols.filter((c) => !off.has(c));
    if (chosen.length === 0) {
      setError(tl('noColumns'));
      return;
    }
    try {
      const r = await host.exec<DedupeResult>({
        cmd: 'removeDuplicates',
        sheet,
        range: list,
        columns: chosen,
        header,
      });
      await onDone();
      onClose();
      onResult(r);
    } catch (e) {
      fail(e);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={tl('title')}
      width={400}
      testId="dedupe-dialog"
      footer={<Footer onClose={onClose} onOk={() => void ok()} testId="dedupe-ok" />}
    >
      <div className={styles.form}>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="dedupe-header"
            checked={header}
            onChange={(e) => setHeader(e.target.checked)}
          />{' '}
          {t('sheetEditor.dataTools.hasHeader')}
        </label>
        <div className={dt.toolbar}>
          <button type="button" className={styles.btn} onClick={() => setOff(new Set())}>
            {tl('selectAll')}
          </button>
          <button type="button" className={styles.btn} onClick={() => setOff(new Set(cols))}>
            {tl('selectNone')}
          </button>
        </div>
        <fieldset className={dt.columns}>
          <legend>{tl('columns')}</legend>
          {cols.map((c, i) => (
            <label className={styles.check} key={c}>
              <input
                type="checkbox"
                data-testid={`dedupe-col-${c}`}
                checked={!off.has(c)}
                onChange={(e) =>
                  setOff((s) => {
                    const n = new Set(s);
                    if (e.target.checked) n.delete(c);
                    else n.add(c);
                    return n;
                  })
                }
              />{' '}
              {label(c, i)}
            </label>
          ))}
        </fieldset>
        {error && (
          <p role="alert" data-testid="dedupe-error">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- Consolidate

function FunctionSelect({
  value,
  onChange,
  testId,
}: {
  value: ConsolidateFunction;
  onChange: (f: ConsolidateFunction) => void;
  testId: string;
}) {
  const { t } = useTranslation();
  return (
    <select
      data-testid={testId}
      value={value}
      onChange={(e) => onChange(e.target.value as ConsolidateFunction)}
    >
      {CONSOLIDATE_FUNCTIONS.map((f) => (
        <option key={f} value={f}>
          {t(`sheetEditor.dataTools.fn.${f}`)}
        </option>
      ))}
    </select>
  );
}

export function ConsolidateDialog({
  host,
  sheet,
  sheets,
  selection,
  onClose,
  onDone,
  fail,
}: Common & { sheets: string[]; selection: string }) {
  const { t } = useTranslation();
  const tl = (k: string) => t(`sheetEditor.dataTools.consolidate.${k}`);
  const [fn, setFn] = useState<ConsolidateFunction>('sum');
  const [ref, setRef] = useState(selection);
  const [sources, setSources] = useState<string[]>([]);
  const [topRow, setTopRow] = useState(false);
  const [leftCol, setLeftCol] = useState(false);
  const [links, setLinks] = useState(false);
  const [dest, setDest] = useState('');
  const [error, setError] = useState('');

  const add = () => {
    const r = ref.trim();
    if (r && !sources.some((s) => s.toLowerCase() === r.toLowerCase())) setSources([...sources, r]);
  };

  const ok = async () => {
    const all = [...sources];
    const r = ref.trim();
    if (all.length === 0 && r) all.push(r);
    if (all.length === 0) {
      setError(tl('errors.noSources'));
      return;
    }
    const at = parseCellAt(dest, sheets, sheet);
    if (!at) {
      setError(tl('errors.badDest'));
      return;
    }
    try {
      await host.exec({
        cmd: 'consolidate',
        sheet,
        sources: all,
        function: fn,
        topRow,
        leftCol,
        links,
        dest: at,
      });
      await onDone();
      onClose();
    } catch (e) {
      const code = errorCode(e, 'consolidate');
      if (code) setError(tl(`errors.${code}`));
      else fail(e);
    }
  };

  const box = (testId: string, v: boolean, set: (b: boolean) => void, label: string) => (
    <label className={styles.check}>
      <input
        type="checkbox"
        data-testid={testId}
        checked={v}
        onChange={(e) => set(e.target.checked)}
      />{' '}
      {label}
    </label>
  );

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={tl('title')}
      width={460}
      testId="consolidate-dialog"
      footer={<Footer onClose={onClose} onOk={() => void ok()} testId="consolidate-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.dataTools.function')}
          <FunctionSelect value={fn} onChange={setFn} testId="consolidate-fn" />
        </label>
        <label>
          {tl('reference')}
          <input
            data-testid="consolidate-ref"
            value={ref}
            onChange={(e) => {
              setRef(e.target.value);
              setError('');
            }}
          />
        </label>
        <div className={dt.toolbar}>
          <button type="button" className={styles.btn} data-testid="consolidate-add" onClick={add}>
            {tl('add')}
          </button>
        </div>
        <ul className={dt.sources} data-testid="consolidate-sources" aria-label={tl('sources')}>
          {sources.map((s) => (
            <li key={s}>
              <span>{s}</span>
              <button
                type="button"
                className={styles.btn}
                aria-label={tl('remove')}
                title={tl('remove')}
                onClick={() => setSources(sources.filter((x) => x !== s))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
        <fieldset>
          <legend>{tl('labels')}</legend>
          {box('consolidate-top', topRow, setTopRow, tl('topRow'))}
          {box('consolidate-left', leftCol, setLeftCol, tl('leftCol'))}
        </fieldset>
        {box('consolidate-links', links, setLinks, tl('links'))}
        <label>
          {tl('dest')}
          <input
            data-testid="consolidate-dest"
            value={dest}
            onChange={(e) => {
              setDest(e.target.value);
              setError('');
            }}
          />
        </label>
        {error && (
          <p role="alert" data-testid="consolidate-error">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- Subtotal

export function SubtotalDialog({
  host,
  sheet,
  range,
  onClose,
  onDone,
  fail,
}: Common & { range: SheetRange }) {
  const { t } = useTranslation();
  const tl = (k: string) => t(`sheetEditor.dataTools.subtotal.${k}`);
  const { list, heads } = useList(host, sheet, range);
  const [groupBy, setGroupBy] = useState<number | null>(null);
  const [fn, setFn] = useState<ConsolidateFunction>('sum');
  const [chosen, setChosen] = useState<Set<number> | null>(null);
  const [replace, setReplace] = useState(true);
  const [breaks, setBreaks] = useState(false);
  const [below, setBelow] = useState(true);
  const [error, setError] = useState('');
  const cols = list ? Array.from({ length: heads.length }, (_, i) => list.c0 + i) : [];
  const label = (c: number) =>
    heads[c - (list?.c0 ?? 0)] || t('sheetEditor.dataTools.columnN', { name: colName(c) });
  const by = groupBy ?? list?.c0 ?? 0;
  // Default: the last column gets the subtotal.
  const sel = chosen ?? new Set(list ? [list.c1] : []);

  const run = async (cmd: 'subtotal' | 'removeSubtotals') => {
    if (!list) return;
    if (cmd === 'subtotal' && sel.size === 0) {
      setError(tl('errors.noColumns'));
      return;
    }
    try {
      if (cmd === 'subtotal') {
        await host.exec<SubtotalResult>({
          cmd: 'subtotal',
          sheet,
          range: list,
          groupBy: by,
          function: fn,
          columns: [...sel].sort((a, b) => a - b),
          replace,
          pageBreaks: breaks,
          summaryBelow: below,
          label: tl('rowLabel'),
          grandLabel: tl('grandLabel'),
        });
      } else {
        await host.exec({ cmd: 'removeSubtotals', sheet, range: list });
      }
      await onDone();
      onClose();
    } catch (e) {
      const code = errorCode(e, 'subtotal');
      if (code) setError(tl(`errors.${code}`));
      else fail(e);
    }
  };

  const box = (testId: string, v: boolean, set: (b: boolean) => void, text: string) => (
    <label className={styles.check}>
      <input
        type="checkbox"
        data-testid={testId}
        checked={v}
        onChange={(e) => set(e.target.checked)}
      />{' '}
      {text}
    </label>
  );

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={tl('title')}
      width={420}
      testId="subtotal-dialog"
      footer={
        <>
          <button
            type="button"
            className={styles.btn}
            data-testid="subtotal-remove"
            onClick={() => void run('removeSubtotals')}
          >
            {tl('removeAll')}
          </button>
          <Footer onClose={onClose} onOk={() => void run('subtotal')} testId="subtotal-ok" />
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {tl('groupBy')}
          <select
            data-testid="subtotal-by"
            value={by}
            onChange={(e) => setGroupBy(Number(e.target.value))}
          >
            {cols.map((c) => (
              <option key={c} value={c}>
                {label(c)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('sheetEditor.dataTools.function')}
          <FunctionSelect value={fn} onChange={setFn} testId="subtotal-fn" />
        </label>
        <fieldset className={dt.columns}>
          <legend>{tl('addTo')}</legend>
          {cols.map((c) => (
            <label className={styles.check} key={c}>
              <input
                type="checkbox"
                data-testid={`subtotal-col-${c}`}
                checked={sel.has(c)}
                onChange={(e) => {
                  const n = new Set(sel);
                  if (e.target.checked) n.add(c);
                  else n.delete(c);
                  setChosen(n);
                  setError('');
                }}
              />{' '}
              {label(c)}
            </label>
          ))}
        </fieldset>
        {box('subtotal-replace', replace, setReplace, tl('replace'))}
        {box('subtotal-breaks', breaks, setBreaks, tl('pageBreaks'))}
        {box('subtotal-below', below, setBelow, tl('below'))}
        {error && (
          <p role="alert" data-testid="subtotal-error">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

// ---------------------------------------------------------------- invalid data circles

/** A rectangle in CSS pixels. */
export interface CircleRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Most circles drawn at once. */
export const MAX_CIRCLES = 1000;

/** Ellipses around invalid cells (positions from the host, in CSS pixels). */
export function InvalidCircles({ rects }: { rects: CircleRect[] }) {
  return (
    <>
      {rects.map((r, i) => (
        <div
          key={i}
          className={dt.circle}
          data-testid="invalid-circle"
          style={{ left: r.x - 3, top: r.y - 2, width: r.w + 6, height: r.h + 4 }}
        />
      ))}
    </>
  );
}
