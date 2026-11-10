//! A minimal reader for the PDF files this crate writes (classic cross-reference
//! table, direct `/Length`, Flate streams), used to check exported structure:
//! objects by number, dictionary entries, decoded streams (ISO 32000-1 §7.3,
//! §7.5). It is not a general PDF parser.

use std::collections::BTreeMap;

/// One indirect object: its dictionary (or other value) text and stream bytes.
#[derive(Debug, Clone)]
pub struct Object {
    /// Object number.
    pub num: u32,
    /// Text of the value before `stream` (the whole value for non-streams).
    pub dict: String,
    /// Raw (still encoded) stream data.
    pub stream: Option<Vec<u8>>,
}

/// Parsed objects of a file plus its trailer dictionary text.
#[derive(Debug, Clone)]
pub struct Probe {
    /// Objects by number.
    pub objects: BTreeMap<u32, Object>,
    /// Trailer dictionary text.
    pub trailer: String,
    /// Header line (`%PDF-1.x`).
    pub header: String,
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

impl Probe {
    /// Parse `data`; `None` when it does not look like a PDF.
    #[must_use]
    pub fn parse(data: &[u8]) -> Option<Probe> {
        if !data.starts_with(b"%PDF-") {
            return None;
        }
        let header_end = data.iter().position(|&b| b == b'\n')?;
        let header = String::from_utf8_lossy(&data[..header_end]).into_owned();
        let mut objects = BTreeMap::new();
        let mut i = 0;
        while let Some(at) = find(data, b" 0 obj", i) {
            // Object number: digits right before " 0 obj", at a line start.
            let mut s = at;
            while s > 0 && data[s - 1].is_ascii_digit() {
                s -= 1;
            }
            let line_start = s == 0 || matches!(data[s - 1], b'\n' | b'\r');
            let body = at + b" 0 obj".len();
            if !line_start || s == at {
                i = body;
                continue;
            }
            let num: u32 = std::str::from_utf8(&data[s..at]).ok()?.parse().ok()?;
            let end = find(data, b"endobj", body)?;
            let stream_kw = find(data, b"stream", body).filter(|&p| p < end);
            let (dict, stream, next) = match stream_kw {
                Some(sk) if !data[..sk].ends_with(b"end") => {
                    let dict = String::from_utf8_lossy(&data[body..sk]).trim().to_owned();
                    let mut start = sk + b"stream".len();
                    if data.get(start) == Some(&b'\r') {
                        start += 1;
                    }
                    if data.get(start) == Some(&b'\n') {
                        start += 1;
                    }
                    let len = dict_get(&dict, "Length")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    let stop = (start + len).min(data.len());
                    let after = find(data, b"endobj", stop).unwrap_or(data.len());
                    (dict, Some(data[start..stop].to_vec()), after)
                }
                _ => (
                    String::from_utf8_lossy(&data[body..end]).trim().to_owned(),
                    None,
                    end,
                ),
            };
            objects.insert(num, Object { num, dict, stream });
            i = next + b"endobj".len();
        }
        let t = find(data, b"trailer", 0)
            .map(|p| String::from_utf8_lossy(&data[p + 7..]).into_owned())
            .unwrap_or_default();
        let trailer = t.split("startxref").next().unwrap_or("").trim().to_owned();
        Some(Probe {
            objects,
            trailer,
            header,
        })
    }

    /// Object `num`.
    #[must_use]
    pub fn get(&self, num: u32) -> Option<&Object> {
        self.objects.get(&num)
    }

    /// Object an indirect reference value (`12 0 R`) points to.
    #[must_use]
    pub fn deref(&self, value: &str) -> Option<&Object> {
        self.get(parse_ref(value)?)
    }

    /// The document catalog (`/Root` of the trailer).
    #[must_use]
    pub fn catalog(&self) -> Option<&Object> {
        self.deref(&dict_get(&self.trailer, "Root")?)
    }

    /// Objects whose dictionary has `/Type /<ty>`.
    pub fn of_type<'a>(&'a self, ty: &'a str) -> impl Iterator<Item = &'a Object> + 'a {
        self.objects.values().filter(move |o| {
            dict_get(&o.dict, "Type").is_some_and(|t| t.trim() == format!("/{ty}"))
        })
    }

    /// Decoded stream of `o` (Flate-decoded when `/Filter /FlateDecode`).
    #[must_use]
    pub fn stream_data(o: &Object) -> Option<Vec<u8>> {
        let raw = o.stream.as_ref()?;
        if dict_get(&o.dict, "Filter").is_some_and(|f| f.contains("FlateDecode")) {
            miniz_oxide::inflate::decompress_to_vec_zlib(raw).ok()
        } else {
            Some(raw.clone())
        }
    }

    /// Pages in order (following `/Kids` of the page tree root).
    #[must_use]
    pub fn pages(&self) -> Vec<&Object> {
        let Some(root) = self
            .catalog()
            .and_then(|c| dict_get(&c.dict, "Pages"))
            .and_then(|v| self.deref(&v))
        else {
            return Vec::new();
        };
        dict_get(&root.dict, "Kids")
            .map(|k| refs(&k).into_iter().filter_map(|r| self.get(r)).collect())
            .unwrap_or_default()
    }
}

fn is_delim(c: u8) -> bool {
    c.is_ascii_whitespace() || b"/<>[]()".contains(&c)
}

/// End of the literal string starting at `i` (`(` … `)` with escapes and nesting).
fn string_end(s: &[u8], mut i: usize) -> usize {
    let mut d = 0;
    while i < s.len() {
        match s[i] {
            b'\\' => i += 1,
            b'(' => d += 1,
            b')' => {
                d -= 1;
                if d == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    s.len()
}

/// End of a plain token (number, keyword) starting at `i`.
fn token_end(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && !is_delim(s[i]) {
        i += 1;
    }
    i
}

fn skip_ws(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// End of the value starting at `i` of `s` (dictionaries, arrays, strings,
/// names; `n g R` counts as one value).
fn value_end(s: &[u8], i: usize) -> usize {
    match s.get(i) {
        None => s.len(),
        Some(b'(') => string_end(s, i),
        Some(b'/') => token_end(s, i + 1),
        Some(b'<') if s.get(i + 1) == Some(&b'<') => {
            let mut j = i + 2;
            loop {
                j = skip_ws(s, j);
                match s.get(j) {
                    None => return s.len(),
                    Some(b'>') if s.get(j + 1) == Some(&b'>') => return j + 2,
                    _ => j = value_end(s, j).max(j + 1),
                }
            }
        }
        Some(b'<') => s[i..]
            .iter()
            .position(|&c| c == b'>')
            .map_or(s.len(), |p| i + p + 1),
        Some(b'[') => {
            let mut j = i + 1;
            loop {
                j = skip_ws(s, j);
                match s.get(j) {
                    None => return s.len(),
                    Some(b']') => return j + 1,
                    _ => j = value_end(s, j).max(j + 1),
                }
            }
        }
        Some(_) => {
            let e = token_end(s, i);
            // `n g R` reference.
            let is_int = |a: usize, b: usize| a < b && s[a..b].iter().all(u8::is_ascii_digit);
            if is_int(i, e) {
                let g0 = skip_ws(s, e);
                let g1 = token_end(s, g0);
                if g0 > e && is_int(g0, g1) {
                    let r0 = skip_ws(s, g1);
                    if r0 > g1 && s.get(r0) == Some(&b'R') && token_end(s, r0) == r0 + 1 {
                        return r0 + 1;
                    }
                }
            }
            e
        }
    }
}

/// Top-level entries of a dictionary text `<< /K v … >>` as (key, value text).
#[must_use]
pub fn dict_entries(dict: &str) -> Vec<(String, String)> {
    let b = dict.as_bytes();
    let Some(open) = dict.find("<<") else {
        return Vec::new();
    };
    let mut i = open + 2;
    let mut out = Vec::new();
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] == b'>' {
            break;
        }
        if b[i] != b'/' {
            i += 1;
            continue;
        }
        let ks = i + 1;
        let mut ke = ks;
        while ke < b.len() && !b[ke].is_ascii_whitespace() && !b"/<[(>]".contains(&b[ke]) {
            ke += 1;
        }
        let key = dict[ks..ke].to_owned();
        let mut vs = ke;
        while vs < b.len() && b[vs].is_ascii_whitespace() {
            vs += 1;
        }
        let ve = value_end(b, vs);
        out.push((key, dict[vs..ve.min(dict.len())].trim().to_owned()));
        i = ve.max(vs + 1);
    }
    out
}

/// Value of top-level `key` in a dictionary text.
#[must_use]
pub fn dict_get(dict: &str, key: &str) -> Option<String> {
    dict_entries(dict)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

/// Object number of a reference value `12 0 R`.
#[must_use]
pub fn parse_ref(v: &str) -> Option<u32> {
    let mut it = v.split_ascii_whitespace();
    let n = it.next()?.parse().ok()?;
    it.next()?;
    (it.next()? == "R").then_some(n)
}

/// Every reference `n 0 R` in a value text (arrays of references).
#[must_use]
pub fn refs(v: &str) -> Vec<u32> {
    let w: Vec<&str> = v
        .trim_matches(|c| c == '[' || c == ']')
        .split_ascii_whitespace()
        .collect();
    w.windows(3)
        .filter(|t| t[2] == "R")
        .filter_map(|t| t[0].parse().ok())
        .collect()
}

/// Decode a PDF text string value: `(literal)` in PDFDocEncoding (ASCII subset
/// with escapes) or `<FEFF…>` UTF-16BE hex.
#[must_use]
pub fn text(v: &str) -> Option<String> {
    let v = v.trim();
    if let Some(h) = v.strip_prefix('<').and_then(|h| h.strip_suffix('>')) {
        let bytes: Vec<u8> = (0..h.len() / 2)
            .filter_map(|i| u8::from_str_radix(h.get(i * 2..i * 2 + 2)?, 16).ok())
            .collect();
        return utf16(&bytes).or_else(|| Some(String::from_utf8_lossy(&bytes).into_owned()));
    }
    let inner = v.strip_prefix('(')?.strip_suffix(')')?;
    let mut out = Vec::new();
    let b = inner.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            i += 1;
            match b[i] {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'0'..=b'7' => {
                    let mut n = 0_u32;
                    let mut k = 0;
                    while k < 3 && i < b.len() && (b'0'..=b'7').contains(&b[i]) {
                        n = n * 8 + u32::from(b[i] - b'0');
                        i += 1;
                        k += 1;
                    }
                    out.push(n as u8);
                    continue;
                }
                c => out.push(c),
            }
        } else {
            out.push(b[i]);
        }
        i += 1;
    }
    utf16(&out).or_else(|| Some(out.iter().map(|&c| char::from(c)).collect()))
}

fn utf16(b: &[u8]) -> Option<String> {
    let rest = b.strip_prefix(&[0xFE, 0xFF])?;
    let units: Vec<u16> = rest
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_and_values() {
        let d = "<< /Type /Page /Kids [3 0 R 4 0 R] /Res << /F << /F1 5 0 R >> >> /T (a\\)b) /P 7 0 R >>";
        let e = dict_entries(d);
        let keys: Vec<&str> = e.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["Type", "Kids", "Res", "T", "P"]);
        assert_eq!(dict_get(d, "Type").as_deref(), Some("/Page"));
        assert_eq!(refs(&dict_get(d, "Kids").unwrap()), [3, 4]);
        assert_eq!(parse_ref(&dict_get(d, "P").unwrap()), Some(7));
        assert_eq!(text(&dict_get(d, "T").unwrap()).as_deref(), Some("a)b"));
        assert_eq!(text("<FEFF0041>").as_deref(), Some("A"));
    }
}
