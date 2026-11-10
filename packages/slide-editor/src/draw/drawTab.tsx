/**
 * The Draw tab of BlueSlide (CORE-INK-001): pen, highlighter, stroke and point
 * erasers, colour and width presets, the ruler and ink replay. The `draw.*`
 * commands are registered in the editor's CommandRegistry through a hub; the
 * hook owns the tool state, the pointer layer and the replay overlay. Every
 * stroke is an `addInk` command, every eraser drag an `eraseInk` command (one
 * undo step each).
 */
import type { CommandRegistry } from '@blueoffice/editor';
import type { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import {
  HandwritingError,
  type HandwritingProvider,
  type HandwritingResult,
  type PageDisplayList,
  type SlideCommand,
  type SlideEditResult,
  type SlideInkObject,
  type SlideInkStrokeIn,
} from '@blueoffice/platform';
import type { RibbonTab } from '@blueoffice/ui';
import {
  Eraser,
  Highlighter,
  MousePointer2,
  PenLine,
  Play,
  Circle,
  Minus,
  RotateCcw,
  RotateCw,
  Ruler as RulerIcon,
  Scissors,
  Lasso,
  Shapes,
  WandSparkles,
  Type,
  Sigma,
} from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { DrawLayer } from './DrawLayer';
import { InkReplay } from './InkReplay';
import {
  HIGHLIGHTER_COLORS,
  INITIAL_DRAW,
  PEN_COLORS,
  RULER_LENGTH,
  handwritingStrokes,
  lassoPick,
  snapAngle,
  type DrawState,
  type HighlighterColor,
  type PenColor,
  type Ruler,
  type WidthPreset,
} from './inkTools';

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonTab['groups'][number]['items'][number];

type T = (key: string, o?: Record<string, unknown>) => string;

const TOOLS = ['select', 'pen', 'highlighter', 'eraserStroke', 'eraserPoint', 'lasso'] as const;
const WIDTH_PRESETS: WidthPreset[] = ['fine', 'medium', 'bold'];

/** Conversion commands (CORE-INK-002). */
export const CONVERT_COMMANDS = [
  'draw.inkToShape',
  'draw.autoShape',
  'draw.inkToText',
  'draw.inkToMath',
] as const;

/** Every command id of the tab. */
export const DRAW_COMMANDS: string[] = [
  ...TOOLS.map((t) => `draw.${t}`),
  ...Object.keys(PEN_COLORS).map((c) => `draw.color.${c}`),
  ...Object.keys(HIGHLIGHTER_COLORS).map((c) => `draw.highlight.${c}`),
  ...WIDTH_PRESETS.map((w) => `draw.width.${w}`),
  'draw.ruler',
  'draw.rulerLeft',
  'draw.rulerRight',
  'draw.replay',
  ...CONVERT_COMMANDS,
];

/** What handwriting recognition offers now (no provider: nothing). */
export interface HandwritingAvailability {
  text: boolean;
  math: boolean;
}

/** What the commands act on (set by [`useDrawTools`]). */
export interface DrawActions {
  state: DrawState;
  enabled: boolean;
  handwriting: HandwritingAvailability;
  run: (id: string) => void;
}

/** Is a command usable (the tab is, and handwriting commands have a recogniser)? */
export function drawEnabled(a: DrawActions, id: string): boolean {
  if (!a.enabled) return false;
  if (id === 'draw.inkToText') return a.handwriting.text;
  if (id === 'draw.inkToMath') return a.handwriting.math;
  return true;
}

export class DrawActionsHub {
  private actions: DrawActions | null = null;
  set(a: DrawActions | null): void {
    this.actions = a;
  }
  get(): DrawActions | null {
    return this.actions;
  }
}

/** Register the `draw.*` commands. */
export function registerDrawCommands<S>(reg: CommandRegistry<S>, hub: DrawActionsHub): void {
  for (const id of DRAW_COMMANDS) {
    reg.register({
      id,
      isEnabled: () => {
        const a = hub.get();
        return a ? drawEnabled(a, id) : false;
      },
      isChecked: () => {
        const a = hub.get();
        return a ? isChecked(a.state, id) : false;
      },
      run: () => hub.get()?.run(id),
    });
  }
}

/** Pressed state of a command. */
export function isChecked(s: DrawState, id: string): boolean {
  const [, kind, arg] = id.split('.');
  if (TOOLS.includes(kind as (typeof TOOLS)[number])) return s.tool === kind;
  if (kind === 'color') return s.penColor === arg && s.tool !== 'highlighter';
  if (kind === 'highlight') return s.highlighterColor === arg && s.tool === 'highlighter';
  if (kind === 'width') {
    return (s.tool === 'highlighter' ? s.highlighterWidth : s.penWidth) === arg;
  }
  if (kind === 'ruler') return s.ruler !== null;
  if (kind === 'autoShape') return s.autoShape;
  return false;
}

/** State after a command (pure; `slide` centres a new ruler). */
export function applyDrawCommand(
  s: DrawState,
  id: string,
  slide: { width: number; height: number },
): DrawState {
  const [, kind, arg] = id.split('.');
  if (TOOLS.includes(kind as (typeof TOOLS)[number])) {
    return { ...s, tool: kind as DrawState['tool'] };
  }
  switch (kind) {
    case 'color':
      return { ...s, penColor: arg as PenColor, tool: 'pen' };
    case 'highlight':
      return { ...s, highlighterColor: arg as HighlighterColor, tool: 'highlighter' };
    case 'width': {
      const w = arg as WidthPreset;
      return s.tool === 'highlighter' ? { ...s, highlighterWidth: w } : { ...s, penWidth: w };
    }
    case 'ruler':
      return {
        ...s,
        ruler: s.ruler ? null : { x: slide.width / 2, y: slide.height / 2, angle: 0 },
      };
    case 'autoShape':
      // Drawing in the automatic mode needs the pen.
      return {
        ...s,
        autoShape: !s.autoShape,
        tool: !s.autoShape && s.tool !== 'pen' ? 'pen' : s.tool,
      };
    case 'rulerLeft':
    case 'rulerRight': {
      const r: Ruler = s.ruler ?? {
        x: slide.width / 2,
        y: slide.height / 2,
        angle: 0,
      };
      return {
        ...s,
        ruler: { ...r, angle: snapAngle(r.angle + (kind === 'rulerLeft' ? -15 : 15)) },
      };
    }
    default:
      return s;
  }
}

/** The Draw tab; `handwriting` says which recognisers exist (disabled items explain why not). */
export function drawTab(
  t: T,
  item: Item,
  s: DrawState,
  handwriting: HandwritingAvailability = { text: false, math: false },
): RibbonTab {
  const sm = 'small' as const;
  const hl = s.tool === 'highlighter';
  const swatch = (hex: string) => <Circle size={16} fill={`#${hex}`} color={`#${hex}`} />;
  return {
    id: 'draw',
    label: t('drawTools.tab'),
    groups: [
      {
        id: 'drawTools',
        label: t('drawTools.groupTools'),
        items: [
          item('draw.select', t('drawTools.select'), <MousePointer2 size={18} />),
          item('draw.pen', t('drawTools.pen'), <PenLine size={18} />),
          item('draw.highlighter', t('drawTools.highlighter'), <Highlighter size={18} />),
          item('draw.eraserStroke', t('drawTools.eraserStroke'), <Eraser size={16} />, sm),
          item('draw.eraserPoint', t('drawTools.eraserPoint'), <Scissors size={16} />, sm),
          item('draw.lasso', t('drawTools.lasso'), <Lasso size={16} />, sm),
        ],
      },
      {
        id: 'drawConvert',
        label: t('drawTools.groupConvert'),
        items: [
          item('draw.inkToShape', t('drawTools.inkToShape'), <Shapes size={18} />),
          item('draw.autoShape', t('drawTools.autoShape'), <WandSparkles size={16} />, sm),
          {
            ...item('draw.inkToText', t('drawTools.inkToText'), <Type size={16} />, sm),
            ...(handwriting.text ? {} : { tooltip: t('drawTools.handwriting.noText') }),
          },
          {
            ...item('draw.inkToMath', t('drawTools.inkToMath'), <Sigma size={16} />, sm),
            ...(handwriting.math ? {} : { tooltip: t('drawTools.handwriting.noMath') }),
          },
        ],
      },
      {
        id: 'drawColors',
        label: t('drawTools.groupColor'),
        items: hl
          ? Object.entries(HIGHLIGHTER_COLORS).map(([k, v]) =>
              item(`draw.highlight.${k}`, t(`drawTools.colors.${k}`), swatch(v), sm),
            )
          : Object.entries(PEN_COLORS).map(([k, v]) =>
              item(`draw.color.${k}`, t(`drawTools.colors.${k}`), swatch(v), sm),
            ),
      },
      {
        id: 'drawWidths',
        label: t('drawTools.groupWidth'),
        items: WIDTH_PRESETS.map((w, i) =>
          item(
            `draw.width.${w}`,
            t(`drawTools.width.${w}`),
            <Minus size={16} strokeWidth={1 + i * 1.5} />,
            sm,
          ),
        ),
      },
      {
        id: 'drawRuler',
        label: t('drawTools.groupRuler'),
        items: [
          item('draw.ruler', t('drawTools.ruler'), <RulerIcon size={18} />),
          item('draw.rulerLeft', t('drawTools.rulerLeft'), <RotateCcw size={16} />, sm),
          item('draw.rulerRight', t('drawTools.rulerRight'), <RotateCw size={16} />, sm),
        ],
      },
      {
        id: 'drawReplay',
        label: t('drawTools.groupReplay'),
        items: [item('draw.replay', t('drawTools.replay'), <Play size={18} />)],
      },
    ],
  };
}

export interface DrawToolsHost {
  t: T;
  hub: DrawActionsHub;
  /** A deck is open and the normal view shows a slide. */
  enabled: boolean;
  /** Current slide index (ink is re-read when it changes). */
  slide: number;
  /** Slide size in points. */
  size: { width: number; height: number };
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  glyphs: GlyphStore;
  images: ImageStore;
  /** Bumped when the slide changes (re-read the ink). */
  revision: number;
  /** Handwriting recognition (ink → text / math; absent: those commands are disabled). */
  handwriting?: HandwritingProvider;
  /** Language of the handwriting (the editor's UI language). */
  lang?: string;
  /** Status line messages (results, errors). */
  notify?: (message: string) => void;
}

/** Tool state, pointer layer and replay overlay of the Draw tab. */
export function useDrawTools(host: DrawToolsHost): {
  state: DrawState;
  /** Ink of the slide as last read (eraser preview). */
  ink: SlideInkObject[];
  /** Ink objects picked with the lasso. */
  picked: number[];
  /** What handwriting recognition offers. */
  handwriting: HandwritingAvailability;
  layer: ReactNode;
  /** The replay overlay at a displayed width (CSS pixels), while playing. */
  replay: (width: number) => ReactNode;
} {
  const { t, hub, enabled, slide, size, run, glyphs, images, revision } = host;
  const [state, setState] = useState<DrawState>(INITIAL_DRAW);
  const [ink, setInk] = useState<SlideInkObject[]>([]);
  const [pickedOn, setPickedOn] = useState<{ slide: number; ids: number[] }>({
    slide,
    ids: [],
  });
  const [hwOf, setHwOf] = useState<
    ({ provider: HandwritingProvider } & HandwritingAvailability) | null
  >(null);
  const [replay, setReplay] = useState<{
    backdrop: PageDisplayList;
    objects: SlideInkObject[];
  } | null>(null);
  const hostRef = useRef(host);
  useEffect(() => {
    hostRef.current = host;
  });
  /** A line just made in the automatic mode: the next strokes may be its arrowhead. */
  const lastLine = useRef<number | null>(null);
  const active = enabled && state.tool !== 'select';

  // Ink of the slide for the eraser preview and the lasso.
  useEffect(() => {
    if (
      !enabled ||
      (state.tool !== 'eraserStroke' && state.tool !== 'eraserPoint' && state.tool !== 'lasso')
    ) {
      return;
    }
    let live = true;
    void hostRef.current.run({ cmd: 'inkInfo' }).then((r) => {
      if (live && r?.ink) setInk(r.ink.objects);
    });
    return () => {
      live = false;
    };
  }, [enabled, state.tool, slide, revision]);

  // Picked ink belongs to its slide; objects that are gone (erased, converted, undone) drop out.
  const picked =
    pickedOn.slide === slide ? pickedOn.ids.filter((id) => ink.some((o) => o.id === id)) : [];
  const setPicked = useCallback((ids: number[]) => setPickedOn({ slide, ids }), [slide]);
  useEffect(() => {
    lastLine.current = null;
  }, [slide]);

  // What the handwriting recogniser offers (re-checked when the language changes).
  const provider = host.handwriting;
  const lang = host.lang ?? 'vi-VN';
  useEffect(() => {
    if (!provider) return;
    let live = true;
    void provider
      .capabilities()
      .catch(() => ({ text: false, math: false, languages: [] }))
      .then((c) => {
        if (live) setHwOf({ provider, text: c.text, math: c.math });
      });
    return () => {
      live = false;
    };
  }, [provider, lang]);
  const hw: HandwritingAvailability =
    provider && hwOf?.provider === provider
      ? { text: hwOf.text, math: hwOf.math }
      : { text: false, math: false };

  const pickedRef = useRef(picked);
  useEffect(() => {
    pickedRef.current = picked;
  });

  const convert = useCallback(async (id: string) => {
    const h = hostRef.current;
    const ids = pickedRef.current;
    const say = (m: string) => h.notify?.(m);
    if (id === 'draw.inkToShape') {
      const r = await h.run({ cmd: 'inkToShapes', ids });
      const c = r?.ink?.converted;
      if (r?.ink) setInk(r.ink.objects);
      if (!c) return;
      setPickedOn((p) => ({ ...p, ids: [] }));
      if (c.shapes.length === 0) {
        say(c.kept === 0 ? h.t('drawTools.noInk') : h.t('drawTools.noShapes'));
      } else {
        say(h.t('drawTools.converted', { count: c.shapes.length, kept: c.kept }));
      }
      return;
    }
    // Ink → text / math: the strokes go to the recogniser, the answer replaces the ink.
    const mode = id === 'draw.inkToMath' ? 'math' : 'text';
    const p = h.handwriting;
    if (!p) return;
    const info = await h.run({ cmd: 'inkInfo' });
    const objects = info?.ink?.objects ?? [];
    const strokes = handwritingStrokes(objects, ids);
    if (strokes.length === 0) {
      say(h.t('drawTools.noInk'));
      return;
    }
    let res: HandwritingResult;
    try {
      res = await p.recognize({ mode, lang: h.lang ?? 'vi-VN', strokes });
    } catch (e) {
      const code = e instanceof HandwritingError ? e.code : 'upstream';
      say(h.t(`drawTools.handwriting.errors.${code}`));
      return;
    }
    const r = await h.run(
      mode === 'math'
        ? { cmd: 'inkToMath', ids, text: res.text, latex: res.format === 'latex' }
        : { cmd: 'inkToText', ids, text: res.text },
    );
    if (r?.ink) setInk(r.ink.objects);
    if (r?.ink?.converted) {
      setPickedOn((p) => ({ ...p, ids: [] }));
      say(h.t('drawTools.recognized', { text: res.text }));
    }
  }, []);

  const exec = useCallback(
    (id: string) => {
      if (id === 'draw.replay') {
        void hostRef.current.run({ cmd: 'inkInfo', backdrop: true }).then((r) => {
          if (r?.ink?.backdrop) setReplay({ backdrop: r.ink.backdrop, objects: r.ink.objects });
        });
        return;
      }
      if (id === 'draw.inkToShape' || id === 'draw.inkToText' || id === 'draw.inkToMath') {
        void convert(id);
        return;
      }
      lastLine.current = null;
      setState((s) => applyDrawCommand(s, id, hostRef.current.size));
    },
    [convert],
  );

  useEffect(() => {
    hub.set({ state, enabled, handwriting: hw, run: exec });
  });
  // Escape puts the pen down.
  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setState((s) => ({ ...s, tool: 'select' }));
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [active]);
  useEffect(() => () => hub.set(null), [hub]);

  const after = (r: SlideEditResult | null) => {
    if (r?.ink) setInk(r.ink.objects);
  };

  /** A pen stroke: ink, or in the automatic mode the recognised shape (one step either way). */
  const stroke = (s: SlideInkStrokeIn) => {
    const auto = state.autoShape && !s.highlighter;
    const attachTo = auto ? lastLine.current : null;
    void run({
      cmd: 'addInk',
      stroke: s,
      name: t('drawTools.inkName'),
      ...(auto ? { autoShape: true } : {}),
      ...(attachTo !== null ? { attachTo } : {}),
    }).then((r) => {
      after(r);
      const made = r?.ink?.converted?.shapes[0];
      lastLine.current = made && (made.kind === 'line' || made.kind === 'arrow') ? made.id : null;
    });
  };

  const pickedObjects = ink.filter((o) => picked.includes(o.id));
  const layer = active ? (
    <DrawLayer
      slide={size}
      state={state}
      ink={ink}
      picked={pickedObjects}
      onStroke={stroke}
      onErase={(points, radius, mode) =>
        void run({ cmd: 'eraseInk', points, radius, mode }).then(after)
      }
      onLasso={(path) => setPicked(lassoPick(ink, path))}
      onRuler={(r) => setState((s) => ({ ...s, ruler: r }))}
      labels={{
        layer: t('drawTools.layer'),
        ruler: t('drawTools.ruler'),
        rulerAngle: t('drawTools.rulerAngle'),
      }}
    />
  ) : enabled && (state.ruler || pickedObjects.length > 0) ? (
    // The ruler and the picked ink stay visible (not interactive) while selecting.
    <svg
      viewBox={`0 0 ${size.width} ${size.height}`}
      style={{
        position: 'absolute',
        inset: 0,
        width: '100%',
        height: '100%',
        pointerEvents: 'none',
      }}
      aria-hidden="true"
    >
      {state.ruler && (
        <g transform={`translate(${state.ruler.x} ${state.ruler.y}) rotate(${state.ruler.angle})`}>
          <rect
            x={-RULER_LENGTH / 2}
            y={-28}
            width={RULER_LENGTH}
            height={56}
            rx={3}
            fill="rgba(250,250,248,0.6)"
            stroke="#8a8a8a"
            strokeWidth={0.75}
          />
        </g>
      )}
      {pickedObjects.map((o) => (
        <rect
          key={o.id}
          data-testid="draw-picked"
          x={o.rect[0]}
          y={o.rect[1]}
          width={o.rect[2]}
          height={o.rect[3]}
          fill="none"
          stroke="#2d6cdf"
          strokeWidth={1}
          strokeDasharray="3 2"
        />
      ))}
    </svg>
  ) : null;

  const replayNode = (width: number) =>
    replay && enabled ? (
      <InkReplay
        backdrop={replay.backdrop}
        objects={replay.objects}
        glyphs={glyphs}
        images={images}
        width={width}
        onDone={() => setReplay(null)}
        labels={{
          region: t('drawTools.replay'),
          stop: t('drawTools.stopReplay'),
          again: t('drawTools.replayAgain'),
          progress: t('drawTools.replayProgress'),
        }}
      />
    ) : null;

  return {
    state,
    ink,
    picked,
    handwriting: hw,
    layer,
    replay: replayNode,
  };
}
