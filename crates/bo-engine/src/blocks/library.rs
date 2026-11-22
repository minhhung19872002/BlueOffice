//! The building block library shipped with BlueDoc, designed by BlueOffice and
//! authored in code: cover pages, headers, footers, page numbers, pull-quote text
//! boxes, tables and the snippets of Vietnamese administrative documents (Nghị định
//! 30/2020/NĐ-CP, Phụ lục I: "Nơi nhận", signature block, national title).
//!
//! Every entry is a `w:docPart` (ECMA-376 Part 1, §17.12.5) of the glossary of
//! [`library`]; `bo-cli blocks` writes it as `assets/blocks/doc/blueoffice-blocks.dotx`.
//! Layout blocks carry no font so they take the fonts of the document they are
//! inserted into; the administrative snippets name Times New Roman as the decree does.

use crate::templates::{self, nd30};
use bo_edit::blocks::for_each_paragraph_mut;
use bo_model::glossary::{DocPart, DocPartProps, Glossary};
use bo_model::{
    Alignment, Anchor, Block, Border, Borders, Cell, CellProps, CellVAlign, Color, Document, Emu,
    HalfPoints, Image, ParaProps, Paragraph, Row, RowProps, Run, RunContent, RunProps, Section,
    Shape, ShapeLine, Style, StyleKind, StyleSheet, TabAlign, TabLeader, TabStop, Table,
    TableProps, TableWidth, TextAnchor, Twips, Wrap,
};
use std::sync::Arc;

/// Accent colour of the library (BlueOffice blue).
const ACCENT: &str = "1F4E79";
/// Light tint of the accent.
const TINT: &str = "DCE6F1";
/// Muted text.
const MUTED: &str = "595959";
/// Text column the tab stops and tables are sized for (A4, 2.5 cm margins), twips.
const WIDTH: i32 = 9_070;

fn color(hex: &str) -> Option<Color> {
    Color::from_hex(hex)
}

/// §17.3.2 run: sz (half-points), b, i.
fn run(text: &str, size: u32, bold: bool, italic: bool) -> Run {
    Run::text(text, templates::run_props(size, bold, italic))
}

fn tinted(mut r: Run, hex: &str) -> Run {
    r.props.color = color(hex);
    r
}

/// §17.16.5 field with its cached result.
fn field(instr: &str, cached: &str, size: u32) -> Run {
    Run {
        props: templates::run_props(size, false, false),
        content: vec![RunContent::Field {
            instr: instr.into(),
            cached: cached.into(),
            locked: false,
            codes: false,
        }],
    }
}

/// §17.3.3.32 tab.
fn tab() -> Run {
    Run {
        props: RunProps::default(),
        content: vec![RunContent::Tab],
    }
}

/// §17.3.1 paragraph without spacing around it.
fn para(runs: Vec<Run>, align: Alignment) -> Paragraph {
    Paragraph::new(
        ParaProps {
            align: Some(align),
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            ..ParaProps::default()
        },
        runs,
    )
}

fn block(p: Paragraph) -> Block {
    Block::Paragraph(p)
}

fn spaced(mut p: Paragraph, before: i32, after: i32) -> Paragraph {
    p.props.space_before = Some(Twips(before));
    p.props.space_after = Some(Twips(after));
    p
}

fn line(size: u32, hex: &str) -> Border {
    Border {
        style: "single".into(),
        size,
        color: color(hex),
        space: 4,
    }
}

/// Centre and right tab stops of a three-part header or footer (§17.3.1.37 tab).
fn three_part_tabs() -> Vec<TabStop> {
    vec![
        TabStop {
            pos: Twips(WIDTH / 2),
            align: TabAlign::Center,
            leader: TabLeader::None,
        },
        TabStop {
            pos: Twips(WIDTH),
            align: TabAlign::Right,
            leader: TabLeader::None,
        },
    ]
}

/// Left ⇥ centre ⇥ right on one line.
fn three_parts(left: Vec<Run>, centre: Vec<Run>, right: Vec<Run>) -> Paragraph {
    let mut runs = left;
    runs.push(tab());
    runs.extend(centre);
    runs.push(tab());
    runs.extend(right);
    let mut p = para(runs, Alignment::Left);
    p.props.tabs = Some(three_part_tabs());
    p
}

fn border_top(mut p: Paragraph, b: Border) -> Paragraph {
    p.props.borders = Some(Borders {
        top: Some(b),
        ..Borders::default()
    });
    p
}

fn border_bottom(mut p: Paragraph, b: Border) -> Paragraph {
    p.props.borders = Some(Borders {
        bottom: Some(b),
        ..Borders::default()
    });
    p
}

/// "Trang X / Y" runs.
fn page_of_pages(size: u32) -> Vec<Run> {
    vec![
        run("Trang ", size, false, false),
        field("PAGE", "1", size),
        run(" / ", size, false, false),
        field("NUMPAGES", "1", size),
    ]
}

// ---- headers (§17.10.4 hdr) ---------------------------------------------------------

fn headers() -> Vec<(&'static str, &'static str, Vec<Block>)> {
    vec![
        (
            "Đầu trang – Ba cột",
            "Ba ô giữ chỗ: trái, giữa, phải",
            vec![block(three_parts(
                vec![run("[Nhập nội dung]", 20, false, false)],
                vec![run("[Nhập nội dung]", 20, false, false)],
                vec![run("[Nhập nội dung]", 20, false, false)],
            ))],
        ),
        (
            "Đầu trang – Tên tài liệu",
            "Tên tài liệu bên trái, đường kẻ bên dưới",
            vec![block(border_bottom(
                para(
                    vec![tinted(run("[Tên tài liệu]", 20, true, false), ACCENT)],
                    Alignment::Left,
                ),
                line(8, ACCENT),
            ))],
        ),
        (
            "Đầu trang – Cơ quan và ngày",
            "Tên cơ quan bên trái, ngày bên phải, đường kẻ bên dưới",
            vec![block(border_bottom(
                three_parts(
                    vec![run("[TÊN CƠ QUAN, TỔ CHỨC]", 20, true, false)],
                    Vec::new(),
                    vec![tinted(
                        run("[Ngày … tháng … năm …]", 20, false, true),
                        MUTED,
                    )],
                ),
                line(4, MUTED),
            ))],
        ),
        (
            "Đầu trang – Dải nền",
            "Tên tài liệu trên dải nền nhạt, số trang bên phải",
            vec![block({
                let mut p = three_parts(
                    vec![tinted(run(" [Tên tài liệu]", 20, true, false), ACCENT)],
                    Vec::new(),
                    vec![field("PAGE", "1", 20), run(" ", 20, false, false)],
                );
                p.props.shading = color(TINT);
                p
            })],
        ),
    ]
}

// ---- footers (§17.10.2 ftr) ---------------------------------------------------------

fn footers() -> Vec<(&'static str, &'static str, Vec<Block>)> {
    vec![
        (
            "Chân trang – Ba cột",
            "Ba ô giữ chỗ: trái, giữa, phải",
            vec![block(three_parts(
                vec![run("[Nhập nội dung]", 20, false, false)],
                vec![run("[Nhập nội dung]", 20, false, false)],
                vec![run("[Nhập nội dung]", 20, false, false)],
            ))],
        ),
        (
            "Chân trang – Tên tài liệu và số trang",
            "Tên tài liệu bên trái, \"Trang X / Y\" bên phải, đường kẻ bên trên",
            vec![block(border_top(
                three_parts(
                    vec![tinted(run("[Tên tài liệu]", 18, false, true), MUTED)],
                    Vec::new(),
                    page_of_pages(18),
                ),
                line(4, MUTED),
            ))],
        ),
        (
            "Chân trang – Lưu hành nội bộ",
            "Dòng \"Tài liệu lưu hành nội bộ\" và số trang ở giữa",
            vec![
                block(para(
                    vec![tinted(
                        run("Tài liệu lưu hành nội bộ", 18, false, true),
                        MUTED,
                    )],
                    Alignment::Center,
                )),
                block(para(vec![field("PAGE", "1", 20)], Alignment::Center)),
            ],
        ),
        (
            "Chân trang – Thông tin liên hệ",
            "Tên, địa chỉ, điện thoại của cơ quan ở giữa, đường kẻ bên trên",
            vec![
                block(border_top(
                    para(
                        vec![run("[TÊN CƠ QUAN, TỔ CHỨC]", 18, true, false)],
                        Alignment::Center,
                    ),
                    line(8, ACCENT),
                )),
                block(para(
                    vec![tinted(
                        run(
                            "[Địa chỉ] · Điện thoại: [số điện thoại] · Email: [địa chỉ thư]",
                            16,
                            false,
                            false,
                        ),
                        MUTED,
                    )],
                    Alignment::Center,
                )),
            ],
        ),
    ]
}

// ---- page numbers (§17.16.5.44 PAGE, §17.16.5.42 NUMPAGES) --------------------------

/// (name, description, gallery, content): `pgNumB` goes to the footer, `pgNumT` to
/// the header (§17.18.15 `ST_DocPartGallery`).
fn page_numbers() -> Vec<(&'static str, &'static str, &'static str, Vec<Block>)> {
    vec![
        (
            "Số trang – Dưới giữa",
            "Số trang ở giữa chân trang",
            "pgNumB",
            vec![block(para(vec![field("PAGE", "1", 22)], Alignment::Center))],
        ),
        (
            "Số trang – Dưới phải",
            "Số trang bên phải chân trang",
            "pgNumB",
            vec![block(para(vec![field("PAGE", "1", 22)], Alignment::Right))],
        ),
        (
            "Số trang – Trang X / Y",
            "\"Trang X / Y\" ở giữa chân trang",
            "pgNumB",
            vec![block(para(page_of_pages(22), Alignment::Center))],
        ),
        (
            "Số trang – Gạch hai bên",
            "\"– X –\" ở giữa chân trang",
            "pgNumB",
            vec![block(para(
                vec![
                    run("– ", 22, false, false),
                    field("PAGE", "1", 22),
                    run(" –", 22, false, false),
                ],
                Alignment::Center,
            ))],
        ),
        (
            "Số trang – Trên giữa",
            "Số trang ở giữa lề trên (thể thức văn bản hành chính)",
            "pgNumT",
            vec![block(para(
                vec![field("PAGE", "1", nd30::SO_TRANG.preferred)],
                Alignment::Center,
            ))],
        ),
        (
            "Số trang – Trên phải",
            "\"Trang X / Y\" bên phải đầu trang",
            "pgNumT",
            vec![block(para(page_of_pages(20), Alignment::Right))],
        ),
    ]
}

// ---- cover pages --------------------------------------------------------------------

fn gap(lines: usize, size: u32) -> Vec<Block> {
    (0..lines)
        .map(|_| block(para(vec![run("", size, false, false)], Alignment::Left)))
        .collect()
}

fn covers() -> Vec<(&'static str, &'static str, Vec<Block>)> {
    // 1: a thick accent rule under a large title.
    let mut band = gap(6, 28);
    band.push(block(para(
        vec![tinted(
            run("[TÊN CƠ QUAN, TỔ CHỨC]", 24, true, false),
            MUTED,
        )],
        Alignment::Left,
    )));
    band.push(block(spaced(
        border_bottom(
            para(
                vec![tinted(run("[Tên tài liệu]", 72, true, false), ACCENT)],
                Alignment::Left,
            ),
            line(36, ACCENT),
        ),
        240,
        120,
    )));
    band.push(block(spaced(
        para(
            vec![run("[Phụ đề hoặc mô tả ngắn]", 32, false, true)],
            Alignment::Left,
        ),
        120,
        0,
    )));
    band.extend(gap(14, 28));
    band.push(block(para(
        vec![run("[Tác giả]", 26, true, false)],
        Alignment::Left,
    )));
    band.push(block(para(
        vec![tinted(
            run("[Địa danh], tháng … năm 20…", 26, false, false),
            MUTED,
        )],
        Alignment::Left,
    )));

    // 2: the title in a framed box in the middle of the page.
    let frame_w = WIDTH - 1_200;
    let frame = Block::Table(Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(Twips(frame_w))),
            align: Some(Alignment::Center),
            borders: Some(Borders {
                top: Some(line(18, ACCENT)),
                left: Some(line(18, ACCENT)),
                bottom: Some(line(18, ACCENT)),
                right: Some(line(18, ACCENT)),
                inside_h: None,
                inside_v: None,
            }),
            fixed_layout: true,
            ..TableProps::default()
        },
        grid: vec![Twips(frame_w)],
        grid_change: None,
        rows: vec![Row {
            props: RowProps::default(),
            cells: vec![Cell {
                props: CellProps {
                    width: Some(TableWidth::Dxa(Twips(frame_w))),
                    v_align: Some(CellVAlign::Center),
                    ..CellProps::default()
                },
                blocks: vec![
                    block(spaced(
                        para(
                            vec![run("[TÊN TÀI LIỆU]", 56, true, false)],
                            Alignment::Center,
                        ),
                        480,
                        120,
                    )),
                    block(spaced(
                        para(
                            vec![tinted(
                                run("[Phụ đề hoặc mô tả ngắn]", 28, false, true),
                                MUTED,
                            )],
                            Alignment::Center,
                        ),
                        0,
                        480,
                    )),
                ],
            }],
        }],
        src: None,
    });
    let mut framed = vec![block(para(
        vec![run("[TÊN CƠ QUAN, TỔ CHỨC]", 28, true, false)],
        Alignment::Center,
    ))];
    framed.extend(gap(9, 28));
    framed.push(frame);
    framed.extend(gap(12, 28));
    framed.push(block(para(
        vec![run("[Tác giả]", 26, false, false)],
        Alignment::Center,
    )));
    framed.push(block(para(
        vec![run("[Địa danh], năm 20…", 26, true, false)],
        Alignment::Center,
    )));

    // 3: minimal — small caps label, title, thin rule, details at the bottom.
    let mut minimal = gap(10, 28);
    minimal.push(block(para(
        vec![tinted(run("BÁO CÁO", 22, true, false), ACCENT)],
        Alignment::Right,
    )));
    minimal.push(block(spaced(
        para(
            vec![run("[Tên tài liệu]", 60, false, false)],
            Alignment::Right,
        ),
        120,
        120,
    )));
    minimal.push(block(border_top(
        spaced(
            para(
                vec![tinted(
                    run("[Phụ đề hoặc mô tả ngắn]", 26, false, false),
                    MUTED,
                )],
                Alignment::Right,
            ),
            120,
            0,
        ),
        line(6, MUTED),
    )));
    minimal.extend(gap(14, 28));
    minimal.push(block(para(
        vec![
            run("[Tác giả]", 24, true, false),
            run("  ·  [Đơn vị]  ·  [Ngày … tháng … năm …]", 24, false, false),
        ],
        Alignment::Right,
    )));

    vec![
        (
            "Bìa – Đường kẻ đậm",
            "Tiêu đề lớn canh trái trên một đường kẻ đậm",
            band,
        ),
        (
            "Bìa – Khung giữa",
            "Tiêu đề trong khung ở giữa trang",
            framed,
        ),
        (
            "Bìa – Tối giản",
            "Tiêu đề canh phải với đường kẻ mảnh",
            minimal,
        ),
    ]
}

// ---- pull quotes (text boxes, §17.17.1 txbxContent) ---------------------------------

/// A floating text box of `width` × `height` EMU holding `text`.
fn text_box(
    width: i64,
    height: i64,
    h_align: &str,
    fill: Option<Color>,
    outline: Option<ShapeLine>,
    text: Vec<Block>,
) -> Block {
    let shape = Shape {
        preset: "rect".into(),
        fill,
        line: outline,
        text: Some(text),
        insets: [Emu(137_160), Emu(91_440), Emu(137_160), Emu(91_440)],
        anchor: TextAnchor::Center,
        ..Shape::default()
    };
    let image = Image {
        media: String::new(),
        width: Emu(width),
        height: Emu(height),
        anchor: Some(Anchor {
            h_relative: "margin".into(),
            v_relative: "paragraph".into(),
            wrap: Wrap::Square,
            dist: [Emu(91_440), Emu(91_440), Emu(182_880), Emu(182_880)],
            h_align: Some(h_align.into()),
            ..Anchor::default()
        }),
        crop: None,
        rotation: 0,
        description: "Khung trích dẫn".into(),
        shape: Some(Box::new(shape)),
        format: None,
        ole: None,
        src: None,
    };
    block(Paragraph::new(
        ParaProps::default(),
        vec![Run {
            props: RunProps::default(),
            content: vec![RunContent::Image(Box::new(image))],
        }],
    ))
}

const QUOTE: &str = "[Trích một câu đáng chú ý trong tài liệu để thu hút người đọc.]";

fn pull_quotes() -> Vec<(&'static str, &'static str, Vec<Block>)> {
    let accent = Color::from_hex(ACCENT).unwrap_or(Color::BLACK);
    vec![
        (
            "Trích dẫn – Khung viền",
            "Khung trích dẫn bên phải, viền mảnh",
            vec![text_box(
                2_286_000,
                1_371_600,
                "right",
                Some(Color::WHITE),
                Some(ShapeLine {
                    width: Emu(12_700),
                    color: accent,
                    dash: None,
                }),
                vec![block(para(
                    vec![tinted(run(QUOTE, 24, false, true), ACCENT)],
                    Alignment::Center,
                ))],
            )],
        ),
        (
            "Trích dẫn – Nền nhạt",
            "Khung trích dẫn bên trái, nền nhạt không viền",
            vec![text_box(
                2_286_000,
                1_371_600,
                "left",
                Color::from_hex(TINT),
                None,
                vec![
                    block(para(vec![run(QUOTE, 24, true, false)], Alignment::Left)),
                    block(spaced(
                        para(
                            vec![tinted(run("— [Nguồn]", 20, false, true), MUTED)],
                            Alignment::Right,
                        ),
                        80,
                        0,
                    )),
                ],
            )],
        ),
        (
            "Trích dẫn – Dải ngang",
            "Khung trích dẫn rộng ở giữa, kẻ trên và dưới",
            vec![text_box(
                4_572_000,
                914_400,
                "center",
                None,
                None,
                vec![block(border_bottom(
                    border_top(
                        para(vec![run(QUOTE, 26, false, true)], Alignment::Center),
                        line(12, ACCENT),
                    ),
                    line(12, ACCENT),
                ))],
            )],
        ),
    ]
}

// ---- tables (§17.4.38 tbl) ----------------------------------------------------------

fn grid_table(widths: &[i32], head: &[&str], rows: &[&[&str]]) -> Block {
    let cell = |w: i32, text: &str, head: bool, align: Alignment| Cell {
        props: CellProps {
            width: Some(TableWidth::Dxa(Twips(w))),
            shading: if head { color(TINT) } else { None },
            v_align: Some(CellVAlign::Center),
            ..CellProps::default()
        },
        blocks: vec![block(spaced(
            para(vec![run(text, 24, head, false)], align),
            60,
            60,
        ))],
    };
    let mut out = vec![Row {
        props: RowProps {
            header: true,
            ..RowProps::default()
        },
        cells: widths
            .iter()
            .zip(head)
            .map(|(&w, t)| cell(w, t, true, Alignment::Center))
            .collect(),
    }];
    for r in rows {
        out.push(Row {
            props: RowProps::default(),
            cells: widths
                .iter()
                .zip(*r)
                .enumerate()
                .map(|(i, (&w, t))| {
                    cell(
                        w,
                        t,
                        false,
                        if i == 0 {
                            Alignment::Center
                        } else {
                            Alignment::Left
                        },
                    )
                })
                .collect(),
        });
    }
    Block::Table(Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(Twips(widths.iter().sum()))),
            borders: Some(Borders::all(&Border::single(4))),
            fixed_layout: true,
            ..TableProps::default()
        },
        grid: widths.iter().copied().map(Twips).collect(),
        grid_change: None,
        rows: out,
        src: None,
    })
}

/// One cell of the Quick Tables designs (§17.4.66 tc): text, width, fill, bold, alignment,
/// grid span.
struct Tc<'a> {
    text: &'a str,
    width: i32,
    fill: Option<&'a str>,
    bold: bool,
    align: Alignment,
    span: u32,
    color: Option<&'a str>,
}

fn tc(text: &str, width: i32) -> Tc<'_> {
    Tc {
        text,
        width,
        fill: None,
        bold: false,
        align: Alignment::Left,
        span: 1,
        color: None,
    }
}

/// A table of rows of [`Tc`] with `borders` (`None`: no lines), first rows repeated as a
/// header (§17.4.49 tblHeader).
fn designed_table(
    rows: Vec<Vec<Tc<'_>>>,
    grid: &[i32],
    borders: Option<Borders>,
    header: usize,
) -> Block {
    let rows = rows
        .into_iter()
        .enumerate()
        .map(|(ri, cells)| Row {
            props: RowProps {
                header: ri < header,
                ..RowProps::default()
            },
            cells: cells
                .into_iter()
                .map(|c| {
                    let mut r = run(c.text, 22, c.bold, false);
                    r.props.color = c.color.and_then(color);
                    Cell {
                        props: CellProps {
                            width: Some(TableWidth::Dxa(Twips(c.width))),
                            grid_span: c.span,
                            shading: c.fill.and_then(color),
                            v_align: Some(CellVAlign::Center),
                            ..CellProps::default()
                        },
                        blocks: vec![block(spaced(para(vec![r], c.align), 40, 40))],
                    }
                })
                .collect(),
        })
        .collect();
    Block::Table(Table {
        props: TableProps {
            width: Some(TableWidth::Dxa(Twips(grid.iter().sum()))),
            borders,
            fixed_layout: true,
            ..TableProps::default()
        },
        grid: grid.iter().copied().map(Twips).collect(),
        grid_change: None,
        rows,
        src: None,
    })
}

/// A month calendar (own design): month title over seven day columns, Monday first; the
/// month begins on `first` (0 = Monday) and has `days` days.
fn calendar(title: &str, first: usize, days: usize) -> Vec<Block> {
    let w = WIDTH / 7;
    let names = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];
    let mut rows = vec![names
        .iter()
        .enumerate()
        .map(|(i, n)| Tc {
            fill: Some(ACCENT),
            bold: true,
            align: Alignment::Center,
            color: Some(if i == 6 { "FFC7CE" } else { "FFFFFF" }),
            ..tc(n, w)
        })
        .collect::<Vec<_>>()];
    let numbers: Vec<String> = (1..=days).map(|d| d.to_string()).collect();
    let mut day = 0usize;
    let mut week = 0usize;
    while day < days {
        let row = (0..7)
            .map(|col| {
                let slot = week * 7 + col;
                let text = if slot >= first && day < days {
                    let t = numbers[day].as_str();
                    day += 1;
                    t
                } else {
                    ""
                };
                Tc {
                    align: Alignment::Center,
                    fill: (week % 2 == 1).then_some(TINT),
                    color: (col == 6).then_some("C00000"),
                    ..tc(text, w)
                }
            })
            .collect();
        rows.push(row);
        week += 1;
    }
    let edge = Border::single(4);
    let mut b = Borders::all(&edge);
    b.inside_v = None;
    let heading = spaced(
        para(
            vec![tinted(run(title, 36, true, false), ACCENT)],
            Alignment::Left,
        ),
        0,
        120,
    );
    vec![block(heading), designed_table(rows, &[w; 7], Some(b), 1)]
}

fn tables() -> Vec<(&'static str, &'static str, Vec<Block>)> {
    let head = |t: &'static str, w: i32| Tc {
        fill: Some(ACCENT),
        bold: true,
        color: Some("FFFFFF"),
        ..tc(t, w)
    };
    let num = |t: &'static str, w: i32| Tc {
        align: Alignment::Right,
        ..tc(t, w)
    };
    let x = |t: &'static str, w: i32| Tc {
        align: Alignment::Center,
        ..tc(t, w)
    };
    let sub = |t: &'static str| Tc {
        fill: Some(TINT),
        bold: true,
        span: 4,
        ..tc(t, WIDTH)
    };
    let hline = Borders {
        top: Some(Border::single(12)),
        bottom: Some(Border::single(12)),
        inside_h: Some(line(4, "BFBFBF")),
        ..Borders::default()
    };
    vec![
        (
            "Lịch tháng",
            "Lịch một tháng, tuần bắt đầu từ thứ Hai, chủ nhật màu đỏ",
            calendar("Tháng 7 năm 2026", 2, 31),
        ),
        (
            "Bảng – Ma trận",
            "Ma trận phân công: công việc theo hàng, người thực hiện theo cột",
            vec![designed_table(
                vec![
                    vec![
                        head("Công việc", 3_070),
                        head("Trưởng phòng", 1_500),
                        head("Chuyên viên", 1_500),
                        head("Kế toán", 1_500),
                        head("Văn thư", 1_500),
                    ],
                    vec![
                        tc("Lập kế hoạch", 3_070),
                        x("x", 1_500),
                        x("x", 1_500),
                        x("", 1_500),
                        x("", 1_500),
                    ],
                    vec![
                        tc("Dự toán kinh phí", 3_070),
                        x("", 1_500),
                        x("x", 1_500),
                        x("x", 1_500),
                        x("", 1_500),
                    ],
                    vec![
                        tc("Phát hành văn bản", 3_070),
                        x("x", 1_500),
                        x("", 1_500),
                        x("", 1_500),
                        x("x", 1_500),
                    ],
                    vec![
                        tc("Lưu hồ sơ", 3_070),
                        x("", 1_500),
                        x("", 1_500),
                        x("", 1_500),
                        x("x", 1_500),
                    ],
                ],
                &[3_070, 1_500, 1_500, 1_500, 1_500],
                Some(Borders::all(&Border::single(4))),
                1,
            )],
        ),
        (
            "Bảng – Có tiêu đề phụ",
            "Bảng kê có các nhóm (tiêu đề phụ tô nền) và cột số căn phải",
            vec![designed_table(
                vec![
                    vec![
                        head("Hạng mục", 4_270),
                        head("Số lượng", 1_400),
                        head("Đơn giá", 1_700),
                        head("Thành tiền", 1_700),
                    ],
                    vec![sub("A. Thiết bị")],
                    vec![
                        tc("Máy tính xách tay", 4_270),
                        num("4", 1_400),
                        num("18.500.000", 1_700),
                        num("74.000.000", 1_700),
                    ],
                    vec![
                        tc("Máy in", 4_270),
                        num("1", 1_400),
                        num("6.200.000", 1_700),
                        num("6.200.000", 1_700),
                    ],
                    vec![sub("B. Dịch vụ")],
                    vec![
                        tc("Lắp đặt, cài đặt", 4_270),
                        num("1", 1_400),
                        num("2.000.000", 1_700),
                        num("2.000.000", 1_700),
                    ],
                    vec![
                        tc("Bảo trì 12 tháng", 4_270),
                        num("1", 1_400),
                        num("3.600.000", 1_700),
                        num("3.600.000", 1_700),
                    ],
                ],
                &[4_270, 1_400, 1_700, 1_700],
                Some(hline.clone()),
                1,
            )],
        ),
        (
            "Bảng – Số liệu theo quý",
            "Danh sách chỉ tiêu với số liệu bốn quý, chỉ có đường kẻ ngang",
            vec![designed_table(
                vec![
                    vec![
                        Tc {
                            bold: true,
                            ..tc("Chỉ tiêu", 3_470)
                        },
                        Tc {
                            bold: true,
                            align: Alignment::Right,
                            ..tc("Quý I", 1_400)
                        },
                        Tc {
                            bold: true,
                            align: Alignment::Right,
                            ..tc("Quý II", 1_400)
                        },
                        Tc {
                            bold: true,
                            align: Alignment::Right,
                            ..tc("Quý III", 1_400)
                        },
                        Tc {
                            bold: true,
                            align: Alignment::Right,
                            ..tc("Quý IV", 1_400)
                        },
                    ],
                    vec![
                        tc("Hồ sơ tiếp nhận", 3_470),
                        num("1.240", 1_400),
                        num("1.315", 1_400),
                        num("1.402", 1_400),
                        num("1.388", 1_400),
                    ],
                    vec![
                        tc("Hồ sơ giải quyết đúng hạn", 3_470),
                        num("1.198", 1_400),
                        num("1.290", 1_400),
                        num("1.371", 1_400),
                        num("1.360", 1_400),
                    ],
                    vec![
                        tc("Tỷ lệ đúng hạn (%)", 3_470),
                        num("96,6", 1_400),
                        num("98,1", 1_400),
                        num("97,8", 1_400),
                        num("98,0", 1_400),
                    ],
                ],
                &[3_470, 1_400, 1_400, 1_400, 1_400],
                Some(hline),
                1,
            )],
        ),
        (
            "Bảng – Danh sách",
            "Bảng ba cột có hàng tiêu đề: STT, nội dung, ghi chú",
            vec![grid_table(
                &[850, 5_670, 2_550],
                &["STT", "Nội dung", "Ghi chú"],
                &[&["1", "", ""], &["2", "", ""], &["3", "", ""]],
            )],
        ),
        (
            "Bảng – Lịch công tác",
            "Bảng bốn cột: thời gian, nội dung, chủ trì, địa điểm",
            vec![grid_table(
                &[1_700, 3_970, 1_700, 1_700],
                &["Thời gian", "Nội dung", "Chủ trì", "Địa điểm"],
                &[
                    &["Thứ Hai", "", "", ""],
                    &["Thứ Ba", "", "", ""],
                    &["Thứ Tư", "", "", ""],
                    &["Thứ Năm", "", "", ""],
                    &["Thứ Sáu", "", "", ""],
                ],
            )],
        ),
    ]
}

// ---- administrative snippets (Nghị định 30/2020/NĐ-CP, Phụ lục I) --------------------

/// Times New Roman on every run, as the decree requires whatever the document's
/// default font is.
fn admin(mut blocks: Vec<Block>) -> Vec<Block> {
    for_each_paragraph_mut(&mut blocks, &mut |p| {
        for r in &mut p.runs {
            r.props.font.get_or_insert_with(|| nd30::FONT.into());
        }
    });
    blocks
}

/// (name, description, behaviour, content).
fn admin_snippets() -> Vec<(&'static str, &'static str, &'static str, Vec<Block>)> {
    vec![
        (
            "Nơi nhận",
            "Khối \"Nơi nhận\" (ô 9b): cỡ 12 đậm nghiêng, danh sách cỡ 11",
            "p",
            admin(templates::noi_nhan("- Như trên;")),
        ),
        (
            "Chữ ký",
            "Quyền hạn, chức vụ, chữ ký và họ tên người ký (ô 7a–7c)",
            "p",
            admin(templates::signer(
                &["[QUYỀN HẠN KÝ]", "[CHỨC VỤ CỦA NGƯỜI KÝ]"],
                "(Chữ ký, dấu)",
            )),
        ),
        (
            "Nơi nhận và chữ ký",
            "Nơi nhận bên trái, khối chữ ký bên phải",
            "p",
            admin(templates::closing("- Như trên;")),
        ),
        (
            "Quốc hiệu",
            "Tên cơ quan, số ký hiệu bên trái; Quốc hiệu, Tiêu ngữ, địa danh và ngày bên phải",
            "p",
            admin(vec![templates::header("Số: …/…-…", None)]),
        ),
        (
            "Kính gửi",
            "Dòng \"Kính gửi:\" cỡ 14",
            "content",
            admin(vec![block(Paragraph::new(
                ParaProps::default(),
                vec![run("Kính gửi: ", nd30::BODY.preferred, false, false)],
            ))]),
        ),
        (
            "Căn cứ",
            "Đoạn căn cứ ban hành văn bản, chữ nghiêng",
            "p",
            admin(vec![templates::body_italic(
                "Căn cứ [tên văn bản, số, ký hiệu, ngày tháng năm ban hành];",
            )]),
        ),
    ]
}

// ---- the library --------------------------------------------------------------------

fn part(
    index: usize,
    name: &str,
    gallery: &str,
    category: &str,
    description: &str,
    behavior: &str,
    blocks: Vec<Block>,
) -> DocPart {
    DocPart {
        props: DocPartProps {
            name: name.into(),
            gallery: gallery.into(),
            category: category.into(),
            description: description.into(),
            types: vec!["normal".into()],
            behaviors: vec![behavior.into()],
            // Stable ids: the committed asset must not change between runs.
            guid: Some(format!("{{B10E0FF1-CE00-4B0C-8000-{:012X}}}", index + 1)),
            style: None,
        },
        blocks,
        src: None,
    }
}

/// Category of the entries designed by BlueOffice.
pub const CATEGORY: &str = "BlueOffice";
/// Category of the administrative snippets.
pub const CATEGORY_ADMIN: &str = "Văn bản hành chính";

/// Every entry of the library, in gallery order.
#[must_use]
pub fn parts() -> Vec<DocPart> {
    let mut v: Vec<DocPart> = Vec::new();
    let mut add = |name: &str, gallery: &str, category: &str, desc: &str, behavior: &str, b| {
        let i = v.len();
        v.push(part(i, name, gallery, category, desc, behavior, b));
    };
    for (n, d, b) in covers() {
        add(n, "coverPg", CATEGORY, d, "pg", b);
    }
    for (n, d, b) in headers() {
        add(n, "hdrs", CATEGORY, d, "p", b);
    }
    for (n, d, b) in footers() {
        add(n, "ftrs", CATEGORY, d, "p", b);
    }
    for (n, d, g, b) in page_numbers() {
        add(n, g, CATEGORY, d, "p", b);
    }
    for (n, d, b) in pull_quotes() {
        add(n, "txtBox", CATEGORY, d, "content", b);
    }
    for (n, d, b) in tables() {
        add(n, "tbls", CATEGORY, d, "p", b);
    }
    for (n, d, behavior, b) in admin_snippets() {
        add(n, "autoTxt", CATEGORY_ADMIN, d, behavior, b);
    }
    v
}

/// The library as a document (a template whose only content is its glossary).
#[must_use]
pub fn library() -> Document {
    let mut normal = Style::new("Normal", "Normal", StyleKind::Paragraph);
    normal.is_default = true;
    let glossary = Glossary {
        parts: parts(),
        styles: StyleSheet {
            styles: vec![normal],
            ..StyleSheet::default()
        },
        // §17.7.5.2 rPrDefault of the glossary: 12 pt.
        default_run: RunProps {
            size: Some(HalfPoints(24)),
            ..RunProps::default()
        },
        ..Glossary::default()
    };
    Document {
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::default())])],
        glossary: Some(Arc::new(glossary)),
        ..Document::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn library_has_the_promised_entries() {
        let parts = parts();
        let count = |g: &str| parts.iter().filter(|p| p.props.gallery == g).count();
        assert_eq!(count("coverPg"), 3);
        assert_eq!(count("hdrs"), 4);
        assert_eq!(count("ftrs"), 4);
        assert_eq!(count("pgNumB") + count("pgNumT"), 6);
        assert_eq!(count("txtBox"), 3);
        assert_eq!(count("tbls"), 6);
        assert!(count("autoTxt") >= 4);
        // Names and ids are unique; every gallery is one BlueDoc knows.
        let names: BTreeSet<_> = parts.iter().map(|p| &p.props.name).collect();
        let guids: BTreeSet<_> = parts.iter().map(|p| &p.props.guid).collect();
        assert_eq!(names.len(), parts.len());
        assert_eq!(guids.len(), parts.len());
        assert!(parts.iter().all(|p| !p.blocks.is_empty()));
        // §17.12.13 name: `ST_String` restricted to text without a comma.
        assert!(parts.iter().all(|p| !p.props.name.contains(',')));
        assert!(parts
            .iter()
            .all(|p| !p.props.description.is_empty() && !p.props.category.is_empty()));
    }

    #[test]
    fn library_round_trips_through_docx() {
        let doc = library();
        let bytes = bo_docx::write_docx(&doc).expect("write");
        let again = bo_docx::read_docx(&bytes).expect("read");
        let (a, b) = (
            doc.glossary.as_deref().expect("glossary"),
            again.glossary.as_deref().expect("glossary"),
        );
        assert_eq!(a.parts.len(), b.parts.len());
        for (x, y) in a.parts.iter().zip(&b.parts) {
            assert_eq!(x.props, y.props);
            assert_eq!(x.blocks.len(), y.blocks.len(), "{}", x.props.name);
        }
        // Deterministic output (the asset is committed).
        assert_eq!(bytes, bo_docx::write_docx(&library()).expect("write"));
    }
}
