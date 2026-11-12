/**
 * Screen-reader access to the text of a BlueDoc document (WCAG 2.2 1.3.1, 4.1.2): the pages
 * are pixels on a canvas, so an off-screen DOM mirror holds the paragraphs of the pages in view
 * (± one page) and of the caret's paragraph — headings, list items, tables with header cells,
 * links and the alternative text of pictures — built from the engine's `speechTree` query. It is
 * visually hidden, not `display: none`, so browse modes of screen readers read it; the caret's
 * paragraph is marked `aria-current` and described to the focused typing box. A polite live
 * region announces caret moves (the character, the word after Ctrl+arrows, the line after
 * up/down/Home/End/page keys) and the text typed or deleted.
 */
import {
  byteToUtf16,
  type DocTreePara,
  type EditResult,
  type EngineHost,
} from '@blueoffice/platform';

/** Localised texts of the mirror. */
export interface MirrorLabels {
  /** Name of the mirror (1-based pages shown). */
  region(from: number, to: number): string;
  /** A picture: with its alternative text, or without (`alt` empty). */
  picture(alt: string): string;
  equation(): string;
  object(): string;
  /** Announcement of deleted text. */
  deleted(text: string): string;
  /** Announcement of an empty line or a caret at a paragraph end. */
  blank(): string;
}

export const DEFAULT_MIRROR_LABELS: MirrorLabels = {
  region: (a, b) => (a === b ? `Text of page ${a}` : `Text of pages ${a}–${b}`),
  picture: (alt) => (alt ? `Picture: ${alt}` : 'Picture without alternative text'),
  equation: () => 'Equation',
  object: () => 'Object',
  deleted: (t) => `Deleted ${t}`,
  blank: () => 'Blank',
};

/** Visually hidden but in the accessibility tree (not `display: none`). */
export const VISUALLY_HIDDEN =
  'position:absolute;left:0;top:0;width:1px;height:1px;margin:-1px;padding:0;border:0;overflow:hidden;clip-path:inset(50%);white-space:nowrap;pointer-events:none;';

/** Link targets kept as links (others are shown as plain text). */
function safeHref(href: string): string | null {
  return /^(https?:|mailto:|#)/i.test(href.trim()) ? href.trim() : null;
}

const OBJECT = '\uFFFC';

/** The inline content of a paragraph: text, links, objects with their text alternative. */
export function paragraphContent(p: DocTreePara, labels: MirrorLabels): DocumentFragment {
  const frag = document.createDocumentFragment();
  const text = p.text;
  const u = (b: number) => byteToUtf16(text, b);
  const links = (p.links ?? []).map((l) => ({ start: u(l.start), end: u(l.end), href: l.href }));
  const objects = new Map((p.objects ?? []).map((o) => [u(o.at), o]));
  const plain = (s: string, into: Node) => {
    // Breaks inside the paragraph (line, page and column breaks, tabs) read as spaces.
    const t = s
      .replace(/[\u2028\t]/g, ' ')
      .replaceAll('\u000b', ' ')
      .replaceAll('\u000c', ' ')
      .replace(/\u00ad/g, '');
    if (t) into.appendChild(document.createTextNode(t));
  };
  const objectNode = (at: number) => {
    const o = objects.get(at);
    const span = document.createElement('span');
    span.setAttribute('role', 'img');
    span.setAttribute(
      'aria-label',
      !o || o.kind === 'object'
        ? labels.object()
        : o.kind === 'equation'
          ? labels.equation()
          : labels.picture(o.alt),
    );
    span.textContent = ' ';
    return span;
  };
  const range = (from: number, to: number, into: Node) => {
    let run = from;
    for (let i = from; i < to; i++) {
      if (text[i] !== OBJECT) continue;
      plain(text.slice(run, i), into);
      into.appendChild(objectNode(i));
      run = i + 1;
    }
    plain(text.slice(run, to), into);
  };
  let at = 0;
  for (const l of links) {
    if (l.start < at) continue;
    range(at, l.start, frag);
    const href = safeHref(l.href);
    const a = document.createElement(href ? 'a' : 'span');
    if (href && a instanceof HTMLAnchorElement) {
      a.href = href;
      a.tabIndex = -1;
    }
    range(l.start, l.end, a);
    frag.appendChild(a);
    at = l.end;
  }
  range(at, text.length, frag);
  return frag;
}

/** Element id of paragraph `p` in a mirror with id prefix `prefix`. */
export const paraId = (prefix: string, p: number) => `${prefix}-p${p}`;

/** Build the mirror's content from the engine's paragraphs (document order). */
export function buildMirror(
  paras: readonly DocTreePara[],
  labels: MirrorLabels,
  prefix: string,
  caretPara: number | null,
): DocumentFragment {
  const out = document.createDocumentFragment();
  let list: HTMLUListElement | null = null;
  let table: { key: string; el: HTMLTableElement; rows: Map<number, HTMLTableRowElement> } | null =
    null;
  const cells = new Map<string, HTMLTableCellElement>();
  for (const p of paras) {
    let el: HTMLElement;
    if (p.heading) {
      el = p.heading <= 6 ? document.createElement(`h${p.heading}`) : document.createElement('div');
      if (p.heading > 6) {
        el.setAttribute('role', 'heading');
        el.setAttribute('aria-level', String(p.heading));
      }
    } else if (p.list) {
      el = document.createElement('li');
      el.setAttribute('aria-level', String(p.list));
    } else {
      el = document.createElement('p');
    }
    el.id = paraId(prefix, p.p);
    el.dataset.p = String(p.p);
    if (p.p === caretPara) el.setAttribute('aria-current', 'true');
    el.appendChild(paragraphContent(p, labels));
    if (!p.text.replace(/\s/g, '')) el.dataset.empty = 'true';

    // Where it goes: a table cell, a list, or the mirror itself.
    let parent: Node = out;
    if (p.cell) {
      list = null;
      const c = p.cell;
      if (!table || table.key !== c.table) {
        const t = document.createElement('table');
        t.dataset.table = c.table;
        table = { key: c.table, el: t, rows: new Map() };
        out.appendChild(t);
      }
      let row = table.rows.get(c.row);
      if (!row) {
        row = document.createElement('tr');
        row.setAttribute('aria-rowindex', String(c.row + 1));
        table.el.appendChild(row);
        table.rows.set(c.row, row);
      }
      table.el.setAttribute('aria-rowcount', String(c.rows));
      const key = `${c.table}:${c.row}:${c.col}`;
      let cell = cells.get(key);
      if (!cell) {
        cell = document.createElement(c.header ? 'th' : 'td');
        if (c.header) cell.setAttribute('scope', 'col');
        cell.setAttribute('aria-colindex', String(c.col + 1));
        row.appendChild(cell);
        cells.set(key, cell);
      }
      parent = cell;
    } else {
      table = null;
    }
    if (p.list && !p.heading) {
      if (!list || list.parentNode !== parent) {
        list = document.createElement('ul');
        parent.appendChild(list);
      }
      list.appendChild(el);
    } else {
      if (!p.cell) list = null;
      parent.appendChild(el);
    }
  }
  return out;
}

/** The word at UTF-16 offset `at` of `text` (the next one when `at` is on a space). */
export function wordAt(text: string, at: number): string {
  const rest = text.slice(at).replace(/^\s+/, '');
  return /^\S+/.exec(rest)?.[0] ?? '';
}

/** What changed in a paragraph between `before` and `after`: inserted and deleted text. */
export function textChange(
  before: string,
  after: string,
): { inserted: string; deleted: string } | null {
  if (before === after) return null;
  let a = 0;
  while (a < before.length && a < after.length && before[a] === after[a]) a++;
  let b = 0;
  while (
    b < before.length - a &&
    b < after.length - a &&
    before[before.length - 1 - b] === after[after.length - 1 - b]
  ) {
    b++;
  }
  return {
    inserted: after.slice(a, after.length - b),
    deleted: before.slice(a, before.length - b),
  };
}

type Pos = EditResult['selection']['focus'];

/** Keys that move the caret by line (the line is announced). */
const LINE_KEYS = new Set(['ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown']);

/**
 * The mirror of one document view. `setRange` follows the pages in view, `invalidate` the
 * edits, `select` the caret; each change is applied once the engine answered (`idle()`).
 */
export class DocMirror {
  /** The visually hidden structure. */
  readonly root: HTMLDivElement;
  /** Polite announcements of caret moves and edits. */
  readonly live: HTMLDivElement;
  private range: [number, number] = [0, 0];
  private caret: Pos | null = null;
  private moved: Pos | null = null;
  private edited = false;
  private texts = new Map<number, string>();
  // A pending caret query can observe an edit before its selection result arrives.
  // Preserve the pre-input text so the eventual edit announcement still has a baseline.
  private editBase: Map<number, string> | null = null;
  private lastKey: { key: string; ctrl: boolean } | null = null;
  private pending: Promise<void> | null = null;
  private again = false;
  private seq = 0;
  private static count = 0;
  private readonly prefix: string;

  constructor(
    parent: HTMLElement,
    private readonly engine: EngineHost,
    private readonly labels: MirrorLabels = DEFAULT_MIRROR_LABELS,
    /** The focused typing box, described by the caret's paragraph. */
    private readonly input?: HTMLElement,
  ) {
    this.prefix = `bo-a11y-${++DocMirror.count}`;
    this.root = document.createElement('div');
    this.root.className = 'bo-a11y-mirror';
    this.root.dataset.testid = 'doc-a11y-mirror';
    this.root.setAttribute('role', 'region');
    this.root.style.cssText = VISUALLY_HIDDEN;
    this.live = document.createElement('div');
    this.live.className = 'bo-a11y-live';
    this.live.dataset.testid = 'doc-a11y-live';
    this.live.setAttribute('aria-live', 'polite');
    this.live.setAttribute('aria-atomic', 'true');
    this.live.style.cssText = VISUALLY_HIDDEN;
    parent.appendChild(this.root);
    parent.appendChild(this.live);
    // The key that moved the caret decides what is announced.
    parent.addEventListener('keydown', this.onKey, true);
    parent.addEventListener('beforeinput', this.onBeforeInput, true);
  }

  private onKey = (e: KeyboardEvent) => {
    const ctrl = e.ctrlKey || e.metaKey;
    this.lastKey = { key: e.key, ctrl };
    if (
      (!ctrl &&
        !e.altKey &&
        (e.key.length === 1 || ['Backspace', 'Delete', 'Enter'].includes(e.key))) ||
      (ctrl && ['v', 'x', 'z', 'y'].includes(e.key.toLowerCase()))
    ) {
      this.editBase ??= new Map(this.texts);
    }
  };

  private onBeforeInput = () => {
    this.editBase ??= new Map(this.texts);
  };

  /** The pages in view (0-based, inclusive): the mirror covers them ± one page. */
  setRange(first: number, last: number): void {
    const next: [number, number] = [Math.max(0, first - 1), last + 1];
    if (next[0] === this.range[0] && next[1] === this.range[1]) return;
    this.range = next;
    this.schedule();
  }

  /** The document changed: read the paragraphs again. */
  invalidate(): void {
    this.schedule();
  }

  /** The caret moved to `focus`; `edited`: by an edit of the document. */
  select(focus: Pos, edited: boolean): void {
    this.moved = focus;
    this.edited ||= edited;
    this.schedule();
  }

  /** Resolves once the pending changes are applied. */
  async idle(): Promise<void> {
    while (this.pending) await this.pending;
  }

  /** Say `text` (polite). */
  announce(text: string): void {
    // A new node each time, so that the same text is announced again.
    const n = document.createElement('div');
    n.textContent = text;
    this.live.replaceChildren(n);
  }

  dispose(): void {
    this.root.parentElement?.removeEventListener('keydown', this.onKey, true);
    this.root.parentElement?.removeEventListener('beforeinput', this.onBeforeInput, true);
    this.root.remove();
    this.live.remove();
  }

  private schedule(): void {
    if (this.pending) {
      this.again = true;
      return;
    }
    this.pending = (async () => {
      // Changes made together (an edit refreshes the view, then moves the caret) are applied
      // together: the edit is compared with the text before it. The mirror follows once the
      // next frame is painted, so that its engine query and DOM work never delay the typed
      // text on screen.
      await new Promise<void>((resolve) => {
        if (typeof requestAnimationFrame !== 'function') {
          resolve();
          return;
        }
        // A hidden page gets no frames: the mirror follows anyway.
        const late = setTimeout(resolve, 100);
        requestAnimationFrame(() =>
          setTimeout(() => {
            clearTimeout(late);
            resolve();
          }, 0),
        );
      });
      try {
        do {
          this.again = false;
          await this.flush();
        } while (this.again);
      } catch {
        // No document (closed meanwhile): nothing to mirror.
      } finally {
        this.pending = null;
      }
    })();
  }

  private async flush(): Promise<void> {
    const seq = ++this.seq;
    const prev = this.caret;
    const focus = this.moved ?? this.caret;
    const edited = this.edited;
    const key = this.lastKey;
    this.moved = null;
    this.edited = false;
    this.lastKey = null;
    const [from, to] = this.range;
    const r = await this.engine.command({
      cmd: 'speechTree',
      fromPage: from,
      toPage: to,
      include: focus?.p,
    });
    if (seq !== this.seq) return;
    const paras = r.speech?.tree ?? [];
    const old = this.texts;
    this.texts = new Map(paras.map((p) => [p.p, p.text]));
    this.caret = focus;
    this.render(paras, focus);
    if (!focus) return;
    const text = this.texts.get(focus.p) ?? '';
    if (edited && prev) {
      // Typed or deleted text (a paragraph joined or split shows as a deletion / insertion).
      const baseline = this.editBase ?? old;
      this.editBase = null;
      const ch = prev.p === focus.p ? textChange(baseline.get(focus.p) ?? '', text) : null;
      if (ch?.deleted && !ch.inserted) this.announce(this.labels.deleted(ch.deleted));
      else if (ch?.inserted) this.announce(ch.inserted);
      return;
    }
    if (!key || !prev || (prev.p === focus.p && prev.o === focus.o)) return;
    const at = byteToUtf16(text, focus.o);
    if (LINE_KEYS.has(key.key)) {
      const l = await this.engine.command({ cmd: 'speechLine', p: focus.p, o: focus.o });
      const [s, e] = l.speech?.line ?? [0, 0];
      const line = text.slice(byteToUtf16(text, s), byteToUtf16(text, e)).trim();
      this.announce(line || this.labels.blank());
    } else if (key.ctrl && (key.key === 'ArrowLeft' || key.key === 'ArrowRight')) {
      this.announce(wordAt(text, at) || this.labels.blank());
    } else if (key.key === 'ArrowLeft' || key.key === 'ArrowRight') {
      const c = String.fromCodePoint(text.codePointAt(at) ?? 0x20);
      this.announce(at >= text.length || !c.trim() ? this.labels.blank() : c);
    }
  }

  private render(paras: DocTreePara[], focus: Pos | null): void {
    const [from, to] = this.range;
    const pages = paras.map((p) => p.page);
    const lo = pages.length ? Math.min(...pages) : from;
    const hi = pages.length ? Math.max(...pages) : to;
    this.root.setAttribute('aria-label', this.labels.region(lo + 1, hi + 1));
    this.root.replaceChildren(buildMirror(paras, this.labels, this.prefix, focus?.p ?? null));
    if (this.input) {
      const id = focus && paras.some((p) => p.p === focus.p) ? paraId(this.prefix, focus.p) : null;
      if (id) this.input.setAttribute('aria-describedby', id);
      else this.input.removeAttribute('aria-describedby');
    }
  }
}
