/**
 * The workbook data model of BlueSheet (SHEET-DM-001): tables, relationships, calculated
 * columns and measures in BlueOffice's own expression language (`bo_sheet::datamodel` in
 * Rust), the engine commands (`bo_engine::sheet::model_cmd::ModelCommand`) and what they
 * return. Pivot tables over the model are created with `modelPivotCreate` and edited with the
 * ordinary pivot commands.
 */

/** A column of a table in `modelInfo`. */
export interface ModelColumnInfo {
  name: string;
  /** A calculated column of the model. */
  calculated: boolean;
  expression?: string | null;
}

/** A sheet table (in the model or not) in `modelInfo`. */
export interface ModelTableInfo {
  name: string;
  inModel: boolean;
  /** The model names a table that no longer exists. */
  missing?: boolean;
  sheet?: number;
  sheetName?: string;
  /** Get & Transform query loading this table. */
  query?: string | null;
  rows?: number;
  columns: ModelColumnInfo[];
}

/** A relationship: `fromTable` is the many side, `toTable` the one side. */
export interface ModelRelationshipInfo {
  index: number;
  fromTable: string;
  fromColumn: string;
  toTable: string;
  toColumn: string;
  active: boolean;
  /** Why it does not work (a column missing, duplicate keys). */
  problem?: string | null;
}

export interface ModelMeasureInfo {
  name: string;
  table: string;
  expression: string;
  format?: string | null;
  error?: string | null;
}

export interface ModelCalcColumnInfo {
  table: string;
  name: string;
  expression: string;
  error?: string | null;
}

/** Data model content written by other software, kept byte for byte (read only). */
export interface ModelForeignPart {
  name: string;
  kind: 'model' | 'customXml' | 'connection';
  size: number;
}

/** `modelInfo` (also returned by every editing command). */
export interface ModelInfo {
  tables: ModelTableInfo[];
  relationships: ModelRelationshipInfo[];
  measures: ModelMeasureInfo[];
  columns: ModelCalcColumnInfo[];
  functions: { name: string; min: number; max: number }[];
  pivots: number;
  foreign: ModelForeignPart[];
}

export type ModelCardinality = 'manyToOne' | 'oneToMany' | 'oneToOne' | 'manyToMany';

/** `modelCheck`. */
export interface ModelCheckResult {
  ok: boolean;
  error?: { message: string; start: number; end: number };
}

/** Data model engine commands (→ the result types above). */
export type SheetModelCommand =
  | { cmd: 'modelInfo' }
  | { cmd: 'modelAddTable'; name: string }
  | { cmd: 'modelRemoveTable'; name: string }
  | {
      cmd: 'modelDetect';
      fromTable: string;
      fromColumn: string;
      toTable: string;
      toColumn: string;
    }
  | {
      cmd: 'modelSetRelationship';
      index?: number;
      fromTable: string;
      fromColumn: string;
      toTable: string;
      toColumn: string;
      active?: boolean;
    }
  | { cmd: 'modelDeleteRelationship'; index: number }
  | { cmd: 'modelCheck'; expression: string; table?: string; name?: string }
  | {
      cmd: 'modelSetMeasure';
      original?: string;
      name: string;
      table: string;
      expression: string;
      format?: string;
    }
  | { cmd: 'modelDeleteMeasure'; name: string }
  | { cmd: 'modelSetColumn'; original?: string; table: string; name: string; expression: string }
  | { cmd: 'modelDeleteColumn'; table: string; name: string }
  | { cmd: 'modelEvaluate'; expression: string }
  | {
      cmd: 'modelPivotCreate';
      dest?: { sheet: number; row: number; col: number } | null;
      labels?: import('./sheet').PivotLabels;
    };
