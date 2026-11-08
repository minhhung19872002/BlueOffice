//! Generators of the binary corpus files (`tests/corpus/{slide,sheet,doc}/binary-*`).
//!
//! The files are assembled record by record from the specifications
//! ([MS-PPT], [MS-ODRAW], [MS-XLS], [MS-DOC], [MS-CFB] through the `cfb`
//! crate) by the test builders of this crate — not by any BlueOffice writer and
//! not copied from third-party files. `BO_WRITE_CORPUS=1 cargo test -p bo-binary
//! binary_corpus` rewrites them; without it the test checks that the committed
//! files still parse to what the generators describe.

use crate::ppt::tests::rec;
use crate::ppt_draw::tests::{drawing, drawing_group_rec, png_blip, sp, styled_text};

fn atom(typ: u16, body: &[u8]) -> Vec<u8> {
    rec(0, 0, typ, body)
}

fn cont(typ: u16, body: &[u8]) -> Vec<u8> {
    rec(0x0F, 0, typ, body)
}

fn chars(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// A PNG of `w × h` pixels filled with `rgb` (generated).
fn png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        if let Ok(mut wr) = enc.write_header() {
            let px: Vec<u8> = (0..w * h)
                .flat_map(|i| {
                    let t = u8::try_from(i % 256).unwrap_or(0);
                    [rgb[0] ^ t, rgb[1], rgb[2]]
                })
                .collect();
            let _ = wr.write_image_data(&px);
        }
    }
    out
}

/// One slide of a generated deck.
struct SlideDef {
    title: &'static str,
    body: &'static [&'static str],
    /// Extra shape containers (OfficeArtSpContainer records).
    shapes: Vec<Vec<u8>>,
    notes: Option<&'static str>,
}

/// A placeholder shape whose text is the `index`-th text of the slide list.
fn placeholder(id: u32, rect: [i32; 4], kind: u8, index: i32) -> Vec<u8> {
    let mut ph = 0i32.to_le_bytes().to_vec();
    ph.extend([kind, 0, 0, 0]);
    let mut extra = cont(0xF011, &atom(0x0BC3, &ph));
    extra.extend(cont(0xF00D, &atom(0x0F9E, &index.to_le_bytes())));
    sp(1, id, rect, &[], &extra)
}

/// A `.ppt` deck: DocumentContainer (slide size, fonts, BLIP store with
/// `pictures`, slide list texts), a main master colour scheme, one
/// SlideContainer per slide with placeholders + `shapes`, notes, persist
/// directory, UserEditAtom and Current User stream.
fn ppt(slides: &[SlideDef], pictures: &[Vec<u8>], size: (i32, i32)) -> Vec<u8> {
    let mut doc = Vec::new();
    let mut offsets: Vec<u32> = Vec::new();
    let n = u32::try_from(slides.len()).unwrap_or(0);
    // Persist ids: 1 = document, 2 = main master, 3.. = slides, then notes.
    let mut d = atom(0x03E9, &{
        let mut b = size.0.to_le_bytes().to_vec();
        b.extend(size.1.to_le_bytes());
        b.extend([0u8; 32]);
        b
    });
    let font = |name: &str| {
        let mut f = chars(name);
        f.resize(64, 0);
        f.extend([0u8; 4]);
        atom(0x0FB7, &f)
    };
    let mut fonts = font("Arial");
    fonts.extend(font("Times New Roman"));
    d.extend(cont(0x03F2, &cont(0x07D5, &fonts)));
    let blips: Vec<Vec<u8>> = pictures.iter().map(|p| png_blip(p)).collect();
    d.extend(drawing_group_rec(&blips));
    // SlideListWithText: per slide a SlidePersistAtom, title + body texts.
    let mut slwt = Vec::new();
    for (i, s) in slides.iter().enumerate() {
        let id = 256 + u32::try_from(i).unwrap_or(0);
        let mut b = (3 + u32::try_from(i).unwrap_or(0)).to_le_bytes().to_vec();
        b.extend([0u8; 8]);
        b.extend(id.to_le_bytes());
        b.extend([0u8; 4]);
        slwt.extend(atom(0x03F3, &b));
        slwt.extend(atom(0x0F9F, &0u32.to_le_bytes()));
        slwt.extend(atom(0x0FA0, &chars(s.title)));
        if !s.body.is_empty() {
            slwt.extend(atom(0x0F9F, &1u32.to_le_bytes()));
            slwt.extend(atom(0x0FA0, &chars(&s.body.join("\r"))));
        }
    }
    d.extend(cont(0x0FF0, &slwt));
    let mut notes_list = Vec::new();
    let mut note_ids = Vec::new();
    for (i, s) in slides.iter().enumerate() {
        if s.notes.is_some() {
            let persist = 3 + n + u32::try_from(note_ids.len()).unwrap_or(0);
            let mut b = persist.to_le_bytes().to_vec();
            b.extend([0u8; 8]);
            b.extend((256 + u32::try_from(i).unwrap_or(0)).to_le_bytes());
            b.extend([0u8; 4]);
            notes_list.extend(atom(0x03F3, &b));
            note_ids.push(i);
        }
    }
    if !notes_list.is_empty() {
        d.extend(rec(0x0F, 2, 0x0FF0, &notes_list));
    }
    offsets.push(u32::try_from(doc.len()).unwrap_or(0));
    doc.extend(cont(0x03E8, &d));
    // Main master: colour scheme (§2.4.17) only.
    let mut scheme = Vec::new();
    for c in [
        0xFF_FFFFu32,
        0x00_0000,
        0x80_8080,
        0x1F_3864,
        0xDE_EBF7,
        0x2E_75B6,
        0x00_70C0,
        0x70_AD47,
    ] {
        scheme.extend([(c >> 16) as u8, (c >> 8) as u8, c as u8, 0]);
    }
    offsets.push(u32::try_from(doc.len()).unwrap_or(0));
    doc.extend(cont(0x03F8, &rec(0, 1, 0x07F0, &scheme)));
    // Slides.
    for s in slides {
        let mut shapes = vec![placeholder(1025, [288, 216, 5472, 864], 0x0D, 0)];
        if !s.body.is_empty() {
            shapes.push(placeholder(1026, [288, 1008, 5472, 3024], 0x0E, 1));
        }
        shapes.extend(s.shapes.iter().cloned());
        let mut body = atom(0x03EF, &[0u8; 24]);
        body.extend(drawing(&shapes));
        offsets.push(u32::try_from(doc.len()).unwrap_or(0));
        doc.extend(cont(0x03EE, &body));
    }
    // Notes.
    for &i in &note_ids {
        let mut nb = atom(0x03F1, &(256 + u32::try_from(i).unwrap_or(0)).to_le_bytes());
        let mut tb = atom(0x0F9F, &2u32.to_le_bytes());
        tb.extend(atom(0x0FA0, &chars(slides[i].notes.unwrap_or(""))));
        nb.extend(drawing(&[cont(0xF004, &cont(0xF00D, &tb))]));
        offsets.push(u32::try_from(doc.len()).unwrap_or(0));
        doc.extend(cont(0x03F0, &nb));
    }
    let count = u32::try_from(offsets.len()).unwrap_or(0);
    let mut dir = (1u32 | (count << 20)).to_le_bytes().to_vec();
    for o in &offsets {
        dir.extend(o.to_le_bytes());
    }
    let dir_off = u32::try_from(doc.len()).unwrap_or(0);
    doc.extend(atom(0x1772, &dir));
    let ue_off = u32::try_from(doc.len()).unwrap_or(0);
    let mut ue = 256u32.to_le_bytes().to_vec();
    ue.extend([0, 0, 0, 3]);
    ue.extend(0u32.to_le_bytes());
    ue.extend(dir_off.to_le_bytes());
    ue.extend(1u32.to_le_bytes());
    ue.extend((count + 1).to_le_bytes());
    ue.extend([0, 0, 0, 0]);
    doc.extend(atom(0x0FF5, &ue));
    let mut cu = 20u32.to_le_bytes().to_vec();
    cu.extend(0xE391_C05Fu32.to_le_bytes());
    cu.extend(ue_off.to_le_bytes());
    cu.extend([0u8; 8]);
    let current = atom(0x0FF6, &cu);
    crate::test_util::cfb_file(
        &[("/PowerPoint Document", doc), ("/Current User", current)],
        &[],
    )
}

const RED: u32 = 0x0000_00C0;
const GREEN: u32 = 0x0050_B000;
/// Scheme index 5 (accent) as an OfficeArtCOLORREF.
const ACCENT: u32 = 0x0800_0005;

/// `.ppt` corpus: shapes, pictures, text styles + groups.
pub fn ppt_files() -> Vec<(&'static str, Vec<u8>)> {
    let shapes = ppt(
        &[
            SlideDef {
                title: "Hình cơ bản",
                body: &[],
                shapes: vec![
                    sp(
                        1,
                        1030,
                        [576, 1152, 1728, 2016],
                        &[(0x0181, RED), (0x01C0, 0x0800_0001), (0x01CB, 25_400)],
                        &[],
                    ),
                    sp(
                        3,
                        1031,
                        [2016, 1152, 3168, 2016],
                        &[(0x0181, ACCENT)],
                        &styled_text("Elip", 0xFF_FFFF),
                    ),
                    sp(
                        5,
                        1032,
                        [3456, 1152, 4608, 2016],
                        &[(0x0181, GREEN), (0x0004, 30 << 16)],
                        &[],
                    ),
                    sp(
                        13,
                        1033,
                        [576, 2304, 2592, 2880],
                        &[(0x0181, 0x0800_0004)],
                        &[],
                    ),
                    sp(
                        20,
                        1034,
                        [2880, 2304, 5184, 2880],
                        &[(0x01BF, 0x0010_0000), (0x01CB, 38_100)],
                        &[],
                    ),
                ],
                notes: Some("Năm hình: chữ nhật, elip, tam giác xoay 30°, mũi tên, đường thẳng"),
            },
            SlideDef {
                title: "Hình đặc biệt",
                body: &["Hình thoi và ngôi sao", "Mặt cười"],
                shapes: vec![
                    sp(4, 1035, [3744, 1296, 4608, 2160], &[(0x0181, RED)], &[]),
                    sp(
                        12,
                        1036,
                        [4752, 1296, 5616, 2160],
                        &[(0x0181, 0x0000_C0FF)],
                        &[],
                    ),
                    sp(
                        96,
                        1037,
                        [3744, 2304, 4608, 3168],
                        &[(0x0181, 0x0000_FFFF)],
                        &[],
                    ),
                ],
                notes: None,
            },
        ],
        &[],
        (5760, 3240),
    );
    let pictures = ppt(
        &[
            SlideDef {
                title: "Ảnh",
                body: &["Hai ảnh PNG trong BLIP store"],
                shapes: vec![
                    sp(75, 1030, [3168, 1152, 4320, 2304], &[(0x0104, 1)], &[]),
                    sp(
                        75,
                        1031,
                        [4464, 1152, 5616, 2304],
                        &[(0x0104, 2), (0x01C0, 0x0000_0000), (0x01CB, 12_700)],
                        &[],
                    ),
                ],
                notes: Some("Ảnh tự sinh"),
            },
            SlideDef {
                title: "Ảnh lặp lại",
                body: &[],
                shapes: vec![sp(75, 1032, [576, 864, 2880, 3024], &[(0x0104, 1)], &[])],
                notes: None,
            },
        ],
        &[
            png(32, 24, [0x2E, 0x75, 0xB6]),
            png(16, 16, [0xC0, 0x30, 0x10]),
        ],
        (5760, 3240),
    );
    // A group of two shapes in its own coordinate space (OfficeArtFSPGR).
    let group = {
        let mut fspgr = Vec::new();
        for v in [0i32, 0, 1000, 1000] {
            fspgr.extend(v.to_le_bytes());
        }
        let mut gfsp = 2000u32.to_le_bytes().to_vec();
        gfsp.extend(0x0201u32.to_le_bytes());
        let mut gsp = rec(1, 0, 0xF009, &fspgr);
        gsp.extend(rec(2, 0, 0xF00A, &gfsp));
        let mut a = Vec::new();
        for v in [1152i32, 2880, 3456, 4032] {
            a.extend(v.to_le_bytes());
        }
        gsp.extend(atom(0xF010, &a));
        let child = |id: u32, spt: u16, r: [i32; 4], color: u32| {
            let mut f = id.to_le_bytes().to_vec();
            f.extend(0x0A02u32.to_le_bytes());
            let mut c = rec(2, spt, 0xF00A, &f);
            c.extend(crate::ppt_draw::tests::opt(&[(0x0181, color)]));
            let mut ca = Vec::new();
            for v in r {
                ca.extend(v.to_le_bytes());
            }
            c.extend(atom(0xF00F, &ca));
            cont(0xF004, &c)
        };
        let mut g = cont(0xF004, &gsp);
        g.extend(child(2001, 1, [0, 0, 450, 1000], RED));
        g.extend(child(2002, 3, [550, 0, 1000, 1000], GREEN));
        cont(0xF003, &g)
    };
    let text = ppt(
        &[
            SlideDef {
                title: "Định dạng chữ",
                body: &["Đoạn một", "Đoạn hai"],
                shapes: vec![sp(
                    202,
                    1030,
                    [3456, 2304, 5472, 2880],
                    &[(0x0087, 1)],
                    &styled_text("Chữ đậm 24pt màu xanh", 0x00_70C0),
                )],
                notes: Some("Ghi chú: định dạng chữ và nhóm hình"),
            },
            SlideDef {
                title: "Nhóm hình",
                body: &[],
                shapes: vec![group],
                notes: None,
            },
        ],
        &[],
        (5760, 4320),
    );
    vec![
        ("slide/binary-ppt-01-shapes.ppt", shapes),
        ("slide/binary-ppt-02-pictures.ppt", pictures),
        ("slide/binary-ppt-03-text-groups.ppt", text),
    ]
}

/// A BIFF8 workbook with one sheet of numbers, strings (SST) and `A+B` formulas.
fn xls_simple(sheet: &str, rows: &[(&str, f64, f64)]) -> Vec<u8> {
    use crate::xls::tests::rec as r;
    let bof = |dt: u16| {
        let mut d = 0x0600u16.to_le_bytes().to_vec();
        d.extend(dt.to_le_bytes());
        d.extend([0u8; 12]);
        r(0x0809, &d)
    };
    let mut g = bof(0x0005);
    let mut x = 0u16.to_le_bytes().to_vec();
    x.extend(0u16.to_le_bytes());
    x.extend([0u8; 16]);
    g.extend(r(0x00E0, &x));
    let mut sst = u32::try_from(rows.len())
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec();
    sst.extend(u32::try_from(rows.len()).unwrap_or(0).to_le_bytes());
    for (s, _, _) in rows {
        sst.extend(
            u16::try_from(s.encode_utf16().count())
                .unwrap_or(0)
                .to_le_bytes(),
        );
        sst.push(1);
        for u in s.encode_utf16() {
            sst.extend(u.to_le_bytes());
        }
    }
    g.extend(r(0x00FC, &sst));
    let bs_at = g.len();
    let mut bs = vec![0, 0, 0, 0, 0, 0];
    bs.push(u8::try_from(sheet.chars().count()).unwrap_or(0));
    bs.push(1);
    for u in sheet.encode_utf16() {
        bs.extend(u.to_le_bytes());
    }
    g.extend(r(0x0085, &bs));
    g.extend(r(0x000A, &[]));
    let hdr = |row: u16, col: u16| {
        let mut v = row.to_le_bytes().to_vec();
        v.extend(col.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v
    };
    let mut s = bof(0x0010);
    for (i, (_, a, b)) in rows.iter().enumerate() {
        let row = u16::try_from(i).unwrap_or(0);
        let mut l = hdr(row, 0);
        l.extend(u32::try_from(i).unwrap_or(0).to_le_bytes());
        s.extend(r(0x00FD, &l));
        for (col, v) in [(1u16, a), (2, b)] {
            let mut n = hdr(row, col);
            n.extend(v.to_le_bytes());
            s.extend(r(0x0203, &n));
        }
        // D = B + C: PtgRef B(row) PtgRef C(row) PtgAdd (relative refs).
        let mut f = hdr(row, 3);
        f.extend((a + b).to_le_bytes());
        f.extend([0u8; 6]);
        let rgce = [
            0x24,
            row as u8,
            (row >> 8) as u8,
            1,
            0xC0,
            0x24,
            row as u8,
            (row >> 8) as u8,
            2,
            0xC0,
            0x03,
        ];
        f.extend(u16::try_from(rgce.len()).unwrap_or(0).to_le_bytes());
        f.extend(rgce);
        s.extend(r(0x0006, &f));
    }
    s.extend(r(0x000A, &[]));
    let p = u32::try_from(g.len()).unwrap_or(0);
    g[bs_at + 4..bs_at + 8].copy_from_slice(&p.to_le_bytes());
    g.extend(s);
    crate::test_util::cfb_file(&[("/Workbook", g)], &[])
}

/// `.xls` and `.doc` corpus files.
pub fn office97_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "sheet/binary-xls-01-features.xls",
            crate::xls::tests::sample_xls(),
        ),
        (
            "sheet/binary-xls-02-budget.xls",
            xls_simple(
                "Ngân sách",
                &[
                    ("Tiền điện", 1_200_000.0, 150_000.0),
                    ("Tiền nước", 300_000.0, 30_000.0),
                    ("Internet", 250_000.0, 25_000.0),
                ],
            ),
        ),
        (
            "sheet/binary-xls-03-scores.xls",
            xls_simple(
                "Điểm",
                &[
                    ("Toán", 8.5, 9.0),
                    ("Văn", 7.0, 7.5),
                    ("Anh", 9.5, 8.0),
                    ("Lý", 6.5, 7.0),
                ],
            ),
        ),
        (
            "doc/binary-doc-01-sample.doc",
            crate::doc::build::sample_doc(),
        ),
        (
            "doc/binary-doc-02-letter.doc",
            crate::doc::build::simple_doc(
                &[
                    "THƯ MỜI",
                    "Kính gửi: Quý khách hàng",
                    "Công ty trân trọng kính mời Quý khách tới dự buổi giới thiệu sản phẩm mới vào lúc 8 giờ 30, ngày 15/12/2026.",
                    "Trân trọng cảm ơn!",
                ],
                3,
                "Times New Roman",
                26,
            ),
        ),
        (
            "doc/binary-doc-03-report.doc",
            crate::doc::build::simple_doc(
                &[
                    "BÁO CÁO TÓM TẮT",
                    "1. Tình hình thực hiện",
                    "2. Khó khăn, vướng mắc",
                    "3. Kiến nghị, đề xuất",
                    "Người lập báo cáo",
                ],
                0,
                "Arial",
                22,
            ),
        ),
    ]
}

fn corpus_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus")
}

#[test]
fn binary_corpus() {
    let write = std::env::var_os("BO_WRITE_CORPUS").is_some();
    for (name, bytes) in ppt_files().into_iter().chain(office97_files()) {
        let path = corpus_dir().join(name);
        if write {
            std::fs::write(&path, &bytes).unwrap();
            continue;
        }
        let file = std::fs::read(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        // The committed file parses exactly like the generator's output.
        if name.ends_with(".ppt") {
            assert_eq!(
                crate::read_ppt(&file).unwrap(),
                crate::read_ppt(&bytes).unwrap(),
                "{name}"
            );
        } else if name.ends_with(".xls") {
            assert_eq!(
                crate::read_xls(&file).unwrap(),
                crate::read_xls(&bytes).unwrap(),
                "{name}"
            );
        } else {
            assert_eq!(
                crate::read_doc(&file).unwrap(),
                crate::read_doc(&bytes).unwrap(),
                "{name}"
            );
        }
    }
}

#[test]
fn generated_decks_have_shapes_pictures_and_groups() {
    let files = ppt_files();
    let shapes = crate::read_ppt(&files[0].1).unwrap();
    assert_eq!(shapes.slides.len(), 2);
    let s1 = &shapes.slides[0].shapes;
    assert_eq!(s1.len(), 6, "title placeholder + 5 shapes");
    assert_eq!(s1[0].placeholder, Some(crate::PptPlaceholder::Title));
    assert_eq!(
        s1[0].text.as_ref().unwrap().paragraphs[0].runs[0].text,
        "Hình cơ bản"
    );
    assert_eq!(s1[1].fill, Some(0xC0_0000));
    assert_eq!(s1[2].geometry, "ellipse");
    assert_eq!(s1[2].fill, Some(0x2E_75B6), "accent from the master scheme");
    assert_eq!(s1[3].rotation, 30 * 60_000);
    assert_eq!(s1[5].geometry, "line");
    assert_eq!(shapes.slides[0].notes.len(), 1);
    let pics = crate::read_ppt(&files[1].1).unwrap();
    assert_eq!(pics.pictures.len(), 2);
    assert_eq!(pics.slides[0].shapes[2].picture, Some(0));
    assert_eq!(pics.slides[1].shapes[1].picture, Some(0));
    let text = crate::read_ppt(&files[2].1).unwrap();
    let r = &text.slides[0].shapes[2].text.as_ref().unwrap().paragraphs[0].runs[0];
    assert!(r.bold);
    assert_eq!(r.size, Some(24));
    assert_eq!(text.slides[1].shapes.len(), 3, "title + two grouped shapes");
}
