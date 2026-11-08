import type { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type {
  EngineHost,
  SlideAnimInfo,
  SlideAnimTrigger,
  SlideCommand,
  SlideEditResult,
  SlideShowInfo,
  SlideTransitionKind,
} from '@blueoffice/platform';
import { ArrowDown, ArrowUp, Play, Trash, X } from 'lucide-react';
import { useCallback, useMemo, useState } from 'react';
import type { ShowPos } from './player';
import styles from './SlideEditor.module.css';
import { ShowCache, ShowStage } from './ShowStage';

/** Transitions offered in the pane (ECMA-376 Part 1, §19.3.1.50 children). */
export const TRANSITIONS: SlideTransitionKind[] = [
  'none',
  'cut',
  'fade',
  'push',
  'wipe',
  'cover',
  'pull',
  'split',
  'random',
];

/** Directions of a transition kind. */
export function transitionDirs(kind: string): string[] {
  switch (kind) {
    case 'push':
    case 'wipe':
      return ['l', 'r', 'u', 'd'];
    case 'cover':
    case 'pull':
      return ['l', 'r', 'u', 'd', 'lu', 'ru', 'ld', 'rd'];
    case 'split':
      return ['horz', 'vert'];
    default:
      return [];
  }
}

const TRIGGERS: SlideAnimTrigger[] = ['onClick', 'withPrevious', 'afterPrevious'];
const ANIM_DIRS = ['b', 't', 'l', 'r'];

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
}: AnimationPaneProps) {
  const { t } = useTranslation();
  const label = useEffectLabel();
  const slide = edit?.slide ?? 0;
  const tr = edit?.transition ?? null;
  const anims = edit?.animations ?? [];
  const kind = tr?.kind ?? 'none';
  const known = !tr || tr.known;
  const selectedIds = new Set((edit?.selection ?? []).map((s) => s.id));

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
  }) =>
    run({
      cmd: 'setTransition',
      transition: {
        kind: (patch.kind ?? 'keep') as SlideTransitionKind | 'keep',
        dir: patch.dir,
        durMs: patch.durMs,
        advanceOnClick: patch.advanceOnClick,
        advanceAfterMs: patch.advanceAfterMs,
      },
    });

  const dirs = transitionDirs(kind);
  return (
    <aside
      className={styles.animPane}
      aria-label={t('slideEditor.animPane')}
      data-testid="anim-pane"
    >
      <header className={styles.animPaneHead}>
        <h3>{t('slideEditor.animPane')}</h3>
        <button type="button" onClick={onClose} aria-label={t('slideEditor.close')}>
          <X size={14} />
        </button>
      </header>

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
            {TRANSITIONS.map((k) => (
              <option key={k} value={k}>
                {t(`slideEditor.transition.${k}`)}
              </option>
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
                  className={selectedIds.has(a.shapeId) ? styles.animItemActive : styles.animItem}
                  data-testid={`anim-item-${a.index}`}
                >
                  <button
                    type="button"
                    className={styles.animName}
                    onClick={() => run({ cmd: 'select', ids: [a.shapeId] })}
                  >
                    <span className={styles.animStep}>{a.click}</span>
                    <span>{a.shapeName || `#${a.shapeId}`}</span>
                    <em>{label(a)}</em>
                  </button>
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
                    {(a.effect === 'flyIn' || a.effect === 'wipe') && (
                      <select
                        aria-label={t('slideEditor.direction')}
                        value={a.direction ?? 'b'}
                        onChange={(e) => set({ direction: e.target.value })}
                      >
                        {ANIM_DIRS.map((d) => (
                          <option key={d} value={d}>
                            {t(`slideEditor.from.${d}`)}
                          </option>
                        ))}
                      </select>
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
    </aside>
  );
}
