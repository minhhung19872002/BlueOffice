/**
 * Spelling and grammar in BlueSheet (CORE-SPELL-006): wavy lines under the misspelt words of the
 * text being typed (cell editor and formula bar) with corrections on a right click, and the F7
 * check of every text cell (`ProofWalkDialog`). Checking runs in the engine
 * (`bo_engine::sheet::spell_cmd`, the BlueDoc checker); corrections of cells are one undoable
 * engine edit. The proofing language is a BlueOffice preference per workbook (SpreadsheetML has
 * no language for cell text), kept by this browser / app.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  CellIssue,
  SheetHost,
  SheetSpellReport,
  SpellInfo,
  TextIssue,
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
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from 'react';
import { createPortal } from 'react-dom';
import { colName } from './grid';

const LANG_KEY = 'bo.spell.sheetLang';
const ON_KEY = 'bo.spell.sheetAsYouType';

/** Proofing language kept for a workbook (by file name); `fallback` when none was chosen. */
export function loadSheetLang(file: string, fallback: string): string {
  try {
    const map: unknown = JSON.parse(localStorage.getItem(LANG_KEY) ?? '{}');
    const v = map && typeof map === 'object' ? (map as Record<string, unknown>)[file] : undefined;
    return typeof v === 'string' ? v : fallback;
  } catch {
    return fallback;
  }
}

export function saveSheetLang(file: string, lang: string): void {
  try {
    const map: unknown = JSON.parse(localStorage.getItem(LANG_KEY) ?? '{}');
    const next = { ...(map && typeof map === 'object' ? map : {}), [file]: lang };
    localStorage.setItem(LANG_KEY, JSON.stringify(next));
  } catch {
    // Storage unavailable (private mode): the language lasts for this session only.
  }
}

function loadOn(): boolean {
  try {
    return localStorage.getItem(ON_KEY) !== '0';
  } catch {
    return true;
  }
}

/** Proofing state of the sheet editor: language, as-you-type switch, dialog. */
export function useSheetSpelling(host: SheetHost | null | undefined, file: string, ui: string) {
  // The language picked for this workbook in this session, else the one kept for it.
  const [chosen, setChosen] = useState<{ file: string; lang: string } | null>(null);
  const lang = chosen?.file === file ? chosen.lang : loadSheetLang(file, ui);
  const [asYouType, setAsYouType] = useState(loadOn);
  const [dialog, setDialog] = useState(false);
  // The user dictionary and ignored words of this browser / app reach the engine once.
  useEffect(() => {
    if (!host) return;
    for (const word of loadUserWords())
      void host.exec({ cmd: 'spellAddWord', word }).catch(() => {});
    for (const word of loadIgnoredWords())
      void host.exec({ cmd: 'spellIgnore', word }).catch(() => {});
  }, [host]);
  const setLang = useCallback(
    (l: string) => {
      setChosen({ file, lang: l });
      saveSheetLang(file, l);
    },
    [file],
  );
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
  return { lang, setLang, asYouType, toggle, dialog, setDialog };
}

/** The cell editor and the formula bar (text areas) or a plain input. */
type TextField = HTMLInputElement | HTMLTextAreaElement;

/** Sets an input's value as typing would (React sees an `input` event). */
function typeInto(el: TextField, value: string): void {
  const proto =
    el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  const set = Object.getOwnPropertyDescriptor(proto, 'value')?.set;
  set?.call(el, value);
  el.dispatchEvent(new Event('input', { bubbles: true }));
}

/** Issues of `text` from the engine, checked a moment after typing stops. */
function useTextIssues(
  host: SheetHost | null | undefined,
  text: string | null,
  lang: string,
  version: number,
): TextIssue[] {
  const [issues, setIssues] = useState<{ text: string; list: TextIssue[] }>({
    text: '',
    list: [],
  });
  useEffect(() => {
    if (!host || !text || text.startsWith('=')) return;
    let live = true;
    const timer = setTimeout(() => {
      void host
        .exec<{ issues: TextIssue[] }>({ cmd: 'spellText', text, lang })
        .then((r) => {
          if (live) setIssues({ text, list: r.issues });
        })
        .catch(() => {});
    }, 200);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [host, text, lang, version]);
  // Results of an older text are never drawn under a newer one.
  return text && issues.text === text ? issues.list : [];
}

/** Where and how an input shows its text (read from the DOM after layout). */
interface Geo {
  left: number;
  top: number;
  width: number;
  height: number;
  border: string;
  padding: string;
  font: string;
  letterSpacing: string;
  scrollLeft: number;
}

/** Wavy lines over a text input: a transparent copy of its text with the issues underlined. */
function Underlay({
  input,
  text,
  issues,
}: {
  input: RefObject<TextField | null>;
  text: string;
  issues: TextIssue[];
}) {
  const [geo, setGeo] = useState<Geo | null>(null);
  useEffect(() => {
    const el = input.current;
    if (!el) return;
    let frame = 0;
    const measure = () => {
      const r = el.getBoundingClientRect();
      const cs = getComputedStyle(el);
      setGeo({
        left: r.left,
        top: r.top,
        width: r.width,
        height: r.height,
        border: `${cs.borderTopWidth} ${cs.borderRightWidth} ${cs.borderBottomWidth} ${cs.borderLeftWidth}`,
        padding: `${cs.paddingTop} ${cs.paddingRight} ${cs.paddingBottom} ${cs.paddingLeft}`,
        font: cs.font,
        letterSpacing: cs.letterSpacing,
        scrollLeft: el.scrollLeft,
      });
    };
    const bump = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };
    bump();
    const events = ['scroll', 'keyup', 'select', 'input', 'focus'] as const;
    events.forEach((e) => el.addEventListener(e, bump));
    window.addEventListener('resize', bump);
    window.addEventListener('scroll', bump, true);
    return () => {
      cancelAnimationFrame(frame);
      events.forEach((e) => el.removeEventListener(e, bump));
      window.removeEventListener('resize', bump);
      window.removeEventListener('scroll', bump, true);
    };
  }, [input, text, issues]);
  if (!geo || geo.width === 0 || geo.height === 0) return null;
  const pieces: ReactNode[] = [];
  let at = 0;
  for (const [k, i] of issues.entries()) {
    if (i.from < at || i.to > text.length) continue;
    pieces.push(text.slice(at, i.from));
    pieces.push(
      <span
        key={k}
        data-testid="cell-spell-mark"
        data-from={i.from}
        data-grammar={i.grammar ? 'true' : undefined}
        style={{
          textDecorationLine: 'underline',
          textDecorationStyle: 'wavy',
          textDecorationColor: i.grammar ? '#1f5fbf' : '#c42b1c',
          textDecorationSkipInk: 'none',
        }}
      >
        {text.slice(i.from, i.to)}
      </span>,
    );
    at = i.to;
  }
  pieces.push(text.slice(at));
  return createPortal(
    <div
      aria-hidden
      data-testid="cell-spell-layer"
      style={{
        position: 'fixed',
        left: geo.left,
        top: geo.top,
        width: geo.width,
        height: geo.height,
        boxSizing: 'border-box',
        borderStyle: 'solid',
        borderColor: 'transparent',
        borderWidth: geo.border,
        padding: geo.padding,
        font: geo.font,
        letterSpacing: geo.letterSpacing,
        color: 'transparent',
        overflow: 'hidden',
        pointerEvents: 'none',
        display: 'flex',
        alignItems: 'center',
        zIndex: 20,
      }}
    >
      <span style={{ whiteSpace: 'pre', transform: `translateX(${-geo.scrollLeft}px)` }}>
        {pieces}
      </span>
    </div>,
    document.body,
  );
}

/** The mark under a client point, among the drawn marks. */
function markAt(x: number, y: number): HTMLElement | null {
  for (const el of document.querySelectorAll<HTMLElement>('[data-testid="cell-spell-mark"]')) {
    const b = el.getBoundingClientRect();
    if (x >= b.left && x <= b.right && y >= b.top - 2 && y <= b.bottom + 2) return el;
  }
  return null;
}

/**
 * Spelling of the text being typed in a cell: underlines in the given inputs (cell editor,
 * formula bar) and the correction menu of a right click on an underlined word.
 */
export function SheetEditSpelling({
  host,
  inputs,
  text,
  lang,
  enabled,
}: {
  host: SheetHost | null | undefined;
  inputs: RefObject<TextField | null>[];
  /** Text being typed (`null`: not editing). */
  text: string | null;
  lang: string;
  enabled: boolean;
}) {
  const [version, setVersion] = useState(0);
  const issues = useTextIssues(host, enabled ? text : null, lang, version);
  const latest = useRef(issues);
  useEffect(() => {
    latest.current = issues;
  }, [issues]);
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    info: SpellInfo;
    input: TextField;
    issue: TextIssue;
  } | null>(null);
  useEffect(() => {
    const els = inputs.map((r) => r.current).filter((e): e is TextField => !!e);
    const onMenu = (e: MouseEvent) => {
      const from = Number(markAt(e.clientX, e.clientY)?.dataset.from ?? NaN);
      const issue = latest.current.find((i) => i.from === from);
      if (!issue) return;
      e.preventDefault();
      e.stopPropagation();
      setMenu({
        x: e.clientX,
        y: e.clientY,
        input: e.currentTarget as TextField,
        issue,
        info: {
          word: issue.word,
          para: 0,
          start: issue.start,
          end: issue.end,
          suggestions: issue.suggestions,
          rule: issue.rule,
        },
      });
    };
    const targets: HTMLElement[] = els;
    targets.forEach((el) => el.addEventListener('contextmenu', onMenu));
    return () => targets.forEach((el) => el.removeEventListener('contextmenu', onMenu));
  }, [inputs]);

  const close = useCallback(() => setMenu(null), []);
  const shown = enabled && text !== null && issues.length > 0;
  return (
    <>
      {shown &&
        inputs.map((ref, i) => <Underlay key={i} input={ref} text={text} issues={issues} />)}
      {menu && (
        <SpellMenu
          menu={menu}
          onClose={close}
          onPick={(s) => {
            const el = menu.input;
            const v = el.value;
            const { from, to, word } = menu.issue;
            setMenu(null);
            if (v.slice(from, to) !== word) return;
            typeInto(el, v.slice(0, from) + s + v.slice(to));
            el.focus();
            el.setSelectionRange(from + s.length, from + s.length);
          }}
          onIgnore={(word) => {
            setMenu(null);
            const cmd = menu.issue.rule
              ? { cmd: 'grammarIgnore' as const, rule: menu.issue.rule, text: word }
              : { cmd: 'spellIgnore' as const, word };
            if (!menu.issue.rule) saveIgnoredWord(word);
            void host
              ?.exec(cmd)
              .then(() => setVersion((n) => n + 1))
              .catch(() => {});
          }}
          onAdd={(word) => {
            setMenu(null);
            saveUserWord(word);
            void host
              ?.exec({ cmd: 'spellAddWord', word })
              .then(() => setVersion((n) => n + 1))
              .catch(() => {});
          }}
        />
      )}
    </>
  );
}

type SheetWalkIssue = WalkIssue & { cell: CellIssue };

/** The language picker of the check dialog: any dictionary, or one language. */
export function SheetLangSelect({
  lang,
  onChange,
}: {
  lang: string;
  onChange: (l: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <label style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 8 }}>
      <span>{t('spell.workbookLanguage')}</span>
      <select
        data-testid="spellcheck-sheet-language"
        value={lang}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value="">{t('spell.anyLanguage')}</option>
        {PROOF_LANGUAGES.map((l) => (
          <option key={l} value={l}>
            {t(`spell.lang.${l}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

/** F7: every text cell of the workbook (formulas, numbers and dates are not checked). */
export function SheetSpellDialog({
  host,
  lang,
  setLang,
  onClose,
  onShow,
  onChanged,
}: {
  host: SheetHost;
  lang: string;
  setLang: (l: string) => void;
  onClose: () => void;
  /** Select the cell of the issue. */
  onShow: (sheet: number, row: number, col: number) => void;
  /** Cells changed (refresh the grid). */
  onChanged: () => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const edit = useCallback(
    async (list: SheetWalkIssue[], text: string) => {
      await host.exec({
        cmd: 'spellReplace',
        edits: list.map(({ cell: c }) => ({
          sheet: c.sheet,
          row: c.row,
          col: c.col,
          start: c.start,
          end: c.end,
          text,
          word: c.word,
        })),
      });
      await onChanged();
    },
    [host, onChanged],
  );
  // Stable while the language stays: a new `load` starts the walk again.
  const load = useCallback(async () => {
    const r = await host.exec<SheetSpellReport>({ cmd: 'spellCheck', lang });
    return {
      unchecked: r.unchecked,
      issues: r.issues.map((c): SheetWalkIssue => ({
        ...c,
        pos: [c.sheet, c.row, c.col, c.start],
        where: t('spell.inCell', { sheet: c.sheetName, cell: `${colName(c.col)}${c.row + 1}` }),
        cell: c,
      })),
    };
  }, [host, lang, t]);
  const actions = useMemo<ProofWalkActions<SheetWalkIssue>>(
    () => ({
      load,
      change: (i, text) => edit([i], text),
      changeAll: (list, text) => edit(list, text),
      ignoreAll: async (i) => {
        if (i.rule) {
          await host.exec({ cmd: 'grammarIgnore', rule: i.rule, text: i.word });
        } else {
          await host.exec({ cmd: 'spellIgnore', word: i.word });
          saveIgnoredWord(i.word);
        }
      },
      add: async (i) => {
        await host.exec({ cmd: 'spellAddWord', word: i.word });
        saveUserWord(i.word);
      },
      show: (i) => onShow(i.cell.sheet, i.cell.row, i.cell.col),
    }),
    [host, load, edit, onShow],
  );
  const langName = (l: string) =>
    (PROOF_LANGUAGES as readonly string[]).includes(l) ? t(`spell.lang.${l}`) : l;
  return (
    <ProofWalkDialog
      actions={actions}
      onClose={onClose}
      reload={lang}
      langName={langName}
      settings={<SheetLangSelect lang={lang} onChange={setLang} />}
    />
  );
}
