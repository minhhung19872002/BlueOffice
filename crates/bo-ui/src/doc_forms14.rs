//! BlueDoc dialogs of parity round 14 (docs/ui/REPORT.md "Vòng 104"), laid out from the
//! reference word processor's dialogs captured at 125 % (window less its 8 px invisible
//! border): the Table of Figures and Table of Authorities tabs of Index and Tables
//! (§17.16.5.68 TOC `\c` / `\a`, §17.16.5.67 TOA) and Mark Citation (§17.16.5.62 TA). The
//! four tabs of Index and Tables are four dialogs of the same size: picking a tab opens the
//! other one in place. A child module of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut, Kind, LEADERS};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_render::Rgba;
use serde_json::json;

/// Caption labels offered by Table of Figures (§17.16.5.56 SEQ identifiers BlueDoc inserts).
const LABELS: [&str; 4] = ["(none)", "Equation", "Figure", "Table"];

/// Citation categories of `TA \c` (1–16).
const CATEGORIES: u8 = 16;

/// Whether `kind` is one of this module's dialogs.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::Tof
            | DocFormKind::Toa
            | DocFormKind::MarkCitation
            | DocFormKind::Watermark
            | DocFormKind::TofOptions
            | DocFormKind::Categories
    )
}

/// The Index and Tables dialog behind tab `tab` (Index, Table of Contents, Table of
/// Figures, Table of Authorities).
pub(super) fn tab_kind(tab: usize) -> DocFormKind {
    match tab {
        0 => DocFormKind::Index,
        1 => DocFormKind::Toc,
        2 => DocFormKind::Tof,
        _ => DocFormKind::Toa,
    }
}

/// Whether `kind` is a tab of Index and Tables.
pub(super) fn index_and_tables(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::Index | DocFormKind::Toc | DocFormKind::Tof | DocFormKind::Toa
    )
}

/// The reference's title of the four tabs.
pub(super) fn index_and_tables_title(s: &Strings) -> String {
    t(s, "doc14.indexAndTables", "Index and Tables")
}

/// Name of citation category `n` in the dialogs' language.
fn category_name(s: &Strings, n: u8) -> String {
    let vi = s.get("doc14.lang") == "vi";
    bo_edit::refs::toa_category_name(n, vi)
}

/// Build round-14 dialog `kind`.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    match kind {
        DocFormKind::Tof => open_tof(s),
        DocFormKind::Toa => open_toa(s, c),
        DocFormKind::Watermark => open_watermark(s, c),
        DocFormKind::TofOptions => open_tof_options(s, c),
        DocFormKind::Categories => open_categories(s, c),
        _ => open_mark_citation(s, c),
    }
}

fn index_tabs(s: &Strings) -> Vec<String> {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    vec![
        tr("docR3.tabIndex", "Index"),
        tr("docR3.tabToc", "Table of Contents"),
        tr("docR3.tabFigures", "Table of Figures"),
        tr("docR3.tabAuthorities", "Table of Authorities"),
    ]
}

/// Table of Figures (766 × 598 as the other tabs): previews, page number options, the
/// caption label and whether entries keep it.
fn open_tof(s: &Strings) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Tof,
        &index_and_tables_title(s),
        (766.0, 598.0),
        index_tabs(s),
    );
    f.form.tab = 2;
    f.label_on(
        2,
        "pp",
        [24.0, 88.0, 200.0, 18.0],
        &tr("docR3.printPreview", "Print Preview"),
    );
    f.label_on(
        2,
        "wp",
        [404.0, 88.0, 200.0, 18.0],
        &tr("docR3.webPreview", "Web Preview"),
    );
    f.form.add(
        "preview",
        Some(2),
        [24.0, 108.0, 330.0, 158.0],
        Kind::Canvas,
    );
    f.form.add(
        "webPreview",
        Some(2),
        [404.0, 108.0, 318.0, 158.0],
        Kind::Canvas,
    );
    f.form
        .add("tofRule", Some(2), [388.0, 118.0, 1.0, 260.0], Kind::Canvas);
    f.check_on(
        2,
        "pageNumbers",
        [24.0, 272.0, 330.0, 18.0],
        &tr("docR3.showPageNumbers", "Show page numbers"),
        true,
    );
    f.check_on(
        2,
        "rightAlign",
        [24.0, 297.0, 330.0, 18.0],
        &tr("docR3.rightAlign", "Right align page numbers"),
        true,
    );
    f.label_on(
        2,
        "leaderL",
        [24.0, 325.0, 80.0, 18.0],
        &tr("docR3.tabLeader", "Tab leader:"),
    );
    f.form.add(
        "leader",
        Some(2),
        [104.0, 320.0, 270.0, 26.0],
        Kind::Combo {
            items: LEADERS.iter().map(|l| l.1.to_owned()).collect(),
            sel: 0,
        },
    );
    f.check_on(
        2,
        "hyperlinks",
        [404.0, 272.0, 330.0, 18.0],
        &tr(
            "docR3.useHyperlinks",
            "Use hyperlinks instead of page numbers",
        ),
        true,
    );
    f.form.add(
        "general",
        Some(2),
        [24.0, 384.0, 718.0, 1.0],
        Kind::Group(tr("docR3.general", "General")),
    );
    f.label_on(
        2,
        "formatsL",
        [38.0, 416.0, 90.0, 18.0],
        &tr("docR3.formats", "Formats:"),
    );
    f.form.add(
        "formats",
        Some(2),
        [132.0, 410.0, 178.0, 26.0],
        Kind::Combo {
            items: vec![tr("docR3.fromTemplate", "From template")],
            sel: 0,
        },
    );
    f.label_on(
        2,
        "labelL",
        [38.0, 448.0, 94.0, 18.0],
        &tr("doc14.captionLabel", "Caption label:"),
    );
    let labels: Vec<String> = LABELS
        .iter()
        .map(|l| {
            if *l == "(none)" {
                tr("doc14.none", "(none)")
            } else {
                (*l).to_owned()
            }
        })
        .collect();
    f.form.add(
        "label",
        Some(2),
        [132.0, 442.0, 178.0, 26.0],
        Kind::Combo {
            items: labels,
            sel: 2,
        },
    );
    f.check_on(
        2,
        "includeLabel",
        [38.0, 476.0, 330.0, 18.0],
        &tr("doc14.includeLabel", "Include label and number"),
        true,
    );
    f.button_on(
        2,
        "options",
        [502.0, 504.0, 112.0, 28.0],
        &tr("docR3.options", "Options..."),
    );
    f.button_on(
        2,
        "modify",
        [629.0, 504.0, 112.0, 28.0],
        &tr("docR3.modify", "Modify..."),
    );
    f.ok_cancel(s);
    f.tof_preview();
    f
}

/// Table of Authorities: preview, passim, original formatting, leader, formats, the
/// category list and Mark Citation….
fn open_toa(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Toa,
        &index_and_tables_title(s),
        (766.0, 598.0),
        index_tabs(s),
    );
    f.form.tab = 3;
    f.label_on(
        3,
        "pp",
        [24.0, 88.0, 200.0, 18.0],
        &tr("docR3.printPreview", "Print Preview"),
    );
    f.form.add(
        "toaPreview",
        Some(3),
        [24.0, 108.0, 330.0, 158.0],
        Kind::Canvas,
    );
    f.label_on(
        3,
        "catL",
        [404.0, 88.0, 200.0, 18.0],
        &tr("doc14.category", "Category:"),
    );
    let mut cats = vec![tr("doc14.all", "All")];
    cats.extend(category_names(s, c));
    f.form.add(
        "category",
        Some(3),
        [404.0, 108.0, 338.0, 157.0],
        Kind::List {
            items: cats,
            sel: Some(0),
            top: 0,
        },
    );
    f.check_on(
        3,
        "passim",
        [24.0, 272.0, 330.0, 18.0],
        &tr("doc14.passim", "Use passim"),
        true,
    );
    f.check_on(
        3,
        "keepFormat",
        [24.0, 297.0, 330.0, 18.0],
        &tr("doc14.keepFormatting", "Keep original formatting"),
        true,
    );
    f.label_on(
        3,
        "leaderL",
        [24.0, 325.0, 80.0, 18.0],
        &tr("docR3.tabLeader", "Tab leader:"),
    );
    f.form.add(
        "leader",
        Some(3),
        [104.0, 320.0, 270.0, 26.0],
        Kind::Combo {
            items: LEADERS.iter().map(|l| l.1.to_owned()).collect(),
            sel: 0,
        },
    );
    f.label_on(
        3,
        "formatsL",
        [24.0, 357.0, 80.0, 18.0],
        &tr("docR3.formats", "Formats:"),
    );
    f.form.add(
        "formats",
        Some(3),
        [104.0, 352.0, 270.0, 26.0],
        Kind::Combo {
            items: vec![tr("docR3.fromTemplate", "From template")],
            sel: 0,
        },
    );
    f.button_on(
        3,
        "markCitation",
        [502.0, 504.0, 112.0, 28.0],
        &tr("doc14.markCitationB", "Mark Citation..."),
    );
    f.button_on(
        3,
        "modify",
        [629.0, 504.0, 112.0, 28.0],
        &tr("docR3.modify", "Modify..."),
    );
    f.ok_cancel(s);
    f.words = vec![
        ("heading".into(), category_name(s, 1)),
        (
            "entry".into(),
            tr("doc14.sampleCase", "Nguyen v. Tran Trading Co.,"),
        ),
        ("cite".into(), tr("doc14.sampleCite", "12 F.3d 345 (2001)")),
    ];
    f
}

/// Mark Citation (552 × 374 measured): the selected text, its category, the short and long
/// citations; Mark / Mark All keep the dialog open, as Mark Index Entry.
fn open_mark_citation(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::MarkCitation,
        &tr("doc14.markCitationTitle", "Mark Citation"),
        (552.0, 374.0),
        vec![],
    );
    let sel = c.selection.lines().next().unwrap_or("").trim().to_owned();
    f.label(
        "selL",
        [10.0, 46.0, 200.0, 18.0],
        &tr("doc14.selectedText", "Selected text:"),
    );
    f.edit("long", [10.0, 67.0, 403.0, 53.0], &sel);
    f.label(
        "catL",
        [10.0, 139.0, 70.0, 18.0],
        &tr("doc14.categoryL", "Category:"),
    );
    f.combo(
        "category",
        [80.0, 135.0, 336.0, 26.0],
        category_names(s, c),
        0,
    );
    f.label(
        "shortL",
        [10.0, 176.0, 200.0, 18.0],
        &tr("doc14.shortCitation", "Short citation:"),
    );
    f.edit("short", [10.0, 198.0, 403.0, 25.0], &sel);
    f.list(
        "shorts",
        [10.0, 224.0, 403.0, 71.0],
        c.extra["r14"]["shorts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default(),
        None,
    );
    f.label(
        "longL",
        [10.0, 301.0, 200.0, 18.0],
        &tr("doc14.longCitation", "Long citation:"),
    );
    // Empty until marked, as the reference (Mark uses the selected text).
    f.edit("longText", [10.0, 321.0, 403.0, 41.0], "");
    f.button(
        "next",
        [429.0, 69.0, 111.0, 28.0],
        &tr("doc14.nextCitation", "Next Citation"),
        false,
    );
    f.button(
        "ok",
        [429.0, 108.0, 111.0, 28.0],
        &tr("docR3.mark", "Mark"),
        true,
    );
    f.button(
        "markAll",
        [429.0, 146.0, 111.0, 28.0],
        &tr("docR3.markAll", "Mark All"),
        false,
    );
    f.button(
        "categories",
        [429.0, 186.0, 111.0, 28.0],
        &tr("doc14.categoryB", "Category..."),
        false,
    );
    f.button(
        "cancel",
        [429.0, 334.0, 111.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    // Next Citation needs a short citation to look for; Category… (renaming categories)
    // is not in BlueDoc's model.
    f.words = vec![("close".into(), tr("docR3.close", "Close"))];
    f.mark_citation_enable();
    f.form.focus("short");
    f.form.select_all("short");
    f
}

impl DocForm {
    /// Index and Tables: the dialog of the tab just picked (`None`: same tab).
    pub(super) fn r14_tab_switch(&self) -> Option<FormOut> {
        if !index_and_tables(self.kind) {
            return None;
        }
        let want = tab_kind(self.form.tab);
        (want != self.kind).then_some(FormOut::Open(want))
    }

    /// Pick tab `tab` of Index and Tables (tests).
    #[cfg(test)]
    pub(crate) fn pick_tab_for_test(&mut self, tab: usize) -> Option<FormOut> {
        self.form.tab = tab;
        self.r14_tab_switch()
    }

    /// Preview lines of Table of Figures for the chosen label.
    fn tof_preview(&mut self) {
        let label = match self.sel("label") {
            Some(0) => String::new(),
            Some(i) => LABELS.get(i).copied().unwrap_or("Figure").to_owned(),
            None => "Figure".to_owned(),
        };
        let include = self.on("includeLabel") && !label.is_empty();
        let page = self.on("pageNumbers");
        self.preview = (1..=3)
            .map(|i| {
                let text = if include {
                    format!("{label} {i}: Text")
                } else {
                    "Text".to_owned()
                };
                if page {
                    format!("{text}\t{}", 2 * i - 1)
                } else {
                    text
                }
            })
            .collect();
        // Entry widths are measured again when painted (none kept: estimated).
        self.label_w.clear();
    }

    /// Mark / Mark All need a long citation; Next Citation a short one.
    fn mark_citation_enable(&mut self) {
        let long = !self.text("longText").trim().is_empty() || !self.text("long").trim().is_empty();
        self.form.enable("ok", long);
        self.form.enable("markAll", long);
        let short = !self.text("short").trim().is_empty();
        self.form.enable("next", short);
    }

    /// The citation of Mark Citation (`None` without a long citation).
    fn citation(&self) -> Option<serde_json::Value> {
        let long = {
            let l = self.text("longText");
            if l.trim().is_empty() {
                self.text("long")
            } else {
                l
            }
        };
        if long.trim().is_empty() {
            return None;
        }
        let category = self.sel("category").map_or(1, |i| i + 1);
        Some(json!({
            "long": long.trim(),
            "short": self.text("short").trim(),
            "category": category,
        }))
    }

    /// A control of a round-14 dialog changed.
    pub(super) fn r14_changed(&mut self, id: &str) {
        match (self.kind, id) {
            (DocFormKind::Tof, "label" | "includeLabel" | "pageNumbers") => {
                let page = self.on("pageNumbers");
                self.form.enable("rightAlign", page);
                let right = page && self.on("rightAlign");
                self.form.enable("leader", right);
                self.form.enable("leaderL", right);
                self.tof_preview();
            }
            (DocFormKind::Tof, "rightAlign") => {
                let right = self.on("rightAlign");
                self.form.enable("leader", right);
                self.form.enable("leaderL", right);
            }
            (DocFormKind::MarkCitation, "shorts") => {
                if let Some(v) = self.sel("shorts").and_then(|i| {
                    match self.form.get("shorts").map(|c| &c.kind) {
                        Some(Kind::List { items, .. }) => items.get(i).cloned(),
                        _ => None,
                    }
                }) {
                    self.set_text("short", &v);
                }
                self.mark_citation_enable();
            }
            (DocFormKind::MarkCitation, _) => self.mark_citation_enable(),
            (DocFormKind::Categories, "cats") => {
                if let Some(v) = self
                    .sel("cats")
                    .and_then(|i| self.list_items("cats").get(i).cloned())
                {
                    self.set_text("with", &v);
                }
            }
            (DocFormKind::Watermark, "wmNone" | "wmPicture" | "wmText") => self.watermark_enable(),
            _ => {}
        }
    }

    /// A push button of a round-14 dialog.
    pub(super) fn r14_clicked(&mut self, id: &str) -> Option<FormOut> {
        Some(match (self.kind, id) {
            (DocFormKind::Toa, "markCitation") => FormOut::Open(DocFormKind::MarkCitation),
            (DocFormKind::Tof, "options") => FormOut::Stash(DocFormKind::TofOptions),
            // Modify…: the style the table's entries take (§17.7, Modify Style).
            (DocFormKind::Tof, "modify") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "styles.modify:TableofFigures" })],
                true,
            ),
            (DocFormKind::Toa, "modify") => FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "styles.modify:TableofAuthorities" })],
                true,
            ),
            (DocFormKind::MarkCitation, "categories") => FormOut::Stash(DocFormKind::Categories),
            (DocFormKind::Categories, "replace") => {
                let with = self.text("with");
                if let Some(i) = self.sel("cats").filter(|_| !with.trim().is_empty()) {
                    let mut items = self.list_items("cats");
                    if let Some(it) = items.get_mut(i) {
                        with.trim().clone_into(it);
                    }
                    self.form.set_items("cats", items, Some(i));
                }
                FormOut::Redraw
            }
            (DocFormKind::Watermark, "apply") => {
                FormOut::Run(vec![self.watermark_command()], false)
            }
            (DocFormKind::MarkCitation, "markAll") => match self.citation() {
                Some(mark) => {
                    self.set_button_label("cancel", &self.close_word());
                    FormOut::Run(
                        vec![json!({ "cmd": "markCitation", "mark": mark, "all": true })],
                        false,
                    )
                }
                None => FormOut::Redraw,
            },
            (DocFormKind::MarkCitation, "next") => {
                let what = self.text("short");
                FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": format!("r14.findNext:{what}") })],
                    false,
                )
            }
            _ => return None,
        })
    }

    fn set_button_label(&mut self, id: &str, text: &str) {
        if let Some(Kind::Button { label, .. }) = self.form.get_mut(id).map(|c| &mut c.kind) {
            text.clone_into(label);
        }
    }

    fn close_word(&self) -> String {
        self.words
            .iter()
            .find(|(k, _)| k == "close")
            .map_or_else(|| "Close".to_owned(), |(_, v)| v.clone())
    }

    /// OK of a round-14 dialog.
    pub(super) fn r14_ok(&mut self) -> Option<FormOut> {
        Some(match self.kind {
            DocFormKind::Tof => {
                let label = match self.sel("label") {
                    Some(0) => None,
                    Some(i) => LABELS.get(i).copied(),
                    None => Some("Figure"),
                };
                // §17.16.5.68 TOC \c "SEQ id" lists the captions with their label and
                // number, \a without them; \h hyperlinks, \z hides tab leaders and page
                // numbers in Web layout, \n omits page numbers.
                let mut instr = String::from("TOC");
                if self.on("hyperlinks") {
                    instr.push_str(" \\h \\z");
                }
                // Options…: a style (\t "style,1") and / or table entry fields (\f X)
                // build the table instead of the captions.
                let opts: Vec<&str> = self.toc_map.as_deref().map_or_else(Vec::new, |m| {
                    m.split(';').filter(|x| !x.is_empty()).collect()
                });
                for o in &opts {
                    if let Some(st) = o.strip_prefix("style:") {
                        instr.push_str(&format!(" \\t \"{st},1\""));
                    } else if let Some(id) = o.strip_prefix("fields:") {
                        instr.push_str(&format!(" \\f {id}"));
                    }
                }
                match label {
                    _ if !opts.is_empty() => {}
                    Some(l) if self.on("includeLabel") => {
                        instr.push_str(&format!(" \\c \"{l}\""));
                    }
                    Some(l) => instr.push_str(&format!(" \\a \"{l}\"")),
                    None => instr.push_str(" \\c"),
                }
                if !self.on("pageNumbers") {
                    instr.push_str(" \\n");
                }
                let leader = if self.on("pageNumbers") && self.on("rightAlign") {
                    LEADERS
                        .get(self.sel("leader").unwrap_or(0))
                        .map_or("dot", |l| l.0)
                } else {
                    "none"
                };
                FormOut::Run(
                    vec![
                        json!({ "cmd": "insertReferenceTable", "instr": instr, "leader": leader }),
                    ],
                    true,
                )
            }
            DocFormKind::Toa => {
                // §17.16.5.67 TOA: \h category headings, \c the category (0: all), \p
                // passim, \f drops the citations' own formatting.
                let category = self.sel("category").unwrap_or(0);
                let mut instr = format!("TOA \\h \\c \"{category}\"");
                if self.on("passim") {
                    instr.push_str(" \\p");
                }
                if !self.on("keepFormat") {
                    instr.push_str(" \\f");
                }
                let leader = LEADERS
                    .get(self.sel("leader").unwrap_or(0))
                    .map_or("dot", |l| l.0);
                FormOut::Run(
                    vec![
                        json!({ "cmd": "insertReferenceTable", "instr": instr, "leader": leader }),
                    ],
                    true,
                )
            }
            DocFormKind::Watermark => FormOut::Run(vec![self.watermark_command()], true),
            DocFormKind::TofOptions => FormOut::TocMap(self.tof_options_value()),
            DocFormKind::Categories => FormOut::Run(
                vec![json!({ "cmd": "ui.action",
                    "id": format!("r14.categories:{}", json!(self.list_items("cats"))) })],
                true,
            ),
            DocFormKind::MarkCitation => match self.citation() {
                Some(mark) => {
                    self.set_button_label("cancel", &self.close_word());
                    FormOut::Run(
                        vec![json!({ "cmd": "markCitation", "mark": mark, "all": false })],
                        false,
                    )
                }
                None => FormOut::Redraw,
            },
            _ => return None,
        })
    }
}

/// Paint a round-14 canvas; `false` when `id` is not one.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
) -> bool {
    if !handles(f.kind) {
        return false;
    }
    match id {
        "tofRule" => {
            p.fill_rect(r[0], r[1], 1.0, r[3], Rgba(0xa0, 0xa0, 0xa0, 255));
            true
        }
        "toaPreview" => {
            paint_toa_preview(f, r, p, font);
            true
        }
        _ => false,
    }
}

/// The Table of Authorities sample: a bold category heading, an entry and its citation
/// with dotted leader to its pages (own sample text, in the dialog's preview sans font).
fn paint_toa_preview(f: &DocForm, r: [f32; 4], p: &mut dyn Painter, font: &Font<'_>) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
    let size = (font.size * 1.75).round();
    let head = Font {
        family: "Arial",
        size,
        weight: 700,
        italic: false,
        underline: false,
    };
    let body = Font {
        family: "Calibri",
        size,
        weight: 400,
        italic: false,
        underline: false,
    };
    let word = |k: &str| {
        f.words
            .iter()
            .find(|(n, _)| n == k)
            .map_or("", |(_, v)| v.as_str())
            .to_owned()
    };
    let ink = Rgba(0, 0, 0, 255);
    let x = r[0] + 4.0;
    p.text_font(&word("heading"), x, r[1] + 8.0, &head, ink);
    p.text_font(&word("entry"), x, r[1] + 8.0 + size * 2.15, &body, ink);
    // Two pages: "passim" would need five or more (bo_edit::refs::PASSIM_PAGES).
    let pages = "5, 6";
    let y = r[1] + 8.0 + size * 3.6;
    let cite = word("cite");
    let indent = size * 1.05;
    p.text_font(&cite, x + indent, y, &body, ink);
    // Dotted leader from the citation to its pages.
    let cite_w = size * 0.43 * cite.chars().count() as f32;
    let page_w = size * 0.5 * pages.chars().count() as f32;
    let end = r[0] + r[2] - 8.0 - page_w;
    let mut dx = x + indent + cite_w + 2.0;
    while dx < end - 3.0 {
        p.fill_rect(dx.round(), y + size * 1.02, 1.5, 1.5, ink);
        dx += size * 0.27;
    }
    p.text_font(pages, end, y, &body, ink);
    p.pop_clip();
}

// ------------------------------------------------------------------ Printed Watermark

/// Fonts offered for a text watermark.
const WM_FONTS: [&str; 6] = [
    "Calibri",
    "Arial",
    "Cambria",
    "Segoe UI",
    "Tahoma",
    "Times New Roman",
];

/// Sizes offered (points; "Auto" first = fills the page).
const WM_SIZES: [u16; 14] = [36, 40, 44, 48, 54, 60, 66, 72, 80, 90, 96, 105, 120, 144];

/// Text colours offered: `RRGGBB` ("" = automatic light grey) and en-US name.
const WM_COLORS: [(&str, &str); 6] = [
    ("", "Automatic"),
    ("C0C0C0", "Silver"),
    ("808080", "Gray"),
    ("C00000", "Dark Red"),
    ("FF0000", "Red"),
    ("0070C0", "Blue"),
];

/// Printed Watermark (472 × 410 measured): No / Picture / Text watermark, the text's
/// language, text, font, size, colour, transparency and layout; Apply, OK, Cancel.
/// The document's watermark (`extra["r14"]["watermark"]`, `WatermarkInfo`) fills it.
pub(super) fn open_watermark(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Watermark,
        &tr("doc14.wmTitle", "Printed Watermark"),
        (472.0, 410.0),
        vec![],
    );
    let v = &c.extra["r14"]["watermark"];
    let text = v["text"].as_str().map(str::to_owned);
    let picture = v["picture"].as_bool() == Some(true);
    f.radio(
        "wmNone",
        "wm",
        [11.0, 48.0, 220.0, 20.0],
        &tr("doc14.wmNone", "No watermark"),
        text.is_none() && !picture,
    );
    f.radio(
        "wmPicture",
        "wm",
        [11.0, 74.0, 220.0, 20.0],
        &tr("doc14.wmPicture", "Picture watermark"),
        picture,
    );
    f.button(
        "selPicture",
        [34.0, 98.0, 111.0, 27.0],
        &tr("doc14.wmSelectPicture", "Select Picture..."),
        false,
    );
    f.label(
        "scaleL",
        [34.0, 135.0, 70.0, 18.0],
        &tr("doc14.wmScale", "Scale:"),
    );
    f.combo(
        "scale",
        [108.0, 131.0, 172.0, 26.0],
        vec![
            tr("doc14.wmAuto", "Auto"),
            "500%".into(),
            "200%".into(),
            "150%".into(),
            "100%".into(),
            "50%".into(),
        ],
        0,
    );
    f.check(
        "washout",
        [292.0, 135.0, 140.0, 18.0],
        &tr("doc14.wmWashout", "Washout"),
        true,
    );
    f.radio(
        "wmText",
        "wm",
        [11.0, 163.0, 220.0, 20.0],
        &tr("doc14.wmText", "Text watermark"),
        text.is_some(),
    );
    let rows: [(&'static str, &str, &str, f32); 5] = [
        ("langL", "doc14.wmLanguage", "Language:", 189.0),
        ("textL", "doc14.wmTextL", "Text:", 218.0),
        ("fontL", "doc14.wmFont", "Font:", 249.0),
        ("sizeL", "doc14.wmSize", "Size:", 279.0),
        ("colorL", "doc14.wmColor", "Color:", 309.0),
    ];
    for (id, key, fb, y) in rows {
        f.label(id, [34.0, y, 72.0, 18.0], &tr(key, fb));
    }
    f.combo(
        "language",
        [108.0, 185.0, 353.0, 26.0],
        vec![
            tr("doc14.wmEnglish", "English (United States)"),
            tr("doc14.wmVietnamese2", "Vietnamese"),
        ],
        usize::from(s.get("doc14.lang") == "vi"),
    );
    f.edit(
        "wmTextBox",
        [108.0, 214.0, 353.0, 26.0],
        text.as_deref().unwrap_or(""),
    );
    let font = v["font"].as_str().unwrap_or("Calibri");
    f.combo(
        "font",
        [108.0, 245.0, 353.0, 26.0],
        WM_FONTS.iter().map(|f| (*f).to_owned()).collect(),
        WM_FONTS.iter().position(|x| *x == font).unwrap_or(0),
    );
    let mut sizes = vec![tr("doc14.wmAuto", "Auto")];
    sizes.extend(WM_SIZES.iter().map(u16::to_string));
    f.combo("size", [108.0, 275.0, 172.0, 26.0], sizes, 0);
    let color = v["color"].as_str().unwrap_or("");
    f.combo(
        "color",
        [108.0, 305.0, 172.0, 26.0],
        WM_COLORS
            .iter()
            .map(|(_, n)| tr(&format!("doc14.wmColor{}", n.replace(' ', "")), n))
            .collect(),
        WM_COLORS
            .iter()
            .position(|(h, _)| h.eq_ignore_ascii_case(color) && !h.is_empty())
            .unwrap_or(0),
    );
    f.check(
        "semi",
        [292.0, 309.0, 170.0, 18.0],
        &tr("doc14.wmSemitransparent", "Semitransparent"),
        v["semitransparent"].as_bool().unwrap_or(true),
    );
    f.label(
        "layoutL",
        [34.0, 343.0, 72.0, 18.0],
        &tr("doc14.wmLayout", "Layout:"),
    );
    let diagonal = v["diagonal"].as_bool().unwrap_or(true);
    f.radio(
        "diagonal",
        "layout",
        [108.0, 341.0, 110.0, 20.0],
        &tr("doc14.wmDiagonal", "Diagonal"),
        diagonal,
    );
    f.radio(
        "horizontal",
        "layout",
        [226.0, 341.0, 120.0, 20.0],
        &tr("doc14.wmHorizontal", "Horizontal"),
        !diagonal,
    );
    f.button(
        "apply",
        [132.0, 372.0, 104.0, 28.0],
        &tr("doc14.wmApply", "Apply"),
        false,
    );
    f.button(
        "ok",
        [244.0, 372.0, 104.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [356.0, 372.0, 104.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.watermark_enable();
    f
}

impl DocForm {
    /// The picture or text controls follow the kind of watermark chosen. A picture needs a
    /// file BlueDoc's dialog cannot pick yet: its controls stay dimmed.
    fn watermark_enable(&mut self) {
        let text = self.chosen("wm") == Some("wmText");
        for id in [
            "langL",
            "language",
            "textL",
            "wmTextBox",
            "fontL",
            "font",
            "sizeL",
            "size",
            "colorL",
            "color",
            "semi",
            "layoutL",
            "diagonal",
            "horizontal",
        ] {
            self.form.enable(id, text);
            if let Some(r) = self.radios.iter_mut().find(|r| r.id == id) {
                r.enabled = text;
            }
        }
        for id in ["selPicture", "scaleL", "scale", "washout"] {
            self.form.enable(id, false);
        }
        if text && self.text("wmTextBox").trim().is_empty() {
            self.set_text("wmTextBox", "CONFIDENTIAL");
        }
    }

    /// The watermark chosen (`setWatermark` command).
    fn watermark_command(&self) -> serde_json::Value {
        let spec = (self.chosen("wm") == Some("wmText"))
            .then(|| self.text("wmTextBox"))
            .filter(|t| !t.trim().is_empty())
            .map(|text| {
                let font = WM_FONTS
                    .get(self.sel("font").unwrap_or(0))
                    .copied()
                    .unwrap_or("Calibri");
                let size = self
                    .sel("size")
                    .filter(|&i| i > 0)
                    .and_then(|i| WM_SIZES.get(i - 1))
                    .map(|s| f32::from(*s));
                let color = WM_COLORS
                    .get(self.sel("color").unwrap_or(0))
                    .map(|c| c.0)
                    .filter(|h| !h.is_empty());
                json!({
                    "text": text.trim(), "font": font, "size": size, "color": color,
                    "semitransparent": self.on("semi"),
                    "diagonal": self.chosen("layout") != Some("horizontal"),
                })
            });
        json!({ "cmd": "setWatermark", "watermark": spec })
    }
}

// ------------------------------------------------- Table of Figures Options, Edit Category

/// Table identifiers of `TC \f` / `TOC \f` (§17.16.5.65, §17.16.5.68).
const IDENTIFIERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Table of Figures Options: build the table from a style (`TOC \t "style,1"`) and / or from
/// table entry fields with an identifier (`TOC \f X`). OK hands the choice back to Table
/// of Figures as `style:<id>` / `fields:<X>` joined by `;`.
pub(super) fn open_tof_options(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::TofOptions,
        &tr("doc14.tofOptionsTitle", "Table of Figures Options"),
        (380.0, 250.0),
        vec![],
    );
    f.label(
        "buildL",
        [16.0, 48.0, 300.0, 18.0],
        &tr("doc14.tofBuildFrom", "Build table of figures from:"),
    );
    f.check(
        "useStyle",
        [28.0, 76.0, 90.0, 20.0],
        &tr("doc14.tofStyle", "Style:"),
        false,
    );
    let paras: Vec<&(String, String, bool)> = c.styles.iter().filter(|x| x.2).collect();
    f.ids = paras.iter().map(|x| x.0.clone()).collect();
    let caption = f.ids.iter().position(|i| i == "Caption").unwrap_or(0);
    f.combo(
        "style",
        [126.0, 72.0, 236.0, 26.0],
        paras.iter().map(|x| x.1.clone()).collect(),
        caption,
    );
    f.check(
        "useFields",
        [28.0, 112.0, 300.0, 20.0],
        &tr("doc14.tofFields", "Table entry fields"),
        false,
    );
    f.label(
        "idL",
        [28.0, 150.0, 110.0, 18.0],
        &tr("doc14.tofIdentifier", "Table identifier:"),
    );
    f.combo(
        "identifier",
        [146.0, 146.0, 70.0, 26.0],
        IDENTIFIERS.chars().map(String::from).collect(),
        5,
    );
    f.ok_cancel(s);
    f
}

/// Edit Category (Mark Citation › Category…): rename the citation categories for this
/// window. OK sends the sixteen names back (`r14.categories:` interface command).
pub(super) fn open_categories(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Categories,
        &tr("doc14.editCategoryTitle", "Edit Category"),
        (420.0, 340.0),
        vec![],
    );
    f.label(
        "catL",
        [16.0, 48.0, 200.0, 18.0],
        &tr("doc14.categoryL", "Category:"),
    );
    f.list(
        "cats",
        [16.0, 70.0, 250.0, 200.0],
        category_names(s, c),
        Some(7),
    );
    f.label(
        "withL",
        [16.0, 280.0, 120.0, 18.0],
        &tr("doc14.replaceWith", "Replace with:"),
    );
    f.edit("with", [130.0, 276.0, 136.0, 26.0], "8");
    f.button(
        "replace",
        [282.0, 70.0, 120.0, 28.0],
        &tr("doc14.replace", "Replace"),
        false,
    );
    f.button(
        "ok",
        [282.0, 236.0, 120.0, 28.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [282.0, 274.0, 120.0, 28.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// The sixteen category names: renamed ones (`extra["r14"]["categories"]`) or BlueDoc's.
fn category_names(s: &Strings, c: &FormContext) -> Vec<String> {
    let renamed: Vec<String> = c.extra["r14"]["categories"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    (1..=CATEGORIES)
        .map(|n| {
            renamed
                .get(usize::from(n) - 1)
                .cloned()
                .unwrap_or_else(|| category_name(s, n))
        })
        .collect()
}

impl DocForm {
    fn list_items(&self, id: &str) -> Vec<String> {
        match self.form.get(id).map(|c| &c.kind) {
            Some(Kind::List { items, .. } | Kind::Combo { items, .. }) => items.clone(),
            _ => Vec::new(),
        }
    }

    /// New category names (Edit Category OK): Mark Citation's drop-down and the Table of
    /// Authorities list follow.
    pub(crate) fn r14_set_categories(&mut self, names: &[String]) {
        match self.kind {
            DocFormKind::MarkCitation => {
                let sel = self.sel("category");
                self.form.set_items("category", names.to_vec(), sel);
            }
            DocFormKind::Toa => {
                let mut items = vec![self
                    .list_items("category")
                    .first()
                    .cloned()
                    .unwrap_or_default()];
                items.extend(names.iter().cloned());
                let sel = self.sel("category");
                self.form.set_items("category", items, sel);
            }
            _ => {}
        }
    }

    /// Table of Figures Options OK: `style:<id>` and / or `fields:<X>`.
    fn tof_options_value(&self) -> String {
        let mut out = Vec::new();
        if self.on("useStyle") {
            if let Some(id) = self.sel("style").and_then(|i| self.ids.get(i)) {
                out.push(format!("style:{id}"));
            }
        }
        if self.on("useFields") {
            let i = self.sel("identifier").unwrap_or(5);
            if let Some(ch) = IDENTIFIERS.chars().nth(i) {
                out.push(format!("fields:{ch}"));
            }
        }
        out.join(";")
    }
}
