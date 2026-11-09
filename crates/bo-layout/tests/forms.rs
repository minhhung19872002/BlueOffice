//! Legacy form fields (ECMA-376 Part 1, §17.16.17): check boxes and drop-down lists show their
//! state, results are shaded, a check box of a fixed size has it.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache, LayoutDocument, PageItem};
use bo_model::form::{FormCheckBox, FormDropDown, FormField, FormFieldKind, FormTextInput};
use bo_model::{
    Block, Color, Document, HalfPoints, ParaProps, Paragraph, Run, RunContent, RunProps, Section,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn layout(content: Vec<RunContent>) -> LayoutDocument {
    let d = Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![Run {
                props: RunProps::default(),
                content,
            }],
        ))])],
        ..Default::default()
    };
    layout_document(&d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn field(kind: FormFieldKind) -> RunContent {
    RunContent::FormField(Box::new(FormField::new(kind, None)))
}

fn t(s: &str) -> RunContent {
    RunContent::Text(s.into())
}

const SHADE: Color = Color {
    r: 0xD9,
    g: 0xD9,
    b: 0xD9,
};

fn shaded(l: &LayoutDocument) -> Vec<f32> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            PageItem::Rect { w, color, .. } if *color == SHADE => Some(*w),
            _ => None,
        })
        .collect()
}

#[test]
fn states_show_and_hidden_results_do_not() {
    let l = layout(vec![
        t("A "),
        field(FormFieldKind::CheckBox(FormCheckBox {
            size: None,
            default: false,
            checked: Some(true),
        })),
        // A result stored inside a check box is not shown.
        t("hidden"),
        RunContent::FormEnd,
        t(" B "),
        field(FormFieldKind::DropDown(FormDropDown {
            result: Some(1),
            default: None,
            entries: vec!["Một".into(), "Hai".into()],
        })),
        RunContent::FormEnd,
        t(" C "),
        field(FormFieldKind::Text(FormTextInput::default())),
        t("Lan"),
        RunContent::FormEnd,
        t(" D"),
    ]);
    assert_eq!(l.page_text(0), "A \u{2612} B Hai C Lan D");
    // Three shaded results: the box, the list and the text.
    assert_eq!(shaded(&l).len(), 3);
}

#[test]
fn check_box_of_a_fixed_size() {
    let frag_size = |size: Option<u32>| {
        let l = layout(vec![
            field(FormFieldKind::CheckBox(FormCheckBox {
                size,
                ..FormCheckBox::default()
            })),
            RunContent::FormEnd,
        ]);
        l.pages[0].lines[0]
            .fragments
            .iter()
            .find(|f| f.text.as_str() == "\u{2610}")
            .map(|f| f.size)
            .unwrap()
    };
    // sizeAuto: the run's 12 pt; size 40 half-points: 20 pt.
    assert!((frag_size(None) - 12.0).abs() < 0.01);
    assert!((frag_size(Some(40)) - 20.0).abs() < 0.01);
}
