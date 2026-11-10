/**
 * Syllables of words for the immersive reader (CORE-LANG-004).
 *
 * English words are split by BlueOffice's own rules (ADR-0011) — the same rules as automatic
 * hyphenation in `crates/bo-layout/src/hyphen.rs`, ported here because the engine does not
 * expose them: common suffixes and a few long prefixes are split off, then the stem is divided
 * between its vowel groups (one consonant goes with the next syllable, two are split unless
 * they sound as one, a longer cluster keeps a valid onset with the next syllable). With the
 * default limits `hyphenPoints` gives exactly the engine's hyphenation points; the reader uses
 * looser limits (`SYLLABLE_LIMITS`) because it shows syllables instead of breaking lines.
 *
 * Vietnamese is written one syllable per space-separated unit, so a Vietnamese word (any word
 * with a letter outside ASCII) is already a single syllable and is never split.
 */

export interface SyllableLimits {
  /** Shorter words are not split. */
  minWord: number;
  /** Letters kept in the first part at least. */
  leftMin: number;
  /** Letters kept in the last part at least. */
  rightMin: number;
  /**
   * A final consonant + "le" is a syllable of its own ("ta·ble", "peo·ple", "pick·le"). The
   * engine's hyphenation does not apply this, so it is off in `HYPHENATION_LIMITS`.
   */
  finalLe?: boolean;
}

/** Limits of automatic hyphenation (`hyphen.rs`: `MIN_WORD`, `LEFT_MIN`, `RIGHT_MIN`). */
export const HYPHENATION_LIMITS: SyllableLimits = { minWord: 6, leftMin: 2, rightMin: 3 };

/** Limits of the reader's syllable marks: short words too ("ta·ble", "wa·ter"). */
export const SYLLABLE_LIMITS: SyllableLimits = {
  minWord: 4,
  leftMin: 2,
  rightMin: 2,
  finalLe: true,
};

/** Suffixes split off before dividing the stem, longest first. */
const SUFFIXES = [
  'ments',
  'ment',
  'nesses',
  'ness',
  'lessly',
  'less',
  'ships',
  'ship',
  'tions',
  'tion',
  'sions',
  'sion',
  'able',
  'ible',
  'fully',
  'ful',
  'ings',
  'ing',
];

/** Prefixes split off when the rest is long enough. */
const PREFIXES = ['counter', 'inter', 'multi', 'over', 'semi', 'super', 'trans', 'under'];

/** Consonant pairs that sound as one consonant and are never split. */
const DIGRAPHS = new Set(['ch', 'ck', 'gh', 'ph', 'sh', 'th', 'wh']);

/** Consonant pairs that may start a syllable. */
const ONSETS = new Set([
  'bl',
  'br',
  'ch',
  'cl',
  'cr',
  'dr',
  'fl',
  'fr',
  'gl',
  'gr',
  'ph',
  'pl',
  'pr',
  'sc',
  'sh',
  'sk',
  'sl',
  'sm',
  'sn',
  'sp',
  'st',
  'sw',
  'th',
  'tr',
  'tw',
  'wh',
  'wr',
]);

/** Consonant triples that may start a syllable. */
const ONSETS3 = new Set(['chr', 'sch', 'scr', 'shr', 'spl', 'spr', 'str', 'thr']);

function isVowel(w: string, i: number): boolean {
  switch (w[i]) {
    case 'a':
    case 'e':
    case 'i':
    case 'o':
      return true;
    // "qu": the u is part of the consonant.
    case 'u':
      return !(i > 0 && w[i - 1] === 'q');
    // y is a vowel after a consonant ("hyphen"), a consonant at the start ("yellow").
    case 'y':
      return i > 0 && !isVowel(w, i - 1);
    default:
      return false;
  }
}

/**
 * Offsets (ascending) where `word` divides into syllables; empty for short words and words
 * that are not plain ASCII letters.
 */
export function hyphenPoints(word: string, limits: SyllableLimits = HYPHENATION_LIMITS): number[] {
  const n = word.length;
  if (n < limits.minWord || !/^[A-Za-z]+$/.test(word)) return [];
  const w = word.toLowerCase();
  const out: number[] = [];
  // 1. Suffix: break before it (between doubled consonants before "-ing": run-ning, but
  //    fall-ing / miss-ing).
  let end = n;
  for (const suf of SUFFIXES) {
    const s = suf.length;
    if (n >= s + 3 && w.endsWith(suf)) {
      let at = n - s;
      if (
        suf.startsWith('ing') &&
        at >= 2 &&
        w[at - 1] === w[at - 2] &&
        !isVowel(w, at - 1) &&
        !'lsfz'.includes(w[at - 1] ?? '')
      ) {
        at -= 1;
      }
      out.push(at);
      end = at;
      break;
    }
  }
  // 2. Prefix.
  let start = 0;
  for (const pre of PREFIXES) {
    const p = pre.length;
    if (end >= p + 4 && w.startsWith(pre)) {
      out.push(p);
      start = p;
      break;
    }
  }
  // 3. Syllables of the stem w[start..end].
  const stem = w.slice(start, end);
  const m = stem.length;
  // Vowel groups [start, end) of the stem; a final silent "e" is not one ("hope-less"), except
  // in a final consonant + "le" syllable ("ta-ble").
  const groups: [number, number][] = [];
  for (let i = 0; i < m;) {
    if (isVowel(stem, i)) {
      const g0 = i;
      while (i < m && isVowel(stem, i)) i++;
      groups.push([g0, i]);
    } else {
      i++;
    }
  }
  const le = m >= 3 && stem.endsWith('le') && !isVowel(stem, m - 3);
  const last = groups[groups.length - 1];
  if (last && last[1] === m && last[1] - last[0] === 1 && stem[last[0]] === 'e' && !le) {
    if (groups.length > 1) groups.pop();
  }
  for (let g = 0; g + 1 < groups.length; g++) {
    const a = groups[g]![1];
    const b = groups[g + 1]![0];
    const c = stem.slice(a, b);
    let at: number;
    if (c.length === 0) continue;
    if (limits.finalLe && le && b === m - 1) {
      at = stem.slice(m - 4, m - 2) === 'ck' ? m - 2 : m - 3;
    } else if (c.length === 1) {
      // V-CV, but x stays with the first syllable (ex-am-ple).
      at = c === 'x' ? b : a;
    } else if (c.length === 2) {
      at = c === 'ck' ? b : DIGRAPHS.has(c) ? a : a + 1;
    } else if (c.length >= 4 && ONSETS3.has(c.slice(-3))) {
      at = b - 3;
    } else if (ONSETS.has(c.slice(-2))) {
      at = b - 2;
    } else {
      at = b - 1;
    }
    out.push(start + at);
  }
  // Both parts keep a vowel and enough letters.
  const hasVowel = (from: number, to: number) => {
    for (let i = from; i < to; i++) if (isVowel(w, i)) return true;
    return false;
  };
  return [...new Set(out)]
    .sort((x, y) => x - y)
    .filter(
      (p) => p >= limits.leftMin && n - p >= limits.rightMin && hasVowel(0, p) && hasVowel(p, n),
    );
}

/**
 * The syllables of one written word (no white space), in order; their concatenation is the
 * word. Punctuation around the letters stays with the first / last syllable; a word with
 * digits, inner punctuation or letters outside ASCII (Vietnamese) is one part.
 */
export function syllables(word: string, limits: SyllableLimits = SYLLABLE_LIMITS): string[] {
  const m = /^([^\p{L}\p{N}]*)([\p{L}\p{N}].*?)([^\p{L}\p{N}]*)$/su.exec(word);
  if (!m) return [word];
  const [, lead = '', core = '', tail = ''] = m;
  const points = hyphenPoints(core, limits);
  if (!points.length) return [word];
  const parts: string[] = [];
  let from = 0;
  for (const p of points) {
    parts.push(core.slice(from, p));
    from = p;
  }
  parts.push(core.slice(from));
  parts[0] = lead + parts[0]!;
  parts[parts.length - 1] += tail;
  return parts;
}
