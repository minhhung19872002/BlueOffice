/**
 * BlueDoc style management (DOC-STYLE-001) over the JSON command bridge: the Styles
 * pane's per-style actions, the Style Inspector, Reveal Formatting and the Organizer
 * (mirrors `bo_engine::doc_styles` / `bo_edit::styles`, ECMA-376 Part 1, §17.7).
 */
import type { Alignment } from './types';

export type DocStyleType = 'paragraph' | 'character' | 'table' | 'numbering';

/** Paragraph box border (§17.3.1.24 pBdr). */
export interface DocStyleBorder {
  /** `ST_Border` (`single`, `double`, `dotted`, …). */
  style: string;
  /** Eighths of a point. */
  size: number;
  /** `RRGGBB`; `null`: automatic. */
  color?: string | null;
}

/** Values of the Modify / New Style dialog (effective values of the style chain; `null`: not set). */
export interface DocStyleSpec {
  name: string;
  type: DocStyleType;
  basedOn: string | null;
  next: string | null;
  /** §17.7.4.14 qFormat. */
  addToGallery: boolean;
  /** §17.7.4.2 autoRedefine. */
  autoUpdate: boolean;
  font: string | null;
  /** Half-points. */
  size: number | null;
  bold: boolean | null;
  italic: boolean | null;
  underline: boolean | null;
  /** `RRGGBB`. */
  color: string | null;
  align: Alignment | null;
  /** Twips. */
  spaceBefore: number | null;
  spaceAfter: number | null;
  /** 240ths of a line. */
  lineSpacing: number | null;
  indentLeft: number | null;
  indentRight: number | null;
  /** Negative: hanging. */
  indentFirstLine: number | null;
  border: DocStyleBorder | null;
  shading: string | null;
  /** §17.3.1.19 numPr numId (`0` switches inherited numbering off). */
  numId: number | null;
  numLevel: number;
}

/** Where a value comes from (§17.7.2). */
export type RevealSource = 'default' | 'paragraphStyle' | 'characterStyle' | 'direct' | 'section';

export interface RevealItem {
  group: 'font' | 'paragraph' | 'section';
  key: string;
  /** Language neutral: `true`/`false`, `12pt`, names, `RRGGBB`. */
  value: string;
  source: RevealSource;
  /** Display name of the style that sets it. */
  style?: string;
}

export interface StyleRefInfo {
  id: string;
  name: string;
}

export interface DirectValue {
  key: string;
  value: string;
}

export interface StyleInspectorInfo {
  paragraphStyle: StyleRefInfo | null;
  paragraphDirect: DirectValue[];
  characterStyle: StyleRefInfo | null;
  characterDirect: DirectValue[];
}

export interface OrganizerStyle {
  id: string;
  name: string;
  type: DocStyleType;
  isDefault: boolean;
}

export interface OrganizerInfo {
  current: OrganizerStyle[];
  file: OrganizerStyle[];
  /** Changed file (base64) after a `toFile` action. */
  output?: string;
  copied?: string[];
}

/** `styleMgr` of an edit result. */
export interface StyleMgrOut {
  spec?: DocStyleSpec;
  created?: string;
  inspector?: StyleInspectorInfo;
  reveal?: RevealItem[];
  organizer?: OrganizerInfo;
}

export type StyleClearLevel =
  'paragraphStyle' | 'paragraphDirect' | 'characterStyle' | 'characterDirect';

export type OrganizerAction = 'copy' | 'delete' | 'rename';

export type DocStyleCommand =
  | { cmd: 'styleDetails'; id: string }
  | { cmd: 'styleModify'; id: string; spec: DocStyleSpec }
  | { cmd: 'styleNew'; spec: Partial<DocStyleSpec> & { name: string }; fromSelection?: boolean }
  | { cmd: 'styleDelete'; id: string }
  | { cmd: 'styleSelectAll'; id: string }
  | { cmd: 'styleUpdateToMatch'; id: string }
  | { cmd: 'styleInspector' }
  | { cmd: 'styleClear'; level: StyleClearLevel }
  | { cmd: 'revealFormatting' }
  | { cmd: 'styleOrganizer'; file: string }
  | {
      cmd: 'styleOrganizerApply';
      file: string;
      action: OrganizerAction;
      ids: string[];
      toFile?: boolean;
      newName?: string;
    };

/** Reveal Formatting compare mode: keys whose value differs between two snapshots. */
export function revealDiff(a: RevealItem[], b: RevealItem[]): Set<string> {
  const key = (i: RevealItem) => `${i.group}.${i.key}`;
  const am = new Map(a.map((i) => [key(i), i.value]));
  const bm = new Map(b.map((i) => [key(i), i.value]));
  const out = new Set<string>();
  for (const k of new Set([...am.keys(), ...bm.keys()])) {
    if (am.get(k) !== bm.get(k)) out.add(k);
  }
  return out;
}
