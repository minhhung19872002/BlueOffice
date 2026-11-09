/**
 * Conditional formatting: data bar and icon set editors (negative bars, axis,
 * thresholds, custom icons — ECMA-376 §18.3.1.28 / §18.3.1.49 and the [MS-XLSX]
 * x14 extensions) and the rules manager (priority order, move up / down, Stop If
 * True, range, edit, delete). Every change goes through the engine command bridge
 * (`addCf`, `listCf`, `editCf`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  CfIconSpec,
  CfOp,
  CfRuleInfo,
  CfRuleSpec,
  CfStyle,
  CfvoSpec,
  DataBarOptions,
  IconSetOptions,
  SheetHost,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';
import styles from './SheetEditor.module.css';
import css from './cfRules.module.css';

/** Every icon set (§18.18.42 `ST_IconSetType` and the x14 sets). */
export const ICON_SET_NAMES = [
  '3Arrows',
  '3ArrowsGray',
  '3Flags',
  '3TrafficLights1',
  '3TrafficLights2',
  '3Signs',
  '3Symbols',
  '3Symbols2',
  '3Stars',
  '3Triangles',
  '4Arrows',
  '4ArrowsGray',
  '4RedToBlack',
  '4Rating',
  '4TrafficLights',
  '5Arrows',
  '5ArrowsGray',
  '5Rating',
  '5Quarters',
  '5Boxes',
] as const;

/** Number of icons of a set (its leading digit). */
export function iconCount(set: string): number {
  const n = Number(set.charAt(0));
  return Number.isInteger(n) && n >= 3 && n <= 5 ? n : 3;
}

const BAR_KINDS = ['autoMin', 'autoMax', 'min', 'max', 'num', 'percent', 'percentile', 'formula'];
const ICON_KINDS = ['num', 'percent', 'percentile', 'formula'];

export function defaultBarOptions(): DataBarOptions {
  return {
    min: { kind: 'autoMin' },
    max: { kind: 'autoMax' },
    minLength: 0,
    maxLength: 100,
    gradient: true,
    border: false,
    borderColor: null,
    negativeColor: 'D64545',
    negativeBorderColor: null,
    negativeSameColor: false,
    negativeSameBorder: true,
    axisPosition: 'automatic',
    axisColor: '202020',
    direction: 'context',
    showValue: true,
  };
}

/** Equal percent bands for an icon set of `n` icons (thresholds above the lowest). */
export function defaultIconOptions(n: number): IconSetOptions {
  return {
    thresholds: Array.from({ length: n - 1 }, (_, i) => ({
      kind: 'percent',
      value: String(Math.floor(((i + 1) * 100) / n)),
      gte: true,
    })),
    showValue: true,
    icons: [],
  };
}

function ColorField({
  label,
  value,
  onChange,
  testId,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  testId?: string;
}) {
  return (
    <label>
      {label}
      <input
        type="color"
        data-testid={testId}
        value={`#${value}`}
        onChange={(e) => onChange(e.target.value.slice(1).toUpperCase())}
      />
    </label>
  );
}

function CfvoField({
  label,
  kinds,
  value,
  onChange,
  showGte,
  testId,
}: {
  label: string;
  kinds: string[];
  value: CfvoSpec;
  onChange: (v: CfvoSpec) => void;
  showGte?: boolean;
  testId?: string;
}) {
  const { t } = useTranslation();
  const fixed = ['autoMin', 'autoMax', 'min', 'max'].includes(value.kind);
  return (
    <div className={css.cfvo} data-testid={testId}>
      <span>{label}</span>
      {showGte && (
        <select
          aria-label={t('sheetEditor.cfm.compare')}
          value={value.gte === false ? 'gt' : 'gte'}
          onChange={(e) => onChange({ ...value, gte: e.target.value === 'gte' })}
        >
          <option value="gte">≥</option>
          <option value="gt">&gt;</option>
        </select>
      )}
      <select
        aria-label={t('sheetEditor.cfm.thresholdType')}
        value={value.kind}
        onChange={(e) => onChange({ ...value, kind: e.target.value })}
      >
        {kinds.map((k) => (
          <option key={k} value={k}>
            {t(`sheetEditor.cfm.cfvo.${k}`)}
          </option>
        ))}
      </select>
      {!fixed && (
        <input
          aria-label={t('sheetEditor.cf.value')}
          value={value.value ?? ''}
          onChange={(e) => onChange({ ...value, value: e.target.value })}
        />
      )}
    </div>
  );
}

/** Data bar editor: colours, thresholds, lengths, fill, border, negative bars, axis. */
export function DataBarEditor({
  color,
  options,
  onChange,
}: {
  color: string;
  options: DataBarOptions;
  onChange: (color: string, options: DataBarOptions) => void;
}) {
  const { t } = useTranslation();
  const set = (patch: Partial<DataBarOptions>) => onChange(color, { ...options, ...patch });
  return (
    <div className={css.editor} data-testid="cf-databar-editor">
      <ColorField
        label={t('sheetEditor.cf.barColor')}
        value={color}
        onChange={(c) => onChange(c, options)}
        testId="cf-bar-color"
      />
      <CfvoField
        label={t('sheetEditor.cfm.minimum')}
        kinds={BAR_KINDS.filter((k) => k !== 'autoMax' && k !== 'max')}
        value={options.min}
        onChange={(min) => set({ min })}
        testId="cf-bar-min"
      />
      <CfvoField
        label={t('sheetEditor.cfm.maximum')}
        kinds={BAR_KINDS.filter((k) => k !== 'autoMin' && k !== 'min')}
        value={options.max}
        onChange={(max) => set({ max })}
        testId="cf-bar-max"
      />
      <label>
        {t('sheetEditor.cfm.minLength')}
        <input
          type="number"
          min={0}
          max={100}
          value={options.minLength ?? 0}
          onChange={(e) => set({ minLength: Number(e.target.value) })}
        />
      </label>
      <label>
        {t('sheetEditor.cfm.maxLength')}
        <input
          type="number"
          min={0}
          max={100}
          value={options.maxLength ?? 100}
          onChange={(e) => set({ maxLength: Number(e.target.value) })}
        />
      </label>
      <label>
        {t('sheetEditor.cfm.fill')}
        <select
          data-testid="cf-bar-fill"
          value={options.gradient === false ? 'solid' : 'gradient'}
          onChange={(e) => set({ gradient: e.target.value === 'gradient' })}
        >
          <option value="gradient">{t('sheetEditor.cfm.gradient')}</option>
          <option value="solid">{t('sheetEditor.cfm.solid')}</option>
        </select>
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={options.border ?? false}
          onChange={(e) => set({ border: e.target.checked })}
        />
        {t('sheetEditor.cfm.border')}
      </label>
      {options.border && (
        <ColorField
          label={t('sheetEditor.cfm.borderColor')}
          value={options.borderColor ?? color}
          onChange={(borderColor) => set({ borderColor })}
        />
      )}
      <fieldset className={css.group}>
        <legend>{t('sheetEditor.cfm.negative')}</legend>
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={options.negativeSameColor ?? false}
            onChange={(e) => set({ negativeSameColor: e.target.checked })}
          />
          {t('sheetEditor.cfm.negativeSameColor')}
        </label>
        {!options.negativeSameColor && (
          <ColorField
            label={t('sheetEditor.cfm.negativeColor')}
            value={options.negativeColor ?? 'D64545'}
            onChange={(negativeColor) => set({ negativeColor })}
            testId="cf-bar-negative"
          />
        )}
        {options.border && (
          <>
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={options.negativeSameBorder ?? true}
                onChange={(e) => set({ negativeSameBorder: e.target.checked })}
              />
              {t('sheetEditor.cfm.negativeSameBorder')}
            </label>
            {options.negativeSameBorder === false && (
              <ColorField
                label={t('sheetEditor.cfm.negativeBorderColor')}
                value={options.negativeBorderColor ?? 'D64545'}
                onChange={(negativeBorderColor) => set({ negativeBorderColor })}
              />
            )}
          </>
        )}
        <label>
          {t('sheetEditor.cfm.axis')}
          <select
            data-testid="cf-bar-axis"
            value={options.axisPosition ?? 'automatic'}
            onChange={(e) => set({ axisPosition: e.target.value })}
          >
            {['automatic', 'middle', 'none'].map((a) => (
              <option key={a} value={a}>
                {t(`sheetEditor.cfm.axisPos.${a}`)}
              </option>
            ))}
          </select>
        </label>
        {options.axisPosition !== 'none' && (
          <ColorField
            label={t('sheetEditor.cfm.axisColor')}
            value={options.axisColor ?? '202020'}
            onChange={(axisColor) => set({ axisColor })}
          />
        )}
      </fieldset>
      <label>
        {t('sheetEditor.cfm.direction')}
        <select
          data-testid="cf-bar-direction"
          value={options.direction ?? 'context'}
          onChange={(e) => set({ direction: e.target.value })}
        >
          {['context', 'leftToRight', 'rightToLeft'].map((d) => (
            <option key={d} value={d}>
              {t(`sheetEditor.cfm.dir.${d}`)}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={options.showValue === false}
          onChange={(e) => set({ showValue: !e.target.checked })}
        />
        {t('sheetEditor.cfm.barOnly')}
      </label>
    </div>
  );
}

/** Icon set editor: set, reverse, icon only, thresholds (type, ≥ / >), custom icons. */
export function IconSetEditor({
  set,
  reverse,
  options,
  onChange,
}: {
  set: string;
  reverse: boolean;
  options: IconSetOptions;
  onChange: (set: string, reverse: boolean, options: IconSetOptions) => void;
}) {
  const { t } = useTranslation();
  const n = iconCount(set);
  const icons = options.icons ?? [];
  const custom = icons.length > 0;
  const setOpts = (patch: Partial<IconSetOptions>) =>
    onChange(set, reverse, { ...options, ...patch });
  const pickSet = (s: string) => {
    const m = iconCount(s);
    onChange(s, reverse, m === n ? { ...options, icons: [] } : defaultIconOptions(m));
  };
  const setIcon = (band: number, v: CfIconSpec) =>
    setOpts({ icons: icons.map((ic, i) => (i === band ? v : ic)) });
  const iconValue = (ic: CfIconSpec) => (ic.set === 'NoIcons' ? 'NoIcons' : `${ic.set}:${ic.id}`);
  const parseIcon = (v: string): CfIconSpec => {
    if (v === 'NoIcons') return { set: 'NoIcons', id: 0 };
    const [s, id] = v.split(':');
    return { set: s ?? set, id: Number(id) || 0 };
  };
  // Highest band first, as the bands are usually read.
  const bands = Array.from({ length: n }, (_, i) => n - 1 - i);
  return (
    <div className={css.editor} data-testid="cf-iconset-editor">
      <label>
        {t('sheetEditor.cf.iconSet')}
        <select data-testid="cf-iconset" value={set} onChange={(e) => pickSet(e.target.value)}>
          {ICON_SET_NAMES.map((s) => (
            <option key={s} value={s}>
              {t(`sheetEditor.cf.icons.${s}`)}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={reverse}
          disabled={custom}
          onChange={(e) => onChange(set, e.target.checked, options)}
        />
        {t('sheetEditor.cfm.reverse')}
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          data-testid="cf-icon-only"
          checked={options.showValue === false}
          onChange={(e) => setOpts({ showValue: !e.target.checked })}
        />
        {t('sheetEditor.cfm.iconOnly')}
      </label>
      <label className={styles.check}>
        <input
          type="checkbox"
          data-testid="cf-icon-custom"
          checked={custom}
          onChange={(e) =>
            setOpts({
              icons: e.target.checked
                ? Array.from({ length: n }, (_, i) => ({
                    set,
                    id: reverse ? n - 1 - i : i,
                  }))
                : [],
            })
          }
        />
        {t('sheetEditor.cfm.customIcons')}
      </label>
      {bands.map((band) => (
        <div key={band} className={css.band}>
          <span className={css.bandName}>{t('sheetEditor.cfm.band', { n: String(n - band) })}</span>
          {custom && (
            <select
              aria-label={t('sheetEditor.cfm.icon')}
              data-testid={`cf-icon-${band}`}
              value={iconValue(icons[band] ?? { set, id: band })}
              onChange={(e) => setIcon(band, parseIcon(e.target.value))}
            >
              <option value="NoIcons">{t('sheetEditor.cfm.noIcon')}</option>
              {ICON_SET_NAMES.flatMap((s) =>
                Array.from({ length: iconCount(s) }, (_, id) => (
                  <option key={`${s}:${id}`} value={`${s}:${id}`}>
                    {t('sheetEditor.cfm.iconOf', {
                      set: t(`sheetEditor.cf.icons.${s}`),
                      n: String(id + 1),
                    })}
                  </option>
                )),
              )}
            </select>
          )}
          {band > 0 ? (
            <CfvoField
              label={t('sheetEditor.cfm.whenValue')}
              kinds={ICON_KINDS}
              value={options.thresholds[band - 1] ?? { kind: 'percent', value: '0' }}
              showGte
              testId={`cf-threshold-${band}`}
              onChange={(v) =>
                setOpts({
                  thresholds: options.thresholds.map((x, i) => (i === band - 1 ? v : x)),
                })
              }
            />
          ) : (
            <span className={css.otherwise}>{t('sheetEditor.cfm.otherwise')}</span>
          )}
        </div>
      ))}
    </div>
  );
}

/** Parameters of a highlight rule (operator values, text, formula, rank, format). */
function HighlightEditor({
  spec,
  onChange,
}: {
  spec: CfRuleSpec;
  onChange: (s: CfRuleSpec) => void;
}) {
  const { t } = useTranslation();
  if (!('style' in spec)) return null;
  const style: CfStyle = spec.style;
  const setStyle = (p: Partial<CfStyle>) => onChange({ ...spec, style: { ...style, ...p } });
  return (
    <div className={css.editor}>
      {spec.kind === 'cellIs' && (
        <>
          <label>
            {t('sheetEditor.cf.value')}
            <input
              value={spec.formula1}
              onChange={(e) => onChange({ ...spec, formula1: e.target.value })}
            />
          </label>
          {(spec.operator === 'between' || spec.operator === 'notBetween') && (
            <label>
              {t('sheetEditor.cf.value2')}
              <input
                value={spec.formula2 ?? ''}
                onChange={(e) => onChange({ ...spec, formula2: e.target.value })}
              />
            </label>
          )}
        </>
      )}
      {spec.kind === 'expression' && (
        <label>
          {t('sheetEditor.cf.formula')}
          <input
            value={spec.formula}
            onChange={(e) => onChange({ ...spec, formula: e.target.value })}
          />
        </label>
      )}
      {spec.kind === 'containsText' && (
        <label>
          {t('sheetEditor.cf.text')}
          <input value={spec.text} onChange={(e) => onChange({ ...spec, text: e.target.value })} />
        </label>
      )}
      {spec.kind === 'top10' && (
        <label>
          {t('sheetEditor.cf.rank')}
          <input
            type="number"
            min={1}
            value={spec.rank}
            onChange={(e) => onChange({ ...spec, rank: Number(e.target.value) || 1 })}
          />
        </label>
      )}
      <ColorField
        label={t('sheetEditor.fmt.fillColor')}
        value={style.fill ?? 'FFFFFF'}
        onChange={(fill) => setStyle({ fill })}
      />
      <ColorField
        label={t('sheetEditor.fmt.color')}
        value={style.fontColor ?? '000000'}
        onChange={(fontColor) => setStyle({ fontColor })}
      />
      <label className={styles.check}>
        <input
          type="checkbox"
          checked={style.bold ?? false}
          onChange={(e) => setStyle({ bold: e.target.checked })}
        />
        {t('sheetEditor.bold')}
      </label>
    </div>
  );
}

/** Editor of any rule definition the manager can change in place. */
export function RuleSpecEditor({
  spec,
  onChange,
}: {
  spec: CfRuleSpec;
  onChange: (s: CfRuleSpec) => void;
}) {
  const { t } = useTranslation();
  switch (spec.kind) {
    case 'dataBar':
      return (
        <DataBarEditor
          color={spec.color}
          options={spec.options ?? defaultBarOptions()}
          onChange={(color, options) => onChange({ kind: 'dataBar', color, options })}
        />
      );
    case 'iconSet':
      return (
        <IconSetEditor
          set={spec.set}
          reverse={spec.reverse ?? false}
          options={spec.options ?? defaultIconOptions(iconCount(spec.set))}
          onChange={(set, reverse, options) => onChange({ kind: 'iconSet', set, reverse, options })}
        />
      );
    case 'colorScale':
      return (
        <div className={css.editor}>
          <ColorField
            label={t('sheetEditor.cf.minColor')}
            value={spec.minColor}
            onChange={(minColor) => onChange({ ...spec, minColor })}
          />
          {spec.midColor != null && (
            <ColorField
              label={t('sheetEditor.cf.midColor')}
              value={spec.midColor}
              onChange={(midColor) => onChange({ ...spec, midColor })}
            />
          )}
          <ColorField
            label={t('sheetEditor.cf.maxColor')}
            value={spec.maxColor}
            onChange={(maxColor) => onChange({ ...spec, maxColor })}
          />
        </div>
      );
    default:
      return <HighlightEditor spec={spec} onChange={onChange} />;
  }
}

/** Short description of a rule for the manager list. */
function describe(t: ReturnType<typeof useTranslation>['t'], r: CfRuleInfo): string {
  const kind = t(`sheetEditor.cf.kinds.${r.kind}`, { defaultValue: r.kind });
  const s = r.spec;
  if (!s) return kind;
  switch (s.kind) {
    case 'cellIs':
      return `${kind}: ${t(`sheetEditor.op.${s.operator}`)} ${s.formula1}${s.formula2 ? ` – ${s.formula2}` : ''}`;
    case 'expression':
      return `${kind}: =${s.formula}`;
    case 'containsText':
      return `${kind}: "${s.text}"`;
    case 'iconSet':
      return `${kind}: ${t(`sheetEditor.cf.icons.${s.set}`)}`;
    default:
      return kind;
  }
}

/** The conditional formatting rules manager of the current sheet. */
export function CfManagerDialog({
  host,
  sheet,
  onClose,
  onDone,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  onClose: () => void;
  onDone: () => void | Promise<void>;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [rules, setRules] = useState<CfRuleInfo[]>([]);
  const [selected, setSelected] = useState(0);
  const [editing, setEditing] = useState<CfRuleSpec | null>(null);
  const [ranges, setRanges] = useState<Record<string, string>>({});

  const load = useCallback(async () => {
    try {
      const r = await host.exec<{ rules: CfRuleInfo[] }>({ cmd: 'listCf', sheet });
      setRules(r.rules);
      setRanges({});
    } catch (e) {
      fail(e);
    }
  }, [host, sheet, fail]);

  useEffect(() => {
    let live = true;
    host
      .exec<{ rules: CfRuleInfo[] }>({ cmd: 'listCf', sheet })
      .then((r) => live && setRules(r.rules))
      .catch(fail);
    return () => {
      live = false;
    };
  }, [host, sheet, fail]);

  const run = async (r: CfRuleInfo, op: CfOp, select?: number) => {
    try {
      await host.exec({ cmd: 'editCf', sheet, block: r.block, rule: r.rule, op });
      await onDone();
      await load();
      if (select !== undefined) setSelected(select);
    } catch (e) {
      fail(e);
    }
  };
  const current = rules[selected];
  const key = (r: CfRuleInfo) => `${r.block}:${r.rule}`;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.cfm.title')}
      width={640}
      testId="cf-manager"
      footer={
        <button type="button" className={styles.btnPrimary} onClick={onClose}>
          {t('sheetEditor.dlg.ok')}
        </button>
      }
    >
      {rules.length === 0 ? (
        <p data-testid="cf-manager-empty">{t('sheetEditor.cfm.empty')}</p>
      ) : (
        <table className={css.table}>
          <thead>
            <tr>
              <th>{t('sheetEditor.cfm.order')}</th>
              <th>{t('sheetEditor.cfm.rule')}</th>
              <th>{t('sheetEditor.cfm.appliesTo')}</th>
              <th>{t('sheetEditor.cfm.stopIfTrue')}</th>
            </tr>
          </thead>
          <tbody>
            {rules.map((r, i) => (
              <tr
                key={key(r)}
                data-testid={`cf-rule-${i}`}
                className={i === selected ? css.selected : undefined}
                onClick={() => {
                  setSelected(i);
                  setEditing(null);
                }}
              >
                <td>{r.priority}</td>
                <td>
                  {describe(t, r)}
                  {r.pivot && <span className={css.tag}>{t('sheetEditor.cfm.pivotTag')}</span>}
                </td>
                <td>
                  <input
                    aria-label={t('sheetEditor.cfm.appliesTo')}
                    value={ranges[key(r)] ?? r.range}
                    onChange={(e) => setRanges({ ...ranges, [key(r)]: e.target.value })}
                    onBlur={() => {
                      const v = ranges[key(r)];
                      if (v !== undefined && v !== r.range)
                        void run(r, { op: 'setRange', range: v });
                    }}
                  />
                </td>
                <td>
                  <input
                    type="checkbox"
                    data-testid={`cf-stop-${i}`}
                    aria-label={t('sheetEditor.cfm.stopIfTrue')}
                    checked={r.stopIfTrue}
                    onChange={(e) => void run(r, { op: 'stopIfTrue', value: e.target.checked }, i)}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {current && (
        <div className={css.actions}>
          <button
            type="button"
            className={styles.btn}
            data-testid="cf-move-up"
            disabled={selected === 0}
            onClick={() => void run(current, { op: 'moveUp' }, selected - 1)}
          >
            {t('sheetEditor.cfm.moveUp')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="cf-move-down"
            disabled={selected >= rules.length - 1}
            onClick={() => void run(current, { op: 'moveDown' }, selected + 1)}
          >
            {t('sheetEditor.cfm.moveDown')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="cf-edit"
            disabled={!current.spec}
            onClick={() => setEditing(current.spec)}
          >
            {t('sheetEditor.cfm.edit')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="cf-delete"
            onClick={() => void run(current, { op: 'delete' }, Math.max(0, selected - 1))}
          >
            {t('sheetEditor.cfm.delete')}
          </button>
        </div>
      )}
      {current && editing && (
        <div className={css.editPanel}>
          <RuleSpecEditor spec={editing} onChange={setEditing} />
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="cf-apply-edit"
            onClick={() => {
              const rule = editing;
              setEditing(null);
              void run(current, { op: 'replace', rule }, selected);
            }}
          >
            {t('sheetEditor.cfm.apply')}
          </button>
        </div>
      )}
    </Dialog>
  );
}
