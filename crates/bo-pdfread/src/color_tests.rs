//! Colour-space failure cases measured on the reference reader (BlueReader round 4).
use super::tests::build_pdf;
use super::*;
use bo_render::{Command, Paint, Rgba};

fn fills(content: &str, spaces: &str, extra: &[&str]) -> Vec<Rgba> {
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
         /Resources << /ColorSpace << {spaces} >> >> >>"
    );
    let content = format!(
        "<< /Length {} >>\nstream\n{content}\nendstream",
        content.len()
    );
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        page,
        content,
    ];
    objs.extend(extra.iter().map(|s| (*s).to_owned()));
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    let mut doc = PdfDocument::open(build_pdf(&refs), standard_fonts()).expect("opens");
    let page = doc.render(0, RenderOptions::default()).expect("renders");
    page.list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::FillRect { color, .. }
            | Command::Path {
                fill: Some(Paint::Solid { color }),
                ..
            } => Some(*color),
            _ => None,
        })
        .collect()
}

fn ps(range: &str, code: &str) -> String {
    format!(
        "<< /FunctionType 4 /Domain [0 1] /Range [{range}] /Length {} >>\nstream\n{code}\nendstream",
        code.len()
    )
}

#[test]
fn postscript_tint_with_the_wrong_result_count_paints_zeros() {
    // misc.pdf: three results for a CMYK alternate → all components 0 (white), not light cyan.
    let c = fills(
        "/S0 cs 0.8 scn 0 0 10 10 re f /S1 cs 0.8 scn 20 0 10 10 re f \
         /S2 cs 0.8 scn 40 0 10 10 re f",
        "/S0 [/Separation /A /DeviceCMYK 5 0 R] /S1 [/Separation /B /DeviceRGB 6 0 R] \
         /S2 [/Separation /C /DeviceRGB 7 0 R]",
        &[
            &ps(
                "0 1 0 1 0 1 0 1",
                "{ dup 0.1 mul exch dup 0.9 mul exch 0 mul exch 0 mul }",
            ),
            &ps("0 1 0 1 0 1", "{ 0.5 }"),
            &ps("0 1 0 1 0 1", "{ 0 0 }"),
        ],
    );
    assert_eq!(c.len(), 3);
    assert_eq!((c[0].0, c[0].1, c[0].2), (255, 255, 255));
    assert_eq!((c[1].0, c[1].1, c[1].2), (0, 0, 0));
    assert_eq!((c[2].0, c[2].1, c[2].2), (204, 0, 0));
}

#[test]
fn unusable_tint_transform_keeps_the_previous_space() {
    // Missing function / two outputs for RGB: `cs` is ignored, `0.8 scn` sets grey 0.8.
    let c = fills(
        "0.85 g /S0 cs 0.8 scn 0 0 10 10 re f 0.85 g /S1 cs 0.8 scn 20 0 10 10 re f",
        "/S0 [/Separation /I /DeviceRGB] \
         /S1 [/Separation /H /DeviceRGB << /FunctionType 2 /Domain [0 1] /C0 [1 1] /C1 [0 0.5] /N 1 >>]",
        &[],
    );
    assert_eq!(c.len(), 2);
    for k in c {
        assert_eq!((k.0, k.1, k.2), (204, 204, 204));
    }
}

#[test]
fn unknown_alternate_space_paints_nothing() {
    let c = fills(
        "/S0 cs 0.8 scn 0 0 10 10 re f 0 g 20 0 10 10 re f",
        "/S0 [/Separation /K /Foo << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >>]",
        &[],
    );
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].0, c[0].1, c[0].2), (0, 0, 0));
}

#[test]
fn text_colour_override_reaches_outlined_text() {
    // Night Mode: stroked (Tr 1) and filled-and-stroked (Tr 2) text turn grey like other text.
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
                /Resources << /Font << /F1 5 0 R >> >> >>";
    let body = "BT /F1 40 Tf 1 Tr 1 0 0 RG 10 50 Td (A) Tj 2 Tr 0 0.6 0 rg (B) Tj ET";
    let content = format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len());
    let pdf = build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        page,
        &content,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]);
    let mut doc = PdfDocument::open(pdf, standard_fonts()).expect("opens");
    let grey = Rgba(127, 127, 127, 255);
    let page = doc
        .render_extra(
            0,
            RenderOptions::default(),
            RenderExtra {
                text_color: Some(grey),
                ..RenderExtra::default()
            },
        )
        .expect("renders");
    let mut strokes = 0;
    for c in &page.list.commands {
        if let Command::Path { fill, stroke, .. } = c {
            if let Some(s) = stroke {
                assert_eq!(s.color, grey);
                strokes += 1;
            }
            if let Some(Paint::Solid { color }) = fill {
                assert_eq!(*color, grey);
            }
        }
    }
    assert!(strokes >= 1);
}

#[test]
fn overprint_simulation_multiplies_inks() {
    let body = "1 0 0 0 k 0 0 50 50 re f q /OP1 gs 0 1 0 0 k 25 25 50 50 re f Q \
                q /OP1 gs 1 0 0 rg 100 0 50 50 re f Q";
    let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
                /Resources << /ExtGState << /OP1 << /OP true /OPM 1 >> >> >> >>";
    let content = format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len());
    let pdf = build_pdf(&[
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        page,
        &content,
    ]);
    let mut doc = PdfDocument::open(pdf, standard_fonts()).expect("opens");
    let groups = |doc: &mut PdfDocument, overprint: bool| {
        let extra = RenderExtra {
            overprint,
            ..RenderExtra::default()
        };
        let page = doc
            .render_extra(0, RenderOptions::default(), extra)
            .expect("renders");
        page.list
            .commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Command::PushGroup {
                        blend: Some(bo_render::BlendMode::Multiply),
                        ..
                    }
                )
            })
            .count()
    };
    // Print What: form fields only leaves the page content out.
    let extra = RenderExtra {
        printing: true,
        print_what: PrintWhat::FormFields,
        ..RenderExtra::default()
    };
    let page = doc
        .render_extra(0, RenderOptions::default(), extra)
        .expect("renders");
    assert_eq!(page.list.commands.len(), 0);
    // Only the CMYK fill under /OP is multiplied; the RGB one is not an ink.
    assert_eq!(groups(&mut doc, true), 1);
    assert_eq!(groups(&mut doc, false), 0);
}
