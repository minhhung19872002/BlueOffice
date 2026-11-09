/**
 * Embedded and linked BlueSheet workbooks in BlueDoc and BlueSlide (CORE-EMBED-001).
 * Mirrors `bo_engine::ole` (`OleCommand`, `OleInfo`, `LinkOut`). The commands go through
 * `EngineHost.command` (BlueDoc) and `EngineHost.slideCommand` (BlueSlide).
 */

/** The object at the selection. */
export interface OleInfo {
  /** File-format identifier of the object's server (never shown). */
  progId: string;
  /** The object holds a workbook BlueSheet can open. */
  workbook: boolean;
  /** Linked to a file (else embedded). */
  linked: boolean;
  /** Linked file path. */
  source?: string;
  /** Item (range) inside the linked file. */
  item?: string;
  autoUpdate: boolean;
  /** Shown size in points. */
  width: number;
  height: number;
  /** Sheet and area an embedded workbook shows. */
  sheet?: string;
  range?: string;
  /** Cell inputs of the area, row by row (`oleInfo` with `cells`). */
  cells?: string[][];
  /** Embedded workbook, base64 (`oleInfo` with `data`). */
  data?: string;
}

/** A linked file of the document. */
export interface OleLink {
  /** Source as stored. */
  source: string;
  /** Path the host reads. */
  path: string;
  item: string;
  /** Objects linked to it. */
  count: number;
}

export type OleCommand =
  | { cmd: 'oleInfo'; cells?: boolean; data?: boolean }
  | {
      cmd: 'oleInsert';
      /** Workbook bytes (base64); absent = a new empty workbook. */
      data?: string;
      /** Area to show, e.g. `Sheet1!A1:C5`. */
      range?: string;
      /** Cell inputs written at the top-left of the area. */
      cells?: string[][];
      /** Link to this file path instead of embedding (desktop). */
      link?: string;
      autoUpdate?: boolean;
    }
  | { cmd: 'oleEdit'; cells: string[][] }
  | { cmd: 'oleSetData'; data: string }
  | { cmd: 'oleLinks' }
  | { cmd: 'oleUpdateLinks'; sources: { source: string; data: string }[] };

/** Extra fields of a command result after an `ole*` command. */
export interface OleResult {
  ole?: OleInfo;
  links?: OleLink[];
  /** Objects updated by `oleUpdateLinks`. */
  count?: number;
  error?: string;
}
