/**
 * Citations and bibliography of BlueDoc (CORE-CITE-001): the sources of the document's
 * bibliography part (ECMA-376 Part 1, §22.6 `b:Sources`), `CITATION` (§17.16.5.8) and
 * `BIBLIOGRAPHY` (§17.16.5.7) fields — the engine commands `cite*`, mirroring
 * `bo_engine::cite::CiteCommand`.
 */

/** Reference style (mirrors `bo_edit::cite::StyleId`; stored in `b:Sources/@StyleName`). */
export type CiteStyleId =
  'APA' | 'MLA' | 'Chicago' | 'IEEE' | 'Harvard' | 'ISO690' | 'ISO690Numeric' | 'VNThesis';

/** Every style, in the order the picker lists them. */
export const CITE_STYLES: CiteStyleId[] = [
  'APA',
  'MLA',
  'Chicago',
  'IEEE',
  'Harvard',
  'ISO690',
  'ISO690Numeric',
  'VNThesis',
];

/** Source types of §22.6.2.63 ST_SourceType. */
export const CITE_SOURCE_TYPES = [
  'Book',
  'BookSection',
  'JournalArticle',
  'ArticleInAPeriodical',
  'ConferenceProceedings',
  'Report',
  'InternetSite',
  'DocumentFromInternetSite',
  'ElectronicSource',
  'Art',
  'SoundRecording',
  'Performance',
  'Film',
  'Interview',
  'Patent',
  'Case',
  'Misc',
] as const;
export type CiteSourceType = (typeof CITE_SOURCE_TYPES)[number];

/** A person's name (§22.6.2.52 Person). */
export interface CitePerson {
  last: string;
  first?: string;
  middle?: string;
}

/** Contributors of one role: people or a corporate name (§22.6.2.17). */
export interface CiteContributor {
  /** `Author`, `Editor`, `Translator`, … */
  role: string;
  corporate?: string;
  people?: CitePerson[];
}

/** One source (§22.6.2.59; mirrors `bo_edit::cite::Source`). */
export interface CiteSource {
  /** `b:Tag`: the name citations refer to (empty: the engine makes one). */
  tag: string;
  /** `b:SourceType`. */
  kind: string;
  contributors?: CiteContributor[];
  /** Other elements by name: `Title`, `Year`, `Publisher`, `URL`, `LCID`, … */
  fields?: Record<string, string>;
}

/** A stretch of a formatted bibliography entry. */
export interface CiteSpan {
  text: string;
  italic?: boolean;
}

/** A source as the source manager lists it. */
export interface CiteSourceInfo extends CiteSource {
  /** A citation refers to it. */
  cited: boolean;
  /** A tag without data, to fill in later. */
  placeholder: boolean;
  /** Its bibliography entry in the current style. */
  preview: CiteSpan[];
  /** Its citation in the current style. */
  citation: string;
}

/** One source of a citation (§17.16.5.8 switches `\p \v \f \s \n \y \t`). */
export interface CiteItem {
  tag: string;
  pages?: string;
  volume?: string;
  prefix?: string;
  suffix?: string;
  noAuthor?: boolean;
  noYear?: boolean;
  noTitle?: boolean;
}

/** A citation: one or more sources (`\m`) and a language (`\l`). */
export interface Citation {
  items: CiteItem[];
  lcid?: string;
}

/** `EditResult.cite` (mirrors `bo_engine::cite::CiteInfo`). */
export interface CiteInfo {
  style: CiteStyleId;
  sources: CiteSourceInfo[];
  /** The citation the caret touches. */
  citation?: Citation;
  hasBibliography: boolean;
  /** Fields whose result the command changed. */
  changed: number;
  /** `citeParse`: the sources read. */
  parsed?: CiteSource[];
  /** `citeExport`: BibTeX text. */
  exported?: string;
  /** `citePreview`: entries of sources outside the document. */
  preview?: CiteSourceInfo[];
}

export type CiteCommand =
  | { cmd: 'citeState' }
  | { cmd: 'citeSetStyle'; style: CiteStyleId }
  | { cmd: 'citeSaveSources'; sources: CiteSource[]; replace?: string }
  | { cmd: 'citeDeleteSource'; tag: string }
  | { cmd: 'citeInsert'; citation: Citation; sources?: CiteSource[] }
  | { cmd: 'citeEdit'; citation: Citation }
  | { cmd: 'citeBibliography'; heading?: string; lcid?: string }
  | { cmd: 'citeParse'; format: 'bibtex' | 'ris'; text: string; taken?: string[] }
  | { cmd: 'citeExport'; sources?: CiteSource[] }
  | { cmd: 'citePreview'; style?: CiteStyleId; sources: CiteSource[] };
