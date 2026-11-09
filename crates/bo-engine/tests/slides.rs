//! BlueSlide through the engine facade: open the corpus decks, snapshot a
//! compact form of every slide's display list (insta), export PDF and PNG.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_render::{Command, Paint, PathSeg};
use std::fmt::Write as _;
use std::path::PathBuf;

fn corpus() -> Vec<(String, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide");
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pptx"))
        .map(|p| {
            (
                p.file_stem().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

fn rgba(c: bo_render::Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.0, c.1, c.2, c.3)
}

fn bbox(segs: &[PathSeg]) -> String {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut add = |x: f32, y: f32| b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    for s in segs {
        match *s {
            PathSeg::M(x, y) | PathSeg::L(x, y) => add(x, y),
            PathSeg::Q(a, c, x, y) => {
                add(a, c);
                add(x, y);
            }
            PathSeg::C(a, c, d, e, x, y) => {
                add(a, c);
                add(d, e);
                add(x, y);
            }
            PathSeg::Z => {}
        }
    }
    format!("[{:.1} {:.1} {:.1} {:.1}]", b[0], b[1], b[2], b[3])
}

fn paint(p: &Paint) -> String {
    match p {
        Paint::Solid { color } => rgba(*color),
        Paint::Linear { stops, .. } => format!("linear({})", stops.len()),
        Paint::Radial { stops, .. } => format!("radial({})", stops.len()),
    }
}

/// One line per command, coordinates rounded to 0.1 pt.
fn summary(cmds: &[Command]) -> String {
    let mut s = String::new();
    let mut depth = 0usize;
    for c in cmds {
        if matches!(
            c,
            Command::PopClip | Command::PopTransform | Command::PopOpacity
        ) {
            depth = depth.saturating_sub(1);
        }
        s.push_str(&"  ".repeat(depth));
        match c {
            Command::FillRect { x, y, w, h, color } => {
                let _ = writeln!(s, "rect {x:.1},{y:.1} {w:.1}x{h:.1} {}", rgba(*color));
            }
            Command::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => {
                let _ = writeln!(s, "line {x1:.1},{y1:.1} → {x2:.1},{y2:.1} w{width:.2}");
            }
            Command::Glyphs {
                size,
                x,
                y,
                text,
                color,
                ..
            } => {
                let _ = writeln!(
                    s,
                    "text {size:.1}pt {x:.1},{y:.1} {} {text:?}",
                    rgba(*color)
                );
            }
            Command::PushClip { x, y, w, h } => {
                let _ = writeln!(s, "clip {x:.1},{y:.1} {w:.1}x{h:.1}");
            }
            Command::PopClip => s.push_str("pop clip\n"),
            Command::Image {
                x,
                y,
                w,
                h,
                image,
                crop,
                ..
            } => {
                let _ = writeln!(s, "image {image} {x:.1},{y:.1} {w:.1}x{h:.1} crop={crop:?}");
            }
            Command::Path { segs, fill, stroke } => {
                let _ = write!(s, "path n={} {}", segs.len(), bbox(segs));
                if let Some(f) = fill {
                    let _ = write!(s, " fill={}", paint(f));
                }
                if let Some(st) = stroke {
                    let _ = write!(
                        s,
                        " stroke={} w{:.2}{}",
                        rgba(st.color),
                        st.width,
                        if st.dash.is_empty() { "" } else { " dashed" }
                    );
                }
                s.push('\n');
            }
            Command::PushTransform { m } => {
                let _ = writeln!(
                    s,
                    "transform [{:.3} {:.3} {:.3} {:.3} {:.1} {:.1}]",
                    m[0], m[1], m[2], m[3], m[4], m[5]
                );
            }
            Command::PopTransform => s.push_str("pop transform\n"),
            Command::PushClipPath { segs } => {
                let _ = writeln!(s, "clip path n={} {}", segs.len(), bbox(segs));
            }
            Command::PushOpacity { alpha } => {
                let _ = writeln!(s, "opacity {alpha:.2}");
            }
            Command::PopOpacity => s.push_str("pop opacity\n"),
            Command::BlurPath {
                segs,
                color,
                radius,
                ..
            } => {
                let _ = writeln!(
                    s,
                    "blur path n={} {} {} r{radius:.1}",
                    segs.len(),
                    bbox(segs),
                    rgba(*color)
                );
            }
        }
        if matches!(
            c,
            Command::PushClip { .. }
                | Command::PushTransform { .. }
                | Command::PushClipPath { .. }
                | Command::PushOpacity { .. }
        ) {
            depth += 1;
        }
    }
    s
}

#[test]
fn slide_display_list_snapshots() {
    let mut e = Engine::with_bundled_fonts();
    for (name, bytes) in corpus() {
        let info = e.open(&bytes, &format!("{name}.pptx")).unwrap();
        assert_eq!(info.kind, "slide");
        assert_eq!(info.page_count, e.slide_count());
        // The transitions deck repeats one blank slide per effect (a transition draws nothing on
        // the slide): its first slides stand for the rest.
        let shown = if name == "slide-edit-transitions-all" || name == "slide-edit-animations-all" {
            e.slide_count().min(3)
        } else {
            e.slide_count()
        };
        for i in 0..shown {
            let page = e.slide(i).unwrap();
            assert!((page.width - 960.0).abs() < 0.01 && (page.height - 540.0).abs() < 0.01);
            insta::assert_snapshot!(format!("{name}-{}", i + 1), summary(&page.commands));
        }
    }
}

#[test]
fn slide_json_is_deterministic_and_balanced() {
    for (name, bytes) in corpus() {
        let mut a = Engine::with_bundled_fonts();
        let mut b = Engine::with_bundled_fonts();
        a.open(&bytes, "a.pptx").unwrap();
        b.open(&bytes, "b.pptx").unwrap();
        for i in 0..a.slide_count() {
            assert_eq!(a.slide_json(i).unwrap(), b.slide_json(i).unwrap(), "{name}");
            let mut depth = 0i32;
            for c in &a.slide(i).unwrap().commands {
                match c {
                    Command::PushClip { .. }
                    | Command::PushClipPath { .. }
                    | Command::PushTransform { .. }
                    | Command::PushOpacity { .. } => depth += 1,
                    Command::PopClip | Command::PopTransform | Command::PopOpacity => depth -= 1,
                    _ => {}
                }
                assert!(depth >= 0, "{name}: unbalanced pop");
            }
            assert_eq!(depth, 0, "{name}: unbalanced push");
        }
    }
}

#[test]
fn slide_exports_pdf_png_and_saves() {
    let decks = corpus();
    let (_, bytes) = decks
        .iter()
        .find(|(n, _)| n == "slide-images-crop")
        .unwrap();
    let mut e = Engine::with_bundled_fonts();
    let info = e.open(bytes, "deck.pptx").unwrap();
    assert!(e.image_bytes("/ppt/media/image1.png").is_some());
    let pdf = e.export_pdf().unwrap();
    let s = String::from_utf8_lossy(&pdf);
    let pages = s.matches("/Type /Page").count() - s.matches("/Type /Pages").count();
    assert_eq!(pages, info.page_count);
    assert!(s.contains("/Subtype /Image"));
    let png = e.render_png(0, 0.5).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    let saved = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&saved, "saved.pptx").unwrap();
    assert_eq!(e2.slide_json(0).unwrap(), e.slide_json(0).unwrap());
    // Thumbnails, notes.
    assert!(e
        .slide_thumbnail_json(0, 160.0)
        .unwrap()
        .contains("pushTransform"));
    let (_, notes) = decks
        .iter()
        .find(|(n, _)| n == "slide-notes-basic")
        .unwrap();
    e.open(notes, "n.pptx").unwrap();
    assert!(e.slide_notes(0).unwrap().contains("Chào mừng"));
    assert!(e.slide_notes(9).is_err());
}

#[test]
fn edit_and_refresh_slide() {
    let decks = corpus();
    let (_, bytes) = decks
        .iter()
        .find(|(n, _)| n == "slide-text-bullets")
        .unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, "d.pptx").unwrap();
    let before = e.slide_json(0).unwrap();
    let pres = e.presentation_mut().unwrap();
    if let bo_model::slide::ShapeNode::Shape(s) = &mut pres.slides[0].data.shapes[0] {
        if let Some(bo_model::slide::TextRun::Run { text, .. }) = s
            .text
            .as_mut()
            .and_then(|t| t.paragraphs.first_mut())
            .and_then(|p| p.runs.first_mut())
        {
            *text = "Tiêu đề đã sửa".into();
        }
    }
    e.refresh_slide(0).unwrap();
    let after = e.slide_json(0).unwrap();
    assert_ne!(before, after);
    assert!(after.contains("sửa"));
}

/// [MS-ODRAWXML] a14:m: a text box whose paragraph holds an Office Math zone, inside
/// mc:AlternateContent (Choice Requires="a14", Fallback text): the equation is laid out and
/// drawn (glyphs + the fraction rule), not the fallback.
#[test]
fn slide_equations_are_drawn() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-anim-fade-appear.pptx");
    let bytes = std::fs::read(path).unwrap();
    let mut pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let slide =
        String::from_utf8(pkg.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap();
    let shape = r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" Requires="a14"><p:sp><p:nvSpPr><p:cNvPr id="90" name="Equation"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="914400" y="3200400"/><a:ext cx="3657600" cy="1371600"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a14:m><m:oMathPara xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:oMath><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f></m:oMath></m:oMathPara></a14:m><a:endParaRPr lang="vi-VN" sz="2800"/></a:p></p:txBody></p:sp></mc:Choice><mc:Fallback><p:sp><p:nvSpPr><p:cNvPr id="90" name="Equation"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="914400" y="3200400"/><a:ext cx="3657600" cy="1371600"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>FALLBACKTEXT</a:t></a:r></a:p></p:txBody></p:sp></mc:Fallback></mc:AlternateContent>"#;
    let slide = slide.replacen("</p:spTree>", &format!("{shape}</p:spTree>"), 1);
    pkg.put_part("/ppt/slides/slide1.xml", slide.into_bytes());
    let deck = pkg.write().unwrap();
    let mut e = bo_engine::Engine::with_bundled_fonts();
    e.open(&deck, "eq.pptx").unwrap();
    let json = e.page_json(0).unwrap();
    assert!(!json.contains("FALLBACKTEXT"), "the fallback is not drawn");
    // The numerator and denominator glyph runs (math italic a, b) and a fraction rule.
    assert!(
        json.contains("\"text\":\"𝑎\"") || json.contains("\"text\":\"a\""),
        "numerator"
    );
    assert!(
        json.contains("\"text\":\"𝑏\"") || json.contains("\"text\":\"b\""),
        "denominator"
    );
    assert!(
        json.contains("\"Line\"") || json.contains("\"line\""),
        "fraction rule"
    );
    // Saved as it was read.
    let saved = e.save_pptx().unwrap();
    let again = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    let xml =
        String::from_utf8(again.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap();
    assert!(xml.contains("<a14:m>") && xml.contains("m:oMathPara"));
}
