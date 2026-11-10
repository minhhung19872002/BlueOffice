//! Web formats: HTML and MHTML (single-file web page) import and export.
//!
//! * [`read_html`] / [`write_html`] — word-processing documents ↔ one HTML file
//!   (HTML Living Standard; inline CSS and simple `<style>` rules; pictures as
//!   `data:` URLs, RFC 2397);
//! * [`read_mht`] / [`write_mht`] — the same page with its pictures as parts of a
//!   `multipart/related` message (RFC 2557);
//! * [`read_workbook_html`] / [`write_workbook_html`] — worksheets ↔ HTML tables.
//!
//! Everything read here is untrusted: scripts are dropped by the parser and never
//! run or kept, no URL is ever fetched, nesting and sizes are bounded, and no
//! input makes a reader fail or panic.
#![forbid(unsafe_code)]

pub mod css;
pub mod doc_read;
pub mod doc_write;
pub mod dom;
pub mod entities;
pub mod image;
pub mod mht;
pub mod sheet;

pub use doc_read::{read_html, read_html_str, read_html_with, ReadOptions};
pub use doc_write::{write_html, write_html_with, Images, Resource, WriteOptions};
pub use mht::{is_mht, read_mht, write_mht, write_mht_page};
pub use sheet::{read_workbook_html, write_workbook_html};

/// The bytes look like an HTML page: after a byte order mark, white space and
/// comments, the text starts with a doctype or an HTML element.
pub fn is_html(bytes: &[u8]) -> bool {
    let head: String = if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        bo_encoding::web::decode_html(&bytes[..bytes.len().min(2048) & !1], None)
    } else {
        String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]).into_owned()
    };
    let mut s = head.trim_start_matches('\u{feff}').trim_start();
    // Leading comments and an XML declaration (XHTML).
    loop {
        if let Some(rest) = s.strip_prefix("<!--") {
            match rest.find("-->") {
                Some(end) => s = rest[end + 3..].trim_start(),
                None => return false,
            }
        } else if s.starts_with("<?xml") {
            match s.find("?>") {
                Some(end) => s = s[end + 2..].trim_start(),
                None => return false,
            }
        } else {
            break;
        }
    }
    let lower = s.get(..s.len().min(64)).unwrap_or("").to_ascii_lowercase();
    let lower = if lower.is_empty() {
        s.chars().take(32).collect::<String>().to_ascii_lowercase()
    } else {
        lower
    };
    [
        "<!doctype html",
        "<html",
        "<head",
        "<body",
        "<meta",
        "<title",
        "<table",
        "<div",
        "<p>",
        "<p ",
        "<h1",
        "<style",
        "<span",
        "<font",
        "<center",
        "<ul",
        "<ol",
    ]
    .iter()
    .any(|p| lower.starts_with(p))
}

#[cfg(test)]
mod tests {
    #[test]
    fn html_sniffing() {
        use super::is_html;
        assert!(is_html(
            b"\xEF\xBB\xBF \r\n<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\"><html>"
        ));
        assert!(is_html(
            b"<!-- saved from url=(0014)about:internet -->\n<HTML><body>"
        ));
        assert!(is_html(
            b"<?xml version=\"1.0\"?><html xmlns=\"http://www.w3.org/1999/xhtml\">"
        ));
        assert!(is_html(b"<table><tr><td>1"));
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend("<html><body>x".encode_utf16().flat_map(u16::to_le_bytes));
        assert!(is_html(&utf16));
        assert!(!is_html(b"plain text with <b>a tag</b>"));
        assert!(!is_html(b"<?xml version=\"1.0\"?><Workbook/>"));
        assert!(!is_html(b"PK\x03\x04"));
        assert!(!is_html(b""));
        assert!(!is_html(b"<!-- never closed"));
    }
}
