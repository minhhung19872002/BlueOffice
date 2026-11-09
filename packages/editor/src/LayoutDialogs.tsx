/**
 * Layout tab dialogs of BlueDoc: text columns (ECMA-376 Part 1, §17.6.4 cols),
 * line numbering (§17.6.8 lnNumType), automatic hyphenation (§17.15.1.10), the
 * page text direction of a section (§17.6.20 textDirection) and large initial
 * letters (drop caps, §17.3.1.11 framePr). Each dialog reads the current state with
 * `{ cmd: 'pageLayout' }` and sends one engine command (one undo step).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ColumnsSpec,
  DropCapSpec,
  EditCommand,
  EditResult,
  HyphenationSettings,
  LineNumbering,
  PageLayoutState,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState, type ReactNode } from 'react';
import { Length } from './DocDialogs';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Column layout presets. */
export const COLUMN_PRESETS = ['one', 'two', 'three', 'left', 'right'] as const;
export type ColumnPreset = (typeof COLUMN_PRESETS)[number];

/** Equal columns filling `textWidth` (twips), `space` apart. */
export function equalColumns(
  count: number,
  space: number,
  textWidth: number,
  separator = false,
): ColumnsSpec {
  const n = Math.max(1, Math.min(45, Math.round(count)));
  const w = Math.max(144, Math.floor((textWidth - space * (n - 1)) / n));
  return {
    count: n,
    space,
    separator: separator && n > 1,
    widths: Array.from({ length: n }, (_, i) => ({ width: w, space: i + 1 < n ? space : 0 })),
  };
}

/** A preset layout: one to three equal columns, or two columns with the narrow one
 * (a third of the text) on the left or the right. */
export function columnsPreset(
  preset: ColumnPreset,
  textWidth: number,
  space = 720,
  separator = false,
): ColumnsSpec {
  switch (preset) {
    case 'one':
      return equalColumns(1, space, textWidth);
    case 'two':
      return equalColumns(2, space, textWidth, separator);
    case 'three':
      return equalColumns(3, space, textWidth, separator);
    case 'left':
    case 'right': {
      const narrow = Math.round((textWidth - space) / 3);
      const wide = textWidth - space - narrow;
      const [a, b] = preset === 'left' ? [narrow, wide] : [wide, narrow];
      return {
        count: 2,
        space,
        separator,
        widths: [
          { width: a, space },
          { width: b, space: 0 },
        ],
      };
    }
  }
}

/** The preset `spec` matches, if any. */
export function presetOf(spec: ColumnsSpec, textWidth: number): ColumnPreset | null {
  return (
    COLUMN_PRESETS.find((p) => {
      const q = columnsPreset(p, textWidth, spec.space, spec.separator);
      return (
        q.count === spec.count &&
        q.widths.every(
          (w, i) =>
            Math.abs(w.width - (spec.widths[i]?.width ?? 0)) < 30 &&
            Math.abs(w.space - (spec.widths[i]?.space ?? 0)) < 30,
        )
      );
    }) ?? null
  );
}

/** The columns are all the same width with the same gaps. */
export function isEqual(spec: ColumnsSpec): boolean {
  const w = spec.widths;
  return (
    w.length !== spec.count ||
    w.every(
      (c, i) =>
        Math.abs(c.width - w[0]!.width) < 2 && (i + 1 === w.length || c.space === w[0]!.space),
    )
  );
}

/** Ask the engine for the Layout tab state. */
function useLayout(run: Run, onState?: (s: PageLayoutState) => void): PageLayoutState | null {
  const [state, setState] = useState<PageLayoutState | null>(null);
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'pageLayout' }).then((r) => {
      if (!alive || !r?.pageLayout) return;
      setState(r.pageLayout);
      onState?.(r.pageLayout);
    });
    return () => {
      alive = false;
    };
    // `onState` only seeds the form once.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [run]);
  return state;
}

function Footer({
  onClose,
  onApply,
  testId,
}: {
  onClose: () => void;
  onApply: () => void;
  testId: string;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.dialogButton} onClick={onClose}>
        {t('layout.cancel')}
      </button>
      <button
        type="button"
        className={styles.dialogPrimary}
        data-testid={`${testId}-ok`}
        onClick={onApply}
      >
        {t('layout.ok')}
      </button>
    </>
  );
}

const clampInt = (v: string, min: number, max: number) => {
  const n = Math.round(Number(v));
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : min;
};

/** Miniature page showing the columns of `spec` (for `textWidth` twips of text). */
export function ColumnsPreview({ spec, textWidth }: { spec: ColumnsSpec; textWidth: number }) {
  const { t } = useTranslation();
  const W = 100;
  const H = 130;
  const k = (W - 20) / Math.max(1, textWidth);
  const cols =
    spec.widths.length === spec.count
      ? spec.widths
      : equalColumns(spec.count, spec.space, textWidth).widths;
  const items: ReactNode[] = [];
  let x = 10;
  cols.forEach((c, i) => {
    const w = c.width * k;
    for (let r = 0; r < 8; r++) {
      items.push(
        <rect key={`${i}-${r}`} x={x} y={14 + r * 13} width={w} height={4} fill="#c8c8c8" />,
      );
    }
    if (spec.separator && i + 1 < cols.length) {
      const sx = x + w + (c.space * k) / 2;
      items.push(
        <line key={`s${i}`} x1={sx} y1={12} x2={sx} y2={H - 14} stroke="#555" strokeWidth={0.6} />,
      );
    }
    x += w + c.space * k;
  });
  return (
    <svg
      width={W}
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      role="img"
      aria-label={t('layout.columns.preview')}
      data-testid="columns-preview"
    >
      <rect x={0} y={0} width={W} height={H} fill="#ffffff" stroke="#c8c8c8" />
      {items}
    </svg>
  );
}

/** Text columns: presets, number, widths and gaps, separator line, where they apply. */
export function ColumnsDialog({
  run,
  language,
  onClose,
}: {
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [spec, setSpec] = useState<ColumnsSpec | null>(null);
  const [equal, setEqual] = useState(true);
  const [scope, setScope] = useState<'section' | 'document' | 'forward'>('section');
  const state = useLayout(run, (s) => {
    setSpec(s.columns);
    setEqual(isEqual(s.columns));
  });
  if (!state || !spec) return null;
  const tw = state.textWidth;
  const preset = presetOf(spec, tw);
  const setCount = (n: number) => setSpec(equalColumns(n, spec.space, tw, spec.separator));
  const setWidth = (i: number, patch: { width?: number; space?: number }) =>
    setSpec({
      ...spec,
      widths: spec.widths.map((w, j) => (j === i ? { ...w, ...patch } : w)),
    });
  const apply = () => {
    onClose();
    const columns = equal ? { ...spec, widths: [] } : spec;
    void run({ cmd: 'setColumns', columns, scope });
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('layout.columns.title')}
      width={600}
      testId="columns-dialog"
      footer={<Footer onClose={onClose} onApply={apply} testId="columns" />}
    >
      <div className={styles.formGrid}>
        <fieldset className={styles.fieldset}>
          <legend>{t('layout.columns.presets')}</legend>
          {COLUMN_PRESETS.map((p) => (
            <button
              key={p}
              type="button"
              aria-pressed={preset === p}
              data-testid={`columns-preset-${p}`}
              className={preset === p ? styles.dialogPrimary : styles.dialogButton}
              onClick={() => {
                const q = columnsPreset(p, tw, spec.space, spec.separator);
                setSpec(q);
                setEqual(p !== 'left' && p !== 'right');
              }}
            >
              {t(`layout.columns.${p}`)}
            </button>
          ))}
        </fieldset>
        <ColumnsPreview spec={spec} textWidth={tw} />
        <label className={styles.field}>
          <span>{t('layout.columns.count')}</span>
          <input
            type="number"
            min="1"
            max="45"
            data-testid="columns-count"
            value={spec.count}
            onChange={(e) => setCount(clampInt(e.target.value, 1, 45))}
          />
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="columns-equal"
            checked={equal}
            onChange={(e) => {
              setEqual(e.target.checked);
              if (e.target.checked)
                setSpec(equalColumns(spec.count, spec.space, tw, spec.separator));
            }}
          />
          <span>{t('layout.columns.equal')}</span>
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="columns-separator"
            checked={spec.separator}
            disabled={spec.count < 2}
            onChange={(e) => setSpec({ ...spec, separator: e.target.checked })}
          />
          <span>{t('layout.columns.separator')}</span>
        </label>
        {equal ? (
          <Length
            label={t('layout.columns.gap')}
            twips={spec.space}
            language={language}
            testId="columns-gap"
            onChange={(space) => setSpec(equalColumns(spec.count, space, tw, spec.separator))}
          />
        ) : (
          spec.widths.map((w, i) => (
            <fieldset key={i} className={styles.fieldset} data-testid={`columns-col-${i}`}>
              <legend>{t('layout.columns.column', { n: i + 1 })}</legend>
              <Length
                label={t('layout.columns.width')}
                twips={w.width}
                language={language}
                testId={`columns-width-${i}`}
                onChange={(width) => setWidth(i, { width })}
              />
              {i + 1 < spec.widths.length && (
                <Length
                  label={t('layout.columns.space')}
                  twips={w.space}
                  language={language}
                  onChange={(space) => setWidth(i, { space })}
                />
              )}
            </fieldset>
          ))
        )}
        <label className={styles.field}>
          <span>{t('layout.applyTo')}</span>
          <select
            data-testid="columns-scope"
            value={scope}
            onChange={(e) => setScope(e.target.value as typeof scope)}
          >
            <option value="section">{t('layout.thisSection')}</option>
            <option value="document">{t('layout.wholeDocument')}</option>
            <option value="forward">{t('layout.forward')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

type NumberMode = 'none' | LineNumbering['restart'];

/** The `setLineNumbering` command for a mode and options. */
export function lineNumberingCommand(
  mode: NumberMode,
  first: number,
  countBy: number,
  distance: number | null,
  scope: 'section' | 'document',
): EditCommand {
  return {
    cmd: 'setLineNumbering',
    scope,
    numbering:
      mode === 'none'
        ? null
        : {
            countBy: Math.max(1, Math.min(100, Math.round(countBy))),
            // Stored less one (§17.6.8 start as written by common word processors).
            start: Math.max(0, Math.round(first) - 1),
            distance,
            restart: mode,
          },
  };
}

/** Line numbering: off / continuous / per page / per section, first number, step, gap. */
export function LineNumbersDialog({
  run,
  language,
  onClose,
}: {
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<NumberMode>('none');
  const [first, setFirst] = useState(1);
  const [countBy, setCountBy] = useState(1);
  const [distance, setDistance] = useState<number | null>(null);
  const [suppress, setSuppress] = useState(false);
  const [scope, setScope] = useState<'section' | 'document'>('section');
  const state = useLayout(run, (s) => {
    const ln = s.lineNumbers;
    setSuppress(s.suppressLineNumbers);
    if (!ln) return;
    setMode(ln.restart);
    setFirst(ln.start + 1);
    setCountBy(ln.countBy);
    setDistance(ln.distance);
  });
  if (!state) return null;
  const apply = async () => {
    onClose();
    await run(lineNumberingCommand(mode, first, countBy, distance, scope));
    if (suppress !== state.suppressLineNumbers) {
      await run({
        cmd: 'formatParagraph',
        format: { kind: 'suppressLineNumbers', on: suppress },
      });
    }
  };
  const off = mode === 'none';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('layout.lineNumbers.title')}
      width={480}
      testId="line-numbers-dialog"
      footer={<Footer onClose={onClose} onApply={() => void apply()} testId="line-numbers" />}
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('layout.lineNumbers.mode')}</span>
          <select
            data-testid="line-numbers-mode"
            value={mode}
            onChange={(e) => setMode(e.target.value as NumberMode)}
          >
            {(['none', 'continuous', 'newPage', 'newSection'] as const).map((m) => (
              <option key={m} value={m}>
                {t(`layout.lineNumbers.${m}`)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('layout.lineNumbers.start')}</span>
          <input
            type="number"
            min="1"
            disabled={off}
            data-testid="line-numbers-start"
            value={first}
            onChange={(e) => setFirst(clampInt(e.target.value, 1, 32_767))}
          />
        </label>
        <label className={styles.field}>
          <span>{t('layout.lineNumbers.countBy')}</span>
          <input
            type="number"
            min="1"
            max="100"
            disabled={off}
            data-testid="line-numbers-count-by"
            value={countBy}
            onChange={(e) => setCountBy(clampInt(e.target.value, 1, 100))}
          />
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            disabled={off}
            data-testid="line-numbers-auto-distance"
            checked={distance === null}
            onChange={(e) => setDistance(e.target.checked ? null : 360)}
          />
          <span>{t('layout.lineNumbers.auto')}</span>
        </label>
        {distance !== null && (
          <Length
            label={t('layout.lineNumbers.distance')}
            twips={distance}
            language={language}
            testId="line-numbers-distance"
            onChange={setDistance}
          />
        )}
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="line-numbers-suppress"
            checked={suppress}
            onChange={(e) => setSuppress(e.target.checked)}
          />
          <span>{t('layout.lineNumbers.suppress')}</span>
        </label>
        <label className={styles.field}>
          <span>{t('layout.applyTo')}</span>
          <select
            data-testid="line-numbers-scope"
            value={scope}
            onChange={(e) => setScope(e.target.value as typeof scope)}
          >
            <option value="section">{t('layout.thisSection')}</option>
            <option value="document">{t('layout.wholeDocument')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

/** Automatic hyphenation: on/off, capitals, zone, consecutive limit, this paragraph. */
export function HyphenationDialog({
  run,
  language,
  onClose,
}: {
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [h, setH] = useState<HyphenationSettings>({
    auto: false,
    zone: null,
    consecutiveLimit: null,
    noCaps: false,
  });
  const [suppress, setSuppress] = useState(false);
  const state = useLayout(run, (s) => {
    setH(s.hyphenation);
    setSuppress(s.suppressAutoHyphens);
  });
  if (!state) return null;
  const apply = async () => {
    onClose();
    await run({ cmd: 'setHyphenation', hyphenation: h });
    if (suppress !== state.suppressAutoHyphens) {
      await run({
        cmd: 'formatParagraph',
        format: { kind: 'suppressAutoHyphens', on: suppress },
      });
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('layout.hyphenation.title')}
      width={500}
      testId="hyphenation-dialog"
      footer={<Footer onClose={onClose} onApply={() => void apply()} testId="hyphenation" />}
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="hyphenation-auto"
            checked={h.auto}
            onChange={(e) => setH({ ...h, auto: e.target.checked })}
          />
          <span>{t('layout.hyphenation.auto')}</span>
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="hyphenation-caps"
            checked={h.noCaps}
            onChange={(e) => setH({ ...h, noCaps: e.target.checked })}
          />
          <span>{t('layout.hyphenation.noCaps')}</span>
        </label>
        <Length
          label={t('layout.hyphenation.zone')}
          twips={h.zone ?? 360}
          language={language}
          testId="hyphenation-zone"
          onChange={(zone) => setH({ ...h, zone })}
        />
        <label className={styles.field}>
          <span>{t('layout.hyphenation.limit')}</span>
          <select
            data-testid="hyphenation-limit"
            value={h.consecutiveLimit ?? 0}
            onChange={(e) => {
              const n = Number(e.target.value);
              setH({ ...h, consecutiveLimit: n > 0 ? n : null });
            }}
          >
            <option value={0}>{t('layout.hyphenation.noLimit')}</option>
            {[1, 2, 3, 4, 5, 6, 8, 10].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="hyphenation-suppress"
            checked={suppress}
            onChange={(e) => setSuppress(e.target.checked)}
          />
          <span>{t('layout.hyphenation.suppress')}</span>
        </label>
        <div className={styles.hint}>{t('layout.hyphenation.note')}</div>
      </div>
    </Dialog>
  );
}

/** Page text direction of the caret's section or every section. */
export function TextDirectionDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [dir, setDir] = useState<'horizontal' | 'tbRl' | 'btLr'>('horizontal');
  const [scope, setScope] = useState<'section' | 'document'>('section');
  const state = useLayout(run, (s) => setDir(s.textDirection ?? 'horizontal'));
  if (!state) return null;
  const apply = () => {
    onClose();
    void run({
      cmd: 'setSectionTextDirection',
      dir: dir === 'horizontal' ? null : dir,
      scope,
    });
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('layout.textDirection.title')}
      width={440}
      testId="text-direction-dialog"
      footer={<Footer onClose={onClose} onApply={apply} testId="text-direction" />}
    >
      <div className={styles.formGrid}>
        <fieldset className={styles.fieldset}>
          <legend>{t('layout.textDirection.title')}</legend>
          {(['horizontal', 'tbRl', 'btLr'] as const).map((d) => (
            <label key={d}>
              <input
                type="radio"
                name="page-text-direction"
                data-testid={`text-direction-${d}`}
                checked={dir === d}
                onChange={() => setDir(d)}
              />{' '}
              {t(`layout.textDirection.${d}`)}
            </label>
          ))}
        </fieldset>
        <label className={styles.field}>
          <span>{t('layout.applyTo')}</span>
          <select
            data-testid="text-direction-scope"
            value={scope}
            onChange={(e) => setScope(e.target.value as typeof scope)}
          >
            <option value="section">{t('layout.thisSection')}</option>
            <option value="document">{t('layout.wholeDocument')}</option>
          </select>
        </label>
      </div>
    </Dialog>
  );
}

/** Typefaces offered for the large initial letter. */
const DROP_FONTS = ['Times New Roman', 'Arial', 'Cambria', 'Calibri', 'Georgia'];

/** Large initial letter (drop cap): none / in the paragraph / in the margin. */
export function DropCapDialog({
  run,
  language,
  onClose,
}: {
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [position, setPosition] = useState<'none' | DropCapSpec['position']>('none');
  const [font, setFont] = useState('');
  const [lines, setLines] = useState(3);
  const [distance, setDistance] = useState(0);
  const state = useLayout(run, (s) => {
    const d = s.dropCap;
    if (!d) return;
    setPosition(d.position);
    setFont(d.font ?? '');
    setLines(d.lines);
    setDistance(d.distance);
  });
  if (!state) return null;
  const apply = () => {
    onClose();
    void run({
      cmd: 'setDropCap',
      dropCap: position === 'none' ? null : { position, font: font || null, lines, distance },
    });
  };
  const off = position === 'none';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('layout.dropCap.title')}
      width={460}
      testId="drop-cap-dialog"
      footer={<Footer onClose={onClose} onApply={apply} testId="drop-cap" />}
    >
      <div className={styles.formGrid}>
        <fieldset className={styles.fieldset}>
          <legend>{t('layout.dropCap.position')}</legend>
          {(['none', 'drop', 'margin'] as const).map((p) => (
            <label key={p}>
              <input
                type="radio"
                name="drop-cap-position"
                data-testid={`drop-cap-${p}`}
                checked={position === p}
                onChange={() => setPosition(p)}
              />{' '}
              {t(`layout.dropCap.${p}`)}
            </label>
          ))}
        </fieldset>
        <label className={styles.field}>
          <span>{t('layout.dropCap.font')}</span>
          <select
            data-testid="drop-cap-font"
            disabled={off}
            value={font}
            onChange={(e) => setFont(e.target.value)}
          >
            <option value="">{t('layout.dropCap.sameFont')}</option>
            {(DROP_FONTS.includes(font) || !font ? DROP_FONTS : [font, ...DROP_FONTS]).map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('layout.dropCap.lines')}</span>
          <input
            type="number"
            min="1"
            max="10"
            disabled={off}
            data-testid="drop-cap-lines"
            value={lines}
            onChange={(e) => setLines(clampInt(e.target.value, 1, 10))}
          />
        </label>
        <Length
          label={t('layout.dropCap.distance')}
          twips={distance}
          language={language}
          testId="drop-cap-distance"
          onChange={setDistance}
        />
      </div>
    </Dialog>
  );
}
