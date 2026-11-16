//! Content stream tokenizer (ISO 32000-1, §7.2 lexical conventions, §7.8.2 content streams,
//! §8.9.7 inline images). Operands are `lopdf::Object`s; malformed input never panics: an
//! unreadable byte is skipped.
use lopdf::{Dictionary, Object, StringFormat};

/// One operator with its operands.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Op {
    /// Operator name (`Tj`, `re`, `BI` for an inline image…).
    pub name: Vec<u8>,
    /// Operands in order.
    pub args: Vec<Object>,
    /// Inline image data (`BI … ID data EI`), with its dictionary as the only operand.
    pub data: Option<Vec<u8>>,
}

fn is_white(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | b'\x0c' | b'\0')
}

fn is_delim(c: u8) -> bool {
    matches!(
        c,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// Tokenizer over one content stream.
pub(crate) struct Lexer<'a> {
    s: &'a [u8],
    pos: usize,
    /// Where the last returned operator's first operand started.
    start: usize,
}

pub(crate) enum Tok {
    Obj(Object),
    ArrayStart,
    ArrayEnd,
    DictStart,
    DictEnd,
    Op(Vec<u8>),
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(s: &'a [u8]) -> Self {
        Self {
            s,
            pos: 0,
            start: 0,
        }
    }

    /// A tokenizer over `s` starting at `pos` (the object reader of [`crate::store`]).
    pub(crate) fn at(s: &'a [u8], pos: usize) -> Self {
        Self { s, pos, start: pos }
    }

    /// Current byte position.
    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// Move to byte position `pos`.
    pub(crate) fn seek(&mut self, pos: usize) {
        self.pos = pos.min(self.s.len());
    }

    /// Byte range of the last returned operator with its operands.
    pub(crate) fn span(&self) -> (usize, usize) {
        (self.start, self.pos)
    }

    pub(crate) fn skip_white(&mut self) {
        while let Some(&c) = self.s.get(self.pos) {
            if is_white(c) {
                self.pos += 1;
            } else if c == b'%' {
                while let Some(&c) = self.s.get(self.pos) {
                    if c == b'\r' || c == b'\n' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    pub(crate) fn token(&mut self) -> Option<Tok> {
        loop {
            self.skip_white();
            let c = *self.s.get(self.pos)?;
            match c {
                b'[' => {
                    self.pos += 1;
                    return Some(Tok::ArrayStart);
                }
                b']' => {
                    self.pos += 1;
                    return Some(Tok::ArrayEnd);
                }
                b'<' if self.s.get(self.pos + 1) == Some(&b'<') => {
                    self.pos += 2;
                    return Some(Tok::DictStart);
                }
                b'>' if self.s.get(self.pos + 1) == Some(&b'>') => {
                    self.pos += 2;
                    return Some(Tok::DictEnd);
                }
                b'<' => return Some(Tok::Obj(self.hex_string())),
                b'(' => return Some(Tok::Obj(self.literal_string())),
                b'/' => return Some(Tok::Obj(self.name())),
                b'0'..=b'9' | b'+' | b'-' | b'.' => {
                    if let Some(n) = self.number() {
                        return Some(Tok::Obj(n));
                    }
                }
                b'{' | b'}' | b')' | b'>' => {
                    // PostScript braces (type 4 functions only) and stray closers.
                    self.pos += 1;
                }
                _ => {
                    let start = self.pos;
                    while let Some(&c) = self.s.get(self.pos) {
                        if is_white(c) || is_delim(c) {
                            break;
                        }
                        self.pos += 1;
                    }
                    if self.pos == start {
                        self.pos += 1;
                        continue;
                    }
                    let word = &self.s[start..self.pos];
                    return Some(match word {
                        b"true" => Tok::Obj(Object::Boolean(true)),
                        b"false" => Tok::Obj(Object::Boolean(false)),
                        b"null" => Tok::Obj(Object::Null),
                        _ => Tok::Op(word.to_vec()),
                    });
                }
            }
        }
    }

    fn number(&mut self) -> Option<Object> {
        let start = self.pos;
        let mut end = self.pos;
        let mut real = false;
        while let Some(&c) = self.s.get(end) {
            match c {
                b'0'..=b'9' => {}
                b'.' => real = true,
                b'+' | b'-' if end == start => {}
                // Some producers write "--5" or "0-5"; stop at a sign inside the number.
                _ => break,
            }
            end += 1;
        }
        // Skip extra signs ("--1").
        let mut text = &self.s[start..end];
        while text.len() > 1 && matches!(text[0], b'+' | b'-') && matches!(text[1], b'+' | b'-') {
            text = &text[1..];
        }
        self.pos = end.max(start + 1);
        let s = std::str::from_utf8(text).ok()?;
        if !real {
            if let Ok(i) = s.parse::<i64>() {
                return Some(Object::Integer(i));
            }
        }
        let v = match s {
            "" | "+" | "-" | "." | "-." | "+." => 0.0,
            _ => s.parse::<f32>().ok()?,
        };
        Some(Object::Real(v))
    }

    fn name(&mut self) -> Object {
        self.pos += 1;
        let mut out = Vec::new();
        while let Some(&c) = self.s.get(self.pos) {
            if is_white(c) || is_delim(c) {
                break;
            }
            if c == b'#' {
                let hex = self.s.get(self.pos + 1..self.pos + 3);
                if let Some(v) = hex
                    .and_then(|h| std::str::from_utf8(h).ok())
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    out.push(v);
                    self.pos += 3;
                    continue;
                }
            }
            out.push(c);
            self.pos += 1;
        }
        Object::Name(out)
    }

    fn hex_string(&mut self) -> Object {
        self.pos += 1;
        let mut out = Vec::new();
        let mut hi: Option<u8> = None;
        while let Some(&c) = self.s.get(self.pos) {
            self.pos += 1;
            if c == b'>' {
                break;
            }
            let v = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => continue,
            };
            match hi.take() {
                Some(h) => out.push(h << 4 | v),
                None => hi = Some(v),
            }
        }
        if let Some(h) = hi {
            out.push(h << 4);
        }
        Object::String(out, StringFormat::Hexadecimal)
    }

    fn literal_string(&mut self) -> Object {
        self.pos += 1;
        let mut out = Vec::new();
        let mut depth = 1_u32;
        while let Some(&c) = self.s.get(self.pos) {
            self.pos += 1;
            match c {
                b'(' => {
                    depth += 1;
                    out.push(c);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                b'\\' => {
                    let Some(&e) = self.s.get(self.pos) else {
                        break;
                    };
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'\r' => {
                            if self.s.get(self.pos) == Some(&b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.s.get(self.pos) {
                                    Some(&d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push((v & 0xff) as u8);
                        }
                        other => out.push(other),
                    }
                }
                _ => out.push(c),
            }
        }
        Object::String(out, StringFormat::Literal)
    }

    /// Parse one object that starts with `first` (arrays and dictionaries nest).
    fn object(&mut self, first: Tok, depth: u32) -> Option<Object> {
        if depth > 32 {
            return None;
        }
        match first {
            Tok::Obj(o) => Some(o),
            Tok::ArrayStart => {
                let mut items = Vec::new();
                while let Some(t) = self.token() {
                    match t {
                        Tok::ArrayEnd => break,
                        Tok::Op(_) | Tok::DictEnd => {}
                        t => {
                            if let Some(o) = self.object(t, depth + 1) {
                                items.push(o);
                            }
                        }
                    }
                }
                Some(Object::Array(items))
            }
            Tok::DictStart => {
                let mut d = Dictionary::new();
                let mut key: Option<Vec<u8>> = None;
                while let Some(t) = self.token() {
                    match t {
                        Tok::DictEnd => break,
                        Tok::Op(_) | Tok::ArrayEnd => {}
                        t => {
                            let Some(o) = self.object(t, depth + 1) else {
                                continue;
                            };
                            match key.take() {
                                None => {
                                    if let Object::Name(n) = o {
                                        key = Some(n);
                                    }
                                }
                                Some(k) => d.set(k, o),
                            }
                        }
                    }
                }
                Some(Object::Dictionary(d))
            }
            Tok::ArrayEnd | Tok::DictEnd | Tok::Op(_) => None,
        }
    }

    /// Inline image: `BI` was read; parse the dictionary up to `ID`, then the data up to `EI`.
    fn inline_image(&mut self) -> Op {
        let mut d = Dictionary::new();
        let mut key: Option<Vec<u8>> = None;
        while let Some(t) = self.token() {
            if let Tok::Op(ref w) = t {
                if w == b"ID" {
                    break;
                }
                continue;
            }
            let Some(o) = self.object(t, 1) else { continue };
            match key.take() {
                None => {
                    if let Object::Name(n) = o {
                        key = Some(n);
                    }
                }
                Some(k) => d.set(k, o),
            }
        }
        // One white-space byte follows ID.
        if self.s.get(self.pos).is_some_and(|&c| is_white(c)) {
            self.pos += 1;
        }
        let start = self.pos;
        let mut end = self.s.len();
        let mut i = start;
        while i + 1 < self.s.len() {
            if self.s[i] == b'E'
                && self.s[i + 1] == b'I'
                && (i == start || is_white(self.s[i - 1]))
                && self
                    .s
                    .get(i + 2)
                    .is_none_or(|&c| is_white(c) || is_delim(c))
                && looks_like_operators(&self.s[i + 2..])
            {
                end = i;
                break;
            }
            i += 1;
        }
        let mut data = self.s[start..end].to_vec();
        // Drop the white space before EI.
        if data.last().is_some_and(|&c| is_white(c)) && end < self.s.len() {
            data.pop();
        }
        self.pos = (end + 2).min(self.s.len());
        Op {
            name: b"BI".to_vec(),
            args: vec![Object::Dictionary(d)],
            data: Some(data),
        }
    }
}

/// Whether the bytes after a candidate `EI` read like more content (ASCII tokens), not binary.
fn looks_like_operators(rest: &[u8]) -> bool {
    rest.iter()
        .take(32)
        .all(|&c| c.is_ascii_graphic() || is_white(c))
}

impl Iterator for Lexer<'_> {
    type Item = Op;

    fn next(&mut self) -> Option<Op> {
        let mut args = Vec::new();
        self.skip_white();
        self.start = self.pos;
        loop {
            let t = self.token()?;
            match t {
                Tok::Op(name) => {
                    if name == b"BI" {
                        return Some(self.inline_image());
                    }
                    return Some(Op {
                        name,
                        args,
                        data: None,
                    });
                }
                Tok::ArrayEnd | Tok::DictEnd => {}
                t => {
                    if let Some(o) = self.object(t, 0) {
                        // Guard against absurd operand runs in garbage streams.
                        if args.len() < 4096 {
                            args.push(o);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops(s: &[u8]) -> Vec<Op> {
        Lexer::new(s).collect()
    }

    #[test]
    fn operators_and_operands() {
        let v = ops(b"q 1 0 0 1 72.5 -.5 cm /F1 12 Tf (a\\(b\\)\\101) Tj [<0041> -250 (c)] TJ Q");
        let names: Vec<&[u8]> = v.iter().map(|o| o.name.as_slice()).collect();
        assert_eq!(names, [&b"q"[..], b"cm", b"Tf", b"Tj", b"TJ", b"Q"]);
        assert_eq!(v[1].args[4], Object::Real(72.5));
        assert_eq!(v[1].args[5], Object::Real(-0.5));
        assert_eq!(v[3].args[0].as_str().ok(), Some(&b"a(b)A"[..]));
        let Object::Array(a) = &v[4].args[0] else {
            panic!("array")
        };
        assert_eq!(a[0].as_str().ok(), Some(&[0, 0x41][..]));
        assert_eq!(a[1], Object::Integer(-250));
    }

    #[test]
    fn inline_image_and_dicts() {
        let v = ops(b"/P <</MCID 3>> BDC BI /W 2 /H 1 /BPC 8 /CS /G ID \x00\xff\nEI\n EMC");
        assert_eq!(v[0].name, b"BDC");
        assert!(matches!(v[0].args[1], Object::Dictionary(_)));
        assert_eq!(v[1].name, b"BI");
        assert_eq!(v[1].data.as_deref(), Some(&b"\x00\xff"[..]));
        assert_eq!(v[2].name, b"EMC");
    }

    #[test]
    fn garbage_does_not_panic() {
        for s in [
            &b"((((("[..],
            b"<<<<[[[[",
            b")))]]]>>>>",
            b"BI /W ID",
            b"/#4",
            b"-- . +",
            b"\\",
        ] {
            let _ = ops(s);
        }
    }
}
