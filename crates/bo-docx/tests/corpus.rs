//! Round-trip fidelity over `tests/corpus/doc` (prompt §7, Phase 2 done criterion: ≥ 95 %).
//!
//! 1. *Lossless*: open → save without edits; every part must be identical after XML
//!    canonicalisation (attributes sorted, insignificant whitespace dropped); binary
//!    parts byte-identical.
//! 2. *Model*: open → drop all verbatim XML → save (everything regenerated) → open;
//!    the modelled content must be unchanged.
//! 3. *Locality*: edit one paragraph → save; every other part stays identical.

use bo_docx::{read_docx, write_docx};
use bo_model::{Block, Document, RunContent};
use bo_opc::{Limits, Package};
use quick_xml::events::Event;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn corpus() -> Vec<PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/doc");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "docx"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn resolve_entity(name: &str) -> String {
    match name {
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        n if n.starts_with("#x") => u32::from_str_radix(&n[2..], 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default(),
        n if n.starts_with('#') => n[1..]
            .parse::<u32>()
            .ok()
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default(),
        n => format!("&{n};"),
    }
}

/// Canonical event list of an XML part.
fn canonical(xml: &[u8]) -> Vec<String> {
    let mut r = quick_xml::Reader::from_reader(xml);
    let mut out = Vec::new();
    let mut text = String::new();
    let flush = |text: &mut String, out: &mut Vec<String>| {
        // Whitespace-only text containing a line break is formatting, not content.
        if (!text.trim().is_empty() || !text.contains('\n')) && !text.is_empty() {
            out.push(format!("#{text}"));
        }
        text.clear();
    };
    let start = |e: &quick_xml::events::BytesStart<'_>| {
        let mut attrs: Vec<String> = e
            .attributes()
            .filter_map(Result::ok)
            .map(|a| {
                format!(
                    "{}={}",
                    <_ as AsRef<str>>::as_ref(&a.key),
                    bo_opc::xml::unescape(&a.value)
                )
            })
            .collect();
        attrs.sort();
        format!(
            "<{} {}",
            <_ as AsRef<str>>::as_ref(&e.name()),
            attrs.join(" ")
        )
    };
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                flush(&mut text, &mut out);
                out.push(start(&e));
            }
            Ok(Event::Empty(e)) => {
                flush(&mut text, &mut out);
                out.push(start(&e));
                out.push("/".into());
            }
            Ok(Event::End(_)) => {
                flush(&mut text, &mut out);
                out.push("/".into());
            }
            Ok(Event::Text(t)) => text.push_str(&bo_opc::xml::unescape(t.as_ref())),
            Ok(Event::CData(t)) => text.push_str(t.as_ref()),
            Ok(Event::GeneralRef(g)) => text.push_str(&resolve_entity(g.as_ref())),
            Ok(Event::Eof) => break,
            Err(e) => {
                out.push(format!("!error {e}"));
                break;
            }
            _ => {}
        }
        buf.clear();
    }
    out
}

fn parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let pkg = Package::read(bytes, &Limits::default()).expect("corpus package");
    pkg.parts()
        .iter()
        .map(|p| (p.name.clone(), p.data.clone()))
        .collect()
}

fn is_xml(name: &str) -> bool {
    name.ends_with(".xml") || name.ends_with(".rels")
}

/// Names of parts that differ between two packages.
fn diff(a: &[u8], b: &[u8]) -> Vec<String> {
    let pa = parts(a);
    let pb = parts(b);
    let mut out = Vec::new();
    for (name, da) in &pa {
        match pb.get(name) {
            None => out.push(format!("{name} (missing)")),
            Some(db) => {
                let same = if is_xml(name) {
                    canonical(da) == canonical(db)
                } else {
                    da == db
                };
                if !same {
                    out.push(name.clone());
                }
            }
        }
    }
    for name in pb.keys() {
        if !pa.contains_key(name) {
            out.push(format!("{name} (added)"));
        }
    }
    out
}

fn strip_blocks(v: &mut [Block]) {
    for b in v {
        match b {
            Block::Paragraph(p) => {
                p.src = None;
                for r in &mut p.runs {
                    for c in &mut r.content {
                        if let RunContent::Image(img) = c {
                            img.src = None;
                        }
                    }
                }
            }
            Block::Table(t) => {
                t.src = None;
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        strip_blocks(&mut cell.blocks);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Drop every verbatim copy so that the writer regenerates all modelled XML.
fn strip(mut d: Document) -> Document {
    d.source = None;
    for s in &mut d.sections {
        s.src = None;
        strip_blocks(&mut s.blocks);
        for hf in [&mut s.headers, &mut s.footers] {
            for b in [&mut hf.default, &mut hf.first, &mut hf.even]
                .into_iter()
                .flatten()
            {
                strip_blocks(b);
            }
        }
    }
    for n in d.footnotes.values_mut().chain(d.endnotes.values_mut()) {
        strip_blocks(n);
    }
    for s in &mut d.styles.styles {
        s.src = None;
    }
    d
}

/// Short description of the first difference between two models.
fn first_diff(a: &Document, b: &Document) -> String {
    let fa = format!("{a:#?}");
    let fb = format!("{b:#?}");
    let la: Vec<&str> = fa.lines().collect();
    let lb: Vec<&str> = fb.lines().collect();
    for (i, (x, y)) in la.iter().zip(&lb).enumerate() {
        if x != y {
            let ctx = la[i.saturating_sub(4)..i].join(" | ");
            return format!("line {i}: [{ctx}] {} != {}", x.trim(), y.trim());
        }
    }
    format!("length {} != {}", la.len(), lb.len())
}

#[test]
fn corpus_round_trips_losslessly() {
    let files = corpus();
    assert!(files.len() >= 30, "corpus has {} files", files.len());
    let mut lossless = 0;
    let mut model = 0;
    let mut report = Vec::new();
    for f in &files {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(f).expect("read corpus file");
        let doc = read_docx(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let saved = write_docx(&doc).unwrap_or_else(|e| panic!("{name}: {e}"));
        let d = diff(&bytes, &saved);
        if d.is_empty() {
            lossless += 1;
        } else {
            report.push(format!("{name}: lossless diff in {d:?}"));
        }
        // Full regeneration keeps the modelled content.
        let regenerated = write_docx(&strip(doc.clone())).unwrap_or_else(|e| panic!("{name}: {e}"));
        let back = read_docx(&regenerated).unwrap_or_else(|e| panic!("{name}: re-read {e}"));
        if strip(back.clone()) == strip(doc.clone()) {
            model += 1;
        } else {
            report.push(format!(
                "{name}: model changed after regeneration: {}",
                first_diff(&strip(back), &strip(doc.clone()))
            ));
        }
    }
    let n = files.len();
    let pct = |k: usize| k as f64 * 100.0 / n as f64;
    println!(
        "round-trip: {lossless}/{n} lossless ({:.1} %), {model}/{n} model-stable ({:.1} %)",
        pct(lossless),
        pct(model)
    );
    for r in &report {
        println!("  {r}");
    }
    assert!(
        pct(lossless) >= 95.0,
        "lossless {:.1} % < 95 %: {report:#?}",
        pct(lossless)
    );
    assert!(
        pct(model) >= 95.0,
        "model-stable {:.1} % < 95 %: {report:#?}",
        pct(model)
    );
}

#[test]
fn editing_one_paragraph_keeps_other_parts() {
    for f in corpus() {
        let name = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(&f).expect("read corpus file");
        let mut doc = read_docx(&bytes).expect("read");
        let Some(Block::Paragraph(p)) = doc
            .sections
            .iter_mut()
            .flat_map(|s| s.blocks.iter_mut())
            .find(|b| matches!(b, Block::Paragraph(_)))
        else {
            continue;
        };
        p.runs.insert(
            0,
            bo_model::Run::text("Sửa: ", bo_model::RunProps::default()),
        );
        let saved = write_docx(&doc).expect("write");
        let d = diff(&bytes, &saved);
        assert!(
            d.iter().all(|n| n == "/word/document.xml"),
            "{name}: unexpected changes {d:?}"
        );
        let back = read_docx(&saved).expect("re-read");
        assert!(back.plain_text().contains("Sửa: "), "{name}");
    }
}

/// Writes every corpus file fully regenerated (no verbatim XML) into `$BO_REGEN_DIR`
/// so the writer's own output can be checked with OpenXmlValidator:
/// `BO_REGEN_DIR=out/regen cargo test -p bo-docx --test corpus -- --ignored`, then
/// `dotnet run --project tools/BlueOffice.Validate -- out/regen`.
#[test]
#[ignore = "manual: dumps regenerated files for schema validation"]
fn dump_regenerated_corpus() {
    let Ok(dir) = std::env::var("BO_REGEN_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    for f in corpus() {
        let bytes = std::fs::read(&f).expect("read corpus file");
        let doc = read_docx(&bytes).expect("read");
        let out = write_docx(&strip(doc)).expect("write");
        let name = f.file_name().expect("file name");
        std::fs::write(std::path::Path::new(&dir).join(name), out).expect("write file");
    }
}

#[test]
fn canonical_ignores_attribute_order_and_formatting() {
    let a = canonical(
        br#"<a x="1" y="2">
  <b/> <t> t </t></a>"#,
    );
    let b = canonical(br#"<a y="2" x="1"><b></b> <t> t </t></a>"#);
    assert_eq!(a, b);
    assert_ne!(canonical(b"<t>x</t>"), canonical(b"<t>y</t>"));
}

// source: ECMA-376 Part 1, §17.18.33 ST_FtnEdn continuationNotice (§17.11.10 footnote)
#[test]
fn continuation_notice_survives_a_footnote_edit() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/notes-06-continuation-notice.docx");
    let mut doc = read_docx(&std::fs::read(path).unwrap()).unwrap();
    let notice = doc
        .continuation_notice
        .clone()
        .expect("notice in the corpus file");
    // Editing a note rewrites the footnotes part.
    let id = *doc.footnotes.keys().next().unwrap();
    doc.footnotes.insert(
        id,
        vec![Block::Paragraph(bo_model::Paragraph::new(
            bo_model::ParaProps::default(),
            vec![bo_model::Run::text("Đã sửa", bo_model::RunProps::default())],
        ))],
    );
    let bytes = write_docx(&doc).unwrap();
    let back = read_docx(&bytes).unwrap();
    assert_eq!(back.continuation_notice.as_ref(), Some(&notice));
    assert_eq!(back.footnotes.len(), doc.footnotes.len());
}
