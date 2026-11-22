//! Table Properties of BlueDoc, parity round 11 (docs/ui/REPORT.md "Vòng 101"): the
//! reference's five tabs (Table, Row, Column, Cell, Alt Text) laid out from its dialog captured
//! at 125 % (580 × 581 window, coordinates less the 8 px invisible border). OK runs the table
//! edits the engine has (preferred width and alignment, text wrapping, row height, header
//! row, column width, cell vertical alignment). A child module of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use crate::Painter;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// Picture choices: id, state slot in `ids`, value.
const PICKS: [(&str, usize, &str); 8] = [
    ("alLeft", 0, "left"),
    ("alCenter", 0, "center"),
    ("alRight", 0, "right"),
    ("wrNone", 1, "none"),
    ("wrAround", 1, "around"),
    ("vaTop", 2, "top"),
    ("vaCenter", 2, "center"),
    ("vaBottom", 2, "bottom"),
];

/// A length the reference's way: `2.17"` or `5.51 cm`.
fn len(twips: i32, inches: bool) -> String {
    if inches {
        let v = format!("{:.2}", f64::from(twips) / 1440.0);
        format!("{}\"", v.trim_end_matches('0').trim_end_matches('.'))
    } else {
        let v = format!("{:.2}", f64::from(twips) / 566.929);
        format!("{} cm", v.trim_end_matches('0').trim_end_matches('.'))
    }
}

/// Twips of a typed length (`2"`, `2 in`, `5 cm`, `50 mm`, `12 pt`; bare: inches or cm).
fn parse(text: &str, inches: bool) -> Option<i32> {
    let s = text.trim().replace(',', ".");
    let split = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let v: f64 = s[..split].trim().parse().ok()?;
    let per = match s[split..].trim() {
        "\"" | "in" => 1440.0,
        "cm" => 566.929,
        "mm" => 56.6929,
        "pt" => 20.0,
        "" if inches => 1440.0,
        "" => 566.929,
        _ => return None,
    };
    Some((v * per).round() as i32)
}

impl DocForm {
    fn pick_state(&self, slot: usize) -> &str {
        self.ids.get(slot).map_or("", String::as_str)
    }

    /// A click on one of the pictures (alignment, wrapping, vertical alignment).
    pub(super) fn tp_canvas(&mut self, id: &str) -> bool {
        let Some(&(_, slot, v)) = PICKS.iter().find(|p| p.0 == id) else {
            return false;
        };
        if self.ids.len() < 3 {
            self.ids.resize(3, String::new());
        }
        self.ids[slot] = v.to_owned();
        true
    }

    /// Dependent boxes follow their check boxes.
    pub(super) fn tp_changed(&mut self, id: &str) {
        match id {
            "tPref" => {
                let on = self.on("tPref");
                self.form.enable("tWidth", on);
                self.form.enable("tMeasure", on);
            }
            "rSpec" => {
                let on = self.on("rSpec");
                self.form.enable("rHeight", on);
                self.form.enable("rIs", on);
            }
            "cPref" => {
                let on = self.on("cPref");
                self.form.enable("cWidth", on);
            }
            _ => {}
        }
    }

    /// OK: the table edits.
    pub(super) fn tp_ok(&mut self) -> FormOut {
        let inches = self.preview.first().is_some_and(|u| u == "in");
        let edit = |e: Value| json!({ "cmd": "tableEdit", "edit": e });
        let mut cmds = Vec::new();
        // bo_model::Alignment / CellVAlign variant names.
        let align = match self.pick_state(0) {
            "center" => "Center",
            "right" => "Right",
            _ => "Left",
        };
        let pct = self.sel("tMeasure") == Some(1);
        let width_text = self.text("tWidth");
        let (width, width_pct) = if !self.on("tPref") {
            (None, None)
        } else if pct {
            let p = width_text
                .trim()
                .trim_end_matches('%')
                .trim()
                .parse::<u32>()
                .ok();
            (None, p)
        } else {
            (parse(&width_text, inches), None)
        };
        cmds.push(edit(json!({
            "kind": "properties", "width": width, "widthPct": width_pct, "align": align,
        })));
        cmds.push(edit(
            json!({ "kind": "wrap", "around": self.pick_state(1) == "around" }),
        ));
        let height = if self.on("rSpec") {
            parse(&self.text("rHeight"), inches)
        } else {
            None
        };
        cmds.push(edit(json!({ "kind": "rowHeight", "twips": height })));
        cmds.push(edit(
            json!({ "kind": "headerRow", "on": self.on("rHeader") }),
        ));
        if self.on("cPref") {
            if let Some(w) = parse(&self.text("cWidth"), inches).filter(|w| *w > 0) {
                cmds.push(edit(json!({ "kind": "columnWidth", "twips": w })));
            }
        }
        let vertical = match self.pick_state(2) {
            "center" => "Center",
            "bottom" => "Bottom",
            _ => "Top",
        };
        let horizontal = self
            .preview
            .get(1)
            .cloned()
            .unwrap_or_else(|| "Left".into());
        cmds.push(edit(json!({
            "kind": "cellAlign", "vertical": vertical, "horizontal": horizontal,
        })));
        FormOut::Run(cmds, true)
    }
}

/// Build the Table Properties dialog for the table at the caret (`c.table`).
pub(super) fn open(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let mut f = DocForm::new(
        DocFormKind::TableProps,
        &tr("docTable.properties", "Table Properties"),
        (564.0, 573.0),
        vec![
            tr("tprops11.table", "Table"),
            tr("tprops11.row", "Row"),
            tr("tprops11.column", "Column"),
            tr("tprops11.cell", "Cell"),
            tr("tprops11.altText", "Alt Text"),
        ],
    );
    let inches = c.inches;
    let tbl = c.table.clone().unwrap_or_default();
    let al = match tbl.align {
        bo_model::Alignment::Center => "center",
        bo_model::Alignment::Right => "right",
        _ => "left",
    };
    let va = match tbl.v_align {
        Some(bo_model::CellVAlign::Center) => "center",
        Some(bo_model::CellVAlign::Bottom) => "bottom",
        _ => "top",
    };
    f.ids = vec![
        al.to_owned(),
        if tbl.wrap { "around" } else { "none" }.to_owned(),
        va.to_owned(),
    ];
    let h_align = match tbl.cell_h_align {
        bo_model::Alignment::Center => "Center",
        bo_model::Alignment::Right => "Right",
        bo_model::Alignment::Justify => "Justify",
        bo_model::Alignment::Left => "Left",
    };
    f.preview = vec![
        if inches { "in" } else { "cm" }.to_owned(),
        h_align.to_owned(),
    ];
    let measure = vec![
        if inches {
            tr("tprops11.inches", "Inches")
        } else {
            tr("tprops11.centimeters", "Centimeters")
        },
        tr("tprops11.percent", "Percent"),
    ];
    let group = |f: &mut DocForm, page: usize, id: &'static str, y: f32, text: &str| {
        f.form.add(
            id,
            Some(page),
            [24.0, y, 524.0, 18.0],
            super::Kind::Group(text.to_owned()),
        );
    };
    let edit = |f: &mut DocForm, page: usize, id: &'static str, r: [f32; 4], v: &str| {
        f.edit_on(page, id, r, v);
    };
    // Table.
    group(&mut f, 0, "tSize", 88.0, &tr("tprops11.size", "Size"));
    let pref = tbl.width > 0 || tbl.width_pct.is_some();
    f.check_on(
        0,
        "tPref",
        [38.0, 116.0, 130.0, 22.0],
        &tr("tprops11.prefWidth", "Preferred width:"),
        pref,
    );
    let wtext = match tbl.width_pct {
        Some(p) => format!("{p}%"),
        None => len(tbl.width.max(0), inches),
    };
    edit(&mut f, 0, "tWidth", [172.0, 114.0, 110.0, 24.0], &wtext);
    f.label_on(
        0,
        "tMeasureL",
        [296.0, 118.0, 80.0, 18.0],
        &tr("tprops11.measureIn", "Measure in:"),
    );
    f.form.add(
        "tMeasure",
        Some(0),
        [379.0, 113.0, 160.0, 26.0],
        super::Kind::Combo {
            items: measure.clone(),
            sel: usize::from(tbl.width_pct.is_some()),
        },
    );
    group(
        &mut f,
        0,
        "tAlign",
        151.0,
        &tr("tprops11.alignment", "Alignment"),
    );
    for (id, x) in [("alLeft", 38.0), ("alCenter", 133.0), ("alRight", 228.0)] {
        f.form
            .add(id, Some(0), [x, 177.0, 82.0, 70.0], super::Kind::Canvas);
    }
    f.label_on(
        0,
        "alLeftL",
        [66.0, 252.0, 40.0, 18.0],
        &tr("tprops11.left", "Left"),
    );
    f.label_on(
        0,
        "alCenterL",
        [152.0, 252.0, 60.0, 18.0],
        &tr("tprops11.center", "Center"),
    );
    f.label_on(
        0,
        "alRightL",
        [253.0, 252.0, 50.0, 18.0],
        &tr("tprops11.right", "Right"),
    );
    f.label_on(
        0,
        "indentL",
        [323.0, 177.0, 150.0, 18.0],
        &tr("tprops11.indent", "Indent from left:"),
    );
    edit(
        &mut f,
        0,
        "tIndent",
        [323.0, 197.0, 130.0, 24.0],
        &len(0, inches),
    );
    f.form.enable("tIndent", false);
    group(
        &mut f,
        0,
        "tWrap",
        278.0,
        &tr("tprops11.textWrapping", "Text wrapping"),
    );
    f.form.add(
        "wrNone",
        Some(0),
        [38.0, 304.0, 82.0, 70.0],
        super::Kind::Canvas,
    );
    f.form.add(
        "wrAround",
        Some(0),
        [133.0, 304.0, 82.0, 70.0],
        super::Kind::Canvas,
    );
    f.label_on(
        0,
        "wrNoneL",
        [62.0, 385.0, 50.0, 18.0],
        &tr("tprops11.none", "None"),
    );
    f.label_on(
        0,
        "wrAroundL",
        [152.0, 385.0, 60.0, 18.0],
        &tr("tprops11.around", "Around"),
    );
    f.button_on(
        0,
        "positioning",
        [427.0, 377.0, 112.0, 28.0],
        &tr("tprops11.positioning", "Positioning..."),
    );
    f.form.enable("positioning", false);
    f.button_on(
        0,
        "borders",
        [254.0, 479.0, 158.0, 28.0],
        &tr("tprops11.borders", "Borders and Shading..."),
    );
    f.form.enable("borders", false);
    f.button_on(
        0,
        "options",
        [427.0, 479.0, 112.0, 28.0],
        &tr("tprops11.options", "Options..."),
    );
    f.form.enable("options", false);
    // Row.
    f.label_on(
        1,
        "rRow",
        [16.0, 88.0, 200.0, 18.0],
        &format!("{} {}:", tr("tprops11.rowN", "Row"), tbl.row + 1),
    );
    f.form.add(
        "rSize",
        Some(1),
        [40.0, 110.0, 508.0, 18.0],
        super::Kind::Group(tr("tprops11.size", "Size")),
    );
    f.check_on(
        1,
        "rSpec",
        [54.0, 137.0, 130.0, 22.0],
        &tr("tprops11.specHeight", "Specify height:"),
        tbl.row_height > 0,
    );
    edit(
        &mut f,
        1,
        "rHeight",
        [178.0, 135.0, 110.0, 24.0],
        &len(tbl.row_height.max(0), inches),
    );
    f.label_on(
        1,
        "rIsL",
        [303.0, 139.0, 95.0, 18.0],
        &tr("tprops11.heightIs", "Row height is:"),
    );
    f.form.add(
        "rIs",
        Some(1),
        [398.0, 134.0, 134.0, 26.0],
        super::Kind::Combo {
            items: vec![
                tr("tprops11.atLeast", "At least"),
                tr("tprops11.exactly", "Exactly"),
            ],
            sel: 0,
        },
    );
    f.form.add(
        "rOpts",
        Some(1),
        [40.0, 174.0, 508.0, 18.0],
        super::Kind::Group(tr("tprops11.optionsG", "Options")),
    );
    f.check_on(
        1,
        "rBreak",
        [54.0, 199.0, 300.0, 22.0],
        &tr("tprops11.allowBreak", "Allow row to break across pages"),
        true,
    );
    f.form.enable("rBreak", false);
    f.check_on(
        1,
        "rHeader",
        [54.0, 224.0, 380.0, 22.0],
        &tr(
            "tprops11.repeatHeader",
            "Repeat as header row at the top of each page",
        ),
        tbl.header_row,
    );
    f.button_on(
        1,
        "prevRow",
        [16.0, 265.0, 112.0, 28.0],
        &tr("tprops11.prevRow", "Previous Row"),
    );
    f.button_on(
        1,
        "nextRow",
        [143.0, 265.0, 112.0, 28.0],
        &tr("tprops11.nextRow", "Next Row"),
    );
    f.form.enable("prevRow", false);
    f.form.enable("nextRow", false);
    // Column.
    f.label_on(
        2,
        "cCol",
        [16.0, 88.0, 200.0, 18.0],
        &format!("{} {}:", tr("tprops11.columnN", "Column"), tbl.col + 1),
    );
    f.form.add(
        "cSize",
        Some(2),
        [40.0, 110.0, 508.0, 18.0],
        super::Kind::Group(tr("tprops11.size", "Size")),
    );
    f.check_on(
        2,
        "cPref",
        [54.0, 137.0, 130.0, 22.0],
        &tr("tprops11.prefWidth", "Preferred width:"),
        tbl.col_width > 0,
    );
    edit(
        &mut f,
        2,
        "cWidth",
        [187.0, 135.0, 110.0, 24.0],
        &len(tbl.col_width.max(0), inches),
    );
    f.label_on(
        2,
        "cMeasureL",
        [312.0, 139.0, 80.0, 18.0],
        &tr("tprops11.measureIn", "Measure in:"),
    );
    f.form.add(
        "cMeasure",
        Some(2),
        [394.0, 134.0, 138.0, 26.0],
        super::Kind::Combo {
            items: vec![measure[0].clone()],
            sel: 0,
        },
    );
    f.button_on(
        2,
        "prevCol",
        [16.0, 183.0, 130.0, 28.0],
        &tr("tprops11.prevCol", "Previous Column"),
    );
    f.button_on(
        2,
        "nextCol",
        [162.0, 183.0, 112.0, 28.0],
        &tr("tprops11.nextCol", "Next Column"),
    );
    f.form.enable("prevCol", false);
    f.form.enable("nextCol", false);
    // Cell.
    group(&mut f, 3, "eSize", 88.0, &tr("tprops11.size", "Size"));
    f.label_on(
        3,
        "eNote",
        [38.0, 116.0, 480.0, 18.0],
        &tr(
            "tprops11.cellWidthNote",
            "The cell's width follows its column (Column tab).",
        ),
    );
    group(
        &mut f,
        3,
        "eVAlign",
        151.0,
        &tr("tprops11.vAlign", "Vertical alignment"),
    );
    for (id, x) in [("vaTop", 38.0), ("vaCenter", 133.0), ("vaBottom", 228.0)] {
        f.form
            .add(id, Some(3), [x, 177.0, 82.0, 70.0], super::Kind::Canvas);
    }
    f.label_on(
        3,
        "vaTopL",
        [66.0, 252.0, 40.0, 18.0],
        &tr("tprops11.top", "Top"),
    );
    f.label_on(
        3,
        "vaCenterL",
        [152.0, 252.0, 60.0, 18.0],
        &tr("tprops11.center", "Center"),
    );
    f.label_on(
        3,
        "vaBottomL",
        [250.0, 252.0, 60.0, 18.0],
        &tr("tprops11.bottom", "Bottom"),
    );
    // Alt Text: the engine keeps no table description yet.
    f.label_on(
        4,
        "aTitleL",
        [24.0, 88.0, 200.0, 18.0],
        &tr("tprops11.title", "Title"),
    );
    edit(&mut f, 4, "aTitle", [24.0, 108.0, 516.0, 24.0], "");
    f.label_on(
        4,
        "aDescL",
        [24.0, 144.0, 200.0, 18.0],
        &tr("tprops11.description", "Description"),
    );
    edit(&mut f, 4, "aDesc", [24.0, 164.0, 516.0, 140.0], "");
    f.form.enable("aTitle", false);
    f.form.enable("aDesc", false);
    f.button(
        "ok",
        [314.0, 532.0, 112.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [441.0, 532.0, 112.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.tp_changed("tPref");
    f.tp_changed("rSpec");
    f.tp_changed("cPref");
    f
}

/// Paint the dialog's pictures (own drawings): a page with a small table placed left, centre
/// or right, text beside it for wrapping around, the cell contents at the top / centre /
/// bottom. The chosen one is framed in the accent blue.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    k: f32,
) -> bool {
    let Some(&(_, slot, v)) = PICKS.iter().find(|x| x.0 == id) else {
        return false;
    };
    let chosen = f.ids.get(slot).is_some_and(|s| s == v);
    let edge = if chosen {
        Rgba(0x00, 0x78, 0xd4, 255)
    } else {
        Rgba(0xad, 0xad, 0xad, 255)
    };
    let bw = if chosen { (2.0 * k).round() } else { 1.0 };
    p.fill_rect(r[0], r[1], r[2], r[3], edge);
    p.fill_rect(
        r[0] + bw,
        r[1] + bw,
        r[2] - 2.0 * bw,
        r[3] - 2.0 * bw,
        Rgba(255, 255, 255, 255),
    );
    let ink = Rgba(0x40, 0x40, 0x40, 255);
    let line =
        |p: &mut dyn Painter, x: f32, y: f32, w: f32| p.fill_rect(x, y, w, (1.5 * k).max(1.0), ink);
    let grid = |p: &mut dyn Painter, x: f32, y: f32| {
        let (w, h) = (20.0 * k, 16.0 * k);
        p.path(
            &[
                PathSeg::M(x, y),
                PathSeg::L(x + w, y),
                PathSeg::L(x + w, y + h),
                PathSeg::L(x, y + h),
                PathSeg::Z,
                PathSeg::M(x + w / 2.0, y),
                PathSeg::L(x + w / 2.0, y + h),
                PathSeg::M(x, y + h / 2.0),
                PathSeg::L(x + w, y + h / 2.0),
            ],
            None,
            Some((Rgba(0, 0, 0, 255), (1.5 * k).max(1.0))),
        );
    };
    let (cx, top) = (r[0] + r[2] / 2.0, r[1] + 14.0 * k);
    match id {
        "alLeft" | "alCenter" | "alRight" | "wrNone" => {
            let gx = match id {
                "alCenter" => cx - 10.0 * k,
                "alRight" => r[0] + r[2] - 34.0 * k,
                _ => r[0] + 14.0 * k,
            };
            grid(p, gx, top);
            for i in 0..3 {
                line(
                    p,
                    gx - 4.0 * k,
                    top + 22.0 * k + i as f32 * 5.0 * k,
                    28.0 * k,
                );
            }
        }
        "wrAround" => {
            let gx = r[0] + 14.0 * k;
            grid(p, gx, top);
            for i in 0..5 {
                let y = top + i as f32 * 4.5 * k;
                line(p, gx + 26.0 * k, y, 22.0 * k);
            }
            for i in 0..2 {
                line(
                    p,
                    gx - 4.0 * k,
                    top + 24.0 * k + i as f32 * 5.0 * k,
                    52.0 * k,
                );
            }
        }
        _ => {
            // A cell with its text at the top, centre or bottom.
            let (bx, by, w, h) = (cx - 20.0 * k, r[1] + 10.0 * k, 40.0 * k, 48.0 * k);
            p.path(
                &[
                    PathSeg::M(bx, by),
                    PathSeg::L(bx + w, by),
                    PathSeg::L(bx + w, by + h),
                    PathSeg::L(bx, by + h),
                    PathSeg::Z,
                ],
                None,
                Some((Rgba(0, 0, 0, 255), (1.5 * k).max(1.0))),
            );
            let ty = match id {
                "vaCenter" => by + h / 2.0 - 6.0 * k,
                "vaBottom" => by + h - 16.0 * k,
                _ => by + 4.0 * k,
            };
            for i in 0..3 {
                line(p, bx + 6.0 * k, ty + i as f32 * 5.0 * k, w - 12.0 * k);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{len, parse};

    // source: the reference shows lengths as 2.17" (inches) and parses typed units.
    #[test]
    fn lengths() {
        assert_eq!(len(3125, true), "2.17\"");
        assert_eq!(len(1440, true), "1\"");
        assert_eq!(parse("2.17\"", true), Some(3125));
        assert_eq!(parse("1", false), Some(567));
        assert_eq!(parse("12 pt", true), Some(240));
        assert_eq!(parse("x", true), None);
    }
}
