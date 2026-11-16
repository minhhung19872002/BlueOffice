//! BlueSlide charts, round 7: Insert > Chart offers the chart types (the shared engine charts,
//! axes and text as the reference draws them), and a selected chart shows the Chart Design /
//! Format tabs of BlueSheet (same commands, menus and icons: Add Chart Element, Quick Layout,
//! Change Colors, Chart Styles, Change Chart Type, Shape Fill / Outline) over the slide
//! engine's `setChart`.
use super::{MenuKind, SlideWindow};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use bo_engine::slide_edit::SlideCommand;
use serde_json::{json, Value};

/// Chart types Insert > Chart offers (engine names), as the reference's Insert Chart list.
const KINDS: [&str; 8] = [
    "column",
    "line",
    "pie",
    "bar",
    "area",
    "doughnut",
    "radar",
    "stackedColumn",
];

/// Commands of the chart tabs (enabled while they show).
pub const COMMANDS: [&str; 8] = [
    "sheet.chartAddElement",
    "sheet.chartQuickLayout",
    "sheet.chartColors",
    "sheet.chartStyles",
    "sheet.chartChangeType",
    "sheet.chartResetStyle",
    "sheet.chartFill",
    "sheet.chartOutline",
];

/// The menus of the chart tabs (BlueSheet's).
const MENUS: [&str; 6] = [
    "sheet.chartAddElement",
    "sheet.chartQuickLayout",
    "sheet.chartColors",
    "sheet.chartStyles",
    "sheet.chartChangeType",
    "sheet.chartFill",
];

/// Contextual tabs of a selected chart: BlueSheet's Chart Design and Format, without the
/// sheet-only commands (Select Data, Move Chart).
pub(super) fn chart_tabs() -> Vec<Tab> {
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
                    label: "sheetChart.groupType",
                    large: vec![L("sheet.chartChangeType")],
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
            ],
        },
    ]
}

impl SlideWindow {
    /// A chart is the one shape selected.
    pub(super) fn chart_selected(&self) -> bool {
        self.view.state().is_some_and(|s| s.chart.is_some()) && self.view.selection().len() == 1
    }

    /// Chart commands of this module; `None` when `id` is not one.
    pub(super) fn chart_action(&mut self, id: &str) -> Option<bool> {
        if id == "sl.chart" {
            self.open_chart_kinds();
            return Some(true);
        }
        if let Some(kind) = id.strip_prefix("slchart:") {
            self.activate_context = true;
            return Some(self.view.run(SlideCommand::InsertChart {
                kind: kind.to_owned(),
                title: None,
                data: None,
                legend: None,
                rect: None,
                template: None,
                texts: bo_sheet::chart_style::LayoutTexts::default(),
            }));
        }
        if let Some(menu) = MENUS
            .iter()
            .chain(&["sheet.chartOutline"])
            .find(|m| **m == id)
        {
            self.open_chart_menu(menu);
            return Some(true);
        }
        if !self.chart_selected() {
            return None;
        }
        let spec = match id {
            "chartEl.title" => json!({ "title": self.t("sheetChart.chartTitle", "Chart Title") }),
            "chartEl.noTitle" => json!({ "title": "" }),
            "chartEl.legendRight" => json!({ "legend": "r" }),
            "chartEl.legendTop" => json!({ "legend": "t" }),
            "chartEl.legendBottom" => json!({ "legend": "b" }),
            "chartEl.legendNone" => json!({ "legend": "none" }),
            "chartEl.labels" => json!({ "elements": { "labels": { "value": true } } }),
            "chartEl.noLabels" => json!({ "elements": { "labels": null } }),
            "chartEl.dataTable" => {
                json!({ "elements": { "dataTable": { "show": true, "keys": true } } })
            }
            "chartEl.axisTitles" => json!({ "elements": {
                "axes": [
                    { "title": self.t("sheetChart.axisTitle", "Axis Title") },
                    { "title": self.t("sheetChart.axisTitle", "Axis Title") }
                ]
            } }),
            "sheet.chartResetStyle" => json!({ "style": bo_sheet::chart_style::STYLES[0] }),
            _ => {
                if let Some(l) = id.strip_prefix("chartLayout.") {
                    json!({ "layout": l })
                } else if let Some(c) = id.strip_prefix("chartColors.") {
                    json!({ "colors": c })
                } else if let Some(s) = id.strip_prefix("chartStyle.") {
                    json!({ "style": s })
                } else if let Some(k) = id.strip_prefix("chartType.") {
                    json!({ "kind": k })
                } else {
                    let (what, color) = id.strip_prefix("chartArea.")?.split_once('.')?;
                    json!({ "elements": { "chartArea": { what: format!("#{color}") } } })
                }
            }
        };
        let mut cmd = json!({ "cmd": "setChart" });
        if let (Some(a), Value::Object(b)) = (cmd.as_object_mut(), spec) {
            a.extend(b);
        }
        Some(serde_json::from_value::<SlideCommand>(cmd).is_ok_and(|c| self.view.run(c)))
    }

    /// Insert > Chart: the chart types (reference: the Insert Chart dialog's list).
    fn open_chart_kinds(&mut self) {
        // Under the button, or under the ribbon when another tab shows (command search).
        let r = self
            .ribbon
            .rect_of(RibbonHit::Command("sl.chart"))
            .unwrap_or([0.0, self.ribbon.bottom(), 1.0, 1.0]);
        let items = KINDS
            .iter()
            .map(|k| MenuItem {
                label: self.t(&format!("sheetInsertChart.{k}"), k),
                choice: Choice::Command(format!("slchart:{k}")),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    fn open_chart_menu(&mut self, id: &'static str) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return;
        };
        let items: Vec<MenuItem> = crate::sheet_window::chart_menu(id)
            .into_iter()
            .map(|(choice, label)| MenuItem {
                label: if label.contains('.') {
                    self.t(&label, &label)
                } else {
                    label
                },
                choice: Choice::Command(choice),
                checked: false,
                dim: false,
            })
            .collect();
        if items.is_empty() {
            return;
        }
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }
}
