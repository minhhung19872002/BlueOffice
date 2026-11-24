//! Thumbnail galleries of BlueDoc round 22 (docs/ui/REPORT.md "Vòng 112"): the drop-downs the
//! reference word processor shows as grids of small pictures instead of word lists — WordArt
//! Quick Styles, Text Effects › Transform, Shape Styles, Picture Styles and the Picture /
//! Shape Effects categories. A gallery is a column of sections (a grey heading band, then a
//! grid of cells) with optional command rows at the bottom; each cell is drawn from
//! [`Thumb`] (BlueOffice's own sample drawings: a letter, a square, the warp's guide
//! shapes).
use crate::theme::Theme;
use crate::{Font, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};

const MENU_EDGE: Rgba = Rgba(198, 198, 198, 255);
const BAND: Rgba = Rgba(240, 240, 240, 255);
const PICKED: Rgba = Rgba(0x9c, 0xc3, 0xe6, 255);

/// A thumbnail path: segments, fill, stroke (colour, width at 100 %).
pub(crate) type ThumbPath = (Vec<PathSeg>, Option<Rgba>, Option<(Rgba, f32)>);

/// What a cell shows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Thumb {
    /// Paths in the unit box (0..1 both ways, y down): (segments, fill, stroke colour and
    /// width in cell pixels at 100 %).
    Paths(Vec<ThumbPath>),
    /// A large letter "A": fill, outline, halo (glow / shadow) colours.
    Letter {
        fill: Rgba,
        outline: Option<Rgba>,
        halo: Option<Rgba>,
    },
    /// Nothing (a "No …" cell): a slashed square.
    None,
}

/// One cell.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cell {
    pub(crate) thumb: Thumb,
    /// Interface command run when it is picked.
    pub(crate) command: String,
    /// Name shown as a tooltip (and read by tests).
    pub(crate) tip: String,
    /// The current setting (framed).
    pub(crate) checked: bool,
}

/// A heading and its grid.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Section {
    pub(crate) title: String,
    pub(crate) cells: Vec<Cell>,
}

/// The open gallery.
#[derive(Debug, Clone)]
pub(crate) struct ThumbGallery {
    sections: Vec<Section>,
    /// Command rows under the grids: (label, command).
    footer: Vec<(String, String)>,
    cols: usize,
    /// Cell size at 100 % (logical px).
    cell: (f32, f32),
    x: f32,
    y: f32,
    scale: f32,
    hover: Option<(usize, usize)>,
    footer_hover: Option<usize>,
}

impl ThumbGallery {
    /// A gallery under `anchor` (window px), kept inside a `screen`-sized window.
    pub(crate) fn open(
        sections: Vec<Section>,
        footer: Vec<(String, String)>,
        (cols, cell): (usize, (f32, f32)),
        anchor: [f32; 4],
        screen: (f32, f32),
        scale: f32,
    ) -> Self {
        let mut g = ThumbGallery {
            sections,
            footer,
            cols: cols.max(1),
            cell,
            x: anchor[0],
            y: anchor[1] + anchor[3],
            scale,
            hover: None,
            footer_hover: None,
        };
        let (w, h) = g.size();
        g.x = g.x.min(screen.0 - w - 2.0).max(0.0);
        // Too tall below the button: moved up only as far as needed (never over the top).
        if g.y + h > screen.1 - 4.0 {
            g.y = (screen.1 - 4.0 - h).max(0.0);
        }
        g
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn band_h(&self) -> f32 {
        self.px(24.0)
    }

    fn row_h(&self) -> f32 {
        self.px(26.0)
    }

    fn cell_px(&self) -> (f32, f32) {
        (self.px(self.cell.0), self.px(self.cell.1))
    }

    fn rows_of(&self, s: &Section) -> usize {
        s.cells.len().div_ceil(self.cols)
    }

    /// Width and height (window px).
    pub(crate) fn size(&self) -> (f32, f32) {
        let (cw, ch) = self.cell_px();
        let w = cw * self.cols as f32 + self.px(12.0);
        let mut h = self.px(2.0);
        for s in &self.sections {
            if !s.title.is_empty() {
                h += self.band_h();
            }
            h += ch * self.rows_of(s) as f32 + self.px(6.0);
        }
        if !self.footer.is_empty() {
            h += self.px(4.0) + self.row_h() * self.footer.len() as f32;
        }
        (w, h + self.px(2.0))
    }

    /// The gallery's rectangle.
    pub(crate) fn rect(&self) -> [f32; 4] {
        let (w, h) = self.size();
        [self.x, self.y, w, h]
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.rect();
        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
    }

    /// Rectangle of cell `i` of section `s`, and where the section's band starts.
    fn layout(&self) -> Vec<(f32, Vec<[f32; 4]>)> {
        let (cw, ch) = self.cell_px();
        let mut y = self.y + self.px(1.0);
        let mut out = Vec::new();
        for s in &self.sections {
            let band = y;
            if !s.title.is_empty() {
                y += self.band_h();
            }
            y += self.px(3.0);
            let cells = (0..s.cells.len())
                .map(|i| {
                    let (c, r) = (i % self.cols, i / self.cols);
                    [
                        self.x + self.px(6.0) + cw * c as f32,
                        y + ch * r as f32,
                        cw,
                        ch,
                    ]
                })
                .collect();
            y += ch * self.rows_of(s) as f32 + self.px(3.0);
            out.push((band, cells));
        }
        out
    }

    fn footer_top(&self) -> f32 {
        let (_, h) = self.size();
        self.y + h - self.px(2.0) - self.row_h() * self.footer.len() as f32
    }

    fn cell_at(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        for (si, (_, cells)) in self.layout().iter().enumerate() {
            if let Some(ci) = cells
                .iter()
                .position(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            {
                return Some((si, ci));
            }
        }
        None
    }

    fn footer_at(&self, x: f32, y: f32) -> Option<usize> {
        if self.footer.is_empty() || !self.contains(x, y) {
            return None;
        }
        let top = self.footer_top();
        (y >= top)
            .then(|| ((y - top) / self.row_h()) as usize)
            .filter(|i| *i < self.footer.len())
    }

    /// Hover at (x, y); `true` when the highlight moved.
    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let (h, f) = (self.cell_at(x, y), self.footer_at(x, y));
        let changed = h != self.hover || f != self.footer_hover;
        self.hover = h;
        self.footer_hover = f;
        changed
    }

    /// The command a press at (x, y) picks.
    pub(crate) fn press(&self, x: f32, y: f32) -> Option<String> {
        if let Some(i) = self.footer_at(x, y) {
            return self.footer.get(i).map(|f| f.1.clone());
        }
        let (s, c) = self.cell_at(x, y)?;
        Some(self.sections.get(s)?.cells.get(c)?.command.clone())
    }

    /// Name of the hovered cell (tooltip).
    #[cfg(test)]
    pub(crate) fn hovered_tip(&self) -> Option<&str> {
        let (s, c) = self.hover?;
        Some(self.sections.get(s)?.cells.get(c)?.tip.as_str())
    }

    /// Every cell (tests and accessibility): (section, tip, command).
    #[cfg(test)]
    pub(crate) fn cells(&self) -> Vec<(usize, &str, &str)> {
        self.sections
            .iter()
            .enumerate()
            .flat_map(|(i, s)| {
                s.cells
                    .iter()
                    .map(move |c| (i, c.tip.as_str(), c.command.as_str()))
            })
            .collect()
    }

    /// Tips of the cells drawn as the current setting (tests).
    #[cfg(test)]
    pub(crate) fn checked_tips(&self) -> Vec<String> {
        self.sections
            .iter()
            .flat_map(|s| s.cells.iter().filter(|c| c.checked).map(|c| c.tip.clone()))
            .collect()
    }

    /// Centre of cell `c` of section `s` (tests).
    #[cfg(test)]
    pub(crate) fn center(&self, s: usize, c: usize) -> (f32, f32) {
        let r = self.layout()[s].1[c];
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme) {
        let (w, h) = self.size();
        p.fill_rect(self.x, self.y, w, h, MENU_EDGE);
        p.fill_rect(self.x + 1.0, self.y + 1.0, w - 2.0, h - 2.0, t.surface);
        let ui = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        for (si, (s, (band, cells))) in self.sections.iter().zip(self.layout()).enumerate() {
            if !s.title.is_empty() {
                p.fill_rect(self.x + 1.0, band, w - 2.0, self.band_h(), BAND);
                p.text(
                    &s.title,
                    self.x + self.px(8.0),
                    band + (self.band_h() - ui.size * 1.33) / 2.0,
                    TextStyle { bold: true, ..ui },
                    t.text,
                );
            }
            for (ci, (c, r)) in s.cells.iter().zip(cells).enumerate() {
                let hovered = self.hover == Some((si, ci));
                if c.checked {
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, PICKED);
                }
                if hovered {
                    p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, t.hover);
                }
                self.paint_thumb(p, &c.thumb, r);
            }
        }
        if !self.footer.is_empty() {
            let top = self.footer_top();
            p.fill_rect(
                self.x + self.px(4.0),
                top - self.px(3.0),
                w - self.px(8.0),
                1.0,
                MENU_EDGE,
            );
            for (i, (label, _)) in self.footer.iter().enumerate() {
                let y = top + self.row_h() * i as f32;
                if self.footer_hover == Some(i) {
                    p.fill_rect(self.x + 1.0, y, w - 2.0, self.row_h(), t.hover);
                }
                p.text(
                    label,
                    self.x + self.px(32.0),
                    y + (self.row_h() - ui.size * 1.33) / 2.0,
                    ui,
                    t.text,
                );
            }
        }
    }

    fn paint_thumb(&self, p: &mut dyn Painter, th: &Thumb, r: [f32; 4]) {
        let pad = self.px(6.0);
        let (bx, by, bw, bh) = (r[0] + pad, r[1] + pad, r[2] - 2.0 * pad, r[3] - 2.0 * pad);
        match th {
            Thumb::Paths(paths) => {
                let map = |x: f32, y: f32| (bx + x * bw, by + y * bh);
                for (segs, fill, stroke) in paths {
                    let segs: Vec<PathSeg> = segs
                        .iter()
                        .map(|s| match *s {
                            PathSeg::M(x, y) => {
                                let (x, y) = map(x, y);
                                PathSeg::M(x, y)
                            }
                            PathSeg::L(x, y) => {
                                let (x, y) = map(x, y);
                                PathSeg::L(x, y)
                            }
                            PathSeg::Q(a, b, x, y) => {
                                let ((a, b), (x, y)) = (map(a, b), map(x, y));
                                PathSeg::Q(a, b, x, y)
                            }
                            PathSeg::C(a, b, c, d, x, y) => {
                                let ((a, b), (c, d), (x, y)) = (map(a, b), map(c, d), map(x, y));
                                PathSeg::C(a, b, c, d, x, y)
                            }
                            PathSeg::Z => PathSeg::Z,
                        })
                        .collect();
                    p.path(
                        &segs,
                        *fill,
                        stroke.map(|(c, w)| (c, (w * self.scale).max(1.0))),
                    );
                }
            }
            Thumb::Letter {
                fill,
                outline,
                halo,
            } => {
                let font = Font {
                    family: "Calibri",
                    size: bh * 1.05,
                    weight: 400,
                    italic: false,
                    underline: false,
                };
                let (x, y) = (bx + bw * 0.22, by - bh * 0.12);
                if let Some(c) = halo {
                    // A soft shadow down-right (a glow colour all round).
                    let d = self.px(1.5);
                    let offs: &[(f32, f32)] = if c.0 == 0 && c.1 == 0 && c.2 == 0 {
                        &[(1.0, 1.0)]
                    } else {
                        &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
                    };
                    for (dx, dy) in offs {
                        p.text_font("A", x + dx * d, y + dy * d, &font, *c);
                    }
                }
                if let Some(c) = outline {
                    let d = self.px(1.0);
                    for (dx, dy) in [(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)] {
                        p.text_font("A", x + dx, y + dy, &font, *c);
                    }
                }
                p.text_font("A", x, y, &font, *fill);
            }
            Thumb::None => {
                let grey = Rgba(0x9a, 0x9a, 0x9a, 255);
                p.path(
                    &[
                        PathSeg::M(bx, by),
                        PathSeg::L(bx + bw, by),
                        PathSeg::L(bx + bw, by + bh),
                        PathSeg::L(bx, by + bh),
                        PathSeg::Z,
                        PathSeg::M(bx, by + bh),
                        PathSeg::L(bx + bw, by),
                    ],
                    None,
                    Some((grey, self.scale.max(1.0))),
                );
            }
        }
    }
}

/// The frame and sizing handles of a selected drawing object (`r` window px), as the
/// reference word processor draws them (measured at 125 %: a 1 px grey frame, eight round
/// white handles 9 px across with a grey ring, a turning handle above the top edge); a text
/// box being edited gets a dashed frame.
pub(crate) fn paint_handles(p: &mut dyn Painter, r: [f32; 4], scale: f32, editing: bool) {
    let grey = Rgba(0x70, 0x70, 0x70, 255);
    let ring_c = Rgba(0x4a, 0x4a, 0x4a, 255);
    let [x, y, w, h] = r.map(f32::round);
    if editing {
        let dash = (3.0 * scale).round().max(2.0);
        let mut t = 0.0;
        while t < w {
            let l = dash.min(w - t);
            p.fill_rect(x + t, y, l, 1.0, grey);
            p.fill_rect(x + t, y + h, l, 1.0, grey);
            t += dash * 2.0;
        }
        let mut t = 0.0;
        while t < h {
            let l = dash.min(h - t);
            p.fill_rect(x, y + t, 1.0, l, grey);
            p.fill_rect(x + w, y + t, 1.0, l, grey);
            t += dash * 2.0;
        }
    } else {
        p.fill_rect(x, y, w, 1.0, grey);
        p.fill_rect(x, y + h, w + 1.0, 1.0, grey);
        p.fill_rect(x, y, 1.0, h, grey);
        p.fill_rect(x + w, y, 1.0, h, grey);
    }
    let rad = 4.6 * scale;
    let ring = |p: &mut dyn Painter, cx: f32, cy: f32| {
        let n = 16;
        let segs: Vec<PathSeg> = (0..=n)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / n as f32;
                let (s, c) = a.sin_cos();
                if i == 0 {
                    PathSeg::M(cx + rad * c, cy + rad * s)
                } else {
                    PathSeg::L(cx + rad * c, cy + rad * s)
                }
            })
            .collect();
        p.path(
            &segs,
            Some(Rgba(255, 255, 255, 255)),
            Some((ring_c, scale.max(1.0))),
        );
    };
    for (u, v) in [
        (0.0, 0.0),
        (0.5, 0.0),
        (1.0, 0.0),
        (0.0, 0.5),
        (1.0, 0.5),
        (0.0, 1.0),
        (0.5, 1.0),
        (1.0, 1.0),
    ] {
        ring(p, x + w * u, y + h * v);
    }
    // The turning handle: a stem and an open circle with an arrow head (own drawing).
    let (cx, top) = (x + w / 2.0, y - 27.0 * scale);
    p.fill_rect(cx, top + 9.0 * scale, 1.0, y - top - 9.0 * scale, grey);
    let r2 = 8.5 * scale;
    let n = 14;
    let segs: Vec<PathSeg> = (0..=n)
        .map(|i| {
            let a = -1.2 + 5.0 * i as f32 / n as f32;
            let (s, c) = a.sin_cos();
            if i == 0 {
                PathSeg::M(cx + r2 * c, top + r2 * s)
            } else {
                PathSeg::L(cx + r2 * c, top + r2 * s)
            }
        })
        .collect();
    p.path(&segs, None, Some((ring_c, 1.8 * scale)));
    let (s, c) = (-1.2f32).sin_cos();
    let (ax, ay) = (cx + r2 * c, top + r2 * s);
    p.path(
        &[
            PathSeg::M(ax - 4.5 * scale, ay - 4.0 * scale),
            PathSeg::L(ax + 2.5 * scale, ay - 0.5 * scale),
            PathSeg::L(ax - 1.5 * scale, ay + 5.0 * scale),
            PathSeg::Z,
        ],
        Some(ring_c),
        None,
    );
}

/// Parse a path `d` of `M x y`, `L x y`, `Z` commands (the engine's warp previews) scaled
/// by (`kx`, `ky`) into the unit box.
pub(crate) fn parse_d(d: &str, kx: f32, ky: f32) -> Vec<PathSeg> {
    let mut out = Vec::new();
    let mut rest = d;
    while let Some(c) = rest.chars().next() {
        rest = &rest[c.len_utf8()..];
        match c {
            'M' | 'L' => {
                let end = rest.find(['M', 'L', 'Z']).unwrap_or(rest.len());
                let mut nums = rest[..end]
                    .split_whitespace()
                    .filter_map(|v| v.parse().ok());
                rest = &rest[end..];
                if let (Some(x), Some(y)) = (nums.next(), nums.next()) {
                    let (x, y): (f32, f32) = (x, y);
                    out.push(if c == 'M' {
                        PathSeg::M(x * kx, y * ky)
                    } else {
                        PathSeg::L(x * kx, y * ky)
                    });
                }
            }
            'Z' => out.push(PathSeg::Z),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ThumbGallery {
        let cell = |i: usize| Cell {
            thumb: Thumb::None,
            command: format!("cmd{i}"),
            tip: format!("Tip {i}"),
            checked: i == 1,
        };
        ThumbGallery::open(
            vec![
                Section {
                    title: "First".into(),
                    cells: (0..5).map(cell).collect(),
                },
                Section {
                    title: String::new(),
                    cells: (5..7).map(cell).collect(),
                },
            ],
            vec![("More…".into(), "more".into())],
            (4, (40.0, 40.0)),
            [10.0, 10.0, 20.0, 20.0],
            (1000.0, 800.0),
            1.25,
        )
    }

    #[test]
    fn cells_and_footer_are_picked_where_drawn() {
        let mut g = sample();
        let (x, y) = g.center(0, 4);
        assert!(g.hover(x, y));
        assert_eq!(g.hovered_tip(), Some("Tip 4"));
        assert_eq!(g.press(x, y).as_deref(), Some("cmd4"));
        let (x, y) = g.center(1, 1);
        assert_eq!(g.press(x, y).as_deref(), Some("cmd6"));
        let r = g.rect();
        assert_eq!(
            g.press(r[0] + 50.0, r[1] + r[3] - 6.0).as_deref(),
            Some("more")
        );
        assert_eq!(g.cells().len(), 7);
        // Two rows of 4 cells, one row, the band and the footer.
        assert!(r[3] > 3.0 * 50.0);
    }

    #[test]
    fn warp_preview_paths_parse() {
        let segs = parse_d("M4.0 5.0L10.0 5.0Z M1 2L3 4", 0.1, 0.1);
        assert_eq!(segs.len(), 5);
        assert_eq!(segs[0], PathSeg::M(0.4, 0.5));
        assert_eq!(segs[2], PathSeg::Z);
    }
}
