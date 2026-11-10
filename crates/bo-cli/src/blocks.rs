//! The BlueDoc building block library as a file (`assets/blocks/doc/`): the entries
//! of `bo_engine::blocks::library` in the glossary document part (ECMA-376 Part 1,
//! §17.12) of a `.dotx` template, with an index of the entries for people to read.

use bo_engine::blocks::library;
use bo_engine::variant::{convert, Variant};

type Error = Box<dyn std::error::Error>;

/// File name of the library template.
pub const LIBRARY_FILE: &str = "blueoffice-blocks.dotx";
/// File name of the index (name, gallery, category, behaviour, description).
pub const INDEX_FILE: &str = "blueoffice-blocks.tsv";

/// `(file name, bytes)` of the library and its index.
pub fn files() -> Result<Vec<(String, Vec<u8>)>, Error> {
    let doc = library::library();
    let bytes = bo_docx::write_docx(&doc)?;
    let dotx = convert(&bytes, Variant::Dotx)?.bytes;
    let mut index = String::from("name\tgallery\tcategory\tbehavior\tdescription\n");
    for p in library::parts() {
        let pr = &p.props;
        index.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            pr.name,
            pr.gallery,
            pr.category,
            pr.behavior(),
            pr.description
        ));
    }
    Ok(vec![
        (LIBRARY_FILE.to_owned(), dotx),
        (INDEX_FILE.to_owned(), index.into_bytes()),
    ])
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    fn dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/blocks/doc")
    }

    /// The committed library is exactly what the generator produces (regenerate with
    /// `cargo run -p bo-cli -- blocks assets/blocks/doc`).
    #[test]
    fn committed_library_is_up_to_date() {
        let files = super::files().expect("library");
        assert_eq!(files.len(), 2);
        for (name, bytes) in files {
            let on_disk = std::fs::read(dir().join(&name)).unwrap_or_default();
            assert!(on_disk == bytes, "{name} is stale: regenerate the library");
        }
    }

    /// The committed template reads back with every entry and is a `.dotx`.
    #[test]
    fn committed_library_reads_back() {
        let bytes = std::fs::read(dir().join(super::LIBRARY_FILE)).expect("library");
        assert_eq!(
            bo_engine::variant::detect(&bytes),
            Some(bo_engine::variant::Variant::Dotx)
        );
        let doc = bo_docx::read_docx(&bytes).expect("dotx");
        let g = doc.glossary.as_deref().expect("glossary");
        let parts = bo_engine::blocks::library::parts();
        assert_eq!(g.parts.len(), parts.len());
        for (a, b) in g.parts.iter().zip(&parts) {
            assert_eq!(a.props, b.props);
        }
        // An engine lists them from the file as the user's store.
        let mut e = bo_engine::Engine::with_bundled_fonts();
        e.load(bo_engine::plain_text_document("x"), "n.docx")
            .expect("load");
        let file = base64_of(&bytes);
        let r = e
            .command_json(&format!(r#"{{"cmd":"loadBlockStore","file":"{file}"}}"#))
            .expect("command");
        let v: serde_json::Value = serde_json::from_str(&r).expect("json");
        let user = v["blocks"]["list"]
            .as_array()
            .map_or(0, |l| l.iter().filter(|b| b["store"] == "user").count());
        assert_eq!(user, parts.len());
    }

    fn base64_of(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for c in bytes.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }
}
