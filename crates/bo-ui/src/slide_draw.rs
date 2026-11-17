//! BlueSlide's Draw tab, round 17 (N6), laid out as the reference presentation app's at
//! 1500 px: Tools (Eraser ▾, Lasso Select), Undo (Undo / Redo stacked), Pens (a gallery of
//! tall pen pictures without labels and Add ▾), Stencils (Ruler), Convert (Ink to Shape,
//! Ink to Math — greyed: there is no handwriting recogniser for formulas), Replay (Ink
//! Replay).
//!
//! - A pen is chosen with one click; a click on the pen in use opens its menu (thickness,
//!   colour), as on the reference. Add ▾ appends a pen, pencil or highlighter (up to
//!   [`MAX_PENS`]). The pens are a session setting.
//! - The pictures are our own drawings (the slanted pen of BlueSlide's icon family, drawn
//!   large, over a bar of the ink's colour and thickness), not the reference's pens.
use super::SlideWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::slide_pickers::PickKind;
use crate::Painter;
use bo_engine::slide_edit::ink::EraseMode;
use bo_render::{PathSeg, Rgba};

/// Kinds of pens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PenKind {
    Pen,
    Pencil,
    Highlighter,
}

/// One pen of the gallery.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PenSlot {
    pub(crate) id: &'static str,
    pub(crate) kind: PenKind,
    /// `RRGGBB`.
    pub(crate) color: String,
    /// Thickness in millimetres.
    pub(crate) mm: f64,
}

/// Ribbon ids of the pen slots, the first six shown from the start.
pub(crate) const PEN_IDS: [&str; 9] = [
    "draw.pen",
    "draw.red",
    "draw.pencil",
    "draw.highlight",
    "draw.blue",
    "draw.green",
    "draw.pen7",
    "draw.pen8",
    "draw.pen9",
];

/// Most pens in the gallery.
pub(crate) const MAX_PENS: usize = PEN_IDS.len();

/// Thicknesses offered (mm) for pens / pencils and highlighters.
const PEN_MM: [f64; 5] = [0.25, 0.35, 0.5, 0.7, 1.0];
const HIGHLIGHT_MM: [f64; 5] = [2.0, 4.0, 6.0, 8.0, 10.0];

/// Commands of the tab handled here.
pub const COMMANDS: [&str; 6] = [
    "draw.inkHelp",
    "draw.add",
    "draw.ruler",
    "draw.inkToMath",
    "draw.replay",
    "draw.eraser",
];

/// The gallery the Draw tab starts with (own colours).
pub(crate) fn default_pens() -> Vec<PenSlot> {
    let pen = |id, kind, color: &str, mm| PenSlot {
        id,
        kind,
        color: color.to_owned(),
        mm,
    };
    vec![
        pen("draw.pen", PenKind::Pen, "000000", 0.5),
        pen("draw.red", PenKind::Pen, "E71224", 0.5),
        pen("draw.pencil", PenKind::Pencil, "595959", 0.5),
        pen("draw.highlight", PenKind::Highlighter, "FFFF00", 6.0),
        pen("draw.blue", PenKind::Pen, "0F5EAA", 0.5),
        pen("draw.green", PenKind::Pen, "1E8A3C", 0.5),
    ]
}

/// Points of a thickness in millimetres.
fn mm_pt(mm: f64) -> f64 {
    mm * 72.0 / 25.4
}

impl PenSlot {
    /// The ink tool drawing with this pen.
    pub(crate) fn tool(&self) -> crate::slide_view::InkTool {
        crate::slide_view::InkTool::Pen {
            color: self.color.clone(),
            width: mm_pt(self.mm),
            highlighter: self.kind == PenKind::Highlighter,
        }
    }
}

/// The Draw tab with `pens` pen slots.
pub(crate) fn draw_tab(pens: usize) -> Tab {
    use Item::{Button as B, Large as L};
    Tab {
        label: "slideNative.draw",
        groups: vec![
            Group {
                label: "slideNative.tools",
                large: vec![L("draw.eraser"), L("draw.lasso")],
                rows: vec![],
            },
            Group {
                label: "slideNative.undoGroup",
                large: vec![],
                rows: vec![vec![B("edit.undo")], vec![B("edit.redo")]],
            },
            Group {
                label: "slideNative.pens",
                large: PEN_IDS
                    .iter()
                    .copied()
                    .take(pens.clamp(1, MAX_PENS))
                    .map(L)
                    .chain(std::iter::once(L("draw.add")))
                    .collect(),
                rows: vec![],
            },
            Group {
                label: "slideNative.stencils",
                large: vec![L("draw.ruler")],
                rows: vec![],
            },
            Group {
                label: "slideNative.convert",
                large: vec![L("draw.inkToShape"), L("draw.inkToMath")],
                rows: vec![],
            },
            Group {
                label: "slideNative.replay",
                large: vec![L("draw.replay")],
                rows: vec![],
            },
            // Ink Help, as the reference's last group (greyed: no help pages yet).
            Group {
                label: "slideNative.helpGroup",
                large: vec![L("draw.inkHelp")],
                rows: vec![],
            },
        ],
    }
}

/// Widths of the pen tiles: reference pitch about 72 device px at 125 %.
pub(crate) fn large_width(id: &str) -> Option<f32> {
    PEN_IDS.contains(&id).then_some(54.4)
}

/// Name the commands (pen tiles have no label, only a tooltip).
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in [
        ("draw.add", "addPen"),
        ("draw.ruler", "ruler"),
        ("draw.inkToMath", "inkToMath"),
        ("draw.replay", "inkReplay"),
        ("draw.inkHelp", "inkHelp"),
    ] {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
    for id in PEN_IDS {
        s.set(&format!("commands.{id}"), "");
    }
    set_tips(s, &default_pens());
}

/// Tooltips of the pen tiles: kind, colour code and thickness.
fn set_tips(s: &mut crate::strings::Strings, pens: &[PenSlot]) {
    for pen in pens {
        let key = match pen.kind {
            PenKind::Pen => "slideNative.add_pen",
            PenKind::Pencil => "slideNative.add_pencil",
            PenKind::Highlighter => "slideNative.add_highlighter",
        };
        let tip = format!("{}: #{}, {} mm", s.get(key), pen.color, pen.mm);
        s.set(&format!("commands.tip.{}", pen.id), &tip);
    }
}

/// Own filled drawings for Undo / Redo (curved arrows), Ruler (a slanted straight-edge with
/// ticks), Ink to Math (a scribble turning into a root sign), Ink Replay (a stroke with a
/// play button) and Add (a plus). Pen tiles are drawn by [`SlideWindow::paint_pens`].
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    let blue = crate::icons::ICON_BLUE;
    let green = Rgba(0x3f, 0xa3, 0x4d, 255);
    let wood = Rgba(0xe9, 0xd8, 0xa6, 255);
    if !icons.has("edit.undo") {
        icons.add_solid(
            "edit.undo",
            &[("M9 5l-5 4l5 4M4 9h10a6 6 0 0 1 0 12h-4", None)],
        );
    }
    if !icons.has("edit.redo") {
        icons.add_solid(
            "edit.redo",
            &[("M15 5l5 4l-5 4M20 9h-10a6 6 0 0 0 0 12h4", None)],
        );
    }
    icons.add_solid(
        "draw.ruler",
        &[
            ("M2 17l15-15l5 5l-15 15Z", Some(wood)),
            (
                "M2 17l15-15l5 5l-15 15ZM6 13l2 2M9 10l3 3M12 7l2 2M15 4l3 3",
                None,
            ),
        ],
    );
    icons.add_solid(
        "draw.inkToMath",
        &[
            ("M2 9c2-4 3 4 5 0", None),
            ("M10 14l2-1l3 7l4-15h4", None),
            ("M8 17l2 2l-2 2Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "draw.replay",
        &[
            ("M2 8c3-5 6 3 9-1s4 1 6-1", None),
            ("M10 17a6 6 0 1 0 12 0a6 6 0 1 0 -12 0Z", Some(green)),
            ("M14.5 14l5 3l-5 3Z", Some(Rgba(0xff, 0xff, 0xff, 255))),
        ],
    );
    icons.add_solid(
        "draw.inkHelp",
        &[
            ("M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0Z", Some(blue)),
            ("M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 .8-1 1.5v.7", None),
            ("M11 16.2h2v2h-2Z", Some(Rgba(255, 255, 255, 255))),
        ],
    );
    icons.add_solid(
        "draw.add",
        &[(
            "M10.5 4h3v6.5h6.5v3h-6.5v6.5h-3v-6.5h-6.5v-3h6.5Z",
            Some(green),
        )],
    );
    for id in PEN_IDS {
        icons.add_solid(id, &[]);
    }
    // View > Show's dialog launcher (Grid and Guides) uses the shared launcher drawing.
    icons.alias("view.gridSettings", "launcher");
}

/// A 24-unit path (absolute `M`/`L` pairs only) scaled to `size` px at (`x`, `y`).
fn shape(pts: &[(f32, f32)], x: f32, y: f32, k: f32) -> Vec<PathSeg> {
    let mut segs: Vec<PathSeg> = pts
        .iter()
        .enumerate()
        .map(|(i, &(px, py))| {
            if i == 0 {
                PathSeg::M(x + px * k, y + py * k)
            } else {
                PathSeg::L(x + px * k, y + py * k)
            }
        })
        .collect();
    segs.push(PathSeg::Z);
    segs
}

/// Outline points of a pen picture.
type Pts<'a> = &'a [(f32, f32)];

fn rgb(hex: &str) -> Rgba {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// Draw one pen tile's picture: the slanted barrel of BlueSlide's pen icons at `size` px,
/// its tip in the ink colour, and a bar below of the ink's colour and thickness.
pub(crate) fn paint_pen(p: &mut dyn Painter, pen: &PenSlot, x: f32, y: f32, size: f32, dim: bool) {
    let k = size / 24.0;
    let ink = rgb(&pen.color);
    let line = if dim {
        Rgba(0xa6, 0xa6, 0xa6, 255)
    } else {
        Rgba(0x44, 0x44, 0x44, 255)
    };
    let (body, tip): (Pts, Pts) = match pen.kind {
        PenKind::Highlighter => (
            &[
                (14.0, 3.0),
                (20.0, 9.0),
                (11.0, 18.0),
                (5.0, 18.0),
                (5.0, 12.0),
            ],
            &[(5.0, 12.0), (5.0, 18.0), (11.0, 18.0)],
        ),
        _ => (
            &[
                (16.0, 3.0),
                (21.0, 8.0),
                (10.0, 19.0),
                (5.0, 20.0),
                (6.0, 15.0),
            ],
            &[(6.0, 15.0), (10.0, 19.0), (5.0, 20.0)],
        ),
    };
    let fill = match pen.kind {
        PenKind::Pencil => Rgba(0xf2, 0xd8, 0x8c, 255),
        _ => Rgba(0xf4, 0xf4, 0xf4, 255),
    };
    p.path(&shape(body, x, y, k), Some(fill), Some((line, 1.0)));
    // A band in the ink colour near the cap (own detail of these tiles).
    let band: &[(f32, f32)] = match pen.kind {
        PenKind::Highlighter => &[(14.0, 3.0), (20.0, 9.0), (18.0, 11.0), (12.0, 5.0)],
        _ => &[(16.0, 3.0), (21.0, 8.0), (19.0, 10.0), (14.0, 5.0)],
    };
    let mut cap = ink;
    if dim {
        cap.3 = 110;
    }
    p.path(&shape(band, x, y, k), Some(cap), Some((line, 1.0)));
    p.path(&shape(tip, x, y, k), Some(cap), Some((line, 1.0)));
    // Thickness bar under the pen.
    let w = match pen.kind {
        PenKind::Highlighter => (pen.mm as f32 * 0.9).clamp(3.0, 8.0),
        _ => (pen.mm as f32 * 4.0).clamp(1.0, 4.0),
    } * k
        / 1.6;
    let mut bar = ink;
    if pen.kind == PenKind::Highlighter {
        bar.3 = 160;
    }
    p.fill_rect(
        (x + 3.0 * k).round(),
        (y + 23.0 * k).round(),
        (18.0 * k).round(),
        w.max(1.0).round(),
        bar,
    );
}

impl SlideWindow {
    /// Run one of the Draw tab's commands; `None` when `id` is not one of them.
    pub(super) fn draw_action(&mut self, id: &str) -> Option<bool> {
        if let Some(i) = self.pens.iter().position(|p| p.id == id) {
            // A second click on the pen in use opens its menu.
            if self.active_pen == Some(self.pens[i].id) && self.view.ink_tool().is_some() {
                self.open_pen_menu(i);
                return Some(true);
            }
            self.active_pen = Some(self.pens[i].id);
            self.view.set_ink_tool(Some(self.pens[i].tool()));
            return Some(true);
        }
        Some(match id {
            "draw.eraser" => {
                if self.view.ink_tool() == Some(&crate::slide_view::InkTool::Eraser) {
                    let items = [
                        (
                            "strokeEraser",
                            "deras:stroke",
                            self.view.erase_mode() == EraseMode::Stroke,
                        ),
                        (
                            "pointEraser",
                            "deras:point",
                            self.view.erase_mode() == EraseMode::Point,
                        ),
                    ]
                    .iter()
                    .map(|(k, c, on)| MenuItem {
                        label: self.t(&format!("slideNative.{k}"), k),
                        choice: Choice::Command((*c).to_owned()),
                        checked: *on,
                        dim: false,
                    })
                    .collect();
                    self.draw_menu(items, "draw.eraser");
                } else {
                    self.active_pen = None;
                    self.view
                        .set_ink_tool(Some(crate::slide_view::InkTool::Eraser));
                }
                true
            }
            "draw.add" => {
                let full = self.pens.len() >= MAX_PENS;
                let items = [
                    ("pen", "dadd:pen"),
                    ("pencil", "dadd:pencil"),
                    ("highlighter", "dadd:hl"),
                ]
                .iter()
                .map(|(k, c)| MenuItem {
                    label: self.t(&format!("slideNative.add_{k}"), k),
                    choice: Choice::Command((*c).to_owned()),
                    checked: false,
                    dim: full,
                })
                .collect();
                self.draw_menu(items, "draw.add");
                true
            }
            "draw.ruler" => {
                self.view.toggle_ruler();
                true
            }
            "draw.replay" => self.view.start_ink_replay(),
            "draw.inkToMath" | "draw.inkHelp" => false,
            _ => return None,
        })
    }

    /// The Draw tab's buttons shown as on: the tool in use, the ruler.
    pub(super) fn draw_toggled(&self, on: &mut Vec<&'static str>) {
        use crate::slide_view::InkTool;
        match self.view.ink_tool() {
            Some(InkTool::Eraser) => on.push("draw.eraser"),
            Some(InkTool::Lasso) => on.push("draw.lasso"),
            Some(InkTool::Pen { .. }) => {
                if let Some(id) = self.active_pen {
                    on.push(id);
                }
            }
            None => {}
        }
        if self.view.draw_ruler().is_some() {
            on.push("draw.ruler");
        }
    }

    fn draw_menu(&mut self, items: Vec<MenuItem>, anchor: &'static str) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(anchor)) else {
            return;
        };
        self.menu_kind = super::MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// The pen menu: thicknesses (the current one checked), then the colour palette.
    fn open_pen_menu(&mut self, i: usize) {
        let pen = &self.pens[i];
        let sizes: &[f64] = if pen.kind == PenKind::Highlighter {
            &HIGHLIGHT_MM
        } else {
            &PEN_MM
        };
        let mut items: Vec<MenuItem> = sizes
            .iter()
            .map(|mm| MenuItem {
                label: format!("{mm} mm"),
                choice: Choice::Command(format!("dpen:{i}:w:{mm}")),
                checked: (pen.mm - mm).abs() < 1e-9,
                dim: false,
            })
            .collect();
        items.push(MenuItem {
            label: self.t("slideNative.penColor", "Pen Color"),
            choice: Choice::Command(format!("dpen:{i}:color")),
            checked: false,
            dim: false,
        });
        let id = pen.id;
        self.draw_menu(items, id);
    }

    /// A choice of the Draw tab's menus (`dpen:…`, `dadd:…`, `deras:…`).
    pub(super) fn choose_draw(&mut self, kind: &str, v: &str) -> Option<bool> {
        Some(match kind {
            "deras" => {
                self.view.set_erase_mode(if v == "point" {
                    EraseMode::Point
                } else {
                    EraseMode::Stroke
                });
                true
            }
            "dadd" => {
                if self.pens.len() >= MAX_PENS {
                    return Some(false);
                }
                let id = PEN_IDS[self.pens.len()];
                let (kind, color, mm) = match v {
                    "pencil" => (PenKind::Pencil, "595959", 0.5),
                    "hl" => (PenKind::Highlighter, "00FF00", 6.0),
                    _ => (PenKind::Pen, "7030A0", 0.5),
                };
                self.pens.push(PenSlot {
                    id,
                    kind,
                    color: color.to_owned(),
                    mm,
                });
                self.update_pen_tips();
                let active = self.ribbon.active_tab();
                let mut tabs = self.fixed_tabs();
                if active >= self.base_tabs {
                    tabs.extend(super::ins::context_tabs(self.context));
                }
                self.ribbon.set_tabs(tabs);
                self.ribbon.set_active_tab(active);
                self.active_pen = Some(id);
                let tool = self.pens[self.pens.len() - 1].tool();
                self.view.set_ink_tool(Some(tool));
                self.relayout_pending = true;
                true
            }
            "dpen" => {
                let mut it = v.split(':');
                let Some(i) = it.next().and_then(|i| i.parse::<usize>().ok()) else {
                    return Some(false);
                };
                if i >= self.pens.len() {
                    return Some(false);
                }
                match (it.next(), it.next()) {
                    (Some("w"), Some(mm)) => {
                        let Ok(mm) = mm.parse::<f64>() else {
                            return Some(false);
                        };
                        self.pens[i].mm = mm;
                    }
                    (Some("color"), _) => {
                        let id = self.pens[i].id;
                        self.open_picker(PickKind::Outline, id);
                        return Some(true);
                    }
                    _ => return Some(false),
                }
                self.pen_changed(i);
                true
            }
            _ => return None,
        })
    }

    /// A colour picked for pen `anchor`; `None` when `anchor` is not a pen.
    pub(super) fn pick_pen_color(&mut self, anchor: &str, value: &str) -> Option<bool> {
        let i = self.pens.iter().position(|p| p.id == anchor)?;
        if value.len() != 6 {
            return Some(false);
        }
        self.pens[i].color = value.to_ascii_uppercase();
        self.pen_changed(i);
        Some(true)
    }

    fn pen_changed(&mut self, i: usize) {
        self.update_pen_tips();
        if self.active_pen == Some(self.pens[i].id) {
            let tool = self.pens[i].tool();
            self.view.set_ink_tool(Some(tool));
        }
    }

    /// Tooltips of the pen tiles: kind, colour code and thickness.
    pub(super) fn update_pen_tips(&mut self) {
        set_tips(&mut self.strings, &self.pens);
    }

    /// Draw the pen tiles over the ribbon buttons the Draw tab reserves for them.
    pub(super) fn paint_pens(&self, p: &mut dyn Painter) {
        for pen in &self.pens {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(pen.id)) else {
                continue;
            };
            let size = (52.0 * self.scale).round();
            paint_pen(
                p,
                pen,
                (r[0] + (r[2] - size) / 2.0).round(),
                r[1] + (1.0 * self.scale).round(),
                size,
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pens_and_tab() {
        let pens = default_pens();
        assert_eq!(pens.len(), 6);
        assert_eq!(pens[0].id, PEN_IDS[0]);
        // 0.5 mm pen ≈ 1.417 pt; highlighter 6 mm is translucent.
        match pens[0].tool() {
            crate::slide_view::InkTool::Pen {
                width, highlighter, ..
            } => {
                assert!((width - 1.417).abs() < 0.01 && !highlighter);
            }
            _ => panic!("pen"),
        }
        assert!(matches!(
            pens[3].tool(),
            crate::slide_view::InkTool::Pen {
                highlighter: true,
                ..
            }
        ));
        let tab = draw_tab(6);
        let labels: Vec<&str> = tab.groups.iter().map(|g| g.label).collect();
        assert_eq!(
            labels,
            [
                "slideNative.tools",
                "slideNative.undoGroup",
                "slideNative.pens",
                "slideNative.stencils",
                "slideNative.convert",
                "slideNative.replay",
                "slideNative.helpGroup"
            ]
        );
        assert_eq!(tab.groups[2].large.len(), 7);
        assert_eq!(draw_tab(8).groups[2].large.len(), 9);
        assert_eq!(large_width("draw.pencil"), Some(54.4));
    }
}
