//! BlueDoc parity round 12 (docs/ui/REPORT.md "Vòng 102"): Borders and Shading (from Home ›
//! Borders ▾, Table Design › Borders ▾, Design › Page Borders and Table Properties), Table
//! Positioning, table alt text. A child module of the shell.
use super::DocWindow;
use crate::doc_forms::DocFormKind;
use bo_engine::edit::Command;
use serde_json::{json, Value};

/// Round-12 state of the window.
#[derive(Default)]
pub(super) struct R12 {
    /// The next Borders and Shading opens on its Page Border tab (Design › Page Borders).
    page_tab: bool,
    /// Insert › Table › Quick Tables, when open.
    pub(super) quick: Option<Box<crate::doc_quick12::QuickTables>>,
}

/// `{style, size, color}` of each edge of `b` (`top`, `left`, `bottom`, `right`, `insideH`,
/// `insideV`).
fn borders_json(b: Option<&bo_model::Borders>) -> Value {
    let Some(b) = b else {
        return Value::Null;
    };
    let line = |l: &Option<bo_model::Border>| {
        l.as_ref().map_or(Value::Null, |l| {
            json!({ "style": l.style, "size": l.size, "color": l.color.map(bo_model::Color::to_hex),
                "shadow": l.shadow, "frame": l.frame })
        })
    };
    json!({
        "top": line(&b.top), "left": line(&b.left), "bottom": line(&b.bottom),
        "right": line(&b.right), "insideH": line(&b.inside_h), "insideV": line(&b.inside_v),
    })
}

/// Lighter (`tint` > 0) or darker (`tint` < 0) shade of `hex`.
fn shade(hex: &str, tint: f32) -> String {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    let ch = |c: u32| {
        let c = c as f32;
        let out = if tint >= 0.0 {
            c + (255.0 - c) * tint
        } else {
            c * (1.0 + tint)
        };
        out.round().clamp(0.0, 255.0) as u32
    };
    format!(
        "{:02X}{:02X}{:02X}",
        ch(v >> 16 & 255),
        ch(v >> 8 & 255),
        ch(v & 255)
    )
}

/// The colour drop-downs' palette: the document theme's ten colours (background, text,
/// background 2, text 2, accents 1–6) with five tints / shades each, then ten standard
/// colours (own list of plain hues).
pub(super) fn palette(theme: &[(String, bo_model::Color)]) -> Vec<String> {
    let get = |name: &str, fallback: &str| {
        theme
            .iter()
            .find(|(n, _)| n == name)
            .map_or_else(|| fallback.to_owned(), |(_, c)| c.to_hex())
    };
    let base = [
        get("lt1", "FFFFFF"),
        get("dk1", "000000"),
        get("lt2", "EEECE1"),
        get("dk2", "1F3A5F"),
        get("accent1", "2E75B6"),
        get("accent2", "C55A11"),
        get("accent3", "7F7F7F"),
        get("accent4", "BF9000"),
        get("accent5", "2F5597"),
        get("accent6", "548235"),
    ];
    let mut out: Vec<String> = base.to_vec();
    for row in 0..5 {
        for (i, b) in base.iter().enumerate() {
            let t = match i {
                0 => [-0.05, -0.15, -0.25, -0.35, -0.5][row],
                1 => [0.5, 0.35, 0.25, 0.15, 0.05][row],
                2 => [-0.1, -0.25, -0.5, -0.75, -0.9][row],
                _ => [0.8, 0.6, 0.4, -0.25, -0.5][row],
            };
            out.push(shade(b, t));
        }
    }
    for h in [
        "B00000", "FF0000", "FFB000", "FFFF00", "8CD050", "00B050", "00B0F0", "0070C0", "002060",
        "7030A0",
    ] {
        out.push(h.to_owned());
    }
    out
}

impl DocWindow {
    /// A round-12 command; `None` when `id` is not one.
    #[inline(never)]
    pub(super) fn r12_action(&mut self, id: &str) -> Option<bool> {
        if let Some(spec) = id.strip_prefix("blocks12.save:") {
            return Some(self.r12_save_block(spec));
        }
        if let Some(rest) = id.strip_prefix("style12.") {
            return Some(self.r12_style_command(rest));
        }
        match id {
            "format.bordersShading" | "table.bordersShading" => {
                Some(self.r12_open(DocFormKind::BordersShading))
            }
            "design.pageBorders" => {
                self.r12.page_tab = true;
                Some(self.r12_open(DocFormKind::BordersShading))
            }
            "table.positioning" => Some(self.r12_open(DocFormKind::TablePosition)),
            "insert.saveQuickTable" => Some(self.r12_open(DocFormKind::NewBlock)),
            // Insert › Table › Quick Tables, beside the Insert Table button (scripted captures).
            "insert.quickTables" => {
                let r = self
                    .ribbon
                    .rect_of(crate::ribbon::RibbonHit::Command("insert.table"))
                    .unwrap_or([0.0, 0.0, 1.0, 1.0]);
                Some(self.r12_open_quick([r[0], r[1] + r[3], 283.0 * self.scale / 1.25, 36.0]))
            }
            _ => None,
        }
    }

    /// Open the Quick Tables gallery beside the Quick Tables row `anchor`.
    pub(super) fn r12_open_quick(&mut self, anchor: [f32; 4]) -> bool {
        let labels = [
            self.t("doc12.builtIn", "Built-In"),
            self.t(
                "doc12.saveQuick",
                "Save Selection to Quick Tables Gallery...",
            ),
            self.t("doc12.general", "General"),
        ];
        // The user's saved Quick Tables (the building blocks file in the user's profile).
        let user: Vec<crate::doc_quick12::QuickEntry> = self
            .view
            .run_json(&json!({ "cmd": "buildingBlocks" }))
            .and_then(|v| v["blocks"]["list"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter(|b| b["store"] == "user" && b["gallery"] == "tbls")
            .map(|b| {
                let key = b["key"].as_str().unwrap_or("").to_owned();
                // A saved table shows a picture of itself, as the built-in designs (round 13).
                let (table, title) = self
                    .view
                    .engine()
                    .block_table(&key)
                    .map_or((None, None), |(t, title)| (Some(t), title));
                crate::doc_quick12::QuickEntry {
                    key,
                    name: b["name"].as_str().unwrap_or("").to_owned(),
                    title,
                    table,
                    text: b["text"].as_str().unwrap_or("").to_owned(),
                    user: true,
                }
            })
            .collect();
        let footer_on = self.view.has_selection();
        self.r12.quick = Some(Box::new(crate::doc_quick12::QuickTables::open(
            anchor, self.size, labels, self.scale, user, footer_on,
        )));
        true
    }

    /// Create New Building Block OK: save the selection in the user's building blocks file
    /// (`spec` as JSON) and ask the host to store the file.
    fn r12_save_block(&mut self, spec: &str) -> bool {
        let Ok(spec) = serde_json::from_str::<Value>(spec) else {
            return false;
        };
        let out = self.view.run_json(&json!({
            "cmd": "saveBuildingBlock", "spec": spec, "store": "user",
        }));
        if let Some(file) = out.as_ref().and_then(|v| v["blocks"]["store"].as_str()) {
            self.requests
                .push(super::Request::SaveBlockStore(file.to_owned()));
            return true;
        }
        false
    }

    /// Load the user's building blocks file (`.dotx`, base64) kept by the host.
    pub fn load_block_store(&mut self, base64: &str) -> bool {
        self.view
            .run_json(&json!({ "cmd": "loadBlockStore", "file": base64 }))
            .is_some()
    }

    /// Input while the Quick Tables gallery is open; `None` lets it through.
    #[inline(never)]
    pub(super) fn r12_input(&mut self, input: &crate::Input) -> Option<bool> {
        use crate::{Input, Key};
        let q = self.r12.quick.as_mut()?;
        // Opened on hover from the table menu: outside the gallery the menu takes the pointer.
        if let Input::Move { x, y } | Input::Drag { x, y } | Input::Press { x, y, .. } = *input {
            if self.table_grid.is_some() && !q.contains(x, y) {
                return None;
            }
        }
        if matches!(input, Input::Press { .. }) {
            self.table_grid = None;
        }
        let q = self.r12.quick.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => q.hover(x, y),
            Input::Wheel(d) => q.wheel(d),
            Input::Press { x, y, .. } => match q.press(x, y) {
                Some(key) if key.is_empty() => true,
                Some(key) if key == "save" => {
                    self.r12.quick = None;
                    self.r12_open(DocFormKind::NewBlock)
                }
                Some(key) => {
                    self.r12.quick = None;
                    self.activate_context = true;
                    let v = json!({ "cmd": "insertBuildingBlock", "key": key });
                    self.view.run_json(&v);
                    true
                }
                None => {
                    self.r12.quick = None;
                    true
                }
            },
            Input::Key(Key::Escape, _) => {
                self.r12.quick = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Release { .. } => true,
            _ => return None,
        })
    }

    pub(super) fn r12_paint(&self, p: &mut dyn crate::Painter) {
        if let Some(q) = &self.r12.quick {
            q.paint(p);
        }
    }

    /// Paragraphs (and runs) of the document in style `id`.
    fn r12_style_uses(&self, id: &str) -> usize {
        let Some(doc) = self.view.engine().document() else {
            return 0;
        };
        let mut n = 0;
        for path in bo_model::index::body_paragraphs(doc) {
            if let Some(p) = bo_model::index::para_at(doc, &path) {
                if p.props.style.as_deref() == Some(id) {
                    n += 1;
                }
                n += p
                    .runs
                    .iter()
                    .filter(|r| r.props.style.as_deref() == Some(id))
                    .count();
            }
        }
        n
    }

    /// Right-click on style tile `i` of the gallery: Update to Match Selection, Modify…,
    /// Select All N Instance(s), Rename…, Remove from Style Gallery (the reference's menu).
    #[inline(never)]
    pub(super) fn r12_style_menu(&mut self, i: usize, x: f32, y: f32) -> bool {
        use crate::popup::{Choice, Menu, MenuItem, MenuLook};
        let Some(st) = self.styles.get(i) else {
            return false;
        };
        let (id, name) = (st.id.clone(), st.name.clone());
        let uses = self.r12_style_uses(&id);
        let fill = |s: String, n: &str| s.replace("{name}", n);
        let rows: Vec<(String, String, bool)> = vec![
            (
                fill(
                    self.t("doc12.updateToMatch", "Update {name} to Match Selection"),
                    &name,
                ),
                format!("style12.update.{id}"),
                false,
            ),
            (
                self.t("doc12.modify", "Modify..."),
                format!("styles.modify:{id}"),
                false,
            ),
            (
                if uses == 0 {
                    self.t("doc12.selectAllNone", "Select All: (No Data)")
                } else {
                    self.t("doc12.selectAll", "Select All {n} Instance(s)")
                        .replace("{n}", &uses.to_string())
                },
                format!("style12.selectAll.{id}"),
                uses == 0,
            ),
            (
                self.t("doc12.rename", "Rename..."),
                format!("styles.modify:{id}"),
                false,
            ),
            (
                fill(
                    self.t("doc12.removeGallery", "Remove from Style Gallery"),
                    &name,
                ),
                format!("style12.remove.{id}"),
                false,
            ),
        ];
        let items: Vec<MenuItem> = rows
            .into_iter()
            .map(|(label, cmd, dim)| MenuItem {
                label,
                choice: Choice::Command(cmd),
                checked: false,
                dim,
            })
            .collect();
        let n = items.len();
        let look = MenuLook {
            item_h: 36.0 / 1.25,
            min_w: 240.0 / 1.25,
            ..MenuLook::default()
        };
        self.menu = Some(
            Menu::open_with(items, [x, y, 1.0, 0.0], self.size, self.scale, look)
                .with_decor(vec![None; n], vec![])
                .with_soft_shadow(),
        );
        true
    }

    /// A style gallery menu command: `update.<id>`, `selectAll.<id>`, `remove.<id>`.
    #[inline(never)]
    fn r12_style_command(&mut self, rest: &str) -> bool {
        let Some((what, id)) = rest.split_once('.') else {
            return false;
        };
        match what {
            "update" => self.r3_run(vec![json!({ "cmd": "styleUpdateToMatch", "id": id })]),
            "selectAll" => self.r3_run(vec![json!({ "cmd": "styleSelectAll", "id": id })]),
            "remove" => {
                let Some(mut spec) = self
                    .view
                    .run_json(&json!({ "cmd": "styleDetails", "id": id }))
                    .map(|v| v["styleMgr"]["spec"].clone())
                    .filter(Value::is_object)
                else {
                    return false;
                };
                spec["addToGallery"] = json!(false);
                self.r3_run(vec![
                    json!({ "cmd": "styleModify", "id": id, "spec": spec }),
                ])
            }
            _ => false,
        }
    }

    #[inline(never)]
    fn r12_open(&mut self, kind: DocFormKind) -> bool {
        self.menu = None;
        self.r3_open_form(kind)
    }

    /// The `extra` context of the round-12 dialogs: selection, borders and shading of the
    /// paragraph, run, cell and table at the caret, the page borders and the palette.
    #[inline(never)]
    pub(super) fn r12_form_extra(&mut self) -> Value {
        let page_tab = std::mem::take(&mut self.r12.page_tab);
        let f = self.view.format().clone();
        let theme = self
            .view
            .engine()
            .document()
            .and_then(|d| d.design.theme_xml.clone())
            .map(|x| bo_engine::theme_file::theme_colors(x.as_bytes()))
            .unwrap_or_default();
        let page = self
            .view
            .exec(Command::Design)
            .and_then(|r| r.design)
            .and_then(|d| d.borders.clone())
            .and_then(|b| serde_json::to_value(b).ok())
            .unwrap_or(Value::Null);
        let para = f.para_borders.as_deref();
        let space = para.map(|b| {
            let s = |l: &Option<bo_model::Border>, d: u32| l.as_ref().map_or(d, |l| l.space);
            json!([s(&b.top, 1), s(&b.left, 4), s(&b.bottom, 1), s(&b.right, 4)])
        });
        let run = f.advanced.border.as_ref().map(|b| {
            let l = json!({ "style": b.style, "size": b.size, "color": b.color });
            json!({ "top": l, "left": l, "bottom": l, "right": l })
        });
        let multi_para = self.view.engine().session().is_some_and(|s| {
            let (a, b) = s.selection().ordered();
            a.para != b.para
        });
        let mut cell_borders =
            borders_json(f.table.as_ref().and_then(|t| t.cell_borders.as_deref()));
        if let Some(t) = &f.table {
            let diagonals = t.cell_diagonals.as_deref().cloned().unwrap_or_default();
            for (k, d) in ["tl2br", "tr2bl"].iter().zip(&diagonals) {
                if let Some(l) = d {
                    if !cell_borders.is_object() {
                        cell_borders = json!({});
                    }
                    cell_borders[*k] = json!({ "style": l.style, "size": l.size,
                        "color": l.color.map(bo_model::Color::to_hex) });
                }
            }
        }
        json!({
            "hasSelection": self.view.has_selection(),
            "multiPara": multi_para,
            "multiCell": f.table.as_ref().is_some_and(|t| t.multi_cell),
            "paraBorders": borders_json(para),
            "paraSpace": space,
            "paraShading": f.shading,
            "runBorders": run,
            "runShading": f.advanced.shading,
            "cellBorders": cell_borders,
            "tableBorders": borders_json(f.table.as_ref().and_then(|t| t.borders.as_deref())),
            "cellShading": f.table.as_ref().and_then(|t| t.shading.clone()),
            "pageBorders": page,
            "pageTab": page_tab,
            "palette": palette(&theme),
        })
    }
}

#[cfg(test)]
#[path = "doc_r12_tests.rs"]
mod tests;
