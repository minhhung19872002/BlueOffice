//! Form controls and ActiveX controls of a worksheet.
//!
//! A form control is described three times in a package:
//! * the legacy VML drawing (`legacyDrawing`, ECMA-376 Part 4 §19 VML with the
//!   `urn:schemas-microsoft-com:office:excel` `x:ClientData` element: `ObjectType`,
//!   `x:Anchor`, `x:Checked`, `x:FmlaLink`, `x:FmlaRange`, `x:Val`, …);
//! * the worksheet `controls` element (§18.3.1.21) inside `mc:AlternateContent`
//!   (`Requires="x14"`), whose `control` (§18.3.1.19) carries the [MS-XLSX] `controlPr`
//!   (macro, anchor) and points at
//! * a control properties part ([MS-XLSX] `formControlPr`, `CT_FormControlPr`).
//!
//! Reading merges the three (control properties win). All parts stay opaque, so a file
//! whose controls are untouched is written back byte-for-byte; a changed control has its
//! control properties part and VML shape patched in place, a removed one loses its entry,
//! part, relationship and shapes, and a new one gets a control properties part, a VML shape
//! (appended to the sheet's legacy drawing, or a new one) and a `controls` entry.
//! ActiveX controls (`control` relationship, [MS-OFORMS] persisted in the `activeX` part)
//! are kept as they are; their `controlPr/@r:id` names the fallback picture.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use bo_model::sheet::{
    CellRef, CheckState, ControlAnchor, ControlPlacement, FormControl, FormControlKind, OpaquePart,
    OpaqueRel, RawXml, SelType, Worksheet,
};
use bo_opc::xml::{escape, unescape};
use bo_opc::{Relationship, TargetMode};

use crate::chart::{parse_dom, Node};

/// Relationship type of a control properties part ([MS-XLSX] 2.1.x).
pub const REL_CTRL_PROP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/ctrlProp";
/// Relationship type of a VML drawing (ECMA-376 Part 1 §15.2.20 legacy drawing).
pub const REL_VML: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing";
/// Relationship type of an ActiveX control part (§15.2.9 Control).
pub const REL_ACTIVEX: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/control";
/// Content type of a control properties part.
pub const CT_CTRL_PROP: &str = "application/vnd.ms-excel.controlproperties+xml";
/// Content type of a VML drawing part.
pub const CT_VML: &str = "application/vnd.openxmlformats-officedocument.vmlDrawing";

const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const NS_MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const NS_XDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";

/// EMU per pixel at 96 dpi (VML `x:Anchor` offsets are pixels).
const EMU_PER_PX: i64 = 9525;

// ------------------------------------------------------------------ tag scanner

/// Kind of a scanned tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TagKind {
    Open,
    Close,
    Empty,
}

/// A tag found by [`tags`]: byte span and qualified-name span.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tag {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: TagKind,
    name: (usize, usize),
}

impl Tag {
    fn qname<'a>(&self, xml: &'a str) -> &'a str {
        &xml[self.name.0..self.name.1]
    }

    pub(crate) fn local<'a>(&self, xml: &'a str) -> &'a str {
        let q = self.qname(xml);
        q.rsplit_once(':').map_or(q, |(_, l)| l)
    }

    pub(crate) fn text<'a>(&self, xml: &'a str) -> &'a str {
        &xml[self.start..self.end]
    }
}

/// The tags of an XML (or loosely-formed VML) text, skipping comments, processing
/// instructions, declarations and CDATA; `>` inside quoted attribute values is honoured.
pub(crate) fn tags(xml: &str) -> Vec<Tag> {
    let b = xml.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = xml[i..].find('<') {
        let s = i + p;
        let rest = &xml[s..];
        let skip_to = |pat: &str| rest.find(pat).map_or(xml.len(), |k| s + k + pat.len());
        if rest.starts_with("<!--") {
            i = skip_to("-->");
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            i = skip_to("]]>");
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            i = skip_to(">");
            continue;
        }
        let mut quote: Option<u8> = None;
        let mut end = None;
        for (k, &c) in b[s + 1..].iter().enumerate() {
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None => match c {
                    b'"' | b'\'' => quote = Some(c),
                    b'>' => {
                        end = Some(s + 1 + k);
                        break;
                    }
                    _ => {}
                },
            }
        }
        let Some(e) = end else { break };
        let close = rest.starts_with("</");
        let kind = if close {
            TagKind::Close
        } else if b[e - 1] == b'/' {
            TagKind::Empty
        } else {
            TagKind::Open
        };
        let ns = if close { s + 2 } else { s + 1 };
        let ne = xml[ns..e]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .map_or(e, |k| ns + k);
        out.push(Tag {
            start: s,
            end: e + 1,
            kind,
            name: (ns, ne),
        });
        i = e + 1;
    }
    out
}

/// Byte spans of the child elements of the element opened by `tags[k]`, and the start of
/// its end tag (`None` when it is not closed).
fn child_spans(tags: &[Tag], k: usize) -> (Vec<(usize, usize)>, Option<usize>) {
    let mut spans = Vec::new();
    let mut depth = 0usize;
    let mut child_start = 0;
    for t in &tags[k + 1..] {
        match t.kind {
            TagKind::Empty if depth == 0 => spans.push((t.start, t.end)),
            TagKind::Empty => {}
            TagKind::Open => {
                if depth == 0 {
                    child_start = t.start;
                }
                depth += 1;
            }
            TagKind::Close => {
                if depth == 0 {
                    return (spans, Some(t.start));
                }
                depth -= 1;
                if depth == 0 {
                    spans.push((child_start, t.end));
                }
            }
        }
    }
    (spans, None)
}

/// Value span (inside the quotes) of attribute `name` in a start tag.
fn attr_span(tag: &str, name: &str) -> Option<(usize, usize, usize)> {
    let b = tag.as_bytes();
    let mut from = 0;
    while let Some(p) = tag[from..].find(name) {
        let at = from + p;
        from = at + name.len();
        if at == 0 || !b[at - 1].is_ascii_whitespace() {
            continue;
        }
        let rest = tag[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest2 = rest.trim_start();
        let Some(q) = rest2.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let vs = tag.len() - rest2.len() + 1;
        let ve = vs + tag[vs..].find(q)?;
        // Span of the whole ` name="value"` (leading whitespace included).
        return Some((at - 1, vs, ve));
    }
    None
}

/// Unescaped value of attribute `name` (qualified, as written) in a start tag.
pub(crate) fn attr_of(tag: &str, name: &str) -> Option<String> {
    attr_span(tag, name).map(|(_, vs, ve)| unescape(&tag[vs..ve]))
}

/// The start tag with attribute `name` set to `value` (or removed for `None`).
fn set_attr(tag: &str, name: &str, value: Option<&str>) -> String {
    match (attr_span(tag, name), value) {
        (Some((_, vs, ve)), Some(v)) => format!("{}{}{}", &tag[..vs], escape(v), &tag[ve..]),
        (Some((s, _, ve)), None) => format!("{}{}", &tag[..s], &tag[ve + 1..]),
        (None, Some(v)) => {
            let cut = if tag.ends_with("/>") {
                tag.len() - 2
            } else {
                tag.len() - 1
            };
            format!("{} {name}=\"{}\"{}", &tag[..cut], escape(v), &tag[cut..])
        }
        (None, None) => tag.to_owned(),
    }
}

// ------------------------------------------------------------------ VML

/// A VML `v:shape` with its `x:ClientData`.
#[derive(Debug, Clone, Default)]
struct VmlShape {
    /// Byte span of the shape element.
    span: (usize, usize),
    /// The number of `_x0000_s<n>` (`o:spid`, else `id`).
    id: u32,
    /// `x:ClientData/@ObjectType`.
    object_type: Option<String>,
    /// `x:ClientData` children: (local name, unescaped text).
    tags: Vec<(String, String)>,
    /// Caption (`v:textbox` text).
    text: Option<String>,
}

/// `_x0000_s1025` → 1025.
fn shape_number(s: &str) -> Option<u32> {
    let digits: String = s
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    s.contains("_s").then_some(())?;
    digits.parse().ok()
}

/// Plain text of the inner markup of a `v:textbox` (line breaks at `br` and between `div`s).
fn textbox_text(inner: &str) -> String {
    let ts = tags(inner);
    let mut out = String::new();
    let mut pos = 0;
    for t in &ts {
        let seg = &inner[pos..t.start];
        if !seg.trim().is_empty() {
            out.push_str(&unescape(seg.trim_matches(|c| c == '\r' || c == '\n')));
        }
        let l = t.local(inner).to_ascii_lowercase();
        if l == "br" || (l == "div" && t.kind == TagKind::Close) {
            out.push('\n');
        }
        pos = t.end;
    }
    let seg = &inner[pos..];
    if !seg.trim().is_empty() {
        out.push_str(&unescape(seg.trim()));
    }
    out.trim().to_owned()
}

/// The shapes of a VML drawing.
fn vml_shapes(xml: &str) -> Vec<VmlShape> {
    let ts = tags(xml);
    let mut out = Vec::new();
    let mut k = 0;
    while k < ts.len() {
        let t = ts[k];
        if t.kind == TagKind::Close || t.local(xml) != "shape" {
            k += 1;
            continue;
        }
        let head = t.text(xml);
        let id = attr_of(head, "o:spid")
            .and_then(|s| shape_number(&s))
            .or_else(|| attr_of(head, "id").and_then(|s| shape_number(&s)))
            .unwrap_or(0);
        if t.kind == TagKind::Empty {
            out.push(VmlShape {
                span: (t.start, t.end),
                id,
                ..VmlShape::default()
            });
            k += 1;
            continue;
        }
        let q = t.qname(xml);
        let mut shape = VmlShape {
            id,
            ..VmlShape::default()
        };
        let mut j = k + 1;
        let mut client = false;
        let mut textbox: Option<usize> = None;
        while j < ts.len() {
            let u = ts[j];
            if u.kind == TagKind::Close && u.qname(xml) == q {
                break;
            }
            match (u.kind, u.local(xml)) {
                (TagKind::Open | TagKind::Empty, "ClientData") => {
                    shape.object_type = attr_of(u.text(xml), "ObjectType");
                    client = u.kind == TagKind::Open;
                }
                (TagKind::Close, "ClientData") => client = false,
                (TagKind::Open, "textbox") => textbox = Some(u.end),
                (TagKind::Close, "textbox") => {
                    if let Some(s) = textbox.take() {
                        shape.text = Some(textbox_text(&xml[s..u.start]));
                    }
                }
                (TagKind::Empty, name) if client => {
                    shape.tags.push((name.to_owned(), String::new()));
                }
                (TagKind::Open, name) if client => {
                    let value = ts
                        .get(j + 1)
                        .filter(|n| n.kind == TagKind::Close && n.qname(xml) == u.qname(xml))
                        .map(|n| unescape(xml[u.end..n.start].trim()))
                        .unwrap_or_default();
                    shape.tags.push((name.to_owned(), value));
                }
                _ => {}
            }
            j += 1;
        }
        let Some(close) = ts.get(j) else { break };
        shape.span = (t.start, close.end);
        out.push(shape);
        k = j + 1;
    }
    out
}

impl VmlShape {
    fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// `x:Anchor`: LeftColumn, LeftOffset, TopRow, TopOffset, RightColumn, RightOffset,
    /// BottomRow, BottomOffset (offsets in pixels).
    fn anchor(&self) -> Option<ControlAnchor> {
        let v: Vec<i64> = self
            .tag("Anchor")?
            .split(',')
            .map(|p| p.trim().parse().unwrap_or(0))
            .collect();
        if v.len() < 8 {
            return None;
        }
        let cell = |c: i64, r: i64| CellRef::new(r.max(0) as u32, c.max(0) as u32);
        Some(ControlAnchor {
            from: cell(v[0], v[2]),
            from_off: (v[1] * EMU_PER_PX, v[3] * EMU_PER_PX),
            to: cell(v[4], v[6]),
            to_off: (v[5] * EMU_PER_PX, v[7] * EMU_PER_PX),
        })
    }
}

// ------------------------------------------------------------------ reading

fn pbool(v: &str) -> bool {
    matches!(v.trim(), "1" | "true" | "True" | "t")
}

fn num<T: std::str::FromStr>(v: &str) -> Option<T> {
    v.trim().parse().ok()
}

/// Settings from the VML client data (the control properties part, if any, wins).
fn apply_vml(c: &mut FormControl, s: &VmlShape) {
    for (name, v) in &s.tags {
        match name.as_str() {
            "Checked" => {
                c.checked = match v.trim() {
                    "1" => CheckState::Checked,
                    "2" => CheckState::Mixed,
                    _ => CheckState::Unchecked,
                };
            }
            "FmlaLink" if !v.is_empty() => c.link = Some(v.clone()),
            "FmlaRange" if !v.is_empty() => c.input_range = Some(v.clone()),
            "FmlaMacro" if !v.is_empty() => c.macro_name = Some(v.clone()),
            "Sel" => c.sel = num(v).unwrap_or(0),
            "Val" => c.val = num(v).unwrap_or(0),
            "Min" => c.min = num(v).unwrap_or(0),
            "Max" => c.max = num(v).unwrap_or(100),
            "Inc" => c.inc = num(v).unwrap_or(1),
            "Page" => c.page = num(v).unwrap_or(10),
            "DropLines" => c.drop_lines = num(v).unwrap_or(8),
            // VML Part 4 §2.4 `x:SelType` / `x:MultiSel` (list box selection).
            "SelType" => c.sel_type = SelType::parse(v).unwrap_or_default(),
            "MultiSel" => c.multi_sel = multi_sel(v),
            "Horiz" => c.horiz = true,
            "NoThreeD" => c.no_three_d = true,
            "FirstButton" => c.first_button = true,
            _ => {}
        }
    }
    if let Some(t) = &s.text {
        c.text.clone_from(t);
    }
    if let Some(a) = s.anchor() {
        c.anchor = a;
    }
}

/// A `multiSel` list ("1,3,4", 1-based item numbers), ascending without duplicates.
fn multi_sel(v: &str) -> Vec<u32> {
    let mut out: Vec<u32> = v
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .filter(|&n| n > 0)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Placement from `controlPr/anchor` ([MS-XLSX] `CT_ObjectAnchor`: `moveWithCells`,
/// `sizeWithCells`, both default false).
fn placement(anchor: &Node) -> ControlPlacement {
    let flag = |n: &str| anchor.attr(n).is_some_and(pbool);
    match (flag("moveWithCells"), flag("sizeWithCells")) {
        (_, true) => ControlPlacement::MoveAndSize,
        (true, false) => ControlPlacement::Move,
        (false, false) => ControlPlacement::Fixed,
    }
}

/// `moveWithCells` / `sizeWithCells` attribute values of a placement.
fn placement_attrs(p: ControlPlacement) -> (Option<&'static str>, Option<&'static str>) {
    match p {
        ControlPlacement::Move => (Some("1"), None),
        ControlPlacement::MoveAndSize => (Some("1"), Some("1")),
        ControlPlacement::Fixed => (None, None),
    }
}

/// §20.5.2.33 `twoCellAnchor/@editAs` of the DrawingML twin for a placement.
fn edit_as(p: ControlPlacement) -> &'static str {
    match p {
        ControlPlacement::Move => "oneCell",
        ControlPlacement::MoveAndSize => "twoCell",
        ControlPlacement::Fixed => "absolute",
    }
}

/// Settings of a `formControlPr` element.
fn apply_ctrl_prop(c: &mut FormControl, n: &Node) {
    if let Some(k) = n
        .attr("objectType")
        .and_then(FormControlKind::from_object_type)
    {
        c.kind = k;
    }
    if let Some(v) = n.attr("checked") {
        c.checked = match v {
            "Checked" => CheckState::Checked,
            "Mixed" => CheckState::Mixed,
            _ => CheckState::Unchecked,
        };
    }
    if let Some(v) = n.attr("fmlaLink").filter(|v| !v.is_empty()) {
        c.link = Some(v.to_owned());
    }
    if let Some(v) = n.attr("fmlaRange").filter(|v| !v.is_empty()) {
        c.input_range = Some(v.to_owned());
    }
    let int = |name: &str| n.attr(name).and_then(num::<i32>);
    if let Some(v) = n.attr("sel").and_then(num) {
        c.sel = v;
    }
    if let Some(v) = int("val") {
        c.val = v;
    }
    if let Some(v) = int("min") {
        c.min = v;
    }
    if let Some(v) = int("max") {
        c.max = v;
    }
    if let Some(v) = int("inc") {
        c.inc = v;
    }
    if let Some(v) = int("page") {
        c.page = v;
    }
    if let Some(v) = n.attr("dropLines").and_then(num) {
        c.drop_lines = v;
    }
    // [MS-XLSX] `CT_FormControlPr` `seltype` (`ST_SelType`) and `multiSel`.
    if let Some(v) = n
        .attr("seltype")
        .or_else(|| n.attr("selType"))
        .and_then(SelType::parse)
    {
        c.sel_type = v;
    }
    if let Some(v) = n.attr("multiSel") {
        c.multi_sel = multi_sel(v);
    }
    if let Some(v) = n.attr("horiz") {
        c.horiz = pbool(v);
    }
    if let Some(v) = n.attr("noThreeD") {
        c.no_three_d = pbool(v);
    }
    if let Some(v) = n.attr("firstButton") {
        c.first_button = pbool(v);
    }
    if let Some(list) = n.child("itemLst") {
        c.items = list
            .children
            .iter()
            .filter(|i| i.name == "item")
            .map(|i| i.attr("val").unwrap_or_default().to_owned())
            .collect();
    }
}

/// `xdr:col` / `colOff` / `row` / `rowOff` of a `from` / `to` marker.
fn marker(n: &Node) -> (CellRef, (i64, i64)) {
    let int = |name: &str| {
        n.child(name)
            .and_then(|c| c.text.trim().parse::<i64>().ok())
            .unwrap_or(0)
    };
    (
        CellRef::new(int("row").max(0) as u32, int("col").max(0) as u32),
        (int("colOff"), int("rowOff")),
    )
}

/// One `control` entry of the worksheet `controls` element.
#[derive(Debug, Clone, Default)]
struct Entry {
    name: Option<String>,
    rid: String,
    macro_name: Option<String>,
    anchor: Option<ControlAnchor>,
    placement: Option<ControlPlacement>,
    image_rid: Option<String>,
}

fn descendants<'a>(n: &'a Node, name: &str, out: &mut Vec<&'a Node>) {
    for c in &n.children {
        if c.name == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

/// `control` entries by shape id (the `mc:Choice` one, with `controlPr`, wins).
fn entries(unknown: &[RawXml]) -> HashMap<u32, Entry> {
    let mut out: HashMap<u32, Entry> = HashMap::new();
    for u in unknown.iter().filter(|u| u.xml.contains("controls")) {
        let Some(dom) = parse_dom(u.xml.as_bytes(), "controls") else {
            continue;
        };
        let mut found = Vec::new();
        if dom.name == "control" {
            found.push(&dom);
        }
        descendants(&dom, "control", &mut found);
        for c in found {
            let Some(id) = c.attr("shapeId").and_then(num::<u32>) else {
                continue;
            };
            let pr = c.child("controlPr");
            if out.get(&id).is_some_and(|_| pr.is_none()) {
                continue;
            }
            let placement = pr.and_then(|p| p.child("anchor")).map(placement);
            let anchor = pr.and_then(|p| p.child("anchor")).and_then(|a| {
                let (from, from_off) = marker(a.child("from")?);
                let (to, to_off) = marker(a.child("to")?);
                Some(ControlAnchor {
                    from,
                    from_off,
                    to,
                    to_off,
                })
            });
            out.insert(
                id,
                Entry {
                    name: c.attr("name").map(str::to_owned),
                    rid: c.attr("id").unwrap_or_default().to_owned(),
                    macro_name: pr
                        .and_then(|p| p.attr("macro"))
                        .filter(|m| !m.is_empty())
                        .map(str::to_owned),
                    anchor,
                    placement,
                    image_rid: pr.and_then(|p| p.attr("id")).map(str::to_owned),
                },
            );
        }
    }
    out
}

/// The form controls and ActiveX controls of a sheet as stored in the package: `rels` are
/// the sheet's relationships (absolute targets), `get` reads a part.
pub(crate) fn file_controls(
    rels: &[OpaqueRel],
    unknown: &[RawXml],
    get: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Vec<FormControl> {
    let rel = |id: &str| rels.iter().find(|r| r.id == id && !r.external);
    let entries = entries(unknown);
    let mut out: Vec<FormControl> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let build = |id: u32, e: Option<&Entry>, shape: Option<(&VmlShape, &str)>| {
        let target = e.and_then(|e| rel(&e.rid));
        let activex = target.is_some_and(|r| r.rel_type == REL_ACTIVEX);
        let ctrl_prop = target.filter(|r| r.rel_type == REL_CTRL_PROP);
        let prop_dom = ctrl_prop
            .and_then(|r| get(&r.target).map(|d| (r, d)))
            .and_then(|(r, d)| parse_dom(&d, &r.target))
            .filter(|n| n.name == "formControlPr");
        let vml_kind = shape
            .and_then(|(s, _)| s.object_type.as_deref())
            .and_then(FormControlKind::from_object_type);
        let prop_kind = prop_dom
            .as_ref()
            .and_then(|n| n.attr("objectType"))
            .and_then(FormControlKind::from_object_type);
        let kind = if activex {
            FormControlKind::ActiveX
        } else {
            prop_kind.or(vml_kind)?
        };
        let mut c = FormControl {
            kind,
            shape_id: id,
            ..FormControl::default()
        };
        if let Some((s, part)) = shape {
            if !activex {
                apply_vml(&mut c, s);
            } else if let Some(a) = s.anchor() {
                c.anchor = a;
            }
            c.vml_part = part.to_owned();
        }
        if let Some(n) = &prop_dom {
            apply_ctrl_prop(&mut c, n);
            c.kind = kind;
        }
        if let Some(r) = ctrl_prop {
            c.ctrl_prop_part.clone_from(&r.target);
        }
        if let Some(e) = e {
            if let Some(n) = &e.name {
                c.name.clone_from(n);
            }
            if let Some(a) = e.anchor {
                c.anchor = a;
            }
            // Without a `controls` anchor (VML only) the control keeps the default
            // "move with cells".
            if let Some(p) = e.placement {
                c.placement = p;
            }
            if e.macro_name.is_some() {
                c.macro_name.clone_from(&e.macro_name);
            }
            c.image_part = e
                .image_rid
                .as_deref()
                .and_then(rel)
                .map(|r| r.target.clone());
        }
        if activex {
            c.activex_part = target.map(|r| r.target.clone()).unwrap_or_default();
            if c.text.is_empty() {
                c.text.clone_from(&c.name);
            }
        }
        Some(c)
    };
    for r in rels.iter().filter(|r| r.rel_type == REL_VML && !r.external) {
        let Some(data) = get(&r.target) else { continue };
        let xml = String::from_utf8_lossy(&data);
        for s in vml_shapes(&xml) {
            if s.id == 0 || seen.contains(&s.id) {
                continue;
            }
            if let Some(c) = build(s.id, entries.get(&s.id), Some((&s, &r.target))) {
                seen.insert(s.id);
                out.push(c);
            }
        }
    }
    // Entries without a VML shape (control properties or ActiveX only).
    let mut rest: Vec<(&u32, &Entry)> = entries
        .iter()
        .filter(|(id, _)| !seen.contains(id))
        .collect();
    rest.sort_by_key(|(id, _)| **id);
    for (id, e) in rest {
        if let Some(c) = build(*id, Some(e), None) {
            out.push(c);
        }
    }
    out
}

// ------------------------------------------------------------------ writing

/// `$B$2`-style marker text of the anchor.
fn vml_anchor(a: &ControlAnchor) -> String {
    format!(
        "{}, {}, {}, {}, {}, {}, {}, {}",
        a.from.col,
        a.from_off.0 / EMU_PER_PX,
        a.from.row,
        a.from_off.1 / EMU_PER_PX,
        a.to.col,
        a.to_off.0 / EMU_PER_PX,
        a.to.row,
        a.to_off.1 / EMU_PER_PX
    )
}

fn checked_vml(c: CheckState) -> Option<&'static str> {
    match c {
        CheckState::Unchecked => None,
        CheckState::Checked => Some("1"),
        CheckState::Mixed => Some("2"),
    }
}

fn checked_attr(c: CheckState) -> Option<&'static str> {
    match c {
        CheckState::Unchecked => None,
        CheckState::Checked => Some("Checked"),
        CheckState::Mixed => Some("Mixed"),
    }
}

/// The settings a kind keeps: (VML tag / `formControlPr` attribute name, value) — `None`
/// removes the setting, `Some("")` is a VML flag element.
struct Setting {
    vml: &'static str,
    attr: &'static str,
    value: Option<String>,
    flag: bool,
}

/// A setting's VML text: `x:SelType` is written `Single` / `Multi` / `Extend`, the
/// `seltype` attribute `single` / `multi` / `extended` ([MS-XLSX] `ST_SelType`).
fn vml_value(st: &Setting, v: &str) -> String {
    match (st.vml, SelType::parse(v)) {
        ("SelType", Some(t)) => t.vml_str().to_owned(),
        _ => v.to_owned(),
    }
}

fn settings(c: &FormControl) -> Vec<Setting> {
    use FormControlKind as K;
    let s = |vml, attr, value: Option<String>| Setting {
        vml,
        attr,
        value,
        flag: false,
    };
    let f = |vml, attr, on: bool| Setting {
        vml,
        attr,
        value: on.then(|| "1".to_owned()),
        flag: true,
    };
    let mut out = vec![
        s("FmlaLink", "fmlaLink", c.link.clone()),
        f("NoThreeD", "noThreeD", c.no_three_d),
    ];
    match c.kind {
        K::CheckBox | K::OptionButton => {
            out.push(s(
                "Checked",
                "checked",
                checked_vml(c.checked).map(str::to_owned),
            ));
            if c.kind == K::OptionButton {
                out.push(f("FirstButton", "firstButton", c.first_button));
            }
        }
        K::ComboBox | K::ListBox => {
            out.push(s("FmlaRange", "fmlaRange", c.input_range.clone()));
            out.push(s("Sel", "sel", (c.sel > 0).then(|| c.sel.to_string())));
            if c.kind == K::ComboBox {
                out.push(s("DropLines", "dropLines", Some(c.drop_lines.to_string())));
            } else {
                let multi = c.sel_type != SelType::Single;
                out.push(s(
                    "SelType",
                    "seltype",
                    multi.then(|| c.sel_type.as_str().to_owned()),
                ));
                let list: Vec<String> = c.multi_sel.iter().map(u32::to_string).collect();
                out.push(s(
                    "MultiSel",
                    "multiSel",
                    (multi && !list.is_empty()).then(|| list.join(",")),
                ));
            }
        }
        K::ScrollBar | K::Spinner => {
            out.push(s("Val", "val", Some(c.val.to_string())));
            out.push(s("Min", "min", Some(c.min.to_string())));
            out.push(s("Max", "max", Some(c.max.to_string())));
            out.push(s("Inc", "inc", Some(c.inc.to_string())));
            if c.kind == K::ScrollBar {
                out.push(s("Page", "page", Some(c.page.to_string())));
            }
            out.push(f("Horiz", "horiz", c.horiz));
        }
        K::Button | K::GroupBox | K::Label | K::ActiveX => {}
    }
    out
}

/// A new control properties part ([MS-XLSX] `formControlPr`).
fn ctrl_prop_xml(c: &FormControl) -> String {
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<formControlPr xmlns=\"{NS_X14}\" objectType=\"{}\"",
        c.kind.object_type().unwrap_or("Button")
    );
    for st in settings(c) {
        let v = if st.attr == "checked" {
            checked_attr(c.checked).map(str::to_owned)
        } else {
            st.value
        };
        if let Some(v) = v {
            let _ = write!(out, " {}=\"{}\"", st.attr, escape(&v));
        }
    }
    if matches!(
        c.kind,
        FormControlKind::Button
            | FormControlKind::CheckBox
            | FormControlKind::OptionButton
            | FormControlKind::GroupBox
            | FormControlKind::Label
    ) {
        out.push_str(" lockText=\"1\"");
    }
    if c.items.is_empty() {
        out.push_str("/>");
    } else {
        out.push_str("><itemLst>");
        for i in &c.items {
            let _ = write!(out, "<item val=\"{}\"/>", escape(i));
        }
        out.push_str("</itemLst></formControlPr>");
    }
    out
}

/// The control properties part with the model's settings patched into its root element.
fn patch_ctrl_prop(xml: &str, c: &FormControl) -> Option<String> {
    let ts = tags(xml);
    let root = ts.iter().find(|t| t.kind != TagKind::Close)?;
    let mut head = root.text(xml).to_owned();
    for st in settings(c) {
        let v = if st.attr == "checked" {
            checked_attr(c.checked).map(str::to_owned)
        } else {
            st.value
        };
        head = set_attr(&head, st.attr, v.as_deref());
    }
    Some(format!("{}{head}{}", &xml[..root.start], &xml[root.end..]))
}

/// The VML shape with the model's settings patched into its client data and caption.
fn patch_vml_shape(shape_xml: &str, c: &FormControl, old_text: Option<&str>) -> String {
    let mut xml = shape_xml.to_owned();
    for st in settings(c) {
        let ts = tags(&xml);
        let Some(open) = ts.iter().position(|t| t.local(&xml) == "ClientData") else {
            break;
        };
        let prefix = {
            let q = ts[open].qname(&xml);
            q.rsplit_once(':')
                .map_or(String::new(), |(p, _)| format!("{p}:"))
        };
        if ts[open].kind != TagKind::Open {
            break;
        }
        let (kids, close) = child_spans(&ts, open);
        let Some(close) = close else { break };
        let existing = kids.into_iter().find(|&(s, e)| {
            let inner = tags(&xml[s..e]);
            inner
                .first()
                .is_some_and(|t| t.local(&xml[s..e]).eq_ignore_ascii_case(st.vml))
        });
        let new = st.value.as_ref().map(|v| {
            if st.flag {
                format!("<{prefix}{}/>", st.vml)
            } else {
                format!(
                    "<{prefix}{0}>{1}</{prefix}{0}>",
                    st.vml,
                    escape(&vml_value(&st, v))
                )
            }
        });
        xml = match (existing, new) {
            (Some((s, e)), n) => format!("{}{}{}", &xml[..s], n.unwrap_or_default(), &xml[e..]),
            (None, Some(n)) => format!("{}{n}{}", &xml[..close], &xml[close..]),
            (None, None) => xml,
        };
    }
    if old_text.is_some_and(|t| t != c.text) {
        let ts = tags(&xml);
        let open = ts
            .iter()
            .find(|t| t.kind == TagKind::Open && t.local(&xml) == "textbox");
        let close = ts
            .iter()
            .find(|t| t.kind == TagKind::Close && t.local(&xml) == "textbox");
        if let (Some(o), Some(cl)) = (open, close) {
            xml = format!(
                "{}{}{}",
                &xml[..o.end],
                textbox_div(&c.text, c.kind),
                &xml[cl.start..]
            );
        }
    }
    xml
}

fn textbox_div(text: &str, kind: FormControlKind) -> String {
    let align = if kind == FormControlKind::Button {
        "center"
    } else {
        "left"
    };
    let lines: Vec<String> = text.lines().map(escape).collect();
    format!(
        "<div style=\"text-align:{align}\"><font face=\"Calibri\" size=\"220\" color=\"auto\">{}</font></div>",
        lines.join("<br/>")
    )
}

/// A new VML shape for control `c` with shape number `id`.
fn vml_shape_xml(c: &FormControl, id: u32, z: usize) -> String {
    use FormControlKind as K;
    let a = &c.anchor;
    // Approximate size (64 px columns, 20 px rows); placement comes from `x:Anchor`.
    let w = ((i64::from(a.to.col) - i64::from(a.from.col)) * 64 * EMU_PER_PX + a.to_off.0
        - a.from_off.0)
        .max(EMU_PER_PX)
        / 12_700;
    let h = ((i64::from(a.to.row) - i64::from(a.from.row)) * 20 * EMU_PER_PX + a.to_off.1
        - a.from_off.1)
        .max(EMU_PER_PX)
        / 12_700;
    let look = if c.kind == K::Button {
        " o:button=\"t\" fillcolor=\"#F0F0F0\" strokecolor=\"#000000\""
    } else {
        " filled=\"f\" stroked=\"f\""
    };
    let mut out = format!(
        "<v:shape id=\"_x0000_s{id}\" type=\"#_x0000_t201\" style=\"position:absolute;margin-left:0;margin-top:0;width:{w}pt;height:{h}pt;z-index:{z};mso-wrap-style:tight\"{look} o:insetmode=\"auto\">"
    );
    if matches!(
        c.kind,
        K::Button | K::CheckBox | K::OptionButton | K::GroupBox | K::Label
    ) {
        let _ = write!(
            out,
            "<v:textbox style=\"mso-direction-alt:auto\" o:singleclick=\"f\">{}</v:textbox>",
            textbox_div(&c.text, c.kind)
        );
    }
    let _ = write!(
        out,
        "<x:ClientData ObjectType=\"{}\"><x:Anchor>{}</x:Anchor>",
        c.kind.vml_object_type().unwrap_or("Button"),
        vml_anchor(a)
    );
    if matches!(c.kind, K::CheckBox | K::OptionButton | K::Button | K::Label) {
        out.push_str("<x:AutoFill>False</x:AutoFill><x:AutoLine>False</x:AutoLine>");
    }
    if let Some(m) = &c.macro_name {
        let _ = write!(out, "<x:FmlaMacro>{}</x:FmlaMacro>", escape(m));
    }
    for st in settings(c) {
        match (&st.value, st.flag) {
            (Some(_), true) => {
                let _ = write!(out, "<x:{}/>", st.vml);
            }
            (Some(v), false) => {
                let _ = write!(
                    out,
                    "<x:{0}>{1}</x:{0}>",
                    st.vml,
                    escape(&vml_value(&st, v))
                );
            }
            (None, _) => {}
        }
    }
    out.push_str("</x:ClientData></v:shape>");
    out
}

/// `controlPr/anchor` marker.
fn marker_xml(tag: &str, c: CellRef, off: (i64, i64)) -> String {
    format!(
        "<{tag}><xdr:col>{}</xdr:col><xdr:colOff>{}</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>{}</xdr:rowOff></{tag}>",
        c.col, off.0, c.row, off.1
    )
}

/// One `controls` entry for a new control (§18.3.1.19 `control` with [MS-XLSX] `controlPr`).
fn entry_xml(c: &FormControl, id: u32, rid: &str) -> String {
    let a = &c.anchor;
    let macro_attr = c
        .macro_name
        .as_ref()
        .map(|m| format!(" macro=\"{}\"", escape(m)))
        .unwrap_or_default();
    let (mv, sz) = placement_attrs(c.placement);
    let mut anchor_attrs = String::new();
    if let Some(v) = mv {
        let _ = write!(anchor_attrs, " moveWithCells=\"{v}\"");
    }
    if let Some(v) = sz {
        let _ = write!(anchor_attrs, " sizeWithCells=\"{v}\"");
    }
    format!(
        "<mc:AlternateContent xmlns:mc=\"{NS_MC}\"><mc:Choice Requires=\"x14\"><control shapeId=\"{id}\" r:id=\"{rid}\" name=\"{}\"><controlPr defaultSize=\"0\" autoFill=\"0\" autoLine=\"0\" autoPict=\"0\"{macro_attr}><anchor{anchor_attrs} xmlns:xdr=\"{NS_XDR}\">{}{}</anchor></controlPr></control></mc:Choice></mc:AlternateContent>",
        escape(&c.name),
        marker_xml("from", a.from, a.from_off),
        marker_xml("to", a.to, a.to_off)
    )
}

/// The `shapetype` new VML shapes refer to (VML `o:spt` 201, a host control).
const SHAPETYPE_201: &str = "<v:shapetype id=\"_x0000_t201\" coordsize=\"21600,21600\" o:spt=\"201\" path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/><v:path shadowok=\"f\" o:extrusionok=\"f\" strokeok=\"f\" fillok=\"f\" o:connecttype=\"rect\"/><o:lock v:ext=\"edit\" shapetype=\"t\"/></v:shapetype>";

/// A new VML drawing holding `shapes`, its shape ids in block `block` (`o:idmap`).
fn vml_xml(block: u32, shapes: &str) -> String {
    format!(
        "<xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\"><o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"{block}\"/></o:shapelayout>{SHAPETYPE_201}{shapes}</xml>"
    )
}

/// First VML shape-id block (`o:idmap/@data`, 1024 ids each) not used by any VML part.
pub(crate) fn first_free_block(parts: &[OpaquePart]) -> u32 {
    let mut max = 0u32;
    for p in parts.iter().filter(|p| {
        p.name.to_ascii_lowercase().ends_with(".vml") || p.content_type.as_deref() == Some(CT_VML)
    }) {
        let xml = String::from_utf8_lossy(&p.data);
        for s in vml_shapes(&xml) {
            max = max.max(s.id / 1024);
        }
        for t in tags(&xml).iter().filter(|t| t.local(&xml) == "idmap") {
            if let Some(d) = attr_of(t.text(&xml), "data") {
                for n in d.split(',').filter_map(|n| n.trim().parse::<u32>().ok()) {
                    max = max.max(n);
                }
            }
        }
    }
    max + 1
}

/// Everything [`write_controls`] needs from the package writer.
pub(crate) struct WriteCtx<'a> {
    /// Absolute name of the sheet part.
    pub sheet_path: &'a str,
    /// Preserved parts.
    pub opaque: &'a [OpaquePart],
    /// Rewritten preserved parts.
    pub overrides: &'a mut HashMap<String, Vec<u8>>,
    /// Preserved parts not written.
    pub dropped: &'a mut HashSet<String>,
    /// The sheet's relationships.
    pub rels: &'a mut Vec<Relationship>,
    /// New parts.
    pub out_parts: &'a mut Vec<(String, Option<String>, Vec<u8>)>,
    /// Next free VML shape-id block of the workbook.
    pub block: &'a mut u32,
}

impl WriteCtx<'_> {
    fn current(&self, name: &str) -> Option<Vec<u8>> {
        self.overrides.get(name).cloned().or_else(|| {
            self.opaque
                .iter()
                .find(|p| p.name == name)
                .map(|p| p.data.clone())
        })
    }
}

/// Removes the top-level children of the root element of `xml` that contain `needle`.
fn remove_top_level(xml: &str, needle: &str) -> Option<String> {
    let ts = tags(xml);
    let root = ts.iter().position(|t| t.kind == TagKind::Open)?;
    let (kids, _) = child_spans(&ts, root);
    let mut out = xml.to_owned();
    let mut changed = false;
    for (s, e) in kids.into_iter().rev() {
        if out[s..e].contains(needle) {
            out.replace_range(s..e, "");
            changed = true;
        }
    }
    changed.then_some(out)
}

/// `xml` with the `col` / `colOff` / `row` / `rowOff` texts of its first `tag` element
/// (local name, ECMA-376 Part 1 §20.5.2.15 `CT_Marker`) set to `cell` and `off` (EMU).
fn patch_marker(xml: &str, tag: &str, cell: CellRef, off: (i64, i64)) -> Option<String> {
    let ts = tags(xml);
    let k = ts
        .iter()
        .position(|t| t.kind == TagKind::Open && t.local(xml) == tag)?;
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for (j, t) in ts.iter().enumerate().skip(k + 1) {
        if t.kind == TagKind::Close && t.local(xml) == tag {
            break;
        }
        if t.kind != TagKind::Open {
            continue;
        }
        let v = match t.local(xml) {
            "col" => cell.col.to_string(),
            "colOff" => off.0.to_string(),
            "row" => cell.row.to_string(),
            "rowOff" => off.1.to_string(),
            _ => continue,
        };
        if let Some(close) = ts.get(j + 1).filter(|n| n.kind == TagKind::Close) {
            edits.push((t.end, close.start, v));
        }
    }
    let mut out = xml.to_owned();
    for (s, e, v) in edits.into_iter().rev() {
        out.replace_range(s..e, &v);
    }
    Some(out)
}

/// `xml` (a `controlPr/anchor` entry or a drawing anchor) with its `from` / `to` markers set.
fn patch_markers(xml: &str, a: &ControlAnchor) -> Option<String> {
    let x = patch_marker(xml, "from", a.from, a.from_off)?;
    patch_marker(&x, "to", a.to, a.to_off)
}

/// Writes the anchors of the file's controls whose placement no longer matches the model
/// (rows / columns inserted or deleted, or the control moved): the `controls` entry
/// ([MS-XLSX] `controlPr/anchor`, EMU), the VML shape (`x:Anchor`, pixels) and the
/// DrawingML twin of the shape ([MS-ODRAWXML] `a14:compatExt/@spid`, §20.5.2.33
/// `twoCellAnchor`). Everything else stays as read.
fn sync_control_anchors(
    ws: &Worksheet,
    unknown: &mut [RawXml],
    ctx: &mut WriteCtx<'_>,
    in_file: &[FormControl],
) {
    for c in ws.form_controls.iter().filter(|c| !c.is_new()) {
        let Some(f) = in_file.iter().find(|f| f.shape_id == c.shape_id) else {
            continue;
        };
        if f.anchor == c.anchor && f.placement == c.placement {
            continue;
        }
        let placed = f.placement != c.placement;
        // `controls` entries (§18.3.1.19 `control` with `controlPr/anchor`).
        for u in unknown.iter_mut() {
            let ts = tags(&u.xml);
            let mut spans = Vec::new();
            for (k, t) in ts.iter().enumerate() {
                if t.kind == TagKind::Open && t.local(&u.xml) == "controls" {
                    spans.extend(child_spans(&ts, k).0);
                }
            }
            spans.sort_unstable();
            let mut out = u.xml.clone();
            for (s, e) in spans.into_iter().rev() {
                let entry = &u.xml[s..e];
                if entry_shape_id(entry) != Some(c.shape_id) || !entry.contains("anchor") {
                    continue;
                }
                let mut p = patch_markers(entry, &c.anchor).unwrap_or_else(|| entry.to_owned());
                if placed {
                    p = patch_placement(&p, c.placement);
                }
                out.replace_range(s..e, &p);
            }
            u.xml = out;
        }
        // VML shape `x:Anchor`.
        if !c.vml_part.is_empty() {
            if let Some(xml) = ctx
                .current(&c.vml_part)
                .and_then(|d| String::from_utf8(d).ok())
            {
                let shape = vml_shapes(&xml).into_iter().find(|s| s.id == c.shape_id);
                let want = vml_anchor(&c.anchor);
                if let Some(s) =
                    shape.filter(|s| s.anchor().map(|a| vml_anchor(&a)) != Some(want.clone()))
                {
                    let sx = &xml[s.span.0..s.span.1];
                    let ts = tags(sx);
                    let open = ts
                        .iter()
                        .position(|t| t.kind == TagKind::Open && t.local(sx) == "Anchor");
                    if let Some(k) = open {
                        if let Some(close) = ts.get(k + 1).filter(|t| t.kind == TagKind::Close) {
                            let (a, b) = (s.span.0 + ts[k].end, s.span.0 + close.start);
                            let out = format!("{}{want}{}", &xml[..a], &xml[b..]);
                            ctx.overrides.insert(c.vml_part.clone(), out.into_bytes());
                        }
                    }
                }
            }
        }
        // The DrawingML twin in the sheet's drawing part.
        let needle = format!("spid=\"_x0000_s{}\"", c.shape_id);
        for r in ws
            .rels
            .iter()
            .filter(|r| !r.external && r.rel_type.ends_with("/drawing"))
        {
            let Some(xml) = ctx
                .current(&r.target)
                .and_then(|d| String::from_utf8(d).ok())
            else {
                continue;
            };
            let ts = tags(&xml);
            let Some(root) = ts.iter().position(|t| t.kind == TagKind::Open) else {
                continue;
            };
            let (kids, _) = child_spans(&ts, root);
            let Some((s, e)) = kids.into_iter().find(|&(s, e)| xml[s..e].contains(&needle)) else {
                continue;
            };
            if let Some(mut p) = patch_markers(&xml[s..e], &c.anchor) {
                if placed {
                    // The twin is (an `mc:AlternateContent` around) a `twoCellAnchor`.
                    let ts = tags(&p);
                    if let Some(t) = ts
                        .iter()
                        .find(|t| t.kind != TagKind::Close && t.local(&p) == "twoCellAnchor")
                    {
                        let head = set_attr(t.text(&p), "editAs", Some(edit_as(c.placement)));
                        p = format!("{}{head}{}", &p[..t.start], &p[t.end..]);
                    }
                }
                let out = format!("{}{p}{}", &xml[..s], &xml[e..]);
                ctx.overrides.insert(r.target.clone(), out.into_bytes());
            }
        }
    }
}

/// A `controls` entry with the `moveWithCells` / `sizeWithCells` of its `controlPr/anchor`
/// set for `p` ([MS-XLSX] `CT_ObjectAnchor`).
fn patch_placement(entry: &str, p: ControlPlacement) -> String {
    let ts = tags(entry);
    let Some(t) = ts
        .iter()
        .find(|t| t.kind != TagKind::Close && t.local(entry) == "anchor")
    else {
        return entry.to_owned();
    };
    let (mv, sz) = placement_attrs(p);
    let head = set_attr(t.text(entry), "moveWithCells", mv);
    let head = set_attr(&head, "sizeWithCells", sz);
    format!("{}{head}{}", &entry[..t.start], &entry[t.end..])
}

/// Shape id of a `controls` entry span.
fn entry_shape_id(entry: &str) -> Option<u32> {
    let ts = tags(entry);
    ts.iter()
        .find(|t| t.kind != TagKind::Close && t.local(entry) == "control")
        .and_then(|t| attr_of(t.text(entry), "shapeId"))
        .and_then(|v| num(&v))
}

fn entry_rid(entry: &str) -> Option<String> {
    let ts = tags(entry);
    ts.iter()
        .find(|t| t.kind != TagKind::Close && t.local(entry) == "control")
        .and_then(|t| attr_of(t.text(entry), "r:id"))
}

/// Writes the sheet's control changes; returns the sheet's unknown children to write
/// (with the `controls` block and `legacyDrawing` added, changed or removed as needed).
pub(crate) fn write_controls(
    ws: &Worksheet,
    unknown: Vec<RawXml>,
    ctx: &mut WriteCtx<'_>,
    fresh: &mut dyn FnMut(&str, &str) -> String,
    alloc_rel: &mut dyn FnMut() -> String,
) -> Vec<RawXml> {
    let mut unknown = unknown;
    let in_file = {
        let get = |n: &str| ctx.current(n);
        file_controls(&ws.rels, &unknown, &get)
    };
    let kept: HashSet<u32> = ws
        .form_controls
        .iter()
        .filter(|c| !c.is_new())
        .map(|c| c.shape_id)
        .collect();
    let removed: Vec<&FormControl> = in_file
        .iter()
        .filter(|c| c.kind != FormControlKind::ActiveX && !kept.contains(&c.shape_id))
        .collect();
    let new: Vec<&FormControl> = ws
        .form_controls
        .iter()
        .filter(|c| c.is_new() && c.kind != FormControlKind::ActiveX)
        .collect();
    let dirty: Vec<&FormControl> = ws
        .form_controls
        .iter()
        .filter(|c| c.dirty && !c.is_new() && c.kind != FormControlKind::ActiveX)
        .collect();
    // Controls moved by inserted / deleted rows and columns (or moved by the user).
    sync_control_anchors(ws, &mut unknown, ctx, &in_file);
    if removed.is_empty() && new.is_empty() && dirty.is_empty() {
        return unknown;
    }
    let block_at = unknown.iter().position(|u| {
        let ts = tags(&u.xml);
        ts.iter()
            .any(|t| t.kind == TagKind::Open && t.local(&u.xml) == "controls")
    });

    // Changed controls: patch their control properties part and VML shape.
    for c in &dirty {
        if !c.ctrl_prop_part.is_empty() {
            if let Some(xml) = ctx
                .current(&c.ctrl_prop_part)
                .and_then(|d| String::from_utf8(d).ok())
            {
                if let Some(p) = patch_ctrl_prop(&xml, c) {
                    ctx.overrides
                        .insert(c.ctrl_prop_part.clone(), p.into_bytes());
                }
            }
        }
        if !c.vml_part.is_empty() {
            if let Some(xml) = ctx
                .current(&c.vml_part)
                .and_then(|d| String::from_utf8(d).ok())
            {
                if let Some(s) = vml_shapes(&xml).into_iter().find(|s| s.id == c.shape_id) {
                    let patched = patch_vml_shape(&xml[s.span.0..s.span.1], c, s.text.as_deref());
                    let out = format!("{}{patched}{}", &xml[..s.span.0], &xml[s.span.1..]);
                    ctx.overrides.insert(c.vml_part.clone(), out.into_bytes());
                }
            }
        }
    }

    // Removed controls: `controls` entry, relationship, part and shapes.
    if !removed.is_empty() {
        let gone: HashSet<u32> = removed.iter().map(|c| c.shape_id).collect();
        if let Some(bi) = block_at {
            let xml = unknown[bi].xml.clone();
            let ts = tags(&xml);
            let mut spans = Vec::new();
            for (k, t) in ts.iter().enumerate() {
                if t.kind == TagKind::Open && t.local(&xml) == "controls" {
                    spans.extend(child_spans(&ts, k).0);
                }
            }
            spans.sort_unstable();
            let mut out = xml.clone();
            for (s, e) in spans.into_iter().rev() {
                let entry = &xml[s..e];
                if entry_shape_id(entry).is_some_and(|id| gone.contains(&id)) {
                    if let Some(rid) = entry_rid(entry) {
                        ctx.rels.retain(|r| r.id != rid);
                    }
                    out.replace_range(s..e, "");
                }
            }
            unknown[bi].xml = out;
        }
        for c in &removed {
            if !c.ctrl_prop_part.is_empty() {
                ctx.dropped.insert(c.ctrl_prop_part.clone());
            }
            let spid = format!("_x0000_s{}\"", c.shape_id);
            if !c.vml_part.is_empty() {
                if let Some(xml) = ctx
                    .current(&c.vml_part)
                    .and_then(|d| String::from_utf8(d).ok())
                {
                    if let Some(s) = vml_shapes(&xml).into_iter().find(|s| s.id == c.shape_id) {
                        let out = format!("{}{}", &xml[..s.span.0], &xml[s.span.1..]);
                        ctx.overrides.insert(c.vml_part.clone(), out.into_bytes());
                    }
                }
            }
            // The DrawingML twin of the shape ([MS-ODRAWXML] `a14:compatExt/@spid`).
            for r in ws
                .rels
                .iter()
                .filter(|r| !r.external && r.rel_type.ends_with("/drawing"))
            {
                if let Some(xml) = ctx
                    .current(&r.target)
                    .and_then(|d| String::from_utf8(d).ok())
                {
                    if let Some(out) = remove_top_level(&xml, &format!("spid=\"{spid}")) {
                        ctx.overrides.insert(r.target.clone(), out.into_bytes());
                    }
                }
            }
        }
    }

    // New controls: control properties parts, VML shapes and `controls` entries.
    if !new.is_empty() {
        // The sheet's legacy drawing (`legacyDrawing/@r:id`), if any.
        let legacy = unknown.iter().find_map(|u| {
            let ts = tags(&u.xml);
            let t = ts.first().filter(|t| t.local(&u.xml) == "legacyDrawing")?;
            attr_of(t.text(&u.xml), "r:id")
        });
        let vml = legacy
            .as_ref()
            .and_then(|id| {
                ws.rels
                    .iter()
                    .find(|r| &r.id == id && r.rel_type == REL_VML)
            })
            .map(|r| r.target.clone())
            .filter(|t| ctx.current(t).is_some());
        let (vml_path, mut next_id, base) = if let Some(p) = &vml {
            let data = ctx.current(p).unwrap_or_default();
            let xml = String::from_utf8_lossy(&data).into_owned();
            let max = vml_shapes(&xml).iter().map(|s| s.id).max().unwrap_or(0);
            let block = tags(&xml)
                .iter()
                .find(|t| t.local(&xml) == "idmap")
                .and_then(|t| attr_of(t.text(&xml), "data"))
                .and_then(|d| {
                    d.split(',')
                        .next()
                        .and_then(|n| n.trim().parse::<u32>().ok())
                })
                .unwrap_or(*ctx.block);
            (p.clone(), max.max(block * 1024) + 1, None)
        } else {
            let block = *ctx.block;
            *ctx.block += 1;
            (
                fresh("/xl/drawings/vmlDrawing", "vml"),
                block * 1024 + 1,
                Some(block),
            )
        };
        let mut shapes = String::new();
        let mut entries = String::new();
        for (z, c) in new.iter().enumerate() {
            let id = next_id;
            next_id += 1;
            let ppath = fresh("/xl/ctrlProps/ctrlProp", "xml");
            let rid = alloc_rel();
            ctx.rels.push(Relationship {
                id: rid.clone(),
                rel_type: REL_CTRL_PROP.into(),
                target: crate::write::relative_target(ctx.sheet_path, &ppath),
                mode: TargetMode::Internal,
            });
            ctx.out_parts.push((
                ppath,
                Some(CT_CTRL_PROP.into()),
                ctrl_prop_xml(c).into_bytes(),
            ));
            shapes.push_str(&vml_shape_xml(c, id, z + 1));
            entries.push_str(&entry_xml(c, id, &rid));
        }
        match base {
            None => {
                let xml = String::from_utf8_lossy(&ctx.current(&vml_path).unwrap_or_default())
                    .into_owned();
                let ts = tags(&xml);
                let at = ts
                    .iter()
                    .rev()
                    .find(|t| t.kind == TagKind::Close)
                    .map_or(xml.len(), |t| t.start);
                let shapetype = if xml.contains("_x0000_t201") {
                    ""
                } else {
                    SHAPETYPE_201
                };
                let out = format!("{}{shapetype}{shapes}{}", &xml[..at], &xml[at..]);
                ctx.overrides.insert(vml_path, out.into_bytes());
            }
            Some(block) => {
                let rid = alloc_rel();
                ctx.rels.push(Relationship {
                    id: rid.clone(),
                    rel_type: REL_VML.into(),
                    target: crate::write::relative_target(ctx.sheet_path, &vml_path),
                    mode: TargetMode::Internal,
                });
                ctx.out_parts.push((
                    vml_path,
                    Some(CT_VML.into()),
                    vml_xml(block, &shapes).into_bytes(),
                ));
                unknown.push(RawXml {
                    after: "legacyDrawing".into(),
                    xml: format!("<legacyDrawing r:id=\"{rid}\"/>"),
                });
            }
        }
        let inserted = block_at.and_then(|bi| {
            let xml = &unknown[bi].xml;
            let ts = tags(xml);
            let k = ts
                .iter()
                .position(|t| t.kind == TagKind::Open && t.local(xml) == "controls")?;
            let close = child_spans(&ts, k).1?;
            let out = format!("{}{entries}{}", &xml[..close], &xml[close..]);
            unknown[bi].xml = out;
            Some(())
        });
        if inserted.is_none() {
            unknown.push(RawXml {
                after: "controls".into(),
                xml: format!(
                    "<mc:AlternateContent xmlns:mc=\"{NS_MC}\"><mc:Choice xmlns:x14=\"{NS_X14}\" Requires=\"x14\"><controls>{entries}</controls></mc:Choice></mc:AlternateContent>"
                ),
            });
        }
    }

    // A `controls` element left without entries is dropped (CT_Controls needs one).
    unknown.retain(|u| {
        let ts = tags(&u.xml);
        let has_controls = ts
            .iter()
            .any(|t| t.kind == TagKind::Open && t.local(&u.xml) == "controls");
        !has_controls
            || ts
                .iter()
                .any(|t| t.kind != TagKind::Close && t.local(&u.xml) == "control")
    });
    unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_vml_client_data() {
        let vml = "<xml xmlns:v='urn:schemas-microsoft-com:vml'><v:shapetype id='_x0000_t201'/>\n <v:shape id=\"_x0000_s1026\" type=\"#_x0000_t201\" style='a:b'>\n  <v:textbox style='x'><div style='text-align:left'><font face=\"Tahoma\">Đồng ý &amp; gửi</font></div></v:textbox>\n  <x:ClientData ObjectType=\"Checkbox\">\n   <x:Anchor>\n    1, 15, 2, 4, 3, 10, 3, 16</x:Anchor>\n   <x:FmlaLink>$D$2</x:FmlaLink><x:NoThreeD/>\n   <x:Checked>1</x:Checked>\n  </x:ClientData>\n </v:shape></xml>";
        let s = vml_shapes(vml);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, 1026);
        assert_eq!(s[0].object_type.as_deref(), Some("Checkbox"));
        assert_eq!(s[0].text.as_deref(), Some("Đồng ý & gửi"));
        assert_eq!(s[0].tag("FmlaLink"), Some("$D$2"));
        assert_eq!(s[0].tag("NoThreeD"), Some(""));
        let a = s[0].anchor().unwrap();
        assert_eq!(a.from, CellRef::new(2, 1));
        assert_eq!(a.from_off, (15 * EMU_PER_PX, 4 * EMU_PER_PX));
        assert_eq!(a.to, CellRef::new(3, 3));
    }

    #[test]
    fn patches_attributes_and_vml_tags() {
        let c = FormControl {
            kind: FormControlKind::ScrollBar,
            val: 7,
            max: 20,
            link: None,
            ..FormControl::default()
        };
        let xml = "<?xml version=\"1.0\"?>\n<formControlPr xmlns=\"x\" objectType=\"Scroll\" fmlaLink=\"$A$1\" val=\"3\" dx=\"16\"/>";
        let p = patch_ctrl_prop(xml, &c).unwrap();
        assert!(p.contains("val=\"7\"") && p.contains("max=\"20\"") && p.contains("dx=\"16\""));
        assert!(!p.contains("fmlaLink"));
        let shape = "<v:shape id=\"_x0000_s1\"><x:ClientData ObjectType=\"Scroll\"><x:Val>3</x:Val><x:FmlaLink>$A$1</x:FmlaLink></x:ClientData></v:shape>";
        let p = patch_vml_shape(shape, &c, None);
        assert!(p.contains("<x:Val>7</x:Val>") && p.contains("<x:Max>20</x:Max>"));
        assert!(!p.contains("FmlaLink"));
    }

    #[test]
    fn removes_top_level_children() {
        let xml = "<r><a><b spid=\"_x0000_s5\"/></a><c/></r>";
        assert_eq!(
            remove_top_level(xml, "spid=\"_x0000_s5\"").as_deref(),
            Some("<r><c/></r>")
        );
        assert_eq!(set_attr("<x a=\"1\" b='2'/>", "b", None), "<x a=\"1\"/>");
        assert_eq!(
            set_attr("<x a=\"1\">", "c", Some("<")),
            "<x a=\"1\" c=\"&lt;\">"
        );
    }
}
