//! Insert › Link (Ctrl+K) as the reference's Insert Hyperlink dialog (round 49, measured at
//! 125 %: 964 × 432 px, `show/sheet-48`): the "Link to" bar (Existing File or Web Page,
//! Place in This Document, Create New Document greyed, E-mail Address; own icons), Text to
//! display, ScreenTip…, and each kind's page. The link is a real cell hyperlink (engine
//! `setHyperlink`, §18.3.1.47), Edit Hyperlink starts from the cell's link.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::Painter;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// The "Link to" bar's entries (label key, fallback) and their pages.
const KINDS: [(&str, &str); 4] = [
    ("sheetLink.existing", "Existing File or Web Page"),
    ("sheetLink.place", "Place in This Document"),
    ("sheetLink.newDoc", "Create New Document"),
    ("sheetLink.email", "E-mail Address"),
];
/// The bar: left, top, width, height; each entry's height.
const BAR: [f32; 4] = [18.0, 74.0, 118.0, 338.0];
const ENTRY_H: f32 = 78.0;

impl SheetWindow {
    /// Insert › Link: the hyperlink of the active cell.
    pub(super) fn open_hyperlink(&mut self) -> bool {
        if self.view.editing() {
            self.view.commit();
        }
        let (r, c) = self.view.active();
        let sheet = self.view.sheet();
        let now = self
            .view
            .query(json!({ "cmd": "hyperlink", "sheet": sheet, "row": r, "col": c }))
            .unwrap_or(Value::Null);
        let s = |k: &str| now.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let shown = if now.is_null() {
            self.view
                .engine()
                .cell(sheet, r, c)
                .map(|d| d.display)
                .unwrap_or_default()
        } else {
            s("display")
        };
        let target = s("target");
        let (page, addr, mail, subject) = match target.strip_prefix("mailto:") {
            Some(m) => {
                let (to, q) = m.split_once('?').unwrap_or((m, ""));
                let subj = q.strip_prefix("subject=").unwrap_or("").replace("%20", " ");
                (3, String::new(), to.to_owned(), subj)
            }
            None if !s("location").is_empty() => (1, String::new(), String::new(), String::new()),
            None => (0, target, String::new(), String::new()),
        };
        let (loc_sheet, loc_cell) = match s("location").split_once('!') {
            Some((a, b)) => (a.trim_matches('\'').to_owned(), b.to_owned()),
            None => (
                String::new(),
                if s("location").is_empty() {
                    "A1".into()
                } else {
                    s("location")
                },
            ),
        };
        let names: Vec<String> = self
            .view
            .query(json!({ "cmd": "nameList" }))
            .and_then(|v| {
                v.get("names")
                    .or(Some(&v))
                    .and_then(Value::as_array)
                    .cloned()
            })
            .unwrap_or_default()
            .iter()
            .filter_map(|n| n.get("name").and_then(Value::as_str).map(str::to_owned))
            .collect();
        let t = |k: &str, f: &str| self.t(k, f);
        let title = if now.is_null() {
            t("sheetInsert.linkTitle", "Insert Hyperlink")
        } else {
            t("sheetLink.editTitle", "Edit Hyperlink")
        };
        let mut f = Form::new(&title, (964.0, 432.0), vec![]);
        let edit = |v: &str| Kind::Edit {
            value: v.chars().collect(),
            caret: v.chars().count(),
        };
        f.add(
            "linkL",
            None,
            [18.0, 50.0, 118.0, 22.0],
            Kind::Label(t("sheetLink.linkTo", "Link to:")),
        );
        f.add("bar", None, BAR, Kind::Canvas);
        f.add(
            "textL",
            None,
            [151.0, 50.0, 95.0, 22.0],
            Kind::Label(t("sheetInsert.linkText", "Text to display:")),
        );
        f.set_access("textL", 0);
        f.add("text", None, [244.0, 47.0, 573.0, 26.0], edit(&shown));
        f.add(
            "tip",
            None,
            [832.0, 46.0, 113.0, 28.0],
            Kind::Button {
                label: t("sheetLink.screenTip", "ScreenTip..."),
                default: false,
            },
        );
        f.set_access("tip", 6);
        f.add("rule", None, [151.0, 86.0, 666.0, 1.0], Kind::Canvas);
        // Page 0: web page or file.
        let p = Some(0);
        f.add(
            "browseL",
            p,
            [151.0, 104.0, 660.0, 22.0],
            Kind::Label(t("sheetLink.recent", "Addresses used in this workbook:")),
        );
        let used = self.used_link_targets();
        f.add(
            "used",
            p,
            [151.0, 126.0, 666.0, 200.0],
            Kind::List {
                items: used,
                sel: None,
                top: 0,
            },
        );
        f.add(
            "addrL",
            p,
            [151.0, 341.0, 95.0, 22.0],
            Kind::Label(t("sheetInsert.linkAddress", "Address:")),
        );
        f.set_access("addrL", 2);
        f.add("addr", p, [248.0, 338.0, 569.0, 26.0], edit(&addr));
        // Page 1: a place in this workbook.
        let p = Some(1);
        f.add(
            "cellL",
            p,
            [151.0, 104.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.cellRef", "Type the cell reference:")),
        );
        f.set_access("cellL", 0);
        f.add("cell", p, [151.0, 126.0, 400.0, 26.0], edit(&loc_cell));
        f.add(
            "placeL",
            p,
            [151.0, 164.0, 400.0, 22.0],
            Kind::Label(t(
                "sheetLink.selectPlace",
                "Or select a place in this document:",
            )),
        );
        f.set_access("placeL", 1);
        let mut places = self
            .view
            .sheet_names()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let pick = places.iter().position(|n| *n == loc_sheet).unwrap_or(sheet);
        let n_sheets = places.len();
        places.extend(names);
        f.add(
            "places",
            p,
            [151.0, 186.0, 666.0, 178.0],
            Kind::List {
                items: places,
                sel: Some(pick.min(n_sheets.saturating_sub(1))),
                top: 0,
            },
        );
        // Page 3: e-mail.
        let p = Some(3);
        f.add(
            "mailL",
            p,
            [151.0, 104.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.mail", "E-mail address:")),
        );
        f.set_access("mailL", 0);
        f.add("mail", p, [151.0, 126.0, 666.0, 26.0], edit(&mail));
        f.add(
            "subjL",
            p,
            [151.0, 164.0, 400.0, 22.0],
            Kind::Label(t("sheetLink.subject", "Subject:")),
        );
        f.set_access("subjL", 1);
        f.add("subject", p, [151.0, 186.0, 666.0, 26.0], edit(&subject));
        f.add("rule2", None, [151.0, 374.0, 794.0, 1.0], Kind::Canvas);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [706.0, 384.0, 111.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [832.0, 384.0, 113.0, 30.0];
        }
        f.set_page(page);
        f.focus(match page {
            1 => "cell",
            3 => "mail",
            _ => "addr",
        });
        self.link_tip = s("tooltip");
        self.form = Some(f);
        self.form_kind = FormKind::Link;
        true
    }

    /// External addresses of the workbook's links (newest first, once each).
    fn used_link_targets(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(wb) = self.view.engine().workbook() {
            for ws in &wb.sheets {
                for h in ws.hyperlinks.iter().rev() {
                    if let Some(t) = h.target.as_ref().filter(|t| !t.starts_with("mailto:")) {
                        if !out.contains(t) {
                            out.push(t.clone());
                        }
                    }
                }
            }
        }
        out
    }

    /// Events of Insert Hyperlink and its ScreenTip box.
    pub(super) fn link_event(&mut self, ev: &Event) {
        if self.form_kind == FormKind::LinkTip {
            match ev {
                Event::Ok | Event::Clicked("ok") => {
                    self.link_tip = self
                        .form
                        .as_ref()
                        .map(|f| f.text("tipText"))
                        .unwrap_or_default();
                    self.link_back();
                }
                Event::Cancel | Event::Clicked("cancel") => self.link_back(),
                _ => {}
            }
            return;
        }
        let Some(f) = self.form.as_mut() else {
            return;
        };
        match ev {
            Event::Cancel | Event::Clicked("cancel") => self.form = None,
            Event::Canvas("bar", _, y) => {
                let i = ((y / ENTRY_H).floor().max(0.0) as usize).min(3);
                // Create New Document is not available.
                if i != 2 {
                    f.set_page(i);
                    f.focus(match i {
                        1 => "cell",
                        3 => "mail",
                        _ => "addr",
                    });
                }
            }
            Event::Changed("used") | Event::Clicked("used") => {
                let pick = match f.get("used").map(|c| &c.kind) {
                    Some(Kind::List { items, sel, .. }) => sel.and_then(|i| items.get(i)).cloned(),
                    _ => None,
                };
                if let Some(a) = pick {
                    f.set_text("addr", &a);
                }
            }
            Event::Clicked("tip") => self.open_link_tip(),
            Event::Ok | Event::Clicked("ok") => {
                let shown = f.text("text");
                let (target, location) = match f.tab {
                    1 => {
                        let place = match f.get("places").map(|c| &c.kind) {
                            Some(Kind::List { items, sel, .. }) => {
                                sel.and_then(|i| items.get(i)).cloned().unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        let sheets = self.view.sheet_names().len();
                        let at = f.sel("places").unwrap_or(0);
                        let loc = if at < sheets {
                            let q = if place.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                place
                            } else {
                                format!("'{}'", place.replace('\'', "''"))
                            };
                            format!("{q}!{}", f.text("cell").trim())
                        } else {
                            place
                        };
                        (None, Some(loc))
                    }
                    3 => {
                        let mail = f.text("mail");
                        let mail = mail.trim().trim_start_matches("mailto:");
                        if mail.is_empty() {
                            (None, None)
                        } else {
                            let subj = f.text("subject");
                            let mut t = format!("mailto:{mail}");
                            if !subj.trim().is_empty() {
                                t.push_str(&format!(
                                    "?subject={}",
                                    subj.trim().replace(' ', "%20")
                                ));
                            }
                            (Some(t), None)
                        }
                    }
                    _ => {
                        let a = f.text("addr").trim().to_owned();
                        // A bare host gets the web scheme, as the reference adds it.
                        let a = if a.starts_with("www.") {
                            format!("http://{a}")
                        } else {
                            a
                        };
                        ((!a.is_empty()).then_some(a), None)
                    }
                };
                self.form = None;
                if target.is_none() && location.is_none() {
                    return;
                }
                let (r, c) = self.view.active();
                let sheet = self.view.sheet();
                let display = (!shown.trim().is_empty()).then_some(shown);
                self.view.run(json!({
                    "cmd": "setHyperlink", "sheet": sheet, "row": r, "col": c,
                    "target": target, "location": location, "display": display,
                    "tooltip": self.link_tip,
                }));
            }
            _ => {}
        }
    }

    fn open_link_tip(&mut self) {
        let t = |k: &str, f: &str| self.t(k, f);
        let mut f = Form::new(
            &t("sheetLink.tipTitle", "Set Hyperlink ScreenTip"),
            (420.0, 190.0),
            vec![],
        );
        f.add(
            "tipL",
            None,
            [18.0, 48.0, 380.0, 22.0],
            Kind::Label(t("sheetLink.tipText", "ScreenTip text:")),
        );
        f.set_access("tipL", 0);
        let tip = self.link_tip.clone();
        f.add(
            "tipText",
            None,
            [18.0, 72.0, 384.0, 26.0],
            Kind::Edit {
                value: tip.chars().collect(),
                caret: tip.chars().count(),
            },
        );
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [176.0, 140.0, 107.0, 30.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [295.0, 140.0, 107.0, 30.0];
        }
        f.focus("tipText");
        if let Some(old) = self.form.take() {
            self.form_stash = Some((old, self.form_kind));
        }
        self.form = Some(f);
        self.form_kind = FormKind::LinkTip;
    }

    fn link_back(&mut self) {
        if let Some((f, k)) = self.form_stash.take() {
            self.form = Some(f);
            self.form_kind = k;
        }
    }

    /// The "Link to" bar: white, framed, the chosen entry shaded, own icons over two-line
    /// captions.
    pub(super) fn paint_link(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let k = f.scale_k();
        if id.starts_with("rule") {
            p.fill_rect(r[0], r[1], r[2], 1.0, Rgba(0xDC, 0xDC, 0xDC, 255));
            return;
        }
        if id != "bar" {
            return;
        }
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x82, 0x87, 0x90, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let font = f.font();
        let line = font.size * 1.33;
        for (i, (key, fb)) in KINDS.iter().enumerate() {
            let y = r[1] + (i as f32 * ENTRY_H * k).round();
            let h = (ENTRY_H * k).round();
            if i == f.tab {
                p.fill_rect(
                    r[0] + 2.0,
                    y + 2.0,
                    r[2] - 4.0,
                    h - 4.0,
                    Rgba(0xCC, 0xE4, 0xF7, 255),
                );
            }
            let ink = if i == 2 {
                Rgba(0x83, 0x83, 0x83, 255)
            } else {
                Rgba(0, 0, 0, 255)
            };
            // Icon: a page with a globe / a sheet with an arrow / a new page / an envelope.
            let (cx, cy) = ((r[0] + r[2] / 2.0).round(), y + (20.0 * k).round());
            let a = (9.0 * k).round();
            let accent = Rgba(0x2E, 0x75, 0xB6, 255);
            if i == 3 {
                {
                    p.fill_rect(cx - a, cy - a * 0.7, 2.0 * a, 1.4 * a, ink);
                    p.fill_rect(
                        cx - a + 1.0,
                        cy - a * 0.7 + 1.0,
                        2.0 * a - 2.0,
                        1.4 * a - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                    p.path(
                        &[
                            PathSeg::M(cx - a, cy - a * 0.7),
                            PathSeg::L(cx, cy + 1.0),
                            PathSeg::L(cx + a, cy - a * 0.7),
                        ],
                        None,
                        Some((accent, k.max(1.0))),
                    );
                }
            } else {
                {
                    p.fill_rect(cx - a * 0.8, cy - a, 1.6 * a, 2.0 * a, ink);
                    p.fill_rect(
                        cx - a * 0.8 + 1.0,
                        cy - a + 1.0,
                        1.6 * a - 2.0,
                        2.0 * a - 2.0,
                        Rgba(255, 255, 255, 255),
                    );
                    let mark = if i == 2 {
                        Rgba(0xF2, 0xB7, 0x00, 255)
                    } else {
                        accent
                    };
                    p.fill_rect(cx - a * 0.3, cy - a * 0.2, a * 0.9, a * 0.9, mark);
                }
            }
            // Caption wrapped on two lines at the space nearest the middle.
            let label = self.t(key, fb);
            let words: Vec<&str> = label.split(' ').collect();
            let lines: Vec<String> = if words.len() > 2 {
                let mid = words.len().div_ceil(2);
                vec![words[..mid].join(" "), words[mid..].join(" ")]
            } else {
                vec![label.clone()]
            };
            for (n, l) in lines.iter().enumerate() {
                let w = f.width(l);
                p.text_font(
                    l,
                    (r[0] + (r[2] - w) / 2.0).round(),
                    (cy + a + 6.0 * k + n as f32 * line).round(),
                    &font,
                    ink,
                );
            }
        }
    }

    /// Texts the bar draws (measured at layout time).
    pub(super) fn link_strings(&self) -> Vec<String> {
        KINDS
            .iter()
            .flat_map(|(key, fb)| {
                let label = self.t(key, fb);
                let words: Vec<String> = label.split(' ').map(str::to_owned).collect();
                let mid = words.len().div_ceil(2);
                vec![words[..mid].join(" "), words[mid..].join(" "), label]
            })
            .collect()
    }
}
