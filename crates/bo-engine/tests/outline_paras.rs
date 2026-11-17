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
