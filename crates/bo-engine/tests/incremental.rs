//! Incremental layout through the editing bridge: after random edits the engine's
//! layout, display lists, word count and `modified` flag equal those computed from
//! scratch for the edited document (BlueOffice-PROMPT §5.4).

mod support;

use bo_edit::{CharFormat, Pos, Selection};
use bo_engine::nav::Motion;
use bo_engine::{Command, Engine};
use bo_layout::UpdateKind;
use bo_model::{Block, HeaderFooter, Paragraph, Run, RunContent, RunProps};
use proptest::prelude::*;
use support::big_doc;

fn engine(paragraphs: usize) -> Engine {
    let mut doc = big_doc::big_doc(paragraphs);
    let sec = &mut doc.sections[0];
    for (i, b) in sec.blocks.iter_mut().enumerate() {
        if let Block::Paragraph(p) = b {
            match i % 17 {
                3 => p.props.keep_next = Some(true),
                9 => p.runs.push(Run {
                    props: RunProps::default(),
                    content: vec![RunContent::PageBreak],
                }),
                12 => p.runs[0].props.bold = Some(true),
                _ => {}
            }
        }
    }
    sec.headers = HeaderFooter {
        default: Some(vec![Block::Paragraph(Paragraph::new(
            bo_model::ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content: vec![
                    RunContent::Text("Trang ".into()),
                    RunContent::Field {
                        instr: "PAGE".into(),
                        cached: "1".into(),
                    },
                ],
            }],
        ))]),
        ..Default::default()
    };
    let mut e = Engine::with_bundled_fonts();
    e.load(doc, "incremental.docx").expect("load");
    e
}

fn op() -> impl Strategy<Value = Command> {
    prop_oneof![
        3 => (any::<u16>(), any::<bool>()).prop_map(|(p, end)| {
            let pos = Pos::new(usize::from(p), if end { usize::MAX } else { 0 });
            Command::SetSelection { selection: Selection::caret(pos) }
        }),
        2 => prop_oneof![Just(Motion::WordRight), Just(Motion::Down), Just(Motion::Right)]
            .prop_map(|motion| Command::Move { motion, extend: false }),
        1 => Just(Command::Move { motion: Motion::WordRight, extend: true }),
        4 => "[a-zà-ỹ ]{1,6}".prop_map(|text| Command::InsertText { text }),
        2 => Just(Command::SplitParagraph),
        2 => Just(Command::DeleteBackward),
        1 => Just(Command::DeleteForward),
        1 => Just(Command::FormatText { format: CharFormat::Bold }),
        1 => Just(Command::InsertPageBreak),
        1 => Just(Command::Undo),
        1 => Just(Command::Redo),
    ]
}

fn check(
    e: &Engine,
    fresh: &mut Engine,
    original: &bo_model::Document,
    modified: bool,
) -> Result<(), TestCaseError> {
    let doc = e.document().expect("document");
    // Loading lays the document out from scratch.
    fresh.load(doc.clone(), "full.docx").expect("load");
    let full = fresh.layout().expect("layout");
    let got = e.layout().expect("layout");
    prop_assert_eq!(got.pages.len(), full.pages.len());
    for (i, p) in full.pages.iter().enumerate() {
        prop_assert_eq!(
            serde_json::to_string(&got.pages[i]).unwrap(),
            serde_json::to_string(p).unwrap(),
            "layout of page {}",
            i
        );
        prop_assert_eq!(
            e.page_json(i).unwrap(),
            fresh.page_json(i).unwrap(),
            "display list of page {}",
            i
        );
    }
    let info = e.info().expect("info");
    prop_assert_eq!(info.word_count, doc.plain_text().split_whitespace().count());
    prop_assert_eq!(modified, doc != original);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    #[test]
    fn random_commands_match_full_layout(cmds in prop::collection::vec(op(), 1..12)) {
        let mut e = engine(45);
        let mut fresh = Engine::with_bundled_fonts();
        let original = e.document().unwrap().clone();
        for c in cmds {
            let r = e.command(c).expect("command");
            check(&e, &mut fresh, &original, r.modified)?;
        }
    }
}

#[test]
fn typing_in_the_middle_is_incremental_and_redraws_one_page() {
    let mut e = engine(400);
    big_doc::caret_at(&mut e, 200, 10);
    let r = e.command(Command::InsertText { text: "a".into() }).unwrap();
    assert_eq!(
        e.last_layout_update(),
        UpdateKind::Incremental { resynced: true }
    );
    assert_eq!(r.dirty.len(), 1, "{:?}", r.dirty);
    let r = e.command(Command::SplitParagraph).unwrap();
    assert!(matches!(
        e.last_layout_update(),
        UpdateKind::Incremental { .. }
    ));
    assert!(r.dirty.len() < r.info.page_count, "{:?}", r.dirty);
}

#[test]
fn typing_in_a_header_redecorates_without_repaginating() {
    let mut e = engine(400);
    let pages = e.info().unwrap().page_count;
    e.open_header_footer(3, true).unwrap();
    let r = e.command(Command::InsertText { text: "x".into() }).unwrap();
    assert_eq!(
        e.last_layout_update(),
        UpdateKind::Incremental { resynced: true }
    );
    assert_eq!(r.info.page_count, pages);
    // Committing the header (one undo step) is incremental too.
    e.close_header_footer().unwrap();
    assert_eq!(
        e.last_layout_update(),
        UpdateKind::Incremental { resynced: true }
    );
    let mut fresh = Engine::with_bundled_fonts();
    fresh
        .load(e.document().unwrap().clone(), "full.docx")
        .unwrap();
    assert_eq!(
        serde_json::to_string(e.layout().unwrap()).unwrap(),
        serde_json::to_string(fresh.layout().unwrap()).unwrap()
    );
    assert!(r.modified);
}
