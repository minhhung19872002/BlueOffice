//! BlueDoc dialogs of parity round 13 (docs/ui/REPORT.md "Vòng 103"), laid out from the
//! reference word processor's dialogs captured at 125 % (window less its 8 px invisible
//! border): Paragraph (Indents and Spacing / Line and Page Breaks, §17.3.1 pPr), Tabs
//! (§17.3.1.37 tabs), Page Setup (Margins / Paper / Layout, §17.6 sectPr) and the New / Modify
//! Style dialog with its Format ▾ menu (§17.7 styles). The Paragraph dialog is the round-11
//! Find Paragraph form filled with the paragraph's values; its boxes are two-state. A child
//! module of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut, Kind};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Map, Value};

/// Round-13 state of a dialog.
#[derive(Default)]
pub(crate) struct R13 {
    /// Values the dialog opened with (`FindFormat` fields, page setup fields).
    pub(crate) initial: Value,
    /// The dialog sets a style's format (New / Modify Style › Format ▾): OK hands the
    /// format back to the style dialog instead of formatting the text.
    pub(crate) style_mode: bool,
    /// Lengths in inches (else centimetres).
    inches: bool,
    /// Tabs: the stops (position twips, alignment, leader) and the positions cleared.
    tabs: Vec<(i32, String, String)>,
    cleared: Vec<i32>,
    /// New / Modify Style: the style's full spec and the format chosen with Format ▾.
    pub(crate) spec: Value,
    pub(crate) fmt: Map<String, Value>,
    /// New / Modify Style toggles: bold, italic, underline.
    toggles: [bool; 3],
    /// New / Modify Style alignment (0 left … 3 justified), line spacing (0 single, 1
    /// 1.5, 2 double), space before / after steps.
    align: Option<usize>,
    line: Option<usize>,
    /// Page Setup: orientation (landscape) and the paper sizes offered.
    landscape: bool,
    /// New Style: the spec it started from (the description lists what differs).
    base: Value,
}

/// Spin arrows beside a number box: arrow canvas id, box id, step (twips), unit.
const SPINS: [(&str, &str, i32, Unit); 22] = [
    ("lnStartSp", "lnStart", 1, Unit::Num),
    ("lnBySp", "lnBy", 1, Unit::Num),
    ("lnFromSp", "lnFrom", 144, Unit::Len),
    ("boTopSp", "boTop", 20, Unit::Pt),
    ("boLeftSp", "boLeft", 20, Unit::Pt),
    ("boBottomSp", "boBottom", 20, Unit::Pt),
    ("boRightSp", "boRight", 20, Unit::Pt),
    ("inSp", "inside", 144, Unit::Len),
    ("outSp", "outside", 144, Unit::Len),
    ("bySp", "by", 144, Unit::Len),
    ("befSp", "before", 120, Unit::Pt),
    ("aftSp", "after", 120, Unit::Pt),
    ("atSp", "at", 0, Unit::At),
    ("topSp", "mTop", 144, Unit::Len),
    ("botSp", "mBottom", 144, Unit::Len),
    ("lefSp", "mLeft", 144, Unit::Len),
    ("rigSp", "mRight", 144, Unit::Len),
    ("gutSp", "gutter", 144, Unit::Len),
    ("wSp", "pWidth", 144, Unit::Len),
    ("hSp", "pHeight", 144, Unit::Len),
    ("hdrSp", "hdr", 144, Unit::Len),
    ("ftrSp", "ftr", 144, Unit::Len),
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Unit {
    /// Inches or centimetres (the window's unit).
    Len,
    /// Points.
    Pt,
    /// Line spacing "At": lines for Multiple, points otherwise.
    At,
    /// A whole number, at least 1 (Line Numbers › Start at / Count by).
    Num,
}

/// Paper sizes offered (twips) with their names: ANSI / ISO sizes.
pub(crate) const PAPERS: [(i32, i32, &str); 8] = [
    (12240, 15840, "Letter (8 ½ x 11 in)"),
    (12240, 20160, "Legal (8 ½ x 14 in)"),
    (10440, 15120, "Executive (7 ¼ x 10 ½ in)"),
    (16838, 23811, "A3 (29.7 x 42 cm)"),
    (11906, 16838, "A4 (21 x 29.7 cm)"),
    (8391, 11906, "A5 (14.8 x 21 cm)"),
    (10319, 14571, "B5 (18.2 x 25.7 cm)"),
    (5940, 13680, "Envelope #10 (4 ⅛ x 9 ½ in)"),
];

/// Section start values (§17.18.77 ST_SectionMark) in the reference's order.
const STARTS: [&str; 5] = [
    "continuous",
    "nextColumn",
    "nextPage",
    "evenPage",
    "oddPage",
];

/// Vertical alignment values (§17.18.101 ST_VerticalJc) in the reference's order.
const VALIGNS: [&str; 4] = ["top", "center", "both", "bottom"];

/// Alignments of the Paragraph dialog: `FindFormat` value.
const ALIGNS: [&str; 4] = ["left", "center", "right", "both"];

/// A length in twips shown in the window's unit (`1"`, `2.54 cm`).
pub(crate) fn show_len(tw: i32, inches: bool) -> String {
    let v = if inches {
        f64::from(tw) / 1440.0
    } else {
        f64::from(tw) / 566.929
    };
    let n = format!("{v:.2}");
    let n = n.trim_end_matches('0').trim_end_matches('.');
    let n = if n == "-0" { "0" } else { n };
    if inches {
        format!("{n}\"")
    } else {
        format!("{n} cm")
    }
}

/// Points shown the reference's way (`0 pt`, `8 pt`).
fn show_pt(tw: i32) -> String {
    let n = format!("{:.1}", f64::from(tw) / 20.0);
    format!("{} pt", n.trim_end_matches('0').trim_end_matches('.'))
}

/// Twips of a typed length (`1"`, `2.5 cm`, `10 mm`, `12 pt`, `1 pi`); a bare number is in
/// `bare` twips per unit. `None` for an empty or unreadable box.
pub(crate) fn parse_len(text: &str, bare: f64) -> Option<i32> {
    let t = text.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    let split = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(t.len());
    let v: f64 = t[..split].trim().parse().ok()?;
    let per = match t[split..].trim() {
        "\"" | "in" | "″" => 1440.0,
        "cm" => 566.929,
        "mm" => 56.6929,
        "pt" => 20.0,
        "pi" => 240.0,
        "li" | "lines" => 240.0,
        "" => bare,
        _ => return None,
    };
    Some((v * per).round() as i32)
}

impl DocForm {
    fn r13(&self) -> &R13 {
        static EMPTY: std::sync::OnceLock<R13> = std::sync::OnceLock::new();
        self.r13
            .as_deref()
            .unwrap_or_else(|| EMPTY.get_or_init(R13::default))
    }

    fn r13_mut(&mut self) -> &mut R13 {
        self.r13.get_or_insert_with(Box::default)
    }

    fn len_per(&self) -> f64 {
        if self.r13().inches {
            1440.0
        } else {
            566.929
        }
    }

    /// The round-13 dialogs' own spin arrows (all dialogs of this round, and the Paragraph
    /// dialog built from Find Paragraph). `true` when handled.
    pub(super) fn r13_canvas(&mut self, id: &str, x: f32, y: f32) -> bool {
        let _ = x;
        if !handles(self.kind) && self.r13.is_none() {
            return false;
        }
        if let Some(&(_, edit, step, unit)) = SPINS.iter().find(|s| s.0 == id) {
            if !self.form.get(edit).is_some_and(|c| c.enabled) {
                return true;
            }
            let h = self.form.get(id).map_or(24.0, |c| c.rect[3]);
            let up = y < h / 2.0;
            let text = self.form.text(edit);
            let new = match unit {
                Unit::Len => {
                    let cur = parse_len(&text, self.len_per()).unwrap_or(0);
                    let step = if self.r13().inches { step } else { 57 };
                    let v = if up { cur + step } else { cur - step };
                    let v = if edit == "by" || edit.starts_with('m') || edit == "gutter" {
                        v.max(0)
                    } else {
                        v
                    };
                    show_len(v, self.r13().inches)
                }
                Unit::Pt => {
                    let cur = parse_len(&text, 20.0).unwrap_or(0);
                    show_pt((if up { cur + step } else { cur - step }).max(0))
                }
                Unit::Num => {
                    let cur = text.trim().parse::<i32>().unwrap_or(1);
                    (if up { cur + step } else { cur - step })
                        .max(1)
                        .to_string()
                }
                Unit::At => {
                    // Multiple: lines in steps of 0.5; At least / Exactly: points.
                    if self.sel("line").unwrap_or(0) >= 5 {
                        let cur = text.trim().replace(',', ".").parse::<f32>().unwrap_or(1.0);
                        let v = if up { cur + 0.5 } else { (cur - 0.5).max(0.5) };
                        let n = format!("{v:.2}");
                        n.trim_end_matches('0').trim_end_matches('.').to_owned()
                    } else {
                        let cur = parse_len(&text, 20.0).unwrap_or(240);
                        show_pt((if up { cur + 20 } else { cur - 20 }).max(20))
                    }
                }
            };
            self.form.set_text(edit, &new);
            return true;
        }
        // Two-state boxes (Paragraph dialog): on ↔ off.
        if self.kind == DocFormKind::Paragraph {
            if let Some(t) = self.tris.iter_mut().find(|t| t.id == id && t.enabled) {
                t.state = if t.state == 1 { 2 } else { 1 };
                return true;
            }
        }
        match (self.kind, id) {
            (DocFormKind::PageSetup, "portrait" | "landscape") => {
                let land = id == "landscape";
                if land != self.r13().landscape {
                    self.r13_mut().landscape = land;
                    // The page turns: width and height swap.
                    let (w, h) = (self.form.text("pWidth"), self.form.text("pHeight"));
                    self.form.set_text("pWidth", &h);
                    self.form.set_text("pHeight", &w);
                }
                true
            }
            (DocFormKind::NewStyle, _) => self.r13_style_canvas(id),
            _ => false,
        }
    }

    /// A list, drop-down or box of a round-13 dialog changed.
    pub(super) fn r13_changed(&mut self, id: &str) {
        match (self.kind, id) {
            (DocFormKind::Paragraph, "special") => {
                // (none) empties By, First line / Hanging start at 0.5".
                match self.sel("special") {
                    Some(0) => self.form.set_text("by", ""),
                    Some(_) if self.form.text("by").trim().is_empty() => {
                        let v = show_len(720, self.r13().inches);
                        self.form.set_text("by", &v);
                    }
                    _ => {}
                }
            }
            (DocFormKind::Paragraph, "line") => {
                let at = match self.sel("line") {
                    Some(3 | 4) => show_pt(240),
                    Some(5) => "3".to_owned(),
                    _ => String::new(),
                };
                self.form.set_text("at", &at);
            }
            (DocFormKind::Tabs, "tabList") => {
                if let Some(i) = self.sel("tabList") {
                    if let Some((pos, align, leader)) = self.r13().tabs.get(i).cloned() {
                        let v = show_len(pos, self.r13().inches);
                        self.form.set_text("tabPos", &v);
                        self.set_tab_radios(&align, &leader);
                    }
                }
            }
            (DocFormKind::PageSetup, "multi") => self.ps_multi_changed(),
            (DocFormKind::LineNumbers, "lnOn") => self.line_numbers_enable(),
            (DocFormKind::Tabs, "tabPos") => {
                // Set needs a position, as the reference.
                let any = !self.form.text("tabPos").trim().is_empty();
                self.form.enable("set", any);
            }
            (DocFormKind::PageSetup, "paper") => {
                if let Some(&(w, h, _)) = self.sel("paper").and_then(|i| PAPERS.get(i)) {
                    let (w, h) = if self.r13().landscape { (h, w) } else { (w, h) };
                    let inches = self.r13().inches;
                    self.form.set_text("pWidth", &show_len(w, inches));
                    self.form.set_text("pHeight", &show_len(h, inches));
                }
            }
            (DocFormKind::NewStyle, "type") => {}
            _ => {}
        }
    }

    fn set_tab_radios(&mut self, align: &str, leader: &str) {
        let a = match align {
            "center" => "tCenter",
            "right" => "tRight",
            "decimal" => "tDecimal",
            "bar" => "tBar",
            _ => "tLeft",
        };
        let l = match leader {
            "dot" => "l2",
            "hyphen" => "l3",
            "underscore" => "l4",
            _ => "l1",
        };
        self.set_radio(a);
        self.set_radio(l);
    }

    /// Mirror margins / book fold: Left / Right read Inside / Outside, the gutter is on the
    /// inside (its position fixed).
    fn ps_multi_changed(&mut self) {
        let m = self.sel("multi").unwrap_or(0);
        let mirror = m == 1 || m == 3;
        let (l, r) = if mirror {
            (
                word(self, "doc13.ps_inside", "Inside:"),
                word(self, "doc13.ps_outside", "Outside:"),
            )
        } else {
            (
                word(self, "doc13.ps_leftL", "Left:"),
                word(self, "doc13.ps_rightL", "Right:"),
            )
        };
        self.set_label("mLeftL", &l);
        self.set_label("mRightL", &r);
        self.form.enable("gutterPos", m == 0);
    }

    /// A push button of a round-13 dialog.
    pub(super) fn r13_clicked(&mut self, id: &str) -> FormOut {
        match (self.kind, id) {
            (DocFormKind::Paragraph, "tabs") => {
                // The reference applies the paragraph's changes and opens Tabs.
                match self.paragraph_ok() {
                    FormOut::Run(mut cmds, _) => {
                        cmds.push(json!({ "cmd": "ui.action", "id": "r13.open.tabs" }));
                        FormOut::Run(cmds, true)
                    }
                    other => other,
                }
            }
            (DocFormKind::Paragraph, "setDefault") => {
                // Set As Default: the Normal style gets these values (this document).
                let m = self.paragraph_changes();
                FormOut::Run(
                    vec![
                        json!({ "cmd": "ui.action", "id": format!("r13.paraDefault:{}", Value::Object(m)) }),
                    ],
                    true,
                )
            }
            (DocFormKind::Tabs, "set") => {
                self.tabs_set();
                FormOut::Redraw
            }
            (DocFormKind::Tabs, "clear") => {
                let pos = parse_len(&self.form.text("tabPos"), self.len_per());
                if let Some(pos) = pos {
                    let r = self.r13_mut();
                    if let Some(i) = r.tabs.iter().position(|t| (t.0 - pos).abs() <= 1) {
                        r.tabs.remove(i);
                        r.cleared.push(pos);
                    }
                    self.refresh_tab_list();
                }
                FormOut::Redraw
            }
            (DocFormKind::Tabs, "clearAll") => {
                let r = self.r13_mut();
                let all: Vec<i32> = r.tabs.drain(..).map(|t| t.0).collect();
                r.cleared.extend(all);
                self.form.set_text("tabPos", "");
                self.refresh_tab_list();
                FormOut::Redraw
            }
            (DocFormKind::PageSetup, "lineNumbers") => FormOut::Stash(DocFormKind::LineNumbers),
            (DocFormKind::PageSetup, "borders") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "design.pageBorders" })],
                true,
            ),
            (DocFormKind::PageSetup, "printOptions") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "file.options" })],
                true,
            ),
            (DocFormKind::PageSetup, "setDefault") => {
                let mut v = self.ps_values();
                v["scope"] = json!("default");
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r13.psDefault:{v}") })],
                    true,
                )
            }
            (DocFormKind::BordersShading, "bOptions") => FormOut::Stash(DocFormKind::BorderOptions),
            (DocFormKind::NewStyle, "format") => {
                // Format ▾: the window opens the menu under the button.
                let r = self
                    .form
                    .get("format")
                    .map_or([0.0; 4], |c| self.form.dev(c.rect));
                FormOut::Run(
                    vec![
                        json!({ "cmd": "ui.action", "id": format!("r13.styleFmtMenu:{},{},{},{}", r[0], r[1], r[2], r[3]) }),
                    ],
                    false,
                )
            }
            _ => FormOut::Redraw,
        }
    }

    /// OK of a round-13 dialog; `None` when `kind` is not one (or a style-mode Format ▾
    /// dialog of round 11, whose format goes back to the style dialog).
    pub(super) fn r13_ok(&mut self) -> Option<FormOut> {
        if self.r13.as_ref().is_some_and(|r| r.style_mode) {
            let m = match self.kind {
                DocFormKind::Paragraph => self.paragraph_changes(),
                DocFormKind::Tabs => {
                    self.tabs_set();
                    let mut m = Map::new();
                    m.insert("tabs".into(), self.tabs_json());
                    m
                }
                DocFormKind::BordersShading => self.style_border_values(),
                _ => match self.r11_ok() {
                    FormOut::FindFormat(_, Value::Object(m), _) => m,
                    _ => Map::new(),
                },
            };
            return Some(FormOut::Run(
                vec![
                    json!({ "cmd": "ui.action", "id": format!("r13.styleFmtDone:{}", Value::Object(m)) }),
                ],
                true,
            ));
        }
        match self.kind {
            DocFormKind::Paragraph => Some(self.paragraph_ok()),
            DocFormKind::Tabs => {
                self.tabs_set();
                Some(FormOut::Run(
                    vec![
                        json!({ "cmd": "formatParagraph", "format": { "kind": "tabs", "tabs": self.tabs_json() } }),
                    ],
                    true,
                ))
            }
            DocFormKind::PageSetup => {
                let v = self.ps_values();
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r13.psApply:{v}") })],
                    true,
                ))
            }
            DocFormKind::NewStyle => Some(self.style_ok()),
            DocFormKind::LineNumbers => Some(self.line_numbers_ok()),
            DocFormKind::PaneOptions => Some(FormOut::Run(
                vec![
                    json!({ "cmd": "ui.action", "id": format!("r13.paneOptions:{}", json!({
                    "show": self.sel("show").unwrap_or(0),
                    "sort": self.sel("sort").unwrap_or(0),
                    "template": self.chosen("scope") == Some("template"),
                })) }),
                ],
                true,
            )),
            DocFormKind::BorderOptions => Some(FormOut::Run(
                vec![json!({ "cmd": "ui.action",
                    "id": format!("r13.bsOptionsDone:{}", json!(self.border_options_values())) })],
                true,
            )),
            _ => None,
        }
    }

    // ------------------------------------------------------------------- Paragraph

    /// The dialog's values as `FindFormat` fields.
    fn paragraph_values(&self) -> Map<String, Value> {
        let mut m = Map::new();
        let per = self.len_per();
        if let Some(i) = self.sel("align") {
            m.insert("align".into(), json!(ALIGNS[i.min(3)]));
        }
        if let Some(i) = self.sel("outline") {
            m.insert("outlineLevel".into(), json!(if i == 0 { 10 } else { i }));
        }
        m.insert(
            "indentLeft".into(),
            json!(parse_len(&self.form.text("inside"), per).unwrap_or(0)),
        );
        m.insert(
            "indentRight".into(),
            json!(parse_len(&self.form.text("outside"), per).unwrap_or(0)),
        );
        let by = parse_len(&self.form.text("by"), per).unwrap_or(0).max(0);
        let first = match self.sel("special") {
            Some(1) => by,
            Some(2) => -by,
            _ => 0,
        };
        m.insert("firstLine".into(), json!(first));
        m.insert(
            "spaceBefore".into(),
            json!(parse_len(&self.form.text("before"), 20.0)
                .unwrap_or(0)
                .max(0)),
        );
        m.insert(
            "spaceAfter".into(),
            json!(parse_len(&self.form.text("after"), 20.0)
                .unwrap_or(0)
                .max(0)),
        );
        let at = self.form.text("at");
        let ls = match self.sel("line") {
            Some(1) => Some("1.5".to_owned()),
            Some(2) => Some("double".to_owned()),
            Some(3) => parse_len(&at, 20.0).map(|t| format!("atLeast:{}", t.max(20))),
            Some(4) => parse_len(&at, 20.0).map(|t| format!("exactly:{}", t.max(20))),
            Some(5) => at
                .trim()
                .replace(',', ".")
                .parse::<f32>()
                .ok()
                .map(|l| format!("multiple:{}", (l * 240.0).round().max(6.0) as i32)),
            _ => Some("single".to_owned()),
        };
        if let Some(ls) = ls {
            m.insert("lineSpacing".into(), json!(ls));
        }
        for (id, key) in [
            ("noSpace", "contextualSpacing"),
            ("widow", "widowControl"),
            ("keepNext", "keepNext"),
            ("keepLines", "keepLines"),
            ("pageBefore", "pageBreakBefore"),
            ("noLineNum", "suppressLineNumbers"),
            ("noHyphen", "suppressAutoHyphens"),
            ("mirror", "mirrorIndents"),
            ("mirror", "mirrorIndents"),
        ] {
            if let Some(v) = self.tri_value(id) {
                m.insert(key.into(), json!(v));
            }
        }
        m
    }

    /// The fields that differ from the paragraph's values when the dialog opened.
    fn paragraph_changes(&self) -> Map<String, Value> {
        let now = self.paragraph_values();
        let was = &self.r13().initial;
        now.into_iter().filter(|(k, v)| was[k] != *v).collect()
    }

    fn paragraph_ok(&self) -> FormOut {
        let m = self.paragraph_changes();
        if m.is_empty() {
            return FormOut::Close;
        }
        FormOut::Run(
            vec![
                json!({ "cmd": "formatParagraph", "format": { "kind": "props", "format": Value::Object(m) } }),
            ],
            true,
        )
    }

    // ------------------------------------------------------------------------ Tabs

    fn tabs_json(&self) -> Value {
        Value::Array(
            self.r13()
                .tabs
                .iter()
                .map(|(pos, align, leader)| json!({ "pos": pos, "align": align, "leader": leader }))
                .collect(),
        )
    }

    /// Set: the stop typed with the alignment and leader chosen.
    fn tabs_set(&mut self) {
        let Some(pos) = parse_len(&self.form.text("tabPos"), self.len_per()) else {
            return;
        };
        let align = match self.chosen("talign") {
            Some("tCenter") => "center",
            Some("tRight") => "right",
            Some("tDecimal") => "decimal",
            Some("tBar") => "bar",
            _ => "left",
        };
        let leader = match self.chosen("leader") {
            Some("l2") => "dot",
            Some("l3") => "hyphen",
            Some("l4") => "underscore",
            _ => "none",
        };
        let r = self.r13_mut();
        r.tabs.retain(|t| (t.0 - pos).abs() > 1);
        r.tabs.push((pos, align.to_owned(), leader.to_owned()));
        r.tabs.sort_by_key(|t| t.0);
        r.cleared.retain(|c| (c - pos).abs() > 1);
        self.refresh_tab_list();
    }

    fn refresh_tab_list(&mut self) {
        let inches = self.r13().inches;
        let items: Vec<String> = self
            .r13()
            .tabs
            .iter()
            .map(|t| show_len(t.0, inches))
            .collect();
        let pos = parse_len(&self.form.text("tabPos"), self.len_per());
        let sel = pos.and_then(|p| self.r13().tabs.iter().position(|t| (t.0 - p).abs() <= 1));
        self.form.set_items("tabList", items, sel);
        let cleared: Vec<String> = self
            .r13()
            .cleared
            .iter()
            .map(|c| show_len(*c, inches))
            .collect();
        self.set_label("clearedL", &cleared.join(", "));
        let any = !self.r13().tabs.is_empty();
        self.form.enable("clear", any);
        // The reference keeps Clear All available.
    }

    // ------------------------------------------------------------------ Page Setup

    /// The dialog's values for the window (`r13.psApply`).
    fn ps_values(&self) -> Value {
        let per = self.len_per();
        let len = |id: &str| parse_len(&self.form.text(id), per);
        let scope = match self.sel("applyTo") {
            Some(i) => self.ids.get(i).cloned().unwrap_or_default(),
            None => "document".to_owned(),
        };
        let multi = self.sel("multi").unwrap_or(0);
        json!({
            "top": len("mTop"), "bottom": len("mBottom"), "left": len("mLeft"), "right": len("mRight"),
            "gutter": len("gutter").map(|g| g.max(0)),
            "gutterTop": self.sel("gutterPos") == Some(1) && multi == 0,
            "landscape": self.r13().landscape,
            "width": len("pWidth"), "height": len("pHeight"),
            "mirror": multi == 1, "twoOnOne": multi == 2, "bookFold": multi == 3,
            "start": STARTS[self.sel("start").unwrap_or(2).min(4)],
            "evenOdd": self.on("evenOdd"), "titlePage": self.on("firstPage"),
            "header": len("hdr"), "footer": len("ftr"),
            "vAlign": VALIGNS[self.sel("vAlign").unwrap_or(0).min(3)],
            "scope": scope,
        })
    }

    // ------------------------------------------------------------- New / Modify Style

    /// Toggle buttons of the New / Modify Style formatting rows.
    fn r13_style_canvas(&mut self, id: &str) -> bool {
        let r = self.r13_mut();
        match id {
            "sBold" => r.toggles[0] = !r.toggles[0],
            "sItalic" => r.toggles[1] = !r.toggles[1],
            "sUnder" => r.toggles[2] = !r.toggles[2],
            "aLeft" | "aCenter" | "aRight" | "aJust" => {
                let i = ["aLeft", "aCenter", "aRight", "aJust"]
                    .iter()
                    .position(|a| *a == id)
                    .unwrap_or(0);
                r.align = Some(i);
            }
            "lSingle" | "l15" | "lDouble" => {
                let i = ["lSingle", "l15", "lDouble"]
                    .iter()
                    .position(|a| *a == id)
                    .unwrap_or(0);
                r.line = Some(i);
            }
            "spMore" | "spLess" => {
                let cur = r.spec["spaceBefore"].as_i64().unwrap_or(0);
                let v = if id == "spMore" {
                    cur + 120
                } else {
                    (cur - 120).max(0)
                };
                r.spec["spaceBefore"] = json!(v);
                let cur = r.spec["spaceAfter"].as_i64().unwrap_or(0);
                let v = if id == "spMore" {
                    cur + 120
                } else {
                    (cur - 120).max(0)
                };
                r.spec["spaceAfter"] = json!(v);
            }
            "inMore" | "inLess" => {
                let cur = r.spec["indentLeft"].as_i64().unwrap_or(0);
                let v = if id == "inMore" {
                    cur + 720
                } else {
                    (cur - 720).max(0)
                };
                r.spec["indentLeft"] = json!(v);
            }
            _ => return false,
        }
        self.style_describe();
        true
    }

    /// Spec of the dialog: the style's own values with the dialog's changes.
    pub(crate) fn style_spec(&self) -> Value {
        let mut spec = self.r13().spec.clone();
        if !spec.is_object() {
            spec = json!({});
        }
        let name = self.text("name");
        let kind = ["paragraph", "character", "paragraph", "table", "numbering"]
            [self.sel("type").unwrap_or(0).min(4)];
        let pick = |k: Option<usize>| -> Value {
            match k.filter(|k| *k > 0).and_then(|k| self.style_ids.get(k - 1)) {
                Some(id) => json!(id),
                None => Value::Null,
            }
        };
        spec["name"] = json!(name.trim());
        spec["type"] = json!(kind);
        spec["basedOn"] = pick(self.sel("basedOn"));
        spec["next"] = pick(self.sel("next"));
        spec["addToGallery"] = json!(self.on("gallery"));
        spec["autoUpdate"] = json!(self.on("auto"));
        let font = self.text("font");
        // "Calibri Light (Headings)" is the font Calibri Light.
        let font = match font.trim().rfind(" (") {
            Some(i) if font.trim().ends_with(')') => font.trim()[..i].to_owned(),
            _ => font.trim().to_owned(),
        };
        if !font.is_empty() {
            spec["font"] = json!(font);
        }
        if let Ok(pt) = self.text("size").trim().replace(',', ".").parse::<f32>() {
            spec["size"] = json!((pt * 2.0).round() as u32);
        }
        let r = self.r13();
        spec["bold"] = json!(r.toggles[0]);
        spec["italic"] = json!(r.toggles[1]);
        spec["underline"] = json!(r.toggles[2]);
        let color = self.form.color("color");
        if !color.is_empty() && color != super::form::NO_COLOR {
            spec["color"] = json!(color);
        }
        if let Some(a) = r.align {
            // `bo_model::Alignment` names (the style spec's serde form).
            spec["align"] = json!(["Left", "Center", "Right", "Justify"][a.min(3)]);
        }
        if let Some(l) = r.line {
            spec["lineSpacing"] = json!([240, 360, 480][l.min(2)]);
        }
        if !r.fmt.is_empty() {
            // Tabs… go to the style's tab stops, the rest are FindFormat fields.
            let mut fmt = r.fmt.clone();
            for k in ["tabs", "border", "shading"] {
                if let Some(v) = fmt.remove(k) {
                    spec[k] = v;
                }
            }
            if !fmt.is_empty() {
                spec["format"] = Value::Object(fmt);
            }
        }
        spec
    }

    fn style_ok(&self) -> FormOut {
        let name = self.text("name");
        if name.trim().is_empty() {
            return FormOut::Redraw;
        }
        let spec = self.style_spec();
        let template = self.chosen("scope") == Some("template");
        let mut cmds = vec![match &self.modify {
            Some(id) => json!({ "cmd": "styleModify", "id": id, "spec": spec }),
            None => json!({ "cmd": "styleNew", "spec": spec, "fromSelection": true }),
        }];
        if template {
            // New documents based on this template: the window keeps the style in BlueDoc's
            // own Normal template (the user's defaults file).
            cmds.push(json!({ "cmd": "ui.action", "id": format!("r13.templateStyle:{}", json!({ "id": self.modify, "spec": spec })) }));
        }
        FormOut::Run(cmds, true)
    }

    /// The description under the preview: the format words of the spec.
    pub(crate) fn style_describe(&mut self) {
        let spec = self.style_spec();
        let base = self.r13().base.clone();
        // A new style lists what differs from the style it is based on (the reference's
        // "Style: Show in the Styles gallery / Based on: …"); Modify Style lists all.
        let differs = |k: &str| self.modify.is_some() || !base.is_object() || spec[k] != base[k];
        let mut words: Vec<String> = Vec::new();
        if let Some(f) = spec["font"]
            .as_str()
            .filter(|_| differs("font") || differs("size"))
        {
            let mut w = format!("{} {f}", word(self, "doc13.ns_fontW", "Font:"));
            if let Some(sz) = spec["size"].as_u64() {
                #[allow(clippy::cast_precision_loss)]
                let pt = sz as f32 / 2.0;
                w.push_str(&format!(", {pt} pt"));
            }
            words.push(w);
        }
        for (k, key, fb) in [
            ("bold", "doc13.ns_bold", "Bold"),
            ("italic", "doc13.ns_italic", "Italic"),
            ("underline", "doc13.ns_underline", "Underline"),
        ] {
            if spec[k].as_bool() == Some(true) && differs(k) {
                words.push(word(self, key, fb));
            }
        }
        if let Some(a) = align_index(&spec["align"])
            .filter(|a| differs("align") && !(align_index(&base["align"]).is_none() && *a == 0))
        {
            let (key, fb) = match a {
                1 => ("doc13.ns_centered", "Centered"),
                2 => ("doc13.ns_right", "Right"),
                3 => ("doc13.ns_justified", "Justified"),
                _ => ("doc13.ns_left", "Left"),
            };
            words.push(word(self, key, fb));
        }
        if let Some(m) = spec["format"].as_object() {
            if !m.is_empty() {
                words.push(word(self, "doc13.ns_moreFormat", "More formatting"));
            }
        }
        let style_w = word(self, "doc13.ns_styleW", "Style:");
        let gallery = if self.on("gallery") {
            word(self, "doc13.ns_showGallery", "Show in the Styles gallery")
        } else {
            String::new()
        };
        let mut line1 = format!("{style_w} {gallery}").trim_end().to_owned();
        if !words.is_empty() {
            line1 = format!("{}, {line1}", words.join(", "));
        }
        let based = self
            .sel("basedOn")
            .filter(|&i| i > 0)
            .map(|i| {
                let n = self.list_item_text("basedOn", i);
                let n = n
                    .strip_prefix("\u{b6}a")
                    .or_else(|| n.strip_prefix('\u{b6}'))
                    .unwrap_or(&n);
                n.trim().to_owned()
            })
            .map(|n| format!("{} {n}", word(self, "doc13.ns_basedOnW", "Based on:")))
            .unwrap_or_default();
        self.set_label("desc1", &line1);
        self.set_label("desc2", &based);
    }

    fn list_item_text(&self, id: &str, i: usize) -> String {
        match self.form.get(id).map(|c| &c.kind) {
            Some(Kind::Combo { items, .. } | Kind::List { items, .. }) => {
                items.get(i).cloned().unwrap_or_default()
            }
            _ => String::new(),
        }
    }

    /// Borders and Shading opened from Format ▾ › Border…: the paragraph box line (one line
    /// for the style, §17.3.1.24 pBdr) and the shading fill (§17.3.1.31 shd), read from the
    /// commands its OK would run.
    fn style_border_values(&mut self) -> Map<String, Value> {
        let mut m = Map::new();
        let FormOut::Run(cmds, _) = self.r12_ok() else {
            return m;
        };
        for c in &cmds {
            let f = &c["format"];
            if c["cmd"] == "formatParagraph" && f["kind"] == "setBorders" {
                let line = ["top", "left", "bottom", "right"]
                    .iter()
                    .map(|k| &f[*k])
                    .find(|v| v.is_object())
                    .cloned()
                    .unwrap_or(Value::Null);
                m.insert("border".into(), line);
            }
            if c["cmd"] == "formatParagraph" && f["kind"] == "shading" {
                m.insert("shading".into(), f["hex"].clone());
            }
        }
        m
    }

    /// Make this dialog set a style's format (New / Modify Style › Format ▾), starting from
    /// the style dialog's `spec`.
    pub(crate) fn set_style_mode(&mut self, spec: &Value) {
        let inches = self.r13.as_ref().is_none_or(|r| r.inches);
        let r = self.r13_mut();
        r.style_mode = true;
        r.inches = inches;
        match self.kind {
            DocFormKind::Paragraph => {
                if let Some(i) = align_index(&spec["align"]) {
                    self.form.set_sel("align", Some(i));
                }
                let len = |k: &str| show_len(spec[k].as_i64().unwrap_or(0) as i32, inches);
                self.form.set_text("inside", &len("indentLeft"));
                self.form.set_text("outside", &len("indentRight"));
                let first = spec["indentFirstLine"].as_i64().unwrap_or(0) as i32;
                self.form.set_sel(
                    "special",
                    Some(match first {
                        0 => 0,
                        x if x > 0 => 1,
                        _ => 2,
                    }),
                );
                self.form.set_text(
                    "by",
                    &if first == 0 {
                        String::new()
                    } else {
                        show_len(first.abs(), inches)
                    },
                );
                self.form.set_text(
                    "before",
                    &show_pt(spec["spaceBefore"].as_i64().unwrap_or(0) as i32),
                );
                self.form.set_text(
                    "after",
                    &show_pt(spec["spaceAfter"].as_i64().unwrap_or(0) as i32),
                );
                match spec["lineSpacing"].as_u64() {
                    Some(360) => self.form.set_sel("line", Some(1)),
                    Some(480) => self.form.set_sel("line", Some(2)),
                    Some(n) if n != 240 => {
                        self.form.set_sel("line", Some(5));
                        #[allow(clippy::cast_precision_loss)]
                        let l = format!("{:.2}", n as f32 / 240.0);
                        self.form
                            .set_text("at", l.trim_end_matches('0').trim_end_matches('.'));
                    }
                    _ => self.form.set_sel("line", Some(0)),
                }
                self.form.enable("tabs", false);
                self.form.enable("setDefault", false);
                let initial = self.paragraph_values();
                self.r13_mut().initial = Value::Object(initial);
            }
            DocFormKind::FindFont => {
                if let Some(f) = spec["font"].as_str() {
                    self.form.set_text("font", f);
                }
                if let Some(hp) = spec["size"].as_u64() {
                    #[allow(clippy::cast_precision_loss)]
                    let pt = format!("{}", hp as f32 / 2.0);
                    self.form.set_text("size", &pt);
                }
            }
            _ => {}
        }
    }

    /// The format a Format ▾ dialog chose, merged into this style dialog.
    pub(crate) fn style_take_format(&mut self, m: &Map<String, Value>) {
        for (k, v) in m {
            match k.as_str() {
                "font" => {
                    if let Some(f) = v.as_str() {
                        self.choose_item("font", f);
                        self.form.set_text("font", f);
                    }
                }
                "size" => {
                    if let Some(hp) = v.as_u64() {
                        #[allow(clippy::cast_precision_loss)]
                        let pt = format!("{}", hp as f32 / 2.0);
                        self.choose_item("size", &pt);
                    }
                }
                "bold" => self.r13_mut().toggles[0] = v.as_bool().unwrap_or(false),
                "italic" => self.r13_mut().toggles[1] = v.as_bool().unwrap_or(false),
                "underline" => self.r13_mut().toggles[2] = v.as_bool().unwrap_or(false),
                "color" => {
                    if let Some(c) = v.as_str() {
                        self.form.set_color("color", c);
                    }
                }
                "align" => {
                    let i = ALIGNS.iter().position(|a| Some(*a) == v.as_str());
                    self.r13_mut().align = i;
                }
                _ => {
                    self.r13_mut().fmt.insert(k.clone(), v.clone());
                }
            }
        }
        self.style_describe();
    }
}

/// Which dialogs this module builds.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::Paragraph
            | DocFormKind::Tabs
            | DocFormKind::PageSetup
            | DocFormKind::BorderOptions
            | DocFormKind::PaneOptions
            | DocFormKind::LineNumbers
    )
}

/// Translated `key` stored in the dialog's words (looked up when it opened).
fn word(f: &DocForm, key: &str, fallback: &str) -> String {
    f.words
        .iter()
        .find(|(k, _)| k == key)
        .map_or_else(|| fallback.to_owned(), |(_, v)| v.clone())
}

/// Words the round-13 dialogs look up after opening.
fn words(s: &Strings) -> Vec<(String, String)> {
    [
        ("doc13.ps_inside", "Inside:"),
        ("doc13.ps_outside", "Outside:"),
        ("doc13.ps_leftL", "Left:"),
        ("doc13.ps_rightL", "Right:"),
        ("doc13.ns_fontW", "Font:"),
        ("doc13.ns_bold", "Bold"),
        ("doc13.ns_italic", "Italic"),
        ("doc13.ns_underline", "Underline"),
        ("doc13.ns_left", "Left"),
        ("doc13.ns_centered", "Centered"),
        ("doc13.ns_right", "Right"),
        ("doc13.ns_justified", "Justified"),
        ("doc13.ns_moreFormat", "More formatting"),
        ("doc13.ns_styleW", "Style:"),
        ("doc13.ns_showGallery", "Show in the Styles gallery"),
        ("doc13.ns_basedOnW", "Based on:"),
    ]
    .iter()
    .map(|(k, f)| ((*k).to_owned(), t(s, k, f)))
    .collect()
}

/// Build round-13 dialog `kind`.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let mut f = match kind {
        DocFormKind::Paragraph => open_paragraph(s, c),
        DocFormKind::Tabs => open_tabs(s, c),
        DocFormKind::BorderOptions => open_border_options(s, c),
        DocFormKind::PaneOptions => open_pane_options(s, c),
        DocFormKind::LineNumbers => open_line_numbers(s, c),
        _ => open_page_setup(s, c),
    };
    f.words.extend(words(s));
    if kind == DocFormKind::PageSetup {
        f.ps_multi_changed();
        // The reference opens with the top margin selected.
        f.form.focus("mTop");
        f.form.select_all("mTop");
    }
    f
}

/// Paragraph: the Find Paragraph form (same measured layout, 600 × 713 window) with the
/// paragraph's values, Left / Right indents, two-state boxes, Tabs… and Set As Default.
fn open_paragraph(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = super::findfmt11::open(DocFormKind::FindParagraph, s, c);
    f.kind = DocFormKind::Paragraph;
    f.set_title(&tr("doc13.para_title", "Paragraph"));
    let v = &c.extra["r13"]["para"];
    let inches = c.inches;
    f.r13 = Some(Box::new(R13 {
        inches,
        ..R13::default()
    }));
    f.set_label("insideL", &tr("doc13.ps_leftL", "Left:"));
    f.set_label("outsideL", &tr("doc13.ps_rightL", "Right:"));
    let aligns: Vec<String> = [
        ("findFmt11.left", "Left"),
        ("findFmt11.centered", "Centered"),
        ("findFmt11.right", "Right"),
        ("findFmt11.justified", "Justified"),
    ]
    .iter()
    .map(|(k, fb)| tr(k, fb))
    .collect();
    let a = ALIGNS
        .iter()
        .position(|x| Some(*x) == v["align"].as_str())
        .unwrap_or(0);
    f.form.set_items("align", aligns, Some(a));
    let mut levels = vec![tr("findFmt11.bodyText", "Body Text")];
    levels.extend((1..=9).map(|i| format!("{} {i}", tr("findFmt11.level", "Level"))));
    let lvl =
        v["outlineLevel"]
            .as_u64()
            .map_or(0, |l| if (1..=9).contains(&l) { l as usize } else { 0 });
    f.form.set_items("outline", levels, Some(lvl));
    let specials = vec![
        tr("findFmt11.none", "(none)"),
        tr("findFmt11.firstLine", "First line"),
        tr("findFmt11.hanging", "Hanging"),
    ];
    let first = v["firstLine"].as_i64().unwrap_or(0) as i32;
    let sp = match first {
        0 => 0,
        x if x > 0 => 1,
        _ => 2,
    };
    f.form.set_items("special", specials, Some(sp));
    let lines: Vec<String> = [
        ("findFmt11.single", "Single"),
        ("findFmt11.lines15", "1.5 lines"),
        ("findFmt11.double", "Double"),
        ("findFmt11.atLeast", "At least"),
        ("findFmt11.exactly", "Exactly"),
        ("findFmt11.multiple", "Multiple"),
    ]
    .iter()
    .map(|(k, fb)| tr(k, fb))
    .collect();
    let ls = v["lineSpacing"].as_str().unwrap_or("single");
    let (li, at) = match ls {
        "single" => (0, String::new()),
        "1.5" => (1, String::new()),
        "double" => (2, String::new()),
        x => match x.split_once(':') {
            Some(("atLeast", n)) => (3, show_pt(n.parse().unwrap_or(240))),
            Some(("exactly", n)) => (4, show_pt(n.parse().unwrap_or(240))),
            Some(("multiple", n)) => {
                let l = n.parse::<f32>().unwrap_or(240.0) / 240.0;
                let s = format!("{l:.2}");
                (5, s.trim_end_matches('0').trim_end_matches('.').to_owned())
            }
            _ => (0, String::new()),
        },
    };
    f.form.set_items("line", lines, Some(li));
    let len = |k: &str| show_len(v[k].as_i64().unwrap_or(0) as i32, inches);
    f.form.set_text("inside", &len("indentLeft"));
    f.form.set_text("outside", &len("indentRight"));
    f.form.set_text(
        "by",
        &if first == 0 {
            String::new()
        } else {
            show_len(first.abs(), inches)
        },
    );
    f.form.set_text(
        "before",
        &show_pt(v["spaceBefore"].as_i64().unwrap_or(0) as i32),
    );
    f.form.set_text(
        "after",
        &show_pt(v["spaceAfter"].as_i64().unwrap_or(0) as i32),
    );
    f.form.set_text("at", &at);
    for (id, key) in [
        ("noSpace", "contextualSpacing"),
        ("widow", "widowControl"),
        ("keepNext", "keepNext"),
        ("keepLines", "keepLines"),
        ("pageBefore", "pageBreakBefore"),
        ("noLineNum", "suppressLineNumbers"),
        ("noHyphen", "suppressAutoHyphens"),
        ("mirror", "mirrorIndents"),
    ] {
        if let Some(t) = f.tris.iter_mut().find(|t| t.id == id) {
            t.state = if v[key].as_bool() == Some(true) { 1 } else { 2 };
        }
    }
    // Collapsed by default (w15:collapsed) is not in BlueDoc's model yet: dimmed.
    for t in &mut f.tris {
        if t.id == "collapsed" {
            t.state = 2;
            t.enabled = false;
        }
    }
    // Line and Page Breaks › Textbox options (text boxes only: dimmed, as the reference).
    f.form.add(
        "textbox",
        Some(1),
        [24.0, 310.0, 544.0, 18.0],
        Kind::Group(tr("doc13.para_textbox", "Textbox options")),
    );
    f.label_on(
        1,
        "tightL",
        [38.0, 336.0, 200.0, 18.0],
        &tr("doc13.para_tightWrap", "Tight wrap:"),
    );
    f.form.add(
        "tight",
        Some(1),
        [38.0, 356.0, 224.0, 28.0],
        Kind::Combo {
            items: vec![tr("doc13.para_noneWrap", "None")],
            sel: 0,
        },
    );
    f.disable("tight");
    f.form.enable("tabs", true);
    f.form
        .enable("setDefault", c.extra["r13"]["styleMode"] != json!(true));
    // The preview repeats its words the way the reference fills its sample.
    f.preview = vec![
        vec![tr("doc13.para_pvBefore", "Previous Paragraph"); 3].join(" "),
        vec![tr("doc13.para_pvSample", "Sample Text"); 15].join(" "),
        vec![tr("doc13.para_pvAfter", "Following Paragraph"); 2].join(" "),
    ];
    let initial = f.paragraph_values();
    if let Some(r) = f.r13.as_mut() {
        r.initial = Value::Object(initial);
    }
    f
}

/// Tabs: 417 × 461 window (the Find Tabs layout) with the paragraph's stops, Default tab
/// stops (document setting, read only here) and working Set / Clear / Clear All.
fn open_tabs(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = super::findfmt11::open(DocFormKind::FindTabs, s, c);
    f.kind = DocFormKind::Tabs;
    f.set_title(&tr("doc13.tabs_title", "Tabs"));
    let v = &c.extra["r13"];
    let tabs: Vec<(i32, String, String)> = v["tabs"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| {
                    (
                        t["pos"].as_i64().unwrap_or(0) as i32,
                        t["align"].as_str().unwrap_or("left").to_owned(),
                        t["leader"].as_str().unwrap_or("none").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    f.r13 = Some(Box::new(R13 {
        inches: c.inches,
        tabs,
        ..R13::default()
    }));
    // Default tab stops (right of the position box) and the stops to be cleared.
    f.label(
        "defL",
        [248.0, 46.0, 150.0, 18.0],
        &tr("doc13.tabs_defaultStops", "Default tab stops:"),
    );
    let def = v["defaultTab"].as_i64().unwrap_or(720) as i32;
    f.edit(
        "defTab",
        [248.0, 68.0, 142.0, 24.0],
        &show_len(def, c.inches),
    );
    f.disable("defTab");
    f.label("clearedL", [248.0, 132.0, 150.0, 18.0], "");
    if let Some((pos, align, leader)) = f.r13().tabs.first().cloned() {
        f.form.set_text("tabPos", &show_len(pos, c.inches));
        f.set_tab_radios(&align, &leader);
    }
    // Set needs a position (dimmed on an empty box, as the reference).
    let any = !f.form.text("tabPos").trim().is_empty();
    f.form.enable("set", any);
    f.refresh_tab_list();
    f
}

/// Page Setup: 539 × 717 window, three tabs (Margins / Paper / Layout), measured.
#[allow(clippy::too_many_lines)]
fn open_page_setup(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let v = &c.extra["r13"]["section"];
    let inches = c.inches;
    let mut f = DocForm::new(
        DocFormKind::PageSetup,
        &tr("doc13.ps_title", "Page Setup"),
        (523.0, 709.0),
        vec![
            tr("doc13.ps_tabMargins", "Margins"),
            tr("doc13.ps_tabPaper", "Paper"),
            tr("doc13.ps_tabLayout", "Layout"),
        ],
    );
    let len = |k: &str| show_len(v[k].as_i64().unwrap_or(0) as i32, inches);
    let width = v["width"].as_i64().unwrap_or(12240) as i32;
    let height = v["height"].as_i64().unwrap_or(15840) as i32;
    let sections = v["count"].as_u64().unwrap_or(1).max(1) as usize;
    f.r13 = Some(Box::new(R13 {
        inches,
        landscape: width > height,
        ..R13::default()
    }));
    let spin = |f: &mut DocForm,
                page: usize,
                id: &'static str,
                sp: &'static str,
                r: [f32; 4],
                val: &str| {
        f.form.add(
            id,
            Some(page),
            [r[0], r[1], r[2] - 19.0, r[3]],
            Kind::Edit {
                value: val.chars().collect(),
                caret: val.chars().count(),
            },
        );
        f.form.add(
            sp,
            Some(page),
            [r[0] + r[2] - 19.0, r[1], 19.0, r[3]],
            Kind::Canvas,
        );
    };
    // Margins.
    group_on(
        &mut f,
        Some(0),
        "gMargins",
        [24.0, 88.0, 483.0, 18.0],
        &tr("doc13.ps_margins", "Margins"),
    );
    #[allow(clippy::type_complexity)]
    let rows: [(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &str,
        &str,
        f32,
        f32,
    ); 4] = [
        (
            "mTopL",
            "mTop",
            "topSp",
            "marginTop",
            "doc13.ps_top",
            "Top:",
            38.0,
            113.0,
        ),
        (
            "mBottomL",
            "mBottom",
            "botSp",
            "marginBottom",
            "doc13.ps_bottom",
            "Bottom:",
            259.0,
            113.0,
        ),
        (
            "mLeftL",
            "mLeft",
            "lefSp",
            "marginLeft",
            "doc13.ps_leftL",
            "Left:",
            38.0,
            143.0,
        ),
        (
            "mRightL",
            "mRight",
            "rigSp",
            "marginRight",
            "doc13.ps_rightL",
            "Right:",
            259.0,
            143.0,
        ),
    ];
    for (lid, id, sp, key, k, fb, x, y) in rows {
        f.label_on(0, lid, [x, y + 5.0, 70.0, 18.0], &tr(k, fb));
        let bx = if x < 100.0 { 109.0 } else { 370.0 };
        spin(&mut f, 0, id, sp, [bx, y, 129.0, 26.0], &len(key));
    }
    f.label_on(
        0,
        "gutterL",
        [38.0, 179.0, 70.0, 18.0],
        &tr("doc13.ps_gutter", "Gutter:"),
    );
    spin(
        &mut f,
        0,
        "gutter",
        "gutSp",
        [109.0, 174.0, 129.0, 26.0],
        &len("gutter"),
    );
    f.label_on(
        0,
        "gutterPosL",
        [259.0, 179.0, 110.0, 18.0],
        &tr("doc13.ps_gutterPos", "Gutter position:"),
    );
    f.form.add(
        "gutterPos",
        Some(0),
        [370.0, 174.0, 129.0, 26.0],
        Kind::Combo {
            items: vec![
                tr("doc13.ps_left", "Left"),
                tr("doc13.ps_top", "Top").trim_end_matches(':').to_owned(),
            ],
            sel: usize::from(v["gutterTop"].as_bool() == Some(true)),
        },
    );
    group_on(
        &mut f,
        Some(0),
        "gOrient",
        [24.0, 212.0, 483.0, 18.0],
        &tr("doc13.ps_orientation", "Orientation"),
    );
    f.form
        .add("portrait", Some(0), [38.0, 237.0, 70.0, 57.0], Kind::Canvas);
    f.form.add(
        "landscape",
        Some(0),
        [123.0, 237.0, 70.0, 57.0],
        Kind::Canvas,
    );
    f.label_on(
        0,
        "portraitL",
        [38.0, 299.0, 80.0, 18.0],
        &tr("doc13.ps_portrait", "Portrait"),
    );
    f.label_on(
        0,
        "landscapeL",
        [127.0, 299.0, 90.0, 18.0],
        &tr("doc13.ps_landscape", "Landscape"),
    );
    group_on(
        &mut f,
        Some(0),
        "gPages",
        [24.0, 327.0, 483.0, 18.0],
        &tr("doc13.ps_pages", "Pages"),
    );
    f.label_on(
        0,
        "multiL",
        [38.0, 358.0, 125.0, 18.0],
        &tr("doc13.ps_multiple", "Multiple pages:"),
    );
    let multi = if v["bookFold"].as_bool() == Some(true) {
        3
    } else if v["twoOnOne"].as_bool() == Some(true) {
        2
    } else {
        usize::from(v["mirror"].as_bool() == Some(true))
    };
    f.form.add(
        "multi",
        Some(0),
        [165.0, 353.0, 150.0, 26.0],
        Kind::Combo {
            items: vec![
                tr("doc13.ps_normal", "Normal"),
                tr("doc13.ps_mirror", "Mirror margins"),
                tr("doc13.ps_twoOnOne", "2 pages per sheet"),
                tr("doc13.ps_bookFold", "Book fold"),
            ],
            sel: multi,
        },
    );
    // Paper.
    group_on(
        &mut f,
        Some(1),
        "gPaper",
        [24.0, 88.0, 483.0, 18.0],
        &tr("doc13.ps_paperSize", "Paper size:"),
    );
    let (pw, ph) = (width.min(height), width.max(height));
    let paper = PAPERS
        .iter()
        .position(|p| (p.0 - pw).abs() <= 20 && (p.1 - ph).abs() <= 20);
    let mut papers: Vec<String> = PAPERS.iter().map(|p| p.2.to_owned()).collect();
    papers.push(tr("doc13.ps_custom", "Custom size"));
    f.form.add(
        "paper",
        Some(1),
        [38.0, 113.0, 237.0, 27.0],
        Kind::Combo {
            sel: paper.unwrap_or(PAPERS.len()),
            items: papers,
        },
    );
    f.label_on(
        1,
        "pWidthL",
        [38.0, 150.0, 55.0, 18.0],
        &tr("doc13.ps_width", "Width:"),
    );
    spin(
        &mut f,
        1,
        "pWidth",
        "wSp",
        [93.0, 145.0, 182.0, 26.0],
        &show_len(width, inches),
    );
    f.label_on(
        1,
        "pHeightL",
        [38.0, 180.0, 55.0, 18.0],
        &tr("doc13.ps_height", "Height:"),
    );
    spin(
        &mut f,
        1,
        "pHeight",
        "hSp",
        [93.0, 175.0, 182.0, 26.0],
        &show_len(height, inches),
    );
    group_on(
        &mut f,
        Some(1),
        "gSource",
        [24.0, 212.0, 483.0, 18.0],
        &tr("doc13.ps_source", "Paper source"),
    );
    f.label_on(
        1,
        "firstL",
        [38.0, 238.0, 120.0, 18.0],
        &tr("doc13.ps_firstPage", "First page:"),
    );
    f.label_on(
        1,
        "otherL",
        [276.0, 238.0, 120.0, 18.0],
        &tr("doc13.ps_otherPages", "Other pages:"),
    );
    let trays = vec![
        tr("doc13.ps_defaultTray", "Default tray (Auto Select)"),
        tr("doc13.ps_autoSelect", "Auto Select"),
    ];
    f.form.add(
        "tray1",
        Some(1),
        [38.0, 258.0, 229.0, 155.0],
        Kind::List {
            items: trays.clone(),
            sel: Some(0),
            top: 0,
        },
    );
    f.form.add(
        "tray2",
        Some(1),
        [276.0, 258.0, 230.0, 155.0],
        Kind::List {
            items: trays,
            sel: Some(0),
            top: 0,
        },
    );
    f.button_on(
        1,
        "printOptions",
        [387.0, 616.0, 111.0, 28.0],
        &tr("doc13.ps_printOptions", "Print Options..."),
    );

    // Layout.
    group_on(
        &mut f,
        Some(2),
        "gSection",
        [24.0, 88.0, 483.0, 18.0],
        &tr("doc13.ps_section", "Section"),
    );
    f.label_on(
        2,
        "startL",
        [38.0, 119.0, 120.0, 18.0],
        &tr("doc13.ps_sectionStart", "Section start:"),
    );
    let start = STARTS
        .iter()
        .position(|s| Some(*s) == v["start"].as_str())
        .unwrap_or(2);
    f.form.add(
        "start",
        Some(2),
        [157.0, 114.0, 164.0, 26.0],
        Kind::Combo {
            items: vec![
                tr("doc13.ps_continuous", "Continuous"),
                tr("doc13.ps_newColumn", "New column"),
                tr("doc13.ps_newPage", "New page"),
                tr("doc13.ps_evenPage", "Even page"),
                tr("doc13.ps_oddPage", "Odd page"),
            ],
            sel: start,
        },
    );
    f.check_on(
        2,
        "noEndnotes",
        [38.0, 147.0, 220.0, 20.0],
        &tr("doc13.ps_suppressEndnotes", "Suppress endnotes"),
        false,
    );
    f.disable("noEndnotes");
    group_on(
        &mut f,
        Some(2),
        "gHf",
        [24.0, 177.0, 483.0, 18.0],
        &tr("doc13.ps_headersFooters", "Headers and footers"),
    );
    f.check_on(
        2,
        "evenOdd",
        [38.0, 204.0, 260.0, 20.0],
        &tr("doc13.ps_evenOdd", "Different odd and even"),
        v["evenOdd"].as_bool() == Some(true),
    );
    f.check_on(
        2,
        "firstPage",
        [38.0, 229.0, 260.0, 20.0],
        &tr("doc13.ps_differentFirst", "Different first page"),
        v["titlePage"].as_bool() == Some(true),
    );
    f.label_on(
        2,
        "edgeL",
        [38.0, 270.0, 100.0, 18.0],
        &tr("doc13.ps_fromEdge", "From edge:"),
    );
    f.label_on(
        2,
        "hdrL",
        [164.0, 257.0, 60.0, 18.0],
        &tr("doc13.ps_header", "Header:"),
    );
    spin(
        &mut f,
        2,
        "hdr",
        "hdrSp",
        [219.0, 252.0, 110.0, 26.0],
        &len("header"),
    );
    f.label_on(
        2,
        "ftrL",
        [164.0, 287.0, 60.0, 18.0],
        &tr("doc13.ps_footer", "Footer:"),
    );
    spin(
        &mut f,
        2,
        "ftr",
        "ftrSp",
        [219.0, 282.0, 110.0, 26.0],
        &len("footer"),
    );
    group_on(
        &mut f,
        Some(2),
        "gPage",
        [24.0, 319.0, 483.0, 18.0],
        &tr("doc13.ps_page", "Page"),
    );
    f.label_on(
        2,
        "vAlignL",
        [38.0, 350.0, 120.0, 18.0],
        &tr("doc13.ps_vAlign", "Vertical alignment:"),
    );
    let va = VALIGNS
        .iter()
        .position(|s| Some(*s) == v["vAlign"].as_str())
        .unwrap_or(0);
    f.form.add(
        "vAlign",
        Some(2),
        [157.0, 345.0, 172.0, 26.0],
        Kind::Combo {
            items: vec![
                tr("doc13.ps_vTop", "Top"),
                tr("doc13.ps_vCenter", "Center"),
                tr("doc13.ps_vJustified", "Justified"),
                tr("doc13.ps_vBottom", "Bottom"),
            ],
            sel: va,
        },
    );
    f.button_on(
        2,
        "lineNumbers",
        [255.0, 616.0, 116.0, 28.0],
        &tr("doc13.ps_lineNumbers", "Line Numbers..."),
    );
    f.button_on(
        2,
        "borders",
        [387.0, 616.0, 111.0, 28.0],
        &tr("doc13.ps_borders", "Borders..."),
    );
    // Every tab: preview, Apply to, Set As Default, OK, Cancel.
    f.group(
        "gPreview",
        [24.0, 431.0, 483.0, 18.0],
        &tr("doc13.ps_preview", "Preview"),
    );
    f.canvas("psPreview", [24.0, 452.0, 483.0, 145.0]);
    f.label(
        "applyL",
        [24.0, 622.0, 66.0, 18.0],
        &tr("doc13.ps_applyTo", "Apply to:"),
    );
    let mut scopes = Vec::new();
    if sections > 1 {
        scopes.push(("section", tr("doc13.ps_thisSection", "This section")));
    }
    scopes.push(("document", tr("doc13.ps_wholeDocument", "Whole document")));
    scopes.push(("forward", tr("doc13.ps_forward", "This point forward")));
    f.ids = scopes.iter().map(|s| s.0.to_owned()).collect();
    f.combo(
        "applyTo",
        [90.0, 617.0, 150.0, 26.0],
        scopes.into_iter().map(|s| s.1).collect(),
        0,
    );
    f.button(
        "setDefault",
        [10.0, 670.0, 112.0, 29.0],
        &tr("docR3.setAsDefault", "Set As Default"),
        false,
    );
    f.button(
        "ok",
        [273.0, 670.0, 112.0, 29.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [400.0, 670.0, 112.0, 29.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// Paint a round-13 canvas; `false` when `id` is not one.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
) -> bool {
    let k = f.form.scale_k();
    if SPINS.iter().any(|s| s.0 == id) && (handles(f.kind) || f.r13.is_some()) {
        let edit = SPINS.iter().find(|s| s.0 == id).map_or("", |s| s.1);
        let enabled = f.form.get(edit).is_none_or(|c| c.enabled);
        paint_spin(p, r, enabled, k);
        return true;
    }
    match (f.kind, id) {
        (DocFormKind::PageSetup, "portrait" | "landscape") => {
            let on = (id == "landscape") == f.r13().landscape;
            paint_orientation(p, r, id == "landscape", on, k);
            true
        }
        (DocFormKind::BorderOptions, "boPreview") => {
            paint_border_options(f, p, r, k);
            true
        }
        (DocFormKind::PageSetup, "psPreview") => {
            paint_page_preview(f, p, r, k);
            true
        }
        (DocFormKind::NewStyle, _) => paint_style_canvas(f, id, r, p, font, k),
        _ => false,
    }
}

fn paint_spin(p: &mut dyn Painter, r: [f32; 4], enabled: bool, k: f32) {
    // The reference's spin box: two stacked buttons (#ADADAD frame, #E1E1E1 face) beside
    // the field, each with a small filled triangle.
    let edge = Rgba(0xad, 0xad, 0xad, 255);
    let face = Rgba(0xe1, 0xe1, 0xe1, 255);
    let half = (r[3] / 2.0).floor();
    let ink = if enabled {
        Rgba(0, 0, 0, 255)
    } else {
        Rgba(0xbc, 0xbc, 0xbc, 255)
    };
    for (y, h, up) in [(r[1], half, true), (r[1] + half, r[3] - half, false)] {
        p.fill_rect(r[0], y, r[2], h, edge);
        p.fill_rect(r[0] + 1.0, y + 1.0, r[2] - 2.0, h - 2.0, face);
        let (cx, cy) = (r[0] + r[2] / 2.0, y + h / 2.0);
        let (w, th) = (3.0 * k, 2.4 * k);
        let s = if up { -1.0 } else { 1.0 };
        p.path(
            &[
                PathSeg::M(cx - w, cy - s * th / 2.0),
                PathSeg::L(cx + w, cy - s * th / 2.0),
                PathSeg::L(cx, cy + s * th / 2.0),
                PathSeg::Z,
            ],
            Some(ink),
            None,
        );
    }
}

/// Portrait / Landscape tiles: a framed tile (blue frame when chosen) with a page whose top
/// corner is folded (own drawing).
fn paint_orientation(p: &mut dyn Painter, r: [f32; 4], land: bool, on: bool, k: f32) {
    let t1 = k.round().max(1.0);
    let frame = if on {
        Rgba(0x00, 0x78, 0xd4, 255)
    } else {
        Rgba(0xad, 0xad, 0xad, 255)
    };
    let bw = if on { 2.0 * t1 } else { t1 };
    p.fill_rect(r[0], r[1], r[2], r[3], frame);
    p.fill_rect(
        r[0] + bw,
        r[1] + bw,
        r[2] - 2.0 * bw,
        r[3] - 2.0 * bw,
        Rgba(255, 255, 255, 255),
    );
    let (pw, ph) = if land {
        (32.0 * k, 22.0 * k)
    } else {
        (22.0 * k, 30.0 * k)
    };
    let x = (r[0] + (r[2] - pw) / 2.0).round();
    let y = (r[1] + (r[3] - ph) / 2.0).round();
    let fold = 7.0 * k;
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    p.path(
        &[
            PathSeg::M(x, y),
            PathSeg::L(x + pw - fold, y),
            PathSeg::L(x + pw, y + fold),
            PathSeg::L(x + pw, y + ph),
            PathSeg::L(x, y + ph),
            PathSeg::Z,
        ],
        Some(Rgba(255, 255, 255, 255)),
        Some((ink, t1)),
    );
    p.path(
        &[
            PathSeg::M(x + pw - fold, y),
            PathSeg::L(x + pw - fold, y + fold),
            PathSeg::L(x + pw, y + fold),
        ],
        None,
        Some((ink, t1)),
    );
}

/// The page preview: a page of the chosen proportions with grey text lines inside its
/// margins (two pages side by side for mirror margins / 2 pages per sheet / book fold).
fn paint_page_preview(f: &DocForm, p: &mut dyn Painter, r: [f32; 4], k: f32) {
    let per = f.len_per();
    let num = |id: &str, d: i32| parse_len(&f.form.text(id), per).unwrap_or(d).max(0);
    let (w, h) = (
        num("pWidth", 12240).max(1440),
        num("pHeight", 15840).max(1440),
    );
    let (mt, mb, ml, mr) = (
        num("mTop", 1440),
        num("mBottom", 1440),
        num("mLeft", 1440),
        num("mRight", 1440),
    );
    let gutter = num("gutter", 0);
    let multi = f.sel("multi").unwrap_or(0);
    let pages = if multi == 0 { 1 } else { 2 };
    let box_h = 132.0 * k;
    let scale = box_h / h.max(w) as f32;
    let (pw, ph) = (w as f32 * scale, h as f32 * scale);
    let total = pw * pages as f32;
    let x0 = (r[0] + 78.0 * k - (total - 104.0 * k) / 2.0).round();
    let y0 = (r[1] + 6.0 * k + (box_h - ph) / 2.0).round();
    let line = Rgba(0x30, 0x30, 0x30, 255);
    for i in 0..pages {
        let x = x0 + i as f32 * pw;
        // The reference's page casts a grey shadow right and below.
        let sh = (2.0 * k).round();
        p.fill_rect(
            x + sh,
            y0 + sh,
            pw.round(),
            ph.round(),
            Rgba(0x80, 0x80, 0x80, 255),
        );
        p.fill_rect(x, y0, pw.round(), ph.round(), Rgba(0, 0, 0, 255));
        p.fill_rect(
            x + 1.0,
            y0 + 1.0,
            pw.round() - 2.0,
            ph.round() - 2.0,
            Rgba(255, 255, 255, 255),
        );
        // Mirrored: the left page's inside margin is on its right.
        let (l, rr) = if multi == 1 && i == 0 {
            (mr, ml + gutter)
        } else {
            (ml + gutter, mr)
        };
        let lx = x + l as f32 * scale;
        let lw = (pw - (l + rr) as f32 * scale).max(4.0);
        let mut y = y0 + mt as f32 * scale;
        let bottom = y0 + ph - mb as f32 * scale;
        // Paragraphs of text lines: four full lines and a short last one, a blank line
        // between paragraphs (the reference's sample page).
        let mut n = 0;
        while y + 1.0 < bottom {
            match n % 6 {
                5 => {}
                m => {
                    let ww = if m == 4 { lw * 0.45 } else { lw };
                    p.fill_rect(lx.round(), y.round(), ww.round(), k.round().max(1.0), line);
                }
            }
            y += 4.0 * k;
            n += 1;
        }
    }
}

/// Toggle and alignment buttons of the New / Modify Style formatting rows.
fn paint_style_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
    k: f32,
) -> bool {
    let st = f.r13();
    let on = match id {
        "sBold" => st.toggles[0],
        "sItalic" => st.toggles[1],
        "sUnder" => st.toggles[2],
        "aLeft" => st.align == Some(0),
        "aCenter" => st.align == Some(1),
        "aRight" => st.align == Some(2),
        "aJust" => st.align == Some(3),
        "lSingle" => st.line == Some(0),
        "l15" => st.line == Some(1),
        "lDouble" => st.line == Some(2),
        "spMore" | "spLess" | "inMore" | "inLess" => false,
        "preview" => {
            paint_style_preview(f, r, p, font, k);
            return true;
        }
        "descBox" => {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xab, 0xad, 0xb3, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            p.fill_rect(
                r[0] + 2.0,
                r[1] + 2.0,
                r[2] - 3.0,
                r[3] - 3.0,
                Rgba(0xf0, 0xf0, 0xf0, 255),
            );
            return true;
        }
        id if id.starts_with("sSep") => {
            p.fill_rect(
                r[0],
                r[1],
                k.round().max(1.0),
                r[3],
                Rgba(0xc8, 0xc8, 0xc8, 255),
            );
            return true;
        }
        _ => return false,
    };
    if on {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x98, 0xc6, 0xf0, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(0xcc, 0xe4, 0xf7, 255),
        );
    }
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    let t1 = k.round().max(1.0);
    let cx = r[0] + r[2] / 2.0;
    let cy = r[1] + r[3] / 2.0;
    match id {
        "sBold" | "sItalic" | "sUnder" => {
            let g = match id {
                "sBold" => "B",
                "sItalic" => "I",
                _ => "U",
            };
            let fnt = Font {
                family: "Segoe UI",
                size: font.size * 1.1,
                weight: if id == "sBold" { 700 } else { 600 },
                italic: id == "sItalic",
                underline: id == "sUnder",
            };
            p.text_font(
                g,
                cx - font.size * 0.32,
                r[1] + (r[3] - fnt.size * 1.33) / 2.0,
                &fnt,
                ink,
            );
        }
        "aLeft" | "aCenter" | "aRight" | "aJust" => {
            let w = 16.0 * k;
            for i in 0..4 {
                let lw = if id == "aJust" || i % 2 == 0 {
                    w
                } else {
                    w * 0.65
                };
                let x = match id {
                    "aCenter" => cx - lw / 2.0,
                    "aRight" => cx + w / 2.0 - lw,
                    _ => cx - w / 2.0,
                };
                p.fill_rect(
                    x.round(),
                    (cy - 6.0 * k + i as f32 * 4.0 * k).round(),
                    lw.round(),
                    t1,
                    ink,
                );
            }
        }
        "lSingle" | "l15" | "lDouble" => {
            let gap = match id {
                "lSingle" => 3.0,
                "l15" => 4.5,
                _ => 6.0,
            } * k;
            let w = 13.0 * k;
            for i in 0..3 {
                let y = cy - gap + i as f32 * gap;
                p.fill_rect(
                    (cx - w / 2.0).round(),
                    y.round(),
                    w.round(),
                    (1.5 * k).round().max(1.0),
                    ink,
                );
            }
        }
        _ => {
            // Space / indent more or less: lines with an arrow (own drawing).
            let w = 12.0 * k;
            let x = cx - 2.0 * k;
            for i in 0..3 {
                p.fill_rect(
                    x.round(),
                    (cy - 5.0 * k + i as f32 * 5.0 * k).round(),
                    w.round(),
                    t1,
                    ink,
                );
            }
            let blue = Rgba(0x2b, 0x57, 0x9a, 255);
            let up = matches!(id, "spMore" | "inLess");
            let ax = cx - 7.0 * k;
            let s = if up { -1.0 } else { 1.0 };
            p.fill_rect(
                (ax - t1 / 2.0).round(),
                (cy - 6.0 * k).round(),
                t1,
                (12.0 * k).round(),
                blue,
            );
            p.path(
                &[
                    PathSeg::M(ax - 3.0 * k, cy + s * 3.0 * k),
                    PathSeg::L(ax + 3.0 * k, cy + s * 3.0 * k),
                    PathSeg::L(ax, cy + s * 7.0 * k),
                    PathSeg::Z,
                ],
                Some(blue),
                None,
            );
        }
    }
    true
}

/// New / Modify Style: 669 × 703 window, measured.
#[allow(clippy::too_many_lines)]
pub(super) fn open_new_style(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let s_ = s;
    let mut f = DocForm::new(
        DocFormKind::NewStyle,
        &tr("docR3.newStyle", "Create New Style from Formatting"),
        (653.0, 695.0),
        vec![],
    );
    f.group(
        "props",
        [10.0, 48.0, 633.0, 18.0],
        &tr("docR3.properties", "Properties"),
    );
    f.label(
        "nameL",
        [24.0, 78.0, 180.0, 18.0],
        &tr("docR3.name", "Name:"),
    );
    f.edit(
        "name",
        [212.0, 73.0, 431.0, 26.0],
        &tr("docR3.style1", "Style1"),
    );
    f.label(
        "typeL",
        [24.0, 109.0, 180.0, 18.0],
        &tr("docR3.styleType", "Style type:"),
    );
    f.combo(
        "type",
        [212.0, 104.0, 431.0, 26.0],
        vec![
            tr("docR3.typeLinked", "Linked (paragraph and character)"),
            tr("docR3.typeCharacter", "Character"),
            tr("docR3.typeParagraph", "Paragraph"),
        ],
        2,
    );
    let paras: Vec<&(String, String, bool)> = c.styles.iter().filter(|s| s.2).collect();
    let mut names = vec![tr("docR3.noStyle", "(no style)")];
    // Built-in styles by their interface names ("heading 1" reads Heading 1).
    // Each name after its kind mark, as the reference (¶a linked, ¶ paragraph).
    names.extend(paras.iter().map(|s| {
        let name =
            crate::commands::style_name_key(&s.1).map_or_else(|| s.1.clone(), |k| t(s_, k, &s.1));
        format!("{}  {name}", style_mark(&s.0))
    }));
    f.style_ids = paras.iter().map(|s| s.0.clone()).collect();
    let current = c.extra["r13"]["style"].as_str().unwrap_or("Normal");
    let based = paras
        .iter()
        .position(|s| s.0 == current)
        .or_else(|| paras.iter().position(|s| s.0 == "Normal"))
        .map_or(0, |i| i + 1);
    f.label(
        "basedL",
        [24.0, 140.0, 180.0, 18.0],
        &tr("docR3.basedOn", "Style based on:"),
    );
    f.combo("basedOn", [212.0, 135.0, 431.0, 26.0], names.clone(), based);
    f.label(
        "nextL",
        [24.0, 171.0, 186.0, 18.0],
        &tr("docR3.following", "Style for following paragraph:"),
    );
    // Following paragraph: the new style itself first (no `w:next`, §17.7.4.10).
    let mut next = names;
    next[0] = tr("docR3.style1", "Style1");
    f.combo("next", [212.0, 166.0, 431.0, 26.0], next, 0);
    f.group(
        "fmt",
        [10.0, 204.0, 633.0, 18.0],
        &tr("docR3.formatting", "Formatting"),
    );
    // A theme font reads "Calibri Light (Headings)" / "Calibri (Body)", as the reference.
    let theme = &c.extra["r13"]["themeFonts"];
    let shown = if theme["major"].as_str() == Some(c.font.as_str()) {
        format!("{} {}", c.font, tr("doc13.ns_headings", "(Headings)"))
    } else if theme["minor"].as_str() == Some(c.font.as_str()) {
        format!("{} {}", c.font, tr("doc13.ns_body", "(Body)"))
    } else {
        c.font.clone()
    };
    let mut fonts = vec![shown];
    for x in [
        "Times New Roman",
        "Arial",
        "Calibri",
        "Cambria",
        "Calibri Light",
    ] {
        if !fonts.iter().any(|f| f == x) {
            fonts.push(x.to_owned());
        }
    }
    f.combo("font", [24.0, 231.0, 141.0, 26.0], fonts, 0);
    let sizes: Vec<String> = [
        8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72,
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let cur = format!("{}", c.size as f32 / 2.0);
    let si = sizes.iter().position(|v| *v == cur).unwrap_or(3);
    f.combo("size", [170.0, 231.0, 48.0, 26.0], sizes, si);
    for (id, x) in [("sBold", 230.0), ("sItalic", 263.0), ("sUnder", 296.0)] {
        f.canvas(id, [x, 231.0, 30.0, 26.0]);
    }
    let color = c.extra["r13"]["color"].as_str().unwrap_or("").to_owned();
    f.form.add(
        "color",
        None,
        [339.0, 231.0, 143.0, 26.0],
        Kind::ColorPick {
            hex: color,
            palette: c.extra["palette"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        },
    );
    let row2 = [
        ("aLeft", 25.0),
        ("aCenter", 68.0),
        ("aRight", 111.0),
        ("aJust", 154.0),
        ("sSep1", 197.0),
        ("lSingle", 214.0),
        ("l15", 256.0),
        ("lDouble", 299.0),
        ("sSep2", 342.0),
        ("spMore", 357.0),
        ("spLess", 400.0),
        ("sSep3", 457.0),
        ("inLess", 472.0),
        ("inMore", 515.0),
    ];
    for (id, x) in row2 {
        let w = if id.starts_with("sSep") { 1.0 } else { 30.0 };
        f.canvas(id, [x, 274.0, w, 26.0]);
    }
    let sample = c.extra["r13"]["sample"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_owned();
    f.preview = vec![
        tr("doc13.para_pvBefore", "Previous Paragraph"),
        if sample.is_empty() {
            tr("docR3.sampleText", "Sample Text Sample Text Sample Text")
        } else {
            sample
        },
        tr("doc13.para_pvAfter", "Following Paragraph"),
    ];
    f.canvas("preview", [25.0, 306.0, 618.0, 200.0]);
    f.form
        .add("descBox", None, [25.0, 511.0, 618.0, 72.0], Kind::Canvas);
    f.label("desc1", [29.0, 515.0, 610.0, 18.0], "");
    f.label("desc2", [45.0, 532.0, 594.0, 18.0], "");
    f.check(
        "gallery",
        [11.0, 596.0, 180.0, 20.0],
        &tr("docR3.addGallery", "Add to the Styles gallery"),
        true,
    );
    f.check(
        "auto",
        [193.0, 596.0, 200.0, 20.0],
        &tr("docR3.autoUpdate", "Automatically update"),
        false,
    );
    f.radio(
        "thisDoc",
        "scope",
        [11.0, 621.0, 170.0, 20.0],
        &tr("docR3.thisDoc", "Only in this document"),
        true,
    );
    f.radio(
        "template",
        "scope",
        [176.0, 621.0, 300.0, 20.0],
        &tr("docR3.newDocs", "New documents based on this template"),
        false,
    );
    f.button(
        "format",
        [11.0, 656.0, 110.0, 29.0],
        &tr("doc13.ns_format", "Format ▾"),
        false,
    );
    f.button(
        "ok",
        [403.0, 656.0, 112.0, 29.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [530.0, 656.0, 112.0, 29.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.r13 = Some(Box::new(R13 {
        inches: c.inches,
        spec: c.extra["r13"]["spec"].clone(),
        ..R13::default()
    }));
    let spec = c.extra["r13"]["spec"].clone();
    if let Some(r) = f.r13.as_mut() {
        r.toggles = [
            spec["bold"].as_bool() == Some(true),
            spec["italic"].as_bool() == Some(true),
            spec["underline"].as_bool() == Some(true),
        ];
        r.align = align_index(&spec["align"]);
        r.line = match spec["lineSpacing"].as_u64() {
            Some(240) => Some(0),
            Some(360) => Some(1),
            Some(480) => Some(2),
            _ => None,
        };
    }
    f.words.extend(words(s));
    // The reference opens with the name selected and its alignment button pressed.
    f.form.focus("name");
    f.form.select_all("name");
    if let Some(r) = f.r13.as_mut() {
        r.align = r.align.or(Some(0));
        r.base = spec;
    }
    f.style_describe();
    f
}

/// Index (0 left … 3 justified) of an alignment written either as `ST_Jc` (`both`) or as
/// `bo_model::Alignment` (`Justify`).
fn align_index(v: &Value) -> Option<usize> {
    match v.as_str()?.to_ascii_lowercase().as_str() {
        "left" | "start" => Some(0),
        "center" => Some(1),
        "right" | "end" => Some(2),
        "both" | "justify" => Some(3),
        _ => None,
    }
}

/// The kind mark shown before a paragraph style's name: built-in styles the reference links
/// to a character style get "¶a", the others "¶".
fn style_mark(id: &str) -> &'static str {
    let linked =
        id.starts_with("Heading") || matches!(id, "Title" | "Subtitle" | "Quote" | "IntenseQuote");
    if linked {
        "\u{b6}a"
    } else {
        "\u{b6}"
    }
}

fn group_on(f: &mut DocForm, page: Option<usize>, id: &'static str, r: [f32; 4], text: &str) {
    f.form.add(id, page, r, Kind::Group(text.to_owned()));
}

/// New / Modify Style preview: the paragraphs before and after in grey, the selection's text
/// in the style's font, size, colour and alignment between them (own sample words).
fn paint_style_preview(f: &DocForm, r: [f32; 4], p: &mut dyn Painter, font: &Font<'_>, k: f32) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xab, 0xad, 0xb3, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
    let spec = f.style_spec();
    let grey = Rgba(0xc0, 0xc0, 0xc0, 255);
    let small = Font {
        family: "Times New Roman",
        size: (11.9 * k).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    // Measured on the reference at 125 %: text 31 px in, 80 px from the right, 16 px lines.
    let x = r[0] + 25.0 * k;
    let w = r[2] - 89.0 * k;
    let lh = (13.0 * k).round();
    let per_line = ((w / (small.size * 0.47)).floor() as usize).max(8);
    let lines_of = |text: &str, reps: usize| -> Vec<String> {
        let all = vec![text; reps].join(" ");
        let mut out = Vec::new();
        let mut line = String::new();
        for word in all.split(' ') {
            if line.chars().count() + word.chars().count() + 1 > per_line && !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            out.push(line);
        }
        out
    };
    let mut y = r[1] + 12.0 * k;
    for l in lines_of(f.preview.first().map_or("", String::as_str), 9) {
        p.text_font(&l, x, y, &small, grey);
        y += lh;
    }
    y += 18.0 * k;
    #[allow(clippy::cast_precision_loss)]
    let pt = spec["size"].as_u64().map_or(11.0, |hp| hp as f32 / 2.0);
    let family = spec["font"].as_str().unwrap_or("Calibri").to_owned();
    let big = Font {
        family: family.as_str(),
        size: (pt.min(16.0) * 1.1 * k).round(),
        weight: if spec["bold"].as_bool() == Some(true) {
            700
        } else {
            400
        },
        italic: spec["italic"].as_bool() == Some(true),
        underline: spec["underline"].as_bool() == Some(true),
    };
    let ink = spec["color"]
        .as_str()
        .and_then(|h| crate::theme::parse_color(&format!("#{h}")))
        .unwrap_or(Rgba(0, 0, 0, 255));
    let sample: String = f
        .preview
        .get(1)
        .map_or("", String::as_str)
        .chars()
        .take(60)
        .collect();
    let tw = sample.chars().count() as f32 * big.size * 0.5;
    let sx = match align_index(&spec["align"]) {
        Some(1) => r[0] + (r[2] - tw) / 2.0,
        Some(2) => r[0] + r[2] - 32.0 * k - tw,
        _ => x,
    };
    p.text_font(&sample, sx, y, &big, ink);
    y += big.size * 1.5 + 14.0 * k;
    let _ = font;
    for l in lines_of(f.preview.get(2).map_or("", String::as_str), 30) {
        if y > r[1] + r[3] {
            break;
        }
        p.text_font(&l, x, y, &small, grey);
        y += lh;
    }
    p.pop_clip();
}

// ------------------------------------------------------------ Manage Styles › Set Defaults

/// Manage Styles › Set Defaults, laid out from the reference's page (629 × 661 window):
/// Font / Size lists with their boxes, Font color, Paragraph Position (Left, Right, Special,
/// By, Alignment) and Paragraph Spacing (Before, After, Line spacing, At). `d` holds the
/// document defaults (§17.7.5.1 docDefaults) as `FindFormat` fields, the font families and
/// the colour palette. OK sets the changed fields (`setDocDefaults`).
pub(crate) fn manage_defaults(f: &mut DocForm, s: &Strings, d: &Value) {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let inches = d["inches"].as_bool().unwrap_or(true);
    f.r13 = Some(Box::new(R13 {
        inches,
        ..R13::default()
    }));
    let pg = Some(3);
    if let Some(cur) = d["current"].as_str() {
        f.words.push(("msCurrent".into(), cur.to_owned()));
    }
    f.label_on(
        3,
        "dFontL",
        [57.0, 88.0, 100.0, 18.0],
        &tr("docR5.font", "Font:"),
    );
    let font = d["font"].as_str().unwrap_or("").to_owned();
    f.edit_on(3, "font", [57.0, 108.0, 267.0, 24.0], &font);
    let mut fonts: Vec<String> = d["fonts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    fonts.sort_by_key(|n| n.to_lowercase());
    fonts.dedup();
    fonts.retain(|n| !n.starts_with('.') && !n.starts_with('@'));
    fonts.splice(0..0, ["+Body".to_owned(), "+Headings".to_owned()]);
    let fsel = fonts.iter().position(|n| *n == font);
    f.form.add(
        "fontList",
        pg,
        [57.0, 134.0, 267.0, 88.0],
        Kind::List {
            items: fonts,
            sel: fsel,
            top: fsel.unwrap_or(0),
        },
    );
    f.label_on(
        3,
        "dSizeL",
        [483.0, 88.0, 100.0, 18.0],
        &tr("docR5.size", "Size:"),
    );
    let size = d["size"].as_u64().map_or_else(String::new, |hp| {
        #[allow(clippy::cast_precision_loss)]
        let pt = hp as f32 / 2.0;
        format!("{pt}")
    });
    f.edit_on(3, "size", [483.0, 108.0, 114.0, 24.0], &size);
    let sizes: Vec<String> = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72]
        .iter()
        .map(ToString::to_string)
        .collect();
    let ssel = sizes.iter().position(|x| *x == size);
    f.form.add(
        "sizeList",
        pg,
        [483.0, 134.0, 114.0, 88.0],
        Kind::List {
            items: sizes,
            sel: ssel,
            top: ssel.map_or(0, |i| i.saturating_sub(4)),
        },
    );
    f.label_on(
        3,
        "dColorL",
        [57.0, 236.0, 120.0, 18.0],
        &tr("findFmt11.fontColorL", "Font color:"),
    );
    f.form.add(
        "color",
        pg,
        [181.0, 232.0, 142.0, 26.0],
        Kind::ColorPick {
            hex: d["color"].as_str().unwrap_or("").to_owned(),
            palette: d["palette"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        },
    );
    group_on(
        f,
        pg,
        "dPosG",
        [43.0, 262.0, 554.0, 18.0],
        &tr("doc13.md_position", "Paragraph Position"),
    );
    let len = |k: &str| show_len(d[k].as_i64().unwrap_or(0) as i32, inches);
    let spin = |f: &mut DocForm, id: &'static str, sp: &'static str, r: [f32; 4], val: &str| {
        f.form.add(
            id,
            pg,
            [r[0], r[1], r[2] - 19.0, r[3]],
            Kind::Edit {
                value: val.chars().collect(),
                caret: val.chars().count(),
            },
        );
        f.form
            .add(sp, pg, [r[0] + r[2] - 19.0, r[1], 19.0, r[3]], Kind::Canvas);
    };
    f.label_on(
        3,
        "insideL",
        [57.0, 292.0, 100.0, 18.0],
        &tr("doc13.ps_leftL", "Left:"),
    );
    spin(
        f,
        "inside",
        "inSp",
        [181.0, 288.0, 141.0, 26.0],
        &len("indentLeft"),
    );
    f.label_on(
        3,
        "outsideL",
        [57.0, 328.0, 100.0, 18.0],
        &tr("doc13.ps_rightL", "Right:"),
    );
    spin(
        f,
        "outside",
        "outSp",
        [181.0, 324.0, 141.0, 26.0],
        &len("indentRight"),
    );
    f.label_on(
        3,
        "specialL",
        [338.0, 292.0, 120.0, 18.0],
        &tr("findFmt11.specialL", "Special:"),
    );
    let first = d["firstLine"].as_i64().unwrap_or(0) as i32;
    f.form.add(
        "special",
        pg,
        [338.0, 324.0, 130.0, 26.0],
        Kind::Combo {
            items: vec![
                tr("findFmt11.none", "(none)"),
                tr("findFmt11.firstLine", "First line"),
                tr("findFmt11.hanging", "Hanging"),
            ],
            sel: match first {
                0 => 0,
                x if x > 0 => 1,
                _ => 2,
            },
        },
    );
    f.label_on(
        3,
        "byL",
        [483.0, 292.0, 60.0, 18.0],
        &tr("findFmt11.byL", "By:"),
    );
    let by = if first == 0 {
        String::new()
    } else {
        show_len(first.abs(), inches)
    };
    spin(f, "by", "bySp", [482.0, 324.0, 115.0, 26.0], &by);
    f.label_on(
        3,
        "alignL",
        [57.0, 360.0, 120.0, 18.0],
        &tr("findFmt11.alignmentL", "Alignment:"),
    );
    let a = ALIGNS
        .iter()
        .position(|x| Some(*x) == d["align"].as_str())
        .unwrap_or(0);
    f.form.add(
        "align",
        pg,
        [181.0, 356.0, 287.0, 26.0],
        Kind::Combo {
            items: [
                ("findFmt11.left", "Left"),
                ("findFmt11.centered", "Centered"),
                ("findFmt11.right", "Right"),
                ("findFmt11.justified", "Justified"),
            ]
            .iter()
            .map(|(k, fb)| tr(k, fb))
            .collect(),
            sel: a,
        },
    );
    group_on(
        f,
        pg,
        "dSpG",
        [43.0, 388.0, 554.0, 18.0],
        &tr("doc13.md_spacing", "Paragraph Spacing"),
    );
    f.label_on(
        3,
        "beforeL",
        [57.0, 418.0, 100.0, 18.0],
        &tr("findFmt11.beforeL", "Before:"),
    );
    let pt = |k: &str| show_pt(d[k].as_i64().unwrap_or(0) as i32);
    spin(
        f,
        "before",
        "befSp",
        [181.0, 414.0, 141.0, 26.0],
        &pt("spaceBefore"),
    );
    f.label_on(
        3,
        "afterL",
        [57.0, 454.0, 100.0, 18.0],
        &tr("findFmt11.afterL", "After:"),
    );
    spin(
        f,
        "after",
        "aftSp",
        [181.0, 450.0, 141.0, 26.0],
        &pt("spaceAfter"),
    );
    f.label_on(
        3,
        "lineL",
        [338.0, 418.0, 120.0, 18.0],
        &tr("findFmt11.lineSpacingL", "Line spacing:"),
    );
    let (li, at) = match d["lineSpacing"].as_str().unwrap_or("single") {
        "1.5" => (1, String::new()),
        "double" => (2, String::new()),
        x => match x.split_once(':') {
            Some(("atLeast", n)) => (3, show_pt(n.parse().unwrap_or(240))),
            Some(("exactly", n)) => (4, show_pt(n.parse().unwrap_or(240))),
            Some(("multiple", n)) => {
                let l = n.parse::<f32>().unwrap_or(240.0) / 240.0;
                let s = format!("{l:.2}");
                (5, s.trim_end_matches('0').trim_end_matches('.').to_owned())
            }
            _ => (0, String::new()),
        },
    };
    f.form.add(
        "line",
        pg,
        [338.0, 450.0, 130.0, 26.0],
        Kind::Combo {
            items: [
                ("findFmt11.single", "Single"),
                ("findFmt11.lines15", "1.5 lines"),
                ("findFmt11.double", "Double"),
                ("findFmt11.atLeast", "At least"),
                ("findFmt11.exactly", "Exactly"),
                ("findFmt11.multiple", "Multiple"),
            ]
            .iter()
            .map(|(k, fb)| tr(k, fb))
            .collect(),
            sel: li,
        },
    );
    f.label_on(
        3,
        "atL",
        [483.0, 418.0, 60.0, 18.0],
        &tr("findFmt11.atL", "At:"),
    );
    spin(f, "at", "atSp", [482.0, 450.0, 115.0, 26.0], &at);
    let initial = f.defaults_values();
    if let Some(r) = f.r13.as_mut() {
        r.initial = Value::Object(initial);
    }
}

impl DocForm {
    /// Set Defaults: the page's values as `FindFormat` fields.
    fn defaults_values(&self) -> Map<String, Value> {
        let mut m = self.paragraph_values();
        let font = self.form.text("font");
        if !font.trim().is_empty() {
            m.insert("font".into(), json!(font.trim()));
        }
        if let Ok(pt) = self
            .form
            .text("size")
            .trim()
            .replace(',', ".")
            .parse::<f32>()
        {
            m.insert(
                "size".into(),
                json!((pt * 2.0).round().clamp(2.0, 3276.0) as u32),
            );
        }
        let c = self.form.color("color");
        if !c.is_empty() && c != super::form::NO_COLOR {
            m.insert("color".into(), json!(c));
        }
        m
    }

    /// Set Defaults: the fields changed since the dialog opened (empty: nothing to set).
    pub(crate) fn defaults_changes(&self) -> Map<String, Value> {
        if self.r13.is_none() {
            return Map::new();
        }
        let now = self.defaults_values();
        let was = &self.r13().initial;
        now.into_iter().filter(|(k, v)| was[k] != *v).collect()
    }

    /// Set Defaults: a list choice fills its box; Special / Line spacing as in Paragraph.
    pub(crate) fn defaults_changed(&mut self, id: &str) {
        match id {
            "fontList" | "sizeList" => {
                if let Some(i) = self.sel(id) {
                    let v = self.list_item_text(id, i);
                    let edit = if id == "fontList" { "font" } else { "size" };
                    self.form.set_text(edit, &v);
                }
            }
            "special" | "line" => {
                let kind = self.kind;
                self.kind = DocFormKind::Paragraph;
                self.r13_changed(id);
                self.kind = kind;
            }
            _ => {}
        }
    }
}

// ------------------------------------------------- Borders and Shading › Options…

/// Border and Shading Options (Borders and Shading › Options…): the distance of a
/// paragraph's border from its text, top / left / bottom / right in points (§17.3.4
/// `w:space` of the pBdr edges), and a preview. OK hands the distances back to Borders and
/// Shading (`r13.bsOptionsDone`).
fn open_border_options(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::BorderOptions,
        &tr("doc13.bo_title", "Border and Shading Options"),
        (400.0, 340.0),
        vec![],
    );
    f.r13 = Some(Box::new(R13::default()));
    let sp = &c.extra["r13"]["bsSpace"];
    let pt = |i: usize| {
        sp[i]
            .as_u64()
            .unwrap_or(if i.is_multiple_of(2) { 1 } else { 4 })
    };
    group_on(
        &mut f,
        None,
        "boG",
        [18.0, 48.0, 364.0, 18.0],
        &tr("doc13.bo_distance", "Distance from text"),
    );
    #[allow(clippy::type_complexity)]
    let rows: [(
        &'static str,
        &'static str,
        &'static str,
        &str,
        &str,
        f32,
        f32,
        usize,
    ); 4] = [
        (
            "boTopL",
            "boTop",
            "boTopSp",
            "doc13.bo_top",
            "Top:",
            32.0,
            80.0,
            0,
        ),
        (
            "boLeftL",
            "boLeft",
            "boLeftSp",
            "doc13.bo_left",
            "Left:",
            212.0,
            80.0,
            1,
        ),
        (
            "boBottomL",
            "boBottom",
            "boBottomSp",
            "doc13.bo_bottom",
            "Bottom:",
            32.0,
            116.0,
            2,
        ),
        (
            "boRightL",
            "boRight",
            "boRightSp",
            "doc13.bo_right",
            "Right:",
            212.0,
            116.0,
            3,
        ),
    ];
    for (lid, id, spid, key, fb, x, y, i) in rows {
        f.label(lid, [x, y + 4.0, 64.0, 18.0], &tr(key, fb));
        let v = format!("{} pt", pt(i));
        f.form.add(
            id,
            None,
            [x + 64.0, y, 81.0, 26.0],
            Kind::Edit {
                caret: v.chars().count(),
                value: v.chars().collect(),
            },
        );
        f.form
            .add(spid, None, [x + 145.0, y, 19.0, 26.0], Kind::Canvas);
    }
    group_on(
        &mut f,
        None,
        "boPvG",
        [18.0, 156.0, 364.0, 18.0],
        &tr("findFmt11.preview", "Preview"),
    );
    f.canvas("boPreview", [110.0, 182.0, 180.0, 96.0]);
    f.button(
        "ok",
        [152.0, 294.0, 110.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [272.0, 294.0, 110.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

impl DocForm {
    /// Border and Shading Options: the four distances in points (top, left, bottom, right).
    fn border_options_values(&self) -> [u32; 4] {
        let mut out = [0; 4];
        for (i, id) in ["boTop", "boLeft", "boBottom", "boRight"]
            .iter()
            .enumerate()
        {
            let tw = parse_len(&self.form.text(id), 20.0).unwrap_or(0);
            // §17.3.4: w:space is 0–31 points.
            out[i] = u32::try_from((tw / 20).clamp(0, 31)).unwrap_or(0);
        }
        out
    }
}

/// Border and Shading Options preview: a grey paragraph inside its box at the distances.
fn paint_border_options(f: &DocForm, p: &mut dyn Painter, r: [f32; 4], k: f32) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
    let [t, l, b, rt] = f.border_options_values().map(|v| v as f32 * k * 1.2);
    let (bx, by) = (r[0] + 24.0 * k, r[1] + 14.0 * k);
    let (bw, bh) = (r[2] - 48.0 * k, r[3] - 28.0 * k);
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    let line = k.max(1.0).round();
    p.fill_rect(bx, by, bw, line, ink);
    p.fill_rect(bx, by + bh - line, bw, line, ink);
    p.fill_rect(bx, by, line, bh, ink);
    p.fill_rect(bx + bw - line, by, line, bh, ink);
    let grey = Rgba(0xb4, 0xb4, 0xb4, 255);
    let tx = bx + line + l;
    let tw = (bw - 2.0 * line - l - rt).max(4.0);
    let mut y = by + line + t;
    let bottom = by + bh - line - b;
    let row = (6.0 * k).round().max(4.0);
    let mut i = 0;
    while y + row * 0.6 <= bottom {
        let w = if i % 3 == 2 { tw * 0.6 } else { tw };
        p.fill_rect(tx, y, w, (row * 0.5).round().max(2.0), grey);
        y += row;
        i += 1;
    }
}

// ------------------------------------------------------------- Style Pane Options

/// Style Pane Options (Styles pane › Options…): which styles the pane lists and their order
/// (working), the formatting shown as styles and the built-in name choices (kept as the
/// reference's defaults, dimmed: BlueDoc lists styles only).
fn open_pane_options(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::PaneOptions,
        &tr("doc13.po_title", "Style Pane Options"),
        (430.0, 470.0),
        vec![],
    );
    let v = &c.extra["r13"]["pane"];
    f.label(
        "showL",
        [18.0, 50.0, 380.0, 18.0],
        &tr("doc13.po_show", "Select styles to show:"),
    );
    f.combo(
        "show",
        [18.0, 72.0, 394.0, 26.0],
        vec![
            tr("doc13.po_recommended", "Recommended"),
            tr("doc13.po_inUse", "In use"),
            tr("doc13.po_inDocument", "In current document"),
            tr("doc13.po_all", "All styles"),
        ],
        v["show"].as_u64().unwrap_or(0).min(3) as usize,
    );
    f.label(
        "sortL",
        [18.0, 110.0, 380.0, 18.0],
        &tr("doc13.po_sort", "Select how list is sorted:"),
    );
    f.combo(
        "sort",
        [18.0, 132.0, 394.0, 26.0],
        vec![
            tr("docR5.sortRecommended", "As Recommended"),
            tr("docR5.sortAlpha", "Alphabetical"),
            tr("doc13.po_basedOn", "Based on"),
            tr("docR5.sortType", "By type"),
        ],
        v["sort"].as_u64().unwrap_or(0).min(3) as usize,
    );
    group_on(
        &mut f,
        None,
        "fmtG",
        [18.0, 172.0, 394.0, 18.0],
        &tr(
            "doc13.po_formatting",
            "Select formatting to show as styles:",
        ),
    );
    for (i, (id, key, fb)) in [
        ("poPara", "doc13.po_para", "Paragraph level formatting"),
        ("poFont", "doc13.po_font", "Font formatting"),
        ("poList", "doc13.po_list", "Bullet and numbering formatting"),
    ]
    .iter()
    .enumerate()
    {
        f.check(
            id,
            [32.0, 198.0 + i as f32 * 26.0, 370.0, 20.0],
            &tr(key, fb),
            false,
        );
        f.disable(id);
    }
    group_on(
        &mut f,
        None,
        "namesG",
        [18.0, 284.0, 394.0, 18.0],
        &tr(
            "doc13.po_names",
            "Select how built-in style names are shown",
        ),
    );
    f.check(
        "poNext",
        [32.0, 310.0, 370.0, 20.0],
        &tr(
            "doc13.po_nextHeading",
            "Show next heading when previous level is used",
        ),
        true,
    );
    f.check(
        "poHide",
        [32.0, 336.0, 370.0, 20.0],
        &tr(
            "doc13.po_hideBuiltin",
            "Hide built-in name when alternate name exists",
        ),
        false,
    );
    f.disable("poNext");
    f.disable("poHide");
    f.radio(
        "thisDoc",
        "scope",
        [18.0, 378.0, 190.0, 20.0],
        &tr("docR3.thisDoc", "Only in this document"),
        true,
    );
    f.radio(
        "template",
        "scope",
        [18.0, 402.0, 394.0, 20.0],
        &tr("docR5.newDocs", "New documents based on this template"),
        false,
    );
    f.button(
        "ok",
        [182.0, 432.0, 110.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [302.0, 432.0, 110.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

// ------------------------------------------------------------------ Line Numbers

/// Line Numbers (Page Setup › Layout › Line Numbers…): §17.6.8 lnNumType of the section —
/// Add line numbering, Start at (`w:start` + 1), From text (`w:distance`, Auto = none),
/// Count by (`w:countBy`) and the restart (`w:restart`). OK sets it and goes back to Page
/// Setup.
fn open_line_numbers(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::LineNumbers,
        &tr("doc13.ln_title", "Line Numbers"),
        (360.0, 340.0),
        vec![],
    );
    let inches = c.inches;
    f.r13 = Some(Box::new(R13 {
        inches,
        ..R13::default()
    }));
    let v = &c.extra["r13"]["section"]["lineNumbering"];
    let on = v.is_object();
    f.check(
        "lnOn",
        [18.0, 50.0, 320.0, 20.0],
        &tr("doc13.ln_add", "Add line numbering"),
        on,
    );
    #[allow(clippy::type_complexity)]
    let rows: [(&'static str, &'static str, &'static str, &str, &str, String); 3] = [
        (
            "lnStartL",
            "lnStart",
            "lnStartSp",
            "doc13.ln_start",
            "Start at:",
            (v["start"].as_u64().unwrap_or(0) + 1).to_string(),
        ),
        (
            "lnFromL",
            "lnFrom",
            "lnFromSp",
            "doc13.ln_from",
            "From text:",
            v["distance"].as_i64().map_or_else(
                || tr("doc13.ln_auto", "Auto"),
                |d| show_len(d as i32, inches),
            ),
        ),
        (
            "lnByL",
            "lnBy",
            "lnBySp",
            "doc13.ln_countBy",
            "Count by:",
            v["countBy"].as_u64().unwrap_or(1).to_string(),
        ),
    ];
    for (i, (lid, id, sp, key, fb, val)) in rows.into_iter().enumerate() {
        let y = 84.0 + i as f32 * 34.0;
        f.label(lid, [44.0, y + 4.0, 110.0, 18.0], &tr(key, fb));
        f.form.add(
            id,
            None,
            [160.0, y, 120.0, 26.0],
            Kind::Edit {
                caret: val.chars().count(),
                value: val.chars().collect(),
            },
        );
        f.form.add(sp, None, [280.0, y, 19.0, 26.0], Kind::Canvas);
    }
    f.label(
        "lnNumL",
        [44.0, 190.0, 200.0, 18.0],
        &tr("doc13.ln_numbering", "Numbering:"),
    );
    let restart = v["restart"].as_str().unwrap_or("newPage");
    for (i, (id, key, fb, val)) in [
        ("lnPage", "doc13.ln_page", "Restart each page", "newPage"),
        (
            "lnSection",
            "doc13.ln_section",
            "Restart each section",
            "newSection",
        ),
        ("lnCont", "doc13.ln_cont", "Continuous", "continuous"),
    ]
    .iter()
    .enumerate()
    {
        f.radio(
            id,
            "lnRestart",
            [58.0, 214.0 + i as f32 * 26.0, 260.0, 20.0],
            &tr(key, fb),
            restart == *val,
        );
    }
    f.button(
        "ok",
        [112.0, 300.0, 110.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [232.0, 300.0, 110.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.line_numbers_enable();
    f
}

impl DocForm {
    /// The boxes follow "Add line numbering".
    fn line_numbers_enable(&mut self) {
        let on = self.on("lnOn");
        for id in [
            "lnStart",
            "lnFrom",
            "lnBy",
            "lnStartL",
            "lnFromL",
            "lnByL",
            "lnNumL",
            "lnPage",
            "lnSection",
            "lnCont",
        ] {
            self.form.enable(id, on);
        }
    }

    /// Line Numbers OK: `setLineNumbering` for the section (`null` turns it off).
    fn line_numbers_ok(&self) -> FormOut {
        let numbering = self.on("lnOn").then(|| {
            let n = |id: &str, d: u64| self.form.text(id).trim().parse::<u64>().unwrap_or(d);
            let restart = match self.chosen("lnRestart") {
                Some("lnSection") => "newSection",
                Some("lnCont") => "continuous",
                _ => "newPage",
            };
            json!({
                "countBy": n("lnBy", 1).clamp(1, 100),
                "start": n("lnStart", 1).clamp(1, 32_768) - 1,
                "distance": parse_len(&self.form.text("lnFrom"), self.len_per()),
                "restart": restart,
            })
        });
        FormOut::Run(
            vec![json!({ "cmd": "setLineNumbering", "numbering": numbering, "scope": "section" })],
            true,
        )
    }
}

impl DocForm {
    /// Press a round-13 canvas (tests).
    #[cfg(test)]
    pub(crate) fn r13_canvas_for_test(&mut self, id: &str) -> bool {
        self.r13_canvas(id, 0.0, 0.0)
    }
}
