/**
 * Parser of the VBA → BlueScript draft converter (CORE-SCRIPT-003): statements
 * and expressions of a practical subset of [MS-VBAL] §5 (procedures, `Dim` /
 * `Const`, `If`, `For`, `For Each`, `Do` / `Loop`, `While` / `Wend`,
 * `Select Case`, `With`, assignments, calls). Everything else becomes an
 * `unsupported` statement that keeps its source text; the parser never throws.
 */
import type {
  Arg,
  Branch,
  CaseClause,
  CaseTest,
  Expr,
  Param,
  Stmt,
  VarDecl,
  VbaWarningReason,
} from './ast';
import { spanText, tokenize, type Token } from './lexer';

/** One statement: the tokens between two statement separators. */
interface Line {
  toks: Token[];
  /** Separator that ended it. */
  sep: '\n' | ':';
  /** First statement of its source line. */
  first: boolean;
  line: number;
  text: string;
}

class ParseFail extends Error {}

const fail = (): never => {
  throw new ParseFail();
};

const isOp = (t: Token | undefined, text: string) => !!t && t.kind === 'op' && t.text === text;
const isWord = (t: Token | undefined, word: string) => !!t && t.kind === 'id' && t.lower === word;

/** Words that end an expression. */
const STOP_WORDS = new Set(['then', 'to', 'step', 'else', 'elseif', 'end', 'in', 'as', 'goto']);

/** Binary operators with their binding power ([MS-VBAL] §5.6.9 operator precedence). */
function binaryOp(t: Token | undefined): { op: string; bp: number } | null {
  if (!t) return null;
  if (t.kind === 'op') {
    switch (t.text) {
      case '^':
        return { op: '^', bp: 14 };
      case '*':
      case '/':
        return { op: t.text, bp: 12 };
      case '\\':
        return { op: '\\', bp: 11 };
      case '+':
      case '-':
        return { op: t.text, bp: 9 };
      case '&':
        return { op: '&', bp: 8 };
      case '=':
      case '<>':
      case '<':
      case '>':
      case '<=':
      case '>=':
        return { op: t.text, bp: 7 };
      default:
        return null;
    }
  }
  if (t.kind !== 'id') return null;
  switch (t.lower) {
    case 'mod':
      return { op: 'mod', bp: 10 };
    case 'like':
    case 'is':
      return { op: t.lower, bp: 7 };
    case 'and':
      return { op: 'and', bp: 5 };
    case 'or':
      return { op: 'or', bp: 4 };
    case 'xor':
      return { op: 'xor', bp: 3 };
    case 'eqv':
      return { op: 'eqv', bp: 2 };
    case 'imp':
      return { op: 'imp', bp: 1 };
    default:
      return null;
  }
}

class ExprParser {
  i = 0;
  constructor(readonly toks: Token[]) {}

  get done(): boolean {
    return this.i >= this.toks.length;
  }
  peek(offset = 0): Token | undefined {
    return this.toks[this.i + offset];
  }
  next(): Token {
    return this.toks[this.i++] ?? fail();
  }
  expectOp(text: string): void {
    if (!isOp(this.peek(), text)) fail();
    this.i++;
  }

  expr(minBp = 0): Expr {
    let left = this.prefix();
    for (;;) {
      const b = binaryOp(this.peek());
      if (!b || b.bp < minBp) return left;
      this.i++;
      const right = this.expr(b.bp + 1);
      left = { t: 'binary', op: b.op, left, right };
    }
  }

  private prefix(): Expr {
    const t = this.peek();
    if (isOp(t, '-') || isOp(t, '+')) {
      this.i++;
      return { t: 'unary', op: t!.text as '-' | '+', expr: this.expr(13) };
    }
    if (isWord(t, 'not')) {
      this.i++;
      return { t: 'unary', op: 'not', expr: this.expr(6) };
    }
    return this.postfix();
  }

  /** A primary expression with its `.member`, `(arguments)` suffixes. */
  postfix(): Expr {
    let e = this.primary();
    for (;;) {
      const t = this.peek();
      if (isOp(t, '(')) {
        this.i++;
        const args = this.args(true);
        this.expectOp(')');
        e = { t: 'call', callee: e, args };
        continue;
      }
      // `a .b`: a new `With` member (an argument), not a member of `a`.
      if (isOp(t, '.') && !(t!.space && !this.peek(1)?.space)) {
        this.i++;
        const name = this.next();
        if (name.kind !== 'id') fail();
        e = { t: 'member', obj: e, name: name.text };
        continue;
      }
      if (isOp(t, '!')) {
        this.i++;
        const name = this.next();
        e = { t: 'bad', reason: 'unsupportedExpression', what: `!${name.text}` };
        continue;
      }
      return e;
    }
  }

  private primary(): Expr {
    const t = this.next();
    switch (t.kind) {
      case 'num':
        return { t: 'num', value: t.text };
      case 'str':
        return { t: 'str', value: t.text };
      case 'date':
        return { t: 'date', text: t.text };
      case 'op': {
        if (t.text === '(') {
          const e = this.expr();
          this.expectOp(')');
          return e;
        }
        if (t.text === '.') {
          const name = this.next();
          if (name.kind !== 'id') fail();
          return { t: 'member', obj: null, name: name.text };
        }
        return fail();
      }
      case 'id': {
        if (t.lower === 'new') {
          let what = 'New';
          while (this.peek()?.kind === 'id' || isOp(this.peek(), '.'))
            what += ` ${this.next().text}`;
          return { t: 'bad', reason: 'unsupportedExpression', what: what.replace(/ \. /g, '.') };
        }
        if (t.lower === 'typeof' || t.lower === 'addressof') {
          this.i = this.toks.length;
          return { t: 'bad', reason: 'unsupportedExpression', what: t.text };
        }
        if (STOP_WORDS.has(t.lower)) return fail();
        return { t: 'name', name: t.text };
      }
      default:
        return fail();
    }
  }

  /** Argument list; `closed`: up to a `)`, else to the end of the statement. */
  args(closed: boolean): Arg[] {
    const out: Arg[] = [];
    const atEnd = () => (closed ? isOp(this.peek(), ')') : this.done);
    if (atEnd()) return out;
    for (;;) {
      if (isOp(this.peek(), ',') || atEnd()) {
        out.push({ value: null });
      } else {
        if (isWord(this.peek(), 'byval') || isWord(this.peek(), 'byref')) this.i++;
        const t = this.peek();
        if (t?.kind === 'id' && isOp(this.peek(1), ':=')) {
          this.i += 2;
          out.push({ name: t.text, value: this.expr() });
        } else {
          out.push({ value: this.expr() });
        }
      }
      if (isOp(this.peek(), ',')) {
        this.i++;
        continue;
      }
      if (!atEnd()) fail();
      return out;
    }
  }
}

/** Splits at the tokens matching `at`, outside parentheses. */
function splitTop(toks: Token[], at: (t: Token) => boolean): Token[][] {
  const out: Token[][] = [[]];
  let depth = 0;
  for (const t of toks) {
    if (isOp(t, '(')) depth++;
    else if (isOp(t, ')')) depth--;
    if (depth === 0 && at(t)) out.push([]);
    else out[out.length - 1]!.push(t);
  }
  return out;
}

/** Index of the first `word` outside parentheses, -1 when absent. */
function findTop(toks: Token[], word: string, from = 0): number {
  let depth = 0;
  for (let i = from; i < toks.length; i++) {
    const t = toks[i]!;
    if (isOp(t, '(')) depth++;
    else if (isOp(t, ')')) depth--;
    else if (depth === 0 && isWord(t, word)) return i;
  }
  return -1;
}

const BAD_SYNTAX = (what: string): Expr => ({ t: 'bad', reason: 'syntax', what });

/** The whole of `toks` as one expression; a `bad` node when it does not parse. */
function exprOf(toks: Token[]): Expr {
  if (toks.length === 0) return BAD_SYNTAX('');
  try {
    const p = new ExprParser(toks);
    const e = p.expr();
    return p.done ? e : BAD_SYNTAX(toks[p.i]?.text ?? '');
  } catch (e) {
    if (e instanceof ParseFail) return BAD_SYNTAX(toks[0]!.text);
    throw e;
  }
}

/** `Dim`-style type name after `As` (`New` dropped, `String * 10` → `String`). */
function typeName(toks: Token[]): string | undefined {
  const words = toks.filter((t) => t.kind === 'id' && t.lower !== 'new').map((t) => t.text);
  return words.length ? words.join('.') : undefined;
}

type Closer = 'if' | 'for' | 'do' | 'while' | 'select' | 'with' | 'proc';

const MODIFIERS = new Set(['public', 'private', 'friend', 'global']);

/** Statement keywords the converter does not translate, with the reason reported. */
const UNSUPPORTED_HEADS = new Map<string, VbaWarningReason>([
  ['goto', 'jump'],
  ['gosub', 'jump'],
  ['return', 'jump'],
  ['on', 'errorHandling'],
  ['resume', 'errorHandling'],
  ['error', 'errorHandling'],
  ['doevents', 'notNeeded'],
  ['beep', 'notNeeded'],
  ...(
    [
      'declare',
      'implements',
      'event',
      'raiseevent',
      'stop',
      'end',
      'open',
      'close',
      'print',
      'write',
      'input',
      'line',
      'get',
      'put',
      'seek',
      'lock',
      'unlock',
      'reset',
      'kill',
      'name',
      'filecopy',
      'mkdir',
      'rmdir',
      'chdir',
      'chdrive',
      'erase',
      'load',
      'unload',
      'randomize',
      'lset',
      'rset',
      'sendkeys',
      'appactivate',
      'defint',
      'deflng',
      'defstr',
      'defbool',
      'defdbl',
      'defvar',
    ] as const
  ).map((w): [string, VbaWarningReason] => [w, 'unsupportedStatement']),
]);

class Parser {
  private pos = 0;
  constructor(
    private readonly lines: Line[],
    private readonly source: string,
  ) {}

  private get cur(): Line | undefined {
    return this.lines[this.pos];
  }

  private static head(ln: Line | undefined, offset = 0): string {
    const t = ln?.toks[offset];
    return t?.kind === 'id' ? t.lower : '';
  }

  /** Tokens of a line without leading access modifiers. */
  private static bare(ln: Line): Token[] {
    let i = 0;
    while (ln.toks[i]?.kind === 'id' && MODIFIERS.has(ln.toks[i]!.lower)) i++;
    if (isWord(ln.toks[i], 'static') && ['sub', 'function'].includes(ln.toks[i + 1]?.lower ?? ''))
      i++;
    return i ? ln.toks.slice(i) : ln.toks;
  }

  private static isProcHeader(ln: Line): boolean {
    const t = Parser.bare(ln);
    return (isWord(t[0], 'sub') || isWord(t[0], 'function')) && t[1]?.kind === 'id';
  }

  /** The block a line closes (or, for `Else` / `Case`, continues). */
  private static closer(ln: Line): Closer | null {
    const a = Parser.head(ln);
    const b = Parser.head(ln, 1);
    // `Next = 1`, `Loop.x`: a variable, not a keyword.
    if (isOp(ln.toks[1], '=') || isOp(ln.toks[1], '.')) return null;
    switch (a) {
      case 'end':
        switch (b) {
          case 'if':
            return 'if';
          case 'select':
            return 'select';
          case 'with':
            return 'with';
          case 'while':
            return 'while';
          case 'sub':
          case 'function':
            return 'proc';
          default:
            return null;
        }
      case 'endif':
      case 'elseif':
      case 'else':
        return 'if';
      case 'next':
        return 'for';
      case 'loop':
        return 'do';
      case 'wend':
        return 'while';
      case 'case':
        return 'select';
      default:
        return Parser.isProcHeader(ln) ? 'proc' : null;
    }
  }

  private note(
    ln: Line | undefined,
    reason: VbaWarningReason,
    construct: string,
    text?: string,
  ): Stmt {
    const last = this.lines[this.lines.length - 1];
    return {
      s: 'unsupported',
      reason,
      construct,
      line: ln?.line ?? last?.line ?? 1,
      text: text ?? ln?.text ?? construct,
    };
  }

  /** Statements up to a line that closes one of the open blocks in `open`. */
  block(open: Closer[]): Stmt[] {
    const out: Stmt[] = [];
    while (this.pos < this.lines.length) {
      const ln = this.cur!;
      const k = Parser.closer(ln);
      if (k) {
        if (open.includes(k)) return out;
        if (!(k === 'proc' && Parser.isProcHeader(ln))) {
          // A closer with no open block.
          out.push(this.note(ln, 'syntax', ln.toks[0]?.text ?? ''));
          this.pos++;
          continue;
        }
      }
      out.push(...this.statement(open));
    }
    return out;
  }

  /** Consumes the closing line when `test` accepts it; else reports the missing closer. */
  private close(test: (ln: Line) => boolean, missing: string, into: Stmt[]): void {
    const ln = this.cur;
    if (ln && test(ln)) this.pos++;
    else {
      const at = this.lines[this.pos - 1] ?? ln;
      into.push(this.note(at, 'syntax', `missing ${missing}`, `missing ${missing}`));
    }
  }

  private statement(open: Closer[]): Stmt[] {
    const ln = this.cur!;
    const base = { line: ln.line, text: ln.text };
    const first = ln.toks[0]!;
    if (first.kind === 'comment') {
      this.pos++;
      return [{ ...base, s: 'comment', comment: first.text }];
    }
    if (first.kind === 'directive') {
      this.pos++;
      return [this.note(ln, 'unsupportedStatement', first.text.split(/\s/)[0] ?? '#')];
    }
    const bare = Parser.bare(ln);
    const hadModifier = bare.length !== ln.toks.length;
    let toks = bare;
    // Line number label.
    if (ln.first && toks[0]?.kind === 'num' && toks.length > 1) toks = toks.slice(1);
    const head = toks[0]?.kind === 'id' ? toks[0].lower : '';
    const second = toks[1];
    const looksLikeName = isOp(second, '=') || isOp(second, '.');

    // `label:` at the start of a line ([MS-VBAL] §5.4.1.1).
    if (
      ln.first &&
      toks.length === 1 &&
      toks[0]!.kind === 'id' &&
      ln.sep === ':' &&
      !UNSUPPORTED_HEADS.has(head) &&
      head !== 'do'
    ) {
      this.pos++;
      return [this.note(ln, 'jump', `${toks[0]!.text}:`, `${ln.text}:`)];
    }
    if (hadModifier && toks[0]?.kind === 'id' && !looksLikeName) {
      // After a modifier: a declaration (`Public total As Long`).
      if (
        ![
          'sub',
          'function',
          'const',
          'dim',
          'declare',
          'type',
          'enum',
          'property',
          'event',
          'withevents',
        ].includes(head)
      ) {
        this.pos++;
        return [{ ...base, s: 'dim', vars: this.varDecls(toks) }];
      }
    }
    if (!looksLikeName) {
      switch (head) {
        case 'sub':
        case 'function':
          if (second?.kind === 'id') return this.proc(ln, toks, open);
          break;
        case 'option':
          this.pos++;
          return [{ ...base, s: 'comment', comment: ` ${ln.text}` }];
        case 'attribute':
          // Module attribute of an exported file: metadata, not code.
          this.pos++;
          return [];
        case 'dim':
        case 'static':
          this.pos++;
          return [{ ...base, s: 'dim', vars: this.varDecls(toks.slice(1)) }];
        case 'const':
          this.pos++;
          return this.consts(ln, toks.slice(1));
        case 'redim':
          this.pos++;
          return [this.redim(ln, toks.slice(1))];
        case 'if':
          return this.ifStatement(ln, toks, open);
        case 'for':
          return this.forStatement(ln, toks, open);
        case 'do':
          return this.doStatement(ln, toks, open);
        case 'while':
          return this.whileStatement(ln, toks, open);
        case 'select':
          if (isWord(second, 'case')) return this.selectStatement(ln, toks, open);
          break;
        case 'with':
          return this.withStatement(ln, toks, open);
        case 'exit': {
          this.pos++;
          const what = second?.kind === 'id' ? second.lower : '';
          if (
            what === 'sub' ||
            what === 'function' ||
            what === 'for' ||
            what === 'do' ||
            what === 'property'
          )
            return [{ ...base, s: 'exit', what }];
          return [this.note(ln, 'syntax', 'Exit')];
        }
        case 'type':
        case 'enum':
        case 'property':
          if (second?.kind === 'id') return this.skipBlock(head);
          break;
        default:
          break;
      }
      const reason = UNSUPPORTED_HEADS.get(head);
      if (reason && !isOp(second, '(')) {
        this.pos++;
        const two = head === 'on' && second?.kind === 'id' ? ` ${second.text}` : '';
        return [this.note(ln, reason, `${toks[0]!.text}${two}`)];
      }
    }
    this.pos++;
    return [this.simple(ln, toks)];
  }

  /** Assignment, call or `Debug.Print`. */
  private simple(ln: Line, toksIn: Token[]): Stmt {
    const base = { line: ln.line, text: ln.text };
    let toks = toksIn;
    try {
      let set = false;
      const head = toks[0]?.kind === 'id' ? toks[0].lower : '';
      if ((head === 'let' || head === 'set') && toks[1]?.kind !== 'op') {
        set = head === 'set';
        toks = toks.slice(1);
      } else if (head === 'call' && toks.length > 1 && toks[1]?.kind !== 'op') {
        const p = new ExprParser(toks.slice(1));
        const expr = p.expr();
        if (!p.done) fail();
        return { ...base, s: 'call', expr };
      }
      if (isWord(toks[0], 'debug') && isOp(toks[1], '.') && isWord(toks[2], 'print')) {
        const args = splitTop(toks.slice(3), (t) => isOp(t, ';') || isOp(t, ','))
          .filter((a) => a.length > 0)
          .map(exprOf);
        return { ...base, s: 'print', args };
      }
      const p = new ExprParser(toks);
      const target = p.postfix();
      if (p.done) return { ...base, s: 'call', expr: target };
      if (isOp(p.peek(), '=')) {
        p.i++;
        const value = p.expr();
        if (!p.done) fail();
        return { ...base, s: 'assign', target, value, set };
      }
      if (set) fail();
      if (target.t === 'call' && isOp(p.peek(), ',')) {
        // `Foo (a), b`: the parentheses belong to the first argument.
        p.i++;
        const rest = p.args(false);
        return {
          ...base,
          s: 'call',
          expr: { t: 'call', callee: target.callee, args: [...target.args, ...rest] },
        };
      }
      const args = p.args(false);
      return { ...base, s: 'call', expr: { t: 'call', callee: target, args } };
    } catch (e) {
      if (e instanceof ParseFail) return this.note(ln, 'syntax', toks[0]?.text ?? '');
      throw e;
    }
  }

  private varDecls(toksIn: Token[]): VarDecl[] {
    const out: VarDecl[] = [];
    for (const part of splitTop(toksIn, (t) => isOp(t, ','))) {
      const toks = isWord(part[0], 'withevents') ? part.slice(1) : part;
      const name = toks[0];
      if (!name || name.kind !== 'id') continue;
      let i = 1;
      let array = false;
      let dims = 0;
      if (isOp(toks[i], '(')) {
        array = true;
        let depth = 0;
        const inner: Token[] = [];
        for (; i < toks.length; i++) {
          const t = toks[i]!;
          if (isOp(t, '(')) depth++;
          if (isOp(t, ')')) depth--;
          if (depth === 0) break;
          if (depth > 1 || !isOp(t, '(')) inner.push(t);
        }
        i++;
        dims = inner.length ? splitTop(inner, (t) => isOp(t, ',')).length : 0;
      }
      const as = isWord(toks[i], 'as') ? typeName(toks.slice(i + 1)) : undefined;
      out.push({ name: name.text, array, dims, ...(as ? { type: as } : {}) });
    }
    return out;
  }

  private consts(ln: Line, toks: Token[]): Stmt[] {
    const out: Stmt[] = [];
    for (const part of splitTop(toks, (t) => isOp(t, ','))) {
      const eq = part.findIndex((t) => isOp(t, '='));
      const name = part[0];
      if (!name || name.kind !== 'id' || eq < 0) {
        out.push(this.note(ln, 'syntax', 'Const'));
        continue;
      }
      out.push({
        line: ln.line,
        text: ln.text,
        s: 'const',
        name: name.text,
        value: exprOf(part.slice(eq + 1)),
      });
    }
    return out;
  }

  private redim(ln: Line, toksIn: Token[]): Stmt {
    const preserve = isWord(toksIn[0], 'preserve');
    const toks = preserve ? toksIn.slice(1) : toksIn;
    const items: { name: string; bound: Expr | null; dims: number }[] = [];
    for (const part of splitTop(toks, (t) => isOp(t, ','))) {
      const name = part[0];
      if (!name || name.kind !== 'id' || !isOp(part[1], '(')) {
        return this.note(ln, 'syntax', 'ReDim');
      }
      const close = part.map((t) => isOp(t, ')')).lastIndexOf(true);
      const inner = part.slice(2, close < 0 ? part.length : close);
      const bounds = inner.length ? splitTop(inner, (t) => isOp(t, ',')) : [];
      let bound: Expr | null = null;
      if (bounds[0]) {
        // `lower To upper`: the upper bound counts.
        const to = findTop(bounds[0], 'to');
        bound = exprOf(to < 0 ? bounds[0] : bounds[0].slice(to + 1));
      }
      items.push({ name: name.text, bound, dims: bounds.length });
    }
    return { line: ln.line, text: ln.text, s: 'redim', preserve, items };
  }

  private params(toks: Token[]): Param[] {
    const out: Param[] = [];
    if (toks.length === 0) return out;
    for (const part of splitTop(toks, (t) => isOp(t, ','))) {
      let i = 0;
      let optional = false;
      let byRef = false;
      let paramArray = false;
      for (; part[i]?.kind === 'id'; i++) {
        const w = part[i]!.lower;
        if (w === 'optional') optional = true;
        else if (w === 'byref') byRef = true;
        else if (w === 'paramarray') paramArray = true;
        else if (w !== 'byval') break;
      }
      const name = part[i];
      if (!name || name.kind !== 'id') continue;
      i++;
      let array = false;
      if (isOp(part[i], '(')) {
        array = true;
        while (i < part.length && !isOp(part[i], ')')) i++;
        i++;
      }
      const eq = part.findIndex((t, k) => k >= i && isOp(t, '='));
      const asEnd = eq < 0 ? part.length : eq;
      const type = isWord(part[i], 'as') ? typeName(part.slice(i + 1, asEnd)) : undefined;
      out.push({
        name: name.text,
        optional,
        byRef,
        paramArray,
        array: array || paramArray,
        ...(type ? { type } : {}),
        ...(eq >= 0 ? { default: exprOf(part.slice(eq + 1)) } : {}),
      });
    }
    return out;
  }

  private proc(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    this.pos++;
    const kind = toks[0]!.lower === 'function' ? 'function' : 'sub';
    const name = toks[1]!.text;
    let params: Param[] = [];
    let i = 2;
    if (isOp(toks[i], '(')) {
      let depth = 0;
      const start = i + 1;
      for (; i < toks.length; i++) {
        if (isOp(toks[i], '(')) depth++;
        if (isOp(toks[i], ')') && --depth === 0) break;
      }
      params = this.params(toks.slice(start, i));
      i++;
    }
    const returnType = isWord(toks[i], 'as') ? typeName(toks.slice(i + 1)) : undefined;
    const body = this.block([...open, 'proc']);
    const out: Stmt[] = [
      {
        line: ln.line,
        text: ln.text,
        s: 'proc',
        kind,
        name,
        params,
        body,
        ...(returnType ? { returnType } : {}),
      },
    ];
    this.close(
      (l) => Parser.head(l) === 'end' && ['sub', 'function'].includes(Parser.head(l, 1)),
      kind === 'sub' ? 'End Sub' : 'End Function',
      out,
    );
    return out;
  }

  /** A synthetic line for statements written after `Then` / `Else` on the same line. */
  private inline(ln: Line, toks: Token[], sep: '\n' | ':'): Line {
    return {
      toks,
      sep,
      first: false,
      line: toks[0]?.line ?? ln.line,
      text: toks.length ? spanText(this.source, toks[0]!, toks[toks.length - 1]!) : '',
    };
  }

  private ifStatement(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    const then = findTop(toks, 'then');
    if (then < 0) {
      this.pos++;
      return [this.note(ln, 'syntax', 'If')];
    }
    const cond = exprOf(toks.slice(1, then));
    const rest = toks.slice(then + 1);
    const branches: Branch[] = [];
    const out: Stmt[] = [{ line: ln.line, text: ln.text, s: 'if', branches }];
    if (rest.length > 0) {
      // Single-line form: the rest of the source line belongs to the statement.
      const parts: Token[][] = [rest];
      let sep = ln.sep;
      this.pos++;
      while (sep === ':' && this.cur && this.cur.toks[0]?.kind !== 'comment') {
        parts.push(this.cur.toks);
        sep = this.cur.sep;
        this.pos++;
      }
      const thenLines: Line[] = [];
      const elseLines: Line[] = [];
      let inElse = false;
      for (const part of parts) {
        const at = inElse ? -1 : findTop(part, 'else');
        if (at < 0) {
          if (part.length) (inElse ? elseLines : thenLines).push(this.inline(ln, part, ':'));
          continue;
        }
        if (at > 0) thenLines.push(this.inline(ln, part.slice(0, at), ':'));
        inElse = true;
        if (at + 1 < part.length) elseLines.push(this.inline(ln, part.slice(at + 1), ':'));
      }
      const sub = (lines: Line[]) => new Parser(lines, this.source).block([]);
      branches.push({ line: ln.line, text: ln.text, cond, body: sub(thenLines) });
      if (inElse) branches.push({ line: ln.line, text: 'Else', cond: null, body: sub(elseLines) });
      return out;
    }
    this.pos++;
    const inner: Closer[] = [...open, 'if'];
    branches.push({ line: ln.line, text: ln.text, cond, body: this.block(inner) });
    for (;;) {
      const c = this.cur;
      const head = Parser.head(c);
      if (c && head === 'elseif') {
        const t = findTop(c.toks, 'then');
        const end = t < 0 ? c.toks.length : t;
        const after = t < 0 ? [] : c.toks.slice(t + 1);
        if (after.length) this.lines[this.pos] = this.inline(c, after, c.sep);
        else this.pos++;
        branches.push({
          line: c.line,
          text: c.text,
          cond: exprOf(c.toks.slice(1, end)),
          body: this.block(inner),
        });
        continue;
      }
      if (c && head === 'else') {
        const after = c.toks.slice(1);
        if (after.length) this.lines[this.pos] = this.inline(c, after, c.sep);
        else this.pos++;
        branches.push({ line: c.line, text: c.text, cond: null, body: this.block(inner) });
        continue;
      }
      break;
    }
    this.close(
      (l) => Parser.head(l) === 'endif' || (Parser.head(l) === 'end' && Parser.head(l, 1) === 'if'),
      'End If',
      out,
    );
    return out;
  }

  private forStatement(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    this.pos++;
    const out: Stmt[] = [];
    const base = { line: ln.line, text: ln.text };
    const inner: Closer[] = [...open, 'for'];
    if (isWord(toks[1], 'each')) {
      const at = findTop(toks, 'in');
      const v = toks[2];
      const ok = at === 3 && v?.kind === 'id';
      const collection = ok ? exprOf(toks.slice(at + 1)) : BAD_SYNTAX('For Each');
      out.push({
        ...base,
        s: 'forEach',
        variable: ok && v ? v.text : '',
        collection,
        body: this.block(inner),
      });
    } else {
      const v = toks[1];
      const to = findTop(toks, 'to');
      const step = findTop(toks, 'step', Math.max(to, 0));
      const ok = v?.kind === 'id' && isOp(toks[2], '=') && to > 3;
      out.push({
        ...base,
        s: 'for',
        variable: ok && v ? v.text : '',
        from: ok ? exprOf(toks.slice(3, to)) : BAD_SYNTAX('For'),
        to: ok ? exprOf(toks.slice(to + 1, step < 0 ? toks.length : step)) : BAD_SYNTAX('For'),
        step: ok && step >= 0 ? exprOf(toks.slice(step + 1)) : null,
        body: this.block(inner),
      });
    }
    this.close((l) => Parser.head(l) === 'next', 'Next', out);
    return out;
  }

  private doStatement(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    this.pos++;
    const word = toks[1]?.kind === 'id' ? toks[1].lower : '';
    let cond: Expr | null = null;
    let until = false;
    let post = false;
    if (word === 'while' || word === 'until') {
      cond = exprOf(toks.slice(2));
      until = word === 'until';
    } else if (toks.length > 1) {
      cond = BAD_SYNTAX('Do');
    }
    const body = this.block([...open, 'do']);
    const end = this.cur;
    if (end && Parser.head(end) === 'loop' && end.toks.length > 1) {
      const w = Parser.head(end, 1);
      if (cond === null && (w === 'while' || w === 'until')) {
        cond = exprOf(end.toks.slice(2));
        until = w === 'until';
        post = true;
      }
    }
    const out: Stmt[] = [{ line: ln.line, text: ln.text, s: 'loop', cond, until, post, body }];
    this.close((l) => Parser.head(l) === 'loop', 'Loop', out);
    return out;
  }

  private whileStatement(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    this.pos++;
    const out: Stmt[] = [
      {
        line: ln.line,
        text: ln.text,
        s: 'loop',
        cond: exprOf(toks.slice(1)),
        until: false,
        post: false,
        body: this.block([...open, 'while']),
      },
    ];
    this.close(
      (l) =>
        Parser.head(l) === 'wend' || (Parser.head(l) === 'end' && Parser.head(l, 1) === 'while'),
      'Wend',
      out,
    );
    return out;
  }

  private caseTests(toks: Token[]): CaseTest[] | null {
    if (isWord(toks[0], 'else')) return null;
    return splitTop(toks, (t) => isOp(t, ',')).map((part): CaseTest => {
      if (isWord(part[0], 'is') && part[1]?.kind === 'op') {
        return { k: 'is', op: part[1].text, value: exprOf(part.slice(2)) };
      }
      const to = findTop(part, 'to');
      if (to > 0)
        return { k: 'range', lo: exprOf(part.slice(0, to)), hi: exprOf(part.slice(to + 1)) };
      return { k: 'eq', value: exprOf(part) };
    });
  }

  private selectStatement(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    this.pos++;
    const inner: Closer[] = [...open, 'select'];
    const lead = this.block(inner);
    const cases: CaseClause[] = [];
    while (this.cur && Parser.head(this.cur) === 'case') {
      const c = this.cur;
      this.pos++;
      cases.push({
        line: c.line,
        text: c.text,
        tests: this.caseTests(c.toks.slice(1)),
        body: this.block(inner),
      });
    }
    const out: Stmt[] = [
      { line: ln.line, text: ln.text, s: 'select', subject: exprOf(toks.slice(2)), cases, lead },
    ];
    this.close(
      (l) => Parser.head(l) === 'end' && Parser.head(l, 1) === 'select',
      'End Select',
      out,
    );
    return out;
  }

  private withStatement(ln: Line, toks: Token[], open: Closer[]): Stmt[] {
    this.pos++;
    const out: Stmt[] = [
      {
        line: ln.line,
        text: ln.text,
        s: 'with',
        target: exprOf(toks.slice(1)),
        body: this.block([...open, 'with']),
      },
    ];
    this.close((l) => Parser.head(l) === 'end' && Parser.head(l, 1) === 'with', 'End With', out);
    return out;
  }

  /** `Type` / `Enum` / `Property` … `End …`: every line is reported, none translated. */
  private skipBlock(kind: string): Stmt[] {
    const out: Stmt[] = [];
    const construct = this.cur!.toks.find((t) => t.kind === 'id' && t.lower === kind)?.text ?? kind;
    while (this.cur) {
      const ln = this.cur;
      this.pos++;
      if (ln.toks[0]?.kind === 'comment') {
        out.push({ line: ln.line, text: ln.text, s: 'comment', comment: ln.toks[0].text });
        continue;
      }
      out.push(this.note(ln, 'unsupportedStatement', construct));
      if (Parser.head(ln) === 'end' && Parser.head(ln, 1) === kind) break;
    }
    return out;
  }
}

/** Parses a VBA module (or a loose snippet of statements). Never throws. */
export function parseVba(source: string): Stmt[] {
  const lines: Line[] = [];
  let cur: Token[] = [];
  let first = true;
  const flush = (sep: '\n' | ':') => {
    if (cur.length) {
      lines.push({
        toks: cur,
        sep,
        first,
        line: cur[0]!.line,
        text: spanText(source, cur[0]!, cur[cur.length - 1]!),
      });
      first = false;
      cur = [];
    }
    if (sep === '\n') first = true;
  };
  for (const t of tokenize(source)) {
    if (t.kind === 'eos') {
      flush(t.text === ':' ? ':' : '\n');
    } else if (t.kind === 'comment') {
      // A trailing comment is its own statement.
      flush('\n');
      cur = [t];
    } else {
      cur.push(t);
    }
  }
  flush('\n');
  return new Parser(lines, source).block([]);
}
