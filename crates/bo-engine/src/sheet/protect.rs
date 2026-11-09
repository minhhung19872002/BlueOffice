//! Sheet and workbook protection for BlueSheet.
//!
//! * `sheetProtection` — ECMA-376 Part 1, §18.3.1.85 (CT_SheetProtection): while
//!   `sheet="1"`, cells whose format is `protection/@locked` (§18.8.33, default
//!   true) cannot be edited, and the actions whose attribute is true
//!   (`formatCells`, `insertRows`, `sort`, … — "true means the action is
//!   protected") are refused.
//! * `workbookProtection` — §18.2.29 (CT_WorkbookProtection): `lockStructure`
//!   forbids adding, deleting, renaming, moving and hiding sheets.
//! * Passwords: the legacy 16-bit `password` verifier ([MS-OFFCRYPTO] §2.3.7.1)
//!   and the iterated `algorithmName`/`hashValue`/`saltValue`/`spinCount` hash
//!   (§2.4.2.4) are both verified; new passwords use SHA-512 with a random salt
//!   and 100 000 iterations (`bo_crypto::protection`).
//!
//! Both elements stay in the models' unknown-XML lists (they are written back at
//! their schema slot, `bo-xlsx` `SHEET_SCHEMA` / `WORKBOOK_SCHEMA`), so files
//! round-trip unchanged until the user protects or unprotects.

use crate::{EngineError, Result};
use bo_model::sheet::{RawXml, Workbook, Worksheet};
use serde::{Deserialize, Serialize};

/// Error message of an edit refused by sheet protection (hosts match it).
pub const SHEET_PROTECTED: &str = "sheet protected";
/// Error message of a sheet operation refused by workbook structure protection.
pub const WORKBOOK_PROTECTED: &str = "workbook protected";
/// Error message of a wrong protection password.
pub const WRONG_PROTECTION_PASSWORD: &str = "wrong protection password";

type Attrs = Vec<(String, String)>;

/// Attributes of the first element of `xml` (`<x a="1" b='2'/>`), unescaped.
fn parse_attrs(xml: &str) -> Attrs {
    let mut out = Vec::new();
    let Some(start) = xml.find('<') else {
        return out;
    };
    let rest = &xml[start + 1..];
    let end = rest.find('>').unwrap_or(rest.len());
    let tag = rest[..end].trim_end_matches('/');
    let mut s = tag
        .find(char::is_whitespace)
        .map_or("", |i| &tag[i..])
        .trim_start();
    while let Some(eq) = s.find('=') {
        let name = s[..eq].trim().to_owned();
        let after = s[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let body = &after[1..];
        let Some(close) = body.find(q) else {
            break;
        };
        out.push((name, bo_opc::xml::unescape(&body[..close])));
        s = body[close + 1..].trim_start();
    }
    out
}

fn local(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, l)| l)
}

fn element_name(xml: &str) -> &str {
    let t = xml.trim_start().trim_start_matches('<');
    let end = t
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(t.len());
    local(&t[..end])
}

fn get<'a>(attrs: &'a Attrs, name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| local(k) == name)
        .map(|(_, v)| v.as_str())
}

fn flag(attrs: &Attrs, name: &str, default: bool) -> bool {
    get(attrs, name).map_or(default, |v| v == "1" || v.eq_ignore_ascii_case("true"))
}

fn find(unknown: &[RawXml], name: &str) -> Option<usize> {
    unknown.iter().position(|x| element_name(&x.xml) == name)
}

fn to_xml(name: &str, attrs: &Attrs) -> String {
    let mut s = format!("<{name}");
    for (k, v) in attrs {
        s.push_str(&format!(" {k}=\"{}\"", bo_opc::xml::escape(v)));
    }
    s.push_str("/>");
    s
}

/// Verifies `password` against the hash attributes (`prefix` is `""` for
/// `sheetProtection`, `workbook` for `workbookProtection`). No hash: any
/// password (or none) is accepted.
fn verify(attrs: &Attrs, legacy: &str, prefix: &str, password: &str) -> bool {
    let key = |n: &str| {
        if prefix.is_empty() {
            n.to_owned()
        } else {
            let mut c = n.chars();
            let first = c.next().map(|f| f.to_ascii_uppercase()).unwrap_or_default();
            format!("{prefix}{first}{}", c.as_str())
        }
    };
    if let (Some(alg), Some(hash), Some(salt)) = (
        get(attrs, &key("algorithmName")),
        get(attrs, &key("hashValue")),
        get(attrs, &key("saltValue")),
    ) {
        let spin = get(attrs, &key("spinCount"))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        return bo_crypto::protection::verify_protection(
            password,
            &bo_crypto::protection::ProtectionHash {
                algorithm_name: alg.to_owned(),
                hash_value: hash.to_owned(),
                salt_value: salt.to_owned(),
                spin_count: spin,
            },
        );
    }
    match get(attrs, legacy) {
        Some(hex) => u16::from_str_radix(hex.trim(), 16)
            .is_ok_and(|h| h == bo_crypto::protection::legacy_password_hash(password)),
        None => true,
    }
}

/// Hash attributes for a new password (empty: none).
fn hash_attrs(prefix: &str, password: &str) -> Result<Attrs> {
    if password.is_empty() {
        return Ok(Vec::new());
    }
    let h = bo_crypto::protection::new_protection_hash(password)
        .map_err(|e| EngineError::Crypto(e.to_string()))?;
    let k = |n: &str| {
        if prefix.is_empty() {
            let mut c = n.chars();
            let first = c.next().map(|f| f.to_ascii_lowercase()).unwrap_or_default();
            format!("{first}{}", c.as_str())
        } else {
            format!("{prefix}{n}")
        }
    };
    Ok(vec![
        (k("AlgorithmName"), h.algorithm_name),
        (k("HashValue"), h.hash_value),
        (k("SaltValue"), h.salt_value),
        (k("SpinCount"), h.spin_count.to_string()),
    ])
}

/// What a protected sheet still allows (the UI's check boxes). Field names
/// mirror the §18.3.1.85 attributes, inverted: `true` = allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetPermissions {
    /// Select locked cells (`selectLockedCells`).
    pub select_locked_cells: bool,
    /// Select unlocked cells (`selectUnlockedCells`).
    pub select_unlocked_cells: bool,
    /// Format cells (`formatCells`).
    pub format_cells: bool,
    /// Format columns: width, hide (`formatColumns`).
    pub format_columns: bool,
    /// Format rows: height, hide (`formatRows`).
    pub format_rows: bool,
    /// Insert columns (`insertColumns`).
    pub insert_columns: bool,
    /// Insert rows (`insertRows`).
    pub insert_rows: bool,
    /// Insert hyperlinks (`insertHyperlinks`).
    pub insert_hyperlinks: bool,
    /// Delete columns (`deleteColumns`).
    pub delete_columns: bool,
    /// Delete rows (`deleteRows`).
    pub delete_rows: bool,
    /// Sort (`sort`).
    pub sort: bool,
    /// Use AutoFilter (`autoFilter`).
    pub auto_filter: bool,
    /// Use PivotTable reports (`pivotTables`).
    pub pivot_tables: bool,
    /// Edit objects: charts, pictures, shapes (`objects`).
    pub objects: bool,
}

impl Default for SheetPermissions {
    /// The defaults of a newly protected sheet: selecting cells only.
    fn default() -> Self {
        SheetPermissions {
            select_locked_cells: true,
            select_unlocked_cells: true,
            format_cells: false,
            format_columns: false,
            format_rows: false,
            insert_columns: false,
            insert_rows: false,
            insert_hyperlinks: false,
            delete_columns: false,
            delete_rows: false,
            sort: false,
            auto_filter: false,
            pivot_tables: false,
            objects: false,
        }
    }
}

/// An action checked against sheet protection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Changing cell formats.
    FormatCells,
    /// Changing column widths / hiding columns.
    FormatColumns,
    /// Changing row heights / hiding rows.
    FormatRows,
    /// Inserting columns.
    InsertColumns,
    /// Inserting rows.
    InsertRows,
    /// Inserting hyperlinks.
    InsertHyperlinks,
    /// Deleting columns.
    DeleteColumns,
    /// Deleting rows.
    DeleteRows,
    /// Sorting a range.
    Sort,
    /// Using AutoFilter.
    AutoFilter,
    /// Changing pivot tables.
    PivotTables,
    /// Editing charts, pictures and shapes.
    Objects,
}

/// Protection state of one sheet (for the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetProtectionInfo {
    /// The sheet is protected (`sheetProtection/@sheet`).
    pub protected: bool,
    /// Unprotecting needs a password (a hash or legacy `password` is stored).
    pub has_password: bool,
    /// What the protection still permits.
    pub allow: SheetPermissions,
}

/// Protection state of a worksheet.
pub fn sheet_info(ws: &Worksheet) -> SheetProtectionInfo {
    let Some(i) = find(&ws.unknown, "sheetProtection") else {
        return SheetProtectionInfo {
            protected: false,
            has_password: false,
            allow: SheetPermissions::default(),
        };
    };
    let a = parse_attrs(&ws.unknown[i].xml);
    SheetProtectionInfo {
        protected: flag(&a, "sheet", false),
        has_password: get(&a, "password").is_some() || get(&a, "hashValue").is_some(),
        allow: SheetPermissions {
            select_locked_cells: !flag(&a, "selectLockedCells", false),
            select_unlocked_cells: !flag(&a, "selectUnlockedCells", false),
            format_cells: !flag(&a, "formatCells", true),
            format_columns: !flag(&a, "formatColumns", true),
            format_rows: !flag(&a, "formatRows", true),
            insert_columns: !flag(&a, "insertColumns", true),
            insert_rows: !flag(&a, "insertRows", true),
            insert_hyperlinks: !flag(&a, "insertHyperlinks", true),
            delete_columns: !flag(&a, "deleteColumns", true),
            delete_rows: !flag(&a, "deleteRows", true),
            sort: !flag(&a, "sort", true),
            auto_filter: !flag(&a, "autoFilter", true),
            pivot_tables: !flag(&a, "pivotTables", true),
            objects: !flag(&a, "objects", false),
        },
    }
}

/// Whether the sheet is protected with its scenarios (`sheetProtection/@scenarios`,
/// §18.3.1.85, default false): scenarios can then only be shown.
pub fn scenarios_protected(ws: &Worksheet) -> bool {
    find(&ws.unknown, "sheetProtection").is_some_and(|i| {
        let a = parse_attrs(&ws.unknown[i].xml);
        flag(&a, "sheet", false) && flag(&a, "scenarios", false)
    })
}

/// Whether `action` is refused on `ws`.
pub fn refuses(ws: &Worksheet, action: Action) -> bool {
    let info = sheet_info(ws);
    if !info.protected {
        return false;
    }
    let p = info.allow;
    !match action {
        Action::FormatCells => p.format_cells,
        Action::FormatColumns => p.format_columns,
        Action::FormatRows => p.format_rows,
        Action::InsertColumns => p.insert_columns,
        Action::InsertRows => p.insert_rows,
        Action::InsertHyperlinks => p.insert_hyperlinks,
        Action::DeleteColumns => p.delete_columns,
        Action::DeleteRows => p.delete_rows,
        Action::Sort => p.sort,
        Action::AutoFilter => p.auto_filter,
        Action::PivotTables => p.pivot_tables,
        Action::Objects => p.objects,
    }
}

/// `Err(sheet protected)` when `action` is refused.
pub fn check(ws: &Worksheet, action: Action) -> Result<()> {
    if refuses(ws, action) {
        Err(EngineError::Sheet(SHEET_PROTECTED.into()))
    } else {
        Ok(())
    }
}

/// Style index that formats (`row`, `col`): the cell's, else the row's
/// (`customFormat`), else the column's (§18.3.1.13 `col/@style`).
fn style_at(ws: &Worksheet, row: u32, col: u32) -> u32 {
    if let Some(c) = ws.cell(row, col) {
        return c.style;
    }
    if let Some(s) = ws.rows.get(&row).and_then(|r| r.style) {
        return s;
    }
    ws.cols
        .iter()
        .find(|c| c.min <= col && col <= c.max)
        .and_then(|c| c.style)
        .unwrap_or(0)
}

/// `protection/@locked` of the cell's format (§18.8.33, default true).
pub fn cell_locked(wb: &Workbook, ws: &Worksheet, row: u32, col: u32) -> bool {
    wb.styles
        .xf(style_at(ws, row, col))
        .and_then(|x| x.protection.as_ref())
        .and_then(|p| p.locked)
        .unwrap_or(true)
}

/// `Err(sheet protected)` when the sheet is protected and one of the cells
/// (`rows` × `cols`, inclusive) is locked. Large ranges are checked per
/// stored cell plus row/column styles to stay fast.
pub fn check_cells(
    wb: &Workbook,
    ws: &Worksheet,
    cells: impl IntoIterator<Item = (u32, u32)>,
) -> Result<()> {
    if !sheet_info(ws).protected {
        return Ok(());
    }
    for (r, c) in cells {
        if cell_locked(wb, ws, r, c) {
            return Err(EngineError::Sheet(SHEET_PROTECTED.into()));
        }
    }
    Ok(())
}

/// [`check_cells`] for a rectangle.
pub fn check_range(
    wb: &Workbook,
    ws: &Worksheet,
    r0: u32,
    c0: u32,
    r1: u32,
    c1: u32,
) -> Result<()> {
    if !sheet_info(ws).protected {
        return Ok(());
    }
    let (r0, r1) = (r0.min(r1), r0.max(r1));
    let (c0, c1) = (c0.min(c1), c0.max(c1));
    // Bounded scan; ranges beyond 1 M cells (whole columns) are refused when the
    // default format is locked, as they cover unformatted locked cells.
    let n = u64::from(r1 - r0 + 1) * u64::from(c1 - c0 + 1);
    if n > 1_000_000 {
        return if cell_locked(wb, &Worksheet::default(), 0, 0) {
            Err(EngineError::Sheet(SHEET_PROTECTED.into()))
        } else {
            Ok(())
        };
    }
    check_cells(
        wb,
        ws,
        (r0..=r1).flat_map(move |r| (c0..=c1).map(move |c| (r, c))),
    )
}

/// Protects a sheet (§18.3.1.85), replacing any previous protection.
pub fn protect_sheet(ws: &mut Worksheet, password: &str, allow: &SheetPermissions) -> Result<()> {
    let b = |v: bool| if v { "1" } else { "0" }.to_owned();
    let mut a = hash_attrs("", password)?;
    a.push(("sheet".into(), "1".into()));
    a.push(("objects".into(), b(!allow.objects)));
    a.push(("scenarios".into(), "1".into()));
    for (name, allowed, default_protected) in [
        ("formatCells", allow.format_cells, true),
        ("formatColumns", allow.format_columns, true),
        ("formatRows", allow.format_rows, true),
        ("insertColumns", allow.insert_columns, true),
        ("insertRows", allow.insert_rows, true),
        ("insertHyperlinks", allow.insert_hyperlinks, true),
        ("deleteColumns", allow.delete_columns, true),
        ("deleteRows", allow.delete_rows, true),
        ("selectLockedCells", allow.select_locked_cells, false),
        ("sort", allow.sort, true),
        ("autoFilter", allow.auto_filter, true),
        ("pivotTables", allow.pivot_tables, true),
        ("selectUnlockedCells", allow.select_unlocked_cells, false),
    ] {
        // Only non-default values are written.
        if allowed == default_protected {
            a.push((name.into(), b(!allowed)));
        }
    }
    let xml = RawXml {
        after: "sheetProtection".into(),
        xml: to_xml("sheetProtection", &a),
    };
    match find(&ws.unknown, "sheetProtection") {
        Some(i) => ws.unknown[i] = xml,
        None => ws.unknown.push(xml),
    }
    Ok(())
}

/// Removes sheet protection after checking `password`.
pub fn unprotect_sheet(ws: &mut Worksheet, password: &str) -> Result<()> {
    let Some(i) = find(&ws.unknown, "sheetProtection") else {
        return Ok(());
    };
    let a = parse_attrs(&ws.unknown[i].xml);
    if !verify(&a, "password", "", password) {
        return Err(EngineError::Sheet(WRONG_PROTECTION_PASSWORD.into()));
    }
    ws.unknown.remove(i);
    Ok(())
}

/// Workbook protection state (for the UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookProtectionInfo {
    /// Sheets cannot be added, deleted, renamed or moved (`lockStructure`).
    pub structure: bool,
    /// Window position and size are locked (`lockWindows`).
    pub windows: bool,
    /// Unprotecting needs a password (`workbookHashValue` or legacy `workbookPassword`).
    pub has_password: bool,
}

/// `workbookProtection` (§18.2.29) of a workbook.
pub fn workbook_info(wb: &Workbook) -> WorkbookProtectionInfo {
    let a = find(&wb.unknown, "workbookProtection")
        .map(|i| parse_attrs(&wb.unknown[i].xml))
        .unwrap_or_default();
    WorkbookProtectionInfo {
        structure: flag(&a, "lockStructure", false),
        windows: flag(&a, "lockWindows", false),
        has_password: get(&a, "workbookPassword").is_some()
            || get(&a, "workbookHashValue").is_some(),
    }
}

/// `Err(workbook protected)` when the structure is locked.
pub fn check_structure(wb: &Workbook) -> Result<()> {
    if workbook_info(wb).structure {
        Err(EngineError::Sheet(WORKBOOK_PROTECTED.into()))
    } else {
        Ok(())
    }
}

/// Locks the workbook structure (§18.2.29 `lockStructure`).
pub fn protect_workbook(wb: &mut Workbook, password: &str) -> Result<()> {
    let mut a = hash_attrs("workbook", password)?;
    a.push(("lockStructure".into(), "1".into()));
    let xml = RawXml {
        after: "workbookProtection".into(),
        xml: to_xml("workbookProtection", &a),
    };
    match find(&wb.unknown, "workbookProtection") {
        Some(i) => wb.unknown[i] = xml,
        None => wb.unknown.push(xml),
    }
    Ok(())
}

/// Removes workbook protection after checking `password`.
pub fn unprotect_workbook(wb: &mut Workbook, password: &str) -> Result<()> {
    let Some(i) = find(&wb.unknown, "workbookProtection") else {
        return Ok(());
    };
    let a = parse_attrs(&wb.unknown[i].xml);
    if !verify(&a, "workbookPassword", "workbook", password) {
        return Err(EngineError::Sheet(WRONG_PROTECTION_PASSWORD.into()));
    }
    wb.unknown.remove(i);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes_with_prefixes_and_entities() {
        let a = parse_attrs(
            r#"<x:sheetProtection password="CBEB" sheet='1' x:sort="0" a="&lt;b&gt;"/>"#,
        );
        assert_eq!(get(&a, "password"), Some("CBEB"));
        assert!(flag(&a, "sheet", false));
        assert!(!flag(&a, "sort", true));
        assert_eq!(get(&a, "a"), Some("<b>"));
        assert_eq!(element_name("<x:sheetProtection/>"), "sheetProtection");
    }

    #[test]
    fn legacy_and_iso_passwords_verify() {
        // "test" → CBEB ([MS-OFFCRYPTO] §2.3.7.1).
        let legacy = parse_attrs(r#"<sheetProtection password="CBEB" sheet="1"/>"#);
        assert!(verify(&legacy, "password", "", "test"));
        assert!(!verify(&legacy, "password", "", "Test"));
        let mut ws = Worksheet::default();
        protect_sheet(&mut ws, "mật khẩu", &SheetPermissions::default()).unwrap();
        let a = parse_attrs(&ws.unknown[0].xml);
        assert_eq!(get(&a, "algorithmName"), Some("SHA-512"));
        assert!(verify(&a, "password", "", "mật khẩu"));
        assert!(unprotect_sheet(&mut ws, "sai").is_err());
        unprotect_sheet(&mut ws, "mật khẩu").unwrap();
        assert_eq!(ws.unknown, Vec::<RawXml>::new());
        let mut wb = Workbook::new();
        protect_workbook(&mut wb, "wb").unwrap();
        assert!(workbook_info(&wb).structure);
        assert!(workbook_info(&wb).has_password);
        assert!(check_structure(&wb).is_err());
        assert!(unprotect_workbook(&mut wb, "x").is_err());
        unprotect_workbook(&mut wb, "wb").unwrap();
        assert!(check_structure(&wb).is_ok());
    }

    #[test]
    fn permissions_round_trip_through_attributes() {
        let mut ws = Worksheet::default();
        let allow = SheetPermissions {
            format_cells: true,
            sort: true,
            ..SheetPermissions::default()
        };
        protect_sheet(&mut ws, "", &allow).unwrap();
        let info = sheet_info(&ws);
        assert!(info.protected && !info.has_password);
        assert_eq!(info.allow, allow);
        assert!(!refuses(&ws, Action::FormatCells));
        assert!(refuses(&ws, Action::InsertRows));
        // No password: unprotecting needs none.
        unprotect_sheet(&mut ws, "").unwrap();
        assert!(!sheet_info(&ws).protected);
    }
}
