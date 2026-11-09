/**
 * Spelling and grammar in BlueSlide (CORE-SPELL-006): wavy lines under the issues of the shown
 * slide's text (each run in its `a:rPr/@lang`, `noProof` runs skipped), corrections on a right
 * click (also while a text box is edited) and the F7 check of the whole deck — slides, tables,
 * SmartArt text and speaker notes (`ProofWalkDialog`). Checking runs in the engine
 * (`bo_engine::slide_edit::spell`, the BlueDoc checker); corrections are undoable transactions.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  EngineHost,
  SlideIssue,
  SlideSpellAnswer,
  SlideSpellCommand,
  SlideSpellMark,
  SpellInfo,
} from '@blueoffice/platform';
import {
  PROOF_LANGUAGES,
  ProofWalkDialog,
  SpellMenu,
  loadIgnoredWords,
  loadUserWords,
  saveIgnoredWord,
  saveUserWord,
  type ProofWalkActions,
  type WalkIssue,
} from '@blueoffice/editor';
import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import type { SlideController } from './controller';

const ON_KEY = 'bo.spell.slideAsYouType';

const ask = async (engine: EngineHost, cmd: SlideSpellCommand) =>
  ((await engine.slideCommand(cmd)) as unknown as SlideSpellAnswer).slideSpell ?? {};

function loadOn(): boolean {
  try {
    return localStorage.getItem(ON_KEY) !== '0';
  } catch {
    return true;
  }
}

/** Points along a wavy line from `a` to `b` (amplitude and step in slide points). */
export function wavePath(
  a: readonly [number, number],
  b: readonly [number, number],
  amp: number,
  step: number,
): string {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = Math.hypot(dx, dy);
  if (len === 0 || step <= 0) return '';
  const ux = dx / len;
  const uy = dy / len;
  // Normal pointing down from the text (the line is under it).
  const nx = -uy;
  const ny = ux;
  const n = Math.max(1, Math.round(len / step));
  const pts: string[] = [];
  for (let i = 0; i <= n; i++) {
    const t = (i / n) * len;
    const o = i % 2 === 0 ? 0 : amp;
    pts.push(`${(a[0] + ux * t + nx * o).toFixed(2)},${(a[1] + uy * t + ny * o).toFixed(2)}`);
  }
  return `M${pts.join('L')}`;
}

/** Is slide point (x, y) on the word above mark `m`? */
export function onMark(m: SlideSpellMark, x: number, y: number, tol = 2): boolean {
  const dx = m.to[0] - m.from[0];
  const dy = m.to[1] - m.from[1];
  const len = Math.hypot(dx, dy);
  if (len === 0) return false;
  const ux = dx / len;
  const uy = dy / len;
  const px = x - m.from[0];
  const py = y - m.from[1];
  const along = px * ux + py * uy;
  // Distance below the line (positive) or above it, where the text is (negative).
  const below = px * -uy + py * ux;
  return along >= -tol && along <= len + tol && below <= tol * 2 && below >= -m.h - tol;
}

/** Spelling state of the slide editor: marks of the shown slide, as-you-type switch, dialog. */
export function useSlideSpelling(
  engine: EngineHost,
  /** Changes whenever the deck may have changed (the last engine result). */
  revision: unknown,
  slide: number,
  hasDeck: boolean,
) {
  const [asYouType, setAsYouType] = useState(loadOn);
  const [dialog, setDialog] = useState(false);
  const [marks, setMarks] = useState<{ slide: number; list: SlideSpellMark[] }>({
    slide: -1,
    list: [],
  });
  const [version, setVersion] = useState(0);
  const loaded = useRef(false);
  useEffect(() => {
    if (!hasDeck || loaded.current) return;
    loaded.current = true;
    // The user dictionary and ignored words of this browser / app reach the engine once.
    for (const word of loadUserWords())
      void ask(engine, { cmd: 'slideSpellAddWord', word }).catch(() => {});
    for (const word of loadIgnoredWords())
      void ask(engine, { cmd: 'slideSpellIgnore', word }).catch(() => {});
  }, [engine, hasDeck]);
  useEffect(() => {
    if (!hasDeck || !asYouType) return;
    let live = true;
    const timer = setTimeout(() => {
      void ask(engine, { cmd: 'slideSpelling', slide })
        .then((r) => {
          if (live) setMarks({ slide, list: r.marks ?? [] });
        })
        .catch(() => {});
    }, 300);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [engine, revision, slide, hasDeck, asYouType, version]);
  const toggle = useCallback(() => {
    setAsYouType((on) => {
      try {
        localStorage.setItem(ON_KEY, on ? '0' : '1');
      } catch {
        // Session only.
      }
      return !on;
    });
  }, []);
  const refresh = useCallback(() => setVersion((n) => n + 1), []);
  const shown = asYouType && marks.slide === slide ? marks.list : [];
  return { asYouType, toggle, dialog, setDialog, marks: shown, refresh };
}

/**
 * Wavy lines over the slide (an SVG in slide points like the selection overlay; it takes no
 * pointer events) and the correction menu of a right click on a marked word, on the slide or in
 * the text box being edited.
 */
export function SlideSpellLayer({
  engine,
  controller,
  container,
  slide,
  size,
  width,
  marks,
  onChanged,
}: {
  engine: EngineHost;
  controller: SlideController;
  /** Element around the slide (receives the right clicks). */
  container: RefObject<HTMLElement | null>;
  slide: number;
  /** Slide size in points. */
  size: { width: number; height: number };
  /** Displayed width in CSS pixels. */
  width: number;
  marks: SlideSpellMark[];
  /** Words or the text changed: check again. */
  onChanged: () => void;
}) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    info: SpellInfo;
    issue: SlideIssue;
  } | null>(null);
  const scale = size.width > 0 ? width / size.width : 1;
  const height = size.height * scale;
  const marksRef = useRef(marks);
  useEffect(() => {
    marksRef.current = marks;
  }, [marks]);

  useEffect(() => {
    const el = container.current;
    if (!el) return;
    const onMenu = (e: MouseEvent) => {
      const r = svgRef.current?.getBoundingClientRect();
      if (!r || r.width === 0) return;
      const x = ((e.clientX - r.left) / r.width) * size.width;
      const y = ((e.clientY - r.top) / r.height) * size.height;
      if (!marksRef.current.some((m) => onMark(m, x, y))) return;
      // The menu replaces the browser's own on a marked word.
      e.preventDefault();
      e.stopPropagation();
      const at = { x: e.clientX, y: e.clientY };
      void ask(engine, { cmd: 'slideSpellAt', slide, x, y })
        .then((a) => {
          const hit = a.hit;
          if (!hit) return;
          setMenu({
            ...at,
            issue: hit,
            info: {
              word: hit.word,
              para: hit.para,
              start: hit.start,
              end: hit.end,
              suggestions: hit.suggestions,
              rule: hit.rule,
            },
          });
        })
        .catch(() => {});
    };
    el.addEventListener('contextmenu', onMenu, true);
    return () => el.removeEventListener('contextmenu', onMenu, true);
  }, [container, engine, slide, size.width, size.height]);

  const close = useCallback(() => setMenu(null), []);
  const amp = 1.6 / scale;
  return (
    <>
      <svg
        ref={svgRef}
        aria-hidden
        data-testid="slide-spell-layer"
        width={width}
        height={height}
        viewBox={`0 0 ${size.width} ${size.height}`}
        style={{ position: 'absolute', inset: 0, pointerEvents: 'none', overflow: 'visible' }}
      >
        {marks.map((m, i) => (
          <path
            key={i}
            data-testid="slide-spell-mark"
            data-grammar={m.grammar ? 'true' : undefined}
            d={wavePath(m.from, m.to, amp, amp * 1.6)}
            fill="none"
            stroke={m.grammar ? '#1f5fbf' : '#c42b1c'}
            strokeWidth={1.2}
            vectorEffect="non-scaling-stroke"
          />
        ))}
      </svg>
      {menu && (
        <SpellMenu
          menu={menu}
          onClose={close}
          onPick={(text) => {
            const { target, para, start, end, word } = menu.issue;
            setMenu(null);
            void controller
              .run({
                cmd: 'slideSpellReplace',
                edits: [{ target, para, start, end, text, word }],
              })
              .then(onChanged);
          }}
          onIgnore={(word) => {
            setMenu(null);
            const rule = menu.issue.rule;
            if (!rule) saveIgnoredWord(word);
            void ask(
              engine,
              rule
                ? { cmd: 'slideGrammarIgnore', rule, text: word }
                : { cmd: 'slideSpellIgnore', word },
            )
              .then(onChanged)
              .catch(() => {});
          }}
          onAdd={(word) => {
            setMenu(null);
            saveUserWord(word);
            void ask(engine, { cmd: 'slideSpellAddWord', word })
              .then(onChanged)
              .catch(() => {});
          }}
        />
      )}
    </>
  );
}

type SlideWalkIssue = WalkIssue & { issue: SlideIssue };

/** F7: every text of the deck — slides, tables, SmartArt, speaker notes. */
export function SlideSpellDialog({
  engine,
  controller,
  onClose,
  onShow,
  onChanged,
}: {
  engine: EngineHost;
  controller: SlideController;
  onClose: () => void;
  /** Show the issue (go to its slide, select its shape). */
  onShow: (issue: SlideIssue) => void;
  onChanged: () => void;
}) {
  const { t } = useTranslation();
  const where = useCallback(
    (i: SlideIssue) => {
      const n = i.slide + 1;
      switch (i.target.kind) {
        case 'cell':
          return t('spell.inSlideTable', { n });
        case 'notes':
          return t('spell.inSlideNotes', { n });
        case 'smartArt':
          return t('spell.inSlideSmartArt', { n });
        default:
          return t('spell.inSlide', { n });
      }
    },
    [t],
  );
  const load = useCallback(async () => {
    const r = await ask(engine, { cmd: 'slideSpellCheck' });
    return {
      unchecked: r.unchecked ?? [],
      issues: (r.issues ?? []).map((i): SlideWalkIssue => ({
        ...i,
        pos: [i.unit, i.para, i.start],
        where: where(i),
        issue: i,
      })),
    };
  }, [engine, where]);
  const edit = useCallback(
    async (list: SlideWalkIssue[], text: string) => {
      await controller.run({
        cmd: 'slideSpellReplace',
        edits: list.map(({ issue: i }) => ({
          target: i.target,
          para: i.para,
          start: i.start,
          end: i.end,
          text,
          word: i.word,
        })),
      });
      onChanged();
    },
    [controller, onChanged],
  );
  const actions = useMemo<ProofWalkActions<SlideWalkIssue>>(
    () => ({
      load,
      change: (i, text) => edit([i], text),
      changeAll: (list, text) => edit(list, text),
      ignoreAll: async (i) => {
        if (i.rule) {
          await ask(engine, { cmd: 'slideGrammarIgnore', rule: i.rule, text: i.word });
        } else {
          await ask(engine, { cmd: 'slideSpellIgnore', word: i.word });
          saveIgnoredWord(i.word);
        }
        onChanged();
      },
      add: async (i) => {
        await ask(engine, { cmd: 'slideSpellAddWord', word: i.word });
        saveUserWord(i.word);
        onChanged();
      },
      show: (i) => onShow(i.issue),
    }),
    [engine, load, edit, onShow, onChanged],
  );
  const langName = (l: string) =>
    (PROOF_LANGUAGES as readonly string[]).includes(l) ? t(`spell.lang.${l}`) : l;
  return <ProofWalkDialog actions={actions} onClose={onClose} langName={langName} />;
}
