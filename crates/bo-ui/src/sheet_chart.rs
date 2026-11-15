//! Charts on BlueSheet's grid (round 6): select a chart with a click (frame and eight handles as
//! the reference draws them), drag it to move or a handle to resize (anchored to cells,
//! `moveChart`), Delete removes it, and the contextual Chart Design / Format tabs (Add Chart
//! Element, Quick Layout, Change Colors, Chart Styles, Change Chart Type; chart area fill and
//! outline) over the engine's `editChart` / `chartDesign` commands.
use super::SheetWindow;
use crate::ribbon::{Group, Item, Tab};
use crate::{Input, Key, Painter};
use bo_render::Rgba;
use serde_json::{json, Value};

/// Frame and handle colours of a selected chart (the reference: grey frame, white round
/// handles with a grey ring).
const FRAME: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const HANDLE_RING: Rgba = Rgba(0x84, 0x84, 0x84, 255);
const WHITE: Rgba = Rgba(255, 255, 255, 255);
const PREVIEW: Rgba = Rgba(0x21, 0x73, 0x46, 255);

/// A chart being dragged: index, which part (0 = body, 1–8 = handles clockwise from the
/// top-left), pointer start, anchors at the start, and the anchors now.
#[derive(Debug, Clone)]
pub(super) struct ChartDrag {
    index: usize,
    part: usize,
    start: (f32, f32),
    from: [u32; 2],
    to: [u32; 2],
    now: ([u32; 2], [u32; 2]),
}

/// Own drawings for the chart tabs' buttons (24-unit grid).
pub(super) fn add_icons(icons: &mut crate::icons::Icons) {
    let blue = Rgba(0x2e, 0x75, 0xb6, 255);
    let orange = Rgba(0xed, 0x7d, 0x31, 255);
    let grey = Rgba(0x8a, 0x8a, 0x8a, 255);
    let pale = Rgba(0xd6, 0xe4, 0xf2, 255);
    icons.add_solid(
        "sheet.chartAddElement",
        &[
            ("M3 4h15v15h-15Z", Some(WHITE)),
            ("M3 4h15v15h-15Z", None),
            ("M6 15h2v-5h-2ZM10 15h2v-8h-2Z", Some(blue)),
            ("M15 13h7M18.5 9.5v7", None),
        ],
    );
    icons.add_solid(
        "sheet.chartQuickLayout",
        &[
            ("M3 3h18v18h-18Z", Some(WHITE)),
            ("M3 3h18v18h-18Z", None),
            ("M5 5h14v3h-14Z", Some(pale)),
            ("M6 18h3v-6h-3ZM11 18h3v-8h-3Z", Some(blue)),
            ("M16 11h3v2h-3ZM16 15h3v2h-3Z", Some(grey)),
        ],
    );
    icons.add_solid(
        "sheet.chartColors",
        &[
            ("M4 4h5v16h-5Z", Some(blue)),
            ("M10 4h5v16h-5Z", Some(orange)),
            ("M16 4h5v16h-5Z", Some(grey)),
        ],
    );
    icons.add_solid(
        "sheet.chartStyles",
        &[
            ("M3 4h18v16h-18Z", Some(pale)),
            ("M3 4h18v16h-18Z", None),
            ("M6 17h3v-6h-3ZM11 17h3v-9h-3ZM16 17h3v-4h-3Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "sheet.chartSwitch",
        &[
            ("M3 3h8v8h-8Z", Some(pale)),
            ("M13 13h8v8h-8Z", Some(pale)),
            ("M3 3h8v8h-8ZM13 13h8v8h-8Z", None),
            ("M14 5h4v4M10 19h-4v-4", None),
        ],
    );
    icons.add_solid(
        "sheet.chartSelectData",
        &[
            ("M3 3h14v14h-14Z", Some(WHITE)),
            ("M3 3h14v14h-14ZM3 8h14M8 3v14", None),
            ("M14 14h7v7h-7Z", Some(blue)),
        ],
    );
    icons.add_solid(
        "sheet.chartChangeType",
        &[
            ("M4 13h4v8h-4ZM10 7h4v14h-4Z", Some(blue)),
            ("M16 10h4v11h-4Z", Some(orange)),
            ("M14 3h7M18 1v4", None),
        ],
    );
    icons.add_solid(
        "sheet.chartMove",
        &[
            ("M3 6h12v12h-12Z", Some(pale)),
            ("M3 6h12v12h-12Z", None),
            ("M14 12h8M19 9l3 3l-3 3", None),
        ],
    );
    icons.add_solid(
        "sheet.chartResetStyle",
        &[("M5 12a7 7 0 1 0 2-5", None), ("M3 4v5h5Z", Some(blue))],
    );
    icons.add_solid(
        "sheet.chartFill",
        &[
            ("M5 12l7-7l7 7l-7 7Z", Some(pale)),
            ("M5 12l7-7l7 7l-7 7Z", None),
            ("M3 20h18v2h-18Z", Some(orange)),
        ],
    );
    icons.add_solid(
        "sheet.chartOutline",
        &[("M5 4h14v12h-14Z", None), ("M3 20h18v2h-18Z", Some(blue))],
    );
    icons.add_solid(
        "sheet.chartDelete",
        &[
            ("M5 7h14l-1 14h-12Z", Some(pale)),
            ("M5 7h14l-1 14h-12ZM3 7h18M9 4h6", None),
        ],
    );
}

/// Contextual tabs of a selected chart.
pub(crate) fn chart_tabs() -> Vec<Tab> {
    use Item::{Labeled as S, Large as L};
    vec![
        Tab {
            label: "sheetChart.tabDesign",
            groups: vec![
                Group {
                    label: "sheetChart.groupLayouts",
                    large: vec![L("sheet.chartAddElement"), L("sheet.chartQuickLayout")],
                    rows: vec![],
                },
                Group {
                    label: "sheetChart.groupStyles",
                    large: vec![L("sheet.chartColors"), L("sheet.chartStyles")],
                    rows: vec![],
                },
                Group {
                    label: "sheetChart.groupData",
                    large: vec![L("sheet.chartSwitch"), L("sheet.chartSelectData")],
                    rows: vec![],
                },
                Group {
                    label: "sheetChart.groupType",
                    large: vec![L("sheet.chartChangeType")],
                    rows: vec![],
                },
                Group {
                    label: "sheetChart.groupLocation",
                    large: vec![L("sheet.chartMove")],
                    rows: vec![],
                },
            ],
        },
        Tab {
            label: "sheetChart.tabFormat",
            groups: vec![
                Group {
                    label: "sheetChart.groupSelection",
                    large: vec![],
                    rows: vec![vec![S("sheet.chartResetStyle")]],
                },
                Group {
                    label: "sheetChart.groupShapeStyles",
                    large: vec![],
                    rows: vec![vec![S("sheet.chartFill")], vec![S("sheet.chartOutline")]],
                },
                Group {
                    label: "sheetChart.groupArrange",
                    large: vec![L("sheet.chartDelete")],
                    rows: vec![],
                },
            ],
        },
    ]
}

/// Menus of the chart tabs (choice id, label key or literal).
pub(crate) fn chart_menu(id: &str) -> Vec<(String, String)> {
    let e = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    match id {
        "sheet.chartAddElement" => vec![
            e("chartEl.title", "sheetChart.elTitle"),
            e("chartEl.noTitle", "sheetChart.elNoTitle"),
            e("chartEl.axisTitles", "sheetChart.elAxisTitles"),
            e("chartEl.labels", "sheetChart.elLabels"),
            e("chartEl.noLabels", "sheetChart.elNoLabels"),
            e("chartEl.dataTable", "sheetChart.elDataTable"),
            e("chartEl.legendRight", "sheetChart.elLegendRight"),
            e("chartEl.legendTop", "sheetChart.elLegendTop"),
            e("chartEl.legendBottom", "sheetChart.elLegendBottom"),
            e("chartEl.legendNone", "sheetChart.elLegendNone"),
        ],
        "sheet.chartQuickLayout" => bo_sheet::chart_style::LAYOUTS
            .iter()
            .enumerate()
            .map(|(i, l)| (format!("chartLayout.{l}"), format!("Layout {}", i + 1)))
            .collect(),
        "sheet.chartColors" => bo_sheet::chart_style::PALETTES
            .iter()
            .map(|p| (format!("chartColors.{p}"), format!("sheetChart.pal_{p}")))
            .collect(),
        "sheet.chartStyles" => bo_sheet::chart_style::STYLES
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("chartStyle.{s}"), format!("Style {}", i + 1)))
            .collect(),
        "sheet.chartChangeType" => [
            "column", "bar", "line", "area", "pie", "doughnut", "scatter", "radar",
        ]
        .iter()
        .map(|k| (format!("chartType.{k}"), format!("sheetInsertChart.{k}")))
        .collect(),
        "sheet.chartFill" | "sheet.chartOutline" => {
            let what = if id == "sheet.chartFill" {
                "fill"
            } else {
                "line"
            };
            [
                ("FFFFFF", "sheetChart.white"),
                ("F2F2F2", "sheetChart.lightGrey"),
                ("DDEBF7", "sheetChart.lightBlue"),
                ("FFF2CC", "sheetChart.lightGold"),
                ("4472C4", "sheetChart.blue"),
                ("000000", "sheetChart.black"),
            ]
            .iter()
            .map(|(c, k)| (format!("chartArea.{what}.{c}"), (*k).to_owned()))
            .collect()
        }
        _ => Vec::new(),
    }
}

impl SheetWindow {
    /// Anchors (`from`, `to`) of the charts of the shown sheet.
    fn chart_anchors(&mut self) -> Vec<([u32; 2], [u32; 2])> {
        let pair = |v: &Value, k: &str| -> Option<[u32; 2]> {
            let a = v.get(k)?.as_array()?;
            Some([a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32])
        };
        self.view
            .query(json!({ "cmd": "charts" }))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| Some((pair(c, "from")?, pair(c, "to")?)))
            .collect()
    }

    /// Window rectangle of anchors, when both corners are on screen.
    fn anchor_rect(&self, from: [u32; 2], to: [u32; 2]) -> Option<[f32; 4]> {
        let a = self.view.cell_rect(from[0], from[1])?;
        let b = self.view.cell_rect(to[0], to[1])?;
        Some([a[0], a[1], (b[0] - a[0]).max(8.0), (b[1] - a[1]).max(8.0)])
    }

    /// Handle centres of a chart rectangle, clockwise from the top-left.
    fn handles(r: [f32; 4]) -> [(f32, f32); 8] {
        let [x, y, w, h] = r;
        [
            (x, y),
            (x + w / 2.0, y),
            (x + w, y),
            (x + w, y + h / 2.0),
            (x + w, y + h),
            (x + w / 2.0, y + h),
            (x, y + h),
            (x, y + h / 2.0),
        ]
    }

    /// Select chart `index` (or none) and show / hide the chart tabs.
    pub(super) fn set_chart_selection(&mut self, index: Option<usize>, activate: bool) {
        if self.chart_sel == index && !activate {
            return;
        }
        self.chart_sel = index;
        self.ribbon.set_contextual(
            if index.is_some() {
                chart_tabs()
            } else {
                Vec::new()
            },
            activate,
        );
    }

    /// After inserting a chart: select the newest one and show Chart Design.
    pub(super) fn select_new_chart(&mut self) {
        let n = self.chart_anchors().len();
        if n > 0 {
            self.set_chart_selection(Some(n - 1), true);
        }
    }

    /// Pointer input over charts: `Some(redraw)` when it was a chart's.
    pub(super) fn chart_input(&mut self, input: &Input) -> Option<bool> {
        match *input {
            Input::Press { x, y, .. } => {
                let [gx, gy, gw, gh] = self.view.grid_frame();
                if x < gx || y < gy || x >= gx + gw || y >= gy + gh {
                    return None;
                }
                let anchors = self.chart_anchors();
                let tol = (5.0 * self.scale).round();
                // Handles of the selected chart first, then any chart's body (top-most last).
                if let Some(i) = self.chart_sel {
                    if let Some(&(from, to)) = anchors.get(i) {
                        if let Some(r) = self.anchor_rect(from, to) {
                            if let Some(h) = Self::handles(r)
                                .iter()
                                .position(|(hx, hy)| (x - hx).abs() <= tol && (y - hy).abs() <= tol)
                            {
                                self.chart_drag = Some(ChartDrag {
                                    index: i,
                                    part: h + 1,
                                    start: (x, y),
                                    from,
                                    to,
                                    now: (from, to),
                                });
                                return Some(true);
                            }
                        }
                    }
                }
                let hit = anchors.iter().enumerate().rev().find_map(|(i, (f, t))| {
                    let r = self.anchor_rect(*f, *t)?;
                    (x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3])
                        .then_some((i, *f, *t))
                });
                let Some((i, from, to)) = hit else {
                    // The press still goes to the grid.
                    self.set_chart_selection(None, false);
                    return None;
                };
                self.set_chart_selection(Some(i), false);
                self.chart_drag = Some(ChartDrag {
                    index: i,
                    part: 0,
                    start: (x, y),
                    from,
                    to,
                    now: (from, to),
                });
                Some(true)
            }
            Input::Drag { x, y } => {
                let d = self.chart_drag.clone()?;
                let (sr, sc) = self.view.cell_under(d.start.0, d.start.1).unwrap_or((0, 0));
                let (r, c) = self.view.cell_under(x, y).unwrap_or((sr, sc));
                let (dr, dc) = (i64::from(r) - i64::from(sr), i64::from(c) - i64::from(sc));
                let mv = |v: u32, by: i64| (i64::from(v) + by).max(0) as u32;
                let (mut f, mut t) = (d.from, d.to);
                match d.part {
                    0 => {
                        f = [mv(f[0], dr), mv(f[1], dc)];
                        t = [mv(t[0], dr), mv(t[1], dc)];
                    }
                    h => {
                        // Handles move the edges they sit on.
                        let (top, right, bottom, left) = match h {
                            1 => (true, false, false, true),
                            2 => (true, false, false, false),
                            3 => (true, true, false, false),
                            4 => (false, true, false, false),
                            5 => (false, true, true, false),
                            6 => (false, false, true, false),
                            7 => (false, false, true, true),
                            _ => (false, false, false, true),
                        };
                        if top {
                            f[0] = mv(f[0], dr).min(t[0].saturating_sub(1));
                        }
                        if left {
                            f[1] = mv(f[1], dc).min(t[1].saturating_sub(1));
                        }
                        if bottom {
                            t[0] = mv(t[0], dr).max(f[0] + 1);
                        }
                        if right {
                            t[1] = mv(t[1], dc).max(f[1] + 1);
                        }
                    }
                }
                if let Some(cd) = self.chart_drag.as_mut() {
                    cd.now = (f, t);
                }
                Some(true)
            }
            Input::Release { .. } => {
                let d = self.chart_drag.take()?;
                if d.now != (d.from, d.to) {
                    self.view.run(json!({
                        "cmd": "moveChart", "index": d.index, "from": d.now.0, "to": d.now.1,
                    }));
                }
                Some(true)
            }
            Input::Key(Key::Delete, _) if self.chart_sel.is_some() && !self.view.editing() => {
                let i = self.chart_sel.unwrap_or(0);
                self.set_chart_selection(None, false);
                Some(self.view.run(json!({ "cmd": "deleteChart", "index": i })))
            }
            Input::Key(Key::Escape, _) if self.chart_sel.is_some() => {
                self.set_chart_selection(None, false);
                Some(true)
            }
            _ => None,
        }
    }

    /// Frame and handles of the selected chart, the outline of a drag in progress.
    pub(super) fn paint_chart_selection(&self, p: &mut dyn Painter) {
        let Some(i) = self.chart_sel else { return };
        let Some(&(from, to)) = self.chart_cache.get(i) else {
            return;
        };
        let [gx, gy, gw, gh] = self.view.grid_frame();
        p.push_clip(gx, gy, gw, gh);
        if let Some(r) = self.anchor_rect(from, to) {
            let [x, y, w, h] = r;
            for (rx, ry, rw, rh) in [
                (x, y, w, 1.0),
                (x, y + h - 1.0, w, 1.0),
                (x, y, 1.0, h),
                (x + w - 1.0, y, 1.0, h),
            ] {
                p.fill_rect(rx, ry, rw, rh, FRAME);
            }
            let s = (4.0 * self.scale).round();
            for (hx, hy) in Self::handles(r) {
                let circle = |rad: f32| {
                    let k = rad * 0.552;
                    vec![
                        bo_render::PathSeg::M(hx + rad, hy),
                        bo_render::PathSeg::C(hx + rad, hy + k, hx + k, hy + rad, hx, hy + rad),
                        bo_render::PathSeg::C(hx - k, hy + rad, hx - rad, hy + k, hx - rad, hy),
                        bo_render::PathSeg::C(hx - rad, hy - k, hx - k, hy - rad, hx, hy - rad),
                        bo_render::PathSeg::C(hx + k, hy - rad, hx + rad, hy - k, hx + rad, hy),
                        bo_render::PathSeg::Z,
                    ]
                };
                p.path(&circle(s), Some(WHITE), Some((HANDLE_RING, 1.0)));
            }
        }
        if let Some(d) = &self.chart_drag {
            if d.now != (d.from, d.to) {
                if let Some([x, y, w, h]) = self.anchor_rect(d.now.0, d.now.1) {
                    for (rx, ry, rw, rh) in [
                        (x, y, w, 1.0),
                        (x, y + h - 1.0, w, 1.0),
                        (x, y, 1.0, h),
                        (x + w - 1.0, y, 1.0, h),
                    ] {
                        p.fill_rect(rx, ry, rw, rh, PREVIEW);
                    }
                }
            }
        }
        p.pop_clip();
    }

    /// Keep the anchors of the charts for painting (refreshed after each input).
    pub(super) fn refresh_chart_cache(&mut self) {
        self.chart_cache = if self.chart_sel.is_some() {
            self.chart_anchors()
        } else {
            Vec::new()
        };
        if let Some(i) = self.chart_sel {
            if i >= self.chart_cache.len() {
                self.set_chart_selection(None, false);
            }
        }
    }

    /// Chart tab commands; `None` for other ids.
    pub(super) fn chart_action(&mut self, id: &str) -> Option<bool> {
        let index = self.chart_sel?;
        let design = |v: Value| {
            json!({ "cmd": "chartDesign", "index": index })
                .as_object()
                .cloned()
                .map(|mut o| {
                    if let Some(extra) = v.as_object() {
                        o.extend(extra.clone());
                    }
                    Value::Object(o)
                })
        };
        let edit = |v: Value| {
            let mut o = json!({ "cmd": "editChart", "index": index });
            if let (Some(a), Some(b)) = (o.as_object_mut(), v.as_object()) {
                a.extend(b.clone());
            }
            o
        };
        let cmd = match id {
            "chartEl.title" => {
                edit(json!({ "title": self.t("sheetChart.chartTitle", "Chart Title") }))
            }
            "chartEl.noTitle" => edit(json!({ "title": "" })),
            "chartEl.axisTitles" => edit(json!({
                "xTitle": self.t("sheetChart.axisTitle", "Axis Title"),
                "yTitle": self.t("sheetChart.axisTitle", "Axis Title"),
            })),
            "chartEl.legendRight" => edit(json!({ "legend": "r" })),
            "chartEl.legendTop" => edit(json!({ "legend": "t" })),
            "chartEl.legendBottom" => edit(json!({ "legend": "b" })),
            "chartEl.legendNone" => edit(json!({ "legend": "none" })),
            "chartEl.labels" => design(json!({ "elements": { "labels": { "value": true } } }))?,
            "chartEl.noLabels" => design(json!({ "elements": { "labels": null } }))?,
            "chartEl.dataTable" => {
                design(json!({ "elements": { "dataTable": { "show": true, "keys": true } } }))?
            }
            "sheet.chartResetStyle" => {
                design(json!({ "style": bo_sheet::chart_style::STYLES[0] }))?
            }
            "sheet.chartDelete" => {
                self.set_chart_selection(None, false);
                json!({ "cmd": "deleteChart", "index": index })
            }
            _ => {
                if let Some(l) = id.strip_prefix("chartLayout.") {
                    design(json!({ "layout": l }))?
                } else if let Some(c) = id.strip_prefix("chartColors.") {
                    design(json!({ "colors": c }))?
                } else if let Some(s) = id.strip_prefix("chartStyle.") {
                    design(json!({ "style": s }))?
                } else if let Some(k) = id.strip_prefix("chartType.") {
                    edit(json!({ "kind": k }))
                } else {
                    let (what, color) = id.strip_prefix("chartArea.")?.split_once('.')?;
                    design(json!({ "elements": { "chartArea": { what: format!("#{color}") } } }))?
                }
            }
        };
        let ok = self.view.run(cmd);
        if !ok {
            self.status = self.view.take_error().unwrap_or_default();
        }
        Some(true)
    }
}
