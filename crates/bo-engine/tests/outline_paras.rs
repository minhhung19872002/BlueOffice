//! Outline View paragraphs: automatic numbers (ECMA-376 Part 1, §21.1.2.4.1 buAutoNum) and
//! run emphasis (§21.1.2.3.9 rPr b / i / u) of the body placeholder.
use bo_engine::slide_edit::outline::slide_outline_paras;
use bo_engine::Engine;

#[test]
fn numbers_and_italic_runs_of_the_numbered_slide() {
    let name = "slide-action-buttons.pptx";
    let path = format!(
        "{}/../../tests/corpus/slide/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(path).unwrap(), name).unwrap();
    let pres = e.presentation().unwrap();
    let paras = pres
        .slides
        .iter()
        .map(slide_outline_paras)
        .find(|p| p.iter().any(|x| x.number.is_some()))
        .expect("a numbered slide");
    let numbers: Vec<_> = paras.iter().filter_map(|p| p.number.clone()).collect();
    assert_eq!(numbers, ["1.", "2.", "a)", "3."]);
    assert!(paras
        .iter()
        .flat_map(|p| &p.runs)
        .any(|r| r.italic && !r.text.trim().is_empty()));
}

/// Numbering set on the layout's body placeholder (not on the paragraphs) shows in the
/// outline, as the reference's ("1.One", round 19); the paragraph's own numbering keeps
/// its hanging indent.
#[test]
fn numbers_inherited_from_the_layout() {
    use bo_engine::slide_edit::outline::outline_paras;
    use bo_model::slide::{Bullet, ListStyle, ParaProps, ShapeNode};
    let name = "slide-action-buttons.pptx";
    let path = format!(
        "{}/../../tests/corpus/slide/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(path).unwrap(), name).unwrap();
    let pres = e.presentation_mut().unwrap();
    // The numbered slide's own numbers come through with their hanging indent.
    let k = (0..pres.slides.len())
        .find(|&i| outline_paras(pres, i).iter().any(|p| p.number.is_some()))
        .expect("a numbered slide");
    let own = outline_paras(pres, k);
    assert!(own
        .iter()
        .filter(|p| p.number.is_some())
        .all(|p| p.hang > 0.0));
    // A slide with plain body text: number its layout's body placeholder.
    let plain = |pres: &bo_model::slide::Presentation, i: usize| {
        let ps = outline_paras(pres, i);
        ps.iter().filter(|p| !p.runs.is_empty()).count() >= 2
            && ps.iter().all(|p| p.number.is_none())
    };
    let j = (0..pres.slides.len())
        .find(|&i| i != k && plain(pres, i))
        .expect("a slide with body paragraphs");
    let layout = pres.slides[j].layout;
    for n in &mut pres.layouts[layout].data.shapes {
        let ShapeNode::Shape(sh) = n else { continue };
        let Some(ph) = &sh.nv.placeholder else {
            continue;
        };
        if matches!(ph.kind.as_deref(), None | Some("body")) {
            let tb = sh.text.get_or_insert_with(Default::default);
            let ls = tb.list_style.get_or_insert_with(ListStyle::default);
            ls.levels[0] = Some(ParaProps {
                bullet: Some(Bullet::AutoNum {
                    scheme: "arabicPeriod".into(),
                    start_at: None,
                }),
                ..ParaProps::default()
            });
        }
    }
    let got: Vec<_> = outline_paras(pres, j)
        .into_iter()
        .filter(|p| p.level == 0 && !p.runs.is_empty())
        .filter_map(|p| p.number)
        .collect();
    assert_eq!(got.first().map(String::as_str), Some("1."), "{got:?}");
}
