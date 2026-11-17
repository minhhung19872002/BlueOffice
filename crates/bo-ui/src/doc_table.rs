//! BlueDoc's table tools beyond the basic buttons, laid out like a desktop word processor's
//! Table Design / Table Layout tabs: the Table Style Options check boxes, a gallery of
//! quick table looks (Bluestar's own presets, previewed as small tables), the Select menu
//! and the nine cell alignment buttons. Commands go to the engine as `tableEdit` JSON
//! (ADR-0008): `look`, `cellAlign`, `select`.
use crate::icons::Icons;
use crate::ribbon::{Ribbon, RibbonHit};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Painter, TextStyle};
use bo_edit::table_tools::LookOptions;
use bo_edit::FormatState;
use bo_engine::edit::Command;
use bo_render::{PathSeg, Rgba};
use serde_json::json;

/// Gallery tiles: command id and engine preset, in gallery order (six fit the window as
/// the reference shows five plus its scroll column).
pub(crate) const LOOK_TILES: [(&str, &str); 6] = [
    ("table.look.plain", "plain"),
    ("table.look.grid", "grid"),
    ("table.look.gridLight", "gridLight"),
    ("table.look.listLight", "listLight"),
    ("table.look.gridBlue", "gridBlue"),
    ("table.look.gridGreen", "gridGreen"),
];

/// Table Style Options check boxes: command id and label key.
pub(crate) const OPTIONS: [(&str, &str); 6] = [
    ("table.opt.headerRow", "docTable.headerRow"),
    ("table.opt.firstColumn", "docTable.firstColumn"),
    ("table.opt.totalRow", "docTable.totalRow"),
    ("table.opt.lastColumn", "docTable.lastColumn"),
    ("table.opt.bandedRows", "docTable.bandedRows"),
    ("table.opt.bandedColumns", "docTable.bandedColumns"),
];

/// The nine alignment buttons: id, vertical (`Top` / `Center` / `Bottom`), horizontal,
/// label key.
pub(crate) const ALIGN: [(&str, &str, &str, &str); 9] = [
    (
        "table.align.topLeft",
        "Top",
        "Left",
        "docTable.alignTopLeft",
    ),
    (
        "table.align.topCenter",
        "Top",
        "Center",
        "docTable.alignTopCenter",
    ),
    (
        "table.align.topRight",
        "Top",
        "Right",
        "docTable.alignTopRight",
    ),
    (
        "table.align.centerLeft",
        "Center",
        "Left",
        "docTable.alignCenterLeft",
    ),
    (
        "table.align.center",
        "Center",
        "Center",
        "docTable.alignCenter",
    ),
    (
        "table.align.centerRight",
        "Center",
        "Right",
        "docTable.alignCenterRight",
    ),
    (
        "table.align.bottomLeft",
        "Bottom",
        "Left",
        "docTable.alignBottomLeft",
    ),
    (
        "table.align.bottomCenter",
        "Bottom",
        "Center",
        "docTable.alignBottomCenter",
    ),
    (
        "table.align.bottomRight",
        "Bottom",
        "Right",
        "docTable.alignBottomRight",
    ),
];

/// Cell Size number boxes: row height, column width (centimetres).
pub(crate) const SPINS: [&str; 2] = ["table.rowHeight", "table.colWidth"];

/// Layout › Paragraph boxes (round 4): Indent Left / Right in the interface unit, Spacing
/// Before / After in points, as the reference's Layout tab.
pub(crate) const PARA_SPINS: [&str; 4] = [
    "layout.indentLeft",
    "layout.indentRight",
    "layout.spaceBefore",
    "layout.spaceAfter",
];

/// The `&'static` id of a ribbon number box.
pub(crate) fn spin_id(id: &str) -> Option<&'static str> {
    SPINS
        .iter()
        .chain(PARA_SPINS.iter())
        .find(|s| **s == id)
        .copied()
}

fn in_points(id: &str) -> bool {
    id == PARA_SPINS[2] || id == PARA_SPINS[3]
}

/// One arrow step of box `id` (twips): 0.1 unit, or 6 pt for spacing.
fn step_of(id: &str, units: Units) -> i32 {
    if in_points(id) {
        120
    } else {
        units.step()
    }
}

/// The number of box `id` while typing (`0,35` / `6`).
pub(crate) fn spin_number(id: &str, units: Units, twips: i32) -> String {
    if in_points(id) {
        format!("{}", twips / 20)
    } else {
        units.number(twips)
    }
}

/// The value of box `id` as shown (`0,35 cm` / `6 pt`).
pub(crate) fn spin_text(id: &str, units: Units, twips: i32) -> String {
    if in_points(id) {
        format!("{} pt", twips / 20)
    } else {
        units.fmt(twips)
    }
}

/// A typed value of box `id` in twips.
pub(crate) fn spin_parse(id: &str, units: Units, text: &str) -> Option<i32> {
    if in_points(id) {
        let t = text.trim().trim_end_matches("pt").trim().replace(',', ".");
        let v: f64 = t.parse().ok()?;
        return (v.is_finite() && (0.0..=1584.0).contains(&v)).then(|| (v * 20.0).round() as i32);
    }
    units.parse(text)
}

/// Measurement unit of the interface, as the reference follows the region's system:
/// inches (US) or centimetres, with the region's decimal separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Units {
    /// Inches (else centimetres).
    pub inches: bool,
    /// Decimal separator.
    pub decimal: char,
}

impl Default for Units {
    fn default() -> Self {
        Units {
            inches: false,
            decimal: ',',
        }
    }
}

impl Units {
    fn per_unit(self) -> f64 {
        if self.inches {
            1440.0
        } else {
            1440.0 / 2.54
        }
    }

    /// One step of the arrows: 0.1 of the unit, in twips.
    pub(crate) fn step(self) -> i32 {
        (self.per_unit() / 10.0).round() as i32
    }

    /// The number alone, two decimals (`0.35`, `1,27`).
    pub(crate) fn number(self, twips: i32) -> String {
        format!("{:.2}", f64::from(twips) / self.per_unit()).replace('.', &self.decimal.to_string())
    }

    /// As the boxes show it: `0.35"` or `1,27 cm`.
    pub(crate) fn fmt(self, twips: i32) -> String {
        let n = self.number(twips);
        if self.inches {
            format!("{n}\"")
        } else {
            format!("{n} cm")
        }
    }

    /// A typed value (either separator, unit optional) in twips.
    pub(crate) fn parse(self, text: &str) -> Option<i32> {
        let t = text
            .trim()
            .trim_end_matches("cm")
            .trim_end_matches('"')
            .trim_end_matches("in")
            .trim()
            .replace(',', ".");
        let v: f64 = t.parse().ok()?;
        let tw = v * self.per_unit();
        (v.is_finite() && (0.0..31_680.0).contains(&tw)).then(|| tw.round() as i32)
    }
}

/// The value box of Cell Size item `r` (right of its icon).
pub(crate) fn spin_rect(r: [f32; 4], scale: f32) -> [f32; 4] {
    let px = |v: f32| (v * scale).round();
    let h = px(24.0).min(r[3]);
    [
        r[0] + px(22.0),
        (r[1] + (r[3] - h) / 2.0).round(),
        r[2] - px(24.0),
        h,
    ]
}

/// Current value (twips) of Cell Size box `id`; an automatic row shows its measured
/// height (`measured`), as the reference does.
pub(crate) fn spin_value(id: &str, f: &FormatState, measured: Option<i32>) -> i32 {
    match PARA_SPINS.iter().position(|s| *s == id) {
        Some(0) => return f.indent_left,
        Some(1) => return f.indent_right,
        Some(2) => return f.space_before,
        Some(_) => return f.space_after,
        None => {}
    }
    let Some(t) = f.table.as_ref() else { return 0 };
    if id == SPINS[0] {
        if t.row_height > 0 {
            t.row_height
        } else {
            measured.unwrap_or(0)
        }
    } else {
        t.col_width
    }
}

/// The engine command setting Cell Size box `id` to `twips` (0 row height: automatic).
pub(crate) fn spin_command(id: &str, twips: i32) -> Option<Command> {
    // ECMA-376 Part 1, §17.3.1.12 ind, §17.3.1.33 spacing.
    let para = match PARA_SPINS.iter().position(|s| *s == id) {
        Some(0) => Some(bo_edit::ParaFormat::IndentLeft { twips }),
        Some(1) => Some(bo_edit::ParaFormat::IndentRight { twips }),
        Some(2) => Some(bo_edit::ParaFormat::SpaceBefore { twips }),
        Some(_) => Some(bo_edit::ParaFormat::SpaceAfter { twips }),
        None => None,
    };
    if let Some(format) = para {
        return Some(Command::FormatParagraph { format });
    }
    let edit = if id == SPINS[0] {
        json!({ "kind": "rowHeight", "twips": (twips > 0).then_some(twips) })
    } else {
        json!({ "kind": "columnWidth", "twips": twips.max(144) })
    };
    crate::catalog::from_json(json!({ "cmd": "tableEdit", "edit": edit }))
}

/// A press on Cell Size box `id` at (`x`, `y`): an arrow steps the value (`Some`
/// command), the field starts typing (`Err` with the text to edit).
pub(crate) fn spin_press(
    id: &'static str,
    r: [f32; 4],
    v: i32,
    units: Units,
    scale: f32,
    (x, y): (f32, f32),
) -> Result<Option<Command>, crate::spin_box::SpinEdit> {
    let step = step_of(id, units);
    let rect = if PARA_SPINS.contains(&id) {
        para_rect(r, scale)
    } else {
        spin_rect(r, scale)
    };
    match crate::spin_box::spin_hit(rect, scale, x, y) {
        Some(crate::spin_box::SpinHit::Up) => Ok(spin_command(id, v + step)),
        Some(crate::spin_box::SpinHit::Down) => Ok(spin_command(id, (v - step).max(0))),
        _ => Err(crate::spin_box::SpinEdit {
            id,
            text: spin_number(id, units, v),
        }),
    }
}

/// Step a value being typed (Up / Down keys).
pub(crate) fn spin_step(id: &str, text: &str, up: bool, units: Units) -> Option<Command> {
    let v = spin_parse(id, units, text)?;
    let step = step_of(id, units);
    spin_command(id, if up { v + step } else { (v - step).max(0) })
}

/// Draw the Cell Size boxes over their placeholders: a small icon, then the value (or
/// the text being typed with a caret).
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_spins(
    p: &mut dyn Painter,
    ribbon: &Ribbon,
    f: &FormatState,
    (units, measured): (Units, Option<i32>),
    edit: Option<&crate::spin_box::SpinEdit>,
    t: &Theme,
    text: &dyn Fn(&str) -> f32,
    scale: f32,
) {
    let px = |v: f32| (v * scale).round();
    for (i, id) in SPINS.iter().enumerate() {
        let Some(r) = ribbon.rect_of(RibbonHit::Command(id)) else {
            continue;
        };
        p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
        // Icon: a two-headed arrow beside a bar (vertical for height, horizontal for width).
        let (cx, cy, k) = (r[0] + px(9.0), r[1] + r[3] / 2.0, px(6.0));
        let ink = Rgba(0x66, 0x69, 0x6e, 255);
        let segs = if i == 0 {
            vec![
                PathSeg::M(cx, cy - k),
                PathSeg::L(cx, cy + k),
                PathSeg::M(cx - 2.5 * scale, cy - k + 2.5 * scale),
                PathSeg::L(cx, cy - k),
                PathSeg::L(cx + 2.5 * scale, cy - k + 2.5 * scale),
                PathSeg::M(cx - 2.5 * scale, cy + k - 2.5 * scale),
                PathSeg::L(cx, cy + k),
                PathSeg::L(cx + 2.5 * scale, cy + k - 2.5 * scale),
            ]
        } else {
            vec![
                PathSeg::M(cx - k, cy),
                PathSeg::L(cx + k, cy),
                PathSeg::M(cx - k + 2.5 * scale, cy - 2.5 * scale),
                PathSeg::L(cx - k, cy),
                PathSeg::L(cx - k + 2.5 * scale, cy + 2.5 * scale),
                PathSeg::M(cx + k - 2.5 * scale, cy - 2.5 * scale),
                PathSeg::L(cx + k, cy),
                PathSeg::L(cx + k - 2.5 * scale, cy + 2.5 * scale),
            ]
        };
        p.path(&segs, None, Some((ink, scale.max(1.0))));
        let editing = edit.filter(|e| e.id == *id);
        let shown = match editing {
            Some(e) => e.text.clone(),
            None => units.fmt(spin_value(id, f, measured)),
        };
        let style = TextStyle {
            size: px(12.0),
            bold: false,
        };
        crate::spin_box::paint_spin(
            p,
            spin_rect(r, scale),
            scale,
            &crate::spin_box::BoxLook {
                text: &shown,
                style,
                ink: t.text,
                enabled: f.table.is_some(),
                caret: editing.map(|e| text(&e.text)),
                hover: None,
            },
        );
    }
}

/// The value box of a Layout › Paragraph item `r` (after its icon and label).
pub(crate) fn para_rect(r: [f32; 4], scale: f32) -> [f32; 4] {
    let px = |v: f32| (v * scale).round();
    let h = px(22.0).min(r[3]);
    [
        r[0] + px(70.0),
        (r[1] + (r[3] - h) / 2.0).round(),
        r[2] - px(72.0),
        h,
    ]
}

/// Draw the Layout › Paragraph boxes over their placeholders: icon, "Left:" label, box.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_para_spins(
    p: &mut dyn Painter,
    ribbon: &Ribbon,
    f: &FormatState,
    units: Units,
    edit: Option<&crate::spin_box::SpinEdit>,
    (t, icons, strings): (&Theme, &Icons, &Strings),
    text: &dyn Fn(&str) -> f32,
    scale: f32,
) {
    let px = |v: f32| (v * scale).round();
    let labels = [
        ("docR4.left", "Left:"),
        ("docR4.right", "Right:"),
        ("docR4.before", "Before:"),
        ("docR4.after", "After:"),
    ];
    let icon_of = [
        "format.indent",
        "format.outdent",
        "format.lineSpacing",
        "format.lineSpacing",
    ];
    // Column headings over the boxes (Indent, Spacing), as the reference.
    for id in crate::doc_r4_fit::HEADINGS {
        let Some(r) = ribbon.rect_of(RibbonHit::Command(id)) else {
            continue;
        };
        p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
        let style = TextStyle {
            size: px(12.0),
            bold: false,
        };
        p.text(
            strings.get(&format!("commands.{id}")),
            r[0] + px(2.0),
            r[1] + (r[3] - px(16.0)) / 2.0,
            style,
            t.text,
        );
    }
    for (i, id) in PARA_SPINS.iter().enumerate() {
        let Some(r) = ribbon.rect_of(RibbonHit::Command(id)) else {
            continue;
        };
        p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
        let s = px(16.0);
        icons.draw(
            p,
            icon_of[i],
            r[0] + px(2.0),
            (r[1] + (r[3] - s) / 2.0).round(),
            s,
            crate::icons::IconColors {
                ink: t.text,
                accent: t.accent,
                on: t.accent_on,
                tinted: true,
            },
        );
        let style = TextStyle {
            size: px(12.0),
            bold: false,
        };
        let label = {
            let v = strings.get(labels[i].0);
            if v == labels[i].0 {
                labels[i].1
            } else {
                v
            }
        };
        p.text(
            label,
            r[0] + px(22.0),
            r[1] + (r[3] - px(16.0)) / 2.0,
            style,
            t.text,
        );
        let editing = edit.filter(|e| e.id == *id);
        let shown = match editing {
            Some(e) => e.text.clone(),
            None => spin_text(id, units, spin_value(id, f, None)),
        };
        crate::spin_box::paint_spin(
            p,
            para_rect(r, scale),
            scale,
            &crate::spin_box::BoxLook {
                text: &shown,
                style,
                ink: t.text,
                enabled: true,
                caret: editing.map(|e| text(&e.text)),
                hover: None,
            },
        );
    }
}

/// Table commands that open a dialog of the window (Properties, Cell Margins).
pub(crate) const DIALOGS: [&str; 2] = ["table.properties", "table.cellMargins"];

/// Header & Footer options drawn as check boxes.
pub(crate) const HF_CHECKS: [&str; 2] = ["hf.differentFirst", "hf.differentOddEven"];

/// Select menu entries.
pub(crate) const SELECT: [(&str, &str); 4] = [
    ("table.select.cell", "docTable.selectCell"),
    ("table.select.column", "docTable.selectColumn"),
    ("table.select.row", "docTable.selectRow"),
    ("table.select.table", "docTable.selectTable"),
];

/// Width (logical px) of the check box items, measured on the reference at 125 %: the
/// first column 138 px ("Banded Rows" with its box), the second 162 px ("Banded Columns").
pub(crate) fn labeled_w(id: &str) -> Option<f32> {
    if let Some(w) = crate::doc_r3_menus::labeled_w(id) {
        return Some(w);
    }
    // Header & Footer options are check boxes too (reference: 224 px at 125 % for
    // "Different Odd & Even Pages").
    if HF_CHECKS.contains(&id) {
        return Some(180.0);
    }
    // Cell Size boxes: icon and a number box (reference: 140 px at 125 %).
    if SPINS.contains(&id) {
        return Some(112.0);
    }
    // Layout › Paragraph boxes: icon, label, number box (reference: 190 px at 125 %); the
    // Indent / Spacing headings above them take the same column.
    if PARA_SPINS.contains(&id) || crate::doc_r4_fit::HEADINGS.contains(&id) {
        return Some(152.0);
    }
    match OPTIONS.iter().position(|(o, _)| *o == id)? {
        i if i % 2 == 0 => Some(110.0),
        _ => Some(130.0),
    }
}

/// Large buttons placed after their group's rows: Text Direction follows the nine
/// alignment buttons, as on the reference.
pub(crate) fn trailing(id: &str) -> bool {
    matches!(
        id,
        "table.textDirection" | "table.cellMargins" | "view.switchWindows"
    )
}

/// Width (logical px) of a gallery tile (the reference's step 108 px at 125 %).
pub(crate) fn large_w(id: &str) -> Option<f32> {
    LOOK_TILES
        .iter()
        .any(|(t, _)| *t == id)
        .then_some(84.0)
        .or_else(|| crate::doc_design::large_w(id))
}

/// The quick look last applied and the style options (the reference keeps them in the
/// table's style; here they belong to the window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableLook {
    pub(crate) preset: &'static str,
    pub(crate) options: LookOptions,
}

impl Default for TableLook {
    fn default() -> Self {
        TableLook {
            preset: "grid",
            options: LookOptions::default(),
        }
    }
}

impl TableLook {
    /// Whether style option `id` is ticked.
    pub(crate) fn option(&self, id: &str) -> bool {
        let o = &self.options;
        match id {
            "table.opt.headerRow" => o.header_row,
            "table.opt.firstColumn" => o.first_column,
            "table.opt.totalRow" => o.total_row,
            "table.opt.lastColumn" => o.last_column,
            "table.opt.bandedRows" => o.banded_rows,
            "table.opt.bandedColumns" => o.banded_columns,
            _ => false,
        }
    }

    /// The engine command of a look tile or style option (which updates `self`).
    pub(crate) fn command(&mut self, id: &str) -> Option<Command> {
        if let Some((_, preset)) = LOOK_TILES.iter().find(|(t, _)| *t == id) {
            self.preset = preset;
        } else if id.starts_with("table.opt.") {
            let o = &mut self.options;
            let flag = match id {
                "table.opt.headerRow" => &mut o.header_row,
                "table.opt.firstColumn" => &mut o.first_column,
                "table.opt.totalRow" => &mut o.total_row,
                "table.opt.lastColumn" => &mut o.last_column,
                "table.opt.bandedRows" => &mut o.banded_rows,
                "table.opt.bandedColumns" => &mut o.banded_columns,
                _ => return None,
            };
            *flag = !*flag;
        } else {
            return None;
        }
        crate::catalog::from_json(json!({
            "cmd": "tableEdit",
            "edit": { "kind": "look", "preset": self.preset, "options": self.options },
        }))
    }
}

/// The engine command of an alignment button or Select menu entry.
pub(crate) fn command(id: &str) -> Option<Command> {
    let edit = if let Some((_, v, h, _)) = ALIGN.iter().find(|a| a.0 == id) {
        json!({ "kind": "cellAlign", "vertical": v, "horizontal": h })
    } else {
        let part = id.strip_prefix("table.select.")?;
        json!({ "kind": "select", "part": part })
    };
    crate::catalog::from_json(json!({ "cmd": "tableEdit", "edit": edit }))
}

/// The alignment button showing the caret cell's alignment.
pub(crate) fn align_on(f: &FormatState) -> Option<&'static str> {
    let t = f.table.as_ref()?;
    let v = match t.v_align {
        Some(bo_model::CellVAlign::Center) => "Center",
        Some(bo_model::CellVAlign::Bottom) => "Bottom",
        _ => "Top",
    };
    let h = match t.cell_h_align {
        bo_model::Alignment::Center => "Center",
        bo_model::Alignment::Right => "Right",
        _ => "Left",
    };
    ALIGN.iter().find(|a| a.1 == v && a.2 == h).map(|a| a.0)
}

/// Names of the new commands (tooltips, labels, menu entries).
pub(crate) fn install_strings(s: &mut Strings) {
    let pairs = OPTIONS
        .iter()
        .chain(SELECT.iter())
        .copied()
        .chain(ALIGN.iter().map(|a| (a.0, a.3)))
        .chain(
            LOOK_TILES
                .iter()
                .zip([
                    "docTable.lookPlain",
                    "docTable.lookGrid",
                    "docTable.lookGridLight",
                    "docTable.lookListLight",
                    "docTable.lookGridBlue",
                    "docTable.lookGridGreen",
                ])
                .map(|((id, _), k)| (*id, k)),
        )
        .chain([
            ("table.select", "docTable.select"),
            ("table.properties", "docTable.properties"),
            ("table.splitTable", "docTable.splitTable"),
            ("table.cellMargins", "docTable.cellMargins"),
            ("view.readMode", "docTable.readMode"),
            ("table.rowHeight", "docTable.rowHeight"),
            ("table.colWidth", "docTable.colWidth"),
            // Title Case labels as on the reference's Table Layout tab.
            ("table.insertRowAbove", "docTable.insertRowAbove"),
            ("table.insertRowBelow", "docTable.insertRowBelow"),
            ("table.insertColumnLeft", "docTable.insertColumnLeft"),
            ("table.insertColumnRight", "docTable.insertColumnRight"),
            ("table.mergeCells", "docTable.mergeCells"),
            ("table.splitCell", "docTable.splitCells"),
            ("table.distributeRows", "docTable.distributeRows"),
            ("table.distributeColumns", "docTable.distributeColumns"),
            ("table.textDirection", "docTable.textDirection"),
            ("table.data", "docTable.data"),
            ("table.headerRow", "docTable.repeatHeaderRows"),
            ("table.toText", "docTable.convertToText"),
        ]);
    for (id, key) in pairs {
        let v = s.get(key).to_owned();
        if v != key {
            s.set(&format!("commands.{id}"), &v);
        }
    }
    for (group, key) in [
        ("ribbon.groups.tableStyleOptions", "docTable.styleOptions"),
        ("ribbon.groups.tableStyles", "docTable.tableStyles"),
        ("ribbon.groups.table", "docTable.table"),
        ("ribbon.groups.cellAlignment", "docTable.alignment"),
        ("ribbon.groups.rowsColumns", "docTable.rowsColumns"),
        ("ribbon.groups.cellSize", "docTable.cellSize"),
    ] {
        let v = s.get(key).to_owned();
        if v != key {
            s.set(group, &v);
        }
    }
}

/// Icons: the nine alignment buttons (three lines placed in a cell frame) and Select
/// (an arrow over a grid) — Bluestar's own drawings.
pub(crate) fn install_icons(icons: &mut Icons) {
    for (id, v, h, _) in ALIGN {
        let y = match v {
            "Top" => 6.0,
            "Center" => 10.0,
            _ => 14.0,
        };
        let line = |dy: f32, w: f32| {
            let x = match h {
                "Left" => 6.0,
                "Center" => 12.0 - w / 2.0,
                _ => 18.0 - w,
            };
            format!("M{x} {}h{w}", y + dy)
        };
        let d = format!(
            "M3 3h18v18h-18Z{}{}{}",
            line(0.0, 12.0),
            line(2.0, 8.0),
            line(4.0, 12.0)
        );
        icons.add_solid(
            id,
            &[
                ("M3 3h18v18h-18Z", Some(Rgba(255, 255, 255, 255))),
                (&d, None),
            ],
        );
    }
    if !icons.has("table.select") {
        icons.add_solid(
            "table.select",
            &[
                ("M3 4h14v12h-14Z", Some(Rgba(255, 255, 255, 255))),
                ("M3 4h14v12h-14ZM3 8h14M3 12h14M8 4v12M12 4v12", None),
                (
                    "M14 11l7 5l-3 0.6l2 3.4l-1.6 0.9l-2-3.4l-2.4 2Z",
                    Some(crate::icons::ICON_BLUE),
                ),
            ],
        );
    }
}

/// Draw the check boxes and gallery previews over their ribbon placeholders.
pub(crate) fn paint(
    p: &mut dyn Painter,
    ribbon: &Ribbon,
    look: &TableLook,
    t: &Theme,
    strings: &Strings,
    scale: f32,
) {
    let checks: Vec<(&'static str, bool)> = OPTIONS
        .iter()
        .map(|(id, _)| (*id, look.option(id)))
        .collect();
    paint_checks(p, ribbon, &checks, t, strings, scale);
    paint_gallery(p, ribbon, look, t, scale);
}

/// Draw check boxes (ticked or not) with their labels over the ribbon placeholders of
/// `items`, in the reference's style (box 13 px at 100 %, label after a 5 px gap).
pub(crate) fn paint_checks(
    p: &mut dyn Painter,
    ribbon: &Ribbon,
    items: &[(&'static str, bool)],
    t: &Theme,
    strings: &Strings,
    scale: f32,
) {
    let px = |v: f32| (v * scale).round();
    let hovered = ribbon.hovered();
    for &(id, on) in items {
        let Some(r) = ribbon.rect_of(RibbonHit::Command(id)) else {
            continue;
        };
        // Cover what the ribbon drew for the placeholder (toggled fill).
        p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
        if hovered == Some(RibbonHit::Command(id)) {
            p.fill_rect(r[0], r[1], r[2], r[3], t.hover);
        }
        let b = px(13.0);
        let bx = (r[0] + px(4.0)).round();
        let by = (r[1] + (r[3] - b) / 2.0).round();
        crate::spin_box::paint_check(p, [bx, by, b, b], on, true, t.text);
        let style = TextStyle {
            size: px(12.0),
            bold: false,
        };
        p.text(
            strings.get(&format!("commands.{id}")),
            bx + b + px(5.0),
            (r[1] + (r[3] - style.size * 1.33) / 2.0).round(),
            style,
            t.text,
        );
    }
}

fn hex(s: &str) -> Rgba {
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// The quick looks as small tables in one white gallery box (reference: frame 210,
/// selected tile framed 136).
fn paint_gallery(p: &mut dyn Painter, ribbon: &Ribbon, look: &TableLook, t: &Theme, scale: f32) {
    let rect = |id: &'static str| ribbon.rect_of(RibbonHit::Command(id));
    let (Some(first), Some(last)) = (rect(LOOK_TILES[0].0), rect(LOOK_TILES[5].0)) else {
        return;
    };
    let px = |v: f32| (v * scale).round();
    let grey = |v: u8| Rgba(v, v, v, 255);
    let top = first[1] + px(4.0);
    let h = first[3] - px(8.0);
    let (left, right) = (first[0] - px(2.0), last[0] + last[2] + px(2.0));
    // Cover the placeholder buttons, then the gallery box.
    p.fill_rect(left - 1.0, first[1], right - left + 2.0, first[3], t.ribbon);
    p.fill_rect(left, top, right - left, h, grey(210));
    p.fill_rect(
        left + 1.0,
        top + 1.0,
        right - left - 2.0,
        h - 2.0,
        grey(255),
    );
    let hovered = ribbon.hovered();
    for (id, preset) in LOOK_TILES {
        let Some(r) = rect(id) else { continue };
        let Some(c) = bo_edit::table_tools::look_preview(preset) else {
            continue;
        };
        let (tx, ty, tw, th) = (r[0] + px(2.0), top + px(3.0), r[2] - px(4.0), h - px(6.0));
        if look.preset == preset {
            p.fill_rect(tx, ty, tw, th, grey(136));
            p.fill_rect(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, grey(210));
        } else if hovered == Some(RibbonHit::Command(id)) {
            p.fill_rect(tx, ty, tw, th, grey(230));
        }
        // A 5 × 5 table: header row, first column and every other row filled as the
        // current options say.
        let (cols, rows) = (5usize, 5usize);
        let (gx, gy) = (tx + px(5.0), ty + px(5.0));
        let (cw, ch) = (
            ((tw - px(10.0)) / cols as f32).floor(),
            ((th - px(10.0)) / rows as f32).floor(),
        );
        let o = look.options;
        for row in 0..rows {
            for col in 0..cols {
                let header = (o.header_row && row == 0) || (o.total_row && row + 1 == rows);
                let edge = (o.first_column && col == 0) || (o.last_column && col + 1 == cols);
                let body_r = row - usize::from(o.header_row && row > 0);
                let body_c = col - usize::from(o.first_column && col > 0);
                let band =
                    (o.banded_rows && body_r % 2 == 0) || (o.banded_columns && body_c % 2 == 0);
                let fill = if header || edge {
                    c.header.or(c.band)
                } else if band {
                    c.band
                } else {
                    None
                };
                if let Some(f) = fill {
                    p.fill_rect(gx + col as f32 * cw, gy + row as f32 * ch, cw, ch, hex(f));
                }
            }
        }
        if let Some(b) = c.border {
            let ink = hex(b);
            let (w, hh) = (cw * cols as f32, ch * rows as f32);
            for row in 0..=rows {
                p.fill_rect(gx, gy + row as f32 * ch, w + 1.0, 1.0, ink);
            }
            if !c.horizontal_only {
                for col in 0..=cols {
                    p.fill_rect(gx + col as f32 * cw, gy, 1.0, hh, ink);
                }
            }
        } else {
            // Plain: dotted guides only, as gridlines show a borderless table.
            let ink = grey(200);
            for row in 0..=rows {
                let mut x = 0.0;
                while x < cw * cols as f32 {
                    p.fill_rect(gx + x, gy + row as f32 * ch, 1.0, 1.0, ink);
                    x += 2.0;
                }
            }
        }
    }
}
