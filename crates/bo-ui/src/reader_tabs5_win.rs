//! BlueReader round 5 (a child of `reader_window`): the commands the reference reader offers
//! that were drawn grey before — Fill & Sign (a contextual tab with text, check / cross marks,
//! dot, line and a signature picture placed on the page as comments), Sign & Certify (PAdES
//! through `bo-sign` with a .p12 / .pfx digital ID, optional time-stamp), Time Stamp Document
//! (a document time-stamp), Validate enabled once a signature on the page is selected, the
//! Comment tab's Summarize Comments / Import / Export column (summary PDF, FDF and XFDF) and
//! File (a file attached on the page); Reset Form greyed when the document has no form.
use super::{inside, ReaderRequest, ReaderWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::{Input, Painter, TextMeasure};
use bo_pdfread::{FieldKind, Markup, MarkupKind, SummaryLabels};
use bo_render::{PathSeg, Rgba};
use std::cell::Cell;

/// Index of the Fill & Sign contextual tab (after Help).
pub(crate) const FILL_TAB: usize = 6;

/// Paths of the outline items marked open (`/Count` > 0), to start expanded.
pub(crate) fn open_items(
    items: &[bo_pdfread::OutlineItem],
    path: &mut Vec<usize>,
    out: &mut std::collections::HashSet<Vec<usize>>,
) {
    for (i, it) in items.iter().enumerate() {
        path.push(i);
        if it.open && !it.children.is_empty() {
            out.insert(path.clone());
            open_items(&it.children, path, out);
        }
        path.pop();
    }
}

/// Icons of the text-markup grid (drawn larger).
pub(crate) fn markup_icon(name: &str) -> bool {
    matches!(
        name,
        "highlight" | "underline" | "strikeout" | "squiggly" | "replaceText" | "insertText"
    )
}

/// What a click on the page places in Fill & Sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fill {
    Text,
    Check,
    Cross,
    Dot,
    Line,
}

/// A digital ID waiting for its signature box.
pub(crate) struct Pending {
    signer: bo_sign::SoftwareSigner,
    reason: String,
    location: String,
    tsa: String,
    picture: Option<Vec<u8>>,
}

/// What an open dialog of the additions asks.
#[derive(Debug, Clone, PartialEq)]
enum Ask {
    /// The document opens a web address: allow it?
    Url(String),
    /// Text for Fill & Sign at a page point.
    FillText(usize, [f32; 2]),
    /// Password, reason, location, time-stamp server, picture for signing.
    Sign,
    /// Time-stamp server for Time Stamp Document.
    Stamp,
}

/// A document state: bytes address and length, edit count.
type DocKey = (usize, usize, u32);

/// State of the round 5 additions.
#[derive(Default)]
pub(crate) struct Tabs5 {
    /// The Fill & Sign tab is shown.
    pub(crate) fill_mode: bool,
    /// The Fill & Sign tool.
    pub(crate) fill: Option<Fill>,
    /// File tool: the next click on a page asks for the file to attach.
    pub(crate) attach: bool,
    /// Where the picked file goes.
    attach_at: Option<(usize, [f32; 2])>,
    /// A dialog and what it asks.
    dialog: Option<(Dialog, Ask)>,
    /// The digital ID file picked.
    cert: Option<Vec<u8>>,
    /// A signer waiting for its box.
    pending: Option<Pending>,
    /// Signature box being dragged: page, start, now (page points).
    sign_drag: Option<(usize, [f32; 2], [f32; 2])>,
    /// The signature field selected on the page (page, rect, name).
    pub(crate) sig_selected: Option<(usize, [f32; 4], String)>,
    /// File › Properties.
    pub(crate) props: Option<super::props5::Props>,
    /// The File page (backstage).
    pub(crate) backstage: Option<super::bs5::Backstage>,
    /// Bookmarks panel: search text, its box has the keyboard, text size step, tool boxes,
    /// the tool under the pointer, the search box.
    pub(crate) bm_search: String,
    pub(crate) bm_focus: bool,
    pub(crate) bm_font: f32,
    pub(crate) bm_tools: Vec<(&'static str, [f32; 4])>,
    pub(crate) bm_hover: Option<&'static str>,
    pub(crate) bm_search_box: [f32; 4],
    /// Whether the document has form fields / signatures, for the bytes and edit count it
    /// was worked out for.
    facts: Cell<Option<(DocKey, bool)>>,
}

impl ReaderWindow {
    /// Items of the Fill & Sign contextual tab, after Hand and Select.
    pub(crate) fn fill_items() -> Vec<(&'static str, super::Kind)> {
        use super::Kind::{Big, Sep};
        vec![
            ("sep1", Sep),
            ("fill.text", Big("addText", "pdfFill.text", false)),
            ("fill.check", Big("check", "pdfFill.check", false)),
            ("fill.cross", Big("cross", "pdfFill.cross", false)),
            ("fill.dot", Big("dot", "pdfFill.dot", false)),
            ("fill.line", Big("line", "pdfFill.line", false)),
            ("sep2", Sep),
            ("fill.sign", Big("fillSign", "pdfFill.signature", false)),
            ("sep3", Sep),
            ("fill.close", Big("close", "pdfFill.close", false)),
        ]
    }

    /// The Comment tab's right end like the reference's: Summarize Comments / Import / Export
    /// as a column of labelled rows, then a column of three small icons.
    pub(crate) fn comment_rows() -> Vec<(&'static str, super::Kind)> {
        use super::Kind::{Row, Sep};
        vec![
            ("sep4", Sep),
            (
                "comment.summarize",
                Row("summarize", "pdfComment.summarize", false),
            ),
            (
                "comment.import",
                Row("importComments", "pdfComment.import", false),
            ),
            (
                "menu.commentExport",
                Row("exportComments", "pdfComment.export", true),
            ),
            ("panel.comments", Row("commentList", "", false)),
            ("menu.commentOptions", Row("commentOptions", "", true)),
            ("comment.keep", Row("pin", "", false)),
        ]
    }

    /// Enabled state of the additions' controls (`None`: not one of them).
    pub(crate) fn tabs5_enabled(&self, id: &str) -> Option<bool> {
        match id {
            "prot.validate" => Some(self.tabs5.sig_selected.is_some()),
            "form.reset" => Some(self.has_form()),
            // The reference keeps it grey until a comment tool is chosen.
            "comment.keep" => Some(false),
            _ => None,
        }
    }

    /// Controls shown pressed.
    pub(crate) fn tabs5_on(&self, id: &str) -> bool {
        let fill = |f| self.tabs5.fill == Some(f);
        match id {
            "fill.text" => fill(Fill::Text),
            "fill.check" => fill(Fill::Check),
            "fill.cross" => fill(Fill::Cross),
            "fill.dot" => fill(Fill::Dot),
            "fill.line" => fill(Fill::Line),
            "comment.file" => self.tabs5.attach,
            "prot.fillSign" => self.tabs5.fill_mode,
            _ => false,
        }
    }

    /// The document has form fields (worked out once per document state).
    fn has_form(&self) -> bool {
        let Some(t) = self.tabs.get(self.active) else {
            return false;
        };
        let doc = t.view.doc();
        let bytes = doc.bytes();
        let key = (bytes.as_ptr() as usize, bytes.len(), doc.edit_count());
        if let Some((k, v)) = self.tabs5.facts.get() {
            if k == key {
                return v;
            }
        }
        let v = !doc.form_fields().is_empty();
        self.tabs5.facts.set(Some((key, v)));
        v
    }

    fn ask(&mut self, title: &str, fields: Vec<(String, String)>, what: Ask, m: &dyn TextMeasure) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            title.to_owned(),
            fields,
            vec![
                (self.t("pdfReader.ok"), DialogButton::Ok),
                (self.t("pdfPrint.cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        );
        d.layout(m, self.size, self.scale);
        self.tabs5.dialog = Some((d, what));
    }

    fn set_fill(&mut self, f: Fill) {
        self.tabs5.fill = if self.tabs5.fill == Some(f) {
            None
        } else {
            Some(f)
        };
        self.tabs5.attach = false;
        if self.tabs5.fill.is_some() {
            self.status = self.t("pdfFill.place");
        }
    }

    fn close_fill(&mut self, m: &dyn TextMeasure) {
        self.tabs5.fill_mode = false;
        self.tabs5.fill = None;
        if self.ribbon_tab == FILL_TAB {
            self.ribbon_tab = 0;
        }
        self.relayout(m);
    }

    /// Commands of the additions; `true` when `cmd` was one.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn command_tabs5(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        match cmd {
            "prot.fillSign" => {
                if self.tabs5.fill_mode && self.ribbon_tab == FILL_TAB {
                    self.close_fill(m);
                } else {
                    self.tabs5.fill_mode = true;
                    self.ribbon_tab = FILL_TAB;
                    self.relayout(m);
                }
            }
            "fill.close" => self.close_fill(m),
            "menu.clipboard" => {
                // The reference's Clipboard menu: Cut and Paste grey in a reader, Copy grey
                // without a selection.
                let selected = self.view().is_some_and(|v| v.selection().is_some());
                let items = [
                    ("edit.cut", "pdfHome.cut", false),
                    ("edit.copy", "pdfReader.copy", selected),
                    ("edit.paste", "pdfHome.paste", false),
                    ("edit.selectAll", "pdfHome.selectAll", true),
                ]
                .iter()
                .map(|(cmd, key, on)| MenuItem {
                    label: self.t(key),
                    choice: Choice::Command(if *on {
                        (*cmd).to_owned()
                    } else {
                        String::new()
                    }),
                    checked: false,
                    dim: !on,
                })
                .collect();
                let r = self.control_rect("menu.clipboard").unwrap_or([0.0; 4]);
                self.menu = Some((
                    Menu::open(items, r, self.size, self.scale),
                    "menu.clipboard",
                ));
            }
            "edit.selectAll" => {
                let ctrl = crate::Mods {
                    ctrl: true,
                    shift: false,
                    alt: false,
                };
                self.key(crate::Key::Letter('a'), ctrl, m);
            }
            "fill.text" => self.set_fill(Fill::Text),
            "fill.check" => self.set_fill(Fill::Check),
            "fill.cross" => self.set_fill(Fill::Cross),
            "fill.dot" => self.set_fill(Fill::Dot),
            "fill.line" => self.set_fill(Fill::Line),
            // A signature picture placed like a custom stamp.
            "fill.sign" => self.requests.push(ReaderRequest::PickFile {
                purpose: "stamp".into(),
                filters: vec![(self.t("pdfComment.images"), "*.png;*.jpg;*.jpeg".into())],
            }),
            "comment.file" => {
                self.tabs5.attach = !self.tabs5.attach;
                self.tabs5.fill = None;
                if self.tabs5.attach {
                    self.status = self.t("pdfComment.fileClick");
                }
            }
            "comment.summarize" => self.summarize(),
            "comment.import" => self.requests.push(ReaderRequest::PickFile {
                purpose: "comments".into(),
                filters: vec![
                    (self.t("pdfComment.commentFiles"), "*.fdf;*.xfdf".into()),
                    (self.t("pdfComment.allFiles"), "*.*".into()),
                ],
            }),
            "menu.commentExport" => {
                let items = [
                    ("comment.exportFdf", "pdfComment.exportFdf"),
                    ("comment.exportXfdf", "pdfComment.exportXfdf"),
                ];
                self.menu_here(&items, "menu.commentExport");
            }
            "menu.commentOptions" => {
                let items = [
                    ("draw.props", "pdfDraw.properties"),
                    ("panel.comments", "pdfComment.list"),
                ];
                self.menu_here(&items, "menu.commentOptions");
            }
            "comment.exportFdf" | "comment.exportXfdf" => self.export_comments(cmd),
            c if c.starts_with("bm.") => return self.bookmark_command(c),
            "file.properties" => self.open_properties(),
            "prot.sign" => {
                let items = [("sign.place", "pdfSign.place")];
                self.menu_here(&items, "prot.sign");
            }
            "sign.place" => self.requests.push(ReaderRequest::PickFile {
                purpose: "certificate".into(),
                filters: vec![(self.t("pdfSign.certificate"), "*.p12;*.pfx".into())],
            }),
            "prot.timestamp" => {
                let fields = vec![(self.t("pdfSign.server"), String::new())];
                let title = self.t("pdfProtect.timestamp");
                self.ask(&title, fields, Ask::Stamp, m);
            }
            _ => return false,
        }
        true
    }

    fn menu_here(&mut self, items: &[(&str, &str)], id: &'static str) {
        let items = items
            .iter()
            .map(|(cmd, key)| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: false,
                dim: false,
            })
            .collect();
        let r = self.control_rect(id).unwrap_or([0.0; 4]);
        self.menu = Some((Menu::open(items, r, self.size, self.scale), id));
    }

    fn stem(&self) -> String {
        self.tabs.get(self.active).map_or_else(
            || "document".into(),
            |t| {
                t.name
                    .strip_suffix(".pdf")
                    .or_else(|| t.name.strip_suffix(".PDF"))
                    .unwrap_or(&t.name)
                    .to_owned()
            },
        )
    }

    /// Summarize Comments: the summary opens as a new document.
    fn summarize(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let file = t.name.clone();
        let s = &self.strings;
        let labels = SummaryLabels {
            title: s.fmt("pdfComment.summaryTitle", &[("file", "{file}")]),
            page: s.fmt("pdfComment.summaryPage", &[("n", "{n}")]),
            number: s.get("pdfComment.summaryNumber").to_owned(),
            author: s.get("pdfComment.summaryAuthor").to_owned(),
            subject: s.get("pdfComment.summarySubject").to_owned(),
            date: s.get("pdfComment.summaryDate").to_owned(),
        };
        match t.view.doc().comment_summary(&file, &labels) {
            Ok(bytes) => {
                let name = format!("{}_summary.pdf", self.stem());
                self.requests
                    .push(ReaderRequest::OpenDocument { name, bytes });
            }
            Err(e) => self.status = e,
        }
    }

    fn export_comments(&mut self, cmd: &str) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let doc = t.view.doc();
        let (ext, bytes) = if cmd == "comment.exportFdf" {
            match doc.export_fdf(&t.name) {
                Ok(b) => ("fdf", b),
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        } else {
            ("xfdf", doc.export_xfdf(&t.name).into_bytes())
        };
        let name = format!("{}.{ext}", self.stem());
        self.requests.push(ReaderRequest::SaveBytes { name, bytes });
    }

    /// A file the host read for the additions (`comments`, `certificate`, `attach`).
    pub fn file_picked5(&mut self, purpose: &str, path: &std::path::Path, bytes: &[u8]) -> bool {
        match purpose {
            "comments" => {
                let Some(t) = self.tabs.get_mut(self.active) else {
                    return true;
                };
                match t.view.doc_mut().import_comments(bytes) {
                    Ok(n) => {
                        self.status = self
                            .strings
                            .fmt("pdfComment.imported", &[("n", &n.to_string())]);
                    }
                    Err(e) => self.status = e,
                }
                self.document_changed();
            }
            "certificate" => {
                self.tabs5.cert = Some(bytes.to_vec());
                let fields = vec![
                    (self.t("pdfSign.password"), String::new()),
                    (self.t("pdfSign.reason"), String::new()),
                    (self.t("pdfSign.location"), String::new()),
                    (self.t("pdfSign.tsa"), String::new()),
                    (self.t("pdfSign.image"), String::new()),
                ];
                let title = self.t("pdfProtect.sign");
                self.ask(&title, fields, Ask::Sign, &crate::editor_window::Estimate);
            }
            "attach" => {
                let Some((page, p)) = self.tabs5.attach_at.take() else {
                    return true;
                };
                let name = path
                    .file_name()
                    .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
                let m = Markup {
                    kind: MarkupKind::File(name.clone(), bytes.to_vec()),
                    page,
                    rect: [p[0], p[1], p[0] + 14.0, p[1] + 20.0],
                    points: Vec::new(),
                    color: [0x1f, 0x5f, 0xbf],
                    opacity: 1.0,
                    width: 1.0,
                    author: self.author.clone(),
                    contents: name,
                    font_size: 12.0,
                };
                self.add_markup5(&m);
            }
            _ => return false,
        }
        true
    }

    fn add_markup5(&mut self, m: &Markup) {
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        if let Err(e) = t.view.doc_mut().add_markup(m) {
            self.status = e;
        }
        self.document_changed();
    }

    /// Place the Fill & Sign mark at a page point.
    fn place_fill(&mut self, f: Fill, page: usize, x: f32, y: f32, m: &dyn TextMeasure) {
        let base = Markup {
            kind: MarkupKind::Pencil,
            page,
            rect: [x, y, x + 1.0, y + 1.0],
            points: Vec::new(),
            color: [0, 0, 0],
            opacity: 1.0,
            width: 1.5,
            author: self.author.clone(),
            contents: String::new(),
            font_size: 12.0,
        };
        // Marks are 12 points square, centred on the click like the reference's.
        let (l, t, s) = (x - 6.0, y - 6.0, 12.0);
        match f {
            Fill::Text => {
                let title = self.t("pdfFill.text");
                let fields = vec![(self.t("pdfForm.value"), String::new())];
                self.ask(&title, fields, Ask::FillText(page, [x, y]), m);
            }
            Fill::Check => self.add_markup5(&Markup {
                points: vec![
                    [l + 1.0, t + s * 0.55],
                    [l + s * 0.4, t + s - 1.5],
                    [l + s - 1.0, t + 1.5],
                ],
                width: 2.0,
                ..base
            }),
            Fill::Cross => {
                for pts in [
                    [[l + 1.5, t + 1.5], [l + s - 1.5, t + s - 1.5]],
                    [[l + s - 1.5, t + 1.5], [l + 1.5, t + s - 1.5]],
                ] {
                    self.add_markup5(&Markup {
                        kind: MarkupKind::Line,
                        points: pts.to_vec(),
                        width: 2.0,
                        ..base.clone()
                    });
                }
            }
            Fill::Dot => self.add_markup5(&Markup {
                kind: MarkupKind::Ellipse,
                rect: [x - 2.5, y - 2.5, x + 2.5, y + 2.5],
                width: 2.5,
                ..base
            }),
            Fill::Line => self.add_markup5(&Markup {
                kind: MarkupKind::Line,
                points: vec![[x - 15.0, y], [x + 15.0, y]],
                ..base
            }),
        }
    }

    /// A link to a web address: ask before opening it, like the reference's security warning.
    pub(crate) fn confirm_url(&mut self, url: String, m: &dyn TextMeasure) {
        let mut d = Dialog::new(
            DialogKind::Comment,
            self.t("pdfLink.title"),
            Vec::new(),
            vec![
                (self.t("pdfLink.allow"), DialogButton::Ok),
                (self.t("pdfLink.block"), DialogButton::Cancel),
            ],
            vec![self.t("pdfLink.ask"), url.clone()],
        );
        d.layout(m, self.size, self.scale);
        self.tabs5.dialog = Some((d, Ask::Url(url)));
    }

    fn answer(&mut self, what: Ask, values: &[String]) {
        let get = |i: usize| values.get(i).cloned().unwrap_or_default();
        match what {
            Ask::Url(u) => self.requests.push(ReaderRequest::OpenUrl(u)),
            Ask::FillText(page, p) => {
                let text = get(0);
                if text.trim().is_empty() {
                    return;
                }
                let m = Markup {
                    kind: MarkupKind::Typewriter,
                    page,
                    // The text's first line sits on the click like the reference's.
                    rect: [p[0], p[1] - 8.0, p[0] + 1.0, p[1] - 7.0],
                    points: Vec::new(),
                    color: [0, 0, 0],
                    opacity: 1.0,
                    width: 1.0,
                    author: self.author.clone(),
                    contents: text,
                    font_size: 12.0,
                };
                self.add_markup5(&m);
            }
            Ask::Sign => {
                let Some(cert) = self.tabs5.cert.take() else {
                    return;
                };
                match bo_sign::SoftwareSigner::from_pkcs12(&cert, &get(0)) {
                    Ok(signer) => {
                        let picture = Some(get(4))
                            .filter(|p| !p.trim().is_empty())
                            .and_then(|p| std::fs::read(p.trim().trim_matches('"')).ok());
                        self.tabs5.pending = Some(Pending {
                            signer,
                            reason: get(1),
                            location: get(2),
                            tsa: get(3).trim().to_owned(),
                            picture,
                        });
                        self.status = self.t("pdfSign.drag");
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
            Ask::Stamp => {
                let url = get(0).trim().to_owned();
                if url.is_empty() {
                    return;
                }
                if let Err(e) = self.time_stamp(&url) {
                    self.status = e;
                }
            }
        }
    }

    fn time_stamp(&mut self, url: &str) -> Result<(), String> {
        let t = self.tabs.get_mut(self.active).ok_or("no document")?;
        let bytes = t.view.doc_mut().save()?;
        let client = tsa_client(url);
        let out =
            bo_sign::pdf::add_document_timestamp(&bytes, &client).map_err(|e| e.to_string())?;
        let name = format!("{}-timestamped.pdf", self.stem());
        self.status = self.t("pdfSign.stamped");
        self.requests.push(ReaderRequest::SaveBytes {
            name: name.clone(),
            bytes: out.clone(),
        });
        self.requests
            .push(ReaderRequest::OpenDocument { name, bytes: out });
        Ok(())
    }

    /// Sign the active document with a visible signature in `rect` (page points) of `page`.
    fn sign_placed(&mut self, p: &Pending, page: usize, rect: [f32; 4]) -> Result<(), String> {
        use bo_sign::Signer as _;
        let name = bo_sign::trust::certificate_subject(p.signer.certificate_der())
            .map_or_else(|| "?".to_owned(), |s| bo_sign::trust::common_name(&s));
        let now = std::time::SystemTime::now();
        let secs = now
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let iso = bo_sign::iso_utc(secs);
        let date = format!(
            "{} {} UTC",
            iso.get(..10).unwrap_or("").replace('-', "."),
            iso.get(11..19).unwrap_or("")
        );
        let mut lines = vec![self.t("pdfSign.signedBy"), name.clone()];
        lines.push(format!("{}: {date}", self.t("pdfSign.date")));
        if !p.reason.is_empty() {
            lines.push(format!("{}: {}", self.t("pdfSign.reason"), p.reason));
        }
        let stem = self.stem();
        let t = self.tabs.get_mut(self.active).ok_or("no document")?;
        let doc = t.view.doc_mut();
        let bytes = doc.save()?;
        let user = doc
            .page_rect_to_user(page, rect)
            .ok_or_else(|| "page".to_owned())?;
        let look = bo_pdfread::signature_appearance(
            user[2] - user[0],
            user[3] - user[1],
            &name,
            &lines,
            p.picture.as_deref(),
        )?;
        let visible = bo_sign::pdf::VisibleSignature {
            page,
            rect: user,
            content: look.content,
            image: look.image,
        };
        let opts = bo_sign::pdf::SignOptions {
            name: Some(name),
            reason: (!p.reason.is_empty()).then(|| p.reason.clone()),
            location: (!p.location.is_empty()).then(|| p.location.clone()),
            time: now,
            ..bo_sign::pdf::SignOptions::default()
        };
        let tsa = (!p.tsa.is_empty()).then(|| tsa_client(&p.tsa));
        let signed = bo_sign::pdf::sign_pdf_visible(
            &bytes,
            &p.signer,
            &opts,
            tsa.as_ref()
                .map(|t| t as &dyn bo_sign::timestamp::TimestampClient),
            &visible,
        )
        .map_err(|e| e.to_string())?;
        let file = format!("{stem}-signed.pdf");
        self.status = self.t("pdfSign.signed");
        self.requests.push(ReaderRequest::SaveBytes {
            name: file.clone(),
            bytes: signed.clone(),
        });
        self.requests.push(ReaderRequest::OpenDocument {
            name: file,
            bytes: signed,
        });
        Ok(())
    }

    /// The signature field under a window point.
    fn signature_at(&self, x: f32, y: f32) -> Option<(usize, [f32; 4], String)> {
        let t = self.tabs.get(self.active)?;
        let (page, px, py) = t.view.window_to_doc(x, y)?;
        t.view
            .doc()
            .form_fields()
            .into_iter()
            .find(|f| {
                f.kind == FieldKind::Signature
                    && f.page == page
                    && px >= f.rect[0]
                    && px <= f.rect[2]
                    && py >= f.rect[1]
                    && py <= f.rect[3]
            })
            .map(|f| (f.page, f.rect, f.name))
    }

    /// Input of the additions first: `Some(redraw)` when handled.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn route_tabs5(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(r) = self.route_props(input) {
            return Some(r);
        }
        if self.tabs5.dialog.is_none() {
            if let Some(r) = self.route_backstage(input, m) {
                return Some(r);
            }
        }
        if self.tabs5.dialog.is_none() {
            if let Some(r) = self.route_bookmarks(input, m) {
                return Some(r);
            }
        }
        if let Some((d, _)) = self.tabs5.dialog.as_mut() {
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
                if let Some((d, what)) = self.tabs5.dialog.take() {
                    if b == DialogButton::Ok {
                        self.answer(what, &d.values());
                    }
                }
            }
            return Some(true);
        }
        let area = self.doc_rect;
        // A signature box being dragged.
        if self.tabs5.pending.is_some() {
            let point = |w: &Self, x: f32, y: f32| {
                w.tabs
                    .get(w.active)
                    .and_then(|t| t.view.window_to_doc(x, y))
            };
            match *input {
                Input::Press { x, y, .. } if inside(area, x, y) => {
                    if let Some((page, px, py)) = point(self, x, y) {
                        self.tabs5.sign_drag = Some((page, [px, py], [px, py]));
                    }
                    return Some(true);
                }
                Input::Drag { x, y } if self.tabs5.sign_drag.is_some() => {
                    if let (Some((page, _, _)), Some((p2, px, py))) =
                        (self.tabs5.sign_drag, point(self, x, y))
                    {
                        if p2 == page {
                            if let Some(g) = self.tabs5.sign_drag.as_mut() {
                                g.2 = [px, py];
                            }
                        }
                    }
                    return Some(true);
                }
                Input::Release { .. } if self.tabs5.sign_drag.is_some() => {
                    if let (Some((page, a, b)), Some(p)) =
                        (self.tabs5.sign_drag.take(), self.tabs5.pending.take())
                    {
                        let r = [
                            a[0].min(b[0]),
                            a[1].min(b[1]),
                            a[0].max(b[0]),
                            a[1].max(b[1]),
                        ];
                        // A click without a drag: a box of the usual size.
                        let r = if r[2] - r[0] < 8.0 || r[3] - r[1] < 8.0 {
                            [r[0], r[1], r[0] + 180.0, r[1] + 54.0]
                        } else {
                            r
                        };
                        if let Err(e) = self.sign_placed(&p, page, r) {
                            self.status = e;
                        }
                    }
                    return Some(true);
                }
                Input::Key(crate::Key::Escape, _) => {
                    self.tabs5.pending = None;
                    self.tabs5.sign_drag = None;
                    self.status.clear();
                    return Some(true);
                }
                _ => {}
            }
        }
        // Fill & Sign and File tools: a click on a page.
        if self.tabs5.fill.is_some() || self.tabs5.attach {
            match *input {
                Input::Press { x, y, .. } if inside(area, x, y) => {
                    let hit = self
                        .tabs
                        .get(self.active)
                        .and_then(|t| t.view.window_to_doc(x, y));
                    if let Some((page, px, py)) = hit {
                        if self.tabs5.attach {
                            self.tabs5.attach_at = Some((page, [px, py]));
                            self.tabs5.attach = false;
                            self.requests.push(ReaderRequest::PickFile {
                                purpose: "attach".into(),
                                filters: vec![(self.t("pdfComment.allFiles"), "*.*".into())],
                            });
                        } else if let Some(f) = self.tabs5.fill {
                            self.place_fill(f, page, px, py, m);
                        }
                    }
                    return Some(true);
                }
                Input::Release { .. } | Input::Drag { .. }
                    if inside(area, self.pointer.0, self.pointer.1) =>
                {
                    return Some(true);
                }
                Input::Key(crate::Key::Escape, _) => {
                    self.tabs5.fill = None;
                    self.tabs5.attach = false;
                    self.status.clear();
                    return Some(true);
                }
                _ => {}
            }
        }
        // Selecting a signature on the page (Validate follows the selection).
        if let Input::Press { x, y, .. } = *input {
            if inside(area, x, y) && self.tool == super::Tool::Hand {
                let sel = self.signature_at(x, y);
                let had = self.tabs5.sig_selected.is_some();
                self.tabs5.sig_selected = sel;
                if self.tabs5.sig_selected.is_some() {
                    return Some(true);
                }
                if had {
                    // Fall through to the usual press with the selection cleared.
                    return None;
                }
            }
        }
        None
    }

    /// The selected signature and the signature box being dragged.
    pub(crate) fn paint_tabs5(&self, p: &mut dyn Painter) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let d = self.doc_rect;
        let accent = self.theme.accent;
        p.push_clip(d[0], d[1], d[2], d[3]);
        if let Some((page, r, _)) = &self.tabs5.sig_selected {
            if let Some(w) = t.view.doc_rect_to_window(*page, *r) {
                frame(p, w, accent, false);
            }
        }
        if let Some((page, a, b)) = self.tabs5.sign_drag {
            let r = [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ];
            if let Some(w) = t.view.doc_rect_to_window(page, r) {
                frame(p, w, accent, true);
            }
        }
        p.pop_clip();
    }

    /// The additions' dialog, over everything else.
    pub(crate) fn paint_tabs5_dialog(&mut self, p: &mut dyn Painter) {
        self.paint_reader_backstage(p);
        self.paint_doc_props(p);
        if let Some((d, _)) = &self.tabs5.dialog {
            d.paint(p, &self.theme, self.size, true);
        }
    }
}

/// A box in the accent colour (filled lightly when `fill`).
fn frame(p: &mut dyn Painter, r: [f32; 4], accent: Rgba, fill: bool) {
    if fill {
        p.fill_rect(
            r[0],
            r[1],
            r[2],
            r[3],
            Rgba(accent.0, accent.1, accent.2, 30),
        );
    }
    let (x0, y0, x1, y1) = (
        r[0].round() + 0.5,
        r[1].round() + 0.5,
        (r[0] + r[2]).round() - 0.5,
        (r[1] + r[3]).round() - 0.5,
    );
    p.path(
        &[
            PathSeg::M(x0, y0),
            PathSeg::L(x1, y0),
            PathSeg::L(x1, y1),
            PathSeg::L(x0, y1),
            PathSeg::Z,
        ],
        None,
        Some((accent, 1.0)),
    );
}

fn tsa_client(url: &str) -> bo_sign::http::HttpTimestampClient {
    bo_sign::http::HttpTimestampClient::new(
        url.to_owned(),
        Box::new(bo_sign::http::UreqTransport::new(
            std::time::Duration::from_secs(20),
        )),
    )
}
