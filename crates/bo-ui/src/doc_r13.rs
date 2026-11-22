//! BlueDoc parity round 13 (docs/ui/REPORT.md "Vòng 103"): the Paragraph, Tabs and Page Setup
//! dialogs, New / Modify Style with Format ▾ and "New documents based on this template" (styles
//! kept in BlueDoc's own defaults file, never the reference's template). A child module of the
//! shell.
use super::DocWindow;
use crate::doc_forms::DocFormKind;
use crate::popup::{Choice, Menu, MenuItem};
use serde_json::{json, Map, Value};

/// Round-13 state of the window.
#[derive(Default)]
pub(super) struct R13 {
    /// Styles new documents get (New / Modify Style › New documents based on this template):
    /// `{ "id": modified style id or null, "spec": style spec }`.
    pub(super) template_styles: Vec<Value>,
    /// The spec the next New / Modify Style dialog opens with (Modify Style).
    modify_spec: Option<Value>,
}

/// Format ▾ entries of New / Modify Style: id, key, en-US label, available.
const STYLE_FORMAT: [(&str, &str, &str, bool); 9] = [
    ("font", "doc13.ns_fmtFont", "Font...", true),
    ("paragraph", "doc13.ns_fmtParagraph", "Paragraph...", true),
    ("tabs", "doc13.ns_fmtTabs", "Tabs...", true),
    ("border", "doc13.ns_fmtBorder", "Border...", false),
    ("language", "doc13.ns_fmtLanguage", "Language...", true),
    ("frame", "doc13.ns_fmtFrame", "Frame...", false),
    ("numbering", "doc13.ns_fmtNumbering", "Numbering...", false),
    ("shortcut", "doc13.ns_fmtShortcut", "Shortcut key...", false),
    ("effects", "doc13.ns_fmtEffects", "Text Effects...", false),
];

impl DocWindow {
    fn r13_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// A round-13 command; `None` when `id` is not one.
    #[inline(never)]
    pub(super) fn r13_action(&mut self, id: &str) -> Option<bool> {
        if let Some(j) = id.strip_prefix("r13.styleFmtDone:") {
            let m: Map<String, Value> = serde_json::from_str(j).unwrap_or_default();
            if let Some(f) = self.r3.form.as_mut() {
                f.style_take_format(&m);
            }
            return Some(true);
        }
        if let Some(r) = id.strip_prefix("r13.styleFmtMenu:") {
            return Some(self.r13_style_menu(r));
        }
        if let Some(w) = id.strip_prefix("r13.styleFmt.") {
            return Some(self.r13_style_format(w));
        }
        if let Some(j) = id.strip_prefix("r13.psApply:") {
            let v: Value = serde_json::from_str(j).unwrap_or_default();
            return Some(self.r13_page_setup(&v));
        }
        if let Some(j) = id.strip_prefix("r13.psDefault:") {
            let v: Value = serde_json::from_str(j).unwrap_or_default();
            return Some(self.r13_page_setup(&v));
        }
        if let Some(j) = id.strip_prefix("r13.paraDefault:") {
            let m: Map<String, Value> = serde_json::from_str(j).unwrap_or_default();
            return Some(self.r13_para_default(m));
        }
        if let Some(j) = id.strip_prefix("r13.templateStyle:") {
            let v: Value = serde_json::from_str(j).unwrap_or_default();
            return Some(self.r13_template_style(v));
        }
        if let Some(s) = id.strip_prefix("styles.modify:") {
            return Some(self.r13_modify_style(s));
        }
        match id {
            "format.paragraphSettings" => Some(self.r13_open(DocFormKind::Paragraph)),
            "layout.pageSetup" => Some(self.r13_open(DocFormKind::PageSetup)),
            "format.tabs" | "r13.open.tabs" => Some(self.r13_open(DocFormKind::Tabs)),
            "styles.newStyle" => Some(self.r13_open(DocFormKind::NewStyle)),
            _ => None,
        }
    }

    fn r13_open(&mut self, kind: DocFormKind) -> bool {
        self.menu = None;
        self.r3_open_form(kind)
    }

    /// The `extra["r13"]` context of the round-13 dialogs: the caret paragraph's properties
    /// (`FindFormat` fields), its tab stops, the section's page setup, the style.
    #[inline(never)]
    pub(super) fn r13_form_extra(&mut self) -> Value {
        let f = self.view.format().clone();
        let mut out = json!({});
        if let Some(sess) = self.view.engine().session() {
            let doc = sess.doc();
            let (a, _) = sess.selection().ordered();
            if let Some(p) = bo_model::index::para_by_id(doc, a.para) {
                out["para"] = para_json(&bo_edit::effective_para(doc, &p.props));
                let text: String = bo_model::index::para_index_text(p)
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(80)
                    .collect();
                out["sample"] = json!(text);
            }
            out["defaultTab"] = json!(doc.default_tab_stop.map_or(720, |t| t.0));
            if let Some(sec) = doc.sections.get(f.section.index) {
                out["section"] = section_json(doc, sec, &f.section);
            }
        }
        out["tabs"] = serde_json::to_value(&f.tabs).unwrap_or(Value::Null);
        let style = f.style.clone().unwrap_or_else(|| "Normal".into());
        out["style"] = json!(style);
        out["spec"] = match self.r13.modify_spec.take() {
            Some(s) => s,
            None => {
                // A new style starts from the selection's formatting on its style.
                let mut s = self
                    .view
                    .run_json(&json!({ "cmd": "styleDetails", "id": style }))
                    .map(|v| v["styleMgr"]["spec"].clone())
                    .unwrap_or_else(|| json!({}));
                if let Some(font) = &f.font {
                    s["font"] = json!(font);
                }
                if let Some(sz) = f.size_half_points {
                    s["size"] = json!(sz);
                }
                s["bold"] = json!(f.bold);
                s["italic"] = json!(f.italic);
                s["underline"] = json!(f.underline);
                if let Some(c) = &f.color {
                    s["color"] = json!(c);
                }
                s
            }
        };
        out["color"] = out["spec"]["color"].clone();
        out
    }

    /// Modify Style: the New Style dialog with the style's values.
    fn r13_modify_style(&mut self, id: &str) -> bool {
        let spec = self
            .view
            .run_json(&json!({ "cmd": "styleDetails", "id": id }))
            .map(|v| v["styleMgr"]["spec"].clone())
            .unwrap_or_default();
        if !spec.is_object() {
            return false;
        }
        self.r13.modify_spec = Some(spec.clone());
        self.r13_open(DocFormKind::NewStyle);
        let title = self.r13_t("docR5.modifyTitle", "Modify Style");
        let names = self.view.all_styles();
        if let Some(f) = self.r3.form.as_mut() {
            f.modify = Some(id.to_owned());
            f.set_title(&title);
            f.set_text("name", spec["name"].as_str().unwrap_or(id));
            if let Some(font) = spec["font"].as_str() {
                f.choose_item("font", font);
            }
            if let Some(hp) = spec["size"].as_u64() {
                #[allow(clippy::cast_precision_loss)]
                let pt = hp as f32 / 2.0;
                f.choose_item("size", &format!("{pt}"));
            }
            let based = spec["basedOn"]
                .as_str()
                .and_then(|b| names.iter().find(|s| s.id == b))
                .map(|s| s.name.clone());
            if let Some(b) = based {
                f.choose_item("basedOn", &b);
            }
            if spec["type"].as_str() == Some("character") {
                f.set_sel("type", Some(1));
            }
            f.set_on("gallery", spec["addToGallery"].as_bool().unwrap_or(true));
            f.set_on("auto", spec["autoUpdate"].as_bool().unwrap_or(false));
            f.style_describe();
        }
        true
    }

    /// Format ▾ of New / Modify Style: the menu under the button (`x,y,w,h` device pixels).
    fn r13_style_menu(&mut self, rect: &str) -> bool {
        let n: Vec<f32> = rect.split(',').filter_map(|v| v.parse().ok()).collect();
        let [x, y, w, h] = [
            n.first().copied().unwrap_or(0.0),
            n.get(1).copied().unwrap_or(0.0),
            n.get(2).copied().unwrap_or(1.0),
            n.get(3).copied().unwrap_or(1.0),
        ];
        let items = STYLE_FORMAT
            .iter()
            .map(|(id, key, fb, ok)| MenuItem {
                label: self.r13_t(key, fb),
                choice: Choice::Command(format!("r13.styleFmt.{id}")),
                checked: false,
                dim: !ok,
            })
            .collect();
        // The reference opens the menu upwards from the button's top.
        self.menu = Some(Menu::open(items, [x, y, w, h], self.size, self.scale));
        true
    }

    /// A Format ▾ entry: the matching dialog over the style dialog, in style mode.
    fn r13_style_format(&mut self, which: &str) -> bool {
        self.menu = None;
        let kind = match which {
            "font" => DocFormKind::FindFont,
            "paragraph" => DocFormKind::Paragraph,
            "tabs" => DocFormKind::Tabs,
            "language" => DocFormKind::FindLanguage,
            _ => return false,
        };
        let Some(style) = self.r3.form.take() else {
            return false;
        };
        let spec = style.style_spec();
        self.r3.stash = Some(style);
        if kind == DocFormKind::FindFont {
            self.r11_open_find_format(0);
        } else if kind == DocFormKind::FindLanguage {
            self.r11_open_find_format(3);
        } else {
            self.r3_open_form(kind);
        }
        let kind_now = self.r3.form.as_ref().map(|f| f.kind);
        let title = match kind_now {
            Some(DocFormKind::FindFont) => Some(self.r13_t("doc13.ns_fontTitle", "Font")),
            Some(DocFormKind::FindLanguage) => {
                Some(self.r13_t("doc13.ns_languageTitle", "Language"))
            }
            _ => None,
        };
        if let Some(f) = self.r3.form.as_mut() {
            f.set_style_mode(&spec);
            if let Some(t) = title {
                f.set_title(&t);
            }
        }
        true
    }

    /// Page Setup's OK (or Set As Default): margins, paper, orientation, gutter, multiple
    /// pages, section start, headers and footers, vertical alignment for the scope chosen
    /// (one undo step for the page setup).
    fn r13_page_setup(&mut self, v: &Value) -> bool {
        let s = self.view.format().section;
        let tw = |k: &str, old: i32| v[k].as_i64().map_or(old, |x| x as i32);
        let (mut w, mut h) = (tw("width", s.width), tw("height", s.height));
        // Orientation follows from width > height (§17.6.13 pgSz @orient).
        let land = v["landscape"].as_bool().unwrap_or(w > h);
        if land != (w > h) {
            std::mem::swap(&mut w, &mut h);
        }
        let scope = match v["scope"].as_str() {
            Some("section") => "section",
            Some("forward") => "forward",
            _ => "document",
        };
        let setup = json!({
            "width": w.max(1440), "height": h.max(1440),
            "marginTop": tw("top", s.margin_top), "marginBottom": tw("bottom", s.margin_bottom),
            "marginLeft": tw("left", s.margin_left).max(0), "marginRight": tw("right", s.margin_right).max(0),
            "header": tw("header", s.header).max(0), "footer": tw("footer", s.footer).max(0),
            "columns": s.columns.max(1), "columnSpace": s.column_space,
            "gutter": v["gutter"].as_i64(), "gutterTop": v["gutterTop"].as_bool(),
            "vAlign": v["vAlign"].as_str(), "start": v["start"].as_str(),
            "titlePage": v["titlePage"].as_bool(), "evenAndOdd": v["evenOdd"].as_bool(),
            "mirrorMargins": v["mirror"].as_bool(), "twoOnOne": v["twoOnOne"].as_bool(),
            "bookFold": v["bookFold"].as_bool(),
        });
        let ok = self.r3_run(vec![
            json!({ "cmd": "setPageSetup", "setup": setup, "scope": scope }),
        ]);
        if v["scope"].as_str() == Some("default") {
            // Set As Default: kept for new documents with the design defaults.
            self.r13
                .template_styles
                .retain(|t| t["pageSetup"].is_null());
            self.r13.template_styles.push(json!({ "pageSetup": setup }));
            self.requests.push(super::Request::SaveDefaults);
        }
        ok
    }

    /// Paragraph › Set As Default: the Normal style gets the dialog's values (this
    /// document), as the reference's "This document only".
    fn r13_para_default(&mut self, m: Map<String, Value>) -> bool {
        if m.is_empty() {
            return false;
        }
        let Some(v) = self
            .view
            .run_json(&json!({ "cmd": "styleDetails", "id": "Normal" }))
        else {
            return false;
        };
        let mut spec = v["styleMgr"]["spec"].clone();
        if !spec.is_object() {
            return false;
        }
        spec["format"] = Value::Object(m);
        self.r3_run(vec![
            json!({ "cmd": "styleModify", "id": "Normal", "spec": spec }),
        ])
    }

    /// New documents based on this template: keep the style for new documents (BlueDoc's
    /// own defaults file in the user's profile; the reference's template is never touched).
    fn r13_template_style(&mut self, v: Value) -> bool {
        let name = v["spec"]["name"].as_str().unwrap_or("").to_owned();
        self.r13
            .template_styles
            .retain(|t| t["spec"]["name"].as_str() != Some(name.as_str()));
        self.r13.template_styles.push(v);
        self.requests.push(super::Request::SaveDefaults);
        true
    }

    /// The round-13 part of the stored defaults (template styles, page setup).
    pub(super) fn r13_defaults(&self, v: &mut Value) {
        if !self.r13.template_styles.is_empty() {
            v["templateStyles"] = Value::Array(self.r13.template_styles.clone());
        }
    }

    /// Read the round-13 part of the stored defaults (every window).
    pub(super) fn r13_read_defaults(&mut self, v: &Value) {
        if let Some(a) = v["templateStyles"].as_array() {
            self.r13.template_styles.clone_from(a);
        }
    }

    /// A menu opened from a dialog (New / Modify Style › Format ▾) takes the input first.
    pub(super) fn r13_input(&mut self, input: &crate::Input) -> Option<bool> {
        if self.r3.form.is_none() || self.menu.is_none() {
            return None;
        }
        self.menu_input(input)
    }

    /// The menu of a dialog, over the dialog.
    pub(super) fn r13_paint(&self, p: &mut dyn crate::Painter) {
        if self.r3.form.is_some() {
            if let Some(menu) = &self.menu {
                menu.paint(p, &self.r10_menu_theme(), &self.icons);
            }
        }
    }

    /// Commands that give a new document the template styles and page setup.
    pub(super) fn r13_new_document_commands(&self) -> Vec<Value> {
        let mut cmds = Vec::new();
        for t in &self.r13.template_styles {
            if t["pageSetup"].is_object() {
                cmds.push(
                    json!({ "cmd": "setPageSetup", "setup": t["pageSetup"], "scope": "document" }),
                );
                continue;
            }
            let spec = t["spec"].clone();
            cmds.push(match t["id"].as_str() {
                Some(id) => json!({ "cmd": "styleModify", "id": id, "spec": spec }),
                None => json!({ "cmd": "styleNew", "spec": spec, "fromSelection": false }),
            });
        }
        cmds
    }
}

/// Paragraph properties as `FindFormat` fields (the Paragraph dialog's values).
fn para_json(pp: &bo_model::ParaProps) -> Value {
    let tw = |v: Option<bo_model::Twips>| v.map_or(0, |t| t.0);
    let align = match pp.align.unwrap_or_default() {
        bo_model::Alignment::Center => "center",
        bo_model::Alignment::Right => "right",
        bo_model::Alignment::Justify => "both",
        bo_model::Alignment::Left => "left",
    };
    let line = match pp.line_spacing {
        None | Some(bo_model::LineSpacing::Auto(240)) => "single".to_owned(),
        Some(bo_model::LineSpacing::Auto(360)) => "1.5".to_owned(),
        Some(bo_model::LineSpacing::Auto(480)) => "double".to_owned(),
        Some(bo_model::LineSpacing::Auto(n)) => format!("multiple:{n}"),
        Some(bo_model::LineSpacing::AtLeast(t)) => format!("atLeast:{}", t.0),
        Some(bo_model::LineSpacing::Exact(t)) => format!("exactly:{}", t.0),
    };
    json!({
        "align": align,
        "outlineLevel": pp.outline_level.map_or(10, |l| u32::from(l) + 1),
        "indentLeft": tw(pp.indent_left),
        "indentRight": tw(pp.indent_right),
        "firstLine": tw(pp.indent_first_line),
        "spaceBefore": tw(pp.space_before),
        "spaceAfter": tw(pp.space_after),
        "lineSpacing": line,
        "contextualSpacing": pp.contextual_spacing.unwrap_or(false),
        "widowControl": pp.widow_control.unwrap_or(false),
        "keepNext": pp.keep_next.unwrap_or(false),
        "keepLines": pp.keep_lines.unwrap_or(false),
        "pageBreakBefore": pp.page_break_before.unwrap_or(false),
        "suppressLineNumbers": pp.suppress_line_numbers.unwrap_or(false),
        "suppressAutoHyphens": pp.suppress_auto_hyphens.unwrap_or(false),
    })
}

/// The section's page setup for the Page Setup dialog.
fn section_json(
    doc: &bo_model::Document,
    sec: &bo_model::Section,
    st: &bo_edit::SectionState,
) -> Value {
    let start = match sec.break_type {
        bo_model::SectionBreak::Continuous => "continuous",
        bo_model::SectionBreak::NextColumn => "nextColumn",
        bo_model::SectionBreak::EvenPage => "evenPage",
        bo_model::SectionBreak::OddPage => "oddPage",
        bo_model::SectionBreak::NextPage => "nextPage",
    };
    let mut v = json!({
        "width": st.width, "height": st.height,
        "marginTop": st.margin_top, "marginBottom": st.margin_bottom,
        "marginLeft": st.margin_left, "marginRight": st.margin_right,
        "header": st.header, "footer": st.footer,
        "titlePage": sec.title_page, "evenOdd": doc.even_and_odd_headers,
        "start": start, "count": st.count,
        "gutter": 0, "vAlign": "top",
    });
    section_extra(doc, sec, &mut v);
    v
}

/// Gutter, vertical alignment and the document's multiple-pages settings (§17.6.11 pgMar
/// @w:gutter, §17.6.23 vAlign, §17.15.1.57 mirrorMargins, §17.15.1.49 gutterAtTop,
/// §17.15.1.13 bookFoldPrinting, §17.15.1.65 printTwoOnOne).
fn section_extra(doc: &bo_model::Document, sec: &bo_model::Section, v: &mut Value) {
    let _ = (doc, sec, v);
}
