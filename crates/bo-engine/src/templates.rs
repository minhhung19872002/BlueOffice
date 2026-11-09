//! Built-in document templates, authored in code by BlueOffice.
//!
//! `nd30`: Vietnamese administrative document following the presentation rules of
//! Nghị định 30/2020/NĐ-CP (Phụ lục I): A4 portrait, margins top/bottom 20 mm, left
//! 30 mm, right 15 mm, Times New Roman 13 pt; national title (quốc hiệu) and motto
//! (tiêu ngữ), issuing body, number/symbol (số, ký hiệu), place and date, document
//! type and summary, body, recipients (nơi nhận) and signature block; page numbers
//! centred in the top margin from the second page.

use bo_model::{
    Alignment, Block, Border, Borders, Cell, CellProps, Document, HalfPoints, HeaderFooter,
    LineSpacing, PageSetup, ParaProps, Paragraph, Row, RowProps, Run, RunContent, RunProps,
    Section, Table, TableProps, TableWidth, Twips,
};

/// Template ids offered by "File → New from template".
pub const TEMPLATES: &[&str] = &["nd30"];

/// Build a template document.
pub fn build(id: &str) -> Option<Document> {
    match id {
        "nd30" => Some(nd30()),
        _ => None,
    }
}

/// Text style of one template line.
#[derive(Clone, Copy, Default)]
struct Look {
    size: u32,
    bold: bool,
    italic: bool,
}

fn look(size: u32) -> Look {
    Look {
        size,
        ..Look::default()
    }
}

impl Look {
    fn bold(self) -> Look {
        Look { bold: true, ..self }
    }
    fn italic(self) -> Look {
        Look {
            italic: true,
            ..self
        }
    }
    /// §17.3.2 run properties: sz (half-points), b, i.
    fn props(self) -> RunProps {
        RunProps {
            size: (self.size > 0).then_some(HalfPoints(self.size * 2)),
            bold: self.bold.then_some(true),
            italic: self.italic.then_some(true),
            ..RunProps::default()
        }
    }
}

fn line(text: &str, l: Look, align: Alignment) -> Block {
    para(vec![Run::text(text, l.props())], align, None)
}

/// §17.3.1 paragraph: jc, spacing (before/after/line), ind firstLine.
fn para(runs: Vec<Run>, align: Alignment, first_line: Option<i32>) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(align),
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            indent_first_line: first_line.map(Twips),
            ..ParaProps::default()
        },
        runs,
    ))
}

/// Body paragraph: justified, first line indented 1 cm, 6 pt after, 1.2 lines
/// (Phụ lục I: paragraph spacing ≥ 6 pt, line spacing single to 1.5 lines).
fn body(runs: Vec<Run>) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps {
            align: Some(Alignment::Justify),
            indent_first_line: Some(Twips(567)),
            space_before: Some(Twips(0)),
            space_after: Some(Twips(120)),
            line_spacing: Some(LineSpacing::Auto(288)),
            ..ParaProps::default()
        },
        runs,
    ))
}

fn nil() -> Border {
    Border {
        style: "nil".into(),
        size: 0,
        color: None,
        space: 0,
    }
}

/// A two-column layout table without borders (§17.4.39 tblBorders = nil).
fn two_columns(widths: [i32; 2], left: Vec<Block>, right: Vec<Block>) -> Block {
    let cell = |w: i32, blocks: Vec<Block>| Cell {
        props: CellProps {
            width: Some(TableWidth::Dxa(Twips(w))),
            ..CellProps::default()
        },
        blocks,
    };
    Block::Table(Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(Twips(widths[0] + widths[1]))),
            borders: Some(Borders::all(&nil())),
            ..TableProps::default()
        },
        grid: vec![Twips(widths[0]), Twips(widths[1])],
        grid_change: None,
        rows: vec![Row {
            props: RowProps::default(),
            cells: vec![cell(widths[0], left), cell(widths[1], right)],
        }],
        src: None,
    })
}

fn nd30() -> Document {
    // A4 (§17.6.13 pgSz 11906 × 16838 twips), margins in twips (1 mm ≈ 56.7 twips).
    let page = PageSetup {
        width: Twips(11_906),
        height: Twips(16_838),
        margin_top: Twips(1134),
        margin_bottom: Twips(1134),
        margin_left: Twips(1701),
        margin_right: Twips(851),
        header: Twips(567),
        footer: Twips(567),
    };
    let text_width = page.width.0 - page.margin_left.0 - page.margin_right.0;
    let left_w = text_width * 2 / 5;
    let c = Alignment::Center;
    let rule = || line("—————", look(13), Alignment::Center);

    let issuer = vec![
        line("TÊN CQ, TC CHỦ QUẢN", look(12), c),
        line("TÊN CƠ QUAN, TỔ CHỨC", look(12).bold(), c),
        rule(),
        line("Số: …/…-…", look(13), c),
    ];
    let national = vec![
        line("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", look(12).bold(), c),
        line("Độc lập - Tự do - Hạnh phúc", look(13).bold(), c),
        rule(),
        line("…, ngày … tháng … năm 20…", look(13).italic(), c),
    ];
    let mut blocks = vec![two_columns([left_w, text_width - left_w], issuer, national)];
    blocks.push(line("", look(13), c));
    blocks.push(line("TÊN LOẠI VĂN BẢN", look(14).bold(), c));
    blocks.push(line("Trích yếu nội dung văn bản", look(14).bold(), c));
    blocks.push(rule());
    blocks.push(line("", look(13), c));
    blocks.push(body(vec![Run::text(
        "Căn cứ …;",
        look(13).italic().props(),
    )]));
    blocks.push(body(vec![Run::text(
        "Nội dung văn bản trình bày bằng chữ in thường, căn đều hai lề, lùi đầu dòng 1 cm; khoảng cách giữa các đoạn văn tối thiểu 6 pt.",
        look(13).props(),
    )]));
    blocks.push(body(vec![Run::text("…", look(13).props())]));
    blocks.push(line("", look(13), c));
    let recipients = vec![
        line("Nơi nhận:", look(12).bold().italic(), Alignment::Left),
        line("- Như trên;", look(11), Alignment::Left),
        line("- …;", look(11), Alignment::Left),
        line("- Lưu: VT, ….", look(11), Alignment::Left),
    ];
    let signer = vec![
        line("QUYỀN HẠN, CHỨC VỤ CỦA NGƯỜI KÝ", look(13).bold(), c),
        line(
            "(Chữ ký của người có thẩm quyền, dấu/chữ ký số của cơ quan, tổ chức)",
            look(13).italic(),
            c,
        ),
        line("", look(13), c),
        line("", look(13), c),
        line("Họ và tên", look(13).bold(), c),
    ];
    blocks.push(two_columns(
        [left_w, text_width - left_w],
        recipients,
        signer,
    ));
    blocks.push(line("", look(13), Alignment::Left));

    // Page number centred in the top margin, not on the first page (§17.10.6 titlePg,
    // §17.16.5.43 PAGE).
    let number = para(
        vec![Run {
            props: look(13).props(),
            content: vec![RunContent::Field {
                instr: "PAGE".into(),
                cached: "2".into(),
                locked: false,
            }],
        }],
        c,
        None,
    );
    let headers = HeaderFooter {
        default: Some(vec![number]),
        first: Some(vec![line("", look(13), c)]),
        even: None,
    };
    Document {
        sections: vec![Section {
            page,
            headers,
            title_page: true,
            ..Section::new(blocks)
        }],
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..RunProps::default()
        },
        ..Document::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nd30_template_follows_the_decree() {
        let d = build("nd30").expect("template");
        let s = &d.sections[0];
        assert_eq!((s.page.width.0, s.page.height.0), (11_906, 16_838));
        // Margins: top/bottom 20–25 mm, left 30–35 mm, right 15–20 mm.
        let mm = |t: Twips| f64::from(t.0) / 56.692;
        assert!((20.0..=25.1).contains(&mm(s.page.margin_top)));
        assert!((20.0..=25.1).contains(&mm(s.page.margin_bottom)));
        assert!((30.0..=35.1).contains(&mm(s.page.margin_left)));
        assert!((15.0..=20.1).contains(&mm(s.page.margin_right)));
        assert!(s.title_page);
        let text = d.plain_text();
        for part in [
            "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM",
            "Độc lập - Tự do - Hạnh phúc",
            "Số:",
            "Nơi nhận:",
        ] {
            assert!(text.contains(part), "{part}");
        }
        assert!(build("nope").is_none());
    }
}
