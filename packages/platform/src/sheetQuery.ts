/**
 * Get & Transform data in BlueSheet (SHEET-PQ-001): the query definition (a source and an
 * ordered list of steps, BlueOffice's own step language — `bo_query::def` in Rust), the
 * engine commands (`bo_engine::sheet::query_cmd::QueryCommand`) and what they return.
 *
 * The engine never reads files or the network: the host reads the bytes of a source
 * through `Platform` (`files`, `dataSources`) and hands them over with `querySource`.
 */

/** Column type of a `changeType` step. */
export type QueryColType = 'any' | 'text' | 'number' | 'integer' | 'date' | 'bool';

/** Condition of a `filterRows` step. */
export type QueryCondition =
  | 'equals'
  | 'notEquals'
  | 'greaterThan'
  | 'greaterOrEqual'
  | 'lessThan'
  | 'lessOrEqual'
  | 'contains'
  | 'notContains'
  | 'beginsWith'
  | 'endsWith'
  | 'isBlank'
  | 'isNotBlank';

/** Aggregate of a `groupBy` step. */
export type QueryAggregateOp =
  'sum' | 'average' | 'min' | 'max' | 'count' | 'countDistinct' | 'median';

/** Join kind of a `merge` step. */
export type QueryJoinKind =
  'leftOuter' | 'rightOuter' | 'fullOuter' | 'inner' | 'leftAnti' | 'rightAnti';

/** Where a query reads its data. */
export type QuerySource =
  | { kind: 'csv'; file: string; delimiter?: string; encoding?: string }
  | { kind: 'json'; file: string; path?: string }
  | { kind: 'xml'; file: string; element?: string }
  | { kind: 'workbook'; file: string; item: string; itemKind?: 'sheet' | 'table' }
  | {
      kind: 'folder';
      folder: string;
      fileKind: 'csv' | 'json' | 'xml' | 'workbook';
      delimiter?: string;
      item?: string;
      promoteHeaders?: boolean;
    }
  | { kind: 'web'; url: string; table?: number }
  | { kind: 'query'; query: string };

/** One step of a query (`op` names it). */
export type QueryStep =
  | { op: 'promoteHeaders' }
  | { op: 'demoteHeaders' }
  | {
      op: 'changeType';
      columns: { column: string; type: QueryColType }[];
      decimal?: string;
      monthFirst?: boolean;
    }
  | { op: 'filterRows'; column: string; condition: QueryCondition; value?: string }
  | { op: 'removeColumns'; columns: string[] }
  | { op: 'keepColumns'; columns: string[] }
  | { op: 'renameColumns'; renames: { from: string; to: string }[] }
  | { op: 'reorderColumns'; columns: string[] }
  | { op: 'splitColumn'; column: string; delimiter: string; parts?: number }
  | {
      op: 'replaceValues';
      columns?: string[];
      find: string;
      replace: string;
      matchEntire?: boolean;
    }
  | {
      op: 'groupBy';
      keys: string[];
      aggregates: { name: string; op: QueryAggregateOp; column?: string }[];
    }
  | { op: 'sort'; keys: { column: string; descending?: boolean }[] }
  | {
      op: 'merge';
      query: string;
      leftKeys: string[];
      rightKeys: string[];
      kind?: QueryJoinKind;
      columns?: string[];
    }
  | { op: 'append'; queries: string[] }
  | { op: 'unpivot'; columns: string[]; other?: boolean; attribute?: string; value?: string }
  | { op: 'addColumn'; name: string; formula: string }
  | { op: 'removeDuplicates'; columns?: string[] }
  | { op: 'keepTop'; count: number }
  | { op: 'removeTop'; count: number }
  | { op: 'removeBlankRows' };

/** Step name (`QueryStep['op']`). */
export type QueryStepOp = QueryStep['op'];

/** A query definition. */
export interface QueryDef {
  name: string;
  source: QuerySource;
  steps: QueryStep[];
  /** The table holding the result (set by the engine on load). */
  load?: { table: string };
  description?: string;
}

/** A preview value: `null`, a number, text (dates as ISO 8601, errors as `#VALUE!`) or a boolean. */
export type QueryValue = string | number | boolean | null;

/** Why a query failed (`code`: `missingSource`, `source`, `column`, `step`, `formula`, `query`, `cycle`). */
export interface QueryErrorInfo {
  /** Failing step (`null`: the source). */
  step: number | null;
  code: string;
  message: string;
  /** Source key the host must provide again. */
  missing: string | null;
}

/** Result of `queryPreview` (and the `preview` of `queryInspect`). */
export interface QueryPreview {
  columns: string[];
  types: QueryColType[];
  rows: QueryValue[][];
  rowCount: number;
  error?: QueryErrorInfo;
}

/** A web table of `queryInspect`. */
export interface QueryWebTable {
  index: number;
  caption: string;
  header: boolean;
  rows: number;
  cols: number;
  preview: string[][];
}

/** Result of `queryInspect`: the navigator of the source and the steps proposed for it. */
export interface QueryInspectResult {
  csv?: { encoding: string; delimiter: string };
  jsonPaths?: string[];
  xmlElements?: { name: string; count: number }[];
  items?: { name: string; kind: 'sheet' | 'table' }[];
  tables?: QueryWebTable[];
  files?: string[];
  suggested?: QueryStep[];
  preview?: QueryPreview;
  error?: QueryErrorInfo;
}

/** A saved query as `queryList` lists it. */
export interface QueryListEntry {
  name: string;
  kind: QuerySource['kind'];
  /** Source key (file, folder, URL); `null` for a query source. */
  key: string | null;
  access: 'file' | 'folder' | 'web' | 'query';
  steps: number;
  table: string | null;
  sheet: number | null;
  sheetName: string | null;
  rows: number | null;
  /** The host has provided the source's bytes this session. */
  available: boolean;
}

/** Result of `queryRefresh`. */
export interface QueryRefreshResult {
  refreshed: { name: string; rows: number }[];
  errors: (QueryErrorInfo & { name: string })[];
}

/** Result of `querySave`. */
export interface QuerySaveResult {
  name: string;
  table: string | null;
  sheet: number | null;
  rows: number;
}

/** Get & Transform engine commands (→ the result types above). */
export type SheetQueryCommand =
  | {
      cmd: 'querySource';
      key: string;
      access: 'file' | 'folder' | 'web';
      files: { name: string; data: string }[];
    }
  | { cmd: 'queryInspect'; source: QuerySource }
  | { cmd: 'queryPreview'; query: QueryDef; upto?: number; limit?: number }
  | { cmd: 'queryList' }
  | { cmd: 'queryGet'; name: string }
  | {
      cmd: 'querySave';
      query: QueryDef;
      original?: string;
      load?: 'newSheet' | 'existing' | 'none';
      sheet?: number;
      row?: number;
      col?: number;
    }
  | { cmd: 'queryRefresh'; names?: string[] }
  | { cmd: 'queryDelete'; name: string; deleteTable?: boolean }
  | { cmd: 'queryMashup' };

/** A folder the user picked: its files (name relative to the folder). */
export interface PickedFolder {
  name: string;
  /** Absolute path on desktop. */
  path?: string;
  files: { name: string; bytes: Uint8Array }[];
}

/** Why a web page could not be fetched. */
export type WebFetchError = 'unavailable' | 'network' | 'status' | 'tooLarge';

/** Sources of Get & Transform beyond single files (absent: not available on this platform). */
export interface QueryDataSources {
  /** Ask the user for a folder; `null` when cancelled. */
  pickFolder?(): Promise<PickedFolder | null>;
  /** Desktop: read a folder again by its path (refresh). */
  readFolder?(path: string): Promise<PickedFolder>;
  /**
   * Fetch a web page: on desktop directly, on the web through the BlueOffice server's proxy
   * when the administrator configured one. Rejects with a `WebFetchError` message.
   */
  fetchWeb?(url: string): Promise<Uint8Array>;
}
