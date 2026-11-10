/**
 * Zooms (SLIDE-ZOOM-001): "Thu phóng tổng quan / phần / trang chiếu". Insert › Zoom commands
 * with their picker dialog, and the contextual Zoom tab of a selected zoom (return to zoom,
 * zoom transition and its duration, zoom background, custom image / slide preview, refresh).
 */
import type { CommandRegistry } from '@blueoffice/editor';
import {
  ZOOM_TRANSITION_MS,
  type SlideCommand,
  type SlideSection,
  type SlideZoomInfo,
  type SlideZoomNames,
} from '@blueoffice/platform';
import { Dialog, type RibbonTab } from '@blueoffice/ui';
import {
  ImagePlus,
  Image as ImageIcon,
  LayoutGrid,
  Layers,
  RefreshCw,
  Undo2,
  Wallpaper,
  Zap,
  ZoomIn,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import styles from './SlideEditor.module.css';

type T = (key: string, vars?: Record<string, string | number>) => string;

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonTab['groups'][number]['items'][number];

export type ZoomInsertKind = 'summary' | 'section' | 'slide';

/** State the zoom commands look at. */
export interface ZoomState {
  /** One zoom is selected. */
  isZoom?: boolean;
  zoom?: SlideZoomInfo;
}

/** The selected zoom's options (all objects alike; the first one decides). */
export function zoomOptions(z: SlideZoomInfo | undefined) {
  const o = z?.objects[0];
  return {
    returnToParent: o?.returnToParent ?? true,
    transition: o ? o.transitionMs !== null : false,
    durationMs: o?.transitionMs ?? ZOOM_TRANSITION_MS,
    showBg: o?.showBg ?? true,
    cover: o?.cover ?? false,
  };
}

/** The command of the insert dialog's choice. */
export function insertCommand(
  kind: ZoomInsertKind,
  picked: number[],
  names: SlideZoomNames,
): SlideCommand | null {
  const list = [...new Set(picked)].filter((i) => Number.isInteger(i) && i >= 0);
  list.sort((a, b) => a - b);
  if (list.length === 0) return null;
  switch (kind) {
    case 'summary':
      return { cmd: 'zoomInsertSummary', slides: list, names };
    case 'section':
      return { cmd: 'zoomInsertSections', sections: list };
    case 'slide':
      return { cmd: 'zoomInsertSlides', slides: list };
  }
}

/** Register the zoom commands; returns the unregister function. */
export function registerZoomCommands<S extends ZoomState & { hasDeck: boolean }>(
  reg: CommandRegistry<S>,
  run: (cmd: SlideCommand) => void,
  opts: { openInsert: (kind: ZoomInsertKind) => void; pickImage: () => void },
): () => void {
  const off: (() => void)[] = [];
  const deck = (s: S) => s.hasDeck;
  const on = (s: S) => !!s.isZoom;
  for (const k of ['summary', 'section', 'slide'] as const) {
    off.push(
      reg.register({ id: `insert.zoom.${k}`, isEnabled: deck, run: () => opts.openInsert(k) }),
    );
  }
  const add = (id: string, exec: (s: S) => void, isChecked?: (s: S) => boolean) =>
    off.push(reg.register({ id, isEnabled: on, run: exec, isChecked }));
  add(
    'zoom.return',
    (s) => run({ cmd: 'zoomOptions', returnToParent: !zoomOptions(s.zoom).returnToParent }),
    (s) => zoomOptions(s.zoom).returnToParent,
  );
  add(
    'zoom.transition',
    (s) => run({ cmd: 'zoomOptions', transition: !zoomOptions(s.zoom).transition }),
    (s) => zoomOptions(s.zoom).transition,
  );
  add(
    'zoom.background',
    (s) => run({ cmd: 'zoomOptions', showBg: !zoomOptions(s.zoom).showBg }),
    (s) => zoomOptions(s.zoom).showBg,
  );
  add('zoom.image', () => opts.pickImage());
  add('zoom.resetImage', () => run({ cmd: 'zoomResetImage' }));
  add('zoom.refresh', () => run({ cmd: 'zoomRefresh' }));
  return () => {
    for (const f of off) f();
  };
}

/** Insert › Zoom buttons. */
export function zoomInsertItems(t: T, item: Item) {
  const sm = 'small' as const;
  return [
    item('insert.zoom.summary', t('slideEditor.zoom.summary'), <LayoutGrid size={16} />, sm),
    item('insert.zoom.section', t('slideEditor.zoom.section'), <Layers size={16} />, sm),
    item('insert.zoom.slide', t('slideEditor.zoom.slide'), <ZoomIn size={16} />, sm),
  ];
}

/** Duration of the zoom transition, in seconds. */
function DurationField({
  label,
  ms,
  enabled,
  onChange,
}: {
  label: string;
  ms: number;
  enabled: boolean;
  onChange: (ms: number) => void;
}) {
  return (
    <label className={styles.zoomDuration}>
      {label}
      <input
        type="number"
        min={0.1}
        max={60}
        step={0.25}
        disabled={!enabled}
        data-testid="zoom-duration"
        // Remounted when the value changes elsewhere (undo, another zoom).
        key={ms}
        defaultValue={(ms / 1000).toString()}
        onBlur={(e) => {
          const v = Number(e.currentTarget.value);
          if (Number.isFinite(v) && v > 0 && Math.round(v * 1000) !== ms) {
            onChange(Math.round(Math.min(60, v) * 1000));
          }
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
        }}
      />
    </label>
  );
}

/** The contextual tab of a selected zoom. */
export function zoomTab(
  t: T,
  item: Item,
  zoom: SlideZoomInfo | undefined,
  run: (cmd: SlideCommand) => void,
): RibbonTab {
  const o = zoomOptions(zoom);
  const sm = 'small' as const;
  return {
    id: 'zoom',
    context: 'zoom',
    label: t('slideEditor.zoom.tab'),
    groups: [
      {
        id: 'zoomOptions',
        label: t('slideEditor.zoom.groupOptions'),
        items: [
          item(
            'zoom.return',
            t('slideEditor.zoom.return'),
            <Undo2 size={16} />,
            sm,
            o.returnToParent,
          ),
          item(
            'zoom.transition',
            t('slideEditor.zoom.transition'),
            <Zap size={16} />,
            sm,
            o.transition,
          ),
          {
            id: 'zoom.duration',
            label: t('slideEditor.zoom.duration'),
            onSelect: () => undefined,
            render: (
              <DurationField
                label={t('slideEditor.zoom.duration')}
                ms={o.durationMs}
                enabled={!!zoom}
                onChange={(ms) => run({ cmd: 'zoomOptions', durationMs: ms })}
              />
            ),
          },
        ],
      },
      {
        id: 'zoomStyle',
        label: t('slideEditor.zoom.groupStyle'),
        items: [
          item(
            'zoom.background',
            t('slideEditor.zoom.background'),
            <Wallpaper size={16} />,
            sm,
            o.showBg,
          ),
          item('zoom.image', t('slideEditor.zoom.image'), <ImagePlus size={16} />, sm),
          item('zoom.resetImage', t('slideEditor.zoom.resetImage'), <ImageIcon size={16} />, sm),
          item('zoom.refresh', t('slideEditor.zoom.refresh'), <RefreshCw size={16} />, sm),
        ],
      },
    ],
  };
}

/** Pick the slides or sections of a new zoom. */
export function ZoomInsertDialog({
  kind,
  count,
  sections,
  thumb,
  t,
  onClose,
  onInsert,
}: {
  kind: ZoomInsertKind;
  count: number;
  sections: SlideSection[];
  /** Thumbnail of slide `i`. */
  thumb: (i: number) => ReactNode;
  t: T;
  onClose: () => void;
  onInsert: (picked: number[]) => void;
}) {
  const [picked, setPicked] = useState<number[]>([]);
  const toggle = (i: number) =>
    setPicked((p) => (p.includes(i) ? p.filter((x) => x !== i) : [...p, i]));
  const bySection = kind === 'section';
  const items = bySection
    ? sections
        .map((s, i) => ({ i, label: s.name, first: s.first, empty: s.count === 0 }))
        .filter((s) => !s.empty)
    : Array.from({ length: count }, (_, i) => ({
        i,
        label: t('slideEditor.zoom.slideN', { n: i + 1 }),
        first: i,
        empty: false,
      }));
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(`slideEditor.zoom.title.${kind}`)}
      testId="zoom-dialog"
      width={560}
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="zoom-insert"
            disabled={picked.length === 0}
            onClick={() => {
              onInsert(picked);
              onClose();
            }}
          >
            {t('slideEditor.zoom.insert')}
          </button>
        </>
      }
    >
      <p className={styles.zoomHint}>{t(`slideEditor.zoom.hint.${kind}`)}</p>
      {items.length === 0 && <p>{t('slideEditor.zoom.noSections')}</p>}
      <div className={styles.zoomPick} role="group">
        {items.map((it) => (
          <label key={it.i} className={styles.zoomPickItem}>
            <input
              type="checkbox"
              data-testid={`zoom-pick-${it.i}`}
              checked={picked.includes(it.i)}
              onChange={() => toggle(it.i)}
            />
            <span className={styles.zoomPickThumb}>{thumb(it.first)}</span>
            <span>{it.label}</span>
          </label>
        ))}
      </div>
    </Dialog>
  );
}
