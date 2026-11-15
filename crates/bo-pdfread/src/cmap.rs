//! CMaps (ISO 32000-1, §9.7.5 and §9.10.3; Adobe Technical Note #5014): code space ranges,
//! character code to CID mappings of Type 0 fonts and `/ToUnicode` mappings.
use crate::lexer::Lexer;
use lopdf::Object;
use std::collections::HashMap;

/// A parsed CMap.
#[derive(Debug, Clone, Default)]
pub(crate) struct CMap {
    /// Code space ranges: byte length, low and high code.
    space: Vec<(usize, u32, u32)>,
    /// Single code to CID.
    cids: HashMap<u32, u32>,
    /// Code ranges to CIDs: low, high, first CID.
    cid_ranges: Vec<(u32, u32, u32)>,
    /// Single code to Unicode text.
    text: HashMap<u32, String>,
    /// Code ranges to Unicode: low, high, text of the low code (last UTF-16 unit counts up).
    text_ranges: Vec<(u32, u32, Vec<u16>)>,
    /// Vertical writing (`/WMode 1`).
    pub vertical: bool,
    /// The CMap only maps 2-byte codes to themselves (Identity-H / Identity-V).
    identity: bool,
}

fn code_of(b: &[u8]) -> u32 {
    b.iter().take(4).fold(0, |acc, &c| acc << 8 | u32::from(c))
}

fn utf16(b: &[u8]) -> Vec<u16> {
    b.chunks(2)
        .map(|c| match c {
            [h, l] => u16::from(*h) << 8 | u16::from(*l),
            [h] => u16::from(*h),
            _ => 0,
        })
        .collect()
}

impl CMap {
    /// Identity-H / Identity-V: two-byte codes equal to CIDs.
    pub(crate) fn identity(vertical: bool) -> Self {
        Self {
            space: vec![(2, 0, 0xffff)],
            vertical,
            identity: true,
            ..Self::default()
        }
    }

    /// One-byte codes (simple fonts' `/ToUnicode` without a code space).
    fn one_byte() -> Vec<(usize, u32, u32)> {
        vec![(1, 0, 0xff)]
    }

    /// Parse CMap program text.
    pub(crate) fn parse(data: &[u8]) -> Self {
        let mut m = Self::default();
        let mut pending: Vec<Object> = Vec::new();
        for op in Lexer::new(data) {
            match op.name.as_slice() {
                b"endcodespacerange" => {
                    for pair in op.args.chunks(2) {
                        if let [Object::String(lo, _), Object::String(hi, _)] = pair {
                            if !lo.is_empty() && lo.len() <= 4 {
                                m.space.push((lo.len(), code_of(lo), code_of(hi)));
                            }
                        }
                    }
                }
                b"endcidchar" => {
                    for pair in op.args.chunks(2) {
                        if let [Object::String(c, _), cid] = pair {
                            if let Ok(cid) = cid.as_i64() {
                                m.cids.insert(code_of(c), cid.max(0) as u32);
                            }
                        }
                    }
                }
                b"endcidrange" => {
                    for t in op.args.chunks(3) {
                        if let [Object::String(lo, _), Object::String(hi, _), cid] = t {
                            if let Ok(cid) = cid.as_i64() {
                                m.cid_ranges
                                    .push((code_of(lo), code_of(hi), cid.max(0) as u32));
                            }
                        }
                    }
                }
                b"endbfchar" => {
                    for pair in op.args.chunks(2) {
                        if let [Object::String(c, _), dst] = pair {
                            let text = match dst {
                                Object::String(s, _) => String::from_utf16_lossy(&utf16(s)),
                                Object::Name(n) => {
                                    crate::encoding::unicode(&String::from_utf8_lossy(n))
                                        .unwrap_or_default()
                                }
                                _ => continue,
                            };
                            m.text.insert(code_of(c), text);
                        }
                    }
                }
                b"endbfrange" => {
                    for t in op.args.chunks(3) {
                        let [Object::String(lo, _), Object::String(hi, _), dst] = t else {
                            continue;
                        };
                        let (lo, hi) = (code_of(lo), code_of(hi));
                        match dst {
                            Object::String(s, _) => m.text_ranges.push((lo, hi, utf16(s))),
                            Object::Array(items) => {
                                for (k, item) in items.iter().enumerate() {
                                    if let Object::String(s, _) = item {
                                        let code = lo.saturating_add(k as u32);
                                        if code > hi {
                                            break;
                                        }
                                        m.text.insert(code, String::from_utf16_lossy(&utf16(s)));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                b"usecmap" => {
                    if let Some(Object::Name(n)) = op.args.first().or(pending.last()) {
                        if n.starts_with(b"Identity") {
                            m.identity = m.cids.is_empty() && m.cid_ranges.is_empty();
                            if m.space.is_empty() {
                                m.space.push((2, 0, 0xffff));
                            }
                        }
                    }
                }
                b"def" => {
                    if let [Object::Name(k), v] = op.args.as_slice() {
                        if k == b"WMode" {
                            m.vertical = v.as_i64().is_ok_and(|v| v == 1);
                        }
                    }
                }
                _ => {}
            }
            pending = op.args;
        }
        m
    }

    /// Split a string into (code, byte length) pairs by the code space ranges.
    pub(crate) fn codes(&self, s: &[u8]) -> Vec<(u32, usize)> {
        let space = if self.space.is_empty() {
            &Self::one_byte()[..]
        } else {
            &self.space[..]
        };
        let shortest = space.iter().map(|r| r.0).min().unwrap_or(1);
        let mut out = Vec::with_capacity(s.len());
        let mut i = 0;
        while i < s.len() {
            let mut found = None;
            for n in 1..=4 {
                if i + n > s.len() {
                    break;
                }
                let c = code_of(&s[i..i + n]);
                if space
                    .iter()
                    .any(|&(len, lo, hi)| len == n && c >= lo && c <= hi)
                {
                    found = Some((c, n));
                    break;
                }
            }
            let (c, n) = found.unwrap_or_else(|| {
                let n = shortest.min(s.len() - i);
                (code_of(&s[i..i + n]), n)
            });
            out.push((c, n));
            i += n;
        }
        out
    }

    /// CID of a code.
    pub(crate) fn cid(&self, code: u32) -> u32 {
        if self.identity {
            return code;
        }
        if let Some(&c) = self.cids.get(&code) {
            return c;
        }
        for &(lo, hi, cid) in &self.cid_ranges {
            if code >= lo && code <= hi {
                return cid + (code - lo);
            }
        }
        if self.cids.is_empty() && self.cid_ranges.is_empty() {
            code
        } else {
            0
        }
    }

    /// Unicode text of a code (`/ToUnicode`).
    pub(crate) fn text(&self, code: u32) -> Option<String> {
        if let Some(t) = self.text.get(&code) {
            return Some(t.clone());
        }
        for (lo, hi, base) in &self.text_ranges {
            if code >= *lo && code <= *hi {
                let mut units = base.clone();
                if let Some(last) = units.last_mut() {
                    *last = last.wrapping_add((code - lo) as u16);
                }
                return Some(String::from_utf16_lossy(&units));
            }
        }
        None
    }

    /// Whether any code maps to text.
    pub(crate) fn has_text(&self) -> bool {
        !self.text.is_empty() || !self.text_ranges.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_unicode_and_cid_ranges() {
        let m = CMap::parse(
            b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap
            1 begincodespacerange <0000> <FFFF> endcodespacerange
            2 beginbfchar <0003> <0020> <0024> <00410042> endbfchar
            2 beginbfrange <0010> <0012> <0061> <0020> <0021> [<00E9> <1EA0>] endbfrange
            1 begincidrange <0100> <01FF> 500 endcidrange
            endcmap",
        );
        assert_eq!(m.text(3).as_deref(), Some(" "));
        assert_eq!(m.text(0x24).as_deref(), Some("AB"));
        assert_eq!(m.text(0x12).as_deref(), Some("c"));
        assert_eq!(m.text(0x21).as_deref(), Some("Ạ"));
        assert_eq!(m.cid(0x105), 505);
        assert_eq!(m.codes(&[0, 3, 1, 5]), vec![(3, 2), (0x105, 2)]);
    }

    #[test]
    fn mixed_code_lengths() {
        let m = CMap::parse(b"2 begincodespacerange <00> <80> <8140> <FFFF> endcodespacerange");
        assert_eq!(m.codes(&[0x41, 0x81, 0x40]), vec![(0x41, 1), (0x8140, 2)]);
    }
}
