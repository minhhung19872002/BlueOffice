//! Type 1 font programs embedded with `/FontFile` (Adobe Type 1 Font Format, ch. 6–8;
//! ISO 32000-1, §9.9): cleartext header (`/FontMatrix`, `/Encoding`), eexec-encrypted private
//! part (`/Subrs`, `/CharStrings`) and the charstring interpreter, producing glyph outlines.
use bo_render::PathSeg;
use std::collections::HashMap;

/// A parsed Type 1 font.
#[derive(Debug, Clone, Default)]
pub(crate) struct Type1 {
    /// Glyph space to text space.
    pub matrix: [f32; 6],
    /// Built-in encoding: code to glyph name.
    pub encoding: HashMap<u8, String>,
    /// The font uses StandardEncoding.
    pub standard_encoding: bool,
    subrs: Vec<Vec<u8>>,
    glyphs: HashMap<String, Vec<u8>>,
}

fn decrypt(data: &[u8], mut r: u16, skip: usize) -> Vec<u8> {
    const C1: u16 = 52845;
    const C2: u16 = 22719;
    let mut out = Vec::with_capacity(data.len());
    for &c in data {
        out.push(c ^ (r >> 8) as u8);
        r = u16::from(c)
            .wrapping_add(r)
            .wrapping_mul(C1)
            .wrapping_add(C2);
    }
    out.drain(..skip.min(out.len()));
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from >= hay.len() || needle.is_empty() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn skip_white(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn read_int(s: &[u8], i: usize) -> Option<(i64, usize)> {
    let i = skip_white(s, i);
    let mut j = i;
    if j < s.len() && (s[j] == b'-' || s[j] == b'+') {
        j += 1;
    }
    while j < s.len() && s[j].is_ascii_digit() {
        j += 1;
    }
    let v = std::str::from_utf8(&s[i..j]).ok()?.parse().ok()?;
    Some((v, j))
}

fn read_token(s: &[u8], i: usize) -> (usize, usize) {
    let i = skip_white(s, i);
    let mut j = i;
    while j < s.len() && !s[j].is_ascii_whitespace() {
        j += 1;
    }
    (i, j)
}

/// Split a PFB file into its segments' payloads, or return the data unchanged.
fn unpfb(data: &[u8]) -> Vec<u8> {
    if data.first() != Some(&0x80) {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i + 6 <= data.len() && data[i] == 0x80 {
        let kind = data[i + 1];
        if kind == 3 {
            break;
        }
        let len = u32::from_le_bytes([data[i + 2], data[i + 3], data[i + 4], data[i + 5]]) as usize;
        let start = i + 6;
        let end = start.saturating_add(len).min(data.len());
        out.extend_from_slice(&data[start..end]);
        i = end;
    }
    out
}

impl Type1 {
    /// Parse a font program (`/Length1` bytes of cleartext, then the encrypted part).
    pub(crate) fn parse(data: &[u8], length1: Option<usize>) -> Option<Self> {
        let data = unpfb(data);
        let split = length1
            .filter(|l| *l > 0 && *l < data.len())
            .or_else(|| find(&data, b"eexec", 0).map(|p| p + 5))?;
        let clear = &data[..split];
        let mut font = Self {
            matrix: [0.001, 0.0, 0.0, 0.001, 0.0, 0.0],
            ..Self::default()
        };
        font.parse_clear(clear);
        let mut enc = &data[split..];
        // Skip white space after "eexec".
        while enc.first().is_some_and(u8::is_ascii_whitespace) {
            enc = &enc[1..];
        }
        let hex = enc.len() >= 4 && enc[..4].iter().all(u8::is_ascii_hexdigit);
        let bin = if hex {
            crate::filters::ascii_hex(enc)
        } else {
            enc.to_vec()
        };
        let plain = decrypt(&bin, 55665, 4);
        font.parse_private(&plain);
        (!font.glyphs.is_empty()).then_some(font)
    }

    fn parse_clear(&mut self, s: &[u8]) {
        if let Some(p) = find(s, b"/FontMatrix", 0) {
            if let (Some(a), Some(b)) = (find(s, b"[", p), find(s, b"]", p)) {
                if a < b && b - a < 200 {
                    let vals: Vec<f32> = String::from_utf8_lossy(&s[a + 1..b])
                        .split_ascii_whitespace()
                        .filter_map(|v| v.parse().ok())
                        .collect();
                    if let Ok(m) = <[f32; 6]>::try_from(vals.as_slice()) {
                        if m.iter().all(|v| v.is_finite()) && (m[0] != 0.0 || m[1] != 0.0) {
                            self.matrix = m;
                        }
                    }
                }
            }
        }
        if let Some(p) = find(s, b"/Encoding", 0) {
            let (a, b) = read_token(s, p + 9);
            if &s[a..b] == b"StandardEncoding" {
                self.standard_encoding = true;
                return;
            }
            // "dup <code> /<name> put" entries.
            let mut i = p;
            while let Some(d) = find(s, b"dup ", i) {
                let Some((code, j)) = read_int(s, d + 4) else {
                    i = d + 4;
                    continue;
                };
                let j = skip_white(s, j);
                if s.get(j) != Some(&b'/') {
                    i = j;
                    continue;
                }
                let (a, b) = read_token(s, j + 1);
                if let Ok(code) = u8::try_from(code) {
                    self.encoding
                        .insert(code, String::from_utf8_lossy(&s[a..b]).into_owned());
                }
                i = b;
                if find(s, b"readonly def", i)
                    .is_some_and(|r| find(s, b"dup ", i).is_none_or(|n| r < n))
                {
                    break;
                }
            }
        }
    }

    fn parse_private(&mut self, s: &[u8]) {
        let len_iv = find(s, b"/lenIV", 0)
            .and_then(|p| read_int(s, p + 6))
            .map_or(4, |(v, _)| v);
        let skip = usize::try_from(len_iv).unwrap_or(0);
        let plain = |b: &[u8]| {
            if len_iv < 0 {
                b.to_vec()
            } else {
                decrypt(b, 4330, skip)
            }
        };
        // /Subrs n array  dup i len RD <bytes> NP
        let chars_at = find(s, b"/CharStrings", 0).unwrap_or(s.len());
        if let Some(p) = find(s, b"/Subrs", 0).filter(|p| *p < chars_at) {
            if let Some((count, mut i)) = read_int(s, p + 6) {
                let count = usize::try_from(count).unwrap_or(0).min(65536);
                self.subrs = vec![Vec::new(); count];
                for _ in 0..count {
                    let Some(d) = find(s, b"dup", i) else { break };
                    if d > chars_at {
                        break;
                    }
                    let Some((idx, j)) = read_int(s, d + 3) else {
                        break;
                    };
                    let Some((len, j)) = read_int(s, j) else {
                        break;
                    };
                    let (_, j) = read_token(s, j); // RD or -|
                    let start = j + 1;
                    let Ok(len) = usize::try_from(len) else { break };
                    let Some(bytes) = s.get(start..start + len) else {
                        break;
                    };
                    if let Some(slot) = usize::try_from(idx)
                        .ok()
                        .and_then(|k| self.subrs.get_mut(k))
                    {
                        *slot = plain(bytes);
                    }
                    i = start + len;
                }
            }
        }
        // /CharStrings n dict dup begin  /name len RD <bytes> ND ... end
        if chars_at < s.len() {
            let mut i = read_int(s, chars_at + 12).map_or(chars_at + 12, |(_, j)| j);
            while let Some(slash) = find(s, b"/", i) {
                // Stop at the end of the dictionary.
                if let Some(e) = find(s, b"end", i) {
                    if e < slash {
                        break;
                    }
                }
                let (a, b) = read_token(s, slash + 1);
                let name = String::from_utf8_lossy(&s[a..b]).into_owned();
                let Some((len, j)) = read_int(s, b) else {
                    i = b;
                    continue;
                };
                let (_, j) = read_token(s, j);
                let start = j + 1;
                let Ok(len) = usize::try_from(len) else { break };
                let Some(bytes) = s.get(start..start + len) else {
                    break;
                };
                self.glyphs.insert(name, plain(bytes));
                i = start + len;
                if self.glyphs.len() > 65536 {
                    break;
                }
            }
        }
    }

    /// Whether the font has a glyph named `name`.
    pub(crate) fn has(&self, name: &str) -> bool {
        self.glyphs.contains_key(name)
    }

    /// Outline of glyph `name` in glyph space (y up).
    pub(crate) fn outline(&self, name: &str) -> Option<Vec<PathSeg>> {
        let cs = self.glyphs.get(name)?;
        let mut st = Interp {
            font: self,
            out: Vec::new(),
            stack: Vec::new(),
            ps: Vec::new(),
            x: 0.0,
            y: 0.0,
            sbx: 0.0,
            open: false,
            flex: None,
            seac: None,
            ops: 0,
        };
        st.run(cs, 0);
        let mut out = std::mem::take(&mut st.out);
        if st.open {
            out.push(PathSeg::Z);
        }
        if let Some([asb, adx, ady, bchar, achar]) = st.seac {
            let std = crate::encoding::standard();
            let base = std[(bchar.clamp(0.0, 255.0)) as usize];
            let accent = std[(achar.clamp(0.0, 255.0)) as usize];
            let mut all = self.outline(base).unwrap_or_default();
            if let Some(acc) = self.outline(accent) {
                let (dx, dy) = (adx + st.sbx - asb, ady);
                all.extend(acc.into_iter().map(|s| shift(s, dx, dy)));
            }
            return Some(all);
        }
        Some(out)
    }
}

fn shift(s: PathSeg, dx: f32, dy: f32) -> PathSeg {
    match s {
        PathSeg::M(x, y) => PathSeg::M(x + dx, y + dy),
        PathSeg::L(x, y) => PathSeg::L(x + dx, y + dy),
        PathSeg::Q(a, b, x, y) => PathSeg::Q(a + dx, b + dy, x + dx, y + dy),
        PathSeg::C(a, b, c, d, x, y) => PathSeg::C(a + dx, b + dy, c + dx, d + dy, x + dx, y + dy),
        PathSeg::Z => PathSeg::Z,
    }
}

struct Interp<'a> {
    font: &'a Type1,
    out: Vec<PathSeg>,
    stack: Vec<f32>,
    ps: Vec<f32>,
    x: f32,
    y: f32,
    sbx: f32,
    open: bool,
    flex: Option<Vec<(f32, f32)>>,
    seac: Option<[f32; 5]>,
    ops: u32,
}

impl Interp<'_> {
    fn move_to(&mut self, dx: f32, dy: f32) {
        self.x += dx;
        self.y += dy;
        if let Some(f) = self.flex.as_mut() {
            f.push((self.x, self.y));
            return;
        }
        if self.open {
            self.out.push(PathSeg::Z);
        }
        self.out.push(PathSeg::M(self.x, self.y));
        self.open = true;
    }

    fn line_to(&mut self, dx: f32, dy: f32) {
        if !self.open {
            self.out.push(PathSeg::M(self.x, self.y));
            self.open = true;
        }
        self.x += dx;
        self.y += dy;
        self.out.push(PathSeg::L(self.x, self.y));
    }

    fn curve(&mut self, d: [f32; 6]) {
        if !self.open {
            self.out.push(PathSeg::M(self.x, self.y));
            self.open = true;
        }
        let (x1, y1) = (self.x + d[0], self.y + d[1]);
        let (x2, y2) = (x1 + d[2], y1 + d[3]);
        let (x3, y3) = (x2 + d[4], y2 + d[5]);
        self.out.push(PathSeg::C(x1, y1, x2, y2, x3, y3));
        self.x = x3;
        self.y = y3;
    }

    /// Run a charstring; returns false when `endchar` was reached.
    #[allow(clippy::too_many_lines)]
    fn run(&mut self, cs: &[u8], depth: u32) -> bool {
        if depth > 10 {
            return false;
        }
        let mut i = 0;
        while i < cs.len() {
            self.ops += 1;
            if self.ops > 100_000 {
                return false;
            }
            let v = cs[i];
            i += 1;
            match v {
                32..=246 => self.stack.push(f32::from(v) - 139.0),
                247..=250 => {
                    let w = f32::from(cs.get(i).copied().unwrap_or(0));
                    i += 1;
                    self.stack.push((f32::from(v) - 247.0) * 256.0 + w + 108.0);
                }
                251..=254 => {
                    let w = f32::from(cs.get(i).copied().unwrap_or(0));
                    i += 1;
                    self.stack.push(-(f32::from(v) - 251.0) * 256.0 - w - 108.0);
                }
                255 => {
                    let b: [u8; 4] = cs
                        .get(i..i + 4)
                        .and_then(|b| b.try_into().ok())
                        .unwrap_or([0; 4]);
                    i += 4;
                    self.stack.push(i32::from_be_bytes(b) as f32);
                }
                _ => {
                    let op = if v == 12 {
                        let e = cs.get(i).copied().unwrap_or(0);
                        i += 1;
                        1200 + u16::from(e)
                    } else {
                        u16::from(v)
                    };
                    let s = std::mem::take(&mut self.stack);
                    let a = |k: usize| s.get(k).copied().unwrap_or(0.0);
                    match op {
                        13 => {
                            // hsbw sbx wx
                            self.sbx = a(0);
                            self.x = a(0);
                            self.y = 0.0;
                        }
                        1207 => {
                            // sbw sbx sby wx wy
                            self.sbx = a(0);
                            self.x = a(0);
                            self.y = a(1);
                        }
                        21 => self.move_to(a(0), a(1)),
                        22 => self.move_to(a(0), 0.0),
                        4 => self.move_to(0.0, a(0)),
                        5 => self.line_to(a(0), a(1)),
                        6 => self.line_to(a(0), 0.0),
                        7 => self.line_to(0.0, a(0)),
                        8 => self.curve([a(0), a(1), a(2), a(3), a(4), a(5)]),
                        30 => self.curve([0.0, a(0), a(1), a(2), a(3), 0.0]),
                        31 => self.curve([a(0), 0.0, a(1), a(2), 0.0, a(3)]),
                        9 => {
                            if self.open {
                                self.out.push(PathSeg::Z);
                                self.open = false;
                            }
                        }
                        10 => {
                            // callsubr: operands before the subr number stay on the stack.
                            let mut s = s;
                            let n = s.pop().unwrap_or(-1.0);
                            self.stack = s;
                            if n >= 0.0 {
                                let idx = n as usize;
                                if let Some(sub) = self.font.subrs.get(idx) {
                                    let sub = sub.clone();
                                    if !self.run(&sub, depth + 1) {
                                        return false;
                                    }
                                }
                            }
                            continue;
                        }
                        11 => {
                            self.stack = s;
                            return true;
                        }
                        14 => return false,
                        1206 => {
                            // seac asb adx ady bchar achar
                            self.seac = Some([a(0), a(1), a(2), a(3), a(4)]);
                            return false;
                        }
                        1212 => {
                            // div: keep the operands below the two divided ones.
                            let mut s = s;
                            let d = s.pop().unwrap_or(1.0);
                            let n = s.pop().unwrap_or(0.0);
                            s.push(if d == 0.0 { 0.0 } else { n / d });
                            self.stack = s;
                            continue;
                        }
                        1216 => {
                            // callothersubr: args… n othersubr#
                            let mut s = s;
                            let which = s.pop().unwrap_or(-1.0) as i32;
                            let n = s.pop().unwrap_or(0.0).max(0.0) as usize;
                            let n = n.min(s.len());
                            let args: Vec<f32> = s.split_off(s.len() - n);
                            self.stack = s;
                            match which {
                                1 => self.flex = Some(Vec::new()),
                                0 => {
                                    if let Some(pts) = self.flex.take() {
                                        if pts.len() >= 7 {
                                            let c = |k: usize| pts[k];
                                            if !self.open {
                                                self.out.push(PathSeg::M(self.x, self.y));
                                                self.open = true;
                                            }
                                            self.out.push(PathSeg::C(
                                                c(1).0,
                                                c(1).1,
                                                c(2).0,
                                                c(2).1,
                                                c(3).0,
                                                c(3).1,
                                            ));
                                            self.out.push(PathSeg::C(
                                                c(4).0,
                                                c(4).1,
                                                c(5).0,
                                                c(5).1,
                                                c(6).0,
                                                c(6).1,
                                            ));
                                            self.x = c(6).0;
                                            self.y = c(6).1;
                                        }
                                    }
                                    // The flex end point is returned through `pop pop`.
                                    self.ps = vec![self.y, self.x];
                                }
                                3 => self.ps = vec![3.0],
                                _ => {
                                    self.ps = args.into_iter().rev().collect();
                                }
                            }
                            continue;
                        }
                        1217 => {
                            // pop: move a value from the othersubr results.
                            let mut s = s;
                            s.push(self.ps.pop().unwrap_or(0.0));
                            self.stack = s;
                            continue;
                        }
                        1233 => {
                            // setcurrentpoint
                            self.x = a(0);
                            self.y = a(1);
                        }
                        // Hints and dotsection: no outline effect.
                        _ => {}
                    }
                }
            }
            if self.stack.len() > 48 {
                self.stack.clear();
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encrypt(plain: &[u8], mut r: u16) -> Vec<u8> {
        let mut out = Vec::new();
        for &p in plain {
            let c = p ^ (r >> 8) as u8;
            out.push(c);
            r = u16::from(c)
                .wrapping_add(r)
                .wrapping_mul(52845)
                .wrapping_add(22719);
        }
        out
    }

    #[test]
    fn parses_and_draws_a_square() {
        // hsbw 0 500; rmoveto 100 0; rlineto 0 100 (v 139+100=239); hlineto 100; closepath; endchar
        let cs_plain = [
            &[0_u8; 4][..],
            &[139, 247, 136, 13, 239, 139, 21, 139, 239, 5, 239, 6, 9, 14][..],
        ]
        .concat();
        let cs = encrypt(&cs_plain, 4330);
        let mut private = b"dup /Private 8 dict dup begin /lenIV 4 def /Subrs 0 array\n2 index /CharStrings 1 dict dup begin\n/sq ".to_vec();
        private.extend_from_slice(format!("{} RD ", cs.len()).as_bytes());
        private.extend_from_slice(&cs);
        private.extend_from_slice(b" ND\nend end");
        let enc = encrypt(&[&[1_u8, 2, 3, 4][..], &private[..]].concat(), 55665);
        let clear = b"%!PS-AdobeFont-1.0: Test\n/FontMatrix [0.001 0 0 0.001 0 0] readonly def\n/Encoding 256 array\ndup 65 /sq put\nreadonly def\ncurrentfile eexec\n";
        let mut data = clear.to_vec();
        let l1 = data.len();
        data.extend_from_slice(&enc);
        let f = Type1::parse(&data, Some(l1)).expect("type 1");
        assert_eq!(f.encoding.get(&65).map(String::as_str), Some("sq"));
        let o = f.outline("sq").expect("glyph");
        assert_eq!(o[0], PathSeg::M(100.0, 0.0));
        assert_eq!(o[1], PathSeg::L(100.0, 100.0));
        assert_eq!(o[2], PathSeg::L(200.0, 100.0));
        assert_eq!(o[3], PathSeg::Z);
        assert!(Type1::parse(b"garbage eexec \x01\x02", None).is_none());
    }
}
