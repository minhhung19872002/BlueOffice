//! Make embedded font programs loadable by the platform text engine (OpenType spec, "The
//! OpenType font file"): subsets in PDFs often lack `name`, `OS/2`, `post` or `cmap`, and bare
//! CFF programs have no sfnt wrapper at all. Every face gets a unique family name so an
//! embedded subset never stands in for an installed font of the same name.
use std::sync::atomic::{AtomicU32, Ordering};

static SERIAL: AtomicU32 = AtomicU32::new(0);

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0_u32;
    for c in data.chunks(4) {
        let mut b = [0_u8; 4];
        b[..c.len()].copy_from_slice(c);
        sum = sum.wrapping_add(u32::from_be_bytes(b));
    }
    sum
}

/// Assemble an sfnt from tables.
fn assemble(mut tables: Vec<([u8; 4], Vec<u8>)>, cff: bool) -> Vec<u8> {
    tables.sort_by_key(|t| t.0);
    let n = tables.len() as u16;
    let mut es = 0_u16;
    while (1_u16 << (es + 1)) <= n {
        es += 1;
    }
    let sr = (1_u16 << es) * 16;
    let mut out = Vec::new();
    out.extend_from_slice(if cff { b"OTTO" } else { &[0, 1, 0, 0] });
    out.extend_from_slice(&be16(n));
    out.extend_from_slice(&be16(sr));
    out.extend_from_slice(&be16(es));
    out.extend_from_slice(&be16(n * 16 - sr));
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&be32(checksum(data)));
        out.extend_from_slice(&be32(offset as u32));
        out.extend_from_slice(&be32(data.len() as u32));
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = 12 + 16 * tables.len() + body.len();
    }
    out.extend_from_slice(&body);
    out
}

/// Split an sfnt into its tables.
fn tables(data: &[u8]) -> Option<Vec<([u8; 4], Vec<u8>)>> {
    let n = usize::from(u16::from_be_bytes([*data.get(4)?, *data.get(5)?]));
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let rec = data.get(12 + 16 * i..28 + 16 * i)?;
        let tag: [u8; 4] = rec[..4].try_into().ok()?;
        let off = u32::from_be_bytes(rec[8..12].try_into().ok()?) as usize;
        let len = u32::from_be_bytes(rec[12..16].try_into().ok()?) as usize;
        let body = data.get(off..off.checked_add(len)?)?;
        out.push((tag, body.to_vec()));
    }
    Some(out)
}

fn name_table(family: &str) -> Vec<u8> {
    let recs: [(u16, String); 4] = [
        (1, family.to_owned()),
        (2, "Regular".to_owned()),
        (4, family.to_owned()),
        (6, family.replace(' ', "")),
    ];
    let mut strings = Vec::new();
    let mut head = Vec::new();
    head.extend_from_slice(&be16(0));
    head.extend_from_slice(&be16(recs.len() as u16));
    head.extend_from_slice(&be16(6 + 12 * recs.len() as u16));
    for (id, s) in &recs {
        let utf16: Vec<u8> = s.encode_utf16().flat_map(u16::to_be_bytes).collect();
        head.extend_from_slice(&be16(3));
        head.extend_from_slice(&be16(1));
        head.extend_from_slice(&be16(0x409));
        head.extend_from_slice(&be16(*id));
        head.extend_from_slice(&be16(utf16.len() as u16));
        head.extend_from_slice(&be16(strings.len() as u16));
        strings.extend_from_slice(&utf16);
    }
    head.extend_from_slice(&strings);
    head
}

fn os2_table() -> Vec<u8> {
    let mut t = Vec::with_capacity(96);
    t.extend_from_slice(&be16(4)); // version
    t.extend_from_slice(&be16(500)); // xAvgCharWidth
    t.extend_from_slice(&be16(400)); // usWeightClass
    t.extend_from_slice(&be16(5)); // usWidthClass
    t.extend_from_slice(&be16(0)); // fsType: installable
    for v in [650_u16, 600, 0, 75, 650, 600, 0, 350, 50, 250] {
        t.extend_from_slice(&be16(v)); // sub/superscript, strikeout
    }
    t.extend_from_slice(&be16(0)); // sFamilyClass
    t.extend_from_slice(&[0; 10]); // panose
    t.extend_from_slice(&[0; 16]); // unicode ranges
    t.extend_from_slice(b"BOPD"); // vendor
    t.extend_from_slice(&be16(0x40)); // fsSelection: regular
    t.extend_from_slice(&be16(0x20)); // first char
    t.extend_from_slice(&be16(0xffff)); // last char
    t.extend_from_slice(&be16(800)); // typo ascender
    t.extend_from_slice(&be16((-200_i16) as u16)); // typo descender
    t.extend_from_slice(&be16(0)); // line gap
    t.extend_from_slice(&be16(1000)); // win ascent
    t.extend_from_slice(&be16(300)); // win descent
    t.extend_from_slice(&[0; 8]); // code page ranges
    t.extend_from_slice(&be16(500)); // x height
    t.extend_from_slice(&be16(700)); // cap height
    t.extend_from_slice(&be16(0)); // default char
    t.extend_from_slice(&be16(0x20)); // break char
    t.extend_from_slice(&be16(1)); // max context
    t
}

fn post_table() -> Vec<u8> {
    let mut t = Vec::with_capacity(32);
    t.extend_from_slice(&be32(0x0003_0000));
    t.extend_from_slice(&[0; 28]);
    t
}

fn cmap_table() -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(&be16(0));
    t.extend_from_slice(&be16(1));
    t.extend_from_slice(&be16(3));
    t.extend_from_slice(&be16(1));
    t.extend_from_slice(&be32(12));
    // Format 4 with the terminating segment only.
    for v in [4_u16, 24, 0, 2, 2, 0, 0, 0xffff, 0, 0xffff, 1, 0] {
        t.extend_from_slice(&be16(v));
    }
    t
}

fn unique_family(name: &str) -> String {
    let n = SERIAL.fetch_add(1, Ordering::Relaxed);
    let clean: String = crate::font::base_name(name)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(24)
        .collect();
    format!("BoPdf{n} {clean}")
}

/// A TrueType / OpenType program with a unique family name and the tables the text engine
/// needs. `None` when the program lacks glyph data.
pub(crate) fn make_loadable(data: &[u8], name: &str) -> Option<Vec<u8>> {
    if data.starts_with(b"ttcf") {
        return None;
    }
    let mut t = tables(data)?;
    let has = |t: &[([u8; 4], Vec<u8>)], tag: &[u8; 4]| t.iter().any(|(k, _)| k == tag);
    let cff = has(&t, b"CFF ");
    for need in [b"head", b"hhea", b"maxp", b"hmtx"] {
        if !has(&t, need) {
            return None;
        }
    }
    if !cff && (!has(&t, b"glyf") || !has(&t, b"loca")) {
        return None;
    }
    t.retain(|(k, _)| k != b"name" && k != b"DSIG");
    t.push((*b"name", name_table(&unique_family(name))));
    if !t.iter().any(|(k, v)| k == b"OS/2" && v.len() >= 78) {
        t.retain(|(k, _)| k != b"OS/2");
        t.push((*b"OS/2", os2_table()));
    }
    if !has(&t, b"post") {
        t.push((*b"post", post_table()));
    }
    if !has(&t, b"cmap") {
        t.push((*b"cmap", cmap_table()));
    }
    // Zero the checksum adjustment of the copied head table.
    if let Some((_, head)) = t.iter_mut().find(|(k, _)| k == b"head") {
        if head.len() >= 12 {
            head[8..12].copy_from_slice(&[0; 4]);
        }
    }
    Some(assemble(t, cff))
}

/// Wrap a bare CFF program in an OpenType file the text engine can load.
pub(crate) fn wrap_cff(cff: &[u8], name: &str, glyphs: u16) -> Option<Vec<u8>> {
    let table = ttf_parser::cff::Table::parse(cff)?;
    let m = table.matrix();
    if m.kx.abs() > 1e-6 || m.ky.abs() > 1e-6 || (m.sx - m.sy).abs() > 1e-6 || m.sx <= 0.0 {
        return None;
    }
    let upem = (1.0 / m.sx).round();
    if !(16.0..=16384.0).contains(&upem) || m.tx.abs() > 1e-6 || m.ty.abs() > 1e-6 {
        return None;
    }
    let upem = upem as u16;
    let mut head = Vec::with_capacity(54);
    head.extend_from_slice(&be32(0x0001_0000));
    head.extend_from_slice(&be32(0x0001_0000));
    head.extend_from_slice(&be32(0));
    head.extend_from_slice(&be32(0x5F0F_3CF5));
    head.extend_from_slice(&be16(0x000b));
    head.extend_from_slice(&be16(upem));
    head.extend_from_slice(&[0; 16]); // created, modified
    let big = upem.saturating_mul(2) as i16;
    for v in [-big, -big, big, big] {
        head.extend_from_slice(&be16(v as u16));
    }
    head.extend_from_slice(&be16(0)); // macStyle
    head.extend_from_slice(&be16(8)); // lowestRecPPEM
    head.extend_from_slice(&be16(2)); // fontDirectionHint
    head.extend_from_slice(&be16(0)); // indexToLocFormat
    head.extend_from_slice(&be16(0)); // glyphDataFormat
    let mut hhea = Vec::with_capacity(36);
    hhea.extend_from_slice(&be32(0x0001_0000));
    hhea.extend_from_slice(&be16(upem * 4 / 5));
    hhea.extend_from_slice(&be16((-((upem / 5) as i16)) as u16));
    hhea.extend_from_slice(&be16(0));
    hhea.extend_from_slice(&be16(upem));
    hhea.extend_from_slice(&[0; 6]); // minLSB, minRSB, xMaxExtent
    hhea.extend_from_slice(&be16(1)); // caretSlopeRise
    hhea.extend_from_slice(&[0; 12]); // caretSlopeRun, caretOffset, reserved
    hhea.extend_from_slice(&be16(0)); // metricDataFormat
    hhea.extend_from_slice(&be16(1)); // numberOfHMetrics
    let mut hmtx = Vec::with_capacity(4 + 2 * usize::from(glyphs));
    hmtx.extend_from_slice(&be16(upem / 2));
    hmtx.extend_from_slice(&be16(0));
    for _ in 1..glyphs {
        hmtx.extend_from_slice(&be16(0));
    }
    let mut maxp = Vec::with_capacity(6);
    maxp.extend_from_slice(&be32(0x0000_5000));
    maxp.extend_from_slice(&be16(glyphs));
    let family = unique_family(name);
    Some(assemble(
        vec![
            (*b"CFF ", cff.to_vec()),
            (*b"head", head),
            (*b"hhea", hhea),
            (*b"hmtx", hmtx),
            (*b"maxp", maxp),
            (*b"name", name_table(&family)),
            (*b"OS/2", os2_table()),
            (*b"post", post_table()),
            (*b"cmap", cmap_table()),
        ],
        true,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuilt_truetype_gets_a_unique_family() {
        let font = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fonts/LiberationSans-Regular.ttf"
        ))
        .expect("bundled font");
        let fixed = make_loadable(&font, "ABCDEF+Arial").expect("loadable");
        let face = ttf_parser::Face::parse(&fixed, 0).expect("parses");
        let fam = face
            .names()
            .into_iter()
            .find(|n| n.name_id == 1)
            .and_then(|n| n.to_string())
            .expect("family");
        assert!(fam.starts_with("BoPdf") && fam.ends_with("Arial"));
        assert_eq!(
            face.glyph_index('A'),
            ttf_parser::Face::parse(&font, 0)
                .ok()
                .and_then(|f| f.glyph_index('A'))
        );
        assert!(make_loadable(b"garbage", "x").is_none());
    }
}
