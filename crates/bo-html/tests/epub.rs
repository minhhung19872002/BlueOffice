//! EPUB 3 export: the container rules of the specification (first entry, required
//! files), a package document whose manifest and spine agree with the files, a
//! navigation document, and well-formed XML everywhere — also for every document
//! of the corpus.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_html::{read_html, write_epub, EpubOptions};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

/// A 2×1 PNG (signature, IHDR, IDAT, IEND).
const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0,
    2, 0, 0, 0, 1, 8, 2, 0, 0, 0, 0x7B, 0x40, 0xE8, 0xDD, 0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE,
    0x42, 0x60, 0x82,
];

fn options() -> EpubOptions {
    EpubOptions {
        title: "Sổ tay <dùng> & thử".into(),
        author: "Nguyễn Văn A".into(),
        language: "vi".into(),
        identifier: "urn:uuid:12345678-1234-4123-8123-123456789abc".into(),
        modified: "2026-11-10T08:30:00Z".into(),
    }
}

/// Entries of the ZIP in file order: (name, compression is "stored", bytes).
fn entries(epub: &[u8]) -> Vec<(String, bool, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(epub)).unwrap();
    let mut out: Vec<(u64, String, bool, Vec<u8>)> = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap();
        out.push((
            f.header_start(),
            f.name().unwrap().into_owned(),
            f.compression() == zip::CompressionMethod::Stored,
            data,
        ));
    }
    out.sort_by_key(|e| e.0);
    out.into_iter().map(|(_, n, s, d)| (n, s, d)).collect()
}

/// What the test needs from an XML file; panics unless it is well-formed.
#[derive(Default)]
struct Xml {
    /// (element local name, attributes) in document order.
    elements: Vec<(String, BTreeMap<String, String>)>,
    text: String,
}

/// The character of a reference: one of the five predefined entities or a
/// character reference. Anything else (`&nbsp;`) is not XML without a DTD.
fn reference(file: &str, name: &str) -> char {
    let code = |digits: &str, radix| {
        u32::from_str_radix(digits, radix)
            .ok()
            .and_then(char::from_u32)
            .unwrap_or_else(|| panic!("{file}: character reference &{name};"))
    };
    match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => match name.strip_prefix("#x") {
            Some(hex) => code(hex, 16),
            None => match name.strip_prefix('#') {
                Some(dec) => code(dec, 10),
                None => panic!("{file}: entity &{name};"),
            },
        },
    }
}

/// An attribute value with its references resolved.
fn unescape(file: &str, raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let end = rest[at..]
            .find(';')
            .unwrap_or_else(|| panic!("{file}: unterminated reference in {raw:?}"));
        out.push(reference(file, &rest[at + 1..at + end]));
        rest = &rest[at + end + 1..];
    }
    assert!(!rest.contains('<'), "{file}: `<` in an attribute value");
    out.push_str(rest);
    out
}

fn parse(name: &str, data: &[u8]) -> Xml {
    let text = std::str::from_utf8(data).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut r = Reader::from_str(text);
    let mut xml = Xml::default();
    let mut depth = 0usize;
    let mut roots = 0;
    loop {
        match r.read_event().unwrap_or_else(|e| panic!("{name}: {e}")) {
            ev @ (Event::Start(_) | Event::Empty(_)) => {
                let (e, open) = match &ev {
                    Event::Start(e) => (e, true),
                    Event::Empty(e) => (e, false),
                    _ => unreachable!(),
                };
                if depth == 0 {
                    roots += 1;
                }
                let mut attrs = BTreeMap::new();
                for a in e.attributes() {
                    let a = a.unwrap_or_else(|err| panic!("{name}: attribute: {err}"));
                    let key = a.key.as_ref().to_owned();
                    let raw: &str = a.value.as_ref();
                    let value = unescape(name, raw);
                    assert!(
                        attrs.insert(key.clone(), value).is_none(),
                        "{name}: duplicate attribute {key}"
                    );
                }
                let local = e.local_name().as_ref().to_owned();
                xml.elements.push((local, attrs));
                if open {
                    depth += 1;
                }
            }
            Event::End(_) => depth -= 1,
            Event::Text(t) => {
                let t = t.xml10_content();
                let t: &str = &t;
                assert!(
                    depth > 0 || t.trim().is_empty(),
                    "{name}: text outside the root"
                );
                xml.text.push_str(t);
            }
            // Only the predefined entities and character references exist in XML
            // without a DTD.
            Event::GeneralRef(g) => {
                let n = g.xml10_content();
                let n: &str = &n;
                xml.text.push(reference(name, n));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(depth, 0, "{name}: unclosed elements");
    assert_eq!(roots, 1, "{name}: one root element");
    xml
}

impl Xml {
    fn all(&self, name: &str) -> Vec<&BTreeMap<String, String>> {
        self.elements
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, a)| a)
            .collect()
    }
}

/// Checks every rule this exporter must keep; returns the chapter files in spine
/// order with their parsed XML.
fn check(epub: &[u8]) -> Vec<(String, Xml)> {
    // OCF: the first entry is `mimetype`, stored, without extra field, so that the
    // media type sits at byte 38 of the file.
    assert_eq!(&epub[..4], b"PK\x03\x04");
    assert_eq!(&epub[8..10], [0, 0], "mimetype is stored");
    assert_eq!(&epub[26..30], [8, 0, 0, 0], "name length 8, no extra field");
    assert_eq!(&epub[30..58], b"mimetypeapplication/epub+zip");
    let files = entries(epub);
    assert_eq!(files[0].0, "mimetype");
    assert!(files[0].1);
    assert_eq!(files[0].2, b"application/epub+zip");
    let get = |name: &str| -> &[u8] {
        &files
            .iter()
            .find(|f| f.0 == name)
            .unwrap_or_else(|| panic!("missing {name}"))
            .2
    };
    // The container names the package document.
    let container = parse("container.xml", get("META-INF/container.xml"));
    let root = &container.all("rootfile")[0];
    assert_eq!(root["full-path"], "OEBPS/content.opf");
    assert_eq!(root["media-type"], "application/oebps-package+xml");
    let opf = parse("content.opf", get("OEBPS/content.opf"));
    let package = &opf.all("package")[0];
    assert_eq!(package["version"], "3.0");
    // Required metadata; the unique identifier is the dc:identifier it names.
    let id = &opf.all("identifier")[0];
    assert_eq!(id["id"], package["unique-identifier"]);
    assert_eq!(opf.all("title").len(), 1);
    assert_eq!(opf.all("language").len(), 1);
    assert!(opf
        .all("meta")
        .iter()
        .any(|m| m.get("property").map(String::as_str) == Some("dcterms:modified")));
    // Manifest ↔ files: every item exists, every file is listed, ids are unique.
    let items = opf.all("item");
    let mut by_id = BTreeMap::new();
    for item in &items {
        let path = format!("OEBPS/{}", item["href"]);
        assert!(files.iter().any(|f| f.0 == path), "manifest item {path}");
        assert!(by_id.insert(item["id"].clone(), *item).is_none());
    }
    for (name, _, _) in &files {
        if let Some(rel) = name.strip_prefix("OEBPS/") {
            assert!(
                rel == "content.opf" || items.iter().any(|i| i["href"] == rel),
                "{name} is not in the manifest"
            );
        }
    }
    // Exactly one navigation document.
    let navs: Vec<_> = items
        .iter()
        .filter(|i| i.get("properties").map(String::as_str) == Some("nav"))
        .collect();
    assert_eq!(navs.len(), 1);
    assert_eq!(navs[0]["media-type"], "application/xhtml+xml");
    let nav = parse("nav.xhtml", get(&format!("OEBPS/{}", navs[0]["href"])));
    assert!(nav
        .all("nav")
        .iter()
        .any(|n| n.get("epub:type").map(String::as_str) == Some("toc")));
    // The spine lists content documents of the manifest; the table of contents
    // links to each of them.
    let spine = opf.all("itemref");
    assert_ne!(spine.len(), 0, "the spine lists the chapters");
    let links: Vec<&str> = nav.all("a").iter().map(|a| a["href"].as_str()).collect();
    let mut chapters = Vec::new();
    for r in spine {
        let item = by_id[&r["idref"]];
        assert_eq!(item["media-type"], "application/xhtml+xml");
        let href = item["href"].clone();
        assert!(
            links.contains(&href.as_str()),
            "no table of contents entry for {href}"
        );
        let xml = parse(&href, get(&format!("OEBPS/{href}")));
        let html = &xml.all("html")[0];
        assert_eq!(html["xmlns"], "http://www.w3.org/1999/xhtml");
        // Pictures and the style sheet are files of the publication.
        for img in xml.all("img") {
            let src = &img["src"];
            assert!(items.iter().any(|i| &i["href"] == src), "{href}: img {src}");
        }
        for link in xml.all("link") {
            assert!(items.iter().any(|i| i["href"] == link["href"]));
        }
        chapters.push((href, xml));
    }
    chapters
}

#[test]
fn document_becomes_chapters_with_navigation_and_pictures() {
    let png = base64::engine::general_purpose::STANDARD.encode(PNG);
    let html = format!(
        "<html><body><p>Lời nói đầu &amp; cảm ơn</p>\
         <h1>Chương 1: Mở đầu</h1><p>a&nbsp;b <b>đậm</b><br>dòng hai</p>\
         <p><img src=\"data:image/png;base64,{png}\" width=\"2\" height=\"1\" alt=\"Hình &quot;1&quot;\"></p>\
         <h2>Mục 1.1</h2><ul><li>một</li><li>hai</li></ul>\
         <h1>Chương 2 &lt;kết&gt;</h1>\
         <table><tr><td>x</td><td>y</td></tr></table>\
         <p><img src=\"data:image/png;base64,{png}\" alt=\"lần nữa\"></p></body></html>"
    );
    let doc = read_html(html.as_bytes());
    let epub = write_epub(&doc, &options()).unwrap();
    let chapters = check(&epub);
    let names: Vec<&str> = chapters.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(
        names,
        ["chapter-1.xhtml", "chapter-2.xhtml", "chapter-3.xhtml"]
    );
    // What precedes the first heading is its own chapter; then one per Heading 1.
    assert!(chapters[0].1.text.contains("Lời nói đầu & cảm ơn"));
    assert_eq!(chapters[0].1.all("h1").len(), 0);
    assert_eq!(chapters[1].1.all("h1").len(), 1);
    assert!(chapters[1].1.text.contains("Chương 1: Mở đầu"));
    assert!(chapters[1].1.text.contains("a\u{a0}b"));
    assert!(chapters[1].1.text.contains("Mục 1.1"));
    assert_eq!(chapters[1].1.all("br").len(), 1);
    assert_eq!(chapters[1].1.all("li").len(), 2);
    assert!(!chapters[1].1.text.contains("Chương 2"));
    assert!(chapters[2].1.text.contains("Chương 2 <kết>"));
    assert_eq!(chapters[2].1.all("td").len(), 2);
    // The same picture twice is one file.
    assert_eq!(chapters[1].1.all("img")[0]["src"], "images/image1.png");
    assert_eq!(chapters[1].1.all("img")[0]["alt"], "Hình \"1\"");
    assert_eq!(chapters[2].1.all("img")[0]["src"], "images/image1.png");
    let files = entries(&epub);
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "mimetype",
            "META-INF/container.xml",
            "OEBPS/content.opf",
            "OEBPS/nav.xhtml",
            "OEBPS/style.css",
            "OEBPS/chapter-1.xhtml",
            "OEBPS/chapter-2.xhtml",
            "OEBPS/chapter-3.xhtml",
            "OEBPS/images/image1.png",
        ]
    );
    let image = files.last().unwrap();
    assert_eq!(image.2, PNG);
    // Metadata and the table of contents, escaped.
    let opf = parse("opf", &files[2].2);
    assert!(opf.text.contains("Sổ tay <dùng> & thử"));
    assert!(opf.text.contains("Nguyễn Văn A"));
    assert!(opf
        .text
        .contains("urn:uuid:12345678-1234-4123-8123-123456789abc"));
    assert!(opf.text.contains("2026-11-10T08:30:00Z"));
    assert!(opf
        .all("item")
        .iter()
        .any(|i| i["href"] == "images/image1.png" && i["media-type"] == "image/png"));
    let nav = parse("nav", &files[3].2);
    let toc: Vec<&str> = nav
        .text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        toc,
        [
            "Sổ tay <dùng> & thử", // <title>
            "Sổ tay <dùng> & thử", // heading of the table of contents
            "Sổ tay <dùng> & thử", // the part before the first heading
            "Chương 1: Mở đầu",
            "Chương 2 <kết>",
        ]
    );
    let css = String::from_utf8(files[4].2.clone()).unwrap();
    assert!(css.contains("table{border-collapse:collapse}"));
    // The same input gives the same bytes.
    assert_eq!(write_epub(&doc, &options()).unwrap(), epub);
}

#[test]
fn empty_and_headingless_documents_are_valid_publications() {
    let empty = write_epub(
        &read_html(b""),
        &EpubOptions {
            title: String::new(),
            author: String::new(),
            language: "not a tag".into(),
            identifier: String::new(),
            modified: "yesterday".into(),
        },
    )
    .unwrap();
    let chapters = check(&empty);
    assert_eq!(chapters.len(), 1);
    let opf = parse("opf", &entries(&empty)[2].2);
    assert_eq!(opf.all("creator").len(), 0);
    assert!(opf.text.contains("und"));
    assert!(opf.text.contains("1970-01-01T00:00:00Z"));
    // No heading: one chapter; characters XML does not allow are dropped.
    let doc = read_html("<p>chỉ một đoạn\u{b}\u{c} &shy;</p>".as_bytes());
    let one = write_epub(&doc, &options()).unwrap();
    let chapters = check(&one);
    assert_eq!(chapters.len(), 1);
    assert!(chapters[0].1.text.contains("chỉ một đoạn"));
}

#[test]
fn every_corpus_document_exports_well_formed() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/doc");
    let mut n = 0;
    let mut split = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("docx") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(doc) = bo_docx::read_docx(&std::fs::read(&path).unwrap()) else {
            continue; // encrypted samples
        };
        let epub = write_epub(&doc, &options()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let chapters = check(&epub);
        if chapters.len() > 1 {
            split += 1;
        }
        // The body text is in the chapters: the first words of the first body
        // paragraph that has plain text (tracked deletions and fields aside).
        let all: String = chapters
            .iter()
            .flat_map(|c| c.1.text.split_whitespace())
            .collect();
        let first = doc
            .sections
            .iter()
            .flat_map(|s| &s.blocks)
            .find_map(|b| match b {
                bo_model::Block::Paragraph(p)
                    if p.runs.iter().all(|r| {
                        r.props.del.is_none()
                            && r.props.hidden != Some(true)
                            && r.content
                                .iter()
                                .all(|c| matches!(c, bo_model::RunContent::Text(_)))
                    }) && !p.plain_text().trim().is_empty() =>
                {
                    Some(p.plain_text().split_whitespace().collect::<String>())
                }
                _ => None,
            });
        if let Some(first) = first {
            assert!(all.contains(&first), "{name}: lost {first:?}");
        }
        n += 1;
    }
    assert!(n >= 60, "corpus documents exported: {n}");
    assert!(split >= 3, "documents with several chapters: {split}");
}
