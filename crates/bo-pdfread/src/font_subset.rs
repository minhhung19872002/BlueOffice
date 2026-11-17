//! Subset an embedded TrueType font program in place (BlueEditor round 10, PDF Optimizer ›
//! Unembed Fonts › "Subset all embedded fonts"): the glyphs nothing shows are emptied and their
//! outlines dropped, glyph ids stay the same so neither the content streams nor `/W`,
//! `/CIDToGIDMap` or the font's `cmap` change (OpenType `glyf` / `loca` / `head` tables;
//! ISO 32000-1 §9.9 embedded font programs). Layout tables a PDF viewer never reads
//! (`GSUB`, `GPOS`, `kern`, `hdmx`, `DSIG`, …) are left out.
use std::collections::BTreeSet;

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

/// Tables not needed to draw glyphs from a PDF.
const DROP: [&[u8; 4]; 13] = [
    b"GSUB", b"GPOS", b"GDEF", b"JSTF", b"BASE", b"kern", b"hdmx", b"LTSH", b"VDMX", b"DSIG",
    b"morx", b"mort", b"feat",
];

fn checksum(d: &[u8]) -> u32 {
    let mut sum = 0_u32;
    for c in d.chunks(4) {
        let mut b = [0_u8; 4];
        b[..c.len()].copy_from_slice(c);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

/// Glyph `g` of `glyf` through `loca`.
fn glyph<'a>(glyf: &'a [u8], loca: &[u8], long: bool, g: usize) -> Option<&'a [u8]> {
    let (a, b) = if long {
        (
            u32_at(loca, g * 4)? as usize,
            u32_at(loca, g * 4 + 4)? as usize,
        )
    } else {
        (
            usize::from(u16_at(loca, g * 2)?) * 2,
            usize::from(u16_at(loca, g * 2 + 2)?) * 2,
        )
    };
    if b < a {
        return None;
    }
    glyf.get(a..b)
}

/// Components of a composite glyph (`numberOfContours` < 0).
fn components(g: &[u8]) -> Vec<u16> {
    let mut out = Vec::new();
    if g.len() < 10 || i16::from_be_bytes([g[0], g[1]]) >= 0 {
        return out;
    }
    let mut o = 10;
    while let (Some(flags), Some(id)) = (u16_at(g, o), u16_at(g, o + 2)) {
        out.push(id);
        o += 4;
        o += if flags & 0x0001 != 0 { 4 } else { 2 };
        if flags & 0x0008 != 0 {
            o += 2;
        } else if flags & 0x0040 != 0 {
            o += 4;
        } else if flags & 0x0080 != 0 {
            o += 8;
        }
        if flags & 0x0020 == 0 {
            break;
        }
    }
    out
}

/// The font with only glyphs `keep` (and `.notdef`, and the parts of kept composites); `None`
/// when it is not a `glyf` TrueType font or is damaged.
pub(crate) fn subset_truetype(font: &[u8], keep: &BTreeSet<u16>) -> Option<Vec<u8>> {
    let version = u32_at(font, 0)?;
    if version != 0x0001_0000 && version != u32::from_be_bytes(*b"true") {
        return None;
    }
    let n = usize::from(u16_at(font, 4)?);
    let mut tables: Vec<([u8; 4], &[u8])> = Vec::with_capacity(n);
    for i in 0..n {
        let r = 12 + i * 16;
        let tag: [u8; 4] = font.get(r..r + 4)?.try_into().ok()?;
        let off = u32_at(font, r + 8)? as usize;
        let len = u32_at(font, r + 12)? as usize;
        tables.push((tag, font.get(off..off.checked_add(len)?)?));
    }
    let find = |t: &[u8; 4]| tables.iter().find(|x| &x.0 == t).map(|x| x.1);
    let head = find(b"head")?;
    let glyf = find(b"glyf")?;
    let loca = find(b"loca")?;
    let glyphs = usize::from(u16_at(find(b"maxp")?, 4)?);
    let long = u16_at(head, 50)? == 1;
    // Glyphs to keep: .notdef, the asked ones and every composite's parts.
    let mut want: BTreeSet<u16> = keep
        .iter()
        .copied()
        .filter(|g| usize::from(*g) < glyphs)
        .collect();
    want.insert(0);
    let mut todo: Vec<u16> = want.iter().copied().collect();
    while let Some(g) = todo.pop() {
        if let Some(data) = glyph(glyf, loca, long, usize::from(g)) {
            for c in components(data) {
                if usize::from(c) < glyphs && want.insert(c) {
                    todo.push(c);
                }
            }
        }
    }
    let mut new_glyf = Vec::new();
    let mut new_loca = Vec::with_capacity((glyphs + 1) * 4);
    for g in 0..glyphs {
        new_loca.extend_from_slice(&u32::try_from(new_glyf.len()).ok()?.to_be_bytes());
        if want.contains(&u16::try_from(g).ok()?) {
            new_glyf.extend_from_slice(glyph(glyf, loca, long, g)?);
            while new_glyf.len() % 4 != 0 {
                new_glyf.push(0);
            }
        }
    }
    new_loca.extend_from_slice(&u32::try_from(new_glyf.len()).ok()?.to_be_bytes());
    let mut new_head = head.to_vec();
    // indexToLocFormat = long; checkSumAdjustment set after the whole file is written.
    new_head
        .get_mut(50..52)?
        .copy_from_slice(&1_u16.to_be_bytes());
    new_head.get_mut(8..12)?.copy_from_slice(&[0; 4]);
    let mut out_tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    for (tag, data) in &tables {
        if DROP.contains(&tag) {
            continue;
        }
        let data = match tag {
            b"glyf" => new_glyf.clone(),
            b"loca" => new_loca.clone(),
            b"head" => new_head.clone(),
            _ => data.to_vec(),
        };
        out_tables.push((*tag, data));
    }
    out_tables.sort_by_key(|t| t.0);
    let count = u16::try_from(out_tables.len()).ok()?;
    let mut pow = 1_u16;
    let mut log = 0_u16;
    while pow * 2 <= count {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(count * 16 - pow * 16).to_be_bytes());
    let mut off = 12 + out_tables.len() * 16;
    let mut head_at = None;
    for (tag, data) in &out_tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&u32::try_from(off).ok()?.to_be_bytes());
        out.extend_from_slice(&u32::try_from(data.len()).ok()?.to_be_bytes());
        if tag == b"head" {
            head_at = Some(off);
        }
        off += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &out_tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(h) = head_at {
        let adj = 0xB1B0_AFBA_u32.wrapping_sub(checksum(&out));
        out.get_mut(h + 8..h + 12)?
            .copy_from_slice(&adj.to_be_bytes());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subset_keeps_the_asked_glyphs_and_their_ids() {
        let font = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fonts/LiberationSans-Regular.ttf"
        ))
        .to_vec();
        let face = ttf_parser::Face::parse(&font, 0).expect("parses");
        let a = face.glyph_index('A').expect("A").0;
        let z = face.glyph_index('z').expect("z").0;
        let keep: BTreeSet<u16> = [a].into_iter().collect();
        let small = subset_truetype(&font, &keep).expect("subsets");
        assert!(
            small.len() * 3 < font.len(),
            "{} → {}",
            font.len(),
            small.len()
        );
        let face = ttf_parser::Face::parse(&small, 0).expect("subset parses");
        // Same glyph id, outline kept; other glyphs empty, metrics untouched.
        assert_eq!(face.glyph_index('A').map(|g| g.0), Some(a));
        let mut sink = Sink(0);
        assert!(face
            .outline_glyph(ttf_parser::GlyphId(a), &mut sink)
            .is_some());
        assert!(face
            .outline_glyph(ttf_parser::GlyphId(z), &mut sink)
            .is_none());
        assert!(face.glyph_hor_advance(ttf_parser::GlyphId(z)).unwrap_or(0) > 0);
    }

    struct Sink(usize);
    impl ttf_parser::OutlineBuilder for Sink {
        fn move_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn line_to(&mut self, _: f32, _: f32) {
            self.0 += 1;
        }
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            self.0 += 1;
        }
        fn close(&mut self) {}
    }
}
