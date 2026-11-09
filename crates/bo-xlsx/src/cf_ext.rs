//! Conditional formatting extensions of a worksheet ([MS-XLSX] x14 schema,
//! `x14:conditionalFormattings` in the worksheet `extLst`, ext uri
//! `{78C0D931-6437-407d-A8EE-F0AAD7539E65}`).
//!
//! * A 2006 rule (§18.3.1.10 `cfRule`) with a 2010 counterpart carries
//!   `cfRule/extLst/ext[@uri={B025F937-…}]/x14:id`; the `x14:cfRule` with that `id`
//!   adds the negative bar / axis / border / gradient settings of a data bar
//!   ([MS-XLSX] x14 `CT_DataBar`). Both are merged into one model rule
//!   ([`CfRule::ext_id`], [`DataBar::x14`]).
//! * Rules the 2006 schema cannot express (icon sets `3Stars`, `3Triangles`, `5Boxes`,
//!   custom icons `x14:cfIcon`, `x14:dxf`, …) live only in the extension; they are read
//!   into blocks with [`ConditionalFormat::ext`] set.
//!
//! On write the extension is regenerated from the model and placed first in the
//! worksheet `extLst`; other `ext` elements are kept verbatim.

use crate::styles::write_color;
use crate::xml::{a, a_opt, a_optb, a_raw, a_true};
use bo_model::sheet::{
    sqref_to_string, CellRange, CfIcon, CfRule, Cfvo, ColorScale, ConditionalFormat, DataBar,
    DataBarX14, IconSet, RawAttr, RawXml, SheetColor, Worksheet,
};
use bo_opc::xml::{escape, unescape, XmlReader};
use quick_xml::events::Event;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// `ext/@uri` of `x14:conditionalFormattings` in a worksheet `extLst`.
pub(crate) const EXT_CF: &str = "{78C0D931-6437-407d-A8EE-F0AAD7539E65}";
/// `ext/@uri` of `x14:id` in a `cfRule/extLst`.
pub(crate) const EXT_CF_ID: &str = "{B025F937-C7B1-47D3-B67F-A62EFF666E3E}";
const X14_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const XM_NS: &str = "http://schemas.microsoft.com/office/excel/2006/main";

// ---------------------------------------------------------------------------
// Small span-aware XML tree
// ---------------------------------------------------------------------------

/// An element with its byte span in the source fragment.
#[derive(Debug, Default)]
struct XNode {
    local: String,
    attrs: Vec<RawAttr>,
    children: Vec<XNode>,
    text: String,
    start: usize,
    end: usize,
}

impl XNode {
    fn attr(&self, local: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.rsplit_once(':').map_or(k.as_str(), |(_, l)| l) == local)
            .map(|(_, v)| v.as_str())
    }

    fn flag(&self, local: &str) -> Option<bool> {
        self.attr(local).map(crate::xml::pbool)
    }

    fn u32(&self, local: &str) -> Option<u32> {
        self.attr(local).and_then(|v| v.trim().parse().ok())
    }

    fn child(&self, local: &str) -> Option<&XNode> {
        self.children.iter().find(|c| c.local == local)
    }
}

fn local_of(q: &str) -> String {
    q.rsplit_once(':').map_or(q, |(_, l)| l).to_owned()
}

/// Parse a fragment with one root element. `None` on malformed XML.
fn parse(xml: &str) -> Option<XNode> {
    let mut r = XmlReader::new(xml.as_bytes(), "extLst");
    let mut stack: Vec<XNode> = vec![XNode::default()];
    loop {
        let start = r.pos();
        match r.read().ok()? {
            Event::Start(e) => {
                stack.push(XNode {
                    local: crate::xml::lname(&e),
                    attrs: crate::xml::attrs(&e),
                    start,
                    ..XNode::default()
                });
            }
            Event::Empty(e) => {
                let n = XNode {
                    local: crate::xml::lname(&e),
                    attrs: crate::xml::attrs(&e),
                    start,
                    end: r.pos(),
                    ..XNode::default()
                };
                stack.last_mut()?.children.push(n);
            }
            Event::End(_) => {
                let mut n = stack.pop()?;
                n.end = r.pos();
                stack.last_mut()?.children.push(n);
            }
            Event::Text(t) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&unescape(&t));
                }
            }
            Event::GeneralRef(g) => {
                if let Some(n) = stack.last_mut() {
                    let t: &str = &g;
                    n.text.push_str(&unescape(&format!("&{t};")));
                }
            }
            Event::CData(c) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(&c);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut root = stack.pop()?;
    if !stack.is_empty() {
        return None;
    }
    root.children.pop()
}

/// `xml` without the `ext` children of its root `extLst` whose uri is `uri`;
/// returns the removed `ext` elements and the rest (`None` = nothing left).
fn split_ext(xml: &str, uri: &str) -> Option<(Vec<XNode>, Option<String>)> {
    let root = parse(xml)?;
    if root.local != "extLst" {
        return None;
    }
    let mut rest = String::new();
    let mut taken = Vec::new();
    let mut cursor = 0;
    let mut left = 0;
    for c in root.children {
        if c.local == "ext" && c.attr("uri").is_some_and(|u| u.eq_ignore_ascii_case(uri)) {
            rest.push_str(xml.get(cursor..c.start)?);
            cursor = c.end;
            taken.push(c);
        } else {
            left += 1;
        }
    }
    if taken.is_empty() {
        return None;
    }
    rest.push_str(xml.get(cursor..)?);
    Some((taken, (left > 0).then_some(rest)))
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

fn color(n: &XNode) -> SheetColor {
    SheetColor {
        rgb: n.attr("rgb").map(str::to_owned),
        theme: n.u32("theme"),
        indexed: n.u32("indexed"),
        tint: n
            .attr("tint")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite()),
        auto: n.flag("auto").unwrap_or(false),
    }
}

/// [MS-XLSX] x14 `CT_Cfvo`: `type`, `gte` and the `xm:f` child.
fn cfvo(n: &XNode) -> Cfvo {
    Cfvo {
        kind: n.attr("type").unwrap_or_default().to_owned(),
        val: n
            .child("f")
            .map(|f| bo_formula::from_storage(f.text.trim())),
        gte: n.flag("gte"),
    }
}

/// [MS-XLSX] x14 `CT_DataBar`.
fn data_bar(n: &XNode) -> DataBarX14 {
    let col = |name: &str| n.child(name).map(color);
    DataBarX14 {
        min_length: n.u32("minLength"),
        max_length: n.u32("maxLength"),
        border: n.flag("border").unwrap_or(false),
        gradient: n.flag("gradient"),
        direction: n.attr("direction").map(str::to_owned),
        negative_same_color: n.flag("negativeBarColorSameAsPositive").unwrap_or(false),
        negative_same_border: n.flag("negativeBarBorderColorSameAsPositive"),
        axis_position: n.attr("axisPosition").map(str::to_owned),
        cfvos: n
            .children
            .iter()
            .filter(|c| c.local == "cfvo")
            .map(cfvo)
            .collect(),
        fill_color: col("fillColor"),
        border_color: col("borderColor"),
        negative_fill_color: col("negativeFillColor"),
        negative_border_color: col("negativeBorderColor"),
        axis_color: col("axisColor"),
    }
}

/// [MS-XLSX] x14 `CT_IconSet` (with `custom` and `x14:cfIcon`).
fn icon_set(n: &XNode) -> IconSet {
    IconSet {
        icon_set: n.attr("iconSet").map(str::to_owned),
        cfvos: n
            .children
            .iter()
            .filter(|c| c.local == "cfvo")
            .map(cfvo)
            .collect(),
        show_value: n.flag("showValue"),
        percent: n.flag("percent"),
        reverse: n.flag("reverse"),
        custom: n.flag("custom").unwrap_or(false),
        icons: n
            .children
            .iter()
            .filter(|c| c.local == "cfIcon")
            .map(|c| CfIcon {
                set: c.attr("iconSet").unwrap_or("NoIcons").to_owned(),
                id: c.u32("iconId").unwrap_or(0),
            })
            .collect(),
    }
}

/// [MS-XLSX] x14 `CT_CfRule`; unknown children (`x14:dxf`, `extLst`) are kept raw.
fn rule(n: &XNode, src: &str) -> CfRule {
    let mut r = CfRule {
        kind: n.attr("type").unwrap_or_default().to_owned(),
        priority: n
            .attr("priority")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(1),
        operator: n.attr("operator").map(str::to_owned),
        stop_if_true: n.flag("stopIfTrue").unwrap_or(false),
        text: n.attr("text").map(str::to_owned),
        ext_id: n.attr("id").map(str::to_owned),
        attrs: n
            .attrs
            .iter()
            .filter(|(k, _)| {
                !matches!(
                    local_of(k).as_str(),
                    "type" | "priority" | "operator" | "stopIfTrue" | "text" | "id"
                )
            })
            .cloned()
            .collect(),
        ..CfRule::default()
    };
    for c in &n.children {
        match c.local.as_str() {
            "f" => r.formulas.push(bo_formula::from_storage(c.text.trim())),
            "colorScale" => {
                let mut cs = ColorScale::default();
                for k in &c.children {
                    match k.local.as_str() {
                        "cfvo" => cs.cfvos.push(cfvo(k)),
                        "color" => cs.colors.push(color(k)),
                        _ => {}
                    }
                }
                r.color_scale = Some(cs);
            }
            "dataBar" => {
                r.data_bar = Some(DataBar {
                    x14: Some(data_bar(c)),
                    ..DataBar::default()
                });
            }
            "iconSet" => r.icon_set = Some(icon_set(c)),
            other => r.unknown.push(RawXml {
                after: other.to_owned(),
                xml: src.get(c.start..c.end).unwrap_or_default().to_owned(),
            }),
        }
    }
    r
}

/// `x14:id` of a 2006 rule (from its raw `extLst`), removing that `ext`.
fn take_rule_id(r: &mut CfRule) -> Option<String> {
    let i = r.unknown.iter().position(|u| u.xml.contains(EXT_CF_ID))?;
    let (taken, rest) = split_ext(&r.unknown[i].xml, EXT_CF_ID)?;
    let id = taken
        .iter()
        .find_map(|e| e.child("id"))
        .map(|n| n.text.trim().to_owned())?;
    match rest {
        Some(x) => r.unknown[i].xml = x,
        None => {
            r.unknown.remove(i);
        }
    }
    Some(id)
}

/// Moves the `x14:conditionalFormattings` extension of `ws` into the model.
pub(crate) fn extract(ws: &mut Worksheet) {
    let Some(i) = ws
        .unknown
        .iter()
        .position(|u| u.after == "extLst" && u.xml.contains(EXT_CF))
    else {
        return;
    };
    let src = ws.unknown[i].xml.clone();
    let Some((taken, rest)) = split_ext(&src, EXT_CF) else {
        return;
    };
    match rest {
        Some(x) => ws.unknown[i].xml = x,
        None => {
            ws.unknown.remove(i);
        }
    }
    // 2006 rules referencing an x14 rule, by id.
    let mut linked: HashMap<String, (usize, usize)> = HashMap::new();
    for (bi, cf) in ws.conditional_formats.iter_mut().enumerate() {
        for (ri, r) in cf.rules.iter_mut().enumerate() {
            if let Some(id) = take_rule_id(r) {
                linked.insert(id.clone(), (bi, ri));
                r.ext_id = Some(id);
            }
        }
    }
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for ext in &taken {
        let Some(list) = ext.child("conditionalFormattings") else {
            continue;
        };
        for block in list
            .children
            .iter()
            .filter(|c| c.local == "conditionalFormatting")
        {
            let sqref = block
                .child("sqref")
                .map(|s| CellRange::parse_sqref(s.text.trim()))
                .unwrap_or_default();
            let mut own = ConditionalFormat {
                sqref,
                pivot: block.flag("pivot").unwrap_or(false),
                rules: Vec::new(),
                ext: true,
            };
            for rn in block.children.iter().filter(|c| c.local == "cfRule") {
                let x = rule(rn, &src);
                match x.ext_id.as_ref().and_then(|id| linked.get(id)).copied() {
                    Some(at) if seen.insert(at) => {
                        merge(&mut ws.conditional_formats[at.0].rules[at.1], x);
                    }
                    _ => own.rules.push(x),
                }
            }
            if !own.rules.is_empty() {
                ws.conditional_formats.push(own);
            }
        }
    }
    // A 2006 rule whose id has no x14 rule keeps its reference raw.
    for (id, (bi, ri)) in linked {
        if !seen.contains(&(bi, ri)) {
            let r = &mut ws.conditional_formats[bi].rules[ri];
            r.ext_id = None;
            r.unknown.push(RawXml {
                after: "extLst".into(),
                xml: format!(
                    "<extLst><ext uri=\"{EXT_CF_ID}\" xmlns:x14=\"{X14_NS}\"><x14:id>{}</x14:id></ext></extLst>",
                    escape(&id)
                ),
            });
        }
    }
}

/// Adds the 2010 settings of `x` to the linked 2006 rule `r`.
fn merge(r: &mut CfRule, x: CfRule) {
    if let Some(db) = x.data_bar.and_then(|d| d.x14) {
        r.data_bar.get_or_insert_with(DataBar::default).x14 = Some(db);
    }
    if let Some(is) = x.icon_set {
        if r.icon_set.is_none() {
            r.icon_set = Some(is);
        }
    }
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// The rule has 2010 settings that need a linked `x14:cfRule`.
fn has_x14(r: &CfRule) -> bool {
    r.data_bar.as_ref().is_some_and(|d| d.x14.is_some())
}

/// The rule is written only in the extension.
pub(crate) fn ext_only(cf: &ConditionalFormat, r: &CfRule) -> bool {
    cf.ext || r.needs_ext()
}

/// `x14:id` of every 2006 rule with a linked x14 rule, by (block, rule) index.
/// Missing ids get a stable generated GUID.
pub(crate) fn rule_ids(ws: &Worksheet) -> HashMap<(usize, usize), String> {
    let mut used: HashSet<String> = ws
        .conditional_formats
        .iter()
        .flat_map(|c| c.rules.iter().filter_map(|r| r.ext_id.clone()))
        .collect();
    let mut out = HashMap::new();
    let mut n: u64 = 0;
    for (bi, cf) in ws.conditional_formats.iter().enumerate() {
        for (ri, r) in cf.rules.iter().enumerate() {
            if ext_only(cf, r) || !has_x14(r) {
                continue;
            }
            let id = r.ext_id.clone().unwrap_or_else(|| loop {
                n += 1;
                let id = format!("{{B0E5CF00-0000-4000-8000-{n:012X}}}");
                if used.insert(id.clone()) {
                    break id;
                }
            });
            out.insert((bi, ri), id);
        }
    }
    out
}

/// The `extLst` fragment holding `x14:id` for a 2006 `cfRule` (merged into the
/// rule's own raw `extLst` when it has one).
pub(crate) fn rule_ext_unknown(unknown: &[RawXml], id: &str) -> Vec<RawXml> {
    let ext = format!(
        "<ext uri=\"{EXT_CF_ID}\" xmlns:x14=\"{X14_NS}\"><x14:id>{}</x14:id></ext>",
        escape(id)
    );
    let mut out = unknown.to_vec();
    if let Some(x) = out.iter_mut().find(|u| u.after == "extLst") {
        if let Some(k) = x
            .xml
            .find('>')
            .filter(|_| !x.xml.trim_end().ends_with("/>"))
        {
            x.xml.insert_str(k + 1, &ext);
            return out;
        }
    }
    out.push(RawXml {
        after: "extLst".into(),
        xml: format!("<extLst>{ext}</extLst>"),
    });
    out
}

fn write_cfvo(out: &mut String, c: &Cfvo) {
    out.push_str("<x14:cfvo");
    a(out, "type", &c.kind);
    a_optb(out, "gte", c.gte);
    match c.val.as_deref() {
        Some(v) if !matches!(c.kind.as_str(), "min" | "max" | "autoMin" | "autoMax") => {
            let _ = write!(
                out,
                "><xm:f>{}</xm:f></x14:cfvo>",
                escape(&bo_formula::to_storage(v))
            );
        }
        _ => out.push_str("/>"),
    }
}

fn write_data_bar(out: &mut String, d: &DataBarX14) {
    out.push_str("<x14:dataBar");
    a_opt(out, "minLength", d.min_length.as_ref());
    a_opt(out, "maxLength", d.max_length.as_ref());
    a_true(out, "border", d.border);
    a_optb(out, "gradient", d.gradient);
    a_opt(out, "direction", d.direction.as_ref());
    a_true(out, "negativeBarColorSameAsPositive", d.negative_same_color);
    a_optb(
        out,
        "negativeBarBorderColorSameAsPositive",
        d.negative_same_border,
    );
    a_opt(out, "axisPosition", d.axis_position.as_ref());
    let empty = d.cfvos.is_empty()
        && d.fill_color.is_none()
        && d.border_color.is_none()
        && d.negative_fill_color.is_none()
        && d.negative_border_color.is_none()
        && d.axis_color.is_none();
    if empty {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &d.cfvos {
        write_cfvo(out, c);
    }
    for (tag, c) in [
        ("x14:fillColor", &d.fill_color),
        ("x14:borderColor", &d.border_color),
        ("x14:negativeFillColor", &d.negative_fill_color),
        ("x14:negativeBorderColor", &d.negative_border_color),
        ("x14:axisColor", &d.axis_color),
    ] {
        if let Some(c) = c {
            write_color(out, tag, c);
        }
    }
    out.push_str("</x14:dataBar>");
}

fn write_icon_set(out: &mut String, is: &IconSet) {
    out.push_str("<x14:iconSet");
    a_opt(out, "iconSet", is.icon_set.as_ref());
    a_optb(out, "showValue", is.show_value);
    a_optb(out, "percent", is.percent);
    a_optb(out, "reverse", is.reverse);
    a_true(out, "custom", is.custom);
    out.push('>');
    for c in &is.cfvos {
        write_cfvo(out, c);
    }
    if is.custom {
        for i in &is.icons {
            out.push_str("<x14:cfIcon");
            a(out, "iconSet", &i.set);
            a(out, "iconId", &i.id.to_string());
            out.push_str("/>");
        }
    }
    out.push_str("</x14:iconSet>");
}

/// A full `x14:cfRule` (rule stored only in the extension).
fn write_full_rule(out: &mut String, r: &CfRule, id: &str) {
    out.push_str("<x14:cfRule");
    a(out, "type", &r.kind);
    a(out, "priority", &r.priority.to_string());
    a_true(out, "stopIfTrue", r.stop_if_true);
    a_opt(out, "operator", r.operator.as_ref());
    a_opt(out, "text", r.text.as_ref());
    a_raw(out, &r.attrs);
    a(out, "id", id);
    out.push('>');
    for f in &r.formulas {
        let _ = write!(out, "<xm:f>{}</xm:f>", escape(&bo_formula::to_storage(f)));
    }
    if let Some(cs) = &r.color_scale {
        out.push_str("<x14:colorScale>");
        for c in &cs.cfvos {
            write_cfvo(out, c);
        }
        for c in &cs.colors {
            write_color(out, "x14:color", c);
        }
        out.push_str("</x14:colorScale>");
    }
    if let Some(d) = r.data_bar.as_ref().and_then(|d| d.x14.as_ref()) {
        write_data_bar(out, d);
    }
    if let Some(is) = &r.icon_set {
        write_icon_set(out, is);
    }
    for slot in ["dxf", "extLst"] {
        for u in r.unknown.iter().filter(|u| u.after == slot) {
            out.push_str(&u.xml);
        }
    }
    out.push_str("</x14:cfRule>");
}

fn open_block(out: &mut String, cf: &ConditionalFormat) {
    out.push_str("<x14:conditionalFormatting");
    let _ = write!(out, " xmlns:xm=\"{XM_NS}\"");
    a_true(out, "pivot", cf.pivot);
    out.push('>');
}

fn close_block(out: &mut String, cf: &ConditionalFormat) {
    let _ = write!(
        out,
        "<xm:sqref>{}</xm:sqref></x14:conditionalFormatting>",
        sqref_to_string(&cf.sqref)
    );
}

/// The worksheet `ext` element holding `x14:conditionalFormattings`, if any rule needs it.
pub(crate) fn sheet_ext(ws: &Worksheet, ids: &HashMap<(usize, usize), String>) -> Option<String> {
    let mut body = String::new();
    // Linked 2010 parts of 2006 rules first, in block order.
    for (bi, cf) in ws.conditional_formats.iter().enumerate() {
        let linked: Vec<(usize, &CfRule)> = cf
            .rules
            .iter()
            .enumerate()
            .filter(|(ri, _)| ids.contains_key(&(bi, *ri)))
            .collect();
        if linked.is_empty() {
            continue;
        }
        open_block(&mut body, cf);
        for (ri, r) in linked {
            body.push_str("<x14:cfRule");
            a(&mut body, "type", &r.kind);
            a(&mut body, "id", &ids[&(bi, ri)]);
            body.push('>');
            if let Some(d) = r.data_bar.as_ref().and_then(|d| d.x14.as_ref()) {
                write_data_bar(&mut body, d);
            }
            body.push_str("</x14:cfRule>");
        }
        close_block(&mut body, cf);
    }
    // Rules stored only in the extension.
    let mut n: u64 = 0;
    let used: HashSet<&str> = ids.values().map(String::as_str).collect();
    for cf in &ws.conditional_formats {
        let own: Vec<&CfRule> = cf.rules.iter().filter(|r| ext_only(cf, r)).collect();
        if own.is_empty() {
            continue;
        }
        open_block(&mut body, cf);
        for r in own {
            let id = r.ext_id.clone().unwrap_or_else(|| loop {
                n += 1;
                let id = format!("{{B0E5CF00-0000-4000-9000-{n:012X}}}");
                if !used.contains(id.as_str()) {
                    break id;
                }
            });
            write_full_rule(&mut body, r, &id);
        }
        close_block(&mut body, cf);
    }
    (!body.is_empty()).then(|| {
        format!(
            "<ext uri=\"{EXT_CF}\" xmlns:x14=\"{X14_NS}\"><x14:conditionalFormattings>{body}</x14:conditionalFormattings></ext>"
        )
    })
}

/// `unknown` with `ext` placed first in the worksheet `extLst` (created when missing).
pub(crate) fn with_first_ext(unknown: &[RawXml], ext: &str) -> Vec<RawXml> {
    let mut out = unknown.to_vec();
    if let Some(x) = out
        .iter_mut()
        .find(|u| u.after == "extLst" && parse(&u.xml).is_some_and(|n| n.local == "extLst"))
    {
        if !x.xml.trim_end().ends_with("/>") {
            if let Some(k) = x.xml.find('>') {
                x.xml.insert_str(k + 1, ext);
                return out;
            }
        }
    }
    out.push(RawXml {
        after: "extLst".into(),
        xml: format!("<extLst>{ext}</extLst>"),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_other_exts_verbatim() {
        let xml = format!(
            r#"<extLst><ext uri="{EXT_CF}" xmlns:x14="{X14_NS}"><x14:conditionalFormattings/></ext><ext uri="{{X}}"><a b="1&amp;2"/></ext></extLst>"#
        );
        let (taken, rest) = split_ext(&xml, EXT_CF).unwrap();
        assert_eq!(taken.len(), 1);
        assert_eq!(
            rest.as_deref(),
            Some(r#"<extLst><ext uri="{X}"><a b="1&amp;2"/></ext></extLst>"#)
        );
        let only = format!(r#"<extLst><ext uri="{EXT_CF}"/></extLst>"#);
        assert_eq!(split_ext(&only, EXT_CF).unwrap().1, None);
    }
}
