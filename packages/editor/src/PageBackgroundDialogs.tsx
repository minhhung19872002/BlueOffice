/**
 * Design tab of BlueDoc: page colour, page borders (line and art borders drawn with
 * BlueOffice's own motifs), watermarks, document themes and paragraph-spacing style
 * sets. Every control sends one engine command (one undo step); dialogs read the
 * current state with `{ cmd: 'design' }`.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  DesignState,
  EditCommand,
  EditResult,
  PageBorderDisplay,
  PageBordersSpec,
  WatermarkSpec,
} from '@blueoffice/platform';
import { Dialog, Popover } from '@blueoffice/ui';
import { ChevronDown, Droplets, Palette, Rows3 } from 'lucide-react';
import { useEffect, useState, type ReactNode } from 'react';
import styles from './Editor.module.css';
import own from './PageBackgroundDialogs.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Line styles offered (§17.18.2 ST_Border). */
export const LINE_STYLES = ['single', 'double', 'dotted', 'dashed', 'thick', 'dotDash'] as const;

/** Line widths in eighths of a point (¼ … 6 pt). */
export const LINE_WIDTHS = [2, 4, 6, 8, 12, 18, 24, 36, 48];

/** Art styles with a BlueOffice motif (same set as `bo_layout::ArtMotif`). */
export const ART_STYLES = [
  'basicBlackDots',
  'basicWhiteDots',
  'basicBlackSquares',
  'basicWhiteSquares',
  'diamondsGray',
  'triangles',
  'zigZag',
  'waveline',
  'checkered',
  'sawtooth',
];

/** Watermark text presets (labels in `design.presets`). */
export const WATERMARK_PRESETS = ['draft', 'confidential', 'copy', 'sample', 'urgent'] as const;

/** Spacing of the style sets (mirrors `bo_edit::design::STYLE_SETS`): pt after, line. */
export const STYLE_SET_SPACING: Record<string, [number, number]> = {
  compact: [0, 1],
  balanced: [6, 1.15],
  airy: [10, 1.5],
  double: [12, 2],
};

/** Fonts offered for text watermarks. */
const WATERMARK_FONTS = ['Calibri', 'Cambria', 'Arial', 'Times New Roman'];

/** Ask the engine for the Design tab state. */
function useDesign(run: Run): DesignState | null {
  const [state, setState] = useState<DesignState | null>(null);
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'design' }).then((r) => {
      if (alive && r?.design) setState(r.design);
    });
    return () => {
      alive = false;
    };
  }, [run]);
  return state;
}

const HEX = /^#?([0-9a-f]{6})$/i;

/** `RRGGBB` of a typed colour, or null. */
export function parseHex(v: string): string | null {
  const m = HEX.exec(v.trim());
  return m?.[1] ? m[1].toUpperCase() : null;
}

/**
 * One tile of an art motif, centred on (cx, cy), `along` × `across` points
 * (`horizontal`: the edge runs along x). Mirrors `bo_layout`'s motif drawing.
 */
export function motifTile(
  style: string,
  cx: number,
  cy: number,
  along: number,
  across: number,
  horizontal: boolean,
  color: string,
  key: string,
): ReactNode {
  const p = (u: number, v: number) => (horizontal ? [cx + u, cy + v] : [cx + v, cy + u]);
  const pts = (list: [number, number][]) => list.map(([u, v]) => p(u, v).join(',')).join(' ');
  const a = along / 2;
  const s = across / 2;
  const r = Math.min(a, s);
  const [x, y] = p(0, 0);
  switch (style) {
    case 'basicBlackDots':
      return <circle key={key} cx={x} cy={y} r={r * 0.6} fill={color} />;
    case 'basicWhiteDots':
      return (
        <circle
          key={key}
          cx={x}
          cy={y}
          r={r * 0.55}
          fill="none"
          stroke={color}
          strokeWidth={r * 0.25}
        />
      );
    case 'basicBlackSquares':
      return (
        <rect
          key={key}
          x={x! - r * 0.6}
          y={y! - r * 0.6}
          width={r * 1.2}
          height={r * 1.2}
          fill={color}
        />
      );
    case 'basicWhiteSquares':
      return (
        <rect
          key={key}
          x={x! - r * 0.55}
          y={y! - r * 0.55}
          width={r * 1.1}
          height={r * 1.1}
          fill="none"
          stroke={color}
          strokeWidth={r * 0.2}
        />
      );
    case 'diamondsGray':
      return (
        <polygon
          key={key}
          points={pts([
            [-a * 0.85, 0],
            [0, -s * 0.85],
            [a * 0.85, 0],
            [0, s * 0.85],
          ])}
          fill={color}
        />
      );
    case 'triangles':
      return (
        <polygon
          key={key}
          points={pts([
            [-a * 0.8, s * 0.75],
            [0, -s * 0.75],
            [a * 0.8, s * 0.75],
          ])}
          fill={color}
        />
      );
    case 'zigZag':
      return (
        <polyline
          key={key}
          points={pts([
            [-a, s * 0.6],
            [0, -s * 0.6],
            [a, s * 0.6],
          ])}
          fill="none"
          stroke={color}
          strokeWidth={across * 0.14}
        />
      );
    case 'waveline': {
      const [x0, y0] = p(-a, 0);
      const [c1x, c1y] = p(-a / 2, -s * 1.1);
      const [c2x, c2y] = p(a / 2, s * 1.1);
      const [x1, y1] = p(a, 0);
      return (
        <path
          key={key}
          d={`M${x0} ${y0}Q${c1x} ${c1y} ${x} ${y}Q${c2x} ${c2y} ${x1} ${y1}`}
          fill="none"
          stroke={color}
          strokeWidth={across * 0.14}
        />
      );
    }
    case 'checkered':
      return (
        <g key={key} fill={color}>
          <polygon
            points={pts([
              [-a, -s],
              [0, -s],
              [0, 0],
              [-a, 0],
            ])}
          />
          <polygon
            points={pts([
              [0, 0],
              [a, 0],
              [a, s],
              [0, s],
            ])}
          />
        </g>
      );
    case 'sawtooth':
      return (
        <polygon
          key={key}
          points={pts([
            [-a, s],
            [a, -s],
            [a, s],
          ])}
          fill={color}
        />
      );
    default:
      return null;
  }
}

/** A strip of `n` motif tiles (art style samples). */
export function MotifSample({ style, color = 'currentColor' }: { style: string; color?: string }) {
  const tiles = [0, 1, 2, 3, 4].map((i) =>
    motifTile(style, 6 + i * 12, 6, 12, 12, true, color, `${i}`),
  );
  return (
    <svg width={60} height={12} viewBox="0 0 60 12" aria-hidden="true" data-motif={style}>
      {tiles}
    </svg>
  );
}

/** Miniature page with the borders of `spec` (a 120 × 170 sheet). */
export function BorderPreview({ spec }: { spec: PageBordersSpec | null }) {
  const W = 120;
  const H = 170;
  const items: ReactNode[] = [];
  if (spec) {
    const color = spec.color ? `#${spec.color}` : '#000000';
    const art = ART_STYLES.includes(spec.style);
    const w = art ? Math.max(4, spec.size / 2) : Math.max(0.75, spec.size / 8);
    const inset = spec.offsetFromPage ? 6 + w / 2 : 14 + w / 2;
    const [top, left, bottom, right] = spec.sides ?? [true, true, true, true];
    const edges: [boolean, boolean, number, number, number][] = [
      [top, true, inset, inset - w / 2, W - inset + w / 2],
      [bottom, true, H - inset, inset - w / 2, W - inset + w / 2],
      [left, false, inset, inset + (art ? w / 2 : -w / 2), H - inset - (art ? w / 2 : -w / 2)],
      [right, false, W - inset, inset + (art ? w / 2 : -w / 2), H - inset - (art ? w / 2 : -w / 2)],
    ];
    edges.forEach(([on, horizontal, at, from, to], e) => {
      if (!on) return;
      if (art) {
        const n = Math.max(1, Math.round((to - from) / w));
        const step = (to - from) / n;
        for (let i = 0; i < n; i++) {
          const c = from + step * (i + 0.5);
          items.push(
            motifTile(
              spec.style,
              horizontal ? c : at,
              horizontal ? at : c,
              step,
              w,
              horizontal,
              color,
              `${e}-${i}`,
            ),
          );
        }
        return;
      }
      const dash =
        spec.style === 'dotted'
          ? `${w} ${w * 2}`
          : spec.style === 'dashed'
            ? `${w * 3} ${w * 2}`
            : undefined;
      const line = (off: number, k: string, sw: number) =>
        horizontal ? (
          <line
            key={k}
            x1={from}
            y1={at + off}
            x2={to}
            y2={at + off}
            stroke={color}
            strokeWidth={sw}
            strokeDasharray={dash}
          />
        ) : (
          <line
            key={k}
            x1={at + off}
            y1={from}
            x2={at + off}
            y2={to}
            stroke={color}
            strokeWidth={sw}
            strokeDasharray={dash}
          />
        );
      if (spec.style === 'double') {
        const sw = Math.max(0.5, w / 3);
        items.push(line(-sw, `${e}a`, sw), line(sw, `${e}b`, sw));
      } else {
        items.push(line(0, `${e}`, spec.style === 'thick' ? w * 1.5 : w));
      }
      if (spec.shadow && (e === 1 || e === 3)) items.push(line(w, `${e}s`, w));
    });
  }
  return (
    <svg
      className={own.preview}
      width={W}
      height={H}
      viewBox={`0 0 ${W} ${H}`}
      data-testid="borders-preview"
      role="img"
    >
      <rect x={0} y={0} width={W} height={H} fill="#ffffff" stroke="#c8c8c8" />
      {[0, 1, 2, 3, 4, 5, 6].map((i) => (
        <rect key={i} x={24} y={32 + i * 16} width={72} height={4} fill="#d6d6d6" />
      ))}
      {items}
    </svg>
  );
}

/** Watermark dropdown: text presets, custom watermark, remove. */
export function WatermarkMenu({
  run,
  disabled,
  onCustom,
}: {
  run: Run;
  disabled: boolean;
  onCustom: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Popover
      label={t('commands.design.watermark')}
      disabled={disabled}
      testId="watermark-menu"
      trigger={
        <>
          <Droplets size={16} />
          <span className={own.triggerLabel}>{t('commands.design.watermark')}</span>
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        <div className={styles.menu} role="menu">
          {WATERMARK_PRESETS.map((p) => (
            <button
              key={p}
              type="button"
              role="menuitem"
              data-preset={p}
              className={own.preset}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                close();
                void run({ cmd: 'setWatermark', watermark: { text: t(`design.presets.${p}`) } });
              }}
            >
              <span className={own.presetSample}>{t(`design.presets.${p}`)}</span>
            </button>
          ))}
          <button
            type="button"
            role="menuitem"
            data-command="design.customWatermark"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              close();
              onCustom();
            }}
          >
            {t('commands.design.customWatermark')}
          </button>
          <button
            type="button"
            role="menuitem"
            data-command="design.removeWatermark"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              close();
              void run({ cmd: 'setWatermark' });
            }}
          >
            {t('commands.design.removeWatermark')}
          </button>
        </div>
      )}
    </Popover>
  );
}

function ThemeList({ run, close }: { run: Run; close: () => void }) {
  const { t } = useTranslation();
  const design = useDesign(run);
  if (!design) return <div className={styles.hint}>…</div>;
  return (
    <div className={own.gallery} role="menu">
      {design.themes.map((th) => (
        <button
          key={th.id}
          type="button"
          role="menuitemradio"
          aria-checked={design.theme === th.id}
          data-theme-id={th.id}
          className={own.galleryItem}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => {
            close();
            void run({ cmd: 'applyTheme', id: th.id });
          }}
        >
          <span className={own.galleryName}>{t(`design.themes.${th.id}`)}</span>
          <span className={own.swatches}>
            {th.colors.slice(4, 10).map((c) => (
              <span key={c} className={own.swatch} style={{ background: `#${c}` }} />
            ))}
          </span>
          <span className={styles.hint}>
            {t('design.themeFonts', { major: th.major, minor: th.minor })}
          </span>
        </button>
      ))}
    </div>
  );
}

/** Theme gallery (BlueOffice themes: own palettes and font pairs). */
export function ThemeGallery({ run, disabled }: { run: Run; disabled: boolean }) {
  const { t } = useTranslation();
  return (
    <Popover
      label={t('commands.design.themes')}
      disabled={disabled}
      testId="theme-gallery"
      trigger={
        <>
          <Palette size={16} />
          <span className={own.triggerLabel}>{t('commands.design.themes')}</span>
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => <ThemeList run={run} close={close} />}
    </Popover>
  );
}

/** Style set gallery (paragraph spacing presets). */
export function StyleSetGallery({ run, disabled }: { run: Run; disabled: boolean }) {
  const { t } = useTranslation();
  return (
    <Popover
      label={t('commands.design.styleSets')}
      disabled={disabled}
      testId="style-set-gallery"
      trigger={
        <>
          <Rows3 size={16} />
          <span className={own.triggerLabel}>{t('commands.design.styleSets')}</span>
          <ChevronDown size={12} />
        </>
      }
    >
      {(close) => (
        <div className={own.gallery} role="menu">
          {Object.entries(STYLE_SET_SPACING).map(([id, [after, line]]) => (
            <button
              key={id}
              type="button"
              role="menuitem"
              data-style-set={id}
              className={own.galleryItem}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                close();
                void run({ cmd: 'applyStyleSet', id });
              }}
            >
              <span className={own.galleryName}>{t(`design.styleSets.${id}`)}</span>
              <span className={own.lines} style={{ gap: `${2 + after / 2}px` }}>
                {[0, 1, 2].map((i) => (
                  <span key={i} style={{ height: 3, marginBottom: (line - 1) * 4 }} />
                ))}
              </span>
              <span className={styles.hint}>{t('design.styleSetHint', { after, line })}</span>
            </button>
          ))}
        </div>
      )}
    </Popover>
  );
}

type Setting = 'none' | 'box' | 'shadow' | 'custom';
type ApplyTo = 'document' | 'section' | 'sectionFirst' | 'sectionNotFirst';

/** The page borders command of the dialog's values. */
export function bordersCommand(
  setting: Setting,
  spec: PageBordersSpec,
  applyTo: ApplyTo,
): EditCommand {
  const scope = applyTo === 'document' ? 'document' : 'section';
  if (setting === 'none') return { cmd: 'setPageBorders', scope };
  const display: PageBorderDisplay =
    applyTo === 'sectionFirst'
      ? 'firstPage'
      : applyTo === 'sectionNotFirst'
        ? 'notFirstPage'
        : 'allPages';
  const borders: PageBordersSpec = {
    ...spec,
    sides: setting === 'custom' ? spec.sides : [true, true, true, true],
    shadow: setting === 'shadow',
    display,
  };
  return { cmd: 'setPageBorders', borders, scope };
}

/** Page borders dialog: setting, line or art style, colour, width, edges, apply to, offset. */
export function PageBordersDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [design, setDesign] = useState<DesignState | null>(null);
  const [setting, setSetting] = useState<Setting>('box');
  const [spec, setSpec] = useState<PageBordersSpec>({
    style: 'single',
    size: 4,
    color: null,
    space: 24,
    sides: [true, true, true, true],
    offsetFromPage: false,
    behind: false,
  });
  const [applyTo, setApplyTo] = useState<ApplyTo>('document');
  const [colorText, setColorText] = useState('');
  // Start from the borders of the caret's section.
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'design' }).then((r) => {
      const d = r?.design;
      if (!alive || !d) return;
      setDesign(d);
      const b = d.borders;
      if (!b) return;
      setSpec({ ...b });
      setColorText(b.color ?? '');
      setSetting(b.sides.every(Boolean) ? (b.shadow ? 'shadow' : 'box') : 'custom');
      setApplyTo(
        b.display === 'firstPage'
          ? 'sectionFirst'
          : b.display === 'notFirstPage'
            ? 'sectionNotFirst'
            : 'document',
      );
    });
    return () => {
      alive = false;
    };
  }, [run]);
  const art = ART_STYLES.includes(spec.style);
  const patch = (p: Partial<PageBordersSpec>) => setSpec((s) => ({ ...s, ...p }));
  const sides = spec.sides ?? [true, true, true, true];
  const shown =
    setting === 'none'
      ? null
      : {
          ...spec,
          sides:
            setting === 'custom' ? sides : ([true, true, true, true] as PageBordersSpec['sides']),
          shadow: setting === 'shadow',
        };
  const num = (v: string, min: number, max: number) => {
    const n = Math.round(Number(v));
    return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : min;
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('design.borders.title')}
      width={560}
      testId="page-borders-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('design.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="page-borders-ok"
            onClick={() => {
              onClose();
              void run(bordersCommand(setting, spec, applyTo));
            }}
          >
            {t('design.ok')}
          </button>
        </>
      }
    >
      <div className={own.columns}>
        <div className={own.settings} role="radiogroup" aria-label={t('design.borders.setting')}>
          {(['none', 'box', 'shadow', 'custom'] as const).map((s) => (
            <button
              key={s}
              type="button"
              role="radio"
              aria-checked={setting === s}
              data-testid={`borders-setting-${s}`}
              className={setting === s ? styles.dialogPrimary : styles.dialogButton}
              onClick={() => setSetting(s)}
            >
              {t(`design.borders.${s}`)}
            </button>
          ))}
        </div>
        <div className={styles.formGrid}>
          <label className={styles.field}>
            <span>{t('design.borders.style')}</span>
            <select
              data-testid="borders-style"
              value={art ? '' : spec.style}
              onChange={(e) => patch({ style: e.target.value, size: 4 })}
            >
              {art && <option value="">—</option>}
              {LINE_STYLES.map((s) => (
                <option key={s} value={s}>
                  {t(`design.lines.${s}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.field}>
            <span>{t('design.borders.art')}</span>
            <select
              data-testid="borders-art"
              value={art ? spec.style : ''}
              onChange={(e) =>
                e.target.value
                  ? patch({ style: e.target.value, size: 12 })
                  : patch({ style: 'single', size: 4 })
              }
            >
              <option value="">{t('design.borders.noArt')}</option>
              {(design?.artStyles ?? ART_STYLES).map((s) => (
                <option key={s} value={s}>
                  {t(`design.art.${s}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.field}>
            <span>{t('design.borders.width')}</span>
            {art ? (
              <input
                type="number"
                min={1}
                max={31}
                data-testid="borders-width"
                value={spec.size}
                onChange={(e) => patch({ size: num(e.target.value, 1, 31) })}
              />
            ) : (
              <select
                data-testid="borders-width"
                value={spec.size}
                onChange={(e) => patch({ size: Number(e.target.value) })}
              >
                {LINE_WIDTHS.map((w) => (
                  <option key={w} value={w}>
                    {w / 8} pt
                  </option>
                ))}
              </select>
            )}
            {art && <span className={styles.unit}>pt</span>}
          </label>
          <label className={styles.field}>
            <span>{t('design.borders.color')}</span>
            <input
              data-testid="borders-color"
              placeholder={t('design.borders.automatic')}
              value={colorText}
              onChange={(e) => {
                setColorText(e.target.value);
                patch({ color: parseHex(e.target.value) });
              }}
            />
          </label>
          <label className={styles.field}>
            <span>{t('design.borders.applyTo')}</span>
            <select
              data-testid="borders-apply-to"
              value={applyTo}
              onChange={(e) => setApplyTo(e.target.value as ApplyTo)}
            >
              {(['document', 'section', 'sectionFirst', 'sectionNotFirst'] as const).map((a) => (
                <option key={a} value={a}>
                  {t(`design.borders.${a}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.field}>
            <span>{t('design.borders.measureFrom')}</span>
            <select
              data-testid="borders-measure"
              value={spec.offsetFromPage ? 'page' : 'text'}
              onChange={(e) => patch({ offsetFromPage: e.target.value === 'page' })}
            >
              <option value="text">{t('design.borders.fromText')}</option>
              <option value="page">{t('design.borders.fromPage')}</option>
            </select>
          </label>
          <label className={styles.field}>
            <span>{t('design.borders.spacing')}</span>
            <input
              type="number"
              min={0}
              max={31}
              data-testid="borders-spacing"
              value={spec.space ?? 0}
              onChange={(e) => patch({ space: num(e.target.value, 0, 31) })}
            />
            <span className={styles.unit}>pt</span>
          </label>
          <label className={styles.field}>
            <input
              type="checkbox"
              data-testid="borders-behind"
              checked={!!spec.behind}
              onChange={(e) => patch({ behind: e.target.checked })}
            />
            {t('design.borders.behind')}
          </label>
          {!spec.offsetFromPage &&
            (['surroundHeader', 'surroundFooter'] as const).map((k) => (
              <label key={k} className={styles.field}>
                <input
                  type="checkbox"
                  data-testid={`borders-${k}`}
                  checked={spec[k] !== false}
                  onChange={(e) => patch({ [k]: e.target.checked })}
                />
                {t(`design.borders.${k}`)}
              </label>
            ))}
          {setting === 'custom' && (
            <fieldset className={own.sides}>
              <legend>{t('design.borders.sides')}</legend>
              {(['top', 'left', 'bottom', 'right'] as const).map((side, i) => (
                <label key={side}>
                  <input
                    type="checkbox"
                    data-testid={`borders-side-${side}`}
                    checked={sides[i]}
                    onChange={(e) => {
                      const next = [...sides] as [boolean, boolean, boolean, boolean];
                      next[i] = e.target.checked;
                      patch({ sides: next });
                    }}
                  />
                  {t(`design.borders.${side}`)}
                </label>
              ))}
            </fieldset>
          )}
        </div>
        <div className={own.previewBox}>
          <div className={styles.menuTitle}>{t('design.borders.preview')}</div>
          <BorderPreview spec={shown} />
        </div>
      </div>
    </Dialog>
  );
}

/** The watermark command of the dialog's values (null: nothing to send yet). */
export function watermarkCommand(
  kind: 'none' | 'text' | 'picture',
  text: WatermarkSpec,
  picture: string | null,
  scale: number | null,
  washout: boolean,
): EditCommand | null {
  if (kind === 'none') return { cmd: 'setWatermark' };
  if (kind === 'picture') {
    if (!picture) return null;
    return { cmd: 'setWatermark', watermark: { picture, scale, washout } };
  }
  if (!text.text?.trim()) return null;
  return { cmd: 'setWatermark', watermark: { ...text, text: text.text.trim() } };
}

/** Custom watermark dialog: none, text (font, size, colour, transparency, layout) or picture. */
export function WatermarkDialog({
  run,
  onClose,
  pickPicture,
}: {
  run: Run;
  onClose: () => void;
  /** Ask the user for a picture; resolves to its bytes in base64 and file name. */
  pickPicture: () => Promise<{ base64: string; name: string } | null>;
}) {
  const { t } = useTranslation();
  const [design, setDesign] = useState<DesignState | null>(null);
  const [kind, setKind] = useState<'none' | 'text' | 'picture'>('text');
  const [text, setText] = useState<WatermarkSpec>({
    text: t('design.presets.draft'),
    font: 'Calibri',
    size: null,
    color: 'C0C0C0',
    semitransparent: true,
    diagonal: true,
    bold: false,
    italic: false,
  });
  const [colorText, setColorText] = useState('C0C0C0');
  const [picture, setPicture] = useState<{ base64: string; name: string } | null>(null);
  const [scale, setScale] = useState<number | null>(null);
  const [washout, setWashout] = useState(true);
  // Start from the document's watermark.
  useEffect(() => {
    let alive = true;
    void run({ cmd: 'design' }).then((r) => {
      const d = r?.design;
      if (!alive || !d) return;
      setDesign(d);
      const w = d.watermark;
      if (!w) return;
      if (w.picture) {
        setKind('picture');
        setWashout(w.washout);
        return;
      }
      setKind('text');
      setText((s) => ({
        ...s,
        text: w.text ?? s.text,
        font: w.font || s.font,
        color: w.color ?? s.color,
        semitransparent: w.semitransparent,
        diagonal: w.diagonal,
      }));
      setColorText(w.color ?? '');
    });
    return () => {
      alive = false;
    };
  }, [run]);
  const patch = (p: Partial<WatermarkSpec>) => setText((s) => ({ ...s, ...p }));
  const command = watermarkCommand(kind, text, picture?.base64 ?? null, scale, washout);
  const current = design?.watermark;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('design.watermark.title')}
      width={460}
      testId="watermark-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('design.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="watermark-ok"
            disabled={!command}
            onClick={() => {
              if (!command) return;
              onClose();
              void run(command);
            }}
          >
            {t('design.ok')}
          </button>
        </>
      }
    >
      {current && (
        <div className={styles.hint} data-testid="watermark-current">
          {current.picture
            ? t('design.watermark.currentPicture')
            : t('design.watermark.current', { text: current.text ?? '' })}
        </div>
      )}
      <div className={styles.menuRow} role="radiogroup" aria-label={t('design.watermark.title')}>
        {(['none', 'text', 'picture'] as const).map((k) => (
          <label key={k} className={styles.field}>
            <input
              type="radio"
              name="watermark-kind"
              data-testid={`watermark-kind-${k}`}
              checked={kind === k}
              onChange={() => setKind(k)}
            />
            {t(`design.watermark.${k}`)}
          </label>
        ))}
      </div>
      {kind === 'text' && (
        <div className={styles.formGrid}>
          <label className={styles.field}>
            <span>{t('design.watermark.content')}</span>
            <input
              data-testid="watermark-text"
              list="bo-watermark-presets"
              maxLength={200}
              value={text.text ?? ''}
              onChange={(e) => patch({ text: e.target.value })}
            />
            <datalist id="bo-watermark-presets">
              {WATERMARK_PRESETS.map((p) => (
                <option key={p} value={t(`design.presets.${p}`)} />
              ))}
            </datalist>
          </label>
          <label className={styles.field}>
            <span>{t('design.watermark.font')}</span>
            <select
              data-testid="watermark-font"
              value={text.font}
              onChange={(e) => patch({ font: e.target.value })}
            >
              {WATERMARK_FONTS.map((f) => (
                <option key={f} value={f}>
                  {f}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.field}>
            <span>{t('design.watermark.size')}</span>
            <input
              type="number"
              min={8}
              max={400}
              data-testid="watermark-size"
              placeholder={t('design.watermark.auto')}
              value={text.size ?? ''}
              onChange={(e) => {
                const v = Number(e.target.value);
                patch({
                  size: e.target.value && Number.isFinite(v) ? Math.min(400, Math.max(8, v)) : null,
                });
              }}
            />
            <span className={styles.unit}>pt</span>
          </label>
          <label className={styles.field}>
            <span>{t('design.watermark.color')}</span>
            <input
              data-testid="watermark-color"
              value={colorText}
              onChange={(e) => {
                setColorText(e.target.value);
                patch({ color: parseHex(e.target.value) });
              }}
            />
            <span
              className={own.swatch}
              style={{ background: text.color ? `#${text.color}` : 'transparent' }}
            />
          </label>
          <label className={styles.field}>
            <input
              type="checkbox"
              data-testid="watermark-semitransparent"
              checked={!!text.semitransparent}
              onChange={(e) => patch({ semitransparent: e.target.checked })}
            />
            {t('design.watermark.semitransparent')}
          </label>
          <label className={styles.field}>
            <span>{t('design.watermark.layout')}</span>
            <select
              data-testid="watermark-layout"
              value={text.diagonal ? 'diagonal' : 'horizontal'}
              onChange={(e) => patch({ diagonal: e.target.value === 'diagonal' })}
            >
              <option value="diagonal">{t('design.watermark.diagonal')}</option>
              <option value="horizontal">{t('design.watermark.horizontal')}</option>
            </select>
          </label>
          <label className={styles.field}>
            <input
              type="checkbox"
              data-testid="watermark-bold"
              checked={!!text.bold}
              onChange={(e) => patch({ bold: e.target.checked })}
            />
            {t('design.watermark.bold')}
          </label>
          <label className={styles.field}>
            <input
              type="checkbox"
              data-testid="watermark-italic"
              checked={!!text.italic}
              onChange={(e) => patch({ italic: e.target.checked })}
            />
            {t('design.watermark.italic')}
          </label>
        </div>
      )}
      {kind === 'picture' && (
        <div className={styles.formGrid}>
          <div className={styles.field}>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="watermark-choose"
              onClick={() => void pickPicture().then((p) => p && setPicture(p))}
            >
              {t('design.watermark.choosePicture')}
            </button>
            <span className={styles.hint} data-testid="watermark-picture-name">
              {picture?.name ?? t('design.watermark.noPicture')}
            </span>
          </div>
          <label className={styles.field}>
            <span>{t('design.watermark.scale')}</span>
            <select
              data-testid="watermark-scale"
              value={scale ?? ''}
              onChange={(e) => setScale(e.target.value ? Number(e.target.value) : null)}
            >
              <option value="">{t('design.watermark.auto')}</option>
              {[50, 100, 150, 200].map((s) => (
                <option key={s} value={s}>
                  {s} %
                </option>
              ))}
            </select>
          </label>
          <label className={styles.field}>
            <input
              type="checkbox"
              data-testid="watermark-washout"
              checked={washout}
              onChange={(e) => setWashout(e.target.checked)}
            />
            {t('design.watermark.washout')}
          </label>
        </div>
      )}
    </Dialog>
  );
}
