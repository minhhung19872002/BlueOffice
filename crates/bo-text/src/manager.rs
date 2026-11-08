use std::collections::HashMap;
use std::sync::Arc;

/// Index of a face inside a [`FontManager`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct FaceId(pub u32);

/// Metadata and metrics of a loaded face (metrics in font units).
#[derive(Debug, Clone)]
pub struct FaceInfo {
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub underline_position: i16,
    pub underline_thickness: i16,
    pub strikeout_position: i16,
}

struct Face {
    data: Arc<Vec<u8>>,
    index: u32,
    info: FaceInfo,
}

/// A shaped glyph in font units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    /// Byte offset of the cluster in the shaped text.
    pub cluster: u32,
    pub x_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}

/// Metric-compatible substitutes (product brief §2). Keys are lower-case.
const FALLBACKS: &[(&str, &[&str])] = &[
    ("times new roman", &["liberation serif", "tinos"]),
    ("times", &["liberation serif", "tinos"]),
    ("arial", &["liberation sans", "arimo"]),
    ("helvetica", &["liberation sans", "arimo"]),
    ("calibri", &["carlito"]),
    ("calibri light", &["carlito"]),
    // Common presentation theme fonts without a metric-compatible substitute:
    // nearest bundled sans / serif design.
    ("aptos", &["carlito"]),
    ("segoe ui", &["carlito", "liberation sans"]),
    ("verdana", &["liberation sans", "arimo"]),
    ("tahoma", &["liberation sans", "arimo"]),
    ("georgia", &["liberation serif", "tinos"]),
    ("cambria", &["caladea"]),
    ("courier new", &["liberation mono", "cousine"]),
];

/// Family used when nothing else matches.
const LAST_RESORT: &[&str] = &["tinos", "liberation serif", "carlito", "liberation sans"];

/// Owns fonts and a shaping cache.
#[derive(Default)]
pub struct FontManager {
    faces: Vec<Face>,
    by_family: HashMap<String, Vec<FaceId>>,
    shape_cache: HashMap<(FaceId, bool, String), Arc<[ShapedGlyph]>>,
    resolve_cache: HashMap<(String, bool, bool), Option<FaceId>>,
    /// Installed OS fonts, loaded per family on first use (native hosts that opt in).
    #[cfg(not(target_arch = "wasm32"))]
    system: Option<crate::SystemFontIndex>,
    /// Lower-case families whose system files were already read.
    #[cfg(not(target_arch = "wasm32"))]
    system_loaded: std::collections::HashSet<String>,
}

impl FontManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// True when no fonts are loaded.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Register a font file (TTF/OTF, or every face of a TTC). Returns the new face ids.
    pub fn add_font(&mut self, data: Vec<u8>) -> Vec<FaceId> {
        let data = Arc::new(data);
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        let mut ids = Vec::new();
        for index in 0..count {
            let Ok(face) = ttf_parser::Face::parse(&data, index) else {
                continue;
            };
            let Some(family) = family_name(&face) else {
                continue;
            };
            let underline = face.underline_metrics();
            let strike = face.strikeout_metrics();
            let info = FaceInfo {
                bold: face.is_bold() || face.weight().to_number() >= 600,
                italic: face.is_italic() || face.is_oblique(),
                units_per_em: face.units_per_em(),
                ascender: face.ascender(),
                descender: face.descender(),
                line_gap: face.line_gap(),
                underline_position: underline
                    .map_or(-(face.units_per_em() as i16) / 10, |m| m.position),
                underline_thickness: underline
                    .map_or((face.units_per_em() / 20) as i16, |m| m.thickness),
                strikeout_position: strike.map_or((face.units_per_em() / 4) as i16, |m| m.position),
                family,
            };
            let id = FaceId(self.faces.len() as u32);
            self.by_family
                .entry(info.family.to_lowercase())
                .or_default()
                .push(id);
            self.faces.push(Face {
                data: data.clone(),
                index,
                info,
            });
            ids.push(id);
        }
        self.resolve_cache.clear();
        ids
    }

    /// Load every `.ttf`/`.otf`/`.ttc` in a directory (native targets only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_fonts_from_dir(&mut self, dir: &std::path::Path) -> usize {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut n = 0;
        for entry in rd.flatten() {
            let p = entry.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                if let Ok(bytes) = std::fs::read(&p) {
                    n += self.add_font(bytes).len();
                }
            }
        }
        n
    }

    /// Uses the installed fonts of `index` for families that are not loaded:
    /// a document asking for one loads its files then (ids after the bundled fonts).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_system_fonts(&mut self, index: crate::SystemFontIndex) {
        self.system = Some(index);
        self.system_loaded.clear();
        self.resolve_cache.clear();
    }

    /// Installed families available through [`FontManager::set_system_fonts`] (sorted).
    pub fn system_families(&self) -> Vec<String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(idx) = &self.system {
            return idx.families();
        }
        Vec::new()
    }

    /// Loads the system files of `family` once; true when the family is now loaded.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_system_family(&mut self, family: &str) -> bool {
        let Some(idx) = self.system.clone() else {
            return false;
        };
        if !idx.contains(family) || !self.system_loaded.insert(family.to_owned()) {
            return false;
        }
        for path in idx.files_of(family) {
            if let Ok(bytes) = std::fs::read(path) {
                self.add_font(bytes);
            }
        }
        self.by_family.contains_key(family)
    }

    #[cfg(target_arch = "wasm32")]
    #[allow(clippy::unused_self)]
    fn load_system_family(&mut self, _family: &str) -> bool {
        false
    }

    /// Face metadata.
    pub fn info(&self, id: FaceId) -> &FaceInfo {
        &self.faces[id.0 as usize].info
    }

    /// Raw font file bytes and collection index (for PDF embedding).
    pub fn data(&self, id: FaceId) -> (&[u8], u32) {
        let f = &self.faces[id.0 as usize];
        (&f.data, f.index)
    }

    /// Parse the face (cheap: ttf-parser is zero-copy).
    pub fn face(&self, id: FaceId) -> Option<ttf_parser::Face<'_>> {
        let f = &self.faces[id.0 as usize];
        ttf_parser::Face::parse(&f.data, f.index).ok()
    }

    /// Resolve a family name and style to a face, applying the fallback table.
    pub fn resolve(&mut self, family: &str, bold: bool, italic: bool) -> Option<FaceId> {
        let key = (family.to_lowercase(), bold, italic);
        if let Some(r) = self.resolve_cache.get(&key) {
            return *r;
        }
        let r = self.resolve_uncached(&key.0, bold, italic);
        self.resolve_cache.insert(key, r);
        r
    }

    /// Family that would display `family` (same order as [`FontManager::resolve`]),
    /// without loading any installed font file.
    pub fn display_family(&self, family: &str) -> Option<String> {
        let lower = family.to_lowercase();
        let mut candidates: Vec<&str> = vec![&lower];
        if let Some((_, subs)) = FALLBACKS.iter().find(|(k, _)| *k == lower) {
            candidates.extend_from_slice(subs);
        }
        candidates.extend_from_slice(LAST_RESORT);
        for fam in candidates {
            if let Some(ids) = self.by_family.get(fam) {
                return ids.first().map(|id| self.info(*id).family.clone());
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(idx) = &self.system {
                if idx.contains(fam) {
                    return idx
                        .families()
                        .into_iter()
                        .find(|f| f.eq_ignore_ascii_case(fam));
                }
            }
        }
        self.faces.first().map(|f| f.info.family.clone())
    }

    fn resolve_uncached(&mut self, family: &str, bold: bool, italic: bool) -> Option<FaceId> {
        let mut candidates: Vec<&str> = vec![family];
        if let Some((_, subs)) = FALLBACKS.iter().find(|(k, _)| *k == family) {
            candidates.extend_from_slice(subs);
        }
        candidates.extend_from_slice(LAST_RESORT);
        for fam in candidates {
            // Loaded (bundled) family first, then the installed font of that name;
            // the metric-compatible substitute only when neither exists.
            if !self.by_family.contains_key(fam) {
                self.load_system_family(fam);
            }
            if let Some(ids) = self.by_family.get(fam) {
                return Some(self.best_style(ids, bold, italic));
            }
        }
        self.faces.first().map(|_| {
            self.best_style(
                &(0..self.faces.len() as u32).map(FaceId).collect::<Vec<_>>(),
                bold,
                italic,
            )
        })
    }

    fn best_style(&self, ids: &[FaceId], bold: bool, italic: bool) -> FaceId {
        *ids.iter()
            .min_by_key(|id| {
                let i = self.info(**id);
                (i.italic != italic) as u8 * 2 + (i.bold != bold) as u8
            })
            .expect("non-empty family")
    }

    /// Does the face have a glyph for `c`?
    pub fn has_glyph(&self, id: FaceId, c: char) -> bool {
        self.face(id).is_some_and(|f| f.glyph_index(c).is_some())
    }

    /// Split `text` into runs of faces: `primary` where it has glyphs, otherwise a
    /// face of the same style that does. Returns byte ranges.
    pub fn itemize(&self, primary: FaceId, text: &str) -> Vec<(std::ops::Range<usize>, FaceId)> {
        let mut out: Vec<(std::ops::Range<usize>, FaceId)> = Vec::new();
        let style = {
            let i = self.info(primary);
            (i.bold, i.italic)
        };
        let primary_face = self.face(primary);
        for (pos, c) in text.char_indices() {
            let covered = c.is_control()
                || is_mark(c)
                || primary_face
                    .as_ref()
                    .is_some_and(|f| f.glyph_index(c).is_some());
            let face = if covered {
                // Combining marks stay with the preceding run's face.
                if is_mark(c) {
                    out.last().map_or(primary, |l| l.1)
                } else {
                    primary
                }
            } else {
                self.fallback_for(c, style).unwrap_or(primary)
            };
            let end = pos + c.len_utf8();
            match out.last_mut() {
                Some(last) if last.1 == face && last.0.end == pos => last.0.end = end,
                _ => out.push((pos..end, face)),
            }
        }
        out
    }

    fn fallback_for(&self, c: char, (bold, italic): (bool, bool)) -> Option<FaceId> {
        let mut best: Option<(u8, FaceId)> = None;
        for (i, f) in self.faces.iter().enumerate() {
            let Ok(face) = ttf_parser::Face::parse(&f.data, f.index) else {
                continue;
            };
            if face.glyph_index(c).is_some() {
                let score = (f.info.italic != italic) as u8 * 2 + (f.info.bold != bold) as u8;
                if best.is_none_or(|(s, _)| score < s) {
                    best = Some((score, FaceId(i as u32)));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// Shape `text` with `face`. Glyph metrics are in font units. Cached.
    pub fn shape(&mut self, id: FaceId, text: &str) -> Arc<[ShapedGlyph]> {
        self.shape_kern(id, text, true)
    }

    /// Shape `text` with pair kerning on or off (OpenType `kern` feature; word
    /// processing applies kerning only from the size given by `w:kern`, ECMA-376
    /// Part 1 §17.3.2.19). Cached.
    pub fn shape_kern(&mut self, id: FaceId, text: &str, kern: bool) -> Arc<[ShapedGlyph]> {
        let key = (id, kern, text.to_owned());
        if let Some(g) = self.shape_cache.get(&key) {
            return g.clone();
        }
        let f = &self.faces[id.0 as usize];
        let glyphs: Arc<[ShapedGlyph]> = match rustybuzz::Face::from_slice(&f.data, f.index) {
            Some(face) => {
                let mut buf = rustybuzz::UnicodeBuffer::new();
                buf.push_str(text);
                let off = [rustybuzz::Feature::new(
                    rustybuzz::ttf_parser::Tag::from_bytes(b"kern"),
                    0,
                    ..,
                )];
                let features: &[rustybuzz::Feature] = if kern { &[] } else { &off };
                let out = rustybuzz::shape(&face, features, buf);
                out.glyph_infos()
                    .iter()
                    .zip(out.glyph_positions())
                    .map(|(i, p)| ShapedGlyph {
                        glyph_id: i.glyph_id as u16,
                        cluster: i.cluster,
                        x_advance: p.x_advance,
                        x_offset: p.x_offset,
                        y_offset: p.y_offset,
                    })
                    .collect()
            }
            None => Arc::from(Vec::new()),
        };
        if self.shape_cache.len() > 200_000 {
            self.shape_cache.clear();
        }
        self.shape_cache.insert(key, glyphs.clone());
        glyphs
    }

    /// Advance width of a shaped text in points at `size_pt`.
    pub fn measure(&mut self, id: FaceId, text: &str, size_pt: f32) -> f32 {
        let upem = f32::from(self.info(id).units_per_em);
        let sum: i32 = self.shape(id, text).iter().map(|g| g.x_advance).sum();
        sum as f32 * size_pt / upem
    }
}

fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    pick_family(face.names())
}

/// Typographic family (name id 16), else family (id 1) — OpenType `name` table.
pub(crate) fn pick_family(names: ttf_parser::name::Names<'_>) -> Option<String> {
    let pick = |id: u16| {
        names
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
    };
    pick(ttf_parser::name_id::TYPOGRAPHIC_FAMILY).or_else(|| pick(ttf_parser::name_id::FAMILY))
}
