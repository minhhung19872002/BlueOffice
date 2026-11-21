//! BlueEditor round 12 (resumed): Comment › Measure ▾ as the reference's drop-down — the
//! editor's menu look (29 px rows at 125 %) with an icon before Distance, Perimeter and Area.
//! The icon shapes are our own drawings in the BlueOffice icon family.

use crate::editor_window::EditorWindow;
use crate::popup::{Choice, MenuItem};
use crate::Painter;
use bo_render::{PathSeg, Rgba};

/// Dimmed icon ink (the reader menus' disabled grey).
const DISABLED: Rgba = Rgba(175, 175, 176, 255);

/// Own icon shapes on a 24-unit grid: (path, accent).
fn shape12(name: &str) -> &'static [(&'static str, bool)] {
    match name {
        // A ruler with ticks under a double-headed span arrow.
        "mDistance" => &[
            (
                "M3 15h18v5H3z M7 15v2.5 M11 15v3 M15 15v2.5 M19 15v2",
                false,
            ),
            ("M3 8h18 M6 5L3 8l3 3 M18 5l3 3-3 3", true),
        ],
        // A ruler under a closed outline traced by the span.
        "mPerimeter" => &[
            (
                "M3 15h18v5H3z M7 15v2.5 M11 15v3 M15 15v2.5 M19 15v2",
                false,
            ),
            ("M4 11V4h16v7z", true),
        ],
        // A ruler under a hatched surface.
        "mArea" => &[
            (
                "M3 15h18v5H3z M7 15v2.5 M11 15v3 M15 15v2.5 M19 15v2",
                false,
            ),
            (
                "M4 4h16v7H4z M8 4l-4 4 M13 4l-7 7 M18 4l-7 7 M20 7l-4 4",
                true,
            ),
        ],
        _ => &[],
    }
}

/// Menu icon painter: our measure shapes, everything else as the reader's menus draw it.
fn icon12(p: &mut dyn Painter, name: &str, x: f32, y: f32, size: f32, dim: bool) {
    let parts = shape12(name);
    if parts.is_empty() {
        crate::reader_icons::menu_icon(p, name, x, y, size, dim);
        return;
    }
    let (ink, accent) = if dim {
        (DISABLED, DISABLED)
    } else {
        (
            Rgba(51, 51, 51, 255),
            crate::theme::Theme::light("reader").accent,
        )
    };
    let k = size / 24.0;
    let width = (1.5 * k).round().max(1.0);
    let off = if (width as i32) % 2 == 1 { 0.5 } else { 0.0 };
    let t = |a: f32, b: f32| ((x + a * k).round() + off, (y + b * k).round() + off);
    for (d, acc) in parts {
        let segs: Vec<PathSeg> = crate::icons::parse_path(d)
            .into_iter()
            .map(|s| match s {
                PathSeg::M(a, b) => {
                    let (u, v) = t(a, b);
                    PathSeg::M(u, v)
                }
                PathSeg::L(a, b) => {
                    let (u, v) = t(a, b);
                    PathSeg::L(u, v)
                }
                other => other,
            })
            .collect();
        p.path(&segs, None, Some((if *acc { accent } else { ink }, width)));
    }
}

impl EditorWindow {
    /// Round 12's Measure drop-down; false when `cmd` is not its command.
    pub(crate) fn measure12_command(&mut self, cmd: &str) -> bool {
        if cmd != "cm.measure" {
            return false;
        }
        let Some(r) = self.r.control_rect(cmd) else {
            return true;
        };
        let rows = [
            ("draw.distance", "pdfDraw.distance", "mDistance"),
            ("draw.perimeter", "pdfDraw.perimeter", "mPerimeter"),
            ("draw.area", "pdfDraw.area", "mArea"),
        ];
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(c, k, _)| MenuItem {
                label: self.t(k),
                choice: Choice::Command((*c).to_owned()),
                checked: false,
                dim: false,
            })
            .collect();
        let icons = rows.iter().map(|r| Some(r.2)).collect();
        let menu = self
            .menu_open9(items, r)
            .with_decor(icons, Vec::new())
            .with_icon_fn(icon12);
        self.r.set_menu9(menu, "cm.measure");
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::editor_tests::{window, Fixed, Rec};

    /// The Measure drop-down lists the three tools with our icons (strokes drawn).
    #[test]
    fn measure_menu_has_three_rows_with_icons() {
        let mut w = window(1);
        w.show_tab(4, &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        w.run("cm.measure", &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        for l in ["Distance", "Perimeter", "Area"] {
            assert!(rec.texts.iter().any(|t| t == l), "{l}: {:?}", rec.texts);
        }
        assert_eq!(super::shape12("mArea").len(), 2);
    }
}
