//! PDF Optimizer round 10: monochrome compression, font subsetting / unembedding, discarding,
//! audit of the space used.
use super::*;
use crate::standard_fonts;
use lopdf::dictionary;

const SANS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/LiberationSans-Regular.ttf"
));

/// A 2400 × 2400 1-bit scan (stripes and dots) drawn 4 in wide (600 ppi), an RGB picture,
/// "Hello" in an embedded TrueType font and "AB" in an embedded Type0 font of the same face,
/// a page thumbnail and a structure tree.
fn doc() -> (PdfDocument, Vec<u8>) {
    let n = 2400_usize;
    let stride = n / 8;
    let mut bits = vec![0xff_u8; stride * n];
    for y in 0..n {
        for x in 0..n {
            if (y / 37) % 3 == 0 && (x / 53) % 2 == 0 || (x * 7 + y * 13) % 101 == 0 {
                bits[y * stride + x / 8] &= !(0x80 >> (x % 8));
            }
        }
    }
    let mut d = lopdf::Document::with_version("1.7");
    let mono = d.add_object(Object::Stream(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => n as i64, "Height" => n as i64,
            "BitsPerComponent" => 1, "ColorSpace" => "DeviceGray", "Filter" => "FlateDecode",
        },
        miniz_oxide::deflate::compress_to_vec_zlib(&bits, 6),
    )));
    let face = ttf_parser::Face::parse(SANS, 0).expect("font");
    let gid = |c: char| face.glyph_index(c).expect("glyph").0;
    let file = d.add_object(Object::Stream(Stream::new(
        dictionary! { "Length1" => SANS.len() as i64 },
        SANS.to_vec(),
    )));
    let desc = d.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "LiberationSans", "Flags" => 32,
        "FontBBox" => vec![0.into(), (-200).into(), 1000.into(), 900.into()],
        "ItalicAngle" => 0, "Ascent" => 900, "Descent" => -200, "CapHeight" => 700, "StemV" => 80,
        "FontFile2" => file,
    });
    let simple = d.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "TrueType", "BaseFont" => "LiberationSans",
        "FirstChar" => 32, "LastChar" => 126, "Encoding" => "WinAnsiEncoding",
        "Widths" => vec![Object::Integer(600); 95], "FontDescriptor" => desc,
    });
    let file2 = d.add_object(Object::Stream(Stream::new(
        dictionary! { "Length1" => SANS.len() as i64 },
        SANS.to_vec(),
    )));
    let desc2 = d.add_object(dictionary! {
        "Type" => "FontDescriptor", "FontName" => "Sans2", "Flags" => 32,
        "FontBBox" => vec![0.into(), (-200).into(), 1000.into(), 900.into()],
        "ItalicAngle" => 0, "Ascent" => 900, "Descent" => -200, "CapHeight" => 700, "StemV" => 80,
        "FontFile2" => file2,
    });
    let cid = d.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "CIDFontType2", "BaseFont" => "Sans2",
        "CIDSystemInfo" => dictionary! { "Registry" => Object::string_literal("Adobe"),
            "Ordering" => Object::string_literal("Identity"), "Supplement" => 0 },
        "FontDescriptor" => desc2, "CIDToGIDMap" => "Identity",
    });
    let type0 = d.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type0", "BaseFont" => "Sans2",
        "Encoding" => "Identity-H", "DescendantFonts" => vec![cid.into()],
    });
    let (a, b) = (gid('A'), gid('B'));
    let mut text = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET BT /F2 12 Tf 72 680 Td <".to_vec();
    text.extend(format!("{a:04X}{b:04X}> Tj ET q 288 0 0 288 72 72 cm /Im1 Do Q").bytes());
    let content = d.add_object(Object::Stream(Stream::new(Dictionary::new(), text)));
    let thumb = d.add_object(Object::Stream(Stream::new(
        dictionary! { "Width" => 1, "Height" => 1, "BitsPerComponent" => 8, "ColorSpace" => "DeviceGray" },
        vec![0],
    )));
    let pages_id = d.new_object_id();
    let page = d.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! {
            "XObject" => dictionary! { "Im1" => mono },
            "Font" => dictionary! { "F1" => simple, "F2" => type0 },
        },
        "Contents" => content, "Thumb" => thumb, "StructParents" => 0,
    });
    d.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
        ),
    );
    let tree = d.add_object(dictionary! { "Type" => "StructTreeRoot" });
    let cat = d.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => pages_id, "StructTreeRoot" => tree,
        "MarkInfo" => dictionary! { "Marked" => true },
    });
    d.trailer.set("Root", cat);
    let mut out = Vec::new();
    d.save_to(&mut out).expect("saves");
    (
        PdfDocument::open(out.clone(), standard_fonts()).expect("opens"),
        out,
    )
}

fn no_jpeg(_: &[u8], _: u32, _: u32, _: u8) -> Option<Vec<u8>> {
    None
}

/// The black pixels of the first picture of page 0, at its own size.
fn picture(d: &mut PdfDocument) -> (u32, u32, usize) {
    let page = d
        .render(0, crate::RenderOptions::default())
        .expect("renders");
    let img = page.images.values().next().expect("picture");
    let black = img
        .pixels
        .chunks_exact(4)
        .filter(|p| p[3] > 0 && p[0] < 128)
        .count();
    (img.width, img.height, black)
}

#[test]
fn monochrome_pictures_are_downsampled_and_written_with_each_compression() {
    let (mut base, _) = doc();
    let (_, _, black0) = picture(&mut base);
    for comp in [
        MonoCompression::Jbig2,
        MonoCompression::Ccitt4,
        MonoCompression::Ccitt3,
        MonoCompression::Zip,
        MonoCompression::RunLength,
    ] {
        let (mut d, orig) = doc();
        let o = Optimize10 {
            mono_compression: comp,
            fonts: false,
            only_smaller: false,
            ..Optimize10::default()
        };
        let (bytes, rep) = d.optimize10(&o, &no_jpeg).expect("optimizes");
        assert_eq!(rep.mono, 1, "{comp:?} {rep:?}");
        let mut again = PdfDocument::open(bytes.clone(), standard_fonts()).expect("opens");
        let (w, h, black) = picture(&mut again);
        // 4 in at 300 ppi.
        assert_eq!((w, h), (1200, 1200), "{comp:?}");
        // About a quarter of the black pixels at half the resolution in each direction.
        let ratio = black as f32 * 4.0 / black0 as f32;
        assert!((0.6..1.4).contains(&ratio), "{comp:?}: {black} vs {black0}");
        if matches!(comp, MonoCompression::Jbig2 | MonoCompression::Ccitt4) {
            assert!(
                bytes.len() < orig.len(),
                "{comp:?}: {} vs {}",
                bytes.len(),
                orig.len()
            );
        }
    }
    // Without downsampling the pixels are the same.
    let (mut d, _) = doc();
    let o = Optimize10 {
        mono: Resample {
            method: Downsample::Off,
            ppi: 300.0,
            above: 450.0,
        },
        mono_compression: MonoCompression::Ccitt4,
        fonts: false,
        ..Optimize10::default()
    };
    let (bytes, _) = d.optimize10(&o, &no_jpeg).expect("optimizes");
    let mut again = PdfDocument::open(bytes, standard_fonts()).expect("opens");
    assert_eq!(picture(&mut again), (2400, 2400, black0));
}

#[test]
fn fonts_are_listed_subset_and_unembedded() {
    let (mut d, orig) = doc();
    let names: Vec<String> = d.embedded_fonts().into_iter().map(|f| f.name).collect();
    assert_eq!(names, vec!["LiberationSans".to_owned(), "Sans2".to_owned()]);
    let o = Optimize10 {
        images: false,
        unembed: vec!["Sans2".to_owned()],
        ..Optimize10::default()
    };
    let (bytes, rep) = d.optimize10(&o, &no_jpeg).expect("optimizes");
    assert_eq!((rep.subset, rep.unembedded), (1, 1), "{rep:?}");
    assert!(
        bytes.len() * 3 < orig.len(),
        "{} vs {}",
        bytes.len(),
        orig.len()
    );
    let again = PdfDocument::open(bytes, standard_fonts()).expect("opens");
    let fonts = again.embedded_fonts();
    assert_eq!(fonts.len(), 1);
    // The subset keeps H e l o with their glyph ids, nothing else.
    let program = again
        .doc
        .objects
        .values()
        .find_map(|o| match o {
            Object::Stream(s) if s.dict.has(b"Length1") => {
                crate::filters::stream_bytes(&again.doc, s).ok()
            }
            _ => None,
        })
        .expect("program");
    let face = ttf_parser::Face::parse(&program, 0).expect("parses");
    let has = |c: char| {
        let g = face.glyph_index(c).expect("mapped");
        face.glyph_bounding_box(g).is_some()
    };
    assert!(has('H') && has('e') && has('l') && has('o'));
    assert!(!has('Z') && !has('A'));
    // Undo puts the fonts back.
    assert!(d.undo());
    assert_eq!(d.embedded_fonts().len(), 2);
}

#[test]
fn objects_and_user_data_are_discarded() {
    let (mut d, _) = doc();
    let o = Optimize10 {
        images: false,
        fonts: false,
        discard: Discard {
            thumbnails: true,
            tags: true,
            ..Discard::default()
        },
        ..Optimize10::default()
    };
    let (bytes, _) = d.optimize10(&o, &no_jpeg).expect("optimizes");
    let s = String::from_utf8_lossy(&bytes);
    let again = lopdf::Document::load_mem(&bytes).expect("loads");
    let cat = again.catalog().expect("catalog");
    assert!(!cat.has(b"StructTreeRoot") && !cat.has(b"MarkInfo"));
    assert!(!s.contains("/Thumb"));
}

#[test]
fn audit_adds_up_to_the_file_size() {
    let (d, orig) = doc();
    let a = d.audit_space();
    assert_eq!(a.len(), AUDIT_KINDS.len());
    assert_eq!(a.iter().sum::<usize>(), orig.len());
    assert!(a[IMAGES] > 1000, "{a:?}");
    assert!(a[FONTS] > SANS.len(), "{a:?}");
    assert!(a[CONTENT] > 50, "{a:?}");
    assert!(a[THUMBNAILS] > 0 && a[STRUCTURE] > 0, "{a:?}");
    assert!(a[XREF] > 20, "{a:?}");
}

#[test]
fn run_length_round_trips() {
    let data: Vec<u8> = (0..1000_u32)
        .map(|i| if i % 300 < 150 { 7 } else { (i % 13) as u8 })
        .collect();
    let enc = run_length(&data);
    assert!(enc.len() < data.len());
    let mut out = Vec::new();
    let mut i = 0;
    while i < enc.len() && enc[i] != 128 {
        let n = enc[i] as usize;
        if n < 128 {
            out.extend_from_slice(&enc[i + 1..i + 2 + n]);
            i += n + 2;
        } else {
            out.extend(std::iter::repeat_n(enc[i + 1], 257 - n));
            i += 2;
        }
    }
    assert_eq!(out, data);
}

#[test]
fn transparency_is_flattened_into_a_picture_over_hidden_content() {
    let mut d = lopdf::Document::with_version("1.7");
    let content = d.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        b"/G1 gs 1 0 0 rg 10 10 100 100 re f BT /F1 12 Tf 20 200 Td (Find me) Tj ET".to_vec(),
    )));
    let font = d.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let pages_id = d.new_object_id();
    let page = d.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 300.into()],
        "Resources" => dictionary! {
            "ExtGState" => dictionary! { "G1" => dictionary! { "ca" => 0.5 } },
            "Font" => dictionary! { "F1" => font },
        },
        "Contents" => content,
    });
    d.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 },
        ),
    );
    let cat = d.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    d.trailer.set("Root", cat);
    let mut out = Vec::new();
    d.save_to(&mut out).expect("saves");
    let mut doc = PdfDocument::open(out, standard_fonts()).expect("opens");
    assert_eq!(doc.transparency_pages(), vec![0]);
    let (w, h) = (20_u32, 30_u32);
    let rgba = vec![200_u8; (w * h * 4) as usize];
    assert_eq!(doc.flatten_transparency(&[(0, w, h, rgba)], 150.0), Ok(1));
    let page = doc
        .render(0, crate::RenderOptions::default())
        .expect("renders");
    let img = page.images.values().next().expect("picture");
    assert_eq!((img.width, img.height), (w, h));
    // The text is still there to find.
    let objs = doc.page_objects(0).expect("objects");
    assert!(crate::text_lines(&objs)
        .iter()
        .any(|l| l.text.contains("Find me")));
    assert!(doc.undo());
}
