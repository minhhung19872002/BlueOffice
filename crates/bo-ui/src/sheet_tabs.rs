//! Sheet tabs and status bar choices of BlueSheet (round 37), as the reference:
//!
//! - tab colours (`setTabColor`, ECMA-376 Part 1, §18.3.1.93): an inactive tab is filled with
//!   its colour, the active one fades from white to 47 % of it towards the bottom (measured);
//! - double-click renames in place (the name selected, Enter / Esc / click elsewhere);
//! - dragging a tab moves the sheet (`moveSheet`, a black mark where it will go);
//! - the tab context menu: Insert, Delete, Rename, Move or Copy…, View Code, Protect Sheet…,
//!   Tab Color, Hide, Unhide…, Select All Sheets;
//! - the status bar context menu chooses the statistics shown for the selection (Average,
//!   Count, Numerical Count, Minimum, Maximum, Sum; Average / Count / Sum on by default).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Key, Painter, TextMeasure};
use bo_render::Rgba;
use serde_json::json;

/// Status bar statistics: (choice id, label key, fallback, on by default).
pub(super) const STATUS_ITEMS: [(&str, &str, &str, bool); 6] = [
    ("status.average", "sheetEditor.average", "Average", true),
    ("status.count", "sheetEditor.count", "Count", true),
    (
        "status.numCount",
        "sheetNative.numCount",
        "Numerical Count",
        false,
    ),
    ("status.min", "sheetNative.min", "Minimum", false),
    ("status.max", "sheetNative.max", "Maximum", false),
    ("status.sum", "sheetEditor.sum", "Sum", true),
];

/// In-place rename of a sheet tab.
#[derive(Debug, Clone)]
pub(super) struct TabEdit {
    pub sheet: usize,
    pub text: Vec<char>,
    /// The whole name is selected (typing replaces it).
    pub all: bool,
}

/// A tab being dragged: sheet, press x, where it would go (index) once moved.
#[derive(Debug, Clone, Copy)]
pub(super) struct TabDrag {
    pub sheet: usize,
    pub x0: f32,
    pub to: Option<usize>,
}

fn hex_rgba(h: &str) -> Option<Rgba> {
    let h = h.trim().trim_start_matches('#');
    let h = &h[h.len().saturating_sub(6)..];
    let v = u32::from_str_radix(h, 16).ok()?;
    (h.len() == 6).then_some(Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255))
}

impl SheetWindow {
    /// Tab colour of sheet `i` (resolved through the theme).
    pub(super) fn tab_color(&self, i: usize) -> Option<Rgba> {
        let wb = self.view.engine().workbook()?;
        let c = wb.sheets.get(i)?.props.tab_color.as_ref()?;
        let rgb = bo_sheet::color::resolve_color(c, wb.theme.as_ref())?;
        Some(Rgba(rgb.0, rgb.1, rgb.2, 255))
    }

    /// Statistics shown in the status bar for a selection.
    pub(super) fn status_item_on(&self, id: &str) -> bool {
        STATUS_ITEMS
            .iter()
            .position(|s| s.0 == id)
            .is_some_and(|i| self.status_items[i])
    }

    /// Tab under the pointer: (sheet, left, width).
    fn tab_at(&self, x: f32) -> Option<(usize, f32, f32)> {
        self.tab_rects
            .iter()
            .find(|(_, tx, tw)| x >= *tx && x < tx + tw)
            .copied()
    }

    /// Sheet tab input: rename in place, drag to move, double-click, context menu.
    pub(super) fn tabs_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        let status_top = self.size.1 - self.status_h();
        let tabs_top = status_top - self.tabs_h();
        let in_strip = |y: f32| y >= tabs_top && y < status_top;
        if self.tab_edit.is_some() {
            match input {
                Input::Text(s) => {
                    if let Some(e) = self.tab_edit.as_mut() {
                        if e.all {
                            e.text.clear();
                            e.all = false;
                        }
                        e.text.extend(s.chars().filter(|c| !c.is_control()));
                    }
                    return Some(true);
                }
                Input::Key(Key::Backspace, _) => {
                    if let Some(e) = self.tab_edit.as_mut() {
                        if e.all {
                            e.text.clear();
                            e.all = false;
                        } else {
                            e.text.pop();
                        }
                    }
                    return Some(true);
                }
                Input::Key(Key::Enter, _) => return Some(self.finish_tab_edit(true, text)),
                Input::Key(Key::Escape, _) => return Some(self.finish_tab_edit(false, text)),
                Input::Key(..) => return Some(false),
                Input::Press { x, y, .. }
                    if !(in_strip(*y)
                        && self.tab_at(*x).is_some_and(|t| {
                            Some(t.0) == self.tab_edit.as_ref().map(|e| e.sheet)
                        })) =>
                {
                    self.finish_tab_edit(true, text);
                }
                _ => {}
            }
        }
        match *input {
            Input::Press { x, y, count, .. } if in_strip(y) => {
                let (i, ..) = self.tab_at(x)?;
                if count >= 2 {
                    let name = self.view.sheet_names().get(i).cloned().flatten()?;
                    self.tab_edit = Some(TabEdit {
                        sheet: i,
                        text: name.chars().collect(),
                        all: true,
                    });
                    self.tab_drag = None;
                    return Some(true);
                }
                let changed = self.view.set_sheet(i);
                if changed {
                    self.sync_view_mode(text);
                }
                self.tab_drag = Some(TabDrag {
                    sheet: i,
                    x0: x,
                    to: None,
                });
                Some(true)
            }
            Input::Drag { x, .. } if self.tab_drag.is_some() => {
                let d = self.tab_drag?;
                if (x - d.x0).abs() < self.px(4.0) && d.to.is_none() {
                    return Some(false);
                }
                // Insertion point: before the first tab whose middle is right of the pointer.
                let to = self
                    .tab_rects
                    .iter()
                    .position(|(_, tx, tw)| x < tx + tw / 2.0)
                    .unwrap_or(self.tab_rects.len());
                if let Some(t) = self.tab_drag.as_mut() {
                    t.to = Some(to);
                }
                Some(true)
            }
            Input::Release { .. } if self.tab_drag.is_some() => {
                let d = self.tab_drag.take()?;
                let Some(slot) = d.to else {
                    return Some(false);
                };
                // `slot` counts visible tabs; turn it into a sheet index.
                let from = d.sheet;
                let target = if slot >= self.tab_rects.len() {
                    self.tab_rects.last().map_or(from, |t| t.0)
                } else {
                    let s = self.tab_rects[slot].0;
                    if s > from {
                        s - 1
                    } else {
                        s
                    }
                };
                if target == from {
                    return Some(true);
                }
                let ok = self
                    .view
                    .run(json!({ "cmd": "moveSheet", "from": from, "to": target }));
                if ok {
                    self.view.set_sheet(target);
                    self.layout_tabs(text);
                }
                Some(ok)
            }
            Input::Context { x, y } if in_strip(y) => {
                let (i, ..) = self.tab_at(x)?;
                if self.view.set_sheet(i) {
                    self.sync_view_mode(text);
                }
                self.tab_menu(x, y);
                Some(true)
            }
            Input::Context { x, y } if y >= status_top => {
                self.status_menu(x, y);
                Some(true)
            }
            _ => None,
        }
    }

    fn finish_tab_edit(&mut self, keep: bool, text: &dyn TextMeasure) -> bool {
        let Some(e) = self.tab_edit.take() else {
            return false;
        };
        if keep {
            let name: String = e.text.iter().collect();
            let name = name.trim();
            if !name.is_empty() {
                match self.view.engine_mut().rename_sheet(e.sheet, name) {
                    Ok(()) => self.view.changed(),
                    Err(err) => self.status = err.to_string(),
                }
            }
        }
        self.layout_tabs(text);
        true
    }

    fn tab_menu(&mut self, x: f32, y: f32) {
        let hidden = self.view.engine().workbook().is_some_and(|w| {
            w.sheets
                .iter()
                .any(|s| s.state != bo_model::sheet::SheetState::Visible)
        });
        let rows: [(&str, &str, &str, bool); 10] = [
            ("tab.insert", "sheetNative.tabInsert", "Insert…", true),
            ("tab.delete", "sheetNative.tabDelete", "Delete", true),
            ("tab.rename", "sheetNative.tabRename", "Rename", true),
            (
                "tab.moveCopy",
                "sheetNative.tabMoveCopy",
                "Move or Copy…",
                false,
            ),
            (
                "tab.viewCode",
                "sheetNative.tabViewCode",
                "View Code",
                false,
            ),
            (
                "sheet.protectSheet",
                "sheetNative.tabProtect",
                "Protect Sheet…",
                true,
            ),
            ("tab.color", "sheetNative.tabColor", "Tab Color", true),
            ("tab.hide", "sheetNative.tabHide", "Hide", true),
            ("tab.unhide", "sheetNative.tabUnhide", "Unhide…", hidden),
            (
                "tab.selectAll",
                "sheetNative.tabSelectAll",
                "Select All Sheets",
                false,
            ),
        ];
        let items = rows
            .iter()
            .map(|(id, key, fallback, on)| MenuItem {
                label: self.t(key, fallback),
                choice: Choice::Command((*id).to_owned()),
                checked: false,
                dim: !on,
            })
            .collect();
        let icons = vec![None; rows.len()];
        // The menu opens above the pointer, as the reference's tab menu.
        self.menu = Some(
            Menu::open(items, [x, y - 2.0, 1.0, 0.0], self.size, self.scale)
                .with_decor(icons, vec![5, 9]),
        );
    }

    fn status_menu(&mut self, x: f32, y: f32) {
        let items = STATUS_ITEMS
            .iter()
            .enumerate()
            .map(|(i, (id, key, fallback, _))| MenuItem {
                label: self.t(key, fallback),
                choice: Choice::Command((*id).to_owned()),
                checked: self.status_items[i],
                dim: false,
            })
            .collect();
        self.menu = Some(Menu::open(
            items,
            [x, y - 2.0, 1.0, 0.0],
            self.size,
            self.scale,
        ));
    }

    /// Tab and status bar menu commands.
    pub(super) fn tabs_action(&mut self, id: &str, text: &dyn TextMeasure) -> Option<bool> {
        let sheet = self.view.sheet();
        if let Some(i) = STATUS_ITEMS.iter().position(|s| s.0 == id) {
            self.status_items[i] = !self.status_items[i];
            return Some(true);
        }
        Some(match id {
            "tab.insert" => {
                let done = self.view.add_sheet();
                self.layout_tabs(text);
                done
            }
            "tab.delete" => {
                let n = self.view.sheet_names().iter().flatten().count();
                if n <= 1 {
                    return Some(false);
                }
                let ok = self.view.engine_mut().delete_sheet(sheet).is_ok();
                if ok {
                    let next = sheet.saturating_sub(1);
                    self.view.changed();
                    self.view.set_sheet_index(next);
                    self.layout_tabs(text);
                }
                ok
            }
            "tab.rename" => {
                let name = self.view.sheet_names().get(sheet).cloned().flatten()?;
                self.tab_edit = Some(TabEdit {
                    sheet,
                    text: name.chars().collect(),
                    all: true,
                });
                true
            }
            "tab.hide" => {
                let ok = self
                    .view
                    .run(json!({ "cmd": "setSheetHidden", "hidden": true }));
                if ok {
                    let next = self
                        .view
                        .sheet_names()
                        .iter()
                        .position(Option::is_some)
                        .unwrap_or(0);
                    self.view.set_sheet_index(next);
                    self.layout_tabs(text);
                } else {
                    self.status = self.view.take_error().unwrap_or_default();
                }
                ok
            }
            "tab.unhide" => self.open_unhide(),
            "tab.color" => self.open_tab_color(),
            _ => return None,
        })
    }

    fn open_unhide(&mut self) -> bool {
        let names: Vec<(usize, String)> = self
            .view
            .engine()
            .workbook()
            .map(|w| {
                w.sheets
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.state == bo_model::sheet::SheetState::Hidden)
                    .map(|(i, s)| (i, s.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if names.is_empty() {
            return false;
        }
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetNative.unhideTitle", "Unhide"),
            (330.0, 300.0),
            vec![],
        );
        f.add(
            "lbl",
            None,
            [16.0, 48.0, 290.0, 20.0],
            Kind::Label(t("sheetNative.unhideSheet", "Unhide sheet:")),
        );
        f.add(
            "list",
            None,
            [16.0, 72.0, 296.0, 160.0],
            Kind::List {
                items: names.iter().map(|n| n.1.clone()).collect(),
                sel: Some(0),
                top: 0,
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        self.unhide_list = names.into_iter().map(|n| n.0).collect();
        self.form = Some(f);
        self.form_kind = FormKind::Unhide;
        true
    }

    fn open_tab_color(&mut self) -> bool {
        let palette = self.palette_hexes();
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(
            &t("sheetNative.tabColor", "Tab Color"),
            (300.0, 270.0),
            vec![],
        );
        let colors: Vec<Rgba> = palette.iter().filter_map(|h| hex_rgba(h)).collect();
        f.add(
            "swatches",
            None,
            [16.0, 48.0, 268.0, 160.0],
            Kind::Swatches {
                colors,
                cols: 10,
                sel: None,
                breaks: vec![0, 5],
            },
        );
        f.add(
            "none",
            None,
            [16.0, 222.0, 120.0, 28.0],
            Kind::Button {
                label: t("sheetNative.noColor", "No Color"),
                default: false,
            },
        );
        f.add(
            "cancel",
            None,
            [172.0, 222.0, 112.0, 28.0],
            Kind::Button {
                label: t("common.cancel", "Cancel"),
                default: false,
            },
        );
        self.tab_palette = palette;
        self.form = Some(f);
        self.form_kind = FormKind::TabColor;
        true
    }

    /// Events of Unhide and Tab Color.
    pub(super) fn tabs_form_event(&mut self, kind: FormKind, ev: &Event) {
        match (kind, ev) {
            (FormKind::Unhide, Event::Ok | Event::Clicked("list")) => {
                let pick = self
                    .form
                    .as_ref()
                    .and_then(|f| f.sel("list"))
                    .and_then(|i| self.unhide_list.get(i).copied());
                self.form = None;
                if let Some(s) = pick {
                    if self
                        .view
                        .run(json!({ "cmd": "setSheetHidden", "sheet": s, "hidden": false }))
                    {
                        self.view.set_sheet_index(s);
                    }
                }
            }
            (FormKind::TabColor, Event::Changed("swatches")) => {
                let pick = self
                    .form
                    .as_ref()
                    .and_then(|f| f.sel("swatches"))
                    .and_then(|i| self.tab_palette.get(i).cloned());
                self.form = None;
                if let Some(h) = pick {
                    self.view.run(json!({ "cmd": "setTabColor", "color": h }));
                }
            }
            (FormKind::TabColor, Event::Clicked("none")) => {
                self.form = None;
                self.view
                    .run(json!({ "cmd": "setTabColor", "color": null }));
            }
            (_, Event::Cancel | Event::Ok | Event::Clicked("cancel")) => self.form = None,
            _ => {}
        }
    }

    /// Paint one sheet tab's background: its colour (inactive) or the faded colour under a
    /// white top (active), as measured.
    pub(super) fn paint_tab_fill(
        &self,
        p: &mut dyn Painter,
        i: usize,
        rect: [f32; 4],
        active: bool,
    ) {
        let Some(c) = self.tab_color(i) else {
            return;
        };
        let [x, y, w, h] = rect;
        if !active {
            p.fill_rect(x, y + 1.0, w - 1.0, h - 1.0, c);
            return;
        }
        // White over the top 15 %, then towards 47 % of the colour at the bottom.
        let rows = h.round().max(1.0) as i32;
        let start = (h * 0.15).round() as i32;
        for r in start..rows {
            let t = 0.47 * (r - start + 1) as f32 / (rows - start).max(1) as f32;
            let mix = |a: u8| (255.0 + (f32::from(a) - 255.0) * t).round() as u8;
            p.fill_rect(
                x + 1.0,
                y + r as f32,
                w - 2.0,
                1.0,
                Rgba(mix(c.0), mix(c.1), mix(c.2), 255),
            );
        }
    }

    /// The rename box over a tab and the drop mark of a dragged tab.
    pub(super) fn paint_tab_extras(&self, p: &mut dyn Painter) {
        let y = self.size.1 - self.status_h() - self.tabs_h();
        let h = self.tabs_h();
        if let Some(e) = &self.tab_edit {
            if let Some((_, x, w)) = self.tab_rects.iter().find(|t| t.0 == e.sheet) {
                let style = self.tab_style(true);
                let s: String = e.text.iter().collect();
                let lw = self
                    .tab_label_w(&s, true)
                    .max(s.chars().count() as f32 * style.size * 0.5);
                let bw = (lw + self.px(8.0)).max(*w - self.px(4.0));
                let (bx, by, bh) = (x + self.px(2.0), y + self.px(3.0), h - self.px(8.0));
                p.fill_rect(bx, by, bw, bh, Rgba(255, 255, 255, 255));
                let line = style.size * 1.33;
                let ty = (by + (bh - line) / 2.0).round();
                if e.all {
                    p.fill_rect(bx + self.px(3.0), ty, lw, line, Rgba(0x00, 0x78, 0xd7, 255));
                }
                let ink = if e.all {
                    Rgba(255, 255, 255, 255)
                } else {
                    Rgba(0, 0, 0, 255)
                };
                p.text(&s, bx + self.px(3.0), ty, style, ink);
                if !e.all {
                    p.fill_rect(
                        bx + self.px(3.0) + lw + 1.0,
                        ty,
                        1.0,
                        line,
                        Rgba(0, 0, 0, 255),
                    );
                }
            }
        }
        if let Some(TabDrag { to: Some(slot), .. }) = self.tab_drag {
            let x = self
                .tab_rects
                .get(slot)
                .map(|t| t.1)
                .or_else(|| self.tab_rects.last().map(|t| t.1 + t.2))
                .unwrap_or(0.0);
            let k = self.px(4.0);
            p.path(
                &[
                    crate::PathSeg::M(x - k, y),
                    crate::PathSeg::L(x + k, y),
                    crate::PathSeg::L(x, y + k),
                    crate::PathSeg::Z,
                ],
                Some(Rgba(0, 0, 0, 255)),
                None,
            );
        }
    }
}
