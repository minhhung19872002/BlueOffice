//! BlueEditor commands beyond page organisation and text editing: forms (fill, highlight,
//! reset, flatten), comments (select, edit, delete), redaction (mark, apply), digital
//! signatures (PAdES through `bo-sign`), and conversion to and from Office formats, images and
//! text.
#![allow(clippy::single_match_else)]
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_window::{inside, EditTool, EditorWindow};
use crate::popup::{Choice, MenuItem};
use crate::reader_window::ReaderRequest;
use crate::{Input, Painter, TextMeasure};
use bo_pdfread::{Annot, FieldKind, FormField, PdfDocument, TextLayer};
use bo_render::{PathSeg, Rgba};

/// What an editor dialog asks (beyond the window's own).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Ask2 {
    FieldText(FormField),
    Comment(Annot),
    Sign,
    /// A message with only OK.
    Info,
}

/// A redaction mark being dragged: page, start and current point (page points).
pub(crate) type Marking = (usize, (f32, f32), (f32, f32));

/// Editor state for forms, comments, redaction and signing.
#[derive(Debug, Default)]
pub(crate) struct Extras {
    pub highlight_fields: bool,
    pub selected_comment: Option<Annot>,
    /// Redaction marks: page, box in page points.
    pub marks: Vec<(usize, [f32; 4])>,
    /// Mark being dragged: page, start point, current point.
    pub marking: Option<Marking>,
    /// Certificate file picked for signing.
    pub certificate: Option<Vec<u8>>,
    /// Fields of the active document, valid for an edit count.
    fields: Option<(u32, Vec<FormField>)>,
}

const OFFICE_FILTERS: [(&str, &str); 1] = [(
    "Documents (*.docx; *.doc; *.odt; *.xlsx; *.xls; *.ods; *.csv; *.pptx; *.ppt; *.odp; *.txt)",
    "*.docx;*.doc;*.odt;*.rtf;*.txt;*.xlsx;*.xls;*.ods;*.csv;*.pptx;*.ppt;*.odp",
)];

/// A one-page blank A4 PDF.
pub(crate) fn blank_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.28 841.89] /Resources << >> /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Text of every page, one line per text line, pages separated by a form feed.
pub(crate) fn document_text(doc: &mut PdfDocument) -> String {
    let mut out = String::new();
    for i in 0..doc.page_count() {
        if let Ok(p) = doc.render(i, bo_pdfread::RenderOptions::default()) {
            if i > 0 {
                out.push('\u{c}');
            }
            out.push_str(&TextLayer::new(p.chars).text());
            out.push('\n');
        }
    }
    out
}

/// A workbook with one sheet per page: each text line a row, cells split at wide gaps.
pub(crate) fn to_xlsx(doc: &mut PdfDocument) -> Result<Vec<u8>, String> {
    let mut sheet = bo_engine::SheetEngine::with_bundled_fonts();
    sheet
        .new_workbook("Converted.xlsx")
        .map_err(|e| e.to_string())?;
    let mut cells = Vec::new();
    let mut row = 0_u32;
    for i in 0..doc.page_count() {
        let objs = doc.page_objects(i)?;
        let mut lines = bo_pdfread::text_lines(&objs);
        lines.sort_by(|a, b| {
            (a.bbox[1] / 4.0)
                .round()
                .total_cmp(&(b.bbox[1] / 4.0).round())
                .then(a.bbox[0].total_cmp(&b.bbox[0]))
        });
        // Lines on one baseline become the cells of one row, left to right.
        let mut last_y: Option<f32> = None;
        let mut col = 0_u32;
        for l in lines {
            let y = l.bbox[3];
            if last_y.is_none_or(|ly| (ly - y).abs() > l.font_size.max(4.0) * 0.5) {
                if last_y.is_some() {
                    row += 1;
                }
                col = 0;
                last_y = Some(y);
            }
            cells.push((row, col, l.text.trim().to_owned()));
            col += 1;
        }
        row += 2;
    }
    sheet.set_inputs(0, &cells).map_err(|e| e.to_string())?;
    sheet.save("xlsx", 0).map_err(|e| e.to_string())
}

/// A deck with one picture slide per page image (PNG), sized like the first page.
pub(crate) fn to_pptx(pngs: Vec<Vec<u8>>, size: (f32, f32)) -> Result<Vec<u8>, String> {
    use bo_pptx::build::{DeckSpec, Picture, ShapeSpec, SlideSpec};
    let emu = |pt: f32| (pt * 12_700.0).round() as i64;
    let (w, h) = (emu(size.0), emu(size.1));
    let slides = pngs
        .into_iter()
        .enumerate()
        .map(|(i, data)| SlideSpec {
            shapes: vec![ShapeSpec {
                name: format!("Page {}", i + 1),
                cx: w,
                cy: h,
                geometry: "rect".into(),
                picture: Some(Picture {
                    data,
                    ext: "png".into(),
                }),
                ..ShapeSpec::default()
            }],
            ..SlideSpec::default()
        })
        .collect();
    let deck = DeckSpec {
        width: w,
        height: h,
        slides,
        title: None,
    };
    bo_pptx::build::build_pptx(&deck).map_err(|e| e.to_string())
}

/// PDF of an Office document (BlueDoc, BlueSheet, BlueSlide formats).
pub(crate) fn office_to_pdf(name: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let lower = name.to_ascii_lowercase();
    let sheet = [".xlsx", ".xlsm", ".xls", ".xlsb", ".ods", ".csv"]
        .iter()
        .any(|e| lower.ends_with(e));
    if sheet {
        let mut s = bo_engine::SheetEngine::with_bundled_fonts();
        s.open(bytes, name).map_err(|e| e.to_string())?;
        s.export_pdf().map_err(|e| e.to_string())
    } else {
        let mut e = bo_engine::Engine::with_bundled_fonts();
        e.open(bytes, name).map_err(|e| e.to_string())?;
        e.export_pdf().map_err(|e| e.to_string())
    }
}

impl EditorWindow {
    /// Commands of this module; false when `cmd` is not one of them.
    pub(crate) fn action(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        match cmd {
            "form.fill" => {
                self.set_tool(None);
                self.x.highlight_fields = true;
                let msg = if self.fields().is_empty() {
                    self.t("pdfEditor.noFields")
                } else {
                    String::new()
                };
                self.set_status(&msg);
            }
            "form.highlight" => self.x.highlight_fields = !self.x.highlight_fields,
            "form.reset" => {
                let r = self.doc_edit(PdfDocument::reset_form);
                self.edited(r);
            }
            "form.flatten" => {
                let r = self.doc_edit(PdfDocument::flatten_form);
                match r {
                    Ok(0) => self.set_status(&self.t("pdfEditor.noFields")),
                    Ok(n) => {
                        self.edited(Ok(()));
                        let s = self.fmt("pdfEditor.flattened", n);
                        self.set_status(&s);
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            "prot.applyRedact" => self.apply_redactions(),
            "prot.sign" => self.push(ReaderRequest::PickFile {
                purpose: "certificate".into(),
                filters: vec![(self.t("pdfEditor.certificate"), "*.p12;*.pfx".into())],
            }),
            "conv.create" | "file.create" => self.push(ReaderRequest::PickFile {
                purpose: "create".into(),
                filters: OFFICE_FILTERS
                    .iter()
                    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                    .collect(),
            }),
            "conv.blank" => {
                let name = format!("{}.pdf", self.t("pdfEditor.untitled"));
                self.push(ReaderRequest::OpenDocument {
                    name,
                    bytes: blank_pdf(),
                });
            }
            "conv.word" => {
                let name = format!("{}.docx", self.doc_stem());
                let r = self.with_doc(|d| {
                    let bytes = d.save()?;
                    let mut e = bo_engine::Engine::with_bundled_fonts();
                    e.open(&bytes, "in.pdf").map_err(|e| e.to_string())?;
                    e.save_docx().map_err(|e| e.to_string())
                });
                self.save_result(name, r);
            }
            "conv.excel" => {
                let name = format!("{}.xlsx", self.doc_stem());
                let r = self.with_doc(to_xlsx);
                self.save_result(name, r);
            }
            "conv.text" => {
                let name = format!("{}.txt", self.doc_stem());
                let r = self.with_doc(|d| Ok(document_text(d).into_bytes()));
                self.save_result(name, r);
            }
            "conv.image" => self.push(ReaderRequest::RenderPages {
                purpose: "images".into(),
                scale: 150.0 / 72.0,
            }),
            "conv.slides" => self.push(ReaderRequest::RenderPages {
                purpose: "slides".into(),
                scale: 2.0,
            }),
            "comment.edit" => {
                if let Some(a) = self.x.selected_comment.clone() {
                    self.ask_comment(a, m);
                }
            }
            _ => return false,
        }
        true
    }

    fn fmt(&self, key: &str, n: usize) -> String {
        self.t(key).replace("{count}", &n.to_string())
    }

    fn save_result(&mut self, name: String, r: Result<Vec<u8>, String>) {
        match r {
            Ok(bytes) => self.push(ReaderRequest::SaveBytes { name, bytes }),
            Err(e) => self.set_status(&e),
        }
    }

    /// Fields of the active document (read again after edits).
    pub(crate) fn fields(&mut self) -> Vec<FormField> {
        let Some(v) = self.view() else {
            return Vec::new();
        };
        let n = v.doc().edit_count();
        if let Some((e, f)) = &self.x.fields {
            if *e == n {
                return f.clone();
            }
        }
        let f = v.doc().form_fields();
        self.x.fields = Some((n, f.clone()));
        f
    }

    fn edited<T>(&mut self, r: Result<T, String>) {
        match r {
            Ok(_) => {
                self.x.fields = None;
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// A press with the hand or select tool: fill a form field or select a comment.
    /// True when it was used.
    pub(crate) fn press_field_or_comment(
        &mut self,
        x: f32,
        y: f32,
        count: u8,
        m: &dyn TextMeasure,
    ) -> bool {
        let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) else {
            return false;
        };
        let field = self
            .fields()
            .into_iter()
            .find(|f| f.page == page && in_box(f.rect, px, py));
        if let Some(f) = field {
            self.x.selected_comment = None;
            self.fill_field(f, m);
            return true;
        }
        let comment = self.view().and_then(|v| {
            v.doc()
                .annotations()
                .into_iter()
                .filter(|a| a.page == page && in_box(a.rect, px, py))
                .min_by(|a, b| area(a.rect).total_cmp(&area(b.rect)))
        });
        match comment {
            Some(a) => {
                if count >= 2 {
                    self.ask_comment(a.clone(), m);
                }
                self.x.selected_comment = Some(a);
                true
            }
            None => {
                let had = self.x.selected_comment.take().is_some();
                let _ = had;
                false
            }
        }
    }

    fn fill_field(&mut self, f: FormField, m: &dyn TextMeasure) {
        match f.kind {
            FieldKind::CheckBox => {
                let on = !f.checked;
                let r = self.doc_edit(|d| d.set_field_checked(&f, on));
                self.edited(r);
            }
            FieldKind::Radio => {
                let r = self.doc_edit(|d| d.set_field_checked(&f, true));
                self.edited(r);
            }
            FieldKind::Combo | FieldKind::List => {
                let Some(r) = self
                    .view()
                    .and_then(|v| v.doc_rect_to_window(f.page, f.rect))
                else {
                    return;
                };
                let items = f
                    .options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| MenuItem {
                        label: o.clone(),
                        choice: Choice::Command(format!("field.choose.{i}")),
                        checked: *o == f.value,
                        dim: false,
                    })
                    .collect();
                self.pending_field = Some(f);
                self.menu_items(items, r);
            }
            FieldKind::Text => {
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    f.name.clone(),
                    vec![(self.t("pdfEditor.fieldValue"), f.value.clone())],
                    vec![
                        (self.t("pdfReader.ok"), DialogButton::Ok),
                        (self.t("pdfReader.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                );
                d.layout(m, self.window_size(), self.ui_scale());
                self.dialog2 = Some((d, Ask2::FieldText(f)));
            }
            FieldKind::Button | FieldKind::Signature => {}
        }
    }

    /// A choice from a field's option menu (`field.choose.<i>`).
    pub(crate) fn choose_option(&mut self, i: usize) {
        let Some(f) = self.pending_field.take() else {
            return;
        };
        let Some(v) = f.options.get(i).cloned() else {
            return;
        };
        let r = self.doc_edit(|d| d.set_field_text(&f, &v));
        self.edited(r);
    }

    fn ask_comment(&mut self, a: Annot, m: &dyn TextMeasure) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t("pdfEditor.editComment"),
            vec![(self.t("pdfReader.commentText"), a.contents.clone())],
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(m, self.window_size(), self.ui_scale());
        self.dialog2 = Some((d, Ask2::Comment(a)));
    }

    /// Delete the selected comment; false when none is selected.
    pub(crate) fn delete_comment(&mut self) -> bool {
        let Some(a) = self.x.selected_comment.take() else {
            return false;
        };
        let r = self.doc_edit(|d| d.delete_annotation(&a));
        self.edited(r);
        true
    }

    /// The answer of an editor dialog.
    pub(crate) fn answer2(&mut self, what: Ask2, values: &[String]) {
        let first = values.first().cloned().unwrap_or_default();
        match what {
            Ask2::FieldText(f) => {
                let r = self.doc_edit(|d| d.set_field_text(&f, &first));
                self.edited(r);
            }
            Ask2::Comment(a) => {
                let r = self.doc_edit(|d| d.set_annotation_contents(&a, &first));
                self.edited(r);
                self.x.selected_comment = None;
            }
            Ask2::Sign => {
                let Some(cert) = self.x.certificate.take() else {
                    return;
                };
                self.sign_answer(&cert, values);
            }
            Ask2::Info => {}
        }
    }

    /// A certificate file was picked: ask its password and the signature's reason.
    pub(crate) fn certificate_picked(&mut self, bytes: &[u8]) {
        self.x.certificate = Some(bytes.to_vec());
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t("pdfEditor.signCertify"),
            vec![
                (self.t("pdfEditor.certPassword"), String::new()),
                (self.t("pdfEditor.signReason"), String::new()),
                (self.t("pdfEditor.signLocation"), String::new()),
                (self.t("pdfEditor.tsaUrl"), String::new()),
                (self.t("pdfEditor.signImage"), String::new()),
            ],
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfReader.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(
            &crate::editor_window::Estimate,
            self.window_size(),
            self.ui_scale(),
        );
        self.dialog2 = Some((d, Ask2::Sign));
    }

    /// An Office file was picked to make a PDF of: open the result in a new tab.
    pub(crate) fn create_picked(&mut self, path: &std::path::Path, bytes: &[u8]) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match office_to_pdf(&name, bytes) {
            Ok(pdf) => {
                let stem = path
                    .file_stem()
                    .map_or_else(|| "document".into(), |s| s.to_string_lossy().into_owned());
                self.push(ReaderRequest::OpenDocument {
                    name: format!("{stem}.pdf"),
                    bytes: pdf,
                });
            }
            Err(e) => self.set_status(&format!("{name}: {e}")),
        }
    }

    /// Slides from rendered page images.
    pub(crate) fn slides_rendered(&mut self, pngs: Vec<Vec<u8>>) {
        let size = self
            .view()
            .and_then(|v| v.doc().page_info(0))
            .map_or((612.0, 792.0), |p| (p.width, p.height));
        let name = format!("{}.pptx", self.doc_stem());
        let r = to_pptx(pngs, size);
        self.save_result(name, r);
    }

    // ---- Redaction ---------------------------------------------------------------------

    /// Pointer input of the redaction tool over the document.
    pub(crate) fn redact_input(&mut self, input: &Input) -> bool {
        let area = self.doc_area();
        match *input {
            Input::Press { x, y, .. } if inside(area, x, y) => {
                if let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) {
                    self.x.marking = Some((page, (px, py), (px, py)));
                }
                true
            }
            Input::Drag { x, y } => {
                let Some((page, a, _)) = self.x.marking else {
                    return false;
                };
                let p = self.view().and_then(|v| {
                    let pr = v.page_rect(page)?;
                    let s = v.scale();
                    let (qx, qy) = ((x - pr[0]) / s, (y - pr[1]) / s);
                    let r = v.unrotate_rect(page, [qx, qy, qx, qy]);
                    Some((r[0], r[1]))
                });
                if let Some(p) = p {
                    self.x.marking = Some((page, a, p));
                }
                true
            }
            Input::Release { .. } => {
                if let Some((page, a, b)) = self.x.marking.take() {
                    let r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
                    if r[2] - r[0] > 2.0 && r[3] - r[1] > 2.0 {
                        self.x.marks.push((page, r));
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn apply_redactions(&mut self) {
        if self.x.marks.is_empty() {
            self.set_status(&self.t("pdfEditor.noRedactions"));
            return;
        }
        let marks = std::mem::take(&mut self.x.marks);
        let mut pages: Vec<usize> = marks.iter().map(|m| m.0).collect();
        pages.sort_unstable();
        pages.dedup();
        let mut total = 0;
        let mut err = None;
        if let Some(v) = self.view_mut() {
            v.doc_mut().begin_group("redact");
            for p in &pages {
                let rects: Vec<[f32; 4]> =
                    marks.iter().filter(|m| m.0 == *p).map(|m| m.1).collect();
                match v.doc_mut().apply_redactions(*p, &rects) {
                    Ok(n) => total += n,
                    Err(e) => err = Some(e),
                }
            }
            v.doc_mut().end_group();
        }
        self.after_doc_edit();
        match err {
            Some(e) => self.set_status(&e),
            None => {
                let s = self.fmt("pdfEditor.redactionsApplied", total);
                self.set_status(&s);
            }
        }
        if self.edit_tool() == Some(EditTool::Redact) {
            self.set_tool(None);
        }
    }

    /// Paint field highlights, the selected comment and redaction marks.
    pub(crate) fn paint_extras(&mut self, p: &mut dyn Painter) {
        if !self.showing_doc() || self.organize_on() {
            return;
        }
        let accent = self.accent();
        let fields = if self.x.highlight_fields {
            self.fields()
        } else {
            Vec::new()
        };
        let Some(v) = self.view() else { return };
        let frame = v.frame();
        p.push_clip(frame[0], frame[1], frame[2], frame[3]);
        let visible = v.visible_pages();
        for f in fields.iter().filter(|f| visible.contains(&f.page)) {
            if let Some(r) = v.doc_rect_to_window(f.page, f.rect) {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(204, 215, 255, 140));
            }
        }
        let outline = |p: &mut dyn Painter, r: [f32; 4], c: Rgba| {
            let (x0, y0) = (r[0].round() + 0.5, r[1].round() + 0.5);
            let (x1, y1) = ((r[0] + r[2]).round() - 0.5, (r[1] + r[3]).round() - 0.5);
            p.path(
                &[
                    PathSeg::M(x0, y0),
                    PathSeg::L(x1, y0),
                    PathSeg::L(x1, y1),
                    PathSeg::L(x0, y1),
                    PathSeg::Z,
                ],
                None,
                Some((c, 1.0)),
            );
        };
        if let Some(a) = &self.x.selected_comment {
            if let Some(r) = v.doc_rect_to_window(a.page, a.rect) {
                outline(p, [r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0], accent);
            }
        }
        let red = Rgba(220, 30, 30, 255);
        let mut marks = self.x.marks.clone();
        if let Some((page, a, b)) = self.x.marking {
            marks.push((
                page,
                [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)],
            ));
        }
        for (page, m) in marks {
            if let Some(r) = v.doc_rect_to_window(page, m) {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(220, 30, 30, 40));
                outline(p, r, red);
            }
        }
        p.pop_clip();
    }
}

fn in_box(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3]
}

fn area(r: [f32; 4]) -> f32 {
    (r[2] - r[0]) * (r[3] - r[1])
}
