#!/usr/bin/env node
// Builds the English thesaurus asset (CORE-THES-001) from Open English WordNet (CC BY 4.0).
//
//   node scripts/build-thesaurus.mjs <english-wordnet-YYYY.xml.gz> [out-dir]
//
// Input: the WN-LMF XML release file of https://github.com/globalwordnet/english-wordnet
// (`english-wordnet-2025.xml.gz`, release `2025-edition`). Output: `<out-dir>/en.json.gz`
// (default `assets/thesaurus/en`), a compact derived dataset:
//
//   {
//     format: 'bo-thesaurus/1', lang: 'en', source, version, license, licenseUrl, url,
//     words:   string[]                                  // synonym / antonym strings
//     synsets: [pos, gloss, number[] /* words */][]      // pos: n v a r
//     entries: { [lemma]: ([synset] | [synset, number[] /* antonym words */])[] }
//     forms:   { [inflected form]: 'lemma' | 'lemma1 lemma2' }   // irregular forms (<Form>)
//   }
//
// Derivation: keys are the single-word, lower-case lemmas of the lexicon (proper nouns, words
// with digits and multi-word expressions are not keys); a sense keeps the other members of its
// synset (adjectives: also the members of `similar` synsets, at most 16 in all) and the lemmas of
// its `antonym` sense relations; senses without any synonym or antonym are dropped. Synonyms may
// be multi-word ("look into") but never contain upper-case letters or digits. Glosses are the
// first definition, cut at a word boundary to 80 characters. Senses keep the lexicon's order.
//
// The XML is read line by line (WN-LMF releases put one element per line); nothing in the input
// is executed.
import { createReadStream, mkdirSync, writeFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { createGunzip, gzipSync, constants } from 'node:zlib';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const input = process.argv[2];
if (!input) {
  console.error('usage: node scripts/build-thesaurus.mjs <english-wordnet.xml.gz> [out-dir]');
  process.exit(2);
}
const outDir = resolve(process.argv[3] ?? join(root, 'assets', 'thesaurus', 'en'));

const GLOSS_MAX = 80;
const SIMILAR_MAX = 16;

const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };
const decode = (s) =>
  s.replace(/&(#x[0-9a-f]+|#\d+|[a-z]+);/gi, (m, e) => {
    if (e[0] === '#')
      return String.fromCodePoint(
        e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10),
      );
    return ENTITIES[e] ?? m;
  });
const attr = (line, name) => {
  const m = line.match(new RegExp(`\\s${name}="([^"]*)"`));
  return m ? decode(m[1]) : null;
};

/** Lexicon metadata (from the `<Lexicon>` element). */
const meta = {};
/** Entry id → { lemma, pos, forms, senses: [{ id, synset, antonyms: senseId[] }] }. */
const entries = new Map();
/** Sense id → lemma. */
const senseLemma = new Map();
/** Synset id → { pos, gloss, members: entryId[], similar: synsetId[] }. */
const synsets = new Map();

let entry = null;
let sense = null;
let synset = null;
let inLexicon = false;

const lines = createInterface({ input: createReadStream(input).pipe(createGunzip()) });
for await (const raw of lines) {
  const line = raw.trim();
  // The `<Lexicon>` start tag spans several lines (one attribute each).
  if (line.startsWith('<Lexicon ')) inLexicon = true;
  if (inLexicon) {
    for (const k of ['id', 'label', 'language', 'license', 'version', 'url']) {
      const v = attr(` ${line}`, k);
      if (v !== null) meta[k] ??= v;
    }
    if (line.endsWith('>')) inLexicon = false;
    continue;
  }
  if (line.startsWith('<LexicalEntry ')) {
    entry = { id: attr(line, 'id'), lemma: null, pos: null, forms: [], senses: [] };
  } else if (entry && line.startsWith('<Lemma ')) {
    entry.lemma = attr(line, 'writtenForm');
    entry.pos = attr(line, 'partOfSpeech');
  } else if (entry && line.startsWith('<Form ')) {
    const f = attr(line, 'writtenForm');
    if (f) entry.forms.push(f);
  } else if (entry && line.startsWith('<Sense ')) {
    sense = { id: attr(line, 'id'), synset: attr(line, 'synset'), antonyms: [] };
    entry.senses.push(sense);
    senseLemma.set(sense.id, entry.lemma);
    if (line.endsWith('/>')) sense = null;
  } else if (sense && line.startsWith('<SenseRelation ')) {
    if (attr(line, 'relType') === 'antonym') sense.antonyms.push(attr(line, 'target'));
  } else if (line.startsWith('</Sense>')) {
    sense = null;
  } else if (line.startsWith('</LexicalEntry>')) {
    if (entry) entries.set(entry.id, entry);
    entry = null;
  } else if (line.startsWith('<Synset ')) {
    synset = {
      id: attr(line, 'id'),
      pos: attr(line, 'partOfSpeech'),
      members: (attr(line, 'members') ?? '').split(/\s+/).filter(Boolean),
      gloss: null,
      similar: [],
    };
    if (line.endsWith('/>')) {
      synsets.set(synset.id, synset);
      synset = null;
    }
  } else if (synset && line.startsWith('<Definition')) {
    const m = line.match(/^<Definition[^>]*>(.*)<\/Definition>$/);
    if (m && synset.gloss === null) synset.gloss = decode(m[1]);
  } else if (synset && line.startsWith('<SynsetRelation ')) {
    if (attr(line, 'relType') === 'similar') synset.similar.push(attr(line, 'target'));
  } else if (line.startsWith('</Synset>')) {
    if (synset) synsets.set(synset.id, synset);
    synset = null;
  }
}
if (meta.license !== 'https://creativecommons.org/licenses/by/4.0') {
  console.error(`unexpected licence ${meta.license}: stop (CORE-THES-001 needs CC BY 4.0)`);
  process.exit(1);
}

const isKey = (w) => /^[a-z][a-z'-]*[a-z]$/.test(w) || /^[a-z]$/.test(w);
const isSynonym = (w) => /^[a-z][a-z' .-]*$/.test(w) && !w.endsWith(' ') && w.length <= 40;
const posOf = (p) => (p === 's' ? 'a' : p);

const memberLemmas = (s) =>
  s.members.map((id) => entries.get(id)?.lemma).filter((w) => w && isSynonym(w));

const cutGloss = (g) => {
  const t = (g ?? '').replace(/\s+/g, ' ').trim();
  if (t.length <= GLOSS_MAX) return t;
  const cut = t.slice(0, GLOSS_MAX);
  const sp = cut.lastIndexOf(' ');
  return `${(sp > 40 ? cut.slice(0, sp) : cut).replace(/[,;:]$/, '')}…`;
};

/** Synonyms of synset `s` (members, then members of similar synsets for adjectives). */
const synonymsOf = (s) => {
  const out = memberLemmas(s);
  if (posOf(s.pos) === 'a') {
    for (const id of s.similar) {
      const o = synsets.get(id);
      if (!o) continue;
      for (const w of memberLemmas(o))
        if (out.length < SIMILAR_MAX && !out.includes(w)) out.push(w);
    }
  }
  return out;
};

const words = new Map();
const wordList = [];
const wordId = (w) => {
  let i = words.get(w);
  if (i === undefined) {
    i = wordList.length;
    wordList.push(w);
    words.set(w, i);
  }
  return i;
};
const synIndex = new Map();
const synList = [];
const synsetId = (s) => {
  let i = synIndex.get(s.id);
  if (i === undefined) {
    i = synList.length;
    synList.push([posOf(s.pos), cutGloss(s.gloss), synonymsOf(s).map(wordId)]);
    synIndex.set(s.id, i);
  }
  return i;
};

const outEntries = Object.create(null);
const forms = Object.create(null);
let senses = 0;
for (const e of entries.values()) {
  const key = e.lemma;
  if (!key || !isKey(key)) continue;
  for (const f of e.forms) {
    if (f === key || !isKey(f)) continue;
    const prev = forms[f] ? forms[f].split(' ') : [];
    if (!prev.includes(key)) forms[f] = [...prev, key].join(' ');
  }
  const list = (outEntries[key] ??= []);
  for (const sn of e.senses) {
    const s = synsets.get(sn.synset);
    if (!s) continue;
    const syn = synonymsOf(s).filter((w) => w !== key);
    const ants = [...new Set(sn.antonyms.map((id) => senseLemma.get(id)))].filter(
      (w) => w && isSynonym(w),
    );
    if (!syn.length && !ants.length) continue;
    const idx = synsetId(s);
    if (list.some((x) => x[0] === idx)) continue;
    list.push(ants.length ? [idx, ants.map(wordId)] : [idx]);
    senses++;
  }
  if (!list.length) delete outEntries[key];
}
// Forms only matter for lemmas that are keys.
for (const [f, l] of Object.entries(forms)) {
  const keep = l.split(' ').filter((x) => outEntries[x]);
  if (keep.length) forms[f] = keep.join(' ');
  else delete forms[f];
}

const data = {
  format: 'bo-thesaurus/1',
  lang: 'en',
  source: meta.label ?? 'Open English Wordnet',
  version: meta.version ?? '',
  license: 'CC-BY-4.0',
  licenseUrl: meta.license,
  url: meta.url ?? 'https://github.com/globalwordnet/english-wordnet',
  words: wordList,
  synsets: synList,
  entries: outEntries,
  forms,
};
const json = JSON.stringify(data);
const gz = gzipSync(json, { level: constants.Z_BEST_COMPRESSION });
mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, 'en.json.gz'), gz);
console.log(
  `thesaurus: ${Object.keys(outEntries).length} entries, ${senses} senses, ` +
    `${synList.length} synsets, ${wordList.length} words, ${Object.keys(forms).length} forms; ` +
    `${(json.length / 1048576).toFixed(2)} MB → ${(gz.length / 1048576).toFixed(2)} MB gzip`,
);
