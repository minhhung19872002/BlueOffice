//! BlueDoc parity round 3 (docs/ui/REPORT.md rounds 65+): Paste split button and Paste
//! Options, Styles pane, picture Wrap Text / Position / Arrange, References dialogs and
//! galleries, Outline view, Compare, Restrict Editing, Language, Envelopes and Labels,
//! Design Effects and Set as Default. A child module of the shell so the window's state is
//! at hand; the widgets live in `doc_r3_ui`, `doc_tiles`, `doc_forms`, `doc_outline`.
use super::{DocWindow, Request};
use crate::doc_forms::{self, DocForm, DocFormKind, FormContext, FormOut};
use crate::doc_outline::{OutlineAction, OutlineView};
use crate::doc_r3_menus as menus;
use crate::doc_r3_ui::{
    PaneAction, PaneStyle, PasteMenu, PastePick, RestrictPane, StylesPane, PASTE_MODES,
    RESTRICT_KINDS,
};
use crate::doc_tiles::{Tile, TileLook, TileMenu, TileSection};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::{Input, Key, Painter, TextMeasure};
use serde_json::{json, Value};

/// Round-3 state of the window.
#[derive(Default)]
pub(super) struct R3 {
    paste_menu: Option<PasteMenu>,
    /// Paste mode of the next paste (Paste Options, Paste Special).
    paste_mode: Option<&'static str>,
    /// Default paste modes: within a document, between documents, from other programs
    /// (0 keep, 1 merge, 2 text).
    paste_defaults: [usize; 3],
    pub(super) form: Option<DocForm>,
    styles_pane: Option<StylesPane>,
    restrict: Option<RestrictPane>,
    tiles: Option<TileMenu>,
    pub(super) outline: Option<OutlineView>,
    /// Align › Align to Page (else to the margin).
    align_page: bool,
    /// Theme effects chosen in Design › Effects.
    effects: Option<String>,
    /// Whether the next picked file is the compare original.
    compare_pending: bool,
    /// Stored as the default by Design › Set as Default (theme, style set, effects).
    pub(super) defaults: Option<(Option<String>, Option<String>, Option<String>)>,
    /// A document made by Envelopes and Labels, waiting for the host to open it.
    pub(super) new_document: Option<Vec<u8>>,
}

fn mode_of(i: usize) -> &'static str {
    PASTE_MODES.get(i).copied().unwrap_or("keep")
}

impl DocWindow {
    /// Width the round-3 panes take at the right.
    pub(super) fn r3_pane_w(&self) -> f32 {
        if self.r3.styles_pane.is_some() {
            StylesPane::width(self.scale)
        } else if self.r3.restrict.is_some() {
            RestrictPane::width(self.scale)
        } else {
            0.0
        }
    }

    /// The paste mode of the next paste: chosen in Paste Options / Paste Special, else the
    /// default for where the clipboard came from (BlueOffice's own format: a document, HTML:
    /// another program).
    pub(super) fn r3_paste_mode(&mut self, internal: bool, html: bool) -> bo_edit::PasteMode {
        let mode = self.r3.paste_mode.take().unwrap_or_else(|| {
            let d = self.r3.paste_defaults;
            mode_of(if internal {
                d[0]
            } else if html {
                d[2]
            } else {
                0
            })
        });
        match mode {
            "merge" => bo_edit::PasteMode::Merge,
            "text" => bo_edit::PasteMode::Text,
            _ => bo_edit::PasteMode::Keep,
        }
    }

    fn r3_t(&self, key: &str, fallback: &str) -> String {
        self.t(key, fallback)
    }

    /// Run JSON engine commands in order.
    fn r3_run(&mut self, cmds: Vec<Value>) -> bool {
        let mut ok = false;
        for c in cmds {
            ok |= self
                .view
                .run_json(&c)
                .is_some_and(|r| r.get("error").is_none_or(Value::is_null));
        }
        self.styles.clear();
        self.relayout_pending = true;
        ok
    }

    fn form_context(&mut self) -> FormContext {
        let f = self.view.format().clone();
        let styles = self
            .view
            .all_styles()
            .into_iter()
            .map(|s| (s.id, s.name, s.kind == "paragraph"))
            .collect();
        let targets = self
            .view
            .exec(bo_engine::edit::Command::CrossReferenceTargets)
            .and_then(|r| r.targets)
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.id, t.label))
            .collect();
        let sources = self
            .view
            .run_json(&json!({ "cmd": "citeState" }))
            .and_then(|v| v["cite"]["sources"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|s| {
                let tag = s["tag"].as_str().unwrap_or("").to_owned();
                let title = s["fields"]["Title"].as_str().unwrap_or("");
                let cite = s["citation"].as_str().unwrap_or("");
                (
                    tag.clone(),
                    format!("{cite} {title}").trim().to_owned(),
                    s["cited"].as_bool().unwrap_or(false),
                )
            })
            .collect();
        let selection = if self.view.has_selection() {
            self.view.copy(false).map(|c| c.text).unwrap_or_default()
        } else {
            String::new()
        };
        FormContext {
            styles,
            font: f.font.clone().unwrap_or_else(|| "Times New Roman".into()),
            size: f.size_half_points.unwrap_or(24),
            lang: f.lang.clone(),
            no_proof: f.no_proof,
            targets,
            sources,
            selection,
            paste_defaults: self.r3.paste_defaults,
        }
    }

    /// Open dialog `kind`.
    pub(super) fn r3_open_form(&mut self, kind: DocFormKind) -> bool {
        self.menu = None;
        let c = self.form_context();
        let mut f = doc_forms::open(kind, &self.strings, &c);
        if kind == DocFormKind::Protect {
            let k = self.r3.restrict.as_ref().map_or(3, |r| r.kind);
            f.ids = vec![RESTRICT_KINDS[k.min(3)].to_owned()];
        }
        self.r3.form = Some(f);
        self.relayout_pending = true;
        true
    }

    /// A file the host read for Compare.
    pub fn compare_file(&mut self, name: &str, base64: &str) -> bool {
        let slot = usize::from(!self.r3.compare_pending);
        if let Some(f) = self.r3.form.as_mut() {
            f.files[slot] = Some((name.to_owned(), base64.to_owned()));
            return true;
        }
        false
    }

    /// The defaults stored by Design › Set as Default, as JSON (theme, style set, effects).
    pub fn design_defaults(&self) -> Option<String> {
        let (theme, style_set, effects) = self.r3.defaults.as_ref()?;
        Some(json!({ "theme": theme, "styleSet": style_set, "effects": effects }).to_string())
    }

    /// Apply stored defaults (JSON of [`DocWindow::design_defaults`]) to a new document, as
    /// one undo step each; the document stays unmodified for saving prompts.
    pub fn apply_design_defaults(&mut self, json: &str) {
        let Ok(v) = serde_json::from_str::<Value>(json) else {
            return;
        };
        let mut cmds = Vec::new();
        if let Some(t) = v["theme"].as_str() {
            self.design_theme = Some(t.to_owned());
            cmds.push(json!({ "cmd": "applyTheme", "id": t }));
        }
        if let Some(s) = v["styleSet"].as_str() {
            self.design_style_set = Some(s.to_owned());
            cmds.push(json!({ "cmd": "applyDocumentStyleSet", "id": s }));
        }
        if let Some(e) = v["effects"].as_str() {
            self.r3.effects = Some(e.to_owned());
            cmds.push(json!({ "cmd": "applyThemeEffects", "id": e }));
        }
        if !cmds.is_empty() {
            self.r3_run(cmds);
            self.view.mark_saved();
        }
    }

    /// Bytes of a document Envelopes and Labels made (the host opens it in a new window).
    pub fn take_new_document(&mut self) -> Option<Vec<u8>> {
        self.r3.new_document.take()
    }

    fn form_out(&mut self, out: FormOut) -> bool {
        match out {
            FormOut::None => false,
            FormOut::Redraw => true,
            FormOut::Close => {
                self.r3.form = None;
                true
            }
            FormOut::Run(cmds, close) => {
                if close {
                    self.r3.form = None;
                }
                self.r3_run(cmds);
                if let Some(rp) = self.r3.restrict.as_mut() {
                    rp.protected = self
                        .view
                        .engine()
                        .document()
                        .is_some_and(|d| d.protection.as_ref().is_some_and(|p| p.enforced));
                }
                true
            }
            FormOut::Paste(mode) => {
                self.r3.form = None;
                self.r3.paste_mode = Some(mode);
                self.requests.push(Request::Paste);
                true
            }
            FormOut::Defaults(d) => {
                self.r3.form = None;
                self.r3.paste_defaults = d;
                true
            }
            FormOut::Open(kind) => {
                self.r3.form = None;
                self.r3_open_form(kind)
            }
            FormOut::PickFile(original) => {
                self.r3.compare_pending = original;
                self.requests.push(Request::PickCompareFile);
                true
            }
            FormOut::NewDocument(cmd) => {
                self.r3.form = None;
                // A blank document of its own gets the envelope or label sheet; the open
                // document is not touched.
                let mut e = bo_engine::Engine::with_bundled_fonts();
                let made = e.new_document("Envelope.docx").is_ok()
                    && e.command_json(&cmd.to_string()).is_ok();
                if let (true, Ok(bytes)) = (made, e.save_docx()) {
                    self.r3.new_document = Some(bytes);
                    self.requests.push(Request::OpenNewDocument);
                }
                true
            }
        }
    }

    fn open_tiles(&mut self, id: &'static str) -> bool {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let tr = |w: &Self, k: &str, f: &str| w.r3_t(k, f);
        let (sections, rows, look, draw): (
            Vec<TileSection>,
            Vec<(String, String, bool)>,
            TileLook,
            crate::doc_tiles::TileFn,
        ) = match id {
            "picture.position" => {
                let tiles = ["tl", "tc", "tr", "ml", "mc", "mr", "bl", "bc", "br"]
                    .iter()
                    .map(|p| Tile {
                        id: format!("picture.pos.{p}"),
                        label: String::new(),
                    })
                    .collect();
                (
                    vec![
                        TileSection {
                            header: tr(self, "docR3.inLine", "In Line with Text"),
                            tiles: vec![Tile {
                                id: "picture.pos.inline".into(),
                                label: String::new(),
                            }],
                        },
                        TileSection {
                            header: tr(self, "docR3.withWrapping", "With Text Wrapping"),
                            tiles,
                        },
                    ],
                    vec![(
                        "picture.layoutOptions".into(),
                        tr(self, "docR3.moreLayout", "More Layout Options..."),
                        false,
                    )],
                    TileLook {
                        cols: 3,
                        tile: (62.0, 62.0),
                        caption_top: false,
                    },
                    crate::doc_tiles::draw_position,
                )
            }
            "references.toc" => (
                vec![TileSection {
                    header: tr(self, "docR3.builtIn", "Built-In"),
                    tiles: vec![
                        Tile {
                            id: "references.toc.auto1".into(),
                            label: tr(self, "docR3.tocAuto1", "Automatic Table 1"),
                        },
                        Tile {
                            id: "references.toc.auto2".into(),
                            label: tr(self, "docR3.tocAuto2", "Automatic Table 2"),
                        },
                        Tile {
                            id: "references.toc.manual".into(),
                            label: tr(self, "docR3.tocManual", "Manual Table"),
                        },
                    ],
                }],
                vec![
                    (
                        "references.toc.custom".into(),
                        tr(self, "docR3.tocCustom", "Custom Table of Contents..."),
                        true,
                    ),
                    (
                        "references.toc.remove".into(),
                        tr(self, "docR3.tocRemove", "Remove Table of Contents"),
                        true,
                    ),
                ],
                TileLook {
                    cols: 1,
                    tile: (300.0, 98.0),
                    caption_top: true,
                },
                crate::doc_tiles::draw_toc,
            ),
            "references.insertBibliography" => (
                vec![TileSection {
                    header: tr(self, "docR3.builtIn", "Built-In"),
                    tiles: ["bibliography", "references", "worksCited"]
                        .iter()
                        .zip([
                            tr(self, "docR3.bibBibliography", "Bibliography"),
                            tr(self, "docR3.bibReferences", "References"),
                            tr(self, "docR3.bibWorksCited", "Works Cited"),
                        ])
                        .map(|(k, label)| Tile {
                            id: format!("references.bib.{k}"),
                            label,
                        })
                        .collect(),
                }],
                vec![(
                    "references.bib.insert".into(),
                    tr(self, "docR3.insertBibliography", "Insert Bibliography"),
                    true,
                )],
                TileLook {
                    cols: 1,
                    tile: (470.0, 100.0),
                    caption_top: true,
                },
                crate::doc_tiles::draw_bibliography,
            ),
            "design.effects" => (
                vec![TileSection {
                    header: tr(self, "docR3.effectsHeader", "BlueOffice"),
                    tiles: crate::doc_tiles::EFFECTS
                        .iter()
                        .map(|(k, name)| Tile {
                            id: format!("design.effects.{k}"),
                            label: tr(self, &format!("docR3.effect.{k}"), name),
                        })
                        .collect(),
                }],
                Vec::new(),
                TileLook {
                    cols: 4,
                    tile: (86.0, 86.0),
                    caption_top: false,
                },
                crate::doc_tiles::draw_effect,
            ),
            _ => return false,
        };
        let mut m = TileMenu::open(anchor, self.size, sections, rows, look, draw, self.scale);
        if id == "design.effects" {
            m.current = Some(format!(
                "design.effects.{}",
                self.r3.effects.as_deref().unwrap_or("flat")
            ));
        }
        self.menu = None;
        self.r3.tiles = Some(m);
        true
    }

    /// A round-3 menu as a popup with icons and separators.
    fn open_r3_menu(&mut self, id: &'static str) -> bool {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let entries = menus::menu(id);
        let wrap = self.picture_wrap();
        let style = self
            .view
            .run_json(&json!({ "cmd": "citeState" }))
            .and_then(|v| v["cite"]["style"].as_str().map(str::to_owned));
        let items: Vec<MenuItem> = entries
            .iter()
            .map(|e| MenuItem {
                label: if e.label.starts_with("docR3.") {
                    self.strings.get(e.label).to_owned()
                } else {
                    e.label.to_owned()
                },
                choice: Choice::Command(e.id.to_owned()),
                checked: match e.id {
                    "picture.alignTo.page" => self.r3.align_page,
                    "picture.alignTo.margin" => !self.r3.align_page,
                    _ => {
                        e.id.strip_prefix("picture.wrap.")
                            .is_some_and(|w| Some(w) == wrap.as_deref())
                            || e.id
                                .strip_prefix("references.style.")
                                .is_some_and(|s| Some(s) == style.as_deref())
                    }
                },
                dim: menus::dim(e.id)
                    || (e.id.starts_with("picture.align.") && wrap.as_deref() == Some("inline")),
            })
            .collect();
        let icons: Vec<Option<&'static str>> = entries.iter().map(|e| menus::icon(e.id)).collect();
        let seps: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| menus::separator(e.id))
            .map(|(i, _)| i)
            .collect();
        let mut m = Menu::open(items, anchor, self.size, self.scale);
        if icons.iter().any(Option::is_some) || !seps.is_empty() {
            m = m.with_decor(icons, seps);
        }
        self.menu = Some(m);
        true
    }

    /// Wrapping name of the selected picture (`inline`, `square`, …).
    fn picture_wrap(&mut self) -> Option<String> {
        if !(self.view.format().picture.is_some() || self.view.format().picture_tools) {
            return None;
        }
        let img = self.view.engine().session()?.tools_picture()?;
        Some(
            match &img.anchor {
                None => "inline",
                Some(a) => match a.wrap {
                    bo_model::Wrap::Square => "square",
                    bo_model::Wrap::Tight => "tight",
                    bo_model::Wrap::Through => "through",
                    bo_model::Wrap::TopAndBottom => "topAndBottom",
                    bo_model::Wrap::None if a.behind => "behind",
                    bo_model::Wrap::None => "inFront",
                },
            }
            .to_owned(),
        )
    }

    fn picture(&mut self, v: Value) -> bool {
        self.view.picture_command(&v)
    }

    /// Show the Styles pane (or hide it).
    fn toggle_styles_pane(&mut self) -> bool {
        if self.r3.styles_pane.take().is_none() {
            self.r3.restrict = None;
            self.spell_pane = None;
            self.comments_pane = None;
            let labels = [
                self.r3_t("docR3.styles", "Styles"),
                self.r3_t("docR3.clearAll", "Clear All"),
                self.r3_t("docR3.showPreview", "Show Preview"),
                self.r3_t("docR3.disableLinked", "Disable Linked Styles"),
                self.r3_t("docR3.optionsDots", "Options..."),
            ];
            let mut pane = StylesPane::new(labels);
            self.refresh_styles_pane_into(&mut pane);
            self.r3.styles_pane = Some(pane);
        }
        self.relayout_pending = true;
        true
    }

    fn refresh_styles_pane_into(&mut self, pane: &mut StylesPane) {
        let all = self.view.all_styles();
        pane.styles = all
            .into_iter()
            .filter(|s| s.recommended && (s.kind == "paragraph" || s.kind == "character"))
            .map(|s| PaneStyle {
                mark: if s.kind == "character" {
                    "a"
                } else if s.linked.is_some() {
                    "¶a"
                } else {
                    "¶"
                },
                name: crate::commands::style_name_key(&s.name)
                    .map_or_else(|| s.name.clone(), |k| self.strings.get(k).to_owned()),
                id: s.id,
                preview: Some(s.preview),
            })
            .collect();
        pane.current = self.view.format().style.clone();
    }

    fn toggle_restrict(&mut self) -> bool {
        if self.r3.restrict.take().is_none() {
            self.r3.styles_pane = None;
            self.spell_pane = None;
            self.comments_pane = None;
            let keys: [(&str, &str); 18] = [
                ("docR3.restrictEditing", "Restrict Editing"),
                ("docR3.formattingRestrictions", "1. Formatting restrictions"),
                (
                    "docR3.limitFormatting",
                    "Limit formatting to a selection of styles",
                ),
                ("docR3.settings", "Settings..."),
                ("docR3.editingRestrictions", "2. Editing restrictions"),
                (
                    "docR3.allowOnly",
                    "Allow only this type of editing in the\ndocument:",
                ),
                ("docR3.kindTracked", "Tracked changes"),
                ("docR3.kindComments", "Comments"),
                ("docR3.kindForms", "Filling in forms"),
                ("docR3.kindReadOnly", "No changes (Read only)"),
                ("docR3.startEnforcement", "3. Start enforcement"),
                (
                    "docR3.ready",
                    "Are you ready to apply these settings? (You\ncan turn them off later)",
                ),
                ("docR3.yesStart", "Yes, Start Enforcing Protection"),
                ("docR3.seeAlso", "See also"),
                ("docR3.restrictPermission", "Restrict permission..."),
                ("docR3.stopProtection", "Stop Protection"),
                ("docR3.yourPermissions", "Your permissions"),
                (
                    "docR3.protectedNote",
                    "This document is protected from unintentional editing.",
                ),
            ];
            let labels = keys.iter().map(|(k, f)| self.r3_t(k, f)).collect();
            let protected = self
                .view
                .engine()
                .document()
                .is_some_and(|d| d.protection.as_ref().is_some_and(|p| p.enforced));
            self.r3.restrict = Some(RestrictPane::new(labels, protected));
        }
        self.relayout_pending = true;
        true
    }

    fn pane_action(&mut self, a: PaneAction) -> bool {
        match a {
            PaneAction::None => false,
            PaneAction::Redraw => true,
            PaneAction::Close => {
                self.r3.styles_pane = None;
                self.r3.restrict = None;
                self.relayout_pending = true;
                true
            }
            PaneAction::ApplyStyle(id) => {
                let ok = if id.is_empty() {
                    self.view
                        .run_command(bo_engine::edit::Command::ClearFormatting)
                } else {
                    self.view
                        .run_command(bo_engine::edit::Command::ApplyStyle { id })
                };
                if let Some(mut p) = self.r3.styles_pane.take() {
                    p.current = self.view.format().style.clone();
                    self.r3.styles_pane = Some(p);
                }
                ok
            }
            PaneAction::Button("new") => self.r3_open_form(DocFormKind::NewStyle),
            PaneAction::Button("start") => self.r3_open_form(DocFormKind::Protect),
            PaneAction::Button("stop") => {
                let has_pw = self
                    .view
                    .engine()
                    .document()
                    .and_then(|d| d.protection.as_ref())
                    .is_some_and(|p| p.has_password());
                if has_pw {
                    self.r3_open_form(DocFormKind::Unprotect)
                } else {
                    let ok = self.r3_run(vec![json!({ "cmd": "unprotectDocument" })]);
                    if let Some(rp) = self.r3.restrict.as_mut() {
                        rp.protected = false;
                    }
                    ok
                }
            }
            PaneAction::Button("editType") => {
                let Some(r) = self.r3.restrict.as_ref().map(|r| r.kind_rect()) else {
                    return false;
                };
                let current = self.r3.restrict.as_ref().map_or(3, |r| r.kind);
                let items = (0..4)
                    .map(|i| MenuItem {
                        label: self
                            .r3
                            .restrict
                            .as_ref()
                            .map_or(String::new(), |r| r.labels[6 + i].clone()),
                        choice: Choice::Command(format!("r3.restrictKind.{i}")),
                        checked: i == current,
                        dim: false,
                    })
                    .collect();
                self.menu = Some(Menu::open(items, r, self.size, self.scale));
                true
            }
            PaneAction::Button(_) => false,
        }
    }

    /// Commands of round 3; `None` passes the id on to the shell's own commands.
    pub(super) fn r3_action(&mut self, id: &str) -> Option<bool> {
        if let Some(menu_id) = menus::MENUS
            .iter()
            .chain(menus::MORE_MENUS.iter())
            .find(|m| **m == id)
        {
            return Some(match *menu_id {
                // The Paste button's upper half pastes (its menu opens from the lower half).
                "edit.paste" => return None,
                "picture.position" | "references.toc" | "references.insertBibliography" => {
                    self.open_tiles(menu_id)
                }
                m => self.open_r3_menu(m),
            });
        }
        if let Some(mode) = id.strip_prefix("paste.") {
            return Some(match mode {
                "special" => self.r3_open_form(DocFormKind::PasteSpecial),
                "defaults" => self.r3_open_form(DocFormKind::PasteDefaults),
                m => {
                    self.r3.paste_mode = PASTE_MODES.iter().find(|x| **x == m).copied();
                    self.requests.push(Request::Paste);
                    true
                }
            });
        }
        if let Some(w) = id.strip_prefix("picture.wrap.") {
            return Some(self.picture(json!({ "cmd": "pictureWrap", "wrap": w })));
        }
        if let Some(p) = id.strip_prefix("picture.pos.") {
            return Some(self.picture(json!({ "cmd": "picturePosition", "position": p })));
        }
        if let Some(a) = id.strip_prefix("picture.align.") {
            let page = self.r3.align_page;
            return Some(self.picture(json!({ "cmd": "pictureAlign", "align": a, "page": page })));
        }
        if let Some(to) = id.strip_prefix("picture.alignTo.") {
            self.r3.align_page = to == "page";
            return Some(true);
        }
        if let Some(dir) = id.strip_prefix("picture.rotate.") {
            let degrees = if dir == "left" { -90 } else { 90 };
            return Some(self.picture(json!({ "cmd": "pictureRotate", "degrees": degrees })));
        }
        if let Some(style) = id.strip_prefix("references.style.") {
            return Some(self.r3_run(vec![json!({ "cmd": "citeSetStyle", "style": style })]));
        }
        if let Some(kind) = id.strip_prefix("references.bib.") {
            let heading = match kind {
                "bibliography" => Some(self.r3_t("docR3.bibBibliography", "Bibliography")),
                "references" => Some(self.r3_t("docR3.bibReferences", "References")),
                "worksCited" => Some(self.r3_t("docR3.bibWorksCited", "Works Cited")),
                _ => None,
            };
            return Some(self.r3_run(vec![
                json!({ "cmd": "citeBibliography", "heading": heading }),
            ]));
        }
        if let Some(k) = id.strip_prefix("r3.restrictKind.") {
            if let (Some(rp), Ok(i)) = (self.r3.restrict.as_mut(), k.parse::<usize>()) {
                rp.kind = i.min(3);
            }
            return Some(true);
        }
        if let Some(level) = id.strip_prefix("outline.level.") {
            let n = level.parse::<u8>().ok();
            if let Some(o) = self.r3.outline.as_mut() {
                o.show_level = n;
            }
            return Some(true);
        }
        if let Some(k) = id.strip_prefix("design.effects.") {
            self.r3.effects = Some(k.to_owned());
            return Some(self.r3_run(vec![json!({ "cmd": "applyThemeEffects", "id": k })]));
        }
        Some(match id {
            "view.stylesPane" => self.toggle_styles_pane(),
            "references.toc.auto1" | "references.toc.auto2" => self.r3_run(vec![
                json!({ "cmd": "insertToc" }),
                json!({ "cmd": "updateFields" }),
            ]),
            "references.toc.manual" => {
                let lines = [
                    self.r3_t("docR3.tocManualTitle", "Table of Contents"),
                    self.r3_t("docR3.tocManual1", "Type chapter title (level 1)\t1"),
                    self.r3_t("docR3.tocManual2", "Type chapter title (level 2)\t2"),
                    self.r3_t("docR3.tocManual3", "Type chapter title (level 3)\t3"),
                ];
                let mut cmds = Vec::new();
                for l in lines {
                    cmds.push(json!({ "cmd": "insertText", "text": l }));
                    cmds.push(json!({ "cmd": "splitParagraph" }));
                }
                self.r3_run(cmds)
            }
            "references.toc.custom" => self.r3_open_form(DocFormKind::Toc),
            "references.toc.remove" => self.remove_toc(),
            "references.notesDialog" => self.r3_open_form(DocFormKind::Notes),
            "references.citation.new" => self.r3_open_form(DocFormKind::NewSource),
            "references.manageSources" => self.r3_open_form(DocFormKind::Sources),
            "references.markEntry" => self.r3_open_form(DocFormKind::MarkEntry),
            "references.insertIndex" => self.r3_open_form(DocFormKind::Index),
            "references.crossReference" => self.r3_open_form(DocFormKind::CrossRef),
            "references.insertCaption" => self.r3_open_form(DocFormKind::Caption),
            "references.tableOfFigures" => self.r3_run(vec![json!({
                "cmd": "insertReferenceTable", "instr": "TOC \\h \\z \\c \"Figure\"", "leader": "dot",
            })]),
            "review.compare.compare" | "review.compare.combine" => {
                self.r3_open_form(DocFormKind::Compare)
            }
            "review.language.set" => self.r3_open_form(DocFormKind::Language),
            "review.restrictEditing" => self.toggle_restrict(),
            "mailings.envelopes" => {
                let ok = self.r3_open_form(DocFormKind::Envelopes);
                if let Some(f) = self.r3.form.as_mut() {
                    f.set_tab(0);
                }
                ok
            }
            "mailings.labels" => {
                let ok = self.r3_open_form(DocFormKind::Envelopes);
                if let Some(f) = self.r3.form.as_mut() {
                    f.set_tab(1);
                }
                ok
            }
            "view.outline" => self.toggle_outline(),
            "outline.close" => {
                self.r3.outline = None;
                self.relayout_pending = true;
                true
            }
            "outline.promote" | "outline.demote" | "outline.promoteHeading" | "outline.demoteBody" | "outline.moveUp" | "outline.moveDown" => {
                self.outline_command(id)
            }
            "outline.showLevel" => self.open_r3_menu_dyn("outline.showLevel"),
            "design.effects" => self.open_tiles("design.effects"),
            "design.setDefault" => {
                self.r3.defaults = Some((
                    self.design_theme.clone(),
                    self.design_style_set.clone(),
                    self.r3.effects.clone(),
                ));
                self.requests.push(Request::SaveDefaults);
                true
            }
            _ => return None,
        })
    }

    fn open_r3_menu_dyn(&mut self, id: &str) -> bool {
        match menus::static_id(id).or_else(|| menus::OUTLINE.iter().find(|m| **m == id).copied()) {
            Some(s) => self.open_r3_menu(s),
            None => false,
        }
    }

    fn remove_toc(&mut self) -> bool {
        self.r3_run(vec![json!({ "cmd": "removeToc" })])
    }

    fn toggle_outline(&mut self) -> bool {
        if self.r3.outline.take().is_none() {
            if self.read_mode.is_some() {
                self.leave_read_mode();
            }
            let items = self.view.outline();
            let mut o = OutlineView::new(self.r3_t("docR3.bodyText", "Body Text"));
            let styles = self.view.all_styles();
            let look = |id: &str| {
                styles
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.preview.clone())
            };
            let mut looks: Vec<_> = (1..=9).map(|n| look(&format!("Heading{n}"))).collect();
            looks.push(look("Normal"));
            o.set_looks(looks);
            o.set_items(items, self.view.caret_para());
            self.r3.outline = Some(o);
        }
        self.relayout_pending = true;
        true
    }

    fn outline_command(&mut self, id: &str) -> bool {
        let Some(o) = self.r3.outline.as_ref() else {
            return false;
        };
        let Some((para, level)) = o.selected() else {
            return false;
        };
        let cmd = match id {
            "outline.promoteHeading" => {
                json!({ "cmd": "setOutlineLevel", "para": para, "level": 0 })
            }
            "outline.promote" => match level {
                Some(0) => return false,
                Some(l) => json!({ "cmd": "setOutlineLevel", "para": para, "level": l - 1 }),
                None => json!({ "cmd": "setOutlineLevel", "para": para, "level": 8 }),
            },
            "outline.demote" => match level {
                Some(l) if l < 8 => {
                    json!({ "cmd": "setOutlineLevel", "para": para, "level": l + 1 })
                }
                _ => json!({ "cmd": "setOutlineLevel", "para": para }),
            },
            "outline.demoteBody" => json!({ "cmd": "setOutlineLevel", "para": para }),
            "outline.moveUp" => json!({ "cmd": "moveOutline", "para": para, "up": true }),
            _ => json!({ "cmd": "moveOutline", "para": para, "up": false }),
        };
        let ok = self.r3_run(vec![cmd]);
        self.refresh_outline();
        ok
    }

    fn refresh_outline(&mut self) {
        if self.r3.outline.is_some() {
            let items = self.view.outline();
            let caret = self.view.caret_para();
            if let Some(o) = self.r3.outline.as_mut() {
                o.set_items(items, caret);
            }
        }
    }

    /// Input for the round-3 widgets: `Some(redraw)` when one of them took it.
    pub(super) fn r3_input(&mut self, input: &Input, _text: &dyn TextMeasure) -> Option<bool> {
        if matches!(
            input,
            Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Maximized(_)
        ) {
            return None;
        }
        // Dialogs are modal.
        if let Some(f) = self.r3.form.as_mut() {
            if matches!(
                input,
                Input::Leave
                    | Input::Drag { .. }
                    | Input::Release { .. }
                    | Input::Context { .. }
                    | Input::Preedit(_)
                    | Input::Zoom(_)
            ) {
                return Some(false);
            }
            let out = f.input(input);
            return Some(self.form_out(out));
        }
        if let Some(m) = self.r3.paste_menu.as_mut() {
            match *input {
                Input::Move { x, y } | Input::Drag { x, y } => return Some(m.hover(x, y)),
                Input::Press { x, y, .. } => {
                    let pick = m.pick(x, y);
                    let inside = m.contains(x, y);
                    if pick.is_some() || !inside {
                        self.r3.paste_menu = None;
                        self.ribbon.clear_pointer();
                    }
                    return Some(match pick {
                        Some(PastePick::Mode(mode)) => {
                            self.r3_action(&format!("paste.{mode}")).unwrap_or(true)
                        }
                        Some(PastePick::Special) => self.r3_open_form(DocFormKind::PasteSpecial),
                        Some(PastePick::Defaults) => self.r3_open_form(DocFormKind::PasteDefaults),
                        None => true,
                    });
                }
                Input::Key(Key::Escape, _) => {
                    self.r3.paste_menu = None;
                    return Some(true);
                }
                Input::Release { .. } | Input::Text(_) | Input::Key(..) | Input::Wheel(_) => {
                    return Some(true)
                }
                _ => {}
            }
        }
        if let Some(m) = self.r3.tiles.as_mut() {
            match *input {
                Input::Move { x, y } | Input::Drag { x, y } => return Some(m.hover(x, y)),
                Input::Press { x, y, .. } => {
                    let pick = m.pick(x, y);
                    if pick.is_some() || !m.contains(x, y) {
                        self.r3.tiles = None;
                        self.ribbon.clear_pointer();
                    }
                    return Some(match pick {
                        Some(id) => self
                            .r3_action(&id)
                            .unwrap_or_else(|| self.action_by_id(&id)),
                        None => true,
                    });
                }
                Input::Key(Key::Escape, _) => {
                    self.r3.tiles = None;
                    return Some(true);
                }
                Input::Release { .. } | Input::Text(_) | Input::Key(..) | Input::Wheel(_) => {
                    return Some(true)
                }
                _ => {}
            }
        }
        // The Paste split button: its lower part (label and ▾) opens Paste Options.
        if let Input::Release { x, y } = *input {
            let paste = RibbonHit::Command("edit.paste");
            if self.menu.is_none()
                && self.ribbon.hit(x, y) == Some(paste)
                && self.ribbon.hit(self.pointer.0, self.pointer.1) == Some(paste)
            {
                if let Some(r) = self.ribbon.rect_of(paste) {
                    if self.pointer.1 >= r[1] + r[3] * 0.52 {
                        self.ribbon.release(x, y);
                        let labels = [
                            self.r3_t("docR3.pasteOptions", "Paste Options:"),
                            self.r3_t("docR3.keepSourceK", "Keep Source Formatting (K)"),
                            self.r3_t("docR3.mergeFormattingM", "Merge Formatting (M)"),
                            self.r3_t("docR3.keepTextT", "Keep Text Only (T)"),
                            self.r3_t("docR3.pasteSpecialDots", "Paste Special..."),
                            self.r3_t("docR3.setDefaultPaste", "Set Default Paste..."),
                        ];
                        self.r3.paste_menu = Some(PasteMenu::open(r, labels, self.scale));
                        return Some(true);
                    }
                }
            }
        }
        // Alt+Ctrl+Shift+S: the Styles pane.
        if let Input::Key(Key::Letter('s'), m) = *input {
            if m.ctrl && m.alt && m.shift {
                return Some(self.toggle_styles_pane());
            }
        }
        if let Some(p) = self.r3.styles_pane.as_mut() {
            match *input {
                Input::Move { x, y } if p.contains(x, y) => return Some(p.hover(x, y)),
                Input::Move { .. } => {
                    p.hover(-1.0, -1.0);
                }
                Input::Press { x, y, .. } if p.contains(x, y) => {
                    let a = p.press(x, y);
                    return Some(self.pane_action(a));
                }
                Input::Release { x, y } | Input::Drag { x, y } if p.contains(x, y) => {
                    return Some(false)
                }
                Input::Wheel(dy) if p.contains(self.pointer.0, self.pointer.1) => {
                    return Some(p.wheel(-dy));
                }
                _ => {}
            }
        }
        if let Some(p) = self.r3.restrict.as_mut() {
            match *input {
                Input::Move { x, y } if p.contains(x, y) => return Some(p.hover(x, y)),
                Input::Press { x, y, .. } if p.contains(x, y) => {
                    let a = p.press(x, y);
                    return Some(self.pane_action(a));
                }
                Input::Release { x, y } | Input::Drag { x, y } if p.contains(x, y) => {
                    return Some(false)
                }
                _ => {}
            }
        }
        if let Some(o) = self.r3.outline.as_mut() {
            let a = match *input {
                Input::Move { x, y } if o.contains(x, y) => return Some(o.hover(x, y)),
                Input::Press { x, y, count, .. } if o.contains(x, y) => o.press(x, y, count),
                Input::Wheel(dy) if o.contains(self.pointer.0, self.pointer.1) => {
                    return Some(o.wheel(-dy));
                }
                Input::Key(k, m) => o.key(k, m),
                Input::Release { x, y } | Input::Drag { x, y } if o.contains(x, y) => {
                    return Some(false)
                }
                Input::Text(_) => OutlineAction::None,
                _ => return None,
            };
            return Some(match a {
                OutlineAction::None => false,
                OutlineAction::Redraw => true,
                OutlineAction::Command(c) => self.outline_command(c),
                OutlineAction::Open(para) => {
                    self.r3.outline = None;
                    self.relayout_pending = true;
                    self.view.goto_para(para)
                }
            });
        }
        None
    }

    fn action_by_id(&mut self, id: &str) -> bool {
        match id {
            "picture.layoutOptions" => false,
            _ => self.r3_action(id).unwrap_or(false),
        }
    }

    /// Lay the round-3 widgets out (each input).
    pub(super) fn r3_layout(&mut self, text: &dyn TextMeasure) {
        // One pane at a time on the right: the proofing and comments panes win.
        if self.spell_pane.is_some() || self.comments_pane.is_some() {
            if self.r3.styles_pane.take().is_some() || self.r3.restrict.take().is_some() {
                self.relayout_pending = true;
            }
        }
        let (top, bottom, right, scale) = (
            self.chrome_bottom(),
            self.size.1 - self.status_h(),
            self.size.0,
            self.scale,
        );
        let current = self.view.format().style.clone();
        if let Some(p) = self.r3.styles_pane.as_mut() {
            p.layout(right, top, bottom, scale);
            p.current = current;
        }
        if let Some(p) = self.r3.restrict.as_mut() {
            p.layout(right, top, bottom, scale);
        }
        let left = self.nav_w() + self.vr_w();
        let pane = self.r3_pane_w();
        if let Some(o) = self.r3.outline.as_mut() {
            o.layout(
                [
                    left,
                    top + if self.show_ruler { 0.0 } else { 0.0 },
                    right - pane - left,
                    bottom - top,
                ],
                scale,
                text,
            );
        }
        let (size, sc) = (self.size, self.scale);
        if let Some(f) = self.r3.form.as_mut() {
            f.layout(text, size, sc);
        }
    }

    /// Paint the round-3 widgets over the window.
    pub(super) fn r3_paint_panes(&self, p: &mut dyn Painter) {
        if let Some(o) = &self.r3.outline {
            o.paint(p);
        }
        if let Some(sp) = &self.r3.styles_pane {
            sp.paint(p, &self.theme, &self.icons);
        }
        if let Some(rp) = &self.r3.restrict {
            rp.paint(p);
        }
    }

    /// Paint the round-3 popups and dialogs (over everything).
    pub(super) fn r3_paint_popups(&self, p: &mut dyn Painter) {
        if let Some(m) = &self.r3.paste_menu {
            m.paint(p);
        }
        if let Some(m) = &self.r3.tiles {
            m.paint(p);
        }
        if let Some(f) = &self.r3.form {
            f.paint(p, self.size, true);
        }
    }

    /// Point of control `id` of the open dialog (tests).
    #[cfg(test)]
    pub(super) fn r3_form_point(&self, id: &str) -> Option<(f32, f32)> {
        self.r3.form.as_ref()?.point_of(id)
    }

    /// Point of tile or row `id` of the open gallery (tests).
    #[cfg(test)]
    pub(super) fn r3_tile_point(&self, id: &str) -> Option<(f32, f32)> {
        self.r3.tiles.as_ref()?.point_of(id)
    }

    /// The open dialog's kind (tests).
    #[cfg(test)]
    pub(super) fn r3_form_kind(&self) -> Option<DocFormKind> {
        self.r3.form.as_ref().map(|f| f.kind)
    }

    /// Whether the Paste Options menu is open (tests).
    #[cfg(test)]
    pub(super) fn r3_paste_open(&self) -> bool {
        self.r3.paste_menu.is_some()
    }

    /// Whether the Styles pane is open (tests).
    #[cfg(test)]
    pub(super) fn r3_styles_open(&self) -> bool {
        self.r3.styles_pane.is_some()
    }
}

#[cfg(test)]
#[path = "doc_r3_tests.rs"]
mod tests;
