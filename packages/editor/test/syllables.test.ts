import { describe, expect, it } from 'vitest';
import { HYPHENATION_LIMITS, hyphenPoints, syllables, type SyllableLimits } from '../src/syllables';

/** The word with a hyphen at every point. */
function show(word: string, limits?: SyllableLimits): string {
  const pts = hyphenPoints(word, limits);
  return [...word].map((c, i) => (pts.includes(i) ? `-${c}` : c)).join('');
}

describe('hyphenPoints (ADR-0011 rules, the engine limits)', () => {
  it('divides words as English dictionaries do', () => {
    // The same words as `crates/bo-layout/src/hyphen.rs` (syllable divisions of dictionaries).
    expect(show('information')).toBe('in-for-ma-tion');
    expect(show('establishment')).toBe('es-tab-lish-ment');
    expect(show('important')).toBe('im-por-tant');
    expect(show('wonderful')).toBe('won-der-ful');
    expect(show('children')).toBe('chil-dren');
    expect(show('beautiful')).toBe('beau-ti-ful');
    expect(show('hopeless')).toBe('hope-less');
    expect(show('running')).toBe('run-ning');
    expect(show('question')).toBe('ques-tion');
    expect(show('Hundred')).toBe('Hun-dred');
    expect(show('pumpkin')).toBe('pump-kin');
    expect(show('monster')).toBe('mon-ster');
  });

  it('follows each documented rule', () => {
    // Doubled consonant before -ing splits in the middle, but not ll / ss.
    expect(show('falling')).toBe('fall-ing');
    expect(show('missing')).toBe('miss-ing');
    // A long prefix is split off whole (the prefix itself is not divided).
    expect(show('understand')).toBe('under-stand');
    expect(show('international')).toMatch(/^inter-na-/);
    // x stays with the first syllable; ck too; a digraph goes with the next syllable.
    expect(show('example')).toBe('ex-am-ple');
    expect(show('pockets')).toBe('pock-ets');
    expect(show('mother')).toBe('mo-ther');
    // y after a consonant is a vowel.
    expect(show('hyphen')).toBe('hy-phen');
  });

  it('leaves short words and words that are not plain ASCII letters alone', () => {
    for (const w of ['table', 'nghiêng', 'thuyền', '2026abc', "don't", '']) {
      expect(hyphenPoints(w), w).toEqual([]);
    }
    // Both sides keep enough letters and a vowel.
    for (const p of hyphenPoints('rhythm')) {
      expect(p).toBeGreaterThanOrEqual(HYPHENATION_LIMITS.leftMin);
      expect(6 - p).toBeGreaterThanOrEqual(HYPHENATION_LIMITS.rightMin);
    }
    expect(show('through')).toBe('through');
  });
});

describe('syllables (the reader limits)', () => {
  it('splits shorter words too, with consonant + le as a syllable', () => {
    expect(syllables('table')).toEqual(['ta', 'ble']);
    expect(syllables('water')).toEqual(['wa', 'ter']);
    expect(syllables('people')).toEqual(['peo', 'ple']);
    expect(syllables('little')).toEqual(['lit', 'tle']);
    expect(syllables('pickle')).toEqual(['pick', 'le']);
    expect(syllables('simple')).toEqual(['sim', 'ple']);
    expect(syllables('reading')).toEqual(['read', 'ing']);
    expect(syllables('information')).toEqual(['in', 'for', 'ma', 'tion']);
  });

  it('keeps one-syllable words whole', () => {
    for (const w of ['the', 'house', 'there', 'time', 'cat', 'strength']) {
      expect(syllables(w), w).toEqual([w]);
    }
  });

  it('keeps punctuation and capitals with their syllable', () => {
    expect(syllables('Hello,')).toEqual(['Hel', 'lo,']);
    expect(syllables('“Wonderful!”')).toEqual(['“Won', 'der', 'ful!”']);
    expect(syllables('(table)').join('')).toBe('(table)');
  });

  it('never splits Vietnamese syllables, numbers or mixed tokens', () => {
    // A written Vietnamese syllable is already one unit.
    for (const w of ['nghiêng', 'Việt', 'thuyền,', 'trường', '2026', 'ab-cdef', 'e-mail', '…']) {
      expect(syllables(w), w).toEqual([w]);
    }
  });
});
