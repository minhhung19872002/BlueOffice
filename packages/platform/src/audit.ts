/**
 * BlueSheet formula auditing and calculation commands (SHEET-AUDIT-001), sent through
 * `SheetHost.exec` like every sheet command; implemented by `bo_engine` `audit_cmd`.
 * Calculation settings map to ECMA-376 Part 1, §18.2.2 `calcPr`; ignored findings are
 * saved as §18.3.1.51 `ignoredErrors`.
 */

/** A cell range of a sheet (inclusive, 0-based). */
export interface AuditArea {
  sheet: number;
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

/** A cell of a sheet (0-based). */
export interface AuditCell {
  sheet: number;
  row: number;
  col: number;
}

/** One trace arrow: from a precedent (range) to the formula cell that reads it. */
export interface TraceArrow {
  from: AuditArea;
  to: AuditCell;
  /** 1 for direct links, 2 for the next level… */
  level: number;
  /** The source cell holds an error value. */
  error: boolean;
}

/** Result of `traceArrows`. */
export interface TraceArrows {
  arrows: TraceArrow[];
  /** Another level would add arrows. */
  more: boolean;
}

/** One step of `evaluateFormula`: the text split around the part evaluated next. */
export interface EvalStep {
  before: string;
  part: string;
  after: string;
  /** Value of the part, as written into the next step. */
  shown: string;
  error: boolean;
  /** The part references this formula cell ("step in"). */
  cell: (AuditCell & { label: string }) | null;
}

/** Result of `evaluateFormula` (`null` for a cell without a formula). */
export interface EvalTrace {
  /** `Sheet1!C1`. */
  cell: string;
  /** Formula with the leading `=`. */
  formula: string;
  steps: EvalStep[];
  /** Final value. */
  result: string;
  truncated: boolean;
}

/** Error-checking rules. */
export type ErrorRule =
  'evalError' | 'inconsistentFormula' | 'numberAsText' | 'emptyRef' | 'unlockedFormula';

/** Which rules are checked. */
export type ErrorRules = Record<
  'evalError' | 'inconsistentFormula' | 'numberAsText' | 'emptyRef' | 'unlockedFormula',
  boolean
>;

/** One flagged cell of `errorCheck`. */
export interface ErrorFinding {
  row: number;
  col: number;
  rule: ErrorRule;
  /** `inconsistentFormula`: the neighbours' formula for this cell (without `=`). */
  suggestion?: string;
  /** `numberAsText`: the number the text reads as. */
  number?: number;
  /** `emptyRef`: the empty cell read. */
  empty?: string;
}

/** A row of the watch window. */
export interface WatchRow {
  book: string;
  sheet: string | null;
  name: string | null;
  cell: string;
  value: string;
  formula: string | null;
}

/** `calcPr/@calcMode`. */
export type CalcMode = 'auto' | 'autoNoTable' | 'manual';

/** Calculation settings (§18.2.2 `calcPr`). */
export interface CalcSettings {
  mode: CalcMode;
  iterate: boolean;
  iterateCount: number;
  iterateDelta: number;
  /** `false` = precision as displayed. */
  fullPrecision: boolean;
  calcOnSave: boolean;
  fullCalcOnLoad: boolean;
}

/** Result of `calcOptions` / `setCalcOptions` / `calculate`. */
export interface CalcState {
  options: CalcSettings;
  /** Results are out of date (manual mode, data tables waiting). */
  needsCalc: boolean;
  /** Cells in circular references. */
  circular: (AuditCell & { label: string })[];
}

/** Answer of `checkFormula`: does the text parse and are its functions known. */
export interface FormulaCheck {
  /** Parses and calls only built-in functions or defined names. */
  ok: boolean;
  /** The syntax error (`pos`: byte offset in the text). */
  error: { message: string; pos: number } | null;
  /** Functions the formula calls (upper case, in order of first use). */
  functions: string[];
  unknownFunctions: string[];
}

/** Auditing and calculation commands. */
export type AuditCommand =
  /** Precedent (`dependents` false) or dependent arrows, `levels` deep → {@link TraceArrows}. */
  | {
      cmd: 'traceArrows';
      sheet: number;
      row: number;
      col: number;
      dependents?: boolean;
      levels?: number;
    }
  /** Step-by-step evaluation → {@link EvalTrace} or `null`. */
  | { cmd: 'evaluateFormula'; sheet: number; row: number; col: number }
  /** Syntax check of a formula text; nothing is written or calculated → {@link FormulaCheck}. */
  | { cmd: 'checkFormula'; formula: string }
  /** Findings of a sheet → {@link ErrorFinding}[]. */
  | { cmd: 'errorCheck'; sheet: number; rules?: Partial<ErrorRules> }
  /** Ignore a finding (saved with the sheet); one undo step. */
  | { cmd: 'ignoreError'; sheet: number; row: number; col: number; rule: ErrorRule }
  /** Forget every ignored finding; one undo step. */
  | { cmd: 'resetIgnoredErrors' }
  /** Watch-window rows → {@link WatchRow}[]. */
  | { cmd: 'watchValues'; cells: AuditCell[] }
  /** → {@link CalcState}. */
  | { cmd: 'calcOptions' }
  /** Change the settings; one undo step → {@link CalcState}. */
  | { cmd: 'setCalcOptions'; options: CalcSettings }
  /** Calculate now / the sheet only / everything (`full`) → {@link CalcState}. */
  | { cmd: 'calculate'; sheet?: number; full?: boolean };
