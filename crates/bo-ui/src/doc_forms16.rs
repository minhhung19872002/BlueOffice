//! BlueDoc dialogs of parity round 16 (docs/ui/REPORT.md "Vòng 106"), laid out from the
//! reference word processor's dialogs captured at 125 % (window less its 8 px invisible
//! border): **Track Changes Options** (Review › Markup launcher: the marks shown, what the
//! balloons show, the Reviewing Pane, Advanced Options… and Change User Name…) and
//! **Advanced Track Changes Options** (how insertions and deletions are marked and their
//! colour, changed lines, comment colour, balloon width and connecting lines). Every control
//! changes the review display (`setMarkup`); nothing in the document changes. A child module
//! of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use serde_json::{json, Value};

/// Whether `kind` is one of this module's dialogs.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::TrackOptions
            | DocFormKind::AdvTrackOptions
            | DocFormKind::MergeNew
            | DocFormKind::MergePrint
            | DocFormKind::CheckErrors
            | DocFormKind::MergeReport
            | DocFormKind::NewList
            | DocFormKind::MergeEmail
            | DocFormKind::NlFind
            | DocFormKind::NlColumns
    )
}

/// Balloons in All Markup view show: (label key, English, deletionBalloons, inline).
const BALLOONS: [(&str, &str, bool, bool); 3] = [
    ("doc16.showRevisions", "Revisions", true, false),
    ("doc16.showNothing", "Nothing", false, true),
    (
        "doc16.showCommentsFormatting",
        "Comments and formatting",
        false,
        false,
    ),
];

/// Reviewing Pane: (value, label key, English).
const PANES: [(&str, &str, &str); 3] = [
    ("off", "doc16.paneOff", "Off"),
    ("vertical", "doc16.paneVertical", "Vertical"),
    ("horizontal", "doc16.paneHorizontal", "Horizontal"),
];

/// Insertions: (value, key, English).
const INS: [(&str, &str, &str); 7] = [
    ("none", "doc16.markNone", "(none)"),
    ("colorOnly", "doc16.colorOnly", "Color only"),
    ("underline", "doc16.underline", "Underline"),
    (
        "doubleUnderline",
        "doc16.doubleUnderline",
        "Double underline",
    ),
    ("strikethrough", "doc16.strikethrough", "Strikethrough"),
    // Round 18: the marks the layout draws since its round 8 (bold, italic).
    ("bold", "doc18.markBold", "Bold"),
    ("italic", "doc18.markItalic", "Italic"),
];

/// Deletions: (value, key, English).
const DEL: [(&str, &str, &str); 10] = [
    ("colorOnly", "doc16.colorOnly", "Color only"),
    ("strikethrough", "doc16.strikethrough", "Strikethrough"),
    (
        "doubleStrikethrough",
        "doc16.doubleStrikethrough",
        "Double strikethrough",
    ),
    ("hidden", "doc16.hidden", "Hidden"),
    ("underline", "doc16.underline", "Underline"),
    (
        "doubleUnderline",
        "doc16.doubleUnderline",
        "Double underline",
    ),
    ("bold", "doc18.markBold", "Bold"),
    ("italic", "doc18.markItalic", "Italic"),
    ("caret", "doc18.markCaret", "^"),
    ("hash", "doc18.markHash", "#"),
];

/// Formatting marks (round 18): (value, key, English).
const FMT: [(&str, &str, &str); 8] = [
    ("none", "doc16.markNone", "(none)"),
    ("bold", "doc18.markBold", "Bold"),
    ("italic", "doc18.markItalic", "Italic"),
    ("underline", "doc16.underline", "Underline"),
    (
        "doubleUnderline",
        "doc16.doubleUnderline",
        "Double underline",
    ),
    ("strikethrough", "doc16.strikethrough", "Strikethrough"),
    (
        "doubleStrikethrough",
        "doc16.doubleStrikethrough",
        "Double strikethrough",
    ),
    ("colorOnly", "doc16.colorOnly", "Color only"),
];

/// Colours of the colour boxes: (hex, key, English); `""` = by author.
const COLORS: [(&str, &str, &str); 18] = [
    ("", "doc16.byAuthor", "By author"),
    ("000000", "doc16.cAuto", "Auto"),
    ("000000", "doc16.cBlack", "Black"),
    ("0000FF", "doc16.cBlue", "Blue"),
    ("00FFFF", "doc16.cTurquoise", "Turquoise"),
    ("00FF00", "doc16.cBrightGreen", "Bright Green"),
    ("FF00FF", "doc16.cPink", "Pink"),
    ("FF0000", "doc16.cRed", "Red"),
    ("FFFF00", "doc16.cYellow", "Yellow"),
    ("FFFFFF", "doc16.cWhite", "White"),
    ("000080", "doc16.cDarkBlue", "Dark Blue"),
    ("008080", "doc16.cTeal", "Teal"),
    ("008000", "doc16.cGreen", "Green"),
    ("800080", "doc16.cViolet", "Violet"),
    ("800000", "doc16.cDarkRed", "Dark Red"),
    ("808000", "doc16.cDarkYellow", "Dark Yellow"),
    ("808080", "doc16.cGray50", "Gray-50%"),
    ("C0C0C0", "doc16.cGray25", "Gray-25%"),
];

/// Changed lines: (value, key, English).
const LINES: [(&str, &str, &str); 4] = [
    ("none", "doc16.markNone", "(none)"),
    ("left", "doc16.leftBorder", "Left border"),
    ("right", "doc16.rightBorder", "Right border"),
    ("outside", "doc16.outsideBorder", "Outside border"),
];

/// Use Balloons (Print and Web Layout): (key, English, deletionBalloons, inline).
const USE_BALLOONS: [(&str, &str, bool, bool); 3] = [
    ("doc16.always", "Always", true, false),
    ("doc16.never", "Never", false, true),
    (
        "doc16.onlyComments",
        "Only for comments/formatting",
        false,
        false,
    ),
];

/// Measure in: (key, English, points per unit).
const UNITS: [(&str, &str, f32); 3] = [
    ("doc16.inches", "Inches", 72.0),
    ("doc16.centimeters", "Centimeters", 72.0 / 2.54),
    ("doc16.points", "Points", 1.0),
];

fn names<const N: usize, T>(
    s: &Strings,
    rows: &[T; N],
    f: impl Fn(&T) -> (&str, &str),
) -> Vec<String> {
    rows.iter()
        .map(|r| {
            let (k, en) = f(r);
            t(s, k, en)
        })
        .collect()
}

/// Open dialog `kind` with the review options in `c.extra["r16"]`.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let m = &c.extra["r16"]["markup"];
    let flag = |k: &str, d: bool| m[k].as_bool().unwrap_or(d);
    let tr = |k: &str, fb: &str| t(s, k, fb);
    if kind == DocFormKind::AdvTrackOptions {
        return open_advanced(s, m);
    }
    if matches!(kind, DocFormKind::MergeNew | DocFormKind::MergePrint) {
        return open_merge(kind, s, c);
    }
    if kind == DocFormKind::CheckErrors {
        return open_check(s);
    }
    if kind == DocFormKind::NewList {
        return open_new_list(s);
    }
    if kind == DocFormKind::MergeEmail {
        return open_merge_email(s, c);
    }
    if kind == DocFormKind::NlFind {
        let mut f = DocForm::new(
            kind,
            &tr("doc16.findEntryTitle", "Find Entry"),
            (380.0, 200.0),
            vec![],
        );
        f.label(
            "whatL",
            [14.0, 52.0, 60.0, 21.0],
            &tr("doc16.findWhat", "Find:"),
        );
        f.edit("what", [80.0, 48.0, 286.0, 28.0], "");
        f.label(
            "inL",
            [14.0, 88.0, 100.0, 21.0],
            &tr("doc16.lookIn", "Look in:"),
        );
        f.radio(
            "allFields",
            "in",
            [30.0, 112.0, 160.0, 24.0],
            &tr("doc16.allFields", "All fields"),
            true,
        );
        f.radio(
            "thisField",
            "in",
            [196.0, 112.0, 170.0, 24.0],
            &tr("doc16.thisField", "This field"),
            false,
        );
        f.button(
            "ok",
            [140.0, 156.0, 106.0, 32.0],
            &tr("doc16.findNext", "Find Next"),
            true,
        );
        f.button(
            "cancel",
            [260.0, 156.0, 106.0, 32.0],
            &tr("common.cancel", "Cancel"),
            false,
        );
        f.form.focus("what");
        return f;
    }
    if kind == DocFormKind::NlColumns {
        return open_nl_columns(s, c);
    }
    if kind == DocFormKind::MergeReport {
        let text = c.extra["r16"]["report"].as_str().unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let h = 120.0 + 24.0 * lines.len().min(12) as f32;
        let mut f = DocForm::new(
            kind,
            &tr("doc16.checkTitle", "Checking and Reporting Errors"),
            (420.0, h),
            vec![],
        );
        for (i, l) in lines.iter().take(12).enumerate() {
            let id = [
                "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11",
            ][i];
            f.label(id, [20.0, 52.0 + 24.0 * i as f32, 380.0, 21.0], l);
        }
        f.button(
            "ok",
            [294.0, h - 48.0, 106.0, 32.0],
            &tr("common.ok", "OK"),
            true,
        );
        return f;
    }
    let mut f = DocForm::new(
        kind,
        &tr("doc16.toTitle", "Track Changes Options"),
        (494.0, 386.0),
        vec![],
    );
    f.group(
        "showG",
        [8.0, 47.0, 478.0, 155.0],
        &tr("doc16.show", "Show"),
    );
    f.check(
        "comments",
        [19.0, 77.0, 200.0, 26.0],
        &tr("commands.review.markupComments", "Comments"),
        flag("comments", true),
    );
    f.check(
        "insDel",
        [19.0, 108.0, 205.0, 26.0],
        &tr("commands.review.markupInsDel", "Insertions and Deletions"),
        flag("insertionsDeletions", true),
    );
    f.check(
        "formatting",
        [19.0, 139.0, 200.0, 26.0],
        &tr("commands.review.markupFormatting", "Formatting"),
        flag("formatting", true),
    );
    f.check(
        "pictures",
        [228.0, 77.0, 220.0, 26.0],
        &tr("doc16.pictures", "Pictures By Comments"),
        flag("pictures", true),
    );
    f.label(
        "balloonsL",
        [19.0, 218.0, 250.0, 21.0],
        &tr("doc16.balloonsShow", "Balloons in All Markup view show:"),
    );
    let (del, inline) = (flag("deletionBalloons", false), flag("inline", false));
    let b = BALLOONS
        .iter()
        .position(|r| r.2 == del && r.3 == inline)
        .unwrap_or(2);
    f.combo(
        "balloons",
        [271.0, 216.0, 215.0, 26.0],
        names(s, &BALLOONS, |r| (r.0, r.1)),
        b,
    );
    f.label(
        "paneL",
        [19.0, 249.0, 200.0, 21.0],
        &tr("doc16.reviewingPane", "Reviewing Pane:"),
    );
    let pane = c.extra["r16"]["pane"].as_str().unwrap_or("off");
    f.combo(
        "pane",
        [271.0, 247.0, 103.0, 26.0],
        names(s, &PANES, |r| (r.1, r.2)),
        PANES.iter().position(|r| r.0 == pane).unwrap_or(0),
    );
    f.button(
        "advanced",
        [19.0, 293.0, 160.0, 36.0],
        &tr("doc16.advanced", "Advanced Options..."),
        false,
    );
    f.button(
        "userName",
        [193.0, 293.0, 168.0, 36.0],
        &tr("doc16.userName", "Change User Name..."),
        false,
    );
    f.button(
        "ok",
        [256.0, 340.0, 108.0, 36.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [378.0, 340.0, 108.0, 36.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// Merge to New Document / Merge to Printer: Merge records All / Current record / From–To.
fn open_merge(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let title = if kind == DocFormKind::MergeNew {
        tr("doc16.mergeNewTitle", "Merge to New Document")
    } else {
        tr("doc16.mergePrintTitle", "Merge to Printer")
    };
    // The reference's dialog: 333 × 210 at 125 %, From / To boxes empty.
    let mut f = DocForm::new(kind, &title, (333.0, 210.0), vec![]);
    let _ = c;
    f.group(
        "recordsG",
        [8.0, 46.0, 317.0, 110.0],
        &tr("doc16.mergeRecords", "Merge records"),
    );
    f.radio(
        "all",
        "range",
        [25.0, 72.0, 90.0, 24.0],
        &tr("doc16.all", "All"),
        true,
    );
    f.radio(
        "current",
        "range",
        [25.0, 98.0, 200.0, 24.0],
        &tr("doc16.currentRecord", "Current record"),
        false,
    );
    f.radio(
        "from",
        "range",
        [25.0, 124.0, 72.0, 24.0],
        &tr("doc16.from", "From:"),
        false,
    );
    f.edit("fromN", [99.0, 121.0, 86.0, 26.0], "");
    f.label("toL", [199.0, 126.0, 30.0, 21.0], &tr("doc16.to", "To:"));
    f.edit("toN", [235.0, 121.0, 86.0, 26.0], "");
    f.button(
        "ok",
        [83.0, 168.0, 106.0, 32.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [210.0, 168.0, 112.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// The columns of a new address list (the reference's default columns).
pub(crate) const LIST_FIELDS: [&str; 13] = [
    "Title",
    "First Name",
    "Last Name",
    "Company Name",
    "Address Line 1",
    "Address Line 2",
    "City",
    "State",
    "ZIP Code",
    "Country or Region",
    "Home Phone",
    "Work Phone",
    "E-mail Address",
];

/// Edit boxes of the entry being typed (one per column).
const FIELD_IDS: [&str; 13] = [
    "f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
];
const LABEL_IDS: [&str; 13] = [
    "l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9", "l10", "l11", "l12",
];

/// New Address List: the entries (one per row, columns joined), the columns of the selected
/// entry to type in, New Entry / Delete Entry; OK makes the list the merge's data source.
fn open_new_list(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::NewList,
        &tr("doc16.newListTitle", "New Address List"),
        (790.0, 520.0),
        vec![],
    );
    f.label(
        "intro",
        [12.0, 46.0, 766.0, 21.0],
        &tr(
            "doc16.newListIntro",
            "Type recipient information in the table. To add more entries, click New Entry.",
        ),
    );
    f.list(
        "entries",
        [12.0, 72.0, 766.0, 150.0],
        vec![String::new()],
        Some(0),
    );
    for (i, name) in LIST_FIELDS.iter().enumerate() {
        let (col, row) = (i / 7, i % 7);
        let (lx, ex, ew) = if col == 0 {
            (12.0, 150.0, 230.0)
        } else {
            (400.0, 548.0, 230.0)
        };
        let y = 234.0 + 32.0 * row as f32;
        f.label(LABEL_IDS[i], [lx, y + 4.0, 134.0, 21.0], name);
        f.edit(FIELD_IDS[i], [ex, y, ew, 26.0], "");
    }
    f.button(
        "newEntry",
        [12.0, 470.0, 128.0, 32.0],
        &tr("doc16.newEntry", "New Entry"),
        false,
    );
    f.button(
        "deleteEntry",
        [148.0, 470.0, 128.0, 32.0],
        &tr("doc16.deleteEntry", "Delete Entry"),
        false,
    );
    f.button(
        "ok",
        [560.0, 476.0, 106.0, 32.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [672.0, 476.0, 106.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.words = vec![
        ("records".into(), "[[]]".into()),
        ("cur".into(), "0".into()),
    ];
    f.form.focus("f0");
    f
}

impl DocForm {
    fn nl_records(&self) -> (Vec<Vec<String>>, usize) {
        let get = |k: &str| {
            self.words
                .iter()
                .find(|(a, _)| a == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let recs: Vec<Vec<String>> = serde_json::from_str(&get("records")).unwrap_or_default();
        let cur = get("cur").parse().unwrap_or(0);
        (recs, cur)
    }

    fn nl_store(&mut self, recs: &[Vec<String>], cur: usize) {
        let text = serde_json::to_string(recs).unwrap_or_default();
        for (k, v) in &mut self.words {
            if k == "records" {
                v.clone_from(&text);
            } else if k == "cur" {
                *v = cur.to_string();
            }
        }
        let items: Vec<String> = recs
            .iter()
            .map(|r| {
                r.iter()
                    .filter(|c| !c.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("  |  ")
            })
            .collect();
        self.form.set_items("entries", items, Some(cur));
    }

    /// The edit boxes into the selected entry.
    fn nl_save_entry(&mut self) -> (Vec<Vec<String>>, usize) {
        let (mut recs, cur) = self.nl_records();
        if recs.is_empty() {
            recs.push(Vec::new());
        }
        let row: Vec<String> = FIELD_IDS.iter().map(|id| self.text(id)).collect();
        let cur = cur.min(recs.len() - 1);
        recs[cur] = row;
        (recs, cur)
    }

    fn nl_load_entry(&mut self, recs: &[Vec<String>], cur: usize) {
        for (i, id) in FIELD_IDS.iter().enumerate() {
            let v = recs
                .get(cur)
                .and_then(|r| r.get(i))
                .cloned()
                .unwrap_or_default();
            self.set_text(id, &v);
        }
    }

    /// New Address List: a typed column, another entry chosen, New / Delete Entry.
    fn nl_event(&mut self, id: &str) -> bool {
        if self.kind != DocFormKind::NewList {
            return false;
        }
        match id {
            "entries" => {
                let (recs, _) = self.nl_records();
                let cur = self
                    .sel("entries")
                    .unwrap_or(0)
                    .min(recs.len().saturating_sub(1));
                self.nl_store(&recs, cur);
                self.nl_load_entry(&recs, cur);
            }
            "newEntry" => {
                let (mut recs, _) = self.nl_save_entry();
                recs.push(Vec::new());
                let cur = recs.len() - 1;
                self.nl_store(&recs, cur);
                self.nl_load_entry(&recs, cur);
                self.form.focus("f0");
            }
            "deleteEntry" => {
                let (mut recs, cur) = self.nl_records();
                if cur < recs.len() {
                    recs.remove(cur);
                }
                if recs.is_empty() {
                    recs.push(Vec::new());
                }
                let cur = cur.min(recs.len() - 1);
                self.nl_store(&recs, cur);
                self.nl_load_entry(&recs, cur);
            }
            _ if FIELD_IDS.contains(&id) => {
                let (recs, cur) = self.nl_save_entry();
                self.nl_store(&recs, cur);
            }
            _ => return false,
        }
        true
    }
}

/// Merge to E-mail: To (a field), Subject line, Mail format (HTML / Plain text / Attachment),
/// Send records All / Current record / From–To.
fn open_merge_email(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::MergeEmail,
        &tr("doc16.mergeEmailTitle", "Merge to E-mail"),
        (420.0, 330.0),
        vec![],
    );
    let fields: Vec<String> = c.extra["r16"]["fields"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let mail = fields
        .iter()
        .position(|f| {
            let l = f.to_lowercase();
            l.contains("mail")
        })
        .unwrap_or(0);
    f.group(
        "msgG",
        [8.0, 46.0, 404.0, 140.0],
        &tr("doc16.messageOptions", "Message options"),
    );
    f.label("toL", [20.0, 76.0, 100.0, 21.0], &tr("doc16.mailTo", "To:"));
    f.combo("to", [140.0, 72.0, 260.0, 28.0], fields, mail);
    f.label(
        "subjL",
        [20.0, 112.0, 120.0, 21.0],
        &tr("doc16.subjectLine", "Subject line:"),
    );
    f.edit("subject", [140.0, 108.0, 260.0, 28.0], "");
    f.label(
        "fmtL",
        [20.0, 148.0, 120.0, 21.0],
        &tr("doc16.mailFormat", "Mail format:"),
    );
    f.combo(
        "format",
        [140.0, 144.0, 260.0, 28.0],
        vec![
            tr("doc16.formatHtml", "HTML"),
            tr("doc16.formatText", "Plain text"),
            tr("doc16.formatAttachment", "Attachment"),
        ],
        0,
    );
    f.group(
        "sendG",
        [8.0, 194.0, 404.0, 82.0],
        &tr("doc16.sendRecords", "Send records"),
    );
    f.radio(
        "all",
        "range",
        [20.0, 218.0, 80.0, 24.0],
        &tr("doc16.all", "All"),
        true,
    );
    f.radio(
        "current",
        "range",
        [110.0, 218.0, 180.0, 24.0],
        &tr("doc16.currentRecord", "Current record"),
        false,
    );
    f.radio(
        "from",
        "range",
        [20.0, 246.0, 72.0, 24.0],
        &tr("doc16.from", "From:"),
        false,
    );
    f.edit("fromN", [94.0, 244.0, 70.0, 26.0], "");
    f.label("toNL", [176.0, 248.0, 30.0, 21.0], &tr("doc16.to", "To:"));
    f.edit("toN", [210.0, 244.0, 70.0, 26.0], "");
    f.button(
        "ok",
        [182.0, 288.0, 106.0, 32.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [300.0, 288.0, 106.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// Customize Address List: the columns (Add / Delete / Rename with the name box, Move Up /
/// Move Down); OK gives the new columns back to New Address List.
fn open_nl_columns(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::NlColumns,
        &tr("doc16.customizeTitle", "Customize Address List"),
        (460.0, 380.0),
        vec![],
    );
    let cols: Vec<String> = c.extra["r16"]["columns"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    f.label(
        "namesL",
        [14.0, 46.0, 200.0, 21.0],
        &tr("doc16.fieldNames", "Field Names"),
    );
    f.list("cols", [14.0, 70.0, 300.0, 220.0], cols.clone(), Some(0));
    f.edit("name", [14.0, 300.0, 300.0, 28.0], "");
    let btn = |f: &mut DocForm, id: &'static str, y: f32, k: &str, en: &str| {
        f.button(id, [328.0, y, 118.0, 30.0], &tr(k, en), false);
    };
    btn(&mut f, "add", 70.0, "doc16.colAdd", "Add...");
    btn(&mut f, "del", 106.0, "doc16.colDelete", "Delete");
    btn(&mut f, "rename", 142.0, "doc16.colRename", "Rename...");
    btn(&mut f, "up", 200.0, "doc16.colUp", "Move Up");
    btn(&mut f, "down", 236.0, "doc16.colDown", "Move Down");
    f.button(
        "ok",
        [222.0, 340.0, 106.0, 32.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [340.0, 340.0, 106.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    // The columns with where each came from (-1: added).
    let state: Vec<(i64, String)> = cols
        .iter()
        .enumerate()
        .map(|(i, n)| (i as i64, n.clone()))
        .collect();
    f.words = vec![(
        "cols".into(),
        serde_json::to_string(&state).unwrap_or_default(),
    )];
    f
}

impl DocForm {
    fn nc_state(&self) -> Vec<(i64, String)> {
        self.words
            .iter()
            .find(|(k, _)| k == "cols")
            .and_then(|(_, v)| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    fn nc_store(&mut self, st: &[(i64, String)], sel: usize) {
        let text = serde_json::to_string(st).unwrap_or_default();
        for (k, v) in &mut self.words {
            if k == "cols" {
                v.clone_from(&text);
            }
        }
        let names = st.iter().map(|(_, n)| n.clone()).collect();
        self.form
            .set_items("cols", names, Some(sel.min(st.len().saturating_sub(1))));
    }

    /// Customize Address List buttons.
    fn nc_clicked(&mut self, id: &str) -> bool {
        if self.kind != DocFormKind::NlColumns {
            return false;
        }
        let mut st = self.nc_state();
        let sel = self
            .sel("cols")
            .unwrap_or(0)
            .min(st.len().saturating_sub(1));
        let name = self.text("name").trim().to_owned();
        let sel = match id {
            "add" if !name.is_empty() => {
                st.insert((sel + 1).min(st.len()), (-1, name));
                self.set_text("name", "");
                (sel + 1).min(st.len() - 1)
            }
            "del" if !st.is_empty() => {
                st.remove(sel);
                sel
            }
            "rename" if !name.is_empty() && sel < st.len() => {
                st[sel].1 = name;
                self.set_text("name", "");
                sel
            }
            "up" if sel > 0 => {
                st.swap(sel, sel - 1);
                sel - 1
            }
            "down" if sel + 1 < st.len() => {
                st.swap(sel, sel + 1);
                sel + 1
            }
            "add" | "del" | "rename" | "up" | "down" => return true,
            _ => return false,
        };
        self.nc_store(&st, sel);
        true
    }
}

/// Checking and Reporting Errors: simulate the merge, complete it pausing on each error, or
/// complete it and report the errors (the reference's three choices).
fn open_check(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::CheckErrors,
        &tr("doc16.checkTitle", "Checking and Reporting Errors"),
        (358.0, 221.0),
        vec![],
    );
    // The reference's dialog: 358 x 221 at 125 %, "pause" chosen when it opens.
    let rows: [(&'static str, &str, &str, f32, bool); 3] = [
        (
            "sim",
            "doc16.checkSimulate",
            "Simulate the merge and report errors in a new document.",
            46.0,
            false,
        ),
        (
            "pause",
            "doc16.checkPause",
            "Complete the merge, pausing to report each error as it occurs.",
            87.0,
            true,
        ),
        (
            "report",
            "doc16.checkReport",
            "Complete the merge without pausing. Report errors in a new document.",
            129.0,
            false,
        ),
    ];
    for (id, k, en, y, on) in rows {
        f.radio(id, "mode", [10.0, y, 340.0, 38.0], &tr(k, en), on);
    }
    f.button(
        "ok",
        [107.0, 173.0, 112.0, 30.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [233.0, 173.0, 112.0, 30.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

fn open_advanced(s: &Strings, m: &Value) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let text = |k: &str, d: &str| m[k].as_str().unwrap_or(d).to_owned();
    let mut f = DocForm::new(
        DocFormKind::AdvTrackOptions,
        &tr("doc16.advTitle", "Advanced Track Changes Options"),
        (543.0, 732.0),
        vec![],
    );
    let color_at = |hex: &str| {
        if hex.is_empty() {
            0
        } else {
            COLORS
                .iter()
                .skip(2)
                .position(|c| c.0.eq_ignore_ascii_case(hex))
                .map_or(0, |i| i + 2)
        }
    };
    let colors = names(s, &COLORS, |r| (r.1, r.2));
    f.group(
        "markupG",
        [8.0, 47.0, 527.0, 230.0],
        &tr("doc16.groupMarkup", "Markup"),
    );
    let rows: [(&'static str, &'static str, &str, f32); 2] = [
        ("insL", "ins", "Insertions:", 75.0),
        ("delL", "del", "Deletions:", 107.0),
    ];
    for (lid, id, en, y) in rows {
        f.label(
            lid,
            [18.0, y + 6.0, 110.0, 21.0],
            &tr(&format!("doc16.{id}Label"), en),
        );
        if id == "ins" {
            let v = text("insMark", "underline");
            f.combo(
                "ins",
                [132.0, y, 158.0, 26.0],
                names(s, &INS, |r| (r.1, r.2)),
                INS.iter().position(|r| r.0 == v).unwrap_or(2),
            );
            f.label(
                "insColorL",
                [312.0, y + 6.0, 70.0, 21.0],
                &tr("doc16.color", "Color:"),
            );
            f.combo(
                "insColor",
                [394.0, y, 138.0, 26.0],
                colors.clone(),
                color_at(&text("insColor", "")),
            );
        } else {
            let v = text("delMark", "strikethrough");
            f.combo(
                "del",
                [132.0, y, 158.0, 26.0],
                names(s, &DEL, |r| (r.1, r.2)),
                DEL.iter().position(|r| r.0 == v).unwrap_or(1),
            );
            f.label(
                "delColorL",
                [312.0, y + 6.0, 70.0, 21.0],
                &tr("doc16.color", "Color:"),
            );
            f.combo(
                "delColor",
                [394.0, y, 138.0, 26.0],
                colors.clone(),
                color_at(&text("delColor", "")),
            );
        }
    }
    f.label(
        "linesL",
        [18.0, 145.0, 110.0, 21.0],
        &tr("doc16.changedLines", "Changed lines:"),
    );
    let lines = text("changedLines", "outside");
    f.combo(
        "lines",
        [132.0, 139.0, 158.0, 26.0],
        names(s, &LINES, |r| (r.1, r.2)),
        LINES.iter().position(|r| r.0 == lines).unwrap_or(3),
    );
    f.label(
        "commentsL",
        [18.0, 233.0, 110.0, 21.0],
        &tr("doc16.commentsLabel", "Comments:"),
    );
    f.combo(
        "commentColor",
        [132.0, 227.0, 158.0, 26.0],
        colors.clone(),
        color_at(&text("commentColor", "")),
    );
    // Round 18: Moves and Formatting, with the marks the layout draws (setMarkup).
    f.group(
        "movesG",
        [8.0, 287.0, 527.0, 120.0],
        &tr("doc18.movesGroup", "Moves"),
    );
    f.check(
        "trackMoves",
        [19.0, 311.0, 300.0, 26.0],
        &tr("doc18.trackMoves", "Track moves"),
        m["trackMoves"].as_bool().unwrap_or(true),
    );
    let move_rows: [(&'static str, &'static str, &'static str, &'static str); 2] = [
        ("fromL", "moveFrom", "moveFromColorL", "moveFromColor"),
        ("toL", "moveTo", "moveToColorL", "moveToColor"),
    ];
    for ((lid, id, clid, cid), (key, en, y, list, def)) in move_rows.into_iter().zip([
        (
            "doc18.movedFrom",
            "Moved from:",
            343.0,
            &DEL[..],
            "doubleStrikethrough",
        ),
        (
            "doc18.movedTo",
            "Moved to:",
            375.0,
            &INS[..],
            "doubleUnderline",
        ),
    ]) {
        f.label(lid, [18.0, y + 6.0, 110.0, 21.0], &tr(key, en));
        let v = text(&format!("{id}Mark"), def);
        f.combo(
            id,
            [132.0, y, 158.0, 26.0],
            list.iter().map(|r| tr(r.1, r.2)).collect(),
            list.iter().position(|r| r.0 == v).unwrap_or(0),
        );
        f.label(
            clid,
            [312.0, y + 6.0, 70.0, 21.0],
            &tr("doc16.color", "Color:"),
        );
        f.combo(
            cid,
            [394.0, y, 138.0, 26.0],
            colors.clone(),
            color_at(&text(cid, "")),
        );
    }
    f.group(
        "fmtG",
        [8.0, 417.0, 527.0, 96.0],
        &tr("doc18.formattingGroup", "Formatting"),
    );
    f.check(
        "trackFormatting",
        [19.0, 441.0, 300.0, 26.0],
        &tr("doc18.trackFormatting", "Track formatting"),
        m["formatting"].as_bool().unwrap_or(true),
    );
    f.label(
        "fmtL",
        [18.0, 479.0, 110.0, 21.0],
        &tr("doc18.formattingLabel", "Formatting:"),
    );
    let fv = text("fmtMark", "none");
    f.combo(
        "fmt",
        [132.0, 473.0, 158.0, 26.0],
        names(s, &FMT, |r| (r.1, r.2)),
        FMT.iter().position(|r| r.0 == fv).unwrap_or(0),
    );
    f.label(
        "fmtColorL",
        [312.0, 479.0, 70.0, 21.0],
        &tr("doc16.color", "Color:"),
    );
    f.combo(
        "fmtColor",
        [394.0, 473.0, 138.0, 26.0],
        colors.clone(),
        color_at(&text("fmtColor", "")),
    );
    f.group(
        "balloonsG",
        [8.0, 523.0, 527.0, 120.0],
        &tr("doc16.balloonsGroup", "Balloons"),
    );
    f.label(
        "useL",
        [18.0, 557.0, 290.0, 21.0],
        &tr("doc16.useBalloons", "Use Balloons (Print and Web Layout):"),
    );
    let (del, inline) = (
        m["deletionBalloons"].as_bool().unwrap_or(false),
        m["inline"].as_bool().unwrap_or(false),
    );
    f.combo(
        "useBalloons",
        [313.0, 551.0, 216.0, 26.0],
        names(s, &USE_BALLOONS, |r| (r.0, r.1)),
        USE_BALLOONS
            .iter()
            .position(|r| r.2 == del && r.3 == inline)
            .unwrap_or(2),
    );
    f.label(
        "widthL",
        [18.0, 589.0, 110.0, 21.0],
        &tr("doc16.preferredWidth", "Preferred width:"),
    );
    let inches = m["balloonWidth"].as_f64().unwrap_or(3.7) as f32;
    f.edit(
        "width",
        [132.0, 583.0, 158.0, 26.0],
        &format!("{inches:.1}\""),
    );
    f.label(
        "unitL",
        [312.0, 589.0, 80.0, 21.0],
        &tr("doc16.measureIn", "Measure in:"),
    );
    f.combo(
        "unit",
        [394.0, 583.0, 128.0, 26.0],
        names(s, &UNITS, |r| (r.0, r.1)),
        0,
    );
    f.check(
        "connectors",
        [19.0, 614.0, 300.0, 26.0],
        &tr("doc16.connectors", "Show lines connecting to text"),
        m["connectors"].as_bool().unwrap_or(true),
    );
    f.button(
        "ok",
        [293.0, 686.0, 110.0, 36.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [420.0, 686.0, 110.0, 36.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

impl DocForm {
    /// A button of a round-16 dialog.
    pub(super) fn r16_clicked(&mut self, id: &str) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        if self.nl_event(id) || self.nc_clicked(id) {
            return Some(FormOut::Redraw);
        }
        match id {
            "advanced" => Some(FormOut::Stash(DocFormKind::AdvTrackOptions)),
            "userName" => Some(FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "file.options" })],
                true,
            )),
            _ => None,
        }
    }

    /// Measure in changed: the width box shows the width in the new unit.
    pub(super) fn r16_changed(&mut self, id: &str) {
        if self.nl_event(id) {
            return;
        }
        if self.kind == DocFormKind::AdvTrackOptions && id == "unit" {
            let pts = self.r16_width_points();
            let u = UNITS[self.sel("unit").unwrap_or(0).min(2)];
            let v = pts / u.2;
            let shown = match u.1 {
                "Inches" => format!("{v:.1}\""),
                "Centimeters" => format!("{v:.2} cm"),
                _ => format!("{v:.0} pt"),
            };
            self.set_text("width", &shown);
        }
    }

    /// The width box read in points (its own unit, else the Measure in unit).
    fn r16_width_points(&self) -> f32 {
        let raw = self.text("width");
        let t = raw.trim();
        let num: String = t
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
            .collect();
        let v: f32 = num.replace(',', ".").parse().unwrap_or(3.7);
        let unit = if t.ends_with('"') || t.ends_with("in") {
            72.0
        } else if t.ends_with("cm") {
            72.0 / 2.54
        } else if t.ends_with("pt") {
            1.0
        } else {
            UNITS[self.sel("unit").unwrap_or(0).min(2)].2
        };
        v * unit
    }

    /// OK of a round-16 dialog: the review display options (and the Reviewing Pane).
    pub(super) fn r16_ok(&mut self) -> Option<FormOut> {
        match self.kind {
            DocFormKind::CheckErrors => {
                let mode = self.chosen("mode").unwrap_or("sim");
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r16.checkErrors:{mode}") })],
                    true,
                ))
            }
            DocFormKind::MergeReport => Some(FormOut::Close),
            DocFormKind::NlFind => {
                let this = u8::from(self.chosen("in") == Some("thisField"));
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action",
                        "id": format!("r16.nl.find:{this}\u{1f}{}", self.text("what")) })],
                    false,
                ))
            }
            DocFormKind::NlColumns => {
                let st = self.nc_state();
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action",
                        "id": format!("r16.nl.cols:{}", serde_json::to_string(&st).unwrap_or_default()) })],
                    true,
                ))
            }
            DocFormKind::MergeEmail => {
                let range = match self.chosen("range") {
                    Some("current") => "cur".to_owned(),
                    Some("from") => {
                        let n = |id: &str, d: usize| {
                            self.text(id).trim().parse::<usize>().unwrap_or(d).max(1)
                        };
                        let from = n("fromN", 1);
                        format!("{from}-{}", n("toN", usize::MAX).max(from))
                    }
                    _ => "all".to_owned(),
                };
                let format = ["html", "text", "attach"][self.sel("format").unwrap_or(0).min(2)];
                let spec = json!({
                    "to": self.text("to"), "subject": self.text("subject"),
                    "format": format, "range": range,
                });
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r16.email:{spec}") })],
                    true,
                ))
            }
            DocFormKind::NewList => {
                let rows: Vec<Vec<String>> = self
                    .nl_save_entry()
                    .0
                    .into_iter()
                    .filter(|r| r.iter().any(|c| !c.trim().is_empty()))
                    .collect();
                if rows.is_empty() {
                    return Some(FormOut::Close);
                }
                Some(FormOut::Run(
                    vec![json!({
                        "cmd": "mergeTable", "name": "Address List",
                        "fields": LIST_FIELDS, "rows": rows,
                    })],
                    true,
                ))
            }
            DocFormKind::MergeNew | DocFormKind::MergePrint => {
                let what = if self.kind == DocFormKind::MergeNew {
                    "edit"
                } else {
                    "print"
                };
                let range = match self.chosen("range") {
                    Some("current") => "cur".to_owned(),
                    Some("from") => {
                        let n =
                            |id: &str| self.text(id).trim().parse::<usize>().unwrap_or(1).max(1);
                        format!("{}-{}", n("fromN"), n("toN").max(n("fromN")))
                    }
                    _ => "all".to_owned(),
                };
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r16.merge:{what}:{range}") })],
                    true,
                ))
            }
            DocFormKind::TrackOptions => {
                let b = BALLOONS[self.sel("balloons").unwrap_or(2).min(2)];
                let pane = PANES[self.sel("pane").unwrap_or(0).min(2)].0;
                Some(FormOut::Run(
                    vec![
                        json!({
                            "cmd": "setMarkup",
                            "comments": self.on("comments"),
                            "insertionsDeletions": self.on("insDel"),
                            "formatting": self.on("formatting"),
                            "pictures": self.on("pictures"),
                            "deletionBalloons": b.2,
                            "inline": b.3,
                        }),
                        json!({ "cmd": "ui.action", "id": format!("r16.pane:{pane}") }),
                    ],
                    true,
                ))
            }
            DocFormKind::AdvTrackOptions => {
                let color = |id: &str| COLORS[self.sel(id).unwrap_or(0).min(17)].0;
                let b = USE_BALLOONS[self.sel("useBalloons").unwrap_or(2).min(2)];
                Some(FormOut::Run(
                    vec![json!({
                        "cmd": "setMarkup",
                        "insMark": INS[self.sel("ins").unwrap_or(2).min(INS.len() - 1)].0,
                        "delMark": DEL[self.sel("del").unwrap_or(1).min(DEL.len() - 1)].0,
                        "trackMoves": self.on("trackMoves"),
                        "moveFromMark": DEL[self.sel("moveFrom").unwrap_or(2).min(DEL.len() - 1)].0,
                        "moveToMark": INS[self.sel("moveTo").unwrap_or(3).min(INS.len() - 1)].0,
                        "moveFromColor": color("moveFromColor"),
                        "moveToColor": color("moveToColor"),
                        "formatting": self.on("trackFormatting"),
                        "fmtMark": FMT[self.sel("fmt").unwrap_or(0).min(FMT.len() - 1)].0,
                        "fmtColor": color("fmtColor"),
                        "insColor": color("insColor"),
                        "delColor": color("delColor"),
                        "commentColor": color("commentColor"),
                        "changedLines": LINES[self.sel("lines").unwrap_or(3).min(3)].0,
                        "balloonWidth": (self.r16_width_points() / 72.0).clamp(0.5, 12.0),
                        "connectors": self.on("connectors"),
                        "deletionBalloons": b.2,
                        "inline": b.3,
                    })],
                    true,
                ))
            }
            _ => None,
        }
    }
}
