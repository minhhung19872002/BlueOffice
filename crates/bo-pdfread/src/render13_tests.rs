//! BlueReader round 13: rendering fidelity fixes found by comparing a generated corpus with
//! the reference reader (uncoloured tiling patterns, axial `/Extend`, greyscale JPEG, CJK
//! fonts that are not embedded, transparency group alpha, even-odd fills).
use super::tests::build_pdf;
use super::*;
use bo_render::{Command, Paint, PathSeg};

fn xstream(dict: &str, body: &str) -> String {
    format!(
        "<< {dict} /Length {} >>\nstream\n{body}\nendstream",
        body.len()
    )
}

/// One 200 × 100 page with `content` and the resources `res`; `extra` objects start at 5.
fn page_pdf(content: &str, res: &str, extra: &[&str]) -> Vec<u8> {
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R /Resources << {res} >> >>"
    );
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        page,
        xstream("", content),
    ];
    objs.extend(extra.iter().map(|s| (*s).to_owned()));
    let refs: Vec<&str> = objs.iter().map(String::as_str).collect();
    build_pdf(&refs)
}

fn render(bytes: Vec<u8>) -> Page {
    let mut doc = PdfDocument::open(bytes, standard_fonts()).expect("opens");
    doc.render(0, RenderOptions::default()).expect("renders")
}

fn solid_fills(p: &Page) -> Vec<Rgba> {
    p.list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Path {
                fill: Some(Paint::Solid { color }),
                ..
            }
            | Command::FillRect { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

#[test]
fn uncoloured_pattern_paints_in_the_given_colour_and_the_page_goes_on() {
    let p = render(page_pdf(
        "/CsP cs 0.8 0.1 0.1 /P2 scn 0 0 50 50 re f 0 0 1 rg 60 0 50 50 re f",
        "/Pattern << /P2 5 0 R >> /ColorSpace << /CsP [/Pattern /DeviceRGB] >>",
        &[&xstream(
            "/Type /Pattern /PatternType 1 /PaintType 2 /TilingType 1 /BBox [0 0 20 20] \
             /XStep 20 /YStep 20 /Resources << >>",
            "10 10 m 16 10 l 13 16 l h f",
        )],
    ));
    let fills = solid_fills(&p);
    // The cells in the scn colour, nine of them, then the blue square after the pattern.
    let red = Rgba(204, 26, 26, 255);
    assert!(
        fills.iter().filter(|c| **c == red).count() >= 9,
        "{fills:?}"
    );
    assert!(!fills.contains(&Rgba(200, 200, 200, 255)), "{fills:?}");
    assert_eq!(fills.last(), Some(&Rgba(0, 0, 255, 255)));
}

#[test]
fn axial_shading_without_extend_paints_only_between_its_ends() {
    let sh = "/Shading << /S << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [50 0 150 0] \
              /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> EXT >> >>";
    let band = |ext: &str| -> (f32, f32) {
        let p = render(page_pdf("/S sh", &sh.replace("EXT", ext), &[]));
        let segs = p
            .list
            .commands
            .iter()
            .find_map(|c| match c {
                Command::Path {
                    segs,
                    fill: Some(Paint::Linear { .. }),
                    ..
                } => Some(segs.clone()),
                _ => None,
            })
            .expect("gradient");
        let xs: Vec<f32> = segs
            .iter()
            .filter_map(|s| match s {
                PathSeg::M(x, _) | PathSeg::L(x, _) => Some(*x),
                _ => None,
            })
            .collect();
        (
            xs.iter().copied().fold(f32::INFINITY, f32::min),
            xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    };
    let (l, r) = band("");
    assert!(
        (l - 50.0).abs() < 0.01 && (r - 150.0).abs() < 0.01,
        "{l} {r}"
    );
    let (l, r) = band("/Extend [true false]");
    assert!(l < 0.0 && (r - 150.0).abs() < 0.01, "{l} {r}");
    let (l, r) = band("/Extend [true true]");
    assert!(l <= -1.0 && r >= 201.0, "{l} {r}");
}

/// An 8 × 8 greyscale JPEG (generated): each row 0, 32, …, 224.
const GREY_JPEG: &str = "ffd8ffe000104a46494600010100000100010000ffdb0043000302020302020303030304030304050805050404050a070706080c0a0c0c0b0a0b0b0d0e12100d0e110e0b0b1016101113141515150c0f171816141812141514ffc0000b080008000801011100ffc4001f0000010501010101010100000000000000000102030405060708090a0bffc400b5100002010303020403050504040000017d01020300041105122131410613516107227114328191a1082342b1c11552d1f02433627282090a161718191a25262728292a3435363738393a434445464748494a535455565758595a636465666768696a737475767778797a838485868788898a92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9faffda0008010100003f00e53fe09b7ff30bff0080d7ffd9";

#[test]
fn greyscale_jpeg_keeps_one_sample_per_pixel() {
    let data = format!("{GREY_JPEG}>");
    let p = render(page_pdf(
        "q 80 0 0 80 10 10 cm /Im Do Q",
        "/XObject << /Im 5 0 R >>",
        &[&xstream(
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Filter [/ASCIIHexDecode /DCTDecode]",
            &data,
        )],
    ));
    let img = p.images.values().next().expect("decoded");
    assert_eq!((img.width, img.height), (8, 8));
    // Every row rises from black to light grey (before: three squeezed copies and black).
    for row in img.pixels.chunks_exact(32) {
        let first = i32::from(row[0]);
        let last = i32::from(row[28]);
        assert!(first < 24 && (200..=240).contains(&last), "{first} {last}");
    }
}

#[test]
fn not_embedded_cjk_font_with_a_unicode_cmap_reads_and_measures() {
    let p = render(page_pdf(
        "BT /C 10 Tf 10 50 Td <4E2D6587002000410042> Tj ET",
        "/Font << /C 5 0 R >>",
        &[
            "<< /Type /Font /Subtype /Type0 /BaseFont /STSong-Light /Encoding /UniGB-UCS2-H \
             /DescendantFonts [6 0 R] >>",
            "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /STSong-Light /CIDSystemInfo \
             << /Registry (Adobe) /Ordering (GB1) /Supplement 4 >> /DW 1000 /W [1 [207] 34 [600 600]] >>",
        ],
    ));
    let text: String = p.chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "中文 AB");
    // Ideographs advance by /DW, ASCII by the widths of CIDs 1–95.
    let x: Vec<f32> = p.chars.iter().map(|c| c.origin[0]).collect();
    assert!((x[1] - x[0] - 10.0).abs() < 0.01, "{x:?}");
    assert!((x[3] - x[2] - 2.07).abs() < 0.01, "{x:?}");
    assert!((x[4] - x[3] - 6.0).abs() < 0.01, "{x:?}");
}

#[test]
fn transparency_group_alpha_applies_to_the_group() {
    let p = render(page_pdf(
        "/H gs /G Do",
        "/ExtGState << /H << /ca 0.5 >> >> /XObject << /G 5 0 R >>",
        &[&xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Group << /S /Transparency >> \
             /Resources << /ExtGState << /I << /ca 0.6 >> >> >>",
            "1 0 0 rg 0 0 10 10 re f /I gs 0 0 1 rg 20 0 10 10 re f",
        )],
    ));
    let c = &p.list.commands;
    assert!(
        matches!(c.first(), Some(Command::PushOpacity { alpha }) if (alpha - 0.5).abs() < 1e-6),
        "{c:?}"
    );
    assert!(matches!(c.last(), Some(Command::PopOpacity)), "{c:?}");
    // Inside: full alpha, then 0.6 (not 0.5 replaced by 0.6 with nothing for the group).
    let fills = solid_fills(&p);
    assert_eq!(fills, vec![Rgba(255, 0, 0, 255), Rgba(0, 0, 255, 153)]);
    // A form that is not a group keeps the alpha per object.
    let p = render(page_pdf(
        "/H gs /G Do",
        "/ExtGState << /H << /ca 0.5 >> >> /XObject << /G 5 0 R >>",
        &[&xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
            "1 0 0 rg 0 0 10 10 re f",
        )],
    ));
    assert_eq!(solid_fills(&p), vec![Rgba(255, 0, 0, 128)]);
}

#[test]
fn even_odd_fill_and_clip_leave_holes() {
    let p = render(page_pdf(
        "0 0 100 100 re 25 25 50 50 re f* 0 0 200 100 re 120 20 40 40 re W* n 0 g 0 0 200 100 re f",
        "",
        &[],
    ));
    let paths: Vec<&Vec<PathSeg>> = p
        .list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Path { segs, .. } => Some(segs),
            _ => None,
        })
        .collect();
    // The inner square of the fill runs the other way round (nonzero then leaves it empty).
    let area = |s: &[PathSeg]| {
        let pts: Vec<(f32, f32)> = s
            .iter()
            .filter_map(|p| match p {
                PathSeg::M(x, y) | PathSeg::L(x, y) => Some((*x, *y)),
                _ => None,
            })
            .collect();
        (0..pts.len())
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum::<f32>()
    };
    let fill = paths[0];
    let split = fill
        .iter()
        .skip(1)
        .position(|s| matches!(s, PathSeg::M(..)))
        .expect("two subpaths")
        + 1;
    assert!(
        area(&fill[..split]) * area(&fill[split..]) < 0.0,
        "{fill:?}"
    );
    let clip = p
        .list
        .commands
        .iter()
        .find_map(|c| match c {
            Command::PushClipPath { segs } => Some(segs),
            _ => None,
        })
        .expect("clip");
    let split = clip
        .iter()
        .skip(1)
        .position(|s| matches!(s, PathSeg::M(..)))
        .expect("two subpaths")
        + 1;
    assert!(
        area(&clip[..split]) * area(&clip[split..]) < 0.0,
        "{clip:?}"
    );
}

#[test]
fn cal_gray_and_cal_rgb_follow_their_gamma_and_matrix() {
    let p = render(page_pdf(
        "/CG cs 0.1 sc 0 0 10 10 re f /CR cs 1 1 1 sc 20 0 10 10 re f 0.5 0.5 0.5 sc 40 0 10 10 re f",
        "/ColorSpace << /CG [/CalGray << /WhitePoint [0.9505 1 1.089] /Gamma 2.2 >>] \
         /CR [/CalRGB << /WhitePoint [0.9505 1 1.089] /Gamma [2.2 2.2 2.2] \
         /Matrix [0.4124 0.2126 0.0193 0.3576 0.7152 0.1192 0.1805 0.0722 0.9505] >>] >>",
        &[],
    ));
    let fills = solid_fills(&p);
    // 0.1 ^ 2.2 in sRGB is 19 of 255 (DeviceGray would give 26).
    assert!((17..=21).contains(&fills[0].0), "{fills:?}");
    // sRGB primaries at D65: white stays white, mid grey becomes the gamma 2.2 grey.
    assert!(
        fills[1].0 >= 253 && fills[1].1 >= 253 && fills[1].2 >= 253,
        "{fills:?}"
    );
    let g = i32::from(fills[2].0);
    assert!((g - 124).abs() <= 4, "{fills:?}");
}

/// Comments of every kind BlueReader makes, saved to `$BO_R13_OUT` for opening in another
/// reader (annotation interop check, round 13): `cargo test -p bo-pdfread
/// write_comment_sample -- --ignored`.
#[test]
#[ignore = "writes a sample file for a manual check"]
fn write_comment_sample() {
    use crate::markup::{Markup, MarkupKind};
    let Ok(out) = std::env::var("BO_R13_OUT") else {
        return;
    };
    let content = "BT /F 14 Tf 40 760 Td (Highlighted text, underlined text, struck text.) Tj ET \
                   BT /F 14 Tf 40 730 Td (Second line for a squiggly underline.) Tj ET";
    let bytes = {
        let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
                    /Resources << /Font << /F 5 0 R >> >> >>";
        build_pdf(&[
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            page,
            &xstream("", content),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ])
    };
    let mut doc = PdfDocument::open(bytes, standard_fonts()).expect("opens");
    let note = |kind, rects: Vec<[f32; 4]>, color, contents: &str| NewAnnot {
        kind,
        page: 0,
        rects,
        color,
        author: "BlueReader Tester".into(),
        contents: contents.into(),
    };
    for a in [
        note(
            AnnotKind::Highlight,
            vec![[40.0, 18.0, 156.0, 36.0]],
            [255, 230, 0],
            "Highlight comment",
        ),
        note(
            AnnotKind::Underline,
            vec![[166.0, 18.0, 270.0, 36.0]],
            [0, 160, 0],
            "Underline comment",
        ),
        note(
            AnnotKind::StrikeOut,
            vec![[280.0, 18.0, 360.0, 36.0]],
            [220, 0, 0],
            "Strike-out comment",
        ),
        note(
            AnnotKind::Squiggly,
            vec![[40.0, 48.0, 250.0, 66.0]],
            [0, 90, 255],
            "Squiggly comment",
        ),
        note(
            AnnotKind::Note,
            vec![[420.0, 20.0, 440.0, 40.0]],
            [255, 210, 0],
            "A sticky note\nwith two lines",
        ),
        note(
            AnnotKind::Square,
            vec![[40.0, 100.0, 180.0, 180.0]],
            [0, 80, 200],
            "Square comment",
        ),
    ] {
        doc.add_annotation(&a).expect("added");
    }
    let mk = |kind, rect, points: Vec<[f32; 2]>, color, contents: &str| Markup {
        kind,
        page: 0,
        rect,
        points,
        color,
        opacity: 1.0,
        width: 2.0,
        author: "BlueReader Tester".into(),
        contents: contents.into(),
        font_size: 14.0,
    };
    for m in [
        mk(
            MarkupKind::Ellipse,
            [220.0, 100.0, 360.0, 180.0],
            vec![],
            [200, 0, 120],
            "Ellipse",
        ),
        mk(
            MarkupKind::Line,
            [400.0, 100.0, 560.0, 180.0],
            vec![[400.0, 180.0], [560.0, 100.0]],
            [0, 0, 0],
            "Line",
        ),
        mk(
            MarkupKind::Arrow,
            [40.0, 220.0, 200.0, 300.0],
            vec![[40.0, 300.0], [200.0, 220.0]],
            [220, 0, 0],
            "Arrow",
        ),
        mk(
            MarkupKind::Polygon,
            [240.0, 220.0, 360.0, 300.0],
            vec![[240.0, 300.0], [300.0, 220.0], [360.0, 300.0]],
            [0, 120, 0],
            "Polygon",
        ),
        mk(
            MarkupKind::Cloud,
            [400.0, 220.0, 560.0, 300.0],
            vec![
                [400.0, 220.0],
                [560.0, 220.0],
                [560.0, 300.0],
                [400.0, 300.0],
            ],
            [255, 0, 0],
            "Cloud",
        ),
        mk(
            MarkupKind::Pencil,
            [40.0, 340.0, 240.0, 420.0],
            vec![
                [40.0, 400.0],
                [80.0, 340.0],
                [120.0, 420.0],
                [160.0, 350.0],
                [240.0, 400.0],
            ],
            [80, 0, 160],
            "Pencil",
        ),
        mk(
            MarkupKind::Typewriter,
            [280.0, 340.0, 560.0, 370.0],
            vec![],
            [0, 0, 0],
            "Typewriter text",
        ),
        mk(
            MarkupKind::Callout,
            [300.0, 460.0, 540.0, 520.0],
            vec![[200.0, 480.0]],
            [255, 0, 0],
            "Callout text",
        ),
        mk(
            MarkupKind::Stamp,
            [40.0, 560.0, 260.0, 620.0],
            vec![],
            [200, 0, 0],
            "APPROVED",
        ),
    ] {
        doc.add_markup(&m).expect("markup");
    }
    // A reply to the sticky note.
    let parent = doc
        .annotations()
        .into_iter()
        .find(|a| a.kind == AnnotKind::Note)
        .expect("note");
    doc.reply_to_annotation(&parent, "Second Reviewer", "A reply to the note")
        .expect("reply");
    std::fs::write(out, doc.save().expect("saves")).expect("written");
}

#[test]
fn knockout_group_objects_do_not_blend_with_each_other() {
    let group = |k: &str| {
        render(page_pdf(
            "/G Do",
            "/XObject << /G 5 0 R >>",
            &[&xstream(
                &format!(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                     /Group << /S /Transparency /K {k} >> \
                     /Resources << /ExtGState << /M << /BM /Multiply >> >> >>"
                ),
                "/M gs 1 0 0 rg 0 0 50 50 re f 0 1 0 rg 25 25 50 50 re f",
            )],
        ))
    };
    let blended = |p: &Page| {
        p.list
            .commands
            .iter()
            .filter(|c| matches!(c, Command::PushGroup { blend: Some(_), .. }))
            .count()
    };
    assert_eq!(blended(&group("false")), 2);
    // An isolated group with blending inside is marked so (a layer of its own); without
    // blending it is not.
    let iso = render(page_pdf(
        "/G Do",
        "/XObject << /G 5 0 R >>",
        &[&xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Group << /S /Transparency /I true >> \
             /Resources << /ExtGState << /M << /BM /Multiply >> >> >>",
            "/M gs 1 0 0 rg 0 0 50 50 re f",
        )],
    ));
    assert!(matches!(
        iso.list.commands.first(),
        Some(Command::PushGroup { isolated: true, .. })
    ));
    let plain = render(page_pdf(
        "/G Do",
        "/XObject << /G 5 0 R >>",
        &[&xstream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Group << /S /Transparency /I true >>",
            "1 0 0 rg 0 0 50 50 re f",
        )],
    ));
    assert!(!plain
        .list
        .commands
        .iter()
        .any(|c| matches!(c, Command::PushGroup { .. })));
    let k = group("true");
    assert_eq!(blended(&k), 0, "{:?}", k.list.commands);
    assert!(k
        .list
        .commands
        .iter()
        .any(|c| matches!(c, Command::PushGroup { knockout: true, .. })));
}

#[test]
fn a_stroke_painted_with_a_pattern_fills_its_outline_with_the_cells() {
    let p = render(page_pdf(
        "/Pattern CS /P SCN 10 w 10 50 m 190 50 l S",
        "/Pattern << /P 5 0 R >>",
        &[&xstream(
            "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] \
             /XStep 10 /YStep 10 /Resources << >>",
            "1 0 0 rg 0 0 5 5 re f",
        )],
    ));
    let c = &p.list.commands;
    // No grey stand-in; a clip to the stroke's outline with red cells inside.
    assert!(!c.iter().any(|c| matches!(
        c,
        Command::Path { stroke: Some(s), .. } if s.color == Rgba(128, 128, 128, 255)
    )));
    assert!(c.iter().any(
        |c| matches!(c, Command::PushClipPath { .. }) || matches!(c, Command::PushClip { .. })
    ));
    assert!(solid_fills(&p).contains(&Rgba(255, 0, 0, 255)), "{c:?}");
}

#[test]
fn a_fine_picture_pattern_paints_its_average_colour() {
    // A word processor's shading pattern: 0.96 pt cells, each an 8 × 8 one-bit picture
    // in an Indexed red / yellow palette (half of each), over 180 × 80 pt — far too many
    // cells to draw one by one. The area reads as orange, not the grey stand-in.
    let p = render(page_pdf(
        "/Pattern cs /P scn 10 10 180 80 re f",
        "/Pattern << /P 5 0 R >>",
        &[
            &xstream(
                "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 2 /BBox [0 0 0.96 0.96] \
                 /XStep 0.96 /YStep 0.96 /Resources << /XObject << /I 6 0 R >> >>",
                "q 0.96 0 0 0.96 0 0 cm /I Do Q",
            ),
            &xstream(
                "/Type /XObject /Subtype /Image /Width 8 /Height 8 \
                 /ColorSpace [/Indexed /DeviceRGB 1 <FF0000FFFF00>] /BitsPerComponent 1 \
                 /Filter /ASCIIHexDecode",
                "FFFFFFFF00000000>",
            ),
        ],
    ));
    let fills = solid_fills(&p);
    assert!(fills.contains(&Rgba(255, 128, 0, 255)), "{fills:?}");
    assert!(!fills.contains(&Rgba(200, 200, 200, 255)));
}

#[test]
fn cjk_substitutes_pick_the_reference_faces_and_weights() {
    use crate::font::cjk_substitutes;
    // The four lines of the corpus file 04 (PyMuPDF's built-in CJK names, /Flags 4).
    let pick = |ordering: &str, enc: &str, base: &str| {
        let (_, list, heavy) = cjk_substitutes(ordering, enc, base, false).expect("cjk");
        (list[0], heavy)
    };
    assert_eq!(pick("GB1", "UniGB-UTF16-H", "Heiti"), ("SimHei", false));
    assert_eq!(pick("CNS1", "UniCNS-UTF16-H", "Fangti"), ("MingLiU", true));
    assert_eq!(
        pick("Japan1", "UniJIS-UTF16-H", "Gothic"),
        ("MS Gothic", true)
    );
    assert_eq!(
        pick("Korea1", "UniKS-UTF16-H", "Dotum"),
        ("Malgun Gothic", true)
    );
    // A Serif-flagged font keeps the regular weight; no ordering and no CJK CMap: none.
    let (_, list, heavy) =
        cjk_substitutes("Japan1", "90ms-RKSJ-H", "MS-Mincho", true).expect("cjk");
    assert_eq!((list[0], heavy), ("MS Mincho", false));
    assert!(cjk_substitutes("Identity", "Identity-H", "Arial", false).is_none());
}

#[test]
fn a_squiggly_comment_is_drawn_as_a_wave_in_its_appearance() {
    let mut doc = PdfDocument::open(page_pdf("", "", &[]), standard_fonts()).expect("opens");
    doc.add_annotation(&NewAnnot {
        kind: AnnotKind::Squiggly,
        page: 0,
        rects: vec![[10.0, 10.0, 150.0, 30.0]],
        color: [0, 0, 255],
        author: "A".into(),
        contents: String::new(),
    })
    .expect("added");
    let saved = doc.save().expect("saves");
    let mut doc = PdfDocument::open(saved, standard_fonts()).expect("reopens");
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    let most = p
        .list
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Path {
                segs,
                stroke: Some(_),
                ..
            } => Some(segs.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert!(most > 20, "a zigzag, not one line: {most}");
}

#[test]
fn a_stamp_keeps_its_word_inside_the_frame() {
    use crate::markup::{Markup, MarkupKind};
    let mut doc = PdfDocument::open(page_pdf("", "", &[]), standard_fonts()).expect("opens");
    doc.add_markup(&Markup {
        kind: MarkupKind::Stamp,
        page: 0,
        rect: [20.0, 20.0, 120.0, 60.0],
        points: Vec::new(),
        color: [200, 0, 0],
        opacity: 1.0,
        width: 2.0,
        author: "A".into(),
        contents: "Approved".into(),
        font_size: 12.0,
    })
    .expect("stamp");
    let p = doc.render(0, RenderOptions::default()).expect("renders");
    let bounds = |stroke: bool| {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for c in &p.list.commands {
            if let Command::Path {
                segs,
                fill,
                stroke: st,
            } = c
            {
                if st.is_some() != stroke || (!stroke && fill.is_none()) {
                    continue;
                }
                for s in segs {
                    if let PathSeg::M(x, y) | PathSeg::L(x, y) | PathSeg::C(.., x, y) = *s {
                        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
                    }
                }
            }
        }
        b
    };
    let (frame, word) = (bounds(true), bounds(false));
    assert!(
        word[0] > frame[0] && word[2] < frame[2],
        "{frame:?} {word:?}"
    );
    assert!(
        word[1] > frame[1] && word[3] < frame[3],
        "{frame:?} {word:?}"
    );
}

#[test]
fn vertical_punctuation_has_its_vertical_forms() {
    use crate::font::vertical_form;
    assert_eq!(vertical_form('\u{3002}'), Some('\u{FE12}'));
    assert_eq!(vertical_form('\u{FF1A}'), Some('\u{FE13}'));
    assert_eq!(vertical_form('\u{300C}'), Some('\u{FE41}'));
    assert_eq!(vertical_form('\u{4E2D}'), None);
}

#[test]
fn text_filled_with_a_shading_pattern_shows_the_gradient() {
    let p = render(page_pdf(
        "/Pattern cs /P scn BT /F 40 Tf 10 30 Td (AB) Tj ET",
        "/Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> \
         /Pattern << /P << /PatternType 2 /Shading << /ShadingType 2 /ColorSpace /DeviceRGB \
         /Coords [0 0 200 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> \
         >> >> >>",
        &[],
    ));
    assert!(
        p.list.commands.iter().any(|c| matches!(
            c,
            Command::Path {
                fill: Some(Paint::Linear { .. }),
                ..
            }
        )),
        "{:?}",
        p.list.commands
    );
    assert!(!p
        .list
        .commands
        .iter()
        .any(|c| matches!(c, Command::Glyphs { .. })));
    // Still searchable.
    let text: String = p.chars.iter().map(|c| c.text.as_str()).collect();
    assert_eq!(text, "AB");
}
