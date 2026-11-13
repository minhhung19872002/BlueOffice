//! Ribbon: tab strip and command groups, self-drawn (ADR-0018). Command ids, labels and icons
//! are those of the React ribbon (`CommandRegistry`, i18n keys, `assets/icons/ui-icons.json`).
use crate::icons::{IconColors, Icons};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{commands, Painter, TextMeasure, TextStyle};
use bo_edit::FormatState;
use bo_render::Rgba;

/// `a` × `k` + `b` × (1 − `k`), channel by channel.
fn mix(a: Rgba, b: Rgba, k: f32) -> Rgba {
    let m = |x: u8, y: u8| (f32::from(x) * k + f32::from(y) * (1.0 - k)).round() as u8;
    Rgba(m(a.0, b.0), m(a.1, b.1), m(a.2, b.2), m(a.3, b.3))
}

/// One control of a group row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    /// Icon button running command `id`; the tooltip / accessible name is `commands.<id>`.
    Button(&'static str),
    /// Font family box (shows the selection's font).
    FontBox,
    /// Font size box.
    SizeBox,
}

struct Group {
    label: &'static str,
    rows: Vec<Vec<Item>>,
}

struct Tab {
    label: &'static str,
    groups: Vec<Group>,
}

/// What a pointer position is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonHit {
    /// The File button.
    File,
    /// Tab `n`.
    Tab(usize),
    /// A command button.
    Command(&'static str),
    /// The font or size box.
    Field,
}

#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

enum Placed {
    File(Rect),
    Tab(usize, Rect),
    Item(Item, Rect),
    GroupLabel(&'static str, Rect, f32),
    Divider(Rect),
}

/// The ribbon of BlueDoc.
pub struct Ribbon {
    tabs: Vec<Tab>,
    active: usize,
    scale: f32,
    width: f32,
    placed: Vec<Placed>,
    hover: Option<RibbonHit>,
    pressed: Option<RibbonHit>,
}

fn home() -> Tab {
    use Item::{Button as B, FontBox, SizeBox};
    Tab {
        label: "ribbon.tabs.home",
        groups: vec![
            Group {
                label: "ribbon.groups.history",
                rows: vec![vec![B("edit.undo")], vec![B("edit.redo")]],
            },
            Group {
                label: "ribbon.groups.font",
                rows: vec![
                    vec![FontBox, SizeBox],
                    vec![
                        B("format.bold"),
                        B("format.italic"),
                        B("format.underline"),
                        B("format.strike"),
                        B("format.subscript"),
                        B("format.superscript"),
                    ],
                ],
            },
            Group {
                label: "ribbon.groups.paragraph",
                rows: vec![
                    vec![
                        B("format.bullets"),
                        B("format.numbering"),
                        B("format.outdent"),
                        B("format.indent"),
                    ],
                    vec![
                        B("format.alignLeft"),
                        B("format.alignCenter"),
                        B("format.alignRight"),
                        B("format.justify"),
                    ],
                ],
            },
        ],
    }
}

fn tab(label: &'static str) -> Tab {
    Tab {
        label,
        groups: Vec::new(),
    }
}

impl Default for Ribbon {
    fn default() -> Self {
        Self::new()
    }
}

impl Ribbon {
    /// BlueDoc's tabs; the Home tab is active.
    pub fn new() -> Self {
        Self {
            tabs: vec![
                home(),
                tab("ribbon.tabs.insert"),
                tab("ribbon.tabs.design"),
                tab("ribbon.tabs.layout"),
                tab("ribbon.tabs.references"),
                tab("ribbon.tabs.mailings"),
                tab("ribbon.tabs.review"),
                tab("ribbon.tabs.view"),
            ],
            active: 0,
            scale: 1.0,
            width: 0.0,
            placed: Vec::new(),
            hover: None,
            pressed: None,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Height of the tab strip.
    fn strip_h(&self) -> f32 {
        self.px(36.0)
    }

    /// Height of the whole ribbon in device pixels.
    pub fn height(&self) -> f32 {
        self.strip_h() + self.px(92.0)
    }

    /// Lay the ribbon out for a window `width` device pixels wide at display `scale`.
    pub fn layout(&mut self, text: &dyn TextMeasure, strings: &Strings, width: f32, scale: f32) {
        self.scale = scale;
        self.width = width;
        self.placed.clear();
        let tab_style = TextStyle {
            size: self.px(13.0),
            bold: false,
        };
        // Tab strip: File button, then the tabs.
        let strip = self.strip_h();
        let pad = self.px(14.0);
        let mut x = self.px(6.0);
        let (fw, _) = text.measure(strings.get("ribbon.tabs.file"), tab_style);
        let file = Rect {
            x,
            y: self.px(4.0),
            w: fw + 2.0 * pad,
            h: strip - self.px(8.0),
        };
        x += file.w + self.px(4.0);
        self.placed.push(Placed::File(file));
        for (i, t) in self.tabs.iter().enumerate() {
            let (w, _) = text.measure(strings.get(t.label), tab_style);
            let r = Rect {
                x,
                y: 0.0,
                w: w + 2.0 * pad,
                h: strip,
            };
            x += r.w;
            self.placed.push(Placed::Tab(i, r));
        }
        // Groups of the active tab.
        let top = strip + self.px(6.0);
        let (btn, gap, row_gap) = (self.px(28.0), self.px(2.0), self.px(4.0));
        let label_h = self.px(18.0);
        let bottom = self.height() - self.px(4.0);
        let mut gx = self.px(8.0);
        let mut placed = Vec::new();
        for g in &self.tabs[self.active].groups {
            let mut group_w: f32 = 0.0;
            for (r, row) in g.rows.iter().enumerate() {
                let y = top + r as f32 * (btn + row_gap);
                let mut x = gx + self.px(4.0);
                for item in row {
                    let w = match item {
                        Item::Button(_) => btn,
                        Item::FontBox => self.px(150.0),
                        Item::SizeBox => self.px(52.0),
                    };
                    placed.push(Placed::Item(*item, Rect { x, y, w, h: btn }));
                    x += w + gap;
                }
                group_w = group_w.max(x - gap - gx + self.px(4.0));
            }
            let label = strings.get(g.label);
            let (lw, _) = text.measure(
                label,
                TextStyle {
                    size: self.px(12.0),
                    bold: false,
                },
            );
            group_w = group_w.max(lw + self.px(16.0));
            placed.push(Placed::GroupLabel(
                g.label,
                Rect {
                    x: gx,
                    y: bottom - label_h,
                    w: group_w,
                    h: label_h,
                },
                lw,
            ));
            gx += group_w;
            placed.push(Placed::Divider(Rect {
                x: gx,
                y: top,
                w: 1.0,
                h: bottom - top - self.px(2.0),
            }));
            gx += self.px(6.0);
        }
        self.placed.extend(placed);
    }

    /// What is under a window position.
    pub fn hit(&self, x: f32, y: f32) -> Option<RibbonHit> {
        self.placed.iter().find_map(|p| match p {
            Placed::File(r) if r.contains(x, y) => Some(RibbonHit::File),
            Placed::Tab(i, r) if r.contains(x, y) => Some(RibbonHit::Tab(*i)),
            Placed::Item(Item::Button(id), r) if r.contains(x, y) => Some(RibbonHit::Command(id)),
            Placed::Item(_, r) if r.contains(x, y) => Some(RibbonHit::Field),
            _ => None,
        })
    }

    /// Pointer moved: `true` when the hovered control changed.
    pub fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.hit(x, y);
        std::mem::replace(&mut self.hover, h) != h
    }

    /// Button pressed at a position: `true` when something changed.
    pub fn press(&mut self, x: f32, y: f32) -> bool {
        self.pressed = self.hit(x, y);
        self.pressed.is_some()
    }

    /// Button released: the control activated (pressed and released over the same one).
    /// Selecting a tab is handled here; `true` in the second value when a redraw is needed.
    pub fn release(&mut self, x: f32, y: f32) -> (Option<RibbonHit>, bool) {
        let was = self.pressed.take();
        let now = self.hit(x, y);
        let fired = was.filter(|w| Some(*w) == now);
        if let Some(RibbonHit::Tab(i)) = fired {
            self.active = i;
            self.placed.clear(); // re-laid out by the window before the next frame
        }
        (fired, was.is_some())
    }

    /// Whether the layout must be redone (tab change).
    pub fn needs_layout(&self) -> bool {
        self.placed.is_empty()
    }

    /// Width the ribbon was laid out for.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Paint the ribbon at the top of the window.
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        icons: &Icons,
        strings: &Strings,
        format: &FormatState,
    ) {
        let strip = self.strip_h();
        p.fill_rect(0.0, 0.0, self.width, self.height(), t.surface);
        // Line under the ribbon.
        p.fill_rect(0.0, self.height() - 1.0, self.width, 1.0, t.border);
        let tab_style = TextStyle {
            size: self.px(13.0),
            bold: false,
        };
        let center_text = |p: &mut dyn Painter, s: &str, r: &Rect, style: TextStyle, c: Rgba| {
            // Vertical centring uses the line box of the style (~1.33 em for Segoe UI).
            let line = style.size * 1.33;
            p.text(s, r.x + self.px(14.0), r.y + (r.h - line) / 2.0, style, c);
        };
        let tint = |hit: RibbonHit| {
            if self.pressed == Some(hit) {
                Some(t.active)
            } else if self.hover == Some(hit) {
                Some(t.hover)
            } else {
                None
            }
        };
        let radius_rect = |p: &mut dyn Painter, r: &Rect, c: Rgba| {
            p.fill_rect(r.x, r.y, r.w, r.h, c);
        };
        for placed in &self.placed {
            match placed {
                Placed::File(r) => {
                    radius_rect(p, r, t.accent_strong);
                    center_text(
                        p,
                        strings.get("ribbon.tabs.file"),
                        r,
                        tab_style,
                        t.accent_on,
                    );
                }
                Placed::Tab(i, r) => {
                    if let Some(c) = tint(RibbonHit::Tab(*i)) {
                        radius_rect(p, r, c);
                    }
                    let active = *i == self.active;
                    let style = TextStyle {
                        bold: active,
                        ..tab_style
                    };
                    let color = if active { t.accent_text } else { t.text };
                    center_text(p, strings.get(self.tabs[*i].label), r, style, color);
                    if active {
                        let h = self.px(3.0);
                        p.fill_rect(
                            r.x + self.px(10.0),
                            strip - h,
                            r.w - self.px(20.0),
                            h,
                            t.accent,
                        );
                    }
                }
                Placed::Item(Item::Button(id), r) => {
                    let on = commands::toggled(id, format);
                    let enabled = commands::enabled(id, format);
                    let hit = RibbonHit::Command(id);
                    if let Some(c) = tint(hit).filter(|_| enabled) {
                        radius_rect(p, r, c);
                    } else if on {
                        radius_rect(p, r, t.active);
                    }
                    let ink = if enabled { t.text } else { t.border };
                    let size = self.px(20.0);
                    icons.draw(
                        p,
                        id,
                        r.x + (r.w - size) / 2.0,
                        r.y + (r.h - size) / 2.0,
                        size,
                        IconColors {
                            ink,
                            accent: t.accent,
                            on: t.accent_on,
                        },
                    );
                }
                Placed::Item(field, r) => {
                    let inner = Rect {
                        x: r.x,
                        y: r.y + self.px(1.0),
                        w: r.w,
                        h: r.h - self.px(2.0),
                    };
                    p.fill_rect(inner.x, inner.y, inner.w, inner.h, t.border);
                    p.fill_rect(
                        inner.x + 1.0,
                        inner.y + 1.0,
                        inner.w - 2.0,
                        inner.h - 2.0,
                        t.surface,
                    );
                    let value = match field {
                        Item::FontBox => format.font.clone().unwrap_or_default(),
                        _ => format
                            .size_half_points
                            .map(|h| {
                                if h % 2 == 0 {
                                    (h / 2).to_string()
                                } else {
                                    format!("{},5", h / 2)
                                }
                            })
                            .unwrap_or_default(),
                    };
                    let style = TextStyle {
                        size: self.px(13.0),
                        bold: false,
                    };
                    let line = style.size * 1.33;
                    p.push_clip(inner.x, inner.y, inner.w - self.px(6.0), inner.h);
                    p.text(
                        &value,
                        inner.x + self.px(8.0),
                        inner.y + (inner.h - line) / 2.0,
                        style,
                        t.text,
                    );
                    p.pop_clip();
                }
                Placed::GroupLabel(key, r, w) => {
                    let style = TextStyle {
                        size: self.px(12.0),
                        bold: false,
                    };
                    let label = strings.get(key);
                    let line = style.size * 1.33;
                    p.text(
                        label,
                        (r.x + ((r.w - w) / 2.0).max(0.0)).round(),
                        r.y + (r.h - line) / 2.0,
                        style,
                        // Near the body text: muted labels read faded next to desktop ribbons.
                        mix(t.text, t.text_muted, 0.8),
                    );
                }
                Placed::Divider(r) => p.fill_rect(r.x, r.y, r.w, r.h, t.border),
            }
        }
    }
}
