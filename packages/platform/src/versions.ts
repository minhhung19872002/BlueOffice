/**
 * Version history of a hosted file and comparing versions (CORE-VERSION-001).
 * The history comes from the host (`GET /api/sessions/{id}/versions`, BOHP /
 * local storage); BlueSheet compares through `SheetHost.exec`
 * (`bo_engine::sheet::compare_cmd`), BlueSlide through `EngineHost.slideCommand`
 * (`bo_engine::slide_compare`).
 */

/** One version of the history (mirrors the server's `HostVersion`). */
export interface FileVersion {
  version: string;
  size?: number;
  /** ISO date-time. */
  modifiedAt?: string | null;
  modifiedBy?: { id: string; name: string } | null;
  note?: string | null;
}

/** Version history of the open file (hosted sessions only). */
export interface VersionHistory {
  /** Newest first. */
  list(): Promise<FileVersion[]>;
  /** Bytes of one version, in the file's own format. */
  contents(version: string): Promise<Uint8Array>;
  /** Whether {@link VersionHistory.restore} is allowed (edit permission). */
  canRestore(): boolean;
  /** Restores a version as the new current one; the editor reloads afterwards. */
  restore(version: string): Promise<{ version: string }>;
  /** Name of the file (picks the reader of a version's bytes). */
  readonly fileName: string;
}

/** What changed in a workbook (mirrors `bo_sheet::diff::ChangeKind`). */
export type SheetChangeKind =
  | 'sheetAdded'
  | 'sheetRemoved'
  | 'sheetRenamed'
  | 'rowsInserted'
  | 'rowsDeleted'
  | 'colsInserted'
  | 'colsDeleted'
  | 'cellAdded'
  | 'cellCleared'
  | 'valueChanged'
  | 'formulaChanged'
  | 'formatChanged';

/** One difference between two workbooks (`bo_sheet::diff::SheetChange`). */
export interface SheetChange {
  kind: SheetChangeKind;
  /** Sheet name in the new workbook (old name for a removed sheet). */
  sheet: string;
  /** Sheet index in the open workbook (`null`: removed). */
  sheetIndex: number | null;
  oldSheet?: string;
  /** `B4`, `5:7` (rows), `C:D` (columns); empty for sheets. */
  address: string;
  oldAddress?: string;
  row?: number;
  col?: number;
  count?: number;
  old?: string;
  new?: string;
}

/** Result of `compareWorkbook`. */
export interface SheetCompareResult {
  changes: SheetChange[];
  total: number;
  truncated: boolean;
}

/** BlueSheet: the open workbook (new side) against another version (old side) → `SheetCompareResult`. */
export type CompareWorkbookCommand = {
  cmd: 'compareWorkbook';
  name: string;
  base64: string;
};

/** What changed in a deck (mirrors `bo_engine::slide_compare::DeckChangeKind`). */
export type DeckChangeKind =
  | 'slideAdded'
  | 'slideRemoved'
  | 'slideMoved'
  | 'layoutChanged'
  | 'backgroundChanged'
  | 'transitionChanged'
  | 'notesChanged'
  | 'shapeAdded'
  | 'shapeRemoved'
  | 'textChanged'
  | 'shapeMoved'
  | 'shapeResized'
  | 'shapeRotated'
  | 'formatChanged'
  | 'contentChanged';

/** One difference between two decks (`bo_engine::slide_compare::DeckChange`). */
export interface DeckChange {
  kind: DeckChangeKind;
  /** Slide index in the open deck (absent: removed slide). */
  slide?: number;
  oldSlide?: number;
  shape?: string;
  shapeId?: number;
  old?: string;
  new?: string;
}

/** BlueSlide: the open deck (new side) against another version → `{ deckChanges, total, truncated }`. */
export type CompareDeckCommand = {
  cmd: 'compareDeck';
  name: string;
  base64: string;
};

/** Answer of `compareDeck`. */
export interface DeckCompareResult {
  deckChanges: DeckChange[];
  total: number;
  truncated: boolean;
}
