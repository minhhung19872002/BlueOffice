//! BlueReader round 12 (a child of `reader_window`), measured on the reference reader at 125 %:
//!
//! - Ribbon metrics: icons drawn 31 px high from y 112, labels from y 154, second lines 19 px
//!   apart, buttons label + 10 px wide and 2 px apart (Hand 52 px).
//! - Home / every tab: Select opens Select Text and Image / Select Annotation from its label.
//! - Comment: Summarize Comments, Import, Export as large buttons, then FDF via Email, Manage
//!   Comments and Keep Tool Selected as labelled rows.
//! - View: Assistant (Marquee zoom; Loupe and Magnifier shown grey).
//! - Form: Form to sheet has its drop-down.
//! - Share: the reference's online commands, grey (they need a review / cloud service).
use super::fill11::{DLG_H, DLG_W};
use super::{inside, Kind, ReaderRequest, ReaderWindow, Tool};
use crate::popup::{Choice, MenuItem};
use crate::{Input, Key, Painter, TextMeasure};
use bo_render::{PathSeg, Rgba};

/// Marquee rectangle while dragging: frame and fill.
const MARQUEE_EDGE: Rgba = Rgba(38, 128, 235, 255);
const MARQUEE_FILL: Rgba = Rgba(38, 128, 235, 40);

/// The View tab's Assistant tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Assist {
    /// Drag a rectangle to zoom to it; a click zooms in one step.
    Marquee,
}

/// State of the round 12 additions.
#[derive(Debug, Default)]
pub(crate) struct Twelve {
    /// Last pointer release (window px): which part of a split button was clicked.
    pub(crate) at: (f32, f32),
    /// Assistant tool in use.
    pub(crate) assist: Option<Assist>,
    /// Marquee drag: start x, y, current x, y (window px).
    pub(crate) marquee: Option<[f32; 4]>,
    /// Select ▸ Select Annotation: the Select tool picks comments, not text.
    pub(crate) select_annot: bool,
    /// Password asked before placing saved signature `.0`: typed text, last try was wrong.
    pub(crate) pw_prompt: Option<(usize, String, bool)>,
}

/// What an element exposed to assistive technology is (UI Automation control types in the
/// Windows host).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRole {
    /// A command button (Invoke).
    Button,
    /// A ribbon tab (Invoke selects it).
    Tab,
    /// The page view.
    Document,
}

/// One element of the window for assistive technology (round 12).
#[derive(Debug, Clone, PartialEq)]
pub struct AccessItem {
    /// Stable key: `tab:<n>`, `cmd:<command id>`, `doc`.
    pub key: String,
    /// Name read out.
    pub name: String,
    /// Kind of element.
    pub role: AccessRole,
    /// Box in client pixels.
    pub rect: [f32; 4],
    /// Can be used now.
    pub enabled: bool,
    /// Selected tab / command shown on.
    pub selected: bool,
}

/// SHA-256 rounds of a signature password hash.
const PW_ROUNDS: usize = 10_000;

fn hex12(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        use std::fmt::Write as _;
        let _ = write!(s, "{x:02x}");
        s
    })
}

fn pw_digest12(salt: &[u8], pw: &str) -> Vec<u8> {
    let sha = bo_sign::HashAlg::Sha256;
    let mut h = sha.digest(&[salt, pw.as_bytes()].concat());
    for _ in 1..PW_ROUNDS {
        h = sha.digest(&[&h[..], salt].concat());
    }
    h
}

/// `salt$hash` (hex) of a signature password: a fresh 16-byte salt, SHA-256 iterated
/// [`PW_ROUNDS`] times. Only this is written to the saved signatures, never the password.
pub(crate) fn hash_password12(pw: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let seed = format!(
        "{now}:{}:{}:{:p}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed),
        &now
    );
    let salt = bo_sign::HashAlg::Sha256.digest(seed.as_bytes());
    let salt = &salt[..16];
    format!("{}${}", hex12(salt), hex12(&pw_digest12(salt, pw)))
}

/// Whether `pw` matches a hash made by [`hash_password12`].
pub(crate) fn check_password12(stored: &str, pw: &str) -> bool {
    let Some((salt, hash)) = stored.split_once('$') else {
        return false;
    };
    let salt: Option<Vec<u8>> = (0..salt.len() / 2)
        .map(|i| u8::from_str_radix(salt.get(2 * i..2 * i + 2)?, 16).ok())
        .collect();
    let Some(salt) = salt else { return false };
    // Compare every byte (no early exit).
    let mine = hex12(&pw_digest12(&salt, pw));
    mine.len() == hash.len()
        && mine
            .bytes()
            .zip(hash.bytes())
            .fold(0_u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

impl ReaderWindow {
    /// Smallest large-button width: the reference's Hand is 52 px, the others fit their label.
    pub(crate) fn big_min12(&self, id: &str) -> f32 {
        if self.profile.is_none() && id == "tool.hand" {
            41.6
        } else {
            self.look.big_min
        }
    }

    /// Small-button grids (text markup, page display): gap between columns, space before the
    /// first column and after the last (logical px). BlueReader, measured: columns 44 px apart,
    /// the Comment tab's grid from x 138 and Note 49 px after it; windows built on it keep 6 / 0
    /// / 0.
    pub(crate) fn grid12(&self) -> (f32, f32, f32) {
        if self.profile.is_none() {
            (7.2, 9.2, 10.8)
        } else {
            (6.0, 0.0, 0.0)
        }
    }

    /// Distance between the two lines of a large button's label (19 px at 125 % in BlueReader).
    pub(crate) fn label_pitch12(&self, size: f32) -> f32 {
        // BlueEditor round 11 (measured on the reference editor at 125 %): 23 px.
        if self.profile.as_ref().is_some_and(|p| p.app == "BlueEditor") {
            return self.px(18.4);
        }
        if self.profile.is_none() {
            self.px(15.2)
        } else {
            (size * 1.33 * 0.93).round()
        }
    }

    /// BlueReader's ribbon items after round 12's changes (windows built on it keep theirs).
    pub(crate) fn ribbon12(&self, items: Vec<(&'static str, Kind)>) -> Vec<(&'static str, Kind)> {
        use Kind::{Big, Row, Sep};
        if self.profile.is_some() {
            return items;
        }
        let comment = items.iter().any(|(id, _)| *id == "comment.summarize");
        let mut out = Vec::with_capacity(items.len() + 8);
        for (id, kind) in items {
            match id {
                "tool.select" => out.push((id, Big("select", "pdfReader.select", true))),
                "form.toSheet" => out.push((id, Big("formToSheet", "pdfForm.toSheet", true))),
                // The Comment tab's right end is rebuilt below.
                "comment.summarize"
                | "comment.import"
                | "menu.commentExport"
                | "panel.comments"
                | "menu.commentOptions"
                | "comment.keep" => {}
                "view.autoScroll" => {
                    out.push((id, kind));
                    out.push(("sep12a", Sep));
                    out.push((
                        "menu.assistant",
                        Big("assistant", "pdfView12.assistant", true),
                    ));
                }
                "share.folder" => {
                    out.push((id, kind));
                    out.push(("sep12a", Sep));
                    out.push((
                        "share.files",
                        Big("sharedFiles", "pdfShare12.sharedFiles", false),
                    ));
                    out.push(("sep12b", Sep));
                    out.push((
                        "share.sharedReview",
                        Big("sharedReview", "pdfShare12.sharedReview", true),
                    ));
                    out.push((
                        "share.emailReview",
                        Big("emailReview", "pdfShare12.emailReview", true),
                    ));
                    out.push(("share.tracker", Big("tracker", "pdfShare12.tracker", false)));
                }
                _ => out.push((id, kind)),
            }
        }
        if comment {
            // Separators the old rows left at the end.
            while matches!(out.last(), Some((_, Sep))) {
                out.pop();
            }
            out.extend([
                ("sep12c", Sep),
                (
                    "comment.summarize",
                    Big("summarize", "pdfComment.summarize", false),
                ),
                (
                    "comment.import",
                    Big("importComments", "pdfComment.import", false),
                ),
                (
                    "menu.commentExport",
                    Big("exportComments", "pdfComment.export", true),
                ),
                (
                    "comment.fdfMail",
                    Row("mail", "pdfComment12.fdfMail", false),
                ),
                (
                    "menu.commentOptions",
                    Row("commentOptions", "pdfComment12.manage", true),
                ),
                ("comment.keep", Row("pin", "pdfComment12.keep", false)),
            ]);
        }
        out
    }

    /// Enabled state of round 12's controls (`None`: not one of them).
    pub(crate) fn twelve_enabled(&self, id: &str) -> Option<bool> {
        match id {
            // Online services the reference reaches through its account; none here.
            "share.files" | "share.sharedReview" | "share.emailReview" | "share.tracker" => {
                Some(false)
            }
            "comment.fdfMail" => Some(self.tabs.get(self.active).is_some()),
            _ => None,
        }
    }

    /// A ribbon click that round 12 handles itself (split buttons): `true` when handled.
    pub(crate) fn click12(&mut self, id: &'static str, r: [f32; 4]) -> bool {
        if self.profile.is_some() {
            return false;
        }
        // The label part of a split button opens its menu, the icon part runs the command.
        let lower = self.twelve.at.1 >= r[1] + self.px(self.look.label_y) - self.px(4.0);
        match id {
            "tool.select" if lower => {
                let text = matches!(self.tool, Tool::Select) && !self.twelve.select_annot;
                let annot = matches!(self.tool, Tool::Select) && self.twelve.select_annot;
                let items = vec![
                    self.item12("select.text", "pdfSelect12.text", text, false),
                    self.item12("select.annot", "pdfSelect12.annot", annot, false),
                ];
                self.menu_at(items, r, "tool.select");
                true
            }
            "form.toSheet" if lower => {
                let items = vec![
                    self.item12("form.toSheet", "pdfForm12.toSheet", false, false),
                    self.item12("form.combine12", "pdfForm12.combine", false, true),
                ];
                self.menu_at(items, r, "form.toSheet");
                true
            }
            _ => false,
        }
    }

    fn item12(&self, cmd: &str, key: &str, checked: bool, dim: bool) -> MenuItem {
        MenuItem {
            label: self.t(key),
            choice: Choice::Command(cmd.to_owned()),
            checked,
            dim,
        }
    }

    /// Round 12's commands: `true` when handled.
    pub(crate) fn command_twelve(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        // Any other tool ends the Assistant tool.
        if self.twelve.assist.is_some()
            && (cmd.starts_with("tool.")
                || cmd.starts_with("draw.")
                || cmd.starts_with("fill.")
                || cmd.starts_with("comment."))
        {
            self.set_assist12(None);
        }
        match cmd {
            "tool.select" => {
                self.twelve.select_annot = false;
                false
            }
            "select.text" => {
                self.twelve.select_annot = false;
                self.command("tool.select", m);
                true
            }
            "select.annot" => {
                self.command("tool.select", m);
                self.twelve.select_annot = true;
                true
            }
            "menu.assistant" => {
                let on = self.twelve.assist == Some(Assist::Marquee);
                let items = vec![
                    self.item12("assist.marquee", "pdfView12.marquee", on, false),
                    self.item12("assist.loupe", "pdfView12.loupe", false, true),
                    self.item12("assist.magnifier", "pdfView12.magnifier", false, true),
                ];
                let r = self.control_rect("menu.assistant").unwrap_or([0.0; 4]);
                self.menu_at(items, r, "menu.assistant");
                true
            }
            "assist.marquee" => {
                if self.twelve.assist == Some(Assist::Marquee) {
                    self.set_assist12(None);
                } else {
                    self.command("tool.hand", m);
                    self.set_assist12(Some(Assist::Marquee));
                }
                true
            }
            "comment.fdfMail" => {
                // The comments as FDF (saved where the user picks), then a new message to
                // attach it to: no mail program is driven behind the user's back.
                self.command("comment.exportFdf", m);
                let name = self
                    .tabs
                    .get(self.active)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();
                let subject = self
                    .strings
                    .fmt("pdfComment12.fdfSubject", &[("name", &name)]);
                let url = format!("mailto:?subject={}", mail_escape12(&subject));
                self.requests.push(ReaderRequest::OpenUrl(url));
                true
            }
            _ => false,
        }
    }

    fn set_assist12(&mut self, a: Option<Assist>) {
        self.twelve.assist = a;
        self.twelve.marquee = None;
        self.tool_hidden = a.is_some();
        if a.is_some() {
            self.on.insert("menu.assistant");
        } else {
            self.on.remove("menu.assistant");
        }
    }

    /// Input for round 12 first: `Some(redraw)` when handled.
    pub(crate) fn route_twelve(&mut self, input: &Input, _m: &dyn TextMeasure) -> Option<bool> {
        match *input {
            Input::Release { x, y } | Input::Press { x, y, .. } => {
                self.twelve.at = (x, y);
            }
            _ => {}
        }
        if self.twelve.pw_prompt.is_some() {
            return Some(self.pw_prompt_input12(input));
        }
        if self.menu.is_some() || self.dialog.is_some() || !self.showing_doc() {
            return None;
        }
        if self.twelve.select_annot && matches!(self.tool, Tool::Select) {
            // Select Annotation: a press away from a comment selects nothing (no text).
            if let Input::Press { x, y, .. } = *input {
                if inside(self.doc_rect, x, y) && self.comment_at(x, y).is_none() {
                    self.extra.selected = None;
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        t.view.set_selection(None);
                    }
                    return Some(true);
                }
            }
        }
        self.twelve.assist?;
        match *input {
            Input::Press { x, y, .. } if inside(self.doc_rect, x, y) => {
                self.twelve.marquee = Some([x, y, x, y]);
                Some(true)
            }
            Input::Drag { x, y } => {
                let d = self.doc_rect;
                let m = self.twelve.marquee.as_mut()?;
                m[2] = x.clamp(d[0], d[0] + d[2]);
                m[3] = y.clamp(d[1], d[1] + d[3]);
                Some(true)
            }
            Input::Release { x, y } => {
                let [x0, y0, ..] = self.twelve.marquee.take()?;
                self.marquee_zoom12([x0, y0, x, y]);
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.set_assist12(None);
                Some(true)
            }
            _ => None,
        }
    }

    /// Zoom so the dragged rectangle fills the view (a click: one zoom step in at the point).
    pub(crate) fn marquee_zoom12(&mut self, r: [f32; 4]) {
        let (x0, x1) = (r[0].min(r[2]), r[0].max(r[2]));
        let (y0, y1) = (r[1].min(r[3]), r[1].max(r[3]));
        let (cx, cy) = (f32::midpoint(x0, x1), f32::midpoint(y0, y1));
        let small = self.px(4.0);
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let v = &mut t.view;
        let Some((page, px, py)) = v.page_at(cx, cy) else {
            return;
        };
        let pt = v.px_per_pt();
        let frame = v.frame();
        if x1 - x0 < small && y1 - y0 < small {
            v.step_zoom(1);
        } else {
            let k = (frame[2] / (x1 - x0).max(1.0)).min(frame[3] / (y1 - y0).max(1.0));
            v.set_zoom(v.zoom() * k);
        }
        // Bring the rectangle's centre to the middle of the view.
        let k = v.px_per_pt() / pt.max(1e-6);
        if let Some(pr) = v.page_rect(page) {
            let wx = pr[0] + px * pt * k;
            let wy = pr[1] + py * pt * k;
            v.scroll_by(
                wx - (frame[0] + frame[2] / 2.0),
                wy - (frame[1] + frame[3] / 2.0),
            );
        }
    }

    /// Round 12's drawing over the page (the marquee rectangle).
    pub(crate) fn paint_twelve(&self, p: &mut dyn Painter) {
        self.paint_pw_prompt12(p);
        let Some([a, b, c, d]) = self.twelve.marquee else {
            return;
        };
        let (x, y) = (a.min(c).round() + 0.5, b.min(d).round() + 0.5);
        let (w, h) = ((a - c).abs().round(), (b - d).abs().round());
        let rect = [
            PathSeg::M(x, y),
            PathSeg::L(x + w, y),
            PathSeg::L(x + w, y + h),
            PathSeg::L(x, y + h),
            PathSeg::Z,
        ];
        p.path(&rect, Some(MARQUEE_FILL), Some((MARQUEE_EDGE, 1.0)));
    }

    // ----- UI Automation ----------------------------------------------------------------

    /// The window's elements for assistive technology: the ribbon tabs, every ribbon and title
    /// bar command (name, box, enabled, on), the page view (round 12; the Windows host turns
    /// them into UI Automation elements).
    pub fn accessibility12(&self) -> Vec<AccessItem> {
        let labels = self.tab_label_keys();
        let mut out: Vec<AccessItem> = self
            .tab_rects
            .iter()
            .enumerate()
            .map(|(i, r)| AccessItem {
                key: format!("tab:{i}"),
                name: self.t(labels[i]),
                role: AccessRole::Tab,
                rect: *r,
                enabled: true,
                selected: if self.backstage_open {
                    i == 0
                } else {
                    i > 0 && i - 1 == self.ribbon_tab
                },
            })
            .collect();
        let tool = self.tool_id();
        for (id, name, rect) in self.accessible_names11() {
            out.push(AccessItem {
                key: format!("cmd:{id}"),
                name,
                role: AccessRole::Button,
                rect,
                enabled: self.enabled(id),
                selected: id == tool || self.on.contains(id),
            });
        }
        if let Some(t) = self.tabs.get(self.active) {
            let page = t.view.current_page() + 1;
            out.push(AccessItem {
                key: "doc".into(),
                name: self.strings.fmt(
                    "pdfA11y12.document",
                    &[
                        ("name", &t.name),
                        ("page", &page.to_string()),
                        ("pages", &t.view.page_count().to_string()),
                    ],
                ),
                role: AccessRole::Document,
                rect: self.doc_rect,
                enabled: true,
                selected: false,
            });
        }
        out
    }

    /// Run an element of [`Self::accessibility12`] (UI Automation Invoke): a tab is selected,
    /// a command runs as if clicked. Unknown or grey elements do nothing; `true` when run.
    pub fn invoke12(&mut self, key: &str, m: &dyn TextMeasure) -> bool {
        if let Some(i) = key
            .strip_prefix("tab:")
            .and_then(|v| v.parse::<usize>().ok())
        {
            if i == 0 {
                let r = self.tab_rects.first().copied().unwrap_or([0.0; 4]);
                if self.file_backstage {
                    self.requests
                        .push(ReaderRequest::Command("file.backstage".into()));
                } else {
                    self.open_file_menu(r);
                }
            } else if i < self.tab_rects.len() {
                self.set_ribbon_tab(i - 1, m);
            }
            return true;
        }
        let Some(id) = key.strip_prefix("cmd:") else {
            return false;
        };
        let Some(c) = self.controls.iter().find(|c| c.id == id).copied() else {
            return false;
        };
        if !self.enabled(c.id) {
            return false;
        }
        self.twelve.at = (c.r[0] + c.r[2] / 2.0, c.r[1] + c.r[3] / 2.0);
        self.click(c.id, c.r, m);
        true
    }

    // ----- Signature password ---------------------------------------------------------

    /// The Options box's Password and Confirm Password boxes (measured on the reference's
    /// 502 × 438 px box at 125 %: 137 × 27 px from x 212, y 267 and 318), from its check box
    /// rows.
    fn pw_boxes12(&self, rows: &[[f32; 4]; 4]) -> ([f32; 4], [f32; 4]) {
        let ox = rows[3][0] - self.px(27.2);
        let oy = rows[3][1] - self.px(172.8);
        let at = |y: f32| {
            [
                ox + self.px(169.6),
                oy + self.px(y),
                self.px(109.6),
                self.px(21.6),
            ]
        };
        (at(213.6), at(254.4))
    }

    /// Input while the Create Signature dialog's Options box is open.
    pub(crate) fn options_input12(
        &mut self,
        input: &Input,
        rows: [[f32; 4]; 4],
        ok: [f32; 4],
        cancel: [f32; 4],
    ) -> bool {
        let (b1, b2) = self.pw_boxes12(&rows);
        let mismatch = self.t("pdfSig12.mismatch");
        let Some(d) = self.eleven.dialog.as_mut() else {
            return false;
        };
        let restore = |d: &mut super::fill11::SigDialog| {
            (d.bw, d.keep_size, d.discard) = d.options_was;
            (d.require_pw, d.pw, d.pw2) = d.pw_was.clone();
            d.pw_field = 0;
            d.options_open = false;
        };
        // OK: the two passwords must match (and not be empty) when one is required.
        let accept = |d: &mut super::fill11::SigDialog| -> bool {
            if d.require_pw && (d.pw.is_empty() || d.pw != d.pw2) {
                d.pw2.clear();
                d.pw_field = 2;
                return false;
            }
            d.pw_field = 0;
            d.options_open = false;
            true
        };
        let mut status = None;
        match input {
            Input::Press { x, y, .. } => {
                let (x, y) = (*x, *y);
                let image = d.tab >= 2;
                if inside(rows[0], x, y) && image {
                    d.bw = !d.bw;
                } else if inside(rows[1], x, y) && image {
                    d.keep_size = !d.keep_size;
                } else if inside(rows[2], x, y) {
                    d.discard = !d.discard;
                } else if inside(rows[3], x, y) {
                    d.require_pw = !d.require_pw;
                    d.pw_field = u8::from(d.require_pw);
                } else if d.require_pw && inside(b1, x, y) {
                    d.pw_field = 1;
                } else if d.require_pw && inside(b2, x, y) {
                    d.pw_field = 2;
                } else if inside(ok, x, y) {
                    if !accept(d) {
                        status = Some(mismatch);
                    }
                } else if inside(cancel, x, y) {
                    restore(d);
                }
            }
            Input::Text(s) if d.pw_field > 0 => {
                let f = if d.pw_field == 1 {
                    &mut d.pw
                } else {
                    &mut d.pw2
                };
                f.extend(s.chars().filter(|c| !c.is_control()));
            }
            Input::Key(Key::Backspace, _) if d.pw_field > 0 => {
                let f = if d.pw_field == 1 {
                    &mut d.pw
                } else {
                    &mut d.pw2
                };
                f.pop();
            }
            Input::Key(Key::Tab, _) if d.require_pw => {
                d.pw_field = if d.pw_field == 1 { 2 } else { 1 };
            }
            Input::Key(Key::Escape, _) => restore(d),
            Input::Key(Key::Enter, _) => {
                if !accept(d) {
                    status = Some(mismatch);
                }
            }
            Input::Resize { .. } | Input::Scale(_) => return false,
            _ => {}
        }
        if let Some(s) = status {
            self.status = s;
        }
        true
    }

    /// Paint the Password / Confirm Password boxes of the Options box (grey unless Require
    /// password is ticked); the typed text shows as dots.
    pub(crate) fn paint_pw_fields12(&self, p: &mut dyn Painter, d: &super::fill11::SigDialog) {
        let rows = self.opt_rows12();
        let (b1, b2) = self.pw_boxes12(&rows);
        let ui = self.style(15.0, false);
        let on = d.require_pw;
        let label_ink = if on {
            Rgba(51, 51, 51, 255)
        } else {
            Rgba(117, 117, 118, 255)
        };
        for (k, (b, key, text)) in [
            (b1, "pdfSig12.password", &d.pw),
            (b2, "pdfSig12.confirm", &d.pw2),
        ]
        .into_iter()
        .enumerate()
        {
            let label = self.t(key);
            let lw = self.width_of(&label, ui);
            p.text(
                &label,
                b[0] - self.px(19.2) - lw,
                b[1] + self.px(3.2),
                ui,
                label_ink,
            );
            let focus = on && usize::from(d.pw_field) == k + 1;
            let edge = if focus {
                MARQUEE_EDGE
            } else {
                Rgba(204, 204, 204, 255)
            };
            p.fill_rect(b[0], b[1], b[2], b[3], edge);
            p.fill_rect(
                b[0] + 1.0,
                b[1] + 1.0,
                b[2] - 2.0,
                b[3] - 2.0,
                if on {
                    Rgba(255, 255, 255, 255)
                } else {
                    Rgba(250, 250, 250, 255)
                },
            );
            let dots: String = "\u{25CF}".repeat(text.chars().count().min(24));
            let small = self.style(10.0, false);
            p.text(
                &dots,
                b[0] + self.px(4.0),
                b[1] + self.px(5.6),
                small,
                Rgba(0, 0, 0, 255),
            );
            if focus {
                let cx = b[0] + self.px(4.0) + self.width_of(&dots, small) + 1.0;
                p.fill_rect(
                    cx.round(),
                    b[1] + self.px(3.2),
                    1.0,
                    b[3] - self.px(6.4),
                    Rgba(0, 0, 0, 255),
                );
            }
        }
    }

    /// The Options box's check box rows (as `opt_parts11` places them).
    fn opt_rows12(&self) -> [[f32; 4]; 4] {
        let (w, h) = (self.px(DLG_W), self.px(DLG_H));
        let (ww, wh) = self.size;
        let d = [((ww - w) / 2.0).round(), ((wh - h) / 2.0).round(), w, h];
        let (ow, oh) = (self.px(401.6), self.px(350.4));
        let o = [
            (d[0] + (d[2] - ow) / 2.0).round(),
            (d[1] + (d[3] - oh) / 2.0).round(),
        ];
        let at = |y: f32| {
            [
                o[0] + self.px(27.2),
                o[1] + self.px(y),
                self.px(340.0),
                self.px(17.6),
            ]
        };
        [at(43.2), at(86.4), at(129.6), at(172.8)]
    }

    /// Saved signature `i` needs a password: ask for it (`true`) before it can be placed.
    pub(crate) fn ask_password12(&mut self, i: usize) -> bool {
        let needs = self
            .eleven
            .sigs
            .get(i)
            .is_some_and(|s| s.password.is_some());
        if needs {
            self.twelve.pw_prompt = Some((i, String::new(), false));
        }
        needs
    }

    /// The password prompt: box, field, OK, Cancel (window px).
    fn pw_prompt_parts12(&self) -> ([f32; 4], [f32; 4], [f32; 4], [f32; 4]) {
        let (w, h) = (self.px(320.0), self.px(150.4));
        let (ww, wh) = self.size;
        let o = [((ww - w) / 2.0).round(), ((wh - h) / 2.0).round(), w, h];
        let at = |x: f32, y: f32, bw: f32, bh: f32| {
            [
                o[0] + self.px(x),
                o[1] + self.px(y),
                self.px(bw),
                self.px(bh),
            ]
        };
        (
            o,
            at(19.2, 64.0, 281.6, 21.6),
            at(121.6, 112.0, 84.0, 25.6),
            at(216.0, 112.0, 84.0, 25.6),
        )
    }

    fn pw_prompt_input12(&mut self, input: &Input) -> bool {
        let (_, _, ok, cancel) = self.pw_prompt_parts12();
        let Some((i, text, wrong)) = self.twelve.pw_prompt.as_mut() else {
            return false;
        };
        let mut done = None;
        match input {
            Input::Text(s) => {
                text.extend(s.chars().filter(|c| !c.is_control()));
                *wrong = false;
            }
            Input::Key(Key::Backspace, _) => {
                text.pop();
            }
            Input::Key(Key::Escape, _) => done = Some(false),
            Input::Key(Key::Enter, _) => done = Some(true),
            Input::Press { x, y, .. } => {
                if inside(ok, *x, *y) {
                    done = Some(true);
                } else if inside(cancel, *x, *y) {
                    done = Some(false);
                }
            }
            Input::Resize { .. } | Input::Scale(_) => return false,
            _ => {}
        }
        let i = *i;
        match done {
            Some(true) => {
                let good = self
                    .eleven
                    .sigs
                    .get(i)
                    .and_then(|s| s.password.as_deref())
                    .zip(self.twelve.pw_prompt.as_ref())
                    .is_some_and(|(h, (_, t, _))| check_password12(h, t));
                if good {
                    self.twelve.pw_prompt = None;
                    self.start_placing11(i);
                } else if let Some((_, t, w)) = self.twelve.pw_prompt.as_mut() {
                    t.clear();
                    *w = true;
                }
            }
            Some(false) => self.twelve.pw_prompt = None,
            None => {}
        }
        true
    }

    fn paint_pw_prompt12(&self, p: &mut dyn Painter) {
        let Some((_, text, wrong)) = &self.twelve.pw_prompt else {
            return;
        };
        let (o, f, ok, cancel) = self.pw_prompt_parts12();
        let (ww, wh) = self.size;
        p.fill_rect(0.0, 0.0, ww, wh, Rgba(0, 0, 0, 40));
        p.fill_rect(
            o[0] - 1.0,
            o[1] - 1.0,
            o[2] + 2.0,
            o[3] + 2.0,
            Rgba(130, 130, 130, 255),
        );
        p.fill_rect(o[0], o[1], o[2], o[3], Rgba(255, 255, 255, 255));
        p.fill_rect(o[0], o[1], o[2], self.px(25.6), Rgba(243, 243, 243, 255));
        let ink = Rgba(0, 0, 0, 255);
        let ts = self.style(17.0, false);
        p.text(
            &self.t("pdfSig12.promptTitle"),
            o[0] + self.px(6.4),
            o[1] + self.px(4.0),
            ts,
            ink,
        );
        let ui = self.style(15.0, false);
        let (msg, col) = if *wrong {
            (self.t("pdfSig12.wrong"), Rgba(196, 43, 28, 255))
        } else {
            (self.t("pdfSig12.prompt"), Rgba(51, 51, 51, 255))
        };
        p.text(&msg, f[0], o[1] + self.px(36.8), ui, col);
        p.fill_rect(f[0], f[1], f[2], f[3], MARQUEE_EDGE);
        p.fill_rect(
            f[0] + 1.0,
            f[1] + 1.0,
            f[2] - 2.0,
            f[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let small = self.style(10.0, false);
        let dots: String = "\u{25CF}".repeat(text.chars().count().min(40));
        p.text(&dots, f[0] + self.px(4.0), f[1] + self.px(5.6), small, ink);
        let cx = f[0] + self.px(4.0) + self.width_of(&dots, small) + 1.0;
        p.fill_rect(
            cx.round(),
            f[1] + self.px(3.2),
            1.0,
            f[3] - self.px(6.4),
            ink,
        );
        for (b, key, primary) in [
            (ok, "pdfReader.ok", true),
            (cancel, "pdfPrint.cancel", false),
        ] {
            let (fill, txt) = if primary {
                (self.theme.accent_strong, Rgba(255, 255, 255, 255))
            } else {
                (Rgba(255, 255, 255, 255), ink)
            };
            p.fill_rect(b[0], b[1], b[2], b[3], Rgba(190, 190, 190, 255));
            p.fill_rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, b[3] - 2.0, fill);
            let label = self.t(key);
            let lw = self.width_of(&label, ui);
            p.text(
                &label,
                b[0] + ((b[2] - lw) / 2.0).round(),
                b[1] + self.px(4.8),
                ui,
                txt,
            );
        }
    }

    /// The Assistant tool in use (tests).
    #[cfg(test)]
    pub(crate) fn assist12(&self) -> Option<Assist> {
        self.twelve.assist
    }
}

#[cfg(test)]
#[path = "reader_r12_tests.rs"]
mod tests;

/// Percent-escape text for a `mailto:` field.
fn mail_escape12(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
