//! Style shortcut keys saved in the document ([MS-DOCX] 2.3.1 Keymap Customizations part,
//! 2.6.x `wne:tcg` / `wne:keymaps` / `wne:keymap` / `wne:acd`; BlueDoc round 22): New / Modify
//! Style › Format › Shortcut key… with "Save changes in" this document.
//!
//! A key is the `wne:kcmPrimary` hexadecimal key code: the low byte is the virtual-key code,
//! the high byte the modifiers (0x01 Shift, 0x02 Ctrl, 0x04 Alt). Each key names an
//! allocated command (`wne:acd`) whose `argValue` is the style name as UTF-16LE after a
//! 16-bit `0x0002` prefix (base64), with `fciIndexBasedOn="0065"` (apply style).
//! Only the style keys are read; other customizations of the part are kept when the keys
//! do not change (the part is then not rewritten).

use base64::Engine as _;

/// Relationship type of the keymap customizations part.
pub(crate) const REL_KEYMAP: &str =
    "http://schemas.microsoft.com/office/2006/relationships/keyMapCustomizations";
/// Content type of the part.
pub(crate) const CT_KEYMAP: &str = "application/vnd.ms-word.keyMapCustomizations+xml";
const WNE: &str = "http://schemas.microsoft.com/office/word/2006/wordml";

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=\"");
    let i = tag.find(&pat)? + pat.len();
    let end = tag[i..].find('"')?;
    Some(bo_opc::xml::unescape(&tag[i..i + end]))
}

/// The tags `<{name} …>` of `xml`.
fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name} ");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = xml[from..].find(&open) {
        let at = from + i;
        let Some(end) = xml[at..].find('>') else {
            break;
        };
        out.push(&xml[at..at + end]);
        from = at + end;
    }
    out
}

/// The style name of an `argValue`.
fn style_of(arg: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(arg.trim())
        .ok()?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let (first, rest) = units.split_first()?;
    if *first != 2 {
        return None;
    }
    let name = String::from_utf16(rest).ok()?;
    let name = name.trim_end_matches('\0').to_owned();
    (!name.is_empty()).then_some(name)
}

/// `argValue` of a style name.
fn arg_of(style: &str) -> String {
    let mut bytes = vec![2u8, 0];
    for u in style.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The style keys of a keymap customizations part: (key code, style name).
pub(crate) fn read(data: &[u8]) -> Vec<(u16, String)> {
    let xml = String::from_utf8_lossy(data);
    let mut acds = Vec::new();
    for tag in tags(&xml, "wne:acd") {
        if let (Some(name), Some(arg)) = (attr(tag, "wne:acdName"), attr(tag, "wne:argValue")) {
            if let Some(style) = style_of(&arg) {
                acds.push((name, style));
            }
        }
    }
    let mut out = Vec::new();
    let maps = xml.find("<wne:keymaps").map_or("", |i| {
        &xml[i..xml[i..].find("</wne:keymaps>").map_or(xml.len(), |e| i + e)]
    });
    let mut from = 0;
    while let Some(i) = maps[from..].find("<wne:keymap ") {
        let at = from + i;
        let end = maps[at..]
            .find("</wne:keymap>")
            .map_or(maps.len(), |e| at + e);
        let block = &maps[at..end];
        from = end.max(at + 1);
        let Some(kcm) = block
            .find('>')
            .and_then(|e| attr(&block[..e], "wne:kcmPrimary"))
            .and_then(|k| u16::from_str_radix(&k, 16).ok())
        else {
            continue;
        };
        let Some(acd) = tags(block, "wne:acd")
            .first()
            .and_then(|t| attr(t, "wne:acdName"))
        else {
            continue;
        };
        if let Some((_, style)) = acds.iter().find(|(n, _)| *n == acd) {
            out.push((kcm, style.clone()));
        }
    }
    out
}

/// A keymap customizations part holding `keys`.
pub(crate) fn write(keys: &[(u16, String)]) -> String {
    let mut x = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<wne:tcg xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wne=\"{WNE}\"><wne:keymaps>"
    );
    for (i, (kcm, _)) in keys.iter().enumerate() {
        x.push_str(&format!(
            "<wne:keymap wne:kcmPrimary=\"{kcm:04X}\"><wne:acd wne:acdName=\"acd{i}\"/></wne:keymap>"
        ));
    }
    x.push_str("</wne:keymaps><wne:acds>");
    for (i, (_, style)) in keys.iter().enumerate() {
        x.push_str(&format!(
            "<wne:acd wne:argValue=\"{}\" wne:acdName=\"acd{i}\" wne:fciIndexBasedOn=\"0065\"/>",
            arg_of(style)
        ));
    }
    x.push_str("</wne:acds></wne:tcg>");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_keys_round_trip() {
        let keys = vec![
            (0x0348, "Heading 1".to_owned()),
            (0x0631, "Tiêu đề".to_owned()),
        ];
        let xml = write(&keys);
        assert!(xml.contains("wne:kcmPrimary=\"0348\""));
        assert!(xml.contains("AgBIAGUAYQBkAGkAbgBnACAAMQA="));
        assert_eq!(read(xml.as_bytes()), keys);
    }
}
