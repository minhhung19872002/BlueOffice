//! BlueReader round 5 (a child of `reader_window`): the Document Properties dialog of File ›
//! Properties, with the reference's first tabs — Description (title, author, subject,
//! keywords, dates, creator, producer, version, location, file size, page size and count,
//! tagged, fast web view), Security (method and what is allowed) and Fonts (each font with its
//! type, encoding and whether it is embedded).
use super::{inside, ReaderWindow, MUTED, TEXT, WHITE};
use crate::{Input, Key, Painter};
use bo_pdfread::{DocInfo, FontInfo};
use bo_render::Rgba;

/// The open Properties dialog.
pub(crate) struct Props {
    tab: usize,
    /// Label, value rows of the Description tab.
    description: Vec<(String, String)>,
    /// Rows of the Security tab.
    security: Vec<(String, String)>,
    fonts: Vec<FontInfo>,
    /// Dialog, tab boxes, OK and close boxes (window pixels; set when painted).
    rect: [f32; 4],
    tabs: Vec<[f32; 4]>,
    ok: [f32; 4],
    close: [f32; 4],
}

/// `D:20261116093005+07'00'` → `2026-11-16 09:30:05`.
fn date(d: &str) -> String {
    let s = d.trim_start_matches("D:");
    let g = |a: usize, b: usize| s.get(a..b).unwrap_or("");
    if s.len() < 8 {
        return s.to_owned();
    }
    let mut out = format!("{}-{}-{}", g(0, 4), g(4, 6), g(6, 8));
    if s.len() >= 14 {
        out.push_str(&format!(" {}:{}:{}", g(8, 10), g(10, 12), g(12, 14)));
    }
    out
}

fn size_text(n: usize) -> String {
    if n >= 1 << 20 {
        format!("{:.2} MB ({n} Bytes)", n as f64 / f64::from(1 << 20))
    } else {
        format!("{:.2} KB ({n} Bytes)", n as f64 / 1024.0)
    }
}

impl ReaderWindow {
    /// Open File › Properties for the active document.
    pub(crate) fn open_properties(&mut self) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let doc = t.view.doc();
        let i: DocInfo = doc.info();
        let yes_no = |b: bool| {
            if b {
                self.t("pdfProps.yes")
            } else {
                self.t("pdfProps.no")
            }
        };
        let (pw, ph) = t.view.page_size(t.view.current_page());
        let location = t
            .path
            .as_ref()
            .and_then(|p| p.parent())
            .map_or_else(String::new, |p| p.display().to_string());
        let description = vec![
            (self.t("pdfProps.file"), t.name.clone()),
            (self.t("pdfProps.title"), i.title.clone()),
            (self.t("pdfProps.author"), i.author.clone()),
            (self.t("pdfProps.subject"), i.subject.clone()),
            (self.t("pdfProps.keywords"), i.keywords.clone()),
            (self.t("pdfProps.created"), date(&i.created)),
            (self.t("pdfProps.modified"), date(&i.modified)),
            (self.t("pdfProps.creator"), i.creator.clone()),
            (self.t("pdfProps.producer"), i.producer.clone()),
            (self.t("pdfProps.version"), i.version.clone()),
            (self.t("pdfProps.location"), location),
            (self.t("pdfProps.size"), size_text(doc.bytes().len())),
            (
                self.t("pdfProps.pageSize"),
                format!(
                    "{:.2} × {:.2} in ({:.0} × {:.0} mm)",
                    pw / 72.0,
                    ph / 72.0,
                    pw / 72.0 * 25.4,
                    ph / 72.0 * 25.4
                ),
            ),
            (self.t("pdfProps.pages"), doc.page_count().to_string()),
            (self.t("pdfProps.tagged"), yes_no(i.tagged)),
            (self.t("pdfProps.fastWeb"), yes_no(i.linearized)),
        ];
        let allowed = |bit: u32| i.permissions.is_none_or(|p| p & (1 << (bit - 1)) != 0);
        let security = vec![
            (
                self.t("pdfProps.method"),
                if i.security.is_empty() {
                    self.t("pdfProps.noSecurity")
                } else {
                    i.security.clone()
                },
            ),
            (self.t("pdfProps.printing"), yes_no(allowed(3))),
            (self.t("pdfProps.changing"), yes_no(allowed(4))),
            (self.t("pdfProps.copying"), yes_no(allowed(5))),
            (self.t("pdfProps.commenting"), yes_no(allowed(6))),
            (self.t("pdfProps.filling"), yes_no(allowed(6) || allowed(9))),
            (self.t("pdfProps.assembly"), yes_no(allowed(11))),
        ];
        self.tabs5.props = Some(Props {
            tab: 0,
            description,
            security,
            fonts: doc.font_list(),
            rect: [0.0; 4],
            tabs: Vec::new(),
            ok: [0.0; 4],
            close: [0.0; 4],
        });
    }

    /// Input while the Properties dialog is open.
    pub(crate) fn route_props(&mut self, input: &Input) -> Option<bool> {
        let d = self.tabs5.props.as_mut()?;
        match input {
            Input::Press { x, y, .. } => {
                if inside(d.ok, *x, *y) || inside(d.close, *x, *y) {
                    self.tabs5.props = None;
                } else if let Some(k) = d.tabs.iter().position(|r| inside(*r, *x, *y)) {
                    d.tab = k;
                }
                Some(true)
            }
            Input::Key(Key::Escape | Key::Enter, _) => {
                self.tabs5.props = None;
                Some(true)
            }
            Input::Key(Key::Tab, m) => {
                d.tab = if m.shift { d.tab + 2 } else { d.tab + 1 } % 3;
                d.tab %= 3;
                Some(true)
            }
            Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Blink => None,
            _ => Some(true),
        }
    }

    /// The Properties dialog over the window.
    pub(crate) fn paint_doc_props(&mut self, p: &mut dyn Painter) {
        let Some(mut d) = self.tabs5.props.take() else {
            return;
        };
        let (w, h) = self.size;
        p.fill_rect(0.0, 0.0, w, h, Rgba(0, 0, 0, 40));
        let (dw, dh) = (self.px(560.0), self.px(520.0));
        let r = [((w - dw) / 2.0).round(), ((h - dh) / 2.0).round(), dw, dh];
        d.rect = r;
        let edge = self.look.edge;
        p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, edge);
        p.fill_rect(r[0], r[1], r[2], r[3], WHITE);
        let title = self.style(15.0, false);
        p.text(
            &self.t("pdfProps.dialog"),
            r[0] + self.px(16.0),
            r[1] + self.px(10.0),
            title,
            TEXT,
        );
        // Close cross.
        let cs = self.px(28.0);
        d.close = [r[0] + r[2] - cs - self.px(4.0), r[1] + self.px(4.0), cs, cs];
        let (cx, cy) = (d.close[0] + cs / 2.0, d.close[1] + cs / 2.0);
        let k = self.px(5.0);
        p.path(
            &[
                bo_render::PathSeg::M(cx - k, cy - k),
                bo_render::PathSeg::L(cx + k, cy + k),
                bo_render::PathSeg::M(cx + k, cy - k),
                bo_render::PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((TEXT, 1.0)),
        );
        // Tabs.
        let ts = self.style(14.0, false);
        let mut x = r[0] + self.px(16.0);
        let ty = r[1] + self.px(40.0);
        d.tabs.clear();
        for (k, key) in [
            "pdfProps.description",
            "pdfProps.security",
            "pdfProps.fonts",
        ]
        .iter()
        .enumerate()
        {
            let label = self.t(key);
            let lw = self
                .width_of(&label, ts)
                .max(label.chars().count() as f32 * ts.size * 0.5);
            let tr = [x, ty, lw + self.px(20.0), self.px(26.0)];
            if k == d.tab {
                p.fill_rect(
                    tr[0],
                    tr[1] + tr[3] - self.px(2.4),
                    tr[2],
                    self.px(2.4),
                    self.theme.accent,
                );
            }
            p.text(
                &label,
                tr[0] + self.px(10.0),
                tr[1] + ((tr[3] - ts.size * 1.33) / 2.0).round(),
                ts,
                if k == d.tab {
                    self.theme.accent_text
                } else {
                    TEXT
                },
            );
            d.tabs.push(tr);
            x += tr[2] + self.px(4.0);
        }
        p.fill_rect(
            r[0] + self.px(16.0),
            ty + self.px(26.0),
            r[2] - self.px(32.0),
            1.0,
            edge,
        );
        // Content.
        let body = self.style(13.0, false);
        let lh = self.px(22.0);
        let mut y = ty + self.px(38.0);
        let left = r[0] + self.px(24.0);
        let val_x = r[0] + self.px(190.0);
        p.push_clip(r[0], y, r[2], r[3] - (y - r[1]) - self.px(52.0));
        match d.tab {
            0 | 1 => {
                let rows = if d.tab == 0 {
                    &d.description
                } else {
                    &d.security
                };
                for (label, value) in rows {
                    p.text(&format!("{label}:"), left, y, body, MUTED);
                    p.text(value, val_x, y, body, TEXT);
                    y += lh;
                }
            }
            _ => {
                if d.fonts.is_empty() {
                    p.text(&self.t("pdfProps.noFonts"), left, y, body, MUTED);
                }
                for f in &d.fonts {
                    p.text(&f.name, left, y, body, TEXT);
                    y += lh * 0.9;
                    let mut detail = format!("{}: {}", self.t("pdfProps.fontType"), f.kind);
                    if !f.encoding.is_empty() {
                        detail.push_str(&format!(
                            "   {}: {}",
                            self.t("pdfProps.encoding"),
                            f.encoding
                        ));
                    }
                    detail.push_str("   ");
                    detail.push_str(&self.t(if f.embedded {
                        "pdfProps.embedded"
                    } else {
                        "pdfProps.notEmbedded"
                    }));
                    p.text(&detail, left + self.px(16.0), y, body, MUTED);
                    y += lh;
                }
            }
        }
        p.pop_clip();
        // OK.
        let (bw, bh) = (self.px(88.0), self.px(28.0));
        d.ok = [
            r[0] + r[2] - bw - self.px(16.0),
            r[1] + r[3] - bh - self.px(14.0),
            bw,
            bh,
        ];
        let o = d.ok;
        p.fill_rect(o[0], o[1], o[2], o[3], self.theme.accent_strong);
        let label = self.t("pdfReader.ok");
        let lw = self
            .width_of(&label, body)
            .max(label.len() as f32 * body.size * 0.5);
        p.text(
            &label,
            o[0] + ((o[2] - lw) / 2.0).round(),
            o[1] + ((o[3] - body.size * 1.33) / 2.0).round(),
            body,
            WHITE,
        );
        self.tabs5.props = Some(d);
    }
}
