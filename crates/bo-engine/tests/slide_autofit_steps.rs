//! "Shrink text on overflow" (§21.1.2.1.3 normAutofit) takes the reference presentation app's
//! steps: line spacing −10 % then −20 % (lnSpcReduction 10 000 / 20 000), then the font in
//! 7.5 % steps (fontScale 92 500, 85 000, 77 500, …). Measured on the reference (COM, saved
//! XML) with a Title and Content body (66, 143.75 pt, 828 × 342.625 pt; 28 pt, 90 % lines,
//! 10 pt before) holding N one-line paragraphs "Line k":
//!
//! | N | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 16 | 17 | 18 |
//! |---|---|---|---|---|---|---|---|---|---|---|
//! | fontScale | – | – | 92.5 | 85 | 77.5 | 62.5 | 55 | 47.5 | 40 | 32.5 |
//! | lnSpcReduction | – | 10 | 20 | 20 | 20 | 20 | 20 | 20 | 20 | 20 |
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::SlideCommand;
use bo_engine::Engine;
use bo_model::slide::{Autofit, ListStyle, ParaProps, RunProps, ShapeNode, Spacing};
use std::path::PathBuf;

fn run(e: &mut Engine, json: &str) -> bo_engine::slide_edit::SlideEditResult {
    let cmd: SlideCommand = serde_json::from_str(json).unwrap();
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

/// (fontScale, lnSpcReduction) in 1/1000 % our engine writes for `n` lines.
fn fit_for(n: usize) -> (Option<i32>, Option<i32>) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-shapes-presets.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "fit.pptx").unwrap();
    let r = run(
        &mut e,
        r#"{"cmd":"insertTextBox","text":"","rect":{"x":66,"y":143.75,"w":828,"h":342.625}}"#,
    );
    let id = r.selection[0].id;
    let slide = r.slide;
    let with_shape = |e: &mut Engine, f: &mut dyn FnMut(&mut bo_model::slide::Shape)| {
        let p = e.presentation_mut().unwrap();
        for node in &mut p.slides[slide].data.shapes {
            if let ShapeNode::Shape(s) = node {
                if s.nv.id == id {
                    f(s);
                }
            }
        }
    };
    with_shape(&mut e, &mut |s| {
        let tb = s.text.get_or_insert_with(Default::default);
        tb.body_pr.autofit = Some(Autofit::Normal {
            font_scale: None,
            ln_spc_reduction: None,
        });
        let mut ls = ListStyle::default();
        ls.levels[0] = Some(ParaProps {
            ln_spc: Some(Spacing::Percent(90_000)),
            spc_bef: Some(Spacing::Points(1000)),
            def_rpr: Some(RunProps {
                sz: Some(2800),
                ..RunProps::default()
            }),
            ..ParaProps::default()
        });
        tb.list_style = Some(ls);
    });
    let text: Vec<String> = (1..=n).map(|k| format!("Line {k}")).collect();
    run(
        &mut e,
        &format!(
            r#"{{"cmd":"setText","id":{id},"text":{}}}"#,
            serde_json::json!(text.join("\n"))
        ),
    );
    // The body's paragraph and character formatting on every paragraph (the shape is
    // selected), each change fitting the text again.
    run(&mut e, r#"{"cmd":"select","ids":[]}"#);
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"paragraph","values":{"beforeText":0,"firstLine":0,"spaceBefore":10,"spaceAfter":0,"lineExact":null}}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"lineSpacing","value":0.9}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"size","points":28}}"#,
    );
    let mut out = (None, None);
    with_shape(&mut e, &mut |s| {
        if let Some(Autofit::Normal {
            font_scale,
            ln_spc_reduction,
        }) = s.text.as_ref().and_then(|t| t.body_pr.autofit.clone())
        {
            out = (font_scale, ln_spc_reduction);
        }
    });
    out
}

#[test]
fn shrink_on_overflow_takes_the_reference_steps() {
    let all: Vec<(usize, Option<i32>, Option<i32>)> = (4..=18)
        .map(|n| {
            let (s, r) = fit_for(n);
            (n, s, r)
        })
        .collect();
    let fit_for = |n: usize| {
        all.iter()
            .find(|a| a.0 == n)
            .map(|a| (a.1, a.2))
            .unwrap_or_else(|| panic!("{all:?}"))
    };
    // Every value we write is one of the reference's steps.
    for n in 4..=18 {
        let (scale, red) = fit_for(n);
        if let Some(s) = scale {
            assert_eq!((100_000 - s) % 7_500, 0, "{n} lines: fontScale {s}");
            assert_eq!(red, Some(20_000), "{n} lines: the spacing goes first");
        }
        if let Some(r) = red {
            assert!(r == 10_000 || r == 20_000, "{n} lines: lnSpcReduction {r}");
        }
    }
    // Few lines fit as they are; the first overflow only tightens the lines.
    assert_eq!(fit_for(5), (None, None));
    assert_eq!(fit_for(9).0, None);
    // As the reference: 10 → 92.5 %, 12 → 77.5 %, 18 → 32.5 % (± one step).
    for (n, want) in [(10, 92_500), (12, 77_500), (16, 47_500), (18, 32_500)] {
        let got = fit_for(n).0.unwrap_or(100_000);
        assert!(
            (got - want).abs() <= 7_500,
            "{n} lines: {got} vs {want}; all {all:?}"
        );
    }
}
