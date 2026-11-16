//! BlueReader round 4 (a child of `reader_window`): the Form and Protect tabs as far as a
//! reader goes (fill text fields, check boxes, radio buttons and lists; reset; field values to
//! a spreadsheet file; field highlighting; signatures verified read-only with `bo-sign`), and
//! Comment tab additions: Eraser, Area Highlight, Custom Stamp from a picture, the comment
//! list's sort and filter.
use super::{inside, ReaderRequest, ReaderWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, MenuItem};
use crate::{Input, Painter, TextMeasure};
use bo_pdfread::{AnnotKind, CommentThread, FieldKind, FormField, NewAnnot};
use bo_render::Rgba;

/// A pointer tool of the additions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Tool4 {
    /// Removes pencil strokes under the pointer.
    Eraser,
    /// Drag a box: a highlight over it.
    AreaHighlight,
    /// Drag a box (or click): the picture as a stamp.
    ImageStamp(Vec<u8>),
    /// Drag a box: that part of the page copied as a picture (SnapShot).
    Snapshot,
}

/// Field highlight of the Form tab (the reference's light blue).
const FIELD_HIGHLIGHT: Rgba = Rgba(204, 215, 255, 110);
/// Rubber band of Area Highlight / Custom Stamp.
const BAND: Rgba = Rgba(0x1f, 0x5f, 0xbf, 255);
/// Area highlight colour (the reference's yellow).
const AREA_YELLOW: [u8; 3] = [255, 230, 0];

/// State of the round 4 tab additions.
#[derive(Default)]
pub(crate) struct Tabs4 {
    pub(crate) tool: Option<Tool4>,
    /// Drag start and now (window pixels).
    drag: Option<(f32, f32, f32, f32)>,
    /// Form fields shaded.
    pub(crate) highlight_fields: bool,
    /// Text being typed into a field.
    pub(crate) field_dialog: Option<(Dialog, FormField)>,
    /// A list or combo field whose choices are shown.
    choice_field: Option<FormField>,
    /// Comment list: sort (0 page, 1 author, 2 date, 3 type) and a kind shown alone.
    pub(crate) sort: u8,
    pub(crate) filter: Option<AnnotKind>,
    /// Sort and filter boxes of the comment list header.
    pub(crate) header: [[f32; 4]; 2],
    /// Replace Text: the strike-out just made asks for the replacement.
    replace: bool,
    /// Search & Highlight: the words asked for.
    pub(crate) search_dialog: Option<Dialog>,
}

/// Kinds offered by the comment list's filter.
const FILTERS: [(AnnotKind, &str); 7] = [
    (AnnotKind::Highlight, "pdfReader.highlight"),
    (AnnotKind::Underline, "pdfReader.underline"),
    (AnnotKind::StrikeOut, "pdfReader.strikeout"),
    (AnnotKind::Note, "pdfReader.note"),
    (AnnotKind::TextBox, "pdfReader.textbox"),
    (AnnotKind::Ink, "pdfDraw.pencil"),
    (AnnotKind::Stamp, "pdfDraw.stamp"),
];

impl ReaderWindow {
    /// Ribbon items of the Form tab (index 3) and the Protect tab (4), after Hand and Select.
    pub(crate) fn tabs4_items(tab: usize) -> Vec<(&'static str, super::Kind)> {
        use super::Kind::{Big, Sep};
        match tab {
            3 => vec![
                ("sep1", Sep),
                ("form.reset", Big("resetForm", "pdfForm.reset", false)),
                ("form.toSheet", Big("formToSheet", "pdfForm.toSheet", false)),
                ("form.import", Big("import", "pdfForm.import", false)),
                // The reference's Export carries a drop-down arrow (greyed in a reader).
                ("form.export", Big("export", "pdfForm.export", true)),
                ("sep2", Sep),
                (
                    "menu.formSettings",
                    Big("viewSetting", "pdfForm.settings", true),
                ),
            ],
            4 => vec![
                ("sep1", Sep),
                (
                    "prot.fillSign",
                    Big("fillSign", "pdfProtect.fillSign", false),
                ),
                ("prot.sign", Big("sign", "pdfProtect.sign", true)),
                (
                    "prot.timestamp",
                    Big("timestamp", "pdfProtect.timestamp", false),
                ),
                (
                    "prot.validate",
                    Big("validate", "pdfProtect.validate", false),
                ),
            ],
            _ => Vec::new(),
        }
    }

    /// Commands that a reader does not offer (drawn disabled).
    pub(crate) fn tabs4_disabled(id: &str) -> bool {
        matches!(id, "form.import" | "form.export")
    }

    /// After a text markup was made: Replace Text asks for the replacement.
    pub(crate) fn after_markup(&mut self, m: &dyn TextMeasure) {
        if !std::mem::take(&mut self.tabs4.replace) {
            return;
        }
        let newest = self.tabs.get(self.active).and_then(|t| {
            t.view
                .doc()
                .annotations()
                .into_iter()
                .filter(|a| a.kind == AnnotKind::StrikeOut)
                .max_by(|a, b| a.modified.cmp(&b.modified))
        });
        if let Some(a) = newest {
            self.ask_comment_text(a, false, m);
        }
        self.tool = super::Tool::Hand;
    }

    /// Highlight every place `words` occur (Search & Highlight).
    fn search_highlight(&mut self, words: &str) {
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let n = t.view.find(words, false, false);
        let hits = t.view.hits().to_vec();
        let mut by_page: std::collections::BTreeMap<usize, Vec<[f32; 4]>> =
            std::collections::BTreeMap::default();
        for h in hits {
            if let Some(layer) = t.view.text(h.page) {
                for r in layer.rects(h.a, h.b) {
                    by_page.entry(h.page).or_default().push(r);
                }
            }
        }
        for (page, rects) in by_page {
            let rects = rects
                .iter()
                .map(|r| t.view.unrotate_rect(page, *r))
                .collect();
            let a = NewAnnot {
                kind: AnnotKind::Highlight,
                page,
                rects,
                color: AREA_YELLOW,
                author: author.clone(),
                contents: String::new(),
            };
            if let Err(e) = t.view.doc_mut().add_annotation(&a) {
                self.status = e;
                break;
            }
        }
        t.view.clear_hits();
        self.status = self
            .strings
            .fmt("pdfComment.highlighted", &[("n", &n.to_string())]);
        self.document_changed();
    }

    /// Commands of the additions; `true` when `cmd` was one.
    pub(crate) fn command_tabs4(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        match cmd {
            "comment.eraser" => self.toggle_tool4(Tool4::Eraser),
            "home.snapshot" => self.toggle_tool4(Tool4::Snapshot),
            // Replace Text: strike the text out and say what replaces it (§12.5.6.10, a
            // strike-out with the replacement as its comment).
            "comment.replace" => {
                self.tool = super::Tool::Markup(AnnotKind::StrikeOut);
                self.tabs4.replace = true;
            }
            // Insert Text: a note where the text goes.
            "comment.insert" => {
                self.tool = super::Tool::Note;
                self.tabs4.replace = false;
            }
            "comment.searchHighlight" => {
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    self.t("pdfComment.searchHighlight"),
                    vec![(self.t("pdfComment.words"), String::new())],
                    vec![
                        (self.t("pdfReader.ok"), DialogButton::Ok),
                        (self.t("pdfPrint.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                );
                d.layout(m, self.size, self.scale);
                self.tabs4.search_dialog = Some(d);
            }
            "comment.areaHighlight" => self.toggle_tool4(Tool4::AreaHighlight),
            "menu.customStamp" => {
                let items = vec![MenuItem {
                    label: self.t("pdfComment.stampFromImage"),
                    choice: Choice::Command("stamp.image".into()),
                    checked: false,
                    dim: false,
                }];
                let r = self.control_rect("menu.customStamp").unwrap_or([0.0; 4]);
                self.menu = Some((self.reader_menu(items, r), "menu.customStamp"));
            }
            "stamp.image" => self.requests.push(ReaderRequest::PickFile {
                purpose: "stamp".into(),
                filters: vec![(self.t("pdfComment.images"), "*.png;*.jpg;*.jpeg".into())],
            }),
            "form.reset" => {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    if let Err(e) = t.view.doc_mut().reset_form() {
                        self.status = e;
                    }
                }
                self.document_changed();
            }
            "form.toSheet" => self.form_to_sheet(),
            "menu.formSettings" => {
                let items = vec![MenuItem {
                    label: self.t("pdfForm.highlight"),
                    choice: Choice::Command("form.highlight".into()),
                    checked: self.tabs4.highlight_fields,
                    dim: false,
                }];
                let r = self.control_rect("menu.formSettings").unwrap_or([0.0; 4]);
                self.menu = Some((self.reader_menu(items, r), "menu.formSettings"));
            }
            "form.highlight" => self.tabs4.highlight_fields = !self.tabs4.highlight_fields,
            "prot.validate" => self.validate_signatures(),
            "menu.commentSort" => {
                let items = [
                    "pdfComment.sortPage",
                    "pdfComment.sortAuthor",
                    "pdfComment.sortDate",
                    "pdfComment.sortType",
                ]
                .iter()
                .enumerate()
                .map(|(k, key)| MenuItem {
                    label: self.t(key),
                    choice: Choice::Command(format!("comments.sort{k}")),
                    checked: usize::from(self.tabs4.sort) == k,
                    dim: false,
                })
                .collect();
                let r = self.tabs4.header[0];
                self.menu = Some((self.reader_menu(items, r), "comments"));
            }
            "menu.commentFilter" => {
                let mut items = vec![MenuItem {
                    label: self.t("pdfComment.filterAll"),
                    choice: Choice::Command("comments.filterAll".into()),
                    checked: self.tabs4.filter.is_none(),
                    dim: false,
                }];
                for (k, (kind, key)) in FILTERS.iter().enumerate() {
                    items.push(MenuItem {
                        label: self.t(key),
                        choice: Choice::Command(format!("comments.filter{k}")),
                        checked: self.tabs4.filter == Some(*kind),
                        dim: false,
                    });
                }
                let r = self.tabs4.header[1];
                self.menu = Some((self.reader_menu(items, r), "comments"));
            }
            "comments.filterAll" => self.tabs4.filter = None,
            c if c.starts_with("comments.sort") => {
                self.tabs4.sort = c[13..].parse().unwrap_or(0);
            }
            c if c.starts_with("comments.filter") => {
                let k: usize = c[15..].parse().unwrap_or(0);
                self.tabs4.filter = FILTERS.get(k).map(|f| f.0);
            }
            _ => return false,
        }
        let _ = m;
        for (id, on) in [
            ("comment.eraser", self.tabs4.tool == Some(Tool4::Eraser)),
            (
                "comment.areaHighlight",
                self.tabs4.tool == Some(Tool4::AreaHighlight),
            ),
            ("home.snapshot", self.tabs4.tool == Some(Tool4::Snapshot)),
        ] {
            if on {
                self.on.insert(id);
            } else {
                self.on.remove(id);
            }
        }
        true
    }

    fn toggle_tool4(&mut self, tool: Tool4) {
        self.tabs4.tool = if self.tabs4.tool.as_ref() == Some(&tool) {
            None
        } else {
            Some(tool)
        };
        self.tabs4.drag = None;
    }

    /// A file the host read for the additions (`stamp`: a picture for Custom Stamp).
    pub fn file_picked4(&mut self, purpose: &str, bytes: &[u8]) -> bool {
        if purpose != "stamp" {
            return false;
        }
        self.tabs4.tool = Some(Tool4::ImageStamp(bytes.to_vec()));
        self.status = self.t("pdfComment.placeStamp");
        true
    }

    /// Field names and values as a CSV file (the reference's Form to sheet).
    fn form_to_sheet(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut csv = String::from("\u{feff}");
        let fields = t.view.doc().form_fields();
        let mut seen: Vec<&str> = Vec::new();
        csv.push_str(&fields.iter().fold(String::new(), |mut row, f| {
            if !seen.contains(&f.name.as_str()) && f.kind != FieldKind::Button {
                seen.push(&f.name);
                if !row.is_empty() {
                    row.push(',');
                }
                row.push_str(&quote(&f.name));
            }
            row
        }));
        csv.push_str("\r\n");
        let mut values = Vec::new();
        for name in &seen {
            let v = fields
                .iter()
                .filter(|f| f.name == *name)
                .find_map(|f| match f.kind {
                    FieldKind::CheckBox | FieldKind::Radio => f.checked.then(|| f.value.clone()),
                    _ => Some(f.value.clone()),
                })
                .unwrap_or_default();
            values.push(quote(&v));
        }
        csv.push_str(&values.join(","));
        csv.push_str("\r\n");
        let stem = t.name.trim_end_matches(".pdf").to_owned();
        self.requests.push(ReaderRequest::SaveBytes {
            name: format!("{stem}.csv"),
            bytes: csv.into_bytes(),
        });
    }

    /// Verify the document's signatures (read-only) and show the result.
    fn validate_signatures(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let lines = match bo_sign::pdf::verify_pdf(t.view.doc().bytes()) {
            Ok(sigs) if sigs.is_empty() => vec![self.t("pdfProtect.noSignatures")],
            Ok(sigs) => sigs
                .iter()
                .enumerate()
                .map(|(k, s)| {
                    let state = if s.covers_whole_file || s.only_validation_data_after {
                        self.t("pdfProtect.valid")
                    } else {
                        self.t("pdfProtect.changedAfter")
                    };
                    format!("{}. {} — {state}", k + 1, s.signer.subject)
                })
                .collect(),
            Err(e) => vec![format!("{}: {e}", self.t("pdfProtect.invalid"))],
        };
        let d = Dialog::new(
            DialogKind::WordCount,
            self.t("pdfProtect.validate"),
            Vec::new(),
            vec![(self.t("pdfReader.ok"), DialogButton::Ok)],
            lines,
        );
        self.dialog = Some((d, None));
    }

    /// Input of the additions first: `Some(redraw)` when handled.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn route_tabs4(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(d) = self.tabs4.search_dialog.as_mut() {
            let done = match input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } => d.press(*x, *y),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) => return None,
                _ => None,
            };
            if let Some(b) = done {
                if let Some(d) = self.tabs4.search_dialog.take() {
                    let words = d.values().into_iter().next().unwrap_or_default();
                    if b == DialogButton::Ok && !words.trim().is_empty() {
                        self.search_highlight(words.trim());
                    }
                }
            }
            return Some(true);
        }
        // Text typed into a form field.
        if let Some((d, _)) = self.tabs4.field_dialog.as_mut() {
            let done = match input {
                Input::Text(s) => {
                    d.text(s);
                    None
                }
                Input::Key(k, mods) => d.key(*k, *mods),
                Input::Press { x, y, .. } => d.press(*x, *y),
                Input::Move { x, y } => {
                    d.hover(*x, *y);
                    None
                }
                Input::Resize { .. } | Input::Scale(_) => return None,
                _ => None,
            };
            if let Some(b) = done {
                if let Some((d, f)) = self.tabs4.field_dialog.take() {
                    if b == DialogButton::Ok {
                        let v = d.values().into_iter().next().unwrap_or_default();
                        self.set_field(&f, Some(&v), None);
                    }
                }
            }
            return Some(true);
        }
        // Comment list header.
        if let Input::Press { x, y, .. } = input {
            if self.panel_open() == Some(super::Panel::Comments) {
                if inside(self.tabs4.header[0], *x, *y) {
                    self.command_tabs4("menu.commentSort", m);
                    return Some(true);
                }
                if inside(self.tabs4.header[1], *x, *y) {
                    self.command_tabs4("menu.commentFilter", m);
                    return Some(true);
                }
            }
        }
        let d = self.doc_rect;
        if let Some(tool) = self.tabs4.tool.clone() {
            match input {
                Input::Press { x, y, .. } if inside(d, *x, *y) => {
                    self.tabs4.drag = Some((*x, *y, *x, *y));
                    if tool == Tool4::Eraser {
                        self.erase_at(*x, *y);
                    }
                    return Some(true);
                }
                Input::Drag { x, y } if self.tabs4.drag.is_some() => {
                    if let Some(g) = self.tabs4.drag.as_mut() {
                        g.2 = *x;
                        g.3 = *y;
                    }
                    if tool == Tool4::Eraser {
                        self.erase_at(*x, *y);
                    }
                    return Some(true);
                }
                Input::Release { x, y } if self.tabs4.drag.is_some() => {
                    let Some((x0, y0, _, _)) = self.tabs4.drag.take() else {
                        return Some(true);
                    };
                    match tool {
                        Tool4::Eraser => {}
                        Tool4::Snapshot => {
                            if let Some((page, rect)) = self.page_box((x0, y0), (*x, *y)) {
                                if rect[2] - rect[0] > 2.0 && rect[3] - rect[1] > 2.0 {
                                    let rect = self
                                        .tabs
                                        .get(self.active)
                                        .map_or(rect, |t| t.view.unrotate_rect(page, rect));
                                    self.requests.push(ReaderRequest::Snapshot { page, rect });
                                    self.status = self.t("pdfHome.copied");
                                }
                            }
                        }
                        Tool4::AreaHighlight => self.area_highlight((x0, y0), (*x, *y)),
                        Tool4::ImageStamp(bytes) => {
                            self.place_image_stamp((x0, y0), (*x, *y), &bytes);
                            self.tabs4.tool = None;
                        }
                    }
                    return Some(true);
                }
                Input::Key(crate::Key::Escape, _) => {
                    self.tabs4.tool = None;
                    self.tabs4.drag = None;
                    self.on.remove("comment.eraser");
                    self.on.remove("comment.areaHighlight");
                    return Some(true);
                }
                _ => {}
            }
        }
        // Filling form fields with the Hand tool.
        if let Input::Press { x, y, count, .. } = input {
            if *count == 1
                && self.tabs4.tool.is_none()
                && self.tool == super::Tool::Hand
                && inside(d, *x, *y)
            {
                if let Some(f) = self.field_at(*x, *y) {
                    self.fill_field(f, m);
                    return Some(true);
                }
            }
        }
        None
    }

    /// The form field under a window point.
    fn field_at(&self, x: f32, y: f32) -> Option<FormField> {
        let t = self.tabs.get(self.active)?;
        let (page, px, py) = t.view.window_to_doc(x, y)?;
        t.view.doc().form_fields().into_iter().find(|f| {
            f.page == page
                && !f.read_only
                && px >= f.rect[0]
                && px <= f.rect[2]
                && py >= f.rect[1]
                && py <= f.rect[3]
        })
    }

    pub(crate) fn fill_field(&mut self, f: FormField, m: &dyn TextMeasure) {
        match f.kind {
            FieldKind::Text => {
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    f.name.clone(),
                    vec![(self.t("pdfForm.value"), f.value.clone())],
                    vec![
                        (self.t("pdfReader.ok"), DialogButton::Ok),
                        (self.t("pdfPrint.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                );
                d.layout(m, self.size, self.scale);
                self.tabs4.field_dialog = Some((d, f));
            }
            FieldKind::CheckBox | FieldKind::Radio => {
                let on = f.kind == FieldKind::Radio || !f.checked;
                self.set_field(&f, None, Some(on));
            }
            FieldKind::Combo | FieldKind::List => {
                let items = f
                    .options
                    .iter()
                    .enumerate()
                    .map(|(k, o)| MenuItem {
                        label: o.clone(),
                        choice: Choice::Command(format!("field.choice{k}")),
                        checked: *o == f.value,
                        dim: false,
                    })
                    .collect();
                let r = self
                    .tabs
                    .get(self.active)
                    .and_then(|t| t.view.doc_rect_to_window(f.page, f.rect))
                    .unwrap_or([0.0; 4]);
                self.menu = Some((self.reader_menu(items, r), "field"));
                self.tabs4.choice_field = Some(f);
            }
            FieldKind::Button | FieldKind::Signature => {}
        }
    }

    /// A choice from a list field's menu.
    pub(crate) fn field_choice(&mut self, cmd: &str) -> bool {
        let Some(k) = cmd.strip_prefix("field.choice") else {
            return false;
        };
        if let Some(f) = self.tabs4.choice_field.take() {
            if let Some(v) = k.parse::<usize>().ok().and_then(|k| f.options.get(k)) {
                let v = v.clone();
                self.set_field(&f, Some(&v), None);
            }
        }
        true
    }

    pub(crate) fn set_field(&mut self, f: &FormField, text: Option<&str>, checked: Option<bool>) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let doc = t.view.doc_mut();
        // The new value and the calculated fields it changes are one undo step (round 11:
        // AFSimple_Calculate in `/CO` order).
        doc.begin_group("fill field");
        let r = match (text, checked) {
            (Some(v), _) => doc.set_field_text(f, v),
            (None, Some(on)) => doc.set_field_checked(f, on),
            _ => Ok(()),
        }
        .and_then(|()| doc.calculate_fields().map(|_| ()));
        doc.end_group();
        if let Err(e) = r {
            self.status = e;
        }
        self.document_changed();
    }

    /// Remove pencil strokes under a window point.
    fn erase_at(&mut self, x: f32, y: f32) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let Some((page, px, py)) = t.view.window_to_doc(x, y) else {
            return;
        };
        let pad = 3.0 / t.view.px_per_pt().max(0.01);
        let hit = t.view.doc().annotations().into_iter().find(|a| {
            a.page == page
                && a.kind == AnnotKind::Ink
                && px >= a.rect[0] - pad
                && px <= a.rect[2] + pad
                && py >= a.rect[1] - pad
                && py <= a.rect[3] + pad
        });
        if let Some(a) = hit {
            if let Err(e) = t.view.doc_mut().delete_annotation(&a) {
                self.status = e;
            }
            self.document_changed();
        }
    }

    /// A dragged box on one page in page points (left, top, right, bottom).
    fn page_box(&self, a: (f32, f32), b: (f32, f32)) -> Option<(usize, [f32; 4])> {
        let t = self.tabs.get(self.active)?;
        let (page, x0, y0) = t.view.window_to_doc(a.0, a.1)?;
        let (x1, y1) = match t.view.window_to_doc(b.0, b.1) {
            Some((p, x, y)) if p == page => (x, y),
            _ => (x0, y0),
        };
        Some((page, [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]))
    }

    fn area_highlight(&mut self, a: (f32, f32), b: (f32, f32)) {
        let Some((page, r)) = self.page_box(a, b) else {
            return;
        };
        if r[2] - r[0] < 2.0 || r[3] - r[1] < 2.0 {
            return;
        }
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let n = NewAnnot {
            kind: AnnotKind::Highlight,
            page,
            rects: vec![r],
            color: AREA_YELLOW,
            author,
            contents: String::new(),
        };
        if let Err(e) = t.view.doc_mut().add_annotation(&n) {
            self.status = e;
        }
        self.document_changed();
    }

    fn place_image_stamp(&mut self, a: (f32, f32), b: (f32, f32), bytes: &[u8]) {
        let Some((page, mut r)) = self.page_box(a, b) else {
            return;
        };
        // A click places the picture 2 inches wide at its own proportions.
        if r[2] - r[0] < 4.0 || r[3] - r[1] < 4.0 {
            let (w, h) = bo_pdfread::image_size(bytes).unwrap_or((4, 3));
            let k = 144.0 / (w.max(1) as f32);
            r = [r[0], r[1], r[0] + 144.0, r[1] + h as f32 * k];
        }
        let author = self.author.clone();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let Err(e) = t.view.doc_mut().add_image_stamp(page, r, bytes, &author) {
            self.status = e;
        }
        self.status = String::new();
        self.document_changed();
    }

    /// Comment threads in the list's order and filter.
    pub(crate) fn comments_shown(&self, mut list: Vec<CommentThread>) -> Vec<CommentThread> {
        if let Some(k) = self.tabs4.filter {
            list.retain(|t| t.comment.kind == k);
        }
        match self.tabs4.sort {
            1 => list.sort_by(|a, b| a.comment.author.cmp(&b.comment.author)),
            2 => list.sort_by(|a, b| b.comment.modified.cmp(&a.comment.modified)),
            3 => list.sort_by_key(|t| super::kind_label(t.comment.kind)),
            _ => {}
        }
        list
    }

    /// Sort and filter boxes at the top of the comment list; returns the height used.
    pub(crate) fn paint_comment_header(&mut self, p: &mut dyn Painter, r: [f32; 4]) -> f32 {
        let st = self.style(12.5, false);
        let h = self.px(24.0);
        let (x, y) = (r[0] + self.px(8.8), r[1] + self.px(4.0));
        let sort = [
            "pdfComment.sortPage",
            "pdfComment.sortAuthor",
            "pdfComment.sortDate",
            "pdfComment.sortType",
        ][usize::from(self.tabs4.sort.min(3))];
        let filter = self
            .tabs4
            .filter
            .and_then(|k| FILTERS.iter().find(|f| f.0 == k))
            .map_or("pdfComment.filterAll", |f| f.1);
        let w = (r[2] - self.px(26.4)) / 2.0;
        let boxes = [[x, y, w, h], [x + w + self.px(8.8), y, w, h]];
        for (b, label) in boxes.iter().zip([
            format!("{} ▾", self.t(sort)),
            format!("{} ▾", self.t(filter)),
        ]) {
            p.fill_rect(b[0], b[1], b[2], b[3], self.look.box_edge);
            p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, super::WHITE);
            p.push_clip(b[0], b[1], b[2], b[3]);
            p.text(
                &label,
                b[0] + self.px(4.8),
                b[1] + ((b[3] - st.size * 1.33) / 2.0).round(),
                st,
                super::TEXT,
            );
            p.pop_clip();
        }
        self.tabs4.header = boxes;
        h + self.px(10.0)
    }

    /// Field shading, the rubber band and the field text dialog.
    pub(crate) fn paint_tabs4(&self, p: &mut dyn Painter) {
        let d = self.doc_rect;
        if let Some(t) = self.tabs.get(self.active) {
            // BlueReader draws the highlight under the fields with the page (round 11).
            if self.tabs4.highlight_fields && !self.tint11_on() {
                p.push_clip(d[0], d[1], d[2], d[3]);
                let shown = t.view.visible_pages();
                for f in t.view.doc().form_fields() {
                    if !shown.contains(&f.page) || f.kind == FieldKind::Button {
                        continue;
                    }
                    if let Some(r) = t.view.doc_rect_to_window(f.page, f.rect) {
                        p.fill_rect(r[0], r[1], r[2], r[3], FIELD_HIGHLIGHT);
                    }
                }
                p.pop_clip();
            }
        }
        if let (Some((x0, y0, x1, y1)), Some(tool)) = (self.tabs4.drag, &self.tabs4.tool) {
            if *tool != Tool4::Eraser {
                let (l, t, r, b) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
                if *tool == Tool4::AreaHighlight {
                    p.fill_rect(l, t, r - l, b - t, Rgba(255, 230, 0, 90));
                }
                for e in [
                    [l, t, r - l, 1.0],
                    [l, b - 1.0, r - l, 1.0],
                    [l, t, 1.0, b - t],
                    [r - 1.0, t, 1.0, b - t],
                ] {
                    p.fill_rect(e[0], e[1], e[2], e[3], BAND);
                }
            }
        }
    }

    /// The field text dialog, over everything else.
    pub(crate) fn paint_tabs4_dialog(&self, p: &mut dyn Painter) {
        if let Some((d, _)) = &self.tabs4.field_dialog {
            d.paint(p, &self.theme, self.size, true);
        }
        if let Some(d) = &self.tabs4.search_dialog {
            d.paint(p, &self.theme, self.size, true);
        }
    }
}
