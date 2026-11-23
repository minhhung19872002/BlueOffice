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
const INS: [(&str, &str, &str); 5] = [
    ("none", "doc16.markNone", "(none)"),
    ("colorOnly", "doc16.colorOnly", "Color only"),
    ("underline", "doc16.underline", "Underline"),
    (
        "doubleUnderline",
        "doc16.doubleUnderline",
        "Double underline",
    ),
    ("strikethrough", "doc16.strikethrough", "Strikethrough"),
];

/// Deletions: (value, key, English).
const DEL: [(&str, &str, &str); 6] = [
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

/// Checking and Reporting Errors: simulate the merge, complete it pausing on each error, or
/// complete it and report the errors (the reference's three choices).
fn open_check(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::CheckErrors,
        &tr("doc16.checkTitle", "Checking and Reporting Errors"),
        (470.0, 250.0),
        vec![],
    );
    f.radio(
        "sim",
        "mode",
        [20.0, 52.0, 430.0, 44.0],
        &tr(
            "doc16.checkSimulate",
            "Simulate the merge and report errors in a new document.",
        ),
        true,
    );
    f.radio(
        "pause",
        "mode",
        [20.0, 100.0, 430.0, 44.0],
        &tr(
            "doc16.checkPause",
            "Complete the merge, pausing to report each error as it occurs.",
        ),
        false,
    );
    f.radio(
        "report",
        "mode",
        [20.0, 148.0, 430.0, 44.0],
        &tr(
            "doc16.checkReport",
            "Complete the merge without pausing. Report errors in a new document.",
        ),
        false,
    );
    f.button(
        "ok",
        [226.0, 204.0, 106.0, 32.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [344.0, 204.0, 106.0, 32.0],
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
        (543.0, 496.0),
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
        colors,
        color_at(&text("commentColor", "")),
    );
    f.group(
        "balloonsG",
        [8.0, 287.0, 527.0, 120.0],
        &tr("doc16.balloonsGroup", "Balloons"),
    );
    f.label(
        "useL",
        [18.0, 321.0, 290.0, 21.0],
        &tr("doc16.useBalloons", "Use Balloons (Print and Web Layout):"),
    );
    let (del, inline) = (
        m["deletionBalloons"].as_bool().unwrap_or(false),
        m["inline"].as_bool().unwrap_or(false),
    );
    f.combo(
        "useBalloons",
        [313.0, 315.0, 216.0, 26.0],
        names(s, &USE_BALLOONS, |r| (r.0, r.1)),
        USE_BALLOONS
            .iter()
            .position(|r| r.2 == del && r.3 == inline)
            .unwrap_or(2),
    );
    f.label(
        "widthL",
        [18.0, 353.0, 110.0, 21.0],
        &tr("doc16.preferredWidth", "Preferred width:"),
    );
    let inches = m["balloonWidth"].as_f64().unwrap_or(3.7) as f32;
    f.edit(
        "width",
        [132.0, 347.0, 158.0, 26.0],
        &format!("{inches:.1}\""),
    );
    f.label(
        "unitL",
        [312.0, 353.0, 80.0, 21.0],
        &tr("doc16.measureIn", "Measure in:"),
    );
    f.combo(
        "unit",
        [394.0, 347.0, 128.0, 26.0],
        names(s, &UNITS, |r| (r.0, r.1)),
        0,
    );
    f.check(
        "connectors",
        [19.0, 378.0, 300.0, 26.0],
        &tr("doc16.connectors", "Show lines connecting to text"),
        m["connectors"].as_bool().unwrap_or(true),
    );
    f.button(
        "ok",
        [293.0, 450.0, 110.0, 36.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [420.0, 450.0, 110.0, 36.0],
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
                        "insMark": INS[self.sel("ins").unwrap_or(2).min(4)].0,
                        "delMark": DEL[self.sel("del").unwrap_or(1).min(5)].0,
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
