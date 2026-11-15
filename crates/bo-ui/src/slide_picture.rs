//! BlueSlide's Picture Format tab, round 5 (N6): Remove Background (the engine's graph-cut
//! removal, CORE-PIC-002, with a preview of the removed areas tinted on the slide and strokes
//! marking areas to keep or remove), Corrections (sharpen / soften, brightness / contrast),
//! Color (saturation, recolour), Transparency — galleries of previews of the selected picture
//! drawn by the backend from derived image ids (`media#fx:…`) — Compress Pictures, Change
//! Picture and Reset, laid out as the reference presentation app's Adjust group.
use super::{InsertFile, SlideWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::{Input, Key, PageStore, Painter, TextStyle};
use base64::Engine as _;
use bo_engine::picture_bg::{MarkSpec, RemovalSpec};
use bo_engine::picture_tools::PictureCommand as P;
use bo_model::picture::PictureFx;
use bo_render::{Command, PageList, Rgba};

/// Keys of the gallery previews in the page store (after the view's own key ranges).
const GALLERY_KEY: usize = 9 << 40;
/// Key of the background removal preview drawn over the picture.
const BG_KEY: usize = 10 << 40;

/// Commands of the Picture Format tab added in this round.
pub const COMMANDS: [&str; 7] = [
    "sl.removeBg",
    "sl.corrections",
    "sl.picColor",
    "sl.transparency",
    "sl.compress",
    "sl.changePicture",
    "sl.bgKeep",
];

/// Commands of the Background Removal tab.
pub const BG_COMMANDS: [&str; 4] = ["sl.bgKeep", "sl.bgRemove", "sl.bgDiscard", "sl.bgDone"];

const LABELS: [(&str, &str); 10] = [
    ("sl.removeBg", "removeBackground"),
    ("sl.corrections", "corrections"),
    ("sl.picColor", "color"),
    ("sl.transparency", "transparency"),
    ("sl.compress", "compressPictures"),
    ("sl.changePicture", "changePicture"),
    ("sl.bgKeep", "markKeep"),
    ("sl.bgRemove", "markRemove"),
    ("sl.bgDiscard", "discardChanges"),
    ("sl.bgDone", "keepChanges"),
];

/// Picture Format (a picture selected), groups as the reference at 1500 px.
pub(super) fn picture_format_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: "slideNative.pictureFormat",
        groups: vec![
            Group {
                label: "slideNative.adjust",
                large: vec![L("sl.removeBg"), L("sl.corrections")],
                rows: vec![
                    vec![S("sl.picColor"), B("sl.compress")],
                    vec![S("sl.transparency"), B("sl.changePicture")],
                    vec![B("sl.resetPicture")],
                ],
            },
            Group {
                label: "slideNative.pictureStyles",
                large: vec![],
                rows: vec![vec![S("sl.picBorder")], vec![S("sl.effects")]],
            },
            super::fmt::accessibility_group(),
            super::fmt::arrange_group(),
            super::ins::size_group(true),
        ],
    }
}

/// The Background Removal tab shown instead of the others while removing a background.
pub(super) fn background_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: "slideNative.backgroundRemoval",
        groups: vec![
            Group {
                label: "slideNative.refine",
                large: vec![L("sl.bgKeep"), L("sl.bgRemove")],
                rows: vec![],
            },
            Group {
                label: "slideNative.close",
                large: vec![L("sl.bgDiscard"), L("sl.bgDone")],
                rows: vec![],
            },
        ],
    }
}

/// Own icons (own drawings).
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    let sky = Rgba(0xcf, 0xe4, 0xf6, 255);
    let hill = Rgba(0x3f, 0x8f, 0x4d, 255);
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    let frame = "M3 4h18v16h-18Z";
    // Remove Background: a picture whose left half is checkered (transparent).
    icons.add_solid(
        "sl.removeBg",
        &[
            ("M12 4h9v16h-9Z", Some(sky)),
            ("M12 20l4-6l5 6Z", Some(hill)),
            ("M3 4h3v3h-3ZM9 4h3v3h-3ZM6 7h3v3h-3ZM3 10h3v3h-3ZM9 10h3v3h-3ZM6 13h3v3h-3ZM3 16h3v3h-3ZM9 16h3v3h-3Z", Some(Rgba(0xc8, 0xc8, 0xc8, 255))),
            ("M12 7a3 3 0 1 0 0.01 0Z", Some(blue)),
            (frame, None),
        ],
    );
    // Corrections: a sun.
    icons.add_solid(
        "sl.corrections",
        &[
            ("M12 7a5 5 0 1 0 0.01 0Z", Some(Rgba(0xf5, 0xb8, 0x2e, 255))),
            (
                "M11 1h2v4h-2ZM11 19h2v4h-2ZM1 11h4v2h-4ZM19 11h4v2h-4ZM4.2 5.6l1.4-1.4l2.8 2.8l-1.4 1.4ZM15.6 17l1.4-1.4l2.8 2.8l-1.4 1.4ZM4.2 18.4l2.8-2.8l1.4 1.4l-2.8 2.8ZM15.6 7l2.8-2.8l1.4 1.4l-2.8 2.8Z",
                Some(Rgba(0xf5, 0xb8, 0x2e, 255)),
            ),
        ],
    );
    // Color: a picture in three bands of colour.
    icons.add_solid(
        "sl.picColor",
        &[
            ("M3 4h6v16h-6Z", Some(Rgba(0xd2, 0x4a, 0x3c, 255))),
            ("M9 4h6v16h-6Z", Some(Rgba(0x3f, 0xa3, 0x4d, 255))),
            ("M15 4h6v16h-6Z", Some(blue)),
            (frame, None),
        ],
    );
    // Transparency: a picture fading into a checkerboard.
    icons.add_solid(
        "sl.transparency",
        &[
            ("M3 4h18v16h-18Z", Some(Rgba(0xe8, 0xe8, 0xe8, 255))),
            ("M3 4h9v16h-9Z", Some(sky)),
            ("M3 20l5-7l4 7Z", Some(hill)),
            (
                "M15 4h3v4h-3ZM12 8h3v4h-3ZM18 8h3v4h-3ZM15 12h3v4h-3ZM12 16h3v4h-3ZM18 16h3v4h-3Z",
                Some(Rgba(0xb4, 0xb4, 0xb4, 255)),
            ),
            (frame, None),
        ],
    );
    // Compress Pictures: a picture with arrows pressing inwards.
    icons.add_solid(
        "sl.compress",
        &[
            ("M6 7h12v10h-12Z", Some(sky)),
            ("M6 17l4-5l3 3l2-2l3 4Z", Some(hill)),
            ("M6 7h12v10h-12Z", None),
            ("M1 1l5 5M23 1l-5 5M1 23l5-5M23 23l-5-5", None),
        ],
    );
    // Change Picture: a picture with a turning arrow.
    icons.add_solid(
        "sl.changePicture",
        &[
            ("M3 4h14v12h-14Z", Some(sky)),
            ("M3 16l5-6l3 3l2-2l4 5Z", Some(hill)),
            ("M3 4h14v12h-14Z", None),
            ("M14 20a5 5 0 0 0 8-4", None),
            ("M22 13l1.5 4h-4Z", Some(blue)),
        ],
    );
    // Background Removal tab: a pencil with a plus / minus, a cross, a tick.
    let pencil = "M15 3l4 4l-11 11l-5 1l1-5Z";
    for (id, sign, color) in [
        (
            "sl.bgKeep",
            "M15 15h2v3h3v2h-3v3h-2v-3h-3v-2h3Z",
            Rgba(0x3f, 0xa3, 0x4d, 255),
        ),
        ("sl.bgRemove", "M12 18h8v2h-8Z", Rgba(0xd2, 0x4a, 0x3c, 255)),
    ] {
        icons.add_solid(
            id,
            &[
                (pencil, Some(Rgba(0xf4, 0xf4, 0xf4, 255))),
                (pencil, None),
                (sign, Some(color)),
            ],
        );
    }
    icons.add_solid(
        "sl.bgDiscard",
        &[(
            "M5 3l7 7l7-7l2 2l-7 7l7 7l-2 2l-7-7l-7 7l-2-2l7-7l-7-7Z",
            Some(Rgba(0xd2, 0x4a, 0x3c, 255)),
        )],
    );
    icons.add_solid(
        "sl.bgDone",
        &[(
            "M2 12l3-3l5 5l10-10l3 3l-13 13Z",
            Some(Rgba(0x3f, 0xa3, 0x4d, 255)),
        )],
    );
}

/// Name the commands.
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in LABELS {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
    // Icons only at this width (the name is the tooltip).
    for id in ["sl.compress", "sl.changePicture", "sl.resetPicture"] {
        let v = s.get(&format!("commands.{id}")).to_owned();
        s.set(&format!("commands.tip.{id}"), &v);
    }
}

/// Which gallery of picture previews is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalleryKind {
    /// Sharpen / soften and brightness / contrast.
    Corrections,
    /// Saturation and recolour.
    Color,
    /// Transparency.
    Transparency,
}

/// A gallery of previews of the selected picture under a ribbon button.
#[derive(Debug, Clone)]
pub struct PicGallery {
    /// The ribbon command it hangs from.
    pub anchor: &'static str,
    /// Outer frame.
    pub rect: [f32; 4],
    /// Section headings (y, title).
    pub heads: Vec<(f32, String)>,
    /// Cells: rectangle, adjustments, the image (store key offset).
    pub cells: Vec<([f32; 4], PictureFx)>,
    /// Picture size in the cells (device px), centred.
    pub pic: (f32, f32),
    /// Hovered cell.
    pub hover: Option<usize>,
    /// The current adjustments (framed).
    pub current: PictureFx,
    /// The media and the previews' store keys are up to date.
    pub drawn: bool,
    /// The picture's media part.
    pub media: String,
}

/// Brightness / contrast steps of the Corrections gallery (−40 % … +40 %).
const STEPS: [i32; 5] = [-40_000, -20_000, 0, 20_000, 40_000];
/// Saturation steps of the Color gallery.
const SATURATIONS: [i32; 7] = [0, 33_000, 66_000, 100_000, 133_000, 200_000, 300_000];

impl PicGallery {
    fn sections(
        kind: GalleryKind,
        current: &PictureFx,
        accents: &[Rgba],
    ) -> Vec<(String, Vec<PictureFx>)> {
        let base = PictureFx {
            alpha: current.alpha,
            ..PictureFx::default()
        };
        match kind {
            GalleryKind::Corrections => vec![
                (
                    "sharpenSoften".into(),
                    [-50_000, -25_000, 0, 25_000, 50_000]
                        .iter()
                        .map(|s| PictureFx {
                            sharpen: *s,
                            ..current.clone()
                        })
                        .collect(),
                ),
                (
                    "brightnessContrast".into(),
                    STEPS
                        .iter()
                        .rev()
                        .flat_map(|c| STEPS.iter().map(move |b| (b, c)))
                        .map(|(b, c)| PictureFx {
                            bright: *b,
                            contrast: *c,
                            ..current.clone()
                        })
                        .collect(),
                ),
            ],
            GalleryKind::Color => {
                let mut recolor = vec![
                    base.clone(),
                    PictureFx {
                        grayscale: true,
                        ..base.clone()
                    },
                    PictureFx {
                        duotone: Some([0x4a_3519, 0xf2_e6d0]),
                        ..base.clone()
                    },
                    PictureFx {
                        bright: 70_000,
                        contrast: -70_000,
                        ..base.clone()
                    },
                    PictureFx {
                        bilevel: Some(50_000),
                        ..base.clone()
                    },
                ];
                for c in accents.iter().take(6) {
                    let rgb = (u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2);
                    recolor.push(PictureFx {
                        duotone: Some([rgb, 0xff_ffff]),
                        ..base.clone()
                    });
                }
                vec![
                    (
                        "saturation".into(),
                        SATURATIONS
                            .iter()
                            .map(|s| PictureFx {
                                saturation: (*s != 100_000).then_some(*s),
                                grayscale: false,
                                ..current.clone()
                            })
                            .collect(),
                    ),
                    ("recolor".into(), recolor),
                ]
            }
            GalleryKind::Transparency => vec![(
                "transparency".into(),
                [0, 15, 30, 50, 65, 80, 95]
                    .iter()
                    .map(|t| PictureFx {
                        alpha: (*t > 0).then_some(100_000 - t * 1000),
                        ..current.without_alpha()
                    })
                    .collect(),
            )],
        }
    }

    /// Lay out under `anchor` (window px) for a picture of `pw` × `ph` pixels; `titles`
    /// names the sections.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        kind: GalleryKind,
        anchor_id: &'static str,
        anchor: [f32; 4],
        window: (f32, f32),
        scale: f32,
        (pw, ph): (u32, u32),
        current: PictureFx,
        media: String,
        title: &dyn Fn(&str) -> String,
        accents: &[Rgba],
    ) -> PicGallery {
        let px = |v: f32| (v * scale).round();
        let cell = px(72.0);
        let gap = px(6.0);
        let head = px(24.0);
        let pad = px(8.0);
        let cols = match kind {
            GalleryKind::Corrections => 5,
            GalleryKind::Color | GalleryKind::Transparency => 7,
        };
        let w = pad * 2.0 + cols as f32 * cell + (cols - 1) as f32 * gap;
        let mut x = anchor[0];
        if x + w > window.0 {
            x = (window.0 - w).max(0.0);
        }
        let top = anchor[1] + anchor[3];
        let mut y = top + pad;
        let mut heads = Vec::new();
        let mut cells = Vec::new();
        for (key, list) in Self::sections(kind, &current, accents) {
            heads.push((y, title(&key)));
            y += head;
            for (i, fx) in list.into_iter().enumerate() {
                let (c, r) = (i % cols, i / cols);
                let cx = x + pad + c as f32 * (cell + gap);
                let cy = y + r as f32 * (cell + gap);
                cells.push(([cx, cy, cell, cell], fx));
            }
            let rows = cells.len().div_ceil(cols).max(1);
            let in_section = cells
                .iter()
                .filter(|(r, _)| r[1] >= y)
                .count()
                .div_ceil(cols)
                .min(rows);
            y += in_section as f32 * (cell + gap) + pad;
        }
        // Picture fitted in a cell, keeping its proportions.
        let inner = cell - px(6.0);
        let (pw, ph) = (pw.max(1) as f32, ph.max(1) as f32);
        let k = (inner / pw).min(inner / ph);
        PicGallery {
            anchor: anchor_id,
            rect: [x, top, w, y - top],
            heads,
            cells,
            pic: ((pw * k).round().max(1.0), (ph * k).round().max(1.0)),
            hover: None,
            current,
            drawn: false,
            media,
        }
    }

    /// Whether a window position is inside.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        crate::spin_box::inside(self.rect, x, y)
    }

    /// The cell at a window position.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<usize> {
        self.cells
            .iter()
            .position(|(r, _)| crate::spin_box::inside(*r, x, y))
    }

    /// Bring the preview images up to date in `store`.
    pub fn prepare(
        &mut self,
        store: &mut dyn PageStore,
        engine: &bo_engine::Engine,
    ) -> Result<(), String> {
        if self.drawn {
            return Ok(());
        }
        let (w, h) = self.pic;
        for (i, (_, fx)) in self.cells.iter().enumerate() {
            let list = PageList {
                width: w,
                height: h,
                commands: vec![Command::Image {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                    image: fx.clamped().derived_id(&self.media),
                    crop: None,
                    rotation: 0.0,
                }],
            };
            store.update(GALLERY_KEY + i, &list, engine, 1.0, None)?;
        }
        self.drawn = true;
        Ok(())
    }

    /// Draw the gallery.
    pub fn paint(&self, p: &mut dyn Painter, ink: Rgba, scale: f32) {
        let [x, y, w, h] = self.rect;
        p.fill_rect(
            x - 1.0,
            y - 1.0,
            w + 2.0,
            h + 2.0,
            Rgba(0xc6, 0xc6, 0xc6, 255),
        );
        p.fill_rect(x, y, w, h, Rgba(255, 255, 255, 255));
        let style = TextStyle {
            size: (12.0 * scale).round(),
            bold: true,
        };
        for (hy, t) in &self.heads {
            p.fill_rect(
                x,
                *hy,
                w,
                (20.0 * scale).round(),
                Rgba(0xf0, 0xf0, 0xf0, 255),
            );
            p.text(
                t,
                x + (8.0 * scale).round(),
                *hy + (2.0 * scale).round(),
                style,
                ink,
            );
        }
        let (pw, ph) = self.pic;
        for (i, (r, fx)) in self.cells.iter().enumerate() {
            let chosen = *fx == self.current;
            if self.hover == Some(i) || chosen {
                let c = if self.hover == Some(i) {
                    Rgba(0xf2, 0x9b, 0x4c, 255)
                } else {
                    Rgba(0x88, 0x88, 0x88, 255)
                };
                p.fill_rect(r[0], r[1], r[2], r[3], c);
                p.fill_rect(
                    r[0] + 2.0,
                    r[1] + 2.0,
                    r[2] - 4.0,
                    r[3] - 4.0,
                    Rgba(255, 255, 255, 255),
                );
            }
            let px = (r[0] + (r[2] - pw) / 2.0).round();
            let py = (r[1] + (r[3] - ph) / 2.0).round();
            // Transparency shows over a checkerboard.
            if fx.alpha.is_some() {
                let s = (6.0 * scale).round().max(2.0);
                let mut yy = 0.0;
                let mut row = 0;
                while yy < ph {
                    let mut xx = if row % 2 == 0 { 0.0 } else { s };
                    while xx < pw {
                        p.fill_rect(
                            px + xx,
                            py + yy,
                            s.min(pw - xx),
                            s.min(ph - yy),
                            Rgba(0xd8, 0xd8, 0xd8, 255),
                        );
                        xx += 2.0 * s;
                    }
                    yy += s;
                    row += 1;
                }
            }
            p.page(GALLERY_KEY + i, px as i32, py as i32);
        }
    }
}

/// The background removal mode.
#[derive(Debug, Clone, Default)]
pub struct BgMode {
    /// Settings so far (the subject rectangle and the strokes).
    pub spec: RemovalSpec,
    /// Marking with the pointer: keep (`true`) or remove.
    pub marking: Option<bool>,
    /// The stroke being drawn (fractions of the picture).
    pub stroke: Vec<[f64; 2]>,
    /// Preview id in the engine's images and whether the store has it.
    pub preview: Option<String>,
    /// Preview drawn into the store.
    pub drawn: bool,
    /// Previews made (each gets a new id).
    pub seq: u32,
    /// Share kept (1/1000 %) of the last preview.
    pub kept: u32,
    /// Picture box on the slide in points: x, y, w, h.
    pub frame: [f64; 4],
}

impl SlideWindow {
    /// The selected picture's info (media, pixels, adjustments).
    pub(super) fn picture_info(&mut self) -> Option<bo_engine::picture_tools::PictureInfo> {
        self.view.run_picture(P::PictureInfo);
        self.view.state()?.picture.as_deref().cloned()
    }

    /// Open a gallery of previews under its button.
    fn open_gallery(&mut self, kind: GalleryKind, anchor: &'static str) -> bool {
        let Some(info) = self.picture_info() else {
            return false;
        };
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(anchor)) else {
            return false;
        };
        let accents = self.theme_colors();
        let strings = &self.strings;
        let title = |k: &str| strings.get(&format!("slideNative.{k}")).to_owned();
        let g = PicGallery::open(
            kind,
            anchor,
            r,
            self.size,
            self.scale,
            (info.pixel_width, info.pixel_height),
            info.fx.clone(),
            info.media.clone(),
            &title,
            &accents,
        );
        self.menu = None;
        self.picker = None;
        self.gallery = Some(g);
        true
    }

    /// Input while a gallery is open.
    pub(super) fn gallery_input(&mut self, input: &Input) -> Option<bool> {
        let g = self.gallery.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => {
                let h = g.cell_at(x, y);
                let changed = h != g.hover;
                g.hover = h;
                changed
            }
            Input::Press { x, y, .. } => {
                let cell = g
                    .cell_at(x, y)
                    .and_then(|i| g.cells.get(i))
                    .map(|(_, f)| f.clone());
                let inside = g.contains(x, y);
                if cell.is_some() || !inside {
                    self.gallery = None;
                    self.ribbon.clear_pointer();
                }
                if let Some(fx) = cell {
                    self.view.run_picture(P::PictureFx { fx: fx.clamped() });
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.gallery = None;
                true
            }
            Input::Release { .. } | Input::Text(_) | Input::Key(..) | Input::Wheel(_) => true,
            _ => return None,
        })
    }

    /// Draw gallery previews and the removal preview into the store.
    pub(super) fn prepare_picture(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        if let Some(g) = self.gallery.as_mut() {
            g.prepare(store, self.view.engine())?;
        }
        if let Some(m) = self.bg_mode.as_mut() {
            if let (false, Some(id)) = (m.drawn, m.preview.clone()) {
                let k = self.view.scale();
                let (w, h) = (
                    (m.frame[2] as f32 * k).round(),
                    (m.frame[3] as f32 * k).round(),
                );
                let list = PageList {
                    width: w.max(1.0),
                    height: h.max(1.0),
                    commands: vec![Command::Image {
                        x: 0.0,
                        y: 0.0,
                        w: w.max(1.0),
                        h: h.max(1.0),
                        image: id,
                        crop: None,
                        rotation: 0.0,
                    }],
                };
                store.update(BG_KEY, &list, self.view.engine(), 1.0, None)?;
                m.drawn = true;
            }
        }
        Ok(())
    }

    /// Paint the open gallery.
    pub(super) fn paint_gallery(&self, p: &mut dyn Painter) {
        if let Some(g) = &self.gallery {
            g.paint(p, self.theme.text, self.scale);
        }
    }

    /// Paint the removal preview over the picture.
    pub(super) fn paint_removal(&self, p: &mut dyn Painter) {
        if let Some(m) = &self.bg_mode {
            if m.preview.is_some() && m.drawn {
                let (ox, oy) = self.view.slide_origin();
                let k = self.view.scale();
                let x = (ox + m.frame[0] as f32 * k).round();
                let y = (oy + m.frame[1] as f32 * k).round();
                let area = self.view.canvas_rect();
                p.push_clip(area[0], area[1], area[2], area[3]);
                p.page(BG_KEY, x as i32, y as i32);
                // The stroke being drawn.
                if m.stroke.len() > 1 {
                    let (w, h) = (m.frame[2] as f32 * k, m.frame[3] as f32 * k);
                    let segs: Vec<bo_render::PathSeg> = m
                        .stroke
                        .iter()
                        .enumerate()
                        .map(|(i, [u, v])| {
                            let (px, py) = (x + *u as f32 * w, y + *v as f32 * h);
                            if i == 0 {
                                bo_render::PathSeg::M(px, py)
                            } else {
                                bo_render::PathSeg::L(px, py)
                            }
                        })
                        .collect();
                    let c = if m.marking == Some(true) {
                        Rgba(0x3f, 0xa3, 0x4d, 255)
                    } else {
                        Rgba(0xd2, 0x4a, 0x3c, 255)
                    };
                    p.path(&segs, None, Some((c, (2.0 * self.scale).round())));
                }
                p.pop_clip();
            }
        }
    }

    /// Start removing the selected picture's background: the Background Removal tab and a
    /// first preview with the default subject rectangle.
    fn start_bg_removal(&mut self) -> bool {
        let Some(info) = self.picture_info() else {
            return false;
        };
        if !info.removable {
            return false;
        }
        let Some(s) = self.view.selection().first().map(|s| s.rect) else {
            return false;
        };
        self.bg_mode = Some(BgMode {
            frame: [s.cx - s.w / 2.0, s.cy - s.h / 2.0, s.w, s.h],
            ..BgMode::default()
        });
        self.refresh_bg_preview();
        self.relayout_tabs();
        true
    }

    /// Run the segmentation with the current marks and show its preview.
    fn refresh_bg_preview(&mut self) {
        let Some(spec) = self.bg_mode.as_ref().map(|m| m.spec.clone()) else {
            return;
        };
        self.view.run_picture(P::PictureBackgroundPreview(spec));
        let prev = self
            .view
            .state()
            .and_then(|s| s.picture.as_ref())
            .and_then(|p| p.background.clone());
        let Some(m) = self.bg_mode.as_mut() else {
            return;
        };
        let Some(prev) = prev else { return };
        let Ok(png) = base64::engine::general_purpose::STANDARD.decode(prev.image.trim()) else {
            return;
        };
        m.seq += 1;
        let id = format!("/ui/background-preview-{}.png", m.seq);
        if let Some(old) = m.preview.take() {
            self.view.engine_mut().remove_ui_image(&old);
        }
        self.view.engine_mut().set_ui_image(&id, png);
        m.preview = Some(id);
        m.kept = prev.kept;
        m.drawn = false;
    }

    /// Leave the mode (keeping the result when `keep`).
    fn end_bg_removal(&mut self, keep: bool) -> bool {
        let Some(m) = self.bg_mode.take() else {
            return false;
        };
        if let Some(id) = &m.preview {
            self.view.engine_mut().remove_ui_image(id);
        }
        let done = keep && self.view.run_picture(P::PictureRemoveBackground(m.spec));
        self.relayout_tabs();
        done || !keep
    }

    /// Rebuild the tabs for the removal mode (its own tab only) or the normal ones.
    fn relayout_tabs(&mut self) {
        if self.bg_mode.is_some() {
            self.ribbon.set_tabs(vec![background_tab()]);
            self.ribbon.set_active_tab(0);
        } else {
            let mut tabs = self.fixed_tabs();
            tabs.extend(super::ins::context_tabs(self.context));
            self.ribbon.set_tabs(tabs);
            self.ribbon
                .set_active_tab(if self.context == super::ins::Context::None {
                    self.last_base_tab
                } else {
                    self.base_tabs
                });
        }
        self.relayout_pending = true;
    }

    /// A slide point (points) as a fraction of the picture in removal.
    fn bg_fraction(&self, x: f32, y: f32) -> Option<[f64; 2]> {
        let m = self.bg_mode.as_ref()?;
        let (ox, oy) = self.view.slide_origin();
        let k = self.view.scale();
        let u = (f64::from((x - ox) / k) - m.frame[0]) / m.frame[2].max(1e-6);
        let v = (f64::from((y - oy) / k) - m.frame[1]) / m.frame[3].max(1e-6);
        Some([u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)])
    }

    /// Pointer input on the slide while removing a background: strokes marking areas.
    pub(super) fn removal_input(&mut self, input: &Input) -> Option<bool> {
        let m = self.bg_mode.as_ref()?;
        let area = self.view.canvas_rect();
        let marking = m.marking;
        match *input {
            Input::Press { x, y, .. } if crate::spin_box::inside(area, x, y) => {
                // Without a marking tool a press on the slide does nothing in this mode.
                if marking.is_none() {
                    return Some(false);
                }
                let f = self.bg_fraction(x, y)?;
                let m = self.bg_mode.as_mut()?;
                m.stroke = vec![f];
                Some(true)
            }
            Input::Drag { x, y } if marking.is_some() => {
                let f = self.bg_fraction(x, y)?;
                let m = self.bg_mode.as_mut()?;
                if m.stroke.is_empty() {
                    return Some(false);
                }
                m.stroke.push(f);
                Some(true)
            }
            Input::Release { .. } if marking.is_some() => {
                let m = self.bg_mode.as_mut()?;
                if m.stroke.is_empty() {
                    return None;
                }
                let mut points = std::mem::take(&mut m.stroke);
                if points.len() == 1 {
                    points.push(points[0]);
                }
                m.spec.marks.push(MarkSpec {
                    keep: marking == Some(true),
                    points,
                });
                self.refresh_bg_preview();
                Some(true)
            }
            Input::Key(Key::Escape, _) => Some(self.end_bg_removal(false)),
            Input::Key(Key::Enter, _) => Some(self.end_bg_removal(true)),
            // Typing and other keys do nothing to the slide in this mode.
            Input::Text(_) | Input::Key(..) => Some(false),
            _ => None,
        }
    }

    /// Compress Pictures: resolution and options in a dialog.
    pub(super) fn open_compress(&mut self) -> bool {
        let label = |k: &str| self.strings.get(&format!("slideNative.{k}")).to_owned();
        let d = Dialog::new(
            DialogKind::Comment,
            label("compressPictures"),
            vec![(label("resolutionPpi"), "150".into())],
            vec![
                (label("ok"), DialogButton::Ok),
                (label("cancel"), DialogButton::Cancel),
            ],
            Vec::new(),
        )
        .with_checks(vec![
            (label("onlyThisPicture"), true),
            (label("deleteCropped"), true),
        ]);
        self.menu = None;
        self.picker = None;
        self.dialog = Some((d, super::ins::SlideDialog::Compress));
        self.dialog_layout = true;
        true
    }

    /// The Compress Pictures dialog's button.
    pub(super) fn compress_button(&mut self, b: DialogButton, values: &[String]) {
        if b != DialogButton::Ok {
            self.dialog = None;
            return;
        }
        let on = |i: usize| values.get(i).is_some_and(|v| v == "1");
        let ppi = values
            .first()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| (20.0..=1000.0).contains(v));
        let ok =
            self.view
                .run_picture(P::PictureCompress(bo_engine::picture_tools::CompressSpec {
                    ppi,
                    drop_cropped: on(2),
                    jpeg_quality: Some(85),
                    all: !on(1),
                }));
        let report = self
            .view
            .state()
            .and_then(|s| s.picture.as_ref())
            .and_then(|p| p.compressed);
        let msg = match report {
            Some(r) if ok => self
                .strings
                .get("slideNative.compressed")
                .replace("{{count}}", &r.pictures.to_string())
                .replace("{{kb}}", &(r.saved() / 1024).to_string()),
            _ => self.strings.get("slideNative.notCompressed").to_owned(),
        };
        // The dialog stays with the result until it is closed.
        if let Some((d, _)) = self.dialog.as_mut() {
            d.set_message(vec![msg]);
        }
    }

    /// Run one of the Picture Format / Background Removal commands.
    pub(super) fn picture_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "sl.removeBg" => self.start_bg_removal(),
            "sl.corrections" => self.open_gallery(GalleryKind::Corrections, "sl.corrections"),
            "sl.picColor" => self.open_gallery(GalleryKind::Color, "sl.picColor"),
            "sl.transparency" => self.open_gallery(GalleryKind::Transparency, "sl.transparency"),
            "sl.compress" => self.open_compress(),
            "sl.changePicture" => {
                self.file_request = Some(InsertFile::ChangePicture);
                true
            }
            "sl.bgKeep" | "sl.bgRemove" => {
                let keep = id == "sl.bgKeep";
                if let Some(m) = self.bg_mode.as_mut() {
                    m.marking = if m.marking == Some(keep) {
                        None
                    } else {
                        Some(keep)
                    };
                }
                true
            }
            "sl.bgDiscard" => self.end_bg_removal(false),
            "sl.bgDone" => self.end_bg_removal(true),
            _ => return None,
        })
    }

    /// Commands shown as on: the marking tool in use.
    pub(super) fn picture_toggled(&self, on: &mut Vec<&'static str>) {
        if let Some(g) = &self.gallery {
            on.push(g.anchor);
        }
        if let Some(m) = &self.bg_mode {
            match m.marking {
                Some(true) => on.push("sl.bgKeep"),
                Some(false) => on.push("sl.bgRemove"),
                None => {}
            }
        }
    }

    /// A file the host picked for request kind.
    pub fn insert_picked(&mut self, kind: InsertFile, bytes: Vec<u8>, name: &str) -> bool {
        if kind == InsertFile::ChangePicture {
            return self.change_picture(&bytes, name);
        }
        self.insert_file_as(bytes, name, kind == InsertFile::AlbumPicture)
    }

    /// Replace the selected picture by a picked file (Change Picture).
    fn change_picture(&mut self, bytes: &[u8], name: &str) -> bool {
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        self.view.run_picture(P::PictureChange {
            data,
            name: name.to_owned(),
        })
    }
}
