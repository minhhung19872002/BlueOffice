//! PDF/A level B (ISO 19005-1/-2/-3), tagged PDF (ISO 32000-1 §14.7–14.8) and
//! bookmarks (§12.3.3), checked structurally with `bo_pdf::probe`. veraPDF is
//! run only when the `VERAPDF` environment variable names its launcher.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_pdf::probe::{dict_get, parse_ref, refs, text, Probe};
use bo_pdf::{
    write_pdf_with_options, Attachment, Bookmark, PdfA, PdfError, PdfMeta, PdfOptions, Role,
    StructKid, StructNode,
};
use bo_render::{Command, DisplayList, Glyph, PageList, Rgba};
use bo_text::FontManager;
use std::collections::BTreeMap;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

/// A shaped glyph run of `s` at 12 pt.
fn run(fm: &mut FontManager, s: &str, x: f32, y: f32, color: Rgba) -> Command {
    let face = fm.resolve("Carlito", false, false).unwrap();
    let scale = 12.0 / f32::from(fm.info(face).units_per_em);
    let mut pen = 0.0;
    let glyphs = fm
        .shape(face, s)
        .iter()
        .map(|g| {
            let out = Glyph(g.glyph_id, pen + g.x_offset as f32 * scale, 0.0, g.cluster);
            pen += g.x_advance as f32 * scale;
            out
        })
        .collect();
    Command::Glyphs {
        face: face.0,
        size: 12.0,
        x,
        y,
        color,
        text: s.to_owned(),
        glyphs,
    }
}

/// 2 × 1 RGBA picture: opaque red, half-transparent blue.
fn tiny_png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 2, 1);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[255, 0, 0, 255, 0, 0, 255, 128])
            .unwrap();
    }
    out
}

fn image() -> Command {
    Command::Image {
        x: 72.0,
        y: 150.0,
        w: 100.0,
        h: 50.0,
        image: "img1".into(),
        crop: None,
        rotation: 0.0,
    }
}

const BLACK: Rgba = Rgba(0, 0, 0, 255);

/// Page 0: title, 50 % black body text, a picture, a list item.
/// Page 1: a table head and cell under a 50 % opacity group, then more text.
fn sample(fonts: &mut FontManager) -> DisplayList {
    DisplayList {
        pages: vec![
            PageList {
                width: 595.0,
                height: 842.0,
                commands: vec![
                    run(fonts, "Title", 72.0, 80.0, BLACK),
                    run(fonts, "Body text", 72.0, 120.0, Rgba(0, 0, 0, 128)),
                    image(),
                    run(fonts, "Item", 90.0, 230.0, BLACK),
                ],
            },
            PageList {
                width: 595.0,
                height: 842.0,
                commands: vec![
                    Command::PushOpacity { alpha: 0.5 },
                    run(fonts, "Head", 72.0, 80.0, BLACK),
                    run(fonts, "Cell 1", 72.0, 100.0, BLACK),
                    Command::PopOpacity,
                    run(fonts, "Next", 72.0, 140.0, BLACK),
                ],
            },
        ],
    }
}

fn images() -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    m.insert("img1".to_owned(), tiny_png());
    m
}

fn meta() -> PdfMeta {
    PdfMeta {
        title: Some("Báo cáo".into()),
    }
}

/// One command of `page` as a content kid.
fn content(page: usize, start: usize) -> StructKid {
    StructKid::Content {
        page,
        start,
        end: start + 1,
    }
}

fn node(role: Role, kids: Vec<StructKid>) -> StructKid {
    StructKid::Node(StructNode {
        role,
        alt: None,
        lang: None,
        kids,
    })
}

/// H1, P, Figure (alt), L > LI > LBody, Table > TR > TH + TD, P.
fn structure() -> Vec<StructNode> {
    let unwrap = |k: StructKid| match k {
        StructKid::Node(n) => n,
        StructKid::Content { .. } => unreachable!(),
    };
    let mut figure = unwrap(node(Role::Figure, vec![content(0, 2)]));
    figure.alt = Some("Biểu đồ".into());
    vec![
        unwrap(node(Role::H(1), vec![content(0, 0)])),
        unwrap(node(Role::P, vec![content(0, 1)])),
        figure,
        unwrap(node(
            Role::L,
            vec![node(Role::LI, vec![node(Role::LBody, vec![content(0, 3)])])],
        )),
        unwrap(node(
            Role::Table,
            vec![node(
                Role::TR,
                vec![
                    node(Role::TH, vec![content(1, 1)]),
                    node(Role::TD, vec![content(1, 2)]),
                ],
            )],
        )),
        unwrap(node(Role::P, vec![content(1, 4)])),
    ]
}

fn flat(s: &str) -> String {
    s.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
}

fn role_of(p: &Probe, r: u32) -> String {
    dict_get(&p.get(r).unwrap().dict, "S").unwrap()
}

fn kids(p: &Probe, r: u32) -> Vec<u32> {
    refs(&dict_get(&p.get(r).unwrap().dict, "K").unwrap_or_default())
}

fn page_num(p: &Probe, i: usize) -> u32 {
    let pages = p.pages();
    p.objects
        .iter()
        .find(|(_, o)| std::ptr::eq(*o, pages[i]))
        .map(|(n, _)| *n)
        .unwrap()
}

fn page_content(p: &Probe, i: usize) -> String {
    let page = p.pages()[i];
    let c = p.deref(&dict_get(&page.dict, "Contents").unwrap()).unwrap();
    String::from_utf8(Probe::stream_data(c).unwrap()).unwrap()
}

#[test]
fn pdfa_1b_metadata_intent_fonts_and_no_transparency() {
    let mut f = fonts();
    let opts = PdfOptions {
        pdfa: Some(PdfA::A1b),
        author: Some("Nguyễn Lan".into()),
        creator: Some("BlueDoc".into()),
        lang: Some("vi-VN".into()),
        ..PdfOptions::default()
    };
    let bytes = write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap();
    let p = Probe::parse(&bytes).unwrap();
    // ISO 19005-1 is based on PDF 1.4.
    assert_eq!(p.header, "%PDF-1.4");
    // §6.1.3: file identifier of two 16-byte strings; §6.1.3 no /Encrypt.
    assert!(dict_get(&p.trailer, "Encrypt").is_none());
    let id = dict_get(&p.trailer, "ID").expect("trailer /ID");
    let parts: Vec<&str> = id
        .trim_matches(|c| c == '[' || c == ']')
        .split_whitespace()
        .collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 2 + 32, "{id}");

    let cat = p.catalog().unwrap();
    // §6.7.2: metadata stream, unfiltered, with the identification schema.
    let md = p.deref(&dict_get(&cat.dict, "Metadata").unwrap()).unwrap();
    assert!(dict_get(&md.dict, "Filter").is_none());
    assert_eq!(dict_get(&md.dict, "Subtype").as_deref(), Some("/XML"));
    let xmp = String::from_utf8(md.stream.clone().unwrap()).unwrap();
    assert!(xmp.contains("<pdfaid:part>1</pdfaid:part>"));
    assert!(xmp.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
    // §6.7.3: XMP values equal the information dictionary.
    let info = p.deref(&dict_get(&p.trailer, "Info").unwrap()).unwrap();
    assert_eq!(
        text(&dict_get(&info.dict, "Title").unwrap()).as_deref(),
        Some("Báo cáo")
    );
    assert!(xmp.contains("<rdf:li xml:lang=\"x-default\">Báo cáo</rdf:li>"));
    assert_eq!(
        text(&dict_get(&info.dict, "Author").unwrap()).as_deref(),
        Some("Nguyễn Lan")
    );
    assert!(xmp.contains("<rdf:Seq><rdf:li>Nguyễn Lan</rdf:li></rdf:Seq>"));
    assert_eq!(
        text(&dict_get(&info.dict, "Creator").unwrap()).as_deref(),
        Some("BlueDoc")
    );
    assert!(xmp.contains("<xmp:CreatorTool>BlueDoc</xmp:CreatorTool>"));
    assert_eq!(
        text(&dict_get(&info.dict, "Producer").unwrap()).as_deref(),
        Some("BlueOffice")
    );
    assert!(xmp.contains("<pdf:Producer>BlueOffice</pdf:Producer>"));

    // §6.2.2: one PDF/A output intent with an embedded RGB profile.
    let oi = flat(&dict_get(&cat.dict, "OutputIntents").unwrap());
    assert!(oi.contains("/S /GTS_PDFA1"), "{oi}");
    let prof_ref = dict_get(
        oi.trim_matches(|c| c == '[' || c == ']'),
        "DestOutputProfile",
    )
    .unwrap();
    let prof = p.deref(&prof_ref).unwrap();
    assert_eq!(dict_get(&prof.dict, "N").as_deref(), Some("3"));
    let icc = Probe::stream_data(prof).unwrap();
    assert_eq!(&icc[36..40], b"acsp");
    assert_eq!(&icc[16..20], b"RGB ");

    // §6.4: no soft masks, no constant alpha, no transparency groups.
    let all = String::from_utf8_lossy(&bytes);
    assert!(!all.contains("/SMask"));
    assert!(!all.contains("/ca "));
    assert!(!all.contains("/Group"));
    // The 50 % black text is mixed over white: 255 × (1 − 128/255) = 127 → 0.498.
    let c0 = page_content(&p, 0);
    assert!(c0.contains("0.49803922 0.49803922 0.49803922 rg"), "{c0}");
    // The picture: red stays, half-transparent blue over white = (127, 127, 255).
    let img = p
        .objects
        .values()
        .find(|o| dict_get(&o.dict, "Subtype").as_deref() == Some("/Image"))
        .unwrap();
    assert_eq!(Probe::stream_data(img).unwrap(), [255, 0, 0, 127, 127, 255]);
    // Page 1's opacity group (0.5) is flattened too: black → 127.5 → 128.
    let c1 = page_content(&p, 1);
    assert!(c1.contains("0.5019608 0.5019608 0.5019608 rg"), "{c1}");

    // §6.3: fonts embedded, complete widths, CIDSet, Identity CIDToGIDMap, ToUnicode.
    let t0 = p
        .objects
        .values()
        .find(|o| dict_get(&o.dict, "Subtype").as_deref() == Some("/Type0"))
        .unwrap();
    assert!(dict_get(&t0.dict, "ToUnicode").is_some());
    let cid = p
        .get(refs(&dict_get(&t0.dict, "DescendantFonts").unwrap())[0])
        .unwrap();
    assert_eq!(
        dict_get(&cid.dict, "CIDToGIDMap").as_deref(),
        Some("/Identity")
    );
    // Distinct glyphs of "Title Body text Item Head Cell 1 Next" plus .notdef:
    // T i t l e ␠ B o d y x I m H a C 1 N → 18 + 1 = 19 widths.
    let w = flat(&dict_get(&cid.dict, "W").unwrap());
    // [0 [w0 w1 …]]: first CID 0, then one width per CID.
    let n_widths = w.replace(['[', ']'], " ").split_whitespace().count() - 1;
    assert_eq!(n_widths, 19, "{w}");
    let fd = p
        .deref(&dict_get(&cid.dict, "FontDescriptor").unwrap())
        .unwrap();
    assert!(dict_get(&fd.dict, "FontFile2").is_some());
    let cidset = p.deref(&dict_get(&fd.dict, "CIDSet").unwrap()).unwrap();
    // 19 CIDs: 0xFF 0xFF 0xE0.
    assert_eq!(Probe::stream_data(cidset).unwrap(), [0xFF, 0xFF, 0xE0]);
}

#[test]
fn pdfa_2b_keeps_transparency_with_pdf_1_7() {
    let mut f = fonts();
    let opts = PdfOptions {
        pdfa: Some(PdfA::A2b),
        ..PdfOptions::default()
    };
    let bytes = write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap();
    let p = Probe::parse(&bytes).unwrap();
    assert_eq!(p.header, "%PDF-1.7");
    let cat = p.catalog().unwrap();
    let md = p.deref(&dict_get(&cat.dict, "Metadata").unwrap()).unwrap();
    let xmp = String::from_utf8(md.stream.clone().unwrap()).unwrap();
    assert!(xmp.contains("<pdfaid:part>2</pdfaid:part>"));
    // ISO 19005-2 §6.4 allows transparency: the picture keeps its soft mask.
    assert!(String::from_utf8_lossy(&bytes).contains("/SMask"));
    // §6.2.3 output intent; §6.2.11.4.1: no CIDSet needed.
    assert!(dict_get(&cat.dict, "OutputIntents").is_some());
}

#[test]
fn pdfa_3b_attaches_the_source_and_2b_refuses() {
    let mut f = fonts();
    let source = Attachment {
        name: "báo cáo.docx".into(),
        mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document".into(),
        description: "Source file".into(),
        data: b"PK\x03\x04 source".to_vec(),
        source: true,
    };
    let opts = PdfOptions {
        pdfa: Some(PdfA::A3b),
        attachments: vec![source.clone()],
        ..PdfOptions::default()
    };
    let bytes = write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap();
    let p = Probe::parse(&bytes).unwrap();
    let cat = p.catalog().unwrap();
    let md = p.deref(&dict_get(&cat.dict, "Metadata").unwrap()).unwrap();
    let xmp = String::from_utf8(md.stream.clone().unwrap()).unwrap();
    assert!(xmp.contains("<pdfaid:part>3</pdfaid:part>"));
    // ISO 19005-3 §6.8: the file is an associated file of the document (/AF) …
    let af = refs(&dict_get(&cat.dict, "AF").unwrap());
    assert_eq!(af.len(), 1);
    let spec = p.get(af[0]).unwrap();
    assert_eq!(dict_get(&spec.dict, "Type").as_deref(), Some("/Filespec"));
    assert_eq!(
        dict_get(&spec.dict, "AFRelationship").as_deref(),
        Some("/Source")
    );
    assert_eq!(
        text(&dict_get(&spec.dict, "UF").unwrap()).as_deref(),
        Some("báo cáo.docx")
    );
    // … with an ASCII /F fallback (ISO 32000-1 §7.11.3).
    assert_eq!(
        text(&dict_get(&spec.dict, "F").unwrap()).as_deref(),
        Some("b_o c_o.docx")
    );
    // … listed in the EmbeddedFiles name tree (ISO 32000-1 §7.7.4).
    let names = flat(&dict_get(&cat.dict, "Names").unwrap());
    assert!(names.contains("/EmbeddedFiles"), "{names}");
    // … and its stream has the MIME type and size (§7.11.4).
    let ef = dict_get(&spec.dict, "EF").unwrap();
    let stream = p
        .deref(&dict_get(&ef, "UF").or_else(|| dict_get(&ef, "F")).unwrap())
        .unwrap();
    assert_eq!(
        dict_get(&stream.dict, "Type").as_deref(),
        Some("/EmbeddedFile")
    );
    assert!(dict_get(&stream.dict, "Subtype")
        .unwrap()
        .starts_with("/application"));
    assert_eq!(Probe::stream_data(stream).unwrap(), source.data);
    let params = flat(&dict_get(&stream.dict, "Params").unwrap());
    assert!(
        params.contains(&format!("/Size {}", source.data.len())),
        "{params}"
    );

    // PDF/A-1 forbids embedded files; PDF/A-2 allows only PDF/A files.
    for part in [PdfA::A1b, PdfA::A2b] {
        let opts = PdfOptions {
            pdfa: Some(part),
            attachments: vec![source.clone()],
            ..PdfOptions::default()
        };
        let err =
            write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap_err();
        assert!(matches!(err, PdfError::Conformance(_)), "{err}");
    }
}

#[test]
fn structure_tree_marked_content_and_parent_tree() {
    let mut f = fonts();
    let opts = PdfOptions {
        structure: Some(structure()),
        lang: Some("vi-VN".into()),
        ..PdfOptions::default()
    };
    let bytes = write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap();
    let p = Probe::parse(&bytes).unwrap();
    let cat = p.catalog().unwrap();
    // §14.7.1: a tagged PDF; §14.9.2.1: document language.
    assert!(flat(&dict_get(&cat.dict, "MarkInfo").unwrap()).contains("/Marked true"));
    assert_eq!(
        text(&dict_get(&cat.dict, "Lang").unwrap()).as_deref(),
        Some("vi-VN")
    );
    // §14.7.2: StructTreeRoot → Document → the elements in reading order.
    let root_num = parse_ref(&dict_get(&cat.dict, "StructTreeRoot").unwrap()).unwrap();
    let root = p.get(root_num).unwrap();
    assert_eq!(
        dict_get(&root.dict, "Type").as_deref(),
        Some("/StructTreeRoot")
    );
    let doc = kids(&p, root_num);
    assert_eq!(doc.len(), 1);
    assert_eq!(role_of(&p, doc[0]), "/Document");
    let top: Vec<String> = kids(&p, doc[0]).iter().map(|&r| role_of(&p, r)).collect();
    assert_eq!(top, ["/H1", "/P", "/Figure", "/L", "/Table", "/P"]);
    let top_refs = kids(&p, doc[0]);
    // §14.9.3: the figure's alternate description.
    let fig = p.get(top_refs[2]).unwrap();
    assert_eq!(
        text(&dict_get(&fig.dict, "Alt").unwrap()).as_deref(),
        Some("Biểu đồ")
    );
    // L > LI > LBody; Table > TR > TH + TD, the header scoped to its column.
    let li = kids(&p, top_refs[3]);
    assert_eq!(role_of(&p, li[0]), "/LI");
    assert_eq!(role_of(&p, kids(&p, li[0])[0]), "/LBody");
    let tr = kids(&p, top_refs[4]);
    assert_eq!(role_of(&p, tr[0]), "/TR");
    let cells: Vec<String> = kids(&p, tr[0]).iter().map(|&r| role_of(&p, r)).collect();
    assert_eq!(cells, ["/TH", "/TD"]);
    let th = p.get(kids(&p, tr[0])[0]).unwrap();
    assert!(flat(&dict_get(&th.dict, "A").unwrap()).contains("/Scope /Column"));
    // §14.7.4.2: an element's content is a marked-content reference on its page.
    let h1 = p.get(top_refs[0]).unwrap();
    assert_eq!(
        parse_ref(&dict_get(&h1.dict, "Pg").unwrap()),
        Some(page_num(&p, 0))
    );
    assert!(flat(&dict_get(&h1.dict, "K").unwrap()).contains("/MCID 0"));

    // §14.6: tagged commands become marked-content sequences with an MCID;
    // everything else is an artifact (§14.8.2.2.2).
    let c0 = flat(&page_content(&p, 0));
    assert!(c0.contains("/H1 << /MCID 0 >> BDC"), "{c0}");
    assert!(c0.contains("/P << /MCID 1 >> BDC"), "{c0}");
    assert!(c0.contains("/Figure << /MCID 2 >> BDC"), "{c0}");
    assert!(c0.contains("/LBody << /MCID 3 >> BDC"), "{c0}");
    let c1 = flat(&page_content(&p, 1));
    assert!(c1.starts_with("/Artifact BMC"), "{c1}");
    assert!(c1.contains("/TH << /MCID 0 >> BDC"), "{c1}");
    assert_eq!(
        c0.matches("BDC").count() + c0.matches("BMC").count(),
        c0.matches("EMC").count()
    );
    assert_eq!(
        c1.matches("BDC").count() + c1.matches("BMC").count(),
        c1.matches("EMC").count()
    );

    // §14.7.4.4: each page's StructParents key maps to its MCID owners.
    for i in 0..2 {
        let pg = p.pages()[i];
        assert_eq!(dict_get(&pg.dict, "StructParents"), Some(i.to_string()));
    }
    let tree = flat(&dict_get(&root.dict, "ParentTree").unwrap());
    // Nums [key ref key ref …]: the object number before each `0 R`.
    let t: Vec<&str> = tree
        .split("/Nums")
        .nth(1)
        .unwrap()
        .split(|c: char| c.is_ascii_whitespace() || c == '[' || c == ']')
        .filter(|x| !x.is_empty())
        .collect();
    let arrays: Vec<u32> = (2..t.len())
        .filter(|&i| t[i] == "R")
        .map(|i| t[i - 2].parse().unwrap())
        .collect();
    assert_eq!(arrays.len(), 2, "{tree}");
    let owners0 = refs(&p.get(arrays[0]).unwrap().dict);
    assert_eq!(owners0.len(), 4);
    assert_eq!(owners0[0], top_refs[0]);
    assert_eq!(owners0[2], top_refs[2]);
    assert_eq!(
        dict_get(&root.dict, "ParentTreeNextKey").as_deref(),
        Some("2")
    );
}

#[test]
fn bookmarks_nest_by_level_with_xyz_destinations() {
    let mut f = fonts();
    let bm = |t: &str, level, page, y| Bookmark {
        title: t.into(),
        level,
        page,
        y,
    };
    let opts = PdfOptions {
        bookmarks: vec![
            bm("Một", 1, 0, 80.0),
            bm("Một.1", 2, 0, 120.0),
            bm("Một.1.1", 3, 0, 150.0),
            bm("Hai", 1, 1, 0.0),
            bm("Bad page", 1, 9, 0.0),
        ],
        ..PdfOptions::default()
    };
    let bytes = write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap();
    let p = Probe::parse(&bytes).unwrap();
    let cat = p.catalog().unwrap();
    assert_eq!(
        dict_get(&cat.dict, "PageMode").as_deref(),
        Some("/UseOutlines")
    );
    let root = p.deref(&dict_get(&cat.dict, "Outlines").unwrap()).unwrap();
    // Four valid items, all open (§12.3.3 Table 152: Count = visible descendants).
    assert_eq!(dict_get(&root.dict, "Count").as_deref(), Some("4"));
    let one = p.deref(&dict_get(&root.dict, "First").unwrap()).unwrap();
    let two = p.deref(&dict_get(&root.dict, "Last").unwrap()).unwrap();
    assert_eq!(
        text(&dict_get(&one.dict, "Title").unwrap()).as_deref(),
        Some("Một")
    );
    assert_eq!(
        text(&dict_get(&two.dict, "Title").unwrap()).as_deref(),
        Some("Hai")
    );
    assert_eq!(dict_get(&one.dict, "Count").as_deref(), Some("2"));
    assert_eq!(
        parse_ref(&dict_get(&one.dict, "Next").unwrap()),
        Some(two.num)
    );
    assert_eq!(
        parse_ref(&dict_get(&two.dict, "Prev").unwrap()),
        Some(one.num)
    );
    let sub = p.deref(&dict_get(&one.dict, "First").unwrap()).unwrap();
    assert_eq!(
        text(&dict_get(&sub.dict, "Title").unwrap()).as_deref(),
        Some("Một.1")
    );
    assert_eq!(
        parse_ref(&dict_get(&sub.dict, "Parent").unwrap()),
        Some(one.num)
    );
    assert_eq!(dict_get(&sub.dict, "Count").as_deref(), Some("1"));
    // Destination [page /XYZ left top zoom]; top = 842 − 120 = 722 in PDF space;
    // zoom 0 = unchanged (§12.3.2.2 Table 151).
    assert_eq!(
        flat(&dict_get(&sub.dict, "Dest").unwrap()),
        format!("[{} 0 R /XYZ 0 722 0]", page_num(&p, 0))
    );
    assert_eq!(
        flat(&dict_get(&two.dict, "Dest").unwrap()),
        format!("[{} 0 R /XYZ 0 842 0]", page_num(&p, 1))
    );
}

/// Optional: validate with veraPDF (MPL-2.0 / GPL dual licence, run as an
/// external program only) when `VERAPDF` points to its command-line launcher.
#[test]
fn verapdf_when_available() {
    let Ok(cli) = std::env::var("VERAPDF") else {
        return;
    };
    let mut f = fonts();
    let dir = std::env::temp_dir().join(format!("bo-pdfa-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (part, flavour) in [(PdfA::A1b, "1b"), (PdfA::A2b, "2b"), (PdfA::A3b, "3b")] {
        let opts = PdfOptions {
            pdfa: Some(part),
            structure: Some(structure()),
            lang: Some("vi-VN".into()),
            bookmarks: vec![Bookmark {
                title: "Title".into(),
                level: 1,
                page: 0,
                y: 80.0,
            }],
            ..PdfOptions::default()
        };
        let bytes = write_pdf_with_options(&sample(&mut f), &f, &meta(), &images(), &opts).unwrap();
        let file = dir.join(format!("sample-{flavour}.pdf"));
        std::fs::write(&file, bytes).unwrap();
        let out = std::process::Command::new(&cli)
            .args(["--flavour", flavour, "--format", "text"])
            .arg(&file)
            .output()
            .expect("run veraPDF");
        let s = String::from_utf8_lossy(&out.stdout);
        assert!(s.contains("PASS"), "veraPDF {flavour}: {s}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
