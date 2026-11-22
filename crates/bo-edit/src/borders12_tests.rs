//! Borders and Shading dialog, table alt text and table positioning edits (BlueDoc parity
//! round 12): ECMA-376 Part 1, §17.3.1.24 pBdr, §17.4.39 tblBorders, §17.4.67 tcBorders,
//! §17.4.33 shd, §17.4.63 tblCaption, §17.4.58 tblDescription, §17.4.58 tblpPr, §17.4.56
//! tblOverlap. Every edit is one undo step.

use super::*;
use crate::table::{BorderEdges, BorderPen};
use bo_model::{Run, TableFloat};

fn doc(paras: &[&str]) -> Document {
    Document {
        sections: vec![Section::new(
            paras
                .iter()
                .map(|t| {
                    Block::Paragraph(Paragraph::new(
                        ParaProps::default(),
                        vec![Run::text(*t, RunProps::default())],
                    ))
                })
                .collect(),
        )],
        ..Default::default()
    }
}

fn table(s: &Session) -> Table {
    s.doc().sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("a table")
}

#[allow(clippy::unnecessary_wraps)]
fn pen(style: &str, size: u32, color: Option<&str>) -> Option<BorderPen> {
    Some(BorderPen {
        style: style.into(),
        size,
        color: color.map(str::to_owned),
        shadow: false,
        frame: false,
    })
}

/// A 3 × 3 table with the caret in its first cell.
fn with_table() -> Session {
    let mut s = Session::new(doc(&["before", "after"]));
    s.set_selection(Selection::caret(Pos::new(0, 6)));
    s.insert_table(3, 3).unwrap();
    s
}

/// Ids of the first paragraph of each cell, row by row.
fn cell_paras(s: &Session) -> Vec<usize> {
    bo_model::index::body_paragraphs(s.doc())
        .into_iter()
        .enumerate()
        .filter(|(_, path)| path.len() >= 5)
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn paragraph_borders_exact_edges_and_space() {
    let mut s = Session::new(doc(&["one", "two"]));
    let original = s.doc().clone();
    s.set_selection(Selection {
        anchor: Pos::new(0, 0),
        focus: Pos::new(1, 3),
    });
    s.format_paragraphs(&ParaFormat::SetBorders {
        edges: Box::new(BorderEdges {
            top: pen("double", 6, Some("FF0000")),
            bottom: pen("single", 4, None),
            inside_h: pen("dotted", 4, None),
            ..BorderEdges::default()
        }),
        space: [2, 4, 3, 4],
    })
    .unwrap();
    let st = s.format_state();
    let b = st.para_borders.expect("borders");
    let top = b.top.expect("top");
    assert_eq!((top.style.as_str(), top.size, top.space), ("double", 6, 2));
    assert_eq!(top.color.map(Color::to_hex).as_deref(), Some("FF0000"));
    assert!(b.left.is_none() && b.right.is_none());
    assert_eq!(b.bottom.map(|x| x.space), Some(3));
    assert_eq!(b.inside_h.map(|x| x.style), Some("dotted".to_owned()));
    // No edge at all removes pBdr.
    s.format_paragraphs(&ParaFormat::SetBorders {
        edges: Box::default(),
        space: [1, 4, 1, 4],
    })
    .unwrap();
    assert!(s.format_state().para_borders.is_none());
    while s.undo() {}
    assert_eq!(s.doc(), &original);
}

#[test]
fn table_and_cell_borders_and_whole_table_shading() {
    let mut s = with_table();
    let plain = table(&s);
    // Grid on the table: thick outside, thin inside.
    s.edit_table(&TableEdit::SetBorders {
        cells: false,
        edges: Box::new(BorderEdges {
            top: pen("single", 18, None),
            left: pen("single", 18, None),
            bottom: pen("single", 18, None),
            right: pen("single", 18, None),
            inside_h: pen("single", 4, None),
            ..BorderEdges::default()
        }),
    })
    .unwrap();
    let t = table(&s);
    let b = t.props.borders.clone().unwrap();
    assert_eq!(b.top.map(|x| x.size), Some(18));
    assert_eq!(b.inside_h.map(|x| x.size), Some(4));
    // A missing edge is written as `nil` so the style's line does not show.
    assert_eq!(b.inside_v.map(|x| x.style), Some("nil".to_owned()));
    let st = s.table_state().unwrap();
    assert_eq!(st.borders.as_deref(), t.props.borders.as_ref());

    // Select cells (0,0)…(1,1): a box around the four cells, a line between the rows.
    let ids = cell_paras(&s);
    s.set_selection(Selection {
        anchor: Pos::new(ids[0], 0),
        focus: Pos::new(ids[4], 0),
    });
    s.edit_table(&TableEdit::SetBorders {
        cells: true,
        edges: Box::new(BorderEdges {
            top: pen("double", 4, None),
            left: pen("double", 4, None),
            bottom: pen("double", 4, None),
            right: pen("double", 4, None),
            inside_h: pen("dashed", 4, None),
            tl2br: pen("single", 8, None),
            ..BorderEdges::default()
        }),
    })
    .unwrap();
    let t = table(&s);
    let c00 = t.rows[0].cells[0].props.borders.clone().unwrap();
    assert_eq!(c00.top.map(|x| x.style), Some("double".to_owned()));
    assert_eq!(c00.bottom.map(|x| x.style), Some("dashed".to_owned()));
    assert_eq!(c00.right.map(|x| x.style), Some("nil".to_owned()));
    let c11 = t.rows[1].cells[1].props.borders.clone().unwrap();
    assert_eq!(c11.right.map(|x| x.style), Some("double".to_owned()));
    assert_eq!(c11.top.map(|x| x.style), Some("dashed".to_owned()));
    assert!(t.rows[2].cells[2].props.borders.is_none());
    // §17.4.80 tl2br on each selected cell, none elsewhere.
    assert_eq!(
        t.rows[1].cells[0].props.diagonals[0]
            .as_ref()
            .map(|b| b.size),
        Some(8)
    );
    assert!(t.rows[2].cells[0].props.diagonals[0].is_none());

    s.edit_table(&TableEdit::Shading {
        hex: Some("FFF2CC".into()),
        whole: true,
        pattern: None,
    })
    .unwrap();
    let t = table(&s);
    assert!(t.rows.iter().flat_map(|r| &r.cells).all(|c| c
        .props
        .shading
        .map(Color::to_hex)
        .as_deref()
        == Some("FFF2CC")));
    // Three edits, three undo steps back to the plain table.
    for _ in 0..3 {
        assert!(s.undo());
    }
    assert_eq!(table(&s), plain);
}

#[test]
fn table_alt_text_and_position() {
    let mut s = with_table();
    s.edit_table(&TableEdit::AltText {
        title: "Sales".into(),
        description: "Quarterly sales by region".into(),
    })
    .unwrap();
    let st = s.table_state().unwrap();
    assert_eq!(
        (st.caption.as_str(), st.description.as_str()),
        ("Sales", "Quarterly sales by region")
    );
    let f = TableFloat {
        left_from_text: Twips(180),
        right_from_text: Twips(180),
        top_from_text: Twips(0),
        bottom_from_text: Twips(144),
        horz_anchor: Some("margin".into()),
        vert_anchor: Some("page".into()),
        x: Twips(0),
        x_align: Some("center".into()),
        y: Twips(1440),
        y_align: None,
    };
    s.edit_table(&TableEdit::Position {
        float: Box::new(f.clone()),
        overlap: false,
    })
    .unwrap();
    let st = s.table_state().unwrap();
    assert!(st.wrap);
    assert_eq!(st.float.as_deref(), Some(&f));
    assert!(!st.overlap);
    assert_eq!(table(&s).props.overlap.as_deref(), Some("never"));
    // Empty texts remove the elements; overlap allowed again drops tblOverlap.
    s.edit_table(&TableEdit::AltText {
        title: String::new(),
        description: String::new(),
    })
    .unwrap();
    let t = table(&s);
    assert!(t.props.caption.is_none() && t.props.description.is_none());
    assert!(s.undo() && s.undo());
    assert!(table(&s).props.float.is_none());
    assert_eq!(table(&s).props.caption.as_deref(), Some("Sales"));
}

/// The JSON forms the dialogs send.
#[test]
fn json_forms() {
    let e: TableEdit = serde_json::from_value(serde_json::json!({
        "kind": "setBorders", "cells": true,
        "top": { "style": "single", "size": 4, "color": null }, "insideH": null,
    }))
    .unwrap();
    assert!(matches!(e, TableEdit::SetBorders { cells: true, .. }));
    let e: TableEdit = serde_json::from_value(serde_json::json!({
        "kind": "position", "overlap": true,
        "float": { "left_from_text": 180, "right_from_text": 180, "top_from_text": 0,
            "bottom_from_text": 0, "horz_anchor": "text", "vert_anchor": "text", "x": 0,
            "x_align": "left", "y": 0, "y_align": null },
    }))
    .unwrap();
    assert!(matches!(e, TableEdit::Position { overlap: true, .. }));
    let p: ParaFormat = serde_json::from_value(serde_json::json!({
        "kind": "setBorders", "top": { "style": "single", "size": 4 },
    }))
    .unwrap();
    assert!(matches!(
        p,
        ParaFormat::SetBorders {
            space: [1, 4, 1, 4],
            ..
        }
    ));
}
