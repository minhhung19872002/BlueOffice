//! The Phase 1 corpus sample: a fictitious administrative document laid out
//! after the presentation rules of Nghị định 30/2020/NĐ-CP (A4, 13–14 pt
//! Times New Roman, justified body, first-line indent). All names and numbers
//! are invented.

use bo_model::{
    Alignment, Block, Color, Document, HalfPoints, LineSpacing, PageSetup, ParaProps, Paragraph,
    Run, RunContent, RunProps, Section, Twips, Underline,
};

fn rp(size: u32) -> RunProps {
    RunProps {
        size: Some(HalfPoints(size)),
        ..Default::default()
    }
}

fn bold(size: u32) -> RunProps {
    RunProps {
        bold: Some(true),
        ..rp(size)
    }
}

fn p(align: Alignment, runs: Vec<Run>) -> Block {
    Block::Paragraph(Paragraph {
        src: None,
        props: ParaProps {
            align: Some(align),
            space_after: Some(Twips(0)),
            ..Default::default()
        },
        runs,
    })
}

fn body(text: &str) -> Block {
    Block::Paragraph(Paragraph {
        src: None,
        props: ParaProps {
            align: Some(Alignment::Justify),
            indent_first_line: Some(Twips(567)),
            space_before: Some(Twips(60)),
            space_after: Some(Twips(60)),
            line_spacing: Some(LineSpacing::Auto(276)),
            ..Default::default()
        },
        runs: vec![Run::text(text, rp(28))],
    })
}

const ARTICLES: &[&str] = &[
    "Văn bản này được soạn thảo bằng BlueDoc để kiểm tra khả năng hiển thị tiếng Việt có dấu, căn lề đều hai bên, thụt lề dòng đầu và giãn dòng theo bội số. Mọi tên cơ quan, số hiệu và nội dung trong văn bản đều là giả định, chỉ phục vụ mục đích kiểm thử.",
    "Phạm vi kiểm thử gồm: ngắt dòng theo chuẩn Unicode, đo chữ bằng phông chữ tương thích kích thước (Tinos thay cho Times New Roman, Liberation Sans thay cho Arial, Carlito thay cho Calibri), phân trang tự động khi nội dung vượt quá chiều cao vùng soạn thảo và ngắt trang thủ công.",
    "Các đơn vị chủ trì phối hợp với bộ phận công nghệ thông tin rà soát kết quả hiển thị trên trình duyệt và ứng dụng máy tính để bàn; kết quả phải giống nhau tuyệt đối vì cùng sử dụng một lõi dàn trang. Tệp PDF xuất ra phải nhúng phông chữ và giữ được khả năng tìm kiếm văn bản.",
    "Trong quá trình thực hiện, nếu phát sinh vướng mắc, đề nghị các cơ quan, tổ chức, cá nhân phản ánh kịp thời về đầu mối tiếp nhận để tổng hợp, báo cáo cấp có thẩm quyền xem xét, quyết định.",
];

/// Build the sample document.
pub fn document() -> Document {
    let navy = Color {
        r: 0x1F,
        g: 0x3A,
        b: 0x68,
    };
    let mut blocks = vec![
        p(
            Alignment::Center,
            vec![Run::text("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", bold(26))],
        ),
        p(
            Alignment::Center,
            vec![Run::text(
                "Độc lập - Tự do - Hạnh phúc",
                RunProps {
                    underline: Some(Underline::Single),
                    ..bold(28)
                },
            )],
        ),
        p(
            Alignment::Right,
            vec![Run::text(
                "Hà Nội, ngày 05 tháng 11 năm 2026",
                RunProps {
                    italic: Some(true),
                    ..rp(28)
                },
            )],
        ),
        p(Alignment::Left, vec![Run::text("", rp(28))]),
        p(
            Alignment::Center,
            vec![Run::text(
                "THÔNG BÁO",
                RunProps {
                    color: Some(navy),
                    ..bold(32)
                },
            )],
        ),
        p(
            Alignment::Center,
            vec![Run::text(
                "Về việc thử nghiệm bộ ứng dụng văn phòng BlueOffice",
                bold(28),
            )],
        ),
        p(Alignment::Center, vec![Run::text("________", rp(28))]),
    ];
    for (i, a) in ARTICLES.iter().enumerate() {
        blocks.push(Block::Paragraph(Paragraph {
            src: None,
            props: ParaProps {
                align: Some(Alignment::Justify),
                indent_first_line: Some(Twips(567)),
                space_before: Some(Twips(120)),
                ..Default::default()
            },
            runs: vec![
                Run::text(format!("Điều {}. ", i + 1), bold(28)),
                Run::text(*a, rp(28)),
            ],
        }));
    }
    blocks.push(Block::Paragraph(Paragraph {
        src: None,
        props: ParaProps {
            align: Some(Alignment::Left),
            indent_left: Some(Twips(567)),
            ..Default::default()
        },
        runs: vec![
            Run::text("Định dạng ký tự: ", rp(28)),
            Run::text("đậm", bold(28)),
            Run::text(", ", rp(28)),
            Run::text(
                "nghiêng",
                RunProps {
                    italic: Some(true),
                    ..rp(28)
                },
            ),
            Run::text(", ", rp(28)),
            Run::text(
                "gạch chân",
                RunProps {
                    underline: Some(Underline::Single),
                    ..rp(28)
                },
            ),
            Run::text(", ", rp(28)),
            Run::text(
                "gạch kép",
                RunProps {
                    underline: Some(Underline::Double),
                    ..rp(28)
                },
            ),
            Run::text(", ", rp(28)),
            Run::text(
                "gạch ngang",
                RunProps {
                    strike: Some(true),
                    ..rp(28)
                },
            ),
            Run::text(", ", rp(28)),
            Run::text(
                "màu đỏ",
                RunProps {
                    color: Some(Color {
                        r: 0xC0,
                        g: 0,
                        b: 0,
                    }),
                    ..rp(28)
                },
            ),
            Run::text(", ", rp(28)),
            Run::text(
                "Arial",
                RunProps {
                    font: Some("Arial".into()),
                    ..rp(26)
                },
            ),
            Run::text(", ", rp(28)),
            Run::text(
                "Calibri",
                RunProps {
                    font: Some("Calibri".into()),
                    ..rp(26)
                },
            ),
            Run::text(".", rp(28)),
        ],
    }));
    blocks.push(Block::Paragraph(Paragraph {
        src: None,
        props: ParaProps {
            indent_left: Some(Twips(567)),
            ..Default::default()
        },
        runs: vec![Run {
            props: rp(28),
            content: vec![
                RunContent::Text("Tab:".into()),
                RunContent::Tab,
                RunContent::Text("cột 1".into()),
                RunContent::Tab,
                RunContent::Text("cột 2".into()),
                RunContent::LineBreak,
                RunContent::Text("Dòng mới trong cùng đoạn (ngắt dòng thủ công).".into()),
            ],
        }],
    }));
    for i in 0..18 {
        blocks.push(body(&format!(
            "{}. Đoạn kiểm thử phân trang số {}: {}",
            i + 1,
            i + 1,
            ARTICLES[i % ARTICLES.len()]
        )));
    }
    blocks.push(Block::Paragraph(Paragraph {
        src: None,
        props: ParaProps {
            align: Some(Alignment::Right),
            indent_right: Some(Twips(567)),
            page_break_before: Some(true),
            ..Default::default()
        },
        runs: vec![Run::text("TM. BAN THỬ NGHIỆM", bold(28))],
    }));
    blocks.push(p(
        Alignment::Right,
        vec![Run::text(
            "(Ký, ghi rõ họ tên)   ",
            RunProps {
                italic: Some(true),
                ..rp(28)
            },
        )],
    ));
    blocks.push(p(
        Alignment::Left,
        vec![Run::text(
            "Nơi nhận:",
            RunProps {
                italic: Some(true),
                ..bold(24)
            },
        )],
    ));
    for line in ["- Như Điều 4;", "- Lưu: VT, BTN."] {
        blocks.push(p(Alignment::Left, vec![Run::text(line, rp(22))]));
    }

    let landscape = Section {
        page: PageSetup { width: Twips(16_838), height: Twips(11_906), ..PageSetup::default() },
        blocks: vec![
            p(Alignment::Center, vec![Run::text("PHỤ LỤC (trang ngang)", bold(28))]),
            body("Section thứ hai dùng khổ A4 xoay ngang để kiểm tra thiết lập trang theo từng section (w:sectPr/w:pgSz)."),
        ],
        ..Section::default()
    };
    Document {
        sections: vec![Section::new(blocks), landscape],
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(28)),
            ..Default::default()
        },
        default_para: ParaProps {
            space_after: Some(Twips(0)),
            line_spacing: Some(LineSpacing::Auto(240)),
            ..Default::default()
        },
        ..Default::default()
    }
}
