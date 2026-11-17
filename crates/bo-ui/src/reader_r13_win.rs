//! BlueReader round 13 (a child of `reader_window`): View ▸ Assistant ▸ Loupe and Magnifier,
//! measured on the reference reader at 125 %.
//!
//! - Loupe: a panel "Loupe Tool" (518 × 522 px) over the left of the page area with the page
//!   magnified around the last place clicked (200 % at first, 100–1600 %), a zoom box, −, a
//!   slider, + and Lock (a locked loupe keeps its place). The area shown is framed on the page.
//! - Magnifier: a round lens (280 px across) under the pointer showing the page enlarged.
//!
//! Both draw from a page image rendered at their own scale (the print preview's image slot, so
//! the text stays sharp), not from the view's image blown up.
use super::r12::Assist;
use super::{inside, ReaderWindow};
use crate::popup::{Choice, MenuItem};
use crate::reader_view::PREVIEW_KEY;
use crate::{Input, Key, Painter};
use bo_render::{PathSeg, Rgba};

/// Zoom steps of the loupe (percent).
const LOUPE_STEPS: [f32; 10] = [
    100.0, 125.0, 150.0, 200.0, 300.0, 400.0, 600.0, 800.0, 1200.0, 1600.0,
];
/// Loupe zoom range (percent): the slider is linear over it.
const LOUPE_MIN: f32 = 100.0;
const LOUPE_MAX: f32 = 1600.0;
/// Magnifier enlargement over the view's zoom.
const LENS_ZOOM: f32 = 2.25;
/// Longest side of the page image rendered for the loupe / magnifier (device px).
const MAX_SIDE: f32 = 6000.0;

const PANEL_BG: Rgba = Rgba(255, 255, 255, 255);
const TITLE_BG: Rgba = Rgba(243, 243, 243, 255);
const VIEW_BG: Rgba = Rgba(128, 128, 128, 255);
const FRAME: Rgba = Rgba(160, 160, 160, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const TRACK: Rgba = Rgba(222, 222, 222, 255);
const THUMB: Rgba = Rgba(0, 120, 215, 255);
const GLYPH: Rgba = Rgba(120, 120, 120, 255);
const AREA: Rgba = Rgba(38, 128, 235, 255);
const RING: Rgba = Rgba(176, 178, 182, 255);
const RING_DARK: Rgba = Rgba(96, 98, 102, 255);
const HANDLE: Rgba = Rgba(46, 48, 52, 255);

/// What a drag on the loupe panel moves.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Grab {
    /// The panel by its title bar: offset of the pointer from the panel's corner.
    Panel(f32, f32),
    /// The zoom slider.
    Slider,
    /// The place on the page.
    Place,
}

/// State of round 13's tools.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Thirteen {
    /// Loupe: page and point (page points from its top-left) at the centre; `None` until a
    /// click on a page.
    pub(crate) place: Option<(usize, f32, f32)>,
    /// Loupe zoom (percent).
    pub(crate) zoom: f32,
    /// The loupe keeps its place on clicks.
    pub(crate) lock: bool,
    /// Panel corner (window px) once moved.
    pub(crate) corner: Option<(f32, f32)>,
    grab: Option<Grab>,
    /// Magnifier: pointer (window px) over the page area.
    pub(crate) pointer: Option<(f32, f32)>,
}

impl Default for Thirteen {
    fn default() -> Self {
        Self {
            place: None,
            zoom: 200.0,
            lock: false,
            corner: None,
            grab: None,
            pointer: None,
        }
    }
}

/// Parts of the loupe panel (window px).
struct Parts {
    panel: [f32; 4],
    title: [f32; 4],
    close: [f32; 4],
    view: [f32; 4],
    combo: [f32; 4],
    minus: [f32; 4],
    track: [f32; 4],
    plus: [f32; 4],
    lock: [f32; 4],
}

impl ReaderWindow {
    fn parts13(&self) -> Parts {
        let d = self.doc_rect;
        let (w, h) = (self.px(414.4), self.px(417.6));
        let (x, y) = self
            .thirteen()
            .corner
            .unwrap_or((d[0] + self.px(2.4), d[1] + self.px(4.0)));
        let title_h = self.px(30.4);
        let bar = self.px(41.6);
        let by = y + h - bar;
        let r = |dx: f32, dy: f32, rw: f32, rh: f32| {
            [x + self.px(dx), by + self.px(dy), self.px(rw), self.px(rh)]
        };
        Parts {
            panel: [x, y, w, h],
            title: [x, y, w, title_h],
            close: [x + w - self.px(36.8), y, self.px(36.8), title_h],
            view: [x + 1.0, y + title_h, w - 2.0, h - title_h - bar],
            combo: r(16.8, 4.0, 64.0, 22.4),
            minus: r(108.0, 6.4, 24.0, 24.0),
            track: r(148.0, 15.2, 152.0, 4.0),
            plus: r(316.0, 6.4, 24.0, 24.0),
            lock: r(352.8, 6.4, 52.0, 16.0),
        }
    }

    fn thirteen(&self) -> &super::r13::Thirteen {
        &self.twelve.thirteen
    }

    /// Device pixels per point of the loupe at its zoom.
    fn loupe_scale13(&self) -> f32 {
        self.thirteen().zoom / 100.0 * self.scale * 96.0 / 72.0
    }

    /// The page image the tool in use wants: (page, device px per point).
    fn wanted13(&self) -> Option<(usize, f32)> {
        let t = self.tabs.get(self.active)?;
        let v = &t.view;
        let (page, s) = match self.twelve.assist? {
            Assist::Loupe => {
                let (page, ..) = self.thirteen().place?;
                (page, self.loupe_scale13())
            }
            Assist::Magnifier => {
                let (x, y) = self.thirteen().pointer?;
                let (page, ..) = v.page_at(x, y)?;
                (page, v.px_per_pt() * LENS_ZOOM)
            }
            Assist::Marquee => return None,
        };
        let (w, h) = v.page_size(page);
        Some((page, s.min(MAX_SIDE / w.max(h).max(1.0))))
    }

    /// Before page images are brought up to date: the loupe / magnifier image (unless the
    /// Print dialog uses the preview images).
    pub(crate) fn prepare_thirteen(&mut self) {
        if self.extra.print.is_some() {
            return;
        }
        let Some(w) = self.wanted13() else { return };
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.view.want_previews(&[w]);
        }
    }

    /// Start or end a round 13 tool (called when the Assistant tool changes).
    pub(crate) fn reset_thirteen(&mut self, keep_place: bool) {
        let t = &mut self.twelve.thirteen;
        t.grab = None;
        t.pointer = None;
        if !keep_place {
            t.place = None;
        }
    }

    /// The Assistant menu's Loupe / Magnifier: `true` when handled.
    pub(crate) fn command_thirteen(&mut self, cmd: &str) -> bool {
        match cmd {
            "assist.loupe" | "assist.magnifier" => {
                let a = if cmd == "assist.loupe" {
                    Assist::Loupe
                } else {
                    Assist::Magnifier
                };
                if self.twelve.assist == Some(a) {
                    self.set_assist12(None);
                } else {
                    self.set_assist12(Some(a));
                }
                true
            }
            "loupe.close" => {
                self.set_assist12(None);
                true
            }
            "loupe.lock" => {
                self.twelve.thirteen.lock = !self.twelve.thirteen.lock;
                true
            }
            "loupe.out" | "loupe.in" => {
                let z = self.thirteen().zoom;
                let next = if cmd == "loupe.in" {
                    LOUPE_STEPS.iter().copied().find(|s| *s > z + 0.5)
                } else {
                    LOUPE_STEPS.iter().rev().copied().find(|s| *s < z - 0.5)
                };
                if let Some(n) = next {
                    self.twelve.thirteen.zoom = n;
                }
                true
            }
            _ => {
                if let Some(z) = cmd
                    .strip_prefix("loupe.zoom:")
                    .and_then(|v| v.parse::<f32>().ok())
                {
                    self.twelve.thirteen.zoom = z.clamp(LOUPE_MIN, LOUPE_MAX);
                    return true;
                }
                false
            }
        }
    }

    /// Menu items of the Assistant button (round 12's Marquee, round 13's tools).
    pub(crate) fn assistant_items13(&self) -> Vec<MenuItem> {
        let item = |cmd: &str, key: &str, on: bool| MenuItem {
            label: self.t(key),
            choice: Choice::Command(cmd.to_owned()),
            checked: on,
            dim: false,
        };
        let a = self.twelve.assist;
        vec![
            item(
                "assist.marquee",
                "pdfView12.marquee",
                a == Some(Assist::Marquee),
            ),
            item("assist.loupe", "pdfView12.loupe", a == Some(Assist::Loupe)),
            item(
                "assist.magnifier",
                "pdfView12.magnifier",
                a == Some(Assist::Magnifier),
            ),
        ]
    }

    fn slider_zoom13(&mut self, x: f32) {
        let tr = self.parts13().track;
        let k = ((x - tr[0]) / tr[2].max(1.0)).clamp(0.0, 1.0);
        self.twelve.thirteen.zoom = (LOUPE_MIN + k * (LOUPE_MAX - LOUPE_MIN)).round();
    }

    fn place_at13(&mut self, x: f32, y: f32) -> bool {
        let Some(t) = self.tabs.get(self.active) else {
            return false;
        };
        let Some((page, px, py)) = t.view.page_at(x, y) else {
            return false;
        };
        self.twelve.thirteen.place = Some((page, px, py));
        true
    }

    /// Input while the Loupe or Magnifier is in use: `Some(redraw)` when handled.
    pub(crate) fn route_thirteen(&mut self, input: &Input) -> Option<bool> {
        let a = self.twelve.assist?;
        if a == Assist::Marquee {
            return None;
        }
        if let Input::Key(Key::Escape, _) = input {
            self.set_assist12(None);
            return Some(true);
        }
        if a == Assist::Magnifier {
            return match *input {
                Input::Move { x, y } | Input::Drag { x, y } => {
                    let over = inside(self.doc_rect, x, y).then_some((x, y));
                    let changed = over != self.thirteen().pointer;
                    self.twelve.thirteen.pointer = over;
                    Some(changed)
                }
                // Clicks do nothing while the lens is up (the reference's Magnifier).
                Input::Press { x, y, .. } if inside(self.doc_rect, x, y) => Some(false),
                _ => None,
            };
        }
        let p = self.parts13();
        match *input {
            Input::Press { x, y, .. } => {
                if inside(p.panel, x, y) {
                    let cmd = if inside(p.close, x, y) {
                        Some("loupe.close")
                    } else if inside(p.minus, x, y) {
                        Some("loupe.out")
                    } else if inside(p.plus, x, y) {
                        Some("loupe.in")
                    } else if inside(p.lock, x, y) {
                        Some("loupe.lock")
                    } else {
                        None
                    };
                    if let Some(c) = cmd {
                        self.command_thirteen(c);
                    } else if inside(p.combo, x, y) {
                        let items = LOUPE_STEPS
                            .iter()
                            .map(|z| MenuItem {
                                label: format!("{z}"),
                                choice: Choice::Command(format!("loupe.zoom:{z}")),
                                checked: (z - self.thirteen().zoom).abs() < 0.5,
                                dim: false,
                            })
                            .collect();
                        self.menu_at(items, p.combo, "menu.assistant");
                    } else if (y - (p.track[1] + p.track[3] / 2.0)).abs() < self.px(10.0)
                        && x >= p.track[0] - self.px(6.0)
                        && x <= p.track[0] + p.track[2] + self.px(6.0)
                    {
                        self.twelve.thirteen.grab = Some(Grab::Slider);
                        self.slider_zoom13(x);
                    } else if inside(p.title, x, y) {
                        self.twelve.thirteen.grab =
                            Some(Grab::Panel(x - p.panel[0], y - p.panel[1]));
                    }
                    return Some(true);
                }
                if inside(self.doc_rect, x, y) {
                    if !self.thirteen().lock && self.place_at13(x, y) {
                        self.twelve.thirteen.grab = Some(Grab::Place);
                    }
                    return Some(true);
                }
                None
            }
            Input::Drag { x, y } => match self.thirteen().grab {
                Some(Grab::Slider) => {
                    self.slider_zoom13(x);
                    Some(true)
                }
                Some(Grab::Panel(dx, dy)) => {
                    let d = self.doc_rect;
                    let nx = (x - dx).clamp(d[0], (d[0] + d[2] - p.panel[2]).max(d[0]));
                    let ny = (y - dy).clamp(d[1], (d[1] + d[3] - p.title[3]).max(d[1]));
                    self.twelve.thirteen.corner = Some((nx, ny));
                    Some(true)
                }
                Some(Grab::Place) => Some(self.place_at13(x, y)),
                None => None,
            },
            Input::Release { .. } => self.twelve.thirteen.grab.take().map(|_| true),
            _ => None,
        }
    }

    /// Draw page `page` around `centre` (page points) at `s` device px per point into `dest`,
    /// from preview image 0 rendered at `s_img`.
    fn zoomed13(
        &self,
        p: &mut dyn Painter,
        page: usize,
        centre: (f32, f32),
        s: f32,
        s_img: f32,
        dest: [f32; 4],
    ) {
        let Some(t) = self.tabs.get(self.active) else {
            return;
        };
        let v = &t.view;
        let (pw, ph) = v.page_size(page);
        // The page's rectangle at scale `s` with `centre` in the middle of `dest`.
        let ox = dest[0] + dest[2] / 2.0 - centre.0 * s;
        let oy = dest[1] + dest[3] / 2.0 - centre.1 * s;
        let x0 = ox.max(dest[0]);
        let y0 = oy.max(dest[1]);
        let x1 = (ox + pw * s).min(dest[0] + dest[2]);
        let y1 = (oy + ph * s).min(dest[1] + dest[3]);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        p.fill_rect(x0, y0, x1 - x0, y1 - y0, self.paper());
        if !v.preview_ready(0, page, s_img) {
            return;
        }
        let k = s_img / s;
        let src = [(x0 - ox) * k, (y0 - oy) * k, (x1 - x0) * k, (y1 - y0) * k];
        p.page_part(
            t.key_base + PREVIEW_KEY + 1,
            src,
            [x0, y0, x1 - x0, y1 - y0],
            1.0,
        );
    }

    /// Round 13's drawing over the page and window (loupe panel, magnifier lens).
    pub(crate) fn paint_thirteen(&self, p: &mut dyn Painter) {
        match self.twelve.assist {
            Some(Assist::Loupe) => self.paint_loupe13(p),
            Some(Assist::Magnifier) => self.paint_lens13(p),
            _ => {}
        }
    }

    fn paint_loupe13(&self, p: &mut dyn Painter) {
        let parts = self.parts13();
        let s = self.loupe_scale13();
        let s_img = self.wanted13().map_or(s, |w| w.1);
        // The area the loupe shows, framed on the page.
        if let (Some((page, cx, cy)), Some(t)) = (self.thirteen().place, self.tabs.get(self.active))
        {
            let v = &t.view;
            if let Some(r) = v.page_rect(page) {
                let k = v.px_per_pt() / s;
                let (w, h) = (parts.view[2] * k, parts.view[3] * k);
                let x = (r[0] + cx * v.px_per_pt() - w / 2.0).round() + 0.5;
                let y = (r[1] + cy * v.px_per_pt() - h / 2.0).round() + 0.5;
                let d = self.doc_rect;
                p.push_clip(d[0], d[1], d[2], d[3]);
                p.path(
                    &rect_path(x, y, w.round(), h.round()),
                    None,
                    Some((AREA, 1.0)),
                );
                p.pop_clip();
            }
        }
        let [x, y, w, h] = parts.panel;
        // Shadow, frame, title bar.
        p.fill_rect(x + 2.0, y + 2.0, w, h, Rgba(0, 0, 0, 30));
        p.fill_rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, FRAME);
        p.fill_rect(x, y, w, h, PANEL_BG);
        let tb = parts.title;
        p.fill_rect(tb[0], tb[1], tb[2], tb[3], TITLE_BG);
        p.text(
            &self.t("pdfView12.loupeTitle"),
            x + self.px(14.4),
            y + self.px(6.4),
            self.style(17.0, false),
            INK,
        );
        // Close cross.
        let c = parts.close;
        let (cx, cy, a) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(4.8));
        p.path(
            &[
                PathSeg::M(cx - a, cy - a),
                PathSeg::L(cx + a, cy + a),
                PathSeg::M(cx + a, cy - a),
                PathSeg::L(cx - a, cy + a),
            ],
            None,
            Some((INK, 1.0)),
        );
        // Magnified page.
        let vr = parts.view;
        p.fill_rect(vr[0], vr[1], vr[2], vr[3], VIEW_BG);
        if let Some((page, px, py)) = self.thirteen().place {
            p.push_clip(vr[0], vr[1], vr[2], vr[3]);
            p.raw_colours(true);
            self.zoomed13(p, page, (px, py), s, s_img, vr);
            p.raw_colours(false);
            p.pop_clip();
        }
        // Bottom bar: zoom box, −, slider, +, Lock.
        let ui = self.style(15.0, false);
        let cb = parts.combo;
        p.fill_rect(cb[0], cb[1], cb[2], cb[3], Rgba(122, 122, 122, 255));
        p.fill_rect(cb[0] + 1.0, cb[1] + 1.0, cb[2] - 2.0, cb[3] - 2.0, PANEL_BG);
        p.text(
            &format!("{}", self.thirteen().zoom.round()),
            cb[0] + self.px(3.2),
            cb[1] + self.px(3.2),
            ui,
            INK,
        );
        let (ax, ay) = (cb[0] + cb[2] - self.px(10.4), cb[1] + cb[3] / 2.0);
        let e = self.px(4.0);
        p.path(
            &[
                PathSeg::M(ax - e, ay - e / 2.0),
                PathSeg::L(ax, ay + e / 2.0),
                PathSeg::L(ax + e, ay - e / 2.0),
            ],
            None,
            Some((GLYPH, 1.0)),
        );
        p.text(
            "%",
            cb[0] + cb[2] + self.px(5.6),
            cb[1] + self.px(3.2),
            ui,
            INK,
        );
        for (b, plus) in [(parts.minus, false), (parts.plus, true)] {
            let (mx, my, l) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0, self.px(4.8));
            p.fill_rect(mx - l, my - 1.0, 2.0 * l, 2.0, GLYPH);
            if plus {
                p.fill_rect(mx - 1.0, my - l, 2.0, 2.0 * l, GLYPH);
            }
        }
        let tr = parts.track;
        p.fill_rect(tr[0], tr[1], tr[2], tr[3], TRACK);
        let k = (self.thirteen().zoom - LOUPE_MIN) / (LOUPE_MAX - LOUPE_MIN);
        let tx = tr[0] + k.clamp(0.0, 1.0) * tr[2];
        let (tw, th) = (self.px(7.2), self.px(12.8));
        p.fill_rect(
            (tx - tw / 2.0).round(),
            (tr[1] + tr[3] / 2.0 - th / 2.0).round(),
            tw,
            th,
            THUMB,
        );
        let lk = parts.lock;
        let bx = self.px(10.4);
        p.fill_rect(lk[0], lk[1] + 1.0, bx, bx, Rgba(51, 51, 51, 255));
        p.fill_rect(lk[0] + 1.0, lk[1] + 2.0, bx - 2.0, bx - 2.0, PANEL_BG);
        if self.thirteen().lock {
            let (u, v0) = (lk[0], lk[1] + 1.0);
            p.path(
                &[
                    PathSeg::M(u + bx * 0.2, v0 + bx * 0.5),
                    PathSeg::L(u + bx * 0.42, v0 + bx * 0.72),
                    PathSeg::L(u + bx * 0.8, v0 + bx * 0.28),
                ],
                None,
                Some((INK, 1.5)),
            );
        }
        p.text(
            &self.t("pdfView12.lock"),
            lk[0] + bx + self.px(4.0),
            lk[1] - self.px(1.6),
            ui,
            INK,
        );
    }

    fn paint_lens13(&self, p: &mut dyn Painter) {
        let (Some((x, y)), Some(t)) = (self.thirteen().pointer, self.tabs.get(self.active)) else {
            return;
        };
        let v = &t.view;
        let Some((page, px, py)) = v.page_at(x, y) else {
            return;
        };
        let s = v.px_per_pt() * LENS_ZOOM;
        let s_img = self.wanted13().map_or(s, |w| w.1);
        let r = self.px(112.0);
        let d = self.doc_rect;
        p.push_clip(d[0], d[1], d[2], d[3]);
        // The lens content in horizontal strips cut to the circle (painters clip to
        // rectangles only).
        let square = [x - r, y - r, 2.0 * r, 2.0 * r];
        let step = 2.0_f32.max(self.px(1.6));
        let mut yy = -r;
        p.raw_colours(true);
        while yy < r {
            let mid = yy + step / 2.0;
            let half = (r * r - mid * mid).max(0.0).sqrt();
            if half > 0.5 {
                p.push_clip(x - half, y + yy, 2.0 * half, step);
                p.fill_rect(x - half, y + yy, 2.0 * half, step, Rgba(128, 128, 128, 255));
                self.zoomed13(p, page, (px, py), s, s_img, square);
                p.pop_clip();
            }
            yy += step;
        }
        p.raw_colours(false);
        // Ring and handle (own drawing: a plain hand lens).
        let ring = self.px(7.2);
        p.path(&circle(x, y, r + ring / 2.0), None, Some((RING, ring)));
        p.path(&circle(x, y, r + ring), None, Some((RING_DARK, 1.0)));
        p.path(&circle(x, y, r), None, Some((RING_DARK, 1.0)));
        let a = std::f32::consts::FRAC_1_SQRT_2;
        let (h0, h1) = (r + ring, r + ring + self.px(72.0));
        p.path(
            &[
                PathSeg::M(x + h0 * a, y + h0 * a),
                PathSeg::L(x + h1 * a, y + h1 * a),
            ],
            None,
            Some((HANDLE, self.px(17.6))),
        );
        p.pop_clip();
    }
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Vec<PathSeg> {
    vec![
        PathSeg::M(x, y),
        PathSeg::L(x + w, y),
        PathSeg::L(x + w, y + h),
        PathSeg::L(x, y + h),
        PathSeg::Z,
    ]
}

/// A circle of four cubic arcs.
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
#[path = "reader_r13_tests.rs"]
mod tests;
