//! BlueDoc parity round 3 (docs/ui/REPORT.md rounds 65+): Paste split button and Paste
//! Options, Styles pane, picture Wrap Text / Position / Arrange, References dialogs and
//! galleries, Outline view, Compare, Restrict Editing, Language, Envelopes and Labels,
//! Design Effects and Set as Default. A child module of the shell so the window's state is
//! at hand; the widgets live in `doc_r3_ui`, `doc_tiles`, `doc_forms`, `doc_outline`.
#![allow(clippy::match_same_arms, clippy::type_complexity)]
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
    pub(super) styles_pane: Option<StylesPane>,
    pub(super) restrict: Option<RestrictPane>,
    pub(super) tiles: Option<TileMenu>,
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
    /// Citation style shown in References › Style (read once, then kept up to date).
    cite_style: Option<String>,
    /// A dialog kept under another one (Table of Contents under TOC Options).
    pub(super) stash: Option<DocForm>,
    /// Mailings › Start Mail Merge document type (`letters`, `directory`, `normal`).
    pub(super) merge_type: String,
}

/// The reference's recommended quick styles (id, name, kind mark ¶ / ¶a / a).
pub(super) const LATENT: [(&str, &str, &str); 17] = [
    ("Normal", "Normal", "¶"),
    ("NoSpacing", "No Spacing", "¶"),
    ("Heading1", "Heading 1", "¶a"),
    ("Heading2", "Heading 2", "¶a"),
    ("Heading3", "Heading 3", "¶a"),
    ("Title", "Title", "¶a"),
    ("Subtitle", "Subtitle", "¶a"),
    ("SubtleEmphasis", "Subtle Emphasis", "a"),
    ("Emphasis", "Emphasis", "a"),
    ("IntenseEmphasis", "Intense Emphasis", "a"),
    ("Strong", "Strong", "a"),
    ("Quote", "Quote", "¶a"),
    ("IntenseQuote", "Intense Quote", "¶a"),
    ("SubtleReference", "Subtle Reference", "a"),
    ("IntenseReference", "Intense Reference", "a"),
    ("BookTitle", "Book Title", "a"),
    ("ListParagraph", "List Paragraph", "¶"),
];

/// BlueOffice's own look of a latent default style (`styleNew` spec).
fn latent_spec(id: &str) -> Option<Value> {
    let (name, _, mark) = LATENT
        .iter()
        .find(|(i, ..)| *i == id)
        .map(|(_, n, m)| (*n, id, *m))?;
    let kind = if mark == "a" {
        "character"
    } else {
        "paragraph"
    };
    let mut spec = json!({ "name": name, "type": kind, "addToGallery": true, "basedOn": "Normal" });
    let o = spec.as_object_mut()?;
    let mut set = |k: &str, v: Value| {
        o.insert(k.to_owned(), v);
    };
    match id {
        "NoSpacing" => {
            set("spaceAfter", json!(0));
            set("lineSpacing", json!(240));
        }
        "Subtitle" => {
            set("color", json!("5A5A5A"));
            set("spaceAfter", json!(160));
        }
        "SubtleEmphasis" => {
            set("italic", json!(true));
            set("color", json!("404040"));
        }
        "Emphasis" => set("italic", json!(true)),
        "IntenseEmphasis" => {
            set("italic", json!(true));
            set("color", json!("2F5496"));
        }
        "Strong" => set("bold", json!(true)),
        "Quote" => {
            set("italic", json!(true));
            set("color", json!("404040"));
            set("align", json!("center"));
        }
        "IntenseQuote" => {
            set("italic", json!(true));
            set("color", json!("2F5496"));
            set("align", json!("center"));
        }
        "SubtleReference" => set("color", json!("5A5A5A")),
        "IntenseReference" => {
            set("bold", json!(true));
            set("color", json!("2F5496"));
        }
        "BookTitle" => {
            set("bold", json!(true));
            set("italic", json!(true));
        }
        "ListParagraph" => set("indentLeft", json!(720)),
        _ => {}
    }
    if kind == "character" {
        o.remove("basedOn");
    }
    Some(spec)
}

fn mode_of(i: usize) -> &'static str {
    PASTE_MODES.get(i).copied().unwrap_or("keep")
}

impl DocWindow {
    /// Width the round-3 panes take at the right.
    pub(super) fn r3_pane_w(&self) -> f32 {
        let own = if self.r3.styles_pane.is_some() {
            StylesPane::width(self.scale)
        } else if self.r3.restrict.is_some() {
            RestrictPane::width(self.scale)
        } else {
            0.0
        };
        // Reveal Formatting / Style Inspector (round 5) sits left of them.
        own + self.r5_pane_w() + self.r7_pane_w() + self.r16_pane_w()
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
    pub(super) fn r3_run(&mut self, cmds: Vec<Value>) -> bool {
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
        let recipients = self
            .view
            .run_json(&json!({ "cmd": "mergeRecords", "limit": 500 }))
            .map(|v| {
                let r = &v["mergeOut"]["records"];
                let fields = r["fields"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let rows = r["rows"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|row| {
                                row["values"]
                                    .as_array()
                                    .map(|v| {
                                        v.iter()
                                            .filter_map(|x| x.as_str().map(str::to_owned))
                                            .collect()
                                    })
                                    .unwrap_or_default()
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (fields, rows)
            })
            .unwrap_or_default();
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
            recipients,
            now: self.r4_now(),
            zoom: self.view.zoom(),
            date_default: self.r4.date_default.unwrap_or(0),
            match_fields: self.r5.match_fields.clone(),
            fonts: Vec::new(),
            table: f.table.clone(),
            inches: self.units.inches,
            extra: {
                let mut e = self.r12_form_extra();
                e["r13"] = self.r13_form_extra();
                e["r14"] = self.r14_form_extra();
                e["r15"] = self.r15_form_extra();
                e["r16"] = self.r16_form_extra();
                // Fill Effects (layout round 8): the page colour and fill effect.
                e["r18"] = self.view.engine().document().map_or(Value::Null, |d| {
                    json!({
                        "color": d.design.background.map(bo_model::Color::to_hex),
                        "fill": d.design.background_fill,
                    })
                });
                e["r17"] = self.r17_form_extra();
                e
            },
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

    /// A picture the host read for Fill Effects › Other Texture… / Select Picture… (layout
    /// round 8); `None` when the dialog is not waiting for one.
    pub(super) fn r18_take_picture(&mut self, name: &str, bytes: &[u8]) -> Option<bool> {
        use base64::Engine as _;
        let f = self.r3.form.as_mut().filter(|f| f.r18_awaiting())?;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        Some(f.r18_picture(name, &data))
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
        let mut v = match self.r3.defaults.as_ref() {
            Some((theme, style_set, effects)) => {
                json!({ "theme": theme, "styleSet": style_set, "effects": effects })
            }
            None => json!({}),
        };
        // Insert › Date and Time › Set As Default (round 4).
        if let Some(i) = self.r4.date_default {
            v["dateFormat"] = json!(crate::doc_forms::DATE_PICTURES[i]);
        }
        // Template styles and page setup (round 13).
        self.r13_defaults(&mut v);
        (v.as_object().is_some_and(|o| !o.is_empty())).then(|| v.to_string())
    }

    /// Apply the stored interface defaults (JSON of [`DocWindow::design_defaults`]) that hold
    /// for every document: the Date and Time format.
    pub fn apply_interface_defaults(&mut self, json: &str) {
        let Ok(v) = serde_json::from_str::<Value>(json) else {
            return;
        };
        if let Some(f) = v["dateFormat"].as_str() {
            self.r4.date_default = crate::doc_forms::DATE_PICTURES.iter().position(|p| *p == f);
        }
        self.r13_read_defaults(&v);
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
        // Styles and page setup kept for new documents (round 13).
        self.r13_read_defaults(&v);
        cmds.extend(self.r13_new_document_commands());
        if !cmds.is_empty() {
            self.r3_run(cmds);
            self.view.mark_saved();
        }
    }

    /// Bytes of a document Envelopes and Labels made (the host opens it in a new window).
    pub fn take_new_document(&mut self) -> Option<Vec<u8>> {
        self.r3.new_document.take()
    }

    pub(super) fn form_out(&mut self, out: FormOut) -> bool {
        match out {
            FormOut::Inspect(cats, remove) => self.r7_inspect(&cats, remove.as_deref()),
            FormOut::FindFormat(group, v, words) => self.r11_find_format(group, v, words),
            FormOut::TableMove(cmds, part) => self.r11_table_move(cmds, part),
            FormOut::None => false,
            FormOut::Redraw => true,
            FormOut::Close => {
                // Back to a dialog kept under this one (TOC Options), else closed.
                let closed = self.r3.form.as_ref().map(|f| f.kind);
                self.r3.form = self.r3.stash.take();
                self.r15_restore_stash(closed);
                true
            }
            FormOut::Run(cmds, close) => {
                if close {
                    // Back to a dialog kept under this one (Table Properties › Options…).
                    let closed = self.r3.form.as_ref().map(|f| f.kind);
                    self.r3.form = self.r3.stash.take();
                    self.r15_restore_stash(closed);
                }
                // Interface commands of round-4 dialogs (View › Zoom).
                let (ui, cmds): (Vec<Value>, Vec<Value>) =
                    cmds.into_iter().partition(|c| c["cmd"] == "ui.action");
                for c in ui {
                    if let Some(id) = c["id"].as_str() {
                        self.action(id);
                    }
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
            FormOut::DateDefault(i) => {
                // Kept for this window and stored like Design › Set as Default.
                self.r4.date_default = Some(i);
                self.requests.push(Request::SaveDefaults);
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
            FormOut::Stash(kind) => {
                self.r15_keep_stash(kind);
                self.r3.stash = self.r3.form.take();
                self.r3_open_form(kind)
            }
            FormOut::TocMap(map) => {
                self.r3.form = self.r3.stash.take();
                if let Some(f) = self.r3.form.as_mut() {
                    f.toc_map = (!map.is_empty()).then_some(map);
                }
                true
            }
            FormOut::PickFile(original) => {
                self.r3.compare_pending = original;
                self.requests.push(Request::PickCompareFile);
                true
            }
            FormOut::Compare(original, (rev_name, rev_b64)) => {
                self.r3.form = None;
                // The revised file in an engine of its own, compared with the original (this
                // document when none was picked); the result opens in a new window.
                let original = original.or_else(|| {
                    use base64::Engine as _;
                    let bytes = self.view.save_docx().ok()?;
                    Some((
                        self.name.clone(),
                        base64::engine::general_purpose::STANDARD.encode(bytes),
                    ))
                });
                let Some((name, b64)) = original else {
                    return true;
                };
                let mut e = bo_engine::Engine::with_bundled_fonts();
                let bytes = bo_engine::merge::base64_decode(&rev_b64).unwrap_or_default();
                let made = e.open(&bytes, &rev_name).is_ok()
                    && e.command_json(
                        &json!({ "cmd": "compareWith", "name": name, "base64": b64 }).to_string(),
                    )
                    .is_ok();
                if let (true, Ok(out)) = (made, e.save_docx()) {
                    self.r3.new_document = Some(out);
                    self.requests.push(Request::OpenNewDocument);
                }
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
        if id == "picture.position" && self.picture_wrap().as_deref() == Some("inline") {
            m.current = Some("picture.pos.inline".into());
        }
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
        if id == "mailings.insertField" {
            let (fields, _) = self.merge_fields();
            if !fields.is_empty() {
                let items: Vec<MenuItem> = fields
                    .iter()
                    .map(|f| MenuItem {
                        label: f.clone(),
                        choice: Choice::Command(format!("mailings.field.{f}")),
                        checked: false,
                        dim: false,
                    })
                    .collect();
                self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
                return true;
            }
        }
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
                    _ if e.id.starts_with("mailings.type.") => {
                        e.id.strip_prefix("mailings.type.")
                            == Some(if self.r3.merge_type.is_empty() {
                                "normal"
                            } else {
                                self.r3.merge_type.as_str()
                            })
                    }
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

    fn picture(&mut self, v: &Value) -> bool {
        self.view.picture_command(v)
    }

    /// Open the Paste Options drop-down under the Paste button.
    fn open_paste_menu(&mut self) -> bool {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command("edit.paste")) else {
            return false;
        };
        let labels = [
            self.r3_t("docR3.pasteOptions", "Paste Options:"),
            self.r3_t("docR3.keepSourceK", "Keep Source Formatting (K)"),
            self.r3_t("docR3.mergeFormattingM", "Merge Formatting (M)"),
            self.r3_t("docR3.keepTextT", "Keep Text Only (T)"),
            self.r3_t("docR3.pasteSpecialDots", "Paste Special..."),
            self.r3_t("docR3.setDefaultPaste", "Set Default Paste..."),
        ];
        self.r3.paste_menu = Some(PasteMenu::open(r, labels, self.scale));
        true
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

    pub(super) fn refresh_styles_pane_into(&mut self, pane: &mut StylesPane) {
        let all = self.view.all_styles();
        let all_default = all
            .iter()
            .find(|s| s.is_default && s.kind == "paragraph")
            .map(|s| s.id.clone());
        let mut list: Vec<PaneStyle> = Vec::new();
        // The reference's recommended list first, in its order: the document's style when
        // it has one, else the latent default (made when first applied).
        for (id, name, mark) in LATENT {
            // By id, else by name (documents may give Normal another id).
            let own = all
                .iter()
                .find(|s| s.id == *id)
                .or_else(|| all.iter().find(|s| s.name.eq_ignore_ascii_case(name)));
            list.push(PaneStyle {
                id: own.map_or_else(|| (*id).to_owned(), |s| s.id.clone()),
                name: own.map_or_else(
                    || (*name).to_owned(),
                    |s| {
                        crate::commands::style_name_key(&s.name)
                            .map_or_else(|| s.name.clone(), |k| self.strings.get(k).to_owned())
                    },
                ),
                mark,
                preview: own.map(|s| s.preview.clone()),
            });
        }
        let rest = all
            .into_iter()
            .filter(|s| s.recommended && (s.kind == "paragraph" || s.kind == "character"))
            .filter(|s| !list.iter().any(|l| l.id == s.id))
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
            .collect::<Vec<_>>();
        list.extend(rest);
        pane.styles = list;
        self.r13_pane_filter(pane);
        // No direct style: the document's default paragraph style.
        pane.default_id = all_default.unwrap_or_else(|| "Normal".into());
        pane.current = Some(
            self.view
                .format()
                .style
                .clone()
                .unwrap_or_else(|| pane.default_id.clone()),
        );
    }

    /// Apply style `id`, first making a latent default style the document lacks.
    fn apply_pane_style(&mut self, id: String) -> bool {
        let exists = self.view.all_styles().iter().any(|s| s.id == id);
        if !exists {
            if let Some(spec) = latent_spec(&id) {
                let created = self
                    .view
                    .run_json(&json!({ "cmd": "styleNew", "spec": spec }))
                    .and_then(|v| v["styleMgr"]["created"].as_str().map(str::to_owned));
                if let Some(c) = created {
                    return self
                        .view
                        .run_command(bo_engine::edit::Command::ApplyStyle { id: c });
                }
            }
        }
        self.view
            .run_command(bo_engine::edit::Command::ApplyStyle { id })
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
                    self.apply_pane_style(id)
                };
                if let Some(mut p) = self.r3.styles_pane.take() {
                    self.refresh_styles_pane_into(&mut p);
                    self.r3.styles_pane = Some(p);
                }
                ok
            }
            PaneAction::Button("new") => self.r3_open_form(DocFormKind::NewStyle),
            PaneAction::Button("inspector") => self.action("styles.inspector"),
            PaneAction::Button("manage") => self.action("styles.manage"),
            PaneAction::Button("options") => self.action("styles.paneOptions"),
            PaneAction::Button("start") => self.r3_open_form(DocFormKind::Protect),
            PaneAction::Button("stop") => {
                let has_pw = self
                    .view
                    .engine()
                    .document()
                    .and_then(|d| d.protection.as_ref())
                    .is_some_and(bo_model::DocProtection::has_password);
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
                let Some(r) = self.r3.restrict.as_ref().map(RestrictPane::kind_rect) else {
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
                // The Paste button's lower half (scripts and keyboard).
                "menu" => self.open_paste_menu(),
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
            return Some(self.picture(&json!({ "cmd": "pictureWrap", "wrap": w })));
        }
        if let Some(p) = id.strip_prefix("picture.pos.") {
            return Some(self.picture(&json!({ "cmd": "picturePosition", "position": p })));
        }
        if let Some(a) = id.strip_prefix("picture.align.") {
            let page = self.r3.align_page;
            return Some(self.picture(&json!({ "cmd": "pictureAlign", "align": a, "page": page })));
        }
        if let Some(to) = id.strip_prefix("picture.alignTo.") {
            self.r3.align_page = to == "page";
            return Some(true);
        }
        if let Some(order) = id.strip_prefix("picture.order.") {
            return Some(self.picture(&json!({ "cmd": "pictureOrder", "order": order })));
        }
        if let Some(axis) = id.strip_prefix("picture.flip.") {
            return Some(self.picture(&json!({ "cmd": "pictureFlip", "axis": axis })));
        }
        if let Some(dir) = id.strip_prefix("picture.rotate.") {
            let degrees = if dir == "left" { -90 } else { 90 };
            return Some(self.picture(&json!({ "cmd": "pictureRotate", "degrees": degrees })));
        }
        if let Some(style) = id.strip_prefix("references.style.") {
            self.r3.cite_style = Some(style.to_owned());
            return Some(self.r3_run(vec![json!({ "cmd": "citeSetStyle", "style": style })]));
        }
        if let Some(level) = id.strip_prefix("references.addText.") {
            let cmd = match level.parse::<u8>() {
                Ok(l) => json!({ "cmd": "setOutlineLevel", "level": l }),
                Err(_) => json!({ "cmd": "setOutlineLevel" }),
            };
            return Some(self.r3_run(vec![cmd]));
        }
        if let Some(which) = id.strip_prefix("references.note.") {
            return Some(self.goto_note(which));
        }
        if let Some(level) = id.strip_prefix("outline.setLevel.") {
            let Some((para, _)) = self.r3.outline.as_ref().and_then(OutlineView::selected) else {
                return Some(false);
            };
            let cmd = match level.parse::<u8>() {
                Ok(l) => json!({ "cmd": "setOutlineLevel", "para": para, "level": l }),
                Err(_) => json!({ "cmd": "setOutlineLevel", "para": para }),
            };
            let ok = self.r3_run(vec![cmd]);
            self.refresh_outline();
            return Some(ok);
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
        if let Some(t) = id.strip_prefix("mailings.type.") {
            if t == "labels" {
                // A labels main document: Update Labels comes on (round 16).
                self.r3.merge_type = t.to_owned();
                return Some(self.r3_action("mailings.labels").unwrap_or(false));
            }
            if t == "envelopes" {
                self.r3.merge_type = t.to_owned();
                return Some(self.r3_action("mailings.envelopes").unwrap_or(false));
            }
            self.r3.merge_type = t.to_owned();
            return Some(true);
        }
        if let Some(name) = id.strip_prefix("mailings.field.") {
            return Some(self.r3_run(vec![json!({ "cmd": "mergeInsertField", "name": name })]));
        }
        if let Some(rule) = id.strip_prefix("mailings.rule.") {
            let rule = match rule {
                "mergeRec" => json!({ "kind": "mergeRec" }),
                "mergeSeq" => json!({ "kind": "mergeSeq" }),
                "next" => json!({ "kind": "next" }),
                "fillIn" => {
                    json!({ "kind": "fillIn", "prompt": self.r3_t("docR3.fillInPrompt", "Fill in") })
                }
                _ => return Some(false),
            };
            return Some(self.r3_run(vec![json!({ "cmd": "mergeInsertRule", "rule": rule })]));
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
            "references.notesDialog" | "references.notesLauncher" => {
                self.r3_open_form(DocFormKind::Notes)
            }
            "references.showNotes" => self.goto_note("show"),
            "mailings.editRecipients" => self.r3_open_form(DocFormKind::Recipients),
            "mailings.addressBlock" => self.insert_address_block(),
            "mailings.greetingLine" => self.insert_greeting_line(),
            "mailings.firstRecord" | "mailings.lastRecord" => {
                let count = self.merge_fields().1;
                if count == 0 {
                    return Some(false);
                }
                let record = if id == "mailings.firstRecord" { 0 } else { count - 1 };
                self.merge_record = Some(record);
                self.r3_run(vec![json!({ "cmd": "mergePreview", "record": record })])
            }
            "mailings.highlightFields" | "mailings.matchFields" | "mailings.updateLabels"
            | "mailings.findRecipient" | "mailings.checkErrors" | "mailings.recordBox" => false,
            "references.updateFigures" => self.r3_run(vec![json!({ "cmd": "updateFields" })]),
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
            "outline.showLevel" | "outline.level" => self.open_r3_menu_dyn(id),
            "outline.expand" | "outline.collapse" => self
                .r3
                .outline
                .as_mut()
                .is_some_and(|o| o.expand(id == "outline.expand")),
            "outline.showFormatting" => self.r3.outline.as_mut().is_some_and(|o| {
                o.formatting = !o.formatting;
                true
            }),
            "outline.firstLine" => self.r3.outline.as_mut().is_some_and(|o| {
                o.first_line = !o.first_line;
                true
            }),
            "outline.showDocument" | "outline.collapseSubdocs" => false,
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

    /// The data source's field names and record count (empty without recipients).
    pub(super) fn merge_fields(&mut self) -> (Vec<String>, usize) {
        let v = self.view.run_json(&json!({ "cmd": "state" }));
        let m = v.as_ref().map(|v| v["merge"].clone()).unwrap_or_default();
        let fields = m["fields"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|f| f.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        (fields, m["count"].as_u64().unwrap_or(0) as usize)
    }

    /// The field whose name contains one of `keys` (case-insensitive).
    fn field_like(fields: &[String], keys: &[&str]) -> Option<String> {
        fields
            .iter()
            .find(|f| {
                let l = f.to_lowercase();
                keys.iter().any(|k| l.contains(k))
            })
            .cloned()
    }

    /// Address Block: name, address and city fields of the data source on their own lines
    /// (BlueOffice's own block of MERGEFIELDs, §17.16.5.35).
    fn insert_address_block(&mut self) -> bool {
        let (fields, _) = self.merge_fields();
        if fields.is_empty() {
            return false;
        }
        let pick = |keys: &[&str]| Self::field_like(&fields, keys);
        let lines: Vec<Vec<String>> = vec![
            [
                pick(&["title", "danh xưng"]),
                pick(&["first", "tên"]),
                pick(&["last", "họ"]),
            ]
            .into_iter()
            .flatten()
            .collect(),
            pick(&["company", "công ty"]).into_iter().collect(),
            pick(&["address", "địa chỉ"]).into_iter().collect(),
            pick(&["city", "thành phố", "tỉnh"]).into_iter().collect(),
        ];
        let lines: Vec<Vec<String>> = lines.into_iter().filter(|l| !l.is_empty()).collect();
        let lines = if lines.is_empty() {
            vec![vec![fields[0].clone()]]
        } else {
            lines
        };
        let mut cmds = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                cmds.push(json!({ "cmd": "splitParagraph" }));
            }
            for (k, f) in line.iter().enumerate() {
                if k > 0 {
                    cmds.push(json!({ "cmd": "insertText", "text": " " }));
                }
                cmds.push(json!({ "cmd": "mergeInsertField", "name": f }));
            }
        }
        self.r3_run(cmds)
    }

    /// Greeting Line: "Dear «first name»," (the first field when no name column).
    fn insert_greeting_line(&mut self) -> bool {
        let (fields, _) = self.merge_fields();
        let Some(name) = Self::field_like(&fields, &["first", "name", "tên"])
            .or_else(|| fields.first().cloned())
        else {
            return false;
        };
        self.r3_run(vec![
            json!({ "cmd": "insertText", "text": self.r3_t("docR3.dear", "Dear ") }),
            json!({ "cmd": "mergeInsertField", "name": name }),
            json!({ "cmd": "insertText", "text": "," }),
        ])
    }

    /// Move to the next / previous footnote or endnote reference (`nextFoot`, `prevFoot`,
    /// `nextEnd`, `prevEnd`), or (`show`) edit the first note on its page.
    fn goto_note(&mut self, which: &str) -> bool {
        let notes = self
            .view
            .run_json(&json!({ "cmd": "notes" }))
            .and_then(|v| v["notes"].as_array().cloned())
            .unwrap_or_default();
        if which == "show" {
            let Some(n) = notes.first() else {
                return false;
            };
            return self.r3_run(vec![json!({
                "cmd": "editNote", "endnote": n["endnote"], "id": n["id"],
            })]);
        }
        let endnote = which.ends_with("End");
        let forward = which.starts_with("next");
        let caret = self.view.caret_para().unwrap_or(0);
        let mut paras: Vec<usize> = notes
            .iter()
            .filter(|n| n["endnote"].as_bool() == Some(endnote))
            .filter_map(|n| n["para"].as_u64().map(|p| p as usize))
            .collect();
        paras.sort_unstable();
        let target = if forward {
            paras.into_iter().find(|p| *p > caret)
        } else {
            paras.into_iter().rev().find(|p| *p < caret)
        };
        target.is_some_and(|p| self.view.goto_para(p))
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
            let labels = self.view.engine().list_labels();
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
            o.set_items(&items, self.view.caret_para(), &labels);
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

    pub(super) fn refresh_outline(&mut self) {
        if self.r3.outline.is_some() {
            let items = self.view.outline();
            let caret = self.view.caret_para();
            let labels = self.view.engine().list_labels();
            if let Some(o) = self.r3.outline.as_mut() {
                o.set_items(&items, caret, &labels);
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
                Input::Leave | Input::Context { .. } | Input::Preedit(_) | Input::Zoom(_)
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
                    // The scroll bar of a scrolling gallery keeps it open.
                    match pick.as_deref() {
                        Some(crate::doc_tiles::SCROLL_UP) => return Some(m.scroll_by(-1)),
                        Some(crate::doc_tiles::SCROLL_DOWN) => return Some(m.scroll_by(1)),
                        _ => {}
                    }
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
                Input::Wheel(d) => {
                    return Some(m.scroll_by(if d > 0.0 { -1 } else { 1 }));
                }
                Input::Release { .. } | Input::Text(_) | Input::Key(..) => return Some(true),
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
                        return Some(self.open_paste_menu());
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
                Input::Text(ref t) => o.type_text(t),
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
                OutlineAction::Edit { para, at, edit } => self.outline_edit(para, at, &edit),
            });
        }
        None
    }

    fn action_by_id(&mut self, id: &str) -> bool {
        match id {
            "picture.layoutOptions" => false,
            _ => self.r3_action(id).unwrap_or_else(|| self.action(id)),
        }
    }

    /// Lay the round-3 widgets out (each input).
    pub(super) fn r3_layout(&mut self, text: &dyn TextMeasure) {
        // One pane at a time on the right: the proofing and comments panes win.
        if (self.spell_pane.is_some() || self.comments_pane.is_some())
            && (self.r3.styles_pane.take().is_some() || self.r3.restrict.take().is_some())
        {
            self.relayout_pending = true;
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
            p.current = Some(current.unwrap_or_else(|| p.default_id.clone()));
        }
        if let Some(p) = self.r3.restrict.as_mut() {
            p.layout(right, top, bottom, scale);
        }
        // The outline covers the vertical ruler too, as the reference shows none.
        let left = self.nav_w();
        let pane = self.r3_pane_w();
        if let Some(o) = self.r3.outline.as_mut() {
            o.layout([left, top, right - pane - left, bottom - top], scale, text);
        }
        if let Some(m) = self.r3.paste_menu.as_mut() {
            m.measure(text);
        }
        let (size, sc) = (self.size, self.scale);
        if let Some(f) = self.r3.form.as_mut() {
            f.layout(text, size, sc);
        }
    }

    /// Current value shown in drop-down box `id` of the ribbon.
    fn box_value(&self, id: &str) -> Option<String> {
        let level = |l: Option<u8>| match l {
            Some(l) => self.r3_t(
                &format!("docR3.level{}", l + 1),
                &format!("Level {}", l + 1),
            ),
            None => self.r3_t("docR3.bodyText", "Body Text"),
        };
        match id {
            "outline.level" => {
                let (_, l) = self.r3.outline.as_ref()?.selected()?;
                Some(level(l))
            }
            "outline.showLevel" => {
                let o = self.r3.outline.as_ref()?;
                Some(match o.show_level {
                    Some(n) => level(Some(n - 1)),
                    None => self.r3_t("docR3.allLevels", "All Levels"),
                })
            }
            "references.citationStyle" => Some(
                crate::doc_r3_menus::STYLES
                    .iter()
                    .find(|(k, _)| {
                        k.strip_prefix("references.style.")
                            == Some(self.r3.cite_style.as_deref().unwrap_or("APA"))
                    })
                    .map_or("APA", |(_, n)| n)
                    .to_owned(),
            ),
            _ => None,
        }
    }

    /// Ribbon drop-down boxes and check boxes of round 3, over their placeholders.
    fn r3_paint_ribbon(&self, p: &mut dyn Painter) {
        let px = |v: f32| (v * self.scale).round();
        let font = crate::Font {
            family: "Segoe UI",
            size: px(12.0),
            weight: 400,
            italic: false,
            underline: false,
        };
        let ink = bo_render::Rgba(0x26, 0x26, 0x26, 255);
        for id in crate::doc_r3_menus::BOXES {
            let (Some(r), Some(value)) = (
                self.ribbon.rect_of(RibbonHit::Command(id)),
                self.box_value(id),
            ) else {
                continue;
            };
            p.fill_rect(r[0], r[1], r[2], r[3], self.theme.ribbon);
            // A caption before the box (Show Level:, Style:), as the reference.
            let caption = match id {
                "outline.showLevel" => Some(self.r3_t("docR3.showLevelCaption", "Show Level:")),
                "references.citationStyle" => Some(self.r3_t("docR3.styleCaption", "Style:")),
                _ => None,
            };
            let mut bx = r[0] + px(2.0);
            if let Some(c) = caption {
                let cw = px(6.5) * c.chars().count() as f32 * 1.05;
                p.text_font(
                    &c,
                    bx,
                    (r[1] + (r[3] - font.size * 1.33) / 2.0).round(),
                    &font,
                    ink,
                );
                bx += cw + px(4.0);
            }
            let b = [
                bx,
                r[1] + px(1.0),
                r[0] + r[2] - bx - px(2.0),
                r[3] - px(2.0),
            ];
            p.fill_rect(
                b[0],
                b[1],
                b[2],
                b[3],
                bo_render::Rgba(0x94, 0x94, 0x94, 255),
            );
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                bo_render::Rgba(255, 255, 255, 255),
            );
            p.push_clip(b[0], b[1], b[2] - px(16.0), b[3]);
            p.text_font(
                &value,
                b[0] + px(4.0),
                (b[1] + (b[3] - font.size * 1.33) / 2.0).round(),
                &font,
                ink,
            );
            p.pop_clip();
            let (cx, cy, k) = (b[0] + b[2] - px(8.0), b[1] + b[3] / 2.0, px(3.0));
            p.path(
                &[
                    bo_render::PathSeg::M(cx - k, cy - k / 2.0),
                    bo_render::PathSeg::L(cx, cy + k / 2.0),
                    bo_render::PathSeg::L(cx + k, cy - k / 2.0),
                ],
                None,
                Some((ink, self.scale.max(1.0))),
            );
        }
        if let Some(r) = self
            .ribbon
            .rect_of(RibbonHit::Command("mailings.recordBox"))
        {
            p.fill_rect(r[0], r[1], r[2], r[3], self.theme.ribbon);
            let b = [
                r[0] + px(2.0),
                r[1] + px(3.0),
                r[2] - px(4.0),
                r[3] - px(6.0),
            ];
            p.fill_rect(
                b[0],
                b[1],
                b[2],
                b[3],
                bo_render::Rgba(0xc8, 0xc8, 0xc8, 255),
            );
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                bo_render::Rgba(255, 255, 255, 255),
            );
            if let Some(n) = self.merge_record.or(self.r5.record_box) {
                let t = (n + 1).to_string();
                p.text_font(
                    &t,
                    b[0] + px(6.0),
                    (b[1] + (b[3] - font.size * 1.33) / 2.0).round(),
                    &font,
                    ink,
                );
            }
        }
        if let Some(o) = &self.r3.outline {
            let checks = [
                ("outline.showFormatting", o.formatting),
                ("outline.firstLine", o.first_line),
            ];
            crate::doc_table::paint_checks(
                p,
                &self.ribbon,
                &checks,
                &self.theme,
                &self.strings,
                self.scale,
            );
        }
    }

    /// Paint the round-3 widgets over the window.
    pub(super) fn r3_paint_panes(&self, p: &mut dyn Painter) {
        if self.read_mode.is_none() {
            self.r3_paint_ribbon(p);
        }
        if let Some(o) = &self.r3.outline {
            o.paint(p);
        }
        if let Some(sp) = &self.r3.styles_pane {
            sp.paint(p, &self.theme, &self.icons);
        }
        if let Some(rp) = &self.r3.restrict {
            rp.paint(p);
        }
        self.r16_paint_panes(p);
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
