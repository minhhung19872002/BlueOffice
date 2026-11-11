import { useTranslation } from '@blueoffice/i18n';
import type { EditCommand, FontChoice, FormatState } from '@blueoffice/platform';
import { Popover } from '@blueoffice/ui';
import { Baseline, ChevronDown, Highlighter, ListChevronsUpDown, PaintBucket } from 'lucide-react';
import { useState } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<unknown>;

/** Sizes offered by the size box (points); any other value can be typed. */
export const FONT_SIZES = [8, 9, 10, 10.5, 11, 12, 13, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72];

/**
 * Text colours: BlueOffice's own palette — neutral greys, then deep, mid and light
 * rows of the hues used in Vietnamese administrative and business documents.
 */
export const TEXT_PALETTE = [
  ['000000', '3A3F47', '5E6672', '8A929E', 'B9C0CA', 'FFFFFF'],
  ['8E1B1B', '9A4A00', '7A6400', '1F6B3A', '0F4C81', '5B2A86'],
  ['C62828', 'D9730D', 'C9A400', '2E8B57', '1D6FD1', '7E4BC2'],
  ['EF9A9A', 'F5B971', 'EBD86A', '8FD3A8', '8DB8F0', 'C2A5E8'],
];

/** Highlight colours (ECMA-376 Part 1, §17.18.40 ST_HighlightColor). */
export const HIGHLIGHT_PALETTE = [
  ['FFFF00', '00FF00', '00FFFF', 'FF00FF', '0000FF', 'FF0000'],
  ['000080', '008080', '008000', '800080', '800000', '808000'],
  ['808080', 'C0C0C0', '000000'],
];

const HEX = /^#?([0-9a-f]{6})$/i;

/** Font family list: loaded families plus substituted names (e.g. "Times New Roman → Tinos"). */
export function FontFamilyPicker({
  format,
  fonts,
  run,
  disabled,
  onDone,
  testId = 'font-family',
}: {
  format: FormatState | null | undefined;
  fonts: FontChoice[];
  run: Run;
  disabled: boolean;
  onDone: () => void;
  testId?: string;
}) {
  const { t } = useTranslation();
  const current = format?.font ?? '';
  const known = fonts.some((f) => f.name === current);
  return (
    <select
      className={styles.fontFamily}
      data-testid={testId}
      aria-label={t('font.family')}
      title={t('font.family')}
      disabled={disabled}
      value={current}
      onChange={(e) => {
        void run({ cmd: 'formatText', format: { kind: 'font', name: e.target.value } }).then(
          onDone,
        );
      }}
    >
      {!known && <option value={current}>{current || t('font.default')}</option>}
      {fonts.map((f) => (
        <option key={f.name} value={f.name}>
          {f.available || !f.substitute
            ? f.name
            : t('font.substitute', { name: f.name, substitute: f.substitute })}
        </option>
      ))}
    </select>
  );
}

/** Size combobox: list of common sizes, any typed value 1–1638 pt (§17.3.2.38 sz). */
export function FontSizeBox({
  format,
  run,
  disabled,
  onDone,
  testId = 'font-size',
}: {
  format: FormatState | null | undefined;
  run: Run;
  disabled: boolean;
  onDone: () => void;
  testId?: string;
}) {
  const { t } = useTranslation();
  const size = format?.sizeHalfPoints != null ? format.sizeHalfPoints / 2 : null;
  const shown = size != null ? String(size) : '';
  // Text being typed; the selection's size otherwise.
  const [edit, setEdit] = useState<string | null>(null);
  const text = edit ?? shown;
  const apply = (value: string) => {
    setEdit(null);
    const v = Number(value.replace(',', '.'));
    if (!Number.isFinite(v) || v < 1 || v > 1638) return;
    void run({ cmd: 'formatText', format: { kind: 'size', halfPoints: Math.round(v * 2) } }).then(
      onDone,
    );
  };
  return (
    <>
      <input
        className={styles.fontSize}
        data-testid={testId}
        aria-label={t('font.size')}
        title={t('font.size')}
        list="bo-font-sizes"
        inputMode="decimal"
        disabled={disabled}
        value={text}
        onChange={(e) => {
          setEdit(e.target.value);
          // Picking from the list applies at once.
          const kind = (e.nativeEvent as InputEvent).inputType;
          if (kind === 'insertReplacementText') apply(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            apply(text);
          }
          if (e.key === 'Escape') {
            setEdit(null);
            onDone();
          }
        }}
        onBlur={() => {
          if (edit !== null && edit !== shown) apply(edit);
          else setEdit(null);
        }}
      />
      <datalist id="bo-font-sizes">
        {FONT_SIZES.map((s) => (
          <option key={s} value={s} />
        ))}
      </datalist>
    </>
  );
}

/** Colour palette with "automatic"/"none" and a custom hex value. */
export function ColorPicker({
  kind,
  value,
  onPick,
  disabled,
  label,
  testId,
}: {
  /** `color` = text, `highlight` = §17.18.40 names, `shading` = cell / paragraph fill. */
  kind: 'color' | 'highlight' | 'shading';
  value: string | null | undefined;
  onPick: (hex: string | null) => void;
  disabled: boolean;
  label: string;
  testId: string;
}) {
  const { t } = useTranslation();
  const [custom, setCustom] = useState('');
  const palette = kind === 'highlight' ? HIGHLIGHT_PALETTE : TEXT_PALETTE;
  const pick = (hex: string | null, close: () => void) => {
    close();
    onPick(hex);
  };
  const Icon = kind === 'color' ? Baseline : kind === 'highlight' ? Highlighter : PaintBucket;
  return (
    <Popover
      label={label}
      disabled={disabled}
      testId={testId}
      trigger={
        <>
          <span className={styles.colorIcon}>
            <Icon size={16} />
            <span
              className={styles.colorBar}
              style={{
                background: value
                  ? `#${value}`
                  : kind === 'color'
                    ? 'var(--bo-text)'
                    : 'transparent',
              }}
            />
          </span>
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        <div className={styles.palette}>
          <button
            type="button"
            className={styles.paletteWide}
            data-color="auto"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => pick(null, close)}
          >
            {t(kind === 'color' ? 'font.automatic' : 'font.noFill')}
          </button>
          {palette.map((row, i) => (
            <div key={i} className={styles.paletteRow}>
              {row.map((hex) => (
                <button
                  key={hex}
                  type="button"
                  className={styles.swatch}
                  data-color={hex}
                  aria-label={`#${hex}`}
                  aria-pressed={value?.toUpperCase() === hex}
                  title={`#${hex}`}
                  style={{ background: `#${hex}` }}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => pick(hex, close)}
                />
              ))}
            </div>
          ))}
          <form
            className={styles.customColor}
            onSubmit={(e) => {
              e.preventDefault();
              const m = HEX.exec(custom.trim());
              if (m?.[1]) pick(m[1].toUpperCase(), close);
            }}
          >
            <input
              aria-label={t('font.customColor')}
              placeholder="#1D4F91"
              value={custom}
              data-testid={`${testId}-custom`}
              onChange={(e) => setCustom(e.target.value)}
            />
            <button type="submit" disabled={!HEX.test(custom.trim())}>
              {t('font.apply')}
            </button>
          </form>
        </div>
      )}
    </Popover>
  );
}

const LINE_CHOICES = [240, 276, 360, 480, 600, 720];
const SPACE_CHOICES = [0, 6, 12, 18, 24];

/** Line spacing and paragraph spacing (§17.3.1.33 spacing). */
export function SpacingMenu({
  format,
  run,
  disabled,
  onDone,
}: {
  format: FormatState | null | undefined;
  run: Run;
  disabled: boolean;
  onDone: () => void;
}) {
  const { t, i18n } = useTranslation();
  const line = format?.lineSpacing;
  const num = (v: number) => v.toLocaleString(i18n.language, { maximumFractionDigits: 2 });
  const choose = (cmd: EditCommand, close: () => void) => {
    close();
    void run(cmd).then(onDone);
  };
  return (
    <Popover
      label={t('font.lineSpacing')}
      disabled={disabled}
      testId="line-spacing"
      trigger={
        <>
          <ListChevronsUpDown size={16} />
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        // A menu of radio items in labelled groups (WAI-ARIA menu pattern): the visible
        // titles name the groups, so they are hidden from the accessibility tree.
        <div className={styles.menu} role="menu" aria-label={t('font.lineSpacing')}>
          <div className={styles.menuTitle} aria-hidden="true">
            {t('font.lineSpacing')}
          </div>
          <div role="group" aria-label={t('font.lineSpacing')}>
            {LINE_CHOICES.map((v) => (
              <button
                key={v}
                type="button"
                role="menuitemradio"
                aria-checked={line?.rule === 'auto' && line.value === v}
                data-line={v}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() =>
                  choose(
                    {
                      cmd: 'formatParagraph',
                      format: { kind: 'lineSpacingRule', rule: 'auto', value: v },
                    },
                    close,
                  )
                }
              >
                {num(v / 240)}
              </button>
            ))}
          </div>
          {line && line.rule !== 'auto' && (
            <div className={styles.menuNote} role="none">
              {t(line.rule === 'exact' ? 'font.lineExact' : 'font.lineAtLeast', {
                value: num(line.value / 20),
              })}
            </div>
          )}
          {(['spaceBefore', 'spaceAfter'] as const).map((k) => (
            <div key={k} role="group" aria-label={t(`font.${k}`)}>
              <div className={styles.menuTitle} aria-hidden="true">
                {t(`font.${k}`)}
              </div>
              <div className={styles.menuRow} role="none">
                {SPACE_CHOICES.map((pt) => (
                  <button
                    key={pt}
                    type="button"
                    role="menuitemradio"
                    aria-checked={(format?.[k] ?? 0) === pt * 20}
                    data-space={`${k}-${pt}`}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() =>
                      choose({ cmd: 'formatParagraph', format: { kind: k, twips: pt * 20 } }, close)
                    }
                  >
                    {t('font.points', { value: pt })}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </Popover>
  );
}
