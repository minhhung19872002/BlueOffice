/**
 * Lexer of the VBA → BlueScript draft converter (CORE-SCRIPT-003).
 *
 * Written from the public language specification [MS-VBAL] (§3.3 lexical
 * tokens: line continuation ` _`, `'` / `Rem` comments, `"…""…"` strings,
 * `&H` / `&O` numbers, `#…#` date literals, type-suffix characters). VBA source
 * is only ever read as text here — nothing is executed.
 */

export type TokenKind =
  | 'id'
  | 'num'
  | 'str'
  | 'date'
  | 'op'
  /** End of statement: a line break (`\n`) or a `:` separator. */
  | 'eos'
  | 'comment'
  /** Conditional compilation line (`#If`, `#Const` …). */
  | 'directive';

export interface Token {
  kind: TokenKind;
  /**
   * `id`: the name as written; `num`: a JavaScript number literal; `str`: the
   * decoded value; `date`: the text between the `#`; `op`: the symbol;
   * `eos`: `\n` or `:`; `comment` / `directive`: the text.
   */
  text: string;
  /** Lower-cased `text` of an `id` (VBA names are case-insensitive), else `text`. */
  lower: string;
  /** 1-based physical line of the first character. */
  line: number;
  start: number;
  end: number;
  /** White space (or a line continuation) precedes the token. */
  space: boolean;
}

const isIdStart = (c: string) => /[\p{L}_]/u.test(c);
const isIdPart = (c: string) => /[\p{L}\p{N}_]/u.test(c);
const isDigit = (c: string) => c >= '0' && c <= '9';
const isNewline = (c: string) => c === '\n' || c === '\r' || c === '\u2028' || c === '\u2029';

const TWO_CHAR_OPS = new Set(['<>', '<=', '>=', ':=', '><', '=<', '=>']);

/** Tokens of a VBA module. Never throws: unknown characters become `op` tokens. */
export function tokenize(source: string): Token[] {
  const s = source;
  const n = s.length;
  const out: Token[] = [];
  let i = 0;
  let line = 1;
  let space = false;

  const push = (kind: TokenKind, text: string, start: number, end: number, startLine: number) => {
    out.push({
      kind,
      text,
      lower: kind === 'id' ? text.toLowerCase() : text,
      line: startLine,
      start,
      end,
      space,
    });
    space = false;
  };
  /** Index after the line break at `p` (CRLF counts once). */
  const afterNewline = (p: number) => (s[p] === '\r' && s[p + 1] === '\n' ? p + 2 : p + 1);
  /** At a statement start: nothing yet, or the previous token ends a statement. */
  const atStatementStart = () => {
    const last = out[out.length - 1];
    return !last || last.kind === 'eos';
  };
  /** Reads to the end of the line, following ` _` continuations; returns the end index. */
  const restOfLine = (from: number): number => {
    let p = from;
    for (;;) {
      while (p < n && !isNewline(s[p]!)) p++;
      // [MS-VBAL] §3.3.1: a comment line ending in ` _` continues on the next line.
      if (p < n && /\s_[ \t]*$/.test(s.slice(from, p))) {
        p = afterNewline(p);
        line++;
        continue;
      }
      return p;
    }
  };

  while (i < n) {
    const c = s[i]!;
    if (isNewline(c)) {
      push('eos', '\n', i, i, line);
      i = afterNewline(i);
      line++;
      continue;
    }
    if (/\s/.test(c)) {
      i++;
      space = true;
      continue;
    }
    // Line continuation: `_` after white space, only white space up to the line break.
    if (c === '_' && (i === 0 || /\s/.test(s[i - 1]!))) {
      let p = i + 1;
      while (p < n && (s[p] === ' ' || s[p] === '\t')) p++;
      if (p >= n || isNewline(s[p]!)) {
        i = p < n ? afterNewline(p) : p;
        line++;
        space = true;
        continue;
      }
    }
    if (c === "'") {
      const startLine = line;
      const end = restOfLine(i);
      push('comment', s.slice(i + 1, end), i, end, startLine);
      i = end;
      continue;
    }
    if (c === '"') {
      let p = i + 1;
      let value = '';
      for (;;) {
        if (p >= n || isNewline(s[p]!)) break; // unterminated: closed at the line end
        if (s[p] === '"') {
          if (s[p + 1] === '"') {
            value += '"';
            p += 2;
            continue;
          }
          p++;
          break;
        }
        value += s[p];
        p++;
      }
      push('str', value, i, p, line);
      i = p;
      continue;
    }
    if (c === '#') {
      if (atStatementStart() && /^#\s*(if|elseif|else|end|const)\b/i.test(s.slice(i, i + 12))) {
        const startLine = line;
        const end = restOfLine(i);
        push('directive', s.slice(i, end), i, end, startLine);
        i = end;
        continue;
      }
      const m = /^#([0-9A-Za-z/\-:., ]{1,40})#/.exec(s.slice(i, i + 42));
      if (m && /\d/.test(m[1]!)) {
        push('date', m[1]!.trim(), i, i + m[0].length, line);
        i += m[0].length;
        continue;
      }
    }
    if (c === '&' && /^&[hH][0-9a-fA-F]+|^&[oO][0-7]+/.test(s.slice(i, i + 20))) {
      const m = /^&([hHoO])([0-9a-fA-F]+)/.exec(s.slice(i, i + 20))!;
      let end = i + m[0].length;
      if (s[end] === '&' || s[end] === '%') end++;
      const hex = m[1]!.toLowerCase() === 'h';
      push('num', (hex ? '0x' : '0o') + m[2]!, i, end, line);
      i = end;
      continue;
    }
    if (isDigit(c) || (c === '.' && i + 1 < n && isDigit(s[i + 1]!))) {
      const m = /^(\d+\.?\d*|\.\d+)([eEdD][+-]?\d+)?/.exec(s.slice(i, i + 64))!;
      let end = i + m[0].length;
      // Type-suffix characters ([MS-VBAL] §3.3.2); `&` only where it cannot be an operator.
      const next = s[end] ?? '';
      if ('#!@%^'.includes(next) && next !== '') {
        if (next !== '#' || !/^#[0-9A-Za-z/\-:., ]{1,40}#/.test(s.slice(end, end + 42))) end++;
      } else if (next === '&' && /^&\s*($|[),:\r\n])/.test(s.slice(end, end + 8))) {
        end++;
      }
      const value = Number(m[0].replace(/[dD]/, 'e'));
      push('num', Number.isFinite(value) ? String(value) : '0', i, end, line);
      i = end;
      continue;
    }
    if (isIdStart(c)) {
      let p = i + 1;
      while (p < n && isIdPart(s[p]!)) p++;
      const word = s.slice(i, p);
      if (word.toLowerCase() === 'rem' && atStatementStart() && (p >= n || /\s/.test(s[p]!))) {
        const startLine = line;
        const end = restOfLine(p);
        push('comment', s.slice(p, end), i, end, startLine);
        i = end;
        continue;
      }
      // `$` / `%` type suffix (`Left$`, `i%`).
      const end = s[p] === '$' || s[p] === '%' ? p + 1 : p;
      push('id', word, i, end, line);
      i = end;
      continue;
    }
    if (c === '[') {
      // Foreign name `[…]` ([MS-VBAL] §3.3.5.3): kept as one name the converter cannot map.
      let p = i + 1;
      while (p < n && s[p] !== ']' && !isNewline(s[p]!)) p++;
      const end = s[p] === ']' ? p + 1 : p;
      push('id', s.slice(i, end), i, end, line);
      i = end;
      continue;
    }
    const two = s.slice(i, i + 2);
    if (TWO_CHAR_OPS.has(two)) {
      push(
        'op',
        two === '><' ? '<>' : two === '=<' ? '<=' : two === '=>' ? '>=' : two,
        i,
        i + 2,
        line,
      );
      i += 2;
      continue;
    }
    if (c === ':') {
      push('eos', ':', i, i + 1, line);
      i++;
      continue;
    }
    push('op', c, i, i + 1, line);
    i++;
  }
  push('eos', '\n', n, n, line);
  return out;
}

/** Source text of a token span as one line (continuations and line breaks folded). */
export function spanText(source: string, first: Token, last: Token): string {
  return source
    .slice(first.start, last.end)
    .replace(/\s+_[ \t]*(\r\n|[\r\n\u2028\u2029])\s*/g, ' ')
    .replace(/[\r\n\u2028\u2029]+/g, ' ')
    .trim();
}
