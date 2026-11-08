//! ODF package container (ODF 1.3 Part 2: Packages).

use crate::{dom, OdfError, Result};
use std::io::{Cursor, Read, Write};

/// Maximum number of ZIP entries.
const MAX_ENTRIES: usize = 20_000;
/// Maximum uncompressed size of one entry (256 MiB).
const MAX_ENTRY: u64 = 256 * 1024 * 1024;
/// Maximum total uncompressed size (1 GiB).
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
/// Maximum compression ratio for entries larger than 1 MiB (zip-bomb guard).
const MAX_RATIO: u64 = 200;

/// The files of an ODF package (names without leading `/`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OdfPackage {
    /// Contents of the `mimetype` entry, if present.
    pub mimetype: Option<String>,
    /// All entries in archive order (including `mimetype`).
    pub files: Vec<(String, Vec<u8>)>,
}

impl OdfPackage {
    /// Read a package from ZIP bytes with zip-bomb limits.
    pub fn read(bytes: &[u8]) -> Result<OdfPackage> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        if zip.len() > MAX_ENTRIES {
            return Err(OdfError::LimitExceeded(format!("{} entries", zip.len())));
        }
        let mut total = 0u64;
        let mut files = Vec::new();
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let name = f.name()?.trim_start_matches('/').to_owned();
            let declared = f.size();
            if declared > MAX_ENTRY {
                return Err(OdfError::LimitExceeded(format!(
                    "{name} is {declared} bytes"
                )));
            }
            if declared > 1024 * 1024 && declared / f.compressed_size().max(1) > MAX_RATIO {
                return Err(OdfError::LimitExceeded(format!("{name} compression ratio")));
            }
            let mut data = Vec::with_capacity(declared.min(16 * 1024 * 1024) as usize);
            (&mut f).take(MAX_ENTRY + 1).read_to_end(&mut data)?;
            if data.len() as u64 > MAX_ENTRY {
                return Err(OdfError::LimitExceeded(format!(
                    "{name} exceeds size limit"
                )));
            }
            total += data.len() as u64;
            if total > MAX_TOTAL {
                return Err(OdfError::LimitExceeded("total uncompressed size".into()));
            }
            files.push((name, data));
        }
        let mimetype = files
            .iter()
            .find(|(n, _)| n == "mimetype")
            .map(|(_, d)| String::from_utf8_lossy(d).trim().to_owned());
        Ok(OdfPackage { mimetype, files })
    }

    /// Entry bytes.
    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d.as_slice())
    }

    /// Entry bytes or [`OdfError::Missing`].
    pub fn require(&self, name: &str) -> Result<&[u8]> {
        self.file(name)
            .ok_or_else(|| OdfError::Missing(name.to_owned()))
    }

    /// Check the `mimetype` (when present) against `expected`.
    pub fn expect_mimetype(&self, expected: &str) -> Result<()> {
        match &self.mimetype {
            Some(m) if m != expected && !m.starts_with(&format!("{expected}-")) => {
                Err(OdfError::WrongType(m.clone()))
            }
            _ => Ok(()),
        }
    }

    /// Serialize. ODF 1.3 Part 2 §3.3: `mimetype` is the first entry, stored
    /// uncompressed and without extra field; the manifest lists every file.
    pub fn write(mimetype: &str, files: &[(&str, &str, Vec<u8>)]) -> Result<Vec<u8>> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        // "Version made by" = MS-DOS (APPNOTE 4.4.2) on every platform:
        // identical bytes on Windows and Linux.
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .system(zip::System::Dos);
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .system(zip::System::Dos);
        w.start_file("mimetype", stored)?;
        w.write_all(mimetype.as_bytes())?;
        for (name, _, data) in files {
            w.start_file(*name, deflated)?;
            w.write_all(data)?;
        }
        // ODF 1.3 Part 2 §4: manifest.
        let mut m = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<manifest:manifest \
             xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" \
             manifest:version=\"1.3\">",
        );
        m.push_str(&format!(
            "<manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\" manifest:media-type=\"{}\"/>",
            dom::escape(mimetype)
        ));
        for (name, media, _) in files {
            m.push_str(&format!(
                "<manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{}\"/>",
                dom::escape(name),
                dom::escape(media)
            ));
        }
        m.push_str("</manifest:manifest>");
        w.start_file("META-INF/manifest.xml", deflated)?;
        w.write_all(m.as_bytes())?;
        Ok(w.finish()?.into_inner())
    }
}

/// Return the `mimetype` of an ODF package, if `bytes` is one.
pub fn detect_mimetype(bytes: &[u8]) -> Option<String> {
    OdfPackage::read(bytes).ok()?.mimetype
}

/// `meta.xml` with the generator name (ODF 1.3 Part 3, §3.1.3).
pub fn meta_xml() -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<office:document-meta \
         xmlns:office=\"{}\" xmlns:meta=\"{}\" office:version=\"1.3\"><office:meta>\
         <meta:generator>BlueOffice/{}</meta:generator></office:meta></office:document-meta>",
        crate::ns::OFFICE,
        crate::ns::META,
        env!("CARGO_PKG_VERSION")
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mimetype_first_and_stored() {
        let bytes = OdfPackage::write(
            crate::MIME_ODT,
            &[("content.xml", "text/xml", b"<a/>".to_vec())],
        )
        .unwrap();
        // Local file header: signature, then at offset 8 the compression method (0 = stored),
        // file name at offset 30 and the data right after it.
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 0);
        assert_eq!(&bytes[30..38], b"mimetype");
        assert_eq!(
            u16::from_le_bytes([bytes[28], bytes[29]]),
            0,
            "no extra field"
        );
        assert_eq!(
            &bytes[38..38 + crate::MIME_ODT.len()],
            crate::MIME_ODT.as_bytes()
        );
        let p = OdfPackage::read(&bytes).unwrap();
        assert_eq!(p.mimetype.as_deref(), Some(crate::MIME_ODT));
        let manifest =
            String::from_utf8(p.require("META-INF/manifest.xml").unwrap().to_vec()).unwrap();
        assert!(manifest.contains("full-path=\"content.xml\""));
    }
}
