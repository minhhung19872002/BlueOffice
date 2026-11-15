//! What a viewer shows about a document: the catalog's viewer preferences (ISO 32000-1 §7.7.2,
//! Table 28), the document information dictionary (§14.3.3, Table 317), the security handler
//! and permissions (§7.6.3.2, Table 22) and the fonts the pages use (§9.6, §9.7).
use crate::{inherited, resolve, text_string, PdfDocument};
use lopdf::Object;
use std::collections::HashSet;

/// Document properties for a Properties dialog.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocInfo {
    /// `/Title`, `/Author`, `/Subject`, `/Keywords`, `/Creator`, `/Producer`.
    pub title: String,
    /// Author.
    pub author: String,
    /// Subject.
    pub subject: String,
    /// Keywords.
    pub keywords: String,
    /// Application that made the original.
    pub creator: String,
    /// Application that made the PDF.
    pub producer: String,
    /// `/CreationDate` and `/ModDate` as written (`D:YYYYMMDDHHmmSS…`).
    pub created: String,
    /// Modification date.
    pub modified: String,
    /// Header version (`1.7`), or the catalog's `/Version` when later.
    pub version: String,
    /// Marked as tagged (`/MarkInfo /Marked true`).
    pub tagged: bool,
    /// Linearised for fast web view (§F.2).
    pub linearized: bool,
    /// Encryption: filter and revision (`Standard, revision 4`), empty when not encrypted.
    pub security: String,
    /// Raw `/P` permission bits when encrypted.
    pub permissions: Option<i64>,
}

/// A font the pages use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontInfo {
    /// `/BaseFont` (subset tag kept).
    pub name: String,
    /// `/Subtype` (`TrueType`, `Type1`, `Type0`, …).
    pub kind: String,
    /// `/Encoding` name, or `Custom`.
    pub encoding: String,
    /// The font program is in the file.
    pub embedded: bool,
}

impl PdfDocument {
    /// `/PageMode` of the catalog (`UseNone`, `UseOutlines`, `UseThumbs`, `FullScreen`, …).
    #[must_use]
    pub fn page_mode(&self) -> Option<String> {
        let root = self.doc.trailer_ref().get(b"Root").ok()?;
        let cat = resolve(&self.doc, root).as_dict().ok()?;
        let mode = resolve(&self.doc, cat.get(b"PageMode").ok()?)
            .as_name()
            .ok()?;
        Some(String::from_utf8_lossy(mode).into_owned())
    }

    /// The document's properties.
    #[must_use]
    pub fn doc_info(&self) -> DocInfo {
        let doc = &self.doc;
        let d = doc
            .trailer_ref()
            .get(b"Info")
            .ok()
            .and_then(|o| resolve(doc, o).as_dict().ok());
        let s = |k: &[u8]| {
            d.and_then(|d| d.get(k).ok())
                .and_then(|o| resolve(doc, o).as_str().ok())
                .map(|t| text_string(t).trim().to_owned())
                .unwrap_or_default()
        };
        let cat = doc
            .trailer_ref()
            .get(b"Root")
            .ok()
            .and_then(|o| resolve(doc, o).as_dict().ok());
        let mut version = doc.pdf_version().to_owned();
        if let Some(v) = cat
            .and_then(|c| c.get(b"Version").ok())
            .and_then(|o| resolve(doc, o).as_name().ok())
        {
            let v = String::from_utf8_lossy(v).into_owned();
            if v > version {
                version = v;
            }
        }
        let tagged = cat
            .and_then(|c| c.get(b"MarkInfo").ok())
            .and_then(|o| resolve(doc, o).as_dict().ok())
            .and_then(|m| m.get(b"Marked").ok())
            .and_then(|o| o.as_bool().ok())
            .unwrap_or(false);
        let head = &self.bytes[..self.bytes.len().min(1024)];
        let linearized = head.windows(11).any(|w| w == b"/Linearized");
        let enc = doc
            .trailer_ref()
            .get(b"Encrypt")
            .ok()
            .and_then(|o| resolve(doc, o).as_dict().ok());
        let (security, permissions) = match enc {
            Some(e) => {
                let filter = e
                    .get(b"Filter")
                    .and_then(Object::as_name)
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .unwrap_or_default();
                let r = e.get(b"R").ok().and_then(|o| o.as_i64().ok()).unwrap_or(0);
                (
                    format!("{filter}, revision {r}"),
                    e.get(b"P").ok().and_then(|o| o.as_i64().ok()),
                )
            }
            None => (String::new(), None),
        };
        DocInfo {
            title: s(b"Title"),
            author: s(b"Author"),
            subject: s(b"Subject"),
            keywords: s(b"Keywords"),
            creator: s(b"Creator"),
            producer: s(b"Producer"),
            created: s(b"CreationDate"),
            modified: s(b"ModDate"),
            version,
            tagged,
            linearized,
            security,
            permissions,
        }
    }

    /// The Advanced properties: base URL (`/URI /Base`), language (`/Lang`), trapped
    /// (`/Trapped`), binding (`/ViewerPreferences /Direction`), print scaling, duplex and
    /// number of copies (`/PrintScaling`, `/Duplex`, `/NumCopies`) as (key, value).
    #[must_use]
    pub fn advanced(&self) -> Vec<(&'static str, String)> {
        let doc = &self.doc;
        fn dict<'a>(
            doc: &'a crate::store::Document,
            o: Option<&'a Object>,
        ) -> Option<&'a lopdf::Dictionary> {
            o.and_then(|o| resolve(doc, o).as_dict().ok())
        }
        let cat = dict(doc, doc.trailer_ref().get(b"Root").ok());
        let info = dict(doc, doc.trailer_ref().get(b"Info").ok());
        let vp = cat.and_then(|c| dict(doc, c.get(b"ViewerPreferences").ok()));
        let text = |d: Option<&lopdf::Dictionary>, k: &[u8]| -> String {
            match d.and_then(|d| d.get(k).ok()).map(|o| resolve(doc, o)) {
                Some(Object::String(s, _)) => text_string(s),
                Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
                Some(Object::Boolean(b)) => b.to_string(),
                Some(Object::Integer(i)) => i.to_string(),
                _ => String::new(),
            }
        };
        let base = cat.and_then(|c| dict(doc, c.get(b"URI").ok()));
        vec![
            ("baseUrl", text(base, b"Base")),
            ("language", text(cat, b"Lang")),
            ("trapped", text(info, b"Trapped")),
            ("binding", text(vp, b"Direction")),
            ("printScaling", text(vp, b"PrintScaling")),
            ("duplex", text(vp, b"Duplex")),
            ("copies", text(vp, b"NumCopies")),
        ]
    }

    /// Fonts of the pages' resources (and of their form XObjects, one level), each once.
    #[must_use]
    pub fn font_list(&self) -> Vec<FontInfo> {
        let doc = &self.doc;
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let mut add_fonts = |res: &lopdf::Dictionary, out: &mut Vec<FontInfo>| {
            let Some(fonts) = res
                .get(b"Font")
                .ok()
                .and_then(|o| resolve(doc, o).as_dict().ok())
            else {
                return;
            };
            for (_, f) in fonts {
                let key = f.as_reference().ok();
                let Ok(fd) = resolve(doc, f).as_dict() else {
                    continue;
                };
                let name = fd
                    .get(b"BaseFont")
                    .and_then(Object::as_name)
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .unwrap_or_default();
                if !seen.insert(key.map_or_else(|| name.clone(), |k| format!("{k:?}"))) {
                    continue;
                }
                let kind = fd
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .map(|n| String::from_utf8_lossy(n).into_owned())
                    .unwrap_or_default();
                let encoding = match fd.get(b"Encoding").ok().map(|o| resolve(doc, o)) {
                    Some(Object::Name(n)) => String::from_utf8_lossy(n).into_owned(),
                    Some(_) => "Custom".into(),
                    None => String::new(),
                };
                // The descriptor (of the descendant font for Type 0).
                let desc_font = fd
                    .get(b"DescendantFonts")
                    .ok()
                    .and_then(|o| resolve(doc, o).as_array().ok())
                    .and_then(|a| a.first())
                    .and_then(|o| resolve(doc, o).as_dict().ok())
                    .unwrap_or(fd);
                let embedded = desc_font
                    .get(b"FontDescriptor")
                    .ok()
                    .and_then(|o| resolve(doc, o).as_dict().ok())
                    .is_some_and(|d| {
                        [&b"FontFile"[..], b"FontFile2", b"FontFile3"]
                            .iter()
                            .any(|k| d.has(k))
                    })
                    || kind == "Type3";
                out.push(FontInfo {
                    name,
                    kind,
                    encoding,
                    embedded,
                });
            }
        };
        for info in &self.pages {
            let Ok(page) = doc.get_dictionary(info.id) else {
                continue;
            };
            let Some(res) = inherited(doc, page, b"Resources").and_then(|o| o.as_dict().ok())
            else {
                continue;
            };
            add_fonts(res, &mut out);
            if let Some(xo) = res
                .get(b"XObject")
                .ok()
                .and_then(|o| resolve(doc, o).as_dict().ok())
            {
                for (_, x) in xo {
                    if let Ok(s) = resolve(doc, x).as_stream() {
                        if let Some(r) = s
                            .dict
                            .get(b"Resources")
                            .ok()
                            .and_then(|o| resolve(doc, o).as_dict().ok())
                        {
                            add_fonts(r, &mut out);
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::PdfDocument;

    fn open(cat: &str, extra: &[&str]) -> PdfDocument {
        let mut objs = vec![
            cat,
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << /Font << /F1 4 0 R /F2 5 0 R >> >> >>",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
            "<< /Type /Font /Subtype /TrueType /BaseFont /ABCDEF+Arial /FontDescriptor 6 0 R >>",
            "<< /Type /FontDescriptor /FontFile2 7 0 R >>",
            "<< /Length 0 >>\nstream\n\nendstream",
        ];
        objs.extend_from_slice(extra);
        let b = crate::tests::build_pdf(&objs);
        PdfDocument::open(b, crate::standard_fonts()).expect("opens")
    }

    #[test]
    fn page_mode_info_and_fonts() {
        let d = open(
            "<< /Type /Catalog /Pages 2 0 R /PageMode /UseOutlines /MarkInfo << /Marked true >> >>",
            &[],
        );
        assert_eq!(d.page_mode().as_deref(), Some("UseOutlines"));
        let i = d.doc_info();
        assert!(i.tagged && !i.linearized && i.security.is_empty());
        assert_eq!(i.version, "1.7");
        let f = d.font_list();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].name, "Helvetica");
        assert!(!f[0].embedded && f[0].encoding == "WinAnsiEncoding");
        assert!(f[1].embedded && f[1].kind == "TrueType");
        assert_eq!(
            open("<< /Type /Catalog /Pages 2 0 R >>", &[]).page_mode(),
            None
        );
        let a = open(
            "<< /Type /Catalog /Pages 2 0 R /Lang (vi-VN) /ViewerPreferences << /Duplex /DuplexFlipLongEdge >> >>",
            &[],
        )
        .advanced();
        assert!(a.contains(&("language", "vi-VN".to_owned())));
        assert!(a.contains(&("duplex", "DuplexFlipLongEdge".to_owned())));
    }
}
