# English thesaurus data (`en.json.gz`)

Used by the "Từ đồng nghĩa" (thesaurus) pane and context menu of BlueDoc, BlueSlide and BlueSheet
(CORE-THES-001, `packages/platform/src/thesaurus.ts`).

## Source and licence

- **Source:** Open English WordNet, **2025 edition** — <https://github.com/globalwordnet/english-wordnet>,
  release `2025-edition`, file `english-wordnet-2025.xml.gz` (WN-LMF 1.3 XML),
  SHA-256 `9ca6d1dcb75f822fdd66617f7d9da48142ace38dd544d6ad5e2feca1674ad3fe`.
- **Authors:** the Open English WordNet contributors (John P. McCrae et al.), based on Princeton
  WordNet 3.1 (© Princeton University).
- **Licence:** Creative Commons Attribution 4.0 International (CC BY 4.0),
  <https://creativecommons.org/licenses/by/4.0/> — as stated by the `license` attribute of the
  release's `<Lexicon>` element. Princeton WordNet, on which it is based, is under the WordNet
  licence (permissive, notice required); its notice follows.

```
  This software and database is being provided to you, the LICENSEE,
  by Princeton University under the following license.  By obtaining,
  using and/or copying this software and database, you agree that you
  have read, understood, and will comply with these terms and
  conditions.:

  Permission to use, copy, modify and distribute this software and
  database and its documentation for any purpose and without fee or
  royalty is hereby granted, provided that you agree to comply with
  the following copyright notice and statements, including the
  disclaimer, and that the same appear on ALL copies of the software,
  database and documentation, including modifications that you make
  for internal use or for distribution.

  WordNet 3.0 Copyright 2006 by Princeton University.  All rights
  reserved.

  THIS SOFTWARE AND DATABASE IS PROVIDED "AS IS" AND PRINCETON
  UNIVERSITY MAKES NO REPRESENTATIONS OR WARRANTIES, EXPRESS OR
  IMPLIED.  BY WAY OF EXAMPLE, BUT NOT LIMITATION, PRINCETON
  UNIVERSITY MAKES NO REPRESENTATIONS OR WARRANTIES OF MERCHANT-
  ABILITY OR FITNESS FOR ANY PARTICULAR PURPOSE OR THAT THE USE OF THE
  LICENSED SOFTWARE, DATABASE OR DOCUMENTATION WILL NOT INFRINGE ANY
  THIRD PARTY PATENTS, COPYRIGHTS, TRADEMARKS OR OTHER RIGHTS.

  The name of Princeton University or Princeton may not be used in
  advertising or publicity pertaining to distribution of the software
  and/or database.  Title to copyright in this software, database and
  any associated documentation shall at all times remain with
  Princeton University and LICENSEE agrees to preserve same.
```

**Attribution:** "Thesaurus data derived from Open English WordNet 2025
(https://github.com/globalwordnet/english-wordnet), licensed under CC BY 4.0. Changes: reduced to
single-word lower-case headwords with their synonyms, antonyms and shortened definitions."

## How it was derived (changes made)

`en.json.gz` is generated, not edited by hand:

```bash
node scripts/build-thesaurus.mjs <path>/english-wordnet-2025.xml.gz   # → assets/thesaurus/en/en.json.gz
```

The script (see its header for the exact rules):

- keeps as keys the single-word, lower-case lemmas (no proper nouns, digits or multi-word
  expressions);
- for each sense: part of speech (`n`, `v`, `a` — adjective and satellite —, `r`), the first
  definition cut to 80 characters, the other members of the synset (adjectives: plus the members of
  `similar` synsets, at most 16) and the lemmas of `antonym` sense relations; senses with neither
  synonyms nor antonyms are dropped; synonyms contain no upper-case letters or digits;
- keeps the irregular inflected forms (`<Form>`) of the kept lemmas (`mice` → `mouse`) for
  lemmatisation;
- writes compact JSON (`format: "bo-thesaurus/1"`), gzip-compressed.

Result (2025 edition): 52,924 headwords, 85,598 senses, 42,897 synsets, 3,648 irregular forms;
5.3 MB JSON, 1.9 MB gzip.
