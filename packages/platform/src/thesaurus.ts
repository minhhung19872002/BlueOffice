/**
 * Thesaurus (CORE-THES-001): synonyms and antonyms of a word, grouped in senses by part of
 * speech. Languages are pluggable like the spelling dictionaries: a {@link ThesaurusProvider}
 * knows which languages it has and loads each one's data lazily on the first lookup (web: a
 * fetched asset `thesaurus/<lang>/<lang>.json.gz`; desktop: the same file bundled with the app's
 * front end). The English data is derived from Open English WordNet (CC BY 4.0,
 * `assets/thesaurus/en/README.md`); there is no Vietnamese data yet.
 *
 * English lookups handle inflections: the word itself, its irregular forms from the data
 * (`mice` → `mouse`), then simple suffix rules (plural and third person `-s`/`-es`/`-ies`,
 * `-ed`/`-ied`, `-ing`, comparative and superlative `-er`/`-est`, doubled consonants and a
 * dropped `e`), keeping the candidates that are headwords of the data.
 */

/** Part of speech of a sense (noun, verb, adjective, adverb). */
export type ThesaurusPos = 'n' | 'v' | 'a' | 'r';

export const THESAURUS_POS: readonly ThesaurusPos[] = ['n', 'v', 'a', 'r'];

/** One meaning of a headword. */
export interface ThesaurusSense {
  /** The headword this sense belongs to (the lemma looked up). */
  lemma: string;
  pos: ThesaurusPos;
  /** Short definition. */
  gloss: string;
  synonyms: string[];
  antonyms: string[];
}

/** Answer of a lookup. */
export interface ThesaurusResult {
  /** The word asked for. */
  word: string;
  lang: string;
  /** Headwords found for it (the word itself and / or its base forms), in order. */
  lemmas: string[];
  senses: ThesaurusSense[];
}

/** Thesaurus of one or more languages. */
export interface ThesaurusProvider {
  /** Primary language subtags with data (`en`). */
  languages(): readonly string[];
  /** Looks `word` up (`lang`: BCP 47, its primary subtag counts; default the first language). */
  lookup(word: string, lang?: string): Promise<ThesaurusResult | null>;
  /** The data of `lang` is loaded (lookups answer at once). */
  loaded(lang?: string): boolean;
}

// ------------------------------------------------------------------------------- data format

/** The derived dataset (`scripts/build-thesaurus.mjs`, format `bo-thesaurus/1`). */
export interface ThesaurusData {
  format: 'bo-thesaurus/1';
  lang: string;
  source: string;
  version: string;
  license: string;
  licenseUrl?: string;
  url?: string;
  /** Synonym and antonym strings. */
  words: string[];
  /** `[pos, gloss, word indexes]`. */
  synsets: [ThesaurusPos, string, number[]][];
  /** Headword → senses `[synset]` or `[synset, antonym word indexes]`. */
  entries: Record<string, ([number] | [number, number[]])[]>;
  /** Irregular form → its lemma(s), space separated. */
  forms: Record<string, string>;
}

export class ThesaurusDataError extends Error {
  constructor(message: string) {
    super(`thesaurus data: ${message}`);
    this.name = 'ThesaurusDataError';
  }
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

/** Checks the shape of parsed thesaurus JSON (not every entry: the data is trusted once built). */
export function parseThesaurus(v: unknown): ThesaurusData {
  if (!isObject(v)) throw new ThesaurusDataError('not an object');
  if (v.format !== 'bo-thesaurus/1') throw new ThesaurusDataError(`format ${String(v.format)}`);
  if (typeof v.lang !== 'string' || !v.lang) throw new ThesaurusDataError('no language');
  if (!Array.isArray(v.words) || !Array.isArray(v.synsets))
    throw new ThesaurusDataError('no words / synsets');
  if (!isObject(v.entries) || !isObject(v.forms)) throw new ThesaurusDataError('no entries');
  return v as unknown as ThesaurusData;
}

const GZIP = (b: Uint8Array) => b.length > 2 && b[0] === 0x1f && b[1] === 0x8b;

/**
 * Decodes the bytes of a data file: gzip (`.json.gz`; a server that already removed the gzip
 * layer with `Content-Encoding` gives plain JSON, which is accepted too).
 */
export async function decodeThesaurus(bytes: Uint8Array): Promise<ThesaurusData> {
  let json = bytes;
  if (GZIP(bytes)) {
    const ds = new DecompressionStream('gzip');
    const out = new Blob([bytes as BlobPart]).stream().pipeThrough(ds);
    json = new Uint8Array(await new Response(out).arrayBuffer());
  }
  return parseThesaurus(JSON.parse(new TextDecoder().decode(json)));
}

// ------------------------------------------------------------------------------- lemmatisation

const VOWEL = /[aeiou]/;
const own = (o: object, k: string) => Object.prototype.hasOwnProperty.call(o, k);

/**
 * Base-form candidates of an English word by suffix rules (the word itself first), most
 * likely first; callers keep those that are headwords. Lower-case input.
 */
export function englishBaseForms(word: string): string[] {
  const w = word.toLowerCase();
  const out: string[] = [w];
  const add = (s: string) => {
    if (s.length >= 2 && !out.includes(s)) out.push(s);
  };
  /** `stem` and, when it ends in a doubled consonant, the stem with one (`stopp` → `stop`). */
  const undouble = (stem: string) => {
    const n = stem.length;
    if (
      n >= 3 &&
      stem[n - 1] === stem[n - 2] &&
      !VOWEL.test(stem[n - 1] ?? '') &&
      !/[lsz]/.test(stem[n - 1] ?? '')
    )
      add(stem.slice(0, -1));
  };
  const n = w.length;
  if (n < 3) return out;
  // Plurals and third person singular.
  if (w.endsWith('ies') && n > 4) add(`${w.slice(0, -3)}y`);
  if (w.endsWith('es') && /(s|x|z|ch|sh|o)es$/.test(w)) add(w.slice(0, -2));
  if (w.endsWith('s') && !w.endsWith('ss') && !w.endsWith('us') && !w.endsWith('is'))
    add(w.slice(0, -1));
  // Past tense and participle.
  if (w.endsWith('ied') && n > 4) add(`${w.slice(0, -3)}y`);
  if (w.endsWith('ed') && n > 3) {
    const stem = w.slice(0, -2);
    undouble(stem);
    add(stem);
    add(`${stem}e`);
  }
  // Present participle.
  if (w.endsWith('ying') && n >= 5) add(`${w.slice(0, -4)}ie`);
  if (w.endsWith('ing') && n > 4) {
    const stem = w.slice(0, -3);
    undouble(stem);
    add(stem);
    add(`${stem}e`);
  }
  // Comparative and superlative.
  for (const suf of ['est', 'er'] as const) {
    if (!w.endsWith(suf) || n <= suf.length + 2) continue;
    const stem = w.slice(0, -suf.length);
    if (stem.endsWith('i')) add(`${stem.slice(0, -1)}y`);
    undouble(stem);
    add(stem);
    add(`${stem}e`);
  }
  return out;
}

// ------------------------------------------------------------------------------- index

/** Lookups in one language's data (synchronous once loaded). */
export class ThesaurusIndex {
  constructor(readonly data: ThesaurusData) {}

  get lang(): string {
    return this.data.lang;
  }

  /** Is `w` (lower case) a headword? */
  has(w: string): boolean {
    return own(this.data.entries, w);
  }

  /**
   * Headwords for `word`: the word itself, its irregular base forms (then no suffix rules: an
   * irregular form is not also regular), else the suffix-rule base forms that are headwords.
   */
  lemmasOf(word: string): string[] {
    const w = word.trim().toLowerCase().replace(/’/g, "'");
    if (!w) return [];
    const out: string[] = [];
    if (this.has(w)) out.push(w);
    const irregular = own(this.data.forms, w) ? (this.data.forms[w] ?? '').split(' ') : [];
    for (const l of irregular) if (l && this.has(l) && !out.includes(l)) out.push(l);
    if (!irregular.length && this.data.lang === 'en')
      for (const l of englishBaseForms(w)) if (this.has(l) && !out.includes(l)) out.push(l);
    return out;
  }

  lookup(word: string): ThesaurusResult | null {
    const lemmas = this.lemmasOf(word);
    if (!lemmas.length) return null;
    const { words, synsets, entries } = this.data;
    const senses: ThesaurusSense[] = [];
    for (const lemma of lemmas) {
      for (const sense of entries[lemma] ?? []) {
        const syn = synsets[sense[0]];
        if (!syn) continue;
        const [pos, gloss, members] = syn;
        const synonyms = members
          .map((i) => words[i] ?? '')
          .filter((s) => s && s !== lemma && s !== word.toLowerCase());
        const antonyms = (sense[1] ?? []).map((i) => words[i] ?? '').filter(Boolean);
        senses.push({ lemma, pos, gloss, synonyms, antonyms });
      }
    }
    return { word, lang: this.data.lang, lemmas, senses };
  }
}

/** The first `n` distinct synonyms of a result, in sense order (context menus). */
export function topSynonyms(r: ThesaurusResult | null | undefined, n = 8): string[] {
  const out: string[] = [];
  for (const s of r?.senses ?? [])
    for (const w of s.synonyms) {
      if (out.length >= n) return out;
      if (!out.includes(w)) out.push(w);
    }
  return out;
}

/** Senses grouped by part of speech (noun, verb, adjective, adverb order). */
export function sensesByPos(
  r: ThesaurusResult | null | undefined,
): { pos: ThesaurusPos; senses: ThesaurusSense[] }[] {
  return THESAURUS_POS.map((pos) => ({
    pos,
    senses: (r?.senses ?? []).filter((s) => s.pos === pos),
  })).filter((g) => g.senses.length > 0);
}

// ------------------------------------------------------------------------------- providers

/** Loads one language's data file (bytes, gzip or plain JSON). */
export type ThesaurusLoader = () => Promise<Uint8Array>;

const primary = (lang: string | undefined) => (lang ?? '').toLowerCase().split(/[-_]/)[0] ?? '';

/** A provider over lazily loaded data files, one loader per language. */
export class LazyThesaurus implements ThesaurusProvider {
  private readonly indexes = new Map<string, ThesaurusIndex>();
  private readonly pending = new Map<string, Promise<ThesaurusIndex>>();

  constructor(private readonly loaders: Record<string, ThesaurusLoader>) {}

  languages(): readonly string[] {
    return Object.keys(this.loaders);
  }

  private langOf(lang?: string): string | null {
    const p = primary(lang);
    if (p && own(this.loaders, p)) return p;
    return lang ? null : (this.languages()[0] ?? null);
  }

  loaded(lang?: string): boolean {
    const l = this.langOf(lang);
    return !!l && this.indexes.has(l);
  }

  /** The index of `lang`, loading it once (a failed load is retried by the next lookup). */
  index(lang?: string): Promise<ThesaurusIndex | null> {
    const l = this.langOf(lang);
    if (!l) return Promise.resolve(null);
    const done = this.indexes.get(l);
    if (done) return Promise.resolve(done);
    let p = this.pending.get(l);
    if (!p) {
      const loader = this.loaders[l];
      if (!loader) return Promise.resolve(null);
      p = loader()
        .then(decodeThesaurus)
        .then((d) => {
          const ix = new ThesaurusIndex(d);
          this.indexes.set(l, ix);
          return ix;
        })
        .finally(() => this.pending.delete(l));
      this.pending.set(l, p);
    }
    return p;
  }

  async lookup(word: string, lang?: string): Promise<ThesaurusResult | null> {
    const ix = await this.index(lang);
    return ix ? ix.lookup(word) : null;
  }
}

/** Fetches `thesaurus/<lang>/<lang>.json.gz` from `baseUrl` (web asset, desktop bundled file). */
export function assetThesaurus(
  baseUrl: string,
  langs: readonly string[] = ['en'],
  fetchImpl: (url: string) => Promise<Response> = (u) => fetch(u),
): LazyThesaurus {
  const base = baseUrl.endsWith('/') ? baseUrl : `${baseUrl}/`;
  return new LazyThesaurus(
    Object.fromEntries(
      langs.map((l) => [
        l,
        async () => {
          const res = await fetchImpl(`${base}thesaurus/${l}/${l}.json.gz`);
          if (!res.ok) throw new Error(`thesaurus ${l}: ${res.status}`);
          return new Uint8Array(await res.arrayBuffer());
        },
      ]),
    ),
  );
}

// ------------------------------------------------------------------------------- text helpers

const WORD_CHAR = /[\p{L}\p{M}\p{N}'’-]/u;

/**
 * The word at UTF-16 offset `at` of `text` (a caret at either end of a word counts), without
 * leading / trailing apostrophes and hyphens; `null` when there is none.
 */
export function wordAtCaret(
  text: string,
  at: number,
): { from: number; to: number; word: string } | null {
  const is = (i: number) => i >= 0 && i < text.length && WORD_CHAR.test(text[i] ?? '');
  let p = Math.max(0, Math.min(at, text.length));
  if (!is(p) && is(p - 1)) p -= 1;
  if (!is(p)) return null;
  let from = p;
  let to = p + 1;
  while (is(from - 1)) from--;
  while (is(to)) to++;
  while (from < to && /['’-]/.test(text[from] ?? '')) from++;
  while (to > from && /['’-]/.test(text[to - 1] ?? '')) to--;
  if (from >= to || !/\p{L}/u.test(text.slice(from, to))) return null;
  return { from, to, word: text.slice(from, to) };
}

/**
 * `replacement` in the capitalisation of `original`: all capitals (two letters or more) stay all
 * capitals, an initial capital stays initial, anything else keeps the replacement as it is.
 */
export function matchCase(original: string, replacement: string, locale = 'en'): string {
  const letters = [...original].filter((c) => /\p{L}/u.test(c));
  if (!letters.length || !replacement) return replacement;
  const upper = (c: string) =>
    c !== c.toLocaleLowerCase(locale) && c === c.toLocaleUpperCase(locale);
  if (letters.length > 1 && letters.every(upper)) return replacement.toLocaleUpperCase(locale);
  if (upper(letters[0] ?? '')) {
    const [first = '', ...rest] = [...replacement];
    return first.toLocaleUpperCase(locale) + rest.join('');
  }
  return replacement;
}
