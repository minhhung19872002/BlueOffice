//! BlueDoc parity round 14 (docs/ui/REPORT.md "Vòng 104"): References tab commands of the
//! Table of Authorities and Index groups (Mark Citation, Insert Table of Authorities,
//! Update Table, Update Index), Insert Table of Figures through its dialog, and Mark
//! Citation › Next Citation. A child module of the shell.
use super::DocWindow;
use crate::doc_forms::DocFormKind;
use serde_json::json;

#[cfg(test)]
#[path = "doc_r14_tests.rs"]
mod tests;

impl DocWindow {
    /// A round-14 command; `None` when `id` is not one.
    #[inline(never)]
    pub(super) fn r14_action(&mut self, id: &str) -> Option<bool> {
        if let Some(j) = id.strip_prefix("r14.categories:") {
            let names: Vec<String> = serde_json::from_str(j).unwrap_or_default();
            if let Some(f) = self.r3.form.as_mut() {
                f.r14_set_categories(&names);
            }
            self.r14.categories = Some(names);
            return Some(true);
        }
        if let Some(q) = id.strip_prefix("r14.findNext:") {
            return Some(self.r14_select_next(q));
        }
        Some(match id {
            "references.markCitation" => self.r3_open_form(DocFormKind::MarkCitation),
            "references.insertToa" => self.r3_open_form(DocFormKind::Toa),
            "references.tableOfFigures" => self.r3_open_form(DocFormKind::Tof),
            "design.watermark" => self.r14_open_watermarks(),
            "design.pageColor" => self.r14_open_page_colors(),
            "design.watermarkCustom" => self.r3_open_form(DocFormKind::Watermark),
            // §17.16.5 fields are refreshed together, as the reference's Update Table.
            "references.updateToa" | "references.updateIndex" => {
                self.r3_run(vec![json!({ "cmd": "updateFields" })])
            }
            _ => return None,
        })
    }

    /// Select the occurrence of `q` after the selected one (the first one otherwise).
    fn r14_select_next(&mut self, q: &str) -> bool {
        if q.trim().is_empty() {
            return false;
        }
        let matches = crate::catalog::from_json(json!({ "cmd": "find", "query": q }))
            .and_then(|c| self.view.exec(c))
            .and_then(|r| r.matches)
            .unwrap_or_default();
        let current = self.view.engine().session().map(|s| {
            let (a, b) = s.selection().ordered();
            (a.para, a.offset, b.offset)
        });
        let at = matches
            .iter()
            .position(|m| Some((m.para, m.start, m.end)) == current)
            .map_or(0, |i| (i + 1) % matches.len().max(1));
        let Some(m) = matches.get(at).copied() else {
            return false;
        };
        crate::catalog::from_json(json!({ "cmd": "selectMatch", "match": m }))
            .is_some_and(|c| self.view.run_command(c))
    }
}

/// Ribbon commands of round 14: id, `doc14.cmd_*` key, en-US label.
pub(crate) const COMMANDS: [(&str, &str, &str); 4] = [
    (
        "references.markCitation",
        "doc14.cmd_markCitation",
        "Mark Citation",
    ),
    (
        "references.insertToa",
        "doc14.cmd_insertToa",
        "Insert Table of Authorities",
    ),
    (
        "references.updateToa",
        "doc14.cmd_updateToa",
        "Update Table",
    ),
    (
        "references.updateIndex",
        "doc14.cmd_updateIndex",
        "Update Index",
    ),
];

/// Labels and BlueOffice's own icon drawings of the round-14 buttons (24-unit grid, the
/// BlueDoc family: a folded page with grey outlines, coloured marks).
pub(crate) fn install(icons: &mut crate::icons::Icons, s: &mut crate::strings::Strings) {
    use bo_render::Rgba;
    for (id, key, en) in COMMANDS {
        let v = s.get(key).to_owned();
        let label = if v == key { en.to_owned() } else { v };
        s.set(&format!("commands.{id}"), &label);
    }
    if s.get("doc14.groupAuthorities") == "doc14.groupAuthorities" {
        s.set("doc14.groupAuthorities", "Table of Authorities");
    }
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    let grey = rgb(0x6e, 0x6e, 0x6e);
    let blue = rgb(0x2f, 0x6f, 0xba);
    let white = rgb(255, 255, 255);
    let red = rgb(0xc0, 0x39, 0x2b);
    let green = rgb(0x2e, 0x8b, 0x57);
    let page = |icons: &mut crate::icons::Icons, name: &str, extra: &[(&str, Option<Rgba>)]| {
        let mut parts: Vec<(&str, Option<Rgba>)> = vec![
            ("M4 2h12l4 4v16h-16Z", white),
            ("M4 2h12l4 4v16h-16ZM5 3v18h14v-14.6l-3.4-3.4Z", grey),
        ];
        parts.extend_from_slice(extra);
        icons.add_solid(name, &parts);
    };
    // Mark Citation: a citation line marked by a red bar, a green plus at the corner and
    // indented blue entry lines.
    page(
        icons,
        "references.markCitation",
        &[
            ("M7 7h10v1.5h-10Z", grey),
            ("M7 11h10v2.5h-10Z", red),
            ("M10 15.5h5v1.3h-5ZM10 18h5v1.3h-5Z", blue),
            ("M7 15h1.5v5h-1.5Z", blue),
            ("M1.5 3.5h2v-2h1.5v2h2v1.5h-2v2h-1.5v-2h-2Z", green),
        ],
    );
    // Insert Table of Authorities: a bold category line over indented entries.
    page(
        icons,
        "references.insertToa",
        &[
            ("M7 6.5h8v2.2h-8Z", blue),
            ("M9 11h8v1.5h-8ZM9 14h6v1.5h-6ZM9 17h8v1.5h-8Z", grey),
        ],
    );
    // Update Table / Update Index: the page with a red exclamation mark.
    for name in ["references.updateToa", "references.updateIndex"] {
        page(
            icons,
            name,
            &[
                ("M7 7h10v1.5h-10Z", grey),
                ("M11 11h2v5h-2ZM11 17.5h2v2h-2Z", red),
            ],
        );
    }
}

/// Round-14 state of the window: the Design tab's Watermark gallery and Page Color palette.
#[derive(Default)]
pub(super) struct R14 {
    wm: Option<Box<crate::doc_wm14::WatermarkGallery>>,
    pal: Option<Box<crate::doc_wm14::PagePalette>>,
    /// Document version and whether it then had a table of figures (TOC `\c` / `\a`).
    tof_at: Option<(u64, bool)>,
    /// Citation category names renamed with Edit Category (this window).
    categories: Option<Vec<String>>,
}

impl DocWindow {
    fn r14_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    fn r14_anchor(&self, id: &'static str) -> [f32; 4] {
        self.ribbon
            .rect_of(crate::RibbonHit::Command(id))
            .unwrap_or([self.size.0 / 2.0, 120.0, 60.0, 90.0])
    }

    /// Design › Watermark: the gallery under its button.
    fn r14_open_watermarks(&mut self) -> bool {
        self.menu = None;
        self.r14.pal = None;
        let anchor = self.r14_anchor("design.watermark");
        let sets = crate::doc_wm14::wm_sets(&|k, en| self.r14_t(k, en));
        let footer = [
            self.r14_t("doc14.wmCustom", "Custom Watermark..."),
            self.r14_t("doc14.wmRemove", "Remove Watermark"),
            self.r14_t(
                "doc14.wmSaveSelection",
                "Save Selection to Watermark Gallery...",
            ),
        ];
        let current = self
            .view
            .engine()
            .session()
            .and_then(bo_edit::Session::watermark);
        let mut g = crate::doc_wm14::WatermarkGallery::open(
            anchor,
            self.size,
            self.scale,
            sets,
            footer,
            current.is_some(),
        );
        g.current = current.and_then(|w| Some((w.text?, w.diagonal)));
        // The texts as outlines of the body font's metric twin (one turned shape each).
        let fonts = self.view.engine().fonts();
        let outlines = g
            .texts()
            .into_iter()
            .filter_map(|t| {
                let (segs, adv) = crate::doc_wm14::text_outline(fonts, "Carlito", &t)?;
                Some((t, segs, adv))
            })
            .collect();
        g.set_outlines(outlines);
        self.r14.wm = Some(Box::new(g));
        true
    }

    /// Design › Page Color: the palette under its button.
    fn r14_open_page_colors(&mut self) -> bool {
        self.menu = None;
        self.r14.wm = None;
        let anchor = self.r14_anchor("design.pageColor");
        let doc = self.view.engine().document();
        // The document's theme colours as read, else those of a theme BlueOffice authored.
        let theme = doc
            .map(|d| d.design.theme_colors.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                doc.and_then(|d| d.design.theme_xml.clone())
                    .map(|x| bo_engine::theme_file::theme_colors(x.as_bytes()))
            })
            .unwrap_or_default();
        let current = doc.and_then(|d| d.design.background.map(bo_model::Color::to_hex));
        let labels = [
            self.r14_t("doc14.pcHighContrast", "High-contrast only"),
            self.r14_t("doc14.pcTheme", "Theme Colors"),
            self.r14_t("doc14.pcStandard", "Standard Colors"),
            self.r14_t("doc14.pcNone", "No Color"),
            self.r14_t("doc14.pcMore", "More Colors..."),
            self.r14_t("doc14.pcFill", "Fill Effects..."),
        ];
        let mut pal = crate::doc_wm14::PagePalette::open(
            anchor,
            self.size,
            self.scale,
            super::r12::palette(&theme),
            labels,
            current,
        );
        // Colours picked with More Colors… (round 15).
        pal.recent.clone_from(&self.r15.recent);
        pal.recent_label = self.r14_t("doc15.recentColors", "Recent Colors");
        self.r14.pal = Some(Box::new(pal));
        true
    }

    /// References tab: Show Notes is dimmed without notes and Captions › Update Table
    /// without a table of figures, as the reference.
    pub(super) fn r14_layout(&mut self) {
        if self.ribbon.active_label() != "ribbon.tabs.references" {
            return;
        }
        let Some(s) = self.view.engine().session() else {
            return;
        };
        let version = s.version();
        let doc = s.doc();
        let notes = doc.footnotes.keys().any(|k| *k > 0) || doc.endnotes.keys().any(|k| *k > 0);
        let tof = match self.r14.tof_at {
            Some((v, t)) if v == version => t,
            _ => {
                let t = bo_edit::fields::block_field_instrs(doc).iter().any(|i| {
                    let i = i.trim_start();
                    i.starts_with("TOC") && (i.contains("\\c") || i.contains("\\a"))
                });
                self.r14.tof_at = Some((version, t));
                t
            }
        };
        let mut dim = Vec::new();
        if !notes {
            dim.push("references.showNotes");
        }
        if !tof {
            dim.push("references.updateFigures");
        }
        self.ribbon.set_extra_dim(dim);
    }

    /// Input to an open round-14 drop-down (`None`: none open, or not for it).
    pub(super) fn r14_input(&mut self, input: &crate::Input) -> Option<bool> {
        use crate::doc_wm14::{PcPick, WmPick};
        use crate::{Input, Key};
        if let Some(g) = self.r14.wm.as_mut() {
            return Some(match *input {
                Input::Move { x, y } | Input::Drag { x, y } => g.hover(x, y),
                Input::Wheel(d) => g.wheel(d),
                Input::Press { x, y, .. } => match g.press(x, y) {
                    Some(WmPick::Nothing) => true,
                    Some(WmPick::Entry(e)) => {
                        self.r14.wm = None;
                        self.r3_run(vec![json!({ "cmd": "setWatermark", "watermark": {
                            "text": e.text, "semitransparent": true, "diagonal": e.diagonal,
                        } })])
                    }
                    Some(WmPick::Custom) => {
                        self.r14.wm = None;
                        self.r3_open_form(DocFormKind::Watermark)
                    }
                    Some(WmPick::Remove) => {
                        self.r14.wm = None;
                        self.r3_run(vec![json!({ "cmd": "setWatermark", "watermark": null })])
                    }
                    None => {
                        self.r14.wm = None;
                        true
                    }
                },
                Input::Key(Key::Escape, _) => {
                    self.r14.wm = None;
                    true
                }
                Input::Text(_) | Input::Key(..) | Input::Release { .. } => true,
                _ => return None,
            });
        }
        let p = self.r14.pal.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => p.hover(x, y),
            Input::Press { x, y, .. } => match p.press(x, y) {
                Some(PcPick::Nothing) => true,
                Some(PcPick::Color(hex)) => {
                    self.r14.pal = None;
                    self.r3_run(vec![json!({ "cmd": "setPageColor", "color": hex })])
                }
                Some(PcPick::None) => {
                    self.r14.pal = None;
                    self.r3_run(vec![json!({ "cmd": "setPageColor", "color": null })])
                }
                Some(PcPick::More) => {
                    self.r14.pal = None;
                    self.action("design.pageColor.more")
                }
                Some(PcPick::Fill) => {
                    self.r14.pal = None;
                    self.action("design.pageColor.fill")
                }
                None => {
                    self.r14.pal = None;
                    true
                }
            },
            Input::Key(Key::Escape, _) => {
                self.r14.pal = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Release { .. } | Input::Wheel(_) => true,
            _ => return None,
        })
    }

    pub(super) fn r14_paint(&self, p: &mut dyn crate::Painter) {
        if let Some(g) = &self.r14.wm {
            g.paint(p);
        }
        if let Some(pal) = &self.r14.pal {
            pal.paint(p);
        }
    }

    /// The `extra["r14"]` context of the round-14 dialogs: the document's watermark.
    pub(super) fn r14_form_extra(&mut self) -> serde_json::Value {
        let wm = self
            .view
            .engine()
            .session()
            .and_then(bo_edit::Session::watermark);
        json!({ "watermark": wm, "categories": self.r14.categories })
    }
}
