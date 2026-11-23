//! Design tab drop-downs of BlueDoc, laid out after a desktop word processor's: Themes as a
//! grid of thumbnails ("Aa" in the heading face over the six accent colours), Colors as rows
//! of eight swatches with the palette name, Fonts as the heading / body pair written in their
//! own faces, Paragraph Spacing as named spacing sets. The themes, palettes and font pairs
//! are BlueOffice's own (`bo_edit::design::THEMES`, `STYLE_SETS`).
use crate::theme::Theme;
use crate::{Font, Painter, TextMeasure, TextStyle};
use bo_edit::design::{ThemeDef, STYLE_SETS, THEMES};
use bo_render::Rgba;

const MENU_EDGE: Rgba = Rgba(198, 198, 198, 255);
const CELL_EDGE: Rgba = Rgba(225, 225, 225, 255);
const MUTED: Rgba = Rgba(96, 96, 96, 255);

/// Which drop-down is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesignMenu {
    Themes,
    Colors,
    Fonts,
    Spacing,
    /// Document style sets as a list (the inline gallery's ▾).
    StyleSets,
}

/// Parse `RRGGBB`.
fn rgb(hex: &str) -> Rgba {
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    Rgba(
        ((v >> 16) & 0xff) as u8,
        ((v >> 8) & 0xff) as u8,
        (v & 0xff) as u8,
        255,
    )
}

/// Display name of a theme (the part after "BlueOffice ").
fn short_name(t: &ThemeDef) -> &'static str {
    t.name.strip_prefix("BlueOffice ").unwrap_or(t.name)
}

/// The open drop-down.
pub(crate) struct DesignGallery {
    pub(crate) menu: DesignMenu,
    x: f32,
    y: f32,
    scale: f32,
    hover: Option<usize>,
    /// Labels of the spacing sets (translated by the window).
    spacing_labels: Vec<String>,
    /// Heading of the drop-down (translated).
    title: String,
    /// Theme in use (drawn framed).
    pub(crate) current: Option<String>,
    /// Widths of the font-pair names, measured.
    widths: Vec<f32>,
    /// Command rows under the entries (round 15): Themes › Reset to Theme from Template,
    /// Browse for Themes…, Save Current Theme…; Colors › Customize Colors…; Fonts ›
    /// Customize Fonts….
    pub(crate) footer: Vec<String>,
    footer_hover: Option<usize>,
}

impl DesignGallery {
    pub(crate) fn open(
        menu: DesignMenu,
        anchor: [f32; 4],
        title: String,
        spacing_labels: Vec<String>,
        scale: f32,
    ) -> Self {
        Self {
            menu,
            x: anchor[0],
            y: anchor[1] + anchor[3],
            scale,
            hover: None,
            spacing_labels,
            title,
            current: None,
            widths: Vec::new(),
            footer: Vec::new(),
            footer_hover: None,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn count(&self) -> usize {
        match self.menu {
            DesignMenu::Spacing => STYLE_SETS.len(),
            DesignMenu::StyleSets => bo_edit::design::DOC_STYLE_SETS.len(),
            _ => THEMES.len(),
        }
    }

    fn title_h(&self) -> f32 {
        self.px(28.0)
    }

    /// Size of an entry: thumbnails for Themes, rows for the others.
    fn cell(&self) -> (f32, f32) {
        match self.menu {
            DesignMenu::Themes => (self.px(82.0), self.px(80.0)),
            DesignMenu::Colors => (self.px(300.0), self.px(26.0)),
            DesignMenu::Fonts => (self.px(300.0), self.px(54.0)),
            DesignMenu::Spacing => (self.px(300.0), self.px(36.0)),
            DesignMenu::StyleSets => (self.px(220.0), self.px(28.0)),
        }
    }

    fn cols(&self) -> usize {
        if self.menu == DesignMenu::Themes {
            4
        } else {
            1
        }
    }

    fn origin(&self) -> (f32, f32) {
        (
            self.x + self.px(6.0),
            self.y + self.title_h() + self.px(4.0),
        )
    }

    fn size(&self) -> (f32, f32) {
        let (cw, ch) = self.cell();
        let rows = self.count().div_ceil(self.cols());
        (
            self.cols() as f32 * cw + self.px(12.0),
            self.title_h() + self.px(10.0) + rows as f32 * ch + self.footer_h(),
        )
    }

    fn rect_of(&self, i: usize) -> [f32; 4] {
        let (cw, ch) = self.cell();
        let (ox, oy) = self.origin();
        let cols = self.cols();
        [
            ox + (i % cols) as f32 * cw,
            oy + (i / cols) as f32 * ch,
            cw,
            ch,
        ]
    }

    fn index_at(&self, x: f32, y: f32) -> Option<usize> {
        (0..self.count()).find(|i| {
            let r = self.rect_of(*i);
            x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
        })
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.index_at(x, y);
        let f = self.footer_at(x, y);
        let changed = h != self.hover || f != self.footer_hover;
        self.hover = h;
        self.footer_hover = f;
        changed
    }

    /// Height of a command row under the entries (36 px at 125 %, as the reference).
    fn footer_row(&self) -> f32 {
        self.px(28.8)
    }

    fn footer_h(&self) -> f32 {
        if self.footer.is_empty() {
            0.0
        } else {
            self.footer.len() as f32 * self.footer_row() + self.px(4.0)
        }
    }

    /// Top of the command rows (below a rule).
    fn footer_top(&self) -> f32 {
        let (_, h) = self.size();
        self.y + h - self.footer_h() + self.px(3.0)
    }

    /// The command row under (x, y).
    fn footer_at(&self, x: f32, y: f32) -> Option<usize> {
        let (w, _) = self.size();
        if self.footer.is_empty() || x < self.x || x >= self.x + w {
            return None;
        }
        let top = self.footer_top();
        (y >= top)
            .then(|| ((y - top) / self.footer_row()) as usize)
            .filter(|i| *i < self.footer.len())
    }

    /// The engine command a press picks (JSON), if any.
    pub(crate) fn press(&self, x: f32, y: f32) -> Option<serde_json::Value> {
        if let Some(i) = self.footer_at(x, y) {
            let menu = match self.menu {
                DesignMenu::Themes => "themes",
                DesignMenu::Colors => "colors",
                DesignMenu::Fonts => "fonts",
                _ => "other",
            };
            return Some(
                serde_json::json!({ "cmd": "ui.action", "id": format!("design.footer.{menu}.{i}") }),
            );
        }
        let i = self.index_at(x, y)?;
        Some(match self.menu {
            DesignMenu::Spacing => {
                serde_json::json!({ "cmd": "applyStyleSet", "id": STYLE_SETS.get(i)?.id })
            }
            DesignMenu::StyleSets => serde_json::json!({
                "cmd": "applyDocumentStyleSet",
                "id": bo_edit::design::DOC_STYLE_SETS.get(i)?.id,
            }),
            _ => serde_json::json!({ "cmd": "applyTheme", "id": THEMES.get(i)?.id }),
        })
    }

    /// Centre of entry `i` (tests).
    #[cfg(test)]
    pub(crate) fn center(&self, i: usize) -> (f32, f32) {
        let r = self.rect_of(i);
        (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0)
    }

    fn face(&self, family: &'static str, size: f32) -> Font<'static> {
        Font {
            family,
            size: self.px(size),
            weight: 400,
            italic: false,
            underline: false,
        }
    }

    /// Measure the texts drawn in document faces.
    pub(crate) fn measure(&mut self, m: &dyn TextMeasure) {
        self.widths = THEMES
            .iter()
            .map(|t| m.measure_font("Aa", &self.face(t.major, 22.0)).0)
            .collect();
    }

    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &Theme) {
        let (w, h) = self.size();
        p.fill_rect(self.x, self.y, w, h, MENU_EDGE);
        p.fill_rect(self.x + 1.0, self.y + 1.0, w - 2.0, h - 2.0, t.surface);
        let ui = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        // Heading band, as the reference's gallery sections ("Built-In").
        p.fill_rect(
            self.x + 1.0,
            self.y + 1.0,
            w - 2.0,
            self.title_h() - 1.0,
            Rgba(240, 240, 240, 255),
        );
        p.text(
            &self.title,
            self.x + self.px(10.0),
            self.y + (self.title_h() - ui.size * 1.33) / 2.0,
            TextStyle { bold: true, ..ui },
            t.text,
        );
        for i in 0..self.count() {
            let r = self.rect_of(i);
            let hovered = self.hover == Some(i);
            if hovered {
                p.fill_rect(r[0], r[1], r[2], r[3], t.hover);
            }
            match self.menu {
                DesignMenu::Themes => self.paint_theme(p, t, i, r),
                DesignMenu::Colors => {
                    let th = &THEMES[i];
                    // dk1 lt1 dk2 lt2 then the six accents, as the reference's palette rows.
                    let s = self.px(16.0);
                    let mut x = r[0] + self.px(6.0);
                    let y = r[1] + (r[3] - s) / 2.0;
                    for hex in th.colors.iter().take(10).skip(2) {
                        p.fill_rect(x, y, s, s, CELL_EDGE);
                        p.fill_rect(x + 1.0, y + 1.0, s - 2.0, s - 2.0, rgb(hex));
                        x += s + self.px(1.0);
                    }
                    p.text(
                        short_name(th),
                        x + self.px(10.0),
                        r[1] + (r[3] - ui.size * 1.33) / 2.0,
                        ui,
                        t.text,
                    );
                }
                DesignMenu::Fonts => {
                    let th = &THEMES[i];
                    p.text(
                        short_name(th),
                        r[0] + self.px(8.0),
                        r[1] + self.px(3.0),
                        TextStyle { bold: true, ..ui },
                        MUTED,
                    );
                    p.text_font(
                        "Aa",
                        r[0] + self.px(8.0),
                        r[1] + self.px(22.0),
                        &self.face(th.major, 18.0),
                        t.text,
                    );
                    p.text_font(
                        th.major,
                        r[0] + self.px(52.0),
                        r[1] + self.px(20.0),
                        &self.face(th.major, 12.0),
                        t.text,
                    );
                    p.text_font(
                        th.minor,
                        r[0] + self.px(52.0),
                        r[1] + self.px(36.0),
                        &self.face(th.minor, 11.0),
                        t.text,
                    );
                }
                DesignMenu::StyleSets => {
                    let label = self.spacing_labels.get(i).map_or("", String::as_str);
                    p.text(
                        label,
                        r[0] + self.px(10.0),
                        r[1] + (r[3] - ui.size * 1.33) / 2.0,
                        ui,
                        t.text,
                    );
                }
                DesignMenu::Spacing => {
                    let set = &STYLE_SETS[i];
                    // Three grey lines spaced as the set spaces text (own drawing).
                    let gap = (set.line as f32 / 240.0) * self.px(4.0);
                    for k in 0..3 {
                        p.fill_rect(
                            r[0] + self.px(8.0),
                            r[1] + self.px(9.0) + k as f32 * gap,
                            self.px(22.0),
                            self.px(2.0),
                            MUTED,
                        );
                    }
                    let label = self.spacing_labels.get(i).map_or(set.id, String::as_str);
                    p.text(
                        label,
                        r[0] + self.px(44.0),
                        r[1] + (r[3] - ui.size * 1.33) / 2.0,
                        ui,
                        t.text,
                    );
                }
            }
        }
        // Command rows under a rule (round 15).
        if !self.footer.is_empty() {
            let top = self.footer_top();
            p.fill_rect(self.x + 1.0, top - self.px(2.0), w - 2.0, 1.0, CELL_EDGE);
            for (i, label) in self.footer.iter().enumerate() {
                let ry = top + i as f32 * self.footer_row();
                if self.footer_hover == Some(i) {
                    p.fill_rect(self.x + 2.0, ry, w - 4.0, self.footer_row(), t.hover);
                }
                p.text(
                    label,
                    self.x + self.px(36.0),
                    ry + (self.footer_row() - ui.size * 1.33) / 2.0,
                    ui,
                    t.text,
                );
                if self.menu == DesignMenu::Themes && i > 0 {
                    let s = self.px(16.0);
                    footer_icon(
                        p,
                        [
                            self.x + self.px(10.0),
                            ry + (self.footer_row() - s) / 2.0,
                            s,
                            s,
                        ],
                        i == 2,
                    );
                }
            }
            // Size grip in the bottom-right corner, as the reference's galleries.
            let (gx, gy) = (self.x + w - self.px(3.0), self.y + h - self.px(3.0));
            let dot = self.px(1.6).max(1.0);
            for (dx, dy) in [
                (1.0, 3.0),
                (2.0, 2.0),
                (2.0, 3.0),
                (3.0, 1.0),
                (3.0, 2.0),
                (3.0, 3.0),
            ] {
                p.fill_rect(
                    gx - (4.0 - dx) * self.px(3.0) - dot,
                    gy - (4.0 - dy) * self.px(3.0) - dot,
                    dot,
                    dot,
                    Rgba(0x9a, 0x9a, 0x9a, 255),
                );
            }
        }
    }

    /// A theme thumbnail: white card, "Aa" in the heading face, six accent bars, the name
    /// under it; the theme in use framed in the accent colour.
    fn paint_theme(&self, p: &mut dyn Painter, t: &Theme, i: usize, r: [f32; 4]) {
        let th = &THEMES[i];
        let pad = self.px(5.0);
        let card = [r[0] + pad, r[1] + pad, r[2] - 2.0 * pad, self.px(52.0)];
        let on = self.current.as_deref() == Some(th.id);
        p.fill_rect(
            card[0],
            card[1],
            card[2],
            card[3],
            if on { t.accent } else { CELL_EDGE },
        );
        let b = if on { self.px(2.0) } else { 1.0 };
        p.fill_rect(
            card[0] + b,
            card[1] + b,
            card[2] - 2.0 * b,
            card[3] - 2.0 * b,
            Rgba(255, 255, 255, 255),
        );
        let aw = self.widths.get(i).copied().unwrap_or(self.px(26.0));
        p.text_font(
            "Aa",
            card[0] + (card[2] - aw) / 2.0,
            card[1] + self.px(4.0),
            &self.face(th.major, 22.0),
            rgb(th.colors[2]),
        );
        let bw = ((card[2] - self.px(12.0)) / 6.0).floor();
        for (k, hex) in th.colors[4..10].iter().enumerate() {
            p.fill_rect(
                card[0] + self.px(6.0) + k as f32 * bw,
                card[1] + card[3] - self.px(12.0),
                bw - 1.0,
                self.px(7.0),
                rgb(hex),
            );
        }
        let ui = TextStyle {
            size: self.px(11.0),
            bold: false,
        };
        p.text(
            short_name(th),
            card[0],
            card[1] + card[3] + self.px(3.0),
            ui,
            t.text,
        );
    }
}

/// Icons of Browse for Themes… (an open folder with a theme page) and Save Current Theme…
/// (a theme page over a disk), BlueOffice's own drawings.
fn footer_icon(p: &mut dyn Painter, r: [f32; 4], save: bool) {
    let k = r[2] / 16.0;
    let grey = Rgba(0x6e, 0x6e, 0x6e, 255);
    let page = |p: &mut dyn Painter, x: f32, y: f32| {
        p.fill_rect(x, y, 9.0 * k, 11.0 * k, grey);
        p.fill_rect(x + k, y + k, 7.0 * k, 9.0 * k, Rgba(255, 255, 255, 255));
        for (j, c) in [Rgba(0x2f, 0x6f, 0xba, 255), Rgba(0xd0, 0x6a, 0x2a, 255)]
            .into_iter()
            .enumerate()
        {
            p.fill_rect(
                x + (2.0 + 3.0 * j as f32) * k,
                y + 7.0 * k,
                2.0 * k,
                2.0 * k,
                c,
            );
        }
    };
    if save {
        page(p, r[0] + 6.0 * k, r[1]);
        let blue = Rgba(0x2f, 0x6f, 0xba, 255);
        p.fill_rect(r[0], r[1] + 7.0 * k, 9.0 * k, 9.0 * k, blue);
        p.fill_rect(
            r[0] + 2.0 * k,
            r[1] + 8.0 * k,
            5.0 * k,
            3.0 * k,
            Rgba(255, 255, 255, 255),
        );
    } else {
        let tan = Rgba(0xe8, 0xb8, 0x4a, 255);
        p.fill_rect(r[0], r[1] + 6.0 * k, 14.0 * k, 9.0 * k, tan);
        p.fill_rect(r[0], r[1] + 4.0 * k, 6.0 * k, 3.0 * k, tan);
        page(p, r[0] + 6.0 * k, r[1]);
    }
}
