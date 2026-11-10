//! A word-processing document as an EPUB 3 publication (export only).
//!
//! Written from the W3C EPUB 3.3 recommendation:
//!
//! * the container (EPUB 3.3, "Open Container Format"): a ZIP file whose first
//!   entry is `mimetype` — stored, not compressed, without extra field, holding
//!   `application/epub+zip` — and `META-INF/container.xml` naming the package
//!   document;
//! * the package document `OEBPS/content.opf` ("Package document"): metadata
//!   (`dc:identifier`, `dc:title`, `dc:language`, the last-modified date), the
//!   manifest of every resource and the spine in reading order;
//! * the navigation document `OEBPS/nav.xhtml` ("EPUB navigation document"): a
//!   `nav` element of type `toc` with one entry per chapter;
//! * XHTML content documents `OEBPS/chapter-N.xhtml` — one per top-level heading,
//!   from the HTML serializer in XHTML syntax ([`crate::write_xhtml_chapters`]) —
//!   sharing `OEBPS/style.css`, and the pictures in `OEBPS/images/`.
//!
//! The output is deterministic: the identifier and the date come from the caller.

use crate::doc_write::{write_xhtml_chapters, Chapter};
use bo_model::Document;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

/// Media type of an EPUB container (the content of its `mimetype` entry).
pub const MIME_EPUB: &str = "application/epub+zip";

/// Metadata of the publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubOptions {
    /// `dc:title` (a placeholder is used when empty: the element is required).
    pub title: String,
    /// `dc:creator`; left out when empty.
    pub author: String,
    /// `dc:language`, a BCP 47 tag such as `vi` or `en-US`.
    pub language: String,
    /// `dc:identifier`, the unique identifier of the publication
    /// (`urn:uuid:…`).
    pub identifier: String,
    /// Last modification, `CCYY-MM-DDThh:mm:ssZ` (UTC, as the specification
    /// requires for `dcterms:modified`).
    pub modified: String,
}

/// Errors while writing the container.
#[derive(Debug, thiserror::Error)]
pub enum EpubError {
    /// The ZIP writer failed.
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    /// Writing to the buffer failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Text for XML element content and attribute values.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Characters XML 1.0 does not allow.
            c if c.is_control() && !matches!(c, '\t' | '\n' | '\r') => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// A language tag made of letters, digits and hyphens; anything else is `und`
/// (undetermined, BCP 47).
fn language(tag: &str) -> &str {
    let t = tag.trim();
    let ok = !t.is_empty()
        && t.len() <= 35
        && t.split('-')
            .all(|p| !p.is_empty() && p.len() <= 8 && p.chars().all(|c| c.is_ascii_alphanumeric()))
        && t.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    if ok {
        t
    } else {
        "und"
    }
}

/// `CCYY-MM-DDThh:mm:ssZ`, else the epoch (the property is required).
fn modified(date: &str) -> &str {
    let b = date.as_bytes();
    let shape = b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if shape {
        date
    } else {
        "1970-01-01T00:00:00Z"
    }
}

fn page(lang: &str, title: &str, head: &str, body: &str, nav: bool) -> String {
    let ops = if nav {
        " xmlns:epub=\"http://www.idpf.org/2007/ops\""
    } else {
        ""
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\"{ops} lang=\"{lang}\" xml:lang=\"{lang}\">\n\
         <head>\n<meta charset=\"utf-8\"/>\n<title>{}</title>\n{head}</head>\n\
         <body>\n{body}</body>\n</html>\n",
        esc(title)
    )
}

/// The document as an EPUB 3 file.
pub fn write_epub(doc: &Document, opts: &EpubOptions) -> Result<Vec<u8>, EpubError> {
    let mut book = write_xhtml_chapters(doc);
    if book.chapters.is_empty() {
        // The spine needs one content document.
        book.chapters.push(Chapter {
            title: None,
            body: String::new(),
        });
    }
    let title = if opts.title.trim().is_empty() {
        "Document"
    } else {
        opts.title.trim()
    };
    let lang = language(&opts.language);
    let identifier = if opts.identifier.trim().is_empty() {
        "urn:uuid:00000000-0000-4000-8000-000000000000"
    } else {
        opts.identifier.trim()
    };

    // Package document.
    let mut opf = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        opf,
        "<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
         unique-identifier=\"pub-id\" xml:lang=\"{lang}\">"
    );
    opf.push_str("<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n");
    let _ = writeln!(
        opf,
        "<dc:identifier id=\"pub-id\">{}</dc:identifier>",
        esc(identifier)
    );
    let _ = writeln!(opf, "<dc:title>{}</dc:title>", esc(title));
    let _ = writeln!(opf, "<dc:language>{lang}</dc:language>");
    if !opts.author.trim().is_empty() {
        let _ = writeln!(opf, "<dc:creator>{}</dc:creator>", esc(opts.author.trim()));
    }
    let _ = writeln!(
        opf,
        "<meta property=\"dcterms:modified\">{}</meta>",
        modified(&opts.modified)
    );
    opf.push_str("</metadata>\n<manifest>\n");
    opf.push_str(
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n\
         <item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n",
    );
    for i in 1..=book.chapters.len() {
        let _ = writeln!(
            opf,
            "<item id=\"chapter-{i}\" href=\"chapter-{i}.xhtml\" media-type=\"application/xhtml+xml\"/>"
        );
    }
    for (i, r) in book.resources.iter().enumerate() {
        let _ = writeln!(
            opf,
            "<item id=\"image-{}\" href=\"{}\" media-type=\"{}\"/>",
            i + 1,
            esc(&r.url),
            esc(&r.mime)
        );
    }
    opf.push_str("</manifest>\n<spine>\n");
    for i in 1..=book.chapters.len() {
        let _ = writeln!(opf, "<itemref idref=\"chapter-{i}\"/>");
    }
    opf.push_str("</spine>\n</package>\n");

    // Navigation document: one entry per chapter.
    let mut toc = format!(
        "<nav epub:type=\"toc\" id=\"toc\">\n<h1>{}</h1>\n<ol>\n",
        esc(title)
    );
    for (i, c) in book.chapters.iter().enumerate() {
        let _ = writeln!(
            toc,
            "<li><a href=\"chapter-{}.xhtml\">{}</a></li>",
            i + 1,
            esc(c.title.as_deref().unwrap_or(title))
        );
    }
    toc.push_str("</ol>\n</nav>\n");

    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // "Version made by" = MS-DOS on every platform: identical bytes everywhere.
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .system(zip::System::Dos);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .system(zip::System::Dos);
    w.start_file("mimetype", stored)?;
    w.write_all(MIME_EPUB.as_bytes())?;
    w.start_file("META-INF/container.xml", deflated)?;
    w.write_all(
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
          <container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
          <rootfiles>\n\
          <rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n\
          </rootfiles>\n</container>\n",
    )?;
    w.start_file("OEBPS/content.opf", deflated)?;
    w.write_all(opf.as_bytes())?;
    w.start_file("OEBPS/nav.xhtml", deflated)?;
    w.write_all(page(lang, title, "", &toc, true).as_bytes())?;
    w.start_file("OEBPS/style.css", deflated)?;
    w.write_all(book.css.as_bytes())?;
    let link = "<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/>\n";
    for (i, c) in book.chapters.iter().enumerate() {
        w.start_file(format!("OEBPS/chapter-{}.xhtml", i + 1), deflated)?;
        let name = c.title.as_deref().unwrap_or(title);
        w.write_all(page(lang, name, link, &c.body, false).as_bytes())?;
    }
    for r in &book.resources {
        // Pictures are compressed already.
        w.start_file(format!("OEBPS/{}", r.url), stored)?;
        w.write_all(&r.data)?;
    }
    Ok(w.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_guards() {
        assert_eq!(language("vi"), "vi");
        assert_eq!(language(" en-US "), "en-US");
        assert_eq!(language(""), "und");
        assert_eq!(language("x\"y"), "und");
        assert_eq!(language("-en"), "und");
        assert_eq!(language("1en"), "und");
        assert_eq!(modified("2026-11-10T08:30:00Z"), "2026-11-10T08:30:00Z");
        assert_eq!(modified("2026-11-10"), "1970-01-01T00:00:00Z");
        assert_eq!(modified("2026-11-10T08:30:00+7"), "1970-01-01T00:00:00Z");
        assert_eq!(esc("a<b>&\"c\"\u{0}\u{b}"), "a&lt;b&gt;&amp;&quot;c&quot;");
    }
}
