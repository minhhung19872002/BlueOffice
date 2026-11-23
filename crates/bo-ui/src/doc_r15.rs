//! BlueDoc parity round 15 (docs/ui/REPORT.md "Vòng 105"): the Colors dialog (More Colors…)
//! behind Page Color and the colour drop-downs of BlueDoc's dialogs, with the recent colours
//! it leaves in the Page Color palette. A child module of the shell.
use super::DocWindow;
use crate::doc_forms::DocFormKind;
use serde_json::json;

#[cfg(test)]
#[path = "doc_r15_tests.rs"]
mod tests;

/// Round-15 state of the window.
#[derive(Default)]
pub(super) struct R15 {
    /// Colours picked with More Colors… (newest first, at most 10), shown as Recent Colors.
    pub(super) recent: Vec<String>,
    /// A dialog kept under the one More Colors… came from (Table Properties under Borders
    /// and Shading), back in place when the Colors dialog closes.
    pub(super) deep: Option<crate::doc_forms::DocForm>,
    /// The Colors dialog changes the page colour (opened from Page Color).
    pub(super) page: bool,
    /// Outline collapse in the editor (view state).
    pub(super) col: Collapse,
    /// The open Design › Paragraph Spacing menu.
    pub(super) spacing: Option<Box<crate::doc_spacing15::SpacingMenu>>,
    /// The document's default spacing when opened (Paragraph Spacing › Default).
    pub(super) spacing_default: Option<(i32, i32, String)>,
    /// Custom theme colour / font sets saved (Custom 1, Custom 2 …).
    pub(super) custom: usize,
}

/// Headings collapsed in the editor: the blocks under them are hidden from the layout.
#[derive(Default)]
pub(super) struct Collapse {
    /// Collapsed headings (body paragraph ids).
    pub(super) heads: Vec<usize>,
    /// The heading under the pointer: its id, whether it is collapsed and its triangle's
    /// rectangle (window pixels).
    pub(super) hover: Option<(usize, bool, [f32; 4])>,
    /// The document was looked at for Collapsed by default.
    opened: bool,
    /// Document version and body paragraph count last seen, the hidden blocks then.
    seen: Option<(u64, usize)>,
    want: Vec<(usize, usize)>,
    /// Caret paragraph last seen (a caret moved under a collapsed heading expands it).
    caret: Option<usize>,
}

impl DocWindow {
    /// A round-15 command; `None` when `id` is not one.
    #[inline(never)]
    pub(super) fn r15_action(&mut self, id: &str) -> Option<bool> {
        if let Some(hex) = id.strip_prefix("r15.recent:") {
            let hex = hex.to_ascii_uppercase();
            self.r15.recent.retain(|h| *h != hex);
            self.r15.recent.insert(0, hex);
            self.r15.recent.truncate(10);
            return Some(true);
        }
        if let Some(hex) = id.strip_prefix("r15.colorBack:") {
            if let Some(f) = self.r3.form.as_mut() {
                f.r15_set_more_color(hex);
            }
            return Some(true);
        }
        // Scripted captures: a colour picked in the open Colors dialog.
        if let Some(hex) = id.strip_prefix("r15.pickColor:") {
            if let Some(f) = self.r3.form.as_mut() {
                f.r15_pick(hex);
            }
            return Some(true);
        }
        // Scripted captures: show tab N of the open dialog.
        if let Some(n) = id.strip_prefix("r15.formTab:") {
            if let (Some(f), Ok(n)) = (self.r3.form.as_mut(), n.parse()) {
                f.set_tab(n);
            }
            return Some(true);
        }
        Some(match id {
            "design.paragraphSpacing" => self.r15_open_spacing(),
            // Design › Themes / Colors / Fonts command rows.
            "design.footer.themes.0" => {
                // Reset to Theme from Template: the theme new documents start with.
                let id = self
                    .r3
                    .defaults
                    .as_ref()
                    .and_then(|d| d.0.clone())
                    .or_else(|| bo_edit::design::THEMES.first().map(|t| t.id.to_owned()))
                    .unwrap_or_default();
                self.design_theme = Some(id.clone());
                self.r3_run(vec![json!({ "cmd": "applyTheme", "id": id })])
            }
            "design.footer.themes.1" => {
                self.requests.push(super::Request::PickThemeFile);
                true
            }
            "design.footer.themes.2" => {
                self.requests
                    .push(super::Request::SaveAsType("thmx".to_owned()));
                true
            }
            "design.footer.colors.0" => {
                self.r15.custom += 1;
                self.r3_open_form(DocFormKind::ThemeColors)
            }
            "design.footer.fonts.0" => {
                self.r15.custom += 1;
                self.r3_open_form(DocFormKind::ThemeFonts)
            }
            "design.pageColor.more" => {
                self.r15.page = true;
                self.r3_open_form(DocFormKind::Colors)
            }
            _ => return None,
        })
    }

    /// The `extra["r15"]` context of the round-15 dialogs: the colour the Colors dialog
    /// starts from and what it changes.
    pub(super) fn r15_form_extra(&mut self) -> serde_json::Value {
        let from_form = self
            .r3
            .stash
            .as_ref()
            .and_then(crate::doc_forms::DocForm::r15_more_color);
        let font = std::mem::take(&mut self.r16.font_more);
        let color = match from_form {
            // Home › Font Color › More Colors… (round 16).
            _ if font => json!({ "target": "font", "current": self.r16.font_color }),
            Some(hex) if !self.r15.page => json!({ "target": "form", "current": hex }),
            _ => {
                let page = self
                    .view
                    .engine()
                    .document()
                    .and_then(|d| d.design.background.map(bo_model::Color::to_hex));
                json!({ "target": "page", "current": page })
            }
        };
        self.r15.page = false;
        let doc = self.view.engine().document();
        let slots = [
            "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5",
            "accent6", "hlink", "folHlink",
        ];
        let scheme: Vec<Option<String>> = slots
            .iter()
            .map(|k| {
                doc.and_then(|d| {
                    d.design
                        .theme_colors
                        .iter()
                        .find(|(n, _)| n == k)
                        .map(|(_, c)| c.to_hex())
                })
            })
            .collect();
        let (major, minor) = doc.map_or((None, None), |d| {
            let f = &d.theme_fonts;
            (
                Some(f.major_latin.clone()).filter(|s| !s.is_empty()),
                Some(f.minor_latin.clone()).filter(|s| !s.is_empty()),
            )
        });
        let fonts: Vec<String> = if self.r3.stash.is_none() {
            self.view.font_list().into_iter().map(|f| f.name).collect()
        } else {
            Vec::new()
        };
        json!({
            "color": color, "scheme": scheme, "major": major, "minor": minor,
            "fonts": fonts, "customName": format!("Custom {}", self.r15.custom.max(1)),
        })
    }

    /// Before the Colors dialog is stashed over a dialog: keep a dialog already kept under
    /// that one.
    pub(super) fn r15_keep_stash(&mut self, kind: DocFormKind) {
        if kind == DocFormKind::Colors {
            if let Some(under) = self.r3.stash.take() {
                self.r15.deep = Some(under);
            }
        }
    }

    /// After a dialog closed back to the one under it: the Colors dialog gives back the
    /// dialog kept deeper.
    pub(super) fn r15_restore_stash(&mut self, closed: Option<DocFormKind>) {
        if closed == Some(DocFormKind::Colors) && self.r3.stash.is_none() {
            self.r3.stash = self.r15.deep.take();
        }
    }
}

/// Outline level (0 = Heading 1) of body paragraph `path` when it is a top-level heading.
fn heading_level(doc: &bo_model::Document, path: &[usize]) -> Option<u8> {
    if path.len() != 2 {
        return None;
    }
    let p = bo_model::index::para_at(doc, path)?;
    bo_edit::fields::outline_level(doc, p)
}

/// The paragraph has Collapsed by default ([MS-DOCX] 2.6.1.1 `w15:collapsed`), its own or
/// through its style.
fn collapsed_by_default(doc: &bo_model::Document, p: &bo_model::Paragraph) -> bool {
    p.props.collapsed.unwrap_or_else(|| {
        p.props
            .style
            .as_deref()
            .and_then(|id| doc.styles.resolve_paragraph(id).0.collapsed)
            .unwrap_or(false)
    })
}

/// The top-level blocks hidden under collapsed headings `heads` (body paragraph ids): from
/// the block after each heading up to the next paragraph of the same or a higher outline
/// level, as the reference's Collapse.
pub(super) fn hidden_under(
    doc: &bo_model::Document,
    paths: &[Vec<usize>],
    heads: &[usize],
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for &h in heads {
        let Some(path) = paths.get(h) else {
            continue;
        };
        let Some(level) = heading_level(doc, path) else {
            continue;
        };
        let (s, b) = (path[0], path[1]);
        let Some(blocks) = doc.sections.get(s).map(|sec| &sec.blocks) else {
            continue;
        };
        for (j, blk) in blocks.iter().enumerate().skip(b + 1) {
            if let bo_model::Block::Paragraph(p) = blk {
                if bo_edit::fields::outline_level(doc, p).is_some_and(|m| m <= level) {
                    break;
                }
            }
            out.push((s, j));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

impl DocWindow {
    /// Layout pass: apply Collapsed by default once, follow edits (ids move with inserted or
    /// removed paragraphs, a caret moved under a collapsed heading expands it) and hide the
    /// blocks under the collapsed headings.
    pub(super) fn r15_layout(&mut self) {
        let Some(s) = self.view.engine().session() else {
            return;
        };
        let version = s.version();
        let doc = s.doc();
        if !self.r15.col.opened {
            self.r15.col.opened = true;
            self.r15.spacing_default = Some(default_spacing(doc));
            let paths = bo_model::index::body_paragraphs(doc);
            self.r15.col.heads = paths
                .iter()
                .enumerate()
                .filter(|(_, p)| heading_level(doc, p).is_some())
                .filter(|(_, p)| {
                    bo_model::index::para_at(doc, p).is_some_and(|q| collapsed_by_default(doc, q))
                })
                .map(|(i, _)| i)
                .collect();
        }
        if self.r15.col.heads.is_empty() && self.r15.col.want.is_empty() {
            return;
        }
        let caret_now = self.view.caret_para();
        let caret_moved = self.r15.col.caret.is_some() && caret_now != self.r15.col.caret;
        if self.r15.col.caret.is_none() {
            self.r15.col.caret = caret_now;
        }
        if caret_moved || self.r15.col.seen.map(|v| v.0) != Some(version) {
            let paths = bo_model::index::body_paragraphs(doc);
            let count = paths.len();
            let caret = self.view.caret_para().unwrap_or(0);
            if let Some((_, before)) = self.r15.col.seen {
                let d = count as i64 - before as i64;
                if d != 0 {
                    let c = caret as i64;
                    self.r15.col.heads = self
                        .r15
                        .col
                        .heads
                        .iter()
                        .filter_map(|&h| {
                            let h = h as i64;
                            if d > 0 {
                                Some(if h > c - d { h + d } else { h })
                            } else if h > c - d {
                                Some(h + d)
                            } else if h > c {
                                None
                            } else {
                                Some(h)
                            }
                        })
                        .filter_map(|h| usize::try_from(h).ok())
                        .collect();
                }
            }
            self.r15.col.heads.retain(|&h| {
                paths
                    .get(h)
                    .is_some_and(|p| heading_level(doc, p).is_some())
            });
            // The caret moved under a collapsed heading expands it.
            self.r15.col.caret = caret_now;
            if let Some(cp) = paths.get(caret).filter(|p| p.len() >= 2 && caret_moved) {
                let at = (cp[0], cp[1]);
                let heads = self.r15.col.heads.clone();
                self.r15
                    .col
                    .heads
                    .retain(|&h| !hidden_under(doc, &paths, &[h]).contains(&at));
                if heads.len() != self.r15.col.heads.len() {
                    self.r15.col.hover = None;
                }
            }
            self.r15.col.want = hidden_under(doc, &paths, &self.r15.col.heads);
            self.r15.col.seen = Some((version, count));
        }
        let want = self.r15.col.want.clone();
        if self.view.set_hidden_blocks(want) {
            self.r15.col.hover = None;
        }
    }

    /// Collapse or expand heading `para` (body paragraph id).
    pub(super) fn r15_toggle(&mut self, para: usize) -> bool {
        if let Some(i) = self.r15.col.heads.iter().position(|&h| h == para) {
            self.r15.col.heads.remove(i);
        } else {
            self.r15.col.heads.push(para);
            // The caret in the content collapsed goes to the heading, as the reference.
            let under = self.view.engine().document().is_some_and(|doc| {
                let paths = bo_model::index::body_paragraphs(doc);
                let caret = self.view.caret_para().and_then(|c| paths.get(c).cloned());
                caret
                    .filter(|c| c.len() >= 2)
                    .is_some_and(|c| hidden_under(doc, &paths, &[para]).contains(&(c[0], c[1])))
            });
            if under {
                self.view
                    .run_command(bo_engine::edit::Command::SetSelection {
                        selection: bo_edit::Selection::caret(bo_edit::Pos::new(para, 0)),
                    });
            }
        }
        self.r15.col.seen = None;
        self.r15.col.hover = None;
        self.relayout_pending = true;
        true
    }

    /// Pointer moved over the document: the heading under it shows its triangle. `true`
    /// when that changed.
    pub(super) fn r15_hover(&mut self, x: f32, y: f32) -> bool {
        let before = self.r15.col.hover.map(|h| (h.0, h.1));
        let f = self.view.frame_rect();
        let inside = x >= f[0] && x < f[0] + f[2] && y >= f[1] && y < f[1] + f[3];
        let hit = if inside
            && self.menu.is_none()
            && self.r3.form.is_none()
            && self.r15.spacing.is_none()
        {
            self.view
                .visible_para_starts()
                .into_iter()
                .find(|(_, r)| y >= r[1] && y < r[1] + r[2] && x >= r[0] - 40.0 * self.scale)
        } else {
            None
        };
        self.r15.col.hover = hit.and_then(|(id, r)| {
            let doc = self.view.engine().document()?;
            let path = bo_model::index::para_path(doc, id)?;
            heading_level(doc, &path)?;
            let k = self.scale / 1.25;
            let s = (10.0 * k).round();
            let tri = [
                (r[0] - 17.0 * k).round(),
                (r[1] + (r[2] - s) / 2.0).round(),
                s,
                s,
            ];
            Some((id, self.r15.col.heads.contains(&id), tri))
        });
        self.r15.col.hover.map(|h| (h.0, h.1)) != before
    }

    /// A press on the hovered heading's triangle collapses or expands it.
    pub(super) fn r15_input(&mut self, input: &crate::Input) -> Option<bool> {
        if let Some(r) = self.r15_spacing_input(input) {
            return Some(r);
        }
        let crate::Input::Press { x, y, .. } = *input else {
            return None;
        };
        let (id, _, r) = self.r15.col.hover?;
        let pad = 3.0 * self.scale / 1.25;
        let on =
            x >= r[0] - pad && x < r[0] + r[2] + pad && y >= r[1] - pad && y < r[1] + r[3] + pad;
        on.then(|| self.r15_toggle(id))
    }

    /// The hovered heading's triangle: ◢ grey when expanded, ▷ outlined when collapsed (own
    /// drawing; the reference shows it on hover only).
    pub(super) fn r15_paint(&self, p: &mut dyn crate::Painter) {
        use bo_render::PathSeg as S;
        if let Some(m) = &self.r15.spacing {
            m.paint(p);
        }
        let Some((_, collapsed, r)) = self.r15.col.hover else {
            return;
        };
        let f = self.view.frame_rect();
        if r[1] < f[1] || r[1] + r[3] > f[1] + f[3] {
            return;
        }
        let grey = bo_render::Rgba(0x5f, 0x5f, 0x5f, 255);
        if collapsed {
            let (x0, y0, w, h) = (r[0] + r[2] * 0.25, r[1], r[2] * 0.6, r[3]);
            p.path(
                &[
                    S::M(x0, y0),
                    S::L(x0 + w, y0 + h / 2.0),
                    S::L(x0, y0 + h),
                    S::Z,
                ],
                None,
                Some((grey, (1.2 * self.scale / 1.25).max(1.0))),
            );
        } else {
            let (x0, y0, w, h) = (
                r[0] + r[2] * 0.15,
                r[1] + r[3] * 0.15,
                r[2] * 0.7,
                r[3] * 0.7,
            );
            p.path(
                &[
                    S::M(x0 + w, y0),
                    S::L(x0 + w, y0 + h),
                    S::L(x0, y0 + h),
                    S::Z,
                ],
                Some(grey),
                None,
            );
        }
    }
}

/// Document default spacing as (before, after twips, `FindFormat::line_spacing` word).
fn default_spacing(doc: &bo_model::Document) -> (i32, i32, String) {
    let p = &doc.default_para;
    let line = match p.line_spacing {
        None | Some(bo_model::LineSpacing::Auto(240)) => "single".to_owned(),
        Some(bo_model::LineSpacing::Auto(360)) => "1.5".to_owned(),
        Some(bo_model::LineSpacing::Auto(480)) => "double".to_owned(),
        Some(bo_model::LineSpacing::Auto(n)) => format!("multiple:{n}"),
        Some(bo_model::LineSpacing::Exact(t)) => format!("exactly:{}", t.0),
        Some(bo_model::LineSpacing::AtLeast(t)) => format!("atLeast:{}", t.0),
    };
    (
        p.space_before.map_or(0, |t| t.0),
        p.space_after.map_or(0, |t| t.0),
        line,
    )
}

impl DocWindow {
    /// Design › Paragraph Spacing: the menu of presets under its button.
    pub(super) fn r15_open_spacing(&mut self) -> bool {
        self.menu = None;
        let anchor = self
            .ribbon
            .rect_of(crate::RibbonHit::Command("design.paragraphSpacing"))
            .unwrap_or([self.size.0 / 2.0, 120.0, 60.0, 30.0]);
        let mut labels = vec![
            self.r15_t("doc15.styleSet", "Style Set"),
            self.r15_t("doc15.spacingDefault", "Default"),
            self.r15_t("doc15.builtIn", "Built-In"),
        ];
        for (id, en, ..) in crate::doc_spacing15::PRESETS {
            labels.push(self.r15_t(&format!("doc15.spacing_{id}"), en));
        }
        labels.push(self.r15_t("doc15.customSpacing", "Custom Paragraph Spacing..."));
        let current = self.view.engine().document().and_then(|d| {
            crate::doc_spacing15::current_row(
                &default_spacing(d),
                self.r15.spacing_default.as_ref(),
            )
        });
        self.r15.spacing = Some(Box::new(crate::doc_spacing15::SpacingMenu::open(
            anchor, self.size, self.scale, labels, current,
        )));
        true
    }

    /// Spacing `(before, after, line)` as document defaults (one undo step).
    fn r15_set_spacing(&mut self, s: (i32, i32, &str)) -> bool {
        self.r3_run(vec![json!({ "cmd": "setDocDefaults", "format": {
            "spaceBefore": s.0, "spaceAfter": s.1, "lineSpacing": s.2,
        } })])
    }

    /// Input to the open Paragraph Spacing menu (`None`: not open).
    pub(super) fn r15_spacing_input(&mut self, input: &crate::Input) -> Option<bool> {
        use crate::doc_spacing15::SpacingPick;
        use crate::{Input, Key};
        let m = self.r15.spacing.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => m.hover(x, y),
            Input::Press { x, y, .. } => {
                let pick = m.press(x, y);
                if pick != Some(SpacingPick::Nothing) {
                    self.r15.spacing = None;
                }
                match pick {
                    Some(SpacingPick::Preset(i)) => {
                        let (_, _, b, a, l) = crate::doc_spacing15::PRESETS[i];
                        self.r15_set_spacing((b, a, l))
                    }
                    Some(SpacingPick::Default) => {
                        let d = self.r15.spacing_default.clone().unwrap_or((
                            0,
                            160,
                            "multiple:259".to_owned(),
                        ));
                        self.r15_set_spacing((d.0, d.1, &d.2))
                    }
                    // Custom Paragraph Spacing…: Manage Styles at Set Defaults, as the
                    // reference.
                    Some(SpacingPick::Custom) => {
                        let opened = self.r3_open_form(DocFormKind::ManageStyles);
                        if let Some(f) = self.r3.form.as_mut() {
                            f.set_tab(3);
                        }
                        opened
                    }
                    Some(SpacingPick::Nothing) | None => true,
                }
            }
            Input::Key(Key::Escape, _) => {
                self.r15.spacing = None;
                true
            }
            Input::Text(_) | Input::Key(..) | Input::Release { .. } | Input::Wheel(_) => true,
            _ => return None,
        })
    }
}

impl DocWindow {
    /// Command rows under a Design drop-down (round 15), as the reference's.
    pub(super) fn r15_design_footer(&self, menu: crate::design_gallery::DesignMenu) -> Vec<String> {
        use crate::design_gallery::DesignMenu;
        match menu {
            DesignMenu::Themes => vec![
                self.r15_t("doc15.resetTheme", "Reset to Theme from Template"),
                self.r15_t("doc15.browseThemes", "Browse for Themes..."),
                self.r15_t("doc15.saveTheme", "Save Current Theme..."),
            ],
            DesignMenu::Colors => vec![self.r15_t("doc15.customizeColors", "Customize Colors...")],
            DesignMenu::Fonts => vec![self.r15_t("doc15.customizeFonts", "Customize Fonts...")],
            _ => Vec::new(),
        }
    }

    /// A theme file (`.thmx`, or a document / workbook / deck) the host read for Browse for
    /// Themes…, base64: its theme becomes the document's (one undo step).
    pub fn apply_theme_file(&mut self, base64: &str) -> bool {
        self.design_theme = None;
        self.r3_run(vec![json!({ "cmd": "applyThemeFile", "file": base64 })])
    }
}
impl DocWindow {
    fn r15_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }
}
