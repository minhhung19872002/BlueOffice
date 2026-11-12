//! Table tools in the layout: vertical text in cells (ECMA-376 Part 1, §17.4.72
//! textDirection), floating tables (§17.4.58 tblpPr) and table measurements.
// Integration tests may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, measure_table, LayoutCache, LayoutDocument, Line};
use bo_model::{
    Block, Document, HalfPoints, HeightRule, PageSetup, ParaProps, Paragraph, Run, RunProps,
    Section, Table, TableFloat, TextDirection, Twips,
};
use bo_text::FontManager;

fn fonts() -> FontManager {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    fm
}

fn p(text: &str) -> Block {
    Block::Paragraph(Paragraph::new(
        ParaProps::default(),
        vec![Run::text(text, RunProps::default())],
    ))
}

fn doc(blocks: Vec<Block>) -> Document {
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    }
}

fn run(d: &Document) -> LayoutDocument {
    layout_document(d, &mut fonts(), &mut LayoutCache::new()).unwrap()
}

fn text(l: &Line) -> String {
    l.fragments.iter().map(|f| f.text.as_str()).collect()
}

/// Visual start and end of a line's text (after its rotation).
fn ends(l: &Line) -> ((f32, f32), (f32, f32)) {
    let f0 = &l.fragments[0];
    let x1 = l
        .fragments
        .iter()
        .map(|f| f.x + f.width)
        .fold(f32::MIN, f32::max);
    let r = l.rotation.unwrap();
    (r.apply(f0.x, f0.y), r.apply(x1, f0.y))
}

/// A 1 × 2 table: the first cell holds `s` with direction `dir`, the second "Ngang".
fn vertical_table(s: &str, dir: TextDirection, height: Option<i32>) -> Table {
    let mut t = Table::simple(1, 2, Twips(6000));
    t.rows[0].cells[0].blocks = vec![p(s)];
    t.rows[0].cells[0].props.text_direction = Some(dir);
    t.rows[0].cells[1].blocks = vec![p("Ngang")];
    if let Some(h) = height {
        t.rows[0].props.height = Some((Twips(h), HeightRule::Exact));
    }
    t
}

#[test]
fn bottom_to_top_text_starts_at_the_cell_bottom() {
    // Row exactly 2 inches (144 pt) tall: the line is 144 pt long.
    let l = run(&doc(vec![Block::Table(vertical_table(
        "Dọc",
        TextDirection::BtLr,
        Some(2880),
    ))]));
    let page = &l.pages[0];
    let line = page.lines.iter().find(|l| text(l) == "Dọc").unwrap();
    let flat = page.lines.iter().find(|l| text(l) == "Ngang").unwrap();
    assert_eq!(line.rotation.map(|r| r.degrees), Some(-90.0));
    assert!(flat.rotation.is_none());
    // Row top = the top of the horizontal cell's line (no top cell margin).
    let row_top = flat.top;
    let ((sx, sy), (ex, ey)) = ends(line);
    // The text reads upwards from the bottom of the 144 pt row.
    assert!(
        (sy - (row_top + 144.0)).abs() < 1.0,
        "start y {sy}, row {row_top}"
    );
    assert!(ey < sy - 10.0, "reads upwards: {sy} → {ey}");
    assert!((sx - ex).abs() < 0.01);
    // Inside the first column (3000 twips = 150 pt wide), left of the second cell's text.
    assert!(sx < flat.fragments[0].x, "{sx}");
}

#[test]
fn top_to_bottom_text_reads_downwards() {
    let l = run(&doc(vec![Block::Table(vertical_table(
        "Dọc xuống",
        TextDirection::TbRl,
        None,
    ))]));
    let page = &l.pages[0];
    let line = page.lines.iter().find(|l| text(l) == "Dọc xuống").unwrap();
    let flat = page.lines.iter().find(|l| text(l) == "Ngang").unwrap();
    assert_eq!(line.rotation.map(|r| r.degrees), Some(90.0));
    let ((sx, sy), (_, ey)) = ends(line);
    let len = ey - sy;
    assert!(len > 30.0, "reads downwards: {sy} → {ey}");
    // Starts at the top of the row, which grew to hold the whole line.
    assert!((sy - flat.top).abs() < 1.0, "{sy} vs {}", flat.top);
    // The first line of top-to-bottom text sits on the right of the cell (150 pt
    // wide, from the left margin): right half.
    let left = PageSetup::default().margin_left.to_pt();
    assert!(sx > left + 75.0 && sx < left + 150.0, "{sx}");
}

#[test]
fn floating_table_sits_at_its_anchor_and_text_wraps() {
    // A 2 inch (144 pt) wide table at the right margin, then a long paragraph.
    let mut t = Table::simple(2, 1, Twips(2880));
    t.rows[0].cells[0].blocks = vec![p("Nổi")];
    t.props.float = Some(TableFloat {
        left_from_text: Twips(200),
        right_from_text: Twips(200),
        horz_anchor: Some("margin".into()),
        vert_anchor: Some("text".into()),
        x_align: Some("right".into()),
        ..TableFloat::default()
    });
    let long = "Văn bản chảy quanh bảng nổi bên phải trang. ".repeat(12);
    let l = run(&doc(vec![Block::Table(t), p(&long)]));
    let page = &l.pages[0];
    let s = PageSetup::default();
    let right = s.width.to_pt() - s.margin_right.to_pt();
    let cell = page.lines.iter().find(|l| text(l) == "Nổi").unwrap();
    // Table left edge = right margin − 144 pt; the text starts one cell margin in.
    let table_x = right - 144.0;
    assert!(
        cell.fragments[0].x > table_x && cell.fragments[0].x < table_x + 10.0,
        "{} vs {table_x}",
        cell.fragments[0].x
    );
    // The paragraph (id 2: the table holds ids 0 and 1) starts beside the table, not
    // below it, and its lines there stop 10 pt (200 twips) before the table.
    let lines: Vec<&Line> = page
        .lines
        .iter()
        .filter(|l| l.source == bo_layout::LineSource::Body(2))
        .collect();
    assert!(lines.len() > 3);
    assert!(
        (lines[0].top - cell.top).abs() < 1.0,
        "{} vs {}",
        lines[0].top,
        cell.top
    );
    let beside = |l: &&&Line| l.top < cell.top + 20.0;
    for l in lines.iter().filter(beside) {
        let end = l
            .fragments
            .iter()
            .map(|f| f.x + f.width)
            .fold(0.0f32, f32::max);
        assert!(end <= table_x - 10.0 + 0.5, "line ends at {end}");
    }
    // Further down the text uses the full width again.
    let widest = lines
        .iter()
        .map(|l| {
            l.fragments
                .iter()
                .map(|f| f.x + f.width)
                .fold(0.0f32, f32::max)
        })
        .fold(0.0f32, f32::max);
    assert!(widest > table_x, "{widest}");
}

#[test]
fn measures_rows_and_content_widths() {
    let mut t = Table::simple(2, 2, Twips(9000));
    t.rows[0].cells[0].blocks = vec![p("Một dòng khá dài trong ô đầu tiên")];
    t.rows[0].cells[1].blocks = vec![p("Ngắn")];
    t.rows[1].cells[0].blocks = vec![p("a"), p("b"), p("c")];
    let d = doc(vec![Block::Table(t.clone())]);
    let m = measure_table(&d, &mut fonts(), &mut LayoutCache::new(), &t, 450.0).unwrap();
    assert_eq!(m.row_heights.len(), 2);
    // Three paragraphs stack three lines: the second row is taller.
    assert!(
        m.row_heights[1] > m.row_heights[0] * 2.5,
        "{:?}",
        m.row_heights
    );
    // The long text is wider than "Ngắn"; both include the two 5.4 pt cell margins.
    assert!(
        m.max_widths[0] > m.max_widths[1] * 3.0,
        "{:?}",
        m.max_widths
    );
    assert!(m.max_widths[1] > 10.8);
}

/// AutoFit to contents with a cell spanning two columns (§17.4.17 gridSpan): the
/// columns it covers grow (in proportion) until its text fits on one line.
#[test]
fn spanning_cell_widens_the_columns_it_covers() {
    let mut t = Table::simple(2, 3, Twips(9000));
    // Row 0: one cell over columns 0–1 with a long title, then a short cell.
    t.rows[0].cells.remove(1);
    t.rows[0].cells[0].props.grid_span = 2;
    t.rows[0].cells[0].blocks = vec![p("Tiêu đề rất dài trải qua hai cột của bảng")];
    t.rows[0].cells[1].blocks = vec![p("C")];
    t.rows[1].cells[0].blocks = vec![p("Mã")];
    t.rows[1].cells[1].blocks = vec![p("Tên hàng")];
    t.rows[1].cells[2].blocks = vec![p("SL")];
    let d = doc(vec![Block::Table(t.clone())]);
    let m = measure_table(&d, &mut fonts(), &mut LayoutCache::new(), &t, 450.0).unwrap();
    // The title alone, measured as a one-column cell.
    let mut alone = Table::simple(1, 1, Twips(9000));
    alone.rows[0].cells[0].blocks = vec![p("Tiêu đề rất dài trải qua hai cột của bảng")];
    let d1 = doc(vec![Block::Table(alone.clone())]);
    let title = measure_table(&d1, &mut fonts(), &mut LayoutCache::new(), &alone, 450.0)
        .unwrap()
        .max_widths[0];
    let w = &m.max_widths;
    assert_eq!(w.len(), 3);
    assert!((w[0] + w[1] - title).abs() < 0.5, "{w:?} vs {title}");
    // In proportion: "Tên hàng" is wider than "Mã", so column 1 stays the wider one.
    assert!(w[1] > w[0], "{w:?}");
    // The third column only holds its own text.
    let mut sl = Table::simple(1, 1, Twips(9000));
    sl.rows[0].cells[0].blocks = vec![p("SL")];
    let d2 = doc(vec![Block::Table(sl.clone())]);
    let own = measure_table(&d2, &mut fonts(), &mut LayoutCache::new(), &sl, 450.0)
        .unwrap()
        .max_widths[0];
    assert!((w[2] - own).abs() < 0.5, "{w:?} vs {own}");
}
