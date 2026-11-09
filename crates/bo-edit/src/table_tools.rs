//! Table tools (ECMA-376 Part 1, §17.4): distribute rows and columns evenly, AutoFit
//! (§17.4.53 tblLayout, §17.4.63 tblW, §17.4.71 tcW), sorting rows, text direction in
//! cells (§17.4.72 textDirection), floating tables (§17.4.58 tblpPr) and converting
//! text to a table and back.
//!
//! The functions here change a [`Table`] value; [`crate::Session::edit_table`] records
//! the change as one reversible block replacement.

use crate::collate::vi_cmp;
use crate::table::{cell_covering, span, starts};
use bo_model::{
    Block, Cell, HeightRule, ParaProps, Paragraph, Run, RunContent, Table, TableFloat, TableWidth,
    TextDirection, Twips, VMerge,
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// How AutoFit sizes the columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoFit {
    /// Columns as wide as their content (`tblLayout type="autofit"`, `tblW type="auto"`).
    #[default]
    Contents,
    /// The table fills the text width (`tblW type="pct"` 100 %).
    Window,
    /// Column widths stay as they are (`tblLayout type="fixed"`).
    Fixed,
}

/// What a sort key compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortKind {
    /// Text in Vietnamese dictionary order.
    #[default]
    Text,
    /// The first number of the cell.
    Number,
    /// A date written day first (`dd/mm/yyyy`) or ISO (`yyyy-mm-dd`).
    Date,
}

/// One sort level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortKey {
    /// Grid column (0-based) holding the key.
    pub column: usize,
    /// How the cells compare.
    #[serde(default)]
    pub kind: SortKind,
    /// Largest first.
    #[serde(default)]
    pub descending: bool,
}

/// Separator between cells when converting text to a table and back: `"\t"` (tab),
/// `","`, `"\n"` (one cell per paragraph) or any other text.
pub(crate) const PARAGRAPH: &str = "\n";

/// Make every cell's preferred width (`tcW`, §17.4.71) the width of its grid columns.
pub(crate) fn sync_cell_widths(t: &mut Table) {
    for row in &mut t.rows {
        let st = starts(row);
        for (c, s) in row.cells.iter_mut().zip(st) {
            let n = span(c);
            let w: i32 = t.grid.iter().skip(s).take(n).map(|g| g.0).sum();
            if w > 0 {
                c.props.width = Some(TableWidth::Dxa(Twips(w)));
            }
        }
    }
}

/// Give rows `r0..=r1` (all rows when only one is selected) the same height: the
/// average of their measured `heights` (twips, one per row), as a minimum height
/// (§17.4.81 trHeight `atLeast`; exact heights stay exact).
pub(crate) fn distribute_rows(t: &mut Table, r0: usize, r1: usize, heights: &[i32]) {
    let (r0, r1) = if r0 == r1 {
        (0, t.rows.len().saturating_sub(1))
    } else {
        (r0, r1)
    };
    let n = r1 + 1 - r0;
    let measured: Vec<i32> = (r0..=r1)
        .map(|r| {
            heights
                .get(r)
                .copied()
                .filter(|h| *h > 0)
                .unwrap_or_else(|| {
                    t.rows
                        .get(r)
                        .and_then(|row| row.props.height)
                        .map_or(0, |(h, _)| h.0)
                })
        })
        .collect();
    let total: i32 = measured.iter().sum();
    if total <= 0 || n == 0 {
        return;
    }
    let each = Twips((total + (n as i32) / 2) / n as i32);
    for row in t.rows.iter_mut().take(r1 + 1).skip(r0) {
        let rule = match row.props.height {
            Some((_, HeightRule::Exact)) => HeightRule::Exact,
            _ => HeightRule::AtLeast,
        };
        row.props.height = Some((each, rule));
    }
}

/// Give grid columns `g0..=g1` (all columns when only one is selected) the same width,
/// keeping their total (§17.4.48 gridCol), and update the cell widths.
pub(crate) fn distribute_columns(t: &mut Table, g0: usize, g1: usize) {
    let cols = t.grid.len();
    if cols == 0 {
        return;
    }
    let (g0, g1) = if g0 == g1 {
        (0, cols - 1)
    } else {
        (g0, g1.min(cols - 1))
    };
    let n = (g1 + 1 - g0) as i32;
    let total: i32 = t.grid[g0..=g1].iter().map(|w| w.0).sum();
    let each = total / n;
    for (i, w) in t.grid[g0..=g1].iter_mut().enumerate() {
        // The last column takes the rounding remainder.
        w.0 = if i as i32 == n - 1 {
            total - each * (n - 1)
        } else {
            each
        };
    }
    sync_cell_widths(t);
}

/// AutoFit the table. `widths` (twips per grid column) are the measured content widths
/// for [`AutoFit::Contents`]; `avail` is the text width the table sits in.
pub(crate) fn auto_fit(t: &mut Table, mode: AutoFit, widths: &[i32], avail: Twips) {
    match mode {
        AutoFit::Contents => {
            // ECMA-376 Part 1, §17.4.53 tblLayout autofit; §17.4.63 tblW auto.
            t.props.fixed_layout = false;
            t.props.width = Some(TableWidth::Auto);
            if widths.len() == t.grid.len() && widths.iter().all(|w| *w > 0) {
                t.grid = widths.iter().map(|w| Twips(*w)).collect();
            }
            sync_cell_widths(t);
        }
        AutoFit::Window => {
            // §17.4.63 tblW type="pct": 5000 fiftieths of a percent = 100 %.
            t.props.fixed_layout = false;
            t.props.width = Some(TableWidth::Pct(5000));
            let total: i64 = t.grid.iter().map(|w| i64::from(w.0)).sum();
            let indent = t.props.indent.map_or(0, |i| i.0.max(0));
            let target = i64::from((avail.0 - indent).max(0));
            if total > 0 && target > 0 {
                let mut acc = 0i64;
                let mut prev = 0i64;
                for w in &mut t.grid {
                    acc += i64::from(w.0);
                    // Scale the running total so the columns add up exactly.
                    let end = acc * target / total;
                    w.0 = i32::try_from(end - prev).unwrap_or(w.0);
                    prev = end;
                }
            }
            sync_cell_widths(t);
        }
        AutoFit::Fixed => {
            // §17.4.53 tblLayout type="fixed": the widths no longer follow the content.
            t.props.fixed_layout = true;
            let total: i32 = t.grid.iter().map(|w| w.0).sum();
            if total > 0 {
                t.props.width = Some(TableWidth::Dxa(Twips(total)));
            }
            sync_cell_widths(t);
        }
    }
}

/// Text of a cell: its paragraphs joined with spaces, trimmed.
pub(crate) fn cell_text(c: &Cell) -> String {
    let mut parts = Vec::new();
    for b in &c.blocks {
        if let Block::Paragraph(p) = b {
            parts.push(p.plain_text());
        }
    }
    parts.join(" ").trim().to_owned()
}

/// The first number in `s` (`1.234,5` and `1,234.5` both read as 1234.5).
pub(crate) fn first_number(s: &str) -> Option<f64> {
    let chars: Vec<char> = s.chars().collect();
    let start = chars.iter().position(char::is_ascii_digit)?;
    let neg = start > 0 && matches!(chars[start - 1], '-' | '\u{2212}');
    let mut end = start;
    while end < chars.len() && (chars[end].is_ascii_digit() || matches!(chars[end], '.' | ',')) {
        end += 1;
    }
    let tok: String = chars[start..end].iter().collect();
    let tok = tok.trim_end_matches(['.', ',']);
    let commas = tok.matches(',').count();
    let dots = tok.matches('.').count();
    let norm = if commas > 0 && dots > 0 {
        // The last separator is the decimal one.
        if tok.rfind(',') > tok.rfind('.') {
            tok.replace('.', "").replace(',', ".")
        } else {
            tok.replace(',', "")
        }
    } else if commas == 1 && tok.split(',').nth(1).is_some_and(|d| d.len() != 3) {
        tok.replace(',', ".")
    } else if commas > 0 {
        tok.replace(',', "")
    } else if dots > 1 {
        tok.replace('.', "")
    } else {
        tok.to_owned()
    };
    let v: f64 = norm.parse().ok()?;
    Some(if neg { -v } else { v })
}

/// A date in `s`: `d/m/yyyy` (day first, `/`, `-` or `.`) or `yyyy-mm-dd`, as
/// `(year, month, day)`. Two-digit years are 2000–2049 or 1950–1999.
pub(crate) fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let nums: Vec<&str> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .take(3)
        .collect();
    if nums.len() < 3 {
        return None;
    }
    let n = |i: usize| nums[i].parse::<i32>().ok();
    let (y, m, d) = if nums[0].len() == 4 {
        (n(0)?, n(1)?, n(2)?)
    } else {
        let mut y = n(2)?;
        if nums[2].len() <= 2 {
            y += if y < 50 { 2000 } else { 1900 };
        }
        (y, n(1)?, n(0)?)
    };
    let (m, d) = (u32::try_from(m).ok()?, u32::try_from(d).ok()?);
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((y, m, d))
}

/// Sort value of a cell for one key (`None` = blank or not readable, sorted last).
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Text(String),
    Num(f64),
    Date((i32, u32, u32)),
}

fn value(text: &str, kind: SortKind) -> Option<Value> {
    if text.is_empty() {
        return None;
    }
    match kind {
        SortKind::Text => Some(Value::Text(text.to_owned())),
        SortKind::Number => first_number(text).map(Value::Num),
        SortKind::Date => parse_date(text).map(Value::Date),
    }
}

fn cmp_values(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Text(x), Value::Text(y)) => vi_cmp(x, y),
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Date(x), Value::Date(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

/// Why rows `r0..=r1` cannot be sorted, if they cannot.
pub(crate) fn sort_blocker(t: &Table, r0: usize, r1: usize) -> Option<&'static str> {
    let rows = t.rows.get(r0..=r1)?;
    if rows
        .iter()
        .any(|r| r.cells.iter().any(|c| c.props.v_merge.is_some()))
    {
        return Some("rows with vertically merged cells cannot be sorted");
    }
    if rows
        .iter()
        .any(|r| r.props.ins.is_some() || r.props.del.is_some())
    {
        return Some("rows with tracked changes cannot be sorted");
    }
    None
}

/// Sort rows `r0..=r1` by `keys` (first key most significant). The sort is stable;
/// blank and unreadable keys go last in both directions.
pub(crate) fn sort_rows(t: &mut Table, r0: usize, r1: usize, keys: &[SortKey]) {
    if r1 >= t.rows.len() || r0 >= r1 || keys.is_empty() {
        return;
    }
    let mut rows: Vec<(Vec<Option<Value>>, bo_model::Row)> = t
        .rows
        .drain(r0..=r1)
        .map(|row| {
            let vals = keys
                .iter()
                .map(|k| {
                    let text = cell_covering(&row, k.column)
                        .map(|i| cell_text(&row.cells[i]))
                        .unwrap_or_default();
                    value(&text, k.kind)
                })
                .collect();
            (vals, row)
        })
        .collect();
    rows.sort_by(|(a, _), (b, _)| {
        for (k, (va, vb)) in keys.iter().zip(a.iter().zip(b)) {
            let o = match (va, vb) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(x), Some(y)) => {
                    let o = cmp_values(x, y);
                    if k.descending {
                        o.reverse()
                    } else {
                        o
                    }
                }
            };
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    });
    for (i, (_, row)) in rows.into_iter().enumerate() {
        t.rows.insert(r0 + i, row);
    }
}

/// Text direction of the cells covering rows `r0..=r1`, grid columns `g0..=g1`
/// (§17.4.72 textDirection; `None` or `lrTb` = horizontal).
pub(crate) fn set_direction(
    t: &mut Table,
    (r0, r1): (usize, usize),
    (g0, g1): (usize, usize),
    dir: Option<TextDirection>,
) {
    let dir = dir.filter(|d| *d != TextDirection::LrTb);
    for row in t.rows.iter_mut().take(r1 + 1).skip(r0) {
        for g in g0..=g1 {
            if let Some(i) = cell_covering(row, g) {
                row.cells[i].props.text_direction = dir;
            }
        }
    }
}

/// Let the text wrap around the table (a §17.4.58 tblpPr at the table's place) or put
/// the table back in line with the text.
pub(crate) fn set_wrap(t: &mut Table, around: bool) {
    if !around {
        t.props.float = None;
        return;
    }
    if t.props.float.is_some() {
        return;
    }
    // 0.125" (180 twips) between the table and the text beside it.
    t.props.float = Some(TableFloat {
        left_from_text: Twips(180),
        right_from_text: Twips(180),
        top_from_text: Twips(0),
        bottom_from_text: Twips(0),
        horz_anchor: Some("text".into()),
        vert_anchor: Some("text".into()),
        x: t.props.indent.unwrap_or(Twips(0)),
        x_align: None,
        y: Twips(0),
        y_align: None,
    });
}

fn clean_props(p: &ParaProps) -> ParaProps {
    ParaProps {
        mark_ins: None,
        mark_del: None,
        change: None,
        ..p.clone()
    }
}

/// Split a paragraph's runs at `sep` (tabs for `"\t"`), keeping run formatting.
fn split_runs(p: &Paragraph, sep: &str) -> Vec<Vec<Run>> {
    let tab = sep == "\t";
    let mut cells: Vec<Vec<Run>> = vec![Vec::new()];
    for run in &p.runs {
        let mut cur = Run {
            props: run.props.clone(),
            content: Vec::new(),
        };
        let flush = |cur: &mut Run, cells: &mut Vec<Vec<Run>>, next: bool| {
            if !cur.content.is_empty() {
                if let Some(last) = cells.last_mut() {
                    last.push(std::mem::replace(
                        cur,
                        Run {
                            props: run.props.clone(),
                            content: Vec::new(),
                        },
                    ));
                }
            }
            if next {
                cells.push(Vec::new());
            }
        };
        for c in &run.content {
            match c {
                RunContent::Tab if tab => flush(&mut cur, &mut cells, true),
                RunContent::Text(s) if !sep.is_empty() && s.contains(sep) => {
                    for (i, part) in s.split(sep).enumerate() {
                        if i > 0 {
                            flush(&mut cur, &mut cells, true);
                        }
                        if !part.is_empty() {
                            cur.content.push(RunContent::Text(part.to_owned()));
                        }
                    }
                }
                other => cur.content.push(other.clone()),
            }
        }
        flush(&mut cur, &mut cells, false);
    }
    cells
}

/// A table made of `paras`: one row per paragraph with cells split at `sep`, or with
/// `sep` = `"\n"` one cell per paragraph, `cols` cells per row. Columns share `width`.
pub(crate) fn paragraphs_to_table(
    paras: &[Paragraph],
    sep: &str,
    cols: Option<usize>,
    width: Twips,
) -> Table {
    let rows: Vec<Vec<Paragraph>> = if sep == PARAGRAPH {
        let n = cols.unwrap_or(1).clamp(1, 63);
        paras.chunks(n).map(<[Paragraph]>::to_vec).collect()
    } else {
        paras
            .iter()
            .map(|p| {
                let props = clean_props(&p.props);
                split_runs(p, sep)
                    .into_iter()
                    .map(|runs| Paragraph::new(props.clone(), runs))
                    .collect()
            })
            .collect()
    };
    let ncols = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(1)
        .max(cols.unwrap_or(1))
        .clamp(1, 63);
    let mut t = Table::simple(rows.len().max(1), ncols, width);
    for (row, src) in t.rows.iter_mut().zip(rows) {
        for (cell, p) in row.cells.iter_mut().zip(src) {
            let mut p = p;
            p.props = clean_props(&p.props);
            p.src = None;
            cell.blocks = vec![Block::Paragraph(p)];
        }
    }
    t
}

/// The blocks replacing a table converted to text: each row becomes a paragraph with
/// its cells joined by `sep`; with `sep` = `"\n"` every cell paragraph stands alone.
/// Further paragraphs and nested tables of a cell follow in order.
pub(crate) fn table_to_blocks(t: &Table, sep: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for row in &t.rows {
        let mut line: Option<Paragraph> = None;
        for c in &row.cells {
            if c.props.v_merge == Some(VMerge::Continue) && sep != PARAGRAPH {
                // The merged cell's text is in the row that starts the merge.
                if let Some(p) = line.as_mut() {
                    push_sep(p, sep);
                }
                continue;
            }
            for (bi, b) in c.blocks.iter().enumerate() {
                match b {
                    Block::Paragraph(p) if bi == 0 && sep != PARAGRAPH => {
                        if let Some(l) = line.as_mut() {
                            push_sep(l, sep);
                            l.runs.extend(p.runs.iter().cloned());
                        } else {
                            let mut p = p.clone();
                            p.props = clean_props(&p.props);
                            p.src = None;
                            line = Some(p);
                        }
                    }
                    Block::Paragraph(p) => {
                        if let Some(l) = line.take() {
                            out.push(Block::Paragraph(l));
                        }
                        let mut p = p.clone();
                        p.props = clean_props(&p.props);
                        p.src = None;
                        if sep == PARAGRAPH {
                            out.push(Block::Paragraph(p));
                        } else {
                            line = Some(p);
                        }
                    }
                    other => {
                        if let Some(l) = line.take() {
                            out.push(Block::Paragraph(l));
                        }
                        out.push(other.clone());
                    }
                }
            }
        }
        if let Some(l) = line.take() {
            out.push(Block::Paragraph(l));
        }
    }
    if out.is_empty() {
        out.push(Block::Paragraph(Paragraph::default()));
    }
    out
}

fn push_sep(p: &mut Paragraph, sep: &str) {
    let props = p.runs.last().map(|r| r.props.clone()).unwrap_or_default();
    let content = if sep == "\t" {
        RunContent::Tab
    } else {
        RunContent::Text(sep.to_owned())
    };
    p.runs.push(Run {
        props,
        content: vec![content],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::RunProps;

    fn para(t: &str) -> Paragraph {
        Paragraph::new(
            ParaProps::default(),
            vec![Run::text(t, RunProps::default())],
        )
    }

    fn table(rows: &[&[&str]]) -> Table {
        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
        let mut t = Table::simple(rows.len(), cols, Twips(9000));
        for (row, src) in t.rows.iter_mut().zip(rows) {
            for (c, s) in row.cells.iter_mut().zip(src.iter()) {
                c.blocks = vec![Block::Paragraph(para(s))];
            }
        }
        t
    }

    fn texts(t: &Table, col: usize) -> Vec<String> {
        t.rows.iter().map(|r| cell_text(&r.cells[col])).collect()
    }

    #[test]
    fn numbers_and_dates() {
        assert_eq!(first_number("Tổng: 1.234,5 đ"), Some(1234.5));
        assert_eq!(first_number("1,234.5"), Some(1234.5));
        assert_eq!(first_number("2.500.000"), Some(2_500_000.0));
        assert_eq!(first_number("-12"), Some(-12.0));
        assert_eq!(first_number("abc"), None);
        assert_eq!(parse_date("05/03/2024"), Some((2024, 3, 5)));
        assert_eq!(parse_date("2024-03-05"), Some((2024, 3, 5)));
        assert_eq!(parse_date("1.2.99"), Some((1999, 2, 1)));
        assert_eq!(parse_date("31/13/2024"), None);
    }

    #[test]
    fn sorts_text_numbers_dates() {
        let mut t = table(&[
            &["Tên", "Số", "Ngày"],
            &["Đức", "10", "02/01/2024"],
            &["An", "9", "15/12/2023"],
            &["Ánh", "", "01/02/2024"],
            &["Bình", "100", ""],
        ]);
        let key = |column, kind, descending| SortKey {
            column,
            kind,
            descending,
        };
        sort_rows(&mut t, 1, 4, &[key(0, SortKind::Text, false)]);
        assert_eq!(texts(&t, 0), ["Tên", "An", "Ánh", "Bình", "Đức"]);
        sort_rows(&mut t, 1, 4, &[key(1, SortKind::Number, true)]);
        // 100 > 10 > 9 (as numbers), the blank last.
        assert_eq!(texts(&t, 1), ["Số", "100", "10", "9", ""]);
        sort_rows(&mut t, 1, 4, &[key(2, SortKind::Date, false)]);
        assert_eq!(
            texts(&t, 2),
            ["Ngày", "15/12/2023", "02/01/2024", "01/02/2024", ""]
        );
    }

    #[test]
    fn sort_refuses_vertical_merges() {
        let mut t = table(&[&["a"], &["b"]]);
        t.rows[0].cells[0].props.v_merge = Some(VMerge::Restart);
        t.rows[1].cells[0].props.v_merge = Some(VMerge::Continue);
        assert!(sort_blocker(&t, 0, 1).is_some());
        assert!(sort_blocker(&table(&[&["a"], &["b"]]), 0, 1).is_none());
    }

    #[test]
    fn distributes() {
        let mut t = table(&[&["a", "b", "c"], &["d", "e", "f"]]);
        t.grid = vec![Twips(1000), Twips(2000), Twips(3001)];
        distribute_columns(&mut t, 0, 0);
        // 6001 / 3 = 2000 rest 1, given to the last column.
        assert_eq!(t.grid, vec![Twips(2000), Twips(2000), Twips(2001)]);
        assert_eq!(
            t.rows[1].cells[2].props.width,
            Some(TableWidth::Dxa(Twips(2001)))
        );
        distribute_rows(&mut t, 0, 0, &[300, 500]);
        assert!(t
            .rows
            .iter()
            .all(|r| r.props.height == Some((Twips(400), HeightRule::AtLeast))));
    }

    #[test]
    fn auto_fit_modes() {
        let mut t = table(&[&["a", "b"]]);
        auto_fit(&mut t, AutoFit::Window, &[], Twips(10_000));
        assert_eq!(t.props.width, Some(TableWidth::Pct(5000)));
        assert_eq!(t.grid, vec![Twips(5000), Twips(5000)]);
        auto_fit(&mut t, AutoFit::Contents, &[800, 1200], Twips(10_000));
        assert_eq!(t.props.width, Some(TableWidth::Auto));
        assert!(!t.props.fixed_layout);
        assert_eq!(t.grid, vec![Twips(800), Twips(1200)]);
        assert_eq!(
            t.rows[0].cells[1].props.width,
            Some(TableWidth::Dxa(Twips(1200)))
        );
        auto_fit(&mut t, AutoFit::Fixed, &[], Twips(10_000));
        assert!(t.props.fixed_layout);
        assert_eq!(t.props.width, Some(TableWidth::Dxa(Twips(2000))));
    }

    #[test]
    fn text_to_table_and_back() {
        let paras = vec![para("Họ tên\tTuổi"), para("Lan\t30"), para("Minh")];
        let t = paragraphs_to_table(&paras, "\t", None, Twips(9000));
        assert_eq!((t.rows.len(), t.grid.len()), (3, 2));
        assert_eq!(texts(&t, 1), ["Tuổi", "30", ""]);
        let csv = paragraphs_to_table(&[para("a,b,c")], ",", None, Twips(9000));
        assert_eq!(csv.rows[0].cells.len(), 3);
        let each = paragraphs_to_table(&paras, PARAGRAPH, Some(2), Twips(9000));
        assert_eq!((each.rows.len(), each.grid.len()), (2, 2));
        assert_eq!(cell_text(&each.rows[1].cells[0]), "Minh");
        let back = table_to_blocks(&t, ";");
        let text: Vec<String> = back
            .iter()
            .filter_map(Block::as_paragraph)
            .map(Paragraph::plain_text)
            .collect();
        assert_eq!(text, ["Họ tên;Tuổi", "Lan;30", "Minh;"]);
        let tabs = table_to_blocks(&t, "\t");
        assert_eq!(
            tabs.first()
                .and_then(Block::as_paragraph)
                .map(Paragraph::plain_text),
            Some("Họ tên\tTuổi".to_owned())
        );
        let lines = table_to_blocks(&t, PARAGRAPH);
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn direction_and_wrap() {
        let mut t = table(&[&["a", "b"]]);
        set_direction(&mut t, (0, 0), (1, 1), Some(TextDirection::BtLr));
        assert_eq!(t.rows[0].cells[0].props.text_direction, None);
        assert_eq!(
            t.rows[0].cells[1].props.text_direction,
            Some(TextDirection::BtLr)
        );
        set_direction(&mut t, (0, 0), (0, 1), Some(TextDirection::LrTb));
        assert_eq!(t.rows[0].cells[1].props.text_direction, None);
        set_wrap(&mut t, true);
        assert!(t.props.float.is_some());
        set_wrap(&mut t, false);
        assert!(t.props.float.is_none());
    }
}
