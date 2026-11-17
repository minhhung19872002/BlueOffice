//! Gallery drop-downs of BlueDoc (round 3): Position, Table of Contents, Bibliography and
//! Effects, laid out like the reference's galleries (grey section header band, framed tiles,
//! menu rows under a separator). Tile drawings are BlueOffice's own.
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0x26, 0x26, 0x26, 255);
const DIM: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const BAND: Rgba = Rgba(0xeb, 0xeb, 0xeb, 255);
const FRAME: Rgba = Rgba(0x94, 0x94, 0x94, 255);
const SEP: Rgba = Rgba(0xe6, 0xe6, 0xe6, 255);
const HOVER: Rgba = Rgba(0xd5, 0xe1, 0xf2, 255);

fn inside(r: [f32; 4], x: f32, y: f32) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn ui_font(size: f32, weight: u16) -> Font<'static> {
    Font {
        family: "Segoe UI",
        size,
        weight,
        italic: false,
        underline: false,
    }
}

/// One tile of a gallery drop-down.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tile {
    /// Command run when chosen.
    pub(crate) id: String,
    /// Caption (under the drawing; above it for wide tiles); may be empty.
    pub(crate) label: String,
}

/// A section of a gallery: grey header band, then tiles.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TileSection {
    pub(crate) header: String,
    pub(crate) tiles: Vec<Tile>,
}

/// Draws tile `id` into a rectangle (window pixels) at display scale.
pub(crate) type TileFn = fn(&mut dyn Painter, &str, [f32; 4], f32);

/// How a gallery is laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TileLook {
    /// Tiles per row.
    pub(crate) cols: usize,
    /// Tile size at 100 % (caption included).
    pub(crate) tile: (f32, f32),
    /// Caption above the drawing (wide tiles) instead of below.
    pub(crate) caption_top: bool,
}

/// A gallery drop-down: sections of tiles, then menu rows (id, label, enabled).
pub(crate) struct TileMenu {
    pub(crate) rect: [f32; 4],
    scale: f32,
    sections: Vec<TileSection>,
    rows: Vec<(String, String, bool)>,
    look: TileLook,
    draw: TileFn,
    hover: Option<usize>,
    /// The tile shown as current.
    pub(crate) current: Option<String>,
    /// Tile rows shown at once (one section; a scroll bar beside them), as the reference's
    /// Cover Page gallery; `None`: every row.
    max_rows: Option<usize>,
    /// First tile row shown.
    first_row: usize,
    /// Icons of the menu rows (own drawings), drawn in the 40 px column at their left; a
    /// negative scale asks for the dimmed look.
    pub(crate) row_icon: Option<TileFn>,
    /// Menu rows that open a submenu (a right-pointing mark at the right edge).
    pub(crate) row_more: Vec<String>,
}

/// Scroll bar width (logical px) of a scrolling gallery, and its arrow parts' ids.
const BAR: f32 = 16.0;
pub(crate) const SCROLL_UP: &str = "tiles.scrollUp";
pub(crate) const SCROLL_DOWN: &str = "tiles.scrollDown";

const HEADER_H: f32 = 22.0;
const ROW_H: f32 = 29.0;
const GAP: f32 = 4.0;

/// A part of the menu: rectangle, command, label, tile (else row), enabled.
type Part<'a> = ([f32; 4], &'a str, &'a str, bool, bool);

impl TileMenu {
    pub(crate) fn open(
        anchor: [f32; 4],
        viewport: (f32, f32),
        sections: Vec<TileSection>,
        rows: Vec<(String, String, bool)>,
        look: TileLook,
        draw: TileFn,
        scale: f32,
    ) -> Self {
        let mut m = Self {
            rect: [0.0; 4],
            scale,
            sections,
            rows,
            look: TileLook {
                cols: look.cols.max(1),
                ..look
            },
            draw,
            hover: None,
            current: None,
            max_rows: None,
            first_row: 0,
            row_icon: None,
            row_more: Vec::new(),
        };
        let cols = m.look.cols;
        let w = m.px(GAP + cols as f32 * (look.tile.0 + GAP) + 2.0);
        let mut h = m.px(2.0);
        for s in &m.sections {
            let n = s.tiles.len().div_ceil(cols);
            h += m.px(HEADER_H) + n as f32 * m.px(look.tile.1 + GAP) + m.px(GAP);
        }
        h += m.rows.len() as f32 * m.px(ROW_H) + m.px(2.0);
        let x = anchor[0].min(viewport.0 - w).max(0.0);
        let y = anchor[1] + anchor[3];
        m.rect = [x, y, w, h.min(viewport.1 - y).max(m.px(40.0))];
        m
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn tile_rows(&self) -> usize {
        self.sections
            .first()
            .map_or(0, |s| s.tiles.len().div_ceil(self.look.cols))
    }

    /// Show `rows` tile rows at once with a scroll bar (one-section galleries); the menu
    /// keeps its top-left corner.
    pub(crate) fn scroll_rows(&mut self, rows: usize, viewport: (f32, f32)) {
        if self.sections.len() != 1 || self.tile_rows() <= rows {
            return;
        }
        self.max_rows = Some(rows.max(1));
        let tile = self.look.tile;
        self.rect[2] = self.px(GAP + self.look.cols as f32 * (tile.0 + GAP) + 2.0 + BAR);
        self.rect[0] = self.rect[0].min(viewport.0 - self.rect[2]).max(0.0);
        let h = self.px(2.0)
            + self.px(HEADER_H)
            + rows as f32 * self.px(tile.1 + GAP)
            + self.px(GAP)
            + self.rows.len() as f32 * self.px(ROW_H)
            + self.px(2.0);
        self.rect[3] = h.min(viewport.1 - self.rect[1]).max(self.px(40.0));
    }

    /// Scroll the tile rows by `rows` (negative: up); `true` when they moved.
    pub(crate) fn scroll_by(&mut self, rows: i32) -> bool {
        let Some(shown) = self.max_rows else {
            return false;
        };
        let last = self.tile_rows().saturating_sub(shown);
        let next = (i64::try_from(self.first_row).unwrap_or(0) + i64::from(rows))
            .clamp(0, i64::try_from(last).unwrap_or(0));
        let next = usize::try_from(next).unwrap_or(0);
        std::mem::replace(&mut self.first_row, next) != next
    }

    fn parts(&self) -> Vec<Part<'_>> {
        let [x, mut y, ..] = self.rect;
        y += 1.0;
        let (cols, tile) = (self.look.cols, self.look.tile);
        let mut out = Vec::new();
        for s in &self.sections {
            y += self.px(HEADER_H);
            let total = s.tiles.len().div_ceil(cols);
            let (first, shown) = match self.max_rows {
                Some(n) => (self.first_row, n),
                None => (0, total),
            };
            for (i, t) in s.tiles.iter().enumerate() {
                let (c, r) = (i % cols, i / cols);
                if r < first || r >= first + shown {
                    continue;
                }
                let r = r - first;
                let rect = [
                    x + self.px(GAP + c as f32 * (tile.0 + GAP)),
                    y + self.px(GAP / 2.0) + r as f32 * self.px(tile.1 + GAP),
                    self.px(tile.0),
                    self.px(tile.1),
                ];
                out.push((rect, t.id.as_str(), t.label.as_str(), true, true));
            }
            let area = shown.min(total) as f32 * self.px(tile.1 + GAP);
            if self.max_rows.is_some() {
                // Scroll bar arrows: parts without a label, drawn by `paint`.
                let b = self.px(BAR);
                let bx = x + self.rect[2] - b - 1.0;
                out.push(([bx, y, b, b], SCROLL_UP, "", false, true));
                out.push(([bx, y + area - b, b, b], SCROLL_DOWN, "", false, true));
            }
            y += area + self.px(GAP);
        }
        for (id, label, enabled) in &self.rows {
            out.push((
                [x + 1.0, y, self.rect[2] - 2.0, self.px(ROW_H)],
                id.as_str(),
                label.as_str(),
                false,
                *enabled,
            ));
            y += self.px(ROW_H);
        }
        out
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        inside(self.rect, x, y)
    }

    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        self.parts().iter().position(|p| p.4 && inside(p.0, x, y))
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Command of the tile or row at (x, y).
    pub(crate) fn pick(&self, x: f32, y: f32) -> Option<String> {
        let i = self.hit(x, y)?;
        self.parts().get(i).map(|p| p.1.to_owned())
    }

    /// Window point of the centre of tile or row `id` (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn point_of(&self, id: &str) -> Option<(f32, f32)> {
        self.parts()
            .iter()
            .find(|p| p.1 == id)
            .map(|p| (p.0[0] + p.0[2] / 2.0, p.0[1] + p.0[3] / 2.0))
    }

    /// The scroll bar of a scrolling gallery: track, thumb, arrows.
    fn paint_bar(&self, p: &mut dyn Painter, parts: &[Part<'_>]) {
        let (Some(shown), Some(up), Some(down)) = (
            self.max_rows,
            parts.iter().find(|q| q.1 == SCROLL_UP),
            parts.iter().find(|q| q.1 == SCROLL_DOWN),
        ) else {
            return;
        };
        let s = self.scale;
        let (bx, top, b) = (up.0[0], up.0[1], up.0[2]);
        let bottom = down.0[1] + down.0[3];
        p.fill_rect(bx, top, b, bottom - top, Rgba(0xf0, 0xf0, 0xf0, 255));
        let total = self.tile_rows().max(1);
        let track = (bottom - top - 2.0 * b).max(1.0);
        let th = (track * shown as f32 / total as f32).max(self.px(12.0));
        let ty = top
            + b
            + (track - th) * self.first_row as f32 / total.saturating_sub(shown).max(1) as f32;
        p.fill_rect(
            bx + 3.0 * s,
            ty,
            b - 6.0 * s,
            th,
            Rgba(0xc2, 0xc2, 0xc2, 255),
        );
        for (r, upward) in [(up.0, true), (down.0, false)] {
            let (cx, cy, k) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, 3.0 * s);
            let d = if upward { -k / 2.0 } else { k / 2.0 };
            p.path(
                &[
                    PathSeg::M(cx - k, cy - d),
                    PathSeg::L(cx, cy + d),
                    PathSeg::L(cx + k, cy - d),
                    PathSeg::Z,
                ],
                Some(Rgba(0x60, 0x60, 0x60, 255)),
                None,
            );
        }
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter) {
        let [x, y, w, h] = self.rect;
        let s = self.scale;
        for (k, a) in [(3.0, 18u8), (2.0, 26), (1.0, 34)] {
            let o = (k * s).round();
            p.fill_rect(
                x - o + 2.0 * s,
                y - o + 3.0 * s,
                w + 2.0 * o,
                h + 2.0 * o,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(x, y, w, h, FRAME);
        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, WHITE);
        p.push_clip(x + 1.0, y + 1.0, w - 2.0, h - 2.0);
        let bold = ui_font(self.px(12.0), 700);
        let font = ui_font(self.px(12.0), 400);
        let small = ui_font(self.px(11.0), 400);
        let mut hy = y + 1.0;
        for sct in &self.sections {
            p.fill_rect(x + 1.0, hy, w - 2.0, self.px(HEADER_H), BAND);
            p.text_font(
                &sct.header,
                x + self.px(8.0),
                hy + self.px(3.0),
                &bold,
                Rgba(0x44, 0x44, 0x44, 255),
            );
            let rows = sct.tiles.len().div_ceil(self.look.cols);
            let rows = self.max_rows.map_or(rows, |n| n.min(rows));
            hy += self.px(HEADER_H) + rows as f32 * self.px(self.look.tile.1 + GAP) + self.px(GAP);
        }
        let parts = self.parts();
        self.paint_bar(p, &parts);
        if let Some(first) = parts.iter().find(|p| !p.3 && !p.2.is_empty()) {
            p.fill_rect(x + 1.0, first.0[1] - 1.0, w - 2.0, 1.0, SEP);
        }
        for (i, (r, id, label, tile, enabled)) in parts.iter().enumerate() {
            if *tile {
                let current = self.current.as_deref() == Some(*id);
                if self.hover == Some(i) || current {
                    let edge = if current {
                        Rgba(0x40, 0x40, 0x40, 255)
                    } else {
                        Rgba(0x80, 0x80, 0x80, 255)
                    };
                    p.fill_rect(r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0, edge);
                    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xc6, 0xc6, 0xc6, 255));
                }
                let cap = if label.is_empty() { 0.0 } else { self.px(18.0) };
                let draw_r = if self.look.caption_top {
                    [
                        r[0] + self.px(4.0),
                        r[1] + cap + self.px(2.0),
                        r[2] - self.px(8.0),
                        r[3] - cap - self.px(6.0),
                    ]
                } else {
                    [
                        r[0] + self.px(4.0),
                        r[1] + self.px(4.0),
                        r[2] - self.px(8.0),
                        r[3] - cap - self.px(6.0),
                    ]
                };
                (self.draw)(p, id, draw_r, s);
                if !label.is_empty() {
                    if self.look.caption_top {
                        p.text_font(label, r[0] + self.px(4.0), r[1] + self.px(1.0), &bold, INK);
                    } else {
                        let tw = small.size * 0.5 * label.chars().count() as f32;
                        let tx = r[0] + ((r[2] - tw) / 2.0).max(self.px(2.0));
                        p.text_font(label, tx, r[1] + r[3] - cap, &small, INK);
                    }
                }
            } else if label.is_empty() {
                // Scroll bar arrows (painted with the bar).
            } else {
                if self.hover == Some(i) && *enabled {
                    p.fill_rect(r[0], r[1], r[2], r[3], HOVER);
                }
                if let Some(icon) = self.row_icon {
                    let k = self.px(16.0);
                    icon(
                        p,
                        id,
                        [r[0] + self.px(12.0), r[1] + (r[3] - k) / 2.0, k, k],
                        if *enabled { s } else { -s },
                    );
                }
                if self.row_more.iter().any(|m| m == id) {
                    let (cx, cy, k) = (r[0] + r[2] - self.px(14.0), r[1] + r[3] / 2.0, 3.5 * s);
                    p.path(
                        &[
                            PathSeg::M(cx - k / 2.0, cy - k),
                            PathSeg::L(cx + k / 2.0, cy),
                            PathSeg::L(cx - k / 2.0, cy + k),
                            PathSeg::Z,
                        ],
                        Some(if *enabled { INK } else { DIM }),
                        None,
                    );
                }
                p.text_font(
                    label,
                    r[0] + self.px(40.0),
                    r[1] + self.px(6.0),
                    &font,
                    if *enabled { INK } else { DIM },
                );
            }
        }
        p.pop_clip();
    }
}

/// A page with text lines (gallery thumbnails of picture positions): `pic` places a picture
/// box at column 0–2 / row 0–2, or in line when `None`.
pub(crate) fn draw_position(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let (w, h) = ((r[3] * 0.72).min(r[2]), r[3]);
    let x0 = r[0] + (r[2] - w) / 2.0;
    let ink = Rgba(0x70, 0x70, 0x70, 255);
    p.fill_rect(x0, r[1], w, h, ink);
    p.fill_rect(x0 + 1.0, r[1] + 1.0, w - 2.0, h - 2.0, WHITE);
    let pos = id.rsplit('.').next().unwrap_or("");
    let cell = |k: char| match k {
        'l' | 't' => 0usize,
        'c' | 'm' => 1,
        _ => 2,
    };
    let mut chars = pos.chars();
    let (v, hh) = (chars.next(), chars.next());
    let (pw, ph) = (w * 0.34, h * 0.26);
    let (mx, my) = (w * 0.12, h * 0.1);
    let pic = match (v, hh) {
        (Some(v), Some(hc)) if pos.len() == 2 => {
            let (c, row) = (cell(hc), cell(v));
            Some([
                x0 + mx + c as f32 * (w - 2.0 * mx - pw) / 2.0,
                r[1] + my + row as f32 * (h - 2.0 * my - ph) / 2.0,
            ])
        }
        _ => None,
    };
    let line = Rgba(0x9a, 0x9a, 0x9a, 255);
    let step = (3.0 * s).max(3.0);
    let mut y = r[1] + my;
    while y < r[1] + h - my {
        let (mut a, b) = (x0 + mx, x0 + w - mx);
        let mut segs = vec![(a, b)];
        if let Some([px, py]) = pic {
            if y >= py - 1.0 && y <= py + ph + 1.0 {
                segs.clear();
                if px - a > 2.0 {
                    segs.push((a, px - 2.0));
                }
                a = px + pw + 2.0;
                if b - a > 2.0 {
                    segs.push((a, b));
                }
            }
        }
        for (a, b) in segs {
            p.fill_rect(a.round(), y.round(), (b - a).round(), 1.0, line);
        }
        y += step;
    }
    let (px, py) = pic.map_or((x0 + mx, r[1] + my), |[px, py]| (px, py));
    p.fill_rect(px, py, pw, ph, Rgba(0x2f, 0x6f, 0xba, 255));
    p.fill_rect(
        px + 1.0,
        py + 1.0,
        pw - 2.0,
        ph - 2.0,
        Rgba(0xcf, 0xe2, 0xf5, 255),
    );
    p.path(
        &[
            PathSeg::M(px + pw * 0.15, py + ph * 0.85),
            PathSeg::L(px + pw * 0.45, py + ph * 0.35),
            PathSeg::L(px + pw * 0.7, py + ph * 0.7),
            PathSeg::L(px + pw * 0.85, py + ph * 0.55),
            PathSeg::L(px + pw * 0.85, py + ph * 0.85),
            PathSeg::Z,
        ],
        Some(Rgba(0x2f, 0x6f, 0xba, 255)),
        None,
    );
}

/// A table of contents preview: a title then three levels with dotted leaders and page
/// numbers (`contents` / `toc` titles; `manual` shows placeholder text).
pub(crate) fn draw_toc(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let title_font = Font {
        family: "Calibri Light",
        size: (13.0 * s).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    let body = Font {
        family: "Calibri",
        size: (10.0 * s).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    let blue = Rgba(0x2f, 0x54, 0x96, 255);
    let title = match id.rsplit('.').next() {
        Some("auto2") => "Table of Contents",
        _ => "Contents",
    };
    p.text_font(title, r[0] + 2.0 * s, r[1], &title_font, blue);
    let manual = id.ends_with("manual");
    let rows = [
        (
            0.0,
            if manual {
                "Type chapter title (level 1)"
            } else {
                "Heading 1"
            },
            "1",
        ),
        (
            10.0,
            if manual {
                "Type chapter title (level 2)"
            } else {
                "Heading 2"
            },
            "1",
        ),
        (
            20.0,
            if manual {
                "Type chapter title (level 3)"
            } else {
                "Heading 3"
            },
            "1",
        ),
    ];
    for (i, (indent, text, page)) in rows.iter().enumerate() {
        let y = r[1] + (24.0 + i as f32 * 16.0) * s;
        let x = r[0] + (2.0 + indent) * s;
        p.text_font(text, x, y, &body, INK);
        let start = x + body.size * 0.5 * text.chars().count() as f32 + 4.0 * s;
        let end = r[0] + r[2] - 12.0 * s;
        let mut dx = start;
        while dx < end {
            p.fill_rect(dx.round(), (y + body.size * 1.05).round(), 1.0, 1.0, INK);
            dx += 3.0 * s;
        }
        p.text_font(page, r[0] + r[2] - 8.0 * s, y, &body, INK);
    }
}

/// A bibliography preview: a heading then three entries with hanging indents.
pub(crate) fn draw_bibliography(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let head = Font {
        family: "Calibri Light",
        size: (12.0 * s).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    let body = Font {
        family: "Times New Roman",
        size: (10.0 * s).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    let title = match id.rsplit('.').next() {
        Some("references") => "References",
        Some("worksCited") => "Works Cited",
        _ => "Bibliography",
    };
    p.text_font(
        title,
        r[0] + 2.0 * s,
        r[1] + 2.0 * s,
        &head,
        Rgba(0x2f, 0x54, 0x96, 255),
    );
    let lines = [
        "Nguyễn, A. (2021). Văn bản hành chính. Hà Nội: NXB Lao Động.",
        "Trần, B. (2019). Soạn thảo tài liệu. TP. HCM: NXB Trẻ.",
        "Lê, C. D. (2023). Quản trị văn phòng. Đà Nẵng: NXB Đà Nẵng.",
    ];
    for (i, l) in lines.iter().enumerate() {
        p.text_font(
            l,
            r[0] + 2.0 * s,
            r[1] + (24.0 + i as f32 * 15.0) * s,
            &body,
            INK,
        );
    }
}

/// Theme effect presets of BlueOffice (own names and looks): how a shape with an arrow over
/// a circle is drawn under each set (§20.1.4.1.14 fmtScheme effect styles).
pub(crate) const EFFECTS: [(&str, &str); 8] = [
    ("flat", "Flat"),
    ("soft", "Soft Shadow"),
    ("lifted", "Lifted"),
    ("glow", "Glow Edge"),
    ("inset", "Inset"),
    ("gloss", "Gloss"),
    ("outline", "Outline"),
    ("deep", "Deep Shadow"),
];

pub(crate) fn draw_effect(p: &mut dyn Painter, id: &str, r: [f32; 4], s: f32) {
    let key = id.rsplit('.').next().unwrap_or("flat");
    let blue = Rgba(0x2f, 0x6f, 0xba, 255);
    let light = Rgba(0x9d, 0xc3, 0xe6, 255);
    let ring = |cx: f32, cy: f32, rad: f32| -> Vec<PathSeg> {
        let mut v = Vec::new();
        for i in 0..=24 {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let (x, y) = (cx + rad * a.cos(), cy + rad * a.sin());
            v.push(if i == 0 {
                PathSeg::M(x, y)
            } else {
                PathSeg::L(x, y)
            });
        }
        v
    };
    let rad = r[3] * 0.38;
    p.path(
        &ring(r[0] + rad + 2.0, r[1] + rad + 2.0, rad),
        None,
        Some((blue, (2.0 * s).max(1.5))),
    );
    let bx = [
        r[0] + r[2] * 0.42,
        r[1] + r[3] * 0.22,
        r[2] * 0.5,
        r[3] * 0.62,
    ];
    let shadow = |p: &mut dyn Painter, d: f32, a: u8| {
        p.fill_rect(bx[0] + d, bx[1] + d, bx[2], bx[3], Rgba(0, 0, 0, a));
    };
    match key {
        "soft" => shadow(p, 2.0 * s, 50),
        "lifted" => shadow(p, 4.0 * s, 70),
        "deep" => shadow(p, 6.0 * s, 110),
        "glow" => p.fill_rect(
            bx[0] - 3.0 * s,
            bx[1] - 3.0 * s,
            bx[2] + 6.0 * s,
            bx[3] + 6.0 * s,
            Rgba(0x9d, 0xc3, 0xe6, 160),
        ),
        _ => {}
    }
    let fill = if key == "outline" { WHITE } else { blue };
    p.fill_rect(bx[0], bx[1], bx[2], bx[3], fill);
    if key == "outline" {
        p.path(
            &[
                PathSeg::M(bx[0], bx[1]),
                PathSeg::L(bx[0] + bx[2], bx[1]),
                PathSeg::L(bx[0] + bx[2], bx[1] + bx[3]),
                PathSeg::L(bx[0], bx[1] + bx[3]),
                PathSeg::Z,
            ],
            None,
            Some((blue, (2.0 * s).max(1.5))),
        );
    }
    if key == "gloss" {
        p.fill_rect(
            bx[0] + 1.0,
            bx[1] + 1.0,
            bx[2] - 2.0,
            bx[3] * 0.4,
            Rgba(255, 255, 255, 110),
        );
    }
    if key == "inset" {
        p.fill_rect(bx[0], bx[1], bx[2], 2.0 * s, Rgba(0, 0, 0, 90));
        p.fill_rect(bx[0], bx[1], 2.0 * s, bx[3], Rgba(0, 0, 0, 90));
    }
    // Arrow over both.
    let (ax, ay, aw, ah) = (
        r[0] + r[2] * 0.18,
        r[1] + r[3] * 0.4,
        r[2] * 0.62,
        r[3] * 0.42,
    );
    p.path(
        &[
            PathSeg::M(ax, ay + ah * 0.28),
            PathSeg::L(ax + aw * 0.6, ay + ah * 0.28),
            PathSeg::L(ax + aw * 0.6, ay),
            PathSeg::L(ax + aw, ay + ah / 2.0),
            PathSeg::L(ax + aw * 0.6, ay + ah),
            PathSeg::L(ax + aw * 0.6, ay + ah * 0.72),
            PathSeg::L(ax, ay + ah * 0.72),
            PathSeg::Z,
        ],
        Some(light),
        Some((WHITE, 1.0)),
    );
}
