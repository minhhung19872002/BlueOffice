/**
 * Slides from an outline (SLIDE-ALBUM-001): the headings of a BlueDoc file
 * (`.docx`) or the indented lines of a text file become title + content slides.
 *
 * - `.docx`: a paragraph's outline level comes from `w:outlineLvl` (ECMA-376
 *   Part 1, §17.3.1.20) on the paragraph or on its style (`w:pStyle`,
 *   §17.3.1.27, following `w:basedOn`), else from a style named `heading N`.
 *   Level 1 → slide title, level 2 and deeper → bullets (level 2 = first bullet
 *   level). Paragraphs that are not headings are left out; a document without
 *   any heading gives one slide per paragraph.
 * - `.txt`: one line per paragraph; a line's level is its indentation (one tab,
 *   or one indentation unit of spaces — the smallest indent used). Lines without
 *   indentation → slide titles, indented lines → bullets.
 *
 * The slides are added by the existing `applyOutline` command: one undo step,
 * shared like any other edit while collaborating.
 */
import type {
  SlideCommand,
  SlideEditResult,
  SlideOutlineEdit,
  SlideOutlineSlide,
} from '@blueoffice/platform';
import { MAX_OUTLINE_LEVEL } from '../outlineModel';
import { unzipEntries } from './unzip';

/** One outline paragraph: level 0 = slide title, 1… = bullet levels. */
export interface OutlineEntry {
  level: number;
  text: string;
}

/** File types the outline import reads. */
export const OUTLINE_FILES = ['.docx', '.docm', '.dotx', '.txt'];

const W_NS = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main';

function child(el: Element, name: string): Element | null {
  for (const c of Array.from(el.children)) if (c.localName === name) return c;
  return null;
}

/** A `w:` attribute (with or without the namespace kept by the parser). */
function attr(el: Element | null, name: string): string | null {
  if (!el) return null;
  return el.getAttributeNS(W_NS, name) ?? el.getAttribute(`w:${name}`) ?? el.getAttribute(name);
}

const val = (el: Element | null) => attr(el, 'val');

/** Heading level 1–9 of `w:outlineLvl` (0–8; 9 = body text), `null` when absent. */
function outlineLevel(pPr: Element | null): number | null | 'body' {
  const v = val(pPr ? child(pPr, 'outlineLvl') : null);
  if (v === null) return null;
  const n = Number(v);
  if (!Number.isInteger(n) || n < 0) return null;
  return n > 8 ? 'body' : n + 1;
}

interface StyleInfo {
  basedOn: string | null;
  level: number | null | 'body';
}

/** Paragraph styles of `styles.xml`: their outline level and parent. */
function readStyles(xml: Document | null): { styles: Map<string, StyleInfo>; byDefault: string } {
  const styles = new Map<string, StyleInfo>();
  let byDefault = '';
  if (!xml) return { styles, byDefault };
  for (const st of Array.from(xml.getElementsByTagName('*'))) {
    if (st.localName !== 'style') continue;
    const type = attr(st, 'type');
    if (type && type !== 'paragraph') continue;
    const id = attr(st, 'styleId');
    if (!id) continue;
    let level = outlineLevel(child(st, 'pPr'));
    if (level === null) {
      // Built-in heading styles are named "heading 1" … "heading 9".
      const m = /^heading ([1-9])$/i.exec((val(child(st, 'name')) ?? '').trim());
      if (m) level = Number(m[1]);
    }
    styles.set(id, { basedOn: val(child(st, 'basedOn')), level });
    const dflt = attr(st, 'default');
    if (dflt === '1' || dflt === 'true') byDefault = id;
  }
  return { styles, byDefault };
}

function styleLevel(styles: Map<string, StyleInfo>, id: string | null): number | null {
  let cur = id;
  for (let depth = 0; cur && depth < 32; depth++) {
    const s = styles.get(cur);
    if (!s) return null;
    if (s.level === 'body') return null;
    if (s.level !== null) return s.level;
    cur = s.basedOn;
  }
  return null;
}

/** Text of a paragraph: runs in order, tabs and line breaks as spaces, deletions left out. */
function paragraphText(p: Element): string {
  let out = '';
  const walk = (el: Element) => {
    for (const c of Array.from(el.children)) {
      switch (c.localName) {
        case 't':
          out += c.textContent ?? '';
          break;
        case 'tab':
        case 'br':
        case 'cr':
          out += ' ';
          break;
        case 'noBreakHyphen':
          out += '-';
          break;
        // Tracked deletions / move sources, properties and nested text boxes are not text.
        case 'del':
        case 'moveFrom':
        case 'pPr':
        case 'rPr':
        case 'txbxContent':
        case 'instrText':
          break;
        default:
          walk(c);
      }
    }
  };
  walk(p);
  return out.replace(/\s+/g, ' ').trim();
}

function parseXml(bytes: Uint8Array | undefined): Document | null {
  if (!bytes) return null;
  const doc = new DOMParser().parseFromString(new TextDecoder().decode(bytes), 'application/xml');
  return doc.getElementsByTagName('parsererror').length ? null : doc;
}

/** Outline of the main document part and styles part (already unzipped XML). */
export function docxOutlineFromXml(
  documentXml: Document,
  stylesXml: Document | null,
): OutlineEntry[] {
  const { styles, byDefault } = readStyles(stylesXml);
  const headings: OutlineEntry[] = [];
  const plain: OutlineEntry[] = [];
  const body = Array.from(documentXml.getElementsByTagName('*')).find(
    (e) => e.localName === 'body',
  );
  if (!body) return [];
  for (const p of Array.from(body.getElementsByTagName('*'))) {
    if (p.localName !== 'p') continue;
    const text = paragraphText(p);
    if (!text) continue;
    const pPr = child(p, 'pPr');
    const direct = outlineLevel(pPr);
    const level =
      direct === 'body'
        ? null
        : (direct ?? styleLevel(styles, val(pPr ? child(pPr, 'pStyle') : null) ?? byDefault));
    if (level !== null) headings.push({ level: level - 1, text });
    plain.push({ level: 0, text });
  }
  return headings.length ? headings : plain;
}

/** Outline of a `.docx` package; `null` when it is not a readable document. */
export async function docxOutline(bytes: Uint8Array): Promise<OutlineEntry[] | null> {
  const parts = await unzipEntries(bytes, ['word/document.xml', 'word/styles.xml']);
  const doc = parseXml(parts.get('word/document.xml'));
  if (!doc) return null;
  return docxOutlineFromXml(doc, parseXml(parts.get('word/styles.xml')));
}

/** Text of a file: UTF-8, or UTF-16 when it starts with a byte order mark. */
export function decodeText(bytes: Uint8Array): string {
  if (bytes[0] === 0xff && bytes[1] === 0xfe) return new TextDecoder('utf-16le').decode(bytes);
  if (bytes[0] === 0xfe && bytes[1] === 0xff) return new TextDecoder('utf-16be').decode(bytes);
  return new TextDecoder('utf-8').decode(bytes);
}

/** Outline of indented text (see the module docs). A leading list mark (`-`, `*`, `•`) is dropped. */
export function textOutline(text: string): OutlineEntry[] {
  const lines = text
    .replace(/^﻿/, '')
    .split(/\r\n|\r|\n/)
    .map((raw) => {
      const indent = /^[\t ]*/.exec(raw)?.[0] ?? '';
      return {
        tabs: indent.split('\t').length - 1,
        spaces: indent.split(' ').length - 1,
        text: raw
          .trim()
          .replace(/^[-*•]\s+/, '')
          .trim(),
      };
    })
    .filter((l) => l.text);
  // One level of spaces = the smallest indentation used.
  const unit = Math.min(...lines.filter((l) => l.spaces > 0).map((l) => l.spaces), Infinity);
  return lines.map((l) => ({
    level: l.tabs + (Number.isFinite(unit) ? Math.floor(l.spaces / unit) : 0),
    text: l.text,
  }));
}

/** Outline of a picked file by its type; `null` when it cannot be read. */
export async function outlineOfFile(
  name: string,
  bytes: Uint8Array,
): Promise<OutlineEntry[] | null> {
  // A ZIP package (local file header signature) is read as a document whatever its name.
  const zip = bytes[0] === 0x50 && bytes[1] === 0x4b;
  if (zip || /\.(docx|docm|dotx|dotm)$/i.test(name)) return docxOutline(bytes);
  return textOutline(decodeText(bytes));
}

/** Title + content slides of an outline: bullets before the first title get an untitled slide. */
export function outlineSlides(entries: OutlineEntry[]): SlideOutlineEdit[] {
  const out: SlideOutlineEdit[] = [];
  for (const e of entries) {
    if (e.level <= 0) {
      out.push({ title: e.text, bullets: [] });
      continue;
    }
    if (out.length === 0) out.push({ title: '', bullets: [] });
    out[out.length - 1]!.bullets.push({
      text: e.text,
      level: Math.min(MAX_OUTLINE_LEVEL, e.level - 1),
    });
  }
  return out;
}

/**
 * The `applyOutline` command adding `slides` after slide `after` of a deck
 * whose outline is `existing` (the existing slides are named unchanged).
 */
export function outlineImportCommand(
  existing: SlideOutlineSlide[],
  after: number,
  slides: SlideOutlineEdit[],
): Extract<SlideCommand, { cmd: 'applyOutline' }> {
  const kept: SlideOutlineEdit[] = existing.map((s, i) => ({
    slide: i,
    title: s.title,
    bullets: s.bullets,
  }));
  const at = Math.max(0, Math.min(existing.length, after + 1));
  return {
    cmd: 'applyOutline',
    slides: [...kept.slice(0, at), ...slides, ...kept.slice(at)],
    current: at,
  };
}

/**
 * Add the slides of an outline after slide `after`: reads the deck's outline,
 * then one `applyOutline`. Resolves the number of slides added (0: nothing to add).
 */
export async function importOutline(
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>,
  entries: OutlineEntry[],
  after: number,
): Promise<number> {
  const slides = outlineSlides(entries);
  if (slides.length === 0) return 0;
  const now = await run({ cmd: 'outline' });
  if (!now || now.error || !now.outline) return 0;
  const r = await run(outlineImportCommand(now.outline, after, slides));
  return r && !r.error ? slides.length : 0;
}
