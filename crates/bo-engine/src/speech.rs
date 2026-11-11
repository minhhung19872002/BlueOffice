//! Read aloud in BlueDoc (`docs/parity/BACKLOG.md` CORE-LANG-003): the text the host speaks and
//! the rectangles of the word and paragraph being spoken. Speaking itself belongs to the platform
//! (W3C Web Speech API `speechSynthesis`); the engine only answers two queries that never change
//! the document or the selection:
//!
//! * `speechText` — body paragraphs in reading order with their index text (the offsets of
//!   `bo_edit::Pos`), the resolved language of their text (§17.3.2.20 lang `@w:val`, through
//!   styles and docDefaults), the language the text reads as, and the byte ranges not to speak
//!   (hidden text §17.3.2.14 vanish, deleted text §17.13.5.14 del).
//! * `speechRects` — highlight rectangles of byte ranges of body paragraphs (word and paragraph).
//! * `speechTree` — the accessible structure of the body paragraphs laid out on a range of
//!   pages (and the caret's paragraph), for the screen-reader mirror of the canvas (WCAG 2.2
//!   1.3.1 / 4.1.2): index text, heading level (§17.3.1.20 outlineLvl through the style
//!   chain), list level (§17.3.1.19 numPr), the table cell holding it (§17.4.38 tbl, header
//!   rows from §17.4.49 tblHeader), hyperlinks (§17.16.22 hyperlink) and the alternative text
//!   of pictures (§20.4.2.5 docPr `@descr`).

use crate::edit::EditResult;
use crate::nav::{self, SelRect};
use crate::{EngineError, Result};
use bo_edit::{Pos, Selection};
use serde::{Deserialize, Serialize};

/// A read-aloud query (`cmd` starts with `speech`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SpeechCommand {
    /// Body paragraphs `from..=to` (absent: the whole document).
    SpeechText {
        /// First paragraph id.
        #[serde(default)]
        from: Option<usize>,
        /// Last paragraph id.
        #[serde(default)]
        to: Option<usize>,
    },
    /// Highlight rectangles of each range.
    SpeechRects {
        /// Byte ranges of body paragraphs.
        ranges: Vec<SpeechRange>,
    },
    /// Accessible structure of the body paragraphs on pages `from_page..=to_page`.
    SpeechTree {
        /// First page (0-based).
        from_page: usize,
        /// Last page (0-based, clamped to the last page).
        to_page: usize,
        /// A paragraph listed even when not on those pages (the caret's).
        #[serde(default)]
        include: Option<usize>,
    },
    /// The laid-out line showing byte `o` of body paragraph `p` (a position at a soft line
    /// end belongs to the next line).
    SpeechLine {
        /// Body paragraph id.
        p: usize,
        /// Byte offset.
        o: usize,
    },
}

/// The table cell of a paragraph (outermost table).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeCell {
    /// Table key (`section.block`): paragraphs with the same key are in one table.
    pub table: String,
    /// Row (0-based).
    pub row: usize,
    /// Column (0-based cell index in the row).
    pub col: usize,
    /// Rows of the table.
    pub rows: usize,
    /// Cells of the row.
    pub cols: usize,
    /// The row repeats as a header row (§17.4.49 tblHeader): its cells are column headers.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub header: bool,
}

/// A hyperlink of a paragraph: byte range of its index text and target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeLink {
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
    /// Target (URL or `#bookmark`).
    pub href: String,
}

/// An object (U+FFFC) of a paragraph and its text alternative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeObject {
    /// Byte offset of the U+FFFC in the index text.
    pub at: usize,
    /// `picture`, `equation` or `object`.
    pub kind: &'static str,
    /// Alternative text (empty: none given).
    pub alt: String,
}

/// One body paragraph of the accessible structure.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TreePara {
    /// Body paragraph id.
    pub p: usize,
    /// Index text (offsets of `bo_edit::Pos`; objects are U+FFFC).
    pub text: String,
    /// Heading level 1–9 (outline level + 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<u8>,
    /// List level 1–9 of a numbered or bulleted paragraph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<u8>,
    /// Table cell holding the paragraph.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<TreeCell>,
    /// Hyperlinks.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<TreeLink>,
    /// Objects and their text alternatives.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<TreeObject>,
    /// First page showing the paragraph (0-based).
    pub page: usize,
}

/// A byte range of a body paragraph's index text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct SpeechRange {
    /// Body paragraph id.
    pub p: usize,
    /// Start byte offset.
    pub start: usize,
    /// End byte offset.
    pub end: usize,
}

/// One body paragraph to speak.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeechPara {
    /// Body paragraph id.
    pub p: usize,
    /// Index text (offsets of `bo_edit::Pos`; objects are U+FFFC).
    pub text: String,
    /// Resolved language of the first spoken run (`None`: not set or `x-none`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Language the text reads as (`bo_spell::lang::detect`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected: Option<&'static str>,
    /// Byte ranges `[start, end)` not to speak (hidden or deleted text).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skip: Vec<[usize; 2]>,
}

/// Answer of a read-aloud query.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct SpeechInfo {
    /// Paragraphs (`speechText`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paras: Option<Vec<SpeechPara>>,
    /// Rectangles of each range, in order (`speechRects`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rects: Option<Vec<Vec<SelRect>>>,
    /// Accessible structure (`speechTree`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<Vec<TreePara>>,
    /// Byte range `[start, end)` of the line (`speechLine`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<[usize; 2]>,
}

impl SpeechCommand {
    /// Parses a JSON command when it is a read-aloud query.
    pub fn parse(v: &serde_json::Value) -> Option<SpeechCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("speech") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// The paragraph's spoken language and unspoken ranges.
fn para_speech(
    doc: &bo_model::Document,
    p: &bo_model::Paragraph,
) -> (Option<String>, Vec<[usize; 2]>) {
    let (_, items) = bo_model::index::para_index(p);
    let mut lang = None;
    let mut skip: Vec<[usize; 2]> = Vec::new();
    for it in items {
        if it.end == it.start {
            continue;
        }
        let direct = &p.runs[it.run].props;
        let eff = bo_edit::effective_run(doc, p, direct);
        // §17.3.2.14 vanish (hidden) and §17.13.5.14 del (deleted run) are not read.
        if eff.hidden.unwrap_or(false) || direct.del.is_some() {
            match skip.last_mut() {
                Some(last) if last[1] == it.start => last[1] = it.end,
                _ => skip.push([it.start, it.end]),
            }
            continue;
        }
        if lang.is_none() {
            lang = eff
                .lang
                .and_then(|l| l.val)
                .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("x-none"));
        }
    }
    (lang, skip)
}

/// The table cell of the paragraph at `path` (outermost table), if any.
fn tree_cell(doc: &bo_model::Document, path: &[usize]) -> Option<TreeCell> {
    if path.len() < 5 {
        return None;
    }
    let Some(bo_model::Block::Table(t)) = doc.sections.get(path[0])?.blocks.get(path[1]) else {
        return None;
    };
    let row = t.rows.get(path[2])?;
    Some(TreeCell {
        table: format!("{}.{}", path[0], path[1]),
        row: path[2],
        col: path[3],
        rows: t.rows.len(),
        cols: row.cells.len(),
        header: row.props.header,
    })
}

/// The accessible description of body paragraph `id` at `path`.
fn tree_para(doc: &bo_model::Document, id: usize, path: &[usize], page: usize) -> Option<TreePara> {
    use bo_model::RunContent;
    let p = bo_model::index::para_at(doc, path)?;
    let (text, items) = bo_model::index::para_index(p);
    let heading = bo_edit::fields::outline_level(doc, p).map(|l| l + 1);
    // §17.3.1.19 numPr, directly or from the paragraph style (numId 0 removes it).
    let num = p.props.num.or_else(|| {
        let style = p.props.style.as_ref()?;
        doc.styles.resolve_paragraph(style).0.num
    });
    let list = num.filter(|n| n.num_id != 0).map(|n| n.ilvl.min(8) + 1);
    let mut links: Vec<TreeLink> = Vec::new();
    let mut objects = Vec::new();
    for it in &items {
        if it.end == it.start {
            continue;
        }
        let run = &p.runs[it.run];
        let eff = bo_edit::effective_run(doc, p, &run.props);
        if let Some(href) = eff.link.filter(|l| !l.is_empty()) {
            match links.last_mut() {
                Some(l) if l.end == it.start && l.href == href => l.end = it.end,
                _ => links.push(TreeLink {
                    start: it.start,
                    end: it.end,
                    href,
                }),
            }
        }
        if text[it.start..].starts_with(bo_model::index::OBJECT) {
            let (kind, alt) = match run.content.get(it.item) {
                Some(RunContent::Image(img)) => ("picture", img.description.clone()),
                Some(RunContent::Markup(m)) if bo_model::index::is_math_xml(&m.xml) => {
                    ("equation", String::new())
                }
                _ => ("object", String::new()),
            };
            objects.push(TreeObject {
                at: it.start,
                kind,
                alt,
            });
        }
    }
    Some(TreePara {
        p: id,
        text,
        heading,
        list,
        cell: tree_cell(doc, path),
        links,
        objects,
        page,
    })
}

impl crate::Engine {
    /// Answer a read-aloud query (never changes the document or the selection).
    pub fn doc_speech_command(&mut self, cmd: SpeechCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let mut info = SpeechInfo::default();
        match cmd {
            SpeechCommand::SpeechText { from, to } => {
                let doc = s.doc();
                let paths = bo_model::index::body_paragraphs(doc);
                let n = paths.len();
                let from = from.unwrap_or(0).min(n);
                let to = to.map_or(n, |t| t.saturating_add(1).min(n));
                let mut paras = Vec::new();
                for (id, path) in paths.iter().enumerate().take(to).skip(from) {
                    let Some(p) = bo_model::index::para_at(doc, path) else {
                        continue;
                    };
                    let text = bo_model::index::para_index_text(p);
                    let (lang, skip) = para_speech(doc, p);
                    paras.push(SpeechPara {
                        p: id,
                        detected: bo_spell::lang::detect(&text),
                        text,
                        lang,
                        skip,
                    });
                }
                info.paras = Some(paras);
            }
            SpeechCommand::SpeechRects { ranges } => {
                let layout = self.layout.document();
                let rects = ranges
                    .iter()
                    .map(|r| {
                        let a = s.clamp(Pos::new(r.p, r.start));
                        let b = s.clamp(Pos::new(r.p, r.end.max(r.start)));
                        nav::selection_rects(
                            layout,
                            Selection {
                                anchor: a,
                                focus: b,
                            },
                        )
                    })
                    .collect();
                info.rects = Some(rects);
            }
            SpeechCommand::SpeechLine { p, o } => {
                let layout = self.layout.document();
                let lines = layout
                    .pages
                    .iter()
                    .flat_map(|pg| pg.lines.iter())
                    .filter(|l| l.source == bo_layout::LineSource::Body(p as u32));
                let mut best = None;
                for l in lines {
                    let (s0, e0) = (l.start as usize, l.end as usize);
                    if o >= s0 && o < e0 {
                        best = Some([s0, e0]);
                        break;
                    }
                    if o >= s0 || best.is_none() {
                        best = Some([s0, e0]);
                    }
                }
                info.line = best;
            }
            SpeechCommand::SpeechTree {
                from_page,
                to_page,
                include,
            } => {
                let layout = self.layout.document();
                // Paragraph id → first page showing it, for the pages asked for.
                let mut on: std::collections::BTreeMap<usize, usize> =
                    std::collections::BTreeMap::new();
                let first_page = |id: usize| {
                    layout.pages.iter().position(|pg| {
                        pg.lines
                            .iter()
                            .any(|l| l.source == bo_layout::LineSource::Body(id as u32))
                    })
                };
                let last = layout.pages.len().saturating_sub(1);
                for pi in from_page.min(last)..=to_page.min(last) {
                    let Some(page) = layout.pages.get(pi) else {
                        continue;
                    };
                    for l in &page.lines {
                        if let bo_layout::LineSource::Body(id) = l.source {
                            on.entry(id as usize).or_insert(pi);
                        }
                    }
                }
                if let Some(id) = include {
                    on.entry(id).or_insert_with(|| first_page(id).unwrap_or(0));
                }
                let doc = s.doc();
                let paths = bo_model::index::body_paragraphs(doc);
                info.tree = Some(
                    on.into_iter()
                        .filter_map(|(id, page)| tree_para(doc, id, paths.get(id)?, page))
                        .collect(),
                );
            }
        }
        let mut r = self.query_result()?;
        r.speech = Some(Box::new(info));
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::Value;

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(crate::plain_text_document(text), "t.docx").unwrap();
        e
    }

    fn cmd(e: &mut Engine, json: &str) -> Value {
        serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
    }

    #[test]
    fn text_lists_paragraphs_with_language() {
        let mut e = engine("Xin chào các bạn, hôm nay trời đẹp\nThe quick brown fox jumps");
        let r = cmd(&mut e, r#"{"cmd":"speechText"}"#);
        let paras = r["speech"]["paras"].as_array().unwrap();
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0]["p"], 0);
        assert_eq!(paras[0]["text"], "Xin chào các bạn, hôm nay trời đẹp");
        assert_eq!(paras[0]["detected"], "vi-VN");
        assert_eq!(paras[1]["p"], 1);
        assert!(paras[1].get("skip").is_none());
        // Range of paragraphs.
        let r = cmd(&mut e, r#"{"cmd":"speechText","from":1,"to":1}"#);
        let paras = r["speech"]["paras"].as_array().unwrap();
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0]["p"], 1);
    }

    #[test]
    fn rects_of_a_word_leave_the_selection_alone() {
        let mut e = engine("một hai ba\nfour five");
        let before = cmd(&mut e, r#"{"cmd":"state"}"#)["selection"].clone();
        // "một" is 5 bytes (ộ is 3 bytes in UTF-8).
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechRects","ranges":[{"p":0,"start":0,"end":5},{"p":1,"start":0,"end":9},{"p":1,"start":2,"end":2}]}"#,
        );
        let rects = r["speech"]["rects"].as_array().unwrap();
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].as_array().unwrap().len(), 1);
        assert!(rects[0][0]["w"].as_f64().unwrap() > 0.0);
        assert_eq!(rects[1].as_array().unwrap().len(), 1);
        // A collapsed range has no rectangle.
        assert_eq!(rects[2], serde_json::json!([]));
        // The whole paragraph is wider than its first word.
        let y0 = rects[0][0]["y"].as_f64().unwrap();
        let y1 = rects[1][0]["y"].as_f64().unwrap();
        assert!(y1 > y0);
        assert_eq!(r["selection"], before);
        // Offsets inside a multi-byte character are clamped, not an error.
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechRects","ranges":[{"p":0,"start":1,"end":4}]}"#,
        );
        assert_eq!(r["speech"]["rects"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn hidden_and_deleted_text_is_skipped_and_language_resolved() {
        use bo_model::{Block, Lang, Paragraph, RevMark, Run, RunProps, Section};
        let lang = |v: &str| {
            Some(Lang {
                val: Some(v.into()),
                ..Default::default()
            })
        };
        // "Một " hidden (§17.3.2.14), "hai " deleted (§17.13.5.14), "ba" tagged vi-VN.
        let runs = vec![
            Run::text(
                "Một ",
                RunProps {
                    hidden: Some(true),
                    lang: lang("en-US"),
                    ..Default::default()
                },
            ),
            Run::text(
                "hai ",
                RunProps {
                    del: Some(RevMark::default()),
                    ..Default::default()
                },
            ),
            Run::text(
                "ba",
                RunProps {
                    lang: lang("vi-VN"),
                    ..Default::default()
                },
            ),
        ];
        let mut doc = crate::plain_text_document("x");
        doc.sections = vec![Section::new(vec![
            Block::Paragraph(Paragraph::new(bo_model::ParaProps::default(), runs)),
            Block::Paragraph(Paragraph::new(
                bo_model::ParaProps::default(),
                vec![Run::text(
                    "plain",
                    RunProps {
                        lang: lang("x-none"),
                        ..Default::default()
                    },
                )],
            )),
        ])];
        let mut e = Engine::with_bundled_fonts();
        e.load(doc, "t.docx").unwrap();
        let r = cmd(&mut e, r#"{"cmd":"speechText"}"#);
        let paras = r["speech"]["paras"].as_array().unwrap();
        assert_eq!(paras[0]["text"], "Một hai ba");
        // "Một " is 6 bytes, "hai " 4: one merged byte range [0, 10).
        assert_eq!(paras[0]["skip"], serde_json::json!([[0, 10]]));
        // The language of the first spoken run, not of the hidden one.
        assert_eq!(paras[0]["lang"], "vi-VN");
        assert!(paras[1].get("lang").is_none());
        // The skipped range maps to the highlight of the spoken word "ba".
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechRects","ranges":[{"p":0,"start":10,"end":12}]}"#,
        );
        assert_eq!(r["speech"]["rects"][0].as_array().unwrap().len(), 1);
    }

    #[test]
    fn tree_describes_headings_lists_tables_links_and_pictures() {
        use bo_model::{
            Block, Cell, Emu, Image, NumRef, ParaProps, Paragraph, Row, RowProps, Run, RunContent,
            RunProps, Section, Table,
        };
        let para = |props: ParaProps, runs: Vec<Run>| Block::Paragraph(Paragraph::new(props, runs));
        let text = |t: &str| Run::text(t, RunProps::default());
        let cell = |t: &str| Cell {
            blocks: vec![para(ParaProps::default(), vec![text(t)])],
            ..Default::default()
        };
        let table = Table {
            rows: vec![
                Row {
                    props: RowProps {
                        header: true,
                        ..Default::default()
                    },
                    cells: vec![cell("Tên"), cell("Tuổi")],
                },
                Row {
                    cells: vec![cell("An"), cell("30")],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let picture = Run {
            props: RunProps::default(),
            content: vec![RunContent::Image(Box::new(Image {
                media: String::new(),
                width: Emu(914_400),
                height: Emu(914_400),
                anchor: None,
                crop: None,
                rotation: 0,
                description: "Biểu đồ doanh thu".into(),
                shape: None,
                format: None,
                src: None,
                ole: None,
            }))],
        };
        let link = Run::text(
            "trang chủ",
            RunProps {
                link: Some("https://example.org/".into()),
                ..Default::default()
            },
        );
        let mut doc = crate::plain_text_document("x");
        doc.sections = vec![Section::new(vec![
            para(
                ParaProps {
                    outline_level: Some(0),
                    ..Default::default()
                },
                vec![text("Chương 1")],
            ),
            para(
                ParaProps {
                    num: Some(NumRef { num_id: 1, ilvl: 1 }),
                    ..Default::default()
                },
                vec![text("Mục a")],
            ),
            Block::Table(table),
            para(ParaProps::default(), vec![text("Xem "), link, picture]),
        ])];
        let mut e = Engine::with_bundled_fonts();
        e.load(doc, "t.docx").unwrap();
        let before = cmd(&mut e, r#"{"cmd":"state"}"#)["selection"].clone();
        let r = cmd(&mut e, r#"{"cmd":"speechTree","fromPage":0,"toPage":0}"#);
        let tree = r["speech"]["tree"].as_array().unwrap();
        assert_eq!(tree.len(), 7, "{tree:?}");
        assert_eq!(tree[0]["text"], "Chương 1");
        assert_eq!(tree[0]["heading"], 1);
        assert_eq!(tree[1]["list"], 2);
        assert!(tree[1].get("heading").is_none());
        // The table: 2 × 2, its first row a header row.
        let c = &tree[2]["cell"];
        assert_eq!((c["row"].clone(), c["col"].clone()), (0.into(), 0.into()));
        assert_eq!((c["rows"].clone(), c["cols"].clone()), (2.into(), 2.into()));
        assert_eq!(c["header"], true);
        assert_eq!(tree[5]["text"], "30");
        assert_eq!(tree[5]["cell"]["row"], 1);
        assert_eq!(tree[5]["cell"]["col"], 1);
        assert!(tree[5]["cell"].get("header").is_none());
        assert_eq!(tree[2]["cell"]["table"], tree[5]["cell"]["table"]);
        // Link (byte range of "trang chủ") and picture (U+FFFC after it) with its alt text.
        let last = &tree[6];
        assert_eq!(last["links"][0]["href"], "https://example.org/");
        assert_eq!(last["links"][0]["start"], 4);
        assert_eq!(last["links"][0]["end"], 4 + "trang chủ".len());
        assert_eq!(last["objects"][0]["kind"], "picture");
        assert_eq!(last["objects"][0]["alt"], "Biểu đồ doanh thu");
        assert_eq!(last["objects"][0]["at"], 4 + "trang chủ".len());
        assert_eq!(last["page"], 0);
        // Pages past the end: clamped to the last page.
        let r = cmd(&mut e, r#"{"cmd":"speechTree","fromPage":5,"toPage":9}"#);
        assert_eq!(r["speech"]["tree"].as_array().unwrap().len(), 7);
        assert_eq!(r["selection"], before);
    }

    #[test]
    fn tree_includes_the_caret_paragraph_and_lines_are_found() {
        let text = (0..400)
            .map(|i| format!("Đoạn {i} có vài chữ."))
            .collect::<Vec<_>>()
            .join(
                "
",
            );
        let mut e = engine(&text);
        let r = cmd(&mut e, r#"{"cmd":"speechTree","fromPage":0,"toPage":0}"#);
        let first = r["speech"]["tree"].as_array().unwrap().clone();
        assert!(!first.is_empty() && first.len() < 400);
        assert!(first.iter().all(|p| p["page"] == 0));
        let r = cmd(
            &mut e,
            r#"{"cmd":"speechTree","fromPage":0,"toPage":0,"include":399}"#,
        );
        let with = r["speech"]["tree"].as_array().unwrap();
        assert_eq!(with.len(), first.len() + 1);
        let last = with.last().unwrap();
        assert_eq!(last["p"], 399);
        assert!(last["page"].as_u64().unwrap() > 0);
        // A long paragraph breaks into lines: the line of an offset is found.
        let long = "chữ ".repeat(200);
        let mut e = engine(long.trim_end());
        let r = cmd(&mut e, r#"{"cmd":"speechLine","p":0,"o":0}"#);
        let [s0, e0] = [
            r["speech"]["line"][0].as_u64().unwrap(),
            r["speech"]["line"][1].as_u64().unwrap(),
        ];
        assert_eq!(s0, 0);
        assert!(e0 > 0 && (e0 as usize) < long.trim_end().len());
        let r = cmd(&mut e, &format!(r#"{{"cmd":"speechLine","p":0,"o":{e0}}}"#));
        assert_eq!(r["speech"]["line"][0].as_u64().unwrap(), e0);
    }

    #[test]
    fn other_commands_are_not_speech() {
        let v: Value = serde_json::from_str(r#"{"cmd":"state"}"#).unwrap();
        assert!(super::SpeechCommand::parse(&v).is_none());
    }
}
