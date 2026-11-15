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
pub(crate) enum Item {
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
        "design.pageColor" | "format.shading" => "format.fillColor",
        "format.paraBorders" => "format.borders",
        "format.sortParagraphs" => "data.sortAsc",
        "design.watermark" | "slide.layout" => "layout.pageSetup",
        "view.layout" | "view.notes" => "view.normal",
        "view.onePage" => "view.reading",
        "view.pageWidth" | "view.fitSlide" => "view.fitWidth",
        "view.formattingMarks" => "style.normal",
        "view.zoomReset" => "edit.find",
        "mailings.recipients" => "mailings.selectRecipients",
        "mailings.finish" => "mailings.mergeDocument",
        "insert.link" => "insert.link",
        "format.fontSettings" | "format.paragraphSettings" => "launcher",
        "insert.shapes" => "insert.rect",
        "insert.dateTime" => "format.date",
        "insert.dropCap" => "format.fontGrow",
        "layout.lineNumbers" => "format.numbering",
        "layout.hyphenation" => "format.wrap",
        other => other,
    }
}

/// Frame of the style gallery box and band of its current tile; outer line of that tile.
const GALLERY_EDGE: Rgba = Rgba(0xd2, 0xd2, 0xd2, 255);
const GALLERY_CURRENT: Rgba = Rgba(0x88, 0x88, 0x88, 255);

/// Frame of the font and size boxes (grey measured on a desktop word processor).
const BOX_EDGE: Rgba = Rgba(0x94, 0x94, 0x94, 255);

/// Fill and frame of ribbon commands that are on.
const TOGGLED_FILL: Rgba = Rgba(0xc6, 0xc6, 0xc6, 255);
const TOGGLED_EDGE: Rgba = Rgba(0x80, 0x80, 0x80, 255);

/// Preview tiles shown in the style gallery.
const TILES: usize = 5;
/// Gallery preview text (Vietnamese letters show diacritics of the style font).
const PREVIEWS: [&str; 3] = ["AaĐđ", "AĐ", "Đ"];

pub(crate) struct Group {
    pub(crate) label: &'static str,
    /// Controls spanning the group's full height, before the rows.
    pub(crate) large: Vec<Item>,
    pub(crate) rows: Vec<Vec<Item>>,
}

pub(crate) struct Tab {
    pub(crate) label: &'static str,
    pub(crate) groups: Vec<Group>,
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
    /// Reveal earlier ribbon groups.
    ScrollLeft,
    /// Reveal later ribbon groups.
    ScrollRight,
    /// Reveal earlier ribbon tabs.
    TabScrollLeft,
    /// Reveal later ribbon tabs.
    TabScrollRight,
}

#[derive(Clone, Copy, PartialEq)]
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

    fn visible_in(&self, clip: Option<Rect>) -> bool {
        clip.is_none_or(|clip| {
            if self.w > clip.w {
                self.x < clip.x + clip.w && self.x + self.w > clip.x
            } else {
                self.x >= clip.x - 0.01 && self.x + self.w <= clip.x + clip.w + 0.01
            }
        })
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
    Scroll(bool, Rect),
    TabScroll(bool, Rect),
}

/// The ribbon of BlueDoc.
pub struct Ribbon {
    tabs: Vec<Tab>,
    active: usize,
    scale: f32,
    width: f32,
    /// Window y of the ribbon's top edge (below the title bar).
    top: f32,
    scroll: f32,
    scroll_max: f32,
    content_clip: Option<Rect>,
    tab_scroll: f32,
    tab_scroll_max: f32,
    tab_clip: Option<Rect>,
    placed: Vec<Placed>,
    hover: Option<RibbonHit>,
    pressed: Option<RibbonHit>,
    /// Measured label widths of large and labeled buttons.
    label_w: std::cell::RefCell<std::collections::HashMap<&'static str, f32>>,
    /// Measured (width, height) of each of [`GLYPH_BUTTONS`] at the current scale.
    glyph_size: [(f32, f32); GLYPH_BUTTONS.len()],
    previews: Vec<(&'static str, f32)>,
    /// Height of the group label row, from the measured label text (it grows with the
    /// platform's interface text size, as the reference ribbon does).
    label_row: f32,
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
                    vec![
                        FontBox,
                        SizeBox,
                        B("format.growStep"),
                        B("format.shrinkStep"),
                        B("format.changeCase"),
                        B("format.clear"),
                    ],
                    vec![
                        B("format.bold"),
                        B("format.italic"),
                        B("format.underline"),
                        B("format.strike"),
                        B("format.subscript"),
                        B("format.superscript"),
                        B("format.textEffects"),
                        B("format.highlight"),
                        B("format.color"),
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
                        B("format.multilevel"),
                        B("format.outdent"),
                        B("format.indent"),
                        B("format.sortParagraphs"),
                        B("view.formattingMarks"),
                    ],
                    vec![
                        B("format.alignLeft"),
                        B("format.alignCenter"),
                        B("format.alignRight"),
                        B("format.justify"),
                        B("format.lineSpacing"),
                        B("format.shading"),
                        B("format.paraBorders"),
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
                label: "ribbon.groups.illustrations",
                large: vec![L("insert.shapes")],
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
                rows: vec![vec![S("insert.pageNumber")]],
            },
            Group {
                label: "ribbon.groups.text",
                large: vec![L("insert.textBox")],
                rows: vec![
                    vec![S("insert.dropCap")],
                    vec![S("insert.dateTime")],
                    vec![S("insert.signatureLine")],
                ],
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
            rows: vec![
                vec![S("layout.breaks")],
                vec![S("layout.lineNumbers")],
                vec![S("layout.hyphenation")],
            ],
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
            Group {
                label: "ribbon.groups.captions",
                large: vec![L("references.insertCaption")],
                rows: vec![],
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
                    vec![S("review.previousComment"), S("review.nextComment")],
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
                    vec![S("review.previous"), S("review.next")],
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
        Self::with_tabs(vec![
            home(),
            insert_tab(),
            design_tab(),
            layout_tab(),
            references_tab(),
            mailings_tab(),
            review_tab(),
            view_tab(),
        ])
    }

    /// A ribbon with the given tabs (BlueSheet, BlueSlide declare their own); the first
    /// is active.
    pub(crate) fn with_tabs(tabs: Vec<Tab>) -> Self {
        Self {
            tabs,
            active: 0,
            scale: 1.0,
            width: 0.0,
            top: 0.0,
            scroll: 0.0,
            scroll_max: 0.0,
            content_clip: None,
            tab_scroll: 0.0,
            tab_scroll_max: 0.0,
            tab_clip: None,
            placed: Vec::new(),
            hover: None,
            pressed: None,
            label_w: std::cell::RefCell::new(std::collections::HashMap::new()),
            glyph_size: [(0.0, 0.0); GLYPH_BUTTONS.len()],
            previews: Vec::new(),
            label_row: 0.0,
        }
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Height of the tab strip.
    fn strip_h(&self) -> f32 {
        self.px(36.0)
    }

    /// Height of the band holding the group commands (three small-button rows high, as
    /// on the reference ribbon; two-row groups are centred in it).
    fn band_h(&self) -> f32 {
        self.px(80.0)
    }

    /// Height of the group label row.
    fn label_h(&self) -> f32 {
        self.px(18.0).max(self.label_row)
    }

    /// Height of the whole ribbon in device pixels.
    pub fn height(&self) -> f32 {
        self.strip_h() + self.px(4.0) + self.band_h() + self.label_h() + self.px(4.0)
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
        let size_changed = self.width != width || self.scale != scale;
        self.scale = scale;
        self.width = width;
        self.top = top;
        self.placed.clear();
        for (i, (_, glyph)) in GLYPH_BUTTONS.iter().enumerate() {
            self.glyph_size[i] = text.measure_font(glyph.text, &glyph.font(self.px(glyph.size)));
        }
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
        let tabs_start = x;
        let mut tabs = Vec::new();
        for (i, t) in self.tabs.iter().enumerate() {
            let (w, _) = text.measure(strings.get(t.label), tab_style);
            let r = Rect {
                x,
                y: 0.0,
                w: w + 2.0 * pad,
                h: strip,
            };
            x += r.w;
            tabs.push(Placed::Tab(i, r));
        }
        self.tab_clip = None;
        self.tab_scroll_max = 0.0;
        if x > width {
            let available = (width - tabs_start).max(0.0);
            let arrow = self.px(28.0).min(available / 3.0);
            let viewport = (available - 2.0 * arrow).max(0.0);
            self.tab_scroll_max = ((x - tabs_start) - viewport).max(0.0);
            self.tab_scroll = self.tab_scroll.clamp(0.0, self.tab_scroll_max);
            if size_changed {
                if let Some(Placed::Tab(_, active)) = tabs.get(self.active) {
                    let left = active.x - tabs_start;
                    let right = left + active.w;
                    if left < self.tab_scroll {
                        self.tab_scroll = left;
                    } else if right > self.tab_scroll + viewport {
                        self.tab_scroll = right - viewport;
                    }
                    self.tab_scroll = self.tab_scroll.clamp(0.0, self.tab_scroll_max);
                }
            }
            self.tab_clip = Some(Rect {
                x: tabs_start + arrow,
                y: top,
                w: viewport,
                h: strip,
            });
            for p in &mut tabs {
                if let Placed::Tab(_, r) = p {
                    r.x += arrow - self.tab_scroll;
                }
            }
            tabs.push(Placed::TabScroll(
                false,
                Rect {
                    x: tabs_start,
                    y: 0.0,
                    w: arrow,
                    h: strip,
                },
            ));
            tabs.push(Placed::TabScroll(
                true,
                Rect {
                    x: width - arrow,
                    y: 0.0,
                    w: arrow,
                    h: strip,
                },
            ));
        } else {
            self.tab_scroll = 0.0;
        }
        self.placed.extend(tabs);
        // Groups of the active tab.
        let label_style = TextStyle {
            size: self.px(12.0),
            bold: false,
        };
        self.label_row = (text.measure("Ag", label_style).1 + self.px(4.0)).ceil();
        let content_top = strip + self.px(4.0);
        let (btn, gap, row_gap) = (self.px(28.0), self.px(2.0), self.px(8.0));
        let label_h = self.label_h();
        let bottom = self.height() - self.px(4.0);
        let content_h = self.band_h();
        // Two rows of small buttons, centred in the band.
        let rows_top = content_top + ((content_h - (2.0 * btn + row_gap)) / 2.0).floor();
        // The gallery is the first group to compact as the window narrows. Its
        // menu remains available even when no preview tile fits.
        let mut tile_count = TILES;
        let (mut placed, content_width) = loop {
            let mut gx = self.px(8.0);
            let mut placed = Vec::new();
            for g in &self.tabs[self.active].groups {
                let mut group_w: f32 = 0.0;
                let mut lx = gx + self.px(4.0);
                for item in &g.large {
                    if *item == Item::Gallery {
                        {
                            let (tw, more) = (self.px(78.0), self.px(20.0));
                            for i in 0..tile_count {
                                let r = Rect {
                                    x: lx + i as f32 * (tw + gap),
                                    y: content_top,
                                    w: tw,
                                    h: content_h,
                                };
                                placed.push(Placed::Tile(i, r));
                            }
                            lx += tile_count as f32 * (tw + gap);
                            placed.push(Placed::More(Rect {
                                x: lx,
                                y: content_top,
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
                                    y: content_top,
                                    w,
                                    h: content_h,
                                },
                            ));
                            lx += w + gap;
                        }
                    }
                }
                group_w = group_w.max(lx - gap - gx + self.px(4.0));
                // Three rows fill the band (shorter buttons, as on the reference ribbon).
                let three = g.rows.len() >= 3;
                let row_h = if three {
                    ((content_h - 2.0 * gap) / 3.0).floor()
                } else {
                    btn
                };
                for (r, row) in g.rows.iter().enumerate() {
                    let y = if three {
                        content_top + r as f32 * (row_h + gap)
                    } else {
                        rows_top + r as f32 * (btn + row_gap)
                    };
                    let mut x = lx;
                    for item in row {
                        let w = match item {
                            Item::FontBox => self.px(150.0),
                            Item::SizeBox => self.px(52.0),
                            Item::Button(id) if crate::catalog::is_menu(id) => self.px(36.0),
                            Item::Labeled(id) => {
                                btn + self.measure_label(text, strings, id) + self.px(8.0)
                            }
                            _ => btn,
                        };
                        placed.push(Placed::Item(*item, Rect { x, y, w, h: row_h }));
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
                // Settings button in the label row's right corner (Font, Paragraph), as on a
                // desktop word processor's ribbon.
                let launcher = match g.label {
                    "ribbon.groups.font" => Some("format.fontSettings"),
                    "ribbon.groups.paragraph" => Some("format.paragraphSettings"),
                    _ => None,
                };
                if let Some(id) = launcher {
                    let s = self.px(16.0);
                    group_w = group_w.max(lw + 2.0 * s + self.px(16.0));
                    placed.push(Placed::Item(
                        Item::Button(id),
                        Rect {
                            x: gx + group_w - s - self.px(2.0),
                            y: bottom - label_h + ((label_h - s) / 2.0).floor(),
                            w: s,
                            h: s,
                        },
                    ));
                }
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
                    y: content_top,
                    w: 1.0,
                    h: bottom - content_top - self.px(2.0),
                }));
                gx += self.px(6.0);
            }
            if gx <= width
                || tile_count == 0
                || !self.tabs[self.active]
                    .groups
                    .iter()
                    .any(|g| g.large.contains(&Item::Gallery))
            {
                break (placed, gx);
            }
            tile_count -= 1;
        };
        self.content_clip = None;
        self.scroll_max = 0.0;
        if content_width > width {
            let arrow = self.px(28.0).min(width.max(0.0) / 3.0);
            let viewport = (width - 2.0 * arrow).max(0.0);
            self.scroll_max = (content_width - viewport).max(0.0);
            self.scroll = self.scroll.clamp(0.0, self.scroll_max);
            self.content_clip = Some(Rect {
                x: arrow,
                y: self.top + strip,
                w: viewport,
                h: self.height() - strip,
            });
            for p in &mut placed {
                let (Placed::Item(_, r)
                | Placed::Tile(_, r)
                | Placed::More(r)
                | Placed::GroupLabel(_, r, _)
                | Placed::Divider(r)) = p
                else {
                    unreachable!("group layout only contains group elements")
                };
                r.x += arrow - self.scroll;
            }
            let h = self.height() - strip;
            placed.push(Placed::Scroll(
                false,
                Rect {
                    x: 0.0,
                    y: strip,
                    w: arrow,
                    h,
                },
            ));
            placed.push(Placed::Scroll(
                true,
                Rect {
                    x: width - arrow,
                    y: strip,
                    w: arrow,
                    h,
                },
            ));
        } else {
            self.scroll = 0.0;
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
                | Placed::Divider(r)
                | Placed::Scroll(_, r)
                | Placed::TabScroll(_, r) => r,
            };
            r.y += self.top;
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
            label.push_str(" \u{25BE}");
        }
        let w = text.measure(&label, style).0;
        self.label_w.borrow_mut().insert(id, w);
        w
    }

    /// Choose a sample that fits each visible style's actual font without clipping.
    pub fn measure_previews(&mut self, text: &dyn TextMeasure, styles: &[StyleEntry]) {
        let available = self.px(78.0) - self.px(8.0);
        self.previews = styles
            .iter()
            .take(TILES)
            .map(|style| {
                let pv = &style.preview;
                let mut font = Font {
                    family: &pv.font,
                    size: (pv.size * self.scale * 96.0 / 72.0).clamp(self.px(11.0), self.px(20.0)),
                    weight: if pv.bold { 700 } else { 400 },
                    italic: pv.italic,
                    underline: pv.underline,
                };
                if let Some(sample) = PREVIEWS
                    .iter()
                    .copied()
                    .find(|sample| text.measure_font(sample, &font).0 <= available)
                {
                    return (sample, font.size);
                }
                while font.size > 1.0 && text.measure_font(PREVIEWS[2], &font).0 > available {
                    font.size = (font.size - 1.0).max(1.0);
                }
                (PREVIEWS[2], font.size)
            })
            .collect();
    }

    /// What is under a window position.
    pub fn hit(&self, x: f32, y: f32) -> Option<RibbonHit> {
        let content_visible = self.content_clip.is_none_or(|clip| clip.contains(x, y));
        let tab_visible = self.tab_clip.is_none_or(|clip| clip.contains(x, y));
        self.placed.iter().find_map(|p| match p {
            Placed::File(r) if r.contains(x, y) => Some(RibbonHit::File),
            Placed::Tab(i, r) if tab_visible && r.visible_in(self.tab_clip) && r.contains(x, y) => {
                Some(RibbonHit::Tab(*i))
            }
            Placed::TabScroll(false, r) if self.tab_scroll > 0.0 && r.contains(x, y) => {
                Some(RibbonHit::TabScrollLeft)
            }
            Placed::TabScroll(true, r)
                if self.tab_scroll < self.tab_scroll_max && r.contains(x, y) =>
            {
                Some(RibbonHit::TabScrollRight)
            }
            Placed::Item(Item::Button(id) | Item::Large(id) | Item::Labeled(id), r)
                if content_visible && r.visible_in(self.content_clip) && r.contains(x, y) =>
            {
                Some(RibbonHit::Command(id))
            }
            Placed::Tile(i, r)
                if content_visible && r.visible_in(self.content_clip) && r.contains(x, y) =>
            {
                Some(RibbonHit::Style(*i))
            }
            Placed::More(r)
                if content_visible && r.visible_in(self.content_clip) && r.contains(x, y) =>
            {
                Some(RibbonHit::StylesMore)
            }
            Placed::Item(Item::FontBox, r)
                if content_visible && r.visible_in(self.content_clip) && r.contains(x, y) =>
            {
                Some(RibbonHit::FontBox)
            }
            Placed::Item(Item::SizeBox, r)
                if content_visible && r.visible_in(self.content_clip) && r.contains(x, y) =>
            {
                Some(RibbonHit::SizeBox)
            }
            Placed::Scroll(false, r) if self.scroll > 0.0 && r.contains(x, y) => {
                Some(RibbonHit::ScrollLeft)
            }
            Placed::Scroll(true, r) if self.scroll < self.scroll_max && r.contains(x, y) => {
                Some(RibbonHit::ScrollRight)
            }
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
                Placed::Scroll(false, r) => (RibbonHit::ScrollLeft, r),
                Placed::Scroll(true, r) => (RibbonHit::ScrollRight, r),
                Placed::TabScroll(false, r) => (RibbonHit::TabScrollLeft, r),
                Placed::TabScroll(true, r) => (RibbonHit::TabScrollRight, r),
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

    // Reveal the closest hidden control completely instead of stepping past a wide one.
    fn content_scroll_step(&self, right: bool) -> f32 {
        let Some(clip) = self.content_clip else {
            return 0.0;
        };
        let mut closest = f32::INFINITY;
        for item in &self.placed {
            let (Placed::Item(_, rect) | Placed::Tile(_, rect) | Placed::More(rect)) = item else {
                continue;
            };
            let left = clip.x;
            let end = clip.x + clip.w;
            let delta = if right {
                if rect.w > clip.w {
                    if rect.x >= end {
                        rect.x - left
                    } else {
                        0.0
                    }
                } else {
                    rect.x + rect.w - end
                }
            } else if rect.w > clip.w {
                if rect.x + rect.w <= left {
                    end - (rect.x + rect.w)
                } else {
                    0.0
                }
            } else {
                left - rect.x
            };
            if delta > 0.01 {
                closest = closest.min(delta);
            }
        }
        if closest.is_finite() {
            closest
        } else if right {
            self.scroll_max - self.scroll
        } else {
            self.scroll
        }
    }

    /// Button released: the control activated (pressed and released over the same one).
    /// Selecting a tab is handled here; `true` in the second value when a redraw is needed.
    pub fn release(&mut self, x: f32, y: f32) -> (Option<RibbonHit>, bool) {
        let was = self.pressed.take();
        let now = self.hit(x, y);
        let fired = was.filter(|w| Some(*w) == now);
        match fired {
            Some(RibbonHit::Tab(i)) => {
                self.active = i;
                self.scroll = 0.0;
                self.placed.clear();
            }
            Some(RibbonHit::ScrollLeft | RibbonHit::ScrollRight) => {
                let step = self.content_scroll_step(fired == Some(RibbonHit::ScrollRight));
                let direction = if fired == Some(RibbonHit::ScrollRight) {
                    1.0
                } else {
                    -1.0
                };
                self.scroll = (self.scroll + direction * step).clamp(0.0, self.scroll_max);
                self.placed.clear();
            }
            Some(RibbonHit::TabScrollLeft | RibbonHit::TabScrollRight) => {
                let step = self.tab_clip.map_or(0.0, |r| r.w * 0.5).max(self.px(28.0));
                let direction = if fired == Some(RibbonHit::TabScrollRight) {
                    1.0
                } else {
                    -1.0
                };
                self.tab_scroll =
                    (self.tab_scroll + direction * step).clamp(0.0, self.tab_scroll_max);
                self.placed.clear();
            }
            _ => {}
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
        p.fill_rect(0.0, self.top, self.width, strip, t.surface);
        p.fill_rect(
            0.0,
            self.top + strip,
            self.width,
            self.height() - strip,
            t.ribbon,
        );
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
        // A command that is on (bold, centred, ruler shown…): grey fill with a darker
        // one-pixel frame, as measured on a desktop word processor's ribbon (198 / 128).
        let toggled_rect = |p: &mut dyn Painter, r: &Rect| {
            let (x, y, w, h) = (r.x.round(), r.y.round(), r.w.round(), r.h.round());
            p.fill_rect(x, y, w, h, TOGGLED_EDGE);
            p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, TOGGLED_FILL);
        };
        let mut current_clip: Option<Rect> = None;
        for placed in &self.placed {
            // Keep partly clipped controls out of view and hit testing; oversized controls
            // remain clipped so they are still usable in an exceptionally narrow window.
            let clipped_control = match placed {
                Placed::Tab(_, r) => !r.visible_in(self.tab_clip),
                Placed::Item(_, r) | Placed::Tile(_, r) | Placed::More(r) => {
                    !r.visible_in(self.content_clip)
                }
                _ => false,
            };
            if clipped_control {
                continue;
            }
            let is_group = matches!(
                placed,
                Placed::Item(..)
                    | Placed::Tile(..)
                    | Placed::More(..)
                    | Placed::GroupLabel(..)
                    | Placed::Divider(..)
            );
            let needed_clip = if is_group {
                self.content_clip
            } else if matches!(placed, Placed::Tab(..)) {
                self.tab_clip
            } else {
                None
            };
            if needed_clip != current_clip {
                if current_clip.is_some() {
                    p.pop_clip();
                }
                if let Some(r) = needed_clip {
                    p.push_clip(r.x, r.y, r.w, r.h);
                }
                current_clip = needed_clip;
            }
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
                        toggled_rect(p, r);
                    }
                    let ink = if enabled { t.text } else { t.border };
                    let size = self.px(20.0);
                    let is_menu = crate::catalog::is_menu(id);
                    if let Some(g) = GLYPH_BUTTONS.iter().position(|(g, _)| g == id) {
                        // Character formatting buttons show the letter itself, set in the
                        // style it applies, as desktop word processors do.
                        let glyph = &GLYPH_BUTTONS[g].1;
                        let font = glyph.font(self.px(glyph.size));
                        let (w, line) = self.glyph_size[g];
                        let room = if is_menu { r.w - self.px(10.0) } else { r.w };
                        let (gx, gy) = (
                            (r.x + (room - w) / 2.0).round(),
                            (r.y + (r.h - line) / 2.0).round(),
                        );
                        // With an index or a mark the letter and its decoration are centred
                        // together.
                        let small = Font {
                            size: self.px(9.0),
                            weight: 600,
                            ..font
                        };
                        let shift = if matches!(
                            glyph.decor,
                            Decor::None | Decor::Strike | Decor::Bar(_) | Decor::Effects
                        ) {
                            0.0
                        } else {
                            -self.px(3.0)
                        };
                        let gx = gx + shift;
                        if glyph.decor == Decor::Effects && enabled {
                            // Outline: the letter in blue one pixel around, white inside.
                            let d = self.scale.round().max(1.0);
                            for (dx, dy) in [(-d, 0.0), (d, 0.0), (0.0, -d), (0.0, d)] {
                                p.text_font(
                                    glyph.text,
                                    gx + dx,
                                    gy + dy,
                                    &font,
                                    crate::icons::ICON_BLUE,
                                );
                            }
                            p.text_font(glyph.text, gx, gy, &font, Rgba(255, 255, 255, 255));
                        } else {
                            p.text_font(glyph.text, gx, gy, &font, ink);
                        }
                        let mark = if enabled {
                            crate::icons::ICON_BLUE
                        } else {
                            ink
                        };
                        // Segoe UI: ascent ≈ 1.08 em, x-height ≈ 0.5 em, cap ≈ 0.7 em.
                        if is_menu {
                            let (cx, cy, k) =
                                (r.x + r.w - self.px(6.0), r.y + r.h / 2.0, self.px(2.0));
                            p.path(
                                &[
                                    bo_render::PathSeg::M(cx - k, cy - k / 2.0),
                                    bo_render::PathSeg::L(cx, cy + k / 2.0),
                                    bo_render::PathSeg::L(cx + k, cy - k / 2.0),
                                ],
                                None,
                                Some((ink, self.scale.max(1.0))),
                            );
                        }
                        match glyph.decor {
                            Decor::None | Decor::Effects => {}
                            Decor::Bar(color) => {
                                let bar = self.px(4.0);
                                let y = (r.y + r.h - bar - self.px(3.0)).round();
                                let bw = self.px(16.0);
                                let c = if enabled { color } else { ink };
                                p.fill_rect((r.x + (r.w - bw) / 2.0).round(), y, bw, bar, c);
                            }
                            Decor::Strike => {
                                let y = (gy + line * 0.56).round();
                                p.fill_rect(gx - 1.0, y, w + 2.0, self.scale.round().max(1.0), ink);
                            }
                            Decor::Sub | Decor::Sup => {
                                let digit_y = if glyph.decor == Decor::Sup {
                                    gy + 0.28 * font.size - 0.38 * small.size
                                } else {
                                    gy + 1.23 * font.size - 1.08 * small.size
                                };
                                p.text_font("2", gx + w, digit_y.round(), &small, mark);
                            }
                            Decor::Grow | Decor::Shrink => {
                                let k = self.px(3.0);
                                let (cx, top) =
                                    ((gx + w + k).round(), (gy + 0.38 * font.size).round());
                                let tri = if glyph.decor == Decor::Grow {
                                    [
                                        bo_render::PathSeg::M(cx - k, top + k),
                                        bo_render::PathSeg::L(cx + k, top + k),
                                        bo_render::PathSeg::L(cx, top),
                                    ]
                                } else {
                                    [
                                        bo_render::PathSeg::M(cx - k, top),
                                        bo_render::PathSeg::L(cx + k, top),
                                        bo_render::PathSeg::L(cx, top + k),
                                    ]
                                };
                                let mut segs = tri.to_vec();
                                segs.push(bo_render::PathSeg::Z);
                                p.path(&segs, Some(mark), None);
                            }
                        }
                        continue;
                    }
                    if *id == "format.highlight" {
                        let small = self.px(16.0);
                        icons.draw(
                            p,
                            icon_of(id),
                            (r.x + (r.w - small) / 2.0).round(),
                            (r.y + self.px(4.0)).round(),
                            small,
                            IconColors {
                                ink,
                                accent: t.accent,
                                on: t.accent_on,
                                tinted: enabled,
                            },
                        );
                        let (bar, bw) = (self.px(4.0), self.px(16.0));
                        p.fill_rect(
                            (r.x + (r.w - bw) / 2.0).round(),
                            (r.y + r.h - bar - self.px(3.0)).round(),
                            bw,
                            bar,
                            if enabled {
                                Rgba(0xff, 0xff, 0, 255)
                            } else {
                                ink
                            },
                        );
                        continue;
                    }
                    icons.draw(
                        p,
                        icon_of(id),
                        if is_menu {
                            r.x + self.px(3.0)
                        } else {
                            r.x + (r.w - size) / 2.0
                        },
                        r.y + (r.h - size) / 2.0,
                        size,
                        IconColors {
                            ink,
                            accent: t.accent,
                            on: t.accent_on,
                            tinted: enabled,
                        },
                    );
                    if is_menu {
                        let (cx, cy, k) = (r.x + r.w - self.px(6.0), r.y + r.h / 2.0, self.px(2.0));
                        p.path(
                            &[
                                bo_render::PathSeg::M(cx - k, cy - k / 2.0),
                                bo_render::PathSeg::L(cx, cy + k / 2.0),
                                bo_render::PathSeg::L(cx + k, cy - k / 2.0),
                            ],
                            None,
                            Some((ink, self.scale.max(1.0))),
                        );
                    }
                }
                Placed::Item(Item::Large(id), r) => {
                    let hit = RibbonHit::Command(id);
                    if let Some(c) = tint(hit) {
                        radius_rect(p, r, c);
                    } else if on_ids.contains(id) {
                        toggled_rect(p, r);
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
                            tinted: true,
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
                    // Tiles share one white box with a light grey frame (the gap to the next
                    // tile is part of it); the style at the selection gets a dark outer
                    // line and a grey inner band, as on a desktop word processor's gallery.
                    let gap = self.px(2.0);
                    let (x, y, w, h) = (r.x.round(), r.y.round(), r.w.round(), r.h.round());
                    p.fill_rect(x, y, w + gap, h, t.surface);
                    p.fill_rect(x, y, w + gap, 1.0, GALLERY_EDGE);
                    p.fill_rect(x, y + h - 1.0, w + gap, 1.0, GALLERY_EDGE);
                    if *i == 0 {
                        p.fill_rect(x, y, 1.0, h, GALLERY_EDGE);
                    }
                    let band = self.px(3.0);
                    if current {
                        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, GALLERY_CURRENT);
                        p.fill_rect(x + 2.0, y + 2.0, w - 4.0, h - 4.0, GALLERY_EDGE);
                        p.fill_rect(
                            x + 2.0 + band,
                            y + 2.0 + band,
                            w - 4.0 - 2.0 * band,
                            h - 4.0 - 2.0 * band,
                            t.surface,
                        );
                    } else if let Some(c) = tint(RibbonHit::Style(*i)) {
                        p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, c);
                    }
                    let pv = &st.preview;
                    // Preview size follows the style, kept inside the tile.
                    let default_size =
                        (pv.size * self.scale * 96.0 / 72.0).clamp(self.px(11.0), self.px(20.0));
                    let (sample, size) = self
                        .previews
                        .get(*i)
                        .copied()
                        .unwrap_or((PREVIEWS[2], default_size));
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
                    // Style name on the bottom line (label-sized, as on the reference
                    // gallery), the sample centred in the space above it.
                    let name_y = (r.y + r.h - self.label_h() - self.px(2.0)).round();
                    let sample_h = size * 1.25;
                    let sample_y = (r.y + (name_y - r.y - sample_h) / 2.0).round();
                    p.text_font(sample, r.x + self.px(6.0), sample_y, &font, ink);
                    let name = TextStyle {
                        size: self.px(12.0),
                        bold: false,
                    };
                    let shown = commands::style_name_key(&st.name)
                        .map_or(st.name.as_str(), |k| strings.get(k));
                    p.text(shown, r.x + self.px(6.0), name_y, name, t.text_muted);
                    p.pop_clip();
                }
                Placed::More(r) => {
                    // Part of the gallery box, behind a divider.
                    let (x, y, w, h) = (r.x.round(), r.y.round(), r.w.round(), r.h.round());
                    p.fill_rect(x, y, w, h, GALLERY_EDGE);
                    let fill = tint(RibbonHit::StylesMore).unwrap_or(t.ribbon);
                    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, fill);
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
                        toggled_rect(p, r);
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
                            tinted: enabled,
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
                    // Measured line height (it includes the platform text size).
                    let line = (self.label_row - self.px(4.0)).max(style.size * 1.33);
                    p.text(
                        &label,
                        r.x + self.px(28.0),
                        (r.y + (r.h - line) / 2.0).round(),
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
                    p.fill_rect(inner.x, inner.y, inner.w, inner.h, BOX_EDGE);
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
                    let arrow = self.px(16.0);
                    p.push_clip(inner.x, inner.y, inner.w - arrow, inner.h);
                    p.text(
                        &value,
                        inner.x + self.px(8.0),
                        (inner.y + (inner.h - line) / 2.0).round(),
                        style,
                        t.text,
                    );
                    p.pop_clip();
                    // Drop-down arrow: a small solid triangle at the right edge.
                    let (cx, cy, k) = (
                        (inner.x + inner.w - arrow / 2.0).round(),
                        (inner.y + inner.h / 2.0).round(),
                        self.px(3.0),
                    );
                    p.path(
                        &[
                            bo_render::PathSeg::M(cx - k, cy - k / 2.0),
                            bo_render::PathSeg::L(cx + k, cy - k / 2.0),
                            bo_render::PathSeg::L(cx, cy + k / 2.0),
                            bo_render::PathSeg::Z,
                        ],
                        Some(t.text_muted),
                        None,
                    );
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
                // Group dividers in mid grey (128, measured on a desktop word processor).
                Placed::Divider(r) => p.fill_rect(r.x, r.y, r.w, r.h, TOGGLED_EDGE),
                Placed::Scroll(right, r) | Placed::TabScroll(right, r) => {
                    let tabs = matches!(placed, Placed::TabScroll(..));
                    let hit = match (tabs, right) {
                        (true, true) => RibbonHit::TabScrollRight,
                        (true, false) => RibbonHit::TabScrollLeft,
                        (false, true) => RibbonHit::ScrollRight,
                        (false, false) => RibbonHit::ScrollLeft,
                    };
                    if let Some(c) = tint(hit) {
                        radius_rect(p, r, c);
                    }
                    let (offset, limit) = if tabs {
                        (self.tab_scroll, self.tab_scroll_max)
                    } else {
                        (self.scroll, self.scroll_max)
                    };
                    let enabled = if *right { offset < limit } else { offset > 0.0 };
                    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                    let k = self.px(4.0);
                    let d = if *right { 1.0 } else { -1.0 };
                    p.path(
                        &[
                            bo_render::PathSeg::M(cx - d * k / 2.0, cy - k),
                            bo_render::PathSeg::L(cx + d * k / 2.0, cy),
                            bo_render::PathSeg::L(cx - d * k / 2.0, cy + k),
                        ],
                        None,
                        Some((if enabled { t.text } else { t.border }, self.scale.max(1.0))),
                    );
                }
            }
        }
        if current_clip.is_some() {
            p.pop_clip();
        }
    }
}

/// A ribbon button drawn as a letter in the formatting it applies.
struct GlyphButton {
    text: &'static str,
    family: &'static str,
    /// Size in interface pixels at 100 %.
    size: f32,
    weight: u16,
    italic: bool,
    underline: bool,
    decor: Decor,
}

/// What is drawn with a [`GlyphButton`]'s letter.
#[derive(Clone, Copy, PartialEq)]
enum Decor {
    None,
    /// A line through the middle.
    Strike,
    /// A small index digit below / above.
    Sub,
    Sup,
    /// A small accent triangle pointing up / down at the top right.
    Grow,
    Shrink,
    /// A colour bar under the letter (the colour the button applies).
    Bar(Rgba),
    /// The letter outlined in blue (text effects).
    Effects,
}

impl GlyphButton {
    fn font(&self, size: f32) -> Font<'static> {
        Font {
            family: self.family,
            size,
            weight: self.weight,
            italic: self.italic,
            underline: self.underline,
        }
    }
}

const GLYPH_BUTTONS: [(&str, GlyphButton); 11] = [
    (
        "format.growStep",
        GlyphButton {
            text: "A",
            family: "Segoe UI",
            size: 18.0,
            weight: 400,
            italic: false,
            underline: false,
            decor: Decor::Grow,
        },
    ),
    (
        "format.shrinkStep",
        GlyphButton {
            text: "A",
            family: "Segoe UI",
            size: 15.0,
            weight: 400,
            italic: false,
            underline: false,
            decor: Decor::Shrink,
        },
    ),
    (
        "format.bold",
        GlyphButton {
            text: "B",
            family: "Segoe UI",
            size: 18.0,
            weight: 700,
            italic: false,
            underline: false,
            decor: Decor::None,
        },
    ),
    (
        "format.italic",
        GlyphButton {
            text: "I",
            family: "Georgia",
            size: 18.0,
            weight: 400,
            italic: true,
            underline: false,
            decor: Decor::None,
        },
    ),
    (
        "format.underline",
        GlyphButton {
            text: "U",
            family: "Segoe UI",
            size: 18.0,
            weight: 400,
            italic: false,
            underline: true,
            decor: Decor::None,
        },
    ),
    (
        "format.strike",
        GlyphButton {
            text: "abc",
            family: "Segoe UI",
            size: 13.0,
            weight: 400,
            italic: false,
            underline: false,
            decor: Decor::Strike,
        },
    ),
    (
        "format.subscript",
        GlyphButton {
            text: "x",
            family: "Segoe UI",
            size: 17.0,
            weight: 600,
            italic: false,
            underline: false,
            decor: Decor::Sub,
        },
    ),
    (
        "format.superscript",
        GlyphButton {
            text: "x",
            family: "Segoe UI",
            size: 17.0,
            weight: 600,
            italic: false,
            underline: false,
            decor: Decor::Sup,
        },
    ),
    (
        "format.color",
        GlyphButton {
            text: "A",
            family: "Segoe UI",
            size: 17.0,
            weight: 400,
            italic: false,
            underline: false,
            decor: Decor::Bar(Rgba(0xff, 0, 0, 255)),
        },
    ),
    (
        "view.formattingMarks",
        GlyphButton {
            text: "\u{b6}",
            family: "Segoe UI",
            size: 17.0,
            weight: 400,
            italic: false,
            underline: false,
            decor: Decor::None,
        },
    ),
    (
        "format.textEffects",
        GlyphButton {
            text: "A",
            family: "Segoe UI",
            size: 18.0,
            weight: 700,
            italic: false,
            underline: false,
            decor: Decor::Effects,
        },
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl TextMeasure for Fixed {
        fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
            (
                text.chars().count() as f32 * style.size * 0.55,
                style.size * 1.33,
            )
        }
    }

    #[test]
    fn every_menu_button_anchors_its_menu() {
        // A menu opens under its button only when the id resolves to a static id;
        // otherwise it would open in the window's top-left corner.
        let ribbon = Ribbon::new();
        for tab in &ribbon.tabs {
            for g in &tab.groups {
                for item in g.large.iter().chain(g.rows.iter().flatten()) {
                    if let Item::Button(id) | Item::Large(id) | Item::Labeled(id) = item {
                        if crate::catalog::is_menu(id) {
                            assert_eq!(crate::commands::static_id(id), *id, "{id}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn gallery_previews_fit_wide_fonts_without_losing_vietnamese() {
        struct Wide;
        impl TextMeasure for Wide {
            fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
                Fixed.measure(text, style)
            }
            fn measure_font(&self, text: &str, font: &Font<'_>) -> (f32, f32) {
                let width = match font.family {
                    "wide" => 25.0,
                    "extra-wide" => 40.0,
                    "oversize" => font.size * 5.0,
                    _ => 5.0,
                };
                (text.chars().count() as f32 * width, font.size)
            }
        }
        let make = |family: &str| StyleEntry {
            id: family.into(),
            name: family.into(),
            kind: "paragraph",
            quick: true,
            preview: bo_engine::gallery::StylePreview {
                font: family.into(),
                size: 16.0,
                bold: false,
                italic: false,
                underline: false,
                color: "000000".into(),
            },
            ui_priority: None,
            recommended: true,
            in_use: true,
            linked: None,
            based_on: None,
            is_default: false,
        };
        let styles = [
            make("regular"),
            make("wide"),
            make("extra-wide"),
            make("oversize"),
        ];
        let mut ribbon = Ribbon::new();
        ribbon.layout(&Wide, &Strings::load("en-US"), 1400.0, 1.0, 0.0);
        ribbon.measure_previews(&Wide, &styles);
        assert_eq!(ribbon.previews[0].0, "AaĐđ");
        assert_eq!(ribbon.previews[1].0, "AĐ");
        assert_eq!(ribbon.previews[2].0, "Đ");
        assert_eq!(ribbon.previews[3], ("Đ", 14.0));
    }

    #[test]
    fn ribbon_controls_follow_the_ribbon_top_at_each_scale() {
        let strings = Strings::load("vi-VN");
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let mut ribbon = Ribbon::new();
            ribbon.layout(&Fixed, &strings, 3000.0, scale, 0.0);
            let controls = [
                RibbonHit::File,
                RibbonHit::Tab(0),
                RibbonHit::FontBox,
                RibbonHit::Command("edit.paste"),
                RibbonHit::StylesMore,
            ];
            let before: Vec<_> = controls
                .iter()
                .map(|&h| ribbon.rect_of(h).unwrap())
                .collect();
            let offset = 57.0 * scale;
            ribbon.layout(&Fixed, &strings, 3000.0, scale, offset);
            for (hit, original) in controls.iter().zip(before) {
                let rect = ribbon.rect_of(*hit).unwrap();
                assert_eq!(rect[1], original[1] + offset);
                assert_eq!(
                    ribbon.hit(rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0),
                    Some(*hit)
                );
                assert!(rect[1] >= ribbon.top() && rect[1] + rect[3] <= ribbon.bottom());
            }
        }
    }

    #[test]
    fn gallery_compacts_before_hiding_later_commands() {
        for locale in ["vi-VN", "en-US"] {
            let strings = Strings::load(locale);
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let mut ribbon = Ribbon::new();
                ribbon.layout(&Fixed, &strings, 4000.0, scale, 40.0);
                let full_end = ribbon.rect_of(RibbonHit::Command("edit.replace")).unwrap();
                let width = full_end[0] + full_end[2] + 8.0 * scale
                    - 2.0 * (ribbon.px(78.0) + ribbon.px(2.0));
                ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                let compact_end = ribbon.rect_of(RibbonHit::Command("edit.replace")).unwrap();
                assert!(
                    compact_end[0] + compact_end[2] <= width,
                    "{locale} at {scale}"
                );
                assert!(ribbon.rect_of(RibbonHit::Style(TILES - 1)).is_none());
                let more = ribbon.rect_of(RibbonHit::StylesMore).unwrap();
                assert!(more[0] + more[2] <= width);
                assert_eq!(
                    ribbon.hit(more[0] + more[2] / 2.0, more[1] + more[3] / 2.0),
                    Some(RibbonHit::StylesMore)
                );
            }
        }
    }

    #[test]
    fn narrow_ribbon_scrolls_to_last_command_and_back() {
        for locale in ["vi-VN", "en-US"] {
            let strings = Strings::load(locale);
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let mut ribbon = Ribbon::new();
                let width = 480.0 * scale;
                ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                assert!(ribbon.scroll_max > 0.0);
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::ScrollRight).unwrap();
                while ribbon.scroll < ribbon.scroll_max {
                    assert!(ribbon.press(x + w / 2.0, y + h / 2.0));
                    assert_eq!(
                        ribbon.release(x + w / 2.0, y + h / 2.0).0,
                        Some(RibbonHit::ScrollRight)
                    );
                    ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                }
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::Command("edit.replace")).unwrap();
                assert_eq!(
                    ribbon.hit(x + w / 2.0, y + h / 2.0),
                    Some(RibbonHit::Command("edit.replace"))
                );
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::ScrollLeft).unwrap();
                while ribbon.scroll > 0.0 {
                    assert!(ribbon.press(x + w / 2.0, y + h / 2.0));
                    ribbon.release(x + w / 2.0, y + h / 2.0);
                    ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                }
                assert_eq!(ribbon.hit(x + w / 2.0, y + h / 2.0), None);
                ribbon.layout(&Fixed, &strings, 4000.0, scale, 40.0);
                assert!(ribbon.content_clip.is_none());
                assert_eq!(ribbon.scroll, 0.0);
            }
        }
    }

    #[test]
    fn narrow_tabs_keep_their_last_command_reachable() {
        let strings = Strings::load("vi-VN");
        let mut ribbon = Ribbon::new();
        for tab in 0..ribbon.tabs.len() {
            let id = ribbon.tabs[tab]
                .groups
                .iter()
                .rev()
                .flat_map(|g| g.rows.iter().rev().flatten().chain(g.large.iter().rev()))
                .find_map(|item| match item {
                    Item::Button(id) | Item::Large(id) | Item::Labeled(id) => Some(*id),
                    _ => None,
                })
                .unwrap();
            ribbon.active = tab;
            ribbon.scroll = 0.0;
            ribbon.layout(&Fixed, &strings, 480.0, 1.0, 40.0);
            for _ in 0..60 {
                if ribbon.scroll >= ribbon.scroll_max {
                    break;
                }
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::ScrollRight).unwrap();
                ribbon.press(x + w / 2.0, y + h / 2.0);
                ribbon.release(x + w / 2.0, y + h / 2.0);
                ribbon.layout(&Fixed, &strings, 480.0, 1.0, 40.0);
            }
            assert_eq!(ribbon.scroll, ribbon.scroll_max, "tab {tab}");
            let [x, y, w, h] = ribbon.rect_of(RibbonHit::Command(id)).unwrap();
            assert_eq!(
                ribbon.hit(x + w / 2.0, y + h / 2.0),
                Some(RibbonHit::Command(id)),
                "tab {tab}: {id}"
            );
        }
    }

    #[test]
    fn scrolling_reveals_every_ribbon_command_in_full() {
        for locale in ["vi-VN", "en-US"] {
            let strings = Strings::load(locale);
            for scale in [1.0, 1.25, 1.5, 2.0] {
                for logical_width in [320.0, 480.0] {
                    let mut ribbon = Ribbon::new();
                    for tab in 0..ribbon.tabs.len() {
                        let ids: Vec<_> = ribbon.tabs[tab]
                            .groups
                            .iter()
                            .flat_map(|group| group.large.iter().chain(group.rows.iter().flatten()))
                            .filter_map(|item| match item {
                                Item::Button(id) | Item::Large(id) | Item::Labeled(id) => Some(*id),
                                _ => None,
                            })
                            .collect();
                        ribbon.active = tab;
                        ribbon.scroll = 0.0;
                        ribbon.layout(&Fixed, &strings, logical_width * scale, scale, 40.0);
                        let mut seen = std::collections::HashSet::new();
                        for _ in 0..30 {
                            for id in &ids {
                                if let Some([x, y, w, h]) = ribbon.rect_of(RibbonHit::Command(id)) {
                                    if ribbon.hit(x + w / 2.0, y + h / 2.0)
                                        == Some(RibbonHit::Command(id))
                                    {
                                        seen.insert(*id);
                                    }
                                }
                            }
                            if ribbon.scroll >= ribbon.scroll_max {
                                break;
                            }
                            let [x, y, w, h] = ribbon.rect_of(RibbonHit::ScrollRight).unwrap();
                            ribbon.press(x + w / 2.0, y + h / 2.0);
                            ribbon.release(x + w / 2.0, y + h / 2.0);
                            ribbon.layout(&Fixed, &strings, logical_width * scale, scale, 40.0);
                        }
                        for id in ids {
                            assert!(
                                seen.contains(id),
                                "{locale}, width {logical_width}, tab {tab}, scale {scale}: {id}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn narrow_tab_strip_reaches_and_selects_last_tab() {
        for locale in ["vi-VN", "en-US"] {
            let strings = Strings::load(locale);
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let mut ribbon = Ribbon::new();
                let width = 480.0 * scale;
                ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                assert!(ribbon.tab_scroll_max > 0.0);
                let file = ribbon.rect_of(RibbonHit::File).unwrap();
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::TabScrollRight).unwrap();
                for _ in 0..20 {
                    if ribbon.tab_scroll >= ribbon.tab_scroll_max {
                        break;
                    }
                    assert!(ribbon.press(x + w / 2.0, y + h / 2.0));
                    ribbon.release(x + w / 2.0, y + h / 2.0);
                    ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                }
                assert_eq!(ribbon.tab_scroll, ribbon.tab_scroll_max);
                assert_eq!(ribbon.rect_of(RibbonHit::File), Some(file));
                let last = ribbon.tabs.len() - 1;
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::Tab(last)).unwrap();
                assert_eq!(
                    ribbon.hit(x + w / 2.0, y + h / 2.0),
                    Some(RibbonHit::Tab(last))
                );
                ribbon.press(x + w / 2.0, y + h / 2.0);
                ribbon.release(x + w / 2.0, y + h / 2.0);
                ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                assert_eq!(ribbon.active, last);
                assert!(ribbon.tab_scroll > 0.0);
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::TabScrollLeft).unwrap();
                for _ in 0..20 {
                    if ribbon.tab_scroll == 0.0 {
                        break;
                    }
                    ribbon.press(x + w / 2.0, y + h / 2.0);
                    ribbon.release(x + w / 2.0, y + h / 2.0);
                    ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                }
                assert_eq!(ribbon.tab_scroll, 0.0);
                ribbon.layout(&Fixed, &strings, 4000.0, scale, 40.0);
                assert!(ribbon.tab_clip.is_none());
                ribbon.layout(&Fixed, &strings, width, scale, 40.0);
                let [x, y, w, h] = ribbon.rect_of(RibbonHit::Tab(last)).unwrap();
                assert_eq!(
                    ribbon.hit(x + w / 2.0, y + h / 2.0),
                    Some(RibbonHit::Tab(last))
                );
            }
        }
    }

    #[test]
    fn scrolling_reveals_every_tab_in_full() {
        for locale in ["vi-VN", "en-US"] {
            let strings = Strings::load(locale);
            for scale in [1.0, 1.25, 1.5, 2.0] {
                for logical_width in [320.0, 480.0] {
                    let mut ribbon = Ribbon::new();
                    ribbon.layout(&Fixed, &strings, logical_width * scale, scale, 40.0);
                    let mut seen = std::collections::HashSet::new();
                    for _ in 0..30 {
                        for tab in 0..ribbon.tabs.len() {
                            if let Some([x, y, w, h]) = ribbon.rect_of(RibbonHit::Tab(tab)) {
                                if ribbon.hit(x + w / 2.0, y + h / 2.0) == Some(RibbonHit::Tab(tab))
                                {
                                    seen.insert(tab);
                                }
                            }
                        }
                        if ribbon.tab_scroll >= ribbon.tab_scroll_max {
                            break;
                        }
                        let [x, y, w, h] = ribbon.rect_of(RibbonHit::TabScrollRight).unwrap();
                        ribbon.press(x + w / 2.0, y + h / 2.0);
                        ribbon.release(x + w / 2.0, y + h / 2.0);
                        ribbon.layout(&Fixed, &strings, logical_width * scale, scale, 40.0);
                    }
                    assert_eq!(
                        seen.len(),
                        ribbon.tabs.len(),
                        "{locale}, width {logical_width}, scale {scale}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_ribbon_button_does_something() {
        let ribbon = Ribbon::new();
        let icons = Icons::load();
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
                        let glyph = GLYPH_BUTTONS.iter().any(|(g, _)| g == id);
                        assert!(glyph || icons.has(icon_of(id)), "{id} has no icon");
                        let key = format!("commands.{id}");
                        assert_ne!(strings.get(&key), key, "missing label {key}");
                    }
                }
            }
        }
    }

    #[test]
    fn controls_do_not_overlap_group_labels_in_any_tab() {
        for locale in ["vi-VN", "en-US"] {
            let strings = Strings::load(locale);
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let mut ribbon = Ribbon::new();
                for tab in 0..ribbon.tabs.len() {
                    ribbon.active = tab;
                    ribbon.layout(&Fixed, &strings, 4000.0, scale, 50.0);
                    let label_top = ribbon
                        .placed
                        .iter()
                        .find_map(|p| match p {
                            Placed::GroupLabel(_, rect, _) => Some(rect.y),
                            _ => None,
                        })
                        .unwrap();
                    for placed in &ribbon.placed {
                        // Group settings buttons sit in the label row on purpose.
                        if let Placed::Item(
                            Item::Button("format.fontSettings" | "format.paragraphSettings"),
                            _,
                        ) = placed
                        {
                            continue;
                        }
                        let (Placed::Item(_, rect) | Placed::Tile(_, rect) | Placed::More(rect)) =
                            placed
                        else {
                            continue;
                        };
                        assert!(
                            rect.y + rect.h <= label_top,
                            "{locale}, tab {tab}, scale {scale}"
                        );
                    }
                }
            }
        }
    }
}
