/**
 * KeyTips (CORE-UI-004 / UI-KEYTIPS-001): letters shown on the ribbon after pressing and
 * releasing Alt. Codes are derived from the visible labels (so they follow the UI language),
 * unique and prefix-free inside one scope (the tab strip, or one tab's commands), and stable:
 * the same labels in the same order always get the same codes, and entries added at the end
 * (contextual tabs, a command that appears) never change the codes before them while the scope
 * stays within 26 entries (larger scopes keep Y and Z as prefixes of two-letter codes).
 */
import { normalize } from './searchRank';

export interface KeyTipEntry {
  id: string;
  label: string;
}

/**
 * Fixed codes by id (commands, `tab:<id>` for tabs, `file` for the File button). Kept small:
 * habits that do not depend on the label language.
 */
export const KEYTIP_OVERRIDES: Readonly<Record<string, string>> = {
  file: 'F',
  'format.bold': '1',
  'format.italic': '2',
  'format.underline': '3',
};

const ALNUM = /[A-Z0-9]/;

/** Upper-case ASCII letters / digits of a label (diacritics dropped: "Định dạng" → DINHDANG). */
function letters(label: string): string[] {
  return [...normalize(label).toUpperCase()].filter((c) => ALNUM.test(c));
}

/** Candidate single letters: word initials first, then the other letters, in order. */
function candidates(label: string): string[] {
  const words = normalize(label)
    .toUpperCase()
    .split(/[^A-Z0-9]+/)
    .filter(Boolean);
  const out: string[] = [];
  const add = (c: string | undefined) => {
    if (c && ALNUM.test(c) && !out.includes(c)) out.push(c);
  };
  words.forEach((w) => add(w[0]));
  letters(label).forEach(add);
  return out;
}

const ALPHABET = [...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'];
const DIGITS = [...'123456789'];

/**
 * Assign a code to every entry. `overrides` (by id) win when they do not clash; `reserved`
 * codes are never handed out (and their letters cannot start a two-letter code).
 */
export function assignKeyTips(
  entries: readonly KeyTipEntry[],
  overrides: Readonly<Record<string, string>> = KEYTIP_OVERRIDES,
  reserved: readonly string[] = [],
): Map<string, string> {
  const out = new Map<string, string>();
  const used = new Set<string>(reserved.map((r) => r.toUpperCase()));
  const singles = new Set<string>([...used].filter((c) => c.length === 1));
  // First letters of two-letter codes; big scopes keep Y / Z for the overflow.
  const prefixes = new Set<string>(entries.length > 26 ? ['Y', 'Z'] : []);
  const free = (code: string) =>
    !used.has(code) &&
    (code.length === 1 ? !prefixes.has(code) : !singles.has(code[0]!)) &&
    ![...used].some((u) => u.length > code.length && u.startsWith(code));
  const take = (id: string, code: string) => {
    out.set(id, code);
    used.add(code);
    if (code.length === 1) singles.add(code);
    else prefixes.add(code[0]!);
  };
  // 1. Overrides (only the ids present).
  for (const e of entries) {
    const o = overrides[e.id]?.toUpperCase();
    if (o && free(o)) take(e.id, o);
  }
  // 2. One letter from the label where possible.
  const rest: KeyTipEntry[] = [];
  for (const e of entries) {
    if (out.has(e.id)) continue;
    const c = candidates(e.label).find((x) => free(x));
    if (c) take(e.id, c);
    else rest.push(e);
  }
  // 3. Two characters: a letter of the label that is not a single code, then another one.
  for (const e of rest) {
    const own = candidates(e.label);
    const firsts = [...own, ...ALPHABET].filter((c, i, a) => a.indexOf(c) === i);
    const seconds = [...letters(e.label), ...ALPHABET, ...DIGITS].filter(
      (c, i, a) => a.indexOf(c) === i,
    );
    let code: string | undefined;
    for (const a of firsts) {
      if (singles.has(a)) continue;
      code = seconds.map((b) => a + b).find((x) => free(x));
      if (code) break;
    }
    // Every other letter is a single code (> 26 entries): Y / Z followed by a digit or letter.
    code ??= ['Y', 'Z']
      .flatMap((a) => [...DIGITS, ...ALPHABET].map((b) => a + b))
      .find((x) => !used.has(x));
    if (code) take(e.id, code);
  }
  return out;
}

/** Codes that start with `typed` (the candidates still reachable). */
export function keyTipMatches(codes: ReadonlyMap<string, string>, typed: string): string[] {
  const t = typed.toUpperCase();
  return [...codes].filter(([, c]) => c.startsWith(t)).map(([id]) => id);
}

/** The id whose code is exactly `typed`. */
export function keyTipHit(codes: ReadonlyMap<string, string>, typed: string): string | undefined {
  const t = typed.toUpperCase();
  for (const [id, c] of codes) if (c === t) return id;
  return undefined;
}
