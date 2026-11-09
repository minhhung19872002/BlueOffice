import { useEffect, useState, type ReactNode } from 'react';
import { Dialog } from './Dialog';
import { Popover } from './Popover';
import styles from './PictureTools.module.css';

/**
 * Picture tools shared by BlueDoc and BlueSlide (CORE-PIC-001): galleries of corrections,
 * colour and transparency with thumbnails the engine draws from the selected picture itself,
 * the crop menu (aspect ratio, shape, fill, fit, custom), the compress dialog and the
 * adjustment dialog. The presets are BlueOffice's own; every string comes from the host (i18n).
 * Values mirror `bo_model::picture::PictureFx` (1/1000 of a percent).
 */

/** Colour adjustments of a picture (same shape as the platform's `PictureFx`). */
export interface PictureFxValue {
  bright?: number;
  contrast?: number;
  sharpen?: number;
  saturation?: number | null;
  grayscale?: boolean;
  bilevel?: number | null;
  duotone?: [number, number] | null;
  colorChange?: { from: number; to: number; alpha: number } | null;
  alpha?: number | null;
}

/** A gallery entry. */
export interface PicturePreset {
  id: string;
  /** Adjustments it sets (merged into the current ones, see `mergeFx`). */
  fx: PictureFxValue;
}

/** Settings of the compress dialog. */
export interface PictureCompressValue {
  ppi: number | null;
  dropCropped: boolean;
  jpegQuality: number | null;
  all: boolean;
}

/** Strings of the picture tools (from the host's i18n). */
export interface PictureToolsLabels {
  corrections: string;
  sharpness: string;
  brightnessContrast: string;
  color: string;
  saturation: string;
  recolor: string;
  transparency: string;
  crop: string;
  cropAspect: string;
  cropShape: string;
  fill: string;
  fit: string;
  customCrop: string;
  options: string;
  loading: string;
  /** Name of a preset (`sharp.soften50`, `recolor.sepia`…). */
  preset: (id: string) => string;
  /** Name of a crop shape preset. */
  shape: (preset: string) => string;
  // Dialogs.
  ok: string;
  cancel: string;
  apply: string;
  adjustTitle: string;
  bright: string;
  contrast: string;
  sharpen: string;
  alpha: string;
  reset: string;
  cropTitle: string;
  left: string;
  top: string;
  right: string;
  bottom: string;
  compressTitle: string;
  resolution: string;
  keepResolution: string;
  ppi: (n: number) => string;
  dropCropped: string;
  jpegQuality: string;
  scopeAll: string;
  current: (info: { pixels: string; ppi: number; size: string }) => string;
  saved: (bytes: string) => string;
  nothingSaved: string;
}

const pct = (v: number) => Math.round(v * 1000);

/** Sharpen / soften gallery. */
export const SHARPNESS_PRESETS: PicturePreset[] = [-50, -25, 0, 25, 50].map((v) => ({
  id: `sharp.${v < 0 ? 'soften' : v > 0 ? 'sharpen' : 'none'}${Math.abs(v) || ''}`,
  fx: { sharpen: pct(v) },
}));

/** Brightness × contrast grid (5 × 5). */
export const LUM_PRESETS: PicturePreset[] = [-40, -20, 0, 20, 40].flatMap((c) =>
  [-40, -20, 0, 20, 40].map((b) => ({
    id: `lum.${b}.${c}`,
    fx: { bright: pct(b), contrast: pct(c) },
  })),
);

/** Saturation gallery. */
export const SATURATION_PRESETS: PicturePreset[] = [0, 33, 66, 100, 150, 200, 300].map((s) => ({
  id: `sat.${s}`,
  fx: { saturation: pct(s) },
}));

/** Two-tone colours of the recolour gallery (BlueOffice's own palette): dark, light. */
export const DUOTONES: Record<string, [number, number]> = {
  sepia: [0x2b1d0e, 0xf2e3c6],
  blue: [0x1f3a5f, 0xdce8f5],
  green: [0x1e4d2b, 0xdff0e2],
  orange: [0x7a3e00, 0xfce6cc],
  red: [0x6b1a1a, 0xf7d9d9],
  purple: [0x3f2a5c, 0xe8dff5],
  teal: [0x0f4c4c, 0xd6f0ef],
};

const RECOLOR_RESET: PictureFxValue = { grayscale: false, bilevel: null, duotone: null };

/** Recolour gallery: none, greyscale, black and white, washed out, two-tone. */
export const RECOLOR_PRESETS: PicturePreset[] = [
  { id: 'recolor.none', fx: { ...RECOLOR_RESET } },
  { id: 'recolor.grayscale', fx: { ...RECOLOR_RESET, grayscale: true } },
  { id: 'recolor.bw25', fx: { ...RECOLOR_RESET, bilevel: 25000 } },
  { id: 'recolor.bw50', fx: { ...RECOLOR_RESET, bilevel: 50000 } },
  { id: 'recolor.bw75', fx: { ...RECOLOR_RESET, bilevel: 75000 } },
  // Washed out: much brighter and flatter (BlueOffice's own values).
  { id: 'recolor.washout', fx: { ...RECOLOR_RESET, bright: 70000, contrast: -70000 } },
  ...Object.entries(DUOTONES).map(([k, d]) => ({
    id: `recolor.${k}`,
    fx: { ...RECOLOR_RESET, duotone: d },
  })),
];

/** Transparency gallery (alphaModFix). */
export const TRANSPARENCY_PRESETS: PicturePreset[] = [0, 15, 30, 50, 65, 80].map((t) => ({
  id: `alpha.${t}`,
  fx: { alpha: t === 0 ? null : pct(100 - t) },
}));

/** Aspect ratios of the crop menu. */
export const CROP_RATIOS = [
  '1:1',
  '2:3',
  '3:2',
  '3:4',
  '4:3',
  '4:5',
  '5:4',
  '9:16',
  '16:9',
  '16:10',
];

/** Preset shapes of the crop menu (§20.1.9.18 ST_ShapeType names). */
export const CROP_SHAPES = [
  'rect',
  'roundRect',
  'ellipse',
  'triangle',
  'diamond',
  'pentagon',
  'hexagon',
  'octagon',
  'star5',
  'heart',
  'cloud',
  'snip1Rect',
];

/** Current adjustments with a preset applied on top. */
export function mergeFx(current: PictureFxValue, preset: PictureFxValue): PictureFxValue {
  return { ...current, ...preset };
}

/** Human-readable byte count. */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

const pngUrl = (b64: string) => `data:image/png;base64,${b64}`;

interface GallerySection {
  title: string;
  presets: PicturePreset[];
  /** Grid columns. */
  columns?: number;
}

function Gallery({
  sections,
  current,
  load,
  onPick,
  labels,
  testId,
  close,
}: {
  sections: GallerySection[];
  current: PictureFxValue;
  load: (fx: PictureFxValue[]) => Promise<string[] | null>;
  onPick: (fx: PictureFxValue) => void;
  labels: PictureToolsLabels;
  testId: string;
  close: () => void;
}) {
  const [previews, setPreviews] = useState<string[] | null>(null);
  useEffect(() => {
    let live = true;
    const all = sections.flatMap((s) => s.presets.map((p) => mergeFx(current, p.fx)));
    load(all)
      .then((r) => {
        if (live) setPreviews(r ?? []);
      })
      .catch(() => {
        if (live) setPreviews([]);
      });
    return () => {
      live = false;
    };
    // The previews depend on the picture and its adjustments when opened.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [load]);
  let k = 0;
  return (
    <div className={styles.gallery} data-testid={testId}>
      {sections.map((s) => (
        <div key={s.title} className={styles.section}>
          <div className={styles.sectionTitle}>{s.title}</div>
          <div
            className={styles.grid}
            role="listbox"
            aria-label={s.title}
            style={{ gridTemplateColumns: `repeat(${s.columns ?? 5}, 64px)` }}
          >
            {s.presets.map((p) => {
              const src = previews?.[k++];
              const name = labels.preset(p.id);
              return (
                <button
                  key={p.id}
                  type="button"
                  role="option"
                  aria-selected={false}
                  className={styles.thumb}
                  title={name}
                  aria-label={name}
                  data-testid={`${testId}-${p.id}`}
                  onClick={() => {
                    close();
                    onPick(mergeFx(current, p.fx));
                  }}
                >
                  {src ? (
                    <img src={pngUrl(src)} alt="" draggable={false} />
                  ) : (
                    <span className={styles.placeholder}>{previews ? '' : '…'}</span>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

interface GalleryButtonProps {
  current: PictureFxValue;
  load: (fx: PictureFxValue[]) => Promise<string[] | null>;
  onPick: (fx: PictureFxValue) => void;
  labels: PictureToolsLabels;
  disabled?: boolean;
  icon?: ReactNode;
  /** Opens the adjustment dialog. */
  onOptions?: () => void;
}

function GalleryButton({
  title,
  sections,
  testId,
  current,
  load,
  onPick,
  labels,
  disabled,
  icon,
  onOptions,
}: GalleryButtonProps & { title: string; sections: GallerySection[]; testId: string }) {
  return (
    <Popover
      trigger={icon ?? <span>{title}</span>}
      label={title}
      disabled={disabled}
      testId={testId}
    >
      {(close) => (
        <>
          <Gallery
            sections={sections}
            current={current}
            load={load}
            onPick={onPick}
            labels={labels}
            testId={`${testId}-grid`}
            close={close}
          />
          {onOptions && (
            <button
              type="button"
              className={styles.menuItem}
              data-testid={`${testId}-options`}
              onClick={() => {
                close();
                onOptions();
              }}
            >
              {labels.options}
            </button>
          )}
        </>
      )}
    </Popover>
  );
}

/** "Corrections": sharpen/soften and brightness/contrast. */
export function PictureCorrectionsGallery(props: GalleryButtonProps) {
  const l = props.labels;
  return (
    <GalleryButton
      {...props}
      title={l.corrections}
      testId="picture-corrections"
      sections={[
        { title: l.sharpness, presets: SHARPNESS_PRESETS },
        { title: l.brightnessContrast, presets: LUM_PRESETS },
      ]}
    />
  );
}

/** "Colour": saturation and recolour. */
export function PictureColorGallery(props: GalleryButtonProps) {
  const l = props.labels;
  return (
    <GalleryButton
      {...props}
      title={l.color}
      testId="picture-color"
      sections={[
        { title: l.saturation, presets: SATURATION_PRESETS, columns: 7 },
        { title: l.recolor, presets: RECOLOR_PRESETS, columns: 7 },
      ]}
    />
  );
}

/** "Transparency" gallery. */
export function PictureTransparencyGallery(props: GalleryButtonProps) {
  return (
    <GalleryButton
      {...props}
      title={props.labels.transparency}
      testId="picture-transparency"
      sections={[{ title: props.labels.transparency, presets: TRANSPARENCY_PRESETS, columns: 6 }]}
    />
  );
}

/** What the crop menu asks for. */
export type CropAction =
  | { kind: 'aspect'; ratio: string }
  | { kind: 'shape'; preset: string }
  | { kind: 'fill' }
  | { kind: 'fit' }
  | { kind: 'custom' };

/** Crop menu: aspect ratios, shapes, fill, fit, custom crop. */
export function PictureCropMenu({
  labels,
  onCrop,
  disabled,
  icon,
}: {
  labels: PictureToolsLabels;
  onCrop: (a: CropAction) => void;
  disabled?: boolean;
  icon?: ReactNode;
}) {
  return (
    <Popover
      trigger={icon ?? <span>{labels.crop}</span>}
      label={labels.crop}
      disabled={disabled}
      testId="picture-crop"
    >
      {(close) => {
        const go = (a: CropAction) => {
          close();
          onCrop(a);
        };
        return (
          <div className={styles.menu} data-testid="picture-crop-menu">
            <div className={styles.sectionTitle}>{labels.cropAspect}</div>
            <div className={styles.chips}>
              {CROP_RATIOS.map((r) => (
                <button
                  key={r}
                  type="button"
                  className={styles.chip}
                  data-testid={`picture-crop-ratio-${r.replace(':', 'x')}`}
                  onClick={() => go({ kind: 'aspect', ratio: r })}
                >
                  {r}
                </button>
              ))}
            </div>
            <div className={styles.sectionTitle}>{labels.cropShape}</div>
            <div className={styles.chips}>
              {CROP_SHAPES.map((s) => (
                <button
                  key={s}
                  type="button"
                  className={styles.chip}
                  data-testid={`picture-crop-shape-${s}`}
                  onClick={() => go({ kind: 'shape', preset: s })}
                >
                  {labels.shape(s)}
                </button>
              ))}
            </div>
            <button type="button" className={styles.menuItem} onClick={() => go({ kind: 'fill' })}>
              {labels.fill}
            </button>
            <button type="button" className={styles.menuItem} onClick={() => go({ kind: 'fit' })}>
              {labels.fit}
            </button>
            <button
              type="button"
              className={styles.menuItem}
              data-testid="picture-crop-custom"
              onClick={() => go({ kind: 'custom' })}
            >
              {labels.customCrop}
            </button>
          </div>
        );
      }}
    </Popover>
  );
}

function NumberRow({
  label,
  value,
  min,
  max,
  onChange,
  testId,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
  testId: string;
}) {
  return (
    <label className={styles.row}>
      <span>{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
      />
      <input
        type="number"
        min={min}
        max={max}
        value={value}
        data-testid={testId}
        className={styles.number}
        onChange={(e) => onChange(Math.max(min, Math.min(max, Number(e.target.value) || 0)))}
      />
      <span>%</span>
    </label>
  );
}

/** Adjustment dialog: brightness, contrast, sharpness, saturation, transparency (in %). */
export function PictureAdjustDialog({
  open,
  fx,
  labels,
  onApply,
  onClose,
}: {
  open: boolean;
  fx: PictureFxValue;
  labels: PictureToolsLabels;
  onApply: (fx: PictureFxValue) => void;
  onClose: () => void;
}) {
  const toPct = (v: number | null | undefined, d = 0) => Math.round((v ?? d * 1000) / 1000);
  const [v, setV] = useState({
    bright: toPct(fx.bright),
    contrast: toPct(fx.contrast),
    sharpen: toPct(fx.sharpen),
    saturation: toPct(fx.saturation, 100),
    transparency: 100 - toPct(fx.alpha, 100),
  });
  const set = (k: keyof typeof v) => (n: number) => setV((o) => ({ ...o, [k]: n }));
  const value = (): PictureFxValue => ({
    ...fx,
    bright: pct(v.bright),
    contrast: pct(v.contrast),
    sharpen: pct(v.sharpen),
    saturation: v.saturation === 100 ? null : pct(v.saturation),
    alpha: v.transparency === 0 ? null : pct(100 - v.transparency),
  });
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={labels.adjustTitle}
      testId="picture-adjust-dialog"
      footer={
        <>
          <button
            type="button"
            onClick={() =>
              setV({ bright: 0, contrast: 0, sharpen: 0, saturation: 100, transparency: 0 })
            }
          >
            {labels.reset}
          </button>
          <button type="button" onClick={() => onApply(value())}>
            {labels.apply}
          </button>
          <button
            type="button"
            data-testid="picture-adjust-ok"
            onClick={() => {
              onApply(value());
              onClose();
            }}
          >
            {labels.ok}
          </button>
          <button type="button" onClick={onClose}>
            {labels.cancel}
          </button>
        </>
      }
    >
      <NumberRow
        label={labels.bright}
        value={v.bright}
        min={-100}
        max={100}
        onChange={set('bright')}
        testId="picture-adjust-bright"
      />
      <NumberRow
        label={labels.contrast}
        value={v.contrast}
        min={-100}
        max={100}
        onChange={set('contrast')}
        testId="picture-adjust-contrast"
      />
      <NumberRow
        label={labels.sharpen}
        value={v.sharpen}
        min={-100}
        max={100}
        onChange={set('sharpen')}
        testId="picture-adjust-sharpen"
      />
      <NumberRow
        label={labels.saturation}
        value={v.saturation}
        min={0}
        max={400}
        onChange={set('saturation')}
        testId="picture-adjust-saturation"
      />
      <NumberRow
        label={labels.alpha}
        value={v.transparency}
        min={0}
        max={100}
        onChange={set('transparency')}
        testId="picture-adjust-alpha"
      />
    </Dialog>
  );
}

/** Custom crop dialog: percentages cut from each edge. */
export function PictureCropDialog({
  open,
  crop,
  labels,
  onApply,
  onClose,
}: {
  open: boolean;
  crop?: [number, number, number, number];
  labels: PictureToolsLabels;
  onApply: (crop: [number, number, number, number]) => void;
  onClose: () => void;
}) {
  const [c, setC] = useState<number[]>((crop ?? [0, 0, 0, 0]).map((x) => x / 1000));
  const edge = (i: number, label: string) => (
    <label className={styles.row} key={i}>
      <span>{label}</span>
      <input
        type="number"
        min={0}
        max={99}
        step={0.5}
        value={c[i]}
        className={styles.number}
        data-testid={`picture-crop-${i}`}
        onChange={(e) => {
          const n = Math.max(0, Math.min(99, Number(e.target.value) || 0));
          setC((o) => o.map((x, j) => (j === i ? n : x)));
        }}
      />
      <span>%</span>
    </label>
  );
  const ok = c[0]! + c[2]! < 100 && c[1]! + c[3]! < 100;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={labels.cropTitle}
      testId="picture-crop-dialog"
      footer={
        <>
          <button
            type="button"
            disabled={!ok}
            data-testid="picture-crop-ok"
            onClick={() => {
              onApply(c.map((x) => Math.round(x * 1000)) as [number, number, number, number]);
              onClose();
            }}
          >
            {labels.ok}
          </button>
          <button type="button" onClick={onClose}>
            {labels.cancel}
          </button>
        </>
      }
    >
      {edge(0, labels.left)}
      {edge(1, labels.top)}
      {edge(2, labels.right)}
      {edge(3, labels.bottom)}
    </Dialog>
  );
}

/** Resolutions offered by the compress dialog (pixels per inch). */
export const COMPRESS_PPI = [330, 220, 150, 96];

/** "Compress pictures" dialog; shows the result of the last run. */
export function PictureCompressDialog({
  open,
  labels,
  info,
  report,
  onApply,
  onClose,
}: {
  open: boolean;
  labels: PictureToolsLabels;
  info?: { pixelWidth: number; pixelHeight: number; ppi: number; bytes: number } | null;
  report?: { pictures: number; before: number; after: number } | null;
  onApply: (v: PictureCompressValue) => void;
  onClose: () => void;
}) {
  const [v, setV] = useState<PictureCompressValue>({
    ppi: 150,
    dropCropped: true,
    jpegQuality: 85,
    all: false,
  });
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={labels.compressTitle}
      testId="picture-compress-dialog"
      footer={
        <>
          <button type="button" data-testid="picture-compress-ok" onClick={() => onApply(v)}>
            {labels.apply}
          </button>
          <button type="button" onClick={onClose}>
            {labels.cancel}
          </button>
        </>
      }
    >
      {info && (
        <p className={styles.note} data-testid="picture-compress-current">
          {labels.current({
            pixels: `${info.pixelWidth} × ${info.pixelHeight}`,
            ppi: Math.round(info.ppi),
            size: formatBytes(info.bytes),
          })}
        </p>
      )}
      <fieldset className={styles.fieldset}>
        <legend>{labels.resolution}</legend>
        {[...COMPRESS_PPI, null].map((p) => (
          <label key={p ?? 'keep'} className={styles.radio}>
            <input
              type="radio"
              name="picture-ppi"
              checked={v.ppi === p}
              data-testid={`picture-compress-ppi-${p ?? 'keep'}`}
              onChange={() => setV({ ...v, ppi: p })}
            />
            {p ? labels.ppi(p) : labels.keepResolution}
          </label>
        ))}
      </fieldset>
      <label className={styles.radio}>
        <input
          type="checkbox"
          checked={v.dropCropped}
          onChange={(e) => setV({ ...v, dropCropped: e.target.checked })}
        />
        {labels.dropCropped}
      </label>
      <label className={styles.row}>
        <span>{labels.jpegQuality}</span>
        <input
          type="number"
          min={10}
          max={100}
          value={v.jpegQuality ?? 85}
          className={styles.number}
          onChange={(e) =>
            setV({ ...v, jpegQuality: Math.max(10, Math.min(100, Number(e.target.value) || 85)) })
          }
        />
      </label>
      <label className={styles.radio}>
        <input
          type="checkbox"
          checked={v.all}
          data-testid="picture-compress-all"
          onChange={(e) => setV({ ...v, all: e.target.checked })}
        />
        {labels.scopeAll}
      </label>
      {report && (
        <p className={styles.note} data-testid="picture-compress-result" role="status">
          {report.after < report.before
            ? labels.saved(formatBytes(report.before - report.after))
            : labels.nothingSaved}
        </p>
      )}
    </Dialog>
  );
}
