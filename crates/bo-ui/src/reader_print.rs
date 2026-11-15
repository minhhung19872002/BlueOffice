//! BlueReader's own Print dialog, laid out like the reference reader's (measured at 125 %:
//! 1535 × 870 px): printer and copies with collate / grayscale, Print Range (current page, all
//! pages, a page list with a subset and reverse order), Print Handling (none, fit to or reduce
//! to the printer margins), auto-rotate / auto-centre, and a preview with page navigation. The
//! host lists the printers and prints the [`PrintSettings`] the dialog returns.
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
    /// Copies.
    pub copies: u32,
    /// Copies collated (each copy all pages) rather than each page `copies` times.
    pub collate: bool,
    /// Page indices in print order.
    pub pages: Vec<usize>,
    /// Page sizing.
    pub scaling: PrintScaling,
    /// Print in shades of grey.
    pub grayscale: bool,
    /// Turn pages to the paper's orientation.
    pub auto_rotate: bool,
    /// Centre pages on the paper (else at the top-left of the printable area).
    pub auto_center: bool,
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
    Current,
    All,
    Pages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Printer,
    PrinterItem(usize),
    Copies,
    CopiesUp,
    CopiesDown,
    Collate,
    Gray,
    Range(Range),
    PagesText,
    Subset,
    Reverse,
    Scaling(PrintScaling),
    Rotate,
    Center,
    PrevPage,
    NextPage,
    Ok,
    Cancel,
    Close,
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
    collate: bool,
    gray: bool,
    range: Range,
    pages_text: Vec<char>,
    /// 0 all pages in range, 1 odd, 2 even.
    subset: u8,
    reverse: bool,
    scaling: PrintScaling,
    rotate: bool,
    center: bool,
    /// Typing goes to the page list (`true`) or the copies box.
    focus_pages: bool,
    count: usize,
    current: usize,
    /// Index into the pages to print of the page shown in the preview.
    preview_at: usize,
    list_open: bool,
    scale: f32,
    frame: [f32; 4],
    parts: Vec<(Part, [f32; 4])>,
    hover: Option<Part>,
    preview_size: (f32, f32),
    preview_key: Option<usize>,
    preview_scale: f32,
    preview_rect: [f32; 4],
}

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const CAPTION: Rgba = Rgba(242, 244, 243, 255);
const GROUP: Rgba = Rgba(220, 220, 220, 255);
const FIELD: Rgba = Rgba(188, 188, 188, 255);
const FIELD_BG: Rgba = Rgba(253, 253, 253, 255);
const FOCUS: Rgba = Rgba(0x00, 0x78, 0xd7, 255);
const TEXT: Rgba = Rgba(0x1a, 0x1a, 0x1a, 255);
const GREY_TEXT: Rgba = Rgba(0x9a, 0x9a, 0x9a, 255);
const BUTTON: Rgba = Rgba(0xfd, 0xfd, 0xfd, 255);
const BUTTON_EDGE: Rgba = Rgba(0xd0, 0xd0, 0xd0, 255);
const BUTTON_HOVER: Rgba = Rgba(0xe5, 0xf1, 0xfb, 255);
const PAGE_FRAME: Rgba = Rgba(102, 102, 102, 255);
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
            collate: true,
            gray: false,
            range: Range::All,
            pages_text: format!("1 - {}", count.max(1)).chars().collect(),
            subset: 0,
            reverse: false,
            scaling: PrintScaling::Fit,
            rotate: true,
            center: true,
            focus_pages: false,
            count: count.max(1),
            current: current.min(count.saturating_sub(1)),
            preview_at: 0,
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

    /// The page shown in the preview.
    pub fn preview_page(&self) -> usize {
        let pages = self.pages();
        pages
            .get(self.preview_at.min(pages.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    }

    /// The preview box (device pixels): the host draws the preview page to fit it.
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

    /// Pages to print, in order.
    pub fn pages(&self) -> Vec<usize> {
        let mut v: Vec<usize> = match self.range {
            Range::All => (0..self.count).collect(),
            Range::Current => vec![self.current],
            Range::Pages => {
                let s: String = self.pages_text.iter().collect();
                parse_pages(&s, self.count)
            }
        };
        // Subset by page number (1-based): odd or even.
        match self.subset {
            1 => v.retain(|p| p % 2 == 0),
            2 => v.retain(|p| p % 2 == 1),
            _ => {}
        }
        if self.reverse {
            v.reverse();
        }
        v
    }

    fn settings(&self) -> Option<PrintSettings> {
        let pages = self.pages();
        if pages.is_empty() {
            return None;
        }
        Some(PrintSettings {
            printer: self.printers.get(self.printer)?.clone(),
            copies: self.copies.clamp(1, 999),
            collate: self.collate,
            pages,
            scaling: self.scaling,
            grayscale: self.gray,
            auto_rotate: self.rotate,
            auto_center: self.center,
        })
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn style(&self) -> TextStyle {
        TextStyle {
            size: self.px(11.2),
            bold: false,
        }
    }

    /// Lay out centred in a window of `size` device pixels at `scale`.
    pub fn layout(&mut self, _m: &dyn TextMeasure, size: (f32, f32), scale: f32) {
        // The reference at 125 %: 1535 × 870; smaller windows get a smaller dialog.
        self.scale = scale.min(size.0 / 1250.0).min(size.1 / 712.0).max(0.5);
        let (w, h) = (self.px(1228.0), self.px(696.0));
        let f = [
            ((size.0 - w) / 2.0).round().max(0.0),
            ((size.1 - h) / 2.0).round().max(0.0),
            w,
            h,
        ];
        self.frame = f;
        self.parts.clear();
        let (x0, y0) = (f[0], f[1]);
        let at = |x: f32, y: f32, w: f32, h: f32| [x0 + x, y0 + y, w, h];
        let s = self.scale;
        let r = |x: f32, y: f32, w: f32, h: f32| at(x * s, y * s, w * s, h * s);
        self.parts.push((Part::Close, r(1192.0, 0.0, 36.0, 26.0)));
        self.parts.push((Part::Printer, r(92.0, 44.0, 293.0, 26.0)));
        if self.list_open {
            for i in 0..self.printers.len() {
                self.parts.push((
                    Part::PrinterItem(i),
                    r(92.0, 70.0 + i as f32 * 24.0, 293.0, 24.0),
                ));
            }
        }
        self.parts.push((Part::Copies, r(92.0, 80.0, 70.0, 26.0)));
        self.parts
            .push((Part::CopiesUp, r(162.0, 80.0, 14.0, 13.0)));
        self.parts
            .push((Part::CopiesDown, r(162.0, 93.0, 14.0, 13.0)));
        self.parts
            .push((Part::Collate, r(203.0, 84.0, 100.0, 20.0)));
        self.parts.push((Part::Gray, r(296.0, 84.0, 140.0, 20.0)));
        self.parts
            .push((Part::Range(Range::Current), r(28.0, 170.0, 200.0, 20.0)));
        self.parts
            .push((Part::Range(Range::All), r(28.0, 196.0, 200.0, 20.0)));
        self.parts
            .push((Part::Range(Range::Pages), r(28.0, 220.0, 80.0, 20.0)));
        self.parts
            .push((Part::PagesText, r(119.0, 220.0, 224.0, 27.0)));
        self.parts
            .push((Part::Subset, r(119.0, 282.0, 172.0, 26.0)));
        self.parts
            .push((Part::Reverse, r(28.0, 312.0, 200.0, 20.0)));
        self.parts.push((
            Part::Scaling(PrintScaling::Actual),
            r(28.0, 464.0, 240.0, 20.0),
        ));
        self.parts.push((
            Part::Scaling(PrintScaling::Fit),
            r(28.0, 494.0, 240.0, 20.0),
        ));
        self.parts.push((
            Part::Scaling(PrintScaling::Shrink),
            r(28.0, 524.0, 240.0, 20.0),
        ));
        self.parts
            .push((Part::Rotate, r(492.0, 232.0, 200.0, 20.0)));
        self.parts
            .push((Part::Center, r(492.0, 264.0, 200.0, 20.0)));
        self.parts
            .push((Part::PrevPage, r(858.0, 613.0, 36.0, 24.0)));
        self.parts
            .push((Part::NextPage, r(998.0, 613.0, 36.0, 24.0)));
        self.parts.push((Part::Ok, r(994.0, 657.0, 95.0, 28.0)));
        self.parts
            .push((Part::Cancel, r(1110.0, 657.0, 95.0, 28.0)));
        self.preview_rect = r(836.0, 102.0, 342.0, 486.0);
    }

    fn rect(&self, part: Part) -> [f32; 4] {
        self.parts
            .iter()
            .find(|(p, _)| *p == part)
            .map_or([0.0; 4], |(_, r)| *r)
    }

    fn part_at(&self, x: f32, y: f32) -> Option<Part> {
        let inside = |r: &[f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if self.list_open {
            if let Some((p, _)) = self
                .parts
                .iter()
                .find(|(p, r)| matches!(p, Part::PrinterItem(_)) && inside(r))
            {
                return Some(*p);
            }
        }
        self.parts
            .iter()
            .find(|(p, r)| !matches!(p, Part::PrinterItem(_)) && inside(r))
            .map(|(p, _)| *p)
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
            Part::Copies => self.focus_pages = false,
            Part::CopiesUp => {
                self.copies = (self.copies + 1).min(999);
                self.sync_copies();
            }
            Part::CopiesDown => {
                self.copies = self.copies.saturating_sub(1).max(1);
                self.sync_copies();
            }
            Part::Collate => self.collate = !self.collate,
            Part::Gray => self.gray = !self.gray,
            Part::Range(r) => {
                self.range = r;
                self.focus_pages = r == Range::Pages;
                self.preview_at = 0;
            }
            Part::PagesText => {
                self.range = Range::Pages;
                self.focus_pages = true;
            }
            Part::Subset => {
                self.subset = (self.subset + 1) % 3;
                self.preview_at = 0;
            }
            Part::Reverse => self.reverse = !self.reverse,
            Part::Scaling(s) => self.scaling = s,
            Part::Rotate => self.rotate = !self.rotate,
            Part::Center => self.center = !self.center,
            Part::PrevPage => self.preview_at = self.preview_at.saturating_sub(1),
            Part::NextPage => {
                let n = self.pages().len();
                self.preview_at = (self.preview_at + 1).min(n.saturating_sub(1));
            }
            Part::Ok => return self.settings().map(PrintAction::Print),
            Part::Cancel | Part::Close => return Some(PrintAction::Cancel),
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
            self.preview_at = 0;
        } else {
            self.copies_text
                .extend(s.chars().filter(char::is_ascii_digit));
            self.copies_text.truncate(3);
            let v: String = self.copies_text.iter().collect();
            self.copies = v.parse().unwrap_or(1).clamp(1, 999);
        }
    }

    /// A key: Enter prints, Escape cancels, Backspace edits, Up / Down change the copies,
    /// Page Up / Down move the preview.
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
            Key::PageDown => {
                let n = self.pages().len();
                self.preview_at = (self.preview_at + 1).min(n.saturating_sub(1));
            }
            Key::PageUp => self.preview_at = self.preview_at.saturating_sub(1),
            Key::Tab => self.focus_pages = !self.focus_pages,
            _ => {}
        }
        None
    }

    fn label(&self, p: &mut dyn Painter, text: &str, x: f32, y: f32, color: Rgba) {
        p.text(text, x, y, self.style(), color);
    }

    /// Text centred vertically in `r`, `dx` from its left.
    fn text_in(&self, p: &mut dyn Painter, text: &str, r: [f32; 4], dx: f32, color: Rgba) {
        let st = self.style();
        let y = r[1] + ((r[3] - st.size * 1.33) / 2.0).round();
        p.text(text, r[0] + dx, y, st, color);
    }

    fn radio(&self, p: &mut dyn Painter, r: [f32; 4], on: bool, label: &str) {
        let d = self.px(12.8);
        let (cx, cy) = (r[0] + d / 2.0 + 1.0, r[1] + r[3] / 2.0);
        p.path(
            &circle(cx, cy, d / 2.0),
            Some(WHITE),
            Some((
                if on { FOCUS } else { Rgba(51, 51, 51, 255) },
                self.scale.max(1.0),
            )),
        );
        if on {
            p.path(&circle(cx, cy, d / 4.0), Some(FOCUS), None);
        }
        self.text_in(p, label, r, d + self.px(6.4), TEXT);
    }

    fn check(&self, p: &mut dyn Painter, r: [f32; 4], on: bool, label: &str) {
        let b = self.px(12.8);
        let (bx, by) = (r[0] + 1.0, (r[1] + (r[3] - b) / 2.0).round());
        if on {
            p.fill_rect(bx, by, b, b, FOCUS);
            let k = b / 4.0;
            p.path(
                &[
                    PathSeg::M(bx + k * 0.9, by + b / 2.0),
                    PathSeg::L(bx + b / 2.0 - k / 4.0, by + b - k * 1.1),
                    PathSeg::L(bx + b - k * 0.9, by + k * 1.1),
                ],
                None,
                Some((WHITE, (1.4 * self.scale).max(1.2))),
            );
        } else {
            p.fill_rect(bx, by, b, b, Rgba(51, 51, 51, 255));
            p.fill_rect(bx + 1.0, by + 1.0, b - 2.0, b - 2.0, WHITE);
        }
        self.text_in(p, label, r, b + self.px(6.4), TEXT);
    }

    fn field(&self, p: &mut dyn Painter, r: [f32; 4], focused: bool, text: &str, caret: bool) {
        p.fill_rect(r[0], r[1], r[2], r[3], if focused { FOCUS } else { FIELD });
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, FIELD_BG);
        p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
        self.text_in(p, text, r, self.px(3.2), TEXT);
        if focused && caret {
            let st = self.style();
            let x = r[0] + self.px(3.2) + text.chars().count() as f32 * st.size * 0.55;
            p.fill_rect(
                x.round(),
                r[1] + self.px(5.0),
                1.0,
                r[3] - self.px(10.0),
                TEXT,
            );
        }
        p.pop_clip();
    }

    fn combo(&self, p: &mut dyn Painter, r: [f32; 4], text: &str, open: bool) {
        self.field(p, r, open, text, false);
        let (ax, ay, k) = (r[0] + r[2] - self.px(10.0), r[1] + r[3] / 2.0, self.px(3.2));
        p.path(
            &[
                PathSeg::M(ax - k, ay - k / 2.0),
                PathSeg::L(ax, ay + k / 2.0),
                PathSeg::L(ax + k, ay - k / 2.0),
            ],
            None,
            Some((TEXT, self.scale.max(1.0))),
        );
    }

    fn group(&self, p: &mut dyn Painter, title: &str, r: [f32; 4]) {
        let s = self.scale;
        let [x, y, w, h] = [
            self.frame[0] + r[0] * s,
            self.frame[1] + r[1] * s,
            r[2] * s,
            r[3] * s,
        ];
        p.fill_rect(x, y, w, 1.0, GROUP);
        p.fill_rect(x, y + h - 1.0, w, 1.0, GROUP);
        p.fill_rect(x, y, 1.0, h, GROUP);
        p.fill_rect(x + w - 1.0, y, 1.0, h, GROUP);
        if !title.is_empty() {
            self.label(p, title, x + self.px(6.4), y - self.px(13.0) + 1.0, TEXT);
        }
    }

    fn button(&self, p: &mut dyn Painter, part: Part, text: &str, default: bool) {
        let r = self.rect(part);
        let hover = self.hover == Some(part);
        let edge = if default { FOCUS } else { BUTTON_EDGE };
        p.fill_rect(r[0], r[1], r[2], r[3], edge);
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            if hover { BUTTON_HOVER } else { BUTTON },
        );
        let st = self.style();
        let tw = text.chars().count() as f32 * st.size * 0.5;
        self.text_in(p, text, r, ((r[2] - tw) / 2.0).round(), TEXT);
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
        p.fill_rect(f[0], f[1], f[2], f[3], Rgba(0, 0, 0, 255));
        p.fill_rect(f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0, WHITE);
        // Caption.
        p.fill_rect(
            f[0] + 1.0,
            f[1] + 1.0,
            f[2] - 2.0,
            self.px(25.6) - 1.0,
            CAPTION,
        );
        self.label(
            p,
            s.get("pdfPrint.title"),
            f[0] + self.px(4.8),
            f[1] + self.px(5.6),
            TEXT,
        );
        let c = self.rect(Part::Close);
        let (cx, cy, k) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.6));
        if self.hover == Some(Part::Close) {
            p.fill_rect(c[0], c[1], c[2], c[3], Rgba(0xc4, 0x2b, 0x1c, 255));
        }
        let xc = if self.hover == Some(Part::Close) {
            WHITE
        } else {
            TEXT
        };
        p.path(
            &[
                PathSeg::M(cx - k, cy - k),
                PathSeg::L(cx + k, cy + k),
                PathSeg::M(cx + k, cy - k),
                PathSeg::L(cx - k, cy + k),
            ],
            None,
            Some((xc, self.scale.max(1.0))),
        );
        // Printer and copies.
        self.group(p, "", [17.0, 35.0, 781.0, 81.0]);
        let pr = self.rect(Part::Printer);
        self.text_in(
            p,
            s.get("pdfPrint.name"),
            [f[0], pr[1], 0.0, pr[3]],
            self.px(28.8),
            TEXT,
        );
        let name = self
            .printers
            .get(self.printer)
            .map_or_else(|| s.get("pdfPrint.noPrinter").to_owned(), Clone::clone);
        self.combo(p, pr, &name, self.list_open);
        let cp = self.rect(Part::Copies);
        self.text_in(
            p,
            s.get("pdfPrint.copies"),
            [f[0], cp[1], 0.0, cp[3]],
            self.px(28.8),
            TEXT,
        );
        let copies: String = self.copies_text.iter().collect();
        self.field(p, cp, !self.focus_pages, &copies, caret);
        for (part, up) in [(Part::CopiesUp, true), (Part::CopiesDown, false)] {
            let r = self.rect(part);
            p.fill_rect(r[0], r[1], r[2], r[3], FIELD);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BUTTON);
            let (mx, my, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(2.4));
            let dy = if up { -k / 2.0 } else { k / 2.0 };
            p.path(
                &[
                    PathSeg::M(mx - k, my - dy),
                    PathSeg::L(mx, my + dy),
                    PathSeg::L(mx + k, my - dy),
                ],
                None,
                Some((TEXT, self.scale.max(1.0))),
            );
        }
        self.check(
            p,
            self.rect(Part::Collate),
            self.collate,
            s.get("pdfPrint.collate"),
        );
        self.check(
            p,
            self.rect(Part::Gray),
            self.gray,
            s.get("pdfPrint.grayscale"),
        );
        // Print Range.
        self.group(p, s.get("pdfPrint.range"), [17.0, 132.0, 449.0, 235.0]);
        self.radio(
            p,
            self.rect(Part::Range(Range::Current)),
            self.range == Range::Current,
            s.get("pdfPrint.current"),
        );
        self.radio(
            p,
            self.rect(Part::Range(Range::All)),
            self.range == Range::All,
            s.get("pdfPrint.all"),
        );
        self.radio(
            p,
            self.rect(Part::Range(Range::Pages)),
            self.range == Range::Pages,
            s.get("pdfPrint.pages"),
        );
        let pt = self.rect(Part::PagesText);
        let pages_text: String = self.pages_text.iter().collect();
        if self.range == Range::Pages {
            self.field(p, pt, self.focus_pages, &pages_text, caret);
        } else {
            p.fill_rect(pt[0], pt[1], pt[2], pt[3], GROUP);
            p.fill_rect(pt[0] + 1.0, pt[1] + 1.0, pt[2] - 2.0, pt[3] - 2.0, WHITE);
            self.text_in(p, &pages_text, pt, self.px(6.4), GREY_TEXT);
        }
        self.text_in(
            p,
            &format!("/ {}", self.count),
            [pt[0] + pt[2], pt[1], 0.0, pt[3]],
            self.px(6.4),
            TEXT,
        );
        self.label(
            p,
            s.get("pdfPrint.sample"),
            pt[0],
            pt[1] + pt[3] + self.px(8.0),
            TEXT,
        );
        let sb = self.rect(Part::Subset);
        self.text_in(
            p,
            s.get("pdfPrint.subset"),
            [f[0], sb[1], 0.0, sb[3]],
            self.px(28.8),
            TEXT,
        );
        let subset = [
            "pdfPrint.subsetAll",
            "pdfPrint.subsetOdd",
            "pdfPrint.subsetEven",
        ][usize::from(self.subset.min(2))];
        self.combo(p, sb, s.get(subset), false);
        self.check(
            p,
            self.rect(Part::Reverse),
            self.reverse,
            s.get("pdfPrint.reverse"),
        );
        // Print Handling.
        self.group(p, s.get("pdfPrint.handling"), [17.0, 386.0, 449.0, 258.0]);
        for (sc, key) in [
            (PrintScaling::Actual, "pdfPrint.none"),
            (PrintScaling::Fit, "pdfPrint.fit"),
            (PrintScaling::Shrink, "pdfPrint.shrink"),
        ] {
            self.radio(
                p,
                self.rect(Part::Scaling(sc)),
                self.scaling == sc,
                s.get(key),
            );
        }
        // Orientation.
        self.group(
            p,
            s.get("pdfPrint.orientation"),
            [479.0, 132.0, 316.0, 235.0],
        );
        self.check(
            p,
            self.rect(Part::Rotate),
            self.rotate,
            s.get("pdfPrint.autoRotate"),
        );
        self.check(
            p,
            self.rect(Part::Center),
            self.center,
            s.get("pdfPrint.autoCenter"),
        );
        // Preview.
        self.group(p, s.get("pdfPrint.preview"), [809.0, 36.0, 396.0, 562.0]);
        let pv = self.preview_rect;
        let (w_pt, h_pt) = self.preview_size;
        self.label(
            p,
            &format!(
                "{}:  {:.1} x {:.1} inch",
                s.get("pdfPrint.document"),
                w_pt / 72.0,
                h_pt / 72.0
            ),
            pv[0] - self.px(18.0),
            pv[1] - self.px(28.0),
            TEXT,
        );
        let k = if self.preview_key.is_some() && self.preview_scale > 0.0 {
            self.preview_scale
        } else {
            (pv[2] / w_pt).min(pv[3] / h_pt)
        };
        let (pw, ph) = ((w_pt * k).round(), (h_pt * k).round());
        let (x0, y0) = (
            (pv[0] + (pv[2] - pw) / 2.0).round(),
            (pv[1] + (pv[3] - ph) / 2.0).round(),
        );
        let fw = self.scale.max(1.0).round() + 1.0;
        p.fill_rect(x0 - fw, y0 - fw, pw + 2.0 * fw, ph + 2.0 * fw, PAGE_FRAME);
        p.fill_rect(x0, y0, pw, ph, WHITE);
        if let Some(key) = self.preview_key {
            p.page(key, x0 as i32, y0 as i32);
        }
        let n = self.pages().len().max(1);
        let at = self.preview_at.min(n - 1) + 1;
        for (part, left) in [(Part::PrevPage, true), (Part::NextPage, false)] {
            let r = self.rect(part);
            let enabled = if left { at > 1 } else { at < n };
            p.fill_rect(r[0], r[1], r[2], r[3], BUTTON_EDGE);
            p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, BUTTON);
            let (mx, my, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, self.px(3.2));
            let dx = if left { k / 2.0 } else { -k / 2.0 };
            p.path(
                &[
                    PathSeg::M(mx + dx, my - k),
                    PathSeg::L(mx - dx, my),
                    PathSeg::L(mx + dx, my + k),
                ],
                None,
                Some((if enabled { TEXT } else { GREY_TEXT }, self.scale.max(1.0))),
            );
        }
        let nr = self.rect(Part::NextPage);
        let page_of = s
            .get("pdfPrint.pageOf")
            .replace("{{n}}", &at.to_string())
            .replace("{{total}}", &n.to_string());
        self.text_in(
            p,
            &page_of,
            [nr[0] + nr[2], nr[1], 0.0, nr[3]],
            self.px(48.0),
            TEXT,
        );
        // Buttons.
        self.button(p, Part::Ok, s.get("pdfPrint.ok"), true);
        self.button(p, Part::Cancel, s.get("pdfPrint.cancel"), false);
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
                p.fill_rect(x, y, first.1[2], h, FIELD);
                p.fill_rect(x + 1.0, y, first.1[2] - 2.0, h - 1.0, WHITE);
            }
            for (i, r) in items {
                if self.hover == Some(Part::PrinterItem(i)) || i == self.printer {
                    p.fill_rect(r[0] + 1.0, r[1], r[2] - 2.0, r[3] - 1.0, ITEM_HOVER);
                }
                self.text_in(p, &self.printers[i], r, self.px(4.8), TEXT);
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
    fn same_size_as_the_reference_at_125_percent() {
        let mut d = PrintDialog::new(vec!["A".into()], None, 3, 0);
        d.layout(&Fixed, (1920.0, 1080.0), 1.25);
        assert_eq!((d.frame[2], d.frame[3]), (1535.0, 870.0));
    }

    #[test]
    fn choose_printer_range_copies_and_print() {
        let mut d = PrintDialog::new(vec!["A".into(), "B".into()], Some("B"), 12, 4);
        d.layout(&Fixed, (1600.0, 1000.0), 1.25);
        let (x, y) = centre(&d, Part::Printer);
        assert_eq!(d.press(x, y), None);
        d.layout(&Fixed, (1600.0, 1000.0), 1.25);
        let (x, y) = centre(&d, Part::PrinterItem(0));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Range(Range::Current));
        d.press(x, y);
        assert_eq!(d.pages(), vec![4]);
        let (x, y) = centre(&d, Part::PagesText);
        d.press(x, y);
        for _ in 0..4 {
            d.key(Key::Backspace, Mods::default());
        }
        d.text("-3,9");
        assert_eq!(d.pages(), vec![0, 1, 2, 8]);
        // Odd pages only, reversed.
        let (x, y) = centre(&d, Part::Subset);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Reverse);
        d.press(x, y);
        assert_eq!(d.pages(), vec![8, 2, 0]);
        let (x, y) = centre(&d, Part::CopiesUp);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Gray);
        d.press(x, y);
        let (x, y) = centre(&d, Part::Scaling(PrintScaling::Actual));
        d.press(x, y);
        let (x, y) = centre(&d, Part::Ok);
        assert_eq!(
            d.press(x, y),
            Some(PrintAction::Print(PrintSettings {
                printer: "A".into(),
                copies: 2,
                collate: true,
                pages: vec![8, 2, 0],
                scaling: PrintScaling::Actual,
                grayscale: true,
                auto_rotate: true,
                auto_center: true,
            }))
        );
        assert_eq!(
            d.key(Key::Escape, Mods::default()),
            Some(PrintAction::Cancel)
        );
    }
}
