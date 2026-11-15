//! Embedded Unicode font for edited and added text whose characters the original font cannot
//! show (Vietnamese, symbols): a bundled TrueType face as a Type 0 font with `Identity-H`
//! encoding, glyph ids as CIDs, every glyph's width and a `/ToUnicode` CMap so the text stays
//! searchable (ISO 32000-1 §9.7 composite fonts, §9.10.3 ToUnicode). The face is embedded once
//! per document and never changes afterwards.
use crate::PdfDocument;
use lopdf::{Dictionary, Object, ObjectId, Stream, StringFormat};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Face of the bundled set: Tinos (serif) 0–3 or Liberation Sans 4–7, each regular, bold,
/// italic, bold italic.
pub(crate) const SANS: usize = 4;

/// The bundled face standing in for a font named `base` (PostScript name).
pub(crate) fn face_for(base: &str) -> usize {
    let b = base.to_ascii_lowercase();
    let serif = [
        "times", "serif", "roman", "tinos", "georgia", "cambria", "garamond",
    ]
    .iter()
    .any(|k| b.contains(k))
        && !b.contains("sans");
    let bold = ["bold", "black", "heavy", "semibold", "demi"]
        .iter()
        .any(|k| b.contains(k));
    let italic = b.contains("italic") || b.contains("oblique");
    (if serif { 0 } else { SANS }) + usize::from(bold) + 2 * usize::from(italic)
}

#[cfg(feature = "bundled-fonts")]
pub(crate) fn face_data(face: usize) -> Option<&'static [u8]> {
    crate::bundled::FONTS.get(face).copied()
}

#[cfg(not(feature = "bundled-fonts"))]
pub(crate) fn face_data(_face: usize) -> Option<&'static [u8]> {
    None
}

fn to_unicode(face: &ttf_parser::Face<'_>) -> Vec<u8> {
    let mut map: BTreeMap<u16, char> = BTreeMap::new();
    if let Some(cmap) = face.tables().cmap {
        for sub in cmap.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {
                if let (Some(c), Some(g)) = (char::from_u32(cp), sub.glyph_index(cp)) {
                    map.entry(g.0).or_insert(c);
                }
            });
        }
    }
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &char)> = map.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (g, c) in chunk {
            let mut units = [0u16; 2];
            let u = c
                .encode_utf16(&mut units)
                .iter()
                .fold(String::new(), |mut acc, u| {
                    let _ = write!(acc, "{u:04X}");
                    acc
                });
            let _ = writeln!(s, "<{g:04X}> <{u}>");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s.into_bytes()
}

impl PdfDocument {
    /// The embedded font (serif or sans) and `text` encoded for it (glyph ids).
    pub(crate) fn embed_text(
        &mut self,
        face_index: usize,
        text: &str,
    ) -> Result<(ObjectId, Vec<u8>), String> {
        let data = face_data(face_index).ok_or("no font to embed")?;
        let face = ttf_parser::Face::parse(data, 0).map_err(|e| e.to_string())?;
        let mut bytes = Vec::with_capacity(text.len() * 2);
        for c in text.chars() {
            let g = face.glyph_index(c).map_or(0, |g| g.0);
            bytes.extend_from_slice(&g.to_be_bytes());
        }
        if let Some(&id) = self.edit.embedded.get(&face_index) {
            if self.doc.objects.contains_key(&id) {
                return Ok((id, bytes));
            }
        }
        let upem = f32::from(face.units_per_em().max(1));
        let k = |v: f32| Object::Integer((v * 1000.0 / upem).round() as i64);
        let mut len1 = Dictionary::new();
        len1.set("Length1", data.len() as i64);
        // Written uncompressed: a save compresses it ([`PdfDocument::save`]), so the first
        // keystroke that needs the face stays fast.
        let file = self.add(Object::Stream(Stream::new(len1, data.to_vec())));
        let serif = face_index < SANS;
        let style = ["", "-Bold", "-Italic", "-BoldItalic"][face_index % 4];
        let name =
            format!("BlueOffice-{}{style}", if serif { "Serif" } else { "Sans" }).into_bytes();
        let name: &[u8] = &name;
        let bb = face.global_bounding_box();
        let mut fd = Dictionary::new();
        fd.set("Type", Object::Name(b"FontDescriptor".to_vec()));
        fd.set("FontName", Object::Name(name.to_vec()));
        fd.set("Flags", if serif { 34 } else { 32 });
        fd.set(
            "FontBBox",
            vec![
                k(f32::from(bb.x_min)),
                k(f32::from(bb.y_min)),
                k(f32::from(bb.x_max)),
                k(f32::from(bb.y_max)),
            ],
        );
        fd.set("ItalicAngle", 0);
        fd.set("Ascent", k(f32::from(face.ascender())));
        fd.set("Descent", k(f32::from(face.descender())));
        fd.set(
            "CapHeight",
            k(f32::from(face.capital_height().unwrap_or(face.ascender()))),
        );
        fd.set("StemV", 80);
        fd.set("FontFile2", Object::Reference(file));
        let fd_id = self.add(Object::Dictionary(fd));
        let widths: Vec<Object> = (0..face.number_of_glyphs())
            .map(|g| {
                k(face
                    .glyph_hor_advance(ttf_parser::GlyphId(g))
                    .map_or(0.0, f32::from))
            })
            .collect();
        let mut info = Dictionary::new();
        info.set(
            "Registry",
            Object::String(b"Adobe".to_vec(), StringFormat::Literal),
        );
        info.set(
            "Ordering",
            Object::String(b"Identity".to_vec(), StringFormat::Literal),
        );
        info.set("Supplement", 0);
        let mut cid = Dictionary::new();
        cid.set("Type", Object::Name(b"Font".to_vec()));
        cid.set("Subtype", Object::Name(b"CIDFontType2".to_vec()));
        cid.set("BaseFont", Object::Name(name.to_vec()));
        cid.set("CIDSystemInfo", info);
        cid.set("FontDescriptor", Object::Reference(fd_id));
        cid.set("CIDToGIDMap", Object::Name(b"Identity".to_vec()));
        cid.set("DW", 1000);
        cid.set("W", vec![Object::Integer(0), Object::Array(widths)]);
        let cid_id = self.add(Object::Dictionary(cid));
        let tu = self.add(Object::Stream(Stream::new(
            Dictionary::new(),
            to_unicode(&face),
        )));
        let mut t0 = Dictionary::new();
        t0.set("Type", Object::Name(b"Font".to_vec()));
        t0.set("Subtype", Object::Name(b"Type0".to_vec()));
        t0.set("BaseFont", Object::Name(name.to_vec()));
        t0.set("Encoding", Object::Name(b"Identity-H".to_vec()));
        t0.set("DescendantFonts", vec![Object::Reference(cid_id)]);
        t0.set("ToUnicode", Object::Reference(tu));
        let font = self.add(Object::Dictionary(t0));
        self.edit.embedded.insert(face_index, font);
        Ok((font, bytes))
    }
}
