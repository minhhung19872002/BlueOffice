//! Documents through HTML and MHTML: `.docx` → HTML → `.docx` keeps text,
//! structure and the formatting subset; foreign HTML opens as expected; hostile
//! input neither runs nor survives, and never panics.
#![allow(clippy::unwrap_used)]

use bo_html::{read_html, read_html_str, read_mht, write_html, write_mht};
use bo_model::{
    Alignment, Block, Border, Borders, Cell, CellProps, Color, Document, Emu, HalfPoints, Image,
    ParaProps, Paragraph, Row, RowProps, Run, RunContent, RunProps, Section, Style, StyleKind,
    Table, TableProps, TableWidth, Twips, Underline, VMerge, VertAlign,
};
use proptest::prelude::*;
use std::sync::Arc;

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(&vec![0x40; (w * h * 3) as usize])
            .unwrap();
    }
    out
}

fn para(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn cell(text: &str, props: CellProps) -> Cell {
    Cell {
        props,
        blocks: vec![para(text)],
    }
}

/// A document exercising everything the HTML subset carries.
fn sample() -> Document {
    let mut doc = Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..RunProps::default()
        },
        ..Document::default()
    };
    let mut normal = Style::new("Normal", "Normal", StyleKind::Paragraph);
    normal.is_default = true;
    let mut h1 = Style::new("Heading1", "heading 1", StyleKind::Paragraph);
    h1.based_on = Some("Normal".into());
    h1.ppr.outline_level = Some(0);
    h1.rpr.bold = Some(true);
    h1.rpr.size = Some(HalfPoints(32));
    let mut h2 = Style::new("Heading2", "heading 2", StyleKind::Paragraph);
    h2.based_on = Some("Normal".into());
    h2.ppr.outline_level = Some(1);
    h2.rpr.italic = Some(true);
    doc.styles.styles.extend([normal, h1, h2]);
    let bullets = doc.numbering.add_simple_list(true);
    let numbers = doc.numbering.add_simple_list(false);
    doc.media
        .insert("/word/media/image1.png".into(), Arc::new(png(4, 2)));
    doc.footnotes.insert(
        2,
        vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![
                Run {
                    props: RunProps {
                        vert_align: Some(VertAlign::Superscript),
                        ..RunProps::default()
                    },
                    content: vec![RunContent::NoteMark],
                },
                Run::text(" Chú thích cuối trang.", RunProps::default()),
            ],
        ))],
    );
    doc.endnotes
        .insert(1, vec![para("Chú thích cuối văn bản.")]);

    let styled = |style: &str, text: &str| {
        Block::Paragraph(Paragraph::new(
            ParaProps {
                style: Some(style.into()),
                ..ParaProps::default()
            },
            vec![Run::text(text, RunProps::default())],
        ))
    };
    let list = |num_id: i32, ilvl: u8, text: &str| {
        Block::Paragraph(Paragraph::new(
            ParaProps {
                num: Some(bo_model::NumRef { num_id, ilvl }),
                ..ParaProps::default()
            },
            vec![Run::text(text, RunProps::default())],
        ))
    };
    let rp = |f: fn(&mut RunProps)| {
        let mut p = RunProps::default();
        f(&mut p);
        p
    };
    let formatted = Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Justify),
            indent_left: Some(Twips(720)),
            indent_first_line: Some(Twips(-360)),
            space_before: Some(Twips(120)),
            space_after: Some(Twips(240)),
            line_spacing: Some(bo_model::LineSpacing::Auto(360)),
            shading: Some(Color::from_hex("F2F2F2").unwrap()),
            ..ParaProps::default()
        },
        vec![
            Run::text("Đậm", rp(|p| p.bold = Some(true))),
            Run::text(" nghiêng", rp(|p| p.italic = Some(true))),
            Run::text(" gạch chân", rp(|p| p.underline = Some(Underline::Single))),
            Run::text(" gạch ngang", rp(|p| p.strike = Some(true))),
            Run::text(
                " màu đỏ cỡ 18",
                rp(|p| {
                    p.color = Color::from_hex("C00000");
                    p.size = Some(HalfPoints(36));
                    p.font = Some("Arial".into());
                }),
            ),
            Run::text(" H", RunProps::default()),
            Run::text("2", rp(|p| p.vert_align = Some(VertAlign::Subscript))),
            Run::text("O", RunProps::default()),
            Run::text(
                " tô sáng",
                rp(|p| p.highlight = Color::from_highlight("yellow")),
            ),
            Run::text(
                " liên kết",
                rp(|p| p.link = Some("https://example.test/a?b=1&c=2".into())),
            ),
            Run {
                props: rp(|p| p.vert_align = Some(VertAlign::Superscript)),
                content: vec![RunContent::FootnoteRef(2)],
            },
            Run {
                props: rp(|p| p.vert_align = Some(VertAlign::Superscript)),
                content: vec![RunContent::EndnoteRef(1)],
            },
        ],
    ));
    let spaces = Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![
                RunContent::Text("  hai  dấu cách ".into()),
                RunContent::Tab,
                RunContent::Text("sau tab".into()),
                RunContent::LineBreak,
                RunContent::Text("dòng mới <&>".into()),
            ],
        }],
    ));
    let picture = Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Center),
            ..ParaProps::default()
        },
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::Image(Box::new(Image {
                media: "/word/media/image1.png".into(),
                width: Emu(914_400),
                height: Emu(457_200),
                anchor: None,
                crop: None,
                rotation: 0,
                description: "Ảnh \"mẫu\"".into(),
                shape: None,
                format: None,
                ole: None,
                src: None,
            }))],
        }],
    ));
    let w = |n: i32| Some(TableWidth::Dxa(Twips(n)));
    let table = Block::Table(Table {
        props: TableProps {
            width: w(6000),
            borders: Some(Borders::all(&Border::single(4))),
            ..TableProps::default()
        },
        grid: vec![Twips(2000); 3],
        grid_change: None,
        rows: vec![
            Row {
                props: RowProps {
                    header: true,
                    ..RowProps::default()
                },
                cells: vec![
                    cell(
                        "Gộp ngang",
                        CellProps {
                            width: w(4000),
                            grid_span: 2,
                            shading: Color::from_hex("DDEBF7"),
                            ..CellProps::default()
                        },
                    ),
                    cell(
                        "Gộp dọc",
                        CellProps {
                            width: w(2000),
                            v_merge: Some(VMerge::Restart),
                            ..CellProps::default()
                        },
                    ),
                ],
            },
            Row {
                props: RowProps::default(),
                cells: vec![
                    cell(
                        "a",
                        CellProps {
                            width: w(2000),
                            ..CellProps::default()
                        },
                    ),
                    cell(
                        "b",
                        CellProps {
                            width: w(2000),
                            ..CellProps::default()
                        },
                    ),
                    cell(
                        "",
                        CellProps {
                            width: w(2000),
                            v_merge: Some(VMerge::Continue),
                            ..CellProps::default()
                        },
                    ),
                ],
            },
        ],
        src: None,
    });
    doc.sections = vec![Section::new(vec![
        styled("Heading1", "Tiêu đề lớn"),
        formatted,
        styled("Heading2", "Mục con"),
        list(bullets, 0, "chấm một"),
        list(bullets, 1, "chấm lồng"),
        list(bullets, 0, "chấm hai"),
        list(numbers, 0, "số một"),
        list(numbers, 0, "số hai"),
        spaces,
        Block::Paragraph(Paragraph::default()),
        picture,
        table,
        para("Hết."),
    ])];
    doc
}

fn body(doc: &Document) -> &[Block] {
    &doc.sections[0].blocks
}

fn paragraphs(blocks: &[Block]) -> Vec<&Paragraph> {
    blocks.iter().filter_map(Block::as_paragraph).collect()
}

/// What must survive: checked on the document read back from HTML.
fn check(back: &Document, original: &Document) {
    let (a, b) = (body(back), body(original));
    assert_eq!(a.len(), b.len(), "block count");
    for (x, y) in a.iter().zip(b) {
        match (x, y) {
            (Block::Paragraph(p), Block::Paragraph(q)) => {
                assert_eq!(p.plain_text(), q.plain_text());
            }
            (Block::Table(_), Block::Table(_)) => {}
            _ => panic!("block kinds differ"),
        }
    }
    let ps = paragraphs(a);
    // Headings keep their level.
    assert_eq!(ps[0].props.style.as_deref(), Some("Heading1"));
    assert_eq!(ps[2].props.style.as_deref(), Some("Heading2"));
    assert_eq!(
        back.styles.resolve_paragraph("Heading1").0.outline_level,
        Some(0)
    );
    // Paragraph formatting.
    let f = &ps[1].props;
    assert_eq!(f.align, Some(Alignment::Justify));
    assert_eq!(f.indent_left, Some(Twips(720)));
    assert_eq!(f.indent_first_line, Some(Twips(-360)));
    assert_eq!(f.space_before, Some(Twips(120)));
    assert_eq!(f.space_after, Some(Twips(240)));
    assert_eq!(f.line_spacing, Some(bo_model::LineSpacing::Auto(360)));
    assert_eq!(f.shading, Color::from_hex("F2F2F2"));
    // Run formatting.
    let run = |text: &str| -> &Run {
        ps[1]
            .runs
            .iter()
            .find(|r| r.plain_text().trim() == text)
            .unwrap_or_else(|| panic!("run {text:?} missing: {:?}", ps[1].runs))
    };
    assert_eq!(run("Đậm").props.bold, Some(true));
    assert_eq!(run("nghiêng").props.italic, Some(true));
    assert_eq!(run("gạch chân").props.underline, Some(Underline::Single));
    assert_eq!(run("gạch ngang").props.strike, Some(true));
    let red = &run("màu đỏ cỡ 18").props;
    assert_eq!(red.color, Color::from_hex("C00000"));
    assert_eq!(red.size, Some(HalfPoints(36)));
    assert_eq!(red.font.as_deref(), Some("Arial"));
    assert_eq!(run("2").props.vert_align, Some(VertAlign::Subscript));
    assert_eq!(
        run("tô sáng").props.highlight,
        Color::from_highlight("yellow")
    );
    assert_eq!(
        run("liên kết").props.link.as_deref(),
        Some("https://example.test/a?b=1&c=2")
    );
    // Notes come back as notes.
    assert_eq!(back.footnotes.len(), 1);
    assert_eq!(back.endnotes.len(), 1);
    let note = back.footnotes.values().next().unwrap();
    let note_p = note[0].as_paragraph().unwrap();
    assert!(note_p.plain_text().contains("Chú thích cuối trang."));
    assert!(note_p
        .runs
        .iter()
        .any(|r| r.content.contains(&RunContent::NoteMark)));
    let fid = *back.footnotes.keys().next().unwrap();
    assert!(ps[1]
        .runs
        .iter()
        .any(|r| r.content.contains(&RunContent::FootnoteRef(fid))));
    assert!(ps[1].runs.iter().any(|r| r
        .content
        .iter()
        .any(|c| matches!(c, RunContent::EndnoteRef(_)))));
    // Lists: two bullet levels, then a numbered list of its own.
    let num = |i: usize| ps[i].props.num.unwrap();
    let is_bullet = |i: usize| {
        matches!(
            back.numbering.level(num(i)).unwrap().format,
            bo_model::NumFormat::Bullet
        )
    };
    assert!(is_bullet(3) && is_bullet(4) && is_bullet(5));
    assert_eq!((num(3).ilvl, num(4).ilvl, num(5).ilvl), (0, 1, 0));
    assert_eq!(num(3).num_id, num(5).num_id);
    assert!(!is_bullet(6));
    assert_eq!(num(6).num_id, num(7).num_id);
    assert_ne!(num(6).num_id, num(3).num_id);
    assert!(ps[8].props.num.is_none());
    // White space, tab and line break.
    assert_eq!(
        ps[8]
            .runs
            .iter()
            .flat_map(|r| r.content.clone())
            .collect::<Vec<_>>(),
        vec![
            RunContent::Text("  hai  dấu cách ".into()),
            RunContent::Tab,
            RunContent::Text("sau tab".into()),
            RunContent::LineBreak,
            RunContent::Text("dòng mới <&>".into()),
        ]
    );
    assert_eq!(ps[9].runs, Vec::new());
    // Picture: same bytes, same size, same description.
    let img = ps[10]
        .runs
        .iter()
        .flat_map(|r| &r.content)
        .find_map(|c| match c {
            RunContent::Image(i) => Some(i),
            _ => None,
        })
        .expect("picture");
    assert_eq!(
        back.media[&img.media],
        original.media["/word/media/image1.png"]
    );
    assert_eq!((img.width, img.height), (Emu(914_400), Emu(457_200)));
    assert_eq!(img.description, "Ảnh \"mẫu\"");
    assert_eq!(ps[10].props.align, Some(Alignment::Center));
    // Table: grid, spans, merge, shading, borders, header row.
    let Block::Table(t) = &a[11] else {
        panic!("table")
    };
    assert_eq!(t.grid, vec![Twips(2000); 3]);
    assert_eq!(t.rows.len(), 2);
    assert!(t.rows[0].props.header);
    let c = &t.rows[0].cells;
    assert_eq!((c[0].props.grid_span, c[1].props.grid_span), (2, 1));
    assert_eq!(c[0].props.shading, Color::from_hex("DDEBF7"));
    assert_eq!(c[1].props.v_merge, Some(VMerge::Restart));
    assert_eq!(t.rows[1].cells.len(), 3);
    assert_eq!(t.rows[1].cells[2].props.v_merge, Some(VMerge::Continue));
    assert!(c[0]
        .props
        .borders
        .as_ref()
        .is_some_and(|b| b.top.is_some() && b.left.is_some()));
    assert_eq!(
        t.rows[1].cells[0].blocks[0]
            .as_paragraph()
            .unwrap()
            .plain_text(),
        "a"
    );
}

#[test]
fn document_survives_html() {
    let doc = sample();
    let html = write_html(&doc, "Mẫu <1>");
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>Mẫu &lt;1&gt;</title>"));
    assert!(
        html.contains("<h1 class=\"s-Heading1\">Tiêu đề lớn</h1>"),
        "{html}"
    );
    assert!(html.contains("src=\"data:image/png;base64,"));
    assert!(html.contains("<ul>") && html.contains("<ol>"));
    assert!(!html.contains("<script"));
    check(&read_html(html.as_bytes()), &doc);
}

#[test]
fn docx_survives_html_and_back_to_docx() {
    let doc = sample();
    // .docx → model → HTML → model → .docx → model.
    let docx = bo_docx::write_docx(&doc).unwrap();
    let opened = bo_docx::read_docx(&docx).unwrap();
    let html = write_html(&opened, "t");
    let from_html = read_html(html.as_bytes());
    check(&from_html, &doc);
    let docx2 = bo_docx::write_docx(&from_html).unwrap();
    let again = bo_docx::read_docx(&docx2).unwrap();
    check(&again, &doc);
}

#[test]
fn document_survives_mhtml() {
    let doc = sample();
    let mht = write_mht(&doc, "Mẫu");
    let text = String::from_utf8(mht.clone()).unwrap();
    assert!(bo_html::is_mht(&mht));
    assert!(text.is_ascii(), "MHTML must be 7-bit clean");
    assert!(text.starts_with(
        "From: <Saved by BlueOffice>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/related;"
    ));
    assert!(text.contains("Content-Transfer-Encoding: quoted-printable"));
    assert!(text.contains("Content-Type: image/png\r\nContent-Transfer-Encoding: base64\r\nContent-ID: <image1.png@blueoffice>"));
    assert!(
        text.lines()
            .all(|l| l.len() <= 76 || l.starts_with("Content-")),
        "long line"
    );
    // The same document always gives the same bytes.
    assert_eq!(write_mht(&doc, "Mẫu"), mht);
    check(&read_mht(&mht).unwrap(), &doc);
}

#[test]
fn foreign_html_opens() {
    let doc = read_html_str(
        r#"<!DOCTYPE html><html><head><title>Trang</title>
        <style>
          body { font-family: Arial, sans-serif; font-size: 11pt }
          .lead { color: #336699; text-align: center; margin: 6pt 0 12pt 24pt }
          td.num { text-align: right; background: #eee }
        </style></head>
        <body>
          <h2>Tiêu   đề
              hai</h2>
          <p class="lead">Mở <b>đậm <i>cả hai</i></b> <font color="red" size="5">đỏ</font></p>
          Văn bản trần<br>dòng hai
          <div style="margin-left: 1in">thụt <span style="font-weight:700;text-decoration:underline">vào</span></div>
          <ol start="3" type="a"><li>ba<li>bốn<ul><li>con</ul></ol>
          <blockquote>trích</blockquote>
          <pre>
mã  nguồn
  thụt</pre>
          <table border="1" width="50%">
            <thead><tr><th>A<th>B</tr></thead>
            <tr><td rowspan="2">1<td class="num">2
            <tr><td>3
          </table>
          <p>Ảnh ngoài <img src="http://example.test/x.png" alt="[logo]"> và
             <a href="mailto:a@example.test">thư</a> <a href="javascript:alert(1)">xấu</a></p>
          <hr>
          <p style="display:none">ẩn</p>
        </body></html>"#,
        None,
    );
    assert_eq!(doc.default_run.font.as_deref(), Some("Arial"));
    assert_eq!(doc.default_run.size, Some(HalfPoints(22)));
    let blocks = body(&doc);
    let texts: Vec<String> = blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(p) => p.plain_text(),
            Block::Table(_) => "<table>".into(),
            Block::Opaque(_) => String::new(),
        })
        .collect();
    assert_eq!(
        texts,
        vec![
            "Tiêu đề hai",
            "Mở đậm cả hai đỏ",
            "Văn bản trần\u{2028}dòng hai",
            "thụt vào",
            "ba",
            "bốn",
            "con",
            "trích",
            "mã  nguồn\u{2028}  thụt",
            "<table>",
            "Ảnh ngoài [logo] và thư xấu",
            "",
        ],
    );
    let ps = paragraphs(blocks);
    assert_eq!(ps[0].props.style.as_deref(), Some("Heading2"));
    let lead = ps[1];
    assert_eq!(lead.props.align, Some(Alignment::Center));
    assert_eq!(lead.props.indent_left, Some(Twips(480)));
    assert_eq!(lead.props.space_before, Some(Twips(120)));
    assert_eq!(lead.props.space_after, Some(Twips(240)));
    assert_eq!(lead.runs[0].props.color, Color::from_hex("336699"));
    let both = lead
        .runs
        .iter()
        .find(|r| r.plain_text().trim() == "cả hai")
        .unwrap();
    assert_eq!(
        (both.props.bold, both.props.italic),
        (Some(true), Some(true))
    );
    let red = lead
        .runs
        .iter()
        .find(|r| r.plain_text().trim() == "đỏ")
        .unwrap();
    assert_eq!(red.props.color, Color::from_hex("FF0000"));
    assert_eq!(red.props.size, Some(HalfPoints(36)));
    assert_eq!(ps[3].props.indent_left, Some(Twips(1440)));
    let under = ps[3]
        .runs
        .iter()
        .find(|r| r.plain_text().trim() == "vào")
        .unwrap();
    assert_eq!(under.props.bold, Some(true));
    assert_eq!(under.props.underline, Some(Underline::Single));
    // The ordered list starts at c. with lower-case letters; the nested list is bullets.
    let n = ps[4].props.num.unwrap();
    let level = doc.numbering.level(n).unwrap();
    assert_eq!(level.format, bo_model::NumFormat::LowerLetter);
    assert_eq!(doc.numbering.start(n), 3);
    assert_eq!(ps[5].props.num, Some(n));
    let nested = ps[6].props.num.unwrap();
    assert_eq!(nested.ilvl, 1);
    assert_eq!(
        doc.numbering.level(nested).unwrap().format,
        bo_model::NumFormat::Bullet
    );
    assert_eq!(ps[7].props.indent_left, Some(Twips(720)));
    assert_eq!(ps[8].runs[0].props.font.as_deref(), Some("Courier New"));
    let Block::Table(t) = &blocks[9] else {
        panic!()
    };
    assert_eq!(t.grid.len(), 2);
    assert!(t.props.borders.is_some());
    assert!(t.rows[0].props.header);
    assert_eq!(t.rows[1].cells[0].props.v_merge, Some(VMerge::Restart));
    assert_eq!(t.rows[2].cells[0].props.v_merge, Some(VMerge::Continue));
    assert_eq!(t.rows[1].cells[1].props.shading, Color::from_hex("EEEEEE"));
    let two = t.rows[1].cells[1].blocks[0].as_paragraph().unwrap();
    assert_eq!(two.props.align, Some(Alignment::Right));
    let head = t.rows[0].cells[0].blocks[0].as_paragraph().unwrap();
    assert_eq!(head.runs[0].props.bold, Some(true));
    // Links: mail kept, script URL dropped (its text stays).
    let links: Vec<Option<&str>> = ps[9].runs.iter().map(|r| r.props.link.as_deref()).collect();
    assert!(links.contains(&Some("mailto:a@example.test")));
    assert!(!links.iter().flatten().any(|l| l.contains("javascript")));
    assert!(doc.media.is_empty());
    // The page saves as a valid package.
    let docx = bo_docx::write_docx(&doc).unwrap();
    assert_eq!(
        bo_docx::read_docx(&docx).unwrap().plain_text(),
        doc.plain_text()
    );
}

#[test]
fn scripts_and_remote_content_never_survive() {
    let html = r#"<html><head><script>alert(1)</script><script src="http://evil.test/x.js"></script></head>
        <body onload="evil()"><p onclick="evil()">a<script>document.write("<p>b</p>")</script>c</p>
        <img src="http://evil.test/track.gif"><img src="file:///etc/passwd" alt="">
        <iframe src="http://evil.test/"></iframe><object data="x.swf"></object>
        <a href="vbscript:msgbox(1)">d</a><a href=" JaVaScRiPt:alert(1)">e</a>
        <svg><script>alert(2)</script></svg><style>p{background:url(http://evil.test/bg.png)}</style>
        <link rel="stylesheet" href="http://evil.test/s.css"><meta http-equiv="refresh" content="0;url=http://evil.test/">
        </body></html>"#;
    let doc = read_html_str(html, None);
    assert_eq!(
        doc.plain_text().split_whitespace().collect::<String>(),
        "acde"
    );
    assert!(doc.media.is_empty());
    let all_links: Vec<&str> = doc
        .paragraphs()
        .flat_map(|p| p.runs.iter().filter_map(|r| r.props.link.as_deref()))
        .collect();
    assert!(all_links.is_empty(), "{all_links:?}");
    let out = write_html(&doc, "x");
    for bad in ["script", "evil", "onload", "onclick", "iframe", "alert"] {
        assert!(!out.to_ascii_lowercase().contains(bad), "{bad} in {out}");
    }
}

#[test]
fn encodings_are_detected() {
    let cp1258 = b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=windows-1258\"></head><body><p>Vi\xEA\xF2t Nam</p></body></html>";
    assert_eq!(read_html(cp1258).plain_text().trim(), "Việt Nam");
    let latin1 = b"<p>caf\xE9</p>";
    assert_eq!(read_html(latin1).plain_text().trim(), "café");
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend(
        "<p>Tiếng Việt</p>"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    assert_eq!(read_html(&utf16).plain_text().trim(), "Tiếng Việt");
}

#[test]
fn deep_and_wide_input_is_bounded() {
    // Deep nesting of every block kind.
    for tag in [
        "div",
        "ul><li",
        "table><tr><td",
        "blockquote",
        "b",
        "ol><li><p",
    ] {
        let open = format!("<{tag}>").repeat(3000);
        let doc = read_html_str(&format!("{open}x"), None);
        assert!(doc.plain_text().contains('x'), "{tag}");
        let _ = bo_docx::write_docx(&doc).unwrap();
        let _ = write_html(&doc, "");
    }
    // Hostile spans.
    let doc = read_html_str(
        "<table><tr><td colspan=100000 rowspan=100000>a<td>b<tr><td>c</table>",
        None,
    );
    let Block::Table(t) = &body(&doc)[0] else {
        panic!()
    };
    assert!(t.grid.len() <= 63);
    let _ = bo_docx::write_docx(&doc).unwrap();
    // Thousands of lists do not grow the numbering without bound.
    let doc = read_html_str(&"<ul><li>a</ul><ol><li>b</ol>".repeat(3000), None);
    assert!(doc.numbering.nums.len() <= 2001);
}

const SEEDS: &[&str] = &[
    "<html><head><style>p{color:red}.a{margin:1em 2em}</style></head><body><h1>T</h1><p class=a>x <b>y</b></p>",
    "<table border=1><tr><td rowspan=2 colspan=2>a<td>b<tr><td><table><tr><th>n</table></table>",
    "<ul><li>a<ol start=5 type=i><li>b</ol><li>c</ul><pre>\n x\ty\n</pre><img src='data:image/png;base64,iVBORw0KGgo='>",
    "<p style='line-height:150%;text-indent:-2em;border:1px solid #000'>x<br><br>y<a href='#z' class=bo-noteref>1</a></p><div class=bo-note id=z>n</div>",
];

const TOKENS: &[&str] = &[
    "<",
    ">",
    "</",
    "<p>",
    "</p>",
    "<table>",
    "<tr>",
    "<td colspan=9999>",
    "</table>",
    "<li>",
    "<ul>",
    "</ul>",
    "&#x110000;",
    "&",
    "&nbsp",
    "<!--",
    "-->",
    "<script>",
    "</script>",
    "<style>",
    "}",
    "{",
    "style='",
    "'",
    "\"",
    "=",
    "<img src=data:;base64,AAAA>",
    "<br>",
    "<a class=bo-noteref href=#z>",
    "\u{0}",
    "\u{feff}",
    "<pre>",
    "<h7>",
    "<td rowspan=0>",
    "<col span=99999 width=1e9>",
    " style='margin:1e30pt;font-size:1e30px;width:-5%;line-height:0' ",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn mutated_html_never_panics(seed in 0usize..4, edits in proptest::collection::vec((any::<u16>(), 0usize..36, 0u8..4), 1..12)) {
        let mut s = SEEDS[seed].to_owned();
        for (at, tok, kind) in edits {
            let mut i = usize::from(at) % (s.len() + 1);
            while !s.is_char_boundary(i) { i -= 1; }
            match kind {
                0 => s.insert_str(i, TOKENS[tok]),
                1 => s.truncate(i),
                2 => { let dup = s[i..].to_owned(); s.insert_str(i, &dup.chars().take(40).collect::<String>()); }
                _ => { s.insert_str(i, TOKENS[tok]); s.insert_str(0, TOKENS[(tok + 7) % TOKENS.len()]); }
            }
        }
        let doc = read_html_str(&s, None);
        let html = write_html(&doc, "t");
        let again = read_html_str(&html, None);
        let words = |d: &Document| d.plain_text().split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        prop_assert_eq!(words(&again), words(&doc));
        let _ = bo_html::sheet::read_workbook_html_str(&s);
        let _ = bo_docx::write_docx(&doc).unwrap();
    }

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..600)) {
        let _ = read_html(&bytes);
        let _ = read_mht(&bytes);
        let _ = bo_html::is_html(&bytes);
        let _ = bo_html::is_mht(&bytes);
        let _ = bo_html::read_workbook_html(&bytes);
        let mut mime = b"MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=b\r\n\r\n--b\r\nContent-Type: text/html\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n".to_vec();
        mime.extend(&bytes);
        let _ = read_mht(&mime);
    }
}
