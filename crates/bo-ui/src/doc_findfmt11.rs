//! Find and Replace › Format ▾ dialogs of BlueDoc, parity round 11 (docs/ui/REPORT.md "Vòng
//! 101"): Find Font, Find Paragraph, Find Tabs, Find Language, Find Frame and Find Style,
//! laid out after the reference word processor's dialogs captured at 125 % (coordinates are
//! those reference pixels less the 8 px invisible window border). Every box starts empty and
//! every check box starts in the "don't care" state (a blue square with a dash, as the
//! reference): only what the user sets becomes part of the search. OK hands the window a
//! `FindFormat` part ([`FormOut::FindFormat`]) and the words shown after "Format:".
//! A child module of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut, LANGUAGES};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Map, Value};

/// A three-state check box of a Find dialog: 0 = any (dash), 1 = on, 2 = off.
pub(super) struct Tri {
    pub(super) id: &'static str,
    pub(super) label: String,
    pub(super) state: u8,
    pub(super) enabled: bool,
}

/// Spin arrows beside a blank number box: arrow canvas id, box id, step and unit.
const SPINS: [(&str, &str, f32, Unit); 10] = [
    ("inSp", "inside", 0.1, Unit::Inch),
    ("outSp", "outside", 0.1, Unit::Inch),
    ("bySp", "by", 0.1, Unit::Inch),
    ("befSp", "before", 6.0, Unit::Pt),
    ("aftSp", "after", 6.0, Unit::Pt),
    ("atSp", "at", 0.5, Unit::Lines),
    ("wAtSp", "wAt", 0.1, Unit::Inch),
    ("hAtSp", "hAt", 0.1, Unit::Inch),
    ("hDistSp", "hDist", 0.1, Unit::Inch),
    ("vDistSp", "vDist", 0.1, Unit::Inch),
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Unit {
    Inch,
    Pt,
    Lines,
}

/// Which round-11 dialog `kind` is.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::FindFont
            | DocFormKind::FindParagraph
            | DocFormKind::FindTabs
            | DocFormKind::FindLanguage
            | DocFormKind::FindFrame
            | DocFormKind::FindStyle
    )
}

/// The Find dialog of Format ▾ entry `i` (0 Font … 5 Style).
pub(crate) fn kind_of(i: usize) -> DocFormKind {
    match i {
        0 => DocFormKind::FindFont,
        1 => DocFormKind::FindParagraph,
        2 => DocFormKind::FindTabs,
        3 => DocFormKind::FindLanguage,
        4 => DocFormKind::FindFrame,
        _ => DocFormKind::FindStyle,
    }
}

/// The Format part a dialog sets.
pub(crate) fn group_of(kind: DocFormKind) -> &'static str {
    match kind {
        DocFormKind::FindFont => "font",
        DocFormKind::FindParagraph => "para",
        DocFormKind::FindTabs => "tabs",
        DocFormKind::FindLanguage => "lang",
        DocFormKind::FindFrame => "frame",
        _ => "style",
    }
}

/// Twips of a length typed in a box: `0.5"`, `0.5 in`, `1 cm`, `5 mm`, `12 pt`, `1 pi`, or a
/// bare number in `unit`. `None` for an empty or unreadable box.
fn twips(text: &str, unit: Unit) -> Option<i32> {
    let t = text.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    let split = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(t.len());
    let v: f32 = t[..split].trim().parse().ok()?;
    let per = match t[split..].trim() {
        "\"" | "in" | "″" => 1440.0,
        "cm" => 566.929,
        "mm" => 56.6929,
        "pt" => 20.0,
        "pi" => 240.0,
        "li" | "lines" => 240.0,
        "" => match unit {
            Unit::Inch => 1440.0,
            Unit::Pt => 20.0,
            Unit::Lines => 240.0,
        },
        _ => return None,
    };
    Some((v * per).round() as i32)
}

/// A length shown the reference's way (`0.5"`, `6 pt`, `1.5 li`).
fn show(v: f32, unit: Unit) -> String {
    let n = format!("{v:.2}");
    let n = n.trim_end_matches('0').trim_end_matches('.').to_owned();
    match unit {
        Unit::Inch => format!("{n}\""),
        Unit::Pt => format!("{n} pt"),
        Unit::Lines => format!("{n} li"),
    }
}

/// Value of a typed length in `unit` (for the spin arrows).
fn value_in(text: &str, unit: Unit) -> f32 {
    let per = match unit {
        Unit::Inch => 1440.0,
        Unit::Pt => 20.0,
        Unit::Lines => 240.0,
    };
    twips(text, unit).map_or(0.0, |t| t as f32 / per)
}

impl DocForm {
    fn tri(&mut self, page: Option<usize>, id: &'static str, r: [f32; 4], label: &str) {
        match page {
            Some(pg) => self.form.add(id, Some(pg), r, super::Kind::Canvas),
            None => self.form.add(id, None, r, super::Kind::Canvas),
        }
        self.tris.push(Tri {
            id,
            label: label.to_owned(),
            state: 0,
            enabled: true,
        });
    }

    fn group_on(&mut self, page: Option<usize>, id: &'static str, r: [f32; 4], text: &str) {
        self.form
            .add(id, page, r, super::Kind::Group(text.to_owned()));
    }

    /// Row `i` of list `id`.
    fn list_item(&self, id: &str, i: usize) -> String {
        self.form
            .get(id)
            .and_then(|c| match &c.kind {
                super::Kind::List { items, .. } => items.get(i).cloned(),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn tri_off(&mut self, id: &str) {
        if let Some(t) = self.tris.iter_mut().find(|t| t.id == id) {
            t.enabled = false;
        }
    }

    /// State of three-state box `id` as a search value (`None`: any).
    pub(crate) fn tri_value(&self, id: &str) -> Option<bool> {
        match self.tris.iter().find(|t| t.id == id).map_or(0, |t| t.state) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    }

    /// Set three-state box `id` (tests).
    #[cfg(test)]
    pub(crate) fn set_tri(&mut self, id: &str, v: Option<bool>) {
        if let Some(t) = self.tris.iter_mut().find(|t| t.id == id) {
            t.state = match v {
                None => 0,
                Some(true) => 1,
                Some(false) => 2,
            };
        }
    }

    /// A blank number box with spin arrows (the arrows are canvas `spin`).
    fn spin_box(&mut self, page: Option<usize>, id: &'static str, spin: &'static str, r: [f32; 4]) {
        let k = super::Kind::Edit {
            value: Vec::new(),
            caret: 0,
        };
        self.form.add(id, page, [r[0], r[1], r[2] - 18.0, r[3]], k);
        self.form.add(
            spin,
            page,
            [r[0] + r[2] - 19.0, r[1], 19.0, r[3]],
            super::Kind::Canvas,
        );
    }

    /// A click in a round-11 canvas: three-state boxes cycle any → on → off, spin arrows step
    /// the box beside them, the Find Frame wrapping pictures choose. `true` when handled.
    pub(super) fn r11_canvas(&mut self, id: &str, y: f32) -> bool {
        if let Some(t) = self.tris.iter_mut().find(|t| t.id == id && t.enabled) {
            t.state = (t.state + 1) % 3;
            return true;
        }
        if let Some(&(_, edit, step, unit)) = SPINS.iter().find(|s| s.0 == id) {
            let h = self.form.get(id).map_or(24.0, |c| c.rect[3]);
            let cur = value_in(&self.form.text(edit), unit);
            let v = if y < h / 2.0 { cur + step } else { cur - step };
            let v = if unit == Unit::Lines { v.max(0.0) } else { v };
            self.form.set_text(edit, &show(v, unit));
            return true;
        }
        if matches!(id, "wrapNone" | "wrapAround") {
            self.ids = vec![id.to_owned()];
            return true;
        }
        false
    }

    /// A round-11 list or box changed: lists fill the box above them (Find Font).
    pub(super) fn r11_changed(&mut self, id: &str) {
        let pairs = [
            ("fontList", "font"),
            ("styleList", "fstyle"),
            ("sizeList", "size"),
            ("tabList", "tabPos"),
        ];
        if let Some(&(_, edit)) = pairs.iter().find(|p| p.0 == id) {
            if let Some(i) = self.sel(id) {
                let v = self.list_item(id, i);
                self.form.set_text(edit, &v);
            }
        }
        if self.kind == DocFormKind::FindStyle && id == "styles" {
            let d = self
                .sel("styles")
                .and_then(|i| self.preview.get(i))
                .cloned()
                .unwrap_or_default();
            self.set_label("desc", &d);
        }
    }

    pub(super) fn r11_clicked(&mut self, id: &str) -> FormOut {
        match (self.kind, id) {
            (DocFormKind::FindTabs, "clearAll") => {
                self.form.set_text("tabPos", "");
                FormOut::Redraw
            }
            (_, "styles" | "list") => self.r11_ok(),
            _ => FormOut::Redraw,
        }
    }

    /// OK: the `FindFormat` part and its words.
    pub(super) fn r11_ok(&mut self) -> FormOut {
        let mut m = Map::new();
        let mut words: Vec<String> = Vec::new();
        let word = |k: &str, f: &str| -> String {
            self.words
                .iter()
                .find(|(key, _)| key == k)
                .map_or_else(|| f.to_owned(), |(_, v)| v.clone())
        };
        match self.kind {
            DocFormKind::FindFont => {
                let font = self.text("font");
                let mut font_words = Vec::new();
                if !font.trim().is_empty() {
                    m.insert("font".into(), json!(font.trim()));
                    font_words.push(font.trim().to_owned());
                }
                let size = self.text("size").trim().replace(',', ".");
                if let Ok(pt) = size.trim_end_matches("pt").trim().parse::<f32>() {
                    if pt > 0.0 {
                        m.insert("size".into(), json!((pt * 2.0).round() as u32));
                        font_words.push(format!("{size} pt"));
                    }
                }
                // Font style list: Regular, Italic, Bold, Bold Italic, Not Bold, Not Italic.
                let style = self.text("fstyle");
                let names: Vec<String> = STYLE_NAMES.iter().map(|(k, f)| word(k, f)).collect();
                match names.iter().position(|n| *n == style.trim()) {
                    Some(0) => {
                        m.insert("bold".into(), json!(false));
                        m.insert("italic".into(), json!(false));
                    }
                    Some(1) => {
                        m.insert("italic".into(), json!(true));
                    }
                    Some(2) => {
                        m.insert("bold".into(), json!(true));
                    }
                    Some(3) => {
                        m.insert("bold".into(), json!(true));
                        m.insert("italic".into(), json!(true));
                    }
                    Some(4) => {
                        m.insert("bold".into(), json!(false));
                    }
                    Some(5) => {
                        m.insert("italic".into(), json!(false));
                    }
                    _ => {}
                }
                if let Some(i) = names.iter().position(|n| *n == style.trim()) {
                    font_words.push(names[i].clone());
                }
                if let Some(i) = self.sel("color").filter(|&i| i > 0) {
                    let (hex, k, f) = COLORS[i - 1];
                    m.insert("color".into(), json!(hex));
                    font_words.push(format!(
                        "{}: {}",
                        word("findFmt11.fontColor", "Font color"),
                        word(k, f)
                    ));
                }
                if let Some(i) = self.sel("underline").filter(|&i| i > 0) {
                    m.insert("underline".into(), json!(i == 2));
                    font_words.push(word(UNDERLINES[i].0, UNDERLINES[i].1));
                }
                for (id, key, k, f) in TRI_FONT {
                    if let Some(v) = self.tri_value(id) {
                        m.insert(key.into(), json!(v));
                        let w = word(k, f);
                        font_words.push(if v {
                            w
                        } else {
                            format!("{} {w}", word("findFmt11.not", "Not"))
                        });
                    }
                }
                if !font_words.is_empty() {
                    words.push(format!(
                        "{} {}",
                        word("findFmt11.fontW", "Font:"),
                        font_words.join(", ")
                    ));
                }
            }
            DocFormKind::FindParagraph => {
                if let Some(i) = self.sel("align").filter(|&i| i > 0) {
                    let (v, k, f) = ALIGNS[i - 1];
                    m.insert("align".into(), json!(v));
                    words.push(word(k, f));
                }
                if let Some(i) = self.sel("outline").filter(|&i| i > 0) {
                    let level = if i == 1 { 10 } else { i - 1 };
                    m.insert("outlineLevel".into(), json!(level));
                    words.push(if level == 10 {
                        word("findFmt11.bodyText", "Body Text")
                    } else {
                        format!("{} {level}", word("findFmt11.level", "Level"))
                    });
                }
                let mut ind = Vec::new();
                if let Some(v) = twips(&self.text("inside"), Unit::Inch) {
                    m.insert("indentLeft".into(), json!(v));
                    ind.push(format!(
                        "{} {}",
                        word("findFmt11.leftW", "Left:"),
                        self.text("inside").trim()
                    ));
                }
                if let Some(v) = twips(&self.text("outside"), Unit::Inch) {
                    m.insert("indentRight".into(), json!(v));
                    ind.push(format!(
                        "{} {}",
                        word("findFmt11.rightW", "Right:"),
                        self.text("outside").trim()
                    ));
                }
                let by = twips(&self.text("by"), Unit::Inch);
                match self.sel("special") {
                    Some(1) => {
                        m.insert("firstLine".into(), json!(0));
                    }
                    Some(2) => {
                        m.insert("firstLine".into(), json!(by.unwrap_or(720)));
                        ind.push(format!(
                            "{} {}",
                            word("findFmt11.firstLineW", "First line:"),
                            self.text("by").trim()
                        ));
                    }
                    Some(3) => {
                        m.insert("firstLine".into(), json!(-by.unwrap_or(720)));
                        ind.push(format!(
                            "{} {}",
                            word("findFmt11.hangingW", "Hanging:"),
                            self.text("by").trim()
                        ));
                    }
                    _ => {}
                }
                if !ind.is_empty() {
                    words.push(format!(
                        "{} {}",
                        word("findFmt11.indentW", "Indent:"),
                        ind.join(", ")
                    ));
                }
                if let Some(v) = twips(&self.text("before"), Unit::Pt) {
                    m.insert("spaceBefore".into(), json!(v));
                    words.push(format!(
                        "{} {}",
                        word("findFmt11.spaceBeforeW", "Space Before:"),
                        self.text("before").trim()
                    ));
                }
                if let Some(v) = twips(&self.text("after"), Unit::Pt) {
                    m.insert("spaceAfter".into(), json!(v));
                    words.push(format!(
                        "{} {}",
                        word("findFmt11.spaceAfterW", "Space After:"),
                        self.text("after").trim()
                    ));
                }
                let at = self.text("at");
                let ls = match self.sel("line") {
                    Some(1) => Some("single".to_owned()),
                    Some(2) => Some("1.5".to_owned()),
                    Some(3) => Some("double".to_owned()),
                    Some(4) => twips(&at, Unit::Pt).map(|t| format!("atLeast:{t}")),
                    Some(5) => twips(&at, Unit::Pt).map(|t| format!("exactly:{t}")),
                    Some(6) => twips(&at, Unit::Lines).map(|t| format!("multiple:{t}")),
                    _ => None,
                };
                if let Some(ls) = ls {
                    m.insert("lineSpacing".into(), json!(ls));
                    let i = self.sel("line").unwrap_or(1);
                    let (k, f) = LINE_SPACINGS[i - 1];
                    words.push(format!(
                        "{} {}",
                        word("findFmt11.lineSpacingW", "Line spacing:"),
                        word(k, f)
                    ));
                }
                for (id, key, k, f) in TRI_PARA {
                    if let Some(v) = self.tri_value(id) {
                        m.insert(key.into(), json!(v));
                        let w = word(k, f);
                        words.push(if v {
                            w
                        } else {
                            format!("{} {w}", word("findFmt11.not", "Not"))
                        });
                    }
                }
            }
            DocFormKind::FindTabs => {
                let pos = self.text("tabPos");
                if let Some(v) = twips(&pos, Unit::Inch) {
                    m.insert("tab".into(), json!(v));
                    words.push(format!(
                        "{} {}",
                        word("findFmt11.tabStopsW", "Tab stops:"),
                        pos.trim()
                    ));
                }
            }
            DocFormKind::FindLanguage => {
                if let Some(i) = self.sel("list") {
                    if let Some((tag, name)) = LANGUAGES.get(i) {
                        m.insert("lang".into(), json!(tag));
                        words.push(word(&format!("findFmt11.lang.{tag}"), name));
                    } else {
                        m.insert("lang".into(), json!("none"));
                        words.push(word("findFmt11.noLanguage", "(no language)"));
                    }
                }
                if let Some(v) = self.tri_value("noProof") {
                    m.insert("noProof".into(), json!(v));
                    words.push(if v {
                        word("findFmt11.noProofW", "Do not check spelling or grammar")
                    } else {
                        word("findFmt11.proofW", "Check spelling and grammar")
                    });
                }
            }
            DocFormKind::FindFrame => {
                if let Some(w) = self.ids.first() {
                    m.insert("frame".into(), json!(true));
                    words.push(if w == "wrapNone" {
                        word("findFmt11.frameNoneW", "Frame: Wrap None")
                    } else {
                        word("findFmt11.frameAroundW", "Frame: Wrap Around")
                    });
                }
            }
            _ => {
                if let Some(i) = self.sel("styles").filter(|&i| i > 0) {
                    if let Some(id) = self.ids.get(i) {
                        m.insert("style".into(), json!(id));
                        let row = self.list_item("styles", i);
                        let name = row
                            .split_once("  ")
                            .map_or(row.as_str(), |r| r.1)
                            .to_owned();
                        words.push(format!(
                            "{} {}",
                            word("findDlg.fmtStyle", "Style:"),
                            name.trim()
                        ));
                    }
                }
            }
        }
        FormOut::FindFormat(group_of(self.kind), Value::Object(m), words.join(", "))
    }
}

/// Font styles of Find Font: key and en-US label.
const STYLE_NAMES: [(&str, &str); 6] = [
    ("findFmt11.regular", "Regular"),
    ("findFmt11.italic", "Italic"),
    ("findFmt11.bold", "Bold"),
    ("findFmt11.boldItalic", "Bold Italic"),
    ("findFmt11.notBold", "Not Bold"),
    ("findFmt11.notItalic", "Not Italic"),
];

/// Font colours offered (after "No Color"): hex, key, en-US name.
const COLORS: [(&str, &str, &str); 8] = [
    ("000000", "findFmt11.cBlack", "Black"),
    ("FF0000", "findFmt11.cRed", "Red"),
    ("0070C0", "findFmt11.cBlue", "Blue"),
    ("00B050", "findFmt11.cGreen", "Green"),
    ("FFC000", "findFmt11.cOrange", "Orange"),
    ("7030A0", "findFmt11.cPurple", "Purple"),
    ("FFFFFF", "findFmt11.cWhite", "White"),
    ("808080", "findFmt11.cGray", "Gray"),
];

/// Underline styles: (blank), (none), Single.
const UNDERLINES: [(&str, &str); 3] = [
    ("findFmt11.blank", ""),
    ("findFmt11.ulNone", "(none)"),
    ("findFmt11.ulSingle", "Single"),
];

/// Three-state effects of Find Font: box id, `FindFormat` field, key, en-US label.
const TRI_FONT: [(&str, &str, &str, &str); 7] = [
    ("strike", "strike", "findFmt11.strike", "Strikethrough"),
    (
        "dstrike",
        "doubleStrike",
        "findFmt11.dstrike",
        "Double strikethrough",
    ),
    (
        "super",
        "superscript",
        "findFmt11.superscript",
        "Superscript",
    ),
    ("sub", "subscript", "findFmt11.subscript", "Subscript"),
    (
        "smallCaps",
        "smallCaps",
        "findFmt11.smallCaps",
        "Small caps",
    ),
    ("allCaps", "caps", "findFmt11.allCaps", "All caps"),
    ("hidden", "hidden", "findFmt11.hidden", "Hidden"),
];

/// Three-state boxes of Find Paragraph › Line and Page Breaks.
const TRI_PARA: [(&str, &str, &str, &str); 7] = [
    (
        "noSpace",
        "contextualSpacing",
        "findFmt11.noSpaceW",
        "Don't add space",
    ),
    (
        "widow",
        "widowControl",
        "findFmt11.widow",
        "Widow/Orphan control",
    ),
    (
        "keepNext",
        "keepNext",
        "findFmt11.keepNext",
        "Keep with next",
    ),
    (
        "keepLines",
        "keepLines",
        "findFmt11.keepLines",
        "Keep lines together",
    ),
    (
        "pageBefore",
        "pageBreakBefore",
        "findFmt11.pageBefore",
        "Page break before",
    ),
    (
        "noLineNum",
        "suppressLineNumbers",
        "findFmt11.noLineNum",
        "Suppress line numbers",
    ),
    (
        "noHyphen",
        "suppressAutoHyphens",
        "findFmt11.noHyphen",
        "Don't hyphenate",
    ),
];

/// Alignments: `FindFormat` value, key, en-US word.
const ALIGNS: [(&str, &str, &str); 4] = [
    ("left", "findFmt11.left", "Left"),
    ("center", "findFmt11.centered", "Centered"),
    ("right", "findFmt11.right", "Right"),
    ("both", "findFmt11.justified", "Justified"),
];

/// Line spacing choices after the blank one.
const LINE_SPACINGS: [(&str, &str); 6] = [
    ("findFmt11.single", "Single"),
    ("findFmt11.lines15", "1.5 lines"),
    ("findFmt11.double", "Double"),
    ("findFmt11.atLeast", "At least"),
    ("findFmt11.exactly", "Exactly"),
    ("findFmt11.multiple", "Multiple"),
];

/// Words looked up by OK (summary words), loaded when the dialog opens.
fn summary_words(s: &Strings) -> Vec<(String, String)> {
    let mut keys: Vec<(&str, &str)> = vec![
        ("findFmt11.fontColor", "Font color"),
        ("findFmt11.not", "Not"),
        ("findFmt11.fontW", "Font:"),
        ("findFmt11.bodyText", "Body Text"),
        ("findFmt11.level", "Level"),
        ("findFmt11.leftW", "Left:"),
        ("findFmt11.rightW", "Right:"),
        ("findFmt11.firstLineW", "First line:"),
        ("findFmt11.hangingW", "Hanging:"),
        ("findFmt11.indentW", "Indent:"),
        ("findFmt11.spaceBeforeW", "Space Before:"),
        ("findFmt11.spaceAfterW", "Space After:"),
        ("findFmt11.lineSpacingW", "Line spacing:"),
        ("findFmt11.tabStopsW", "Tab stops:"),
        ("findFmt11.noSpaceW", "Don't add space"),
        ("findFmt11.noLanguage", "(no language)"),
        ("findFmt11.noProofW", "Do not check spelling or grammar"),
        ("findFmt11.proofW", "Check spelling and grammar"),
        ("findFmt11.frameNoneW", "Frame: Wrap None"),
        ("findFmt11.frameAroundW", "Frame: Wrap Around"),
        ("findDlg.fmtStyle", "Style:"),
    ];
    keys.extend(STYLE_NAMES);
    keys.extend(COLORS.iter().map(|c| (c.1, c.2)));
    keys.extend(UNDERLINES);
    keys.extend(TRI_FONT.iter().map(|c| (c.2, c.3)));
    keys.extend(TRI_PARA.iter().map(|c| (c.2, c.3)));
    keys.extend(ALIGNS.iter().map(|c| (c.1, c.2)));
    keys.extend(LINE_SPACINGS);
    keys.iter()
        .map(|(k, f)| ((*k).to_owned(), t(s, k, f)))
        .collect()
}

/// Build round-11 dialog `kind`.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let mut f = match kind {
        // Measured: 600 × 665 window (584 × 657 here).
        DocFormKind::FindFont => {
            let mut f = DocForm::new(
                kind,
                &tr("findFmt11.findFont", "Find Font"),
                (584.0, 657.0),
                vec![
                    tr("findFmt11.tabFont", "Font"),
                    tr("findFmt11.tabAdvanced", "Advanced"),
                ],
            );
            f.label_on(
                0,
                "fontL",
                [49.0, 113.0, 200.0, 18.0],
                &tr("findFmt11.fontL", "Font:"),
            );
            f.edit_on(0, "font", [49.0, 134.0, 242.0, 24.0], "");
            let mut fonts = c.fonts.clone();
            fonts.sort_by_key(|n| n.to_lowercase());
            fonts.dedup();
            // The theme's body / heading fonts first; no hidden ".…" system families (the
            // reference's list goes "+Headings", "AcadEref", … with "+Body" just above).
            fonts.retain(|n| !n.starts_with('.') && !n.starts_with('@'));
            fonts.splice(0..0, ["+Body".to_owned(), "+Headings".to_owned()]);
            f.form.add(
                "fontList",
                Some(0),
                [49.0, 160.0, 242.0, 88.0],
                super::Kind::List {
                    items: fonts,
                    sel: None,
                    top: 1,
                },
            );
            f.label_on(
                0,
                "fstyleL",
                [305.0, 113.0, 100.0, 18.0],
                &tr("findFmt11.fontStyleL", "Font style:"),
            );
            f.edit_on(0, "fstyle", [305.0, 134.0, 100.0, 24.0], "");
            let styles = STYLE_NAMES.iter().map(|(k, fb)| tr(k, fb)).collect();
            f.form.add(
                "styleList",
                Some(0),
                [305.0, 160.0, 100.0, 88.0],
                super::Kind::List {
                    items: styles,
                    sel: None,
                    top: 0,
                },
            );
            f.label_on(
                0,
                "sizeL",
                [419.0, 113.0, 89.0, 18.0],
                &tr("findFmt11.sizeL", "Size:"),
            );
            f.edit_on(0, "size", [419.0, 134.0, 89.0, 24.0], "");
            let sizes = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72]
                .iter()
                .map(ToString::to_string)
                .collect();
            f.form.add(
                "sizeList",
                Some(0),
                [419.0, 160.0, 89.0, 88.0],
                super::Kind::List {
                    items: sizes,
                    sel: None,
                    top: 0,
                },
            );
            f.label_on(
                0,
                "colorL",
                [49.0, 261.0, 150.0, 18.0],
                &tr("findFmt11.fontColorL", "Font color:"),
            );
            let mut colors = vec![tr("findFmt11.noColor", "No Color")];
            colors.extend(COLORS.iter().map(|(_, k, fb)| tr(k, fb)));
            f.form.add(
                "color",
                Some(0),
                [49.0, 280.0, 144.0, 26.0],
                super::Kind::Combo {
                    items: colors,
                    sel: 0,
                },
            );
            f.label_on(
                0,
                "ulL",
                [207.0, 261.0, 150.0, 18.0],
                &tr("findFmt11.ulStyleL", "Underline style:"),
            );
            let uls = UNDERLINES.iter().map(|(k, fb)| tr(k, fb)).collect();
            f.form.add(
                "underline",
                Some(0),
                [207.0, 280.0, 144.0, 22.0],
                super::Kind::Combo { items: uls, sel: 0 },
            );
            f.label_on(
                0,
                "ulcL",
                [365.0, 261.0, 150.0, 18.0],
                &tr("findFmt11.ulColorL", "Underline color:"),
            );
            f.form.add(
                "ulColor",
                Some(0),
                [365.0, 280.0, 143.0, 26.0],
                super::Kind::Combo {
                    items: {
                        let mut v = vec![tr("findFmt11.noColor", "No Color")];
                        v.extend(COLORS.iter().map(|(_, k, fb)| tr(k, fb)));
                        v
                    },
                    sel: 0,
                },
            );
            f.group_on(
                Some(0),
                "effects",
                [24.0, 314.0, 544.0, 18.0],
                &tr("findFmt11.effects", "Effects"),
            );
            f.tri(
                Some(0),
                "strike",
                [38.0, 341.0, 220.0, 20.0],
                &tr("findFmt11.strike", "Strikethrough"),
            );
            f.tri(
                Some(0),
                "dstrike",
                [38.0, 366.0, 220.0, 20.0],
                &tr("findFmt11.dstrike", "Double strikethrough"),
            );
            f.tri(
                Some(0),
                "super",
                [38.0, 391.0, 220.0, 20.0],
                &tr("findFmt11.superscript", "Superscript"),
            );
            f.tri(
                Some(0),
                "sub",
                [38.0, 416.0, 220.0, 20.0],
                &tr("findFmt11.subscript", "Subscript"),
            );
            f.tri(
                Some(0),
                "smallCaps",
                [396.0, 341.0, 160.0, 20.0],
                &tr("findFmt11.smallCaps", "Small caps"),
            );
            f.tri(
                Some(0),
                "allCaps",
                [396.0, 366.0, 160.0, 20.0],
                &tr("findFmt11.allCaps", "All caps"),
            );
            f.tri(
                Some(0),
                "hidden",
                [396.0, 391.0, 160.0, 20.0],
                &tr("findFmt11.hidden", "Hidden"),
            );
            f.group_on(
                Some(0),
                "previewL",
                [24.0, 453.0, 544.0, 18.0],
                &tr("findFmt11.preview", "Preview"),
            );
            f.form.add(
                "fontPreview",
                Some(0),
                [38.0, 479.0, 522.0, 52.0],
                super::Kind::Canvas,
            );
            f.label_on(
                1,
                "advNote",
                [24.0, 100.0, 520.0, 40.0],
                &tr(
                    "findFmt11.advanced",
                    "Character spacing and OpenType features are not searched.",
                ),
            );
            f.button(
                "setDefault",
                [10.0, 617.0, 112.0, 28.0],
                &tr("findFmt11.setDefault", "Set As Default"),
                false,
            );
            f.disable("setDefault");
            f.button(
                "ok",
                [334.0, 617.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [461.0, 617.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // Measured: 600 × 713 window (584 × 705 here).
        DocFormKind::FindParagraph => {
            let mut f = DocForm::new(
                kind,
                &tr("findFmt11.findParagraph", "Find Paragraph"),
                (584.0, 705.0),
                vec![
                    tr("findFmt11.tabIndents", "Indents and Spacing"),
                    tr("findFmt11.tabBreaks", "Line and Page Breaks"),
                ],
            );
            f.group_on(
                Some(0),
                "general",
                [24.0, 88.0, 544.0, 18.0],
                &tr("findFmt11.general", "General"),
            );
            f.label_on(
                0,
                "alignL",
                [38.0, 118.0, 90.0, 18.0],
                &tr("findFmt11.alignmentL", "Alignment:"),
            );
            let mut aligns = vec![String::new()];
            aligns.extend(ALIGNS.iter().map(|(_, k, fb)| tr(k, fb)));
            f.form.add(
                "align",
                Some(0),
                [128.0, 114.0, 159.0, 26.0],
                super::Kind::Combo {
                    items: aligns,
                    sel: 0,
                },
            );
            f.label_on(
                0,
                "outlineL",
                [38.0, 150.0, 90.0, 18.0],
                &tr("findFmt11.outlineL", "Outline level:"),
            );
            let mut levels = vec![String::new(), tr("findFmt11.bodyText", "Body Text")];
            levels.extend((1..=9).map(|i| format!("{} {i}", tr("findFmt11.level", "Level"))));
            f.form.add(
                "outline",
                Some(0),
                [128.0, 146.0, 159.0, 26.0],
                super::Kind::Combo {
                    items: levels,
                    sel: 0,
                },
            );
            f.tri(
                Some(0),
                "collapsed",
                [302.0, 149.0, 200.0, 20.0],
                &tr("findFmt11.collapsed", "Collapsed by default"),
            );

            f.group_on(
                Some(0),
                "indentation",
                [24.0, 208.0, 544.0, 18.0],
                &tr("findFmt11.indentation", "Indentation"),
            );
            f.label_on(
                0,
                "insideL",
                [38.0, 238.0, 80.0, 18.0],
                &tr("findFmt11.insideL", "Inside:"),
            );
            f.spin_box(Some(0), "inside", "inSp", [126.0, 235.0, 84.0, 24.0]);
            f.label_on(
                0,
                "outsideL",
                [38.0, 269.0, 80.0, 18.0],
                &tr("findFmt11.outsideL", "Outside:"),
            );
            f.spin_box(Some(0), "outside", "outSp", [126.0, 266.0, 84.0, 24.0]);
            f.label_on(
                0,
                "specialL",
                [301.0, 238.0, 120.0, 18.0],
                &tr("findFmt11.specialL", "Special:"),
            );
            let specials = vec![
                String::new(),
                tr("findFmt11.none", "(none)"),
                tr("findFmt11.firstLine", "First line"),
                tr("findFmt11.hanging", "Hanging"),
            ];
            f.form.add(
                "special",
                Some(0),
                [301.0, 264.0, 169.0, 28.0],
                super::Kind::Combo {
                    items: specials,
                    sel: 0,
                },
            );
            f.label_on(
                0,
                "byL",
                [476.0, 238.0, 80.0, 18.0],
                &tr("findFmt11.byL", "By:"),
            );
            f.spin_box(Some(0), "by", "bySp", [476.0, 266.0, 84.0, 24.0]);
            f.tri(
                Some(0),
                "mirror",
                [38.0, 303.0, 200.0, 20.0],
                &tr("findFmt11.mirror", "Mirror indents"),
            );

            f.group_on(
                Some(0),
                "spacing",
                [24.0, 352.0, 544.0, 18.0],
                &tr("findFmt11.spacing", "Spacing"),
            );
            f.label_on(
                0,
                "beforeL",
                [38.0, 382.0, 80.0, 18.0],
                &tr("findFmt11.beforeL", "Before:"),
            );
            f.spin_box(Some(0), "before", "befSp", [126.0, 379.0, 84.0, 24.0]);
            f.label_on(
                0,
                "afterL",
                [38.0, 413.0, 80.0, 18.0],
                &tr("findFmt11.afterL", "After:"),
            );
            f.spin_box(Some(0), "after", "aftSp", [126.0, 410.0, 84.0, 24.0]);
            f.label_on(
                0,
                "lineL",
                [301.0, 382.0, 120.0, 18.0],
                &tr("findFmt11.lineSpacingL", "Line spacing:"),
            );
            let mut lines = vec![String::new()];
            lines.extend(LINE_SPACINGS.iter().map(|(k, fb)| tr(k, fb)));
            f.form.add(
                "line",
                Some(0),
                [301.0, 408.0, 169.0, 28.0],
                super::Kind::Combo {
                    items: lines,
                    sel: 0,
                },
            );
            f.label_on(
                0,
                "atL",
                [476.0, 382.0, 80.0, 18.0],
                &tr("findFmt11.atL", "At:"),
            );
            f.spin_box(Some(0), "at", "atSp", [476.0, 410.0, 84.0, 24.0]);
            f.tri(
                Some(0),
                "noSpace",
                [38.0, 441.0, 420.0, 20.0],
                &tr(
                    "findFmt11.noSpace",
                    "Don't add space between paragraphs of the same style",
                ),
            );
            f.group_on(
                Some(0),
                "previewL",
                [24.0, 496.0, 544.0, 18.0],
                &tr("findFmt11.preview", "Preview"),
            );
            f.form.add(
                "paraPreview",
                Some(0),
                [38.0, 522.0, 522.0, 113.0],
                super::Kind::Canvas,
            );
            f.group_on(
                Some(1),
                "pagination",
                [24.0, 88.0, 544.0, 18.0],
                &tr("findFmt11.pagination", "Pagination"),
            );
            f.tri(
                Some(1),
                "widow",
                [38.0, 114.0, 260.0, 20.0],
                &tr("findFmt11.widow", "Widow/Orphan control"),
            );
            f.tri(
                Some(1),
                "keepNext",
                [38.0, 139.0, 260.0, 20.0],
                &tr("findFmt11.keepNext", "Keep with next"),
            );
            f.tri(
                Some(1),
                "keepLines",
                [38.0, 164.0, 260.0, 20.0],
                &tr("findFmt11.keepLines", "Keep lines together"),
            );
            f.tri(
                Some(1),
                "pageBefore",
                [38.0, 189.0, 260.0, 20.0],
                &tr("findFmt11.pageBefore", "Page break before"),
            );
            f.group_on(
                Some(1),
                "exceptions",
                [24.0, 230.0, 544.0, 18.0],
                &tr("findFmt11.exceptions", "Formatting exceptions"),
            );
            f.tri(
                Some(1),
                "noLineNum",
                [38.0, 256.0, 260.0, 20.0],
                &tr("findFmt11.noLineNum", "Suppress line numbers"),
            );
            f.tri(
                Some(1),
                "noHyphen",
                [38.0, 281.0, 260.0, 20.0],
                &tr("findFmt11.noHyphen", "Don't hyphenate"),
            );
            f.button(
                "tabs",
                [10.0, 666.0, 112.0, 28.0],
                &tr("findFmt11.tabsBtn", "Tabs..."),
                false,
            );
            f.disable("tabs");
            f.button(
                "setDefault",
                [172.0, 666.0, 112.0, 28.0],
                &tr("findFmt11.setDefault", "Set As Default"),
                false,
            );
            // Disabled: BlueDoc never writes the document's defaults from a search dialog (the
            // reference enables it; a button that did nothing would be worse).
            f.disable("setDefault");
            f.preview = vec![
                tr(
                    "findFmt11.pvBefore",
                    "Text of the paragraph before this one.",
                ),
                tr(
                    "findFmt11.pvSample",
                    "Sample of the paragraph searched for.",
                ),
                tr("findFmt11.pvAfter", "Text of the paragraph after this one."),
            ];
            f.button(
                "ok",
                [334.0, 666.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [461.0, 666.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // Measured: 417 × 461 window (401 × 453 here).
        DocFormKind::FindTabs => {
            let mut f = DocForm::new(
                kind,
                &tr("findFmt11.findTabs", "Find Tabs"),
                (401.0, 453.0),
                vec![],
            );
            f.label(
                "posL",
                [10.0, 46.0, 230.0, 18.0],
                &tr("findFmt11.tabPosL", "Tab stop position:"),
            );
            f.edit("tabPos", [10.0, 68.0, 222.0, 24.0], "");
            f.list("tabList", [10.0, 94.0, 222.0, 106.0], Vec::new(), None);
            f.label(
                "clearL",
                [248.0, 110.0, 150.0, 18.0],
                &tr("findFmt11.toClear", "Tab stops to be cleared:"),
            );
            f.group_on(
                None,
                "alignG",
                [10.0, 204.0, 380.0, 18.0],
                &tr("findFmt11.alignment", "Alignment"),
            );
            f.radio(
                "tLeft",
                "talign",
                [24.0, 230.0, 120.0, 20.0],
                &tr("findFmt11.left", "Left"),
                true,
            );
            f.radio(
                "tCenter",
                "talign",
                [152.0, 230.0, 120.0, 20.0],
                &tr("findFmt11.center", "Center"),
                false,
            );
            f.radio(
                "tRight",
                "talign",
                [278.0, 230.0, 110.0, 20.0],
                &tr("findFmt11.right", "Right"),
                false,
            );
            f.radio(
                "tDecimal",
                "talign",
                [24.0, 256.0, 120.0, 20.0],
                &tr("findFmt11.decimal", "Decimal"),
                false,
            );
            f.radio(
                "tBar",
                "talign",
                [152.0, 256.0, 120.0, 20.0],
                &tr("findFmt11.bar", "Bar"),
                false,
            );
            f.group_on(
                None,
                "leaderG",
                [10.0, 286.0, 380.0, 18.0],
                &tr("findFmt11.leader", "Leader"),
            );
            f.radio(
                "l1",
                "leader",
                [24.0, 312.0, 120.0, 20.0],
                &tr("findFmt11.leaderNone", "1 None"),
                true,
            );
            f.radio(
                "l2",
                "leader",
                [152.0, 312.0, 120.0, 20.0],
                "2 .......",
                false,
            );
            f.radio(
                "l3",
                "leader",
                [278.0, 312.0, 110.0, 20.0],
                "3 -------",
                false,
            );
            f.radio("l4", "leader", [24.0, 338.0, 120.0, 20.0], "4 ___", false);
            f.button(
                "set",
                [24.0, 368.0, 112.0, 28.0],
                &tr("findFmt11.set", "Set"),
                false,
            );
            f.button(
                "clear",
                [152.0, 368.0, 112.0, 28.0],
                &tr("findFmt11.clear", "Clear"),
                false,
            );
            f.button(
                "clearAll",
                [278.0, 368.0, 112.0, 28.0],
                &tr("findFmt11.clearAll", "Clear All"),
                false,
            );
            f.disable("set");
            f.disable("clear");
            f.button(
                "ok",
                [152.0, 413.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [278.0, 413.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // Measured: 403 × 440 window (387 × 432 here), as the Language dialog.
        DocFormKind::FindLanguage => {
            let mut f = DocForm::new(
                kind,
                &tr("findFmt11.findLanguage", "Find Language"),
                (387.0, 432.0),
                vec![],
            );
            f.label(
                "forL",
                [10.0, 46.0, 360.0, 18.0],
                &tr("docR3.changeFor", "Change proofing language for:"),
            );
            f.radio(
                "selection",
                "scope",
                [10.0, 69.0, 110.0, 20.0],
                &tr("docR3.selectedText", "Selected text"),
                false,
            );
            f.radio(
                "document",
                "scope",
                [124.0, 69.0, 200.0, 20.0],
                &tr("docR3.currentDocument", "Current Document"),
                true,
            );
            f.disable("selection");
            f.disable("document");
            f.label(
                "langL",
                [10.0, 94.0, 360.0, 18.0],
                &tr("findFmt11.languageL", "Language:"),
            );
            let mut langs: Vec<String> = LANGUAGES.iter().map(|l| l.1.to_owned()).collect();
            langs.push(tr("findFmt11.noLanguage", "(no language)"));
            f.list("list", [10.0, 113.0, 365.0, 164.0], langs, None);
            // Two lines, broken at the space nearest the middle (the reference wraps it).
            let note = tr(
                "docR3.languageNote",
                "The speller and other proofing tools automatically use dictionaries of the selected language, if available.",
            );
            let mut mid = (note.len() / 2 + note.len() / 10).min(note.len());
            while !note.is_char_boundary(mid) {
                mid -= 1;
            }
            let cut = note[..mid].rfind(' ').unwrap_or(note.len());
            f.label("note", [10.0, 282.0, 370.0, 18.0], note[..cut].trim());
            f.label("note2", [10.0, 299.0, 370.0, 18.0], note[cut..].trim());
            f.tri(
                None,
                "noProof",
                [10.0, 333.0, 360.0, 20.0],
                &tr("findFmt11.noProofW", "Do not check spelling or grammar"),
            );
            f.tri(
                None,
                "detect",
                [10.0, 358.0, 360.0, 20.0],
                &tr("findFmt11.detect", "Detect language automatically"),
            );
            f.tri_off("detect");
            f.button(
                "setDefault",
                [10.0, 391.0, 112.0, 28.0],
                &tr("findFmt11.setDefault", "Set As Default"),
                false,
            );
            f.disable("setDefault");
            f.button(
                "ok",
                [137.0, 391.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [264.0, 391.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // Measured: 404 × 561 window (388 × 553 here).
        DocFormKind::FindFrame => {
            let mut f = DocForm::new(
                kind,
                &tr("findFmt11.findFrame", "Find Frame"),
                (388.0, 553.0),
                vec![],
            );
            f.group_on(
                None,
                "wrapG",
                [10.0, 48.0, 370.0, 18.0],
                &tr("findFmt11.textWrapping", "Text wrapping"),
            );
            f.canvas("wrapNone", [66.0, 72.0, 72.0, 64.0]);
            f.canvas("wrapAround", [248.0, 72.0, 72.0, 64.0]);
            f.label(
                "wrapNoneL",
                [66.0, 142.0, 72.0, 18.0],
                &tr("findFmt11.wrapNone", "None"),
            );
            f.label(
                "wrapAroundL",
                [248.0, 142.0, 72.0, 18.0],
                &tr("findFmt11.wrapAround", "Around"),
            );
            f.group_on(
                None,
                "sizeG",
                [10.0, 170.0, 370.0, 18.0],
                &tr("findFmt11.size", "Size"),
            );
            let sizes = vec![
                String::new(),
                tr("findFmt11.auto", "Auto"),
                tr("findFmt11.atLeast", "At least"),
                tr("findFmt11.exactly", "Exactly"),
            ];
            f.label(
                "wL",
                [24.0, 200.0, 60.0, 18.0],
                &tr("findFmt11.widthL", "Width:"),
            );
            f.form.add(
                "w",
                None,
                [87.0, 195.0, 94.0, 26.0],
                super::Kind::Combo {
                    items: sizes.clone(),
                    sel: 0,
                },
            );
            f.label(
                "wAtL",
                [245.0, 200.0, 30.0, 18.0],
                &tr("findFmt11.atL", "At:"),
            );
            f.spin_box(None, "wAt", "wAtSp", [273.0, 195.0, 90.0, 24.0]);
            f.label(
                "hL",
                [24.0, 232.0, 60.0, 18.0],
                &tr("findFmt11.heightL", "Height:"),
            );
            f.form.add(
                "h",
                None,
                [87.0, 227.0, 94.0, 26.0],
                super::Kind::Combo {
                    items: sizes,
                    sel: 0,
                },
            );
            f.label(
                "hAtL",
                [245.0, 232.0, 30.0, 18.0],
                &tr("findFmt11.atL", "At:"),
            );
            f.spin_box(None, "hAt", "hAtSp", [273.0, 227.0, 90.0, 24.0]);
            f.group_on(
                None,
                "horG",
                [10.0, 264.0, 370.0, 18.0],
                &tr("findFmt11.horizontal", "Horizontal"),
            );
            f.label(
                "hPosL",
                [24.0, 295.0, 60.0, 18.0],
                &tr("findFmt11.positionL", "Position:"),
            );
            f.form.add(
                "hPos",
                None,
                [87.0, 290.0, 94.0, 26.0],
                super::Kind::Combo {
                    items: vec![String::new()],
                    sel: 0,
                },
            );
            f.label(
                "hRelL",
                [195.0, 295.0, 80.0, 18.0],
                &tr("findFmt11.relativeL", "Relative to:"),
            );
            f.form.add(
                "hRel",
                None,
                [273.0, 290.0, 104.0, 26.0],
                super::Kind::Combo {
                    items: vec![String::new()],
                    sel: 0,
                },
            );
            f.label(
                "hDistL",
                [150.0, 326.0, 125.0, 18.0],
                &tr("findFmt11.distanceL", "Distance from text:"),
            );
            f.spin_box(None, "hDist", "hDistSp", [273.0, 322.0, 104.0, 24.0]);
            f.group_on(
                None,
                "verG",
                [10.0, 359.0, 370.0, 18.0],
                &tr("findFmt11.vertical", "Vertical"),
            );
            f.label(
                "vPosL",
                [24.0, 390.0, 60.0, 18.0],
                &tr("findFmt11.positionL", "Position:"),
            );
            f.form.add(
                "vPos",
                None,
                [87.0, 385.0, 94.0, 26.0],
                super::Kind::Combo {
                    items: vec![String::new()],
                    sel: 0,
                },
            );
            f.label(
                "vRelL",
                [195.0, 390.0, 80.0, 18.0],
                &tr("findFmt11.relativeL", "Relative to:"),
            );
            f.form.add(
                "vRel",
                None,
                [273.0, 385.0, 104.0, 26.0],
                super::Kind::Combo {
                    items: vec![String::new()],
                    sel: 0,
                },
            );
            f.label(
                "vDistL",
                [150.0, 421.0, 125.0, 18.0],
                &tr("findFmt11.distanceL", "Distance from text:"),
            );
            f.spin_box(None, "vDist", "vDistSp", [273.0, 417.0, 104.0, 24.0]);
            f.tri(
                None,
                "moveWith",
                [10.0, 453.0, 200.0, 20.0],
                &tr("findFmt11.moveWith", "Move with text"),
            );
            f.tri(
                None,
                "lockAnchor",
                [10.0, 478.0, 200.0, 20.0],
                &tr("findFmt11.lockAnchor", "Lock anchor"),
            );
            for id in [
                "w", "h", "hPos", "hRel", "vPos", "vRel", "wAt", "hAt", "hDist", "vDist",
            ] {
                f.disable(id);
            }
            f.tri_off("moveWith");
            f.tri_off("lockAnchor");
            f.button(
                "removeFrame",
                [10.0, 513.0, 112.0, 28.0],
                &tr("findFmt11.removeFrame", "Remove Frame"),
                false,
            );
            f.disable("removeFrame");
            f.button(
                "ok",
                [138.0, 513.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [265.0, 513.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
        // Measured: 350 × 352 window (334 × 344 here).
        _ => {
            let mut f = DocForm::new(
                kind,
                &tr("findFmt11.findStyle", "Find Style"),
                (334.0, 344.0),
                vec![],
            );
            f.label(
                "whatL",
                [10.0, 46.0, 300.0, 18.0],
                &tr("findFmt11.findWhatStyle", "Find what style:"),
            );
            let mut items = vec![format!("    {}", tr("findFmt11.noStyle", "(no style)"))];
            let mut ids = vec![String::new()];
            let mut desc = vec![String::new()];
            for (id, name, para) in &c.styles {
                // ¶ marks a paragraph style, a a character style, as the reference's icons.
                // Built-in names show capitalised ("heading 1" → "Heading 1"), as the reference.
                let mut cs = name.chars();
                let shown: String = cs
                    .next()
                    .map(|c| c.to_uppercase().chain(cs).collect())
                    .unwrap_or_default();
                items.push(format!("{}  {shown}", if *para { '\u{b6}' } else { 'a' }));
                ids.push(id.clone());
                desc.push(if *para {
                    tr("findFmt11.paraStyle", "Paragraph style")
                } else {
                    tr("findFmt11.charStyle", "Character style")
                });
            }
            f.list("styles", [10.0, 68.0, 312.0, 106.0], items, Some(0));
            f.ids = ids;
            f.preview = desc;
            f.group_on(
                None,
                "descG",
                [10.0, 190.0, 312.0, 18.0],
                &tr("findFmt11.description", "Description"),
            );
            f.label("desc", [10.0, 214.0, 312.0, 70.0], "");
            f.button(
                "ok",
                [84.0, 303.0, 112.0, 28.0],
                &tr("common.ok", "OK"),
                true,
            );
            f.button(
                "cancel",
                [211.0, 303.0, 112.0, 28.0],
                &tr("common.cancel", "Cancel"),
                false,
            );
            f
        }
    };
    f.words = summary_words(s);
    f
}

/// Paint a round-11 canvas: three-state boxes, spin arrows, frame wrapping pictures,
/// previews. `true` when `id` is one of them.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
    k: f32,
) -> bool {
    if let Some(t) = f.tris.iter().find(|t| t.id == id) {
        paint_tri(p, r, t, font, k);
        return true;
    }
    if SPINS.iter().any(|s| s.0 == id) {
        let enabled = f.form.get(id).is_none_or(|c| c.enabled);
        let edge = Rgba(0xab, 0xad, 0xb3, 255);
        p.fill_rect(r[0], r[1], r[2], r[3], edge);
        p.fill_rect(
            r[0],
            r[1] + 1.0,
            r[2] - 1.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 2.0,
            r[2] - 3.0,
            r[3] / 2.0 - 2.0,
            Rgba(0xf0, 0xf0, 0xf0, 255),
        );
        p.fill_rect(
            r[0] + 1.0,
            r[1] + r[3] / 2.0 + 1.0,
            r[2] - 3.0,
            r[3] / 2.0 - 3.0,
            Rgba(0xf0, 0xf0, 0xf0, 255),
        );
        let c = if enabled {
            Rgba(0x60, 0x60, 0x60, 255)
        } else {
            Rgba(0xbc, 0xbc, 0xbc, 255)
        };
        let cx = r[0] + (r[2] - 1.0) / 2.0;
        let (w, h) = (3.5 * k, 2.5 * k);
        for (cy, up) in [(r[1] + r[3] * 0.27, true), (r[1] + r[3] * 0.73, false)] {
            let s = if up { -1.0 } else { 1.0 };
            p.path(
                &[
                    PathSeg::M(cx - w, cy - s * h / 2.0),
                    PathSeg::L(cx + w, cy - s * h / 2.0),
                    PathSeg::L(cx, cy + s * h / 2.0),
                    PathSeg::Z,
                ],
                Some(c),
                None,
            );
        }
        return true;
    }
    match id {
        "wrapNone" | "wrapAround" => {
            let chosen = f.ids.first().is_some_and(|c| c == id);
            paint_wrap(p, r, id == "wrapAround", chosen, k);
            true
        }
        "fontPreview" => {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x82, 0x82, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            let y = r[1] + r[3] * 0.62;
            let line = Rgba(0, 0, 0, 255);
            p.fill_rect(r[0] + 2.0 * k, y, 100.0 * k, 1.0, line);
            p.fill_rect(r[0] + r[2] - 102.0 * k, y, 100.0 * k, 1.0, line);
            let sample = Font {
                size: font.size * 1.25,
                ..*font
            };
            p.text_font(
                "AaBbCcYyZz",
                r[0] + r[2] / 2.0 - 56.0 * k,
                r[1] + r[3] * 0.22,
                &sample,
                line,
            );
            true
        }
        "paraPreview" => {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x82, 0x82, 255));
            p.fill_rect(
                r[0] + 1.0,
                r[1] + 1.0,
                r[2] - 2.0,
                r[3] - 2.0,
                Rgba(255, 255, 255, 255),
            );
            // Sample text as the reference lays it out: the paragraph before in grey, the
            // paragraph searched for in black, the one after in grey (BlueOffice's own words).
            // Measured on the reference's Paragraph dialog (round 13): regular 9 px text,
            // 13 px lines, 32 px in and 46 px from the right.
            let small = Font {
                size: (font.size * 0.74).round(),
                weight: 400,
                ..*font
            };
            let x = r[0] + 32.0 * k;
            let w = r[2] - 78.0 * k;
            let line_h = (10.4 * k).round();
            let per_line = ((w / (small.size * 0.5)).floor() as usize).max(8);
            let grey = Rgba(0xa6, 0xa6, 0xa6, 255);
            let mut y = r[1] + 10.0 * k;
            p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
            let parts: [(usize, usize, Rgba); 3] =
                [(0, 3, grey), (1, 1, Rgba(0, 0, 0, 255)), (2, 7, grey)];
            for (i, reps, color) in parts {
                let one = f.preview.get(i).cloned().unwrap_or_default();
                let text = vec![one.as_str(); reps].join(" ");
                let mut line = String::new();
                let mut lines = Vec::new();
                for word in text.split(' ') {
                    if !line.is_empty()
                        && line.chars().count() + word.chars().count() + 1 > per_line
                    {
                        lines.push(std::mem::take(&mut line));
                    }
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(word);
                }
                lines.push(line);
                for l in lines {
                    p.text_font(&l, x, y, &small, color);
                    y += line_h;
                }
                y += line_h * 0.6;
            }
            p.pop_clip();
            true
        }
        _ => false,
    }
}

/// A three-state check box: any = blue rounded square with a light dash (measured 16 px at
/// 125 %: #005FB8, dash #609BD3), on = blue with a white tick, off = white with a dark frame.
fn paint_tri(p: &mut dyn Painter, r: [f32; 4], t: &Tri, font: &Font<'_>, k: f32) {
    let d = (16.0 * k).round();
    let (x, y) = (r[0], (r[1] + (r[3] - d) / 2.0).round());
    let blue = if t.enabled {
        Rgba(0x00, 0x5f, 0xb8, 255)
    } else {
        Rgba(0xbc, 0xbc, 0xbc, 255)
    };
    let rad = 2.0 * k;
    let rounded = |inset: f32| -> Vec<PathSeg> {
        let (l, tp, rr, b) = (x + inset, y + inset, x + d - inset, y + d - inset);
        let q = (rad - inset).max(0.5);
        vec![
            PathSeg::M(l + q, tp),
            PathSeg::L(rr - q, tp),
            PathSeg::Q(rr, tp, rr, tp + q),
            PathSeg::L(rr, b - q),
            PathSeg::Q(rr, b, rr - q, b),
            PathSeg::L(l + q, b),
            PathSeg::Q(l, b, l, b - q),
            PathSeg::L(l, tp + q),
            PathSeg::Q(l, tp, l + q, tp),
            PathSeg::Z,
        ]
    };
    match t.state {
        0 => {
            p.path(&rounded(0.0), Some(blue), None);
            let dash = if t.enabled {
                Rgba(0x60, 0x9b, 0xd3, 255)
            } else {
                Rgba(0xe6, 0xe6, 0xe6, 255)
            };
            p.fill_rect(
                x + d * 0.25,
                y + d / 2.0 - k,
                d * 0.5,
                (2.0 * k).round().max(1.0),
                dash,
            );
        }
        1 => {
            p.path(&rounded(0.0), Some(blue), None);
            let q = d / 4.0;
            p.path(
                &[
                    PathSeg::M(x + q * 0.9, y + d / 2.0),
                    PathSeg::L(x + d / 2.0 - q / 3.0, y + d - q * 1.1),
                    PathSeg::L(x + d - q * 0.8, y + q),
                ],
                None,
                Some((Rgba(255, 255, 255, 255), (1.6 * k).max(1.4))),
            );
        }
        _ => {
            let edge = if t.enabled {
                Rgba(0x33, 0x33, 0x33, 255)
            } else {
                Rgba(0xbc, 0xbc, 0xbc, 255)
            };
            p.path(
                &rounded(0.0),
                Some(Rgba(255, 255, 255, 255)),
                Some((edge, 1.0)),
            );
        }
    }
    let line = font.size * 1.33;
    p.text_font(
        &t.label,
        x + d + 5.0 * k,
        (r[1] + (r[3] - line) / 2.0).round(),
        font,
        if t.enabled {
            Rgba(0, 0, 0, 255)
        } else {
            Rgba(0x83, 0x83, 0x83, 255)
        },
    );
}

/// Find Frame's wrapping pictures: lines of text around a small frame (own drawing).
fn paint_wrap(p: &mut dyn Painter, r: [f32; 4], around: bool, chosen: bool, k: f32) {
    let edge = if chosen {
        Rgba(0x00, 0x5f, 0xb8, 255)
    } else {
        Rgba(0x82, 0x82, 0x82, 255)
    };
    p.fill_rect(r[0], r[1], r[2], r[3], edge);
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let ink = Rgba(0x50, 0x50, 0x50, 255);
    let (bx, by, bw, bh) = (
        r[0] + r[2] / 2.0 - 9.0 * k,
        r[1] + r[3] / 2.0 - 7.0 * k,
        18.0 * k,
        14.0 * k,
    );
    let mut y = r[1] + 8.0 * k;
    while y < r[1] + r[3] - 6.0 * k {
        let beside = y > by - 3.0 * k && y < by + bh + 2.0 * k;
        if !beside {
            p.fill_rect(r[0] + 8.0 * k, y, r[2] - 16.0 * k, 1.0, ink);
        } else if around {
            p.fill_rect(r[0] + 8.0 * k, y, bx - r[0] - 12.0 * k, 1.0, ink);
            p.fill_rect(
                bx + bw + 4.0 * k,
                y,
                r[0] + r[2] - bx - bw - 12.0 * k,
                1.0,
                ink,
            );
        }
        y += 4.0 * k;
    }
    p.fill_rect(bx, by, bw, bh, Rgba(0, 0, 0, 255));
    p.fill_rect(
        bx + 1.5 * k,
        by + 1.5 * k,
        bw - 3.0 * k,
        bh - 3.0 * k,
        Rgba(255, 255, 255, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::{twips, Unit};

    // source: units of the reference's boxes (1" = 1440 twips, 1 pt = 20 twips, 1 cm = 567).
    #[test]
    fn lengths_typed_in_boxes() {
        assert_eq!(twips("0.5\"", Unit::Inch), Some(720));
        assert_eq!(twips("0.5", Unit::Inch), Some(720));
        assert_eq!(twips("6 pt", Unit::Inch), Some(120));
        assert_eq!(twips("12", Unit::Pt), Some(240));
        assert_eq!(twips("1 cm", Unit::Pt), Some(567));
        assert_eq!(twips("1,5 li", Unit::Pt), Some(360));
        assert_eq!(twips("", Unit::Pt), None);
        assert_eq!(twips("abc", Unit::Pt), None);
    }
}
