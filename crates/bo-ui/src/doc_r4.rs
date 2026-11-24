//! BlueDoc parity round 4 (docs/ui/REPORT.md rounds 72+): the Insert, Layout and View tabs
//! laid out like the reference's — Cover Page, Header / Footer / Text Box / Equation /
//! Shapes / SmartArt / Chart / WordArt galleries, Page Number and Quick Parts menus, Date and
//! Time, Margins / Orientation / Size / Columns galleries, Indent / Spacing boxes, Arrange
//! menus, Focus, Gridlines, Zoom. A child module of the shell so the window's state is at
//! hand; menus, labels and drawings live in `doc_r4_menus`.
#![allow(clippy::match_same_arms)]
use super::DocWindow;
use crate::doc_r4_menus as m4;
use crate::doc_tiles::{Tile, TileLook, TileMenu, TileSection};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use serde_json::{json, Value};

/// Round-4 state of the window.
#[derive(Default)]
pub(super) struct R4 {
    /// View › Gridlines: a drawing grid over the pages.
    pub(super) gridlines: bool,
    /// View › Focus: the document alone (ribbon, rulers and status bar hidden).
    pub(super) focus: bool,
    /// Local time minus UTC in minutes (from the host), for Date and Time.
    utc_offset: i32,
    /// The ruler shown before Focus hid it.
    ruler: bool,
    /// Date and Time › Set As Default: the format offered first.
    pub(super) date_default: Option<usize>,
}

/// A gallery's sections, menu rows, look and tile drawing.
type Gallery = (
    Vec<TileSection>,
    Vec<(String, String, bool)>,
    TileLook,
    crate::doc_tiles::TileFn,
);

impl DocWindow {
    /// The host's local time minus UTC in minutes (Insert › Date & Time shows local time).
    pub fn set_utc_offset_minutes(&mut self, minutes: i32) {
        self.r4.utc_offset = minutes;
    }

    /// Local date and time now: year, month, day, weekday (0 = Sunday), hour, minute, second.
    pub(super) fn r4_now(&self) -> [i32; 7] {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
            + i64::from(self.r4.utc_offset) * 60;
        civil(secs)
    }

    fn r4_t(&self, key: &str, fallback: &str) -> String {
        self.t(key, fallback)
    }

    /// Run JSON engine commands in order (`true` when any changed the document).
    fn r4_run(&mut self, cmds: &[Value]) -> bool {
        let mut ok = false;
        for c in cmds {
            ok |= self.view.run_json(c).is_some();
        }
        self.relayout_pending = true;
        ok
    }

    /// The document has a cover page (Remove Current Cover Page is available).
    pub(super) fn has_cover_page(&self) -> bool {
        self.view
            .engine()
            .session()
            .is_some_and(|s| s.cover_page().is_some())
    }

    fn tiles_of(&self, prefix: &str, list: &[(&str, &str)], key: &str) -> Vec<Tile> {
        list.iter()
            .map(|(id, label)| Tile {
                id: format!("{prefix}.{id}"),
                label: self.r4_t(&format!("docR4.{key}.{id}"), label),
            })
            .collect()
    }

    fn gallery(&self, id: &str) -> Option<Gallery> {
        let tr = |k: &str, f: &str| self.r4_t(k, f);
        // The reference's round-4 galleries spell it "Built-in".
        let built_in = tr("docR4.builtIn", "Built-in");
        let section = |header: String, tiles: Vec<Tile>| vec![TileSection { header, tiles }];
        let row = |id: &str, k: &str, f: &str, on: bool| (id.to_owned(), tr(k, f), on);
        Some(match id {
            "insert.coverPage" => (
                section(built_in, self.tiles_of(id, &m4::COVERS, "cover")),
                vec![
                    row(
                        "insert.coverPage.more",
                        "docR4.moreCovers",
                        "More Cover Pages",
                        false,
                    ),
                    row(
                        "insert.coverPage.remove",
                        "docR4.removeCover",
                        "Remove Current Cover Page",
                        self.has_cover_page(),
                    ),
                    row(
                        "insert.coverPage.save",
                        "docR4.saveCover",
                        "Save Selection to Cover Page Gallery...",
                        false,
                    ),
                ],
                // The reference's tiles: 166 x 235 px at 125 % with the name under the page.
                TileLook {
                    cols: 3,
                    tile: (133.0, 188.0),
                    caption_top: false,
                },
                m4::draw_cover,
            ),
            "insert.header" | "insert.footer" => {
                let header = id == "insert.header";
                let (edit, remove) = if header {
                    (
                        tr("docR4.editHeader", "Edit Header"),
                        tr("docR4.removeHeader", "Remove Header"),
                    )
                } else {
                    (
                        tr("docR4.editFooter", "Edit Footer"),
                        tr("docR4.removeFooter", "Remove Footer"),
                    )
                };
                (
                    section(built_in, self.tiles_of(id, &m4::BANDS, "band")),
                    vec![
                        (format!("{id}.edit"), edit, true),
                        (format!("{id}.remove"), remove, true),
                    ],
                    TileLook {
                        cols: 1,
                        tile: (480.0, 86.0),
                        caption_top: true,
                    },
                    m4::draw_band,
                )
            }
            "insert.textBox" => (
                section(built_in, self.tiles_of(id, &m4::BOXES, "box")),
                vec![row(
                    "insert.textBox.draw",
                    "docR4.drawTextBox",
                    "Draw Text Box",
                    true,
                )],
                TileLook {
                    cols: 3,
                    tile: (130.0, 145.0),
                    caption_top: false,
                },
                m4::draw_box,
            ),
            "insert.equation" => {
                let tiles = m4::EQUATIONS
                    .iter()
                    .map(|(k, label, ..)| Tile {
                        id: format!("insert.equation.{k}"),
                        label: tr(&format!("docR4.eq.{k}"), label),
                    })
                    .collect();
                (
                    section(built_in, tiles),
                    vec![
                        row(
                            "insert.equation.new",
                            "docR4.newEquation",
                            "Insert New Equation",
                            true,
                        ),
                        row(
                            "insert.equation.ink",
                            "docR4.inkEquation",
                            "Ink Equation",
                            false,
                        ),
                    ],
                    TileLook {
                        cols: 1,
                        tile: (480.0, 70.0),
                        caption_top: true,
                    },
                    m4::draw_equation,
                )
            }
            "insert.shapes" => (
                m4::SHAPES
                    .iter()
                    .map(|(header, presets)| TileSection {
                        header: tr(&format!("docR4.shapes.{}", header.replace(' ', "")), header),
                        tiles: presets
                            .iter()
                            .map(|p| Tile {
                                id: format!("insert.shape.{p}"),
                                label: String::new(),
                            })
                            .collect(),
                    })
                    .collect(),
                Vec::new(),
                TileLook {
                    cols: 10,
                    tile: (26.0, 26.0),
                    caption_top: false,
                },
                m4::draw_shape,
            ),
            "insert.smartArt" => (
                section(
                    tr("docR4.smartArtHeader", "Choose a SmartArt Graphic"),
                    self.tiles_of(id, &m4::SMARTART, "smart"),
                ),
                Vec::new(),
                TileLook {
                    cols: 4,
                    tile: (110.0, 78.0),
                    caption_top: false,
                },
                m4::draw_smartart,
            ),
            "insert.chart" => (
                section(
                    tr("docR4.chartHeader", "Insert Chart"),
                    self.tiles_of(id, &m4::CHARTS, "chart"),
                ),
                Vec::new(),
                TileLook {
                    cols: 3,
                    tile: (110.0, 72.0),
                    caption_top: false,
                },
                m4::draw_chart,
            ),
            "insert.wordArt" => (
                vec![TileSection {
                    header: String::new(),
                    tiles: m4::WORDART
                        .iter()
                        .map(|w| Tile {
                            id: format!("insert.wordArt.{}", w.0),
                            label: String::new(),
                        })
                        .collect(),
                }],
                Vec::new(),
                TileLook {
                    cols: 5,
                    tile: (58.0, 50.0),
                    caption_top: false,
                },
                m4::draw_wordart,
            ),
            "layout.margins" => (
                section(
                    String::new(),
                    m4::MARGINS
                        .iter()
                        .map(|(k, _, _)| Tile {
                            id: format!("layout.margins.{k}"),
                            label: String::new(),
                        })
                        .chain(std::iter::empty())
                        .collect::<Vec<_>>()
                        .into_iter()
                        .map(|mut t| {
                            t.label.clear();
                            t
                        })
                        .collect(),
                ),
                vec![row(
                    "layout.pageSetup",
                    "docR4.customMargins",
                    "Custom Margins...",
                    true,
                )],
                TileLook {
                    cols: 1,
                    tile: (290.0, 60.0),
                    caption_top: false,
                },
                m4::draw_margins,
            ),
            "layout.orientation" => (
                section(
                    String::new(),
                    m4::ORIENTATIONS
                        .iter()
                        .map(|(k, _)| Tile {
                            id: format!("layout.orientation.{k}"),
                            label: String::new(),
                        })
                        .collect(),
                ),
                Vec::new(),
                TileLook {
                    cols: 1,
                    tile: (150.0, 44.0),
                    caption_top: false,
                },
                m4::draw_orientation,
            ),
            "layout.size" => (
                section(
                    String::new(),
                    m4::SIZES
                        .iter()
                        .map(|(k, ..)| Tile {
                            id: format!("layout.size.{k}"),
                            label: String::new(),
                        })
                        .collect(),
                ),
                vec![row(
                    "layout.pageSetup",
                    "docR4.morePaper",
                    "More Paper Sizes...",
                    true,
                )],
                TileLook {
                    cols: 1,
                    tile: (220.0, 48.0),
                    caption_top: false,
                },
                m4::draw_size,
            ),
            "layout.columns" => (
                section(
                    String::new(),
                    m4::COLUMNS
                        .iter()
                        .map(|(k, _)| Tile {
                            id: format!("layout.columns.{k}"),
                            label: String::new(),
                        })
                        .collect(),
                ),
                vec![row(
                    "layout.pageSetup",
                    "docR4.moreColumns",
                    "More Columns...",
                    true,
                )],
                TileLook {
                    cols: 1,
                    tile: (150.0, 44.0),
                    caption_top: false,
                },
                m4::draw_columns,
            ),
            _ => return None,
        })
    }

    /// The tile shown as current in gallery `id` (page setup galleries).
    fn gallery_current(&self, id: &str) -> Option<String> {
        let sec = &self.view.format().section;
        let cm = |tw: i32| tw as f32 * 2.54 / 1440.0;
        match id {
            "layout.margins" => m4::MARGINS
                .iter()
                .find(|(_, _, m)| {
                    (cm(sec.margin_top) - m[0]).abs() < 0.03
                        && (cm(sec.margin_left) - m[2]).abs() < 0.03
                        && (cm(sec.margin_right) - m[3]).abs() < 0.03
                })
                .map(|(k, ..)| format!("layout.margins.{k}")),
            "layout.orientation" => Some(if sec.width > sec.height {
                "layout.orientation.landscape".into()
            } else {
                "layout.orientation.portrait".into()
            }),
            "layout.size" => {
                let (w, h) = (sec.width.min(sec.height), sec.width.max(sec.height));
                m4::SIZES
                    .iter()
                    .find(|(_, _, a, b)| (cm(w) - a).abs() < 0.1 && (cm(h) - b).abs() < 0.1)
                    .map(|(k, ..)| format!("layout.size.{k}"))
            }
            "layout.columns" => Some(format!("layout.columns.{}", sec.columns.max(1))),
            _ => None,
        }
    }

    fn open_r4_gallery(&mut self, id: &'static str) -> bool {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        self.r4_open_gallery_at(id, anchor)
    }

    /// Gallery `id` under `anchor` (round 24: Equation › Equation opens Insert's equation
    /// gallery under its own button).
    pub(super) fn r4_open_gallery_at(&mut self, id: &'static str, anchor: [f32; 4]) -> bool {
        let Some((sections, rows, look, draw)) = self.gallery(id) else {
            return false;
        };
        self.menu = None;
        let mut m = TileMenu::open(anchor, self.size, sections, rows, look, draw, self.scale);
        m.current = self.gallery_current(id);
        if id == "insert.coverPage" {
            // Two rows at once with a scroll bar, menu rows with icons, as the reference.
            m.scroll_rows(2, self.size);
            m.row_icon = Some(m4::draw_cover_row);
            m.row_more = vec!["insert.coverPage.more".to_owned()];
        }
        self.r3.tiles = Some(m);
        true
    }

    fn open_r4_menu(&mut self, id: &'static str) -> bool {
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let wrap_on = |w: &Self, e: &str| match e.strip_prefix("picture.wrap.") {
            Some(_) => false,
            None => w.view.format().picture.is_none() && e.starts_with("picture."),
        };
        let mut entries = m4::menu(id);
        if entries.is_empty() {
            entries = crate::doc_r4_fit::menu(id);
        }
        let picture = self.view.format().picture.is_some() || self.view.format().picture_tools;
        let items: Vec<MenuItem> = entries
            .into_iter()
            .map(|e| MenuItem {
                // The wrap commands of Bring Forward / Send Backward read "Bring in Front of
                // Text" / "Send Behind Text" there (the Wrap Text menu's own words otherwise).
                label: match (id, e.id) {
                    ("layout.bringForward", "picture.wrap.inFront") => {
                        self.r4_t("docR3.bringInFrontOfText", e.label)
                    }
                    ("layout.sendBackward", "picture.wrap.behind") => {
                        self.r4_t("docR3.sendBehindText", e.label)
                    }
                    _ => self.r4_t(&format!("docR4.m.{}", e.id.replace('.', "_")), e.label),
                },
                choice: Choice::Command(e.id.to_owned()),
                checked: false,
                dim: m4::UNAVAILABLE.contains(&e.id)
                    || crate::doc_r4_fit::UNAVAILABLE.contains(&e.id)
                    || (crate::doc_r4_fit::ARRANGE.contains(&e.id) && !picture)
                    || (e.id.starts_with("picture.order.") && !picture)
                    || matches!(
                        e.id,
                        "insert.pageNumber.format" | "insert.pageNumber.margins"
                    )
                    || wrap_on(self, e.id)
                    || e.id.starts_with("picture.wrap.") && self.view.format().picture.is_none(),
            })
            .collect();
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// Edit the header (or footer) of the current page and run `then` inside it.
    fn in_header(&mut self, header: bool, then: &[Value]) -> bool {
        let page = self.view.current_page();
        let mut cmds = vec![json!({ "cmd": "editHeaderFooter", "page": page, "header": header })];
        cmds.extend_from_slice(then);
        self.r4_run(&cmds)
    }

    /// Run round-4 interface command `id` (`None`: not a round-4 command).
    #[allow(clippy::too_many_lines)]
    pub(super) fn r4_action(&mut self, id: &str) -> Option<bool> {
        if let Some(g) = m4::GALLERIES.iter().find(|g| **g == id) {
            return Some(self.open_r4_gallery(g));
        }
        if let Some(menu) = m4::MENUS.iter().find(|g| **g == id) {
            return Some(self.open_r4_menu(menu));
        }
        // Collapsed groups, split buttons and menus added to fit the reference's tabs.
        if !crate::doc_r4_fit::menu(id).is_empty() {
            let sid = crate::commands::static_id(id);
            return Some(!sid.is_empty() && self.open_r4_menu(sid));
        }
        match id {
            "insert.comment" => return Some(self.action("review.newComment")),
            "insert.picture.device" => {
                self.requests.push(super::Request::PickPicture);
                return Some(false);
            }
            _ if crate::doc_r4_fit::UNAVAILABLE.contains(&id) => return Some(false),
            _ => {}
        }
        if m4::UNAVAILABLE.contains(&id) {
            return Some(false);
        }
        if id == "insert.coverPage.remove" {
            return Some(self.r4_run(&[json!({ "cmd": "removeCoverPage" })]));
        }
        if let Some(d) = id.strip_prefix("insert.coverPage.") {
            if !m4::COVERS.iter().any(|c| c.0 == d) {
                return Some(false);
            }
            // At the document's start, then a page break so the body starts on page 2, in a
            // cover-page control (§17.5.2.11 docPartObj) that replaces the current cover.
            return Some(
                self.r4_run(&[json!({ "cmd": "insertCoverPage", "html": m4::cover_html(d) })]),
            );
        }
        for (prefix, header) in [("insert.header.", true), ("insert.footer.", false)] {
            if let Some(d) = id.strip_prefix(prefix) {
                return Some(match d {
                    "edit" => self.in_header(header, &[]),
                    "remove" => self.in_header(
                        header,
                        &[
                            json!({ "cmd": "selectAll" }),
                            json!({ "cmd": "deleteBackward" }),
                        ],
                    ),
                    d if m4::BANDS.iter().any(|b| b.0 == d) => self.in_header(
                        header,
                        &[
                            json!({ "cmd": "selectAll" }),
                            json!({ "cmd": "paste", "html": m4::band_html(d) }),
                        ],
                    ),
                    _ => false,
                });
            }
        }
        if let Some(d) = id.strip_prefix("insert.pageNumber.") {
            let page = json!({ "cmd": "insertField", "instr": "PAGE" });
            return Some(match d {
                "top" => self.in_header(true, &[page]),
                "bottom" => self.in_header(false, &[page]),
                "current" => self.r4_run(&[page]),
                "remove" => {
                    // Page numbers live in the header / footer: clear both on this page.
                    let clear = [
                        json!({ "cmd": "selectAll" }),
                        json!({ "cmd": "deleteBackward" }),
                    ];
                    self.in_header(true, &clear) | self.in_header(false, &clear)
                }
                _ => false,
            });
        }
        if let Some(d) = id.strip_prefix("insert.textBox.") {
            let (w, h) = match d {
                "sidebar" => (150.0, 500.0),
                "band" => (450.0, 60.0),
                "quote" | "corner" => (240.0, 90.0),
                "note" => (360.0, 110.0),
                _ => (144.0, 72.0),
            };
            return Some(self.r4_run(&[json!({
                "cmd": "insertShape", "textBox": true, "width": w, "height": h,
            })]));
        }
        if let Some(k) = id.strip_prefix("insert.equation.") {
            return Some(match k {
                "new" => {
                    self.r4_run(&[json!({ "cmd": "insertEquation", "text": "", "display": true })])
                }
                k => match m4::equation_text(k) {
                    Some(t) => self.r4_run(&[json!({
                        "cmd": "insertEquation", "text": t, "latex": true, "display": true,
                    })]),
                    None => false,
                },
            });
        }
        if let Some(layout) = id.strip_prefix("insert.smartArt.") {
            let text = self.r4_t("docR4.text", "[Text]");
            let items: Vec<Value> = (0..3)
                .map(|_| json!({ "text": text, "level": 0 }))
                .collect();
            return Some(self.r4_run(&[json!({
                "cmd": "insertSmartArt", "layout": layout, "items": items,
            })]));
        }
        if let Some(kind) = id.strip_prefix("insert.chart.") {
            return Some(self.r4_run(&[json!({ "cmd": "insertChart", "kind": kind })]));
        }
        if let Some(style) = id.strip_prefix("insert.wordArt.") {
            let Some((_, fill, outline)) = m4::WORDART.iter().find(|w| w.0 == style) else {
                return Some(false);
            };
            let text = self.r4_t("docR4.wordArtText", "Your text here");
            let mut patch = json!({ "color": fill });
            if let Some(o) = outline {
                patch["textOutline"] = json!({ "color": o, "width": 0.75 });
            }
            return Some(self.r4_run(&[
                json!({ "cmd": "insertShape", "textBox": true, "width": 300.0, "height": 60.0 }),
                json!({ "cmd": "insertText", "text": text }),
                json!({ "cmd": "move", "motion": "lineStart", "extend": true }),
                json!({ "cmd": "formatText", "format": { "kind": "size", "halfPoints": 72 } }),
                json!({ "cmd": "formatText", "format": { "kind": "advanced", "patch": patch } }),
            ]));
        }
        if let Some(p) = id.strip_prefix("insert.quickParts.") {
            let instr = match p {
                "title" => "TITLE",
                "author" => "AUTHOR",
                "subject" => "SUBJECT",
                "company" => "DOCPROPERTY Company",
                _ => return Some(false),
            };
            return Some(self.r4_run(&[json!({ "cmd": "insertField", "instr": instr })]));
        }
        if let Some(n) = id.strip_prefix("layout.columns.") {
            let widths = match n {
                "left" => Some((0.33, 0.67)),
                "right" => Some((0.67, 0.33)),
                _ => None,
            };
            if let Some((a, b)) = widths {
                let sec = &self.view.format().section;
                let text_w = (sec.width - sec.margin_left - sec.margin_right - 720).max(1440);
                let w = |f: f64| (f64::from(text_w) * f).round() as i32;
                return Some(self.r4_run(&[json!({
                    "cmd": "setColumns",
                    "columns": { "count": 2, "space": 720, "separator": false, "widths": [{ "width": w(a), "space": 720 }, { "width": w(b) }] },
                    "scope": "document",
                })]));
            }
            return None;
        }
        if let Some(k) = id.strip_prefix("layout.margins.") {
            if k == "mirrored" {
                let cm = |v: f32| (v * 1440.0 / 2.54).round() as i32;
                return Some(self.r4_run(&[json!({
                    "cmd": "setPageSetup", "margins": [cm(2.54), cm(2.54), cm(3.18), cm(2.54)],
                })]));
            }
            return None;
        }
        if id == "layout.size.b5" {
            let cm = |v: f32| (v * 1440.0 / 2.54).round() as i32;
            return Some(
                self.r4_run(&[json!({ "cmd": "setPageSetup", "size": [cm(18.2), cm(25.7)] })]),
            );
        }
        if let Some(z) = id.strip_prefix("view.zoomTo.") {
            let zoom = match z {
                "pageWidth" | "textWidth" => self.view.fit_zoom(false),
                "wholePage" => self.view.fit_zoom(true),
                pct => match pct.trim().trim_end_matches('%').parse::<f32>() {
                    Ok(v) => (v / 100.0).clamp(0.1, 5.0),
                    Err(_) => return Some(false),
                },
            };
            self.relayout_pending = true;
            return Some(self.view.set_zoom(zoom));
        }
        Some(match id {
            "insert.blankPage" => self.r4_run(&[
                json!({ "cmd": "insertPageBreak" }),
                json!({ "cmd": "insertPageBreak" }),
                json!({ "cmd": "move", "motion": "left" }),
            ]),
            "insert.textBox.draw" => self.r4_run(&[json!({
                "cmd": "insertShape", "textBox": true, "width": 144.0, "height": 72.0,
            })]),
            "layout.bringForward" => return self.r3_action("picture.bringForward"),
            "layout.sendBackward" => return self.r3_action("picture.sendBackward"),
            "layout.group" => false,
            "view.gridlines" => {
                self.r4.gridlines = !self.r4.gridlines;
                true
            }
            "view.focus" => {
                self.r4.focus = !self.r4.focus;
                if self.r4.focus {
                    self.r4.ruler = self.show_ruler;
                    self.show_ruler = false;
                } else {
                    self.show_ruler = self.r4.ruler;
                }
                self.relayout_pending = true;
                true
            }
            // One page per row again before the shell's own zoom commands.
            "view.onePage" | "view.pageWidth" | "view.zoomReset" => {
                self.view.set_columns(1);
                return None;
            }
            "view.vertical" => true,
            "insert.dateTime" => self.r3_open_form(crate::doc_forms::DocFormKind::DateTime),
            "view.zoomDialog" => self.r3_open_form(crate::doc_forms::DocFormKind::Zoom),
            // Two pages side by side, both whole in the window.
            "view.multiplePages" => {
                self.view.set_columns(2);
                let z = self.view.fit_zoom(true);
                self.relayout_pending = true;
                self.view.set_zoom((z * 0.5).max(0.1));
                true
            }
            _ => return None,
        })
    }
}

impl DocWindow {
    /// View › Gridlines: a light grid over each visible page's text area, every 0.5 cm
    /// (own spacing; the reference draws its drawing grid).
    pub(super) fn r4_paint_grid(&self, p: &mut dyn crate::Painter) {
        if !self.r4.gridlines {
            return;
        }
        let line = bo_render::Rgba(0xd9, 0xd9, 0xd9, 255);
        let sec = &self.view.format().section;
        let s = self.view.scale();
        let tw = |v: i32| v as f32 / 20.0 * s;
        let step = (0.5 / 2.54 * 72.0 * s).max(4.0);
        for slot in self.view.slots() {
            let (x0, y0) = (
                slot.x as f32 + tw(sec.margin_left),
                slot.y as f32 + tw(sec.margin_top),
            );
            let (x1, y1) = (
                (slot.x + slot.width) as f32 - tw(sec.margin_right),
                (slot.y + slot.height) as f32 - tw(sec.margin_bottom),
            );
            if y1 < 0.0 || y0 > self.size.1 {
                continue;
            }
            let mut y = y0;
            while y <= y1 {
                p.fill_rect(x0, y.round(), x1 - x0, 1.0, line);
                y += step;
            }
            let mut x = x0;
            while x <= x1 {
                p.fill_rect(x.round(), y0, 1.0, y1 - y0, line);
                x += step;
            }
        }
    }
}

/// Calendar fields of `secs` seconds since 1970 (proleptic Gregorian, days from civil).
fn civil(secs: i64) -> [i32; 7] {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    // 1970-01-01 was a Thursday.
    let wd = (days + 4).rem_euclid(7);
    let n = |v: i64| i32::try_from(v).unwrap_or(0);
    [
        n(y),
        n(m),
        n(d),
        n(wd),
        n(rem / 3600),
        n(rem % 3600 / 60),
        n(rem % 60),
    ]
}

#[cfg(test)]
#[path = "doc_r4_tests.rs"]
mod tests;
