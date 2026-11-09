//! Built-in BlueDoc templates (`assets/templates/doc/`): the administrative documents of
//! Nghị định 30/2020/NĐ-CP built by `bo_engine::templates`, written as `.docx`, with a
//! first-page thumbnail drawn by the BlueOffice renderer for the template gallery.

use bo_engine::templates::{build, TEMPLATES};
use bo_engine::Engine;

/// Pixels per point of the thumbnails (A4 → 179 × 253 px).
pub const THUMBNAIL_SCALE: f32 = 0.3;

type Error = Box<dyn std::error::Error>;

/// `(file name, bytes)` of every template and its thumbnail (`<id>.docx`, `<id>.png`).
pub fn files() -> Result<Vec<(String, Vec<u8>)>, Error> {
    let mut engine = Engine::with_bundled_fonts();
    let mut out = Vec::new();
    for id in TEMPLATES {
        let doc = build(id).ok_or_else(|| format!("unknown template {id}"))?;
        out.push((format!("{id}.docx"), bo_docx::write_docx(&doc)?));
        engine.load(doc, &format!("{id}.docx"))?;
        out.push((format!("{id}.png"), engine.render_png(0, THUMBNAIL_SCALE)?));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/templates/doc")
    }

    fn pixels(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
        let mut reader = decoder.read_info().expect("png");
        let mut buf = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut buf).expect("frame");
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    /// The committed templates are exactly what the generator produces; thumbnails
    /// match within rasteriser rounding (regenerate with
    /// `cargo run -p bo-cli -- templates assets/templates/doc`).
    #[test]
    fn committed_templates_are_up_to_date() {
        let files = super::files().expect("templates");
        assert_eq!(files.len(), 2 * bo_engine::templates::TEMPLATES.len());
        for (name, bytes) in files {
            let on_disk = std::fs::read(dir().join(&name)).unwrap_or_default();
            if name.ends_with(".png") {
                let (w, h, a) = pixels(&bytes);
                let (w2, h2, b) = pixels(&on_disk);
                assert_eq!(
                    (w, h),
                    (w2, h2),
                    "{name} is stale: regenerate the templates"
                );
                let diff: u64 = a
                    .iter()
                    .zip(&b)
                    .map(|(x, y)| u64::from(x.abs_diff(*y)))
                    .sum();
                assert!(
                    diff <= a.len() as u64 / 50,
                    "{name} is stale: regenerate the templates"
                );
            } else {
                assert!(
                    on_disk == bytes,
                    "{name} is stale: regenerate the templates"
                );
            }
        }
    }

    /// The gallery of the editor offers exactly the engine's templates, in order.
    #[test]
    fn editor_gallery_lists_the_engine_templates() {
        let src = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/editor/src/TemplateDialogs.tsx"),
        )
        .expect("TemplateDialogs.tsx");
        let list = src
            .split("export const TEMPLATES = [")
            .nth(1)
            .and_then(|s| s.split(']').next())
            .expect("TEMPLATES");
        let ids: Vec<&str> = list
            .split(',')
            .map(|s| s.trim().trim_matches('\''))
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(ids, bo_engine::templates::TEMPLATES);
    }

    /// Every committed template passes the decree format check after a round trip
    /// through the `.docx` reader.
    #[test]
    fn committed_templates_pass_the_format_check() {
        for id in bo_engine::templates::TEMPLATES {
            let bytes = std::fs::read(dir().join(format!("{id}.docx"))).expect("template");
            let doc = bo_docx::read_docx(&bytes).expect("docx");
            let r = bo_engine::templates::check::check(&doc);
            assert!(r.issues.is_empty(), "{id}: {:#?}", r.issues);
        }
    }
}
