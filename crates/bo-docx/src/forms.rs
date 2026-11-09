//! Legacy form fields (ECMA-376 Part 1, §17.16.17 ffData) and the document protection setting
//! (§17.15.1.29 documentProtection): reading and writing.

use bo_model::form::{
    DocProtection, FormCheckBox, FormDropDown, FormField, FormFieldKind, FormHelpText,
    FormTextInput, TextInputType,
};
use bo_model::{Run, RunContent};
use bo_opc::xml::{attr, escape, XmlReader};
use quick_xml::events::{BytesStart, Event};
use std::fmt::Write as _;

fn local(e: &BytesStart<'_>) -> Vec<u8> {
    e.local_name().as_ref().as_bytes().to_vec()
}

/// §17.17.4 `ST_OnOff`: an absent `w:val` means true.
fn on_off(e: &BytesStart<'_>) -> bool {
    !matches!(attr(e, b"val").as_deref(), Some("0" | "false" | "off"))
}

fn num(e: &BytesStart<'_>) -> Option<u32> {
    attr(e, b"val").and_then(|v| v.trim().parse::<u32>().ok())
}

/// Default data of a field of type `keyword` without a type element.
fn default_kind(keyword: &str) -> Option<FormFieldKind> {
    Some(match keyword {
        "FORMTEXT" => FormFieldKind::Text(FormTextInput::default()),
        "FORMCHECKBOX" => FormFieldKind::CheckBox(FormCheckBox::default()),
        "FORMDROPDOWN" => FormFieldKind::DropDown(FormDropDown::default()),
        _ => return None,
    })
}

/// Parse a `w:ffData` element (§17.16.17) of a field whose instruction keyword is `keyword`.
/// Unknown children are kept verbatim. `None` when the XML cannot be read.
pub(crate) fn parse_ff_data(xml: &str, keyword: &str) -> Option<FormField> {
    let mut f = FormField::new(default_kind(keyword)?, None);
    f.separate = false;
    let mut r = XmlReader::new(xml.as_bytes(), "/ffData");
    // The ffData start tag.
    loop {
        match r.read().ok()? {
            Event::Start(e) if local(&e) == b"ffData" => break,
            Event::Empty(e) if local(&e) == b"ffData" => {
                f.ff_xml = Some(xml.to_owned());
                return Some(f);
            }
            Event::Eof => return None,
            _ => {}
        }
    }
    let end = r.depth() - 1;
    loop {
        let before = r.pos();
        let (e, start) = match r.read().ok()? {
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(_) if r.depth() == end => break,
            Event::Eof => return None,
            _ => continue,
        };
        let help = |e: &BytesStart<'_>| FormHelpText {
            auto_text: attr(e, b"type").as_deref() == Some("autoText"),
            text: attr(e, b"val").unwrap_or_default(),
        };
        match local(&e).as_slice() {
            // §17.16.27 name
            b"name" => f.name = attr(&e, b"val"),
            // §17.16.14 enabled
            b"enabled" => f.enabled = on_off(&e),
            // §17.16.6 calcOnExit
            b"calcOnExit" => f.calc_on_exit = on_off(&e),
            // §17.16.15 entryMacro / §17.16.16 exitMacro
            b"entryMacro" => f.entry_macro = attr(&e, b"val"),
            b"exitMacro" => f.exit_macro = attr(&e, b"val"),
            // §17.16.21 helpText / §17.16.31 statusText
            b"helpText" => f.help_text = Some(help(&e)),
            b"statusText" => f.status_text = Some(help(&e)),
            // §17.16.7 checkBox
            b"checkBox" => {
                let mut c = FormCheckBox::default();
                if start {
                    read_children(&mut r, &mut |e| match local(e).as_slice() {
                        b"size" => c.size = num(e),                // §17.16.29 size
                        b"sizeAuto" => c.size = None,              // §17.16.30 sizeAuto
                        b"default" => c.default = on_off(e),       // §17.16.10 default
                        b"checked" => c.checked = Some(on_off(e)), // §17.16.8 checked
                        _ => {}
                    })?;
                }
                f.kind = FormFieldKind::CheckBox(c);
            }
            // §17.16.9 ddList
            b"ddList" => {
                let mut d = FormDropDown::default();
                if start {
                    read_children(&mut r, &mut |e| match local(e).as_slice() {
                        b"result" => d.result = num(e),   // §17.16.28 result
                        b"default" => d.default = num(e), // §17.16.12 default
                        // §17.16.25 listEntry
                        b"listEntry" => d.entries.push(attr(e, b"val").unwrap_or_default()),
                        _ => {}
                    })?;
                }
                f.kind = FormFieldKind::DropDown(d);
            }
            // §17.16.33 textInput
            b"textInput" => {
                let mut t = FormTextInput::default();
                if start {
                    read_children(&mut r, &mut |e| match local(e).as_slice() {
                        // §17.16.34 type
                        b"type" => {
                            t.input_type =
                                TextInputType::parse(&attr(e, b"val").unwrap_or_default());
                        }
                        b"default" => t.default = attr(e, b"val").unwrap_or_default(), // §17.16.11
                        b"maxLength" => t.max_length = num(e).unwrap_or(0),            // §17.16.26
                        b"format" => t.format = attr(e, b"val").unwrap_or_default(),   // §17.16.20
                        _ => {}
                    })?;
                }
                f.kind = FormFieldKind::Text(t);
            }
            // §17.16.24 label, §17.16.32 tabIndex and anything else: verbatim.
            _ => {
                if start {
                    r.skip_element().ok()?;
                }
                f.unknown.push(r.slice(before, r.pos()));
                continue;
            }
        }
        if start && !matches!(local(&e).as_slice(), b"checkBox" | b"ddList" | b"textInput") {
            r.skip_element().ok()?;
        }
    }
    f.ff_xml = Some(xml.to_owned());
    Some(f)
}

/// Visit the (empty or simple) child elements of the element just started.
fn read_children(r: &mut XmlReader<'_>, f: &mut dyn FnMut(&BytesStart<'_>)) -> Option<()> {
    let end = r.depth() - 1;
    loop {
        match r.read().ok()? {
            Event::Start(e) => {
                f(&e);
                r.skip_element().ok()?;
            }
            Event::Empty(e) => f(&e),
            Event::End(_) if r.depth() == end => return Some(()),
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// `w:ffData` of a form field: the source XML while the parsed data is unchanged, else written
/// from the model (unknown children kept).
pub(crate) fn ff_data_xml(f: &FormField) -> String {
    if let Some(xml) = &f.ff_xml {
        if parse_ff_data(xml, f.kind.keyword()).is_some_and(|read| read.data() == f.data()) {
            return xml.clone();
        }
    }
    let mut s = String::from("<w:ffData>");
    if let Some(n) = &f.name {
        let _ = write!(s, "<w:name w:val=\"{}\"/>", escape(n));
    }
    for u in &f.unknown {
        s.push_str(u);
    }
    // Written explicitly: readers differ on the meaning of an omitted element.
    s.push_str(if f.enabled {
        "<w:enabled/>"
    } else {
        "<w:enabled w:val=\"0\"/>"
    });
    s.push_str(if f.calc_on_exit {
        "<w:calcOnExit/>"
    } else {
        "<w:calcOnExit w:val=\"0\"/>"
    });
    if let Some(m) = &f.entry_macro {
        let _ = write!(s, "<w:entryMacro w:val=\"{}\"/>", escape(m));
    }
    if let Some(m) = &f.exit_macro {
        let _ = write!(s, "<w:exitMacro w:val=\"{}\"/>", escape(m));
    }
    for (tag, h) in [("helpText", &f.help_text), ("statusText", &f.status_text)] {
        if let Some(h) = h {
            let ty = if h.auto_text { "autoText" } else { "text" };
            let _ = write!(
                s,
                "<w:{tag} w:type=\"{ty}\" w:val=\"{}\"/>",
                escape(&h.text)
            );
        }
    }
    match &f.kind {
        FormFieldKind::CheckBox(c) => {
            s.push_str("<w:checkBox>");
            match c.size {
                Some(n) => {
                    let _ = write!(s, "<w:size w:val=\"{}\"/>", n.max(1));
                }
                None => s.push_str("<w:sizeAuto/>"),
            }
            if c.default {
                s.push_str("<w:default/>");
            }
            if let Some(ch) = c.checked {
                s.push_str(if ch {
                    "<w:checked/>"
                } else {
                    "<w:checked w:val=\"0\"/>"
                });
            }
            s.push_str("</w:checkBox>");
        }
        FormFieldKind::DropDown(d) => {
            s.push_str("<w:ddList>");
            if let Some(r) = d.result {
                let _ = write!(s, "<w:result w:val=\"{r}\"/>");
            }
            if let Some(r) = d.default {
                let _ = write!(s, "<w:default w:val=\"{r}\"/>");
            }
            for e in &d.entries {
                let _ = write!(s, "<w:listEntry w:val=\"{}\"/>", escape(e));
            }
            s.push_str("</w:ddList>");
        }
        FormFieldKind::Text(t) => {
            s.push_str("<w:textInput>");
            if t.input_type != TextInputType::Regular {
                let _ = write!(s, "<w:type w:val=\"{}\"/>", t.input_type.as_str());
            }
            if !t.default.is_empty() {
                let _ = write!(s, "<w:default w:val=\"{}\"/>", escape(&t.default));
            }
            if t.max_length > 0 {
                let _ = write!(s, "<w:maxLength w:val=\"{}\"/>", t.max_length);
            }
            if !t.format.is_empty() {
                let _ = write!(s, "<w:format w:val=\"{}\"/>", escape(&t.format));
            }
            s.push_str("</w:textInput>");
        }
    }
    s.push_str("</w:ffData>");
    s
}

/// The `w:ffData` element inside a `w:fldChar` begin element, if any.
fn ff_data_of(fld_char: &str) -> Option<&str> {
    let s = fld_char
        .find("<w:ffData")
        .or_else(|| fld_char.find(":ffData"))?;
    // Back up to the `<` of a prefixed tag.
    let s = fld_char[..=s].rfind('<')?;
    let rest = &fld_char[s..];
    if let Some(e) = rest.find("</w:ffData>") {
        return Some(&rest[..e + "</w:ffData>".len()]);
    }
    // Self-closing `<w:ffData/>`.
    let gt = rest.find('>')?;
    rest[..gt].ends_with('/').then(|| &rest[..=gt])
}

fn fld_char_type(c: &RunContent) -> Option<&'static str> {
    let RunContent::Opaque(raw) = c else {
        return None;
    };
    let x = &raw.xml;
    if !x.contains("fldChar") {
        return None;
    }
    ["begin", "separate", "end"]
        .into_iter()
        .find(|k| x.contains(&format!("fldCharType=\"{k}\"")))
}

fn instr_of(c: &RunContent) -> Option<String> {
    let RunContent::Opaque(raw) = c else {
        return None;
    };
    let x = &raw.xml;
    if !(x.starts_with("<w:instrText") || x.contains(":instrText")) {
        return None;
    }
    let gt = x.find('>')?;
    if x[..gt].ends_with('/') {
        return Some(String::new());
    }
    let rest = &x[gt + 1..];
    let end = rest.rfind("</").unwrap_or(rest.len());
    Some(bo_opc::xml::unescape(&rest[..end]))
}

/// Turn the complex fields `FORMTEXT` / `FORMCHECKBOX` / `FORMDROPDOWN` of a paragraph whose
/// begin character carries ffData (§17.16.17) into [`RunContent::FormField`] …
/// [`RunContent::FormEnd`]. Fields that are not complete in the paragraph, that nest other
/// fields or whose instruction holds anything else are kept as they are.
pub(crate) fn collapse_form_fields(runs: &mut Vec<Run>) {
    let at: Vec<(usize, usize)> = runs
        .iter()
        .enumerate()
        .flat_map(|(ri, r)| (0..r.content.len()).map(move |ii| (ri, ii)))
        .collect();
    let item = |runs: &[Run], k: usize| -> RunContent {
        let (ri, ii) = at[k];
        runs[ri].content[ii].clone()
    };
    let mut remove: Vec<(usize, usize)> = Vec::new();
    let mut k = 0;
    while k < at.len() {
        let begin = item(runs, k);
        let ff = match (&begin, fld_char_type(&begin)) {
            (RunContent::Opaque(raw), Some("begin")) => ff_data_of(&raw.xml).map(str::to_owned),
            _ => None,
        };
        let Some(ff) = ff else {
            k += 1;
            continue;
        };
        // Instruction, separator and end at the top level.
        let mut instr = String::new();
        let mut instr_items = Vec::new();
        let mut sep = None;
        let mut end = None;
        let mut ok = true;
        for j in k + 1..at.len() {
            let c = item(runs, j);
            match fld_char_type(&c) {
                Some("separate") if sep.is_none() => sep = Some(j),
                // A nested field or a second separator.
                Some("begin" | "separate") => {
                    ok = false;
                    break;
                }
                Some(_) => {
                    end = Some(j);
                    break;
                }
                None => {}
            }
            if sep.is_none() {
                match instr_of(&c) {
                    Some(t) => {
                        instr.push_str(&t);
                        instr_items.push(j);
                    }
                    // Only instruction text before the separator.
                    None if matches!(c, RunContent::Markup(_)) => {}
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
        }
        let keyword = instr
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let field = (ok && end.is_some())
            .then(|| parse_ff_data(&ff, &keyword))
            .flatten();
        let (Some(mut field), Some(end)) = (field, end) else {
            k += 1;
            continue;
        };
        if let RunContent::Opaque(raw) = &begin {
            // §17.16.18 fldLock
            let tag = &raw.xml[..raw.xml.find('>').unwrap_or(raw.xml.len())];
            field.locked = tag.contains("fldLock=\"1\"") || tag.contains("fldLock=\"true\"");
        }
        field.instr = instr;
        field.separate = sep.is_some();
        let (ri, ii) = at[k];
        runs[ri].content[ii] = RunContent::FormField(Box::new(field));
        let (ri, ii) = at[end];
        runs[ri].content[ii] = RunContent::FormEnd;
        remove.extend(instr_items.iter().map(|&j| at[j]));
        if let Some(s) = sep {
            remove.push(at[s]);
        }
        k = end + 1;
    }
    if remove.is_empty() {
        return;
    }
    remove.sort_unstable();
    for &(ri, ii) in remove.iter().rev() {
        runs[ri].content.remove(ii);
    }
    // Runs that held only field characters go; runs that were empty before stay.
    let emptied: Vec<usize> = {
        let mut v: Vec<usize> = remove.iter().map(|x| x.0).collect();
        v.dedup();
        v
    };
    for ri in emptied.into_iter().rev() {
        if runs[ri].content.is_empty() {
            runs.remove(ri);
        }
    }
}

/// Read §17.15.1.29 documentProtection.
pub(crate) fn read_protection(e: &BytesStart<'_>) -> DocProtection {
    let mut p = DocProtection::default();
    for a in e.attributes().flatten() {
        let k = a.key.local_name().as_ref().to_owned();
        let v = bo_opc::xml::unescape(&a.value);
        match k.as_str() {
            "edit" => p.edit = v,
            "enforcement" => p.enforced = !matches!(v.as_str(), "0" | "false" | "off"),
            "formatting" => p.formatting = !matches!(v.as_str(), "0" | "false" | "off"),
            _ => p.attrs.push((k, v)),
        }
    }
    p
}

/// §17.15.1.29 documentProtection element.
pub(crate) fn protection_xml(p: &DocProtection) -> String {
    let mut s = String::from("<w:documentProtection");
    if !p.edit.is_empty() {
        let _ = write!(s, " w:edit=\"{}\"", escape(&p.edit));
    }
    if p.formatting {
        s.push_str(" w:formatting=\"1\"");
    }
    let _ = write!(s, " w:enforcement=\"{}\"", u8::from(p.enforced));
    for (k, v) in &p.attrs {
        let _ = write!(s, " w:{k}=\"{}\"", escape(v));
    }
    s.push_str("/>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_children() {
        let xml = "<w:ffData><w:name w:val=\"Age\"/><w:label w:val=\"3\"/><w:enabled/><w:calcOnExit/><w:entryMacro w:val=\"In\"/><w:exitMacro w:val=\"Out\"/><w:helpText w:type=\"text\" w:val=\"F1 help\"/><w:statusText w:type=\"autoText\" w:val=\"Entry\"/><w:textInput><w:type w:val=\"number\"/><w:default w:val=\"18\"/><w:maxLength w:val=\"3\"/><w:format w:val=\"0.00\"/></w:textInput></w:ffData>";
        let f = parse_ff_data(xml, "FORMTEXT").expect("parsed");
        assert_eq!(f.name.as_deref(), Some("Age"));
        assert!(f.enabled && f.calc_on_exit);
        assert_eq!(f.entry_macro.as_deref(), Some("In"));
        assert_eq!(f.exit_macro.as_deref(), Some("Out"));
        assert_eq!(
            f.help_text,
            Some(FormHelpText {
                auto_text: false,
                text: "F1 help".into()
            })
        );
        assert!(f.status_text.as_ref().is_some_and(|s| s.auto_text));
        assert_eq!(f.unknown, vec!["<w:label w:val=\"3\"/>".to_owned()]);
        assert_eq!(
            f.kind,
            FormFieldKind::Text(FormTextInput {
                input_type: TextInputType::Number,
                default: "18".into(),
                max_length: 3,
                format: "0.00".into(),
            })
        );
        // Unchanged: verbatim.
        assert_eq!(ff_data_xml(&f), xml);
        // Changed: written from the model, the unknown label kept, and read back equal.
        let mut g = f.clone();
        g.name = Some("Years".into());
        let out = ff_data_xml(&g);
        assert!(out.contains("<w:label w:val=\"3\"/>"));
        let back = parse_ff_data(&out, "FORMTEXT").expect("parsed");
        assert_eq!(back.data(), g.data());
    }

    #[test]
    fn check_box_and_list() {
        let xml = "<w:ffData><w:name w:val=\"Check1\"/><w:enabled w:val=\"0\"/><w:calcOnExit w:val=\"0\"/><w:checkBox><w:size w:val=\"24\"/><w:default/><w:checked w:val=\"0\"/></w:checkBox></w:ffData>";
        let f = parse_ff_data(xml, "FORMCHECKBOX").expect("parsed");
        assert!(!f.enabled && !f.calc_on_exit);
        assert_eq!(
            f.kind,
            FormFieldKind::CheckBox(FormCheckBox {
                size: Some(24),
                default: true,
                checked: Some(false)
            })
        );
        assert_eq!(f.display().as_deref(), Some("\u{2610}"));
        let mut g = f.clone();
        g.ff_xml = None;
        assert_eq!(
            parse_ff_data(&ff_data_xml(&g), "FORMCHECKBOX").map(|x| x.data()),
            Some(f.data())
        );

        let xml = "<w:ffData><w:name w:val=\"Dropdown1\"/><w:enabled/><w:calcOnExit w:val=\"0\"/><w:ddList><w:result w:val=\"2\"/><w:default w:val=\"1\"/><w:listEntry w:val=\"Đỏ\"/><w:listEntry w:val=\"Xanh\"/><w:listEntry w:val=\"Vàng &amp; cam\"/></w:ddList></w:ffData>";
        let f = parse_ff_data(xml, "FORMDROPDOWN").expect("parsed");
        assert_eq!(f.display().as_deref(), Some("Vàng & cam"));
        let mut g = f.clone();
        g.ff_xml = None;
        let out = ff_data_xml(&g);
        assert!(out.contains("Vàng &amp; cam"));
        assert_eq!(
            parse_ff_data(&out, "FORMDROPDOWN").map(|x| x.data()),
            Some(f.data())
        );
    }

    #[test]
    fn protection_attributes() {
        let xml = "<w:documentProtection w:edit=\"forms\" w:enforcement=\"1\" w:algorithmName=\"SHA-512\" w:hashValue=\"abc\" w:saltValue=\"c2FsdA==\" w:spinCount=\"100000\"/>";
        let mut r = XmlReader::new(xml.as_bytes(), "/settings");
        let Ok(Event::Empty(e)) = r.read() else {
            panic!("element")
        };
        let p = read_protection(&e);
        assert!(p.forms_enforced() && p.has_password());
        assert_eq!(p.attr("spinCount"), Some("100000"));
        assert_eq!(protection_xml(&p), xml);
    }
}
