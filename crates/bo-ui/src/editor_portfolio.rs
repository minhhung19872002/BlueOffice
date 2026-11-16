//! PDF portfolio details view (round 5), as the reference editor shows a portfolio when it
//! opens: instead of the cover page, a list of the embedded files with Name, Description,
//! Modified and Size columns (125 %: path row 32 px, header 31 px, rows 46 px, columns from the
//! right edge at −358 / −210 / −85 px). Read only: a click selects, a double click opens a PDF
//! in a new tab or saves another file; Convert › PDF Portfolio switches to the cover page and
//! back.
use crate::editor_window::{inside, EditorWindow};
use crate::reader_icons::{self, Ink};
use crate::reader_window::ReaderRequest;
use crate::{Input, Key, Painter, TextStyle};
use bo_pdfread::EmbeddedFile;
use bo_render::Rgba;

/// The details view of the portfolio open in document tab `tab`.
#[derive(Debug, Clone, Default)]
pub(crate) struct PortfolioView {
    pub tab: usize,
    pub files: Vec<EmbeddedFile>,
    pub selected: Option<usize>,
    /// Row rectangles from the last paint (window pixels).
    pub rows: Vec<[f32; 4]>,
    /// Showing the cover page instead of the list.
    pub cover: bool,
}

const TEXT: Rgba = Rgba(51, 51, 51, 255);
const LINE: Rgba = Rgba(214, 214, 214, 255);
const BACK: Rgba = Rgba(171, 171, 171, 255);
const PATH_BG: Rgba = Rgba(250, 250, 252, 255);

/// "31 Bytes" / "98.91 KB" as the reference writes sizes.
pub(crate) fn size_label(n: usize) -> String {
    if n < 1024 {
        format!("{n} Bytes")
    } else if n < 1024 * 1024 {
        format!("{:.2} KB", n as f32 / 1024.0)
    } else {
        format!("{:.2} MB", n as f32 / 1024.0 / 1024.0)
    }
}

impl EditorWindow {
    /// A document was opened: a portfolio shows its details view.
    pub(crate) fn portfolio_opened(&mut self) {
        let (portfolio, files) = self
            .with_doc(|d| Ok((d.is_portfolio(), d.embedded_files())))
            .unwrap_or_default();
        self.five.portfolio = (portfolio && !files.is_empty()).then(|| PortfolioView {
            tab: self.r.active_tab(),
            files,
            ..PortfolioView::default()
        });
    }

    /// The details view is showing.
    pub(crate) fn portfolio_shown(&self) -> bool {
        self.five.portfolio.as_ref().is_some_and(|p| {
            !p.cover && p.tab == self.r.active_tab() && self.showing_doc() && !self.organize_on()
        })
    }

    /// Convert › PDF Portfolio on a portfolio: switch between the list and the cover page.
    pub(crate) fn portfolio_toggle(&mut self) -> bool {
        let tab = self.r.active_tab();
        match self.five.portfolio.as_mut() {
            Some(p) if p.tab == tab => {
                p.cover = !p.cover;
                true
            }
            _ => false,
        }
    }

    /// Open (a PDF, in a new tab) or save (another file) embedded file `i`.
    fn portfolio_open(&mut self, i: usize) {
        let Some(name) = self
            .five
            .portfolio
            .as_ref()
            .and_then(|p| p.files.get(i))
            .map(|f| f.name.clone())
        else {
            return;
        };
        match self.with_doc(|d| d.embedded_file_bytes(i)) {
            Ok(bytes) if bytes.starts_with(b"%PDF") => {
                self.push(ReaderRequest::OpenDocument { name, bytes });
            }
            Ok(bytes) => self.push(ReaderRequest::SaveBytes { name, bytes }),
            Err(e) => self.r.set_status(&e),
        }
    }

    /// Input over the details view; true when used.
    pub(crate) fn portfolio_input(&mut self, input: &Input) -> bool {
        if !self.portfolio_shown() {
            return false;
        }
        let area = self.doc_area();
        match *input {
            Input::Press { x, y, count, .. } if inside(area, x, y) => {
                let hit = self
                    .five
                    .portfolio
                    .as_ref()
                    .and_then(|p| p.rows.iter().position(|r| inside(*r, x, y)));
                if let Some(p) = self.five.portfolio.as_mut() {
                    p.selected = hit;
                }
                if let (Some(i), true) = (hit, count >= 2) {
                    self.portfolio_open(i);
                }
                true
            }
            Input::Key(Key::Enter, _) => {
                if let Some(i) = self.five.portfolio.as_ref().and_then(|p| p.selected) {
                    self.portfolio_open(i);
                }
                true
            }
            Input::Key(k @ (Key::Up | Key::Down), _) => {
                if let Some(p) = self.five.portfolio.as_mut() {
                    let n = p.files.len();
                    let cur = p.selected.unwrap_or(0);
                    p.selected = Some(if k == Key::Up {
                        cur.saturating_sub(1)
                    } else {
                        (cur + 1).min(n.saturating_sub(1))
                    });
                }
                true
            }
            Input::Wheel(_) if inside(area, self.pointer().0, self.pointer().1) => true,
            _ => false,
        }
    }

    /// Paint the details view over the document area.
    pub(crate) fn paint_portfolio(&mut self, p: &mut dyn Painter) {
        if !self.portfolio_shown() {
            return;
        }
        let k = self.ui_scale();
        let px = |v: f32| (v * k).round();
        let st = TextStyle {
            size: 15.0 / 1.375 * k,
            bold: false,
        };
        let a = self.doc_area();
        let (x0, y0, w, h) = (a[0], a[1], a[2], a[3]);
        let right = x0 + w;
        p.fill_rect(x0, y0, w, h, BACK);
        // Path row ("Home ▸").
        let path_h = px(26.0);
        p.fill_rect(x0, y0, w, path_h, PATH_BG);
        p.text(
            &self.t("pdfEditor.portfolioHome"),
            x0 + px(12.0),
            y0 + px(6.0),
            st,
            TEXT,
        );
        // Header row in the accent.
        let hy = y0 + path_h + px(6.0);
        let hh = px(25.0);
        let accent = self.accent();
        p.fill_rect(x0, hy, w, hh, accent);
        let cols = [
            (x0 + px(94.0), "pdfEditor.colName"),
            (right - px(286.0), "pdfEditor.colDescription"),
            (right - px(168.0), "pdfEditor.colModified"),
            (right - px(68.0), "pdfEditor.colSize"),
        ];
        let white = Rgba(255, 255, 255, 255);
        for (i, (cx, key)) in cols.iter().enumerate() {
            if i > 0 {
                p.fill_rect(cx - px(12.0), hy, 1.0, hh, Rgba(255, 255, 255, 120));
            }
            p.text(&self.t(key), *cx, hy + px(6.0), st, white);
        }
        // Rows.
        let rh = px(37.0);
        let mut rows = Vec::new();
        let (files, sel) = self
            .five
            .portfolio
            .as_ref()
            .map(|v| (v.files.clone(), v.selected))
            .unwrap_or_default();
        let unknown = self.t("pdfEditor.unknown");
        for (i, f) in files.iter().enumerate() {
            let y = hy + hh + rh * i as f32;
            let r = [x0, y, w, rh];
            let bg = if sel == Some(i) {
                Rgba(accent.0, accent.1, accent.2, 40)
            } else {
                Rgba(255, 255, 255, 255)
            };
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
            p.fill_rect(r[0], r[1], r[2], r[3], bg);
            p.fill_rect(x0, y + rh - 1.0, w, 1.0, LINE);
            let icon = if f.name.to_ascii_lowercase().ends_with(".pdf") {
                "toOther"
            } else {
                "summary"
            };
            reader_icons::draw_weight(
                p,
                icon,
                x0 + px(28.0),
                y + px(5.0),
                px(27.0),
                Ink {
                    ink: Rgba(110, 110, 110, 255),
                    accent: Rgba(110, 110, 110, 255),
                },
                0.8,
            );
            let ty = y + px(11.0);
            p.text(&f.name, cols[0].0, ty, st, TEXT);
            p.text(&f.description, cols[1].0, ty, st, TEXT);
            let m = if f.modified.is_empty() {
                unknown.clone()
            } else {
                f.modified.clone()
            };
            p.text(&m, cols[2].0, ty, st, TEXT);
            p.text(&size_label(f.size), cols[3].0, ty, st, TEXT);
            rows.push(r);
        }
        if let Some(v) = self.five.portfolio.as_mut() {
            v.rows = rows;
        }
    }
}
