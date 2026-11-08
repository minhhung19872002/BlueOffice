//! Fonts installed on the operating system (native targets, prompt §3.4
//! `fonts`: "desktop + font hệ thống").
//!
//! [`SystemFontIndex::scan`] walks the OS font folders and reads only the
//! table directory and the `name` table of each file (OpenType spec, "Font
//! file" §Organization of an OpenType font, §TTC header; `name` table §Naming
//! table) — never whole files — so indexing hundreds of fonts stays cheap.
//! [`crate::FontManager`] loads the files of a family lazily, the first time a
//! document asks for it; the faces get ids after the bundled fonts, so the
//! deterministic order of the bundled fonts (ADR-0003) is unchanged.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Largest font file indexed (bigger files are skipped, they are never loaded either).
const MAX_FILE: u64 = 64 * 1024 * 1024;
/// Largest `name` table read while indexing.
const MAX_NAME_TABLE: u32 = 1024 * 1024;
/// Folder depth walked below each font folder (Linux keeps fonts in sub-folders).
const MAX_DEPTH: usize = 5;

/// One installed font file and the family names of its faces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemFontFile {
    pub path: PathBuf,
    /// Family of every face in the file (a collection has several).
    pub families: Vec<String>,
}

/// Installed fonts by family (case-insensitive). Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct SystemFontIndex {
    /// Lower-case family → (display name, files holding it).
    by_family: Arc<BTreeMap<String, (String, Vec<PathBuf>)>>,
}

impl SystemFontIndex {
    /// Index built from already probed files (tests, custom folders).
    pub fn from_files(files: impl IntoIterator<Item = SystemFontFile>) -> Self {
        let mut map: BTreeMap<String, (String, Vec<PathBuf>)> = BTreeMap::new();
        for f in files {
            for fam in &f.families {
                let e = map
                    .entry(fam.to_lowercase())
                    .or_insert_with(|| (fam.clone(), Vec::new()));
                if !e.1.contains(&f.path) {
                    e.1.push(f.path.clone());
                }
            }
        }
        Self {
            by_family: Arc::new(map),
        }
    }

    /// Scans font folders (see [`SystemFontIndex::default_dirs`]); unreadable files are skipped.
    pub fn scan(dirs: &[PathBuf]) -> Self {
        Self::scan_cached(dirs, None)
    }

    /// Like [`SystemFontIndex::scan`], reusing the families of files whose size and
    /// modification time are unchanged since the previous scan (`cache` file, rewritten).
    /// Opening hundreds of font files is slow on a cold start (virus scanners); a
    /// `stat` per file is not.
    pub fn scan_cached(dirs: &[PathBuf], cache: Option<&Path>) -> Self {
        let mut files = Vec::new();
        for d in dirs {
            walk(d, 0, &mut files);
        }
        // Stable order whatever the file system returns.
        files.sort();
        files.dedup();
        let old = cache.map(read_cache).unwrap_or_default();
        let mut lines = String::new();
        let mut found = Vec::new();
        for p in files {
            let Some(stamp) = stamp_of(&p) else { continue };
            let families = match old.get(&p) {
                Some((s, fams)) if *s == stamp => Some(fams.clone()),
                _ => probe_file(&p).map(|f| f.families),
            };
            let Some(families) = families else { continue };
            if let Some(path) = p.to_str().filter(|s| !s.contains(['\t', '\n', '\r'])) {
                lines.push_str(&format!("{stamp}\t{path}\t{}\n", families.join("\u{1f}")));
            }
            found.push(SystemFontFile { path: p, families });
        }
        if let Some(c) = cache {
            if let Some(dir) = c.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(c, format!("{CACHE_HEADER}\n{lines}"));
        }
        Self::from_files(found)
    }

    /// Font folders of the current OS (user folders included).
    pub fn default_dirs() -> Vec<PathBuf> {
        let mut v = Vec::new();
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        if cfg!(windows) {
            if let Some(w) = env("WINDIR").or_else(|| env("SystemRoot")) {
                v.push(w.join("Fonts"));
            }
            if let Some(l) = env("LOCALAPPDATA") {
                v.push(l.join("Microsoft").join("Windows").join("Fonts"));
            }
        } else if cfg!(target_os = "macos") {
            v.push(PathBuf::from("/System/Library/Fonts"));
            v.push(PathBuf::from("/Library/Fonts"));
            if let Some(h) = env("HOME") {
                v.push(h.join("Library").join("Fonts"));
            }
        } else {
            v.push(PathBuf::from("/usr/share/fonts"));
            v.push(PathBuf::from("/usr/local/share/fonts"));
            if let Some(d) = env("XDG_DATA_HOME") {
                v.push(d.join("fonts"));
            }
            if let Some(h) = env("HOME") {
                v.push(h.join(".local").join("share").join("fonts"));
                v.push(h.join(".fonts"));
            }
        }
        v
    }

    /// Number of families.
    pub fn len(&self) -> usize {
        self.by_family.len()
    }

    /// No fonts found.
    pub fn is_empty(&self) -> bool {
        self.by_family.is_empty()
    }

    /// Family display names, sorted case-insensitively.
    pub fn families(&self) -> Vec<String> {
        self.by_family.values().map(|(n, _)| n.clone()).collect()
    }

    /// Is `family` installed (case-insensitive)?
    pub fn contains(&self, family: &str) -> bool {
        self.by_family.contains_key(&family.to_lowercase())
    }

    /// Files holding `family` (case-insensitive).
    pub fn files_of(&self, family: &str) -> &[PathBuf] {
        self.by_family
            .get(&family.to_lowercase())
            .map_or(&[], |(_, f)| f.as_slice())
    }
}

/// First line of the index cache (format version).
const CACHE_HEADER: &str = "bo-system-fonts 1";

/// `size:mtime` of a file (seconds since the epoch).
fn stamp_of(p: &Path) -> Option<String> {
    let m = std::fs::metadata(p).ok()?;
    let t = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(format!("{}:{}", m.len(), t.as_secs()))
}

/// Cached families by path: `stamp \t path \t family\x1ffamily…` per line.
fn read_cache(path: &Path) -> std::collections::HashMap<PathBuf, (String, Vec<String>)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return std::collections::HashMap::new();
    };
    let mut lines = text.lines();
    if lines.next() != Some(CACHE_HEADER) {
        return std::collections::HashMap::new();
    }
    lines
        .filter_map(|l| {
            let mut parts = l.splitn(3, '\t');
            let stamp = parts.next()?.to_owned();
            let path = PathBuf::from(parts.next()?);
            let fams: Vec<String> = parts
                .next()?
                .split('\u{1f}')
                .filter(|f| !f.is_empty())
                .map(str::to_owned)
                .collect();
            (!fams.is_empty()).then_some((path, (stamp, fams)))
        })
        .collect()
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk(&p, depth + 1, out);
        } else if is_font_file(&p) {
            out.push(p);
        }
    }
}

fn is_font_file(p: &Path) -> bool {
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc")
}

/// Reads byte ranges of a font file.
struct Ranges {
    file: File,
    len: u64,
}

impl Ranges {
    fn read(&mut self, offset: u64, len: u32) -> Option<Vec<u8>> {
        if offset.checked_add(u64::from(len))? > self.len {
            return None;
        }
        self.file.seek(SeekFrom::Start(offset)).ok()?;
        let mut buf = vec![0; len as usize];
        self.file.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

/// Families of the faces in a font file, read from its `name` tables only.
pub fn probe_file(path: &Path) -> Option<SystemFontFile> {
    let file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if !(12..=MAX_FILE).contains(&len) {
        return None;
    }
    let mut r = Ranges { file, len };
    let head = r.read(0, 12)?;
    // TTC header (OpenType "Font Collections"): 'ttcf', version, numFonts, offsets.
    let offsets: Vec<u64> = if &head[0..4] == b"ttcf" {
        let n = be32(&head, 8)?.min(256);
        let table = r.read(12, n * 4)?;
        (0..n as usize)
            .filter_map(|i| be32(&table, i * 4).map(u64::from))
            .collect()
    } else {
        vec![0]
    };
    let mut families = Vec::new();
    for off in offsets {
        if let Some(f) = probe_face(&mut r, off) {
            if !families.contains(&f) {
                families.push(f);
            }
        }
    }
    (!families.is_empty()).then(|| SystemFontFile {
        path: path.to_path_buf(),
        families,
    })
}

/// Family of the face whose table directory starts at `off`.
fn probe_face(r: &mut Ranges, off: u64) -> Option<String> {
    // Table directory: sfntVersion u32, numTables u16, searchRange…, then 16-byte records
    // (tag, checksum, offset, length).
    let dir = r.read(off, 12)?;
    let version = be32(&dir, 0)?;
    // 0x00010000 (TrueType), 'OTTO' (CFF), 'true' (Apple TrueType).
    if !matches!(version, 0x0001_0000 | 0x4F54_544F | 0x7472_7565) {
        return None;
    }
    let count = u32::from(be16(&dir, 4)?.min(512));
    let records = r.read(off + 12, count * 16)?;
    let (mut name_off, mut name_len) = (None, 0);
    for i in 0..count as usize {
        let rec = records.get(i * 16..i * 16 + 16)?;
        if &rec[0..4] == b"name" {
            name_off = Some(u64::from(be32(rec, 8)?));
            name_len = be32(rec, 12)?;
        }
    }
    let name_len = name_len.min(MAX_NAME_TABLE);
    let table = r.read(name_off?, name_len)?;
    let names = ttf_parser::name::Table::parse(&table)?.names;
    crate::manager::pick_family(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts")
    }

    #[test]
    fn probes_family_names_without_loading_fonts() {
        let f = probe_file(&fonts_dir().join("Carlito-Bold.ttf")).expect("probe");
        assert_eq!(f.families, ["Carlito"]);
        assert!(probe_file(&fonts_dir().join("manifest.json")).is_none());
        assert!(probe_file(&fonts_dir().join("missing.ttf")).is_none());
    }

    #[test]
    fn scans_a_folder_into_families() {
        let idx = SystemFontIndex::scan(&[fonts_dir()]);
        assert!(idx.contains("carlito") && idx.contains("Liberation Sans"));
        assert_eq!(idx.files_of("CARLITO").len(), 4);
        let fams = idx.families();
        let mut sorted = fams.clone();
        sorted.sort_by_key(|s| s.to_lowercase());
        assert_eq!(fams, sorted);
        assert_eq!(idx.files_of("No Such Font").len(), 0);
    }

    #[test]
    fn rejects_truncated_files() {
        let dir = std::env::temp_dir().join(format!("bo-text-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let src = std::fs::read(fonts_dir().join("Tinos-Regular.ttf")).expect("font");
        let cut = dir.join("cut.ttf");
        std::fs::write(&cut, &src[..200]).expect("write");
        assert!(probe_file(&cut).is_none());
        let bogus = dir.join("ttc.ttc");
        let mut b = b"ttcf\x00\x01\x00\x00\xff\xff\xff\xff".to_vec();
        b.extend_from_slice(&[0; 8]);
        std::fs::write(&bogus, b).expect("write");
        assert!(probe_file(&bogus).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_reuses_unchanged_files() {
        let dir = std::env::temp_dir().join(format!("bo-text-cache-{}", std::process::id()));
        let fonts = dir.join("fonts");
        std::fs::create_dir_all(&fonts).expect("dir");
        std::fs::copy(fonts_dir().join("Caladea-Regular.ttf"), fonts.join("a.ttf")).expect("copy");
        let cache = dir.join("index.txt");
        let a = SystemFontIndex::scan_cached(std::slice::from_ref(&fonts), Some(&cache));
        assert!(a.contains("Caladea"));
        // A cached entry is trusted while size and time match (here: a renamed family).
        let text = std::fs::read_to_string(&cache).expect("cache");
        assert!(text.starts_with(CACHE_HEADER));
        std::fs::write(&cache, text.replace("Caladea", "Cached Family")).expect("write");
        let b = SystemFontIndex::scan_cached(std::slice::from_ref(&fonts), Some(&cache));
        assert!(b.contains("cached family") && !b.contains("Caladea"));
        // A corrupt cache is ignored.
        std::fs::write(&cache, "garbage").expect("write");
        let c = SystemFontIndex::scan_cached(std::slice::from_ref(&fonts), Some(&cache));
        assert!(c.contains("Caladea"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Timing on this machine's real font folders: `cargo test -p bo-text -- --ignored --nocapture`.
    #[test]
    #[ignore = "depends on the installed fonts"]
    fn scan_installed_fonts() {
        let t = std::time::Instant::now();
        let idx = SystemFontIndex::scan(&SystemFontIndex::default_dirs());
        println!("{} families in {:?}", idx.len(), t.elapsed());
    }
}
