import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  ListLevelSpec,
  ListNumbering,
  ListNumFormat,
} from '@blueoffice/platform';
import { Dialog, Popover } from '@blueoffice/ui';
import { ChevronDown, ListTree } from 'lucide-react';
import { useState } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Level number formats offered (ECMA-376 Part 1, §17.18.59 ST_NumberFormat subset). */
export const LIST_FORMATS: ListNumFormat[] = [
  'decimal',
  'decimalZero',
  'lowerLetter',
  'upperLetter',
  'lowerRoman',
  'upperRoman',
  'bullet',
  'none',
];

/** Default level `i` (0-based) of a new list: `1.`, `1.1.`, … (mirrors `bo_edit::lists::default_level`). */
export function defaultLevel(i: number): ListLevelSpec {
  let text = '';
  for (let k = 1; k <= i + 1; k++) text += `%${k}.`;
  return { format: 'decimal', text, start: 1, indent: 720 * (i + 1), hanging: 360 + 144 * i };
}

/** The nine default levels. */
export function defaultLevels(): ListLevelSpec[] {
  return Array.from({ length: 9 }, (_, i) => defaultLevel(i));
}

function letters(n: number): string {
  if (n <= 0) return String(n);
  const c = String.fromCharCode(65 + ((n - 1) % 26));
  return c.repeat(Math.floor((n - 1) / 26) + 1);
}

function roman(n: number): string {
  if (n < 1 || n >= 4000) return String(n);
  const table: [number, string][] = [
    [1000, 'M'],
    [900, 'CM'],
    [500, 'D'],
    [400, 'CD'],
    [100, 'C'],
    [90, 'XC'],
    [50, 'L'],
    [40, 'XL'],
    [10, 'X'],
    [9, 'IX'],
    [5, 'V'],
    [4, 'IV'],
    [1, 'I'],
  ];
  let s = '';
  let v = n;
  for (const [k, r] of table) {
    while (v >= k) {
      s += r;
      v -= k;
    }
  }
  return s;
}

/** A number in a list format (same rules as `bo_model::format_number`). */
export function formatNumber(n: number, f: ListNumFormat): string {
  switch (f) {
    case 'decimalZero':
      return n < 10 && n >= 0 ? `0${n}` : String(n);
    case 'lowerLetter':
      return letters(n).toLowerCase();
    case 'upperLetter':
      return letters(n);
    case 'lowerRoman':
      return roman(n).toLowerCase();
    case 'upperRoman':
      return roman(n);
    case 'bullet':
    case 'none':
      return '';
    default:
      return String(n);
  }
}

/** Label of level `ilvl` when each level `k` stands at `counts[k]` (default: its start). */
export function levelLabel(levels: ListLevelSpec[], ilvl: number, counts: number[] = []): string {
  const lv = levels[ilvl];
  if (!lv) return '';
  // §17.9.11 lvlText: %1…%9 are replaced by the levels' current numbers.
  return lv.text.replace(/%([1-9])/g, (_, d: string) => {
    const k = Number(d) - 1;
    const l = levels[k];
    if (!l) return '';
    // §17.9.4 isLgl: every number of a legal-style level as a decimal.
    const legal = lv.legal && l.format !== 'bullet' && l.format !== 'none';
    return formatNumber(counts[k] ?? l.start, legal ? 'decimal' : l.format);
  });
}

const HEADINGS = Array.from({ length: 9 }, (_, i) => `Heading${i + 1}`);

/** Define a new multilevel list: per level format, text, start, indents and linked style. */
export function DefineListDialog({
  onApply,
  onClose,
}: {
  onApply: (levels: ListLevelSpec[], apply: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [levels, setLevels] = useState<ListLevelSpec[]>(defaultLevels);
  const [sel, setSel] = useState(0);
  const [apply, setApply] = useState(true);
  const lv = levels[sel] ?? defaultLevel(sel);
  const patch = (p: Partial<ListLevelSpec>) =>
    setLevels((ls) => ls.map((l, i) => (i === sel ? { ...l, ...p } : l)));
  const pt = (tw: number) => Math.round(tw / 2) / 10;
  const num = (v: string, min: number, max: number) => {
    const n = Math.round(Number(v));
    return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : min;
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('listDialog.title')}
      testId="define-list-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="define-list-ok"
            onClick={() =>
              onApply(
                levels.map((l) => (l.style ? l : { ...l, style: null })),
                apply,
              )
            }
          >
            {t('listDialog.ok')}
          </button>
        </>
      }
    >
      <div className={styles.menuRow} role="radiogroup" aria-label={t('listDialog.level')}>
        {levels.map((_, i) => (
          <button
            key={i}
            type="button"
            role="radio"
            aria-checked={i === sel}
            className={i === sel ? styles.dialogPrimary : styles.dialogButton}
            data-testid={`list-level-${i + 1}`}
            onClick={() => setSel(i)}
          >
            {i + 1}
          </button>
        ))}
      </div>
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('listDialog.format')}</span>
          <select
            data-testid="list-format"
            value={lv.format}
            onChange={(e) => {
              const format = e.target.value as ListNumFormat;
              patch(format === 'bullet' ? { format, text: '•' } : { format });
            }}
          >
            {LIST_FORMATS.map((f) => (
              <option key={f} value={f}>
                {t(`listDialog.formats.${f}`)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('listDialog.text')}</span>
          <input
            data-testid="list-text"
            value={lv.text}
            maxLength={255}
            onChange={(e) => patch({ text: e.target.value })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('listDialog.start')}</span>
          <input
            type="number"
            min={0}
            max={32767}
            data-testid="list-start"
            value={lv.start}
            onChange={(e) => patch({ start: num(e.target.value, 0, 32767) })}
          />
        </label>
        <label className={styles.field}>
          <span>{t('listDialog.style')}</span>
          <select
            data-testid="list-style"
            value={lv.style ?? ''}
            onChange={(e) => patch({ style: e.target.value || null })}
          >
            <option value="">{t('listDialog.noStyle')}</option>
            {HEADINGS.map((h, i) => (
              <option key={h} value={h}>
                {t('listDialog.heading', { n: i + 1 })}
              </option>
            ))}
          </select>
        </label>
        {sel > 0 && (
          <label className={styles.field}>
            <span>{t('listDialog.restartAfter')}</span>
            {/* §17.9.10 lvlRestart: one-based level, 0 = never, absent = the level above. */}
            <select
              data-testid="list-restart"
              value={lv.restart == null ? '' : String(lv.restart)}
              onChange={(e) =>
                patch({ restart: e.target.value === '' ? null : Number(e.target.value) })
              }
            >
              <option value="">{t('listDialog.restartDefault')}</option>
              {Array.from({ length: sel }, (_, k) => (
                <option key={k} value={String(k + 1)}>
                  {t('listDialog.levelSample', { n: k + 1 })}
                </option>
              ))}
              <option value="0">{t('listDialog.restartNever')}</option>
            </select>
          </label>
        )}
        <label className={styles.field}>
          {/* §17.9.4 isLgl. */}
          <input
            type="checkbox"
            data-testid="list-legal"
            checked={!!lv.legal}
            onChange={(e) => patch({ legal: e.target.checked })}
          />
          {t('listDialog.legal')}
        </label>
        <label className={styles.field}>
          <span>{t('listDialog.indent')}</span>
          <input
            type="number"
            min={0}
            step={0.5}
            data-testid="list-indent"
            value={pt(lv.indent)}
            onChange={(e) => {
              const indent = num(String(Number(e.target.value) * 20), 0, 31680);
              patch({ indent, hanging: Math.min(lv.hanging, indent) });
            }}
          />
          <span className={styles.unit}>pt</span>
        </label>
        <label className={styles.field}>
          <span>{t('listDialog.hanging')}</span>
          <input
            type="number"
            min={0}
            step={0.5}
            data-testid="list-hanging"
            value={pt(lv.hanging)}
            onChange={(e) =>
              patch({ hanging: num(String(Number(e.target.value) * 20), 0, lv.indent) })
            }
          />
          <span className={styles.unit}>pt</span>
        </label>
        <div className={styles.hint}>{t('listDialog.textHint')}</div>
      </div>
      <div className={styles.menuRow}>
        <button
          type="button"
          className={styles.dialogButton}
          data-testid="list-link-headings"
          onClick={() => setLevels((ls) => ls.map((l, i) => ({ ...l, style: HEADINGS[i] })))}
        >
          {t('listDialog.linkHeadings')}
        </button>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="list-apply"
            checked={apply}
            onChange={(e) => setApply(e.target.checked)}
          />
          {t('listDialog.apply')}
        </label>
      </div>
      <div className={styles.menuTitle}>{t('listDialog.preview')}</div>
      <div data-testid="list-preview" aria-live="polite">
        {levels.map((l, i) => (
          <div
            key={i}
            data-level={i + 1}
            style={{
              paddingLeft: `${Math.max(0, l.indent - l.hanging) / 40}px`,
              fontWeight: i === sel ? 600 : undefined,
            }}
          >
            <span>{levelLabel(levels, i)}</span>{' '}
            <span className={styles.hint}>
              {l.style
                ? t('listDialog.heading', { n: Number(l.style.replace('Heading', '')) || i + 1 })
                : t('listDialog.levelSample', { n: i + 1 })}
            </span>
          </div>
        ))}
      </div>
    </Dialog>
  );
}

/** Set numbering value: start a new list at a value or continue the list above. */
export function SetNumberingDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<'restart' | 'continue'>('restart');
  const [value, setValue] = useState(1);
  const [error, setError] = useState(false);
  const submit = async () => {
    const numbering: ListNumbering =
      mode === 'restart' ? { kind: 'restart', value } : { kind: 'continue' };
    const r = await run({ cmd: 'setListNumbering', numbering });
    if (!r || r.error) {
      setError(true);
      return;
    }
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('listDialog.setValueTitle')}
      testId="set-numbering-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="set-numbering-ok"
            onClick={() => void submit()}
          >
            {t('listDialog.ok')}
          </button>
        </>
      }
    >
      <div className={styles.fieldset} role="radiogroup">
        {(['restart', 'continue'] as const).map((m) => (
          <label key={m} className={styles.field}>
            <input
              type="radio"
              name="list-mode"
              data-testid={`numbering-${m}`}
              checked={mode === m}
              onChange={() => setMode(m)}
            />
            {t(`listDialog.${m}`)}
          </label>
        ))}
      </div>
      <label className={styles.field}>
        <span>{t('listDialog.value')}</span>
        <input
          type="number"
          min={0}
          max={32767}
          data-testid="numbering-value"
          disabled={mode !== 'restart'}
          value={value}
          onChange={(e) => {
            const n = Math.round(Number(e.target.value));
            setValue(Number.isFinite(n) ? Math.min(32767, Math.max(0, n)) : 1);
          }}
        />
      </label>
      {error && (
        <div className={styles.hint} role="alert" data-testid="numbering-error">
          {t('listDialog.notInList')}
        </div>
      )}
    </Dialog>
  );
}

/** One entry of the multilevel list menu (a command-registry command). */
export interface ListMenuEntry {
  id: string;
  label: string;
  enabled: boolean;
  onSelect: () => void;
}

/** Multilevel list ribbon dropdown. */
export function MultilevelMenu({
  entries,
  disabled,
}: {
  entries: ListMenuEntry[];
  disabled: boolean;
}) {
  const { t } = useTranslation();
  return (
    <Popover
      label={t('commands.format.multilevel')}
      disabled={disabled}
      testId="multilevel-list"
      trigger={
        <>
          <ListTree size={16} />
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        <div className={styles.menu} role="menu">
          {entries.map((e) => (
            <button
              key={e.id}
              type="button"
              role="menuitem"
              data-command={e.id}
              disabled={!e.enabled}
              onMouseDown={(ev) => ev.preventDefault()}
              onClick={() => {
                close();
                e.onSelect();
              }}
            >
              {e.label}
            </button>
          ))}
        </div>
      )}
    </Popover>
  );
}
