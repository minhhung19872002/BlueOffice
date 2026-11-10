//! Single-file web pages: MHTML, a `multipart/related` MIME message holding the
//! HTML page and the resources it refers to (RFC 2557; MIME: RFC 2045 §6
//! transfer encodings, RFC 2046 §5.1 multipart, RFC 2387 `multipart/related`,
//! RFC 2392 `cid:` URLs).
//!
//! Reading resolves a picture by `Content-ID` (`cid:` URLs) or `Content-Location`
//! (absolute, or relative to the location of the page, RFC 2557 §8.2). Nothing is
//! fetched: a reference without a matching part stays unresolved.

use crate::doc_read::{read_html_with, ReadOptions};
use crate::doc_write::{write_html_with, Images, Resource, WriteOptions};
use base64::Engine as _;
use bo_model::Document;
use std::fmt::Write as _;

/// Deepest nesting of multipart bodies read.
const MAX_MULTIPART_DEPTH: usize = 8;
/// Most parts read from one file.
const MAX_PARTS: usize = 10_000;

/// A leaf part of the message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Part {
    /// Media type, lower case (`text/html`).
    pub content_type: String,
    /// `charset` parameter of the content type.
    pub charset: Option<String>,
    /// `Content-Location`.
    pub location: Option<String>,
    /// `Content-ID` without the angle brackets.
    pub content_id: Option<String>,
    /// Decoded body.
    pub data: Vec<u8>,
}

/// A parsed MHTML file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Archive {
    /// Leaf parts in file order.
    pub parts: Vec<Part>,
    /// Index of the root (the HTML page).
    pub root: Option<usize>,
}

/// The bytes look like an MHTML file: MIME headers announcing a multipart message
/// (or a single HTML part) at the very start.
pub fn is_mht(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_ascii_lowercase();
    let head = head.trim_start_matches('\u{feff}').trim_start();
    // The first line is a header field.
    let first = head.lines().next().unwrap_or("");
    let is_header = first.split_once(':').is_some_and(|(k, _)| {
        !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    });
    is_header
        && head.contains("mime-version:")
        && (head.contains("multipart/related") || head.contains("content-type:"))
}

/// Header fields of a MIME entity: (lower-case name, unfolded value).
type Headers = Vec<(String, String)>;

/// Split an entity into its header fields and body (RFC 5322 §2.1–2.2: fields end
/// at the first empty line; a line starting with white space continues a field).
fn split_entity(data: &[u8]) -> (Headers, &[u8]) {
    let mut headers: Headers = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let end = data[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(data.len(), |k| pos + k);
        let line = &data[pos..end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let next = (end + 1).min(data.len());
        if line.is_empty() {
            return (headers, &data[next..]);
        }
        let text = String::from_utf8_lossy(line);
        if line[0] == b' ' || line[0] == b'\t' {
            if let Some(last) = headers.last_mut() {
                last.1.push(' ');
                last.1.push_str(text.trim());
            }
        } else if let Some((k, v)) = text.split_once(':') {
            if headers.len() < 256 {
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
            }
        } else {
            // Not a header: the entity has no header block.
            return (headers, &data[pos..]);
        }
        pos = next;
        if end == data.len() {
            break;
        }
    }
    (headers, &[])
}

fn header<'h>(h: &'h Headers, name: &str) -> Option<&'h str> {
    h.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// Media type and parameters of a `Content-Type` value (RFC 2045 §5.1).
fn content_type(value: &str) -> (String, Vec<(String, String)>) {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in value.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                cur.push(c);
            }
            ';' if !quoted => parts.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    let mut it = parts.into_iter();
    let ty = it.next().unwrap_or_default().trim().to_ascii_lowercase();
    let params = it
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((
                k.trim().to_ascii_lowercase(),
                v.trim().trim_matches('"').to_owned(),
            ))
        })
        .collect();
    (ty, params)
}

/// Decode quoted-printable (RFC 2045 §6.7): `=XX` is a byte, `=` at a line end joins lines.
pub fn decode_quoted_printable(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b != b'=' {
            out.push(b);
            i += 1;
            continue;
        }
        let hex = |c: Option<&u8>| c.and_then(|&c| char::from(c).to_digit(16));
        if let (Some(h), Some(l)) = (hex(data.get(i + 1)), hex(data.get(i + 2))) {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else if data.get(i + 1) == Some(&b'\n') {
            i += 2;
        } else if data.get(i + 1) == Some(&b'\r') && data.get(i + 2) == Some(&b'\n') {
            i += 3;
        } else {
            out.push(b);
            i += 1;
        }
    }
    out
}

/// Encode as quoted-printable with lines of at most 76 characters (RFC 2045 §6.7).
pub fn encode_quoted_printable(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() + data.len() / 8);
    let mut col = 0;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b == b'\n' {
            out.push_str("\r\n");
            col = 0;
            i += 1;
            continue;
        }
        if b == b'\r' && data.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        // White space before a line end must be encoded (rule 3).
        let at_end = matches!(data.get(i + 1), None | Some(b'\n' | b'\r'));
        let literal = (b == b'\t' || (b' '..=b'~').contains(&b))
            && b != b'='
            && !((b == b' ' || b == b'\t') && at_end);
        let len = if literal { 1 } else { 3 };
        if col + len > 75 {
            out.push_str("=\r\n");
            col = 0;
        }
        if literal {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "={b:02X}");
        }
        col += len;
        i += 1;
    }
    out
}

fn decode_body(body: &[u8], encoding: &str) -> Vec<u8> {
    match encoding.trim().to_ascii_lowercase().as_str() {
        "base64" => {
            let clean: Vec<u8> = body
                .iter()
                .copied()
                .filter(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/'))
                .collect();
            // A truncated final quantum is dropped rather than failing the part.
            let keep = clean.len() - usize::from(clean.len() % 4 == 1);
            base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(&clean[..keep])
                .unwrap_or_default()
        }
        "quoted-printable" => decode_quoted_printable(body),
        _ => body.to_vec(),
    }
}

/// Bodies between the boundary delimiter lines of a multipart body (RFC 2046 §5.1.1).
fn split_multipart<'d>(body: &'d [u8], boundary: &str) -> Vec<&'d [u8]> {
    let delim = format!("--{boundary}");
    let delim = delim.as_bytes();
    let mut parts = Vec::new();
    let mut start: Option<usize> = None;
    let mut pos = 0;
    while pos <= body.len() {
        let end = body[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(body.len(), |k| pos + k);
        let line = &body[pos..end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.starts_with(delim) {
            let rest = &line[delim.len()..];
            let closing = rest.starts_with(b"--");
            if rest.iter().all(u8::is_ascii_whitespace) || closing {
                if let Some(s) = start {
                    // The line break before the delimiter belongs to it.
                    let mut e = pos;
                    if e > s && body[e - 1] == b'\n' {
                        e -= 1;
                    }
                    if e > s && body[e - 1] == b'\r' {
                        e -= 1;
                    }
                    parts.push(&body[s..e.max(s)]);
                }
                start = (!closing).then_some((end + 1).min(body.len()));
                if closing || parts.len() >= MAX_PARTS {
                    return parts;
                }
            }
        }
        if end >= body.len() {
            break;
        }
        pos = end + 1;
    }
    // No closing delimiter: the last part runs to the end.
    if let Some(s) = start {
        parts.push(&body[s..]);
    }
    parts
}

fn collect(data: &[u8], depth: usize, out: &mut Vec<Part>, start: &mut Option<String>) {
    if out.len() >= MAX_PARTS {
        return;
    }
    let (headers, body) = split_entity(data);
    let (ty, params) = content_type(header(&headers, "content-type").unwrap_or("text/plain"));
    let param = |name: &str| {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    if ty.starts_with("multipart/") {
        let Some(boundary) = param("boundary").filter(|b| !b.is_empty()) else {
            return;
        };
        if depth >= MAX_MULTIPART_DEPTH {
            return;
        }
        if start.is_none() {
            *start = param("start").map(|s| s.trim_matches(|c| c == '<' || c == '>').to_owned());
        }
        for part in split_multipart(body, &boundary) {
            collect(part, depth + 1, out, start);
        }
        return;
    }
    let encoding = header(&headers, "content-transfer-encoding").unwrap_or("7bit");
    out.push(Part {
        content_type: ty,
        charset: param("charset"),
        location: header(&headers, "content-location").map(str::to_owned),
        content_id: header(&headers, "content-id")
            .map(|v| v.trim().trim_matches(|c| c == '<' || c == '>').to_owned()),
        data: decode_body(body, encoding),
    });
}

/// Parse an MHTML file. Never fails: a file that is not MIME yields no parts.
pub fn parse(bytes: &[u8]) -> Archive {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    // Leading blank lines are not part of the header block.
    let skip = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let mut parts = Vec::new();
    let mut start = None;
    collect(&bytes[skip..], 0, &mut parts, &mut start);
    let is_html = |p: &Part| {
        matches!(
            p.content_type.as_str(),
            "text/html" | "application/xhtml+xml"
        )
    };
    // The root: the part named by `start`, else the first HTML part (RFC 2387 §3.2).
    let root = start
        .and_then(|id| {
            parts
                .iter()
                .position(|p| p.content_id.as_deref() == Some(id.as_str()))
        })
        .filter(|&i| is_html(&parts[i]))
        .or_else(|| parts.iter().position(is_html));
    Archive { parts, root }
}

impl Archive {
    /// The part a URL of the page refers to: `cid:` by `Content-ID`, anything else by
    /// `Content-Location` (as written, resolved against the page's location, or by
    /// its last path segment when nothing else matches).
    pub fn resolve(&self, url: &str) -> Option<&Part> {
        let url = url.trim();
        if let Some(cid) = url
            .get(..4)
            .filter(|p| p.eq_ignore_ascii_case("cid:"))
            .map(|_| &url[4..])
        {
            let cid = String::from_utf8_lossy(&crate::image::percent_decode(cid)).into_owned();
            return self
                .parts
                .iter()
                .find(|p| p.content_id.as_deref() == Some(cid.as_str()));
        }
        let by_location = |want: &str| {
            self.parts
                .iter()
                .enumerate()
                .find(|(i, p)| Some(*i) != self.root && p.location.as_deref() == Some(want))
                .map(|(_, p)| p)
        };
        if let Some(p) = by_location(url) {
            return Some(p);
        }
        // Relative to the page: replace the last segment of its location.
        let base = self
            .root
            .and_then(|i| self.parts[i].location.as_deref())
            .unwrap_or("");
        if let Some(cut) = base.rfind('/') {
            if let Some(p) = by_location(&format!("{}{}", &base[..=cut], url)) {
                return Some(p);
            }
        }
        let tail = |s: &str| {
            s.rsplit(['/', '\\'])
                .next()
                .unwrap_or(s)
                .to_ascii_lowercase()
        };
        let want = tail(url);
        if want.is_empty() {
            return None;
        }
        self.parts
            .iter()
            .enumerate()
            .find(|(i, p)| {
                Some(*i) != self.root && p.location.as_deref().is_some_and(|l| tail(l) == want)
            })
            .map(|(_, p)| p)
    }
}

/// Read the page of an MHTML file as a document (`None`: no HTML part).
pub fn read_mht(bytes: &[u8]) -> Option<Document> {
    let archive = parse(bytes);
    let root = &archive.parts[archive.root?];
    let resolver = |url: &str| archive.resolve(url).map(|p| p.data.clone());
    Some(read_html_with(
        &root.data,
        &ReadOptions {
            charset: root.charset.as_deref(),
            resolver: Some(&resolver),
        },
    ))
}

/// A boundary that does not occur in the message: derived from the content
/// (FNV-1a), so the same document always gives the same file.
fn boundary(html: &str, resources: &[Resource]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(html.as_bytes());
    for r in resources {
        feed(r.name.as_bytes());
        feed(&r.data);
    }
    let mut n = h;
    loop {
        let b = format!("----=_BlueOffice_{n:016X}");
        if !html.contains(&b) {
            return b;
        }
        n = n.wrapping_add(1);
    }
}

/// Save a document as one MHTML file: the HTML page (quoted-printable) and its
/// pictures (base64), referenced by `cid:` URLs.
pub fn write_mht(doc: &Document, title: &str) -> Vec<u8> {
    let (html, resources) = write_html_with(
        doc,
        &WriteOptions {
            title: title.to_owned(),
            images: Images::Cid,
        },
    );
    write_mht_page(&html, &resources)
}

/// One MHTML file (RFC 2557) from an HTML page and the resources it references by
/// `cid:` URL: the page quoted-printable, each resource base64. A page without
/// pictures (workbook tables) has no resources.
pub fn write_mht_page(html: &str, resources: &[Resource]) -> Vec<u8> {
    let b = boundary(html, resources);
    let mut out = String::with_capacity(html.len() * 2);
    out.push_str("From: <Saved by BlueOffice>\r\n");
    out.push_str("MIME-Version: 1.0\r\n");
    let _ = write!(
        out,
        "Content-Type: multipart/related;\r\n\ttype=\"text/html\";\r\n\tboundary=\"{b}\"\r\n\r\n"
    );
    out.push_str("This is a multi-part message in MIME format.\r\n\r\n");
    let _ = write!(out, "--{b}\r\n");
    out.push_str("Content-Type: text/html; charset=\"utf-8\"\r\n");
    out.push_str("Content-Transfer-Encoding: quoted-printable\r\n");
    out.push_str("Content-Location: file:///C:/document.htm\r\n\r\n");
    out.push_str(&encode_quoted_printable(html.as_bytes()));
    out.push_str("\r\n");
    for r in resources {
        let _ = write!(out, "--{b}\r\n");
        let _ = write!(out, "Content-Type: {}\r\n", r.mime);
        out.push_str("Content-Transfer-Encoding: base64\r\n");
        let _ = write!(
            out,
            "Content-ID: <{}>\r\n",
            r.url.trim_start_matches("cid:")
        );
        let _ = write!(
            out,
            "Content-Location: file:///C:/document_files/{}\r\n\r\n",
            r.name
        );
        let b64 = base64::engine::general_purpose::STANDARD.encode(&r.data);
        for line in b64.as_bytes().chunks(76) {
            out.push_str(std::str::from_utf8(line).unwrap_or_default());
            out.push_str("\r\n");
        }
        out.push_str("\r\n");
    }
    let _ = write!(out, "--{b}--\r\n");
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_printable_round_trip() {
        let text = "Tiếng Việt = đẹp \t\r\ncuối dòng có dấu cách \r\n".repeat(5) + &"x".repeat(200);
        let qp = encode_quoted_printable(text.as_bytes());
        assert!(qp.split("\r\n").all(|l| l.len() <= 76), "{qp}");
        assert!(qp.is_ascii());
        assert_eq!(
            String::from_utf8(decode_quoted_printable(qp.as_bytes())).unwrap(),
            text.replace("\r\n", "\n").replace('\n', "\r\n")
        );
        assert_eq!(
            decode_quoted_printable(b"a=3Db=\r\nc=\nd=ZZ="),
            b"a=bcd=ZZ="
        );
    }

    const SAMPLE: &str = "From: x\r\nMIME-Version: 1.0\r\nContent-Type: multipart/related;\r\n\tboundary=\"B1\"; type=\"text/html\"\r\n\r\npreamble\r\n--B1\r\nContent-Type: text/html; charset=\"windows-1252\"\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Location: http://example.test/dir/page.html\r\n\r\n<p>caf=E9 <img src=3D\"pic.gif\"><img src=3D\"cid:logo@x\"><img src=3D\"http://e=\r\nxample.test/missing.png\" alt=3D\"gone\"></p>\r\n--B1\r\nContent-Type: image/gif\r\nContent-Transfer-Encoding: base64\r\nContent-Location: http://example.test/dir/pic.gif\r\n\r\nR0lGODlhAgADAAAAAA==\r\n--B1\r\nContent-Type: image/gif\r\nContent-ID: <logo@x>\r\nContent-Transfer-Encoding: base64\r\n\r\nR0lGODlh\r\nBAAFAAAAAA==\r\n--B1--\r\nepilogue";

    #[test]
    fn reads_parts_and_resolves_references() {
        assert!(is_mht(SAMPLE.as_bytes()));
        assert!(!is_mht(b"<html>MIME-Version: 1.0"));
        let a = parse(SAMPLE.as_bytes());
        assert_eq!(a.parts.len(), 3);
        assert_eq!(a.root, Some(0));
        assert_eq!(a.parts[0].charset.as_deref(), Some("windows-1252"));
        assert!(a.resolve("pic.gif").is_some());
        assert!(a.resolve("http://example.test/dir/pic.gif").is_some());
        assert!(a.resolve("CID:logo@x").is_some());
        assert!(a.resolve("http://example.test/missing.png").is_none());
        assert!(a.resolve("page.html").is_none());
        let doc = read_mht(SAMPLE.as_bytes()).unwrap();
        let text = doc.plain_text();
        assert!(text.contains("café") && text.contains("gone"), "{text}");
        assert_eq!(doc.media.len(), 2);
    }

    #[test]
    fn broken_messages_do_not_fail() {
        for s in [
            "",
            "MIME-Version: 1.0",
            "MIME-Version: 1.0\r\nContent-Type: multipart/related\r\n\r\n--x",
            "Content-Type: multipart/related; boundary=\"\"\r\n\r\n",
            "Content-Type: multipart/related; boundary=b\r\n\r\n--b\r\n--b\r\n--b",
            "Content-Type: text/html\r\nContent-Transfer-Encoding: base64\r\n\r\n!!!A",
        ] {
            let _ = parse(s.as_bytes());
            let _ = read_mht(s.as_bytes());
        }
        // Nested multiparts stop at the depth limit.
        let mut deep = String::new();
        for i in 0..50 {
            deep.push_str(&format!(
                "Content-Type: multipart/mixed; boundary=b{i}\r\n\r\n--b{i}\r\n"
            ));
        }
        deep.push_str("Content-Type: text/html\r\n\r\n<p>x</p>");
        assert_eq!(parse(deep.as_bytes()).parts.len(), 0);
    }
}
