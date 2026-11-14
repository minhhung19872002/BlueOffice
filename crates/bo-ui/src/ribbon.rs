//! Ribbon: tab strip and command groups, self-drawn (ADR-0018). Command ids, labels and icons
//! are those of the React ribbon (`CommandRegistry`, i18n keys, `assets/icons/ui-icons.json`).
use crate::icons::{IconColors, Icons};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{commands, Font, Painter, TextMeasure, TextStyle};
use bo_edit::FormatState;
use bo_engine::gallery::StyleEntry;
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
    /// Large button (icon over its label) running command `id`.
    Large(&'static str),
    /// Small button with its label to the right of the icon.
    Labeled(&'static str),
    /// Style gallery: preview tiles of the quick styles.
    Gallery,
}

/// Icon of a command whose own icon is missing from the shared set.
fn icon_of(id: &str) -> &str {
    match id {
        "layout.breaks" => "insert.sectionBreak",
        "references.toc" => "references.insertToc",
        "review.rejectAll" => "review.reject",
        "review.deleteComment" => "review.resolveComment",
        "review.wordCount" => "review.checkDocument",
        "design.pageColor" => "format.fillColor",
        "design.watermark" => "layout.pageSetup",
        "view.layout" => "view.normal",
        "view.onePage" => "view.reading",
        "view.pageWidth" => "view.fitWidth",
        "mailings.recipients" => "mailings.selectRecipients",
        "mailings.finish" => "mailings.mergeDocument",
        "insert.link" => "insert.link",
        other => other,
    }
}

/// Preview tiles shown in the style gallery.
const TILES: usize = 5;
/// Gallery preview text (Vietnamese letters show diacritics of the style font).
const PREVIEW: &str = "AaBbĐđ";

struct Group {
    label: &'static str,
    /// Controls spanning the group's full height, before the rows.
    large: Vec<Item>,
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
    /// The font family box.
    FontBox,
    /// The font size box.
    SizeBox,
    /// Style tile `n` of the gallery (index into the style list).
    Style(usize),
    /// The gallery's "more styles" button.
    StylesMore,
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
    /// Gallery tile with its style index.
    Tile(usize, Rect),
    /// The gallery's "more" button.
    More(Rect),
    GroupLabel(&'static str, Rect, f32),
    Divider(Rect),
}

/// The ribbon of BlueDoc.
pub struct Ribbon {
    tabs: Vec<Tab>,
    active: usize,
    scale: f32,
    width: f32,
    /// Window y of the ribbon's top edge (below the title bar).
    top: f32,
    placed: Vec<Placed>,
    hover: Option<RibbonHit>,
    pressed: Option<RibbonHit>,
    /// Measured label widths of large and labeled buttons.
    label_w: std::cell::RefCell<std::collections::HashMap<&'static str, f32>>,
}

fn home() -> Tab {
    use Item::{Button as B, FontBox, SizeBox};
    Tab {
        label: "ribbon.tabs.home",
        groups: vec![
            Group {
                label: "ribbon.groups.history",
                large: vec![],
                rows: vec![vec![B("edit.undo")], vec![B("edit.redo")]],
            },
            Group {
                label: "ribbon.groups.clipboard",
                large: vec![Item::Large("edit.paste")],
                rows: vec![
                    vec![B("edit.cut"), B("edit.copy")],
                    vec![B("format.painter")],
                ],
            },
            Group {
                label: "ribbon.groups.font",
                large: vec![],
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
                large: vec![],
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
            Group {
                label: "ribbon.groups.styles",
                large: vec![Item::Gallery],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.editing",
                large: vec![],
                rows: vec![
                    vec![Item::Labeled("edit.find")],
                    vec![Item::Labeled("edit.replace")],
                ],
            },
        ],
    }
}

fn insert_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.insert",
        groups: vec![
            Group {
                label: "ribbon.groups.pages",
                large: vec![L("insert.pageBreak")],
                rows: vec![vec![S("layout.breaks")]],
            },
            Group {
                label: "ribbon.groups.insertObjects",
                large: vec![L("insert.table")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.links",
                large: vec![L("insert.link")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.comments",
                large: vec![L("review.newComment")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.headerFooter",
                large: vec![L("insert.header"), L("insert.footer")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.symbols",
                large: vec![L("insert.equation"), L("insert.symbol")],
                rows: vec![],
            },
        ],
    }
}

fn design_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.design",
        groups: vec![Group {
            label: "ribbon.groups.pageBackground",
            large: vec![L("design.watermark"), L("design.pageColor")],
            rows: vec![],
        }],
    }
}

fn layout_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.layout",
        groups: vec![Group {
            label: "ribbon.groups.pageSetup",
            large: vec![
                L("layout.margins"),
                L("layout.orientation"),
                L("layout.size"),
                L("layout.columns"),
            ],
            rows: vec![vec![S("layout.breaks")]],
        }],
    }
}

fn references_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.references",
        groups: vec![
            Group {
                label: "ribbon.groups.toc",
                large: vec![L("references.toc")],
                rows: vec![vec![S("references.updateToc")]],
            },
            Group {
                label: "ribbon.groups.footnotes",
                large: vec![L("insert.footnote")],
                rows: vec![vec![S("insert.endnote")]],
            },
        ],
    }
}

fn mailings_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.mailings",
        groups: vec![
            Group {
                label: "ribbon.groups.mergeData",
                large: vec![L("mailings.recipients")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.mergePreview",
                large: vec![L("mailings.preview")],
                rows: vec![
                    vec![S("mailings.previousRecord")],
                    vec![S("mailings.nextRecord")],
                ],
            },
            Group {
                label: "ribbon.groups.mergeFinish",
                large: vec![L("mailings.finish")],
                rows: vec![],
            },
        ],
    }
}

fn review_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.review",
        groups: vec![
            Group {
                label: "ribbon.groups.proofing",
                large: vec![L("review.wordCount")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.comments",
                large: vec![L("review.newComment")],
                rows: vec![
                    vec![S("review.deleteComment")],
                    vec![S("review.previousComment")],
                    vec![S("review.nextComment")],
                ],
            },
            Group {
                label: "ribbon.groups.tracking",
                large: vec![L("review.trackChanges")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.changes",
                large: vec![L("review.acceptAll")],
                rows: vec![
                    vec![S("review.rejectAll")],
                    vec![S("review.previous")],
                    vec![S("review.next")],
                ],
            },
        ],
    }
}

fn view_tab() -> Tab {
    #[allow(unused_imports)]
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.view",
        groups: vec![
            Group {
                label: "ribbon.groups.views",
                large: vec![L("view.layout")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.show",
                large: vec![],
                rows: vec![vec![S("view.ruler")], vec![S("view.formattingMarks")]],
            },
            Group {
                label: "ribbon.groups.zoom",
                large: vec![L("view.zoomReset")],
                rows: vec![vec![S("view.zoomIn")], vec![S("view.zoomOut")]],
            },
        ],
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
                insert_tab(),
                design_tab(),
                layout_tab(),
                references_tab(),
                mailings_tab(),
                review_tab(),
                view_tab(),
            ],
            active: 0,
            scale: 1.0,
            width: 0.0,
            top: 0.0,
            placed: Vec::new(),
            hover: None,
            pressed: None,
            label_w: std::cell::RefCell::new(std::collections::HashMap::new()),
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
    pub fn layout(
        &mut self,
        text: &dyn TextMeasure,
        strings: &Strings,
        width: f32,
        scale: f32,
        top: f32,
    ) {
        self.scale = scale;
        self.width = width;
        self.top = top;
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
        let content_h = 2.0 * btn + row_gap;
        for g in &self.tabs[self.active].groups {
            let mut group_w: f32 = 0.0;
            let mut lx = gx + self.px(4.0);
            for item in &g.large {
                if *item == Item::Gallery {
                    {
                        let (tw, more) = (self.px(78.0), self.px(20.0));
                        for i in 0..TILES {
                            let r = Rect {
                                x: lx + i as f32 * (tw + gap),
                                y: top,
                                w: tw,
                                h: content_h,
                            };
                            placed.push(Placed::Tile(i, r));
                        }
                        lx += TILES as f32 * (tw + gap);
                        placed.push(Placed::More(Rect {
                            x: lx,
                            y: top,
                            w: more,
                            h: content_h,
                        }));
                        lx += more + gap;
                    }
                } else {
                    {
                        let id = match item {
                            Item::Large(id) => *id,
                            _ => "",
                        };
                        let lw = self.measure_label(text, strings, id);
                        let w = self.px(52.0).max(lw + self.px(14.0));
                        placed.push(Placed::Item(
                            *item,
                            Rect {
                                x: lx,
                                y: top,
                                w,
                                h: content_h,
                            },
                        ));
                        lx += w + gap;
                    }
                }
            }
            group_w = group_w.max(lx - gap - gx + self.px(4.0));
            for (r, row) in g.rows.iter().enumerate() {
                let y = top + r as f32 * (btn + row_gap);
                let mut x = lx;
                for item in row {
                    let w = match item {
                        Item::FontBox => self.px(150.0),
                        Item::SizeBox => self.px(52.0),
                        Item::Labeled(id) => {
                            btn + self.measure_label(text, strings, id) + self.px(8.0)
                        }
                        _ => btn,
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
        for p in &mut self.placed {
            let r = match p {
                Placed::File(r)
                | Placed::Tab(_, r)
                | Placed::Item(_, r)
                | Placed::Tile(_, r)
                | Placed::More(r)
                | Placed::GroupLabel(_, r, _)
                | Placed::Divider(r) => r,
            };
            r.y += top;
        }
    }

    /// Window y just below the ribbon.
    pub fn bottom(&self) -> f32 {
        self.top + self.height()
    }

    /// Window y of the ribbon's top edge.
    pub fn top(&self) -> f32 {
        self.top
    }

    fn measure_label(&self, text: &dyn TextMeasure, strings: &Strings, id: &'static str) -> f32 {
        let style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        let mut label = strings.get(&format!("commands.{id}")).to_owned();
        if crate::catalog::is_menu(id) {
            label.push_str("  ");
        }
        let w = text.measure(&label, style).0;
        self.label_w.borrow_mut().insert(id, w);
        w
    }

    /// What is under a window position.
    pub fn hit(&self, x: f32, y: f32) -> Option<RibbonHit> {
        self.placed.iter().find_map(|p| match p {
            Placed::File(r) if r.contains(x, y) => Some(RibbonHit::File),
            Placed::Tab(i, r) if r.contains(x, y) => Some(RibbonHit::Tab(*i)),
            Placed::Item(Item::Button(id) | Item::Large(id) | Item::Labeled(id), r)
                if r.contains(x, y) =>
            {
                Some(RibbonHit::Command(id))
            }
            Placed::Tile(i, r) if r.contains(x, y) => Some(RibbonHit::Style(*i)),
            Placed::More(r) if r.contains(x, y) => Some(RibbonHit::StylesMore),
            Placed::Item(Item::FontBox, r) if r.contains(x, y) => Some(RibbonHit::FontBox),
            Placed::Item(Item::SizeBox, r) if r.contains(x, y) => Some(RibbonHit::SizeBox),
            _ => None,
        })
    }

    /// Rectangle (x, y, w, h) of a control.
    pub fn rect_of(&self, hit: RibbonHit) -> Option<[f32; 4]> {
        self.placed.iter().find_map(|p| {
            let (h, r) = match p {
                Placed::File(r) => (RibbonHit::File, r),
                Placed::Tab(i, r) => (RibbonHit::Tab(*i), r),
                Placed::Item(Item::Button(id) | Item::Large(id) | Item::Labeled(id), r) => {
                    (RibbonHit::Command(id), r)
                }
                Placed::Tile(i, r) => (RibbonHit::Style(*i), r),
                Placed::More(r) => (RibbonHit::StylesMore, r),
                Placed::Item(Item::FontBox, r) => (RibbonHit::FontBox, r),
                Placed::Item(Item::SizeBox, r) => (RibbonHit::SizeBox, r),
                _ => return None,
            };
            (h == hit).then_some([r.x, r.y, r.w, r.h])
        })
    }

    /// The control under the pointer.
    pub fn hovered(&self) -> Option<RibbonHit> {
        self.hover
    }

    /// Forget hover and press (pointer left the ribbon or a menu took over).
    pub fn clear_pointer(&mut self) -> bool {
        let changed = self.hover.is_some() || self.pressed.is_some();
        (self.hover, self.pressed) = (None, None);
        changed
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
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &self,
        p: &mut dyn Painter,
        t: &Theme,
        icons: &Icons,
        strings: &Strings,
        format: &FormatState,
        styles: &[StyleEntry],
        on_ids: &[&str],
    ) {
        let strip = self.strip_h();
        p.fill_rect(0.0, self.top, self.width, self.height(), t.surface);
        // Line under the ribbon.
        p.fill_rect(0.0, self.bottom() - 1.0, self.width, 1.0, t.border);
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
                            self.top + strip - h,
                            r.w - self.px(20.0),
                            h,
                            t.accent,
                        );
                    }
                }
                Placed::Item(Item::Button(id), r) => {
                    let on = commands::toggled(id, format) || on_ids.contains(id);
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
                Placed::Item(Item::Large(id), r) => {
                    let hit = RibbonHit::Command(id);
                    if let Some(c) = tint(hit) {
                        radius_rect(p, r, c);
                    } else if on_ids.contains(id) {
                        radius_rect(p, r, t.active);
                    }
                    let size = self.px(32.0);
                    icons.draw(
                        p,
                        icon_of(id),
                        r.x + (r.w - size) / 2.0,
                        r.y + self.px(4.0),
                        size,
                        IconColors {
                            ink: t.text,
                            accent: t.accent,
                            on: t.accent_on,
                        },
                    );
                    let mut label = strings.get(&format!("commands.{id}")).to_owned();
                    if crate::catalog::is_menu(id) {
                        label.push_str(" \u{25BE}");
                    }
                    let style = TextStyle {
                        size: self.px(12.0),
                        bold: false,
                    };
                    let w = self.label_w.borrow().get(id).copied().unwrap_or(0.0);
                    p.text(
                        &label,
                        (r.x + (r.w - w) / 2.0).round(),
                        r.y + self.px(40.0),
                        style,
                        t.text,
                    );
                }
                Placed::Tile(i, r) => {
                    let Some(st) = styles.get(*i) else { continue };
                    let current = format.style.as_deref() == Some(st.id.as_str())
                        || (format.style.is_none() && st.id == "Normal");
                    if let Some(c) = tint(RibbonHit::Style(*i)) {
                        radius_rect(p, r, c);
                    }
                    // Tile frame: accent when it is the style at the selection.
                    let edge = if current { t.accent } else { t.border };
                    p.fill_rect(r.x, r.y, r.w, 1.0, edge);
                    p.fill_rect(r.x, r.y + r.h - 1.0, r.w, 1.0, edge);
                    p.fill_rect(r.x, r.y, 1.0, r.h, edge);
                    p.fill_rect(r.x + r.w - 1.0, r.y, 1.0, r.h, edge);
                    let pv = &st.preview;
                    // Preview size follows the style, kept inside the tile.
                    let size =
                        (pv.size * self.scale * 96.0 / 72.0).clamp(self.px(11.0), self.px(20.0));
                    let font = Font {
                        family: &pv.font,
                        size,
                        weight: if pv.bold { 700 } else { 400 },
                        italic: pv.italic,
                        underline: pv.underline,
                    };
                    let ink =
                        crate::theme::parse_color(&format!("#{}", pv.color)).unwrap_or(t.text);
                    p.push_clip(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0);
                    p.text_font(PREVIEW, r.x + self.px(6.0), r.y + self.px(6.0), &font, ink);
                    let name = TextStyle {
                        size: self.px(11.0),
                        bold: false,
                    };
                    let shown = commands::style_name_key(&st.name)
                        .map_or(st.name.as_str(), |k| strings.get(k));
                    p.text(
                        shown,
                        r.x + self.px(6.0),
                        r.y + r.h - self.px(20.0),
                        name,
                        t.text_muted,
                    );
                    p.pop_clip();
                }
                Placed::More(r) => {
                    if let Some(c) = tint(RibbonHit::StylesMore) {
                        radius_rect(p, r, c);
                    }
                    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                    let k = self.px(4.0);
                    p.path(
                        &[
                            bo_render::PathSeg::M(cx - k, cy - k / 2.0),
                            bo_render::PathSeg::L(cx, cy + k / 2.0),
                            bo_render::PathSeg::L(cx + k, cy - k / 2.0),
                        ],
                        None,
                        Some((t.text, self.scale.max(1.0))),
                    );
                }
                Placed::Item(Item::Labeled(id), r) => {
                    let hit = RibbonHit::Command(id);
                    let enabled = commands::enabled(id, format);
                    if let Some(c) = tint(hit).filter(|_| enabled) {
                        radius_rect(p, r, c);
                    } else if on_ids.contains(id) || commands::toggled(id, format) {
                        radius_rect(p, r, t.active);
                    }
                    let ink = if enabled { t.text } else { t.border };
                    let size = self.px(18.0);
                    icons.draw(
                        p,
                        icon_of(id),
                        r.x + self.px(5.0),
                        r.y + (r.h - size) / 2.0,
                        size,
                        IconColors {
                            ink,
                            accent: t.accent,
                            on: t.accent_on,
                        },
                    );
                    let mut label = strings.get(&format!("commands.{id}")).to_owned();
                    if crate::catalog::is_menu(id) {
                        label.push_str(" \u{25BE}");
                    }
                    let style = TextStyle {
                        size: self.px(12.0),
                        bold: false,
                    };
                    p.text(
                        &label,
                        r.x + self.px(28.0),
                        r.y + (r.h - style.size * 1.33) / 2.0,
                        style,
                        ink,
                    );
                }
                Placed::Item(Item::Gallery, _) => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ribbon_button_does_something() {
        let ribbon = Ribbon::new();
        let format = FormatState {
            section: bo_edit::SectionState {
                width: 11906,
                height: 16838,
                margin_left: 1440,
                margin_right: 1440,
                columns: 1,
                ..bo_edit::SectionState::default()
            },
            can_undo: true,
            can_redo: true,
            ..FormatState::default()
        };
        let strings = Strings::load("en-US");
        for tab in &ribbon.tabs {
            assert!(!tab.groups.is_empty(), "{} is empty", tab.label);
            for g in &tab.groups {
                assert_ne!(strings.get(g.label), g.label, "missing label {}", g.label);
                for item in g.large.iter().chain(g.rows.iter().flatten()) {
                    if let Item::Button(id) | Item::Large(id) | Item::Labeled(id) = item {
                        assert!(commands::enabled(id, &format), "{id} has no action");
                        let key = format!("commands.{id}");
                        assert_ne!(strings.get(&key), key, "missing label {key}");
                    }
                }
            }
        }
    }
}
