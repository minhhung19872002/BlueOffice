//! PDF fonts (ISO 32000-1, §9.5–9.10): simple fonts (Type 1, TrueType, Type 3) with their
//! encodings, composite fonts (Type 0 with CIDFontType0 / CIDFontType2), widths, ToUnicode,
//! embedded programs (FontFile / FontFile2 / FontFile3) and substitutes for fonts that are not
//! embedded (the standard 14 fonts map to metric-compatible faces).
//!
//! A glyph is drawn either by the platform text engine from a face registered in the
//! document's [`FontManager`] (crisp, hinted) or as its outline path (Type 1 programs and
//! fonts the text engine cannot load).
use crate::cmap::CMap;
use crate::encoding;
use crate::filters;
use crate::sfnt;
use crate::store::Document;
use crate::type1::Type1;
use bo_render::PathSeg;
use bo_text::{FaceId, FontManager};
use lopdf::{Dictionary, Object, ObjectId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Font program used for outlines.
#[derive(Clone)]
enum Program {
    /// TrueType / OpenType file.
    Sfnt(Arc<Vec<u8>>),
    /// Bare CFF font (FontFile3 /Type1C or /CIDFontType0C).
    Cff(Arc<Vec<u8>>),
    /// Type 1 font program.
    Type1(Arc<Type1>),
    None,
}

/// How a code reaches a glyph.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GlyphRef {
    /// Glyph index in the font program (or the substitute face).
    Gid(u16),
    /// Glyph name in a Type 1 program.
    Name(String),
}

/// Type 3 glyph procedures.
pub(crate) struct Type3 {
    pub procs: HashMap<Vec<u8>, Object>,
    pub resources: Option<Dictionary>,
}

/// A loaded font.
pub(crate) struct Font {
    /// Type 0 (composite) font.
    composite: bool,
    cmap: CMap,
    to_unicode: Option<CMap>,
    /// Simple fonts: glyph names by code (encoding with differences applied).
    names: Vec<Option<String>>,
    /// Simple fonts: widths by code from `/FirstChar`.
    widths: Vec<Option<f32>>,
    first_char: u32,
    /// Composite fonts: widths by CID.
    cid_widths: HashMap<u32, f32>,
    default_width: f32,
    /// Vertical writing: default vertical displacement and position (`/DW2`).
    pub vertical: bool,
    dw2: [f32; 2],
    program: Program,
    /// Registered face that draws this font's glyph ids (embedded program or substitute).
    pub face: Option<FaceId>,
    /// The face is a substitute: glyphs are looked up by Unicode.
    substitute: bool,
    /// Substitute face is a symbol font with a (3,0) cmap at U+F0xx.
    symbol_cmap: bool,
    cid_to_gid: Option<Vec<u16>>,
    /// CID-keyed CFF: CID to glyph index.
    cff_cids: HashMap<u16, u16>,
    /// Glyph space to text space (outline drawing).
    pub matrix: [f32; 6],
    /// Ascent and descent in text space units (1000 = 1 em) for text boxes.
    pub ascent: f32,
    pub descent: f32,
    pub type3: Option<Type3>,
    gid_cache: RefCell<HashMap<u32, Option<GlyphRef>>>,
    outlines: RefCell<HashMap<u16, Option<Arc<Vec<PathSeg>>>>>,
    named_outlines: RefCell<HashMap<String, Option<Arc<Vec<PathSeg>>>>>,
    /// Symbolic flag of the font descriptor.
    symbolic: bool,
    /// Simple fonts drawn by a substitute: glyph ids and advances (thousandths) by code.
    sub_gids: Vec<Option<u16>>,
    /// Composite fonts drawn by a substitute: the face file, looked up by Unicode.
    sub_data: Option<(Arc<Vec<u8>>, u32)>,
    face_widths: Vec<Option<f32>>,
    /// Has a (3,1) Unicode cmap / (3,0) symbol cmap / (1,0) Mac cmap (embedded TrueType).
    cmaps: (bool, bool, bool),
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map_or(o, |(_, o)| o)
}

fn dict_get<'a>(doc: &'a Document, d: &'a Dictionary, k: &[u8]) -> Option<&'a Object> {
    d.get(k).ok().map(|o| resolve(doc, o))
}

fn num(o: &Object) -> Option<f32> {
    o.as_float().ok()
}

/// Strip a subset tag (`ABCDEF+Name`).
pub(crate) fn base_name(name: &str) -> &str {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.bytes().all(|c| c.is_ascii_uppercase()) => rest,
        _ => name,
    }
}

/// Family candidates and style of a PostScript-like font name.
pub(crate) fn substitute_names(name: &str, serif: bool, fixed: bool) -> (Vec<String>, bool, bool) {
    let lower = name.to_ascii_lowercase();
    let bold = lower.contains("bold")
        || lower.contains("black")
        || lower.contains("heavy")
        || lower.contains("semibold")
        || lower.contains("demi");
    let italic = lower.contains("italic") || lower.contains("oblique");
    let family = name
        .split([',', '-'])
        .next()
        .unwrap_or(name)
        .trim_end_matches("PSMT")
        .trim_end_matches("MT")
        .trim_end_matches("PS");
    let mut out = Vec::new();
    let std =
        if lower.starts_with("times") || lower.contains("roman") && !lower.contains("newroman") {
            Some("Times New Roman")
        } else if lower.starts_with("helvetica") || lower.starts_with("arial") {
            Some("Arial")
        } else if lower.starts_with("courier") {
            Some("Courier New")
        } else if lower.starts_with("symbol") {
            // Greek and mathematical signs by Unicode (the built-in encoding's names) in a
            // serif face: Windows' own Symbol face keeps its glyphs at U+F0xx and is not
            // drawn by the page renderer.
            Some("Times New Roman")
        } else if lower.starts_with("zapfdingbats") {
            Some("Segoe UI Symbol")
        } else {
            None
        };
    if let Some(s) = std {
        out.push(s.to_owned());
    }
    // "TimesNewRoman" → "Times New Roman".
    let mut spaced = String::new();
    for (i, c) in family.chars().enumerate() {
        if i > 0 && c.is_ascii_uppercase() && !spaced.ends_with(' ') {
            let prev_upper = spaced
                .chars()
                .last()
                .is_some_and(|p| p.is_ascii_uppercase());
            if !prev_upper {
                spaced.push(' ');
            }
        }
        spaced.push(c);
    }
    out.push(family.to_owned());
    if spaced != family {
        out.push(spaced);
    }
    out.push(
        if fixed {
            "Courier New"
        } else if serif {
            "Times New Roman"
        } else {
            "Arial"
        }
        .to_owned(),
    );
    (out, bold, italic)
}

/// Unicode vertical presentation form of CJK punctuation (U+FE10–FE19, U+FE30–FE44).
pub(crate) fn vertical_form(c: char) -> Option<char> {
    Some(match c {
        '\u{FF0C}' => '\u{FE10}',
        '\u{3001}' => '\u{FE11}',
        '\u{3002}' => '\u{FE12}',
        '\u{FF1A}' => '\u{FE13}',
        '\u{FF1B}' => '\u{FE14}',
        '\u{FF01}' => '\u{FE15}',
        '\u{FF1F}' => '\u{FE16}',
        '\u{3016}' => '\u{FE17}',
        '\u{3017}' => '\u{FE18}',
        '\u{2026}' => '\u{FE19}',
        '\u{2014}' => '\u{FE31}',
        '\u{2013}' => '\u{FE32}',
        '\u{FF08}' => '\u{FE35}',
        '\u{FF09}' => '\u{FE36}',
        '\u{FF5B}' => '\u{FE37}',
        '\u{FF5D}' => '\u{FE38}',
        '\u{3014}' => '\u{FE39}',
        '\u{3015}' => '\u{FE3A}',
        '\u{3010}' => '\u{FE3B}',
        '\u{3011}' => '\u{FE3C}',
        '\u{300A}' => '\u{FE3D}',
        '\u{300B}' => '\u{FE3E}',
        '\u{3008}' => '\u{FE3F}',
        '\u{3009}' => '\u{FE40}',
        '\u{300C}' => '\u{FE41}',
        '\u{300D}' => '\u{FE42}',
        '\u{300E}' => '\u{FE43}',
        '\u{300F}' => '\u{FE44}',
        _ => return None,
    })
}

/// Installed faces for a Chinese, Japanese or Korean composite font that is not embedded:
/// a character the face must have and the families to try, by the CIDFont's `/Ordering`
/// (Adobe-GB1, -CNS1, -Japan1, -Korea1) or the encoding CMap's name (§9.7.5.2), serif
/// (Song / Ming / Mincho / Batang) or sans (Hei / Gothic / Gulim) by the font's name.
pub(crate) fn cjk_substitutes(
    ordering: &str,
    encoding: &str,
    base: &str,
    serif: bool,
) -> Option<(char, Vec<&'static str>, bool)> {
    let flag_serif = serif;
    let e = encoding.to_ascii_lowercase();
    let script = match ordering {
        "GB1" => 0,
        "CNS1" => 1,
        "Japan1" => 2,
        "Korea1" => 3,
        _ if e.starts_with("unigb") || e.starts_with("gb") => 0,
        _ if e.starts_with("unicns") || e.starts_with("b5") || e.starts_with("eten") => 1,
        _ if e.starts_with("unijis")
            || e.contains("rksj")
            || e.starts_with("ext-")
            || e.starts_with("euc-")
            || e.starts_with("83pv") =>
        {
            2
        }
        _ if e.starts_with("uniks") || e.starts_with("ksc") => 3,
        _ => return None,
    };
    let b = base.to_ascii_lowercase();
    let sans = ["hei", "gothic", "gulim", "dotum", "sans"]
        .iter()
        .any(|k| b.contains(k));
    let serif = !sans
        && (serif
            || ["song", "ming", "mincho", "batang", "myungjo", "kai", "fang"]
                .iter()
                .any(|k| b.contains(k)));
    let mut list: Vec<&'static str> = match (script, serif) {
        (0, true) => vec!["SimSun", "NSimSun", "Microsoft YaHei", "DengXian"],
        // Without SimHei the reference shows the Song face (SimSun), not another sans.
        (0, false) => vec!["SimHei", "SimSun", "Microsoft YaHei", "DengXian"],
        (1, true) => vec!["MingLiU", "PMingLiU", "Microsoft JhengHei"],
        (1, false) => vec!["Microsoft JhengHei", "MingLiU", "PMingLiU"],
        (2, true) => vec![
            "MS Mincho",
            "Yu Mincho",
            "MS PMincho",
            "Yu Gothic",
            "MS Gothic",
        ],
        (2, false) => vec!["MS Gothic", "Yu Gothic", "Meiryo", "MS PGothic"],
        (3, true) => vec!["Batang", "Malgun Gothic", "Gulim"],
        _ => vec!["Malgun Gothic", "Gulim", "Dotum", "Batang"],
    };
    // Han characters of another script's face beat empty boxes.
    if script != 3 {
        list.extend([
            "Microsoft YaHei",
            "SimSun",
            "Microsoft JhengHei",
            "Yu Gothic",
            "MS Gothic",
        ]);
    }
    let sample = ['\u{4e2d}', '\u{4e2d}', '\u{65e5}', '\u{d55c}'][script];
    // Measured on the reference: a Traditional Chinese, Japanese or Korean font without the
    // Serif flag that is not installed (Fangti, Gothic, Dotum) shows in a bold substitute,
    // a Simplified Chinese one (Heiti) in the regular Song face. The caller applies it only
    // when the font itself is not installed.
    let heavy = script != 0 && !flag_serif;
    Some((sample, list, heavy))
}

impl Font {
    /// Load the font dictionary `dict` (`id` when it is an indirect object).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn load(doc: &Document, dict: &Dictionary, fonts: &mut FontManager) -> Self {
        let subtype = dict_get(doc, dict, b"Subtype")
            .and_then(|o| o.as_name().ok())
            .unwrap_or(b"Type1")
            .to_vec();
        let composite = subtype == b"Type0";
        let mut font = Font {
            composite,
            cmap: CMap::default(),
            to_unicode: None,
            names: vec![None; 256],
            widths: Vec::new(),
            first_char: 0,
            cid_widths: HashMap::new(),
            default_width: if composite { 1000.0 } else { 0.0 },
            vertical: false,
            dw2: [880.0, -1000.0],
            program: Program::None,
            face: None,
            substitute: false,
            symbol_cmap: false,
            cid_to_gid: None,
            cff_cids: HashMap::new(),
            matrix: [0.001, 0.0, 0.0, 0.001, 0.0, 0.0],
            ascent: 800.0,
            descent: -200.0,
            type3: None,
            gid_cache: RefCell::new(HashMap::new()),
            outlines: RefCell::new(HashMap::new()),
            named_outlines: RefCell::new(HashMap::new()),
            symbolic: false,
            sub_gids: Vec::new(),
            sub_data: None,
            face_widths: Vec::new(),
            cmaps: (false, false, false),
        };
        if let Some(tu) = dict_get(doc, dict, b"ToUnicode").and_then(|o| o.as_stream().ok()) {
            if let Ok(bytes) = filters::stream_bytes(doc, tu) {
                let m = CMap::parse(&bytes);
                if m.has_text() {
                    font.to_unicode = Some(m);
                }
            }
        }
        // Composite fonts: encoding CMap and the descendant CIDFont.
        let (desc_font, base_font) = if composite {
            font.cmap = match dict_get(doc, dict, b"Encoding") {
                Some(Object::Name(n)) => CMap::predefined(n),
                Some(Object::Stream(s)) => filters::stream_bytes(doc, s)
                    .map_or_else(|_| CMap::identity(false), |b| CMap::parse(&b)),
                _ => CMap::identity(false),
            };
            font.vertical = font.cmap.vertical;
            let cid = dict_get(doc, dict, b"DescendantFonts")
                .and_then(|o| o.as_array().ok())
                .and_then(|a| a.first())
                .map(|o| resolve(doc, o))
                .and_then(|o| o.as_dict().ok());
            (cid, cid)
        } else {
            (None, Some(dict))
        };
        let cid_dict = desc_font;
        let main = base_font.unwrap_or(dict);
        let base = dict_get(doc, dict, b"BaseFont")
            .or_else(|| dict_get(doc, main, b"BaseFont"))
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default();
        let descriptor = dict_get(doc, main, b"FontDescriptor").and_then(|o| o.as_dict().ok());
        let flags = descriptor
            .and_then(|d| dict_get(doc, d, b"Flags"))
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        font.symbolic = flags & 4 != 0;
        if let Some(d) = descriptor {
            if let Some(a) = dict_get(doc, d, b"Ascent").and_then(num) {
                if a > 0.0 && a < 2000.0 {
                    font.ascent = a;
                }
            }
            if let Some(a) = dict_get(doc, d, b"Descent").and_then(num) {
                if a < 0.0 && a > -1000.0 {
                    font.descent = a;
                }
            }
            if let Some(w) = dict_get(doc, d, b"MissingWidth").and_then(num) {
                if !composite {
                    font.default_width = w;
                }
            }
        }

        // Widths.
        if composite {
            if let Some(cd) = cid_dict {
                if let Some(dw) = dict_get(doc, cd, b"DW").and_then(num) {
                    font.default_width = dw;
                }
                if let Some(w) = dict_get(doc, cd, b"W").and_then(|o| o.as_array().ok()) {
                    cid_widths(doc, w, &mut font.cid_widths);
                }
                if let Some(dw2) = dict_get(doc, cd, b"DW2").and_then(|o| o.as_array().ok()) {
                    if let (Some(a), Some(b)) = (
                        dw2.first().map(|o| resolve(doc, o)).and_then(num),
                        dw2.get(1).map(|o| resolve(doc, o)).and_then(num),
                    ) {
                        font.dw2 = [a, b];
                    }
                }
            }
        } else {
            font.first_char = dict_get(doc, dict, b"FirstChar")
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0)
                .clamp(0, 255) as u32;
            if let Some(w) = dict_get(doc, dict, b"Widths").and_then(|o| o.as_array().ok()) {
                font.widths = w.iter().take(256).map(|o| num(resolve(doc, o))).collect();
            }
        }

        // Type 3.
        if subtype == b"Type3" {
            if let Some(m) = dict_get(doc, dict, b"FontMatrix").and_then(|o| o.as_array().ok()) {
                let v: Vec<f32> = m.iter().filter_map(|o| num(resolve(doc, o))).collect();
                if let Ok(m) = <[f32; 6]>::try_from(v.as_slice()) {
                    font.matrix = m;
                }
            }
            let procs = dict_get(doc, dict, b"CharProcs")
                .and_then(|o| o.as_dict().ok())
                .map(|d| d.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            font.type3 = Some(Type3 {
                procs,
                resources: dict_get(doc, dict, b"Resources")
                    .and_then(|o| o.as_dict().ok())
                    .cloned(),
            });
            font.apply_encoding(doc, dict, None);
            return font;
        }

        // Embedded program.
        let mut program_matrix = None;
        if let Some(d) = descriptor {
            if let Some(s) = dict_get(doc, d, b"FontFile2").and_then(|o| o.as_stream().ok()) {
                if let Ok(b) = filters::stream_bytes(doc, s) {
                    font.program = Program::Sfnt(Arc::new(b));
                }
            } else if let Some(s) = dict_get(doc, d, b"FontFile3").and_then(|o| o.as_stream().ok())
            {
                let sub = dict_get(doc, &s.dict, b"Subtype").and_then(|o| o.as_name().ok());
                if let Ok(b) = filters::stream_bytes(doc, s) {
                    font.program = if sub == Some(b"OpenType")
                        || b.starts_with(b"OTTO")
                        || b.starts_with(&[0, 1, 0, 0])
                    {
                        Program::Sfnt(Arc::new(b))
                    } else {
                        Program::Cff(Arc::new(b))
                    };
                }
            } else if let Some(s) = dict_get(doc, d, b"FontFile").and_then(|o| o.as_stream().ok()) {
                let l1 = dict_get(doc, &s.dict, b"Length1")
                    .and_then(|o| o.as_i64().ok())
                    .and_then(|v| usize::try_from(v).ok());
                if let Ok(b) = filters::stream_bytes(doc, s) {
                    if let Some(t1) = Type1::parse(&b, l1) {
                        program_matrix = Some(t1.matrix);
                        font.program = Program::Type1(Arc::new(t1));
                    } else if b.starts_with(&[1, 0, 4]) || b.starts_with(&[1, 0, 4, 2]) {
                        font.program = Program::Cff(Arc::new(b));
                    }
                }
            }
        }
        // Validate the program and register a drawable face.
        match font.program.clone() {
            Program::Sfnt(data) => {
                if let Ok(face) = ttf_parser::Face::parse(&data, 0) {
                    let upem = f32::from(face.units_per_em().max(16));
                    font.matrix = [1.0 / upem, 0.0, 0.0, 1.0 / upem, 0.0, 0.0];
                    font.cmaps = cmap_kinds(&face);
                    // CFF-flavoured OpenType: glyph ids follow the CFF charset.
                    if let Some(cff) = face.tables().cff {
                        font.cff_cids = cid_map(&cff);
                    }
                    let fixed = sfnt::make_loadable(&data, &base);
                    if let Some(bytes) = fixed {
                        font.face = fonts.add_font(bytes).first().copied();
                    }
                } else {
                    font.program = Program::None;
                }
            }
            Program::Cff(data) => {
                if let Some(cff) = ttf_parser::cff::Table::parse(&data) {
                    let m = cff.matrix();
                    font.matrix = [m.sx, m.ky, m.kx, m.sy, m.tx, m.ty];
                    font.cff_cids = cid_map(&cff);
                    if let Some(otf) = sfnt::wrap_cff(&data, &base, cff.number_of_glyphs()) {
                        font.face = fonts.add_font(otf).first().copied();
                    }
                } else {
                    font.program = Program::None;
                }
            }
            Program::Type1(_) => {
                if let Some(m) = program_matrix {
                    font.matrix = m;
                }
            }
            Program::None => {}
        }
        // CIDToGIDMap (CIDFontType2).
        if let Some(cd) = cid_dict {
            if let Some(Object::Stream(s)) = dict_get(doc, cd, b"CIDToGIDMap") {
                if let Ok(b) = filters::stream_bytes(doc, s) {
                    font.cid_to_gid = Some(
                        b.chunks_exact(2)
                            .map(|c| u16::from(c[0]) << 8 | u16::from(c[1]))
                            .collect(),
                    );
                }
            }
        }
        // Substitute for fonts that are not embedded (or unreadable).
        if matches!(font.program, Program::None) {
            let serif = flags & 2 != 0;
            let fixed = flags & 1 != 0;
            let (families, bold, italic) = substitute_names(base_name(&base), serif, fixed);
            let bold = bold || flags & (1 << 18) != 0;
            let italic = italic || flags & (1 << 6) != 0;
            // Chinese, Japanese and Korean fonts that are not embedded: an installed face of
            // that script which has its characters (the bundled faces have none).
            if composite {
                let ordering = cid_dict
                    .and_then(|cd| dict_get(doc, cd, b"CIDSystemInfo"))
                    .and_then(|o| o.as_dict().ok())
                    .and_then(|d| dict_get(doc, d, b"Ordering"))
                    .and_then(|o| o.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned())
                    .unwrap_or_default();
                let encoding = dict_get(doc, dict, b"Encoding")
                    .and_then(|o| o.as_name().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned())
                    .unwrap_or_default();
                if let Some((sample, list, heavy)) =
                    cjk_substitutes(&ordering, &encoding, &base, serif)
                {
                    // Weight from the descriptor (§9.8.1 `/FontWeight`, `/StemV`): CJK names
                    // rarely say "Bold".
                    let value = |k: &[u8]| {
                        descriptor
                            .and_then(|d| dict_get(doc, d, k))
                            .and_then(num)
                            .unwrap_or(0.0)
                    };
                    let squash = |s: &str| {
                        s.chars()
                            .filter(char::is_ascii_alphanumeric)
                            .collect::<String>()
                            .to_ascii_lowercase()
                    };
                    // The PostScript name as installed family names ("MS-Gothic" → "MS
                    // Gothic", "MalgunGothic" → "Malgun Gothic").
                    let name = base_name(&base).split(',').next().unwrap_or("");
                    let wanted = squash(name);
                    let mut spaced = String::new();
                    let mut prev_lower = false;
                    for c in name.chars() {
                        if c == '-' || c == '_' {
                            spaced.push(' ');
                        } else {
                            if c.is_ascii_uppercase() && prev_lower {
                                spaced.push(' ');
                            }
                            spaced.push(c);
                        }
                        prev_lower = c.is_ascii_lowercase();
                    }
                    let installed = [name, spaced.as_str()].iter().any(|n| {
                        fonts
                            .display_family(n)
                            .is_some_and(|f| squash(&f) == wanted)
                    });
                    let bold = bold
                        || (heavy && !installed)
                        || value(b"FontWeight") >= 600.0
                        || value(b"StemV") >= 140.0;
                    font.face = list.iter().find_map(|fam| {
                        fonts
                            .resolve(fam, bold, italic)
                            .filter(|&id| fonts.has_glyph(id, sample))
                    });
                }
            }
            for fam in &families {
                if font.face.is_some() {
                    break;
                }
                if let Some(id) = fonts.resolve(fam, bold, italic) {
                    font.face = Some(id);
                    break;
                }
            }
            font.substitute = true;
            if composite {
                font.sub_data = font
                    .face
                    .map(|id| fonts.data(id))
                    .map(|(d, i)| (Arc::new(d.to_vec()), i));
            }
            if let Some(id) = font.face {
                if let Some(face) = fonts.face(id) {
                    let upem = f32::from(face.units_per_em().max(16));
                    font.matrix = [1.0 / upem, 0.0, 0.0, 1.0 / upem, 0.0, 0.0];
                    let k = cmap_kinds(&face);
                    font.symbol_cmap = k.1 && !k.0;
                }
            }
        }
        if !composite {
            let std_name = base_name(&base).to_ascii_lowercase();
            let builtin = match &font.program {
                Program::Type1(t) if !t.standard_encoding && !t.encoding.is_empty() => {
                    Some(t.encoding.clone())
                }
                // Standard 14 symbol fonts not embedded: their built-in encodings (Annex D.5,
                // D.6), whatever face substitutes them.
                Program::None if std_name.starts_with("zapfdingbats") => Some(
                    (0..256_usize)
                        .filter_map(|c| encoding::zapf_dingbats(c).map(|n| (c as u8, n)))
                        .collect(),
                ),
                Program::None if std_name == "symbol" || std_name.starts_with("symbol,") => Some(
                    encoding::symbol()
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| !n.is_empty())
                        .map(|(c, n)| (c as u8, (*n).to_owned()))
                        .collect(),
                ),
                _ => None,
            };
            font.apply_encoding(doc, dict, builtin);
            if font.substitute {
                if let Some(face) = font.face.and_then(|f| fonts.face(f)) {
                    let upem = f32::from(face.units_per_em().max(16));
                    font.sub_gids = (0..256).map(|c| font.substitute_glyph(c, &face)).collect();
                    font.face_widths = font
                        .sub_gids
                        .iter()
                        .map(|g| {
                            g.and_then(|g| face.glyph_hor_advance(ttf_parser::GlyphId(g)))
                                .map(|a| f32::from(a) * 1000.0 / upem)
                        })
                        .collect();
                }
            }
            // An embedded TrueType font without /Widths (allowed for symbolic fonts in old
            // files): the program's own advances, as other readers use (else every glyph would
            // advance by /MissingWidth, often 0).
            if font.widths.is_empty() && !font.substitute {
                if let Program::Sfnt(data) = &font.program {
                    let data = Arc::clone(data);
                    if let Ok(face) = ttf_parser::Face::parse(&data, 0) {
                        let upem = f32::from(face.units_per_em().max(16));
                        font.face_widths = (0..256)
                            .map(|c| match font.glyph(c) {
                                Some(GlyphRef::Gid(g)) => face
                                    .glyph_hor_advance(ttf_parser::GlyphId(g))
                                    .map(|a| f32::from(a) * 1000.0 / upem),
                                _ => None,
                            })
                            .collect();
                    }
                }
            }
        }
        font
    }

    /// Fill `names` from `/Encoding` (base + differences) or the built-in encoding.
    fn apply_encoding(
        &mut self,
        doc: &Document,
        dict: &Dictionary,
        builtin: Option<HashMap<u8, String>>,
    ) {
        let enc = dict_get(doc, dict, b"Encoding");
        let base_name = match enc {
            Some(Object::Name(n)) => Some(n.as_slice()),
            Some(Object::Dictionary(d)) => {
                dict_get(doc, d, b"BaseEncoding").and_then(|o| o.as_name().ok())
            }
            _ => None,
        };
        let symbolic_std =
            matches!(self.program, Program::None) && self.face.is_some() && self.symbol_cmap;
        // A non-embedded Symbol / ZapfDingbats keeps its built-in encoding even when the font
        // dictionary names a base encoding (symbolic fonts, §9.6.6.1; PyMuPDF writes
        // `/WinAnsiEncoding` on its ZapfDingbats check marks and readers show the ✓ glyph);
        // `/Differences` still apply.
        let std_symbol = matches!(self.program, Program::None) && builtin.is_some();
        let base = base_name.filter(|_| !std_symbol).and_then(encoding::base);
        if let Some(b) = base {
            for (i, n) in b.iter().enumerate() {
                if !n.is_empty() {
                    self.names[i] = Some((*n).to_owned());
                }
            }
        } else if let Some(b) = builtin {
            for (k, v) in b {
                self.names[usize::from(k)] = Some(v);
            }
        } else if symbolic_std {
            for (i, n) in encoding::symbol().iter().enumerate() {
                if !n.is_empty() {
                    self.names[i] = Some((*n).to_owned());
                }
            }
        } else if !self.symbolic || matches!(self.program, Program::None) {
            let table = if matches!(self.program, Program::Sfnt(_)) || self.type3.is_some() {
                encoding::win_ansi()
            } else {
                encoding::standard()
            };
            for (i, n) in table.iter().enumerate() {
                if !n.is_empty() {
                    self.names[i] = Some((*n).to_owned());
                }
            }
        }
        if let Some(Object::Dictionary(d)) = enc {
            if let Some(diff) = dict_get(doc, d, b"Differences").and_then(|o| o.as_array().ok()) {
                let mut code = 0_usize;
                for o in diff {
                    match resolve(doc, o) {
                        Object::Integer(i) => code = (*i).clamp(0, 255) as usize,
                        Object::Name(n) => {
                            if code < 256 {
                                self.names[code] = Some(String::from_utf8_lossy(n).into_owned());
                            }
                            code += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Split a shown string into codes with their byte lengths.
    pub(crate) fn codes(&self, s: &[u8]) -> Vec<(u32, usize)> {
        if self.composite {
            self.cmap.codes(s)
        } else {
            s.iter().map(|&b| (u32::from(b), 1)).collect()
        }
    }

    /// Horizontal advance of a code in text space thousandths (vertical fonts: the vertical one).
    pub(crate) fn width(&self, code: u32) -> f32 {
        if self.composite {
            if self.vertical {
                return -self.dw2[1];
            }
            let cid = if self.cmap.unicode {
                // The CIDs of a Unicode CMap are not known without Adobe's tables; printable
                // ASCII is CID 1–95 in all four Adobe CJK orderings (Adobe-GB1, -CNS1,
                // -Japan1, -Korea1), anything else takes the default width.
                if (0x20..=0x7e).contains(&code) {
                    code - 31
                } else {
                    u32::MAX
                }
            } else {
                self.cmap.cid(code)
            };
            return self
                .cid_widths
                .get(&cid)
                .copied()
                .unwrap_or(self.default_width);
        }
        let i = code.wrapping_sub(self.first_char) as usize;
        if code >= self.first_char {
            if let Some(Some(w)) = self.widths.get(i) {
                return *w;
            }
        }
        if !self.widths.is_empty() && self.default_width > 0.0 {
            return self.default_width;
        }
        // No widths (standard 14 fonts in old files): the face's own advance.
        self.face_widths
            .get(code as usize)
            .copied()
            .flatten()
            .unwrap_or(self.default_width)
    }

    /// Unicode text of a code.
    pub(crate) fn text(&self, code: u32) -> String {
        if let Some(t) = self.to_unicode.as_ref().and_then(|m| m.text(code)) {
            return t;
        }
        if self.composite && self.cmap.unicode {
            // Predefined Unicode CMaps: the code is the character (§9.7.5.2).
            return char::from_u32(code).map(String::from).unwrap_or_default();
        }
        if !self.composite {
            if let Some(Some(n)) = self.names.get(code as usize) {
                if let Some(t) = encoding::unicode(n) {
                    return t;
                }
            }
            return char::from_u32(code).map(String::from).unwrap_or_default();
        }
        String::new()
    }

    /// Vertical position vector and displacement of vertical fonts (thousandths).
    pub(crate) fn vertical_metrics(&self, code: u32) -> (f32, f32) {
        let cid = self.cmap.cid(code);
        let w0 = self
            .cid_widths
            .get(&cid)
            .copied()
            .unwrap_or(self.default_width);
        (w0 / 2.0, self.dw2[0])
    }

    /// Glyph of a code.
    pub(crate) fn glyph(&self, code: u32) -> Option<GlyphRef> {
        if let Some(g) = self.gid_cache.borrow().get(&code) {
            return g.clone();
        }
        let g = self.lookup(code);
        self.gid_cache.borrow_mut().insert(code, g.clone());
        g
    }

    fn lookup(&self, code: u32) -> Option<GlyphRef> {
        if self.composite {
            let cid = self.cmap.cid(code);
            if self.substitute {
                return None; // resolved by Unicode through `substitute_glyph`
            }
            return match &self.program {
                Program::Sfnt(_) => Some(GlyphRef::Gid(match &self.cid_to_gid {
                    Some(map) => map.get(cid as usize).copied().unwrap_or(0),
                    None => u16::try_from(cid).unwrap_or(0),
                }))
                .map(|g| match (g, self.cff_cids.is_empty()) {
                    (GlyphRef::Gid(gid), false) => GlyphRef::Gid(
                        self.cff_cids
                            .get(&u16::try_from(cid).unwrap_or(0))
                            .copied()
                            .unwrap_or(gid),
                    ),
                    (g, _) => g,
                }),
                Program::Cff(_) => {
                    let cid16 = u16::try_from(cid).unwrap_or(0);
                    Some(GlyphRef::Gid(if self.cff_cids.is_empty() {
                        cid16
                    } else {
                        self.cff_cids.get(&cid16).copied().unwrap_or(0)
                    }))
                }
                Program::Type1(_) | Program::None => None,
            };
        }
        if self.substitute {
            return self
                .sub_gids
                .get(code as usize)
                .copied()
                .flatten()
                .map(GlyphRef::Gid);
        }
        let name = self.names.get(code as usize).and_then(Clone::clone);
        match &self.program {
            Program::Type1(t) => {
                let n = name
                    .filter(|n| t.has(n))
                    .or_else(|| t.encoding.get(&u8::try_from(code).ok()?).cloned())?;
                Some(GlyphRef::Name(n))
            }
            Program::Cff(data) => {
                let cff = ttf_parser::cff::Table::parse(data)?;
                if let Some(n) = &name {
                    if let Some(g) = cff.glyph_index_by_name(n) {
                        return Some(GlyphRef::Gid(g.0));
                    }
                }
                cff.glyph_index(u8::try_from(code).ok()?)
                    .map(|g| GlyphRef::Gid(g.0))
            }
            Program::Sfnt(data) => {
                let face = ttf_parser::Face::parse(data, 0).ok()?;
                truetype_gid(&face, code, name.as_deref(), self.symbolic, self.cmaps)
                    .map(GlyphRef::Gid)
            }
            Program::None => None,
        }
    }

    /// Codes that show each character with a glyph this font has (to write edited text in
    /// the run's own font): simple fonts by their encoding, composite fonts with a two-byte
    /// identity CMap through `/ToUnicode`. `None` when text cannot be encoded for this font.
    pub(crate) fn encoder(&self) -> Option<HashMap<char, Vec<u8>>> {
        let mut out: HashMap<char, Vec<u8>> = HashMap::new();
        let mut add = |text: String, bytes: Vec<u8>| {
            let mut it = text.chars();
            if let (Some(c), None) = (it.next(), it.next()) {
                out.entry(c).or_insert(bytes);
            }
        };
        if self.composite {
            if self.substitute || self.to_unicode.is_none() {
                return None;
            }
            for code in 0..=0xffff_u32 {
                let b = [(code >> 8) as u8, code as u8];
                if self.cmap.codes(&b) != [(code, 2)] {
                    return None;
                }
                let t = self.text(code);
                if t.is_empty() {
                    continue;
                }
                if matches!(self.glyph(code), Some(GlyphRef::Gid(0)) | None) {
                    continue;
                }
                add(t, b.to_vec());
            }
        } else {
            if self.type3.is_some() {
                return None;
            }
            for code in 0..=255_u32 {
                let ok = match self.glyph(code) {
                    Some(GlyphRef::Gid(g)) => g != 0,
                    Some(GlyphRef::Name(n)) => n != ".notdef",
                    None => false,
                };
                if ok {
                    add(self.text(code), vec![code as u8]);
                }
            }
        }
        (!out.is_empty()).then_some(out)
    }

    /// Glyph name of a simple font's code (Type 3 glyph procedures).
    pub(crate) fn glyph_name(&self, code: u32) -> Option<&str> {
        self.names.get(code as usize).and_then(|n| n.as_deref())
    }

    /// Whether the program is a Type 1 font drawn from outlines.
    pub(crate) fn is_type1_program(&self) -> bool {
        matches!(self.program, Program::Type1(_))
    }

    /// Glyph of a code in a substitute face (`face`'s data), by Unicode.
    pub(crate) fn substitute_glyph(&self, code: u32, face: &ttf_parser::Face<'_>) -> Option<u16> {
        if self.symbol_cmap && !self.composite {
            for base in [0xf000_u32, 0] {
                if let Some(g) = char::from_u32(base + code).and_then(|c| face.glyph_index(c)) {
                    return Some(g.0);
                }
            }
        }
        let text = self.text(code);
        let c = text.chars().next()?;
        // Vertical writing with a substitute face: punctuation in its vertical presentation
        // form when the face has one (what a font's `vert` feature gives; the reference
        // turns "：" and puts "。" in the upper right of its cell).
        if self.vertical {
            if let Some(g) = vertical_form(c).and_then(|v| face.glyph_index(v)) {
                return Some(g.0);
            }
        }
        face.glyph_index(c).map(|g| g.0)
    }

    /// Face file of a composite font's substitute.
    pub(crate) fn substitute_data(&self) -> Option<(Arc<Vec<u8>>, u32)> {
        self.sub_data.clone()
    }

    /// Outline of a glyph in glyph space (y up), for drawing as a path.
    pub(crate) fn outline(&self, g: &GlyphRef, fonts: &FontManager) -> Option<Arc<Vec<PathSeg>>> {
        match g {
            GlyphRef::Name(n) => {
                if let Some(o) = self.named_outlines.borrow().get(n) {
                    return o.clone();
                }
                let o = match &self.program {
                    Program::Type1(t) => t.outline(n).map(Arc::new),
                    _ => None,
                };
                self.named_outlines
                    .borrow_mut()
                    .insert(n.clone(), o.clone());
                o
            }
            GlyphRef::Gid(gid) => {
                if let Some(o) = self.outlines.borrow().get(gid) {
                    return o.clone();
                }
                let mut b = Builder(Vec::new());
                let id = ttf_parser::GlyphId(*gid);
                let ok = match &self.program {
                    Program::Sfnt(d) => ttf_parser::Face::parse(d, 0)
                        .ok()
                        .and_then(|f| f.outline_glyph(id, &mut b))
                        .is_some(),
                    Program::Cff(d) => ttf_parser::cff::Table::parse(d)
                        .and_then(|c| c.outline(id, &mut b).ok())
                        .is_some(),
                    _ => self
                        .face
                        .and_then(|f| fonts.face(f))
                        .and_then(|f| f.outline_glyph(id, &mut b))
                        .is_some(),
                };
                let o = ok.then(|| Arc::new(b.0));
                self.outlines.borrow_mut().insert(*gid, o.clone());
                o
            }
        }
    }
}

/// Collects outline segments.
struct Builder(Vec<PathSeg>);

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(PathSeg::M(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(PathSeg::L(x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.push(PathSeg::Q(x1, y1, x, y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(PathSeg::C(x1, y1, x2, y2, x, y));
    }
    fn close(&mut self) {
        self.0.push(PathSeg::Z);
    }
}

fn cmap_kinds(face: &ttf_parser::Face<'_>) -> (bool, bool, bool) {
    let mut k = (false, false, false);
    if let Some(cmap) = face.tables().cmap {
        for s in cmap.subtables {
            match (s.platform_id, s.encoding_id) {
                (ttf_parser::PlatformId::Windows, 1 | 10)
                | (ttf_parser::PlatformId::Unicode, _) => {
                    k.0 = true;
                }
                (ttf_parser::PlatformId::Windows, 0) => k.1 = true,
                (ttf_parser::PlatformId::Macintosh, 0) => k.2 = true,
                _ => {}
            }
        }
    }
    k
}

fn cid_map(cff: &ttf_parser::cff::Table<'_>) -> HashMap<u16, u16> {
    let mut m = HashMap::new();
    for g in 0..cff.number_of_glyphs() {
        if let Some(cid) = cff.glyph_cid(ttf_parser::GlyphId(g)) {
            m.insert(cid, g);
        }
    }
    // Not CID-keyed: glyph_cid is None for every glyph.
    m
}

/// Glyph of a simple TrueType font's code (ISO 32000-1, §9.6.6.4).
fn truetype_gid(
    face: &ttf_parser::Face<'_>,
    code: u32,
    name: Option<&str>,
    symbolic: bool,
    kinds: (bool, bool, bool),
) -> Option<u16> {
    let cmap = face.tables().cmap?;
    let sub = |pid: ttf_parser::PlatformId, eid: Option<u16>, c: u32| {
        cmap.subtables
            .into_iter()
            .filter(|s| s.platform_id == pid && eid.is_none_or(|e| s.encoding_id == e))
            .find_map(|s| s.glyph_index(c))
            .map(|g| g.0)
            .filter(|g| *g != 0)
    };
    use ttf_parser::PlatformId::{Macintosh, Unicode, Windows};
    if kinds.0 && (!symbolic || !kinds.1) {
        if let Some(u) = name
            .and_then(encoding::unicode)
            .and_then(|t| t.chars().next())
        {
            if let Some(g) = sub(Windows, None, u as u32).or_else(|| sub(Unicode, None, u as u32)) {
                return Some(g);
            }
        }
    }
    if kinds.1 {
        for base in [0xf000, 0xf100, 0xf200, 0] {
            if let Some(g) = sub(Windows, Some(0), base + code) {
                return Some(g);
            }
        }
    }
    if kinds.2 {
        let c = name
            .and_then(encoding::mac_roman_code)
            .map_or(code, u32::from);
        if let Some(g) = sub(Macintosh, Some(0), c) {
            return Some(g);
        }
    }
    if let Some(g) = name.and_then(|n| face.glyph_index_by_name(n)) {
        return Some(g.0);
    }
    if kinds.0 {
        if let Some(g) = sub(Windows, None, code).or_else(|| sub(Unicode, None, code)) {
            return Some(g);
        }
    }
    // No usable cmap: codes are glyph ids (common in subset fonts).
    u16::try_from(code).ok()
}

fn cid_widths(doc: &Document, w: &[Object], out: &mut HashMap<u32, f32>) {
    let mut i = 0;
    while i < w.len() {
        let Some(first) = resolve(doc, &w[i]).as_i64().ok() else {
            i += 1;
            continue;
        };
        match w.get(i + 1).map(|o| resolve(doc, o)) {
            Some(Object::Array(ws)) => {
                for (k, v) in ws.iter().enumerate() {
                    if let Some(v) = num(resolve(doc, v)) {
                        out.insert((first.max(0) as u32).saturating_add(k as u32), v);
                    }
                }
                i += 2;
            }
            Some(last) => {
                let last = last.as_i64().unwrap_or(first);
                let v = w
                    .get(i + 2)
                    .map(|o| resolve(doc, o))
                    .and_then(num)
                    .unwrap_or(1000.0);
                let span = (last - first).clamp(0, 65535);
                for c in first..=first + span {
                    out.insert(c.max(0) as u32, v);
                }
                i += 3;
            }
            None => break,
        }
    }
}

/// Cache of loaded fonts by object id.
#[derive(Default)]
pub(crate) struct FontCache {
    by_id: HashMap<ObjectId, Rc<Font>>,
    /// Direct font dictionaries by their content (a dictionary's address is not stable:
    /// resources resolved into temporaries reuse addresses).
    inline: HashMap<String, Rc<Font>>,
}

impl FontCache {
    /// The font for a resource entry.
    pub(crate) fn get(
        &mut self,
        doc: &Document,
        o: &Object,
        fonts: &mut FontManager,
    ) -> Option<Rc<Font>> {
        match o {
            Object::Reference(id) => {
                if let Some(f) = self.by_id.get(id) {
                    return Some(f.clone());
                }
                let d = doc.get_object(*id).ok()?.as_dict().ok()?;
                let f = Rc::new(Font::load(doc, d, fonts));
                self.by_id.insert(*id, f.clone());
                Some(f)
            }
            Object::Dictionary(d) => {
                let key = format!("{d:?}");
                if let Some(f) = self.inline.get(&key) {
                    return Some(f.clone());
                }
                let f = Rc::new(Font::load(doc, d, fonts));
                self.inline.insert(key, f.clone());
                Some(f)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_names_for_standard_fonts() {
        let (f, b, i) = substitute_names("Helvetica-BoldOblique", false, false);
        assert_eq!(f[0], "Arial");
        assert!(b && i);
        let (f, b, _) = substitute_names("TimesNewRomanPS-BoldMT", false, false);
        assert!(f.iter().any(|n| n == "Times New Roman"));
        assert!(b);
        assert_eq!(base_name("ABCDEF+Calibri"), "Calibri");
        assert_eq!(base_name("Calibri"), "Calibri");
    }
}
