//! A style definition change reaches the laid-out text through a shared
//! LayoutCache (ECMA-376 Part 1, §17.7.2 style hierarchy): paragraphs only name
//! their styles, so the cache key must cover the definitions they resolve to.
#![allow(clippy::unwrap_used)]
use bo_layout::{layout_document, LayoutCache};
use bo_model::*;

#[test]
fn style_run_props_change_is_not_served_from_the_cache() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = bo_text::FontManager::new();
    fm.add_fonts_from_dir(&dir);
    let mut doc = Document::default();
    let mut st = Style::new("H", "H", StyleKind::Paragraph);
    st.rpr.size = Some(HalfPoints(24));
    doc.styles.styles.push(st);
    let mut p = Paragraph::default();
    p.props.style = Some("H".into());
    p.runs.push(Run {
        content: vec![RunContent::Text("Tiêu đề ".repeat(30))],
        ..Default::default()
    });
    doc.sections.push(Section::new(vec![Block::Paragraph(p)]));
    let mut cache = LayoutCache::default();
    let a = layout_document(&doc, &mut fm, &mut cache).unwrap();
    doc.styles.styles[0].rpr.size = Some(HalfPoints(56));
    let b = layout_document(&doc, &mut fm, &mut cache).unwrap();
    let fresh = layout_document(&doc, &mut fm, &mut LayoutCache::default()).unwrap();
    let n = |d: &bo_layout::LayoutDocument| d.pages[0].lines.len();
    assert_eq!((n(&a), n(&b)), (3, n(&fresh)));

    // A character style on the run, then its definition changes.
    let mut c = Style::new("C", "C", StyleKind::Character);
    c.rpr.size = Some(HalfPoints(24));
    doc.styles.styles.push(c);
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] {
        p.props.style = None;
        p.runs[0].props.style = Some("C".into());
    }
    let small = layout_document(&doc, &mut fm, &mut cache).unwrap();
    doc.styles.styles[1].rpr.size = Some(HalfPoints(56));
    let big = layout_document(&doc, &mut fm, &mut cache).unwrap();
    let fresh = layout_document(&doc, &mut fm, &mut LayoutCache::default()).unwrap();
    assert!(n(&big) > n(&small));
    assert_eq!(n(&big), n(&fresh));
}
