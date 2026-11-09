/**
 * BlueSheet form controls over the grid ([MS-XLSX] control properties, legacy VML client
 * data, read by `bo_xlsx::formctl`): buttons, check boxes, option buttons, drop-down and list
 * boxes, scroll bars, up-down buttons, group frames and labels, drawn in BlueOffice's own
 * style and working through the engine (`formControl*` commands write the linked cell, one
 * undo step each). ActiveX controls show their fallback picture and are kept as they are.
 *
 * Right-click selects a control (design mode): drag moves it, Enter / double-click opens the
 * settings dialog, Delete removes it, Escape leaves design mode.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  CheckState,
  FormControlInfo,
  FormControlKind,
  FormatControlSpec,
  SheetCommand,
  SheetHost,
  SheetView,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import editorStyles from './SheetEditor.module.css';
import styles from './formControls.module.css';

/** A screen box in CSS pixels, relative to the grid canvas. */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A control with its screen box. */
export interface PlacedControl {
  info: FormControlInfo;
  box: Box;
}

/** Kinds the ribbon group inserts, in order. */
export const INSERTABLE: FormControlKind[] = [
  'button',
  'checkBox',
  'optionButton',
  'comboBox',
  'listBox',
  'scrollBar',
  'spinner',
  'groupBox',
  'label',
];

/** Kinds with a caption. */
const CAPTIONED: FormControlKind[] = ['button', 'checkBox', 'optionButton', 'groupBox', 'label'];
/** Kinds that write a linked cell. */
const LINKED: FormControlKind[] = [
  'checkBox',
  'optionButton',
  'comboBox',
  'listBox',
  'scrollBar',
  'spinner',
];

/**
 * Screen box of a control: `whole` is the rect of its anchor cells (from-cell to to-cell),
 * `toCell` the rect of the to-cell (null when off screen); rects and offsets in points.
 */
export function controlBox(
  info: Pick<FormControlInfo, 'fromOff' | 'toOff'>,
  whole: Box,
  toCell: Box | null,
  scale: number,
): Box {
  const x = whole.x + info.fromOff[0];
  const y = whole.y + info.fromOff[1];
  const right = toCell ? toCell.x + info.toOff[0] : whole.x + whole.w;
  const bottom = toCell ? toCell.y + info.toOff[1] : whole.y + whole.h;
  return {
    x: x * scale,
    y: y * scale,
    w: Math.max(right - x, 4) * scale,
    h: Math.max(bottom - y, 4) * scale,
  };
}

/** Places the controls of `list` on the current view (controls off screen are left out). */
export async function placeControls(
  host: Pick<SheetHost, 'rangeRect'>,
  view: SheetView,
  scale: number,
  list: FormControlInfo[],
): Promise<PlacedControl[]> {
  const placed = await Promise.all(
    list.map(async (info) => {
      const [whole, toCell] = await Promise.all([
        host.rangeRect(view, info.from[0], info.from[1], info.to[0], info.to[1]),
        host.rangeRect(view, info.to[0], info.to[1], info.to[0], info.to[1]),
      ]);
      return whole ? { info, box: controlBox(info, whole, toCell, scale) } : null;
    }),
  );
  return placed.filter((p): p is PlacedControl => !!p);
}

/** Item `sel` (1-based) moved by `delta`, kept within 1..count. */
export function moveSel(sel: number, delta: number, count: number): number {
  if (count === 0) return 0;
  const next = (sel === 0 ? (delta > 0 ? 0 : count + 1) : sel) + delta;
  return Math.min(Math.max(next, 1), count);
}

/** A scroll bar / up-down button is drawn across when flagged or wider than tall. */
export function isAcross(info: FormControlInfo, box: Box): boolean {
  return info.horiz || box.w > box.h;
}

export interface FormControlLayerProps {
  host: SheetHost;
  sheet: number;
  view: SheetView;
  /** CSS px per point. */
  scale: number;
  /** Changes whenever the workbook changed (reload). */
  revision: number;
  /** Runs a command and refreshes the editor. */
  exec: <T>(cmd: SheetCommand) => Promise<T | null>;
  /** Control in design mode (selected), if any. */
  selected: number | null;
  onSelect: (index: number | null) => void;
  /** Open the settings dialog for a control. */
  onFormat: (info: FormControlInfo) => void;
  /** Show a message in the status bar. */
  onNotice: (text: string) => void;
  /** Called with the current list after each load. */
  onList?: (list: FormControlInfo[]) => void;
}

/** The controls of the sheet over the grid. */
export function FormControlLayer(props: FormControlLayerProps) {
  const { host, sheet, view, scale, revision, onList } = props;
  const [placed, setPlaced] = useState<PlacedControl[]>([]);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const list = (await host.exec<FormControlInfo[]>({ cmd: 'formControls', sheet })) ?? [];
      const p = await placeControls(host, view, scale, list);
      if (cancelled) return;
      setPlaced(p);
      onList?.(list);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [host, sheet, view, scale, revision, onList]);
  return (
    <>
      {placed.map((p) => (
        <ControlView key={`${p.info.index}:${p.info.kind}`} {...props} placed={p} />
      ))}
    </>
  );
}

function ControlView({
  placed: { info, box },
  host,
  sheet,
  view,
  scale,
  exec,
  selected,
  onSelect,
  onFormat,
  onNotice,
}: FormControlLayerProps & { placed: PlacedControl }) {
  const { t } = useTranslation();
  const [drag, setDrag] = useState<{ x: number; y: number; dx: number; dy: number } | null>(null);
  const frameRef = useRef<HTMLDivElement>(null);
  const isSelected = selected === info.index;
  const label = info.text || info.name;

  useEffect(() => {
    if (isSelected) frameRef.current?.focus({ preventScroll: true });
  }, [isSelected]);

  const click = async () => {
    const r = await exec<{ changed: boolean; macro: string | null; button?: boolean }>({
      cmd: 'formControlClick',
      sheet,
      index: info.index,
    });
    if (r?.button)
      onNotice(
        r.macro
          ? t('sheetEditor.formControl.macroKept', { name: label, macro: r.macro })
          : t('sheetEditor.formControl.noMacro', { name: label }),
      );
  };
  const select = (sel: number) =>
    void exec({ cmd: 'formControlSelect', sheet, index: info.index, sel });
  const step = (n: number, page = false) =>
    void exec({ cmd: 'formControlValue', sheet, index: info.index, step: n, page });

  const onContextMenu = (e: ReactMouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onSelect(info.index);
  };

  // Design mode: moving with the mouse, keys on the frame.
  const startDrag = (e: ReactMouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    setDrag({ x: e.clientX, y: e.clientY, dx: 0, dy: 0 });
  };
  useEffect(() => {
    if (!drag) return;
    const move = (e: MouseEvent) =>
      setDrag((d) => d && { ...d, dx: e.clientX - d.x, dy: e.clientY - d.y });
    const up = (e: MouseEvent) => {
      const dx = e.clientX - drag.x;
      const dy = e.clientY - drag.y;
      setDrag(null);
      if (Math.abs(dx) < 3 && Math.abs(dy) < 3) return;
      void (async () => {
        // Top-left corner in sheet points → the cell under it and the offset into it.
        const x = (box.x + dx) / scale;
        const y = (box.y + dy) / scale;
        const hit = await host.hitTest(view, Math.max(x, 0), Math.max(y, 0));
        if (hit.kind !== 'cell') return;
        const cell = await host.rangeRect(view, hit.row, hit.col, hit.row, hit.col);
        const off: [number, number] = cell
          ? [Math.max(x - cell.x, 0), Math.max(y - cell.y, 0)]
          : [0, 0];
        await exec({
          cmd: 'moveFormControl',
          sheet,
          index: info.index,
          row: hit.row,
          col: hit.col,
          offset: off,
        });
      })();
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up, { once: true });
    return () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
  }, [drag, box, scale, host, view, exec, sheet, info.index]);

  const onFrameKey = (e: ReactKeyboardEvent) => {
    if (e.key === 'Delete' && info.kind !== 'activeX') {
      e.preventDefault();
      onSelect(null);
      void exec({ cmd: 'deleteFormControl', sheet, index: info.index });
    } else if (e.key === 'Enter' && info.kind !== 'activeX') {
      e.preventDefault();
      onFormat(info);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onSelect(null);
    }
  };

  const style = {
    left: box.x + (drag?.dx ?? 0),
    top: box.y + (drag?.dy ?? 0),
    width: box.w,
    height: box.h,
    fontSize: Math.max(Math.round(11 * scale * 0.75), 8),
  };
  const common = {
    'data-testid': `form-control-${info.kind}`,
    'data-index': info.index,
    onContextMenu,
  };

  let body: ReactNode;
  switch (info.kind) {
    case 'button':
      body = (
        <button
          type="button"
          className={styles.button}
          onClick={() => void click()}
          title={info.macro ?? undefined}
        >
          {label}
        </button>
      );
      break;
    case 'checkBox':
    case 'optionButton': {
      const radio = info.kind === 'optionButton';
      const state: CheckState = info.checked;
      body = (
        <div
          role={radio ? 'radio' : 'checkbox'}
          aria-checked={state === 'mixed' ? 'mixed' : state === 'checked'}
          aria-label={label}
          tabIndex={0}
          className={styles.check}
          onClick={() => void click()}
          onKeyDown={(e) => {
            if (e.key === ' ' || e.key === 'Enter') {
              e.preventDefault();
              void click();
            }
          }}
        >
          <span
            className={`${radio ? styles.radioMark : styles.checkMark} ${
              state === 'checked' ? styles.on : state === 'mixed' ? styles.mixed : ''
            } ${info.noThreeD ? styles.flat : ''}`}
          />
          <span className={styles.caption}>{info.text}</span>
        </div>
      );
      break;
    }
    case 'comboBox':
      body = (
        <select
          className={styles.combo}
          aria-label={info.name}
          value={info.sel}
          onChange={(e) => select(Number(e.target.value))}
        >
          {info.sel === 0 && <option value={0} />}
          {(info.items ?? []).map((item, i) => (
            <option key={i} value={i + 1}>
              {item}
            </option>
          ))}
        </select>
      );
      break;
    case 'listBox': {
      const items = info.items ?? [];
      body = (
        <div
          role="listbox"
          aria-label={info.name}
          tabIndex={0}
          className={styles.list}
          onKeyDown={(e) => {
            const d = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
            if (d === 0) return;
            e.preventDefault();
            select(moveSel(info.sel, d, items.length));
          }}
        >
          {items.map((item, i) => (
            <div
              key={i}
              role="option"
              aria-selected={info.sel === i + 1}
              className={info.sel === i + 1 ? styles.itemOn : styles.item}
              onClick={() => select(i + 1)}
            >
              {item || ' '}
            </div>
          ))}
        </div>
      );
      break;
    }
    case 'scrollBar':
    case 'spinner': {
      const across = isAcross(info, box);
      const span = info.max - info.min;
      const pos = span === 0 ? 0 : (info.val - Math.min(info.min, info.max)) / Math.abs(span);
      const less = t('sheetEditor.formControl.decrease');
      const more = t('sheetEditor.formControl.increase');
      const arrows = (first: string, second: string) => [
        <button
          key="a"
          type="button"
          className={styles.arrow}
          aria-label={less}
          onClick={() => step(-1)}
        >
          {first}
        </button>,
        <button
          key="b"
          type="button"
          className={styles.arrow}
          aria-label={more}
          onClick={() => step(1)}
        >
          {second}
        </button>,
      ];
      // Up-down: the upper / right arrow increases.
      if (info.kind === 'spinner') {
        const [dec, inc] = arrows(across ? '◂' : '▾', across ? '▸' : '▴');
        body = (
          <div
            role="spinbutton"
            aria-valuenow={info.val}
            aria-valuemin={info.min}
            aria-valuemax={info.max}
            aria-label={info.name}
            className={across ? styles.spinAcross : styles.spin}
          >
            {across ? [dec, inc] : [inc, dec]}
          </div>
        );
      } else {
        const [dec, inc] = arrows(across ? '◂' : '▴', across ? '▸' : '▾');
        body = (
          <div
            role="scrollbar"
            aria-valuenow={info.val}
            aria-valuemin={info.min}
            aria-valuemax={info.max}
            aria-orientation={across ? 'horizontal' : 'vertical'}
            aria-label={info.name}
            className={across ? styles.scrollAcross : styles.scroll}
          >
            {dec}
            <div
              className={styles.track}
              data-testid="form-control-track"
              onClick={(e) => {
                const r = e.currentTarget.getBoundingClientRect();
                const at = across
                  ? (e.clientX - r.left) / Math.max(r.width, 1)
                  : (e.clientY - r.top) / Math.max(r.height, 1);
                step(at < pos ? -1 : 1, true);
              }}
            >
              <div
                className={styles.thumb}
                style={across ? { left: `${pos * 80}%` } : { top: `${pos * 80}%` }}
              />
            </div>
            {inc}
          </div>
        );
      }
      break;
    }
    case 'groupBox':
      body = (
        <fieldset className={`${styles.group} ${info.noThreeD ? styles.flat : ''}`}>
          <legend className={styles.legend}>{info.text}</legend>
        </fieldset>
      );
      break;
    case 'label':
      body = <span className={styles.label}>{info.text}</span>;
      break;
    case 'activeX':
      body = info.image ? (
        <img
          className={styles.picture}
          src={info.image}
          alt={info.name}
          onClick={() => onNotice(t('sheetEditor.formControl.activeXKept', { name: info.name }))}
        />
      ) : (
        <div
          className={styles.placeholder}
          onClick={() => onNotice(t('sheetEditor.formControl.activeXKept', { name: info.name }))}
        >
          {info.name}
        </div>
      );
      break;
  }

  return (
    <div
      {...common}
      className={`${styles.control} ${info.kind === 'groupBox' ? styles.passThrough : ''}`}
      style={style}
    >
      {body}
      {isSelected && (
        <div
          ref={frameRef}
          className={styles.selected}
          data-testid="form-control-selected"
          role="button"
          tabIndex={0}
          aria-label={t('sheetEditor.formControl.selected', { name: info.name })}
          title={t('sheetEditor.formControl.designHint')}
          onMouseDown={startDrag}
          onDoubleClick={() => info.kind !== 'activeX' && onFormat(info)}
          onKeyDown={onFrameKey}
          onContextMenu={onContextMenu}
        />
      )}
    </div>
  );
}

// ------------------------------------------------------------------ settings dialog

interface Draft {
  name: string;
  text: string;
  link: string;
  inputRange: string;
  checked: CheckState;
  val: string;
  min: string;
  max: string;
  inc: string;
  page: string;
  dropLines: string;
  noThreeD: boolean;
  horiz: boolean;
  macroName: string;
}

const draftOf = (i: FormControlInfo): Draft => ({
  name: i.name,
  text: i.text,
  link: i.link ?? '',
  inputRange: i.inputRange ?? '',
  checked: i.checked,
  val: String(i.val),
  min: String(i.min),
  max: String(i.max),
  inc: String(i.inc),
  page: String(i.page),
  dropLines: String(i.dropLines),
  noThreeD: i.noThreeD,
  horiz: i.horiz,
  macroName: i.macro ?? '',
});

const INT = /^-?\d+$/;

/** The dialog's settings as a `formatFormControl` spec, or an i18n error key. */
export function specOf(kind: FormControlKind, d: Draft): FormatControlSpec | string {
  const spec: FormatControlSpec = { name: d.name };
  if (CAPTIONED.includes(kind)) spec.text = d.text;
  if (LINKED.includes(kind)) spec.link = d.link.trim();
  if (kind === 'button') spec.macroName = d.macroName.trim();
  if (kind === 'checkBox' || kind === 'optionButton') spec.checked = d.checked;
  if (kind === 'comboBox' || kind === 'listBox') {
    spec.inputRange = d.inputRange.trim();
    if (kind === 'comboBox') {
      if (!INT.test(d.dropLines.trim()) || Number(d.dropLines) < 1)
        return 'sheetEditor.formControl.badNumber';
      spec.dropLines = Number(d.dropLines);
    }
  }
  if (kind === 'scrollBar' || kind === 'spinner') {
    const fields =
      kind === 'scrollBar'
        ? (['val', 'min', 'max', 'inc', 'page'] as const)
        : (['val', 'min', 'max', 'inc'] as const);
    for (const f of fields) {
      if (!INT.test(d[f].trim())) return 'sheetEditor.formControl.badNumber';
      spec[f] = Number(d[f]);
    }
    if ((spec.min ?? 0) > (spec.max ?? 0)) return 'sheetEditor.formControl.badLimits';
    if ((spec.inc ?? 1) < 1 || (spec.page ?? 1) < 1) return 'sheetEditor.formControl.badStep';
    spec.horiz = d.horiz;
  }
  if (kind !== 'button' && kind !== 'label') spec.noThreeD = d.noThreeD;
  return spec;
}

/** Settings of one form control: caption, linked cell, source range, limits, state. */
export function FormatControlDialog({
  info,
  onClose,
  onApply,
}: {
  info: FormControlInfo;
  onClose: () => void;
  onApply: (spec: FormatControlSpec) => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const [d, setD] = useState<Draft>(() => draftOf(info));
  const [error, setError] = useState('');
  const k = info.kind;
  const set = (patch: Partial<Draft>) => setD((x) => ({ ...x, ...patch }));
  const field = (key: keyof Draft, labelKey: string, testId: string, placeholder?: string) => (
    <label>
      {t(labelKey)}
      <input
        data-testid={testId}
        value={String(d[key])}
        placeholder={placeholder}
        onChange={(e) => set({ [key]: e.target.value } as Partial<Draft>)}
      />
    </label>
  );
  const ok = async () => {
    const spec = specOf(k, d);
    if (typeof spec === 'string') {
      setError(t(spec));
      return;
    }
    await onApply(spec);
  };
  const flagRow = (key: 'noThreeD' | 'horiz', labelKey: string, testId: string) => (
    <label className={editorStyles.check}>
      <input
        type="checkbox"
        data-testid={testId}
        checked={d[key]}
        onChange={(e) => set({ [key]: e.target.checked } as Partial<Draft>)}
      />
      {t(labelKey)}
    </label>
  );
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.formControl.formatTitle', {
        kind: t(`sheetEditor.formControl.kinds.${k}`),
      })}
      testId="format-control-dialog"
      width={420}
      footer={
        <>
          <button type="button" className={editorStyles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={editorStyles.btnPrimary}
            data-testid="format-control-ok"
            onClick={() => void ok()}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={editorStyles.form}>
        {field('name', 'sheetEditor.formControl.name', 'fc-name')}
        {CAPTIONED.includes(k) && field('text', 'sheetEditor.formControl.caption', 'fc-text')}
        {LINKED.includes(k) && field('link', 'sheetEditor.formControl.link', 'fc-link', '$B$2')}
        {(k === 'comboBox' || k === 'listBox') &&
          field('inputRange', 'sheetEditor.formControl.inputRange', 'fc-range', '$A$1:$A$10')}
        {k === 'comboBox' && field('dropLines', 'sheetEditor.formControl.dropLines', 'fc-drop')}
        {(k === 'scrollBar' || k === 'spinner') && (
          <>
            {field('val', 'sheetEditor.formControl.value', 'fc-val')}
            {field('min', 'sheetEditor.formControl.min', 'fc-min')}
            {field('max', 'sheetEditor.formControl.max', 'fc-max')}
            {field('inc', 'sheetEditor.formControl.inc', 'fc-inc')}
            {k === 'scrollBar' && field('page', 'sheetEditor.formControl.page', 'fc-page')}
            {flagRow('horiz', 'sheetEditor.formControl.horiz', 'fc-horiz')}
          </>
        )}
        {(k === 'checkBox' || k === 'optionButton') && (
          <fieldset className={styles.states}>
            <legend>{t('sheetEditor.formControl.state')}</legend>
            {(k === 'checkBox'
              ? (['unchecked', 'checked', 'mixed'] as const)
              : (['unchecked', 'checked'] as const)
            ).map((s) => (
              <label key={s} className={editorStyles.check}>
                <input
                  type="radio"
                  name="fc-state"
                  data-testid={`fc-state-${s}`}
                  checked={d.checked === s}
                  onChange={() => set({ checked: s })}
                />
                {t(`sheetEditor.formControl.states.${s}`)}
              </label>
            ))}
          </fieldset>
        )}
        {k === 'button' && (
          <>
            {field('macroName', 'sheetEditor.formControl.macro', 'fc-macro')}
            <p className={editorStyles.hint}>{t('sheetEditor.formControl.macroHint')}</p>
          </>
        )}
        {k !== 'button' &&
          k !== 'label' &&
          flagRow('noThreeD', 'sheetEditor.formControl.flat', 'fc-flat')}
        {error && (
          <p className={editorStyles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
