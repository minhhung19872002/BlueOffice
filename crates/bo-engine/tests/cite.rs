//! Citations and bibliography through the JSON command bridge (CORE-CITE-001): the corpus
//! files `cite-*.docx` (sources part §22.6, CITATION §17.16.5.8, BIBLIOGRAPHY §17.16.5.7),
//! style switching, save and reopen, BibTeX / RIS.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use serde_json::{json, Value};

#[allow(clippy::needless_pass_by_value)] // test helper: called with `json!` temporaries
fn cmd(e: &mut Engine, v: Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn lines(e: &Engine) -> Vec<String> {
    e.document()
        .unwrap()
        .plain_text()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn open(name: &str) -> Engine {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc")
        .join(name);
    let bytes = std::fs::read(path).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, name).unwrap();
    e
}

/// Write a saved file where `BO_CITE_OUT` points (to validate it with `bo-cli validate`).
fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_CITE_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

fn tags(r: &Value) -> Vec<&str> {
    r["cite"]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["tag"].as_str().unwrap())
        .collect()
}

/// The results stored in the corpus files are the ones the formatters give: choosing the
/// style the file already has changes nothing, and the files round-trip.
#[test]
fn corpus_results_are_current() {
    for (name, style, n) in [
        ("cite-01-apa-author-date.docx", "APA", 4),
        ("cite-02-ieee-numeric.docx", "IEEE", 3),
        ("cite-03-vn-thesis.docx", "VNThesis", 6),
    ] {
        let mut e = open(name);
        let before = lines(&e);
        let r = cmd(&mut e, json!({"cmd": "citeState"}));
        assert_eq!(r["cite"]["style"], style, "{name}");
        assert_eq!(tags(&r).len(), n, "{name}");
        assert_eq!(r["cite"]["hasBibliography"], true, "{name}");
        let r = cmd(&mut e, json!({"cmd": "citeSetStyle", "style": style}));
        assert!(r["error"].is_null(), "{name}: {r}");
        assert_eq!(r["cite"]["changed"], 0, "{name}");
        assert_eq!(lines(&e), before, "{name}");
        // Saved and opened again: same text, same sources.
        let saved = e.save_docx().unwrap();
        let mut f = Engine::with_bundled_fonts();
        f.open(&saved, name).unwrap();
        assert_eq!(lines(&f), before, "{name}");
        let again = cmd(&mut f, json!({"cmd": "citeState"}));
        assert_eq!(again["cite"]["sources"], r["cite"]["sources"], "{name}");
    }
}

#[test]
fn apa_corpus_switches_style_in_one_undo_step() {
    let mut e = open("cite-01-apa-author-date.docx");
    assert_eq!(
        lines(&e),
        [
            "Phương pháp nghiên cứu được trình bày trong (Smith & Doe, 2020).",
            "Kết quả gần đây (Nguyen et al., 2021, p. 105; World Health Organization, 2022) cho thấy xu hướng tương tự.",
            "Nguồn sẽ bổ sung sau (Placeholder1).",
            "Tài liệu tham khảo",
            "Nguyen, M. T., Smith, J. K., & Tran, L. (2021). Deep learning for text. Journal of Data, 12(3), 100–120. https://doi.org/10.1000/jd.2021.5",
            "Smith, J. K., & Doe, J. (2020). Research methods (2nd ed.). Blue Press.",
            "World Health Organization. (2022). Global report. WHO Press.",
            "Hết.",
        ]
    );
    let r = cmd(&mut e, json!({"cmd": "citeState"}));
    assert_eq!(tags(&r), ["Smi20", "Ngu21", "WHO22", "Placeholder1"]);
    let s = &r["cite"]["sources"];
    assert_eq!(s[0]["cited"], true);
    assert_eq!(s[3]["placeholder"], true);
    assert_eq!(s[0]["citation"], "(Smith & Doe, 2020)");
    assert_eq!(
        s[0]["preview"][1],
        json!({"text": "Research methods", "italic": true})
    );
    // IEEE: numbers in order of first citation; the placeholder keeps its tag.
    let r = cmd(&mut e, json!({"cmd": "citeSetStyle", "style": "IEEE"}));
    // Three citations and the bibliography.
    assert_eq!(r["cite"]["changed"], 4);
    assert_eq!(r["cite"]["style"], "IEEE");
    assert_eq!(
        lines(&e)[..7],
        [
            "Phương pháp nghiên cứu được trình bày trong [1].",
            "Kết quả gần đây [2, p. 105], [3] cho thấy xu hướng tương tự.",
            "Nguồn sẽ bổ sung sau [Placeholder1].",
            "Tài liệu tham khảo",
            "[1]\tJ. K. Smith and J. Doe, Research methods, 2nd ed. Hanoi: Blue Press, 2020.",
            "[2]\tM. T. Nguyen, J. K. Smith, and L. Tran, “Deep learning for text,” Journal of Data, vol. 12, no. 3, pp. 100–120, 2021, doi: 10.1000/jd.2021.5.",
            "[3]\tWorld Health Organization, “Global report,” WHO Press, Geneva, 2022.",
        ]
    );
    let saved = e.save_docx().unwrap();
    keep("cite-switched-ieee.docx", &saved);
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(lines(&f), lines(&e));
    assert_eq!(
        cmd(&mut f, json!({"cmd": "citeState"}))["cite"]["style"],
        "IEEE"
    );
    // One undo step brings every citation and the list back.
    cmd(&mut e, json!({"cmd": "undo"}));
    assert_eq!(
        lines(&e)[0],
        "Phương pháp nghiên cứu được trình bày trong (Smith & Doe, 2020)."
    );
    assert_eq!(
        lines(&e)[6],
        "World Health Organization. (2022). Global report. WHO Press."
    );
    assert_eq!(
        cmd(&mut e, json!({"cmd": "citeState"}))["cite"]["style"],
        "APA"
    );
    // "Update fields" leaves current results alone.
    let r = cmd(&mut e, json!({"cmd": "updateFields"}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(
        lines(&e)[1],
        "Kết quả gần đây (Nguyen et al., 2021, p. 105; World Health Organization, 2022) cho thấy xu hướng tương tự."
    );
}

#[test]
fn vietnamese_thesis_corpus_to_harvard() {
    let mut e = open("cite-03-vn-thesis.docx");
    assert_eq!(
        lines(&e)[0],
        "Lúa lai đã được nghiên cứu từ sớm [2, tr. 10-16], [5], [6]."
    );
    cmd(&mut e, json!({"cmd": "citeSetStyle", "style": "Harvard"}));
    // The citations carry `\l 1066`: Vietnamese "và", "tr.".
    assert_eq!(
        lines(&e)[..2],
        [
            "Lúa lai đã được nghiên cứu từ sớm (Quách, 1992, tr. 10–16), (Anderson, 1985; Borkakati và Virmani, 1997).",
            "Việc xử lý dữ liệu cá nhân tuân theo quy định hiện hành (Chính phủ, 2023) và giáo trình (Nguyễn và Trần, 2015).",
        ]
    );
}

/// A new document: sources, a citation, a bibliography; saved and opened again.
#[test]
fn new_document_cite_save_reopen() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    cmd(&mut e, json!({"cmd": "insertText", "text": "Theo "}));
    let book = json!({
        "tag": "", "kind": "Book",
        "contributors": [{"role": "Author", "people": [{"last": "Smith", "first": "John"}]}],
        "fields": {"Title": "Research methods", "Year": "2020", "Publisher": "Blue Press"}
    });
    let r = cmd(&mut e, json!({"cmd": "citeSaveSources", "sources": [book]}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(tags(&r), ["Smi20"]);
    let r = cmd(
        &mut e,
        json!({"cmd": "citeInsert", "citation": {"items": [{"tag": "Smi20", "pages": "12"}], "lcid": "1033"}}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(lines(&e), ["Theo (Smith, 2020, p. 12)"]);
    // The caret touches the new citation: it can be edited.
    assert_eq!(r["cite"]["citation"]["items"][0]["pages"], "12");
    cmd(&mut e, json!({"cmd": "splitParagraph"}));
    let r = cmd(
        &mut e,
        json!({"cmd": "citeBibliography", "heading": "References"}),
    );
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(
        lines(&e)[..3],
        [
            "Theo (Smith, 2020, p. 12)",
            "References",
            "Smith, J. (2020). Research methods. Blue Press.",
        ]
    );
    // A cited source cannot be deleted.
    let r = cmd(&mut e, json!({"cmd": "citeDeleteSource", "tag": "Smi20"}));
    assert_eq!(r["error"], "cited");
    let saved = e.save_docx().unwrap();
    keep("cite-new-document.docx", &saved);
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    assert_eq!(lines(&f)[..3], lines(&e)[..3]);
    let r = cmd(&mut f, json!({"cmd": "citeState"}));
    assert_eq!(tags(&r), ["Smi20"]);
    assert_eq!(r["cite"]["sources"][0]["fields"]["Publisher"], "Blue Press");
    assert_eq!(r["cite"]["hasBibliography"], true);
    // Switching style in the reopened file rewrites both fields.
    let r = cmd(&mut f, json!({"cmd": "citeSetStyle", "style": "MLA"}));
    assert_eq!(r["cite"]["changed"], 2);
    assert_eq!(
        lines(&f)[..3],
        [
            "Theo (Smith 12)",
            "References",
            "Smith, John. Research methods. Blue Press, 2020.",
        ]
    );
    keep("cite-reopened-mla.docx", &f.save_docx().unwrap());
}

#[test]
fn import_export_and_preview() {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    let bib = "@article{k1, author = {Nguyen, Minh and Tran, Lan}, title = {Deep learning}, journal = {Journal of Data}, year = 2021}";
    let r = cmd(
        &mut e,
        json!({"cmd": "citeParse", "format": "bibtex", "text": bib, "taken": ["k1"]}),
    );
    let parsed = r["cite"]["parsed"].clone();
    assert_eq!(parsed[0]["tag"], "k11");
    assert_eq!(parsed[0]["kind"], "JournalArticle");
    assert_eq!(parsed[0]["contributors"][0]["people"][1]["last"], "Tran");
    // Parsing does not change the document.
    assert_eq!(tags(&r).len(), 0);
    let ris = "TY  - BOOK\nAU  - Lee, Anna\nTI  - Sampling\nPY  - 2019\nER  - \n";
    let r = cmd(
        &mut e,
        json!({"cmd": "citeParse", "format": "ris", "text": ris}),
    );
    assert_eq!(r["cite"]["parsed"][0]["tag"], "Lee19");
    let r = cmd(
        &mut e,
        json!({"cmd": "citeParse", "format": "xml", "text": ""}),
    );
    assert_eq!(r["error"], "badFormat");
    // Entries of sources that are not in the document (master list), in a chosen style.
    let r = cmd(
        &mut e,
        json!({"cmd": "citePreview", "style": "IEEE", "sources": parsed}),
    );
    assert_eq!(r["cite"]["preview"][0]["citation"], "[1]");
    assert_eq!(
        r["cite"]["preview"][0]["preview"][0]["text"],
        "M. Nguyen and L. Tran, “Deep learning,” "
    );
    // Added to the document and exported again.
    let r = cmd(&mut e, json!({"cmd": "citeSaveSources", "sources": parsed}));
    assert_eq!(tags(&r), ["k11"]);
    let r = cmd(&mut e, json!({"cmd": "citeExport"}));
    let text = r["cite"]["exported"].as_str().unwrap();
    assert!(
        text.starts_with("@article{k11,\n  author = {Nguyen, Minh and Tran, Lan},\n"),
        "{text}"
    );
    // A source nothing cites can be deleted.
    let r = cmd(&mut e, json!({"cmd": "citeDeleteSource", "tag": "k11"}));
    assert!(r["error"].is_null(), "{r}");
    assert_eq!(tags(&r).len(), 0);
}
