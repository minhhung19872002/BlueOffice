//! Presentation package: a zip (PKWARE APPNOTE) holding the saved `.pptx`, its
//! audio / video clips (ECMA-376 Part 1, §15.2.2 audio, §15.2.17 video
//! relationships and the [MS-PPTX] media relationship) as separate files, and
//! the fonts the slides use whose licence allows redistribution — only the
//! bundled SIL Open Font License 1.1 families, each with its licence text
//! (OFL condition 2). Clips linked outside the package (`TargetMode="External"`,
//! ECMA-376 Part 2, §9.3) cannot be read by the engine; their targets are
//! listed in `links.txt`.

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

/// Build the package of the open presentation.
pub fn build(engine: &Engine) -> Result<Vec<u8>> {
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
    if !links.is_empty() {
        let list: String = links.iter().flat_map(|l| [l.as_str(), "\r\n"]).collect();
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
}
