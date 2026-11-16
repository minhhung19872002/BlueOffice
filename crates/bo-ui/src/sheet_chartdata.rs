//! Chart data dialogs of BlueSheet (round 27): Select Data Source (chart data range, Switch
//! Row/Column, the series list with Add / Edit / Remove / order, the category labels), Edit
//! Series, Axis Labels and Move Chart (new sheet / object in a sheet), over the engine's
//! `chartData`, `setChartData`, `switchRowColumn` and `moveChartTo`.

use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use serde_json::{json, Value};

/// One series row of Select Data Source.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SeriesRow {
    /// Shown name (the name cell's text or a typed name).
    pub name: String,
    /// `=Sheet1!$B$1` or empty.
    pub name_ref: String,
    /// `=Sheet1!$B$2:$B$5`.
    pub val_ref: String,
}

/// State of an open Select Data Source form.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChartDataState {
    /// The chart (index on the shown sheet).
    pub index: usize,
    pub series: Vec<SeriesRow>,
    /// Category labels reference (`=Sheet1!$A$2:$A$5`, or empty).
    pub categories: String,
    /// Category labels as shown.
    pub labels: Vec<String>,
    /// The data range when the form opened (a changed range replots the whole range).
    pub range: String,
    /// The series list or the labels changed in the form.
    pub edited: bool,
    /// Edits applied while the form is open (Switch Row/Column), undone by Cancel.
    pub applied: u32,
    /// Series being edited in Edit Series (`None`: a new one).
    pub editing: Option<usize>,
}

fn eq_ref(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() || s.starts_with('=') {
        s.to_owned()
    } else {
        format!("={s}")
    }
}

impl SheetWindow {
    fn cd_t(&self, k: &str, d: &str) -> String {
        self.t(k, d)
    }

    /// Read chart `index`'s data into the dialog state.
    fn cd_load(&mut self, index: usize) -> Option<ChartDataState> {
        let d = self
            .view
            .query(json!({ "cmd": "chartData", "index": index }))?;
        let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        let series: Vec<SeriesRow> = d["series"]
            .as_array()
            .map(|a| {
                a.iter()
                    .enumerate()
                    .map(|(i, v)| SeriesRow {
                        name: v["name"]
                            .as_str()
                            .map_or_else(|| format!("Series{}", i + 1), str::to_owned),
                        name_ref: v["nameRef"].as_str().map(eq_ref).unwrap_or_default(),
                        val_ref: v["valRef"].as_str().map(eq_ref).unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let labels = d["series"][0]["categories"]
            .as_array()
            .map(|a| a.iter().map(&s).collect())
            .unwrap_or_default();
        Some(ChartDataState {
            index,
            series,
            categories: d["categories"].as_str().map(eq_ref).unwrap_or_default(),
            labels,
            range: d["range"].as_str().map(eq_ref).unwrap_or_default(),
            ..ChartDataState::default()
        })
    }

    /// Select Data Source for the selected chart.
    pub(super) fn open_select_data(&mut self) -> bool {
        let Some(index) = self.chart_sel else {
            return false;
        };
        let Some(state) = self.cd_load(index) else {
            return false;
        };
        let t = |k: &str, d: &str| self.cd_t(k, d);
        let mut f = Form::new(
            &t("sheetChart.selectDataTitle", "Select Data Source"),
            (800.0, 470.0),
            vec![],
        );
        f.add(
            "rangeLabel",
            None,
            [20.0, 60.0, 0.0, 0.0],
            Kind::Label(t("sheetChart.dataRange", "Chart data range:")),
        );
        f.add(
            "range",
            None,
            [160.0, 56.0, 620.0, 25.0],
            Kind::Edit {
                value: state.range.chars().collect(),
                caret: state.range.chars().count(),
            },
        );
        f.add(
            "switch",
            None,
            [320.0, 100.0, 160.0, 30.0],
            Kind::Button {
                label: t("sheetChart.switchRowColumn", "Switch Row/Column"),
                default: false,
            },
        );
        f.add(
            "seriesGroup",
            None,
            [20.0, 146.0, 370.0, 0.0],
            Kind::Group(t("sheetChart.legendEntries", "Legend Entries (Series)")),
        );
        for (id, key, label, x, w) in [
            ("add", "sheetChart.add", "Add", 20.0, 76.0),
            ("editSeries", "sheetChart.edit", "Edit", 102.0, 76.0),
            ("remove", "sheetChart.remove", "Remove", 184.0, 84.0),
            ("up", "sheetChart.moveUp", "\u{25B2}", 300.0, 40.0),
            ("down", "sheetChart.moveDown", "\u{25BC}", 346.0, 40.0),
        ] {
            f.add(
                id,
                None,
                [x, 172.0, w, 28.0],
                Kind::Button {
                    label: t(key, label),
                    default: false,
                },
            );
        }
        f.add(
            "series",
            None,
            [20.0, 208.0, 370.0, 190.0],
            Kind::List {
                items: state.series.iter().map(|s| s.name.clone()).collect(),
                sel: (!state.series.is_empty()).then_some(0),
                top: 0,
            },
        );
        f.add(
            "catGroup",
            None,
            [410.0, 146.0, 370.0, 0.0],
            Kind::Group(t(
                "sheetChart.categoryLabels",
                "Horizontal (Category) Axis Labels",
            )),
        );
        f.add(
            "editCats",
            None,
            [410.0, 172.0, 76.0, 28.0],
            Kind::Button {
                label: t("sheetChart.edit", "Edit"),
                default: false,
            },
        );
        f.add(
            "cats",
            None,
            [410.0, 208.0, 370.0, 190.0],
            Kind::List {
                items: state.labels.clone(),
                sel: None,
                top: 0,
            },
        );
        f.add(
            "hidden",
            None,
            [20.0, 422.0, 190.0, 28.0],
            Kind::Button {
                label: t("sheetChart.hiddenCells", "Hidden and Empty Cells"),
                default: false,
            },
        );
        f.enable("hidden", false);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(f);
        self.form_kind = FormKind::ChartData;
        self.chart_data = Some(state);
        self.cd_buttons();
        true
    }

    /// Enable the series buttons for the chosen series.
    fn cd_buttons(&mut self) {
        let n = self.chart_data.as_ref().map_or(0, |s| s.series.len());
        let Some(f) = self.form.as_mut() else { return };
        let sel = f.sel("series");
        f.enable("editSeries", sel.is_some());
        f.enable("remove", sel.is_some() && n > 1);
        f.enable("up", sel.is_some_and(|i| i > 0));
        f.enable("down", sel.is_some_and(|i| i + 1 < n));
    }

    /// Show the state's series and labels in the open form.
    fn cd_refresh(&mut self, choose: Option<usize>) {
        let Some(state) = self.chart_data.as_ref() else {
            return;
        };
        let names: Vec<String> = state.series.iter().map(|s| s.name.clone()).collect();
        let labels = state.labels.clone();
        let range = state.range.clone();
        if let Some(f) = self.form.as_mut() {
            f.set_items("series", names, choose);
            f.set_items("cats", labels, None);
            f.set_text("range", &range);
        }
        self.cd_buttons();
    }

    /// Edit Series (`at`: a series to edit, `None`: add one) over the stashed form.
    fn open_edit_series(&mut self, at: Option<usize>) {
        let row = at
            .and_then(|i| self.chart_data.as_ref()?.series.get(i).cloned())
            .unwrap_or_default();
        let t = |k: &str, d: &str| self.cd_t(k, d);
        let mut f = Form::new(
            &t("sheetChart.editSeries", "Edit Series"),
            (450.0, 240.0),
            vec![],
        );
        f.add(
            "nameLabel",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(t("sheetChart.seriesName", "Series name:")),
        );
        let name = if row.name_ref.is_empty() {
            row.name.clone()
        } else {
            row.name_ref.clone()
        };
        f.add(
            "name",
            None,
            [20.0, 78.0, 410.0, 25.0],
            Kind::Edit {
                caret: name.chars().count(),
                value: name.chars().collect(),
            },
        );
        f.add(
            "valLabel",
            None,
            [20.0, 116.0, 0.0, 0.0],
            Kind::Label(t("sheetChart.seriesValues", "Series values:")),
        );
        let vals = if row.val_ref.is_empty() {
            "={1}".to_owned()
        } else {
            row.val_ref.clone()
        };
        f.add(
            "values",
            None,
            [20.0, 138.0, 410.0, 25.0],
            Kind::Edit {
                caret: vals.chars().count(),
                value: vals.chars().collect(),
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("name");
        if let Some(main) = self.form.take() {
            self.form_stash = Some((main, self.form_kind));
        }
        if let Some(s) = self.chart_data.as_mut() {
            s.editing = at;
        }
        self.form = Some(f);
        self.form_kind = FormKind::ChartSeries;
    }

    /// Axis Labels (the category range) over the stashed form.
    fn open_axis_labels(&mut self) {
        let cats = self
            .chart_data
            .as_ref()
            .map(|s| s.categories.clone())
            .unwrap_or_default();
        let t = |k: &str, d: &str| self.cd_t(k, d);
        let mut f = Form::new(
            &t("sheetChart.axisLabels", "Axis Labels"),
            (450.0, 180.0),
            vec![],
        );
        f.add(
            "label",
            None,
            [20.0, 56.0, 0.0, 0.0],
            Kind::Label(t("sheetChart.axisLabelRange", "Axis label range:")),
        );
        f.add(
            "cats",
            None,
            [20.0, 78.0, 410.0, 25.0],
            Kind::Edit {
                caret: cats.chars().count(),
                value: cats.chars().collect(),
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        f.focus("cats");
        if let Some(main) = self.form.take() {
            self.form_stash = Some((main, self.form_kind));
        }
        self.form = Some(f);
        self.form_kind = FormKind::ChartAxisLabels;
    }

    /// Back to Select Data Source after Edit Series / Axis Labels.
    fn cd_unstash(&mut self, choose: Option<usize>) {
        if let Some((f, kind)) = self.form_stash.take() {
            self.form = Some(f);
            self.form_kind = kind;
        }
        self.cd_refresh(choose);
    }

    /// Live labels of a category reference.
    fn cd_labels(&self, f: &str) -> Vec<String> {
        let Some((si, r)) = self.cd_resolve(f) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for row in r.first.row..=r.last.row {
            for col in r.first.col..=r.last.col {
                out.push(
                    self.view
                        .engine()
                        .cell(si, row, col)
                        .map(|d| d.display)
                        .unwrap_or_default(),
                );
            }
        }
        out
    }

    /// Sheet and cells of a reference typed in a chart data form.
    fn cd_resolve(&self, f: &str) -> Option<(usize, bo_model::sheet::CellRange)> {
        let wb = self.view.engine().workbook()?;
        bo_sheet::cells::resolve_ref(wb, self.view.sheet(), f)
    }

    /// Events of the chart data forms; `true` when handled.
    pub(super) fn chart_data_event(&mut self, kind: FormKind, ev: &Event) -> bool {
        match (kind, ev) {
            (FormKind::ChartData, Event::Changed("series")) => self.cd_buttons(),
            (FormKind::ChartData, Event::Clicked("switch")) => {
                let Some(index) = self.chart_data.as_ref().map(|s| s.index) else {
                    return true;
                };
                if self
                    .view
                    .run(json!({ "cmd": "switchRowColumn", "index": index }))
                {
                    let applied = self.chart_data.as_ref().map_or(0, |s| s.applied) + 1;
                    if let Some(mut s) = self.cd_load(index) {
                        s.applied = applied;
                        self.chart_data = Some(s);
                    }
                    self.cd_refresh(Some(0));
                } else {
                    self.status = self.view.take_error().unwrap_or_default();
                }
            }
            (FormKind::ChartData, Event::Clicked("add")) => self.open_edit_series(None),
            (FormKind::ChartData, Event::Clicked("editSeries" | "series")) => {
                let at = self.form.as_ref().and_then(|f| f.sel("series"));
                if at.is_some() {
                    self.open_edit_series(at);
                }
            }
            (FormKind::ChartData, Event::Clicked("remove")) => {
                let at = self.form.as_ref().and_then(|f| f.sel("series"));
                if let (Some(i), Some(s)) = (at, self.chart_data.as_mut()) {
                    if s.series.len() > 1 && i < s.series.len() {
                        s.series.remove(i);
                        s.edited = true;
                        let n = s.series.len();
                        self.cd_refresh(Some(i.min(n - 1)));
                    }
                }
            }
            (FormKind::ChartData, Event::Clicked(b @ ("up" | "down"))) => {
                let at = self.form.as_ref().and_then(|f| f.sel("series"));
                if let (Some(i), Some(s)) = (at, self.chart_data.as_mut()) {
                    let j = if *b == "up" {
                        i.checked_sub(1)
                    } else {
                        Some(i + 1).filter(|j| *j < s.series.len())
                    };
                    if let Some(j) = j {
                        s.series.swap(i, j);
                        s.edited = true;
                        self.cd_refresh(Some(j));
                    }
                }
            }
            (FormKind::ChartData, Event::Clicked("editCats")) => self.open_axis_labels(),
            (FormKind::ChartData, Event::Ok) => self.cd_apply(),
            (FormKind::ChartData, Event::Cancel) => {
                let applied = self.chart_data.take().map_or(0, |s| s.applied);
                for _ in 0..applied {
                    self.view.undo();
                }
                self.form = None;
            }
            (FormKind::ChartSeries, Event::Ok) => {
                let (name, vals) = self
                    .form
                    .as_ref()
                    .map_or_else(Default::default, |f| (f.text("name"), f.text("values")));
                // A reference (`=Sheet1!$B$1`) names the series from a cell, other text is
                // the name itself.
                let is_ref = name.trim().starts_with('=') && self.cd_resolve(name.trim()).is_some();
                let shown = if is_ref {
                    self.cd_labels(name.trim())
                        .first()
                        .cloned()
                        .unwrap_or_default()
                } else {
                    name.trim().to_owned()
                };
                let mut choose = None;
                if let Some(s) = self.chart_data.as_mut() {
                    let row = SeriesRow {
                        name: if shown.is_empty() {
                            format!("Series{}", s.series.len() + 1)
                        } else {
                            shown
                        },
                        name_ref: if is_ref { eq_ref(&name) } else { String::new() },
                        val_ref: eq_ref(&vals),
                    };
                    match s.editing.take() {
                        Some(i) if i < s.series.len() => {
                            s.series[i] = row;
                            choose = Some(i);
                        }
                        _ => {
                            s.series.push(row);
                            choose = Some(s.series.len() - 1);
                        }
                    }
                    s.edited = true;
                }
                self.cd_unstash(choose);
            }
            (FormKind::ChartSeries | FormKind::ChartAxisLabels, Event::Cancel) => {
                let choose = self.form_stash.as_ref().and_then(|(f, _)| f.sel("series"));
                self.cd_unstash(choose);
            }
            (FormKind::ChartAxisLabels, Event::Ok) => {
                let cats = self
                    .form
                    .as_ref()
                    .map(|f| eq_ref(&f.text("cats")))
                    .unwrap_or_default();
                let labels = self.cd_labels(&cats);
                if let Some(s) = self.chart_data.as_mut() {
                    s.categories = cats;
                    s.labels = labels;
                    s.edited = true;
                }
                let choose = self.form_stash.as_ref().and_then(|(f, _)| f.sel("series"));
                self.cd_unstash(choose);
            }
            (FormKind::MoveChart, Event::Changed("newSheet")) => {
                if let Some(f) = self.form.as_mut() {
                    let on = f.on("newSheet");
                    f.set_on("objectIn", !on);
                    f.enable("newName", on);
                    f.enable("sheetList", !on);
                }
            }
            (FormKind::MoveChart, Event::Changed("objectIn")) => {
                if let Some(f) = self.form.as_mut() {
                    let on = f.on("objectIn");
                    f.set_on("newSheet", !on);
                    f.enable("newName", !on);
                    f.enable("sheetList", on);
                }
            }
            (FormKind::MoveChart, Event::Ok) => self.move_chart_ok(),
            (FormKind::MoveChart, Event::Cancel) => self.form = None,
            _ => return false,
        }
        true
    }

    /// OK of Select Data Source: a changed range replots it, else the edited series list.
    fn cd_apply(&mut self) {
        let Some(s) = self.chart_data.take() else {
            self.form = None;
            return;
        };
        let range = self
            .form
            .as_ref()
            .map(|f| f.text("range"))
            .unwrap_or_default();
        let cmd = if eq_ref(&range) != s.range && !range.trim().is_empty() {
            Some(json!({ "cmd": "setChartData", "index": s.index, "range": eq_ref(&range) }))
        } else if s.edited {
            let series: Vec<Value> = s
                .series
                .iter()
                .map(|r| {
                    if r.name_ref.is_empty() {
                        json!({ "name": r.name, "valRef": r.val_ref })
                    } else {
                        json!({ "nameRef": r.name_ref, "valRef": r.val_ref })
                    }
                })
                .collect();
            Some(
                json!({ "cmd": "setChartData", "index": s.index, "series": series,
                "categories": s.categories }),
            )
        } else {
            None
        };
        if let Some(cmd) = cmd {
            if !self.view.run(cmd) {
                // Keep the form open with the engine's reason in the status bar.
                self.status = self.view.take_error().unwrap_or_default();
                self.chart_data = Some(s);
                return;
            }
        }
        self.form = None;
    }

    /// Move Chart for the selected chart.
    pub(super) fn open_move_chart(&mut self) -> bool {
        if self.chart_sel.is_none() {
            return false;
        }
        let names: Vec<String> = self.view.sheet_names().into_iter().flatten().collect();
        let current = self.view.sheet();
        let visible_index = self
            .view
            .sheet_names()
            .iter()
            .take(current)
            .filter(|n| n.is_some())
            .count();
        let t = |k: &str, d: &str| self.cd_t(k, d);
        // The next free "ChartN" name, as the reference proposes.
        let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
        let free = (1..10_000)
            .map(|n| format!("{}{n}", t("sheetChart.chartSheetPrefix", "Chart")))
            .find(|n| !lower.contains(&n.to_lowercase()))
            .unwrap_or_default();
        let mut f = Form::new(
            &t("sheetChart.moveChartTitle", "Move Chart"),
            (520.0, 230.0),
            vec![],
        );
        f.add(
            "where",
            None,
            [20.0, 56.0, 480.0, 0.0],
            Kind::Group(t(
                "sheetChart.moveWhere",
                "Choose where you want the chart to be placed:",
            )),
        );
        f.add(
            "newSheet",
            None,
            [40.0, 92.0, 130.0, 20.0],
            Kind::Check {
                label: t("sheetChart.newSheet", "New sheet:"),
                on: false,
            },
        );
        f.add(
            "newName",
            None,
            [180.0, 88.0, 320.0, 25.0],
            Kind::Edit {
                caret: free.chars().count(),
                value: free.chars().collect(),
            },
        );
        f.enable("newName", false);
        f.add(
            "objectIn",
            None,
            [40.0, 138.0, 130.0, 20.0],
            Kind::Check {
                label: t("sheetChart.objectIn", "Object in:"),
                on: true,
            },
        );
        f.add(
            "sheetList",
            None,
            [180.0, 134.0, 320.0, 25.0],
            Kind::Combo {
                items: names,
                sel: visible_index,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.form = Some(f);
        self.form_kind = FormKind::MoveChart;
        true
    }

    fn move_chart_ok(&mut self) {
        let Some(index) = self.chart_sel else {
            self.form = None;
            return;
        };
        let Some(f) = self.form.as_ref() else { return };
        let cmd = if f.on("newSheet") {
            json!({ "cmd": "moveChartTo", "index": index, "newSheet": f.text("newName") })
        } else {
            // The list holds the visible sheets: map the choice back to a sheet index.
            let pick = f.sel("sheetList").unwrap_or(0);
            let to = self
                .view
                .sheet_names()
                .iter()
                .enumerate()
                .filter(|(_, n)| n.is_some())
                .nth(pick)
                .map_or(self.view.sheet(), |(i, _)| i);
            json!({ "cmd": "moveChartTo", "index": index, "to": to })
        };
        let Some(out) = self.view.query(cmd) else {
            self.status = self.view.take_error().unwrap_or_default();
            return;
        };
        self.view.changed();
        self.form = None;
        let (sheet, at) = (
            out["sheet"].as_u64().unwrap_or(0) as usize,
            out["index"].as_u64().unwrap_or(0) as usize,
        );
        // The moved chart stays selected on its sheet, as the reference shows it.
        self.set_chart_selection(None, false);
        if sheet != self.view.sheet() {
            self.view.set_sheet(sheet);
        }
        self.view.reload();
        self.set_chart_selection(Some(at), false);
    }
}
