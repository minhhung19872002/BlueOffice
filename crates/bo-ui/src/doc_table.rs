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
use bo_render::Rgba;
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
    match OPTIONS.iter().position(|(o, _)| *o == id)? {
        i if i % 2 == 0 => Some(110.0),
        _ => Some(130.0),
    }
}

/// Width (logical px) of a gallery tile (the reference's step 108 px at 125 %).
pub(crate) fn large_w(id: &str) -> Option<f32> {
    LOOK_TILES.iter().any(|(t, _)| *t == id).then_some(84.0)
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
    } else if let Some(part) = id.strip_prefix("table.select.") {
        json!({ "kind": "select", "part": part })
    } else {
        return None;
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
        .chain([("table.select", "docTable.select")]);
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
    let px = |v: f32| (v * scale).round();
    let hovered = ribbon.hovered();
    for (id, _) in OPTIONS {
        let Some(r) = ribbon.rect_of(RibbonHit::Command(id)) else {
            continue;
        };
        if hovered == Some(RibbonHit::Command(id)) {
            p.fill_rect(r[0], r[1], r[2], r[3], t.hover);
        }
        let b = px(13.0);
        let bx = (r[0] + px(4.0)).round();
        let by = (r[1] + (r[3] - b) / 2.0).round();
        crate::spin_box::paint_check(p, [bx, by, b, b], look.option(id), true, t.text);
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
    paint_gallery(p, ribbon, look, t, scale);
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
