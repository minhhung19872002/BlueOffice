import { readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync, gzipSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import {
  LazyThesaurus,
  THESAURUS_POS,
  ThesaurusIndex,
  assetThesaurus,
  decodeThesaurus,
  englishBaseForms,
  matchCase,
  parseThesaurus,
  sensesByPos,
  topSynonyms,
  wordAtCaret,
  type ThesaurusData,
} from '../src/thesaurus';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const DATA = join(root, 'assets', 'thesaurus', 'en', 'en.json.gz');
const bytes = new Uint8Array(readFileSync(DATA));
const real: ThesaurusData = parseThesaurus(JSON.parse(gunzipSync(bytes).toString('utf8')));
const en = new ThesaurusIndex(real);

/** A small hand-made dataset (independent of the real one). */
const tiny: ThesaurusData = {
  format: 'bo-thesaurus/1',
  lang: 'en',
  source: 'test',
  version: '1',
  license: 'CC0-1.0',
  words: ['glad', 'joyful', 'unhappy', 'sprint', 'dash', 'large', 'city', 'town', 'rodent'],
  synsets: [
    ['a', 'feeling joy', [0, 1]],
    ['v', 'move fast on foot', [3, 4]],
    ['a', 'of great size', [5]],
    ['n', 'large town', [6, 7]],
    ['n', 'small rodent', [8]],
  ],
  entries: {
    happy: [[0, [2]]],
    run: [[1]],
    big: [[2]],
    city: [[3]],
    mouse: [[4]],
    stop: [[1]],
    make: [[1]],
    lie: [[1]],
    try: [[1]],
  },
  forms: { mice: 'mouse', ran: 'run' },
};
const small = new ThesaurusIndex(tiny);

describe('thesaurus data (assets/thesaurus/en)', () => {
  it('is a gzip file under 4 MB with the attribution next to it', () => {
    expect(statSync(DATA).size).toBeLessThan(4 * 1024 * 1024);
    expect(bytes[0]).toBe(0x1f);
    expect(bytes[1]).toBe(0x8b);
    const readme = readFileSync(join(dirname(DATA), 'README.md'), 'utf8');
    expect(readme).toContain('Open English WordNet');
    expect(readme).toContain('2025');
    expect(readme).toContain('CC BY 4.0');
    expect(readme).toContain('https://creativecommons.org/licenses/by/4.0/');
    expect(readme).toContain('scripts/build-thesaurus.mjs');
    expect(readme).toContain('Princeton University');
    const notices = readFileSync(join(root, 'THIRD_PARTY_NOTICES.md'), 'utf8');
    expect(notices).toContain('assets/thesaurus/en/en.json.gz');
    expect(notices).toContain('CC BY 4.0');
  });

  it('has the format, source and licence of its README', () => {
    expect(real.format).toBe('bo-thesaurus/1');
    expect(real.lang).toBe('en');
    expect(real.source).toBe('Open English Wordnet');
    expect(real.version).toBe('2025');
    expect(real.license).toBe('CC-BY-4.0');
    expect(real.licenseUrl).toBe('https://creativecommons.org/licenses/by/4.0');
  });

  it('is consistent: indexes in range, lower-case single-word keys, known parts of speech', () => {
    const nWords = real.words.length;
    const nSyn = real.synsets.length;
    for (const [pos, gloss, members] of real.synsets) {
      expect(THESAURUS_POS).toContain(pos);
      expect(typeof gloss).toBe('string');
      expect(gloss.length).toBeLessThanOrEqual(81);
      for (const i of members) expect(i >= 0 && i < nWords).toBe(true);
    }
    for (const w of real.words) expect(w).toMatch(/^[a-z][a-z' .-]*$/);
    const keys = Object.keys(real.entries);
    expect(keys.length).toBeGreaterThan(40_000);
    for (const k of keys) {
      expect(k).toMatch(/^[a-z][a-z'-]*$/);
      const senses = real.entries[k] ?? [];
      expect(senses.length).toBeGreaterThan(0);
      for (const s of senses) {
        expect(s[0] >= 0 && s[0] < nSyn).toBe(true);
        for (const a of s[1] ?? []) expect(a >= 0 && a < nWords).toBe(true);
      }
    }
    for (const [f, l] of Object.entries(real.forms))
      for (const lemma of l.split(' ')) {
        expect(f).not.toBe(lemma);
        expect(Object.hasOwn(real.entries, lemma)).toBe(true);
      }
  });

  it('answers well-known WordNet synonyms and antonyms', () => {
    const happy = en.lookup('happy');
    expect(happy?.senses[0]?.pos).toBe('a');
    expect(happy?.senses[0]?.antonyms).toContain('unhappy');
    const big = en.lookup('big');
    expect(big?.senses.some((s) => s.synonyms.includes('large'))).toBe(true);
    expect(big?.senses.some((s) => s.antonyms.includes('little'))).toBe(true);
    // The headword itself is not its own synonym.
    for (const s of big?.senses ?? []) expect(s.synonyms).not.toContain('big');
    // Object keys of the prototype are not headwords.
    expect(en.lookup('__proto__')).toBeNull();
    expect(en.lookup('hasOwnProperty')).toBeNull();
    expect(en.lookup('qwzxv')).toBeNull();
  });

  it('finds the lemma of inflected words in the real data', () => {
    const lemma = (w: string) => en.lemmasOf(w);
    expect(lemma('cities')).toContain('city');
    expect(lemma('walked')).toContain('walk');
    expect(lemma('stopped')).toContain('stop');
    expect(lemma('running')).toContain('run');
    expect(lemma('making')).toContain('make');
    expect(lemma('faster')).toContain('fast');
    expect(lemma('happier')).toEqual(['happy']);
    expect(lemma('bigger')).toContain('big');
    expect(lemma('larger')).toContain('large');
    expect(lemma('tried')).toContain('try');
    expect(lemma('mice')).toEqual(['mouse']);
    expect(lemma('went')).toContain('go');
    expect(lemma('Houses')).toContain('house');
  });
});

describe('englishBaseForms', () => {
  it('derives base forms by suffix rules, the word first', () => {
    expect(englishBaseForms('cities')).toEqual(['cities', 'city', 'citie']);
    expect(englishBaseForms('boxes')).toContain('box');
    expect(englishBaseForms('stopped').slice(0, 2)).toEqual(['stopped', 'stop']);
    expect(englishBaseForms('baked')).toContain('bake');
    expect(englishBaseForms('running')).toContain('run');
    expect(englishBaseForms('lying')).toContain('lie');
    expect(englishBaseForms('happiest')).toContain('happy');
    expect(englishBaseForms('bigger')).toContain('big');
    expect(englishBaseForms('class')).toEqual(['class']);
    expect(englishBaseForms('go')).toEqual(['go']);
  });
});

describe('ThesaurusIndex', () => {
  it('looks a word up through irregular forms and suffix rules', () => {
    expect(small.lemmasOf('mice')).toEqual(['mouse']);
    expect(small.lemmasOf('ran')).toEqual(['run']);
    expect(small.lemmasOf('runs')).toEqual(['run']);
    expect(small.lemmasOf('running')).toEqual(['run']);
    expect(small.lemmasOf('stopped')).toEqual(['stop']);
    expect(small.lemmasOf('making')).toEqual(['make']);
    expect(small.lemmasOf('lying')).toEqual(['lie']);
    expect(small.lemmasOf('tries')).toEqual(['try']);
    expect(small.lemmasOf('cities')).toEqual(['city']);
    expect(small.lemmasOf('bigger')).toEqual(['big']);
    expect(small.lemmasOf('happiest')).toEqual(['happy']);
    expect(small.lemmasOf('HAPPY')).toEqual(['happy']);
    expect(small.lemmasOf('sad')).toEqual([]);
  });

  it('returns senses with synonyms, antonyms, groups and top synonyms', () => {
    const r = small.lookup('Happy');
    expect(r).toEqual({
      word: 'Happy',
      lang: 'en',
      lemmas: ['happy'],
      senses: [
        {
          lemma: 'happy',
          pos: 'a',
          gloss: 'feeling joy',
          synonyms: ['glad', 'joyful'],
          antonyms: ['unhappy'],
        },
      ],
    });
    expect(topSynonyms(r, 1)).toEqual(['glad']);
    expect(topSynonyms(null)).toEqual([]);
    const city = small.lookup('cities');
    expect(sensesByPos(city).map((g) => g.pos)).toEqual(['n']);
    // The asked word is not offered as its own synonym.
    expect(city?.senses[0]?.synonyms).toEqual(['town']);
  });
});

describe('LazyThesaurus', () => {
  it('loads each language once, on the first lookup, from gzip or plain JSON', async () => {
    let calls = 0;
    const t = new LazyThesaurus({
      en: async () => {
        calls++;
        return new Uint8Array(gzipSync(JSON.stringify(tiny)));
      },
    });
    expect(t.languages()).toEqual(['en']);
    expect(t.loaded('en-US')).toBe(false);
    expect(calls).toBe(0);
    const [a, b] = await Promise.all([t.lookup('happy', 'en-US'), t.lookup('run')]);
    expect(calls).toBe(1);
    expect(a?.senses[0]?.synonyms).toEqual(['glad', 'joyful']);
    expect(b?.lemmas).toEqual(['run']);
    expect(t.loaded()).toBe(true);
    expect(await t.lookup('nhà', 'vi-VN')).toBeNull();
    const plain = await decodeThesaurus(new TextEncoder().encode(JSON.stringify(tiny)));
    expect(plain.entries.happy).toEqual([[0, [2]]]);
  });

  it('retries a failed load and rejects data of another format', async () => {
    let fail = true;
    const t = new LazyThesaurus({
      en: async () => {
        if (fail) throw new Error('offline');
        return new TextEncoder().encode(JSON.stringify(tiny));
      },
    });
    await expect(t.lookup('happy')).rejects.toThrow('offline');
    fail = false;
    expect((await t.lookup('happy'))?.lemmas).toEqual(['happy']);
    await expect(
      decodeThesaurus(new TextEncoder().encode(JSON.stringify({ format: 'other' }))),
    ).rejects.toThrow('format');
  });

  it('fetches thesaurus/<lang>/<lang>.json.gz from the base URL', async () => {
    const urls: string[] = [];
    const t = assetThesaurus('/app', ['en'], async (u) => {
      urls.push(u);
      return new Response(gzipSync(JSON.stringify(tiny)));
    });
    expect((await t.lookup('mice'))?.lemmas).toEqual(['mouse']);
    expect(urls).toEqual(['/app/thesaurus/en/en.json.gz']);
  });
});

describe('text helpers', () => {
  it('finds the word at a caret', () => {
    const s = "The quick-witted fox's den, «happy»";
    expect(wordAtCaret(s, 0)).toEqual({ from: 0, to: 3, word: 'The' });
    expect(wordAtCaret(s, 3)?.word).toBe('The');
    expect(wordAtCaret(s, 6)?.word).toBe('quick-witted');
    expect(wordAtCaret(s, 19)?.word).toBe("fox's");
    expect(wordAtCaret(s, 31)?.word).toBe('happy');
    expect(wordAtCaret('  ', 1)).toBeNull();
    expect(wordAtCaret('a 123 b', 3)).toBeNull();
    expect(wordAtCaret("'tis", 0)?.word).toBe('tis');
  });

  it('keeps the capitalisation of the replaced word', () => {
    expect(matchCase('happy', 'glad')).toBe('glad');
    expect(matchCase('Happy', 'glad')).toBe('Glad');
    expect(matchCase('HAPPY', 'glad')).toBe('GLAD');
    expect(matchCase('Happy', 'not bad')).toBe('Not bad');
    expect(matchCase('A', 'one')).toBe('One');
    expect(matchCase('iPhone', 'phone')).toBe('phone');
  });
});
