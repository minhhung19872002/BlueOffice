//! Dialogs of BlueDoc round 23 (docs/ui/REPORT.md "Vòng 113"): the Font dialog with its Font
//! and Advanced tabs as the reference word processor's (Character Spacing: scale §17.3.2.43
//! w, spacing §17.3.2.35, position §17.3.2.24, kerning §17.3.2.19; OpenType Features:
//! ligatures, number spacing, number forms, stylistic sets, contextual alternates —
//! [MS-DOCX] w14), Set Numbering Value (§17.9.26 startOverride), Define New Bullet / Number
//! Format / Multilevel List (§17.9.6 lvl) and Sort Text. Laid out after the reference's
//! dialogs at 125 % (the Font dialog has the Find Font dialog's size, 584 × 657 here).
//! A child module of `doc_forms`.
use super::{DocForm, DocFormKind, FormContext, FormOut, Kind};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_edit::{CharFormat, ListNumbering};
use bo_render::Rgba;
use serde_json::{json, Value};

fn t(s: &Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

/// Which round-23 dialog `kind` is.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::Font23
            | DocFormKind::SetNumbering
            | DocFormKind::DefineBullet
            | DocFormKind::DefineNumber
            | DocFormKind::DefineMultilevel
            | DocFormKind::SortText
            | DocFormKind::Field23
            | DocFormKind::SelPane23
    )
}

/// Font style list (key, English): the reference's four.
const STYLES: [(&str, &str); 4] = [
    ("doc23.fsRegular", "Regular"),
    ("doc23.fsItalic", "Italic"),
    ("doc23.fsBold", "Bold"),
    ("doc23.fsBoldItalic", "Bold Italic"),
];

/// Font colours offered (hex, key, English) after Automatic.
const COLORS: [(&str, &str, &str); 10] = [
    ("000000", "doc23.cBlack", "Black"),
    ("FF0000", "doc23.cRed", "Red"),
    ("C00000", "doc23.cDarkRed", "Dark Red"),
    ("FFC000", "doc23.cOrange", "Orange"),
    ("FFFF00", "doc23.cYellow", "Yellow"),
    ("00B050", "doc23.cGreen", "Green"),
    ("00B0F0", "doc23.cLightBlue", "Light Blue"),
    ("0070C0", "doc23.cBlue", "Blue"),
    ("002060", "doc23.cDarkBlue", "Dark Blue"),
    ("7030A0", "doc23.cPurple", "Purple"),
];

/// Underline styles (§17.18.99 name, key, English), as the reference's list.
const UNDERLINES: [(&str, &str, &str); 12] = [
    ("", "doc23.uNone", "(none)"),
    ("words", "doc23.uWords", "Words only"),
    ("single", "doc23.uSingle", "Single"),
    ("double", "doc23.uDouble", "Double"),
    ("thick", "doc23.uThick", "Thick"),
    ("dotted", "doc23.uDotted", "Dotted"),
    ("dash", "doc23.uDash", "Dash"),
    ("dashLong", "doc23.uDashLong", "Long dash"),
    ("dotDash", "doc23.uDotDash", "Dot dash"),
    ("dotDotDash", "doc23.uDotDotDash", "Dot dot dash"),
    ("wave", "doc23.uWave", "Wave"),
    ("wavyDouble", "doc23.uWavyDouble", "Double wave"),
];

/// Scale choices (percent).
const SCALES: [u32; 8] = [200, 150, 100, 90, 80, 66, 50, 33];

/// Ligatures (w14 value, key, English): the reference's five.
const LIGATURES: [(&str, &str, &str); 5] = [
    ("", "doc23.ligNone", "None"),
    ("standard", "doc23.ligStandard", "Standard Only"),
    (
        "standardContextual",
        "doc23.ligStdCtx",
        "Standard and Contextual",
    ),
    (
        "historicalDiscretional",
        "doc23.ligHistDisc",
        "Historical and Discretionary",
    ),
    ("all", "doc23.ligAll", "All"),
];

const NUM_SPACING: [(&str, &str, &str); 3] = [
    ("", "doc23.default", "Default"),
    ("proportional", "doc23.proportional", "Proportional"),
    ("tabular", "doc23.tabular", "Tabular"),
];

const NUM_FORMS: [(&str, &str, &str); 3] = [
    ("", "doc23.default", "Default"),
    ("lining", "doc23.lining", "Lining"),
    ("oldStyle", "doc23.oldStyle", "Old-style"),
];

/// Effects check boxes: (id, key, English).
const EFFECTS: [(&str, &str, &str); 7] = [
    ("strike", "doc23.strike", "Strikethrough"),
    ("dstrike", "doc23.dstrike", "Double strikethrough"),
    ("super", "doc23.superscript", "Superscript"),
    ("sub", "doc23.subscript", "Subscript"),
    ("smallCaps", "doc23.smallCaps", "Small caps"),
    ("allCaps", "doc23.allCaps", "All caps"),
    ("hidden", "doc23.hidden", "Hidden"),
];

/// Field names of the Field dialog (§17.16.5) with a short description of BlueOffice's own.
const FIELDS: [(&str, &str); 14] = [
    ("AUTHOR", "The document's author (document property)."),
    ("CREATEDATE", "The date the document was created."),
    ("DATE", "Today's date."),
    ("FILENAME", "The document's file name."),
    ("NUMPAGES", "The number of pages in the document."),
    ("NUMWORDS", "The number of words in the document."),
    ("PAGE", "The number of the page the field is on."),
    ("PRINTDATE", "The date the document was last printed."),
    ("SAVEDATE", "The date the document was last saved."),
    ("SECTION", "The number of the current section."),
    ("SECTIONPAGES", "The number of pages in the section."),
    ("SUBJECT", "The document's subject (document property)."),
    ("TIME", "The time now."),
    ("TITLE", "The document's title (document property)."),
];

/// Number styles of Define New Number Format: (numFmt, sample of 1).
pub(crate) const NUMBER_STYLES: [(&str, &str); 6] = [
    ("decimal", "1, 2, 3, ..."),
    ("upperRoman", "I, II, III, ..."),
    ("lowerRoman", "i, ii, iii, ..."),
    ("upperLetter", "A, B, C, ..."),
    ("lowerLetter", "a, b, c, ..."),
    ("decimalZero", "01, 02, 03, ..."),
];

/// The first number of a number style as the format box shows it.
fn first_of(fmt: &str) -> &'static str {
    match fmt {
        "upperRoman" => "I",
        "lowerRoman" => "i",
        "upperLetter" => "A",
        "lowerLetter" => "a",
        "decimalZero" => "01",
        _ => "1",
    }
}

fn text_cmd(format: CharFormat) -> Value {
    json!({ "cmd": "formatText", "format": serde_json::to_value(format).unwrap_or(Value::Null) })
}

fn pt_text(half: i32) -> String {
    let v = f64::from(half) / 2.0;
    if (v - v.round()).abs() < 1e-6 {
        format!("{} pt", v.round())
    } else {
        format!("{v:.1} pt")
    }
}

/// Points typed in a box ("1,5 pt", "2") in half-points or twips (`k` per point).
fn pt_of(text: &str, k: f32) -> Option<i32> {
    let v: f32 = text
        .trim()
        .trim_end_matches("pt")
        .trim()
        .replace(',', ".")
        .parse()
        .ok()?;
    Some((v * k).round() as i32)
}

/// Build round-23 dialog `kind` (`c.extra["r23"]` holds the selection's state).
#[allow(clippy::too_many_lines)]
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let x = &c.extra["r23"];
    match kind {
        DocFormKind::SetNumbering => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.setValueTitle", "Set Numbering Value"),
                (384.0, 266.0),
                vec![],
            );
            f.radio(
                "start",
                "how",
                [24.0, 54.0, 330.0, 20.0],
                &tr("doc23.startNew", "Start new list"),
                true,
            );
            f.radio(
                "continue",
                "how",
                [24.0, 82.0, 330.0, 20.0],
                &tr("doc23.continueList", "Continue from previous list"),
                false,
            );
            f.check(
                "advance",
                [48.0, 108.0, 300.0, 20.0],
                &tr("doc23.advance", "Advance value (skip numbers)"),
                false,
            );
            f.disable("advance");
            f.label(
                "valueL",
                [24.0, 144.0, 200.0, 18.0],
                &tr("doc23.setValueTo", "Set value to:"),
            );
            let v = x["value"].as_i64().unwrap_or(1) as i32;
            f.spin("value", [24.0, 166.0, 120.0, 26.0], v, 0, 32_767);
            f.label(
                "previewL",
                [170.0, 144.0, 190.0, 18.0],
                &tr("doc23.preview", "Preview:"),
            );
            f.canvas("numPreview", [170.0, 166.0, 120.0, 26.0]);
            f.ok_cancel(s);
            f
        }
        DocFormKind::DefineBullet => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.defineBulletTitle", "Define New Bullet"),
                (384.0, 360.0),
                vec![],
            );
            f.group(
                "charG",
                [16.0, 46.0, 352.0, 18.0],
                &tr("doc23.bulletChar", "Bullet character"),
            );
            f.label(
                "charL",
                [30.0, 76.0, 120.0, 18.0],
                &tr("doc23.character", "Character:"),
            );
            f.edit("char", [150.0, 72.0, 60.0, 26.0], "\u{2022}");
            f.button(
                "symbol",
                [224.0, 72.0, 112.0, 28.0],
                &tr("doc23.symbol", "Symbol..."),
                false,
            );
            f.label(
                "alignL",
                [30.0, 116.0, 120.0, 18.0],
                &tr("doc23.alignment", "Alignment:"),
            );
            f.combo(
                "align",
                [150.0, 112.0, 120.0, 26.0],
                vec![
                    tr("doc23.left", "Left"),
                    tr("doc23.centered", "Centered"),
                    tr("doc23.right", "Right"),
                ],
                0,
            );
            f.group(
                "previewG",
                [16.0, 150.0, 352.0, 18.0],
                &tr("doc23.previewG", "Preview"),
            );
            f.canvas("listPreview", [30.0, 176.0, 324.0, 120.0]);
            f.ok_cancel(s);
            f
        }
        DocFormKind::DefineNumber => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.defineNumberTitle", "Define New Number Format"),
                (384.0, 390.0),
                vec![],
            );
            f.group(
                "fmtG",
                [16.0, 46.0, 352.0, 18.0],
                &tr("doc23.numberFormat", "Number format"),
            );
            f.label(
                "styleL",
                [30.0, 74.0, 200.0, 18.0],
                &tr("doc23.numberStyle", "Number style:"),
            );
            f.combo(
                "style",
                [30.0, 94.0, 200.0, 26.0],
                NUMBER_STYLES.iter().map(|(_, l)| (*l).to_owned()).collect(),
                0,
            );
            f.label(
                "fmtL",
                [30.0, 130.0, 200.0, 18.0],
                &tr("doc23.numberFmt", "Number format:"),
            );
            f.edit("format", [30.0, 150.0, 200.0, 26.0], "1.");
            f.label(
                "alignL",
                [250.0, 74.0, 100.0, 18.0],
                &tr("doc23.alignment", "Alignment:"),
            );
            f.combo(
                "align",
                [250.0, 94.0, 104.0, 26.0],
                vec![
                    tr("doc23.left", "Left"),
                    tr("doc23.centered", "Centered"),
                    tr("doc23.right", "Right"),
                ],
                0,
            );
            f.group(
                "previewG",
                [16.0, 190.0, 352.0, 18.0],
                &tr("doc23.previewG", "Preview"),
            );
            f.canvas("listPreview", [30.0, 216.0, 324.0, 110.0]);
            f.ok_cancel(s);
            f
        }
        DocFormKind::DefineMultilevel => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.defineMultilevelTitle", "Define new Multilevel list"),
                (560.0, 420.0),
                vec![],
            );
            f.label(
                "levelL",
                [16.0, 46.0, 160.0, 18.0],
                &tr("doc23.clickLevel", "Click level to modify:"),
            );
            f.list(
                "level",
                [16.0, 68.0, 60.0, 190.0],
                (1..=9).map(|i| i.to_string()).collect(),
                Some(0),
            );
            f.canvas("mlPreview", [90.0, 68.0, 454.0, 190.0]);
            f.group(
                "fmtG",
                [16.0, 270.0, 528.0, 18.0],
                &tr("doc23.numberFormat", "Number format"),
            );
            f.label(
                "fmtL",
                [30.0, 296.0, 220.0, 18.0],
                &tr("doc23.enterFormat", "Enter formatting for number:"),
            );
            f.edit("format", [30.0, 316.0, 200.0, 26.0], "%1.");
            f.label(
                "styleL",
                [250.0, 296.0, 220.0, 18.0],
                &tr("doc23.styleForLevel", "Number style for this level:"),
            );
            f.combo(
                "style",
                [250.0, 316.0, 180.0, 26.0],
                NUMBER_STYLES.iter().map(|(_, l)| (*l).to_owned()).collect(),
                0,
            );
            f.label(
                "linkL",
                [30.0, 352.0, 200.0, 18.0],
                &tr("doc23.linkStyle", "Link level to style:"),
            );
            let mut link = vec![tr("doc23.noStyle", "(no style)")];
            link.extend((1..=9).map(|i| format!("Heading {i}")));
            f.combo("link", [200.0, 348.0, 180.0, 26.0], link, 0);
            // Nine levels: format text | numFmt index | linked heading (0 none).
            f.preview = (0..9)
                .map(|i| {
                    let text = (1..=i + 1).fold(String::new(), |mut t, n| {
                        t.push_str(&format!("%{n}."));
                        t
                    });
                    format!("{text}|0|0")
                })
                .collect();
            f.set_text("format", "%1.");
            f.ok_cancel(s);
            f
        }
        DocFormKind::SortText => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.sortTitle", "Sort Text"),
                (520.0, 330.0),
                vec![],
            );
            f.label(
                "byL",
                [16.0, 50.0, 100.0, 18.0],
                &tr("doc23.sortBy", "Sort by"),
            );
            f.combo(
                "by",
                [16.0, 72.0, 180.0, 26.0],
                vec![tr("doc23.paragraphs", "Paragraphs")],
                0,
            );
            f.label(
                "typeL",
                [214.0, 76.0, 50.0, 18.0],
                &tr("doc23.type", "Type:"),
            );
            f.combo(
                "type",
                [262.0, 72.0, 110.0, 26.0],
                vec![tr("doc23.text", "Text")],
                0,
            );
            f.radio(
                "asc",
                "order",
                [392.0, 64.0, 120.0, 20.0],
                &tr("doc23.ascending", "Ascending"),
                true,
            );
            f.radio(
                "desc",
                "order",
                [392.0, 88.0, 120.0, 20.0],
                &tr("doc23.descending", "Descending"),
                false,
            );
            f.group(
                "listG",
                [16.0, 136.0, 488.0, 18.0],
                &tr("doc23.myList", "My list has"),
            );
            f.radio(
                "header",
                "head",
                [30.0, 164.0, 140.0, 20.0],
                &tr("doc23.headerRow", "Header row"),
                false,
            );
            f.radio(
                "noHeader",
                "head",
                [190.0, 164.0, 160.0, 20.0],
                &tr("doc23.noHeaderRow", "No header row"),
                true,
            );
            f.ok_cancel(s);
            f
        }
        DocFormKind::Field23 => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.fieldTitle", "Field"),
                (640.0, 430.0),
                vec![],
            );
            f.group(
                "chooseG",
                [16.0, 46.0, 280.0, 18.0],
                &tr("doc23.chooseField", "Please choose a field"),
            );
            f.label(
                "catL",
                [30.0, 72.0, 120.0, 18.0],
                &tr("doc23.categories", "Categories:"),
            );
            f.combo(
                "cat",
                [30.0, 92.0, 250.0, 26.0],
                vec![tr("doc23.allCats", "(All)")],
                0,
            );
            f.label(
                "namesL",
                [30.0, 128.0, 120.0, 18.0],
                &tr("doc23.fieldNames", "Field names:"),
            );
            f.list(
                "names",
                [30.0, 148.0, 250.0, 200.0],
                FIELDS.iter().map(|(n, _)| (*n).to_owned()).collect(),
                Some(0),
            );
            f.group(
                "propsG",
                [310.0, 46.0, 314.0, 18.0],
                &tr("doc23.fieldCodes", "Field codes"),
            );
            // Descriptions in the interface language, looked up when the name changes.
            f.preview = FIELDS
                .iter()
                .map(|(n, d)| tr(&format!("doc23.fd{n}"), d))
                .collect();
            f.label("descL", [324.0, 72.0, 290.0, 40.0], &f.preview[0].clone());
            f.label(
                "codeL",
                [324.0, 128.0, 290.0, 18.0],
                &tr("doc23.fieldCode", "Field code:"),
            );
            f.edit("code", [324.0, 148.0, 290.0, 26.0], FIELDS[0].0);
            f.check(
                "keep",
                [324.0, 190.0, 290.0, 20.0],
                &tr("doc23.preserve", "Preserve formatting during updates"),
                true,
            );
            f.ok_cancel(s);
            f
        }
        DocFormKind::SelPane23 => {
            let mut f = DocForm::new(
                kind,
                &tr("doc23.selPaneTitle", "Selection"),
                (360.0, 420.0),
                vec![],
            );
            f.label(
                "head",
                [16.0, 46.0, 330.0, 18.0],
                &tr("doc23.shapesOnPage", "Shapes on This Page"),
            );
            let objs = x["objects"].as_array().cloned().unwrap_or_default();
            let labels: Vec<String> = objs
                .iter()
                .map(|o| o[0].as_str().unwrap_or("").to_owned())
                .collect();
            f.ids = objs
                .iter()
                .map(|o| o[1].as_str().unwrap_or("").to_owned())
                .collect();
            let n = labels.len();
            f.list(
                "objects",
                [16.0, 70.0, 328.0, 280.0],
                labels,
                (n > 0).then_some(0),
            );
            f.button(
                "ok",
                [114.0, 370.0, 112.0, 28.0],
                &tr("doc23.selectObj", "Select"),
                true,
            );
            f.button(
                "cancel",
                [232.0, 370.0, 112.0, 28.0],
                &tr("docR3.close", "Close"),
                false,
            );
            if n == 0 {
                f.disable("ok");
            }
            f
        }
        _ => open_font(kind, s, c),
    }
}

#[allow(clippy::too_many_lines)]
fn open_font(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let x = &c.extra["r23"];
    let mut f = DocForm::new(
        kind,
        &tr("doc23.fontTitle", "Font"),
        (584.0, 657.0),
        vec![
            tr("doc23.tabFont", "Font"),
            tr("doc23.tabAdvanced", "Advanced"),
        ],
    );
    let font = x["font"].as_str().unwrap_or("").to_owned();
    f.label_on(
        0,
        "fontL",
        [49.0, 113.0, 200.0, 18.0],
        &tr("doc23.fontL", "Font:"),
    );
    f.edit_on(0, "font", [49.0, 134.0, 242.0, 24.0], &font);
    let mut fonts = c.fonts.clone();
    fonts.sort_by_key(|n| n.to_lowercase());
    fonts.dedup();
    fonts.retain(|n| !n.starts_with('.') && !n.starts_with('@'));
    fonts.splice(0..0, ["+Body".to_owned(), "+Headings".to_owned()]);
    let at = fonts.iter().position(|n| *n == font);
    f.form.add(
        "fontList",
        Some(0),
        [49.0, 160.0, 242.0, 88.0],
        Kind::List {
            items: fonts,
            sel: at,
            top: at.unwrap_or(1).saturating_sub(1),
        },
    );
    let (bold, italic) = (x["bold"] == true, x["italic"] == true);
    let style_i = usize::from(italic) + 2 * usize::from(bold);
    let styles: Vec<String> = STYLES.iter().map(|(k, fb)| tr(k, fb)).collect();
    f.label_on(
        0,
        "fstyleL",
        [305.0, 113.0, 100.0, 18.0],
        &tr("doc23.fontStyleL", "Font style:"),
    );
    f.edit_on(0, "fstyle", [305.0, 134.0, 100.0, 24.0], &styles[style_i]);
    f.form.add(
        "styleList",
        Some(0),
        [305.0, 160.0, 100.0, 88.0],
        Kind::List {
            items: styles,
            sel: Some(style_i),
            top: 0,
        },
    );
    let size = x["size"].as_i64().map_or(String::new(), |h| {
        let h = h as i32;
        if h % 2 == 0 {
            (h / 2).to_string()
        } else {
            format!("{}.5", h / 2)
        }
    });
    f.label_on(
        0,
        "sizeL",
        [419.0, 113.0, 89.0, 18.0],
        &tr("doc23.sizeL", "Size:"),
    );
    f.edit_on(0, "size", [419.0, 134.0, 89.0, 24.0], &size);
    let sizes: Vec<String> = [8, 9, 10, 11, 12, 14, 16, 18, 20, 22, 24, 26, 28, 36, 48, 72]
        .iter()
        .map(ToString::to_string)
        .collect();
    let si = sizes.iter().position(|v| *v == size);
    f.form.add(
        "sizeList",
        Some(0),
        [419.0, 160.0, 89.0, 88.0],
        Kind::List {
            items: sizes,
            sel: si,
            top: si.unwrap_or(0).saturating_sub(1),
        },
    );
    f.label_on(
        0,
        "colorL",
        [49.0, 261.0, 150.0, 18.0],
        &tr("doc23.fontColorL", "Font color:"),
    );
    let mut colors = vec![tr("doc23.automatic", "Automatic")];
    colors.extend(COLORS.iter().map(|(_, k, fb)| tr(k, fb)));
    let cur = x["color"].as_str().unwrap_or("");
    let ci = COLORS
        .iter()
        .position(|c| c.0.eq_ignore_ascii_case(cur))
        .map_or(0, |i| i + 1);
    f.form.add(
        "color",
        Some(0),
        [49.0, 280.0, 144.0, 26.0],
        Kind::Combo {
            items: colors,
            sel: ci,
        },
    );
    f.label_on(
        0,
        "ulL",
        [207.0, 261.0, 150.0, 18.0],
        &tr("doc23.ulStyleL", "Underline style:"),
    );
    let ul = x["underline"].as_str().unwrap_or("");
    let ui = UNDERLINES.iter().position(|u| u.0 == ul).unwrap_or(2);
    f.form.add(
        "underline",
        Some(0),
        [207.0, 280.0, 144.0, 26.0],
        Kind::Combo {
            items: UNDERLINES.iter().map(|(_, k, fb)| tr(k, fb)).collect(),
            sel: ui,
        },
    );
    f.label_on(
        0,
        "ulcL",
        [365.0, 261.0, 150.0, 18.0],
        &tr("doc23.ulColorL", "Underline color:"),
    );
    f.form.add(
        "ulColor",
        Some(0),
        [365.0, 280.0, 143.0, 26.0],
        Kind::Combo {
            items: vec![tr("doc23.automatic", "Automatic")],
            sel: 0,
        },
    );
    f.group_page(
        0,
        "effects",
        [24.0, 314.0, 544.0, 18.0],
        &tr("doc23.effects", "Effects"),
    );
    for (i, (id, k, fb)) in EFFECTS.iter().enumerate() {
        let (cx, cy) = if i < 4 {
            (38.0, 341.0 + 25.0 * i as f32)
        } else {
            (396.0, 341.0 + 25.0 * (i - 4) as f32)
        };
        f.check_on(0, id, [cx, cy, 200.0, 20.0], &tr(k, fb), x[*id] == true);
    }
    f.group_page(
        0,
        "previewL",
        [24.0, 453.0, 544.0, 18.0],
        &tr("doc23.previewG", "Preview"),
    );
    f.form
        .add("prev23", Some(0), [38.0, 479.0, 522.0, 52.0], Kind::Canvas);
    f.label_on(
        0,
        "note",
        [24.0, 545.0, 544.0, 36.0],
        &tr(
            "doc23.fontNote",
            "The same font is used on screen and when printing.",
        ),
    );
    // Advanced: Character Spacing.
    f.group_page(
        1,
        "spacingG",
        [24.0, 89.0, 544.0, 18.0],
        &tr("doc23.charSpacing", "Character Spacing"),
    );
    f.label_on(
        1,
        "scaleL",
        [38.0, 110.0, 60.0, 18.0],
        &tr("doc23.scale", "Scale:"),
    );
    let scale = x["scale"].as_u64().unwrap_or(100) as u32;
    let mut scales: Vec<String> = SCALES.iter().map(|v| format!("{v}%")).collect();
    let sc = SCALES.iter().position(|v| *v == scale).unwrap_or_else(|| {
        scales.push(format!("{scale}%"));
        scales.len() - 1
    });
    f.form.add(
        "scale",
        Some(1),
        [101.0, 106.0, 223.0, 25.0],
        Kind::Combo {
            items: scales,
            sel: sc,
        },
    );
    let spacing = x["spacing"].as_i64().unwrap_or(0) as i32;
    f.label_on(
        1,
        "spacingL",
        [38.0, 141.0, 60.0, 18.0],
        &tr("doc23.spacing", "Spacing:"),
    );
    f.form.add(
        "spacing",
        Some(1),
        [101.0, 137.0, 223.0, 25.0],
        Kind::Combo {
            items: vec![
                tr("doc23.normal", "Normal"),
                tr("doc23.expanded", "Expanded"),
                tr("doc23.condensed", "Condensed"),
            ],
            sel: match spacing.signum() {
                1 => 1,
                -1 => 2,
                _ => 0,
            },
        },
    );
    f.label_on(
        1,
        "spByL",
        [338.0, 141.0, 30.0, 18.0],
        &tr("doc23.by", "By:"),
    );
    f.edit_on(
        1,
        "spBy",
        [369.0, 137.0, 110.0, 25.0],
        &if spacing == 0 {
            String::new()
        } else {
            pt_text(spacing.abs() / 10)
        },
    );
    let position = x["position"].as_i64().unwrap_or(0) as i32;
    f.label_on(
        1,
        "posL",
        [38.0, 173.0, 60.0, 18.0],
        &tr("doc23.position", "Position:"),
    );
    f.form.add(
        "position",
        Some(1),
        [101.0, 169.0, 93.0, 25.0],
        Kind::Combo {
            items: vec![
                tr("doc23.normal", "Normal"),
                tr("doc23.raised", "Raised"),
                tr("doc23.lowered", "Lowered"),
            ],
            sel: match position.signum() {
                1 => 1,
                -1 => 2,
                _ => 0,
            },
        },
    );
    f.label_on(
        1,
        "posByL",
        [338.0, 173.0, 30.0, 18.0],
        &tr("doc23.by", "By:"),
    );
    f.edit_on(
        1,
        "posBy",
        [369.0, 169.0, 110.0, 25.0],
        &if position == 0 {
            String::new()
        } else {
            pt_text(position.abs())
        },
    );
    let kern = x["kern"].as_i64();
    f.check_on(
        1,
        "kern",
        [38.0, 207.0, 170.0, 20.0],
        &tr("doc23.kerning", "Kerning for fonts:"),
        kern.is_some(),
    );
    f.edit_on(
        1,
        "kernAt",
        [212.0, 204.0, 60.0, 25.0],
        &kern.map_or(String::new(), |k| pt_text(k as i32)),
    );
    f.label_on(
        1,
        "kernL",
        [280.0, 208.0, 200.0, 18.0],
        &tr("doc23.pointsAbove", "Points and above"),
    );
    // OpenType Features.
    f.group_page(
        1,
        "otG",
        [24.0, 244.0, 544.0, 18.0],
        &tr("doc23.openType", "OpenType Features"),
    );
    let combo = |f: &mut DocForm,
                 id: &'static str,
                 y: f32,
                 label: String,
                 items: Vec<String>,
                 sel: usize| {
        f.form.add(
            match id {
                "lig" => "ligL",
                "numSp" => "numSpL",
                "numForm" => "numFormL",
                _ => "setsL",
            },
            Some(1),
            [38.0, y + 4.0, 110.0, 18.0],
            Kind::Label(label),
        );
        f.form.add(
            id,
            Some(1),
            [151.0, y, 223.0, 25.0],
            Kind::Combo { items, sel },
        );
    };
    let lig = x["ligatures"].as_str().unwrap_or("");
    combo(
        &mut f,
        "lig",
        270.0,
        tr("doc23.ligatures", "Ligatures:"),
        LIGATURES.iter().map(|(_, k, fb)| tr(k, fb)).collect(),
        LIGATURES.iter().position(|l| l.0 == lig).unwrap_or(0),
    );
    let nsp = x["numSpacing"].as_str().unwrap_or("");
    combo(
        &mut f,
        "numSp",
        302.0,
        tr("doc23.numSpacing", "Number spacing:"),
        NUM_SPACING.iter().map(|(_, k, fb)| tr(k, fb)).collect(),
        NUM_SPACING.iter().position(|l| l.0 == nsp).unwrap_or(0),
    );
    let nfm = x["numForm"].as_str().unwrap_or("");
    combo(
        &mut f,
        "numForm",
        334.0,
        tr("doc23.numForms", "Number forms:"),
        NUM_FORMS.iter().map(|(_, k, fb)| tr(k, fb)).collect(),
        NUM_FORMS.iter().position(|l| l.0 == nfm).unwrap_or(0),
    );
    let mut sets = vec![tr("doc23.default", "Default")];
    sets.extend((1..=20).map(|i| i.to_string()));
    combo(
        &mut f,
        "sets",
        366.0,
        tr("doc23.stylisticSets", "Stylistic sets:"),
        sets,
        x["stylisticSet"].as_u64().unwrap_or(0).min(20) as usize,
    );
    f.check_on(
        1,
        "ctxAlts",
        [38.0, 402.0, 300.0, 20.0],
        &tr("doc23.ctxAlts", "Use Contextual Alternates"),
        x["contextualAlternates"] == true,
    );
    f.group_page(
        1,
        "previewL2",
        [24.0, 442.0, 544.0, 18.0],
        &tr("doc23.previewG", "Preview"),
    );
    f.form
        .add("prev23b", Some(1), [38.0, 468.0, 522.0, 52.0], Kind::Canvas);
    f.button(
        "setDefault",
        [10.0, 617.0, 112.0, 28.0],
        &tr("doc23.setDefault", "Set As Default"),
        false,
    );
    f.button(
        "textEffects",
        [132.0, 617.0, 112.0, 28.0],
        &tr("doc23.textEffects", "Text Effects..."),
        false,
    );
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
    // The state the dialog opened with (OK changes only what differs); the preview shows
    // the selected text as the reference.
    f.words = vec![("r23".to_owned(), x.to_string())];
    f.preview = vec![c
        .selection
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(60)
        .collect()];
    f
}

impl DocForm {
    fn group_page(&mut self, page: usize, id: &'static str, r: [f32; 4], text: &str) {
        self.form
            .add(id, Some(page), r, Kind::Group(text.to_owned()));
    }

    /// The selection's state the Font dialog opened with.
    fn r23_initial(&self) -> Value {
        self.words
            .iter()
            .find(|(k, _)| k == "r23")
            .and_then(|(_, v)| serde_json::from_str(v).ok())
            .unwrap_or(Value::Null)
    }

    /// A round-23 list or box changed: the lists fill the boxes above them, a multilevel
    /// list's level shows its settings.
    pub(crate) fn r23_changed(&mut self, id: &str) {
        for (list, edit) in [
            ("fontList", "font"),
            ("styleList", "fstyle"),
            ("sizeList", "size"),
        ] {
            if id == list {
                if let Some(i) = self.sel(list) {
                    let v = self
                        .form
                        .get(list)
                        .and_then(|c| match &c.kind {
                            Kind::List { items, .. } => items.get(i).cloned(),
                            _ => None,
                        })
                        .unwrap_or_default();
                    self.form.set_text(edit, &v);
                }
            }
        }
        if self.kind == DocFormKind::Field23 && id == "names" {
            let i = self.sel("names").unwrap_or(0).min(FIELDS.len() - 1);
            let desc = self.preview.get(i).cloned().unwrap_or_default();
            self.form.set_text("code", FIELDS[i].0);
            self.set_label("descL", &desc);
        }
        if self.kind == DocFormKind::DefineNumber && id == "style" {
            let fmt = NUMBER_STYLES[self.sel("style").unwrap_or(0).min(5)].0;
            let old = self.text("format");
            let rest: String = old
                .chars()
                .skip_while(char::is_ascii_alphanumeric)
                .collect();
            self.form
                .set_text("format", &format!("{}{rest}", first_of(fmt)));
        }
        if self.kind == DocFormKind::DefineMultilevel {
            self.r23_ml_store();
            if id == "level" {
                let l = self.sel("level").unwrap_or(0).min(8);
                let row = self.preview.get(l).cloned().unwrap_or_default();
                let mut parts = row.split('|');
                let text = parts.next().unwrap_or("").to_owned();
                let st: usize = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                let link: usize = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                self.form.set_text("format", &text);
                self.set_sel("style", Some(st));
                self.set_sel("link", Some(link));
                self.label_w = vec![l as f32];
            }
        }
    }

    /// Multilevel: keep the shown level's settings.
    fn r23_ml_store(&mut self) {
        let l = self.label_w.first().map_or(0, |v| *v as usize).min(8);
        let row = format!(
            "{}|{}|{}",
            self.text("format"),
            self.sel("style").unwrap_or(0),
            self.sel("link").unwrap_or(0)
        );
        if let Some(r) = self.preview.get_mut(l) {
            *r = row;
        }
    }

    pub(super) fn r23_clicked(&mut self, id: &str) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        Some(match (self.kind, id) {
            (DocFormKind::Font23, "textEffects") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "format.textEffects" })],
                true,
            ),
            (DocFormKind::Font23, "setDefault") => match self.r23_font_ok() {
                FormOut::Run(mut cmds, _) => {
                    cmds.push(json!({ "cmd": "ui.action", "id": "r23.fontDefault" }));
                    FormOut::Run(cmds, true)
                }
                other => other,
            },
            (DocFormKind::DefineBullet, "symbol") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "insert.symbol" })],
                true,
            ),
            _ => FormOut::Redraw,
        })
    }

    pub(super) fn r23_ok(&mut self) -> Option<FormOut> {
        Some(match self.kind {
            DocFormKind::Font23 => self.r23_font_ok(),
            DocFormKind::SetNumbering => {
                let numbering = if self.chosen("how") == Some("continue") {
                    ListNumbering::Continue
                } else {
                    ListNumbering::Restart {
                        value: self.form.num("value"),
                    }
                };
                FormOut::Run(
                    vec![json!({ "cmd": "setListNumbering",
                        "numbering": serde_json::to_value(numbering).unwrap_or(Value::Null) })],
                    true,
                )
            }
            DocFormKind::DefineBullet => {
                let ch: String = self.text("char").chars().take(1).collect();
                let ch = if ch.trim().is_empty() {
                    "\u{2022}".to_owned()
                } else {
                    ch
                };
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r23.bullet:{ch}") })],
                    true,
                )
            }
            DocFormKind::DefineNumber => {
                let fmt = NUMBER_STYLES[self.sel("style").unwrap_or(0).min(5)].0;
                let typed = self.text("format");
                let first = first_of(fmt);
                let text = if typed.contains(first) {
                    typed.replacen(first, "%1", 1)
                } else {
                    format!("%1{typed}")
                };
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action",
                        "id": format!("r23.numfmt:{fmt}|{text}") })],
                    true,
                )
            }
            DocFormKind::DefineMultilevel => {
                self.r23_ml_store();
                let levels: Vec<Value> = self
                    .preview
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        let mut p = row.split('|');
                        let text = p.next().unwrap_or("").to_owned();
                        let st: usize = p.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        let link: usize = p.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        json!({
                            "format": NUMBER_STYLES[st.min(5)].0,
                            "text": text,
                            "indent": 360 * (i + 1),
                            "hanging": 360,
                            "style": (link > 0).then(|| format!("Heading{link}")),
                        })
                    })
                    .collect();
                FormOut::Run(
                    vec![json!({ "cmd": "defineList", "levels": levels, "apply": true })],
                    true,
                )
            }
            DocFormKind::SelPane23 => match self.sel("objects").and_then(|i| self.ids.get(i)) {
                Some(at) => FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r23.selObj:{at}") })],
                    true,
                ),
                None => FormOut::Close,
            },
            DocFormKind::Field23 => {
                let mut instr = self.text("code").trim().to_owned();
                if instr.is_empty() {
                    return Some(FormOut::Redraw);
                }
                if self.on("keep") && !instr.contains("MERGEFORMAT") {
                    instr.push_str(" \\* MERGEFORMAT");
                }
                FormOut::Run(vec![json!({ "cmd": "insertField", "instr": instr })], true)
            }
            DocFormKind::SortText => {
                let descending = self.chosen("order") == Some("desc");
                let header = self.chosen("head") == Some("header");
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action",
                        "id": format!("r23.sort:{}{}",
                            if descending { "desc" } else { "asc" },
                            if header { ":header" } else { "" }) })],
                    true,
                )
            }
            _ => return None,
        })
    }

    /// Font dialog OK: the commands that change what differs from the opening state.
    #[allow(clippy::too_many_lines)]
    fn r23_font_ok(&self) -> FormOut {
        let x = self.r23_initial();
        let mut cmds = Vec::new();
        let font = self.text("font");
        let font = font.trim();
        if !font.is_empty() && x["font"].as_str() != Some(font) {
            cmds.push(text_cmd(CharFormat::Font {
                name: font.to_owned(),
            }));
        }
        if let Some(half) = pt_of(&self.text("size"), 2.0) {
            if half > 0 && x["size"].as_i64() != Some(i64::from(half)) {
                cmds.push(text_cmd(CharFormat::Size {
                    half_points: half.unsigned_abs().min(3276),
                }));
            }
        }
        let style = self.text("fstyle");
        let names: Vec<String> = (0..4)
            .map(|i| {
                self.form
                    .get("styleList")
                    .and_then(|c| match &c.kind {
                        Kind::List { items, .. } => items.get(i).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default()
            })
            .collect();
        if let Some(i) = names.iter().position(|n| *n == style.trim()) {
            let (b, it) = (i >= 2, i % 2 == 1);
            if b != (x["bold"] == true) {
                cmds.push(text_cmd(CharFormat::Bold));
            }
            if it != (x["italic"] == true) {
                cmds.push(text_cmd(CharFormat::Italic));
            }
        }
        let ci = self.sel("color").unwrap_or(0);
        let hex = (ci > 0).then(|| COLORS[(ci - 1).min(COLORS.len() - 1)].0.to_owned());
        let old = x["color"].as_str().map(str::to_uppercase);
        if hex != old && !(hex.is_none() && x["color"].is_null()) {
            cmds.push(text_cmd(CharFormat::Color { hex }));
        }
        let mut patch = serde_json::Map::new();
        let ul = UNDERLINES[self.sel("underline").unwrap_or(0).min(UNDERLINES.len() - 1)].0;
        if x["underline"].as_str().unwrap_or("") != ul {
            patch.insert("underline".into(), json!(ul));
        }
        // Toggles of the effects that differ.
        for (id, fmt) in [
            ("strike", Some(CharFormat::Strike)),
            ("super", Some(CharFormat::Superscript)),
            ("sub", Some(CharFormat::Subscript)),
            ("smallCaps", Some(CharFormat::SmallCaps)),
            ("allCaps", Some(CharFormat::AllCaps)),
            ("dstrike", None),
            ("hidden", None),
        ] {
            let on = self.on(id);
            if on == (x[id] == true) {
                continue;
            }
            match fmt {
                Some(f) => cmds.push(text_cmd(f)),
                None if id == "dstrike" => {
                    patch.insert("doubleStrike".into(), json!(on));
                }
                None => {
                    patch.insert("hidden".into(), json!(on));
                }
            }
        }
        // Advanced.
        let scale = self
            .sel("scale")
            .and_then(|i| SCALES.get(i).copied())
            .unwrap_or_else(|| x["scale"].as_u64().unwrap_or(100) as u32);
        if x["scale"].as_u64().unwrap_or(100) as u32 != scale {
            patch.insert("scale".into(), json!(scale));
        }
        let by = pt_of(&self.text("spBy"), 20.0).unwrap_or(20).abs();
        let spacing = match self.sel("spacing") {
            Some(1) => by,
            Some(2) => -by,
            _ => 0,
        };
        if x["spacing"].as_i64().unwrap_or(0) as i32 != spacing {
            patch.insert("spacing".into(), json!(spacing));
        }
        let by = pt_of(&self.text("posBy"), 2.0).unwrap_or(6).abs();
        let position = match self.sel("position") {
            Some(1) => by,
            Some(2) => -by,
            _ => 0,
        };
        if x["position"].as_i64().unwrap_or(0) as i32 != position {
            patch.insert("position".into(), json!(position));
        }
        let kern = self
            .on("kern")
            .then(|| pt_of(&self.text("kernAt"), 2.0).unwrap_or(2).max(1));
        if x["kern"].as_i64().map(|k| k as i32) != kern {
            patch.insert("kern".into(), json!(kern));
        }
        let pick = |id: &str, list: &[(&str, &str, &str)]| {
            list[self.sel(id).unwrap_or(0).min(list.len() - 1)]
                .0
                .to_owned()
        };
        for (id, key, list) in [
            ("lig", "ligatures", &LIGATURES[..]),
            ("numSp", "numSpacing", &NUM_SPACING[..]),
            ("numForm", "numForm", &NUM_FORMS[..]),
        ] {
            let v = pick(id, list);
            if x[key].as_str().unwrap_or("") != v {
                patch.insert(key.into(), json!(v));
            }
        }
        let sets = self.sel("sets").unwrap_or(0) as u64;
        if x["stylisticSet"].as_u64().unwrap_or(0) != sets {
            patch.insert("stylisticSet".into(), json!(sets));
        }
        let alts = self.on("ctxAlts");
        if (x["contextualAlternates"] == true) != alts {
            patch.insert("contextualAlternates".into(), json!(alts));
        }
        if !patch.is_empty() {
            cmds.push(json!({ "cmd": "formatText",
                "format": { "kind": "advanced", "patch": Value::Object(patch) } }));
        }
        FormOut::Run(cmds, true)
    }
}

/// Paint a round-23 canvas: the Font dialog's previews, the list previews.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
) -> bool {
    let frame = |p: &mut dyn Painter| {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x82, 0x82, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
    };
    let ink = Rgba(0, 0, 0, 255);
    match id {
        "prev23" | "prev23b" => {
            frame(p);
            let family = f.text("font");
            let family = if family.trim().is_empty() || family.starts_with('+') {
                font.family.to_owned()
            } else {
                family.trim().to_owned()
            };
            let style = f.text("fstyle");
            let names: Vec<String> = STYLES.iter().map(|(_, e)| (*e).to_owned()).collect();
            let i = names.iter().position(|n| *n == style.trim());
            let size = pt_of(&f.text("size"), 1.0).unwrap_or(11).clamp(6, 28) as f32;
            let scale = r[3] / 52.0;
            let sample_font = Font {
                family: &family,
                size: size * 1.33 * scale,
                weight: if i.is_some_and(|i| i >= 2) || style.contains("Bold") {
                    700
                } else {
                    400
                },
                italic: i.is_some_and(|i| i % 2 == 1) || style.contains("Italic"),
                underline: f.sel("underline").unwrap_or(0) > 0,
            };
            let ci = f.sel("color").unwrap_or(0);
            let color = if ci == 0 {
                ink
            } else {
                let h = COLORS[(ci - 1).min(COLORS.len() - 1)].0;
                let v = u32::from_str_radix(h, 16).unwrap_or(0);
                Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
            };
            let mut sample = match f.preview.first().filter(|s| !s.trim().is_empty()) {
                Some(s) => s.clone(),
                None if family.is_empty() => "AaBbCcYyZz".to_owned(),
                None => family.clone(),
            };
            if f.on("allCaps") {
                sample = sample.to_uppercase();
            }
            let y = r[1] + r[3] * 0.62;
            p.fill_rect(r[0] + 2.0 * scale, y, 100.0 * scale, 1.0, ink);
            p.fill_rect(r[0] + r[2] - 102.0 * scale, y, 100.0 * scale, 1.0, ink);
            p.text_font(
                &sample,
                r[0] + r[2] / 2.0 - 60.0 * scale,
                r[1] + r[3] * 0.62 - sample_font.size,
                &sample_font,
                color,
            );
            if f.on("strike") || f.on("dstrike") {
                let sy = r[1] + r[3] * 0.62 - sample_font.size * 0.35;
                p.fill_rect(
                    r[0] + r[2] / 2.0 - 60.0 * scale,
                    sy,
                    120.0 * scale,
                    1.0,
                    color,
                );
                if f.on("dstrike") {
                    p.fill_rect(
                        r[0] + r[2] / 2.0 - 60.0 * scale,
                        sy + 3.0 * scale,
                        120.0 * scale,
                        1.0,
                        color,
                    );
                }
            }
            true
        }
        "listPreview" | "mlPreview" | "numPreview" => {
            frame(p);
            let grey = Rgba(0xb0, 0xb0, 0xb0, 255);
            let k = font.size / 12.0;
            let markers: Vec<String> = match (f.kind, id) {
                (DocFormKind::DefineBullet, _) => {
                    let ch: String = f.text("char").chars().take(1).collect();
                    vec![ch.clone(), ch.clone(), ch]
                }
                (DocFormKind::DefineNumber, _) => {
                    let typed = f.text("format");
                    let fmt = NUMBER_STYLES[f.sel("style").unwrap_or(0).min(5)].0;
                    let first = first_of(fmt);
                    let seq: [&str; 3] = match fmt {
                        "upperRoman" => ["I", "II", "III"],
                        "lowerRoman" => ["i", "ii", "iii"],
                        "upperLetter" => ["A", "B", "C"],
                        "lowerLetter" => ["a", "b", "c"],
                        "decimalZero" => ["01", "02", "03"],
                        _ => ["1", "2", "3"],
                    };
                    seq.iter().map(|n| typed.replacen(first, n, 1)).collect()
                }
                (_, "numPreview") => vec![f.form.num("value").to_string()],
                _ => f
                    .preview
                    .iter()
                    .take(9)
                    .map(|row| {
                        row.split('|')
                            .next()
                            .unwrap_or("")
                            .replace("%1", "1")
                            .replace("%2", "1")
                            .replace("%3", "1")
                            .replace("%4", "1")
                            .replace("%5", "1")
                            .replace("%6", "1")
                            .replace("%7", "1")
                            .replace("%8", "1")
                            .replace("%9", "1")
                    })
                    .collect(),
            };
            let n = markers.len().max(1);
            let pitch = ((r[3] - 8.0 * k) / n as f32).min(30.0 * k);
            for (i, m) in markers.iter().enumerate() {
                let y = r[1] + 4.0 * k + pitch * i as f32;
                let indent = if id == "mlPreview" {
                    14.0 * k * i as f32
                } else {
                    0.0
                };
                p.text_font(m, r[0] + 10.0 * k + indent, y, font, ink);
                let lx = r[0] + 50.0 * k + indent;
                if lx < r[0] + r[2] - 12.0 * k && id != "numPreview" {
                    p.fill_rect(
                        lx,
                        y + font.size * 0.6,
                        r[0] + r[2] - 12.0 * k - lx,
                        2.0,
                        grey,
                    );
                }
            }
            true
        }
        _ => false,
    }
}

/// The round-23 state a dialog of `kind` opens with (`FormContext.extra["r23"]`).
pub(crate) fn extra(f: &bo_edit::FormatState) -> Value {
    let a = &f.advanced;
    json!({
        "font": f.font,
        "size": f.size_half_points,
        "bold": f.bold,
        "italic": f.italic,
        "color": f.color,
        "underline": if a.underline.is_empty() && f.underline { "single" } else { a.underline.as_str() },
        "strike": f.strike,
        "dstrike": a.double_strike,
        "super": f.superscript,
        "sub": f.subscript,
        "hidden": a.hidden,
        "scale": a.scale,
        "spacing": a.spacing,
        "position": a.position,
        "kern": a.kern,
        "ligatures": a.ligatures,
        "numSpacing": a.num_spacing,
        "numForm": a.num_form,
        "stylisticSet": a.stylistic_set,
        "contextualAlternates": a.contextual_alternates,
        "smallCaps": a.small_caps,
        "allCaps": a.all_caps,
    })
}
