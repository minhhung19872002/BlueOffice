//! Presentation package: a zip (PKWARE APPNOTE) holding the saved `.pptx`, its
//! audio / video clips (ECMA-376 Part 1, §15.2.2 audio, §15.2.17 video
//! relationships and the [MS-PPTX] media relationship) as separate files, and
//! the fonts the slides use whose licence allows redistribution — only the
//! bundled SIL Open Font License 1.1 families, each with its licence text
//! (OFL condition 2). Clips linked outside the package (`TargetMode="External"`,
//! ECMA-376 Part 2, §9.3) are copied too when the host names the deck's folder
//! (`linkBase`, native builds only) and the link is a local file — a relative
//! path keeps its place beside the deck so the link still resolves, an
//! absolute one goes to `media/`; network locations (URLs, UNC shares,
//! `file://host/…`) are never read. Links not copied are listed in `links.txt`.

use crate::{Engine, EngineError, Result};
use bo_render::Command;
use bo_text::FaceId;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;

/// Bundled OFL families (lower case) and their licence file.
const OFL_FONTS: &[(&str, &str, &str)] = &[
    (
        "liberation sans",
        "OFL-Liberation.txt",
        include_str!("../../../../assets/fonts/OFL-Liberation.txt"),
    ),
    (
        "carlito",
        "OFL-Carlito.txt",
        include_str!("../../../../assets/fonts/OFL-Carlito.txt"),
    ),
    (
        "caladea",
        "OFL-Caladea.txt",
        include_str!("../../../../assets/fonts/OFL-Caladea.txt"),
    ),
    (
        "stix two math",
        "OFL-STIXTwo.txt",
        include_str!("../../../../assets/fonts/OFL-STIXTwo.txt"),
    ),
];

/// Write `files` (name, bytes) as a deflated zip.
pub fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .system(zip::System::Dos);
    let err = |e: &dyn std::fmt::Display| EngineError::Format(format!("zip: {e}"));
    for (name, data) in files {
        w.start_file(name.as_str(), opts).map_err(|e| err(&e))?;
        w.write_all(data).map_err(|e| err(&e))?;
    }
    Ok(w.finish().map_err(|e| err(&e))?.into_inner())
}

/// File name of a part (`/ppt/media/media1.mp4` → `media1.mp4`).
fn file_name(part: &str) -> &str {
    part.rsplit('/').next().unwrap_or(part)
}

fn is_media_rel(rel_type: &str) -> bool {
    rel_type.ends_with("/video") || rel_type.ends_with("/audio") || rel_type.ends_with("/media")
}

/// Font file name of a face: family without spaces, then the style.
fn font_file(family: &str, bold: bool, italic: bool) -> String {
    let style = match (bold, italic) {
        (false, false) => "Regular",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (true, true) => "BoldItalic",
    };
    let fam: String = family.chars().filter(|c| !c.is_whitespace()).collect();
    format!("{fam}-{style}.ttf")
}

/// Largest linked clip copied into a package (bytes).
const MAX_LINKED: u64 = 1 << 30;

/// Where a linked clip's target points on this computer: a relative path, an
/// absolute path or a `file:` URI without host (RFC 8089). `None` for anything
/// on the network (other URI schemes, UNC shares, `file://host/…`).
pub fn local_link(target: &str) -> Option<LocalLink> {
    let t = target.trim();
    if t.is_empty() || t.starts_with("\\\\") || t.starts_with("//") {
        return None;
    }
    if let Some(rest) = strip_prefix_ci(t, "file:") {
        // file:///C:/a/b.mp4, file:/C:/a.mp4, file:///home/a.mp4; a host is the network.
        let rest = match rest.strip_prefix("//") {
            Some(r) => {
                let (host, tail) = r.split_at(r.find('/').unwrap_or(r.len()));
                if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                    return None;
                }
                tail
            }
            None => rest,
        };
        let decoded = percent_decode(rest)?;
        if decoded.starts_with("\\\\") || decoded.starts_with("//") {
            return None;
        }
        // `/C:/…` → `C:/…`.
        let b = decoded.as_bytes();
        let path = if b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
            decoded[1..].to_owned()
        } else {
            decoded
        };
        return Some(LocalLink {
            path,
            relative: false,
        });
    }
    // Another scheme (http:, https:, ftp:, smb:…); a one-letter one is a drive.
    if let Some(i) = t.find(':') {
        let scheme = &t[..i];
        if scheme.len() > 1
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        {
            return None;
        }
    }
    let path = percent_decode(t).unwrap_or_else(|| t.to_owned());
    let b = path.as_bytes();
    let absolute = path.starts_with('/')
        || path.starts_with('\\')
        || (b.len() > 1 && b[0].is_ascii_alphabetic() && b[1] == b':');
    Some(LocalLink {
        path,
        relative: !absolute,
    })
}

/// A linked clip on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalLink {
    /// File path as written (either separator).
    pub path: String,
    /// Relative to the deck's folder.
    pub relative: bool,
}

fn strip_prefix_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    s.get(..p.len())
        .filter(|h| h.eq_ignore_ascii_case(p))
        .map(|_| &s[p.len()..])
}

/// RFC 3986 §2.1 percent-decoding (`None` on invalid UTF-8).
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// Name of a linked clip inside the package: a relative path without `..`
/// keeps its place beside the deck, anything else goes to `media/<file name>`.
fn link_entry(link: &LocalLink) -> String {
    let norm = link.path.replace('\\', "/");
    let parts: Vec<&str> = norm
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    if link.relative && !parts.is_empty() && !parts.contains(&"..") {
        return parts.join("/");
    }
    format!("media/{}", parts.last().copied().unwrap_or("clip"))
}

/// Read a local linked clip relative to `base` (native builds only).
#[cfg(not(target_arch = "wasm32"))]
fn read_link(link: &LocalLink, base: &str) -> Option<Vec<u8>> {
    let p = std::path::Path::new(&link.path);
    let full = if link.relative {
        std::path::Path::new(base).join(p)
    } else {
        p.to_path_buf()
    };
    let meta = std::fs::metadata(&full).ok()?;
    if !meta.is_file() || meta.len() > MAX_LINKED {
        return None;
    }
    std::fs::read(&full).ok()
}

/// No file system in the browser: linked clips stay links.
#[cfg(target_arch = "wasm32")]
fn read_link(_link: &LocalLink, _base: &str) -> Option<Vec<u8>> {
    let _ = MAX_LINKED;
    None
}

/// A free name for `name` among `taken` (lower case): `dir/2-file`, `dir/3-file`…
fn free_name(name: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(&name.to_ascii_lowercase()) {
        return name.to_owned();
    }
    let (dir, file) = name.rsplit_once('/').map_or(("", name), |(d, f)| (d, f));
    (2..)
        .map(|k| {
            if dir.is_empty() {
                format!("{k}-{file}")
            } else {
                format!("{dir}/{k}-{file}")
            }
        })
        .find(|n| !taken.contains(&n.to_ascii_lowercase()))
        .unwrap_or_default()
}

/// Build the package of the open presentation; `link_base` is the deck's
/// folder, for linked clips.
pub fn build(engine: &Engine, link_base: Option<&str>) -> Result<Vec<u8>> {
    let pres = engine.deck()?;
    let base = engine
        .name
        .rsplit_once('.')
        .map_or(engine.name.as_str(), |(b, _)| b);
    let base = if base.is_empty() {
        "presentation"
    } else {
        base
    };
    let mut files = vec![(format!("{base}.pptx"), engine.save_pptx()?)];

    // Audio / video clips.
    let mut seen = BTreeSet::new();
    let mut links = BTreeSet::new();
    for slide in &pres.slides {
        for rel in slide.info.rels.iter().filter(|r| is_media_rel(&r.rel_type)) {
            if rel.external {
                links.insert(rel.target.clone());
                continue;
            }
            let part = bo_opc::resolve_target(&slide.info.part_name, &rel.target);
            if !seen.insert(part.clone()) {
                continue;
            }
            if let Some(data) = pres.part_data(&part) {
                files.push((format!("media/{}", file_name(&part)), data.to_vec()));
            }
        }
    }
    // Linked clips that are local files.
    let mut left = Vec::new();
    let mut taken: BTreeSet<String> = files.iter().map(|(n, _)| n.to_ascii_lowercase()).collect();
    for target in links {
        let link = local_link(&target);
        let data = link
            .as_ref()
            .zip(link_base)
            .and_then(|(l, base)| read_link(l, base));
        match (link, data) {
            (Some(link), Some(data)) => {
                let want = link_entry(&link);
                let name = free_name(&want, &taken);
                taken.insert(name.to_ascii_lowercase());
                // A relative link copied to its own place still resolves.
                if !(link.relative && name == want) {
                    left.push(format!("{target} -> {name}"));
                }
                files.push((name, data));
            }
            _ => left.push(target),
        }
    }
    if !left.is_empty() {
        let list: String = left.iter().flat_map(|l| [l.as_str(), "\r\n"]).collect();
        files.push(("links.txt".into(), list.into_bytes()));
    }

    // Faces drawn on the slides that are bundled OFL fonts.
    let mut faces = BTreeSet::new();
    for i in 0..engine.slide_count() {
        for c in &engine.page(i)?.commands {
            if let Command::Glyphs { face, .. } = c {
                faces.insert(*face);
            }
        }
    }
    let mut licences = BTreeMap::new();
    let mut fonts_seen = BTreeSet::new();
    for face in faces {
        if face as usize >= engine.fonts.len() {
            continue;
        }
        let info = engine.fonts.info(FaceId(face));
        let family = info.family.to_lowercase();
        let Some((_, lic_name, lic)) = OFL_FONTS.iter().find(|(f, _, _)| *f == family) else {
            continue;
        };
        let name = font_file(&info.family, info.bold, info.italic);
        if !fonts_seen.insert(name.clone()) {
            continue;
        }
        let (data, _) = engine.fonts.data(FaceId(face));
        files.push((format!("fonts/{name}"), data.to_vec()));
        licences.insert(*lic_name, *lic);
    }
    for (name, text) in licences {
        files.push((format!("fonts/{name}"), text.as_bytes().to_vec()));
    }
    zip(&files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(
            font_file("Liberation Sans", true, false),
            "LiberationSans-Bold.ttf"
        );
        assert_eq!(font_file("Carlito", false, false), "Carlito-Regular.ttf");
        assert_eq!(file_name("/ppt/media/media1.mp4"), "media1.mp4");
        assert!(is_media_rel(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video"
        ));
        assert!(!is_media_rel(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
        ));
    }

    #[test]
    fn local_links_only() {
        let l = |t: &str| local_link(t);
        assert_eq!(
            l("clips/intro%20a.mp4"),
            Some(LocalLink {
                path: "clips/intro a.mp4".into(),
                relative: true
            })
        );
        assert_eq!(
            l("file:///C:/Videos/a.mp4"),
            Some(LocalLink {
                path: "C:/Videos/a.mp4".into(),
                relative: false
            })
        );
        assert_eq!(l("C:\\Videos\\a.mp4").map(|x| x.relative), Some(false));
        assert_eq!(
            l("file://localhost/home/a.mp4").map(|x| x.path),
            Some("/home/a.mp4".into())
        );
        for net in [
            "https://example.com/a.mp4",
            "http://x/a.mp4",
            "ftp://x/a.mp4",
            "\\\\server\\share\\a.mp4",
            "//server/share/a.mp4",
            "file://server/share/a.mp4",
            "file:////server/share/a.mp4",
            "smb://server/a.mp4",
            "",
        ] {
            assert_eq!(l(net), None, "{net}");
        }
        let entry = |t: &str| link_entry(&local_link(t).unwrap());
        assert_eq!(entry("clips/a.mp4"), "clips/a.mp4");
        assert_eq!(entry("./a.mp4"), "a.mp4");
        assert_eq!(entry("../other/a.mp4"), "media/a.mp4");
        assert_eq!(entry("C:\\Videos\\b.wav"), "media/b.wav");
        let taken: BTreeSet<String> = ["media/a.mp4".to_owned()].into();
        assert_eq!(free_name("media/A.mp4", &taken), "media/2-A.mp4");
        assert_eq!(free_name("b.mp4", &taken), "b.mp4");
    }
}
