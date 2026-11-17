//! BlueEditor round 11: every property of a form field as the reference editor's field
//! Properties dialog shows it (General / Appearance / Position / Options / Format / Validate /
//! Calculate / Actions), read from and written back to the field and its widget
//! (ISO 32000-1 §12.7 interactive forms, §12.5.6.19 widget annotations, §12.6 actions), and the
//! design tools around it: tab order (`/Tabs`, §12.5 Table 30), alignment, centring and sizes of
//! several widgets.
//!
//! Format, Validate and Calculate are the usual Acrobat form JavaScript calls (`AFNumber_Format`,
//! `AFPercent_Format`, `AFDate_FormatEx`, `AFTime_FormatEx`, `AFSpecial_Format`,
//! `AFRange_Validate`, `AFSimple_Calculate`) in `/AA /F /K /V /C` (§12.6.3 Table 196), which
//! other readers run as they are.
// Field names follow the dialog; each struct documents its fields as a whole.
#![allow(missing_docs)]
use crate::annot::{nums, text_obj};
use crate::forms::{FieldKind, FormField};
use crate::{resolve, text_string, PdfDocument};
use lopdf::{Dictionary, Object, ObjectId, Stream};

fn n(v: f32) -> String {
    crate::objects::num(v)
}

/// How a field's value is shown (Format tab).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FieldFormat {
    /// As typed.
    #[default]
    None,
    /// Number: decimals, separator style (0 `1,234.56`, 1 `1234.56`, 2 `1.234,56`,
    /// 3 `1234,56`, 4 `1'234.56`), currency symbol (empty: none), parentheses, red.
    Number {
        decimals: u8,
        sep: u8,
        currency: String,
        parens: bool,
        red: bool,
    },
    /// Percentage: decimals and separator style.
    Percent { decimals: u8, sep: u8 },
    /// Date with its format (`m/d/yyyy`).
    Date(String),
    /// Time: 0 `HH:MM`, 1 `h:MM tt`, 2 `HH:MM:ss`, 3 `h:MM:ss tt`.
    Time(u8),
    /// Special: 0 Zip Code, 1 Zip Code+4, 2 Phone Number, 3 Social Security Number;
    /// with a mask: Arbitrary Mask.
    Special(u8, String),
    /// Custom format and keystroke scripts.
    Custom { format: String, keystroke: String },
}

/// Validation of a field's value (Validate tab).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FieldValidate {
    /// Not validated.
    #[default]
    None,
    /// In range: from / to (`None`: no bound).
    Range(Option<f64>, Option<f64>),
    /// Custom validation script.
    Script(String),
}

/// How a field's value is calculated (Calculate tab).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FieldCalc {
    /// Not calculated.
    #[default]
    None,
    /// `SUM` / `PRD` / `AVG` / `MIN` / `MAX` of the named fields.
    Simple(String, Vec<String>),
    /// Simplified field notation (`a + b * 2`).
    Notation(String),
    /// Custom calculation script.
    Script(String),
}

/// What an action does (Actions tab, §12.6.4).
#[derive(Debug, Clone, PartialEq)]
pub enum FieldActionKind {
    /// Execute a menu item (`/Named`).
    MenuItem(String),
    /// Go to a page view (0-based page).
    GoTo(usize),
    /// Import form data from a file.
    ImportData(String),
    /// Open a web link.
    WebLink(String),
    /// Open / execute a file.
    OpenFile(String),
    /// Reset a form.
    ResetForm,
    /// Run a JavaScript.
    JavaScript(String),
    /// Show (false) or hide (true) a field.
    ShowHide(String, bool),
    /// Submit a form to an address.
    Submit(String),
}

/// One action of a field: its trigger (`U` Mouse Up, `D` Mouse Down, `E` Mouse Enter,
/// `X` Mouse Exit, `Fo` On Receiving Focus, `Bl` On Losing Focus) and what it does.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAction {
    pub trigger: String,
    pub kind: FieldActionKind,
}

/// Every property of a field the Properties dialog shows.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldProps {
    /// Partial name (`/T`).
    pub name: String,
    /// Tooltip (`/TU`).
    pub tooltip: String,
    /// 0 Visible, 1 Hidden, 2 Visible but doesn't print, 3 Hidden but printable.
    pub visibility: u8,
    /// 0 / 90 / 180 / 270 (`/MK /R`).
    pub rotation: u16,
    pub read_only: bool,
    pub required: bool,
    /// Locked (annotation flag 8): the field cannot be moved or changed in design.
    pub locked: bool,
    /// Line style: 0 Solid, 1 Dashed, 2 Beveled, 3 Inset, 4 Underline.
    pub line_style: u8,
    /// Border width: 0 none, 1 thin, 2 medium, 3 thick.
    pub thickness: u8,
    /// Border / fill colour (`None`: no colour).
    pub border_color: Option<[f32; 3]>,
    pub fill_color: Option<[f32; 3]>,
    /// Font size (0: auto).
    pub font_size: f32,
    pub text_color: [f32; 3],
    /// Font name as listed (Helvetica, Times Roman, Courier…).
    pub font: String,
    pub rtl: bool,
    /// Widget box in PDF user space (left, bottom, right, top).
    pub rect: [f32; 4],
    /// Alignment: 0 left, 1 centre, 2 right (`/Q`).
    pub align: u8,
    /// Default value (`/DV`).
    pub default_value: String,
    pub scroll: bool,
    pub rich: bool,
    /// Maximum characters (0: no limit).
    pub max_len: u32,
    pub password: bool,
    pub file_select: bool,
    pub spell_check: bool,
    pub comb: bool,
    pub multiline: bool,
    /// Choices (combo / list boxes).
    pub options: Vec<String>,
    /// Combo box: custom text allowed; list box: several choices; both: sorted, commit at once.
    pub editable: bool,
    pub multi_select: bool,
    pub sort: bool,
    pub commit_now: bool,
    /// Check box / radio button: export value and checked by default.
    pub export_value: String,
    pub default_on: bool,
    /// Push button caption.
    pub caption: String,
    pub format: FieldFormat,
    pub validate: FieldValidate,
    pub calculate: FieldCalc,
    pub actions: Vec<FieldAction>,
}

impl Default for FieldProps {
    fn default() -> Self {
        Self {
            name: String::new(),
            tooltip: String::new(),
            visibility: 0,
            rotation: 0,
            read_only: false,
            required: false,
            locked: false,
            line_style: 0,
            thickness: 0,
            border_color: None,
            fill_color: None,
            font_size: 0.0,
            text_color: [0.0; 3],
            font: "Helvetica".into(),
            rtl: false,
            rect: [0.0; 4],
            align: 0,
            default_value: String::new(),
            scroll: true,
            rich: false,
            max_len: 0,
            password: false,
            file_select: false,
            spell_check: true,
            comb: false,
            multiline: false,
            options: Vec::new(),
            editable: false,
            multi_select: false,
            sort: false,
            commit_now: false,
            export_value: "Yes".into(),
            default_on: false,
            caption: String::new(),
            format: FieldFormat::None,
            validate: FieldValidate::None,
            calculate: FieldCalc::None,
            actions: Vec::new(),
        }
    }
}

/// Font names offered (the standard 14 families; §9.6.2.2) and their `/DA` resource names.
pub const FIELD_FONTS: [(&str, &str); 12] = [
    ("Times Roman", "TiRo"),
    ("Times Bold", "TiBo"),
    ("Times Italic", "TiIt"),
    ("Times BoldItalic", "TiBI"),
    ("Helvetica", "Helv"),
    ("Helvetica Bold", "HeBo"),
    ("Helvetica Oblique", "HeOb"),
    ("Helvetica BoldOblique", "HeBO"),
    ("Courier", "Cour"),
    ("Courier Bold", "CoBo"),
    ("Courier Oblique", "CoOb"),
    ("Courier BoldOblique", "CoBO"),
];

/// Date formats of the Format tab.
pub const DATE_FORMATS: [&str; 18] = [
    "m/d",
    "m/d/yy",
    "m/d/yyyy",
    "mm/dd/yy",
    "mm/dd/yyyy",
    "mm/yy",
    "mm/yyyy",
    "d-mmm",
    "d-mmm-yy",
    "d-mmm-yyyy",
    "dd-mmm-yy",
    "dd-mmm-yyyy",
    "yy-mm-dd",
    "yyyy-mm-dd",
    "mmm-yy",
    "mmmm-yy",
    "mmm d, yyyy",
    "mmmm d, yyyy",
];

/// Calculation operators of the Calculate tab (`AFSimple_Calculate` names).
pub const CALC_OPS: [&str; 5] = ["SUM", "PRD", "AVG", "MIN", "MAX"];

/// Action triggers in the Actions tab's order.
pub const TRIGGERS: [&str; 6] = ["U", "D", "E", "X", "Fo", "Bl"];

fn get<'a>(doc: &'a crate::store::Document, d: &'a Dictionary, k: &[u8]) -> Option<&'a Object> {
    d.get(k).ok().map(|o| resolve(doc, o))
}

fn obj_text(o: &Object) -> String {
    match o {
        Object::String(s, _) => text_string(s),
        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
        _ => String::new(),
    }
}

fn color_of(o: Option<&Object>) -> Option<[f32; 3]> {
    let a = o?.as_array().ok()?;
    let v: Vec<f32> = a.iter().filter_map(|x| x.as_float().ok()).collect();
    match v.as_slice() {
        [g] => Some([*g, *g, *g]),
        [r, g, b] => Some([*r, *g, *b]),
        [c, m, y, k] => Some([
            (1.0 - c) * (1.0 - k),
            (1.0 - m) * (1.0 - k),
            (1.0 - y) * (1.0 - k),
        ]),
        _ => None,
    }
}

/// Size, colour and font of a `/DA` string (`/Helv 12 Tf 0 0 1 rg`).
fn parse_da(da: &str) -> (String, f32, [f32; 3]) {
    let parts: Vec<&str> = da.split_whitespace().collect();
    let mut font = "Helvetica".to_owned();
    let mut size = 0.0;
    let mut color = [0.0; 3];
    for (i, p) in parts.iter().enumerate() {
        let num = |k: usize| -> f32 {
            i.checked_sub(k)
                .and_then(|j| parts.get(j))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0)
        };
        match *p {
            "Tf" => {
                size = num(1);
                if let Some(res) = i.checked_sub(2).and_then(|j| parts.get(j)) {
                    let res = res.trim_start_matches('/');
                    if let Some((name, _)) = FIELD_FONTS.iter().find(|(_, r)| *r == res) {
                        (*name).clone_into(&mut font);
                    } else if !res.is_empty() {
                        res.clone_into(&mut font);
                    }
                }
            }
            "g" => {
                let g = num(1);
                color = [g, g, g];
            }
            "rg" => color = [num(3), num(2), num(1)],
            "k" => {
                let (c, m, y, k) = (num(4), num(3), num(2), num(1));
                color = [
                    (1.0 - c) * (1.0 - k),
                    (1.0 - m) * (1.0 - k),
                    (1.0 - y) * (1.0 - k),
                ];
            }
            _ => {}
        }
    }
    (font, size, color)
}

fn js_of(doc: &crate::store::Document, a: &Dictionary) -> Option<String> {
    match get(doc, a, b"JS")? {
        Object::String(s, _) => Some(text_string(s)),
        Object::Stream(s) => crate::filters::stream_bytes(doc, s)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned()),
        _ => None,
    }
}

/// Arguments of the first `name(...)` call in `js`, split at top-level commas, quotes removed.
fn call_args(js: &str, name: &str) -> Option<Vec<String>> {
    let at = js.find(name)? + name.len();
    let rest = js.get(at..)?.trim_start();
    let rest = rest.strip_prefix('(')?;
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut args = Vec::new();
    let mut cur = String::new();
    for c in rest.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[') => {
                depth += 1;
                cur.push(c);
            }
            (None, ')' | ']') if depth > 0 => {
                depth -= 1;
                cur.push(c);
            }
            (None, ')') => {
                args.push(cur.trim().to_owned());
                return Some(args);
            }
            (None, ',') if depth == 0 => {
                args.push(cur.trim().to_owned());
                cur.clear();
            }
            (None, c) => cur.push(c),
        }
    }
    None
}

fn js_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn format_from_js(f: &str, k: &str) -> FieldFormat {
    if let Some(a) = call_args(f, "AFNumber_Format") {
        let num = |i: usize| a.get(i).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
        let neg = num(2);
        return FieldFormat::Number {
            decimals: num(0),
            sep: num(1),
            currency: a.get(4).cloned().unwrap_or_default(),
            parens: neg >= 2,
            red: neg == 1 || neg == 3,
        };
    }
    if let Some(a) = call_args(f, "AFPercent_Format") {
        let num = |i: usize| a.get(i).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
        return FieldFormat::Percent {
            decimals: num(0),
            sep: num(1),
        };
    }
    if let Some(a) = call_args(f, "AFDate_FormatEx") {
        return FieldFormat::Date(a.first().cloned().unwrap_or_default());
    }
    if let Some(a) = call_args(f, "AFTime_Format") {
        return FieldFormat::Time(a.first().and_then(|s| s.parse().ok()).unwrap_or(0));
    }
    if let Some(a) = call_args(k, "AFSpecial_KeystrokeEx") {
        return FieldFormat::Special(4, a.first().cloned().unwrap_or_default());
    }
    if let Some(a) = call_args(f, "AFSpecial_Format") {
        return FieldFormat::Special(
            a.first().and_then(|s| s.parse().ok()).unwrap_or(0),
            String::new(),
        );
    }
    if f.trim().is_empty() && k.trim().is_empty() {
        FieldFormat::None
    } else {
        FieldFormat::Custom {
            format: f.to_owned(),
            keystroke: k.to_owned(),
        }
    }
}

/// The format and keystroke scripts of a format.
#[must_use]
pub fn format_scripts(f: &FieldFormat) -> (String, String) {
    match f {
        FieldFormat::None => (String::new(), String::new()),
        FieldFormat::Number {
            decimals,
            sep,
            currency,
            parens,
            red,
        } => {
            let neg = u8::from(*red) + 2 * u8::from(*parens);
            let args = format!(
                "{decimals}, {sep}, {neg}, 0, \"{}\", true",
                js_str(currency)
            );
            (
                format!("AFNumber_Format({args});"),
                format!("AFNumber_Keystroke({args});"),
            )
        }
        FieldFormat::Percent { decimals, sep } => (
            format!("AFPercent_Format({decimals}, {sep});"),
            format!("AFPercent_Keystroke({decimals}, {sep});"),
        ),
        FieldFormat::Date(d) => (
            format!("AFDate_FormatEx(\"{}\");", js_str(d)),
            format!("AFDate_KeystrokeEx(\"{}\");", js_str(d)),
        ),
        FieldFormat::Time(t) => (
            format!("AFTime_Format({t});"),
            format!("AFTime_Keystroke({t});"),
        ),
        FieldFormat::Special(4, mask) => (
            String::new(),
            format!("AFSpecial_KeystrokeEx(\"{}\");", js_str(mask)),
        ),
        FieldFormat::Special(k, _) => (
            format!("AFSpecial_Format({k});"),
            format!("AFSpecial_Keystroke({k});"),
        ),
        FieldFormat::Custom { format, keystroke } => (format.clone(), keystroke.clone()),
    }
}

/// An example of the format, as the dialog shows under its options.
#[must_use]
pub fn format_example(f: &FieldFormat) -> String {
    fn group(int: &str, sep: char) -> String {
        let mut out = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i).is_multiple_of(3) {
                out.push(sep);
            }
            out.push(c);
        }
        out
    }
    let number = |v: f64, decimals: u8, sep: u8| {
        let s = format!("{v:.*}", usize::from(decimals));
        let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
        let (g, d) = match sep {
            0 => (Some(','), '.'),
            1 => (None, '.'),
            2 => (Some('.'), ','),
            3 => (None, ','),
            _ => (Some('\''), '.'),
        };
        let mut out = g.map_or_else(|| int.to_owned(), |g| group(int, g));
        if !frac.is_empty() {
            out.push(d);
            out.push_str(frac);
        }
        out
    };
    match f {
        FieldFormat::Number {
            decimals,
            sep,
            currency,
            ..
        } => format!("{currency}{}", number(4123.03, *decimals, *sep)),
        FieldFormat::Percent { decimals, sep } => {
            format!("{}%", number(412_303.45, *decimals, *sep))
        }
        FieldFormat::Date(d) => d
            .replace("yyyy", "2026")
            .replace("yy", "26")
            .replace("mmmm", "November")
            .replace("mmm", "Nov")
            .replace("mm", "11")
            .replace("dd", "17")
            .replace('d', "17")
            .replace('m', "11"),
        FieldFormat::Time(t) => ["13:27", "1:27 pm", "13:27:00", "1:27:00 pm"]
            .get(usize::from(*t))
            .copied()
            .unwrap_or("13:27")
            .to_owned(),
        _ => String::new(),
    }
}

fn action_of(
    doc: &crate::store::Document,
    a: &Dictionary,
    pages: &[ObjectId],
) -> Option<FieldActionKind> {
    let s = get(doc, a, b"S")?.as_name().ok()?;
    Some(match s {
        b"Named" => FieldActionKind::MenuItem(get(doc, a, b"N").map(obj_text).unwrap_or_default()),
        b"GoTo" => {
            let d = get(doc, a, b"D")?.as_array().ok()?;
            let p = d.first()?.as_reference().ok()?;
            FieldActionKind::GoTo(pages.iter().position(|x| *x == p).unwrap_or(0))
        }
        b"ImportData" => {
            FieldActionKind::ImportData(get(doc, a, b"F").map(obj_text).unwrap_or_default())
        }
        b"URI" => FieldActionKind::WebLink(get(doc, a, b"URI").map(obj_text).unwrap_or_default()),
        b"Launch" => FieldActionKind::OpenFile(get(doc, a, b"F").map(obj_text).unwrap_or_default()),
        b"ResetForm" => FieldActionKind::ResetForm,
        b"JavaScript" => FieldActionKind::JavaScript(js_of(doc, a).unwrap_or_default()),
        b"Hide" => FieldActionKind::ShowHide(
            get(doc, a, b"T").map(obj_text).unwrap_or_default(),
            get(doc, a, b"H")
                .and_then(|h| h.as_bool().ok())
                .unwrap_or(true),
        ),
        b"SubmitForm" => {
            let f = get(doc, a, b"F");
            let url = match f {
                Some(Object::Dictionary(d)) => get(doc, d, b"F").map(obj_text).unwrap_or_default(),
                Some(o) => obj_text(o),
                None => String::new(),
            };
            FieldActionKind::Submit(url)
        }
        _ => return None,
    })
}

fn action_dict(k: &FieldActionKind, pages: &[ObjectId]) -> Dictionary {
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"Action".to_vec()));
    let mut s = |name: &[u8]| d.set("S", Object::Name(name.to_vec()));
    match k {
        FieldActionKind::MenuItem(n) => {
            s(b"Named");
            d.set("N", Object::Name(n.as_bytes().to_vec()));
        }
        FieldActionKind::GoTo(p) => {
            s(b"GoTo");
            if let Some(id) = pages.get(*p) {
                d.set(
                    "D",
                    vec![Object::Reference(*id), Object::Name(b"Fit".to_vec())],
                );
            }
        }
        FieldActionKind::ImportData(f) => {
            s(b"ImportData");
            d.set("F", text_obj(f));
        }
        FieldActionKind::WebLink(u) => {
            s(b"URI");
            d.set(
                "URI",
                Object::String(u.as_bytes().to_vec(), lopdf::StringFormat::Literal),
            );
        }
        FieldActionKind::OpenFile(f) => {
            s(b"Launch");
            d.set("F", text_obj(f));
        }
        FieldActionKind::ResetForm => s(b"ResetForm"),
        FieldActionKind::JavaScript(js) => {
            s(b"JavaScript");
            d.set("JS", text_obj(js));
        }
        FieldActionKind::ShowHide(t, h) => {
            s(b"Hide");
            d.set("T", text_obj(t));
            d.set("H", *h);
        }
        FieldActionKind::Submit(u) => {
            s(b"SubmitForm");
            let mut f = Dictionary::new();
            f.set("FS", Object::Name(b"URL".to_vec()));
            f.set("F", text_obj(u));
            d.set("F", f);
        }
    }
    d
}

fn js_action(js: &str) -> Object {
    let mut d = Dictionary::new();
    d.set("S", Object::Name(b"JavaScript".to_vec()));
    d.set("JS", text_obj(js));
    Object::Dictionary(d)
}

/// The script Acrobat-style readers keep for simplified field notation.
fn notation_script(expr: &str) -> String {
    format!(
        "/** BVCALC {expr} EVCALC **/ event.value = AFSimpleCalc(\"{}\");",
        js_str(expr)
    )
}

const BIT_READ_ONLY: i64 = 1;
const BIT_REQUIRED: i64 = 1 << 1;
const BIT_MULTILINE: i64 = 1 << 12;
const BIT_PASSWORD: i64 = 1 << 13;
const BIT_COMBO: i64 = 1 << 17;
const BIT_EDIT: i64 = 1 << 18;
const BIT_SORT: i64 = 1 << 19;
const BIT_FILE_SELECT: i64 = 1 << 20;
const BIT_MULTI_SELECT: i64 = 1 << 21;
const BIT_NO_SPELL: i64 = 1 << 22;
const BIT_NO_SCROLL: i64 = 1 << 23;
const BIT_COMB: i64 = 1 << 24;
const BIT_RICH: i64 = 1 << 25;
const BIT_COMMIT: i64 = 1 << 26;

impl PdfDocument {
    fn attr(&self, f: &FormField, key: &[u8]) -> Option<Object> {
        // The widget first (a merged field / widget), then the field and its parents.
        let mut cur = f.widget;
        for _ in 0..32 {
            let d = self.doc.get_dictionary(cur).ok()?;
            if let Some(v) = get(&self.doc, d, key).filter(|v| !matches!(v, Object::Null)) {
                return Some(v.clone());
            }
            cur = d.get(b"Parent").ok()?.as_reference().ok()?;
        }
        None
    }

    /// Every property of the field of widget `f`.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn field_props(&self, f: &FormField) -> FieldProps {
        let doc = &self.doc;
        let mut p = FieldProps::default();
        let field = doc.get_dictionary(f.field).ok();
        let widget = doc.get_dictionary(f.widget).ok();
        p.name = field
            .and_then(|d| get(doc, d, b"T"))
            .map_or_else(|| f.name.clone(), obj_text);
        p.tooltip = self
            .attr(f, b"TU")
            .map(|o| obj_text(&o))
            .unwrap_or_default();
        let flags = self
            .attr(f, b"Ff")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let aflags = widget
            .and_then(|d| get(doc, d, b"F"))
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(4);
        p.visibility = match (aflags & 2 != 0, aflags & 32 != 0, aflags & 4 != 0) {
            (true, _, _) => 1,
            (false, true, _) => 3,
            (false, false, true) => 0,
            (false, false, false) => 2,
        };
        p.locked = aflags & 128 != 0;
        p.read_only = flags & BIT_READ_ONLY != 0;
        p.required = flags & BIT_REQUIRED != 0;
        let mk = widget
            .and_then(|d| get(doc, d, b"MK"))
            .and_then(|o| o.as_dict().ok());
        p.rotation = mk
            .and_then(|m| get(doc, m, b"R"))
            .and_then(|o| o.as_i64().ok())
            .map_or(0, |r| u16::try_from(r.rem_euclid(360)).unwrap_or(0));
        p.border_color = color_of(mk.and_then(|m| get(doc, m, b"BC")));
        p.fill_color = color_of(mk.and_then(|m| get(doc, m, b"BG")));
        let bs = widget
            .and_then(|d| get(doc, d, b"BS"))
            .and_then(|o| o.as_dict().ok());
        p.line_style = match bs
            .and_then(|b| get(doc, b, b"S"))
            .and_then(|o| o.as_name().ok())
        {
            Some(b"D") => 1,
            Some(b"B") => 2,
            Some(b"I") => 3,
            Some(b"U") => 4,
            _ => 0,
        };
        let width = bs
            .and_then(|b| get(doc, b, b"W"))
            .and_then(|o| o.as_float().ok())
            .unwrap_or(if p.border_color.is_some() { 1.0 } else { 0.0 });
        p.thickness = if p.border_color.is_none() || width <= 0.0 {
            0
        } else if width < 1.5 {
            1
        } else if width < 2.5 {
            2
        } else {
            3
        };
        let da = self
            .attr(f, b"DA")
            .map(|o| obj_text(&o))
            .unwrap_or_default();
        let (font, size, color) = parse_da(&da);
        p.font = font;
        p.font_size = size;
        p.text_color = color;
        if let Some(r) = widget
            .and_then(|d| get(doc, d, b"Rect"))
            .and_then(|o| o.as_array().ok())
        {
            let v: Vec<f32> = r
                .iter()
                .filter_map(|o| resolve(doc, o).as_float().ok())
                .collect();
            if let [a, b, c, d] = v.as_slice() {
                p.rect = [a.min(*c), b.min(*d), a.max(*c), b.max(*d)];
            }
        }
        p.align = self
            .attr(f, b"Q")
            .and_then(|o| o.as_i64().ok())
            .map_or(0, |q| u8::try_from(q.clamp(0, 2)).unwrap_or(0));
        p.default_value = self
            .attr(f, b"DV")
            .map(|o| obj_text(&o))
            .unwrap_or_default();
        p.scroll = flags & BIT_NO_SCROLL == 0;
        p.rich = flags & BIT_RICH != 0;
        p.max_len = self
            .attr(f, b"MaxLen")
            .and_then(|o| o.as_i64().ok())
            .map_or(0, |m| u32::try_from(m.max(0)).unwrap_or(0));
        p.password = flags & BIT_PASSWORD != 0;
        p.file_select = flags & BIT_FILE_SELECT != 0;
        p.spell_check = flags & BIT_NO_SPELL == 0;
        p.comb = flags & BIT_COMB != 0;
        p.multiline = flags & BIT_MULTILINE != 0;
        p.options.clone_from(&f.options);
        p.editable = flags & BIT_EDIT != 0;
        p.multi_select = flags & BIT_MULTI_SELECT != 0;
        p.sort = flags & BIT_SORT != 0;
        p.commit_now = flags & BIT_COMMIT != 0;
        if let Some(on) = &f.on_state {
            p.export_value = String::from_utf8_lossy(on).into_owned();
        }
        p.default_on = self
            .attr(f, b"DV")
            .and_then(|o| o.as_name().ok().map(<[u8]>::to_vec))
            .is_some_and(|v| v != b"Off" && Some(&v) == f.on_state.as_ref());
        p.caption = mk
            .and_then(|m| get(doc, m, b"CA"))
            .map(obj_text)
            .unwrap_or_default();
        if f.kind == FieldKind::Button && p.caption.is_empty() {
            p.caption.clone_from(&f.value);
        }
        // Additional actions of the field (format, keystroke, validate, calculate) and of the
        // widget (mouse and focus triggers), §12.6.3.
        let aa = |id: ObjectId| {
            doc.get_dictionary(id)
                .ok()
                .and_then(|d| get(doc, d, b"AA"))
                .and_then(|o| o.as_dict().ok())
        };
        let script = |key: &[u8]| {
            [f.field, f.widget]
                .into_iter()
                .filter_map(aa)
                .find_map(|a| get(doc, a, key).and_then(|o| o.as_dict().ok()))
                .and_then(|a| js_of(doc, a))
                .unwrap_or_default()
        };
        p.format = format_from_js(&script(b"F"), &script(b"K"));
        let v = script(b"V");
        p.validate = if let Some(a) = call_args(&v, "AFRange_Validate") {
            let b = |i: usize| a.get(i).is_some_and(|s| s == "true");
            let x = |i: usize| a.get(i).and_then(|s| s.parse::<f64>().ok());
            FieldValidate::Range(x(1).filter(|_| b(0)), x(3).filter(|_| b(2)))
        } else if v.trim().is_empty() {
            FieldValidate::None
        } else {
            FieldValidate::Script(v)
        };
        let c = script(b"C");
        p.calculate = if let Some(i) = c.find("BVCALC") {
            let rest = &c[i + 6..];
            let end = rest.find("EVCALC").unwrap_or(rest.len());
            FieldCalc::Notation(rest[..end].trim().to_owned())
        } else if let Some((op, names)) = crate::forms11::parse_simple_calculate(&c) {
            FieldCalc::Simple(op, names)
        } else if c.trim().is_empty() {
            FieldCalc::None
        } else {
            FieldCalc::Script(c)
        };
        let pages = self.pages.iter().map(|p| p.id).collect::<Vec<_>>();
        if let Some(a) = widget
            .and_then(|d| get(doc, d, b"A"))
            .and_then(|o| o.as_dict().ok())
            .and_then(|a| action_of(doc, a, &pages))
        {
            p.actions.push(FieldAction {
                trigger: "U".into(),
                kind: a,
            });
        }
        if let Some(a) = aa(f.widget) {
            for t in TRIGGERS {
                if let Some(k) = get(doc, a, t.as_bytes())
                    .and_then(|o| o.as_dict().ok())
                    .and_then(|d| action_of(doc, d, &pages))
                {
                    p.actions.push(FieldAction {
                        trigger: t.into(),
                        kind: k,
                    });
                }
            }
        }
        p
    }

    /// Write `p` to the field of widget `f` and redraw its appearance (one undo step).
    ///
    /// # Errors
    /// When the name is empty or the box too small.
    #[allow(clippy::too_many_lines)]
    pub fn set_field_props(&mut self, f: &FormField, p: &FieldProps) -> Result<(), String> {
        if p.name.trim().is_empty() {
            return Err("the field needs a name".into());
        }
        if p.rect[2] - p.rect[0] < 1.0 || p.rect[3] - p.rect[1] < 1.0 {
            return Err("the field is too small".into());
        }
        let pages = self.pages.iter().map(|p| p.id).collect::<Vec<_>>();
        self.begin("field properties");
        self.touch(f.field);
        self.touch(f.widget);
        let old_flags = self
            .attr(f, b"Ff")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let mut flags = old_flags
            & !(BIT_READ_ONLY
                | BIT_REQUIRED
                | BIT_MULTILINE
                | BIT_PASSWORD
                | BIT_EDIT
                | BIT_SORT
                | BIT_FILE_SELECT
                | BIT_MULTI_SELECT
                | BIT_NO_SPELL
                | BIT_NO_SCROLL
                | BIT_COMB
                | BIT_RICH
                | BIT_COMMIT);
        let set = |flags: &mut i64, bit: i64, on: bool| {
            if on {
                *flags |= bit;
            }
        };
        set(&mut flags, BIT_READ_ONLY, p.read_only);
        set(&mut flags, BIT_REQUIRED, p.required);
        match f.kind {
            FieldKind::Text => {
                set(&mut flags, BIT_MULTILINE, p.multiline);
                set(&mut flags, BIT_PASSWORD, p.password);
                set(&mut flags, BIT_FILE_SELECT, p.file_select);
                set(&mut flags, BIT_NO_SPELL, !p.spell_check);
                set(&mut flags, BIT_NO_SCROLL, !p.scroll);
                set(&mut flags, BIT_COMB, p.comb && p.max_len > 0);
                set(&mut flags, BIT_RICH, p.rich);
            }
            FieldKind::Combo | FieldKind::List => {
                set(&mut flags, BIT_COMBO, f.kind == FieldKind::Combo);
                set(
                    &mut flags,
                    BIT_EDIT,
                    p.editable && f.kind == FieldKind::Combo,
                );
                set(&mut flags, BIT_SORT, p.sort);
                set(
                    &mut flags,
                    BIT_MULTI_SELECT,
                    p.multi_select && f.kind == FieldKind::List,
                );
                set(&mut flags, BIT_NO_SPELL, !p.spell_check);
                set(&mut flags, BIT_COMMIT, p.commit_now);
            }
            _ => {}
        }
        let font_res = FIELD_FONTS
            .iter()
            .find(|(n, _)| *n == p.font)
            .map_or("Helv", |(_, r)| *r);
        let [r, g, b] = p.text_color;
        let da = format!(
            "/{font_res} {} Tf {} {} {} rg",
            n(p.font_size),
            n(r),
            n(g),
            n(b)
        );
        let (fmt_js, key_js) = format_scripts(&p.format);
        let val_js = match &p.validate {
            FieldValidate::None => String::new(),
            FieldValidate::Range(a, b) => format!(
                "AFRange_Validate({}, {}, {}, {});",
                a.is_some(),
                a.unwrap_or(0.0),
                b.is_some(),
                b.unwrap_or(0.0)
            ),
            FieldValidate::Script(s) => s.clone(),
        };
        let calc_js = match &p.calculate {
            FieldCalc::None => String::new(),
            FieldCalc::Simple(op, names) => format!(
                "AFSimple_Calculate(\"{op}\", new Array ({}));",
                names
                    .iter()
                    .map(|n| format!("\"{}\"", js_str(n)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            FieldCalc::Notation(e) => notation_script(e),
            FieldCalc::Script(s) => s.clone(),
        };
        let same = f.field == f.widget;
        // Field-level entries.
        if let Ok(d) = self.doc.get_dictionary_mut(f.field) {
            d.set("T", text_obj(p.name.trim()));
            if p.tooltip.is_empty() {
                d.remove(b"TU");
            } else {
                d.set("TU", text_obj(&p.tooltip));
            }
            d.set("Ff", flags);
            if matches!(f.kind, FieldKind::Text | FieldKind::Combo | FieldKind::List) {
                d.set("DA", text_obj(&da));
                d.set("Q", i64::from(p.align.min(2)));
            }
            match f.kind {
                FieldKind::Text => {
                    if p.max_len > 0 {
                        d.set("MaxLen", i64::from(p.max_len));
                    } else {
                        d.remove(b"MaxLen");
                    }
                    if p.default_value.is_empty() {
                        d.remove(b"DV");
                    } else {
                        d.set("DV", text_obj(&p.default_value));
                    }
                }
                FieldKind::Combo | FieldKind::List => {
                    let mut opts = p.options.clone();
                    if p.sort {
                        opts.sort();
                    }
                    d.set("Opt", opts.iter().map(|o| text_obj(o)).collect::<Vec<_>>());
                    if p.default_value.is_empty() {
                        d.remove(b"DV");
                    } else {
                        d.set("DV", text_obj(&p.default_value));
                    }
                }
                FieldKind::CheckBox | FieldKind::Radio => {
                    if p.default_on {
                        d.set("DV", Object::Name(p.export_value.as_bytes().to_vec()));
                    } else {
                        d.set("DV", Object::Name(b"Off".to_vec()));
                    }
                }
                _ => {}
            }
            let mut aa = d
                .get(b"AA")
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            for (k, js) in [
                (b"F".as_slice(), &fmt_js),
                (b"K".as_slice(), &key_js),
                (b"V".as_slice(), &val_js),
                (b"C".as_slice(), &calc_js),
            ] {
                if js.is_empty() {
                    aa.remove(k);
                } else {
                    aa.set(k.to_vec(), js_action(js));
                }
            }
            if same {
                // Merged field / widget: the mouse triggers join below.
                d.set("AA", aa);
            } else if aa.is_empty() {
                d.remove(b"AA");
            } else {
                d.set("AA", aa);
            }
        }
        // Widget-level entries.
        let mut aflags = self
            .doc
            .get_dictionary(f.widget)
            .ok()
            .and_then(|d| d.get(b"F").ok())
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(4)
            & !(2 | 4 | 32 | 128);
        aflags |= match p.visibility {
            1 => 2,
            2 => 0,
            3 => 32 | 4,
            _ => 4,
        };
        if p.locked {
            aflags |= 128;
        }
        if let Ok(d) = self.doc.get_dictionary_mut(f.widget) {
            d.set("F", aflags);
            d.set("Rect", nums(&p.rect));
            let mut mk = d
                .get(b"MK")
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            mk.set("R", i64::from(p.rotation % 360));
            match p.border_color {
                Some(c) if p.thickness > 0 => mk.set("BC", nums(&c)),
                _ => {
                    mk.remove(b"BC");
                }
            }
            match p.fill_color {
                Some(c) => mk.set("BG", nums(&c)),
                None => {
                    mk.remove(b"BG");
                }
            }
            if f.kind == FieldKind::Button {
                mk.set("CA", text_obj(&p.caption));
            }
            d.set("MK", mk);
            let mut bs = Dictionary::new();
            bs.set(
                "S",
                Object::Name(vec![b"SDBIU"[usize::from(p.line_style.min(4))]]),
            );
            bs.set("W", i64::from(p.thickness.min(3)));
            if p.line_style == 1 {
                bs.set("D", vec![Object::Integer(3)]);
            }
            d.set("BS", bs);
            d.remove(b"A");
            let mut aa = d
                .get(b"AA")
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            for t in TRIGGERS {
                aa.remove(t.as_bytes());
            }
            for a in &p.actions {
                if TRIGGERS.contains(&a.trigger.as_str()) {
                    aa.set(
                        a.trigger.as_bytes().to_vec(),
                        Object::Dictionary(action_dict(&a.kind, &pages)),
                    );
                }
            }
            if aa.is_empty() {
                d.remove(b"AA");
            } else {
                d.set("AA", aa);
            }
        }
        let r = self.field_appearance11(f, p);
        self.commit();
        r
    }

    /// Redraw the appearance of widget `f` from its properties: fill, border in its style,
    /// then the value (text and choice fields), the check mark, or the caption.
    fn field_appearance11(&mut self, f: &FormField, p: &FieldProps) -> Result<(), String> {
        let (w, h) = (p.rect[2] - p.rect[0], p.rect[3] - p.rect[1]);
        let (w, h) = if p.rotation % 180 == 90 {
            (h, w)
        } else {
            (w, h)
        };
        let bw = f32::from(p.thickness.min(3));
        let mut base = String::new();
        if let Some([r, g, b]) = p.fill_color {
            base.push_str(&format!(
                "{} {} {} rg 0 0 {} {} re f\n",
                n(r),
                n(g),
                n(b),
                n(w),
                n(h)
            ));
        }
        if let (Some([r, g, b]), true) = (p.border_color, bw > 0.0) {
            let half = bw / 2.0;
            base.push_str(&format!("{} {} {} RG {} w ", n(r), n(g), n(b), n(bw)));
            match p.line_style {
                4 => base.push_str(&format!("0 {half} m {} {half} l S\n", n(w))),
                1 => base.push_str(&format!(
                    "[3] 0 d {half} {half} {} {} re S [] 0 d\n",
                    n(w - bw),
                    n(h - bw)
                )),
                _ => base.push_str(&format!("{half} {half} {} {} re S\n", n(w - bw), n(h - bw))),
            }
            if matches!(p.line_style, 2 | 3) {
                // Beveled / inset: a light and a dark edge inside the border (§12.7.3.3).
                let (light, dark) = if p.line_style == 2 {
                    ("1 g", "0.5 g")
                } else {
                    ("0.5 g", "0.75 g")
                };
                base.push_str(&format!(
                    "{light} {bw} {bw} m {bw} {} l {} {} l {} {} l {} {} l {} {} l f\n",
                    n(h - bw),
                    n(w - bw),
                    n(h - bw),
                    n(w - 2.0 * bw),
                    n(h - 2.0 * bw),
                    n(2.0 * bw),
                    n(h - 2.0 * bw),
                    n(2.0 * bw),
                    n(2.0 * bw),
                ));
                base.push_str(&format!(
                    "{dark} {} {bw} m {} {} l {} {} l {} {} l {} {} l {bw} {bw} l f\n",
                    n(w - bw),
                    n(w - bw),
                    n(h - bw),
                    n(w - 2.0 * bw),
                    n(h - 2.0 * bw),
                    n(w - 2.0 * bw),
                    n(2.0 * bw),
                    n(2.0 * bw),
                    n(2.0 * bw),
                ));
            }
        }
        let matrix = match p.rotation % 360 {
            90 => Some([0.0, 1.0, -1.0, 0.0, h, 0.0]),
            180 => Some([-1.0, 0.0, 0.0, -1.0, w, h]),
            270 => Some([0.0, -1.0, 1.0, 0.0, 0.0, w]),
            _ => None,
        };
        let form = |me: &mut Self, content: String, res: Dictionary| -> ObjectId {
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"XObject".to_vec()));
            d.set("Subtype", Object::Name(b"Form".to_vec()));
            d.set("BBox", nums(&[0.0, 0.0, w, h]));
            if let Some(m) = matrix {
                d.set("Matrix", nums(&m));
            }
            d.set("Resources", res);
            me.add(Object::Stream(Stream::new(d, content.into_bytes())))
        };
        let [tr, tg, tb] = p.text_color;
        let ink = format!("{} {} {} rg", n(tr), n(tg), n(tb));
        let value_text = |me: &mut Self,
                          value: &str,
                          align: u8,
                          multiline: bool|
         -> Result<(String, Dictionary), String> {
            let mut res = Dictionary::new();
            let mut s = String::from("/Tx BMC\n");
            if !value.is_empty() {
                let pad = bw.max(1.0) + 1.0;
                let size = if p.font_size > 0.0 {
                    p.font_size
                } else if multiline {
                    10.0
                } else {
                    (h * 0.7).clamp(4.0, 12.0)
                };
                let (font, _) = me.embed_text(crate::fallback::SANS, "")?;
                let mut fonts = Dictionary::new();
                fonts.set("BoF", Object::Reference(font));
                res.set("Font", fonts);
                let lines: Vec<&str> = if multiline {
                    value.lines().collect()
                } else {
                    vec![value.lines().next().unwrap_or("")]
                };
                let first_y = if multiline {
                    h - pad - size
                } else {
                    (h - size * 0.7) / 2.0
                };
                s.push_str(&format!(
                    "q {pad} {pad} {} {} re W n BT /BoF {} Tf {ink}\n",
                    n(w - 2.0 * pad),
                    n(h - 2.0 * pad),
                    n(size)
                ));
                for (k, line) in lines.iter().enumerate() {
                    let shown: String = if p.password && f.kind == FieldKind::Text {
                        "*".repeat(line.chars().count())
                    } else {
                        (*line).to_owned()
                    };
                    let tw = crate::forms11::typed_width(&shown, crate::fallback::SANS) * size;
                    let x = match align {
                        1 => (w - tw) / 2.0,
                        2 => w - pad - 1.0 - tw,
                        _ => pad + 1.0,
                    };
                    let y = first_y - size * 1.15 * k as f32;
                    let (_, bytes) = me.embed_text(crate::fallback::SANS, &shown)?;
                    let mut hex = String::from("<");
                    for b in bytes {
                        hex.push_str(&format!("{b:02X}"));
                    }
                    hex.push('>');
                    s.push_str(&format!("1 0 0 1 {} {} Tm {hex} Tj\n", n(x), n(y)));
                }
                s.push_str("ET Q\n");
            }
            s.push_str("EMC\n");
            Ok((s, res))
        };
        let ap = match f.kind {
            FieldKind::Text | FieldKind::Combo | FieldKind::List => {
                let (s, res) = value_text(self, &f.value, p.align, p.multiline)?;
                let mut content = base.clone();
                if f.kind == FieldKind::Combo {
                    let a = (h * 0.8).min(14.0);
                    content.push_str(&format!(
                        "0.75 g {} {} {} {} re f 0 g {} {} m {} {} l {} {} l f\n",
                        n(w - a - bw),
                        n(bw),
                        n(a),
                        n(h - 2.0 * bw),
                        n(w - a * 0.8 - bw),
                        n(h * 0.6),
                        n(w - a * 0.2 - bw),
                        n(h * 0.6),
                        n(w - a * 0.5 - bw),
                        n(h * 0.35)
                    ));
                }
                content.push_str(&s);
                let id = form(self, content, res);
                let mut d = Dictionary::new();
                d.set("N", Object::Reference(id));
                d
            }
            FieldKind::CheckBox | FieldKind::Radio => {
                let on_name = if p.export_value.trim().is_empty() {
                    b"Yes".to_vec()
                } else {
                    p.export_value.trim().as_bytes().to_vec()
                };
                let s = w.min(h);
                let mark = if f.kind == FieldKind::Radio {
                    let r = s * 0.25;
                    let (cx, cy) = (w / 2.0, h / 2.0);
                    let k = 0.5523 * r;
                    format!(
                        "{ink} {} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c f\n",
                        n(cx + r), n(cy),
                        n(cx + r), n(cy + k), n(cx + k), n(cy + r), n(cx), n(cy + r),
                        n(cx - k), n(cy + r), n(cx - r), n(cy + k), n(cx - r), n(cy),
                        n(cx - r), n(cy - k), n(cx - k), n(cy - r), n(cx), n(cy - r),
                        n(cx + k), n(cy - r), n(cx + r), n(cy - k), n(cx + r), n(cy),
                    )
                } else {
                    format!(
                        "{} {} {} RG {} w 1 J 1 j {} {} m {} {} l {} {} l S\n",
                        n(tr),
                        n(tg),
                        n(tb),
                        n((s * 0.1).max(1.0)),
                        n(w * 0.22),
                        n(h * 0.5),
                        n(w * 0.42),
                        n(h * 0.28),
                        n(w * 0.78),
                        n(h * 0.75)
                    )
                };
                let on = form(self, format!("{base}{mark}"), Dictionary::new());
                let off = form(self, base.clone(), Dictionary::new());
                let mut states = Dictionary::new();
                states.set(on_name.clone(), Object::Reference(on));
                states.set("Off", Object::Reference(off));
                let checked = f.checked;
                if let Ok(d) = self.doc.get_dictionary_mut(f.widget) {
                    d.set(
                        "AS",
                        Object::Name(if checked { on_name } else { b"Off".to_vec() }),
                    );
                }
                let mut d = Dictionary::new();
                d.set("N", states);
                d
            }
            FieldKind::Button => {
                let (s, res) = value_text(self, &p.caption, 1, false)?;
                let id = form(self, format!("{base}{s}"), res);
                let mut d = Dictionary::new();
                d.set("N", Object::Reference(id));
                d
            }
            FieldKind::Signature => {
                let id = form(self, base, Dictionary::new());
                let mut d = Dictionary::new();
                d.set("N", Object::Reference(id));
                d
            }
        };
        if let Ok(d) = self.doc.get_dictionary_mut(f.widget) {
            d.set("AP", ap);
        }
        Ok(())
    }

    /// Set how Tab moves through the fields of page `page`: `R` rows, `C` columns, `S` document
    /// structure, or `None` (unspecified: the order of `/Annots`, set by [`Self::set_field_order`]).
    ///
    /// # Errors
    /// When there is no such page.
    pub fn set_tab_order(&mut self, page: usize, order: Option<&str>) -> Result<(), String> {
        let id = self.pages.get(page).ok_or("no such page")?.id;
        self.begin("tab order");
        self.touch(id);
        if let Ok(d) = self.doc.get_dictionary_mut(id) {
            match order {
                Some(o) => d.set("Tabs", Object::Name(o.as_bytes().to_vec())),
                None => {
                    d.remove(b"Tabs");
                }
            }
        }
        self.commit();
        Ok(())
    }

    /// The tab order of page `page`: `R`, `C`, `S` or empty (unspecified).
    #[must_use]
    pub fn tab_order(&self, page: usize) -> String {
        self.pages
            .get(page)
            .and_then(|i| self.doc.get_dictionary(i.id).ok())
            .and_then(|d| d.get(b"Tabs").ok())
            .and_then(|t| t.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default()
    }

    /// Order manually: the widgets `order` go first in page `page`'s `/Annots`, in that order,
    /// and the page's `/Tabs` is removed (one undo step).
    ///
    /// # Errors
    /// When there is no such page.
    pub fn set_field_order(&mut self, page: usize, order: &[FormField]) -> Result<(), String> {
        let id = self.pages.get(page).ok_or("no such page")?.id;
        let current: Vec<ObjectId> = self
            .doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"Annots").ok())
            .map(|o| resolve(&self.doc, o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default();
        let mut out: Vec<ObjectId> = order
            .iter()
            .map(|f| f.widget)
            .filter(|w| current.contains(w))
            .collect();
        let rest: Vec<ObjectId> = current
            .iter()
            .copied()
            .filter(|w| !out.contains(w))
            .collect();
        out.extend(rest);
        self.begin("tab order");
        self.touch(id);
        if let Ok(d) = self.doc.get_dictionary_mut(id) {
            d.set(
                "Annots",
                out.into_iter().map(Object::Reference).collect::<Vec<_>>(),
            );
            d.remove(b"Tabs");
        }
        self.commit();
        Ok(())
    }

    /// Move and size widgets: each `(widget, rect)` gets the new box (PDF user space, left,
    /// bottom, right, top) and a redrawn appearance (one undo step).
    ///
    /// # Errors
    /// When a box is too small.
    pub fn set_field_rects(&mut self, moves: &[(FormField, [f32; 4])]) -> Result<(), String> {
        self.begin("arrange fields");
        let mut r = Ok(());
        for (f, rect) in moves {
            let mut p = self.field_props(f);
            p.rect = *rect;
            if let Err(e) = self.set_field_props(f, &p) {
                r = Err(e);
                break;
            }
        }
        self.commit();
        r
    }

    /// The box of widget `f` in PDF user space (left, bottom, right, top).
    #[must_use]
    pub fn field_rect(&self, f: &FormField) -> [f32; 4] {
        self.field_props(f).rect
    }

    /// The page's crop box (left, bottom, right, top) in PDF user space.
    #[must_use]
    pub fn page_box(&self, page: usize) -> [f32; 4] {
        self.pages.get(page).map_or([0.0; 4], |i| i.crop)
    }

    /// Delete widgets (and their fields when no widget is left): out of the pages' `/Annots`,
    /// the AcroForm's `/Fields` and their parents' `/Kids` (one undo step).
    ///
    /// # Errors
    /// When a page is missing.
    pub fn delete_fields(&mut self, fields: &[FormField]) -> Result<(), String> {
        let root = self
            .doc
            .trailer
            .get(b"Root")
            .and_then(Object::as_reference)
            .map_err(|e| e.to_string())?;
        self.begin("delete fields");
        let mut r = Ok(());
        for f in fields {
            if let Err(e) = self.remove_annots(f.page, &[f.widget]) {
                r = Err(e);
            }
            let gone: Vec<ObjectId> = if f.field == f.widget {
                vec![f.widget]
            } else {
                let left = self
                    .form_fields()
                    .iter()
                    .filter(|x| x.field == f.field && x.widget != f.widget)
                    .count();
                if left == 0 {
                    vec![f.widget, f.field]
                } else {
                    vec![f.widget]
                }
            };
            // Out of the parents' kids and the AcroForm's fields.
            let mut holders: Vec<ObjectId> = gone
                .iter()
                .filter_map(|id| {
                    self.doc
                        .get_dictionary(*id)
                        .ok()?
                        .get(b"Parent")
                        .ok()?
                        .as_reference()
                        .ok()
                })
                .collect();
            let acro = self
                .doc
                .get_dictionary(root)
                .ok()
                .and_then(|c| c.get(b"AcroForm").ok().cloned());
            if let Some(Object::Reference(a)) = acro {
                holders.push(a);
            }
            for h in holders {
                self.touch(h);
                if let Ok(d) = self.doc.get_dictionary_mut(h) {
                    for key in [b"Kids".as_slice(), b"Fields".as_slice()] {
                        if let Ok(Object::Array(v)) = d.get_mut(key) {
                            v.retain(|o| o.as_reference().map_or(true, |x| !gone.contains(&x)));
                        }
                    }
                }
            }
            if !matches!(acro, Some(Object::Reference(_))) {
                self.touch(root);
                if let Ok(c) = self.doc.get_dictionary_mut(root) {
                    if let Ok(Object::Dictionary(a)) = c.get_mut(b"AcroForm") {
                        if let Ok(Object::Array(v)) = a.get_mut(b"Fields") {
                            v.retain(|o| o.as_reference().map_or(true, |x| !gone.contains(&x)));
                        }
                    }
                }
            }
        }
        self.commit();
        r
    }
}

#[cfg(test)]
#[path = "field_props_tests.rs"]
mod tests;
