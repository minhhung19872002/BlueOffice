//! BlueEditor's form design (Form tab: text field, check box, radio button, combo box, list
//! box, push button, dragged on the page, then named in a properties dialog) and page design
//! (Organize tab: header & footer, watermark, background, Bates numbering), through
//! `bo_pdfread::{NewField, PageMarks}`.
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::editor_actions::Ask2;
use crate::editor_window::{inside, EditorWindow};
use crate::reader_window::{Kind, Tool};
use crate::{Input, Painter};
use bo_pdfread::{NewField, NewFieldKind, PageMarks};
use bo_render::{PathSeg, Rgba};

/// Form design state: the field tool and the box being dragged.
#[derive(Debug, Clone, Default)]
pub(crate) struct DesignState {
    pub tool: Option<NewFieldKind>,
    pub drag: Option<crate::editor_actions::Marking>,
}

/// Ribbon items of the Form tab (as the reference's; features we do not have are disabled).
pub(crate) fn form_tab_items() -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Sep, Small};
    vec![
        ("sep1", Sep),
        (
            "form.recognize",
            Big("formFields", "pdfEditor.recognizeFields", false),
        ),
        (
            "form.assistant",
            Big("properties", "pdfEditor.designAssistant", false),
        ),
        ("sep2", Sep),
        ("fd.button", Small("fieldButton")),
        ("fd.check", Small("fieldCheck")),
        ("fd.radio", Small("fieldRadio")),
        ("fd.date", Small("fieldDate")),
        ("fd.text", Small("fieldText")),
        ("fd.combo", Small("fieldCombo")),
        ("fd.list", Small("fieldList")),
        ("fd.barcode", Small("fieldBarcode")),
        ("fd.image", Small("fieldImage")),
        ("fd.signature", Small("fieldSignature")),
        ("sep3", Sep),
        (
            "form.pageTemplates",
            Big("template", "pdfEditor.pageTemplates", false),
        ),
        ("form.xfa", Big("formPdf", "pdfEditor.editXfa", false)),
        (
            "form.calcOrder",
            Big("calcOrder", "pdfEditor.calcOrder", false),
        ),
        (
            "form.tooltip",
            Big("tooltip", "pdfEditor.addTooltip", false),
        ),
        ("sep4", Sep),
        ("form.reset", Big("resetForm", "pdfEditor.resetForm", false)),
        (
            "form.toSheetMenu",
            Big("formToSheet", "pdfEditor.formToSheet", true),
        ),
        (
            "form.importData",
            Big("import", "pdfEditor.importData", false),
        ),
        (
            "form.exportMenu",
            Big("export", "pdfEditor.exportData", true),
        ),
        ("sep5", Sep),
        ("form.actions", Big("javascript", "pdfEditor.actions", true)),
        ("sep6", Sep),
        (
            "form.settings",
            Big("viewSetting", "pdfEditor.toolSettings", true),
        ),
        ("sep7", Sep),
    ]
}

/// Form tab commands with no feature yet.
pub(crate) const FORM_TAB_LATER: [&str; 1] = ["form.assistant"];

fn field_kind(cmd: &str) -> Option<NewFieldKind> {
    Some(match cmd {
        "fd.button" => NewFieldKind::Button,
        "fd.check" => NewFieldKind::Check,
        "fd.radio" => NewFieldKind::Radio,
        "fd.text" | "fd.date" => NewFieldKind::Text,
        "fd.combo" => NewFieldKind::Combo,
        "fd.list" => NewFieldKind::List,
        "fd.signature" => NewFieldKind::Signature,
        "fd.image" => NewFieldKind::Image,
        "fd.barcode" => NewFieldKind::Barcode,
        _ => return None,
    })
}

fn kind_prefix(k: NewFieldKind) -> &'static str {
    match k {
        NewFieldKind::Text => "Text",
        NewFieldKind::Check => "Check Box",
        NewFieldKind::Radio => "Radio Button",
        NewFieldKind::Combo => "Combo Box",
        NewFieldKind::List => "List Box",
        NewFieldKind::Button => "Button",
        NewFieldKind::Signature => "Signature",
        NewFieldKind::Image => "Image",
        NewFieldKind::Barcode => "Barcode",
    }
}

impl EditorWindow {
    /// Form and page design commands; false when `cmd` is not one.
    pub(crate) fn design_command(&mut self, cmd: &str) -> bool {
        if let Some(k) = field_kind(cmd) {
            self.set_tool(None);
            self.comment.tool = None;
            self.r.set_tool(Tool::Hand);
            self.design.tool = if self.design.tool == Some(k) {
                None
            } else {
                Some(k)
            };
            return true;
        }
        let ask = |w: &mut Self, what: Ask2, title: &str, fields: Vec<(String, String)>| {
            let mut d = Dialog::new(
                DialogKind::Comment,
                w.t(title),
                fields,
                vec![
                    (w.t("pdfReader.ok"), DialogButton::Ok),
                    (w.t("pdfReader.cancel"), DialogButton::Cancel),
                ],
                Vec::new(),
            );
            d.layout(
                &crate::editor_window::Estimate,
                w.r.window_size(),
                w.r.ui_scale(),
            );
            w.dialog2 = Some((d, what));
        };
        match cmd {
            "org.headerFooter" => {
                let f = |w: &Self, k: &str, v: &str| (w.t(k), v.to_owned());
                let fields = vec![
                    f(self, "pdfEditor.headerLeft", ""),
                    f(self, "pdfEditor.headerCenter", ""),
                    f(self, "pdfEditor.headerRight", ""),
                    f(self, "pdfEditor.footerLeft", ""),
                    f(self, "pdfEditor.footerCenter", "<<page>> / <<pages>>"),
                    f(self, "pdfEditor.footerRight", ""),
                    f(self, "pdfEditor.markSize", "10"),
                ];
                ask(self, Ask2::HeaderFooter, "pdfEditor.headerFooter", fields);
            }
            "org.watermark" => {
                let fields = vec![
                    (
                        self.t("pdfEditor.watermarkText"),
                        self.t("pdfEditor.stamps.confidential").to_uppercase(),
                    ),
                    (self.t("pdfEditor.markSize"), "72".into()),
                    (self.t("pdfEditor.opacity"), "30".into()),
                    (self.t("pdfEditor.angle"), "45".into()),
                ];
                ask(self, Ask2::Watermark, "pdfEditor.watermark", fields);
            }
            "org.background" => {
                let items = [
                    ("org.bg.0", "pdfEditor.bg.cream"),
                    ("org.bg.1", "pdfEditor.bg.blue"),
                    ("org.bg.2", "pdfEditor.bg.green"),
                    ("org.bg.3", "pdfEditor.bg.gray"),
                ];
                self.menu(&items, "org.background");
            }
            "org.bates" => {
                let fields = vec![
                    (self.t("pdfEditor.batesPrefix"), "BO".into()),
                    (self.t("pdfEditor.batesStart"), "1".into()),
                    (self.t("pdfEditor.batesDigits"), "6".into()),
                    (self.t("pdfEditor.batesSuffix"), String::new()),
                ];
                ask(self, Ask2::Bates, "pdfEditor.bates", fields);
            }
            _ => {
                let Some(i) = cmd
                    .strip_prefix("org.bg.")
                    .and_then(|s| s.parse::<usize>().ok())
                else {
                    return false;
                };
                let colors = [
                    [0xFF, 0xF8, 0xE1],
                    [0xE8, 0xF0, 0xFE],
                    [0xE8, 0xF5, 0xE9],
                    [0xF2, 0xF2, 0xF2],
                ];
                let m = PageMarks {
                    background: colors.get(i).copied(),
                    ..PageMarks::default()
                };
                self.apply_marks(&m);
            }
        }
        true
    }

    fn apply_marks(&mut self, m: &PageMarks) {
        let pages: Vec<usize> = Vec::new();
        match self.doc_edit(|d| d.add_page_marks(&pages, m)) {
            Ok(()) => self.after_doc_edit(),
            Err(e) => self.set_status(&e),
        }
    }

    /// Answers of the page design and field dialogs.
    pub(crate) fn design_answer(&mut self, what: &Ask2, v: &[String]) {
        let get = |i: usize| v.get(i).cloned().unwrap_or_default();
        let num = |i: usize, d: f32| get(i).trim().parse::<f32>().unwrap_or(d);
        let date = {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            bo_sign::iso_utc(secs).get(..10).unwrap_or("").to_owned()
        };
        match what {
            Ask2::HeaderFooter => {
                let m = PageMarks {
                    header: [get(0), get(1), get(2)],
                    footer: [get(3), get(4), get(5)],
                    margin: 28.0,
                    size: num(6, 10.0).clamp(4.0, 72.0),
                    date,
                    ..PageMarks::default()
                };
                self.apply_marks(&m);
            }
            Ask2::Watermark => {
                let m = PageMarks {
                    watermark: get(0),
                    watermark_size: num(1, 72.0).clamp(6.0, 400.0),
                    watermark_color: [0xC0, 0x00, 0x00],
                    watermark_opacity: num(2, 30.0).clamp(5.0, 100.0) / 100.0,
                    watermark_angle: num(3, 45.0),
                    date,
                    ..PageMarks::default()
                };
                if !m.watermark.trim().is_empty() {
                    self.apply_marks(&m);
                }
            }
            Ask2::Bates => {
                let m = PageMarks {
                    bates: Some((
                        get(0),
                        num(1, 1.0).max(0.0) as u32,
                        num(2, 6.0).clamp(1.0, 12.0) as usize,
                        get(3),
                    )),
                    margin: 28.0,
                    size: 10.0,
                    date,
                    ..PageMarks::default()
                };
                self.apply_marks(&m);
            }
            Ask2::Field(kind, page, rect) => {
                let options: Vec<String> = get(2)
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                let f = NewField {
                    kind: *kind,
                    page: *page,
                    rect: *rect,
                    name: get(0),
                    tooltip: get(1),
                    options,
                    required: get(3) == "1",
                    read_only: get(4) == "1",
                };
                match self.doc_edit(|d| d.add_field(&f)) {
                    Ok(()) => {
                        if let Some(v) = self.r.view_mut() {
                            v.invalidate_page(*page);
                        }
                        self.after_doc_edit();
                    }
                    Err(e) => self.set_status(&e),
                }
            }
            _ => {}
        }
    }

    /// Pointer input of a field tool: drag the field's box.
    pub(crate) fn design_input(&mut self, input: &Input) -> bool {
        let Some(kind) = self.design.tool else {
            return false;
        };
        let area = self.r.doc_area();
        match *input {
            Input::Press { x, y, .. } if inside(area, x, y) => {
                if let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) {
                    self.design.drag = Some((page, (px, py), (px, py)));
                }
                true
            }
            Input::Drag { x, y } => {
                let Some((page, a, _)) = self.design.drag else {
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
                    self.design.drag = Some((page, a, p));
                }
                true
            }
            Input::Release { .. } => {
                let Some((page, a, b)) = self.design.drag.take() else {
                    return false;
                };
                let mut r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
                // A click places a field of the usual size.
                if r[2] - r[0] < 6.0 || r[3] - r[1] < 6.0 {
                    let (w, h) = match kind {
                        NewFieldKind::Check | NewFieldKind::Radio => (14.0, 14.0),
                        NewFieldKind::List | NewFieldKind::Image => (140.0, 60.0),
                        NewFieldKind::Signature => (160.0, 50.0),
                        NewFieldKind::Barcode => (140.0, 50.0),
                        _ => (140.0, 22.0),
                    };
                    r = [a.0, a.1, a.0 + w, a.1 + h];
                }
                let name = self
                    .view()
                    .map(|v| v.doc().free_field_name(kind_prefix(kind)))
                    .unwrap_or_default();
                let options = match kind {
                    NewFieldKind::Combo | NewFieldKind::List => "1, 2, 3".to_owned(),
                    NewFieldKind::Button => self.t("pdfReader.ok"),
                    _ => String::new(),
                };
                let mut d = Dialog::new(
                    DialogKind::Comment,
                    self.t("pdfEditor.fieldProperties"),
                    vec![
                        (self.t("pdfEditor.fieldName"), name),
                        (self.t("pdfEditor.fieldTooltip"), String::new()),
                        (self.t("pdfEditor.fieldOptions"), options),
                    ],
                    vec![
                        (self.t("pdfReader.ok"), DialogButton::Ok),
                        (self.t("pdfReader.cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                )
                .with_checks(vec![
                    (self.t("pdfEditor.fieldRequired"), false),
                    (self.t("pdfEditor.fieldReadOnly"), false),
                ]);
                d.layout(
                    &crate::editor_window::Estimate,
                    self.r.window_size(),
                    self.r.ui_scale(),
                );
                self.dialog2 = Some((d, Ask2::Field(kind, page, r)));
                true
            }
            Input::Key(crate::Key::Escape, _) => {
                self.design = DesignState::default();
                true
            }
            _ => false,
        }
    }

    /// The field box being dragged.
    pub(crate) fn paint_design(&mut self, p: &mut dyn Painter) {
        let Some((page, a, b)) = self.design.drag else {
            return;
        };
        let Some(v) = self.view() else { return };
        let r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        if let Some(w) = v.doc_rect_to_window(page, r) {
            let c = Rgba(0x1F, 0x4E, 0xA8, 255);
            p.fill_rect(w[0], w[1], w[2], w[3], Rgba(0x1F, 0x4E, 0xA8, 40));
            let (x0, y0, x1, y1) = (w[0], w[1], w[0] + w[2], w[1] + w[3]);
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
        }
    }
}
