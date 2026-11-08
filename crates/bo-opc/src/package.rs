//! Package container — ECMA-376 Part 2, §8 (Package model) and Annex C (ZIP mapping).

use crate::content_types::ContentTypes;
use crate::rels::{self, rels_part_name, resolve_target, Relationship, TargetMode};
use crate::{OpcError, Result};
use std::io::{Cursor, Read, Write};

/// Zip-bomb and resource limits applied when reading untrusted packages.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum number of ZIP entries.
    pub max_entries: usize,
    /// Maximum uncompressed size of a single part.
    pub max_part_size: u64,
    /// Maximum total uncompressed size.
    pub max_total_size: u64,
    /// Maximum compression ratio for parts larger than 1 MiB.
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_entries: 20_000,
            max_part_size: 512 * 1024 * 1024,
            max_total_size: 2 * 1024 * 1024 * 1024,
            max_ratio: 200,
        }
    }
}

/// A part: name (absolute, e.g. `/word/document.xml`) and raw bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// Absolute part name.
    pub name: String,
    /// Raw part content.
    pub data: Vec<u8>,
}

/// An in-memory OPC package. `[Content_Types].xml` is stored as a regular entry
/// (name `/[Content_Types].xml`) so that untouched packages round-trip byte-for-byte
/// at the part level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Package {
    parts: Vec<Part>,
}

/// Name of the content types stream.
pub const CONTENT_TYPES: &str = "/[Content_Types].xml";

impl Package {
    /// Create an empty package with an empty content-type stream.
    pub fn new() -> Self {
        let mut p = Package { parts: Vec::new() };
        p.set_content_types(&ContentTypes {
            defaults: vec![
                (
                    "rels".into(),
                    "application/vnd.openxmlformats-package.relationships+xml".into(),
                ),
                ("xml".into(), "application/xml".into()),
            ],
            overrides: vec![],
        });
        p
    }

    /// Declared uncompressed size of the XML parts of a ZIP package, from the
    /// central directory only (nothing is inflated). Hosts use it to size their
    /// heap before opening big files; `None` when the bytes are not a ZIP file.
    pub fn xml_size(bytes: &[u8]) -> Option<u64> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        let mut total = 0u64;
        for i in 0..zip.len() {
            let Ok(f) = zip.by_index_raw(i) else {
                continue;
            };
            let xml = f.name().is_ok_and(|n| {
                let n = n.to_ascii_lowercase();
                n.ends_with(".xml") || n.ends_with(".rels")
            });
            if xml {
                total = total.saturating_add(f.size());
            }
        }
        Some(total)
    }

    /// Read a package from ZIP bytes.
    pub fn read(bytes: &[u8], limits: &Limits) -> Result<Package> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        if zip.len() > limits.max_entries {
            return Err(OpcError::LimitExceeded(format!("{} entries", zip.len())));
        }
        let mut total: u64 = 0;
        let mut parts = Vec::with_capacity(zip.len());
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let name = format!("/{}", f.name()?.trim_start_matches('/'));
            if name.contains('\\') || name.split('/').any(|s| s == "..") {
                return Err(OpcError::InvalidPartName(name));
            }
            let declared = f.size();
            let compressed = f.compressed_size().max(1);
            if declared > limits.max_part_size {
                return Err(OpcError::LimitExceeded(format!(
                    "part {name} is {declared} bytes"
                )));
            }
            if declared > 1024 * 1024 && declared / compressed > limits.max_ratio {
                return Err(OpcError::LimitExceeded(format!(
                    "part {name} compression ratio {}",
                    declared / compressed
                )));
            }
            // Do not trust the declared size: cap the actual read as well.
            let mut data = Vec::with_capacity(declared.min(64 * 1024 * 1024) as usize);
            (&mut f)
                .take(limits.max_part_size + 1)
                .read_to_end(&mut data)?;
            if data.len() as u64 > limits.max_part_size {
                return Err(OpcError::LimitExceeded(format!(
                    "part {name} exceeds size limit"
                )));
            }
            total += data.len() as u64;
            if total > limits.max_total_size {
                return Err(OpcError::LimitExceeded("total uncompressed size".into()));
            }
            parts.push(Part { name, data });
        }
        let pkg = Package { parts };
        if pkg.part(CONTENT_TYPES).is_none() {
            return Err(OpcError::MissingPart(CONTENT_TYPES.into()));
        }
        Ok(pkg)
    }

    /// Serialize to ZIP bytes. Parts are written in their current order.
    pub fn write(&self) -> Result<Vec<u8>> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        // §C.3: the content types stream should come first for streaming consumers.
        let ordered = self
            .parts
            .iter()
            .filter(|p| p.name == CONTENT_TYPES)
            .chain(self.parts.iter().filter(|p| p.name != CONTENT_TYPES));
        for p in ordered {
            w.start_file(p.name.trim_start_matches('/'), opts)?;
            w.write_all(&p.data)?;
        }
        Ok(w.finish()?.into_inner())
    }

    /// All parts in order.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// Look up a part by name (ASCII case-insensitive, §9.1.1.1).
    pub fn part(&self, name: &str) -> Option<&Part> {
        self.parts
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Insert or replace a part's bytes (keeps the original position when replacing).
    pub fn put_part(&mut self, name: &str, data: Vec<u8>) {
        if let Some(p) = self
            .parts
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(name))
        {
            p.data = data;
        } else {
            self.parts.push(Part {
                name: name.to_owned(),
                data,
            });
        }
    }

    /// Insert a part and register its content type override.
    pub fn add_part(&mut self, name: &str, content_type: &str, data: Vec<u8>) -> Result<()> {
        let mut ct = self.content_types()?;
        let ext_default = name
            .rsplit_once('.')
            .and_then(|(_, e)| ct.defaults.iter().find(|(d, _)| d.eq_ignore_ascii_case(e)))
            .map(|(_, t)| t.clone());
        if ext_default.as_deref() != Some(content_type) {
            ct.set_override(name, content_type);
            self.set_content_types(&ct);
        }
        self.put_part(name, data);
        Ok(())
    }

    /// Remove a part (and nothing else).
    pub fn remove_part(&mut self, name: &str) -> Option<Part> {
        let i = self
            .parts
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))?;
        Some(self.parts.remove(i))
    }

    /// Parsed content types.
    pub fn content_types(&self) -> Result<ContentTypes> {
        let p = self
            .part(CONTENT_TYPES)
            .ok_or_else(|| OpcError::MissingPart(CONTENT_TYPES.into()))?;
        ContentTypes::parse(&p.data)
    }

    /// Replace the content types stream.
    pub fn set_content_types(&mut self, ct: &ContentTypes) {
        self.put_part(CONTENT_TYPES, ct.to_xml().into_bytes());
    }

    /// Relationships whose source is `source` (`"/"` for the package).
    pub fn relationships(&self, source: &str) -> Result<Vec<Relationship>> {
        let name = rels_part_name(source);
        match self.part(&name) {
            Some(p) => rels::parse(&p.data, &name),
            None => Ok(Vec::new()),
        }
    }

    /// Replace the relationships of `source`.
    pub fn set_relationships(&mut self, source: &str, rels: &[Relationship]) {
        self.put_part(&rels_part_name(source), rels::to_xml(rels).into_bytes());
    }

    /// First internal target part of relationship type `rel_type` from `source`.
    pub fn related_part(&self, source: &str, rel_type: &str) -> Result<Option<String>> {
        Ok(self
            .relationships(source)?
            .into_iter()
            .find(|r| r.rel_type == rel_type && r.mode == TargetMode::Internal)
            .map(|r| resolve_target(source, &r.target)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::REL_OFFICE_DOCUMENT;

    fn sample() -> Package {
        let mut p = Package::new();
        p.add_part(
            "/word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
            b"<w:document/>".to_vec(),
        )
        .unwrap();
        p.set_relationships(
            "/",
            &[Relationship {
                id: "rId1".into(),
                rel_type: REL_OFFICE_DOCUMENT.into(),
                target: "word/document.xml".into(),
                mode: TargetMode::Internal,
            }],
        );
        // An unknown, binary part that must survive untouched.
        p.put_part("/custom/blob.bin", vec![0, 1, 2, 255, 254]);
        p
    }

    #[test]
    fn roundtrip_preserves_all_parts() {
        let p = sample();
        let bytes = p.write().unwrap();
        let q = Package::read(&bytes, &Limits::default()).unwrap();
        assert_eq!(
            q.part("/custom/blob.bin").unwrap().data,
            vec![0, 1, 2, 255, 254]
        );
        assert_eq!(
            q.related_part("/", REL_OFFICE_DOCUMENT).unwrap().as_deref(),
            Some("/word/document.xml")
        );
        assert_eq!(
            q.content_types().unwrap().get("/word/document.xml"),
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
            )
        );
        // Second round trip is stable at the part level.
        let r = Package::read(&q.write().unwrap(), &Limits::default()).unwrap();
        let names = |p: &Package| {
            p.parts()
                .iter()
                .map(|x| (x.name.clone(), x.data.clone()))
                .collect::<Vec<_>>()
        };
        let mut a = names(&q);
        let mut b = names(&r);
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_zip_bomb() {
        let mut p = Package::new();
        p.put_part("/big.xml", vec![b'a'; 4 * 1024 * 1024]);
        let bytes = p.write().unwrap();
        let limits = Limits {
            max_ratio: 50,
            ..Limits::default()
        };
        assert!(matches!(
            Package::read(&bytes, &limits),
            Err(OpcError::LimitExceeded(_))
        ));
        let limits = Limits {
            max_part_size: 1024,
            ..Limits::default()
        };
        assert!(matches!(
            Package::read(&bytes, &limits),
            Err(OpcError::LimitExceeded(_))
        ));
    }

    #[test]
    fn rejects_garbage() {
        assert!(Package::read(b"not a zip", &Limits::default()).is_err());
    }

    #[test]
    fn requires_content_types() {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        w.start_file("a.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"<a/>").unwrap();
        let bytes = w.finish().unwrap().into_inner();
        assert!(matches!(
            Package::read(&bytes, &Limits::default()),
            Err(OpcError::MissingPart(_))
        ));
    }
}
