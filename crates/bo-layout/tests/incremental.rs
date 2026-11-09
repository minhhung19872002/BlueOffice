//! Incremental layout (BlueOffice-PROMPT §5.4): random edits applied through
//! [`IncrementalLayout`] give exactly the layout of a full layout from scratch.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{
    layout_document_with, IncrementalLayout, LayoutCache, LayoutOptions, LineSource, Page,
};
use bo_model::change::BlockChanges;
use bo_model::{
    AbstractNum, Alignment, Anchor, Block, Cell, CellProps, Columns, Document, Emu, HalfPoints,
    HeaderFooter, Image, Level, Num, NumFormat, NumRef, Numbering, ParaProps, Paragraph, Row,
    RowProps, Run, RunContent, RunProps, Section, SectionBreak, Table, Twips, Wrap,
};
use bo_text::FontManager;
use proptest::prelude::*;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

const WORDS: &[&str] = &[
    "Căn",
    "cứ",
    "Luật",
    "Tổ",
    "chức",
    "Chính",
    "phủ",
    "ngày",
    "tháng",
    "năm",
    "2015;",
    "sửa",
    "đổi,",
    "bổ",
    "sung",
    "một",
    "số",
    "điều",
    "của",
    "địa",
    "phương",
    "Bộ",
    "trưởng",
    "Nội",
    "vụ.",
];

fn text(seed: u32, words: u32) -> String {
    (0..words)
        .map(|i| WORDS[((seed.wrapping_mul(31) + i * 7) as usize) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn image(wrap: Wrap) -> RunContent {
    RunContent::Image(Box::new(Image {
        media: "/word/media/image1.png".into(),
        width: Emu(914_400),
        height: Emu(685_800),
        anchor: Some(Anchor {
            h_relative: "column".into(),
            v_relative: "paragraph".into(),
            x: Emu(0),
            y: Emu(0),
            wrap,
            behind: false,
            ..Default::default()
        }),
        crop: None,
        rotation: 0,
        description: String::new(),
        shape: None,
        format: None,
        src: None,
    }))
}

/// One generated block: `kind` picks the feature, `seed`/`words` the text.
#[derive(Debug, Clone, Copy)]
struct Spec {
    kind: u8,
    seed: u32,
    words: u32,
}

fn block(s: Spec) -> Block {
    let mut props = ParaProps {
        space_after: Some(Twips(120)),
        ..Default::default()
    };
    let mut content = vec![RunContent::Text(text(s.seed, s.words))];
    match s.kind {
        0..=9 => {}
        10 => props.keep_next = Some(true),
        11 => props.page_break_before = Some(true),
        12 => props.keep_lines = Some(true),
        13 => props.align = Some(Alignment::Justify),
        14 | 15 => {
            props.num = Some(NumRef { num_id: 1, ilvl: 0 });
        }
        16 => content.push(RunContent::FootnoteRef(1 + (s.seed % 2) as i32)),
        17 => content.insert(
            0,
            RunContent::Field {
                instr: "PAGE".into(),
                cached: "1".into(),
                locked: false,
            },
        ),
        18 => content.push(RunContent::PageBreak),
        19 => {
            let wrap = match s.seed % 3 {
                0 => Wrap::Square,
                1 => Wrap::Tight,
                _ => Wrap::TopAndBottom,
            };
            content.insert(0, image(wrap));
        }
        20 => props.contextual_spacing = Some(true),
        // The total page count in the body (laid out with a probe layout).
        24 => content.push(RunContent::Field {
            instr: "NUMPAGES".into(),
            cached: "1".into(),
            locked: false,
        }),
        _ => {
            let cell = |t: String| Cell {
                props: CellProps::default(),
                blocks: vec![Block::Paragraph(Paragraph::new(
                    ParaProps::default(),
                    vec![Run::text(t, RunProps::default())],
                ))],
            };
            return Block::Table(Table {
                grid: vec![Twips(4000), Twips(4000)],
                rows: (0..2)
                    .map(|r| Row {
                        props: RowProps::default(),
                        cells: vec![
                            cell(text(s.seed + r, s.words / 3 + 1)),
                            cell(text(s.seed + r + 9, 2)),
                        ],
                    })
                    .collect(),
                ..Default::default()
            });
        }
    }
    Block::Paragraph(Paragraph::new(
        props,
        vec![Run {
            props: RunProps::default(),
            content,
        }],
    ))
}

fn note(t: &str) -> Vec<Block> {
    vec![Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::NoteMark, RunContent::Text(t.into())],
        }],
    ))]
}

fn field_para(prefix: &str, instr: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![
                RunContent::Text(prefix.into()),
                RunContent::Field {
                    instr: instr.into(),
                    cached: "1".into(),
                    locked: false,
                },
            ],
        }],
    ))
}

fn document(specs: &[Spec], split: usize, variant: u8) -> Document {
    let blocks: Vec<Block> = specs.iter().map(|s| block(*s)).collect();
    let split = split.min(blocks.len());
    let mut s1 = Section::new(blocks[..split].to_vec());
    let mut s2 = Section::new(blocks[split..].to_vec());
    s1.headers = HeaderFooter {
        default: Some(vec![field_para("Trang ", "PAGE")]),
        ..Default::default()
    };
    if variant & 1 == 1 {
        s1.footers.default = Some(vec![field_para("Tổng ", "NUMPAGES")]);
    }
    if variant & 2 == 2 {
        s2.columns = Columns::equal(2, Twips(720));
        s2.break_type = SectionBreak::Continuous;
    }
    if variant & 4 == 4 {
        s2.page_number_start = Some(1);
    }
    let mut d = Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..Default::default()
        },
        sections: vec![s1, s2],
        numbering: Numbering {
            abstracts: vec![AbstractNum {
                id: 0,
                levels: vec![Level {
                    ilvl: 0,
                    start: 1,
                    format: NumFormat::Decimal,
                    text: "%1.".into(),
                    align: Alignment::Left,
                    ppr: ParaProps::default(),
                    rpr: RunProps::default(),
                    restart: None,
                    suffix: "tab".into(),
                    pstyle: None,
                }],
            }],
            nums: vec![Num {
                id: 1,
                abstract_id: 0,
                start_overrides: Vec::new(),
            }],
        },
        ..Default::default()
    };
    d.footnotes.insert(1, note(" Chú thích một"));
    d.footnotes
        .insert(2, note(" Chú thích hai dài hơn một chút"));
    d
}

/// An edit of the generated document.
#[derive(Debug, Clone)]
enum Edit {
    Replace(usize, Spec),
    Insert(usize, Spec),
    Delete(usize),
    /// New default header of a section (`seed` picks it, `words` the length: long
    /// headers are taller and push the body).
    Header(u32, u32),
}

fn spec() -> impl Strategy<Value = Spec> {
    (0u8..26, 0u32..1000, 0u32..90).prop_map(|(kind, seed, words)| Spec { kind, seed, words })
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        4 => (any::<usize>(), spec()).prop_map(|(i, s)| Edit::Replace(i, s)),
        2 => (any::<usize>(), spec()).prop_map(|(i, s)| Edit::Insert(i, s)),
        2 => any::<usize>().prop_map(Edit::Delete),
        1 => (any::<u32>(), 0u32..60).prop_map(|(s, w)| Edit::Header(s, w)),
    ]
}

/// Apply `e` to `doc`, recording the change.
fn apply(doc: &mut Document, e: &Edit, changes: &mut BlockChanges) {
    let lens: Vec<usize> = doc.sections.iter().map(|s| s.blocks.len()).collect();
    let total: usize = lens.iter().sum();
    let locate = |i: usize, insert: bool| -> (usize, usize) {
        let n = if insert { total + 1 } else { total.max(1) };
        let mut i = i % n;
        for (si, &l) in lens.iter().enumerate() {
            if i < l || (insert && i == l && si + 1 == lens.len()) {
                return (si, i);
            }
            i -= l;
        }
        (lens.len() - 1, lens[lens.len() - 1])
    };
    match e {
        Edit::Replace(..) | Edit::Delete(_) if total == 0 => {}
        Edit::Replace(i, s) => {
            let (si, k) = locate(*i, false);
            doc.sections[si].blocks[k] = block(*s);
            changes.splice(si, k, 1, 1);
        }
        Edit::Insert(i, s) => {
            let (si, k) = locate(*i, true);
            doc.sections[si].blocks.insert(k, block(*s));
            changes.splice(si, k, 0, 1);
        }
        Edit::Delete(i) => {
            let (si, k) = locate(*i, false);
            doc.sections[si].blocks.remove(k);
            changes.splice(si, k, 1, 0);
        }
        Edit::Header(seed, words) => {
            let si = *seed as usize % doc.sections.len();
            doc.sections[si].headers.default = Some(vec![field_para(
                &format!("{} ", text(*seed, *words)),
                "PAGE",
            )]);
            changes.mark_headers(si);
        }
    }
}

/// What a page draws: its JSON without the paragraph ids of lines.
fn visual(p: &Page) -> String {
    let mut p = p.clone();
    for l in &mut p.lines {
        l.source = LineSource::Other;
    }
    serde_json::to_string(&p).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn random_edits_match_full_layout(
        specs in prop::collection::vec(spec(), 20..70),
        split in 0usize..70,
        variant in 0u8..8,
        steps in prop::collection::vec(prop::collection::vec(edit(), 1..3), 1..6),
    ) {
        let mut fm = fonts();
        let opts = LayoutOptions::default();
        let mut doc = document(&specs, split, variant);
        let mut inc = IncrementalLayout::new();
        let mut cache = LayoutCache::new();
        inc.update(&doc, &mut fm, &mut cache, opts, &BlockChanges::everything()).unwrap();
        for step in &steps {
            let before: Vec<String> = inc.document().pages.iter().map(visual).collect();
            let mut changes = BlockChanges::default();
            for e in step {
                apply(&mut doc, e, &mut changes);
            }
            let dirty = inc.update(&doc, &mut fm, &mut cache, opts, &changes).unwrap();
            if std::env::var_os("BO_LAYOUT_TRACE").is_some() {
                println!("{:?}", inc.last_update());
            }
            let full = layout_document_with(&doc, &mut fm, &mut LayoutCache::new(), opts).unwrap();
            prop_assert_eq!(inc.document().pages.len(), full.pages.len());
            for (i, p) in full.pages.iter().enumerate() {
                let want = serde_json::to_string(p).unwrap();
                let got = serde_json::to_string(&inc.document().pages[i]).unwrap();
                prop_assert_eq!(&got, &want, "page {} after {:?}", i, step);
                // Every page whose drawing changed is reported (paragraph ids are not drawn).
                if before.get(i) != Some(&visual(p)) {
                    prop_assert!(dirty.contains(&i), "page {} changed but not reported", i);
                }
            }
        }
    }
}

/// Typing in one paragraph of a long document re-paginates only around it.
#[test]
fn typing_reports_only_the_edited_page() {
    let mut fm = fonts();
    let opts = LayoutOptions::default();
    let specs: Vec<Spec> = (0..300)
        .map(|i| Spec {
            kind: 0,
            seed: i,
            words: 40,
        })
        .collect();
    let mut doc = document(&specs, 300, 0);
    let mut inc = IncrementalLayout::new();
    let mut cache = LayoutCache::new();
    inc.update(&doc, &mut fm, &mut cache, opts, &BlockChanges::everything())
        .unwrap();
    let pages = inc.document().pages.len();
    assert!(pages > 10);
    let mut changes = BlockChanges::default();
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[150] else {
        unreachable!()
    };
    p.runs[0].content[0] = RunContent::Text(format!("{} x", text(150, 40)));
    changes.splice(0, 150, 1, 1);
    let dirty = inc
        .update(&doc, &mut fm, &mut cache, opts, &changes)
        .unwrap();
    assert_eq!(dirty.len(), 1, "{dirty:?}");
    let full = layout_document_with(&doc, &mut fm, &mut LayoutCache::new(), opts).unwrap();
    assert_eq!(
        serde_json::to_string(inc.document()).unwrap(),
        serde_json::to_string(&full).unwrap()
    );
}

#[test]
fn per_page_footnote_numbers_stay_incremental() {
    // §17.11.19 numRestart eachPage: typing that moves no footnote to another page
    // is laid out incrementally (it used to force a full layout), and the result
    // equals a full layout, also when a note does change pages.
    use bo_layout::UpdateKind;
    use bo_model::NoteRestart;
    let mut fm = fonts();
    let opts = LayoutOptions::default();
    let specs: Vec<Spec> = (0..120)
        .map(|i| Spec {
            kind: 0,
            seed: i,
            words: 30,
        })
        .collect();
    let mut doc = document(&specs, 120, 0);
    doc.sections[0].footnote_pr.restart = Some(NoteRestart::EachPage);
    for (k, bi) in [(1, 10usize), (2, 11), (3, 60), (4, 100)] {
        let Block::Paragraph(p) = &mut doc.sections[0].blocks[bi] else {
            unreachable!()
        };
        p.runs.push(Run {
            props: RunProps::default(),
            content: vec![RunContent::FootnoteRef(k)],
        });
        doc.footnotes.insert(
            k,
            vec![Block::Paragraph(Paragraph::new(
                ParaProps::default(),
                vec![Run {
                    props: RunProps::default(),
                    content: vec![RunContent::NoteMark, RunContent::Text(" Ghi chú".into())],
                }],
            ))],
        );
    }
    let mut inc = IncrementalLayout::new();
    let mut cache = LayoutCache::new();
    inc.update(&doc, &mut fm, &mut cache, opts, &BlockChanges::everything())
        .unwrap();
    for (bi, extra) in [(80usize, " x"), (11, &" thêm nhiều chữ nữa".repeat(40))] {
        let mut changes = BlockChanges::default();
        let Block::Paragraph(p) = &mut doc.sections[0].blocks[bi] else {
            unreachable!()
        };
        if let RunContent::Text(t) = &mut p.runs[0].content[0] {
            t.push_str(extra);
        }
        changes.splice(0, bi, 1, 1);
        inc.update(&doc, &mut fm, &mut cache, opts, &changes)
            .unwrap();
        if bi == 80 {
            assert!(
                matches!(inc.last_update(), UpdateKind::Incremental { .. }),
                "{:?}",
                inc.last_update()
            );
        }
        let full = layout_document_with(&doc, &mut fm, &mut LayoutCache::new(), opts).unwrap();
        assert_eq!(
            serde_json::to_string(inc.document()).unwrap(),
            serde_json::to_string(&full).unwrap()
        );
    }
}

/// Lay `doc` out after `changes` and check the result against a full layout.
fn update_and_compare(
    inc: &mut IncrementalLayout,
    doc: &Document,
    fm: &mut FontManager,
    cache: &mut LayoutCache,
    changes: &BlockChanges,
) -> Vec<usize> {
    let opts = LayoutOptions::default();
    let dirty = inc.update(doc, fm, cache, opts, changes).unwrap();
    let full = layout_document_with(doc, fm, &mut LayoutCache::new(), opts).unwrap();
    assert_eq!(
        serde_json::to_string(inc.document()).unwrap(),
        serde_json::to_string(&full).unwrap()
    );
    dirty
}

#[test]
fn numpages_in_the_body_stays_incremental() {
    use bo_layout::UpdateKind;
    let mut fm = fonts();
    let specs: Vec<Spec> = (0..200)
        .map(|i| Spec {
            kind: if i % 50 == 7 { 24 } else { 0 },
            seed: i,
            words: 40,
        })
        .collect();
    let mut doc = document(&specs, 200, 1);
    let mut inc = IncrementalLayout::new();
    let mut cache = LayoutCache::new();
    update_and_compare(
        &mut inc,
        &doc,
        &mut fm,
        &mut cache,
        &BlockChanges::everything(),
    );
    let pages = inc.document().pages.len();
    assert!(pages > 10);
    // Typing that keeps the page count re-paginates around the edit only.
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[120] else {
        unreachable!()
    };
    p.runs[0].content[0] = RunContent::Text(format!("{} x", text(120, 40)));
    let mut changes = BlockChanges::default();
    changes.splice(0, 120, 1, 1);
    let dirty = update_and_compare(&mut inc, &doc, &mut fm, &mut cache, &changes);
    assert_eq!(
        inc.last_update(),
        UpdateKind::Incremental { resynced: true }
    );
    assert!(dirty.len() < 3, "{dirty:?}");
    // A new page changes every total: laid out again, still equal to a full layout.
    let long = Spec {
        kind: 0,
        seed: 5,
        words: 400,
    };
    for _ in 0..4 {
        doc.sections[0].blocks.insert(30, block(long));
    }
    let mut changes = BlockChanges::default();
    changes.splice(0, 30, 0, 4);
    update_and_compare(&mut inc, &doc, &mut fm, &mut cache, &changes);
    assert!(inc.document().pages.len() > pages);
}

#[test]
fn header_edits_redecorate_without_repaginating() {
    use bo_layout::UpdateKind;
    let mut fm = fonts();
    let specs: Vec<Spec> = (0..200)
        .map(|i| Spec {
            kind: 0,
            seed: i,
            words: 40,
        })
        .collect();
    // Two sections; the second inherits the first one's header (§17.10.5).
    let mut doc = document(&specs, 120, 0);
    let mut inc = IncrementalLayout::new();
    let mut cache = LayoutCache::new();
    update_and_compare(
        &mut inc,
        &doc,
        &mut fm,
        &mut cache,
        &BlockChanges::everything(),
    );
    let pages = inc.document().pages.len();
    let second = inc
        .document()
        .pages
        .iter()
        .position(|p| p.section == 1)
        .unwrap();
    // Typing in the second section's own header: only its pages change.
    doc.sections[1].headers.default = Some(vec![field_para("Phần hai, trang ", "PAGE")]);
    let mut changes = BlockChanges::default();
    changes.mark_headers(1);
    let dirty = update_and_compare(&mut inc, &doc, &mut fm, &mut cache, &changes);
    assert_eq!(
        inc.last_update(),
        UpdateKind::Incremental { resynced: true }
    );
    assert_eq!(dirty, (second..pages).collect::<Vec<_>>());
    // The first section's header: its pages (the second keeps its own header now).
    doc.sections[0].headers.default = Some(vec![field_para("Mục lục, trang ", "PAGE")]);
    let mut changes = BlockChanges::default();
    changes.mark_headers(0);
    let dirty = update_and_compare(&mut inc, &doc, &mut fm, &mut cache, &changes);
    assert_eq!(
        inc.last_update(),
        UpdateKind::Incremental { resynced: true }
    );
    assert!(dirty.contains(&0) && dirty.contains(&(second - 1)));
    // Together with a body edit.
    doc.sections[0].headers.default = Some(vec![field_para("Trang số ", "PAGE")]);
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[60] else {
        unreachable!()
    };
    p.runs[0].content[0] = RunContent::Text(format!("{} y", text(60, 40)));
    let mut changes = BlockChanges::default();
    changes.mark_headers(0);
    changes.splice(0, 60, 1, 1);
    update_and_compare(&mut inc, &doc, &mut fm, &mut cache, &changes);
    assert!(matches!(inc.last_update(), UpdateKind::Incremental { .. }));
    // A header tall enough to push the body: laid out again.
    let tall: Vec<Block> = (0..6).map(|i| field_para(&text(i, 30), "PAGE")).collect();
    doc.sections[0].headers.default = Some(tall);
    let mut changes = BlockChanges::default();
    changes.mark_headers(0);
    update_and_compare(&mut inc, &doc, &mut fm, &mut cache, &changes);
    assert_eq!(inc.last_update(), UpdateKind::Full);
}
