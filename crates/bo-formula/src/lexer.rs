//! Formula tokenizer (ECMA-376 Part 1, §18.17.2 formula lexical grammar).

use crate::ast::{BinaryOp, StructItem, StructRef};
use crate::parser::{ParseError, ParseOptions, RefStyle};
use crate::reference::{
    parse_row_number, scan_a1_cell, scan_col_only, scan_row_only, CellAddr, RefKind, Reference,
    SheetRef, MAX_COLS, MAX_ROWS,
};
use crate::value::ErrorKind;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorKind),
    Ref(Reference),
    Name(Option<SheetRef>, String),
    Func(String),
    Struct(StructRef),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Colon,
    Percent,
    At,
    /// Postfix `#` after a cell reference (spill range).
    Hash,
    Op(BinaryOp),
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub tok: Tok,
    pub space_before: bool,
    pub pos: usize,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '\\'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '\\' || c == '?'
}

pub(crate) struct Lexer<'a> {
    src: &'a str,
    pos: usize,
    opts: &'a ParseOptions,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(src: &'a str, opts: &'a ParseOptions) -> Self {
        Lexer { src, pos: 0, opts }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn err(&self, msg: &str) -> ParseError {
        ParseError {
            pos: self.pos,
            message: msg.to_owned(),
        }
    }

    pub(crate) fn tokenize(mut self) -> Result<Vec<Token>, ParseError> {
        let mut out = Vec::new();
        loop {
            let mut space = false;
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() {
                    space = true;
                    self.pos += c.len_utf8();
                } else {
                    break;
                }
            }
            let Some(c) = self.peek_char() else { break };
            let start = self.pos;
            let tok = self.next_token(c)?;
            let is_ref = matches!(tok, Tok::Ref(_));
            out.push(Token {
                tok,
                space_before: space,
                pos: start,
            });
            // `A1#`: a '#' right after a reference that does not start an error literal.
            if is_ref && self.peek_char() == Some('#') {
                let save = self.pos;
                if self.error_literal().is_err() {
                    self.pos = save + 1;
                    out.push(Token {
                        tok: Tok::Hash,
                        space_before: false,
                        pos: save,
                    });
                } else {
                    self.pos = save;
                }
            }
        }
        Ok(out)
    }

    fn next_token(&mut self, c: char) -> Result<Tok, ParseError> {
        let simple = match c {
            '(' => Some(Tok::LParen),
            ')' => Some(Tok::RParen),
            '{' => Some(Tok::LBrace),
            '}' => Some(Tok::RBrace),
            ',' => Some(Tok::Comma),
            ';' => Some(Tok::Semi),
            ':' => Some(Tok::Colon),
            '%' => Some(Tok::Percent),
            '@' => Some(Tok::At),
            '+' => Some(Tok::Op(BinaryOp::Add)),
            '-' => Some(Tok::Op(BinaryOp::Sub)),
            '*' => Some(Tok::Op(BinaryOp::Mul)),
            '/' => Some(Tok::Op(BinaryOp::Div)),
            '^' => Some(Tok::Op(BinaryOp::Pow)),
            '&' => Some(Tok::Op(BinaryOp::Concat)),
            '=' => Some(Tok::Op(BinaryOp::Eq)),
            _ => None,
        };
        if let Some(t) = simple {
            self.pos += 1;
            return Ok(t);
        }
        match c {
            '<' => {
                let r = self.rest();
                if r.starts_with("<=") {
                    self.pos += 2;
                    Ok(Tok::Op(BinaryOp::Le))
                } else if r.starts_with("<>") {
                    self.pos += 2;
                    Ok(Tok::Op(BinaryOp::Ne))
                } else {
                    self.pos += 1;
                    Ok(Tok::Op(BinaryOp::Lt))
                }
            }
            '>' => {
                if self.rest().starts_with(">=") {
                    self.pos += 2;
                    Ok(Tok::Op(BinaryOp::Ge))
                } else {
                    self.pos += 1;
                    Ok(Tok::Op(BinaryOp::Gt))
                }
            }
            '"' => self.string(),
            '#' => self.error_literal(),
            '\'' => {
                let sheet = self.quoted_sheet()?;
                self.after_sheet(sheet)
            }
            '[' => {
                let items = self.struct_body()?;
                Ok(Tok::Struct(StructRef { table: None, items }))
            }
            _ => self.word(),
        }
    }

    fn string(&mut self) -> Result<Tok, ParseError> {
        self.pos += 1;
        let mut s = String::new();
        loop {
            let Some(c) = self.peek_char() else {
                return Err(self.err("unterminated string"));
            };
            self.pos += c.len_utf8();
            if c == '"' {
                if self.peek_char() == Some('"') {
                    self.pos += 1;
                    s.push('"');
                } else {
                    return Ok(Tok::Text(s));
                }
            } else {
                s.push(c);
            }
        }
    }

    fn error_literal(&mut self) -> Result<Tok, ParseError> {
        let rest = self.rest();
        let mut best: Option<ErrorKind> = None;
        for e in ErrorKind::ALL {
            let s = e.as_str();
            if rest.len() >= s.len()
                && rest.is_char_boundary(s.len())
                && rest[..s.len()].eq_ignore_ascii_case(s)
                && best.is_none_or(|b| b.as_str().len() < s.len())
            {
                best = Some(e);
            }
        }
        match best {
            Some(e) => {
                self.pos += e.as_str().len();
                Ok(Tok::Error(e))
            }
            None => Err(self.err("unknown error literal")),
        }
    }

    /// `'Sheet name'!` or `'Sheet1:Sheet3'!`.
    fn quoted_sheet(&mut self) -> Result<SheetRef, ParseError> {
        self.pos += 1;
        let mut name = String::new();
        loop {
            let Some(c) = self.peek_char() else {
                return Err(self.err("unterminated sheet name"));
            };
            self.pos += c.len_utf8();
            if c == '\'' {
                if self.peek_char() == Some('\'') {
                    self.pos += 1;
                    name.push('\'');
                } else {
                    break;
                }
            } else {
                name.push(c);
            }
        }
        if self.peek_char() != Some('!') {
            return Err(self.err("expected '!' after sheet name"));
        }
        self.pos += 1;
        Ok(match name.split_once(':') {
            Some((a, b)) => SheetRef {
                first: a.to_owned(),
                last: Some(b.to_owned()),
            },
            None => SheetRef::single(name),
        })
    }

    /// Token following a sheet qualifier: a reference, a name or `#REF!`.
    fn after_sheet(&mut self, sheet: SheetRef) -> Result<Tok, ParseError> {
        if self.peek_char() == Some('#') {
            return self.error_literal();
        }
        if let Some((kind, len)) = self.scan_ref(self.rest()) {
            self.pos += len;
            return Ok(Tok::Ref(Reference {
                sheet: Some(sheet),
                kind,
            }));
        }
        let ident = self.ident();
        if ident.is_empty() {
            return Err(self.err("expected reference after sheet name"));
        }
        Ok(Tok::Name(Some(sheet), ident.to_owned()))
    }

    fn ident(&mut self) -> &'a str {
        let rest = self.rest();
        let len = rest
            .char_indices()
            .find(|&(_, c)| !is_ident_char(c))
            .map_or(rest.len(), |(i, _)| i);
        self.pos += len;
        &rest[..len]
    }

    /// Try an unquoted sheet qualifier `Name!` / `Name1:Name2!` at the current position.
    fn try_unquoted_sheet(&mut self) -> Option<SheetRef> {
        let rest = self.rest();
        let name_len = |s: &str| {
            s.char_indices()
                .find(|&(_, c)| !(c.is_alphanumeric() || c == '_' || c == '.'))
                .map_or(s.len(), |(i, _)| i)
        };
        let n1 = name_len(rest);
        if n1 == 0 {
            return None;
        }
        let after = &rest[n1..];
        if after.starts_with('!') {
            self.pos += n1 + 1;
            return Some(SheetRef::single(&rest[..n1]));
        }
        if let Some(tail) = after.strip_prefix(':') {
            let n2 = name_len(tail);
            if n2 > 0 && tail[n2..].starts_with('!') {
                self.pos += n1 + 1 + n2 + 1;
                return Some(SheetRef {
                    first: rest[..n1].to_owned(),
                    last: Some(tail[..n2].to_owned()),
                });
            }
        }
        None
    }

    fn word(&mut self) -> Result<Tok, ParseError> {
        let c = self.peek_char().unwrap_or(' ');
        if c.is_alphanumeric() || c == '_' || c == '.' {
            if let Some(sheet) = self.try_unquoted_sheet() {
                return self.after_sheet(sheet);
            }
        }
        if let Some((kind, len)) = self.scan_ref(self.rest()) {
            self.pos += len;
            return Ok(Tok::Ref(Reference { sheet: None, kind }));
        }
        if c.is_ascii_digit() || c == '.' {
            return self.number();
        }
        if !(is_ident_start(c) || c == '$') {
            return Err(self.err(&format!("unexpected character '{c}'")));
        }
        let ident = self.ident();
        if ident.is_empty() {
            return Err(self.err("unexpected character"));
        }
        match self.peek_char() {
            Some('(') => Ok(Tok::Func(normalize_function_name(ident))),
            Some('[') => {
                let items = self.struct_body()?;
                Ok(Tok::Struct(StructRef {
                    table: Some(ident.to_owned()),
                    items,
                }))
            }
            _ if ident.eq_ignore_ascii_case("TRUE") => Ok(Tok::Bool(true)),
            _ if ident.eq_ignore_ascii_case("FALSE") => Ok(Tok::Bool(false)),
            _ => Ok(Tok::Name(None, ident.to_owned())),
        }
    }

    fn number(&mut self) -> Result<Tok, ParseError> {
        let b = self.rest().as_bytes();
        let mut i = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            let ds = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > ds {
                i = j;
            }
        }
        let text = &self.rest()[..i];
        let n: f64 = text.parse().map_err(|_| self.err("invalid number"))?;
        if !n.is_finite() {
            return Err(self.err("number out of range"));
        }
        self.pos += i;
        Ok(Tok::Number(n))
    }

    /// Scan a reference token (A1 or R1C1 per options) at the start of `s`.
    fn scan_ref(&self, s: &str) -> Option<(RefKind, usize)> {
        let found = match self.opts.style {
            RefStyle::A1 => scan_a1_ref(s.as_bytes()),
            RefStyle::R1C1 => {
                scan_r1c1_ref(s.as_bytes(), self.opts.origin_row, self.opts.origin_col)
            }
        }?;
        // A reference must not run into an identifier or a call: `LOG10(`, `A1B`.
        let next = s[found.1..].chars().next();
        if next.is_some_and(|c| is_ident_char(c) || c == '(' || c == '[' || c == '!') {
            return None;
        }
        Some(found)
    }

    /// Parse a bracketed structured-reference body starting at `[`.
    fn struct_body(&mut self) -> Result<Vec<StructItem>, ParseError> {
        let rest = self.rest();
        let b = rest.as_bytes();
        let mut depth = 0usize;
        let mut i = 0;
        let mut end = None;
        while i < b.len() {
            match b[i] {
                b'\'' => i += 1,
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let end = end.ok_or_else(|| self.err("unterminated structured reference"))?;
        let body = &rest[1..end];
        let items = parse_struct_items(body).ok_or_else(|| self.err("bad structured reference"))?;
        self.pos += end + 1;
        Ok(items)
    }
}

/// Upper-case a function name and strip storage prefixes (`_xlfn.`, `_xlws.`).
pub(crate) fn normalize_function_name(name: &str) -> String {
    let mut up = name.to_uppercase();
    for p in ["_XLFN._XLWS.", "_XLFN.", "_XLWS.", "_XLUDF."] {
        if let Some(r) = up.strip_prefix(p) {
            up = r.to_owned();
            break;
        }
    }
    up
}

fn scan_a1_ref(b: &[u8]) -> Option<(RefKind, usize)> {
    if let Some((a, n)) = scan_a1_cell(b) {
        if b.get(n) == Some(&b':') {
            if let Some((c, m)) = scan_a1_cell(&b[n + 1..]) {
                return Some((RefKind::Range(a, c), n + 1 + m));
            }
        }
        return Some((RefKind::Cell(a), n));
    }
    if let Some(((first, fa), n)) = scan_col_only(b) {
        if b.get(n) == Some(&b':') {
            if let Some(((last, la), m)) = scan_col_only(&b[n + 1..]) {
                return Some((
                    RefKind::Cols {
                        first,
                        last,
                        first_abs: fa,
                        last_abs: la,
                    },
                    n + 1 + m,
                ));
            }
        }
    }
    if let Some(((first, fa), n)) = scan_row_only(b) {
        if b.get(n) == Some(&b':') {
            if let Some(((last, la), m)) = scan_row_only(&b[n + 1..]) {
                return Some((
                    RefKind::Rows {
                        first,
                        last,
                        first_abs: fa,
                        last_abs: la,
                    },
                    n + 1 + m,
                ));
            }
        }
    }
    None
}

/// One R1C1 coordinate part: `R`, `R5` or `R[-2]` (letter already consumed by caller).
/// Returns (value, absolute, length).
fn scan_r1c1_part(b: &[u8], origin: u32, max: u32) -> Option<(u32, bool, usize)> {
    if b.first() == Some(&b'[') {
        let mut i = 1;
        if b.get(i) == Some(&b'-') || b.get(i) == Some(&b'+') {
            i += 1;
        }
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if b.get(i) != Some(&b']') {
            return None;
        }
        let off: i64 = std::str::from_utf8(&b[1..i]).ok()?.parse().ok()?;
        let v = i64::from(origin) + off;
        if v < 0 || v >= i64::from(max) {
            return None;
        }
        return Some((v as u32, false, i + 1));
    }
    let n = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if n == 0 {
        return Some((origin, false, 0));
    }
    let v = parse_row_number(&b[..n])?;
    (v < max).then_some((v, true, n))
}

#[derive(Clone, Copy)]
enum R1c1Item {
    Cell(CellAddr),
    Row(u32, bool),
    Col(u32, bool),
}

fn scan_r1c1_item(b: &[u8], orow: u32, ocol: u32) -> Option<(R1c1Item, usize)> {
    let first = b.first()?.to_ascii_uppercase();
    if first == b'R' {
        let (row, row_abs, n) = scan_r1c1_part(&b[1..], orow, MAX_ROWS)?;
        let i = 1 + n;
        if b.get(i).map(u8::to_ascii_uppercase) == Some(b'C') {
            let (col, col_abs, m) = scan_r1c1_part(&b[i + 1..], ocol, MAX_COLS)?;
            return Some((
                R1c1Item::Cell(CellAddr {
                    row,
                    col,
                    row_abs,
                    col_abs,
                }),
                i + 1 + m,
            ));
        }
        return Some((R1c1Item::Row(row, row_abs), i));
    }
    if first == b'C' {
        let (col, col_abs, n) = scan_r1c1_part(&b[1..], ocol, MAX_COLS)?;
        return Some((R1c1Item::Col(col, col_abs), 1 + n));
    }
    None
}

fn scan_r1c1_ref(b: &[u8], orow: u32, ocol: u32) -> Option<(RefKind, usize)> {
    let (a, n) = scan_r1c1_item(b, orow, ocol)?;
    let second = if b.get(n) == Some(&b':') {
        scan_r1c1_item(&b[n + 1..], orow, ocol).map(|(x, m)| (x, n + 1 + m))
    } else {
        None
    };
    match (a, second) {
        (R1c1Item::Cell(x), Some((R1c1Item::Cell(y), len))) => Some((RefKind::Range(x, y), len)),
        (R1c1Item::Cell(x), _) => Some((RefKind::Cell(x), n)),
        (R1c1Item::Row(r1, a1), Some((R1c1Item::Row(r2, a2), len))) => Some((
            RefKind::Rows {
                first: r1,
                last: r2,
                first_abs: a1,
                last_abs: a2,
            },
            len,
        )),
        (R1c1Item::Row(r, a), _) => Some((
            RefKind::Rows {
                first: r,
                last: r,
                first_abs: a,
                last_abs: a,
            },
            n,
        )),
        (R1c1Item::Col(c1, a1), Some((R1c1Item::Col(c2, a2), len))) => Some((
            RefKind::Cols {
                first: c1,
                last: c2,
                first_abs: a1,
                last_abs: a2,
            },
            len,
        )),
        (R1c1Item::Col(c, a), _) => Some((
            RefKind::Cols {
                first: c,
                last: c,
                first_abs: a,
                last_abs: a,
            },
            n,
        )),
    }
}

/// Remove `'` escapes from a structured-reference column name.
fn unescape_column(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\'' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

fn special_item(s: &str) -> Option<StructItem> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("#All") {
        Some(StructItem::All)
    } else if t.eq_ignore_ascii_case("#Data") {
        Some(StructItem::Data)
    } else if t.eq_ignore_ascii_case("#Headers") {
        Some(StructItem::Headers)
    } else if t.eq_ignore_ascii_case("#Totals") {
        Some(StructItem::Totals)
    } else if t.eq_ignore_ascii_case("#This Row") {
        Some(StructItem::ThisRow)
    } else {
        None
    }
}

fn simple_item(s: &str) -> StructItem {
    special_item(s).unwrap_or_else(|| StructItem::Column(unescape_column(s)))
}

/// Split a list of `[a],[b]:[c]` into bracket contents and separators.
fn bracket_list(s: &str) -> Option<Vec<(String, char)>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut sep = ',';
    loop {
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        if b[i] != b'[' {
            return None;
        }
        let start = i + 1;
        let mut j = start;
        while j < b.len() && b[j] != b']' {
            if b[j] == b'\'' {
                j += 1;
            }
            j += 1;
        }
        if j >= b.len() {
            return None;
        }
        out.push((s[start..j].to_owned(), sep));
        i = j + 1;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        sep = match b[i] {
            b',' => ',',
            b':' => ':',
            _ => return None,
        };
        i += 1;
    }
    Some(out)
}

/// Parse the inside of a structured reference's outer brackets.
pub(crate) fn parse_struct_items(body: &str) -> Option<Vec<StructItem>> {
    let t = body.trim();
    if t.is_empty() {
        return Some(Vec::new());
    }
    if let Some(rest) = t.strip_prefix('@') {
        let rest = rest.trim();
        if rest.is_empty() {
            return Some(vec![StructItem::ThisRow]);
        }
        let mut items = vec![StructItem::ThisRow];
        if rest.starts_with('[') {
            items.extend(parse_struct_items(rest)?);
        } else {
            items.push(StructItem::Column(unescape_column(rest)));
        }
        return Some(items);
    }
    if !t.starts_with('[') {
        return Some(vec![simple_item(t)]);
    }
    let parts = bracket_list(t)?;
    let mut items: Vec<StructItem> = Vec::new();
    for (content, sep) in parts {
        let item = simple_item(&content);
        if sep == ':' {
            let prev = items.pop()?;
            match (prev, item) {
                (StructItem::Column(a), StructItem::Column(b)) => {
                    items.push(StructItem::ColumnRange(a, b));
                }
                _ => return None,
            }
        } else {
            items.push(item);
        }
    }
    Some(items)
}
