//! Tables (ECMA-376 Part 1, §18.5): creating a table over a range.

use bo_model::sheet::{CellRange, Table, TableColumn, TableStyleInfo, Workbook};

use crate::numfmt::Locale;
use crate::render::format_cell;

/// Turn `range` of sheet `sheet` into a table whose first row holds the column names (blank
/// or repeated headers get `Cột n`). Returns the table's index on the sheet.
///
/// # Errors
/// A range of one row, or one that overlaps another table.
pub fn create_table(
    wb: &mut Workbook,
    sheet: usize,
    range: CellRange,
    locale: &Locale,
) -> Result<usize, String> {
    if range.last.row <= range.first.row {
        return Err("a table needs a header row and data".into());
    }
    let ws = wb.sheets.get(sheet).ok_or("no such sheet")?;
    let overlaps = |r: &CellRange| {
        r.first.row <= range.last.row
            && range.first.row <= r.last.row
            && r.first.col <= range.last.col
            && range.first.col <= r.last.col
    };
    if ws.tables.iter().any(|t| overlaps(&t.range)) {
        return Err("the range overlaps a table".into());
    }
    // Column names from the header row, unique (§18.5.1.3 tableColumn/@name).
    let mut names: Vec<String> = Vec::new();
    for (k, col) in (range.first.col..=range.last.col).enumerate() {
        let text = ws
            .cells
            .get(&(range.first.row, col))
            .map(|c| format_cell(wb, c, locale).text)
            .unwrap_or_default();
        let base = if text.trim().is_empty() {
            format!("Cột {}", k + 1)
        } else {
            text.trim().to_owned()
        };
        let mut name = base.clone();
        let mut n = 2;
        while names.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            name = format!("{base}{n}");
            n += 1;
        }
        names.push(name);
    }
    // Table ids and names are unique in the workbook.
    let id = wb
        .sheets
        .iter()
        .flat_map(|s| &s.tables)
        .map(|t| t.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut n = id;
    let taken = |name: &str| {
        wb.sheets
            .iter()
            .flat_map(|s| &s.tables)
            .any(|t| t.name.eq_ignore_ascii_case(name))
            || wb
                .defined_names
                .iter()
                .any(|d| d.name.eq_ignore_ascii_case(name))
    };
    let mut name = format!("Bảng{n}");
    while taken(&name) {
        n += 1;
        name = format!("Bảng{n}");
    }
    let ws = &mut wb.sheets[sheet];
    ws.tables.push(Table {
        id,
        name: name.clone(),
        display_name: name,
        range,
        columns: names
            .into_iter()
            .enumerate()
            .map(|(k, name)| TableColumn {
                id: k as u32 + 1,
                name,
                ..TableColumn::default()
            })
            .collect(),
        style: Some(TableStyleInfo {
            name: Some("TableStyleMedium2".into()),
            show_row_stripes: true,
            ..TableStyleInfo::default()
        }),
        auto_filter: Some(format!("<autoFilter ref=\"{range}\"/>")),
        ..Table::default()
    });
    Ok(ws.tables.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{CellValue, Worksheet};

    #[test]
    fn creates_a_table_with_unique_names() {
        let mut ws = Worksheet::new("S");
        ws.set_a1("A1", CellValue::Text("Tên".into()));
        ws.set_a1("B1", CellValue::Text("Tên".into()));
        ws.set_a1("A2", CellValue::Number(1.0));
        let mut wb = Workbook::new();
        wb.sheets = vec![ws];
        let r = CellRange::parse("A1:C3").unwrap();
        let i = create_table(&mut wb, 0, r, &Locale::VI_VN).unwrap();
        let t = &wb.sheets[0].tables[i];
        assert_eq!(t.name, "Bảng1");
        let cols: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(cols, ["Tên", "Tên2", "Cột 3"]);
        assert!(create_table(
            &mut wb,
            0,
            CellRange::parse("B2:D4").unwrap(),
            &Locale::VI_VN
        )
        .is_err());
        let j = create_table(
            &mut wb,
            0,
            CellRange::parse("F1:G2").unwrap(),
            &Locale::VI_VN,
        )
        .unwrap();
        assert_eq!(wb.sheets[0].tables[j].id, 2);
        assert_eq!(wb.sheets[0].tables[j].name, "Bảng2");
    }
}
