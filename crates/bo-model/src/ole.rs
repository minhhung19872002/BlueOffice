//! Embedded and linked objects (CORE-EMBED-001).
//!
//! WordprocessingML keeps an object in `w:object` (ECMA-376 Part 1, §17.3.3.19 object)
//! holding a VML picture shape (`v:shape` / `v:imagedata`, ECMA-376 Part 4) with the
//! preview image and an `o:OLEObject` (Office VML `CT_OLEObject`) that names the server
//! (`ProgID`), embeds a package part (`Type="Embed"`, relationship `r:id`) or links an
//! external file (`Type="Link"`). PresentationML uses `p:oleObj` (ISO/IEC 29500-1,
//! §19.3.2.4) with `p:embed` (§19.3.2.2) or `p:link` (§19.3.2.3) and a `p:pic` preview.
//!
//! The model only records what the editor needs: the preview is the picture's media
//! part, the object itself is never run or interpreted.

use serde::{Deserialize, Serialize};

/// ProgID of an embedded SpreadsheetML package (`.xlsx`): [MS-OI29500] §3.10
/// "Activating embedded packages in Office (Part 1 Section L.7.2 Embeddings)" maps the
/// `.xlsx` content type to this identifier. A file-format identifier required for
/// interoperability; never shown in the UI.
pub const PROG_ID_XLSX: &str = "Excel.Sheet.12";

/// Content type of an embedded SpreadsheetML package (ECMA-376 Part 1, §12.3).
pub const CT_XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// Content type of an embedded OLE object part (`oleObject*.bin`, ECMA-376 Part 1, §15.2.15).
pub const CT_OLE_BIN: &str = "application/vnd.openxmlformats-officedocument.oleObject";

/// Relationship type of an embedded package (ECMA-376 Part 1, §15.2.11).
pub const REL_PACKAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";

/// Relationship type of an embedded or linked OLE object (ECMA-376 Part 1, §15.2.15).
pub const REL_OLE_OBJECT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject";

/// A linked object's source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OleLink {
    /// Source file (path or URI of the external relationship target).
    pub source: String,
    /// Item inside the source (a range such as `Sheet1!R1C1:R3C4`), from the
    /// §17.16.5.32 LINK field or `p:link`; empty = the whole file.
    #[serde(default)]
    pub item: String,
    /// Update automatically (`UpdateMode="Always"` / `p:link@updateAutomatic`).
    #[serde(default)]
    pub auto_update: bool,
}

/// An embedded or linked object shown through its preview picture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OleObject {
    /// Server identifier (`ProgID` / `progId`), e.g. [`PROG_ID_XLSX`].
    pub prog_id: String,
    /// Embedded part (absolute part name); empty for a linked object.
    #[serde(default)]
    pub package: String,
    /// Source of a linked object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<OleLink>,
    /// Shown as an icon (`DrawAspect="Icon"` / `showAsIcon`).
    #[serde(default)]
    pub icon: bool,
}

impl OleObject {
    /// The object holds a SpreadsheetML workbook BlueSheet can open.
    pub fn is_workbook(&self) -> bool {
        let p = self.package.to_ascii_lowercase();
        p.ends_with(".xlsx")
            || p.ends_with(".xlsm")
            || self.prog_id.starts_with("Excel.Sheet")
            || self.link.as_ref().is_some_and(|l| {
                let s = l.source.to_ascii_lowercase();
                s.ends_with(".xlsx") || s.ends_with(".xlsm")
            })
    }
}

/// Item of a LINK field instruction (§17.16.5.32: `LINK ProgID "file" "item" switches`):
/// (prog id, file, item).
pub fn parse_link_instr(instr: &str) -> Option<(String, String, String)> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    let mut chars = instr.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            // `\\` inside a quoted path is a backslash (§17.16.1 field syntax).
            '\\' if quoted && chars.peek() == Some(&'\\') => {
                chars.next();
                cur.push('\\');
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        words.push(cur);
    }
    let mut it = words.into_iter();
    if !it.next()?.eq_ignore_ascii_case("LINK") {
        return None;
    }
    let prog = it.next()?;
    let file = it.next()?;
    let item = it
        .next()
        .filter(|w| !w.starts_with('\\'))
        .unwrap_or_default();
    Some((prog, file, item))
}

/// LINK field instruction for a linked object (§17.16.5.32); `\a` = update automatically,
/// `\p` = the result is a picture.
pub fn link_instr(prog_id: &str, link: &OleLink) -> String {
    let q = |s: &str| s.replace('\\', "\\\\").replace('"', "");
    let mut s = format!("LINK {} \"{}\"", prog_id, q(&link.source));
    if !link.item.is_empty() {
        s.push_str(&format!(" \"{}\"", q(&link.item)));
    }
    if link.auto_update {
        s.push_str(" \\a");
    }
    s.push_str(" \\p");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_link_field() {
        let (p, f, i) = parse_link_instr(
            r#" LINK Excel.Sheet.12 "C:\\data\\book.xlsx" "Sheet1!R1C1:R3C2" \a \p "#,
        )
        .unwrap();
        assert_eq!(p, "Excel.Sheet.12");
        assert_eq!(f, r"C:\data\book.xlsx");
        assert_eq!(i, "Sheet1!R1C1:R3C2");
        let (_, f, i) = parse_link_instr(r#"LINK Excel.Sheet.12 "b.xlsx" \p"#).unwrap();
        assert_eq!((f.as_str(), i.as_str()), ("b.xlsx", ""));
        assert!(parse_link_instr("PAGE").is_none());
    }

    #[test]
    fn link_instr_round_trips() {
        let l = OleLink {
            source: r"D:\a b\c.xlsx".into(),
            item: "Sheet1!R1C1:R2C2".into(),
            auto_update: true,
        };
        let s = link_instr(PROG_ID_XLSX, &l);
        assert_eq!(
            s,
            r#"LINK Excel.Sheet.12 "D:\\a b\\c.xlsx" "Sheet1!R1C1:R2C2" \a \p"#
        );
        let (_, f, i) = parse_link_instr(&s).unwrap();
        assert_eq!(
            (f.as_str(), i.as_str()),
            (l.source.as_str(), l.item.as_str())
        );
    }
}
