/**
 * Spelling and grammar in BlueSheet cells and BlueSlide text (CORE-SPELL-006). Sheet commands go
 * through `SheetHost.exec` (`bo_engine::sheet::spell_cmd`), slide commands through
 * `EngineHost.slideCommand` (`bo_engine::slide_edit::spell`); both use the BlueDoc checker.
 */

/** One issue of a text (mirrors `bo_engine::proof_text::TextIssue`). */
export interface TextIssue {
  /** Byte offsets in the text (what corrections send back). */
  start: number;
  end: number;
  /** UTF-16 offsets in the text (what a text input selects). */
  from: number;
  to: number;
  word: string;
  grammar?: boolean;
  /** Grammar rule id (`extraSpace`, `capitalize`, `toneStyle` …). */
  rule?: string;
  suggestions: string[];
  lang?: string;
  /** Sentence text around the flagged text. */
  before: string;
  after: string;
}

/** Proofing options of a sheet or slide engine. */
export interface SpellOptions {
  enabled: boolean;
  grammar: boolean;
  ignoreCaps: boolean;
  ignoreDigits: boolean;
  /** Languages with a dictionary (`vi`, `en`). */
  languages: string[];
}

/** A correction of a text: replace bytes `start..end`; `word` is the text expected there. */
export interface TextReplace {
  start: number;
  end: number;
  text: string;
  word?: string;
}

/** An issue of a cell (`spellCheck`). */
export interface CellIssue extends TextIssue {
  sheet: number;
  sheetName: string;
  row: number;
  col: number;
}

/** BlueSheet spelling commands (`SheetHost.exec`). */
export type SheetSpellCommand =
  /** → `{ issues: TextIssue[]; unchecked: string[] }` (a text starting with `=` is not checked). */
  | { cmd: 'spellText'; text: string; lang?: string }
  /** → `SheetSpellReport`. */
  | { cmd: 'spellCheck'; lang?: string }
  /** One undoable edit → `{ changed: number }`. */
  | {
      cmd: 'spellReplace';
      edits: (TextReplace & { sheet: number; row: number; col: number })[];
    }
  | { cmd: 'spellIgnore' | 'spellAddWord'; word: string }
  | { cmd: 'grammarIgnore'; rule: string; text: string }
  /** → `SpellOptions`. */
  | ({ cmd: 'spellOptions' } & Partial<Omit<SpellOptions, 'languages'>>);

/** Result of `spellCheck`. */
export interface SheetSpellReport {
  issues: CellIssue[];
  unchecked: string[];
  languages: string[];
}

/** Where a text of a deck lives (mirrors `bo_engine::slide_edit::spell::SpellTarget`). */
export type SlideSpellTarget =
  | { kind: 'shape'; slide: number; id: number }
  | { kind: 'cell'; slide: number; id: number; row: number; col: number }
  | { kind: 'notes'; slide: number }
  | { kind: 'smartArt'; slide: number; id: number };

/** An issue of a deck (`slideSpellCheck`, `slideSpellAt`). */
export interface SlideIssue extends TextIssue {
  target: SlideSpellTarget;
  slide: number;
  /** Position of the text unit in checking order. */
  unit: number;
  /** Paragraph of the unit (`start`/`end` are byte offsets in it). */
  para: number;
  /** UTF-16 offsets in the unit's editing text (paragraphs joined by `\n`). */
  bodyFrom: number;
  bodyTo: number;
}

/** A wavy line under an issue on a slide (slide points). */
export interface SlideSpellMark {
  from: [number, number];
  to: [number, number];
  /** Height of the text line above the mark (points). */
  h: number;
  grammar?: boolean;
  /** Shape id. */
  id: number;
}

/** BlueSlide spelling commands (`EngineHost.slideCommand`). */
export type SlideSpellCommand =
  /** → `{ slideSpell: { marks: SlideSpellMark[] } }`. */
  | { cmd: 'slideSpelling'; slide?: number }
  /** → `{ slideSpell: { hit: SlideIssue | null } }`. */
  | { cmd: 'slideSpellAt'; x: number; y: number; slide?: number }
  /** → `{ slideSpell: SlideSpellReport }`. */
  | { cmd: 'slideSpellCheck' }
  /** One undoable transaction → `SlideEditResult`. */
  | {
      cmd: 'slideSpellReplace';
      edits: (TextReplace & { target: SlideSpellTarget; para: number })[];
    }
  | { cmd: 'slideSpellIgnore' | 'slideSpellAddWord'; word: string }
  | { cmd: 'slideGrammarIgnore'; rule: string; text: string }
  /** → `{ slideSpell: { options: SpellOptions } }`. */
  | ({ cmd: 'slideSpellOptions' } & Partial<Omit<SpellOptions, 'languages'>>);

/** Result of `slideSpellCheck`. */
export interface SlideSpellReport {
  issues: SlideIssue[];
  unchecked: string[];
  languages: string[];
}

/** Answer of the slide spelling queries. */
export interface SlideSpellAnswer {
  slideSpell?: {
    marks?: SlideSpellMark[];
    hit?: SlideIssue | null;
    issues?: SlideIssue[];
    unchecked?: string[];
    languages?: string[];
    options?: SpellOptions;
    ok?: boolean;
  };
}
