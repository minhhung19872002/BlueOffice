//! BlueReader's own Print dialog: printer, page range (all, current, list), copies and page
//! sizing, with a preview of the first page to print. The host lists the printers and prints
//! the [`PrintSettings`] the dialog returns.
use crate::theme::Theme;
use crate::{Key, Mods, Painter, Strings, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// How pages are sized on paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintScaling {
    /// Grow or shrink to the printable area.
    Fit,
    /// Shrink oversized pages only.
    Shrink,
    /// 100 %.
    Actual,
}

/// What to print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintSettings {
    /// Printer name.
    pub printer: String,
    /// Copies (collated).
    pub copies: u32,
    /// Page indices in print order.
    pub pages: Vec<usize>,
    /// Page sizing.
    pub scaling: PrintScaling,
}

/// Answer of a click or key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintAction {
    /// Print with these settings.
    Print(PrintSettings),
    /// Closed without printing.
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Range {
    All,
    Current,
    Pages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Printer,
    PrinterItem(usize),
    Range(Range),
    PagesText,
    Copies,
    CopiesDown,
    CopiesUp,
    Scaling(PrintScaling),
    Print,
    Cancel,
}

/// Parse a page list such as `1-3, 5, 8-` (1-based) into page indices within `count`.
pub fn parse_pages(s: &str, count: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for part in s.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let num = |v: &str| v.trim().parse::<usize>().ok();
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (
                num(a).unwrap_or(1),
                if b.trim().is_empty() {
                    Some(count)
                } else {
                    num(b)
                },
            ),
            None => match num(part) {
                Some(v) => (v, Some(v)),
                None => continue,
            },
        };
        let Some(b) = b else { continue };
        let (lo, hi) = (a.min(b).max(1), a.max(b).min(count));
        for p in lo..=hi {
            out.push(p - 1);
        }
    }
    out
}

/// The Print dialog.
pub struct PrintDialog {
    printers: Vec<String>,
    printer: usize,
    copies: u32,
    copies_text: Vec<char>,
    range: Range,
    pages_text: Vec<char>,
    /// Typing goes to the page list (`true`) or the copies box.
    focus_pages: bool,
    scaling: PrintScaling,
    count: usize,
    current: usize,
    list_open: bool,
    scale: f32,
    frame: [f32; 4],
    parts: Vec<(Part, [f32; 4])>,
    hover: Option<Part>,
    /// Page size of the preview page (points).
    preview_size: (f32, f32),
    /// Image key of the preview page, if the host has it.
    preview_key: Option<usize>,
    /// Device pixels per point of that image.
    preview_scale: f32,
    preview_rect: [f32; 4],
}

const FACE: Rgba = Rgba(0xff, 0xff, 0xff, 255);
const EDGE: Rgba = Rgba(0xc7, 0xcc, 0xd9, 255);
const FIELD_EDGE: Rgba = Rgba(0x8a, 0x8a, 0x8a, 255);
const FOCUS: Rgba = Rgba(0x00, 0x78, 0xd7, 255);
const TEXT: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const GREY_TEXT: Rgba = Rgba(0x6e, 0x6e, 0x6e, 255);
const BUTTON: Rgba = Rgba(0xf0, 0xf1, 0xf5, 255);
const BUTTON_HOVER: Rgba = Rgba(0xe1, 0xe5, 0xee, 255);
const PRIMARY: Rgba = Rgba(0x1f, 0x5f, 0xbf, 255);
const PRIMARY_HOVER: Rgba = Rgba(0x18, 0x4f, 0xa3, 255);
const PREVIEW_BG: Rgba = Rgba(0xe9, 0xea, 0xee, 255);
const ITEM_HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);

impl PrintDialog {
    /// A dialog for a document of `count` pages showing page `current`; `printers` with the
    /// `default` one chosen.
    pub fn new(printers: Vec<String>, default: Option<&str>, count: usize, current: usize) -> Self {
        let printer = default
            .and_then(|d| printers.iter().position(|p| p == d))
            .unwrap_or(0);
        Self {
            printers,
            printer,
            copies: 1,
            copies_text: vec!['1'],
            range: Range::All,
            pages_text: format!("1-{}", count.max(1)).chars().collect(),
            focus_pages: false,
            scaling: PrintScaling::Fit,
            count: count.max(1),
            current: current.min(count.saturating_sub(1)),
            list_open: false,
            scale: 1.0,
            frame: [0.0; 4],
            parts: Vec::new(),
            hover: None,
            preview_size: (612.0, 792.0),
            preview_key: None,
            preview_scale: 0.0,
            preview_rect: [0.0; 4],
        }
    }

    /// The page shown in the preview: its index, for the host to provide an image of.
    pub fn preview_page(&self) -> usize {
        self.pages().first().copied().unwrap_or(0)
    }

    /// Size of the preview box in device pixels (for the host's preview image).
    pub fn preview_box(&self) -> [f32; 4] {
        self.preview_rect
    }

    /// Show page image `key` drawn at `scale` device pixels per point, of a page of `size`
    /// points.
    pub fn set_preview(&mut self, key: Option<usize>, size: (f32, f32), scale: f32) {
        self.preview_key = key;
        self.preview_size = size;
        self.preview_scale = scale;
    }

    /// Pages to print.
    pub fn pages(&self) -> Vec<usize> {
        match self.range {
            Range::All => (0..self.count).collect(),
            Range::Current => vec![self.current],
            Range::Pages => {
                let s: String = self.pages_text.iter().collect();
                parse_pages(&s, self.count)
            }
        }
    }

    fn settings(&self) -> Option<PrintSettings> {
        let pages = self.pages();
        if pages.is_empty() {
            return None;
        }
        Some(PrintSettings {
            printer: self.printers.get(self.printer)?.clone(),
            copies: self.copies.clamp(1, 999),
            pages,
            scaling: self.scaling,
        })
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn style(&self, bold: bool) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold,
        }
    }

    /// Lay out centred in a window of `size` device pixels at `scale`.
    pub fn layout(&mut self, _m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        self.scale = scale;
        let (w, h) = (self.px(640.0), self.px(420.0));
        let f = [
            ((size.0 - w) / 2.0).round(),
            ((size.1 - h) / 2.0).round().max(0.0),
            w,
            h,
        ];
        self.frame = f;
        self.parts.clear();
        let left = f[0] + self.px(24.0);
        let col_w = self.px(330.0);
        let row = self.px(28.0);
        // Printer drop-down.
        let mut y = f[1] + self.px(70.0);
        self.parts.push((Part::Printer, [left, y, col_w, row]));
        if self.list_open {
            for i in 0..self.printers.len() {
                self.parts.push((
                    Part::PrinterItem(i),
                    [left, y + row + i as f32 * row, col_w, row],
                ));
            }
        }
        // Page range radios.
        y += row + self.px(46.0);
        for (k, r) in [Range::All, Range::Current, Range::Pages]
            .into_iter()
            .enumerate()
        {
            self.parts.push((
                Part::Range(r),
                [
                    left,
                    y + k as f32 * self.px(28.0),
                    self.px(120.0),
                    self.px(24.0),
                ],
            ));
        }
        self.parts.push((
            Part::PagesText,
            [
                left + self.px(124.0),
                y + 2.0 * self.px(28.0) - self.px(2.0),
                col_w - self.px(124.0),
                row,
            ],
        ));
        // Copies.
        y += 3.0 * self.px(28.0) + self.px(38.0);
        let cw = self.px(64.0);
        let cx = left + self.px(80.0);
        self.parts.push((Part::Copies, [cx, y, cw, row]));
        self.parts
            .push((Part::CopiesDown, [cx + cw + self.px(4.0), y, row, row]));
        self.parts
            .push((Part::CopiesUp, [cx + cw + row + self.px(8.0), y, row, row]));
        // Sizing radios, to the right of the preview.
        let px0 = left + col_w + self.px(32.0);
        let pw = f[0] + w - self.px(24.0) - px0;
        self.preview_rect = [px0, f[1] + self.px(70.0), pw, self.px(200.0)];
        let sy = self.preview_rect[1] + self.preview_rect[3] + self.px(16.0);
        for (k, s) in [
            PrintScaling::Fit,
            PrintScaling::Shrink,
            PrintScaling::Actual,
        ]
        .into_iter()
        .enumerate()
        {
            self.parts.push((
                Part::Scaling(s),
                [px0, sy + k as f32 * self.px(26.0), pw, self.px(24.0)],
            ));
        }
        // Buttons.
        let (bw, bh) = (self.px(96.0), self.px(32.0));
        let by = f[1] + h - bh - self.px(18.0);
        let bx = f[0] + w - self.px(24.0) - bw;
        self.parts.push((Part::Cancel, [bx, by, bw, bh]));
        self.parts
            .push((Part::Print, [bx - bw - self.px(10.0), by, bw, bh]));
    }

    fn part_at(&self, x: f32, y: f32) -> Option<Part> {
        let inside = |r: &[f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        // The open printer list lies over the other controls.
        if self.list_open {
            if let Some((p, _)) = self
                .parts
                .iter()
                .find(|(p, r)| matches!(p, Part::PrinterItem(_)) && inside(r))
            {
                return Some(*p);
            }
        }
        self.parts.iter().find(|(_, r)| inside(r)).map(|(p, _)| *p)
    }

    /// Whether a window position is over the dialog.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let f = self.frame;
        x >= f[0] && x < f[0] + f[2] && y >= f[1] && y < f[1] + f[3]
    }

    /// Pointer moved: `true` when the hover changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.part_at(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    fn sync_copies(&mut self) {
        self.copies_text = self.copies.to_string().chars().collect();
    }

    /// A click.
    pub fn press(&mut self, x: f32, y: f32) -> Option<PrintAction> {
        let part = self.part_at(x, y);
        if self.list_open && !matches!(part, Some(Part::PrinterItem(_) | Part::Printer)) {
            self.list_open = false;
            return None;
        }
        match part? {
            Part::Printer => self.list_open = !self.list_open,
            Part::PrinterItem(i) => {
                self.printer = i;
                self.list_open = false;
            }
            Part::Range(r) => {
                self.range = r;
                self.focus_pages = r == Range::Pages;
            }
            Part::PagesText => {
                self.range = Range::Pages;
                self.focus_pages = true;
            }
            Part::Copies => self.focus_pages = false,
            Part::CopiesDown => {
                self.copies = self.copies.saturating_sub(1).max(1);
                self.sync_copies();
            }
            Part::CopiesUp => {
                self.copies = (self.copies + 1).min(999);
                self.sync_copies();
            }
            Part::Scaling(s) => self.scaling = s,
            Part::Print => return self.settings().map(PrintAction::Print),
            Part::Cancel => return Some(PrintAction::Cancel),
        }
        None
    }

    /// Typed text: page list or copies.
    pub fn text(&mut self, s: &str) {
        if self.focus_pages {
            self.pages_text.extend(
                s.chars()
                    .filter(|c| c.is_ascii_digit() || matches!(c, ',' | '-' | ' ' | ';')),
            );
        } else {
            self.copies_text
                .extend(s.chars().filter(char::is_ascii_digit));
            self.copies_text.truncate(3);
            let v: String = self.copies_text.iter().collect();
            self.copies = v.parse().unwrap_or(1).clamp(1, 999);
        }
    }

    /// A key: Enter prints, Escape cancels, Backspace edits, Up / Down change the copies.
    pub fn key(&mut self, key: Key, _m: Mods) -> Option<PrintAction> {
        match key {
            Key::Enter => return self.settings().map(PrintAction::Print),
            Key::Escape => {
                if self.list_open {
                    self.list_open = false;
                    return None;
                }
                return Some(PrintAction::Cancel);
            }
            Key::Backspace => {
                if self.focus_pages {
                    self.pages_text.pop();
                } else {
                    self.copies_text.pop();
                    let v: String = self.copies_text.iter().collect();
                    self.copies = v.parse().unwrap_or(1).clamp(1, 999);
                }
            }
            Key::Up => {
                self.copies = (self.copies + 1).min(999);
                self.sync_copies();
            }
            Key::Down => {
                self.copies = self.copies.saturating_sub(1).max(1);
                self.sync_copies();
            }
            Key::Tab => self.focus_pages = !self.focus_pages,
            _ => {}
        }
        None
    }

    fn rect(&self, part: Part) -> [f32; 4] {
        self.parts
            .iter()
            .find(|(p, _)| *p == part)
            .map_or([0.0; 4], |(_, r)| *r)
    }

    fn radio(&self, p: &mut dyn Painter, r: [f32; 4], on: bool, label: &str) {
        let d = self.px(14.0);
        let (cx, cy) = (r[0] + d / 2.0, r[1] + r[3] / 2.0);
        p.path(
            &circle(cx, cy, d / 2.0),
            Some(FACE),
            Some((if on { FOCUS } else { FIELD_EDGE }, self.scale.max(1.0))),
        );
        if on {
            p.path(&circle(cx, cy, d / 4.5), Some(FOCUS), None);
        }
        let st = self.style(false);
        p.text(
            label,
            r[0] + d + self.px(8.0),
            cy - st.size * 0.67,
            st,
            TEXT,
        );
    }

    fn field(&self, p: &mut dyn Painter, r: [f32; 4], focused: bool, text: &str, caret: bool) {
        p.fill_rect(
            r[0],
            r[1],
            r[2],
            r[3],
            if focused { FOCUS } else { FIELD_EDGE },
        );
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, FACE);
        let st = self.style(false);
        let ty = r[1] + (r[3] - st.size * 1.33) / 2.0;
        p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
        p.text(text, r[0] + self.px(8.0), ty, st, TEXT);
        if focused && caret {
            // Approximate caret after the text (digits are about 0.55 em wide).
            let x = r[0] + self.px(8.0) + text.chars().count() as f32 * st.size * 0.55;
            p.fill_rect(
                x.round(),
                r[1] + self.px(6.0),
                1.0,
                r[3] - self.px(12.0),
                TEXT,
            );
        }
        p.pop_clip();
    }

    /// Paint over a dimmed window of `window` size.
    #[allow(clippy::too_many_lines)]
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        _t: &Theme,
        s: &Strings,
        window: (f32, f32),
        caret: bool,
    ) {
        p.fill_rect(0.0, 0.0, window.0, window.1, Rgba(0, 0, 0, 40));
        let f = self.frame;
        for (k, a) in [(8.0, 10u8), (4.0, 18), (1.0, 28)] {
            let o = self.px(k);
            p.fill_rect(
                f[0] - o,
                f[1] - o + self.px(3.0),
                f[2] + 2.0 * o,
                f[3] + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(f[0], f[1], f[2], f[3], EDGE);
        p.fill_rect(f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0, FACE);
        let left = f[0] + self.px(24.0);
        p.text(
            s.get("pdfPrint.title"),
            left,
            f[1] + self.px(18.0),
            TextStyle {
                size: self.px(16.0),
                bold: true,
            },
            TEXT,
        );
        let label = |p: &mut dyn Painter, text: &str, x: f32, y: f32| {
            p.text(text, x, y, self.style(true), TEXT);
        };
        // Printer.
        let pr = self.rect(Part::Printer);
        label(p, s.get("pdfPrint.printer"), left, pr[1] - self.px(22.0));
        let name = self
            .printers
            .get(self.printer)
            .map_or_else(|| s.get("pdfPrint.noPrinter").to_owned(), Clone::clone);
        self.field(p, pr, self.list_open, &name, false);
        // Drop-down arrow.
        let (ax, ay) = (pr[0] + pr[2] - self.px(18.0), pr[1] + pr[3] / 2.0);
        let k = self.px(4.0);
        p.path(
            &[
                PathSeg::M(ax - k, ay - k / 2.0),
                PathSeg::L(ax, ay + k / 2.0),
                PathSeg::L(ax + k, ay - k / 2.0),
            ],
            None,
            Some((TEXT, self.scale.max(1.0))),
        );
        // Page range.
        let r_all = self.rect(Part::Range(Range::All));
        label(p, s.get("pdfPrint.range"), left, r_all[1] - self.px(24.0));
        self.radio(p, r_all, self.range == Range::All, s.get("pdfPrint.all"));
        self.radio(
            p,
            self.rect(Part::Range(Range::Current)),
            self.range == Range::Current,
            s.get("pdfPrint.current"),
        );
        self.radio(
            p,
            self.rect(Part::Range(Range::Pages)),
            self.range == Range::Pages,
            s.get("pdfPrint.pages"),
        );
        let pages: String = self.pages_text.iter().collect();
        self.field(
            p,
            self.rect(Part::PagesText),
            self.focus_pages,
            &pages,
            caret,
        );
        // Copies.
        let c = self.rect(Part::Copies);
        label(p, s.get("pdfPrint.copies"), left, c[1] + self.px(5.0));
        let copies: String = self.copies_text.iter().collect();
        self.field(p, c, !self.focus_pages, &copies, caret);
        for (part, minus) in [(Part::CopiesDown, true), (Part::CopiesUp, false)] {
            let r = self.rect(part);
            let bg = if self.hover == Some(part) {
                BUTTON_HOVER
            } else {
                BUTTON
            };
            p.fill_rect(r[0], r[1], r[2], r[3], EDGE);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, bg);
            let (mx, my, l) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(5.0));
            let w = self.scale.max(1.0).round();
            p.fill_rect((mx - l).round(), (my - w / 2.0).round(), 2.0 * l, w, TEXT);
            if !minus {
                p.fill_rect((mx - w / 2.0).round(), (my - l).round(), w, 2.0 * l, TEXT);
            }
        }
        // Preview: the paper with the first page to print.
        let pv = self.preview_rect;
        p.fill_rect(pv[0], pv[1], pv[2], pv[3], PREVIEW_BG);
        let (w_pt, h_pt) = self.preview_size;
        let fit = ((pv[2] - self.px(24.0)) / w_pt).min((pv[3] - self.px(24.0)) / h_pt);
        // The page image is drawn 1:1: its scale when it fits, else the fitted size (no image).
        let k = if self.preview_key.is_some()
            && self.preview_scale > 0.0
            && self.preview_scale <= fit
        {
            self.preview_scale
        } else {
            fit
        };
        let (pw, ph) = ((w_pt * k).round(), (h_pt * k).round());
        let (x0, y0) = (
            (pv[0] + (pv[2] - pw) / 2.0).round(),
            (pv[1] + (pv[3] - ph) / 2.0).round(),
        );
        p.fill_rect(
            x0 - 1.0,
            y0 - 1.0,
            pw + 2.0,
            ph + 2.0,
            Rgba(0x77, 0x78, 0x7a, 255),
        );
        p.fill_rect(x0, y0, pw, ph, FACE);
        if let Some(key) = self
            .preview_key
            .filter(|_| (k - self.preview_scale).abs() < 1e-4)
        {
            p.page(key, x0 as i32, y0 as i32);
        }
        let n = self.pages().len();
        let count = s
            .get("pdfPrint.sheets")
            .replace("{{n}}", &(n * self.copies as usize).to_string());
        p.text(
            &count,
            pv[0],
            pv[1] - self.px(22.0),
            self.style(false),
            GREY_TEXT,
        );
        // Sizing.
        let fit = self.rect(Part::Scaling(PrintScaling::Fit));
        label(
            p,
            s.get("pdfPrint.sizing"),
            fit[0],
            fit[1] - self.px(2.0) - self.px(20.0),
        );
        for (sc, key) in [
            (PrintScaling::Fit, "pdfPrint.fit"),
            (PrintScaling::Shrink, "pdfPrint.shrink"),
            (PrintScaling::Actual, "pdfPrint.actual"),
        ] {
            self.radio(
                p,
                self.rect(Part::Scaling(sc)),
                self.scaling == sc,
                s.get(key),
            );
        }
        // Buttons.
        for (part, key, primary) in [
            (Part::Print, "pdfPrint.print", true),
            (Part::Cancel, "pdfPrint.cancel", false),
        ] {
            let r = self.rect(part);
            let hover = self.hover == Some(part);
            let (bg, fg, edge) = match (primary, hover) {
                (true, false) => (PRIMARY, FACE, PRIMARY),
                (true, true) => (PRIMARY_HOVER, FACE, PRIMARY_HOVER),
                (false, h) => (if h { BUTTON_HOVER } else { BUTTON }, TEXT, EDGE),
            };
            p.fill_rect(r[0], r[1], r[2], r[3], edge);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, bg);
            let st = self.style(primary);
            let text = s.get(key);
            let tw = text.chars().count() as f32 * st.size * 0.55;
            p.text(
                text,
                (r[0] + (r[2] - tw) / 2.0).round(),
                r[1] + (r[3] - st.size * 1.33) / 2.0,
                st,
                fg,
            );
        }
        // The open printer list on top.
        if self.list_open {
            let items: Vec<(usize, [f32; 4])> = self
                .parts
                .iter()
                .filter_map(|(p, r)| match p {
                    Part::PrinterItem(i) => Some((*i, *r)),
                    _ => None,
                })
                .collect();
            if let (Some(first), Some(last)) = (items.first(), items.last()) {
                let (x, y) = (first.1[0], first.1[1]);
                let h = last.1[1] + last.1[3] - y;
                p.fill_rect(x, y, first.1[2], h, EDGE);
                p.fill_rect(x + 1.0, y, first.1[2] - 2.0, h - 1.0, FACE);
            }
            for (i, r) in items {
                if self.hover == Some(Part::PrinterItem(i)) || i == self.printer {
                    p.fill_rect(r[0] + 1.0, r[1], r[2] - 2.0, r[3] - 1.0, ITEM_HOVER);
                }
                let st = self.style(false);
                p.text(
                    &self.printers[i],
                    r[0] + self.px(8.0),
                    r[1] + (r[3] - st.size * 1.33) / 2.0,
                    st,
                    TEXT,
                );
            }
        }
    }
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = 0.552_284_8 * r;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (text.chars().count() as f32 * style.size * 0.5, style.size)
        }
    }

    fn centre(d: &PrintDialog, part: Part) -> (f32, f32) {
        let r = d.rect(part);
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    #[test]
    fn page_lists() {
        assert_eq!(parse_pages("1-3, 5", 10), vec![0, 1, 2, 4]);
        assert_eq!(parse_pages("8-", 10), vec![7, 8, 9]);
        assert_eq!(parse_pages("4-2;x;12", 10), vec![1, 2, 3]);
        assert_eq!(parse_pages("", 3), Vec::<usize>::new());
    }

    #[test]
    fn choose_printer_range_copies_and_print() {
        let mut d = PrintDialog::new(vec!["A".into(), "B".into()], Some("B"), 12, 4);
        d.layout(&Fixed, (1500.0, 1000.0), 1.25);
        let (x, y) = centre(&d, Part::Printer);
        assert_eq!(d.press(x, y), None);
        d.layout(&Fixed, (1500.0, 1000.0), 1.25);
        let (x, y) = centre(&d, Part::PrinterItem(0));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Range(Range::Current));
        d.press(x, y);
        assert_eq!(d.pages(), vec![4]);
        let (x, y) = centre(&d, Part::PagesText);
        d.press(x, y);
        d.key(Key::Backspace, Mods::default());
        d.key(Key::Backspace, Mods::default());
        d.text("3,9");
        assert_eq!(d.pages(), vec![0, 1, 2, 8]);
        let (x, y) = centre(&d, Part::CopiesUp);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Scaling(PrintScaling::Actual));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Print);
        assert_eq!(
            d.press(x, y),
            Some(PrintAction::Print(PrintSettings {
                printer: "A".into(),
                copies: 2,
                pages: vec![0, 1, 2, 8],
                scaling: PrintScaling::Actual,
            }))
        );
        assert_eq!(
            d.key(Key::Escape, Mods::default()),
            Some(PrintAction::Cancel)
        );
    }
}
