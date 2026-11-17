//! BlueEditor round 10: the Protect tab laid out and grouped as the reference editor's (black
//! box at 125 %, `docs/ui/REPORT-editor.md` round 10): Mark for Redaction ▾ (Text & Images,
//! Pages, Search & Redact, Apply Redactions), Whiteout, Protect Redactions ▾, Hidden Data ▾
//! (Sanitize Document, Remove Hidden Information), Secure Document ▾ (Password Protect,
//! Certificate Protect, Security Policies, Remove Security, Security Properties), Restrict
//! Access, Batch Encrypt, Fill & Sign, Sign & Certify ▾, Time Stamp Document, Validate.
//! Redaction removes the content for real (`bo_pdfread::PdfDocument::apply_redactions`).
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, check, edit, label, Form, On, Val};
use crate::editor_window::EditorWindow;
use crate::popup::{Choice, MenuItem};
use crate::reader_window::Kind;

/// A menu row: command, label key, icon, dimmed.
pub(crate) type Row10 = (&'static str, &'static str, Option<&'static str>, bool);

/// Ribbon items of the Protect tab.
pub(crate) fn protect_tab_items() -> Vec<(&'static str, Kind)> {
    use Kind::{Big, Sep};
    vec![
        ("sep1", Sep),
        (
            "prot.redactMenu",
            Big("redact", "pdfEditor.markRedaction", true),
        ),
        (
            "prot.whiteout",
            Big("whiteout", "pdfEditor.whiteout", false),
        ),
        (
            "prot.protRedactMenu",
            Big("applyRedact", "pdfEditor10.protectRedactions", true),
        ),
        ("sep2", Sep),
        (
            "prot.hiddenMenu",
            Big("hiddenData", "pdfEditor.hiddenData", true),
        ),
        ("sep3", Sep),
        ("prot.secureMenu", Big("secure", "pdfEditor.secure", true)),
        ("sep4", Sep),
        (
            "prot.restrictMenu",
            Big("restrictAccess", "pdfEditor.restrictAccess", true),
        ),
        (
            "prot.batchEncrypt",
            Big("secure", "pdfEditor.batchEncrypt", false),
        ),
        // Round 11: Settings ▾ and Sensitivity ▾ as the reference's.
        (
            "prot.settingsMenu",
            Big("secSettings", "pdfEditor11.settings", true),
        ),
        ("sep5a", Sep),
        (
            "prot.sensMenu",
            Big("sensitivity", "pdfEditor11.sensitivity", true),
        ),
        ("sep5", Sep),
        ("form.fill", Big("fillSign", "pdfEditor.fillSign", false)),
        ("prot.signMenu", Big("sign", "pdfEditor.signCertify", true)),
        (
            "prot.timestamp",
            Big("timestamp", "pdfEditor.timeStamp", false),
        ),
        (
            "prot.validate",
            Big("validate", "pdfProtect.validate", false),
        ),
        ("sep6", Sep),
    ]
}

impl EditorWindow {
    /// A drop-down of `id` as the round-9 menus (icons, a rule before the rows in
    /// `separators`): rows are (command, label key, icon, dimmed).
    pub(crate) fn menu10(&mut self, rows: &[Row10], separators: Vec<usize>, id: &'static str) {
        let Some(r) = self.r.control_rect(id) else {
            return;
        };
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(cmd, key, _, dim)| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        let icons = rows.iter().map(|r| r.2).collect();
        let menu = self
            .menu_open9(items, r)
            .with_decor(icons, separators)
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.r.set_menu9(menu, id);
    }

    /// Round 11: a drop-down like [`Self::menu10`] with ticked rows.
    pub(crate) fn menu11(&mut self, rows: &[Row10], ticked: &[usize], id: &'static str) {
        let Some(r) = self.r.control_rect(id) else {
            return;
        };
        let items: Vec<MenuItem> = rows
            .iter()
            .enumerate()
            .map(|(i, (cmd, key, _, dim))| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: ticked.contains(&i),
                dim: *dim,
            })
            .collect();
        let icons = rows.iter().map(|r| r.2).collect();
        let menu = self
            .menu_open9(items, r)
            .with_decor(icons, Vec::new())
            .with_icon_fn(crate::reader_icons::menu_icon);
        self.r.set_menu9(menu, id);
    }

    /// Commands of the round-10 Protect tab; false when not one of them.
    pub(crate) fn prot10_command(&mut self, cmd: &str) -> bool {
        let encrypted = self.view().is_some_and(|v| v.doc().is_encrypted());
        match cmd {
            // Round 11 (measured on the reference: Restrict Access ▾ has Unrestricted Access,
            // ticked and greyed, and rights-management templates; Settings ▾ has Custom
            // Templates, greyed, and Security Watermark; Sensitivity ▾ one service row).
            "prot.restrictMenu" => self.menu11(
                &[
                    ("prot.unrestricted", "pdfEditor11.unrestricted", None, true),
                    ("prot.drm", "pdfEditor11.drmTemplates", None, true),
                ],
                &[0],
                "prot.restrictMenu",
            ),
            "prot.settingsMenu" => self.menu11(
                &[
                    (
                        "prot.customTemplates",
                        "pdfEditor11.customTemplates",
                        None,
                        true,
                    ),
                    (
                        "org8.add.watermark",
                        "pdfEditor11.securityWatermark",
                        None,
                        false,
                    ),
                ],
                &[],
                "prot.settingsMenu",
            ),
            "prot.sensMenu" => self.menu11(
                &[("prot.sensService", "pdfEditor11.sensService", None, true)],
                &[],
                "prot.sensMenu",
            ),
            "prot.redactMenu" => {
                let none = self.x.marks.is_empty();
                self.menu10(
                    &[
                        (
                            "prot.redact",
                            "pdfEditor10.textImages",
                            Some("redact"),
                            false,
                        ),
                        (
                            "prot.redactPages",
                            "pdfEditor10.pages",
                            Some("pages"),
                            false,
                        ),
                        (
                            "prot.searchRedact",
                            "pdfEditor10.searchRedact",
                            Some("search"),
                            false,
                        ),
                        (
                            "prot.applyRedact",
                            "pdfEditor.applyRedactions",
                            Some("applyRedact"),
                            none,
                        ),
                    ],
                    vec![3],
                    "prot.redactMenu",
                );
            }
            "prot.protRedactMenu" => self.menu10(
                &[
                    (
                        "prot.unprotect",
                        "pdfEditor10.unprotectRedaction",
                        Some("check"),
                        true,
                    ),
                    ("prot.drm", "pdfEditor10.drm", None, true),
                ],
                Vec::new(),
                "prot.protRedactMenu",
            ),
            "prot.hiddenMenu" => self.menu10(
                &[
                    (
                        "prot.sanitize",
                        "pdfEditor10.sanitize",
                        Some("eraser"),
                        false,
                    ),
                    (
                        "prot.hiddenData",
                        "pdfEditor10.removeHidden",
                        Some("search"),
                        false,
                    ),
                ],
                Vec::new(),
                "prot.hiddenMenu",
            ),
            "prot.secureMenu" => self.menu10(
                &[
                    (
                        "prot.password",
                        "pdfEditor.passwordProtect",
                        Some("secure"),
                        false,
                    ),
                    (
                        "prot.certProtect",
                        "pdfEditor10.certificateProtect",
                        Some("sign"),
                        true,
                    ),
                    (
                        "prot.policies",
                        "pdfEditor10.securityPolicies",
                        Some("properties"),
                        true,
                    ),
                    (
                        "prot.removeSecurity",
                        "pdfEditor.removeSecurity",
                        Some("unsecure"),
                        !encrypted,
                    ),
                    (
                        "prot.securityProps",
                        "pdfEditor10.securityProperties",
                        Some("info"),
                        false,
                    ),
                ],
                Vec::new(),
                "prot.secureMenu",
            ),
            "prot.signMenu" => self.menu10(
                &[
                    (
                        "prot.sign",
                        "pdfEditor10.placeSignature",
                        Some("sign"),
                        false,
                    ),
                    (
                        "prot.signDoc",
                        "pdfEditor10.signDocument",
                        Some("sign"),
                        true,
                    ),
                    (
                        "prot.certifyVisible",
                        "pdfEditor10.certifyVisible",
                        Some("validate"),
                        true,
                    ),
                    (
                        "prot.certifyInvisible",
                        "pdfEditor10.certifyInvisible",
                        Some("validate"),
                        true,
                    ),
                    (
                        "prot.digitalIds",
                        "pdfEditor10.digitalIds",
                        Some("properties"),
                        true,
                    ),
                    (
                        "prot.trusted",
                        "pdfEditor10.trustedCerts",
                        Some("check"),
                        true,
                    ),
                ],
                vec![4],
                "prot.signMenu",
            ),
            "prot.redactPages" => self.open_redact_pages(),
            "prot.searchRedact" => self.open_search_redact(),
            "prot.sanitize" => self.open_sanitize(),
            "prot.securityProps" => self.open_security_props(),
            _ => return false,
        }
        true
    }

    /// Mark for Redaction › Pages: the pages to redact whole.
    fn open_redact_pages(&mut self) {
        let n = self
            .view()
            .map_or(0, crate::reader_view::ReaderView::page_count);
        let current = self
            .view()
            .map_or(0, crate::reader_view::ReaderView::current_page)
            + 1;
        let t = |k: &str| self.t(k);
        let c = vec![
            crate::editor_form8::group(t("pdfEditor10.pageRange"), [17.0, 55.0, 410.0, 120.0]),
            crate::editor_form8::radio("which", 0, 30.0, 88.0, t("pdfEditor10.currentPage")),
            crate::editor_form8::radio("which", 1, 30.0, 120.0, t("pdfEditor10.pagesFrom")),
            edit("pages", [140.0, 107.0, 160.0, 27.0], On::If("which", 1)),
            label(
                310.0,
                120.0,
                t("pdfEditor10.ofN").replace("{n}", &n.to_string()),
            ),
            button(
                "ok",
                [236.0, 196.0, 91.0, 28.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [336.0, 196.0, 91.0, 28.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor10.markPages"), (444.0, 240.0), c)
            .with("which", Val::Pick(0))
            .with("pages", Val::Text(format!("{current}-{n}")));
        self.form8.push((f, FormWhat::RedactPages10));
    }

    /// OK of Mark Pages: every chosen page gets a mark over all of it.
    pub(crate) fn redact_pages_ok(&mut self, f: &Form) {
        let Some(v) = self.view() else { return };
        let n = v.page_count();
        let pages: Vec<usize> = if f.pick("which") == 0 {
            vec![v.current_page()]
        } else {
            crate::editor_dlg8::page_groups(&f.text("pages"), n)
                .into_iter()
                .flatten()
                .collect()
        };
        let sizes: Vec<(usize, (f32, f32))> = pages.iter().map(|p| (*p, v.page_size(*p))).collect();
        for (p, (w, h)) in sizes {
            self.x.marks.push((p, [0.0, 0.0, w, h]));
        }
        let s = self
            .t("pdfEditor10.marked")
            .replace("{count}", &pages.len().to_string());
        self.set_status(&s);
    }

    /// Search & Redact: text to find, whole words, match case.
    fn open_search_redact(&mut self) {
        let t = |k: &str| self.t(k);
        let c = vec![
            label(17.0, 62.0, t("pdfEditor10.findText")),
            edit("find", [17.0, 78.0, 410.0, 28.0], On::Always),
            check("whole", 17.0, 128.0, t("pdfEditor10.wholeWords")),
            check("case", 17.0, 158.0, t("pdfEditor10.caseSensitive")),
            button(
                "ok",
                [236.0, 196.0, 91.0, 28.0],
                t("pdfEditor10.searchAndMark"),
                On::Always,
            ),
            button(
                "cancel",
                [336.0, 196.0, 91.0, 28.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor10.searchRedact"), (444.0, 240.0), c)
            .with("whole", Val::Pick(0))
            .with("case", Val::Pick(0))
            .focused("find");
        self.form8.push((f, FormWhat::SearchRedact10));
    }

    /// OK of Search & Redact: every match is marked for redaction.
    pub(crate) fn search_redact_ok(&mut self, f: &Form) {
        let needle = f.text("find");
        if needle.trim().is_empty() {
            return;
        }
        let (whole, case) = (f.pick("whole") == 1, f.pick("case") == 1);
        let n = self
            .view()
            .map_or(0, crate::reader_view::ReaderView::page_count);
        let mut found = 0;
        for page in 0..n {
            let rects = self
                .with_doc(|d| Ok(search_boxes(d, page, &needle, whole, case)))
                .unwrap_or_default();
            found += rects.len();
            for r in rects {
                self.x.marks.push((page, r));
            }
        }
        let s = self
            .t("pdfEditor10.marked")
            .replace("{count}", &found.to_string());
        self.set_status(&s);
    }

    /// Hidden Data › Sanitize Document: confirm, then remove everything hidden at once.
    fn open_sanitize(&mut self) {
        let t = |k: &str| self.t(k);
        let c = vec![
            label(17.0, 66.0, t("pdfEditor10.sanitizeText1")),
            label(17.0, 92.0, t("pdfEditor10.sanitizeText2")),
            button(
                "ok",
                [236.0, 136.0, 91.0, 28.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [336.0, 136.0, 91.0, 28.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor10.sanitize"), (444.0, 180.0), c);
        self.form8.push((f, FormWhat::Sanitize10));
    }

    /// OK of Sanitize Document.
    pub(crate) fn sanitize_ok(&mut self) {
        let r = self.doc_edit(|d| d.remove_hidden_data(bo_pdfread::HiddenData::all()));
        match r {
            Ok(()) => {
                self.after_doc_edit();
                let s = self.t("pdfEditor10.sanitized");
                self.set_status(&s);
            }
            Err(e) => self.set_status(&e),
        }
    }

    /// Secure Document › Security Properties: the security method and what is allowed.
    fn open_security_props(&mut self) {
        let info = self.with_doc(|d| Ok(d.security_info())).ok().flatten();
        let t = |k: &str| self.t(k);
        let yes = t("pdfEditor10.allowed");
        let no = t("pdfEditor10.notAllowed");
        let (method, level, perms) = match &info {
            Some(i) => (
                t("pdfEditor10.passwordSecurity"),
                i.algorithm.clone(),
                i.permissions,
            ),
            None => (t("pdfEditor10.noSecurity"), String::new(), -1_i32),
        };
        let bit = |b: u32| {
            if perms & (1 << (b - 1)) != 0 {
                yes.clone()
            } else {
                no.clone()
            }
        };
        let rows = [
            (t("pdfEditor10.method"), method),
            (t("pdfEditor10.encryptionLevel"), level),
            (t("pdfEditor10.printing"), bit(3)),
            (t("pdfEditor10.changing"), bit(4)),
            (t("pdfEditor10.copying"), bit(5)),
            (t("pdfEditor10.commenting"), bit(6)),
            (t("pdfEditor10.fillingFields"), bit(9)),
            (t("pdfEditor10.assembly"), bit(11)),
        ];
        let mut c = vec![crate::editor_form8::group(
            t("pdfEditor10.securitySummary"),
            [17.0, 55.0, 470.0, 300.0],
        )];
        for (k, (name, v)) in rows.iter().enumerate() {
            let y = 88.0 + 33.0 * k as f32;
            c.push(label(30.0, y, name.clone()));
            c.push(label(250.0, y, v.clone()));
        }
        c.push(button(
            "ok",
            [396.0, 376.0, 91.0, 28.0],
            t("pdfReader.ok"),
            On::Always,
        ));
        let f = Form::new(t("pdfEditor10.securityProperties"), (504.0, 420.0), c);
        self.form8.push((f, FormWhat::Sub));
    }
}

/// Boxes (page points) of the matches of `needle` on `page`, one per match and line.
pub(crate) fn search_boxes(
    d: &mut bo_pdfread::PdfDocument,
    page: usize,
    needle: &str,
    whole: bool,
    case: bool,
) -> Vec<[f32; 4]> {
    let Ok(objs) = d.page_objects(page) else {
        return Vec::new();
    };
    let fold = |s: &str| {
        if case {
            s.to_owned()
        } else {
            s.to_lowercase()
        }
    };
    let n = fold(needle);
    let mut out = Vec::new();
    for line in bo_pdfread::text_lines(&objs) {
        let text = fold(&line.text);
        let chars: Vec<char> = text.chars().collect();
        let nc: Vec<char> = n.chars().collect();
        if nc.is_empty() || chars.len() != line.char_boxes.len() {
            continue;
        }
        let mut i = 0;
        while i + nc.len() <= chars.len() {
            if chars[i..i + nc.len()] == nc[..] {
                let word = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric());
                let ok = !whole
                    || (!word(i.checked_sub(1).and_then(|j| chars.get(j)))
                        && !word(chars.get(i + nc.len())));
                if ok {
                    let bx = &line.char_boxes[i..i + nc.len()];
                    out.push([
                        bx.iter().map(|b| b[0]).fold(f32::MAX, f32::min),
                        bx.iter().map(|b| b[1]).fold(f32::MAX, f32::min),
                        bx.iter().map(|b| b[2]).fold(f32::MIN, f32::max),
                        bx.iter().map(|b| b[3]).fold(f32::MIN, f32::max),
                    ]);
                    i += nc.len();
                    continue;
                }
            }
            i += 1;
        }
    }
    out
}
