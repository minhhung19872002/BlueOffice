/**
 * Code generator of the VBA → BlueScript draft converter (CORE-SCRIPT-003).
 *
 * Turns the syntax tree of `parser.ts` into BlueScript (JavaScript) source for
 * one application. The object-model idioms that have an equivalent in the
 * BlueScript API (`bluescript.d.ts`) are mapped; everything else is emitted as a
 * `// TODO(VBA): <original line>` comment and reported as a warning — nothing
 * is dropped silently, and the output always parses as JavaScript.
 */
import type { ScriptApp } from '../sandbox';
import type { Arg, Expr, Param, Stmt, VbaWarningReason } from './ast';

export interface VbaWarning {
  /** 1-based line of the VBA source. */
  line: number;
  /** The construct that could not be converted (`Application.ScreenUpdating`, `GoTo` …). */
  construct: string;
  reason: VbaWarningReason;
  /** The source line, as written in the `TODO(VBA)` comment. */
  source: string;
}

/** Marker of every line the converter could not translate. */
export const TODO_MARK = 'TODO(VBA):';

type Kind =
  // BlueSheet
  | 'workbook'
  | 'sheets'
  | 'sheet'
  | 'range'
  | 'interior'
  // BlueDoc
  | 'doc'
  | 'paragraphs'
  | 'paragraph'
  | 'paraRange'
  | 'docContent'
  | 'docWords'
  | 'docSel'
  | 'docFind'
  // BlueSlide
  | 'presentation'
  | 'slides'
  | 'slide'
  | 'shapes'
  | 'shape'
  | 'textFrame'
  | 'textRange'
  | 'fill'
  | 'line'
  | 'foreColor'
  // shared
  | 'font'
  | 'app';

/** An emitted expression. */
interface Val {
  code: string;
  /** JavaScript precedence of `code` (20: primary … 2: conditional). */
  prec: number;
  kind?: Kind;
  /** Statically a string. */
  str?: boolean;
  /** A cell value: null when the cell is empty (VBA `Empty`: 0 in sums, "" in text). */
  nullable?: boolean;
  /** Already a `#RRGGBB` colour. */
  color?: boolean;
  /** Object this one belongs to (`Font` → its range …). */
  owner?: Val;
  /** A sub-object with no BlueScript value of its own (`Font`, `Selection` …). */
  virtual?: boolean;
}

interface VarInfo {
  js: string;
  kind?: Kind;
  /** Kind fixed by the declared type. */
  typed: boolean;
  array: boolean;
  /** Declared where it stands (`const`), not hoisted. */
  inPlace: boolean;
  init?: string;
  param: boolean;
  /** Its in-place `const` line was written. */
  declared?: boolean;
}

interface ProcInfo {
  js: string;
  kind: 'sub' | 'function';
  params: Param[];
}

class Unmappable extends Error {
  constructor(
    readonly reason: VbaWarningReason,
    readonly construct: string,
  ) {
    super(construct);
  }
}

const unmappable = (reason: VbaWarningReason, construct: string): never => {
  throw new Unmappable(reason, construct);
};

const P = { primary: 20, postfix: 16, unary: 15, pow: 14, mul: 13, add: 12, rel: 10, eq: 9 };

const PLACEHOLDER = `undefined /* ${TODO_MARK} condition */`;

/** Words that cannot be used as a JavaScript identifier in a script. */
const RESERVED = new Set(
  (
    'break case catch class const continue debugger default delete do else enum export extends ' +
    'false finally for function if import in instanceof new null return super switch this throw ' +
    'true try typeof var void while with yield let static implements interface package private ' +
    'protected public await async arguments eval undefined NaN Infinity ' +
    'sheet workbook doc presentation console Math String Number Boolean Array Date JSON Object ' +
    'Promise Error globalThis'
  ).split(' '),
);

/** Names of the VBA host object models: never implicit variables. */
const HOST_ROOTS = new Set([
  'application',
  'activesheet',
  'activeworkbook',
  'thisworkbook',
  'worksheets',
  'sheets',
  'range',
  'cells',
  'rows',
  'columns',
  'selection',
  'activecell',
  'activewindow',
  'activedocument',
  'thisdocument',
  'documents',
  'activepresentation',
  'presentations',
  'workbooks',
  'debug',
  'me',
]);

/** `Application` members that only tune the VBA host. */
const NOT_NEEDED = new Set([
  'screenupdating',
  'displayalerts',
  'enableevents',
  'calculation',
  'statusbar',
  'cutcopymode',
  'displaystatusbar',
  'calculate',
]);

const HELPERS: Record<string, { needs?: string[]; code: string }> = {
  vbaRgb: {
    code: 'const vbaRgb = (r, g, b) => "#" + [r, g, b].map((v) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0")).join("");',
  },
  vbaColor: {
    needs: ['vbaRgb'],
    code: 'const vbaColor = (c) => (typeof c === "number" ? vbaRgb(c & 255, (c >> 8) & 255, (c >> 16) & 255) : String(c));',
  },
  vbaMid: {
    code: 'const vbaMid = (s, start, length) => String(s).slice(start - 1, length === undefined ? undefined : start - 1 + length);',
  },
  vbaRight: { code: 'const vbaRight = (s, n) => (n > 0 ? String(s).slice(-n) : "");' },
  vbaInStr: {
    code: 'const vbaInStr = (...a) => (a.length >= 3 ? String(a[1]).indexOf(String(a[2]), a[0] - 1) + 1 : String(a[0]).indexOf(String(a[1])) + 1);',
  },
  vbaRound: {
    code: 'const vbaRound = (x, digits = 0) => Math.round(x * 10 ** digits) / 10 ** digits;',
  },
  vbaIsNumeric: {
    code: 'const vbaIsNumeric = (v) => v !== null && v !== "" && !Number.isNaN(Number(v));',
  },
  vbaVal: {
    code: 'const vbaVal = (s) => { const n = parseFloat(String(s)); return Number.isNaN(n) ? 0 : n; };',
  },
  vbaKey: { code: 'const vbaKey = (k) => (typeof k === "number" ? k - 1 : k);' },
  vbaItem: {
    code: 'const vbaItem = (list, key) => (typeof key === "number" ? list[key - 1] : list.find((x) => x.name === key));',
  },
};

const COLORS: Record<string, string> = {
  vbblack: '#000000',
  vbred: '#ff0000',
  vbgreen: '#00ff00',
  vbyellow: '#ffff00',
  vbblue: '#0000ff',
  vbmagenta: '#ff00ff',
  vbcyan: '#00ffff',
  vbwhite: '#ffffff',
};

const TRUE_NAMES = new Set([
  'true',
  'msotrue',
  'msoctrue',
  'wdunderlinesingle',
  'xlunderlinestylesingle',
]);
const FALSE_NAMES = new Set(['false', 'msofalse', 'wdunderlinenone', 'xlunderlinestylenone']);

const H_ALIGN: Record<string, string> = {
  xlleft: 'left',
  xlcenter: 'center',
  xlright: 'right',
  xljustify: 'justify',
  xlgeneral: 'general',
  xlfill: 'fill',
};
const V_ALIGN: Record<string, string> = { xltop: 'top', xlcenter: 'center', xlbottom: 'bottom' };
const P_ALIGN: Record<string, string> = {
  wdalignparagraphleft: 'left',
  wdalignparagraphcenter: 'center',
  wdalignparagraphright: 'right',
  wdalignparagraphjustify: 'justify',
};
/** Preset geometry names of ECMA-376 Part 1, §20.1.10.56 ST_ShapeType. */
const SHAPE_PRESETS: Record<string, string> = {
  msoshaperectangle: 'rect',
  msoshapeoval: 'ellipse',
  msoshaperoundedrectangle: 'roundRect',
  msoshaperightarrow: 'rightArrow',
  msoshapeleftarrow: 'leftArrow',
  msoshapeisoscelestriangle: 'triangle',
  msoshapediamond: 'diamond',
};

const NUMERIC_TYPES = new Set([
  'integer',
  'long',
  'longlong',
  'longptr',
  'single',
  'double',
  'currency',
  'byte',
  'decimal',
]);

/** Text safe inside a `//` comment. */
const clean = (s: string) => s.replace(/[\r\n\u2028\u2029]+/g, ' ');

const num = (e: Expr | null | undefined): number | null => {
  if (!e) return null;
  if (e.t === 'num') return Number(e.value);
  if (e.t === 'unary' && e.op === '-' && e.expr.t === 'num') return -Number(e.expr.value);
  return null;
};

const lowerName = (e: Expr | null | undefined): string =>
  e && e.t === 'name' ? e.name.toLowerCase() : '';

export class Emitter {
  readonly warnings: VbaWarning[] = [];
  private out: string[] = [];
  private indent = 0;
  private readonly procs = new Map<string, ProcInfo>();
  private readonly procNames = new Set<string>();
  private readonly moduleVars = new Map<string, VarInfo>();
  private locals: Map<string, VarInfo> | null = null;
  private fn: { lower: string; result: string } | null = null;
  private loops: ('for' | 'do')[] = [];
  private withs: (Val | null)[] = [];
  private readonly helpers = new Set<string>();
  private tmp = 0;

  constructor(private readonly app: ScriptApp) {}

  // ------------------------------------------------------------ module

  module(stmts: Stmt[]): { code: string; procedures: string[] } {
    this.registerProcs(stmts);
    this.collect(stmts, this.moduleVars, 0);
    const hoisted = this.hoists(this.moduleVars);
    this.block(stmts);
    const runnable = stmts.some(
      (s) => !['proc', 'dim', 'const', 'comment', 'unsupported'].includes(s.s),
    );
    const procedures = stmts.flatMap((s) => (s.s === 'proc' ? [s.name] : []));
    if (!runnable) {
      // A module only declares procedures: the draft calls the first macro that takes no argument.
      const entry = stmts.find(
        (s) =>
          s.s === 'proc' && s.kind === 'sub' && s.params.every((p) => p.optional || p.paramArray),
      );
      if (entry && entry.s === 'proc') {
        this.line('');
        this.line(`await ${this.procs.get(entry.name.toLowerCase())!.js}();`);
      }
    }
    const head: string[] = [];
    const added = new Set<string>();
    const need = (name: string) => {
      if (added.has(name)) return;
      const h = HELPERS[name];
      if (!h) return;
      h.needs?.forEach(need);
      added.add(name);
      head.push(h.code);
    };
    [...this.helpers].sort().forEach(need);
    return { code: [...head, ...hoisted, ...this.out].join('\n') + '\n', procedures };
  }

  private registerProcs(stmts: Stmt[]): void {
    for (const s of stmts) {
      if (s.s === 'proc') {
        const lower = s.name.toLowerCase();
        // A second procedure of the same name (malformed module) gets its own name.
        const dup = this.procs.has(lower);
        const js = dup ? `${this.jsName(s.name)}_${s.line}` : this.jsName(s.name);
        this.procNames.add(js);
        this.procs.set(dup ? `${lower}#${s.line}` : lower, {
          js,
          kind: s.kind,
          params: s.params,
        });
        this.registerProcs(s.body);
      } else if (s.s === 'if') s.branches.forEach((b) => this.registerProcs(b.body));
      else if (s.s === 'select') s.cases.forEach((c) => this.registerProcs(c.body));
      else if ('body' in s) this.registerProcs(s.body);
    }
  }

  private jsName(name: string): string {
    let n = name.replace(/[^\p{L}\p{Nd}_$]/gu, '_');
    if (!/^[\p{L}_$]/u.test(n)) n = `_${n}`;
    if (RESERVED.has(n) || n in HELPERS || /^(with|sel|end)\d+$/.test(n)) n += '_';
    return n;
  }

  /** Name of a variable: never the name of a generated function. */
  private varName(name: string): string {
    let n = this.jsName(name);
    while (this.procNames.has(n) || this.procNames.has(n.replace(/Result$/, ''))) n += '_';
    return n;
  }

  private kindOfType(type: string | undefined): Kind | undefined {
    const t = (type ?? '').toLowerCase().split('.').pop() ?? '';
    if (this.app === 'sheet') {
      if (t === 'worksheet') return 'sheet';
      if (t === 'range') return 'range';
      if (t === 'workbook') return 'workbook';
    } else if (this.app === 'doc') {
      if (t === 'document') return 'doc';
      if (t === 'paragraph') return 'paragraph';
    } else {
      if (t === 'presentation') return 'presentation';
      if (t === 'slide') return 'slide';
      if (t === 'shape') return 'shape';
    }
    return undefined;
  }

  private varInfo(
    name: string,
    type: string | undefined,
    array: boolean,
    inPlace = false,
  ): VarInfo {
    const t = (type ?? '').toLowerCase();
    const kind = array ? undefined : this.kindOfType(type);
    let init: string | undefined;
    if (array) init = '[]';
    else if (NUMERIC_TYPES.has(t)) init = '0';
    else if (t === 'string') init = '""';
    else if (t === 'boolean') init = 'false';
    return {
      js: this.varName(name),
      typed: !!kind,
      array,
      inPlace,
      param: false,
      ...(kind ? { kind } : {}),
      ...(init ? { init } : {}),
    };
  }

  private lookup(lower: string): VarInfo | undefined {
    return this.locals?.get(lower) ?? this.moduleVars.get(lower);
  }

  /** Declares the variables of a scope: `Dim`, `Const`, `ReDim` and names only assigned to. */
  private collect(stmts: Stmt[], into: Map<string, VarInfo>, depth: number): void {
    const known = (lower: string) =>
      into.has(lower) ||
      this.moduleVars.has(lower) ||
      this.procs.has(lower) ||
      HOST_ROOTS.has(lower) ||
      lower === '';
    const implicit = (name: string, array = false) => {
      const lower = name.toLowerCase();
      if (!known(lower)) into.set(lower, this.varInfo(name, undefined, array));
    };
    for (const s of stmts) {
      switch (s.s) {
        case 'dim':
          for (const v of s.vars) {
            if (!into.has(v.name.toLowerCase()))
              into.set(v.name.toLowerCase(), this.varInfo(v.name, v.type, v.array));
          }
          break;
        case 'const':
          if (!into.has(s.name.toLowerCase()))
            into.set(s.name.toLowerCase(), this.varInfo(s.name, undefined, false, depth === 0));
          break;
        case 'redim':
          s.items.forEach((i) => implicit(i.name, true));
          break;
        case 'assign':
          if (s.target.t === 'name') implicit(s.target.name);
          break;
        case 'for':
        case 'forEach':
          implicit(s.variable);
          this.collect(s.body, into, depth + 1);
          break;
        case 'if':
          s.branches.forEach((b) => this.collect(b.body, into, depth + 1));
          break;
        case 'select':
          s.cases.forEach((c) => this.collect(c.body, into, depth + 1));
          break;
        case 'loop':
        case 'with':
          this.collect(s.body, into, depth + 1);
          break;
        default:
          break;
      }
    }
  }

  private hoists(vars: Map<string, VarInfo>): string[] {
    return [...vars.values()]
      .filter((v) => !v.inPlace && !v.param)
      .map((v) => `let ${v.js}${v.init ? ` = ${v.init}` : ''};`);
  }

  // ------------------------------------------------------------ output

  private line(text: string): void {
    this.out.push(text ? '  '.repeat(this.indent) + text : '');
  }

  private todo(at: { line: number; text: string }, reason: VbaWarningReason, construct: string) {
    const source = clean(at.text);
    this.line(`// ${TODO_MARK} ${source}`);
    this.warnings.push({ line: at.line, construct: clean(construct), reason, source });
  }

  /** Runs `make`; a construct it cannot map becomes a TODO line and `fallback`. */
  private attempt<T>(at: { line: number; text: string }, make: () => T, fallback: T): T {
    try {
      return make();
    } catch (e) {
      if (!(e instanceof Unmappable)) throw e;
      this.todo(at, e.reason, e.construct);
      return fallback;
    }
  }

  // ------------------------------------------------------------ statements

  private block(stmts: Stmt[]): void {
    for (const s of stmts) this.statement(s);
  }

  private statement(st: Stmt): void {
    switch (st.s) {
      case 'comment':
        this.line(`//${clean(st.comment)}`);
        return;
      case 'unsupported':
        this.todo(st, st.reason, st.construct);
        return;
      case 'dim':
        return; // hoisted to the top of the scope
      case 'const': {
        const v = this.lookup(st.name.toLowerCase());
        if (!v) return;
        const code = this.attempt(st, () => this.val(st.value).code, null);
        if (v.inPlace && v.declared) {
          this.todo(st, 'syntax', `Const ${st.name}`);
        } else if (code === null) {
          if (v.inPlace) this.line(`let ${v.js};`);
        } else this.line(v.inPlace ? `const ${v.js} = ${code};` : `${v.js} = ${code};`);
        v.declared = true;
        return;
      }
      case 'redim':
      case 'assign':
      case 'call':
      case 'print':
      case 'exit': {
        const lines = this.attempt(st, () => this.simple(st), []);
        lines.forEach((l) => this.line(l));
        return;
      }
      case 'if':
        st.branches.forEach((b, i) => {
          const cond =
            b.cond === null ? null : this.attempt(b, () => this.val(b.cond!).code, PLACEHOLDER);
          if (i === 0) this.line(`if (${cond ?? 'true'}) {`);
          else this.line(cond === null ? '} else {' : `} else if (${cond}) {`);
          this.nested(b.body);
        });
        this.line('}');
        return;
      case 'for':
        this.forLoop(st);
        return;
      case 'forEach':
        this.forEach(st);
        return;
      case 'loop': {
        const raw = st.cond === null ? 'true' : this.attempt(st, () => this.val(st.cond!), null);
        let cond: string;
        if (raw === null) cond = PLACEHOLDER;
        else if (typeof raw === 'string') cond = raw;
        else cond = st.until ? `!${this.c(raw, P.unary)}` : raw.code;
        this.line(st.post ? 'do {' : `while (${cond}) {`);
        this.loops.push('do');
        this.nested(st.body);
        this.loops.pop();
        this.line(st.post ? `} while (${cond});` : '}');
        return;
      }
      case 'select':
        this.select(st);
        return;
      case 'with': {
        this.line('{');
        this.indent++;
        const target = this.attempt(
          st,
          (): Val | null => {
            const t = this.expr(st.target);
            if (t.virtual) return t;
            const name = `with${++this.tmp}`;
            this.line(`const ${name} = ${t.code};`);
            return {
              code: name,
              prec: P.primary,
              ...(t.kind ? { kind: t.kind } : {}),
              ...(t.owner ? { owner: t.owner } : {}),
            };
          },
          null,
        );
        this.withs.push(target);
        this.block(st.body);
        this.withs.pop();
        this.indent--;
        this.line('}');
        return;
      }
      case 'proc':
        this.proc(st);
        return;
      default:
        return;
    }
  }

  private nested(body: Stmt[]): void {
    this.indent++;
    this.block(body);
    this.indent--;
  }

  private forLoop(st: Extract<Stmt, { s: 'for' }>): void {
    const head = this.attempt(
      st,
      () => {
        const v = this.lookup(st.variable.toLowerCase());
        if (!v || st.variable === '') return unmappable('syntax', 'For');
        const from = this.val(st.from).code;
        const to = this.val(st.to);
        const stepN = st.step === null ? 1 : num(st.step);
        const pre: string[] = [];
        let end = this.c(to, P.rel + 1);
        if (to.code.includes('await')) {
          // The bound is evaluated once, as in VBA.
          end = `end${++this.tmp}`;
          pre.push(`const ${end} = ${to.code};`);
        }
        let test: string;
        let inc: string;
        if (stepN !== null) {
          test = `${v.js} ${stepN < 0 ? '>=' : '<='} ${end}`;
          inc = stepN === 1 ? `${v.js}++` : stepN === -1 ? `${v.js}--` : `${v.js} += ${stepN}`;
        } else {
          const step = this.val(st.step!);
          const s = this.c(step, P.add + 1);
          test = `${s} >= 0 ? ${v.js} <= ${end} : ${v.js} >= ${end}`;
          inc = `${v.js} += ${s}`;
        }
        return { pre, open: `for (${v.js} = ${from}; ${test}; ${inc}) {` };
      },
      { pre: [], open: `for (const _ of [] /* ${TODO_MARK} loop */) {` },
    );
    const wrap = head.pre.length > 0;
    if (wrap) {
      this.line('{');
      this.indent++;
      head.pre.forEach((l) => this.line(l));
    }
    this.line(head.open);
    this.loops.push('for');
    this.nested(st.body);
    this.loops.pop();
    this.line('}');
    if (wrap) {
      this.indent--;
      this.line('}');
    }
  }

  private forEach(st: Extract<Stmt, { s: 'forEach' }>): void {
    const ELEMENT: Partial<Record<Kind, Kind>> = {
      sheets: 'sheet',
      paragraphs: 'paragraph',
      slides: 'slide',
      shapes: 'shape',
    };
    const open = this.attempt(
      st,
      () => {
        const v = this.lookup(st.variable.toLowerCase());
        if (!v || st.variable === '') return unmappable('syntax', 'For Each');
        const coll = this.expr(st.collection);
        if (coll.virtual) return unmappable('unknownMember', `For Each … In ${coll.kind ?? ''}`);
        const element = coll.kind ? ELEMENT[coll.kind] : undefined;
        if (coll.kind && !element) return unmappable('unknownMember', `For Each … In ${coll.kind}`);
        if (!v.typed) {
          if (element) v.kind = element;
          else delete v.kind;
        }
        return `for (${v.js} of ${coll.code}) {`;
      },
      `for (const _ of [] /* ${TODO_MARK} loop */) {`,
    );
    this.line(open);
    this.loops.push('for');
    this.nested(st.body);
    this.loops.pop();
    this.line('}');
  }

  private select(st: Extract<Stmt, { s: 'select' }>): void {
    this.line('{');
    this.indent++;
    const sel = `sel${++this.tmp}`;
    const subject = this.attempt(st, () => this.val(st.subject).code, PLACEHOLDER);
    this.line(`const ${sel} = ${subject};`);
    this.block(st.lead);
    const rel: Record<string, string> = {
      '=': '==',
      '<>': '!=',
      '<': '<',
      '>': '>',
      '<=': '<=',
      '>=': '>=',
    };
    st.cases.forEach((c, i) => {
      const cond =
        c.tests === null
          ? null
          : this.attempt(
              c,
              () => {
                const parts = c.tests!.map((t) => {
                  if (t.k === 'eq') return `${sel} == ${this.c(this.val(t.value), P.rel)}`;
                  if (t.k === 'range')
                    return `${sel} >= ${this.c(this.val(t.lo), P.rel + 1)} && ${sel} <= ${this.c(this.val(t.hi), P.rel + 1)}`;
                  const op = rel[t.op] ?? unmappable('syntax', `Case Is ${t.op}`);
                  return `${sel} ${op} ${this.c(this.val(t.value), P.rel + 1)}`;
                });
                return parts.length === 1 ? parts[0]! : parts.map((p) => `(${p})`).join(' || ');
              },
              PLACEHOLDER,
            );
      if (i === 0) this.line(`if (${cond ?? 'true'}) {`);
      else this.line(cond === null ? '} else {' : `} else if (${cond}) {`);
      this.nested(c.body);
    });
    if (st.cases.length) this.line('}');
    this.indent--;
    this.line('}');
  }

  private proc(st: Extract<Stmt, { s: 'proc' }>): void {
    const info =
      this.procs.get(`${st.name.toLowerCase()}#${st.line}`) ??
      this.procs.get(st.name.toLowerCase());
    if (!info) return;
    const saved = { locals: this.locals, fn: this.fn, loops: this.loops, withs: this.withs };
    const locals = new Map<string, VarInfo>();
    this.locals = locals;
    this.loops = [];
    this.withs = [];
    const params: string[] = [];
    for (const p of st.params) {
      const v = { ...this.varInfo(p.name, p.type, p.array), param: true };
      if (locals.has(p.name.toLowerCase())) v.js = `${v.js}_${params.length}`;
      locals.set(p.name.toLowerCase(), v);
      if (p.byRef)
        this.todo({ line: st.line, text: `ByRef ${p.name}` }, 'byRef', `ByRef ${p.name}`);
      if (p.paramArray) {
        params.push(`...${v.js}`);
        continue;
      }
      let def: string | null = null;
      if (p.default) {
        def = this.attempt(
          st,
          () => {
            const d = this.val(p.default!).code;
            return d.includes('await')
              ? unmappable('unsupportedExpression', `Optional ${p.name}`)
              : d;
          },
          null,
        );
      }
      params.push(def === null ? v.js : `${v.js} = ${def}`);
    }
    this.fn =
      st.kind === 'function' ? { lower: st.name.toLowerCase(), result: `${info.js}Result` } : null;
    this.collect(st.body, locals, 0);
    this.line(`async function ${info.js}(${params.join(', ')}) {`);
    this.indent++;
    this.hoists(locals).forEach((l) => this.line(l));
    if (this.fn) {
      const init = this.varInfo('x', st.returnType, false).init;
      this.line(`let ${this.fn.result}${init ? ` = ${init}` : ''};`);
    }
    this.block(st.body);
    if (this.fn) this.line(`return ${this.fn.result};`);
    this.indent--;
    this.line('}');
    Object.assign(this, saved);
  }

  /** Lines of a statement without nested statements; throws {@link Unmappable}. */
  private simple(st: Stmt): string[] {
    switch (st.s) {
      case 'print':
        return [`console.log(${st.args.map((a) => this.val(a).code).join(', ')});`];
      case 'exit': {
        if (st.what === 'for' || st.what === 'do') {
          const top = this.loops[this.loops.length - 1];
          if (top !== st.what)
            return unmappable('unsupportedStatement', `Exit ${st.what === 'for' ? 'For' : 'Do'}`);
          return ['break;'];
        }
        if (st.what === 'property') return unmappable('unsupportedStatement', 'Exit Property');
        return [this.fn ? `return ${this.fn.result};` : 'return;'];
      }
      case 'redim':
        return st.items.map((item) => {
          const v = this.lookup(item.name.toLowerCase());
          if (!v) return unmappable('unknownName', item.name);
          if (item.dims > 1) return unmappable('unsupportedExpression', 'multi-dimensional array');
          if (st.preserve && item.bound) {
            return `${v.js}.length = ${this.c(this.val(item.bound), P.add)} + 1;`;
          }
          return `${v.js} = [];`;
        });
      case 'assign':
        return [this.assign(st)];
      case 'call':
        return [this.callStatement(st.expr)];
      default:
        return [];
    }
  }

  private assign(st: Extract<Stmt, { s: 'assign' }>): string {
    const { target } = st;
    const rhs = (): Val => {
      const v = st.set ? this.expr(st.value) : this.val(st.value);
      if (v.virtual) return unmappable('unknownMember', v.kind ?? 'object');
      return v;
    };
    if (target.t === 'name') {
      const lower = target.name.toLowerCase();
      const v = this.lookup(lower);
      if (v) {
        const value = rhs();
        if (!v.typed) {
          if (value.kind) v.kind = value.kind;
          else delete v.kind;
        }
        return `${v.js} = ${value.code};`;
      }
      if (this.fn && lower === this.fn.lower) return `${this.fn.result} = ${rhs().code};`;
    }
    if (target.t === 'call' && target.callee.t === 'name') {
      const v = this.lookup(target.callee.name.toLowerCase());
      if (v && !v.kind) return `${v.js}[${this.index(target.args)}] = ${rhs().code};`;
    }
    if (target.t === 'member') {
      const owner = target.obj ? this.expr(target.obj) : this.withTarget();
      return this.setter(owner, target.name, st.value);
    }
    // Default member: `Range("A1") = 5`.
    const t = this.expr(target);
    if (t.kind === 'range') return `await ${this.obj(t)}.setValue(${this.val(st.value).code});`;
    return unmappable('unknownMember', t.kind ?? 'assignment');
  }

  private callStatement(e: Expr): string {
    const callee = e.t === 'call' ? e.callee : e;
    const args = e.t === 'call' ? e.args : [];
    if (callee.t === 'name') {
      const lower = callee.name.toLowerCase();
      if (lower === 'msgbox' && !this.lookup(lower) && !this.procs.has(lower)) {
        // No dialogs in a script: the message goes to the output console.
        const prompt =
          args.find((a) => a.name?.toLowerCase() === 'prompt') ?? args.find((a) => !a.name);
        if (!prompt?.value) return unmappable('syntax', 'MsgBox');
        return `console.log(${this.val(prompt.value).code});`;
      }
    }
    const v = this.expr(e);
    if (v.virtual || !v.code.startsWith('await ')) {
      return unmappable(
        'unsupportedStatement',
        v.kind ?? (callee.t === 'name' ? callee.name : 'expression'),
      );
    }
    return `${v.code};`;
  }

  // ------------------------------------------------------------ expressions

  /** `code` of `v`, in parentheses when it binds looser than `min`. */
  private c(v: Val, min: number): string {
    return v.prec >= min ? v.code : `(${v.code})`;
  }

  /** `v` as the object of a member access. */
  private obj(v: Val): string {
    if (v.virtual) return unmappable('unknownMember', v.kind ?? 'object');
    return this.c(v, P.primary);
  }

  /** Value of an expression: a range reads its (top-left) cell value. */
  private val(e: Expr): Val {
    return this.read(this.expr(e));
  }

  private read(v: Val): Val {
    if (v.virtual) return unmappable('unknownMember', v.kind ?? 'object');
    if (v.kind === 'range') {
      return { code: `(await ${this.obj(v)}.values)[0][0]`, prec: P.primary, nullable: true };
    }
    return v;
  }

  /** `v` where text is expected: an empty cell reads as "". */
  private text(v: Val, min: number): string {
    return v.nullable ? `(${v.code} ?? "")` : this.c(v, min);
  }

  private withTarget(): Val {
    if (this.withs.length === 0) return unmappable('syntax', '.member');
    return this.withs[this.withs.length - 1] ?? unmappable('unknownName', 'With');
  }

  private positional(args: Arg[] | null, what: string): Expr[] {
    return (args ?? []).map((a) => {
      if (a.name !== undefined) return unmappable('unsupportedExpression', `${what}(${a.name}:=…)`);
      return a.value ?? unmappable('unsupportedExpression', `${what}(, )`);
    });
  }

  /** Single array index. */
  private index(args: Arg[]): string {
    const list = this.positional(args, 'array');
    if (list.length !== 1) return unmappable('unsupportedExpression', 'multi-dimensional array');
    return this.val(list[0]!).code;
  }

  /** VBA 1-based index as a 0-based one. */
  private idx0(e: Expr): Val {
    const n = num(e);
    if (n !== null && Number.isInteger(n))
      return { code: String(n - 1), prec: n >= 1 ? P.primary : P.unary };
    return { code: `${this.c(this.val(e), P.add)} - 1`, prec: P.add };
  }

  private helper(name: string): string {
    this.helpers.add(name);
    return name;
  }

  private bool(e: Expr): string {
    const n = lowerName(e);
    if (TRUE_NAMES.has(n) && !this.lookup(n)) return 'true';
    if (FALSE_NAMES.has(n) && !this.lookup(n)) return 'false';
    return `Boolean(${this.val(e).code})`;
  }

  private colorOf(e: Expr): string {
    const v = this.val(e);
    return v.color ? v.code : `${this.helper('vbaColor')}(${v.code})`;
  }

  private enumOf(e: Expr, table: Record<string, string>, what: string): string {
    const found = table[lowerName(e)];
    return found
      ? JSON.stringify(found)
      : unmappable('unknownName', e.t === 'name' ? e.name : what);
  }

  expr(e: Expr): Val {
    switch (e.t) {
      case 'num':
        return { code: e.value, prec: P.primary };
      case 'str':
        return { code: JSON.stringify(e.value), prec: P.primary, str: true };
      case 'date':
        return { code: `new Date(${JSON.stringify(e.text)})`, prec: P.primary };
      case 'bad':
        return unmappable(e.reason, e.what);
      case 'name':
        return this.name(e.name, null);
      case 'member':
        return this.member(e.obj ? this.expr(e.obj) : this.withTarget(), e.name, null);
      case 'call':
        if (e.callee.t === 'name') return this.name(e.callee.name, e.args);
        if (e.callee.t === 'member') {
          const owner = e.callee.obj ? this.expr(e.callee.obj) : this.withTarget();
          return this.member(owner, e.callee.name, e.args);
        }
        return unmappable('unsupportedExpression', '(…)(…)');
      case 'unary': {
        const o = this.val(e.expr);
        if (e.op === '+') return o;
        const s = this.c(o, P.unary);
        if (e.op === 'not') return { code: `!${s}`, prec: P.unary };
        return { code: s.startsWith('-') ? `-(${o.code})` : `-${s}`, prec: P.unary };
      }
      case 'binary':
        return this.binary(e);
      default:
        return unmappable('syntax', 'expression');
    }
  }

  private binary(e: Extract<Expr, { t: 'binary' }>): Val {
    if (e.op === 'is') {
      const l = this.expr(e.left);
      if (l.virtual) return unmappable('unknownMember', l.kind ?? 'object');
      if (lowerName(e.right) === 'nothing')
        return { code: `${this.c(l, P.rel)} == null`, prec: P.eq };
      const r = this.expr(e.right);
      if (r.virtual) return unmappable('unknownMember', r.kind ?? 'object');
      return { code: `${this.c(l, P.rel)} === ${this.c(r, P.rel)}`, prec: P.eq };
    }
    const l = this.val(e.left);
    const r = this.val(e.right);
    const bin = (op: string, prec: number, left = prec, right = prec + 1): Val => ({
      code: `${this.c(l, left)} ${op} ${this.c(r, right)}`,
      prec,
    });
    switch (e.op) {
      case '+':
        return { ...bin('+', P.add), ...(l.str && r.str ? { str: true } : {}) };
      case '-':
        return bin('-', P.add);
      case '*':
        return bin('*', P.mul);
      case '/':
        return bin('/', P.mul);
      case 'mod':
        return bin('%', P.mul);
      case '\\':
        return {
          code: `Math.trunc(${this.c(l, P.mul)} / ${this.c(r, P.mul + 1)})`,
          prec: P.primary,
        };
      case '^':
        // `-a ** b` is not valid JavaScript: the base is parenthesised unless it is a primary.
        return bin('**', P.pow, P.postfix, P.pow);
      case '&': {
        let left: string;
        if (l.str || r.str) left = this.text(l, P.add);
        else left = `String(${l.nullable ? `${l.code} ?? ""` : l.code})`;
        return { code: `${left} + ${this.text(r, P.add + 1)}`, prec: P.add, str: true };
      }
      case '=':
      case '<>': {
        // An empty cell equals "" (VBA `Empty`).
        const side = (v: Val, other: Val) => (other.str ? this.text(v, P.rel) : this.c(v, P.rel));
        return { code: `${side(l, r)} ${e.op === '=' ? '==' : '!='} ${side(r, l)}`, prec: P.eq };
      }
      case '<':
      case '>':
      case '<=':
      case '>=':
        return bin(e.op, P.rel, P.rel + 1, P.rel + 1);
      case 'and':
        return bin('&&', 5);
      case 'or':
        return bin('||', 4);
      case 'xor':
        return { code: `Boolean(${l.code}) !== Boolean(${r.code})`, prec: P.eq };
      default:
        return unmappable('unsupportedExpression', e.op);
    }
  }

  /** A name, alone (`args` null) or applied to arguments. */
  private name(name: string, args: Arg[] | null): Val {
    const lower = name.toLowerCase();
    const v = this.lookup(lower);
    if (v) {
      if (args === null)
        return { code: v.js, prec: P.primary, ...(v.kind ? { kind: v.kind } : {}) };
      if (v.kind) return unmappable('unknownMember', `${name}(…)`);
      return { code: `${v.js}[${this.index(args)}]`, prec: P.primary };
    }
    if (this.fn && lower === this.fn.lower && args === null) {
      return { code: this.fn.result, prec: P.primary };
    }
    const p = this.procs.get(lower);
    if (p) {
      const list = (args ?? []).map((a) => {
        if (a.name !== undefined)
          return unmappable('unsupportedExpression', `${name}(${a.name}:=…)`);
        return a.value ? this.passed(a.value) : 'undefined';
      });
      return { code: `await ${p.js}(${list.join(', ')})`, prec: P.unary };
    }
    if (args === null) {
      const k = this.constant(lower);
      if (k) return k;
    }
    if (lower === 'msgbox' || lower === 'inputbox') return unmappable('interactive', name);
    const f = this.builtin(lower, args);
    if (f) return f;
    return this.root(name, args);
  }

  /** An argument of a user procedure: objects are passed as they are. */
  private passed(e: Expr): string {
    const v = this.expr(e);
    if (v.virtual) return unmappable('unknownMember', v.kind ?? 'object');
    return v.code;
  }

  private constant(lower: string): Val | null {
    switch (lower) {
      case 'true':
      case 'msotrue':
        return { code: 'true', prec: P.primary };
      case 'false':
      case 'msofalse':
        return { code: 'false', prec: P.primary };
      case 'nothing':
      case 'null':
      case 'empty':
        return { code: 'null', prec: P.primary };
      case 'vbcrlf':
      case 'vbnewline':
      case 'vblf':
      case 'vbcr':
        // BlueScript text uses "\n" between lines and paragraphs.
        return { code: '"\\n"', prec: P.primary, str: true };
      case 'vbtab':
        return { code: '"\\t"', prec: P.primary, str: true };
      case 'vbnullstring':
        return { code: '""', prec: P.primary, str: true };
      case 'now':
      case 'date':
      case 'time':
        return { code: 'new Date()', prec: P.primary };
      case 'rnd':
        return { code: 'Math.random()', prec: P.primary };
      default: {
        const color = COLORS[lower];
        return color
          ? { code: JSON.stringify(color), prec: P.primary, str: true, color: true }
          : null;
      }
    }
  }

  /** Functions of the VBA standard library ([MS-VBAL] §6.1.2) with a direct JavaScript form. */
  private builtin(lower: string, args: Arg[] | null): Val | null {
    const known =
      'len left right mid trim ltrim rtrim ucase lcase instr replace str cstr cint clng cdbl csng ' +
      'ccur cbool cdate val int fix abs sqr sgn round isempty isnull isnumeric isarray chr chrw asc ' +
      'ascw year month day hour minute second split join ubound lbound array iif space string ' +
      'strreverse rgb now date time rnd';
    if (!known.split(' ').includes(lower)) return null;
    const a = this.positional(args, lower).map((x) => this.val(x));
    const count = (min: number, max = min) => {
      if (a.length < min || a.length > max) unmappable('unsupportedExpression', `${lower}()`);
    };
    const x = (i: number) => a[i]!.code;
    const s = (i: number) => {
      if (a[i]!.str) return this.c(a[i]!, P.primary);
      return `String(${a[i]!.nullable ? `${x(i)} ?? ""` : x(i)})`;
    };
    const call = (fn: string, str = false): Val => ({
      code: `${fn}(${a.map((v) => v.code).join(', ')})`,
      prec: P.primary,
      ...(str ? { str: true } : {}),
    });
    const prim = (code: string, str = false): Val => ({
      code,
      prec: P.primary,
      ...(str ? { str: true } : {}),
    });
    switch (lower) {
      case 'len':
        count(1);
        return prim(`${s(0)}.length`);
      case 'left':
        count(2);
        return prim(`${s(0)}.slice(0, ${x(1)})`, true);
      case 'right':
        count(2);
        return call(this.helper('vbaRight'), true);
      case 'mid':
        count(2, 3);
        return call(this.helper('vbaMid'), true);
      case 'trim':
        count(1);
        return prim(`${s(0)}.trim()`, true);
      case 'ltrim':
        count(1);
        return prim(`${s(0)}.trimStart()`, true);
      case 'rtrim':
        count(1);
        return prim(`${s(0)}.trimEnd()`, true);
      case 'ucase':
        count(1);
        return prim(`${s(0)}.toUpperCase()`, true);
      case 'lcase':
        count(1);
        return prim(`${s(0)}.toLowerCase()`, true);
      case 'strreverse':
        count(1);
        return prim(`[...${s(0)}].reverse().join("")`, true);
      case 'instr':
        count(2, 3);
        return call(this.helper('vbaInStr'));
      case 'replace':
        count(3);
        return prim(`${s(0)}.replaceAll(${x(1)}, ${x(2)})`, true);
      case 'str':
      case 'cstr':
        count(1);
        return call('String', true);
      case 'cint':
      case 'clng':
        count(1);
        return prim(`Math.round(Number(${x(0)}))`);
      case 'cdbl':
      case 'csng':
      case 'ccur':
        count(1);
        return call('Number');
      case 'cbool':
        count(1);
        return call('Boolean');
      case 'cdate':
        count(1);
        return prim(`new Date(${x(0)})`);
      case 'val':
        count(1);
        return call(this.helper('vbaVal'));
      case 'int':
        count(1);
        return call('Math.floor');
      case 'fix':
        count(1);
        return call('Math.trunc');
      case 'abs':
        count(1);
        return call('Math.abs');
      case 'sqr':
        count(1);
        return call('Math.sqrt');
      case 'sgn':
        count(1);
        return call('Math.sign');
      case 'round':
        count(1, 2);
        return call(this.helper('vbaRound'));
      case 'rnd':
        return prim('Math.random()');
      case 'isempty':
      case 'isnull':
        count(1);
        return prim(`[null, ""].includes(${x(0)})`);
      case 'isnumeric':
        count(1);
        return call(this.helper('vbaIsNumeric'));
      case 'isarray':
        count(1);
        return call('Array.isArray');
      case 'chr':
      case 'chrw':
        count(1);
        return call('String.fromCharCode', true);
      case 'asc':
      case 'ascw':
        count(1);
        return prim(`${s(0)}.charCodeAt(0)`);
      case 'year':
        count(1);
        return prim(`new Date(${x(0)}).getFullYear()`);
      case 'month':
        count(1);
        return { code: `new Date(${x(0)}).getMonth() + 1`, prec: P.add };
      case 'day':
        count(1);
        return prim(`new Date(${x(0)}).getDate()`);
      case 'hour':
        count(1);
        return prim(`new Date(${x(0)}).getHours()`);
      case 'minute':
        count(1);
        return prim(`new Date(${x(0)}).getMinutes()`);
      case 'second':
        count(1);
        return prim(`new Date(${x(0)}).getSeconds()`);
      case 'now':
      case 'date':
      case 'time':
        count(0);
        return prim('new Date()');
      case 'split':
        count(1, 2);
        return prim(`${s(0)}.split(${a[1] ? x(1) : '" "'})`);
      case 'join':
        count(1, 2);
        return prim(`${this.c(a[0]!, P.primary)}.join(${a[1] ? x(1) : '" "'})`, true);
      case 'ubound':
        count(1);
        return { code: `${this.c(a[0]!, P.primary)}.length - 1`, prec: P.add };
      case 'lbound':
        count(1);
        return prim('0');
      case 'array':
        return prim(`[${a.map((v) => v.code).join(', ')}]`);
      case 'iif':
        count(3);
        return { code: `${this.c(a[0]!, 3)} ? ${x(1)} : ${x(2)}`, prec: 2 };
      case 'space':
        count(1);
        return prim(`" ".repeat(${x(0)})`, true);
      case 'string':
        count(2);
        return prim(`${s(1)}.charAt(0).repeat(${x(0)})`, true);
      case 'rgb':
        count(3);
        return { ...call(this.helper('vbaRgb'), true), color: true };
      default:
        return null;
    }
  }

  // ------------------------------------------------------------ object model

  private readonly activeSheet: Val = { code: 'sheet', prec: P.primary, kind: 'sheet' };

  /** A root object of the host object model, for the application of the draft. */
  private root(name: string, args: Arg[] | null): Val {
    const lower = name.toLowerCase();
    const noArgs = () => {
      if (args && args.length) unmappable('unknownMember', `${name}(…)`);
    };
    if (lower === 'application') {
      noArgs();
      return { code: '', prec: P.primary, kind: 'app', virtual: true };
    }
    if (this.app === 'sheet') {
      switch (lower) {
        case 'activesheet':
          noArgs();
          return this.activeSheet;
        case 'activeworkbook':
        case 'thisworkbook':
          noArgs();
          return { code: 'workbook', prec: P.primary, kind: 'workbook' };
        case 'worksheets':
        case 'sheets':
          return this.sheetsOf(args);
        case 'range':
          return this.rangeOf(this.activeSheet, args);
        case 'cells':
          return this.cellOf(this.activeSheet, args);
        default:
          break;
      }
    } else if (this.app === 'doc') {
      switch (lower) {
        case 'activedocument':
        case 'thisdocument':
          noArgs();
          return { code: 'doc', prec: P.primary, kind: 'doc' };
        case 'selection':
          noArgs();
          return { code: '', prec: P.primary, kind: 'docSel', virtual: true };
        default:
          break;
      }
    } else if (lower === 'activepresentation') {
      noArgs();
      return { code: 'presentation', prec: P.primary, kind: 'presentation' };
    }
    return unmappable('unknownName', name);
  }

  private sheetsOf(args: Arg[] | null): Val {
    if (!args || args.length === 0) {
      return { code: 'await workbook.sheets', prec: P.unary, kind: 'sheets' };
    }
    const list = this.positional(args, 'Worksheets');
    if (list.length !== 1) return unmappable('unsupportedExpression', 'Worksheets(…)');
    const key = list[0]!;
    let code: string;
    if (key.t === 'str') code = JSON.stringify(key.value);
    else if (num(key) !== null) code = this.idx0(key).code;
    else code = `${this.helper('vbaKey')}(${this.val(key).code})`;
    return { code: `await workbook.sheet(${code})`, prec: P.unary, kind: 'sheet' };
  }

  private rangeOf(sheet: Val, args: Arg[] | null): Val {
    const list = this.positional(args, 'Range');
    const address = (e: Expr): Val => {
      const v = this.expr(e);
      if (v.kind === 'range') return { code: `${this.obj(v)}.address`, prec: P.primary, str: true };
      return this.read(v);
    };
    if (list.length === 1) {
      return {
        code: `${this.obj(sheet)}.range(${address(list[0]!).code})`,
        prec: P.primary,
        kind: 'range',
      };
    }
    if (list.length === 2) {
      const [a, b] = [list[0]!, list[1]!];
      const code =
        a.t === 'str' && b.t === 'str'
          ? JSON.stringify(`${a.value}:${b.value}`)
          : `${this.c(address(a), P.add)} + ":" + ${this.c(address(b), P.add + 1)}`;
      return { code: `${this.obj(sheet)}.range(${code})`, prec: P.primary, kind: 'range' };
    }
    return unmappable('unsupportedExpression', 'Range(…)');
  }

  private cellOf(sheet: Val, args: Arg[] | null): Val {
    const list = this.positional(args, 'Cells');
    if (list.length !== 2) return unmappable('unknownMember', 'Cells');
    const [row, col] = [list[0]!, list[1]!];
    if (col.t === 'str') {
      // Column given by its letter: `Cells(2, "B")`.
      const r = num(row);
      const code =
        r !== null
          ? JSON.stringify(`${col.value}${r}`)
          : `${JSON.stringify(col.value)} + ${this.c(this.val(row), P.add + 1)}`;
      return { code: `${this.obj(sheet)}.range(${code})`, prec: P.primary, kind: 'range' };
    }
    return {
      code: `${this.obj(sheet)}.cell(${this.idx0(row).code}, ${this.idx0(col).code})`,
      prec: P.primary,
      kind: 'range',
    };
  }

  /** Item `key` (1-based or by name) of a list held in `list`. */
  private item(list: Val, key: Expr, kind: Kind, owner?: Val): Val {
    const n = num(key);
    const code =
      n !== null
        ? `${this.c(list, P.primary)}[${this.idx0(key).code}]`
        : `${this.helper('vbaItem')}(${list.code}, ${this.val(key).code})`;
    return { code, prec: P.primary, kind, ...(owner ? { owner } : {}) };
  }

  /** `owner.name` or `owner.name(args)` read or called. */
  private member(owner: Val, name: string, args: Arg[] | null): Val {
    const lower = name.toLowerCase();
    const none = () => {
      if (args && args.length) unmappable('unknownMember', `${name}(…)`);
    };
    const one = (): Expr => {
      const list = this.positional(args, name);
      return list.length === 1 ? list[0]! : unmappable('unknownMember', `${name}(…)`);
    };
    const virtual = (kind: Kind, of: Val = owner): Val => {
      none();
      return { code: '', prec: P.primary, kind, virtual: true, owner: of };
    };
    const action = (code: string, kind?: Kind): Val => ({
      code: `await ${code}`,
      prec: P.unary,
      ...(kind ? { kind } : {}),
    });
    const prim = (code: string, str = false): Val => ({
      code,
      prec: P.primary,
      ...(str ? { str: true } : {}),
    });
    const hasArgs = !!args && args.length > 0;

    switch (owner.kind) {
      case 'app':
        if (NOT_NEEDED.has(lower)) return unmappable('notNeeded', `Application.${name}`);
        try {
          return this.root(name, args);
        } catch (e) {
          if (e instanceof Unmappable && e.reason === 'unknownName') {
            return unmappable('unknownMember', `Application.${name}`);
          }
          throw e;
        }

      // ------------------------------------------------------ BlueSheet
      case 'workbook':
        if (lower === 'worksheets' || lower === 'sheets') return this.sheetsOf(args);
        if (lower === 'activesheet') {
          none();
          return action('workbook.activeSheet', 'sheet');
        }
        break;
      case 'sheets':
        if (lower === 'count') return prim(`${this.obj(owner)}.length`);
        if (lower === 'item') return this.sheetsOf(args);
        if (lower === 'add' && !hasArgs) return action('workbook.addSheet()', 'sheet');
        break;
      case 'sheet':
        if (lower === 'range') return this.rangeOf(owner, args);
        if (lower === 'cells') return this.cellOf(owner, args);
        if (lower === 'name') return prim(`${this.obj(owner)}.name`, true);
        if (lower === 'index') return { code: `${this.obj(owner)}.index + 1`, prec: P.add };
        break;
      case 'range': {
        const r = this.obj(owner);
        switch (lower) {
          case 'value':
          case 'value2':
            none();
            return this.read(owner);
          case 'formula':
            none();
            return prim(`(await ${r}.formulas)[0][0]`, true);
          case 'text':
            none();
            return prim(`String(${this.read(owner).code} ?? "")`, true);
          case 'address':
            if (!hasArgs) return prim(`${r}.address`, true);
            break;
          case 'font':
            return virtual('font');
          case 'interior':
            return virtual('interior');
          case 'clearcontents':
            none();
            return action(`${r}.clear("contents")`);
          case 'clearformats':
            none();
            return action(`${r}.clear("formats")`);
          case 'clear':
            none();
            return action(`${r}.clear("all")`);
          case 'merge':
            none();
            return action(`${r}.merge()`);
          default:
            break;
        }
        break;
      }

      // ------------------------------------------------------ BlueDoc
      case 'doc':
        if (lower === 'paragraphs') {
          if (hasArgs) return action(`doc.paragraph(${this.idx0(one()).code})`, 'paragraph');
          return { code: 'await doc.paragraphs', prec: P.unary, kind: 'paragraphs' };
        }
        if ((lower === 'range' || lower === 'content') && !hasArgs) return virtual('docContent');
        if (lower === 'words') return virtual('docWords');
        if (lower === 'name') return prim('(await doc.info).name', true);
        break;
      case 'docWords':
        if (lower === 'count') return prim('(await doc.info).wordCount');
        break;
      case 'paragraphs':
        if (lower === 'count') return prim(`${this.obj(owner)}.length`);
        if (lower === 'item') return this.item(owner, one(), 'paragraph');
        break;
      case 'paragraph':
        if (lower === 'range') return virtual('paraRange');
        break;
      case 'paraRange':
        if (lower === 'text') return prim(`${this.obj(owner.owner!)}.text`, true);
        if (lower === 'font') return virtual('font', owner.owner);
        break;
      case 'docContent':
        if (lower === 'text') return { code: 'await doc.text', prec: P.unary, str: true };
        if (lower === 'find') return virtual('docFind');
        break;
      case 'docSel':
        switch (lower) {
          case 'typetext': {
            const text = args?.find((a) => a.name?.toLowerCase() === 'text')?.value ?? one();
            return action(`doc.insertText(${this.val(text).code})`);
          }
          case 'typeparagraph':
            none();
            return action('doc.insertText("\\n")');
          case 'wholestory':
            none();
            return action('doc.selectAll()');
          case 'insertbreak': {
            const kind = args?.[0]?.value;
            if (kind && lowerName(kind) !== 'wdpagebreak' && num(kind) !== 7) break;
            return action('doc.insertPageBreak()');
          }
          case 'font':
            return virtual('font');
          case 'find':
            return virtual('docFind');
          default:
            break;
        }
        break;
      case 'docFind':
        if (lower === 'execute') return this.findExecute(args ?? []);
        break;

      // ------------------------------------------------------ BlueSlide
      case 'presentation':
        if (lower === 'slides') {
          if (hasArgs)
            return {
              code: `presentation.slide(${this.idx0(one()).code})`,
              prec: P.primary,
              kind: 'slide',
            };
          return { code: 'await presentation.slides', prec: P.unary, kind: 'slides' };
        }
        if (lower === 'name') return prim('(await presentation.info).name', true);
        break;
      case 'slides':
        if (lower === 'count') return prim(`${this.obj(owner)}.length`);
        if (lower === 'item') return this.item(owner, one(), 'slide');
        break;
      case 'slide': {
        const s = this.obj(owner);
        if (lower === 'shapes') {
          const list: Val = { code: `await ${s}.shapes`, prec: P.unary, kind: 'shapes', owner };
          return hasArgs ? this.item(list, one(), 'shape') : list;
        }
        if (lower === 'slideindex') return { code: `${s}.index + 1`, prec: P.add };
        if (lower === 'delete' && !hasArgs) return action(`${s}.delete()`);
        if (lower === 'duplicate' && !hasArgs) return action(`${s}.duplicate()`, 'slide');
        break;
      }
      case 'shapes': {
        if (lower === 'count') return prim(`${this.obj(owner)}.length`);
        if (lower === 'item') return this.item(owner, one(), 'shape');
        const slide = owner.owner
          ? this.obj(owner.owner)
          : unmappable('unknownMember', `Shapes.${name}`);
        if (lower === 'addtextbox' || lower === 'addshape') {
          const list = this.positional(args, name);
          if (list.length !== 5) break;
          const [x, y, w, h] = list.slice(1).map((e) => this.val(e).code);
          const rect = `{ x: ${x}, y: ${y}, w: ${w}, h: ${h} }`;
          if (lower === 'addtextbox') return action(`${slide}.addTextBox("", ${rect})`, 'shape');
          return action(
            `${slide}.addShape(${this.enumOf(list[0]!, SHAPE_PRESETS, 'shape type')}, ${rect})`,
            'shape',
          );
        }
        break;
      }
      case 'shape': {
        const s = this.obj(owner);
        const rect: Record<string, string> = { left: 'x', top: 'y', width: 'w', height: 'h' };
        if (lower === 'textframe' || lower === 'textframe2') return virtual('textFrame');
        if (lower === 'fill') return virtual('fill');
        if (lower === 'line') return virtual('line');
        if (lower === 'name') return prim(`${s}.name`, true);
        if (rect[lower]) return prim(`${s}.rect.${rect[lower]}`);
        if (lower === 'hastextframe') return { code: `${s}.text !== null`, prec: P.eq };
        if (lower === 'delete' && !hasArgs) return action(`${s}.delete()`);
        break;
      }
      case 'textFrame':
        if (lower === 'textrange') return virtual('textRange', owner.owner);
        break;
      case 'textRange':
        if (lower === 'text') return prim(`(${this.obj(owner.owner!)}.text ?? "")`, true);
        if (lower === 'font') return virtual('font', owner.owner);
        break;
      case 'fill':
      case 'line':
        if (lower === 'forecolor') return virtual('foreColor');
        break;
      default:
        if (!owner.kind) return unmappable('unknownName', `${owner.code || 'object'}.${name}`);
        break;
    }
    return unmappable('unknownMember', `${owner.kind ?? 'object'}.${name}`);
  }

  /** `Find.Execute FindText:=…, ReplaceWith:=…, Replace:=wdReplaceAll` → `doc.replace`. */
  private findExecute(args: Arg[]): Val {
    const named = new Map<string, Expr>();
    const order = ['findtext', 'matchcase', 'matchwholeword', 'matchwildcards'];
    args.forEach((a, i) => {
      const key = a.name?.toLowerCase() ?? order[i];
      if (key && a.value) named.set(key, a.value);
      else if (a.value) unmappable('unknownMember', 'Find.Execute');
    });
    const find = named.get('findtext');
    const replacement = named.get('replacewith');
    const mode = named.get('replace');
    if (!find || !replacement || !(lowerName(mode) === 'wdreplaceall' || num(mode) === 2)) {
      return unmappable('unknownMember', 'Find.Execute');
    }
    const options: string[] = [];
    const flag = (key: string, option: string) => {
      const e = named.get(key);
      if (e) options.push(`${option}: ${this.bool(e)}`);
      named.delete(key);
    };
    flag('matchcase', 'matchCase');
    flag('matchwholeword', 'wholeWord');
    flag('matchwildcards', 'wildcards');
    ['findtext', 'replacewith', 'replace', 'forward', 'wrap'].forEach((k) => named.delete(k));
    if (named.size)
      return unmappable('unknownMember', `Find.Execute ${[...named.keys()].join(', ')}`);
    const opts = options.length ? `, { ${options.join(', ')} }` : '';
    return {
      code: `await doc.replace(${this.val(find).code}, ${this.val(replacement).code}${opts})`,
      prec: P.unary,
    };
  }

  /** `owner.name = value` as a statement. */
  private setter(owner: Val, name: string, value: Expr): string {
    const lower = name.toLowerCase();
    const v = () => this.val(value).code;
    switch (owner.kind) {
      case 'app':
        if (NOT_NEEDED.has(lower)) return unmappable('notNeeded', `Application.${name}`);
        break;
      case 'range': {
        const r = this.obj(owner);
        switch (lower) {
          case 'value':
          case 'value2':
            return `await ${r}.setValue(${v()});`;
          case 'formula':
            return `await ${r}.setFormulas([[${v()}]]);`;
          case 'numberformat':
            return `await ${r}.format({ numFmt: ${v()} });`;
          case 'horizontalalignment':
            return `await ${r}.format({ hAlign: ${this.enumOf(value, H_ALIGN, name)} });`;
          case 'verticalalignment':
            return `await ${r}.format({ vAlign: ${this.enumOf(value, V_ALIGN, name)} });`;
          case 'wraptext':
            return `await ${r}.format({ wrap: ${this.bool(value)} });`;
          default:
            break;
        }
        break;
      }
      case 'interior':
        if (lower === 'color') {
          return `await ${this.obj(owner.owner!)}.format({ fillColor: ${this.colorOf(value)} });`;
        }
        break;
      case 'font':
        return this.fontSetter(owner, name, value);
      case 'sheet':
        if (lower === 'name') return `await ${this.obj(owner)}.rename(${v()});`;
        break;
      case 'paragraph':
        if (lower === 'style') return `await ${this.obj(owner)}.setStyle(${v()});`;
        if (lower === 'alignment') {
          return `await ${this.obj(owner)}.setFormat({ align: ${this.enumOf(value, P_ALIGN, name)} });`;
        }
        break;
      case 'paraRange':
        if (lower === 'text') return `await ${this.obj(owner.owner!)}.setText(${v()});`;
        break;
      case 'docSel':
        if (lower === 'text') return `await doc.insertText(${v()});`;
        break;
      case 'textRange':
        if (lower === 'text') return `await ${this.obj(owner.owner!)}.setText(${v()});`;
        break;
      case 'foreColor': {
        const part = owner.owner!;
        if (lower === 'rgb' && part.owner) {
          const method = part.kind === 'fill' ? 'setFill' : 'setLine';
          return `await ${this.obj(part.owner)}.${method}(${this.colorOf(value)});`;
        }
        break;
      }
      default:
        if (!owner.kind) return unmappable('unknownName', `${owner.code || 'object'}.${name}`);
        break;
    }
    return unmappable('unknownMember', `${owner.kind ?? 'object'}.${name}`);
  }

  private fontSetter(font: Val, name: string, value: Expr): string {
    const lower = name.toLowerCase();
    const of = font.owner;
    const flavour = of?.kind === 'range' ? 'sheet' : of?.kind === 'shape' ? 'slide' : 'doc';
    const target =
      of?.kind === 'docSel' ? 'doc' : of ? this.obj(of) : unmappable('unknownMember', 'Font');
    const keys: Record<string, Record<string, string>> = {
      sheet: {
        bold: 'bold',
        italic: 'italic',
        underline: 'underline',
        strikethrough: 'strike',
        size: 'fontSize',
        name: 'fontName',
        color: 'fontColor',
      },
      doc: {
        bold: 'bold',
        italic: 'italic',
        underline: 'underline',
        strikethrough: 'strike',
        superscript: 'superscript',
        subscript: 'subscript',
        size: 'size',
        name: 'font',
        color: 'color',
      },
      slide: {
        bold: 'bold',
        italic: 'italic',
        underline: 'underline',
        size: 'size',
        name: 'font',
        color: 'color',
      },
    };
    const key = keys[flavour]![lower];
    if (!key) return unmappable('unknownMember', `Font.${name}`);
    let code: string;
    if (lower === 'color') code = this.colorOf(value);
    else if (lower === 'size' || lower === 'name') code = this.val(value).code;
    else if (lower === 'underline' && flavour === 'sheet') {
      const b = this.bool(value);
      code = b === 'true' ? '"single"' : b === 'false' ? '"none"' : `${b} ? "single" : "none"`;
    } else code = this.bool(value);
    return `await ${target}.format({ ${key}: ${code} });`;
  }
}
