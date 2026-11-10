/** Syntax tree of the VBA subset understood by the converter (CORE-SCRIPT-003). */

/** Why a construct could not be converted (UI text: `script.vba.reason.<code>`). */
export type VbaWarningReason =
  /** Statement kind outside the supported subset (file I/O, `Type`, `Property` …). */
  | 'unsupportedStatement'
  /** Expression form outside the subset (`New`, `TypeOf`, named arguments of a procedure …). */
  | 'unsupportedExpression'
  /** Name that is neither declared nor part of the mapped object model. */
  | 'unknownName'
  /** Member of a known object with no BlueScript equivalent. */
  | 'unknownMember'
  /** `On Error` / `Resume`: rewrite with `try` / `catch`. */
  | 'errorHandling'
  /** `GoTo`, `GoSub`, labels. */
  | 'jump'
  /** `ByRef` parameter: BlueScript passes values. */
  | 'byRef'
  /** The code could not be parsed. */
  | 'syntax'
  /** Dialogs that wait for the user (`InputBox`, `MsgBox` result). */
  | 'interactive'
  /** Host switches with no meaning in BlueScript (`ScreenUpdating`, `DoEvents` …). */
  | 'notNeeded';

export type Expr =
  | { t: 'num'; value: string }
  | { t: 'str'; value: string }
  | { t: 'date'; text: string }
  | { t: 'name'; name: string }
  /** `obj.name`; `obj` null inside a `With` block (`.name`). */
  | { t: 'member'; obj: Expr | null; name: string }
  | { t: 'call'; callee: Expr; args: Arg[] }
  | { t: 'unary'; op: '-' | '+' | 'not'; expr: Expr }
  | { t: 'binary'; op: string; left: Expr; right: Expr }
  /** Not parseable or not supported. */
  | { t: 'bad'; reason: VbaWarningReason; what: string };

export interface Arg {
  /** `name:=value`. */
  name?: string;
  /** Null: omitted (`f a, , c`). */
  value: Expr | null;
}

export interface VarDecl {
  name: string;
  type?: string;
  array: boolean;
  /** Number of declared dimensions. */
  dims: number;
}

export interface Param {
  name: string;
  type?: string;
  optional: boolean;
  byRef: boolean;
  paramArray: boolean;
  array: boolean;
  default?: Expr;
}

export type CaseTest =
  | { k: 'eq'; value: Expr }
  | { k: 'range'; lo: Expr; hi: Expr }
  | { k: 'is'; op: string; value: Expr };

interface Base {
  /** 1-based source line. */
  line: number;
  /** Source of the statement (for a block: its header line). */
  text: string;
}

export interface Branch extends Base {
  /** Null: `Else`. */
  cond: Expr | null;
  body: Stmt[];
}

export interface CaseClause extends Base {
  /** Null: `Case Else`. */
  tests: CaseTest[] | null;
  body: Stmt[];
}

export type Stmt = Base &
  (
    | { s: 'comment'; comment: string }
    | { s: 'unsupported'; reason: VbaWarningReason; construct: string }
    | { s: 'dim'; vars: VarDecl[] }
    | { s: 'const'; name: string; value: Expr }
    | { s: 'redim'; preserve: boolean; items: { name: string; bound: Expr | null; dims: number }[] }
    | { s: 'assign'; target: Expr; value: Expr; set: boolean }
    | { s: 'call'; expr: Expr }
    | { s: 'print'; args: Expr[] }
    | { s: 'if'; branches: Branch[] }
    | { s: 'for'; variable: string; from: Expr; to: Expr; step: Expr | null; body: Stmt[] }
    | { s: 'forEach'; variable: string; collection: Expr; body: Stmt[] }
    | {
        s: 'loop';
        /** Null: `Do … Loop` without a condition. */
        cond: Expr | null;
        until: boolean;
        /** Condition tested after the body (`Loop While`). */
        post: boolean;
        body: Stmt[];
      }
    | { s: 'select'; subject: Expr; cases: CaseClause[]; lead: Stmt[] }
    | { s: 'with'; target: Expr; body: Stmt[] }
    | { s: 'exit'; what: 'sub' | 'function' | 'for' | 'do' | 'property' }
    | {
        s: 'proc';
        kind: 'sub' | 'function';
        name: string;
        params: Param[];
        returnType?: string;
        body: Stmt[];
      }
  );
