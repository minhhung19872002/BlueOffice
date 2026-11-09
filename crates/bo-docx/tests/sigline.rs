//! Signature lines (CORE-SIGLINE-001): VML picture shapes with `o:signatureline`
//! (ECMA-376 Part 4, §19.2.2.30) — reading their attributes, writing an untouched
//! document back byte for byte, writing new lines, and the signed state read from
//! a package signature's `SignatureInfoV1` ([MS-OFFCRYPTO] idOfficeObject).
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_docx::{read_docx, write_docx, CT_DOCUMENT};
use bo_model::{
    Block, Document, Emu, Image, ParaProps, Paragraph, Run, RunContent, RunProps, Section, Shape,
    ShapeArt, SignatureLine,
};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};
use std::sync::Arc;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const V: &str = "urn:schemas-microsoft-com:vml";
const O: &str = "urn:schemas-microsoft-com:office:office";
const IMG: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

/// A 1×1 PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

const ID1: &str = "{6F1B2C3D-4E5F-4061-8273-94A5B6C7D8E9}";
const ID2: &str = "{0A1B2C3D-4E5F-4061-8273-94A5B6C7D8EA}";

fn rel(id: &str, ty: &str, target: &str) -> Relationship {
    Relationship {
        id: id.into(),
        rel_type: ty.into(),
        target: target.into(),
        mode: TargetMode::Internal,
    }
}

/// Two inline signature lines: the first with every attribute, the second
/// minimal (defaults: no comments, date shown).
fn sample() -> Vec<u8> {
    let mut p = Package::new();
    let line1 = format!(
        r#"<w:pict><v:shape id="SigLine1" style="width:192pt;height:96pt" stroked="f" filled="f"><v:imagedata r:id="rId5" o:title=""/><o:signatureline v:ext="edit" id="{ID1}" provid="{{00000000-0000-0000-0000-000000000000}}" o:suggestedsigner="Nguyễn Văn A" o:suggestedsigner2="Giám đốc" o:suggestedsigneremail="a@example.vn" signinginstructionsset="t" signinginstructions="Xin ký &amp; ghi ngày" allowcomments="t" showsigndate="f" issignatureline="t"/></v:shape></w:pict>"#
    );
    let line2 = format!(
        r#"<w:pict><v:shape id="SigLine2" style="width:192pt;height:96pt" stroked="f" filled="f"><v:imagedata r:id="rId6" o:title=""/><o:signatureline v:ext="edit" id="{ID2}" provid="{{00000000-0000-0000-0000-000000000000}}" o:suggestedsigner="Trần Thị B" issignatureline="t"/></v:shape></w:pict>"#
    );
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{W}" xmlns:r="{R}" xmlns:v="{V}" xmlns:o="{O}"><w:body><w:p><w:r><w:t>Bên A</w:t></w:r></w:p><w:p><w:r>{line1}</w:r></w:p><w:p><w:r><w:t>Bên B</w:t></w:r></w:p><w:p><w:r>{line2}</w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1134" w:right="850" w:bottom="1134" w:left="1701" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    p.add_part("/word/media/image1.png", "image/png", PNG.to_vec())
        .unwrap();
    p.add_part("/word/media/image2.png", "image/png", PNG.to_vec())
        .unwrap();
    p.set_relationships(
        "/word/document.xml",
        &[
            rel("rId5", IMG, "media/image1.png"),
            rel("rId6", IMG, "media/image2.png"),
        ],
    );
    p.set_relationships(
        "/",
        &[rel("rId1", REL_OFFICE_DOCUMENT, "word/document.xml")],
    );
    p.write().unwrap()
}

fn lines(doc: &Document) -> Vec<(Image, SignatureLine)> {
    let mut out = Vec::new();
    for b in &doc.sections[0].blocks {
        let Block::Paragraph(p) = b else { continue };
        for c in p.runs.iter().flat_map(|r| &r.content) {
            if let RunContent::Image(img) = c {
                if let Some(l) = img
                    .shape
                    .as_ref()
                    .and_then(|s| s.art.as_ref())
                    .and_then(|a| a.signature_line.as_ref())
                {
                    out.push(((**img).clone(), (**l).clone()));
                }
            }
        }
    }
    out
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name).unwrap().data.clone()
}

#[test]
fn reads_signature_line_attributes() {
    let doc = read_docx(&sample()).unwrap();
    let l = lines(&doc);
    assert_eq!(l.len(), 2);
    let (img, a) = &l[0];
    // 192 pt × 96 pt = 2 438 400 × 1 219 200 EMU (12 700 EMU per point), inline.
    assert_eq!((img.width, img.height), (Emu(2_438_400), Emu(1_219_200)));
    assert!(img.anchor.is_none());
    assert_eq!(
        img.shape
            .as_ref()
            .unwrap()
            .art
            .as_ref()
            .unwrap()
            .picture
            .as_deref(),
        Some("/word/media/image1.png")
    );
    assert_eq!(a.id, ID1);
    assert_eq!(a.provider_id, "{00000000-0000-0000-0000-000000000000}");
    assert_eq!(a.suggested_signer, "Nguyễn Văn A");
    assert_eq!(a.suggested_signer2, "Giám đốc");
    assert_eq!(a.suggested_signer_email, "a@example.vn");
    assert_eq!(a.signing_instructions, "Xin ký & ghi ngày");
    assert!(a.allow_comments && !a.show_sign_date && a.is_signature_line);
    assert!(a.signed.is_none());
    let (_, b) = &l[1];
    assert_eq!(b.id, ID2);
    assert_eq!(b.suggested_signer, "Trần Thị B");
    assert!(!b.allow_comments && b.show_sign_date);
    assert_eq!(b.suggested_signer2, "");
}

#[test]
fn untouched_signature_lines_are_written_back_byte_for_byte() {
    let src = sample();
    let doc = read_docx(&src).unwrap();
    let out = write_docx(&doc).unwrap();
    assert_eq!(
        part(&out, "/word/document.xml"),
        part(&src, "/word/document.xml")
    );
}

#[test]
fn edited_document_keeps_the_lines() {
    let src = sample();
    let mut doc = read_docx(&src).unwrap();
    // Edit the first paragraph only: the lines' paragraphs stay verbatim.
    if let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] {
        p.runs = vec![Run::text("Bên A (đã sửa)", RunProps::default())];
    }
    let out = write_docx(&doc).unwrap();
    let back = read_docx(&out).unwrap();
    let before: Vec<SignatureLine> = lines(&doc).into_iter().map(|(_, l)| l).collect();
    let after: Vec<SignatureLine> = lines(&back).into_iter().map(|(_, l)| l).collect();
    assert_eq!(before, after);
}

fn new_line(media: &str, line: SignatureLine) -> Paragraph {
    let img = Image {
        media: String::new(),
        width: Emu(2_438_400),
        height: Emu(1_219_200),
        anchor: None,
        crop: None,
        rotation: 0,
        description: String::new(),
        shape: Some(Box::new(Shape {
            preset: "rect".into(),
            vml: true,
            art: Some(Box::new(ShapeArt {
                picture: Some(media.into()),
                signature_line: Some(Box::new(line)),
                ..ShapeArt::default()
            })),
            ..Shape::default()
        })),
        src: None,
    };
    Paragraph::new(
        ParaProps::default(),
        vec![Run {
            content: vec![RunContent::Image(Box::new(img))],
            ..Run::text("", RunProps::default())
        }],
    )
}

#[test]
fn new_signature_lines_are_written_and_read_back() {
    let l1 = SignatureLine {
        id: ID1.into(),
        is_signature_line: true,
        suggested_signer: "Lê Văn C <\"quoted\">".into(),
        suggested_signer2: "Kế toán trưởng".into(),
        suggested_signer_email: "c@example.vn".into(),
        signing_instructions: "Ký vào ô này".into(),
        allow_comments: true,
        show_sign_date: false,
        ..SignatureLine::default()
    };
    let l2 = SignatureLine {
        id: ID2.into(),
        is_signature_line: true,
        show_sign_date: true,
        ..SignatureLine::default()
    };
    let mut doc = Document {
        sections: vec![Section::new(vec![
            Block::Paragraph(new_line("/word/media/image1.png", l1.clone())),
            Block::Paragraph(new_line("/word/media/image2.png", l2.clone())),
        ])],
        ..Document::default()
    };
    doc.media
        .insert("/word/media/image1.png".into(), Arc::new(PNG.to_vec()));
    doc.media
        .insert("/word/media/image2.png".into(), Arc::new(PNG.to_vec()));
    let out = write_docx(&doc).unwrap();
    let xml = String::from_utf8(part(&out, "/word/document.xml")).unwrap();
    assert!(xml.contains("o:suggestedsigner=\"Lê Văn C &lt;&quot;quoted&quot;&gt;\""));
    assert!(xml.contains("signinginstructionsset=\"t\""));
    let back = read_docx(&out).unwrap();
    let got: Vec<SignatureLine> = lines(&back).into_iter().map(|(_, l)| l).collect();
    // The default provider GUID is written when none is set.
    let l2 = SignatureLine {
        provider_id: "{00000000-0000-0000-0000-000000000000}".into(),
        ..l2
    };
    let l1 = SignatureLine {
        provider_id: "{00000000-0000-0000-0000-000000000000}".into(),
        ..l1
    };
    assert_eq!(got, vec![l1, l2]);
    let (img, _) = &lines(&back)[0];
    assert_eq!((img.width, img.height), (Emu(2_438_400), Emu(1_219_200)));
    assert!(img.anchor.is_none());
}

#[test]
fn signed_state_comes_from_the_package_signature() {
    let mut pkg = Package::read(&sample(), &Limits::default()).unwrap();
    let sig = format!(
        r##"<?xml version="1.0" encoding="UTF-8"?><Signature xmlns="http://www.w3.org/2000/09/xmldsig#" Id="idPackageSignature"><Object Id="idOfficeObject"><SignatureProperties><SignatureProperty Id="idOfficeV1Details" Target="#idPackageSignature"><SignatureInfoV1 xmlns="http://schemas.microsoft.com/office/2006/digsig"><SetupID>{}</SetupID><SignatureText>Trần Thị B</SignatureText><SignatureImage></SignatureImage></SignatureInfoV1></SignatureProperty></SignatureProperties></Object><Object><xd:QualifyingProperties xmlns:xd="http://uri.etsi.org/01903/v1.3.2#"><xd:SignedProperties><xd:SignedSignatureProperties><xd:SigningTime>2026-11-09T08:00:00Z</xd:SigningTime></xd:SignedSignatureProperties></xd:SignedProperties></xd:QualifyingProperties></Object></Signature>"##,
        ID2.to_ascii_lowercase()
    );
    pkg.add_part(
        "/_xmlsignatures/sig1.xml",
        "application/vnd.openxmlformats-package.digital-signature-xmlsignature+xml",
        sig.into_bytes(),
    )
    .unwrap();
    let bytes = pkg.write().unwrap();
    let doc = read_docx(&bytes).unwrap();
    let l = lines(&doc);
    assert!(l[0].1.signed.is_none());
    let s = l[1].1.signed.as_ref().unwrap();
    assert_eq!(s.text, "Trần Thị B");
    assert!(s.image.is_none());
    assert_eq!(s.date, "2026-11-09");
    // The signed state is display only: the document part is written back as read.
    let out = write_docx(&doc).unwrap();
    assert_eq!(
        part(&out, "/word/document.xml"),
        part(&bytes, "/word/document.xml")
    );
}
