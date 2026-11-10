//! Web and single-file XML formats of the engines (CORE-FMT-003):
//!
//! * HTML and MHTML (single-file web page) for documents — `bo-html`;
//! * Flat OPC (`pkg:package`, one XML file holding a whole package) for documents,
//!   workbooks and presentations — `bo_opc::flat`;
//! * HTML tables and XML Spreadsheet 2003 for workbooks (`SheetEngine`, in
//!   [`crate::sheet`]);
//! * EPUB 3 export of documents — `bo_html::epub` (CORE-FMT-004).
//!
//! Opening sniffs the content first and falls back to the file extension. HTML is
//! untrusted: scripts are never run or kept and no external resource is fetched.

use crate::{Engine, EngineError, Result};
use bo_model::Document;

/// What a text-based file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebKind {
    /// An HTML page.
    Html,
    /// An MHTML file (single-file web page).
    Mht,
    /// A Flat OPC document.
    FlatOpc,
    /// An XML Spreadsheet 2003 workbook.
    XmlSpreadsheet,
}

impl WebKind {
    /// Short format name (`html`, `mht`, `xml-flat`, `xml-2003`).
    pub fn name(self) -> &'static str {
        match self {
            WebKind::Html => "html",
            WebKind::Mht => "mht",
            WebKind::FlatOpc => "xml-flat",
            WebKind::XmlSpreadsheet => "xml-2003",
        }
    }
}

/// Recognise a web / single-file XML format: by content, then by the extension of
/// `name` (`.html`, `.htm`, `.xhtml`, `.mht`, `.mhtml`). ZIP and compound files are
/// never one of them.
pub fn detect(bytes: &[u8], name: &str) -> Option<WebKind> {
    if bytes.starts_with(b"PK") || bytes.starts_with(crate::CFB_SIGNATURE) {
        return None;
    }
    if bo_opc::flat::is_flat_opc(bytes) {
        return Some(WebKind::FlatOpc);
    }
    if bo_xlsx::is_xml_spreadsheet(bytes) {
        return Some(WebKind::XmlSpreadsheet);
    }
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map_or("", |(_, e)| e);
    if bo_html::is_mht(bytes) || matches!(ext, "mht" | "mhtml") {
        return Some(WebKind::Mht);
    }
    if bo_html::is_html(bytes) || matches!(ext, "html" | "htm" | "xhtml") {
        return Some(WebKind::Html);
    }
    None
}

/// ZIP package bytes of a Flat OPC document; `None` when `bytes` are not Flat OPC.
pub(crate) fn flat_package(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    match bo_opc::flat::flat_to_zip(bytes) {
        None => Ok(None),
        Some(Ok(zip)) => Ok(Some(zip)),
        Some(Err(e)) => Err(EngineError::Format(e.to_string())),
    }
}

/// A word-processing document read from an HTML page or an MHTML file.
pub(crate) fn read_web_document(bytes: &[u8], kind: WebKind) -> Result<Document> {
    match kind {
        WebKind::Html => Ok(bo_html::read_html(bytes)),
        WebKind::Mht => bo_html::read_mht(bytes)
            .ok_or_else(|| EngineError::Format("the MHTML file has no HTML page".into())),
        WebKind::FlatOpc | WebKind::XmlSpreadsheet => {
            Err(EngineError::Unsupported(kind.name().to_owned()))
        }
    }
}

/// File name without its extension, as a page title.
fn title_of(name: &str) -> &str {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.rsplit_once('.').map_or(base, |(b, _)| b)
}

impl Engine {
    /// The open file in another format (CORE-FMT-003):
    ///
    /// * `html` — one filtered HTML file, pictures as `data:` URLs (documents);
    /// * `mht` — a single-file web page, RFC 2557 (documents);
    /// * `xml` — Flat OPC of the `.docx` / `.pptx` package. A save password does
    ///   not apply: the format has no encryption;
    /// * `epub` — an EPUB 3 publication, one chapter per top-level heading
    ///   (documents; CORE-FMT-004), dated with the system clock.
    pub fn export_as(&self, format: &str) -> Result<Vec<u8>> {
        self.export_as_at(format, None)
    }

    /// [`Engine::export_as`] at host time `now` (Unix milliseconds; the system
    /// clock when absent) — the modification date of an EPUB publication.
    pub fn export_as_at(&self, format: &str, now: Option<i64>) -> Result<Vec<u8>> {
        match format.to_ascii_lowercase().as_str() {
            "epub" => {
                let doc = self.document().ok_or(EngineError::NoDocument)?;
                let props = self.props.get();
                let title = if props.title.trim().is_empty() {
                    title_of(&self.name)
                } else {
                    props.title.trim()
                };
                // The publication identifier follows the content: the same
                // document exports to the same file.
                let id = crate::guid::guid_nth(&(title, doc.plain_text()), 0);
                let modified = now
                    .or_else(crate::review::system_ms)
                    .map_or_else(|| props.modified.clone(), crate::review::iso_date);
                let language = doc
                    .default_run
                    .lang
                    .as_ref()
                    .and_then(|l| l.val.clone())
                    .unwrap_or_else(|| "und".into());
                bo_html::write_epub(
                    doc,
                    &bo_html::EpubOptions {
                        title: title.to_owned(),
                        author: props.author.clone(),
                        language,
                        identifier: format!(
                            "urn:uuid:{}",
                            id.trim_matches(['{', '}']).to_ascii_lowercase()
                        ),
                        modified,
                    },
                )
                .map_err(|e| EngineError::Format(e.to_string()))
            }
            "html" | "htm" => {
                let doc = self.document().ok_or(EngineError::NoDocument)?;
                Ok(bo_html::write_html(doc, title_of(&self.name)).into_bytes())
            }
            "mht" | "mhtml" => {
                let doc = self.document().ok_or(EngineError::NoDocument)?;
                Ok(bo_html::write_mht(doc, title_of(&self.name)))
            }
            "xml" | "xml-flat" | "flat" => {
                let zip = if self.pres.is_some() {
                    self.pptx_package()?
                } else {
                    self.docx_package()?
                };
                bo_opc::flat::zip_to_flat(&zip).map_err(|e| EngineError::Format(e.to_string()))
            }
            other => Err(EngineError::Unsupported(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_prefers_content() {
        let flat = format!(
            "<?xml version=\"1.0\"?><pkg:package xmlns:pkg=\"{}\"/>",
            bo_opc::flat::NS_PKG
        );
        assert_eq!(detect(flat.as_bytes(), "a.xml"), Some(WebKind::FlatOpc));
        let ss = "<Workbook xmlns=\"urn:schemas-microsoft-com:office:spreadsheet\"/>";
        assert_eq!(
            detect(ss.as_bytes(), "a.html"),
            Some(WebKind::XmlSpreadsheet)
        );
        assert_eq!(detect(b"<html><body>x", "a.txt"), Some(WebKind::Html));
        assert_eq!(detect(b"just text", "a.HTM"), Some(WebKind::Html));
        assert_eq!(
            detect(
                b"MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=x\r\n",
                "a"
            ),
            Some(WebKind::Mht)
        );
        assert_eq!(detect(b"just text", "a.mhtml"), Some(WebKind::Mht));
        assert_eq!(detect(b"just text", "a.txt"), None);
        assert_eq!(detect(b"PK\x03\x04", "a.html"), None);
        assert_eq!(title_of("C:\\x\\Báo cáo.v2.docx"), "Báo cáo.v2");
    }
}
