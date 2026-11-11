//! Multi-level sort of a cell range: rows reordered top-to-bottom, or columns
//! left-to-right (ECMA-376 Part 1, §18.3.1.92 `sortState/@columnSort`), by
//! value, by cell / font colour or by conditional-format icon (§18.3.1.91
//! `sortCondition/@sortBy`), or in the order of a custom list
//! (`sortCondition/@customList`).
//!
//! Ascending order follows the usual spreadsheet convention: numbers, then text
//! (case-insensitive unless requested), then logical values (FALSE < TRUE), then
//! errors; blank cells always sort last, in both directions. The sort is stable.
//! Only the cells inside the range move (values, formulas, styles); formula
//! references are not rewritten here (see [`crate::calc`]).

use crate::calc::{FormulaAdjuster, NoFormulas};
use bo_model::sheet::{Cell, CellRange, CellValue, Workbook, Worksheet};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// What a sort level compares (§18.18.79 `ST_SortBy`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SortBy {
    /// Compare cell values (numbers before text, blanks last).
    #[default]
    Value,
    /// Cells filled with this colour first (last when descending).
    CellColor((u8, u8, u8)),
    /// Cells whose font has this colour first (last when descending).
    FontColor((u8, u8, u8)),
    /// Cells showing icon `id` (zero-based) of conditional-format icon set `set`
    /// (§18.18.42 `ST_IconSetType`) first (last when descending).
    Icon {
        /// Icon set name, e.g. `3Arrows`.
        set: String,
        /// Zero-based icon of the set (0 = icon of the lowest band).
        id: u32,
    },
}

/// One sort level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    /// Absolute zero-based column of the key (the key row when sorting left to right).
    pub col: u32,
    /// Sort from largest to smallest (`descending`).
    pub descending: bool,
    /// Text comparison distinguishes case (`caseSensitive`).
    pub case_sensitive: bool,
    /// What the level compares.
    pub by: SortBy,
    /// Custom order of a value key (`sortCondition/@customList`): text matching an
    /// entry (case-insensitively) sorts in list order, before any other value;
    /// descending reverses the list.
    pub custom_list: Option<Vec<String>>,
}

impl SortKey {
    /// Ascending, case-insensitive key on `col`.
    pub fn asc(col: u32) -> SortKey {
        SortKey {
            col,
            descending: false,
            case_sensitive: false,
            by: SortBy::Value,
            custom_list: None,
        }
    }

    /// Descending, case-insensitive key on `col`.
    pub fn desc(col: u32) -> SortKey {
        SortKey {
            col,
            descending: true,
            case_sensitive: false,
            by: SortBy::Value,
            custom_list: None,
        }
    }
}

fn rank(v: &CellValue) -> u8 {
    match v {
        CellValue::Number(_) | CellValue::Date(_) => 0,
        CellValue::Text(_) | CellValue::RichText(_) => 1,
        CellValue::Bool(_) => 2,
        CellValue::Error(_) => 3,
        CellValue::Empty => 4,
    }
}

fn is_blank(v: Option<&CellValue>) -> bool {
    match v {
        None | Some(CellValue::Empty) => true,
        Some(CellValue::Text(s)) => s.is_empty(),
        _ => false,
    }
}

/// Compare two non-blank values in ascending order.
pub fn compare_values(a: &CellValue, b: &CellValue, case_sensitive: bool) -> Ordering {
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (CellValue::Number(x), CellValue::Number(y)) => x.total_cmp(y),
        (CellValue::Bool(x), CellValue::Bool(y)) => x.cmp(y),
        (CellValue::Date(x), CellValue::Date(y)) => x.cmp(y),
        // Same rank: numbers before ISO dates (a total order for the sort).
        (CellValue::Number(_), CellValue::Date(_)) => Ordering::Less,
        (CellValue::Date(_), CellValue::Number(_)) => Ordering::Greater,
        _ => {
            let (sa, sb) = (
                a.as_text().unwrap_or_default(),
                b.as_text().unwrap_or_default(),
            );
            if case_sensitive {
                // Lower case before upper case for otherwise equal text.
                text_key(&sa)
                    .cmp(&text_key(&sb))
                    .then_with(|| sa.to_lowercase().cmp(&sb.to_lowercase()))
                    .then_with(|| sb.cmp(&sa))
            } else {
                text_key(&sa)
                    .cmp(&text_key(&sb))
                    .then_with(|| sa.to_lowercase().cmp(&sb.to_lowercase()))
            }
        }
    }
}

/// Collation key for text: lower case with diacritics removed (NFD, combining marks
/// dropped) and `đ` folded to `d`, so Vietnamese words sort by their base letters.
pub fn text_key(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    if s.is_ascii() {
        // NFD leaves ASCII unchanged.
        return s.to_ascii_lowercase();
    }
    s.nfd()
        .filter(|c| !matches!(*c as u32, 0x0300..=0x036F))
        .map(|c| match c {
            'đ' | 'Đ' => 'd',
            c => c,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// Split a `customList` attribute value into its entries (comma-separated).
pub fn split_custom_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_owned())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Sort the rows of `range` by `keys` (first key most significant). With
/// `has_header` the first row stays in place. Keys outside the range are ignored.
/// Formula text moves unchanged; see [`sort_range_with`].
pub fn sort_range(ws: &mut Worksheet, range: CellRange, keys: &[SortKey], has_header: bool) {
    sort_range_with(ws, range, keys, has_header, &NoFormulas);
}

/// [`sort_range`] that rewrites the formulas of moved rows through `adj`
/// ([`FormulaAdjuster::translate`] by the row distance), so relative
/// references keep pointing at the same row, as when the row is cut and pasted.
/// Colour keys are ignored (they need the stylesheet, see [`sort_lines`]).
pub fn sort_range_with(
    ws: &mut Worksheet,
    range: CellRange,
    keys: &[SortKey],
    has_header: bool,
    adj: &dyn FormulaAdjuster,
) {
    let keys: Vec<SortKey> = keys
        .iter()
        .filter(|k| k.by == SortBy::Value)
        .cloned()
        .collect();
    let mut wb = Workbook {
        sheets: vec![std::mem::take(ws)],
        ..Workbook::default()
    };
    sort_lines(&mut wb, 0, range, &keys, has_header, false, adj);
    *ws = wb.sheets.pop().unwrap_or_default();
}

/// Text of a value prepared for comparison: the collation key ([`text_key`]), the
/// lower-cased text when it differs from the key, and the original text (only kept
/// for case-sensitive keys).
#[derive(Debug, Default)]
struct PrepText {
    key: String,
    lower: Option<String>,
    orig: String,
}

impl PrepText {
    fn of(s: &str, case_sensitive: bool) -> PrepText {
        let key = text_key(s);
        let lower = s.to_lowercase();
        PrepText {
            lower: (lower != key).then_some(lower),
            key,
            orig: if case_sensitive {
                s.to_owned()
            } else {
                String::new()
            },
        }
    }

    fn lower(&self) -> &str {
        self.lower.as_deref().unwrap_or(&self.key)
    }
}

/// The non-blank value of a key cell prepared once per line, so that comparing two
/// lines allocates nothing (same order as [`compare_values`]).
#[derive(Debug)]
enum PrepVal {
    Number(f64),
    Bool(bool),
    Date(String),
    /// Text and rich text; errors compare as empty text.
    Text(PrepText),
}

/// One line's key for one level.
#[derive(Debug)]
enum Prep {
    Blank,
    Value {
        rank: u8,
        /// Position in the level's custom list.
        list: Option<usize>,
        v: PrepVal,
    },
    /// Whether the key cell matches a colour / icon level.
    Match(bool),
}

static EMPTY_TEXT: PrepText = PrepText {
    key: String::new(),
    lower: None,
    orig: String::new(),
};

impl PrepVal {
    fn text(&self) -> &PrepText {
        match self {
            PrepVal::Text(t) => t,
            _ => &EMPTY_TEXT,
        }
    }
}

fn prep_value(v: Option<&CellValue>, k: &SortKey, list: Option<&[String]>) -> Prep {
    let Some(v) = v.filter(|v| !is_blank(Some(v))) else {
        return Prep::Blank;
    };
    let list = list.and_then(|l| {
        let t = v.as_text()?.to_lowercase();
        l.iter().position(|x| *x == t)
    });
    let pv = match v {
        CellValue::Number(n) => PrepVal::Number(*n),
        CellValue::Bool(b) => PrepVal::Bool(*b),
        CellValue::Date(d) => PrepVal::Date(d.clone()),
        CellValue::Text(s) => PrepVal::Text(PrepText::of(s, k.case_sensitive)),
        CellValue::RichText(_) => PrepVal::Text(PrepText::of(
            &v.as_text().unwrap_or_default(),
            k.case_sensitive,
        )),
        CellValue::Error(_) | CellValue::Empty => PrepVal::Text(PrepText::default()),
    };
    Prep::Value {
        rank: rank(v),
        list,
        v: pv,
    }
}

/// [`compare_values`] on prepared values of the same rank.
fn prep_values_cmp(a: &PrepVal, b: &PrepVal, case_sensitive: bool) -> Ordering {
    match (a, b) {
        (PrepVal::Number(x), PrepVal::Number(y)) => x.total_cmp(y),
        (PrepVal::Bool(x), PrepVal::Bool(y)) => x.cmp(y),
        (PrepVal::Date(x), PrepVal::Date(y)) => x.cmp(y),
        (PrepVal::Number(_), PrepVal::Date(_)) => Ordering::Less,
        (PrepVal::Date(_), PrepVal::Number(_)) => Ordering::Greater,
        _ => {
            let (ta, tb) = (a.text(), b.text());
            let o = ta.key.cmp(&tb.key).then_with(|| ta.lower().cmp(tb.lower()));
            if case_sensitive {
                // Lower case before upper case for otherwise equal text.
                o.then_with(|| tb.orig.cmp(&ta.orig))
            } else {
                o
            }
        }
    }
}

/// [`key_cmp`] (value levels) or the colour / icon order on prepared keys.
fn prep_cmp(a: &Prep, b: &Prep, k: &SortKey) -> Ordering {
    let dir = |o: Ordering| if k.descending { o.reverse() } else { o };
    match (a, b) {
        (Prep::Blank, Prep::Blank) => Ordering::Equal,
        (Prep::Blank, _) => Ordering::Greater,
        (_, Prep::Blank) => Ordering::Less,
        // Matching cells on top (descending: at the bottom).
        (Prep::Match(x), Prep::Match(y)) => dir(y.cmp(x)),
        (
            Prep::Value {
                rank: ra,
                list: la,
                v: va,
            },
            Prep::Value {
                rank: rb,
                list: lb,
                v: vb,
            },
        ) => {
            if k.custom_list.is_some() {
                // Listed values first, in list order (reversed when descending); the
                // others after them, by value in the level's direction.
                match (la, lb) {
                    (Some(x), Some(y)) => return dir(x.cmp(y)),
                    (Some(_), None) => return Ordering::Less,
                    (None, Some(_)) => return Ordering::Greater,
                    (None, None) => {}
                }
            }
            dir(ra
                .cmp(rb)
                .then_with(|| prep_values_cmp(va, vb, k.case_sensitive)))
        }
        _ => Ordering::Equal,
    }
}

/// Sort `range` of sheet `sheet`: rows by key columns, or with `by_columns`
/// the columns left to right by key rows. With `has_header` the first row
/// (first column) stays in place. Colour keys use the workbook's styles, icon keys
/// the icons its conditional formats show (§18.3.1.49 `iconSet`).
///
/// Every level is reduced to one ordinal per line (column-oriented): the distinct
/// key values are prepared and ordered once, so the stable sort of the line
/// permutation compares integers. Rows are then permuted in place (see
/// [`permute_rows_in_place`]).
pub fn sort_lines(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    keys: &[SortKey],
    has_header: bool,
    by_columns: bool,
    adj: &dyn FormulaAdjuster,
) -> Option<LinePerm> {
    let (lo, hi) = if by_columns {
        (range.first.row, range.last.row)
    } else {
        (range.first.col, range.last.col)
    };
    let keys: Vec<SortKey> = keys
        .iter()
        .filter(|k| k.col >= lo && k.col <= hi)
        .cloned()
        .collect();
    let (start, end) = if by_columns {
        (range.first.col + u32::from(has_header), range.last.col)
    } else {
        (range.first.row + u32::from(has_header), range.last.row)
    };
    if keys.is_empty() || start >= end || sheet >= wb.sheets.len() {
        return None;
    }
    let ords = key_ordinals(wb, sheet, range, &keys, (start, end), by_columns);
    let mut perm: Vec<u32> = (0..=end - start).collect();
    match ords.as_slice() {
        [a] => perm.sort_by_key(|&i| a[i as usize]),
        [a, b] => {
            perm.sort_by_key(|&i| (u64::from(a[i as usize]) << 32) | u64::from(b[i as usize]));
        }
        _ => perm.sort_by(|&x, &y| {
            ords.iter()
                .map(|o| o[x as usize].cmp(&o[y as usize]))
                .find(|o| *o != Ordering::Equal)
                .unwrap_or(Ordering::Equal)
        }),
    }
    if perm.iter().enumerate().all(|(i, p)| i as u32 == *p) {
        return None;
    }
    let ws = &mut wb.sheets[sheet];
    if by_columns {
        move_columns(ws, range, start, &perm, adj);
    } else {
        permute_rows(ws, range, start, &perm, Some(adj));
    }
    Some(LinePerm { start, perm })
}

/// The lines a sort moved: line `start + n` received line `start + perm[n]`
/// (rows, or columns of a left-to-right sort).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinePerm {
    /// First moved line (after the header).
    pub start: u32,
    /// Source line of every destination line, relative to `start`.
    pub perm: Vec<u32>,
}

/// Permute the rows `start..start + perm.len()` inside `range`'s columns: row
/// `start + n` receives the cells of row `start + perm[n]`. With `adj` the
/// formulas of moved cells are translated by the distance moved (a sort);
/// without, cells move as they are (undoing a sort, whose formula texts are
/// restored separately).
pub fn permute_rows(
    ws: &mut Worksheet,
    range: CellRange,
    start: u32,
    perm: &[u32],
    adj: Option<&dyn FormulaAdjuster>,
) {
    if perm.is_empty() {
        return;
    }
    if !permute_rows_in_place(ws, range, start, perm, adj) {
        move_rows(ws, range, start, perm, adj);
    }
}

/// Identity of a key value for sharing its preparation among lines (values with
/// the same identity compare equal).
#[derive(PartialEq, Eq, Hash)]
enum Distinct<'a> {
    Blank,
    Number(u64),
    Bool(bool),
    Date(&'a str),
    Text(&'a str),
    Error(&'a bo_model::sheet::CellError),
    Match(bool),
}

/// The ordinal of every line `lines.0..=lines.1` for every level: lines with equal
/// ordinals compare equal, smaller ordinals sort first ([`prep_cmp`] order).
fn key_ordinals(
    wb: &Workbook,
    sheet: usize,
    range: CellRange,
    keys: &[SortKey],
    lines: (u32, u32),
    by_columns: bool,
) -> Vec<Vec<u32>> {
    // Icons shown by conditional formats.
    let icons = if keys.iter().any(|k| matches!(k.by, SortBy::Icon { .. })) {
        crate::cf::evaluate(
            wb,
            sheet,
            range.first.row..range.last.row + 1,
            range.first.col..range.last.col + 1,
            None,
        )
        .into_iter()
        .filter_map(|(k, f)| f.icon.map(|i| (k, (i.set, i.index))))
        .collect::<std::collections::HashMap<(u32, u32), (String, usize)>>()
    } else {
        std::collections::HashMap::new()
    };
    let ws = &wb.sheets[sheet];
    let n = (lines.1 - lines.0 + 1) as usize;
    // Key cells of every line: one pass over the rows when sorting rows (no tree
    // lookup per line), lookups when sorting columns.
    let mut key_cells: Vec<Vec<Option<&Cell>>> = vec![vec![None; n]; keys.len()];
    if by_columns {
        for (k, out) in keys.iter().zip(&mut key_cells) {
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = ws.cells.get(&(k.col, lines.0 + i as u32));
            }
        }
    } else {
        for (&(r, c), cell) in ws.cells.range((lines.0, 0)..=(lines.1, u32::MAX)) {
            for (k, out) in keys.iter().zip(&mut key_cells) {
                if k.col == c {
                    out[(r - lines.0) as usize] = Some(cell);
                }
            }
        }
    }
    keys.iter()
        .zip(&key_cells)
        .map(|(k, cells)| {
            let list: Option<Vec<String>> = k
                .custom_list
                .as_ref()
                .map(|l| l.iter().map(|x| x.to_lowercase()).collect());
            let mut seen: std::collections::HashMap<Distinct<'_>, u32> =
                std::collections::HashMap::new();
            let mut distinct: Vec<Prep> = Vec::new();
            let mut of_line: Vec<u32> = Vec::with_capacity(n);
            for (i, cell) in cells.iter().enumerate() {
                let line = lines.0 + i as u32;
                let matched = |m: bool| (Distinct::Match(m), Prep::Match(m));
                let (id, prep) = match &k.by {
                    SortBy::Value => {
                        let v = cell.map(|c| &c.value).filter(|v| !is_blank(Some(v)));
                        let id = match v {
                            None => Some(Distinct::Blank),
                            Some(CellValue::Number(x)) => Some(Distinct::Number(x.to_bits())),
                            Some(CellValue::Bool(b)) => Some(Distinct::Bool(*b)),
                            Some(CellValue::Date(d)) => Some(Distinct::Date(d)),
                            Some(CellValue::Text(s)) => Some(Distinct::Text(s)),
                            Some(CellValue::Error(e)) => Some(Distinct::Error(e)),
                            // Rich text: not shared.
                            Some(_) => None,
                        };
                        if let Some(x) = id.as_ref().and_then(|id| seen.get(id)) {
                            of_line.push(*x);
                            continue;
                        }
                        (id, prep_value(v, k, list.as_deref()))
                    }
                    SortBy::CellColor(c) => {
                        let (id, p) =
                            matched(cell.is_some_and(|cell| {
                                crate::color::cell_fill_rgb(wb, cell) == Some(*c)
                            }));
                        (Some(id), p)
                    }
                    SortBy::FontColor(c) => {
                        let (id, p) = matched(cell.is_some_and(|cell| {
                            crate::color::cell_font_rgb(wb, cell).unwrap_or((0, 0, 0)) == *c
                        }));
                        (Some(id), p)
                    }
                    SortBy::Icon { set, id } => {
                        let at = if by_columns {
                            (k.col, line)
                        } else {
                            (line, k.col)
                        };
                        let (d, p) = matched(
                            icons
                                .get(&at)
                                .is_some_and(|(s, x)| s == set && *x == *id as usize),
                        );
                        (Some(d), p)
                    }
                };
                if let Some(x) = id.as_ref().and_then(|id| seen.get(id)) {
                    of_line.push(*x);
                    continue;
                }
                let x = distinct.len() as u32;
                distinct.push(prep);
                if let Some(id) = id {
                    seen.insert(id, x);
                }
                of_line.push(x);
            }
            // Order the distinct values; equal ones share an ordinal.
            let mut order: Vec<u32> = (0..distinct.len() as u32).collect();
            order.sort_by(|a, b| prep_cmp(&distinct[*a as usize], &distinct[*b as usize], k));
            let mut rank = vec![0u32; distinct.len()];
            let mut r = 0u32;
            for (j, &d) in order.iter().enumerate() {
                if j > 0
                    && prep_cmp(&distinct[order[j - 1] as usize], &distinct[d as usize], k)
                        != Ordering::Equal
                {
                    r += 1;
                }
                rank[d as usize] = r;
            }
            of_line.iter().map(|d| rank[*d as usize]).collect()
        })
        .collect()
}

/// Marker style of the empty cells [`permute_rows_in_place`] adds while permuting.
const PLACEHOLDER_STYLE: u32 = u32::MAX;

/// Permute the rows `start..` of `range` in place: row `start + n` receives the
/// cells of row `start + perm[n]`. Gaps are first filled with marked empty cells
/// so that every row holds the same columns; the cells then swap places through
/// the permutation's cycles (each moved once, the tree is not rebuilt) and the
/// marked cells are removed. Returns `false` (nothing changed) when the rows are
/// too sparse for this to pay off.
fn permute_rows_in_place(
    ws: &mut Worksheet,
    range: CellRange,
    start: u32,
    perm: &[u32],
    adj: Option<&dyn FormulaAdjuster>,
) -> bool {
    let n = perm.len();
    let end = start + (n as u32 - 1);
    let (c0, c1) = (range.first.col, range.last.col);
    let inside = |c: u32| (c0..=c1).contains(&c);
    // Columns used by any row, and the number of cells.
    let mut used = vec![false; (c1 - c0 + 1) as usize];
    let mut present = 0usize;
    for (&(_, c), cell) in ws.cells.range((start, c0)..=(end, c1)) {
        if inside(c) {
            if cell.style == PLACEHOLDER_STYLE {
                return false;
            }
            used[(c - c0) as usize] = true;
            present += 1;
        }
    }
    let cols: Vec<u32> = (c0..=c1).filter(|c| used[(c - c0) as usize]).collect();
    let k = cols.len();
    let missing = n * k - present;
    if k == 0 || missing > present / 2 + 4096 {
        return false;
    }
    // The (row, column) slots without a cell.
    let mut gaps: Vec<(u32, u32)> = Vec::with_capacity(missing);
    if missing > 0 {
        let mut row = start;
        let mut j = 0usize;
        let close = |row: u32, j: usize, gaps: &mut Vec<(u32, u32)>| {
            gaps.extend(cols[j..].iter().map(|c| (row, *c)));
        };
        for &(r, c) in ws.cells.range((start, c0)..=(end, c1)).map(|(key, _)| key) {
            if !inside(c) {
                continue;
            }
            while row < r {
                close(row, j, &mut gaps);
                row += 1;
                j = 0;
            }
            while cols[j] < c {
                gaps.push((row, cols[j]));
                j += 1;
            }
            j += 1;
        }
        loop {
            close(row, j, &mut gaps);
            if row == end {
                break;
            }
            row += 1;
            j = 0;
        }
    }
    for &key in &gaps {
        ws.cells.insert(
            key,
            Cell {
                style: PLACEHOLDER_STYLE,
                ..Cell::default()
            },
        );
    }
    {
        let mut slots: Vec<&mut Cell> = ws
            .cells
            .range_mut((start, c0)..=(end, c1))
            .filter(|((_, c), _)| inside(*c))
            .map(|(_, cell)| cell)
            .collect();
        debug_assert_eq!(slots.len(), n * k);
        let mut done = vec![false; n];
        let mut tmp: Vec<Cell> = Vec::with_capacity(k);
        // Line `to` takes the cells of line `from` (already taken out when it is
        // the start of the cycle, `tmp`).
        for i in 0..n {
            if done[i] {
                continue;
            }
            done[i] = true;
            if perm[i] as usize == i {
                continue;
            }
            tmp.clear();
            tmp.extend(
                slots[i * k..(i + 1) * k]
                    .iter_mut()
                    .map(|c| std::mem::take(&mut **c)),
            );
            let mut to = i;
            loop {
                let from = perm[to] as usize;
                let d = to as i64 - from as i64;
                if from == i {
                    for (s, mut cell) in tmp.drain(..).enumerate() {
                        translate(&mut cell, d, 0, adj);
                        *slots[to * k + s] = cell;
                    }
                    break;
                }
                for s in 0..k {
                    let mut cell = std::mem::take(&mut *slots[from * k + s]);
                    translate(&mut cell, d, 0, adj);
                    *slots[to * k + s] = cell;
                }
                done[from] = true;
                to = from;
            }
        }
    }
    if !gaps.is_empty() {
        let mut to_of = vec![0u32; n];
        for (to, &from) in perm.iter().enumerate() {
            to_of[from as usize] = to as u32;
        }
        for (r, c) in gaps {
            let at = (start + to_of[(r - start) as usize], c);
            ws.cells.remove(&at);
        }
    }
    true
}

/// Rewrite the formula of a cell moved by `(dr, dc)`.
fn translate(cell: &mut Cell, dr: i64, dc: i64, adj: Option<&dyn FormulaAdjuster>) {
    let Some(adj) = adj else {
        return;
    };
    if let Some(f) = &mut cell.formula {
        if (dr != 0 || dc != 0) && !f.text.is_empty() {
            if let Some(t) = adj.translate(&f.text, dr, dc) {
                f.text = t;
            }
        }
    }
}

/// Move the cells of rows `start..` inside `range`'s columns so that row
/// `start + n` receives the cells of row `start + perm[n]`. The rows are cut out of
/// the map once and rebuilt in order (no per-cell tree removal / insertion).
fn move_rows(
    ws: &mut Worksheet,
    range: CellRange,
    start: u32,
    perm: &[u32],
    adj: Option<&dyn FormulaAdjuster>,
) {
    let end = start + (perm.len() as u32 - 1);
    let (c0, c1) = (range.first.col, range.last.col);
    let mid = ws.cells.split_off(&(start, 0));
    let mut mid = mid;
    let mut after = match end.checked_add(1) {
        Some(next) => mid.split_off(&(next, 0)),
        None => BTreeMap::new(),
    };
    // Cells inside the range, line by line (`offsets[i]..offsets[i + 1]`), and the
    // cells of the same rows outside the range (they stay where they are).
    let mut moved: Vec<(u32, Cell)> = Vec::with_capacity(mid.len());
    let mut fixed: Vec<((u32, u32), Cell)> = Vec::new();
    let mut offsets: Vec<usize> = Vec::with_capacity(perm.len() + 1);
    offsets.push(0);
    let mut row = start;
    for ((r, c), cell) in mid {
        while row < r {
            offsets.push(moved.len());
            row += 1;
        }
        if (c0..=c1).contains(&c) {
            moved.push((c, cell));
        } else {
            fixed.push(((r, c), cell));
        }
    }
    while offsets.len() <= perm.len() {
        offsets.push(moved.len());
    }
    let mut out: Vec<((u32, u32), Cell)> = Vec::with_capacity(moved.len() + fixed.len());
    let mut fixed = fixed.into_iter().peekable();
    for (n, &from) in perm.iter().enumerate() {
        let to = start + n as u32;
        while let Some(x) = fixed.next_if(|((r, c), _)| *r < to || (*r == to && *c < c0)) {
            out.push(x);
        }
        let d = i64::from(to) - i64::from(start + from);
        let from = from as usize;
        for (c, cell) in &mut moved[offsets[from]..offsets[from + 1]] {
            let mut cell = std::mem::take(cell);
            translate(&mut cell, d, 0, adj);
            out.push(((to, *c), cell));
        }
    }
    out.extend(fixed);
    // Keys are in order: the map is built in bulk.
    let mut rebuilt: BTreeMap<(u32, u32), Cell> = out.into_iter().collect();
    ws.cells.append(&mut rebuilt);
    ws.cells.append(&mut after);
}

/// Move the cells of columns `start..` inside `range`'s rows so that column
/// `start + n` receives the cells of column `start + perm[n]`.
fn move_columns(
    ws: &mut Worksheet,
    range: CellRange,
    start: u32,
    perm: &[u32],
    adj: &dyn FormulaAdjuster,
) {
    let mut lines: Vec<Vec<(u32, Cell)>> = (0..perm.len() as u32)
        .map(|i| {
            (range.first.row..=range.last.row)
                .filter_map(|r| ws.cells.remove(&(r, start + i)).map(|cell| (r, cell)))
                .collect()
        })
        .collect();
    for (n, &from) in perm.iter().enumerate() {
        let to = start + n as u32;
        let d = i64::from(to) - i64::from(start + from);
        for (r, mut cell) in std::mem::take(&mut lines[from as usize]) {
            translate(&mut cell, 0, d, Some(adj));
            ws.cells.insert((r, to), cell);
        }
    }
}

/// Distinct conditional-format icons `(icon set, icon id)` shown in one column (row
/// when `by_columns`) of `range`, for the "sort / filter by icon" choices.
pub fn line_icons(
    wb: &Workbook,
    sheet: usize,
    range: CellRange,
    key: u32,
    by_columns: bool,
) -> Vec<(String, u32)> {
    let (rows, cols) = if by_columns {
        (key..key + 1, range.first.col..range.last.col + 1)
    } else {
        (range.first.row..range.last.row + 1, key..key + 1)
    };
    let mut found: Vec<((u32, u32), (String, u32))> =
        crate::cf::evaluate(wb, sheet, rows, cols, None)
            .into_iter()
            .filter_map(|(k, f)| f.icon.map(|i| (k, (i.set, i.index as u32))))
            .collect();
    found.sort_by_key(|(k, _)| *k);
    let mut out: Vec<(String, u32)> = Vec::new();
    for (_, icon) in found {
        if !out.contains(&icon) {
            out.push(icon);
        }
    }
    out
}

/// Distinct fill and font colours (`RRGGBB`) in one column (row when
/// `by_columns`) of `range`, for the "sort by colour" choices.
pub fn line_colors(
    wb: &Workbook,
    sheet: usize,
    range: CellRange,
    key: u32,
    by_columns: bool,
) -> (Vec<String>, Vec<String>) {
    let mut fills = Vec::new();
    let mut fonts = Vec::new();
    let Some(ws) = wb.sheets.get(sheet) else {
        return (fills, fonts);
    };
    let cells: Vec<&Cell> = if by_columns {
        (range.first.col..=range.last.col)
            .filter_map(|c| ws.cell(key, c))
            .collect()
    } else {
        (range.first.row..=range.last.row)
            .filter_map(|r| ws.cell(r, key))
            .collect()
    };
    for cell in cells.into_iter().take(100_000) {
        if let Some(c) = crate::color::cell_fill_rgb(wb, cell) {
            let h = crate::color::hex(c);
            if !fills.contains(&h) {
                fills.push(h);
            }
        }
        if !matches!(cell.value, CellValue::Empty) {
            let h = crate::color::hex(crate::color::cell_font_rgb(wb, cell).unwrap_or((0, 0, 0)));
            if !fonts.contains(&h) {
                fonts.push(h);
            }
        }
    }
    (fills, fonts)
}

/// Whether the first row of `range` looks like a row of column labels, the guess a
/// spreadsheet's Sort / AutoFilter makes for "my data has headers":
/// * every filled cell of the first row is text, and
/// * in some column the first value below it is not text (a number, date, logical),
///   or the first row is formatted differently (font or fill) from the second.
///
/// A block of text formatted alike, or with a number in its first row, has none.
pub fn guess_header(wb: &Workbook, sheet: usize, range: CellRange) -> bool {
    let Some(ws) = wb.sheets.get(sheet) else {
        return false;
    };
    if range.rows() < 2 {
        return false;
    }
    let (r0, c0, c1) = (
        range.first.row,
        range.first.col,
        range.last.col.min(range.first.col + 1000),
    );
    let is_text = |v: &CellValue| matches!(v, CellValue::Text(_) | CellValue::RichText(_));
    let has_value = |c: &&Cell| !matches!(c.value, CellValue::Empty);
    let looks = |c: Option<&Cell>| {
        c.and_then(|c| wb.styles.xf(c.style))
            .map_or((0, 0), |x| (x.font_id, x.fill_id))
    };
    let mut any = false;
    let mut typed_below = false;
    let mut styled = false;
    for col in c0..=c1 {
        let top = ws.cell(r0, col);
        let Some(head) = top.filter(has_value) else {
            continue;
        };
        if !is_text(&head.value) {
            return false;
        }
        any = true;
        let below =
            (r0 + 1..=range.last.row.min(r0 + 50)).find_map(|r| ws.cell(r, col).filter(has_value));
        if below.is_some_and(|b| !is_text(&b.value)) {
            typed_below = true;
        }
        if looks(top) != looks(ws.cell(r0 + 1, col)) {
            styled = true;
        }
    }
    any && (typed_below || styled)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bo_model::sheet::{CellError, CellRef};

    fn sheet(rows: &[(&str, CellValue)]) -> Worksheet {
        let mut ws = Worksheet::new("S");
        ws.set_a1("A1", CellValue::Text("Name".into()));
        ws.set_a1("B1", CellValue::Text("Score".into()));
        for (i, (name, v)) in rows.iter().enumerate() {
            let r = i as u32 + 1;
            ws.set_cell(r, 0, Cell::new(CellValue::Text((*name).into())));
            ws.set_cell(r, 1, Cell::new(v.clone()).with_style(r));
        }
        ws
    }

    fn col(ws: &Worksheet, c: u32) -> Vec<String> {
        (1..=ws.used_range().unwrap().last.row)
            .map(|r| match ws.cell(r, c).map(|c| &c.value) {
                Some(CellValue::Number(n)) => n.to_string(),
                Some(v) => v.as_text().unwrap_or_else(|| format!("{v:?}")),
                None => "-".into(),
            })
            .collect()
    }

    #[test]
    fn multi_level_with_header_and_blanks() {
        let mut ws = sheet(&[
            ("Bình", CellValue::Number(7.0)),
            ("an", CellValue::Number(9.0)),
            ("Chi", CellValue::Empty),
            ("An", CellValue::Number(7.0)),
            ("Dũng", CellValue::Text("x".into())),
        ]);
        let range = CellRange::parse("A1:B6").unwrap();
        sort_range(&mut ws, range, &[SortKey::desc(1), SortKey::asc(0)], true);
        assert_eq!(
            ws.cell_a1("A1").unwrap().value,
            CellValue::Text("Name".into())
        );
        // Descending: text ("x") ranks above numbers; blanks still last.
        assert_eq!(col(&ws, 0), ["Dũng", "an", "An", "Bình", "Chi"]);
        // Styles travel with the cells.
        assert_eq!(ws.cell(2, 1).unwrap().style, 2);
        sort_range(&mut ws, range, &[SortKey::asc(1), SortKey::asc(0)], true);
        assert_eq!(col(&ws, 0), ["An", "Bình", "an", "Dũng", "Chi"]);
    }

    #[test]
    fn type_order() {
        use CellValue::*;
        let mut v = [
            Error(CellError::NA),
            Bool(true),
            Text("b".into()),
            Number(2.0),
            Bool(false),
            Text("A".into()),
            Number(-1.0),
        ];
        v.sort_by(|a, b| compare_values(a, b, false));
        assert_eq!(
            v,
            [
                Number(-1.0),
                Number(2.0),
                Text("A".into()),
                Text("b".into()),
                Bool(false),
                Bool(true),
                Error(CellError::NA)
            ]
        );
        assert_eq!(
            compare_values(&Text("a".into()), &Text("A".into()), true),
            Ordering::Less
        );
    }

    #[test]
    fn left_to_right_and_by_colour() {
        use bo_model::sheet::{CellXf, Fill, Font, SheetColor};
        let mut wb = Workbook::new();
        let red = wb.styles.add_fill(Fill::solid("FF0000"));
        let red_xf = wb.styles.add_xf(CellXf {
            fill_id: red,
            ..CellXf::default()
        });
        let blue_font = wb.styles.add_font(Font {
            color: Some(SheetColor::rgb("0000FF")),
            ..Font::default()
        });
        let blue_xf = wb.styles.add_xf(CellXf {
            font_id: blue_font,
            ..CellXf::default()
        });
        let ws = &mut wb.sheets[0];
        // Header column A, then three columns sorted by row 2 (left to right).
        for (c, (name, v)) in [("Năm", 0.0), ("c", 3.0), ("a", 1.0), ("b", 2.0)]
            .iter()
            .enumerate()
        {
            ws.set_cell(0, c as u32, Cell::new(CellValue::Text((*name).into())));
            ws.set_cell(1, c as u32, Cell::new(CellValue::Number(*v)));
        }
        let range = CellRange::parse("A1:D2").unwrap();
        let by_row2 = SortKey::asc(1);
        sort_lines(&mut wb, 0, range, &[by_row2], true, true, &NoFormulas);
        let ws = &wb.sheets[0];
        let row0: Vec<String> = (0..4)
            .map(|c| {
                ws.cell(0, c)
                    .and_then(|x| x.value.as_text())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(row0, ["Năm", "a", "b", "c"]);
        // Colour sort: red-filled rows first, then blue text.
        let ws = &mut wb.sheets[0];
        ws.cells.clear();
        for (r, (v, style)) in [(1.0, 0), (2.0, red_xf), (3.0, blue_xf), (4.0, red_xf)]
            .iter()
            .enumerate()
        {
            ws.set_cell(
                r as u32,
                0,
                Cell::new(CellValue::Number(*v)).with_style(*style),
            );
        }
        let range = CellRange::parse("A1:A4").unwrap();
        let red_key = SortKey {
            by: SortBy::CellColor((255, 0, 0)),
            ..SortKey::asc(0)
        };
        let blue_key = SortKey {
            by: SortBy::FontColor((0, 0, 255)),
            ..SortKey::asc(0)
        };
        sort_lines(
            &mut wb,
            0,
            range,
            &[red_key.clone(), blue_key],
            false,
            false,
            &NoFormulas,
        );
        assert_eq!(col_vals(&wb.sheets[0]), [2.0, 4.0, 3.0, 1.0]);
        let (fills, fonts) = line_colors(&wb, 0, range, 0, false);
        assert_eq!(fills, ["FF0000"]);
        assert_eq!(fonts, ["000000", "0000FF"]);
        // Descending colour key: red at the bottom.
        let red_desc = SortKey {
            descending: true,
            ..red_key
        };
        sort_lines(&mut wb, 0, range, &[red_desc], false, false, &NoFormulas);
        assert_eq!(col_vals(&wb.sheets[0])[2..], [2.0, 4.0]);
    }

    fn col_vals(ws: &Worksheet) -> Vec<f64> {
        (0..4)
            .map(|r| match ws.cell(r, 0).map(|c| &c.value) {
                Some(CellValue::Number(n)) => *n,
                _ => f64::NAN,
            })
            .collect()
    }

    /// Icon set rule on `sqref`: below 10 → icon 0, 10–19 → icon 1, 20 and up → icon 2.
    pub(crate) fn arrows(wb: &mut Workbook, sqref: &str) {
        use bo_model::sheet::{CfRule, Cfvo, ConditionalFormat, IconSet};
        let num = |v: &str| Cfvo {
            kind: "num".into(),
            val: Some(v.into()),
            gte: None,
        };
        wb.sheets[0].conditional_formats.push(ConditionalFormat {
            sqref: vec![CellRange::parse(sqref).unwrap()],
            pivot: false,
            ext: false,
            rules: vec![CfRule {
                kind: "iconSet".into(),
                priority: 1,
                icon_set: Some(IconSet {
                    icon_set: Some("3Arrows".into()),
                    cfvos: vec![num("0"), num("10"), num("20")],
                    ..IconSet::default()
                }),
                ..CfRule::default()
            }],
        });
    }

    // source: ECMA-376 Part 1, §18.3.1.91 sortCondition (sortBy="icon", iconSet, iconId)
    #[test]
    fn by_conditional_format_icon() {
        let mut wb = Workbook::new();
        for (r, v) in [5.0, 25.0, 12.0, 30.0, 8.0, 15.0].iter().enumerate() {
            wb.sheets[0].set_cell(r as u32, 0, Cell::new(CellValue::Number(*v)));
        }
        arrows(&mut wb, "A1:A6");
        let range = CellRange::parse("A1:A6").unwrap();
        // Icons: 5→0, 25→2, 12→1, 30→2, 8→0, 15→1 (first seen order).
        assert_eq!(
            line_icons(&wb, 0, range, 0, false),
            [
                ("3Arrows".to_owned(), 0),
                ("3Arrows".to_owned(), 2),
                ("3Arrows".to_owned(), 1)
            ]
        );
        let icon = |id: u32| SortKey {
            by: SortBy::Icon {
                set: "3Arrows".into(),
                id,
            },
            ..SortKey::asc(0)
        };
        let vals = |wb: &Workbook| -> Vec<f64> {
            (0..6)
                .map(|r| match wb.sheets[0].cell(r, 0).map(|c| &c.value) {
                    Some(CellValue::Number(n)) => *n,
                    _ => f64::NAN,
                })
                .collect()
        };
        let mut one = wb.clone();
        sort_lines(&mut one, 0, range, &[icon(2)], false, false, &NoFormulas);
        // Up-arrow cells on top, the rest keep their order (stable).
        assert_eq!(vals(&one), [25.0, 30.0, 5.0, 12.0, 8.0, 15.0]);
        sort_lines(
            &mut wb,
            0,
            range,
            &[icon(2), icon(1)],
            false,
            false,
            &NoFormulas,
        );
        assert_eq!(vals(&wb), [25.0, 30.0, 12.0, 15.0, 5.0, 8.0]);
    }

    // source: ECMA-376 Part 1, §18.3.1.91 sortCondition/@customList
    #[test]
    fn by_custom_list() {
        let words = ["Cao", "Thấp", "Khác", "Vừa", "thấp", "", "A"];
        let mk = || {
            let mut ws = Worksheet::new("S");
            for (r, w) in words.iter().enumerate() {
                if !w.is_empty() {
                    ws.set_cell(r as u32, 0, Cell::new(CellValue::Text((*w).into())));
                }
            }
            ws
        };
        let list = split_custom_list("Thấp, Vừa,Cao");
        assert_eq!(list, ["Thấp", "Vừa", "Cao"]);
        let range = CellRange::parse("A1:A7").unwrap();
        let read = |ws: &Worksheet| -> Vec<String> {
            (0..7)
                .map(|r| {
                    ws.cell(r, 0)
                        .and_then(|c| c.value.as_text())
                        .unwrap_or_default()
                })
                .collect()
        };
        let mut ws = mk();
        let key = SortKey {
            custom_list: Some(list.clone()),
            ..SortKey::asc(0)
        };
        sort_range(&mut ws, range, std::slice::from_ref(&key), false);
        // List order (case-insensitive), then other values ascending, blanks last.
        assert_eq!(read(&ws), ["Thấp", "thấp", "Vừa", "Cao", "A", "Khác", ""]);
        let mut ws = mk();
        let desc = SortKey {
            descending: true,
            ..key
        };
        sort_range(&mut ws, range, &[desc], false);
        assert_eq!(read(&ws), ["Cao", "Vừa", "Thấp", "thấp", "Khác", "A", ""]);
    }

    // ------------------------------------------------- prepared keys vs. naive sort

    /// The comparison of the original implementation: values compared cell by cell
    /// with [`compare_values`] (collation keys built on every comparison).
    fn naive_key_cmp(a: Option<&CellValue>, b: Option<&CellValue>, k: &SortKey) -> Ordering {
        match (is_blank(a), is_blank(b)) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                let (Some(a), Some(b)) = (a, b) else {
                    return Ordering::Equal;
                };
                if let Some(list) = k.custom_list.as_deref() {
                    let pos = |v: &CellValue| {
                        let t = v.as_text()?.to_lowercase();
                        list.iter().position(|x| x.to_lowercase() == t)
                    };
                    match (pos(a), pos(b)) {
                        (Some(x), Some(y)) => {
                            return if k.descending { y.cmp(&x) } else { x.cmp(&y) };
                        }
                        (Some(_), None) => return Ordering::Less,
                        (None, Some(_)) => return Ordering::Greater,
                        (None, None) => {}
                    }
                }
                let o = compare_values(a, b, k.case_sensitive);
                if k.descending {
                    o.reverse()
                } else {
                    o
                }
            }
        }
    }

    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n
        }
    }

    fn random_value(g: &mut Lcg) -> Option<CellValue> {
        const WORDS: [&str; 10] = [
            "an", "An", "ân", "Bình", "binh", "Đức", "duc", "Ánh", "x", "",
        ];
        match g.next(9) {
            0 => None,
            1 | 2 => Some(CellValue::Number(g.next(20) as f64 - 5.0)),
            3 | 4 => Some(CellValue::Text(WORDS[g.next(10) as usize].into())),
            5 => Some(CellValue::Bool(g.next(2) == 1)),
            6 => Some(CellValue::Error(CellError::NA)),
            7 => Some(CellValue::RichText(vec![bo_model::sheet::TextRun {
                text: WORDS[g.next(10) as usize].into(),
                font: None,
            }])),
            _ => Some(CellValue::Date(format!("2024-0{}-01", 1 + g.next(9)))),
        }
    }

    /// Prepared keys + permutation give exactly the stable order of the cell-by-cell
    /// comparison, for every value type, both directions, case sensitivity and
    /// custom lists; the id column (not a key) shows that equal lines keep their order.
    #[test]
    fn prepared_sort_matches_naive_stable_sort() {
        let mut g = Lcg(7);
        let n = 3000u32;
        let mut ws = Worksheet::new("S");
        for r in 0..n {
            for c in 0..3 {
                if let Some(v) = random_value(&mut g) {
                    ws.set_cell(r, c, Cell::new(v));
                }
            }
            ws.set_cell(r, 3, Cell::new(CellValue::Number(f64::from(r))));
        }
        let list = Some(vec!["Bình".to_owned(), "x".to_owned(), "an".to_owned()]);
        let levels: Vec<Vec<SortKey>> = vec![
            vec![SortKey::asc(0)],
            vec![SortKey::desc(1), SortKey::asc(0)],
            vec![
                SortKey {
                    case_sensitive: true,
                    ..SortKey::asc(0)
                },
                SortKey::desc(2),
            ],
            vec![
                SortKey {
                    custom_list: list.clone(),
                    ..SortKey::asc(1)
                },
                SortKey::asc(2),
            ],
            vec![
                SortKey {
                    custom_list: list,
                    ..SortKey::desc(0)
                },
                SortKey {
                    case_sensitive: true,
                    ..SortKey::desc(1)
                },
            ],
        ];
        let range = CellRange::new(CellRef::new(0, 0), CellRef::new(n - 1, 3));
        for keys in levels {
            let mut lines: Vec<Vec<Option<CellValue>>> = (0..n)
                .map(|r| {
                    (0..4)
                        .map(|c| ws.cell(r, c).map(|x| x.value.clone()))
                        .collect()
                })
                .collect();
            lines.sort_by(|a, b| {
                keys.iter()
                    .map(|k| {
                        naive_key_cmp(a[k.col as usize].as_ref(), b[k.col as usize].as_ref(), k)
                    })
                    .find(|o| *o != Ordering::Equal)
                    .unwrap_or(Ordering::Equal)
            });
            let mut sorted = ws.clone();
            sort_range(&mut sorted, range, &keys, false);
            for (r, line) in lines.iter().enumerate() {
                for (c, v) in line.iter().enumerate() {
                    assert_eq!(
                        sorted.cell(r as u32, c as u32).map(|x| &x.value),
                        v.as_ref(),
                        "row {r} col {c} keys {keys:?}"
                    );
                }
            }
        }
    }

    /// Cells left and right of the range and rows above / below it stay; moved
    /// formulas follow their row; a range ending on the last row of the sheet works.
    #[test]
    fn rows_move_inside_the_range_only() {
        use bo_model::sheet::MAX_ROWS;
        let last = MAX_ROWS - 1;
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        ws.set_a1("A1", CellValue::Text("above".into()));
        for (i, v) in [3.0, 1.0, 2.0].iter().enumerate() {
            let r = last - 2 + i as u32;
            ws.set_cell(r, 0, Cell::new(CellValue::Text(format!("left{i}"))));
            ws.set_cell(r, 1, Cell::new(CellValue::Number(*v)));
            ws.set_cell(
                r,
                2,
                Cell::new(CellValue::Number(v * 10.0)).with_formula(&format!("B{}*10", r + 1)),
            );
            ws.set_cell(r, 3, Cell::new(CellValue::Text(format!("right{i}"))));
        }
        let range = CellRange::new(CellRef::new(last - 2, 1), CellRef::new(last, 2));
        sort_lines(
            &mut wb,
            0,
            range,
            &[SortKey::asc(1)],
            false,
            false,
            &crate::formula::BoFormula::default(),
        );
        let ws = &wb.sheets[0];
        let num = |r: u32, c: u32| match ws.cell(r, c).map(|x| &x.value) {
            Some(CellValue::Number(n)) => *n,
            _ => f64::NAN,
        };
        assert_eq!(
            [num(last - 2, 1), num(last - 1, 1), num(last, 1)],
            [1.0, 2.0, 3.0]
        );
        let f = |r: u32| {
            ws.cell(r, 2)
                .and_then(|x| x.formula.as_ref())
                .map(|f| f.text.clone())
        };
        assert_eq!(f(last - 2), Some(format!("B{}*10", last - 1)));
        assert_eq!(f(last), Some(format!("B{}*10", last + 1)));
        for (i, r) in (last - 2..=last).enumerate() {
            assert_eq!(
                ws.cell(r, 0).and_then(|x| x.value.as_text()),
                Some(format!("left{i}"))
            );
            assert_eq!(
                ws.cell(r, 3).and_then(|x| x.value.as_text()),
                Some(format!("right{i}"))
            );
        }
        assert_eq!(
            ws.cell(0, 0).and_then(|x| x.value.as_text()),
            Some("above".into())
        );
        assert_eq!(ws.cells.len(), 13);
    }
}

#[cfg(test)]
mod sparse_tests {
    use super::*;
    use bo_model::sheet::CellRef;

    /// Rows with few cells (the tree is rebuilt) and rows with some gaps (cells
    /// permuted in place around marked empty cells) give the same order.
    #[test]
    fn sparse_and_dense_rows_sort_alike() {
        for every in [1u32, 3, 200] {
            let mut ws = Worksheet::new("S");
            let n = 12_000u32;
            for r in 0..n {
                let key = f64::from((r * 7919) % 1000);
                ws.set_cell(r, 1, Cell::new(CellValue::Number(key)));
                if r % every == 0 {
                    ws.set_cell(r, 4, Cell::new(CellValue::Number(f64::from(r))));
                }
                ws.set_cell(r, 6, Cell::new(CellValue::Text(format!("out{r}"))));
            }
            let before = ws.clone();
            let range = CellRange::new(CellRef::new(0, 0), CellRef::new(n - 1, 5));
            sort_range(&mut ws, range, &[SortKey::asc(1)], false);
            let mut want: Vec<u32> = (0..n).collect();
            want.sort_by_key(|r| (r * 7919) % 1000);
            for (to, from) in want.iter().enumerate() {
                let to = to as u32;
                for c in [1, 4] {
                    assert_eq!(
                        ws.cell(to, c).map(|x| &x.value),
                        before.cell(*from, c).map(|x| &x.value),
                        "every {every} row {to} col {c}"
                    );
                }
                assert_eq!(ws.cell(to, 6), before.cell(to, 6));
            }
            assert_eq!(ws.cells.len(), before.cells.len());
            assert!(ws.cells.values().all(|c| c.style != PLACEHOLDER_STYLE));
        }
    }
}

#[cfg(test)]
mod header_tests {
    use super::guess_header;
    use bo_model::sheet::{Cell, CellRange, CellValue, CellXf, Font, Workbook};

    fn book(rows: &[&[&str]]) -> Workbook {
        let mut wb = Workbook::new();
        let ws = &mut wb.sheets[0];
        for (r, row) in rows.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let value = v
                    .parse::<f64>()
                    .map_or_else(|_| CellValue::Text((*v).into()), CellValue::Number);
                ws.set_cell(r as u32, c as u32, Cell::new(value));
            }
        }
        wb
    }

    #[test]
    fn labels_over_numbers_are_a_header() {
        let r = CellRange::parse("A1:B3").unwrap();
        assert!(guess_header(
            &book(&[&["Tên", "Điểm"], &["Lan", "7"], &["An", "9"]]),
            0,
            r
        ));
        // Numbers only: no header.
        assert!(!guess_header(
            &book(&[&["3", "1"], &["1", "2"], &["2", "3"]]),
            0,
            r
        ));
        // A number in the first row: no header.
        assert!(!guess_header(
            &book(&[&["Tên", "1"], &["Lan", "7"], &["An", "9"]]),
            0,
            r
        ));
        // Text over text, formatted alike: no header.
        assert!(!guess_header(
            &book(&[&["Tên", "Lớp"], &["Lan", "A"], &["An", "B"]]),
            0,
            r
        ));
    }

    #[test]
    fn a_bold_first_row_of_text_is_a_header() {
        let mut wb = book(&[&["Tên", "Lớp"], &["Lan", "A"], &["An", "B"]]);
        let font = wb.styles.add_font(Font {
            bold: true,
            ..Font::default()
        });
        let style = wb.styles.add_xf(CellXf {
            font_id: font,
            ..CellXf::default()
        });
        for c in 0..2 {
            if let Some(cell) = wb.sheets[0].cells.get_mut(&(0, c)) {
                cell.style = style;
            }
        }
        assert!(guess_header(&wb, 0, CellRange::parse("A1:B3").unwrap()));
    }
}
