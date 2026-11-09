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

pub(crate) type Attrs = Vec<(String, String)>;

/// Attributes of the first element of `xml` (`<x a="1" b='2'/>`), unescaped.
pub(crate) fn parse_attrs(xml: &str) -> Attrs {
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

pub(crate) fn local(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, l)| l)
}

pub(crate) fn element_name(xml: &str) -> &str {
    let t = xml.trim_start().trim_start_matches('<');
    let end = t
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(t.len());
    local(&t[..end])
}

pub(crate) fn get<'a>(attrs: &'a Attrs, name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| local(k) == name)
        .map(|(_, v)| v.as_str())
}

pub(crate) fn flag(attrs: &Attrs, name: &str, default: bool) -> bool {
    get(attrs, name).map_or(default, |v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub(crate) fn find(unknown: &[RawXml], name: &str) -> Option<usize> {
    unknown.iter().position(|x| element_name(&x.xml) == name)
}

pub(crate) fn to_xml(name: &str, attrs: &Attrs) -> String {
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
pub(crate) fn verify(attrs: &Attrs, legacy: &str, prefix: &str, password: &str) -> bool {
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
pub(crate) fn hash_attrs(prefix: &str, password: &str) -> Result<Attrs> {
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
/// is locked and outside the open protected ranges ([`range_open`]; `open` =
/// names unlocked with their password in this session). Large ranges are
/// checked per stored cell plus row/column styles to stay fast.
pub fn check_cells(
    wb: &Workbook,
    ws: &Worksheet,
    open: &[String],
    cells: impl IntoIterator<Item = (u32, u32)>,
) -> Result<()> {
    if !sheet_info(ws).protected {
        return Ok(());
    }
    let ranges = protected_ranges(ws);
    for (r, c) in cells {
        if cell_locked(wb, ws, r, c) && !in_open_range(&ranges, open, r, c) {
            return Err(EngineError::Sheet(SHEET_PROTECTED.into()));
        }
    }
    Ok(())
}

/// [`check_cells`] for a rectangle (`rows` × `cols`, inclusive).
pub fn check_range(
    wb: &Workbook,
    ws: &Worksheet,
    open: &[String],
    (r0, c0): (u32, u32),
    (r1, c1): (u32, u32),
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
        open,
        (r0..=r1).flat_map(move |r| (c0..=c1).map(move |c| (r, c))),
    )
}

// ------------------------------------------------------------ protected ranges

/// One `protectedRange` (ECMA-376 Part 1, §18.3.1.71): cells users may edit
/// on the protected sheet, with or without a password of their own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedRange {
    /// `name` (unique on the sheet).
    pub name: String,
    /// `sqref`: the cells, e.g. `B2:C9 E4`.
    pub sqref: String,
    /// A password is needed before the range can be edited.
    pub has_password: bool,
    /// The range carries Windows access rules (`securityDescriptor`); they are
    /// kept in the file but not evaluated.
    pub has_security_descriptor: bool,
    /// Attributes of the element (the password hash, for verification).
    #[serde(skip)]
    pub attrs: Attrs,
}

/// Start and end byte offsets of every `protectedRange` element in `xml`.
fn range_spans(xml: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = xml[i..].find('<') {
        let start = i + k;
        let rest = &xml[start + 1..];
        let end_name = rest
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(rest.len());
        let qname = &rest[..end_name];
        if local(qname) != "protectedRange" {
            i = start + 1;
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag_end = start + 1 + gt + 1;
        let end = if rest[..gt].ends_with('/') {
            tag_end
        } else {
            let close = format!("</{qname}>");
            xml[tag_end..]
                .find(&close)
                .map_or(xml.len(), |c| tag_end + c + close.len())
        };
        out.push((start, end));
        i = end;
    }
    out
}

/// Protected ranges of a sheet (`protectedRanges`, §18.3.1.72), in file order.
pub fn protected_ranges(ws: &Worksheet) -> Vec<ProtectedRange> {
    let Some(i) = find(&ws.unknown, "protectedRanges") else {
        return Vec::new();
    };
    let xml = &ws.unknown[i].xml;
    range_spans(xml)
        .into_iter()
        .map(|(a, b)| {
            let el = &xml[a..b];
            let attrs = parse_attrs(el);
            ProtectedRange {
                name: get(&attrs, "name").unwrap_or_default().to_owned(),
                sqref: get(&attrs, "sqref").unwrap_or_default().to_owned(),
                has_password: get(&attrs, "password").is_some()
                    || get(&attrs, "hashValue").is_some(),
                has_security_descriptor: get(&attrs, "securityDescriptor").is_some()
                    || el.contains("securityDescriptor>"),
                attrs,
            }
        })
        .collect()
}

/// Whether cell (`r`, `c`) lies in an open protected range: one without a
/// password, or one named in `open`.
pub(crate) fn in_open_range(ranges: &[ProtectedRange], open: &[String], r: u32, c: u32) -> bool {
    let at = bo_model::sheet::CellRef::new(r, c);
    ranges.iter().any(|p| {
        (!p.has_password || open.iter().any(|n| n.eq_ignore_ascii_case(&p.name)))
            && bo_model::sheet::CellRange::parse_sqref(&p.sqref)
                .iter()
                .any(|q| q.contains(at))
    })
}

/// [`in_open_range`] reading the sheet's ranges.
pub fn range_open(ws: &Worksheet, open: &[String], r: u32, c: u32) -> bool {
    in_open_range(&protected_ranges(ws), open, r, c)
}

/// Adds (or replaces, by name) a protected range; `password` empty = none.
/// Other ranges keep their exact XML (security descriptors included).
pub fn add_protected_range(
    ws: &mut Worksheet,
    name: &str,
    sqref: &str,
    password: &str,
) -> Result<()> {
    let name = name.trim();
    let refs = bo_model::sheet::CellRange::parse_sqref(sqref);
    if name.is_empty() || refs.is_empty() {
        return Err(EngineError::BadCommand(
            "protected range: name and cells".into(),
        ));
    }
    let sq = refs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    let mut a = hash_attrs("", password)?;
    a.push(("sqref".into(), sq));
    a.push(("name".into(), name.to_owned()));
    let new_el = to_xml("protectedRange", &a);
    let xml = find(&ws.unknown, "protectedRanges")
        .map(|i| ws.unknown[i].xml.clone())
        .unwrap_or_default();
    let mut kept = String::new();
    for (a0, b0) in range_spans(&xml) {
        let el = &xml[a0..b0];
        if get(&parse_attrs(el), "name").is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            continue;
        }
        kept.push_str(el);
    }
    kept.push_str(&new_el);
    set_ranges_xml(ws, &xml, &kept);
    Ok(())
}

/// Writes the `protectedRanges` wrapper around `inner` (keeping the source
/// wrapper's start tag), or removes it when `inner` is empty.
fn set_ranges_xml(ws: &mut Worksheet, old: &str, inner: &str) {
    let slot = find(&ws.unknown, "protectedRanges");
    if inner.is_empty() {
        if let Some(i) = slot {
            ws.unknown.remove(i);
        }
        return;
    }
    let open = old
        .find('>')
        .map(|g| &old[..=g])
        .filter(|t| !t.ends_with("/>"))
        .unwrap_or("<protectedRanges>");
    let t = open.trim_start().trim_start_matches('<');
    let qname = &t[..t
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(t.len())];
    let raw = RawXml {
        after: "protectedRanges".into(),
        xml: format!("{open}{inner}</{qname}>"),
    };
    match slot {
        Some(i) => ws.unknown[i] = raw,
        None => ws.unknown.push(raw),
    }
}

/// Removes a protected range by name; `false` when there is none.
pub fn remove_protected_range(ws: &mut Worksheet, name: &str) -> bool {
    let Some(i) = find(&ws.unknown, "protectedRanges") else {
        return false;
    };
    let xml = ws.unknown[i].xml.clone();
    let mut kept = String::new();
    let mut found = false;
    for (a, b) in range_spans(&xml) {
        let el = &xml[a..b];
        if get(&parse_attrs(el), "name").is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            found = true;
        } else {
            kept.push_str(el);
        }
    }
    if found {
        set_ranges_xml(ws, &xml, &kept);
    }
    found
}

/// Checks the password of a protected range (`true` when it has none).
pub fn verify_range_password(ws: &Worksheet, name: &str, password: &str) -> Result<bool> {
    let r = protected_ranges(ws)
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| EngineError::BadCommand(format!("no protected range {name}")))?;
    Ok(verify(&r.attrs, "password", "", password))
}

// ------------------------------------------------------------ file sharing

/// Write protection of the workbook (`fileSharing`, §18.2.12).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSharingInfo {
    /// `readOnlyRecommended`: suggest opening read-only.
    pub read_only_recommended: bool,
    /// A password to modify is set (`reservationPassword` or `hashValue`).
    pub has_password: bool,
    /// `userName` of the person who set it.
    pub user_name: Option<String>,
}

/// `fileSharing` of a workbook.
pub fn file_sharing_info(wb: &Workbook) -> FileSharingInfo {
    let Some(i) = find(&wb.unknown, "fileSharing") else {
        return FileSharingInfo::default();
    };
    let a = parse_attrs(&wb.unknown[i].xml);
    FileSharingInfo {
        read_only_recommended: flag(&a, "readOnlyRecommended", false),
        has_password: get(&a, "reservationPassword").is_some() || get(&a, "hashValue").is_some(),
        user_name: get(&a, "userName").map(str::to_owned),
    }
}

/// Checks the password to modify (`true` when none is set).
pub fn verify_file_sharing(wb: &Workbook, password: &str) -> bool {
    find(&wb.unknown, "fileSharing").is_none_or(|i| {
        verify(
            &parse_attrs(&wb.unknown[i].xml),
            "reservationPassword",
            "",
            password,
        )
    })
}

/// Sets `readOnlyRecommended` and/or the password to modify (`Some("")`
/// removes it); the element is dropped when neither remains.
pub fn set_file_sharing(
    wb: &mut Workbook,
    recommended: Option<bool>,
    password: Option<&str>,
    user: Option<&str>,
) -> Result<()> {
    let slot = find(&wb.unknown, "fileSharing");
    let mut a = slot
        .map(|i| parse_attrs(&wb.unknown[i].xml))
        .unwrap_or_default();
    let hash_keys = [
        "reservationPassword",
        "algorithmName",
        "hashValue",
        "saltValue",
        "spinCount",
    ];
    if let Some(pw) = password {
        a.retain(|(k, _)| !hash_keys.contains(&local(k)) && local(k) != "userName");
        if !pw.is_empty() {
            if let Some(u) = user.filter(|u| !u.trim().is_empty()) {
                a.push(("userName".into(), u.trim().to_owned()));
            }
            a.extend(hash_attrs("", pw)?);
        }
    }
    if let Some(on) = recommended {
        a.retain(|(k, _)| local(k) != "readOnlyRecommended");
        if on {
            a.insert(0, ("readOnlyRecommended".into(), "1".into()));
        }
    }
    let keep = a
        .iter()
        .any(|(k, _)| local(k) == "readOnlyRecommended" || hash_keys.contains(&local(k)));
    match (slot, keep) {
        (Some(i), false) => {
            wb.unknown.remove(i);
        }
        (Some(i), true) => wb.unknown[i].xml = to_xml("fileSharing", &a),
        (None, true) => wb.unknown.push(RawXml {
            after: "fileSharing".into(),
            xml: to_xml("fileSharing", &a),
        }),
        (None, false) => {}
    }
    Ok(())
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
    fn protected_ranges_open_locked_cells() {
        let wb = Workbook::new();
        let mut ws = Worksheet::default();
        ws.unknown.push(RawXml {
            after: "protectedRanges".into(),
            xml: r#"<protectedRanges><protectedRange sqref="B2:C3" name="Nhap" securityDescriptor="O:WDG:WDD:(A;;CC;;;WD)"/></protectedRanges>"#.into(),
        });
        protect_sheet(&mut ws, "", &SheetPermissions::default()).unwrap();
        // B2 (row 1, col 1) and C3 are in the open range; A1 is not.
        assert!(check_cells(&wb, &ws, &[], [(1, 1), (2, 2)]).is_ok());
        assert!(check_cells(&wb, &ws, &[], [(0, 0)]).is_err());
        let r = protected_ranges(&ws);
        assert_eq!(r.len(), 1);
        assert!(r[0].has_security_descriptor && !r[0].has_password);
        // A range with a password stays closed until unlocked.
        add_protected_range(&mut ws, "Kho", "E5:E9", "kho").unwrap();
        assert!(check_cells(&wb, &ws, &[], [(4, 4)]).is_err());
        assert!(!verify_range_password(&ws, "kho", "sai").unwrap());
        assert!(verify_range_password(&ws, "Kho", "kho").unwrap());
        assert!(check_cells(&wb, &ws, &["Kho".into()], [(4, 4)]).is_ok());
        // The first range keeps its exact XML.
        let x = &ws.unknown[find(&ws.unknown, "protectedRanges").unwrap()].xml;
        assert!(x.contains(
            r#"<protectedRange sqref="B2:C3" name="Nhap" securityDescriptor="O:WDG:WDD:(A;;CC;;;WD)"/>"#
        ));
        assert!(remove_protected_range(&mut ws, "Nhap"));
        assert!(remove_protected_range(&mut ws, "Kho"));
        assert!(find(&ws.unknown, "protectedRanges").is_none());
    }

    #[test]
    fn file_sharing_recommendation_and_password() {
        let mut wb = Workbook::new();
        set_file_sharing(&mut wb, Some(true), None, None).unwrap();
        assert_eq!(
            wb.unknown[0].xml,
            r#"<fileSharing readOnlyRecommended="1"/>"#
        );
        set_file_sharing(&mut wb, None, Some("sửa"), Some("An")).unwrap();
        let i = file_sharing_info(&wb);
        assert!(i.read_only_recommended && i.has_password);
        assert_eq!(i.user_name.as_deref(), Some("An"));
        assert!(verify_file_sharing(&wb, "sửa") && !verify_file_sharing(&wb, "x"));
        set_file_sharing(&mut wb, Some(false), Some(""), None).unwrap();
        assert_eq!(wb.unknown, Vec::<RawXml>::new());
        // Legacy 16-bit reservation password ("test" → CBEB).
        wb.unknown.push(RawXml {
            after: "fileSharing".into(),
            xml: r#"<fileSharing reservationPassword="CBEB"/>"#.into(),
        });
        assert!(verify_file_sharing(&wb, "test"));
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
