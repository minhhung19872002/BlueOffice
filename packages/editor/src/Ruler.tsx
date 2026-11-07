import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  FormatState,
  PageSetupSpec,
  TabKind,
  TabSpec,
} from '@blueoffice/platform';
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import styles from './Ruler.module.css';

/** Ruler thickness in CSS pixels. */
export const RULER_SIZE = 22;

/** Page box in workspace coordinates (CSS pixels). */
export interface PageBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

type Run = (cmd: EditCommand) => Promise<unknown>;

/** Units: centimetres (vi-VN) or inches (en-US). */
function unitsOf(language: string) {
  return language === 'en-US'
    ? { unit: 1440, minor: 180, snap: 90, label: 'in' }
    : { unit: 567, minor: 141.75, snap: 70.875, label: 'cm' };
}

const TAB_KINDS: TabKind[] = ['left', 'center', 'right', 'decimal'];
const TAB_GLYPH: Record<TabKind, string> = {
  left: '⌊',
  center: '⊥',
  right: '⌋',
  decimal: '⊥.',
  bar: '|',
};

/** Page setup of the caret's section as a `setPageSetup` spec. */
export function setupOf(f: FormatState | null | undefined): PageSetupSpec | null {
  const s = f?.section;
  if (!s || !s.width) return null;
  return {
    width: s.width,
    height: s.height,
    marginTop: s.marginTop,
    marginBottom: s.marginBottom,
    marginLeft: s.marginLeft,
    marginRight: s.marginRight,
    header: s.header,
    footer: s.footer,
    columns: s.columns,
    columnSpace: s.columnSpace,
  };
}

type DragKind =
  | 'marginLeft'
  | 'marginRight'
  | 'marginTop'
  | 'marginBottom'
  | 'first'
  | 'hanging'
  | 'left'
  | 'right'
  | 'tab';

interface Drag {
  kind: DragKind;
  /** Current position in twips along the ruler (from the page edge). */
  pos: number;
  /** Pointer moved away from the ruler (tab removal). */
  away: boolean;
  tab?: number;
}

function ticks(length: number, unit: number, minor: number, origin: number) {
  const out: { at: number; major: boolean; label?: number }[] = [];
  const n0 = Math.ceil(-origin / minor);
  const n1 = Math.floor((length - origin) / minor);
  const per = Math.round(unit / minor);
  for (let n = n0; n <= n1; n++) {
    const major = n % per === 0;
    out.push({
      at: origin + n * minor,
      major,
      label: major && n !== 0 ? Math.abs(n / per) : undefined,
    });
  }
  return out;
}

/**
 * Horizontal and vertical rulers of the caret's page: margins (ECMA-376 Part 1,
 * §17.6.11 pgMar), paragraph indents (§17.3.1.12 ind) and tab stops (§17.3.1.37 tabs)
 * are dragged; clicking the horizontal ruler adds a tab stop of the selected kind,
 * dragging one off the ruler removes it. Every change is an engine command.
 */
export function Rulers({
  box,
  format,
  run,
  language,
  disabled,
}: {
  box: PageBox | null;
  format: FormatState | null | undefined;
  run: Run;
  language: string;
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const [tabKind, setTabKind] = useState<TabKind>('left');
  const [drag, setDrag] = useState<Drag | null>(null);
  const dragRef = useRef<Drag | null>(null);
  const track = useRef<HTMLDivElement>(null);
  const vtrack = useRef<HTMLDivElement>(null);
  const setup = setupOf(format);
  const u = unitsOf(language);

  useEffect(() => {
    dragRef.current = drag;
  }, [drag]);

  if (!box || !setup) {
    return (
      <>
        <div className={styles.corner} />
        <div className={styles.hruler} data-testid="ruler" />
        <div className={styles.vruler} data-testid="vruler" />
      </>
    );
  }
  const k = box.w / setup.width; // px per twip
  const kv = box.h / setup.height;
  const ml = setup.marginLeft;
  const textRight = setup.width - setup.marginRight;
  const left = format?.indentLeft ?? 0;
  const right = format?.indentRight ?? 0;
  const first = format?.firstLine ?? 0;
  const tabs: TabSpec[] = format?.tabs ?? [];
  const snap = (tw: number, alt: boolean) =>
    alt ? Math.round(tw) : Math.round(Math.round(tw / u.snap) * u.snap);

  const posOf = (e: { clientX: number; clientY: number }, vertical: boolean, alt: boolean) => {
    if (vertical) {
      const r = vtrack.current?.getBoundingClientRect();
      return snap(r ? (e.clientY - r.top) / kv : 0, alt);
    }
    const r = track.current?.getBoundingClientRect();
    return snap(r ? (e.clientX - r.left) / k : 0, alt);
  };

  const commit = (d: Drag) => {
    const clampH = (v: number) => Math.max(0, Math.min(setup.width, v));
    const clampV = (v: number) => Math.max(0, Math.min(setup.height, v));
    const indents = (l: number, r: number, f: number) =>
      run({ cmd: 'formatParagraph', format: { kind: 'indents', left: l, right: r, firstLine: f } });
    switch (d.kind) {
      case 'marginLeft':
        return run({ cmd: 'setPageSetup', setup: { ...setup, marginLeft: clampH(d.pos) } });
      case 'marginRight':
        return run({
          cmd: 'setPageSetup',
          setup: { ...setup, marginRight: clampH(setup.width - d.pos) },
        });
      case 'marginTop':
        return run({ cmd: 'setPageSetup', setup: { ...setup, marginTop: clampV(d.pos) } });
      case 'marginBottom':
        return run({
          cmd: 'setPageSetup',
          setup: { ...setup, marginBottom: clampV(setup.height - d.pos) },
        });
      case 'first':
        return indents(left, right, d.pos - ml - left);
      case 'hanging': {
        const l = Math.max(0, d.pos - ml);
        return indents(l, right, left + first - l);
      }
      case 'left':
        return indents(Math.max(0, d.pos - ml), right, first);
      case 'right':
        return indents(left, textRight - d.pos, first);
      case 'tab': {
        const rest = tabs.filter((_, i) => i !== d.tab);
        const next = d.away
          ? rest
          : [...rest, { ...tabs[d.tab ?? 0]!, pos: Math.max(0, d.pos - ml) }];
        return run({
          cmd: 'formatParagraph',
          format: { kind: 'tabs', tabs: next.sort((a, b) => a.pos - b.pos) },
        });
      }
    }
  };

  const start = (e: ReactPointerEvent, kind: DragKind, tab?: number) => {
    if (disabled || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const vertical = kind === 'marginTop' || kind === 'marginBottom';
    const origin = { x: e.clientX, y: e.clientY };
    const d: Drag = { kind, pos: posOf(e, vertical, e.altKey), away: false, tab };
    setDrag(d);
    dragRef.current = d;
    const move = (ev: PointerEvent) => {
      const away = kind === 'tab' && Math.abs(ev.clientY - origin.y) > RULER_SIZE;
      const n: Drag = { kind, pos: posOf(ev, vertical, ev.altKey), away, tab };
      dragRef.current = n;
      setDrag(n);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      const last = dragRef.current;
      setDrag(null);
      dragRef.current = null;
      if (last) void commit(last);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  // Positions (twips from the page's left edge), following a drag in progress.
  const live = (kind: DragKind, value: number) => (drag?.kind === kind ? drag.pos : value);
  const mlLive = live('marginLeft', ml);
  const mrLive = live('marginRight', textRight);
  const leftPos = live('left', live('hanging', ml + left));
  const firstPos =
    drag?.kind === 'first'
      ? drag.pos
      : drag?.kind === 'left'
        ? drag.pos + first
        : ml + left + first;
  const rightPos = live('right', textRight - right);
  const px = (tw: number) => tw * k;

  const addTab = (e: ReactPointerEvent) => {
    if (disabled || e.button !== 0) return;
    const pos = posOf(e, false, e.altKey) - ml;
    if (pos <= 0 || pos >= textRight - ml) return;
    const next = [...tabs.filter((x) => x.pos !== pos), { pos, align: tabKind }];
    void run({
      cmd: 'formatParagraph',
      format: { kind: 'tabs', tabs: next.sort((a, b) => a.pos - b.pos) },
    });
  };

  const hTicks = ticks(setup.width, u.unit, u.minor, ml);
  const vTicks = ticks(setup.height, u.unit, u.minor, setup.marginTop);
  const mt = live('marginTop', setup.marginTop);
  const mb = live('marginBottom', setup.height - setup.marginBottom);
  const twLabel = (tw: number) =>
    `${(tw / u.unit).toLocaleString(language, { maximumFractionDigits: 2 })} ${u.label}`;

  return (
    <>
      <button
        type="button"
        className={styles.corner}
        data-testid="ruler-tab-type"
        data-kind={tabKind}
        title={t('ruler.tabType', { kind: t(`ruler.tab.${tabKind}`) })}
        aria-label={t('ruler.tabType', { kind: t(`ruler.tab.${tabKind}`) })}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setTabKind(TAB_KINDS[(TAB_KINDS.indexOf(tabKind) + 1) % TAB_KINDS.length]!)}
      >
        {TAB_GLYPH[tabKind]}
      </button>
      <div className={styles.hruler} data-testid="ruler" aria-label={t('ruler.horizontal')}>
        <div
          ref={track}
          className={styles.track}
          style={{ left: box.x - RULER_SIZE, width: box.w }}
          onPointerDown={addTab}
        >
          <div className={styles.margin} style={{ left: 0, width: px(mlLive) }} />
          <div
            className={styles.margin}
            style={{ left: px(mrLive), width: Math.max(0, box.w - px(mrLive)) }}
          />
          {hTicks.map((tk) => (
            <span
              key={tk.at}
              className={tk.major ? styles.major : styles.minor}
              style={{ left: px(tk.at) }}
            >
              {tk.label !== undefined && <span className={styles.num}>{tk.label}</span>}
            </span>
          ))}
          <div
            className={styles.marginHandle}
            data-testid="ruler-margin-left"
            data-twips={ml}
            title={t('ruler.marginLeft', { value: twLabel(mlLive) })}
            style={{ left: px(mlLive) - 3 }}
            onPointerDown={(e) => start(e, 'marginLeft')}
          />
          <div
            className={styles.marginHandle}
            data-testid="ruler-margin-right"
            data-twips={setup.marginRight}
            title={t('ruler.marginRight', { value: twLabel(setup.width - mrLive) })}
            style={{ left: px(mrLive) - 3 }}
            onPointerDown={(e) => start(e, 'marginRight')}
          />
          <div
            className={`${styles.marker} ${styles.first}`}
            data-testid="ruler-first-line"
            data-twips={first}
            title={t('ruler.firstLine')}
            style={{ left: px(firstPos) - 5 }}
            onPointerDown={(e) => start(e, 'first')}
          />
          <div
            className={`${styles.marker} ${styles.hanging}`}
            data-testid="ruler-hanging"
            title={t('ruler.hanging')}
            style={{ left: px(drag?.kind === 'left' ? drag.pos : leftPos) - 5 }}
            onPointerDown={(e) => start(e, 'hanging')}
          />
          <div
            className={`${styles.marker} ${styles.leftBox}`}
            data-testid="ruler-left-indent"
            data-twips={left}
            title={t('ruler.leftIndent')}
            style={{ left: px(drag?.kind === 'hanging' ? drag.pos : leftPos) - 5 }}
            onPointerDown={(e) => start(e, 'left')}
          />
          <div
            className={`${styles.marker} ${styles.hanging}`}
            data-testid="ruler-right-indent"
            data-twips={right}
            title={t('ruler.rightIndent')}
            style={{ left: px(rightPos) - 5 }}
            onPointerDown={(e) => start(e, 'right')}
          />
          {tabs.map((tab, i) => {
            const at = drag?.kind === 'tab' && drag.tab === i ? drag.pos : ml + tab.pos;
            const gone = drag?.kind === 'tab' && drag.tab === i && drag.away;
            return (
              <div
                key={`${tab.pos}-${i}`}
                className={styles.tabStop}
                data-testid="ruler-tab"
                data-twips={tab.pos}
                data-kind={tab.align ?? 'left'}
                title={t(`ruler.tab.${tab.align ?? 'left'}`)}
                style={{ left: px(at) - 4, opacity: gone ? 0.3 : 1 }}
                onPointerDown={(e) => start(e, 'tab', i)}
              >
                {TAB_GLYPH[tab.align ?? 'left']}
              </div>
            );
          })}
          {drag && drag.kind !== 'marginTop' && drag.kind !== 'marginBottom' && (
            <div className={styles.guide} style={{ left: px(drag.pos) }} />
          )}
        </div>
      </div>
      <div className={styles.vruler} data-testid="vruler" aria-label={t('ruler.vertical')}>
        <div
          ref={vtrack}
          className={styles.vtrack}
          style={{ top: box.y - RULER_SIZE, height: box.h }}
        >
          <div className={styles.vmargin} style={{ top: 0, height: mt * kv }} />
          <div
            className={styles.vmargin}
            style={{ top: mb * kv, height: Math.max(0, box.h - mb * kv) }}
          />
          {vTicks.map((tk) => (
            <span
              key={tk.at}
              className={tk.major ? styles.vmajor : styles.vminor}
              style={{ top: tk.at * kv }}
            >
              {tk.label !== undefined && <span className={styles.vnum}>{tk.label}</span>}
            </span>
          ))}
          <div
            className={styles.vmarginHandle}
            data-testid="ruler-margin-top"
            data-twips={setup.marginTop}
            title={t('ruler.marginTop', { value: twLabel(mt) })}
            style={{ top: mt * kv - 3 }}
            onPointerDown={(e) => start(e, 'marginTop')}
          />
          <div
            className={styles.vmarginHandle}
            data-testid="ruler-margin-bottom"
            data-twips={setup.marginBottom}
            title={t('ruler.marginBottom', { value: twLabel(setup.height - mb) })}
            style={{ top: mb * kv - 3 }}
            onPointerDown={(e) => start(e, 'marginBottom')}
          />
        </div>
      </div>
    </>
  );
}
