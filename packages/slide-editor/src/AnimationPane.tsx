import type { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type {
  EngineHost,
  SlideAnimInfo,
  SlideAnimTrigger,
  SlideCommand,
  SlideEditResult,
  SlideShowInfo,
  SlideAnimEffect,
  SlideTransitionKind,
} from '@blueoffice/platform';
import { ArrowDown, ArrowUp, GripVertical, Play, Trash, X } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import { BUILTIN_SOUNDS, builtinName, builtinOf, builtinWav, type BuiltinSound } from './actions';
import type { ShowPos } from './player';
import { ALL_TRANSITIONS, TRANSITION_GROUPS } from './transitionFx';
import styles from './SlideEditor.module.css';
import { ShowCache, ShowStage } from './ShowStage';

/** Transitions offered in the pane (ECMA-376 Part 1, §19.3.1.50 children, [MS-PPTX] p14 / p15). */
export const TRANSITIONS: SlideTransitionKind[] = ['none', ...ALL_TRANSITIONS];

/**
 * Directions of a transition kind before the engine reports them (`SlideTransitionInfo.dirs` is the
 * source once the transition is set).
 */
export function transitionDirs(kind: string): string[] {
  switch (kind) {
    case 'push':
    case 'wipe':
      return ['l', 'u', 'r', 'd'];
    case 'cover':
    case 'pull':
      return ['l', 'u', 'r', 'd', 'lu', 'ru', 'ld', 'rd'];
    case 'split':
      return ['horz', 'vert', 'horzIn', 'vertIn'];
    default:
      return [];
  }
}

/** Bytes → base64 (a WAV sound for the engine). */
function base64Of(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

const TRIGGERS: SlideAnimTrigger[] = ['onClick', 'withPrevious', 'afterPrevious'];

/** Effects by class (`bo_pptx::anim_catalog`; entrance effects also run as exits). */
export const ENTRANCE_EFFECTS: SlideAnimEffect[] = [
  'appear',
  'fade',
  'flyIn',
  'wipe',
  'blinds',
  'box',
  'checkerboard',
  'circle',
  'crawl',
  'diamond',
  'dissolve',
  'peek',
  'plus',
  'randomBars',
  'split',
  'strips',
  'swivel',
  'wedge',
  'wheel',
  'zoom',
  'bounce',
  'float',
  'growTurn',
  'spinner',
  'expand',
];
export const EMPHASIS_EFFECTS: SlideAnimEffect[] = [
  'growShrink',
  'spin',
  'transparency',
  'pulse',
  'teeter',
  'blink',
  'fillColor',
];
export const PATH_EFFECTS: SlideAnimEffect[] = [
  'lineDown',
  'lineUp',
  'lineLeft',
  'lineRight',
  'arcDown',
  'arcUp',
  'turnDown',
  'circlePath',
  'squarePath',
  'diamondPath',
  'triangle',
  'loop',
  'wave',
];

/** The effects that can replace one of class `cls`. */
function effectsOf(cls: string): SlideAnimEffect[] {
  if (cls === 'emph') return EMPHASIS_EFFECTS;
  if (cls === 'path') return PATH_EFFECTS;
  return ENTRANCE_EFFECTS;
}

/** Build choices of each kind of shape (`SlideAnimSpec.build`); text adds one per outline level. */
export function buildChoices(b: SlideAnimInfo['build']): string[] {
  if (!b) return [];
  switch (b.kind) {
    case 'text':
      return [
        'whole',
        'allAtOnce',
        ...Array.from({ length: Math.max(1, b.levels) }, (_, i) => `p${i + 1}`),
      ];
    case 'chart':
      return ['whole', 'allAtOnce', 'series', 'category', 'seriesEl', 'categoryEl'];
    case 'diagram':
      return ['whole', 'allAtOnce', 'one', 'lvlAtOnce', 'lvlOne'];
  }
}

/**
 * The build animates the shape (text) or draws the chart background first as a step of its own:
 * text built paragraph by paragraph, a chart built element by element.
 */
export function hasBgOption(b: NonNullable<SlideAnimInfo['build']>): boolean {
  if (b.kind === 'text') return b.value !== 'whole';
  if (b.kind === 'chart') return b.value !== 'whole' && b.value !== 'allAtOnce';
  return false;
}

/** Where an effect moves in the list when dragged from `from` and dropped on row `over`. */
export function dropIndex(from: number, over: number, count: number): number | null {
  if (from < 0 || over < 0 || from >= count || over >= count || from === over) return null;
  return over;
}

/** Seconds field ↔ milliseconds. */
const toSec = (ms: number) => Math.round(ms / 100) / 10;
const toMs = (v: string) => {
  const n = Number(v.replace(',', '.'));
  return Number.isFinite(n) && n >= 0 ? Math.round(n * 1000) : null;
};

export interface AnimationPaneProps {
  engine: EngineHost;
  glyphs: GlyphStore;
  images: ImageStore;
  edit: SlideEditResult | null;
  slideSize: { width: number; height: number };
  /** Bumped when the deck changes (refreshes the preview). */
  revision: number;
  run: (cmd: SlideCommand) => void;
  onClose: () => void;
  /** Start the preview from outside (ribbon command). */
  previewToken: number;
  /** Ask for a WAV file (through the platform); `null` when cancelled. */
  pickSound?: () => Promise<{ bytes: Uint8Array; name: string } | null>;
  /** Inside a task pane (CORE-UI-006): the host draws the title bar and close button. */
  embedded?: boolean;
  /** Start drawing a motion path for the selection (command `anim.drawPath`). */
  onDrawPath?: () => void;
  /** Effect whose motion path points are edited on the slide. */
  editingPath?: number | null;
  /** Edit the points of effect `index` on the slide (`null`: stop). */
  onEditPath?: (index: number | null) => void;
}

/** What part of its shape an effect plays on (`đoạn 2–3`, `chuỗi 1`, …); empty: the whole shape. */
function usePartLabel() {
  const { t } = useTranslation();
  return useCallback(
    (a: SlideAnimInfo): string => {
      const g = a.target;
      if (!g) return '';
      switch (g.kind) {
        case 'background':
          return t('slideEditor.partOf.background');
        case 'paragraphs':
          return g.start === g.end
            ? t('slideEditor.partOf.paragraph', { n: (g.start ?? 0) + 1 })
            : t('slideEditor.partOf.paragraphs', {
                start: (g.start ?? 0) + 1,
                end: (g.end ?? 0) + 1,
              });
        case 'chart':
          if (g.step === 'gridLegend') return t('slideEditor.partOf.background');
          if (g.series != null && g.category != null)
            return t('slideEditor.partOf.point', { s: g.series + 1, c: g.category + 1 });
          if (g.series != null) return t('slideEditor.partOf.series', { n: g.series + 1 });
          if (g.category != null) return t('slideEditor.partOf.category', { n: g.category + 1 });
          return '';
        case 'diagram':
          return g.node != null ? t('slideEditor.partOf.node', { n: g.node }) : '';
      }
    },
    [t],
  );
}

/** Effect label of an animation (`Xuất hiện`, `Mờ dần (biến mất)`, …). */
function useEffectLabel() {
  const { t } = useTranslation();
  return useCallback(
    (a: SlideAnimInfo) => {
      const name = t(`slideEditor.effect.${a.effect}`);
      return a.class === 'entr' ? name : `${name} (${t(`slideEditor.animClass.${a.class}`)})`;
    },
    [t],
  );
}

/**
 * Side pane of the Transitions / Animations tabs: the slide's transition
 * (effect, direction, duration, advance) and its animation sequence (order,
 * trigger, duration, delay), with a preview of the slide.
 */
export function AnimationPane({
  engine,
  glyphs,
  images,
  edit,
  slideSize,
  revision,
  run,
  onClose,
  previewToken,
  pickSound,
  embedded = false,
  onDrawPath,
  editingPath = null,
  onEditPath,
}: AnimationPaneProps) {
  const { t } = useTranslation();
  const label = useEffectLabel();
  const partLabel = usePartLabel();
  const slide = edit?.slide ?? 0;
  const tr = edit?.transition ?? null;
  const anims = edit?.animations ?? [];
  const kind = tr?.kind ?? 'none';
  const known = !tr || tr.known;
  const selectedIds = new Set((edit?.selection ?? []).map((s) => s.id));
  const objects = edit?.objects ?? [];
  // Timeline: each effect's bar on its click step (or trigger) scale.
  const span = new Map<string, number>();
  for (const a of anims) {
    const g = `${a.triggerShape ?? 'main'}:${a.click}`;
    span.set(g, Math.max(span.get(g) ?? 0, a.endMs));
  }
  // Animation painter: the next selection gets the effects of the shape picked here.
  const [painter, setPainter] = useState<number | null>(null);
  // Drag and drop in the list: the effect dragged and the row it is over.
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);
  const firstSelected = edit?.selection?.[0]?.id ?? null;
  const [painterSeen, setPainterSeen] = useState<number | null>(null);
  if (painter !== null && firstSelected !== null && firstSelected !== painterSeen) {
    // Adjusting state while rendering (React docs): apply once to the new selection.
    setPainterSeen(firstSelected);
    if (firstSelected !== painter) {
      setPainter(null);
      run({ cmd: 'copyAnimations', from: painter });
    }
  }

  // Preview: the slide plays with its transition and every click step in turn.
  // Each run has its own show data (`run` = deck revision + slide + start).
  const [preview, setPreview] = useState<(ShowPos & { run: string }) | null>(null);
  const [token, setToken] = useState(previewToken);
  const runId = `${revision}:${slide}`;
  const startPreview = useCallback(
    () => setPreview({ slide, click: 0, enter: 'transition', run: `${runId}:${Date.now()}` }),
    [slide, runId],
  );
  if (token !== previewToken) {
    // Asked from the ribbon (adjusting state while rendering, React docs).
    setToken(previewToken);
    setPreview({ slide, click: 0, enter: 'transition', run: `${runId}:${previewToken}` });
  }
  const active = preview && preview.run.startsWith(`${runId}:`) ? preview : null;
  const runKey = active?.run ?? '';
  const cache = useMemo(() => new ShowCache(engine, runKey), [engine, runKey]);
  const onIdle = useCallback((s: SlideShowInfo) => {
    setPreview((p) => {
      if (!p || p.slide !== s.slide) return p;
      if (p.click < s.clicks) return { ...p, click: p.click + 1, enter: 'click' };
      return p;
    });
  }, []);

  const setTransition = (patch: {
    kind?: string;
    dir?: string;
    durMs?: number;
    advanceOnClick?: boolean;
    advanceAfterMs?: number;
    sound?: 'none' | 'stop';
    soundData?: string;
    soundName?: string;
    soundLoop?: boolean;
  }) =>
    run({
      cmd: 'setTransition',
      transition: {
        kind: (patch.kind ?? 'keep') as SlideTransitionKind | 'keep',
        dir: patch.dir,
        durMs: patch.durMs,
        advanceOnClick: patch.advanceOnClick,
        advanceAfterMs: patch.advanceAfterMs,
        sound: patch.sound,
        soundData: patch.soundData,
        soundName: patch.soundName,
        soundLoop: patch.soundLoop,
      },
    });
  const sound = tr?.sound ?? null;
  const soundValue = !sound ? 'none' : sound.stop ? 'stop' : 'file';
  const onSound = async (v: string) => {
    if (v === 'none' || v === 'stop') {
      setTransition({ sound: v });
      return;
    }
    const f = pickSound ? await pickSound() : null;
    if (f) setTransition({ soundData: base64Of(f.bytes), soundName: f.name });
  };

  const dirs = tr?.dirs ?? transitionDirs(kind);
  const Root = embedded ? 'div' : 'aside';
  return (
    <Root
      className={embedded ? styles.animPaneEmbedded : styles.animPane}
      aria-label={embedded ? undefined : t('slideEditor.animPane')}
      data-testid="anim-pane"
    >
      {!embedded && (
        <header className={styles.animPaneHead}>
          <h3>{t('slideEditor.animPane')}</h3>
          <button type="button" onClick={onClose} aria-label={t('slideEditor.close')}>
            <X size={14} />
          </button>
        </header>
      )}

      <section className={styles.animSection}>
        <h4>{t('slideEditor.transitionTab')}</h4>
        <label>
          {t('slideEditor.transitionEffect')}
          <select
            data-testid="transition-kind"
            value={known ? kind : `other:${kind}`}
            onChange={(e) => setTransition({ kind: e.target.value })}
          >
            {!known && (
              <option value={`other:${kind}`} disabled>
                {t('slideEditor.transition.other', { name: kind })}
              </option>
            )}
            <option value="none">{t('slideEditor.transition.none')}</option>
            {TRANSITION_GROUPS.map((g) => (
              <optgroup key={g.group} label={t(`slideEditor.transitionGroup.${g.group}`)}>
                {g.kinds.map((k) => (
                  <option key={k} value={k}>
                    {t(`slideEditor.transition.${k}`)}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </label>
        {dirs.length > 0 && (
          <label>
            {t('slideEditor.direction')}
            <select
              data-testid="transition-dir"
              value={tr?.dir ?? dirs[0]}
              onChange={(e) => setTransition({ kind, dir: e.target.value })}
            >
              {dirs.map((d) => (
                <option key={d} value={d}>
                  {t(`slideEditor.dir.${d}`)}
                </option>
              ))}
            </select>
          </label>
        )}
        {tr && (
          <>
            <label>
              {t('slideEditor.durationSec')}
              <input
                key={`d-${slide}-${tr.durMs}`}
                type="number"
                min={0.1}
                step={0.1}
                data-testid="transition-duration"
                defaultValue={toSec(tr.durMs)}
                onBlur={(e) => {
                  const ms = toMs(e.target.value);
                  if (ms && ms !== tr.durMs) setTransition({ durMs: ms });
                }}
              />
            </label>
            <label>
              {t('slideEditor.transitionSound')}
              <select
                data-testid="transition-sound"
                value={soundValue}
                onChange={(e) => void onSound(e.target.value)}
              >
                <option value="none">{t('slideEditor.sound.none')}</option>
                <option value="stop">{t('slideEditor.sound.stop')}</option>
                <option value="file">
                  {sound && !sound.stop ? sound.name : t('slideEditor.sound.file')}
                </option>
              </select>
            </label>
            {sound && !sound.stop && (
              <label className={styles.check}>
                <input
                  type="checkbox"
                  checked={sound.loop}
                  onChange={(e) => setTransition({ soundLoop: e.target.checked })}
                />
                {t('slideEditor.sound.loop')}
              </label>
            )}
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={tr.advanceOnClick}
                onChange={(e) => setTransition({ advanceOnClick: e.target.checked })}
              />
              {t('slideEditor.advanceOnClick')}
            </label>
            <label>
              {t('slideEditor.advanceAfter')}
              <input
                key={`a-${slide}-${tr.advanceAfterMs ?? 0}`}
                type="number"
                min={0}
                step={0.5}
                data-testid="transition-after"
                defaultValue={tr.advanceAfterMs ? toSec(tr.advanceAfterMs) : ''}
                placeholder={t('slideEditor.never')}
                onBlur={(e) => {
                  const ms = e.target.value === '' ? 0 : toMs(e.target.value);
                  if (ms !== null && ms !== (tr.advanceAfterMs ?? 0)) {
                    setTransition({ advanceAfterMs: ms });
                  }
                }}
              />
            </label>
          </>
        )}
      </section>

      <section className={styles.animSection}>
        <h4>{t('slideEditor.animationsTab')}</h4>
        <button
          type="button"
          data-testid="anim-painter"
          className={painter !== null ? styles.animPainterOn : undefined}
          disabled={firstSelected === null || !anims.some((a) => a.shapeId === firstSelected)}
          onClick={() => {
            setPainterSeen(firstSelected);
            setPainter(painter === null ? firstSelected : null);
          }}
        >
          {painter !== null ? t('slideEditor.painterPick') : t('slideEditor.painter')}
        </button>
        <label>
          {t('slideEditor.addEffect')}
          <select
            data-testid="anim-add"
            value=""
            disabled={selectedIds.size === 0}
            onChange={(e) => {
              const [cls, effect] = e.target.value.split(':');
              if (!effect) return;
              if (effect === 'customPath') {
                onDrawPath?.();
                return;
              }
              run({
                cmd: 'addAnimation',
                effect: effect as SlideAnimEffect,
                class: cls === 'exit' ? 'exit' : 'entr',
              });
            }}
          >
            <option value="">{t('slideEditor.addEffectPick')}</option>
            {(
              [
                ['entr', ENTRANCE_EFFECTS],
                ['emph', EMPHASIS_EFFECTS],
                ['exit', ENTRANCE_EFFECTS],
                ['path', PATH_EFFECTS],
              ] as const
            ).map(([cls, list]) => (
              <optgroup key={cls} label={t(`slideEditor.effectGroup.${cls}`)}>
                {list.map((e) => (
                  <option key={e} value={`${cls}:${e}`}>
                    {t(`slideEditor.effect.${e}`)}
                  </option>
                ))}
                {cls === 'path' && onDrawPath && (
                  <option value="path:customPath">{t('slideEditor.drawPath')}</option>
                )}
              </optgroup>
            ))}
          </select>
        </label>
        {anims.length === 0 ? (
          <p className={styles.animEmpty}>{t('slideEditor.noAnimations')}</p>
        ) : (
          <ol className={styles.animList} data-testid="anim-list">
            {anims.map((a) => {
              const set = (
                spec: Omit<Extract<SlideCommand, { cmd: 'setAnimation' }>, 'cmd' | 'index'>,
              ) => run({ cmd: 'setAnimation', index: a.index, ...spec });
              return (
                <li
                  key={`${a.index}-${a.shapeId}`}
                  className={`${selectedIds.has(a.shapeId) ? styles.animItemActive : styles.animItem}${
                    dragOver === a.index && dragFrom !== null && dragFrom !== a.index
                      ? ` ${styles.animDropTarget}`
                      : ''
                  }`}
                  data-testid={`anim-item-${a.index}`}
                  draggable
                  onDragStart={(e) => {
                    setDragFrom(a.index);
                    e.dataTransfer.effectAllowed = 'move';
                    e.dataTransfer.setData('text/plain', String(a.index));
                  }}
                  onDragOver={(e) => {
                    if (dragFrom === null) return;
                    e.preventDefault();
                    e.dataTransfer.dropEffect = 'move';
                    if (dragOver !== a.index) setDragOver(a.index);
                  }}
                  onDragLeave={() => {
                    if (dragOver === a.index) setDragOver(null);
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    const raw = Number(e.dataTransfer.getData('text/plain'));
                    const from = dragFrom ?? (Number.isFinite(raw) ? raw : -1);
                    const to = dropIndex(from, a.index, anims.length);
                    setDragFrom(null);
                    setDragOver(null);
                    if (to !== null) run({ cmd: 'moveAnimation', from, to });
                  }}
                  onDragEnd={() => {
                    setDragFrom(null);
                    setDragOver(null);
                  }}
                >
                  <button
                    type="button"
                    className={styles.animName}
                    onClick={() => run({ cmd: 'select', ids: [a.shapeId] })}
                  >
                    <span className={styles.animGrip} title={t('slideEditor.dragReorder')}>
                      <GripVertical size={12} aria-hidden="true" />
                    </span>
                    <span className={styles.animStep}>
                      {a.triggerShape != null ? '⚡' : a.click}
                    </span>
                    <span>
                      {a.shapeName || `#${a.shapeId}`}
                      {a.target && (
                        <span className={styles.animPart} data-testid={`anim-part-${a.index}`}>
                          {' · '}
                          {partLabel(a)}
                        </span>
                      )}
                    </span>
                    <em>{label(a)}</em>
                  </button>
                  <div className={styles.animTimeline} aria-hidden="true">
                    <span
                      className={styles.animBar}
                      style={{
                        left: `${(100 * a.startMs) / Math.max(1, span.get(`${a.triggerShape ?? 'main'}:${a.click}`) ?? 1)}%`,
                        width: `${Math.max(2, (100 * (a.endMs - a.startMs)) / Math.max(1, span.get(`${a.triggerShape ?? 'main'}:${a.click}`) ?? 1))}%`,
                      }}
                    />
                  </div>
                  <div className={styles.animFields}>
                    <select
                      aria-label={t('slideEditor.trigger')}
                      data-testid={`anim-trigger-${a.index}`}
                      value={a.trigger}
                      onChange={(e) => set({ trigger: e.target.value as SlideAnimTrigger })}
                    >
                      {TRIGGERS.map((g) => (
                        <option key={g} value={g}>
                          {t(`slideEditor.triggers.${g}`)}
                        </option>
                      ))}
                    </select>
                    {a.effect !== 'other' && a.effect !== 'customPath' && (
                      <select
                        aria-label={t('slideEditor.transitionEffect')}
                        data-testid={`anim-effect-${a.index}`}
                        value={a.effect}
                        onChange={(e) => set({ effect: e.target.value as SlideAnimEffect })}
                      >
                        {effectsOf(a.class).map((e) => (
                          <option key={e} value={e}>
                            {t(`slideEditor.effect.${e}`)}
                          </option>
                        ))}
                      </select>
                    )}
                    {(a.dirs ?? []).length > 0 && (
                      <select
                        aria-label={t('slideEditor.direction')}
                        data-testid={`anim-dir-${a.index}`}
                        value={a.direction ?? a.dirs?.[0] ?? ''}
                        onChange={(e) => set({ direction: e.target.value })}
                      >
                        {(a.dirs ?? []).map((d) => (
                          <option key={d} value={d}>
                            {t(`slideEditor.animDir.${d}`)}
                          </option>
                        ))}
                      </select>
                    )}
                    {a.class !== 'path' && !a.mediaCmd && (
                      <select
                        aria-label={t('slideEditor.textBy')}
                        data-testid={`anim-text-by-${a.index}`}
                        value={a.textBy === 'wd' ? 'word' : a.textBy === 'lt' ? 'letter' : 'all'}
                        onChange={(e) =>
                          set({ textBy: e.target.value as 'all' | 'word' | 'letter' })
                        }
                      >
                        {(['all', 'word', 'letter'] as const).map((o) => (
                          <option key={o} value={o}>
                            {t(`slideEditor.textByOpt.${o}`)}
                          </option>
                        ))}
                      </select>
                    )}
                    {(a.textBy === 'wd' || a.textBy === 'lt') && (
                      <label>
                        {t('slideEditor.textDelayPct')}
                        <input
                          key={`p-${a.index}-${a.textStepMs ?? 0}`}
                          type="number"
                          min={0}
                          max={100}
                          step={1}
                          data-testid={`anim-text-delay-${a.index}`}
                          defaultValue={
                            a.durMs > 0 ? Math.round((100 * (a.textStepMs ?? 0)) / a.durMs) : 0
                          }
                          onBlur={(e) => {
                            const v = Math.round(Number(e.target.value));
                            if (Number.isFinite(v) && v >= 0)
                              set({ textDelayPct: Math.min(100, v) });
                          }}
                        />
                      </label>
                    )}
                    {a.build?.head && !a.mediaCmd && (
                      <>
                        <select
                          aria-label={t(`slideEditor.build.${a.build.kind}`)}
                          data-testid={`anim-build-${a.index}`}
                          value={a.build.value}
                          onChange={(e) => set({ build: e.target.value })}
                        >
                          {buildChoices(a.build).map((v) => (
                            <option key={v} value={v}>
                              {/^p\d$/.test(v)
                                ? t('slideEditor.buildOpt.byLevel', { n: Number(v.slice(1)) })
                                : t(`slideEditor.buildOpt.${v}`)}
                            </option>
                          ))}
                        </select>
                        {a.build.kind !== 'chart' && a.build.value !== 'whole' && (
                          <label className={styles.check}>
                            <input
                              type="checkbox"
                              data-testid={`anim-build-reverse-${a.index}`}
                              checked={a.build.reverse}
                              onChange={(e) => set({ buildReverse: e.target.checked })}
                            />
                            {t('slideEditor.buildReverse')}
                          </label>
                        )}
                        {hasBgOption(a.build) ? (
                          <label className={styles.check}>
                            <input
                              type="checkbox"
                              data-testid={`anim-build-bg-${a.index}`}
                              checked={a.build.bg}
                              onChange={(e) => set({ buildBg: e.target.checked })}
                            />
                            {a.build.kind === 'chart'
                              ? t('slideEditor.buildBgChart')
                              : t('slideEditor.buildBgText')}
                          </label>
                        ) : null}
                      </>
                    )}
                    {!a.mediaCmd && (
                      <>
                        <select
                          aria-label={t('slideEditor.afterAnim')}
                          data-testid={`anim-after-${a.index}`}
                          value={a.after ?? 'none'}
                          onChange={(e) =>
                            set({
                              after: e.target.value as 'none' | 'dim' | 'hide' | 'hideOnClick',
                            })
                          }
                        >
                          {(['none', 'dim', 'hide', 'hideOnClick'] as const).map((o) => (
                            <option key={o} value={o}>
                              {t(`slideEditor.afterOpt.${o}`)}
                            </option>
                          ))}
                        </select>
                        {a.after === 'dim' && (
                          <input
                            type="color"
                            aria-label={t('slideEditor.dimColor')}
                            data-testid={`anim-dim-color-${a.index}`}
                            value={`#${(a.dimColor ?? '808080').toLowerCase()}`}
                            onChange={(e) =>
                              set({ dimColor: e.target.value.slice(1).toUpperCase() })
                            }
                          />
                        )}
                        <label className={styles.check}>
                          <input
                            type="checkbox"
                            data-testid={`anim-rewind-${a.index}`}
                            checked={a.rewind === true}
                            onChange={(e) => set({ rewind: e.target.checked })}
                          />
                          {t('slideEditor.rewind')}
                        </label>
                        <select
                          aria-label={t('slideEditor.effectSound')}
                          data-testid={`anim-sound-${a.index}`}
                          value={!a.sound ? 'none' : (builtinOf(a.sound.name) ?? 'keep')}
                          onChange={(e) => {
                            const v = e.target.value;
                            if (v === 'none') {
                              set({ sound: 'none' });
                            } else if (v === 'file') {
                              void (pickSound ? pickSound() : Promise.resolve(null)).then((f) => {
                                if (f) set({ soundData: base64Of(f.bytes), soundName: f.name });
                              });
                            } else if ((BUILTIN_SOUNDS as readonly string[]).includes(v)) {
                              const id = v as BuiltinSound;
                              set({
                                soundData: base64Of(builtinWav(id)),
                                soundName: builtinName(id),
                              });
                            }
                          }}
                        >
                          <option value="none">{t('slideActions.soundNone')}</option>
                          {BUILTIN_SOUNDS.map((b) => (
                            <option key={b} value={b}>
                              {t(`slideActions.builtin.${b}`)}
                            </option>
                          ))}
                          {a.sound && !builtinOf(a.sound.name) && (
                            <option value="keep">{a.sound.name}</option>
                          )}
                          <option value="file">{t('slideActions.soundFile')}</option>
                        </select>
                      </>
                    )}
                    {a.class === 'path' && a.path && (
                      <span className={styles.animButtons}>
                        <button
                          type="button"
                          data-testid={`anim-reverse-path-${a.index}`}
                          onClick={() => set({ reversePath: true })}
                        >
                          {t('slideEditor.reversePath')}
                        </button>
                        <label className={styles.check}>
                          <input
                            type="checkbox"
                            data-testid={`anim-lock-path-${a.index}`}
                            checked={a.pathLocked === true}
                            onChange={(e) => set({ lockPath: e.target.checked })}
                          />
                          {t('slideEditor.lockPath')}
                        </label>
                        {onEditPath && (
                          <button
                            type="button"
                            data-testid={`anim-edit-path-${a.index}`}
                            aria-pressed={editingPath === a.index}
                            onClick={() => onEditPath(editingPath === a.index ? null : a.index)}
                          >
                            {t('slideEditor.editPathPoints')}
                          </button>
                        )}
                      </span>
                    )}
                    <label>
                      {t('slideEditor.durationSec')}
                      <input
                        key={`d-${a.index}-${a.durMs}`}
                        type="number"
                        min={0.1}
                        step={0.1}
                        data-testid={`anim-duration-${a.index}`}
                        defaultValue={toSec(a.durMs)}
                        onBlur={(e) => {
                          const ms = toMs(e.target.value);
                          if (ms && ms !== a.durMs) set({ durMs: ms });
                        }}
                      />
                    </label>
                    <select
                      aria-label={t('slideEditor.repeat')}
                      data-testid={`anim-repeat-${a.index}`}
                      value={a.repeat ?? 1}
                      onChange={(e) => set({ repeat: Number(e.target.value) })}
                    >
                      {[1, 2, 3, 4, 5, 10, 0].map((n) => (
                        <option key={n} value={n}>
                          {n === 0
                            ? t('slideEditor.repeatUntilClick')
                            : t('slideEditor.repeatTimes', { n })}
                        </option>
                      ))}
                    </select>
                    <select
                      aria-label={t('slideEditor.triggerShape')}
                      data-testid={`anim-trigger-shape-${a.index}`}
                      value={a.triggerShape ?? 0}
                      onChange={(e) => set({ triggerShape: Number(e.target.value) })}
                    >
                      <option value={0}>{t('slideEditor.triggerSequence')}</option>
                      {objects
                        .filter((o) => o.id !== a.shapeId)
                        .map((o) => (
                          <option key={o.id} value={o.id}>
                            {t('slideEditor.triggerOnClickOf', { name: o.name || `#${o.id}` })}
                          </option>
                        ))}
                    </select>
                    <label>
                      {t('slideEditor.delaySec')}
                      <input
                        key={`w-${a.index}-${a.delayMs}`}
                        type="number"
                        min={0}
                        step={0.1}
                        data-testid={`anim-delay-${a.index}`}
                        defaultValue={toSec(a.delayMs)}
                        onBlur={(e) => {
                          const ms = toMs(e.target.value);
                          if (ms !== null && ms !== a.delayMs) set({ delayMs: ms });
                        }}
                      />
                    </label>
                    <span className={styles.animButtons}>
                      <button
                        type="button"
                        aria-label={t('slideEditor.moveEarlier')}
                        title={t('slideEditor.moveEarlier')}
                        disabled={a.index === 0}
                        onClick={() =>
                          run({ cmd: 'moveAnimation', from: a.index, to: a.index - 1 })
                        }
                      >
                        <ArrowUp size={13} />
                      </button>
                      <button
                        type="button"
                        aria-label={t('slideEditor.moveLater')}
                        title={t('slideEditor.moveLater')}
                        disabled={a.index === anims.length - 1}
                        onClick={() =>
                          run({ cmd: 'moveAnimation', from: a.index, to: a.index + 1 })
                        }
                      >
                        <ArrowDown size={13} />
                      </button>
                      <button
                        type="button"
                        aria-label={t('slideEditor.removeAnimation')}
                        title={t('slideEditor.removeAnimation')}
                        data-testid={`anim-remove-${a.index}`}
                        onClick={() => run({ cmd: 'removeAnimation', index: a.index })}
                      >
                        <Trash size={13} />
                      </button>
                    </span>
                  </div>
                </li>
              );
            })}
          </ol>
        )}
      </section>

      <section className={styles.animSection}>
        <button type="button" data-testid="anim-preview" onClick={startPreview}>
          <Play size={14} /> {t('slideEditor.preview')}
        </button>
        {active && (
          <div className={styles.animPreview}>
            <ShowStage
              engine={engine}
              glyphs={glyphs}
              images={images}
              cache={cache}
              pos={active}
              from={null}
              slide={slideSize}
              width={240}
              media={false}
              onIdle={onIdle}
              label={t('slideEditor.preview')}
            />
          </div>
        )}
      </section>
    </Root>
  );
}
