//! BlueReader round 9 (a child of `reader_window`): the dark skin and ribbon tooltips.
//!
//! Dark skin: every surface of the window (ribbon, panels, File page, menus, dialogs, status
//! bar, tooltips) is painted through [`DarkPainter`], which maps each colour through one skin
//! palette function ([`dark_ink`]): lightness is turned over on a perceptual scale (OKLab L)
//! along a curve chosen so text keeps at least 4.5 : 1 on every background, hue and most of
//! the chroma stay. Document content (pages, paper, marks on the page, print preview, page
//! thumbnails) is drawn between `raw_colours(true)` / `raw_colours(false)` and keeps its
//! colours, as the reference's dark skin keeps the page white.
use super::{Kind, ReaderWindow};
use crate::popup::paint_tooltip;
use crate::{Font, Painter, TextMeasure, TextStyle};
use bo_render::{PathSeg, Rgba};

/// Skin index of the dark skin (`skins8::SKINS`).
pub(crate) const DARK: usize = 3;

/// Ticks (~100 ms) the pointer rests on a ribbon command before its tooltip shows.
const TIP_TICKS: u32 = 5;

/// Lightness curve of the dark skin: OKLab L of the light skin → OKLab L of the dark skin
/// (monotone, decreasing). Light backgrounds land at 0.27–0.36, body text at ~0.9, the
/// secondary greys at ≥ 0.73 so they keep 4.5 : 1 on hover and pressed tints too.
const CURVE: [(f32, f32); 8] = [
    (0.0, 0.96),
    (0.35, 0.91),
    (0.66, 0.79),
    (0.74, 0.70),
    (0.80, 0.56),
    (0.87, 0.42),
    (0.93, 0.34),
    (1.0, 0.27),
];

fn curve(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    for w in CURVE.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if l <= x1 {
            return y0 + (y1 - y0) * (l - x0) / (x1 - x0);
        }
    }
    CURVE[CURVE.len() - 1].1
}

fn to_linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    // In 0..=255 after the clamp.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let b = (s * 255.0).round() as u8;
    b
}

/// sRGB → OKLab (L, a, b).
#[allow(clippy::many_single_char_names)]
fn oklab(c: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (to_linear(c.0), to_linear(c.1), to_linear(c.2));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

/// OKLab → sRGB with alpha `alpha`.
#[allow(clippy::many_single_char_names)]
fn from_oklab(lab: (f32, f32, f32), alpha: u8) -> Rgba {
    let (ll, a, b) = lab;
    let l = (ll + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m = (ll - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s = (ll - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    Rgba(
        to_srgb(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
        to_srgb(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
        to_srgb(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        alpha,
    )
}

/// The dark skin's colour for `c` of the light skin (the skin palette function).
pub(crate) fn dark_ink(c: Rgba) -> Rgba {
    let (l, a, b) = oklab(c);
    // Keep the hue; a little less chroma so tints read as greys on the dark ground.
    from_oklab((curve(l), a * 0.85, b * 0.85), c.3)
}

/// A painter that draws the window in the dark skin over `inner`.
pub(crate) struct DarkPainter<'a> {
    pub(crate) inner: &'a mut dyn Painter,
    /// Depth of [`Painter::raw_colours`] (document content keeps its colours).
    raw: u32,
}

impl<'a> DarkPainter<'a> {
    pub(crate) fn new(inner: &'a mut dyn Painter) -> Self {
        Self { inner, raw: 0 }
    }

    fn ink(&self, c: Rgba) -> Rgba {
        if self.raw > 0 {
            c
        } else {
            dark_ink(c)
        }
    }
}

impl Painter for DarkPainter<'_> {
    fn clear(&mut self, color: Rgba) {
        let c = self.ink(color);
        self.inner.clear(c);
    }
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        let c = self.ink(color);
        self.inner.fill_rect(x, y, w, h, c);
    }
    fn page(&mut self, index: usize, x: i32, y: i32) {
        self.inner.page(index, x, y);
    }
    fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba) {
        let c = self.ink(color);
        self.inner.text(text, x, y, style, c);
    }
    fn text_font(&mut self, text: &str, x: f32, y: f32, font: &Font<'_>, color: Rgba) {
        let c = self.ink(color);
        self.inner.text_font(text, x, y, font, c);
    }
    fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, stroke: Option<(Rgba, f32)>) {
        let fill = fill.map(|c| self.ink(c));
        let stroke = stroke.map(|(c, w)| (self.ink(c), w));
        self.inner.path(segs, fill, stroke);
    }
    fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.inner.push_clip(x, y, w, h);
    }
    fn pop_clip(&mut self) {
        self.inner.pop_clip();
    }
    fn page_part(&mut self, index: usize, src: [f32; 4], dest: [f32; 4], opacity: f32) {
        self.inner.page_part(index, src, dest, opacity);
    }
    fn page_part_rotated(
        &mut self,
        index: usize,
        src: [f32; 4],
        dest: [f32; 4],
        opacity: f32,
        degrees: f32,
    ) {
        self.inner
            .page_part_rotated(index, src, dest, opacity, degrees);
    }
    fn layer_begin(&mut self, id: usize, rect: [f32; 4], key: u64) -> bool {
        // A cached layer of the light skin is not this skin's picture.
        self.inner
            .layer_begin(id, rect, key ^ 0xDA4C_5C1D_0000_0009)
    }
    fn layer_end(&mut self, id: usize) {
        self.inner.layer_end(id);
    }
    fn raw_colours(&mut self, on: bool) {
        if on {
            self.raw += 1;
        } else {
            self.raw = self.raw.saturating_sub(1);
        }
        self.inner.raw_colours(on);
    }
}

/// Round 9 state: the tooltip of the hovered ribbon command.
#[derive(Debug, Default)]
pub(crate) struct Nine {
    /// Control hovered and the ticks it has been hovered.
    tip_id: Option<&'static str>,
    tip_ticks: u32,
    /// Tooltip text and its measured width.
    tip: Option<(String, f32)>,
}

impl ReaderWindow {
    /// The dark skin is on (BlueReader only).
    pub(crate) fn dark_skin(&self) -> bool {
        self.profile.is_none() && self.prefs6.prefs.skin == DARK
    }

    /// Tooltip text of ribbon control `id` (labelled commands; small icons with a
    /// `pdfTip.<id>` string).
    fn tip_text9(&self, id: &str) -> Option<String> {
        let c = self.controls.iter().find(|c| c.id == id)?;
        match c.kind {
            Kind::Big(_, key, _) | Kind::Row(_, key, _) if !key.is_empty() => {
                Some(self.strings.get(key).replace('\n', " "))
            }
            Kind::Big(..) | Kind::Row(..) | Kind::Small(_) | Kind::Quick(_) | Kind::Nav(_) => {
                let key = format!("pdfTip.{id}");
                let s = self.strings.get(&key);
                (s != key).then(|| s.to_owned())
            }
            _ => None,
        }
    }

    /// One tick: count the rest on the hovered control; true when the tooltip appears or goes.
    pub(crate) fn tick9(&mut self, m: &dyn TextMeasure) -> bool {
        let busy = self.menu.is_some() || self.dialog.is_some() || self.pressed.is_some();
        let id = if busy { None } else { self.hover };
        if id != self.nine.tip_id {
            self.nine.tip_id = id;
            self.nine.tip_ticks = 0;
            return self.nine.tip.take().is_some();
        }
        let Some(id) = id else { return false };
        self.nine.tip_ticks += 1;
        if self.nine.tip_ticks == TIP_TICKS {
            if let Some(text) = self.tip_text9(id) {
                let st = self.tip_style9();
                let w = m.measure(&text, st).0;
                self.nine.tip = Some((text, w));
                return true;
            }
        }
        false
    }

    /// A press or a key hides the tooltip until the pointer moves to another command.
    pub(crate) fn hide_tip9(&mut self) {
        self.nine.tip = None;
        self.nine.tip_ticks = TIP_TICKS + 1;
    }

    fn tip_style9(&self) -> TextStyle {
        TextStyle {
            size: (12.0 * self.scale).round(),
            bold: false,
        }
    }

    /// Paint the tooltip under its command.
    pub(crate) fn paint_tip9(&self, p: &mut dyn Painter) {
        let (Some((text, w)), Some(id)) = (&self.nine.tip, self.nine.tip_id) else {
            return;
        };
        if self.hover != Some(id) || self.menu.is_some() || self.dialog.is_some() {
            return;
        }
        let Some(r) = self.control_rect(id) else {
            return;
        };
        paint_tooltip(p, &self.theme, text, *w, r, self.size.0, self.scale);
    }

    /// Tooltip shown (tests).
    #[cfg(test)]
    pub(crate) fn tip9(&self) -> Option<&str> {
        self.nine.tip.as_ref().map(|t| t.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2.2 contrast ratio of two sRGB colours.
    pub(crate) fn contrast(a: Rgba, b: Rgba) -> f32 {
        let lum =
            |c: Rgba| 0.2126 * to_linear(c.0) + 0.7152 * to_linear(c.1) + 0.0722 * to_linear(c.2);
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn palette_function_turns_lightness_over() {
        let white = dark_ink(Rgba(255, 255, 255, 255));
        let ink = dark_ink(Rgba(51, 51, 51, 255));
        assert!(white.0 < 60 && white.0 == white.1, "{white:?}");
        assert!(ink.0 > 200, "{ink:?}");
        assert!(contrast(white, ink) > 9.0);
        // Hue kept: the accent stays blue, lighter.
        let accent = dark_ink(Rgba(0x1f, 0x5f, 0xbf, 255));
        assert!(accent.2 > accent.0 && accent.2 > 0xbf, "{accent:?}");
        // Alpha kept.
        assert_eq!(dark_ink(Rgba(0, 0, 0, 30)).3, 30);
        // Round trip of OKLab.
        for c in [Rgba(12, 200, 99, 255), Rgba(230, 233, 240, 255)] {
            assert_eq!(from_oklab(oklab(c), 255), c);
        }
    }

    use super::super::Panel;
    use crate::reader_tests::{pdf, Fixed};
    use crate::{Input, Key, Mods};
    use bo_pdfread::PdfDocument;

    /// Records each text with the colour under its centre (fills composited in order, clipped).
    #[derive(Default)]
    struct Probe {
        fills: Vec<([f32; 4], Rgba)>,
        clips: Vec<[f32; 4]>,
        raw: u32,
        /// (surface, text, ink, background).
        texts: Vec<(String, String, Rgba, Rgba)>,
        surface: String,
    }

    fn meet(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
        let x0 = a[0].max(b[0]);
        let y0 = a[1].max(b[1]);
        let x1 = (a[0] + a[2]).min(b[0] + b[2]);
        let y1 = (a[1] + a[3]).min(b[1] + b[3]);
        [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
    }

    impl Probe {
        fn clip(&self) -> [f32; 4] {
            self.clips.last().copied().unwrap_or([-1e6, -1e6, 2e6, 2e6])
        }
        fn fill(&mut self, r: [f32; 4], c: Rgba) {
            let r = meet(r, self.clip());
            if r[2] > 0.0 && r[3] > 0.0 && c.3 > 0 {
                self.fills.push((r, c));
            }
        }
        fn under(&self, x: f32, y: f32) -> Rgba {
            let mut bg = [0.0f32; 3];
            for (r, c) in &self.fills {
                if x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3] {
                    let a = f32::from(c.3) / 255.0;
                    for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
                        bg[k] = bg[k] * (1.0 - a) + f32::from(v) * a;
                    }
                }
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Rgba(
                bg[0].round() as u8,
                bg[1].round() as u8,
                bg[2].round() as u8,
                255,
            )
        }
    }

    impl Painter for Probe {
        fn clear(&mut self, color: Rgba) {
            self.fills.clear();
            self.fills.push(([-1e6, -1e6, 2e6, 2e6], color));
        }
        fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
            self.fill([x, y, w, h], color);
        }
        fn page(&mut self, _: usize, _: i32, _: i32) {}
        fn text(&mut self, text: &str, x: f32, y: f32, style: TextStyle, color: Rgba) {
            if self.raw > 0 || text.trim().is_empty() {
                return;
            }
            let (w, h) = Fixed.measure(text, style);
            let (cx, cy) = (x + w.min(style.size) / 2.0, y + h / 2.0);
            let c = self.clip();
            if cx < c[0] || cx >= c[0] + c[2] || cy < c[1] || cy >= c[1] + c[3] {
                return;
            }
            let bg = self.under(cx, cy);
            self.texts
                .push((self.surface.clone(), text.to_owned(), color, bg));
        }
        fn path(&mut self, segs: &[PathSeg], fill: Option<Rgba>, _: Option<(Rgba, f32)>) {
            let Some(c) = fill else { return };
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for s in segs {
                let pts: &[(f32, f32)] = &match *s {
                    PathSeg::M(x, y) | PathSeg::L(x, y) => vec![(x, y)],
                    PathSeg::C(a, b, c, d, e, f) => vec![(a, b), (c, d), (e, f)],
                    _ => vec![],
                };
                for &(x, y) in pts {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
            // Only large shapes (boxes, cards, buttons) count as a background; icons do not.
            if x1 - x0 > 30.0 && y1 - y0 > 14.0 {
                self.fill([x0, y0, x1 - x0, y1 - y0], c);
            }
        }
        fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
            let r = meet([x, y, w, h], self.clip());
            self.clips.push(r);
        }
        fn pop_clip(&mut self) {
            self.clips.pop();
        }
        fn raw_colours(&mut self, on: bool) {
            if on {
                self.raw += 1;
            } else {
                self.raw = self.raw.saturating_sub(1);
            }
        }
    }

    /// Text greys of disabled commands (exempt from WCAG 1.4.3), in the light skin.
    const DISABLED: [Rgba; 4] = [
        Rgba(175, 175, 176, 255),
        Rgba(0xC1, 0xC1, 0xC1, 255),
        Rgba(0x9a, 0x9a, 0x9a, 255),
        Rgba(160, 160, 162, 255),
    ];

    fn dark_window(open: bool) -> ReaderWindow {
        let mut w = ReaderWindow::new("en-US");
        w.prefs6.prefs.skin = DARK;
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1100.0,
            },
            &Fixed,
        );
        if open {
            let doc = PdfDocument::open(pdf(3), bo_pdfread::standard_fonts()).expect("pdf");
            w.open(doc, "test.pdf", None);
            // One input lets the panels measure their strings, as the host does.
            w.handle(Input::Move { x: 1.0, y: 1.0 }, &Fixed);
        }
        w
    }

    fn probe(w: &mut ReaderWindow, surface: &str, out: &mut Probe) {
        out.surface = surface.to_owned();
        out.fills.clear();
        out.clips.clear();
        out.raw = 0;
        w.paint(out);
    }

    fn esc(w: &mut ReaderWindow) {
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    }

    #[test]
    fn every_reader_surface_keeps_text_contrast_in_the_dark_skin() {
        let mut p = Probe::default();
        let mut w = dark_window(false);
        probe(&mut w, "start", &mut p);
        let mut w = dark_window(true);
        assert!(w.dark_skin());
        probe(&mut w, "document", &mut p);
        let tabs = w.tab_label_keys().len();
        // Every ribbon tab, and every command of it run on a fresh window (its menu, dialog,
        // pane or mode painted).
        for tab in 0..tabs - 1 {
            let mut w = dark_window(true);
            w.set_ribbon_tab(tab, &Fixed);
            probe(&mut w, &format!("tab {tab}"), &mut p);
            let ids: Vec<&'static str> = w
                .controls
                .iter()
                .filter(|c| !matches!(c.kind, Kind::Sep | Kind::Win(_)))
                .map(|c| c.id)
                .collect();
            for id in ids {
                if matches!(id, "view.fullscreen" | "file.close" | "file.exit") {
                    continue;
                }
                let mut w = dark_window(true);
                w.set_ribbon_tab(tab, &Fixed);
                w.run_command(id, &Fixed);
                w.handle(Input::Move { x: 2.0, y: 2.0 }, &Fixed);
                probe(&mut w, id, &mut p);
            }
        }
        // Navigation panels.
        for panel in [Panel::Bookmarks, Panel::Pages, Panel::Comments] {
            let mut w = dark_window(true);
            w.set_panel(Some(panel));
            probe(&mut w, &format!("{panel:?}"), &mut p);
        }
        // The File page and each of its pages.
        for page in ["bs.open", "bs.properties", "bs.share", "bs.skins"] {
            let mut w = dark_window(true);
            w.open_reader_backstage();
            if let Some(b) = w.tabs5.backstage.as_mut() {
                b.page = page;
            }
            probe(&mut w, page, &mut p);
        }
        for cmd in ["bs.prefs", "file.properties"] {
            let mut w = dark_window(true);
            w.run_command(cmd, &Fixed);
            probe(&mut w, cmd, &mut p);
        }
        // Print dialog.
        let mut w = dark_window(true);
        w.open_print(vec!["Printer A".into()], Some("Printer A"), &Fixed);
        probe(&mut w, "print", &mut p);
        // Context menu on the page and on empty canvas.
        let d = w.doc_area();
        for (name, x, y) in [
            ("page menu", d[0] + d[2] / 2.0, d[1] + 120.0),
            ("canvas menu", d[0] + 8.0, d[1] + d[3] - 8.0),
        ] {
            let mut w = dark_window(true);
            w.handle(Input::Context { x, y }, &Fixed);
            probe(&mut w, name, &mut p);
            esc(&mut w);
        }
        // A ribbon tooltip.
        let mut w = dark_window(true);
        let c = w
            .controls
            .iter()
            .find(|c| matches!(c.kind, Kind::Big(..)))
            .map(|c| c.r)
            .expect("a big button");
        w.handle(
            Input::Move {
                x: c[0] + 4.0,
                y: c[1] + 4.0,
            },
            &Fixed,
        );
        for _ in 0..6 {
            w.handle(Input::Tick, &Fixed);
        }
        assert!(w.tip9().is_some());
        probe(&mut w, "tooltip", &mut p);

        let disabled: Vec<Rgba> = DISABLED.iter().map(|&c| dark_ink(c)).collect();
        let mut bad = Vec::new();
        let mut checked = 0;
        for (surface, text, ink, bg) in &p.texts {
            if disabled.contains(ink) {
                continue;
            }
            checked += 1;
            let k = contrast(*ink, *bg);
            if k < 4.5 {
                bad.push(format!("{surface}: {text:?} {ink:?} on {bg:?} = {k:.2}"));
            }
        }
        bad.sort();
        bad.dedup();
        let worst = p
            .texts
            .iter()
            .filter(|t| !disabled.contains(&t.2))
            .map(|t| (contrast(t.2, t.3), t.1.clone(), t.0.clone()))
            .fold((99.0f32, String::new(), String::new()), |a, b| {
                if b.0 < a.0 {
                    b
                } else {
                    a
                }
            });
        eprintln!("checked {checked} of {} worst {worst:?}", p.texts.len());
        assert!(checked > 500, "{checked} texts");
        assert!(
            bad.is_empty(),
            "{} low-contrast texts:\n{}",
            bad.len(),
            bad.join("\n")
        );
    }
}
