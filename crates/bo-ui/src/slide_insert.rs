//! BlueSlide's Insert tab, the Home tab's Drawing and Editing groups and the contextual
//! Shape Format, Picture Format, Table Design and Table Layout tabs (N6, round 4), laid out
//! like the reference presentation app's: pickers for tables, shapes, icons, symbols, quick
//! styles and colours, the Header and Footer and Find / Replace dialogs, and files the host
//! picks (pictures, video, audio).
use super::{MenuKind, SlideWindow};
use crate::dialog::{Dialog, DialogButton, DialogKind};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::slide_pickers::{self as pk, PickKind, Picker};
use crate::spin_box::{self as sb, BoxLook, SpinHit};
use crate::{Input, Key, Painter, TextMeasure};
use base64::Engine as _;
use bo_engine::slide_edit::geom::HandleKind;
use bo_engine::slide_edit::hf::{HfDate, HfSpec};
use bo_engine::slide_edit::{AlignTo, Axis, SlideCommand, Transform};
use bo_render::Rgba;

/// A file the host is asked to pick, read and hand to [`SlideWindow::insert_file`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertFile {
    /// A picture (PNG, JPEG, SVG, …).
    Picture,
    /// A picture for a new slide (Photo Album).
    AlbumPicture,
    /// A 3-D model (glTF).
    Model3d,
    /// A video.
    Video,
    /// An audio clip.
    Audio,
    /// A picture replacing the selected one (Change Picture).
    ChangePicture,
    /// A workbook to embed (Insert Object from a file).
    Object,
}

/// Dialogs of these tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlideDialog {
    HeaderFooter,
    Find,
    Replace,
    SlideSize,
    /// Insert a hyperlink on the selection.
    Link,
    /// Slide Show tab dialogs.
    Show(super::showtab::ShowDialog),
    /// Compress Pictures.
    Compress,
    /// Add Section (`None`) or Rename Section.
    Section(Option<usize>),
    /// Keep the ink drawn in the show that ended?
    KeepInk,
    /// Paragraph indents and spacing.
    Paragraph,
}

/// Which contextual tabs the selection shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Context {
    #[default]
    None,
    Shape,
    Picture,
    Table,
    /// A chart: Chart Design and Format.
    Chart,
}

/// Commands of these tabs (always enabled unless [`needs_selection`]).
pub const COMMANDS: [&str; 50] = [
    "sl.object",
    "sl.photoAlbum",
    "sl.model3d",
    "sl.smartart",
    "sl.links",
    "sl.textStyles",
    "sl.qs0",
    "sl.qs1",
    "sl.qs2",
    "sl.table",
    "sl.pictures",
    "sl.shapes",
    "sl.icons",
    "sl.chart",
    "sl.comment",
    "sl.textBox",
    "sl.headerFooter",
    "sl.dateTime",
    "sl.slideNumber",
    "sl.symbols",
    "sl.media",
    "sl.arrange",
    "sl.quickStyles",
    "sl.fill",
    "sl.outline",
    "sl.effects",
    "sl.editing",
    "sl.painter",
    "sl.align",
    "sl.group",
    "sl.rotate",
    "sl.forward",
    "sl.backward",
    "sl.height",
    "sl.width",
    "sl.crop",
    "sl.resetPicture",
    "sl.picBorder",
    "sl.headerRow",
    "sl.bandedRows",
    "sl.shading",
    "sl.tblDelete",
    "sl.insAbove",
    "sl.insBelow",
    "sl.insLeft",
    "sl.insRight",
    "sl.merge",
    "sl.split",
    "sl.find",
    "sl.replace",
];

/// Style tiles of the Shape Format gallery: id, theme accent (6 = dark 1), quick style row.
/// The reference shows three at 1500 px (an outlined, a dark and an accent-filled one).
pub const STYLE_TILES: [(&str, usize, usize); 3] =
    [("sl.qs0", 5, 0), ("sl.qs1", 6, 1), ("sl.qs2", 0, 1)];

/// WordArt-free text styles of Insert > Text Styles: label key, fill, outline (theme accent
/// index, 6 = dark 1, 7 = light 1; `None` = none).
pub const TEXT_STYLES: [(&str, Option<usize>, Option<usize>); 5] = [
    ("tsDark", Some(6), None),
    ("tsAccent1", Some(0), None),
    ("tsAccent2", Some(1), None),
    ("tsOutlined", Some(7), Some(0)),
    ("tsOutline", None, Some(6)),
];

/// Width (logical px) of the style tiles (reference pitch 83 px at 125 %).
pub fn large_width(id: &str) -> Option<f32> {
    STYLE_TILES.iter().any(|(t, _, _)| *t == id).then_some(64.4)
}

/// Commands that act on the selected shapes (dimmed without a selection, as on the
/// reference).
pub fn needs_selection(id: &str) -> bool {
    matches!(
        id,
        "sl.quickStyles"
            | "sl.fill"
            | "sl.outline"
            | "sl.effects"
            | "sl.align"
            | "sl.group"
            | "sl.rotate"
            | "sl.forward"
            | "sl.backward"
            | "sl.height"
            | "sl.width"
    )
}

/// Label key of each command (`slideNative.*`).
pub const LABELS: [(&str, &str); 48] = [
    ("sl.screenshot", "screenshot"),
    ("sl.object", "object"),
    ("sl.photoAlbum", "photoAlbum"),
    ("sl.model3d", "model3d"),
    ("sl.smartart", "smartart"),
    ("sl.links", "links"),
    ("sl.textStyles", "textStyles"),
    ("sl.table", "table"),
    ("sl.pictures", "pictures"),
    ("sl.shapes", "shapes"),
    ("sl.icons", "icons"),
    ("sl.chart", "chart"),
    ("sl.comment", "comment"),
    ("sl.textBox", "textBox"),
    ("sl.headerFooter", "headerFooter"),
    ("sl.dateTime", "dateTime"),
    ("sl.slideNumber", "slideNumber"),
    ("sl.symbols", "symbols"),
    ("sl.media", "media"),
    ("sl.arrange", "arrange"),
    ("sl.quickStyles", "quickStyles"),
    ("sl.fill", "shapeFill"),
    ("sl.outline", "shapeOutline"),
    ("sl.effects", "shapeEffects"),
    ("sl.editing", "editing"),
    ("sl.painter", "formatPainter"),
    ("sl.align", "align"),
    ("sl.group", "group"),
    ("sl.rotate", "rotate"),
    ("sl.forward", "bringForward"),
    ("sl.backward", "sendBackward"),
    ("sl.height", "height"),
    ("sl.width", "width"),
    ("sl.crop", "crop"),
    ("sl.resetPicture", "resetPicture"),
    ("sl.picBorder", "pictureBorder"),
    ("sl.headerRow", "headerRow"),
    ("sl.bandedRows", "bandedRows"),
    ("sl.shading", "shading"),
    ("sl.tblDelete", "delete"),
    ("sl.insAbove", "insertAbove"),
    ("sl.insBelow", "insertBelow"),
    ("sl.insLeft", "insertLeft"),
    ("sl.insRight", "insertRight"),
    ("sl.merge", "mergeCells"),
    ("sl.split", "splitCells"),
    ("sl.find", "find"),
    ("sl.replace", "replace"),
];

/// Width (logical px) of the Size group's slots (label and spin box), measured on the
/// reference at 125 %: "Height:" then a 100 px box.
pub fn row_width(id: &str) -> Option<f32> {
    match id {
        // Icon, gap, then the 89.6 px box (reference x 1316-1428 at 125 %).
        "sl.height" | "sl.width" => Some(116.0),
        _ => None,
    }
}

/// Home tab: Drawing group (Shapes, Arrange, Quick Styles; fill, outline, effects).
pub(super) fn drawing_group() -> Group {
    use Item::{Button as B, Large as L};
    Group {
        label: "slideEditor.groupDrawing",
        large: vec![L("sl.shapes"), L("sl.arrange"), L("sl.quickStyles")],
        // Icons with their drop-down marks, as the reference shows them at 1500 px.
        rows: vec![
            vec![B("sl.fill")],
            vec![B("sl.outline")],
            vec![B("sl.effects")],
        ],
    }
}

/// Home tab: Editing group (one menu at this width on the reference).
pub(super) fn editing_group() -> Group {
    Group {
        label: COLLAPSED,
        large: vec![Item::Large("sl.editing")],
        rows: vec![],
    }
}

/// The Insert tab, groups as on the reference.
pub(super) fn insert_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: "ribbon.tabs.insert",
        groups: vec![
            Group {
                label: "slideEditor.groupSlides",
                large: vec![L("slide.new")],
                rows: vec![],
            },
            Group {
                label: "slideNative.tables",
                large: vec![L("sl.table")],
                rows: vec![],
            },
            Group {
                label: "slideNative.images",
                large: vec![L("sl.pictures")],
                // Photo Album: each picture on a new slide.
                rows: vec![vec![S("sl.screenshot")], vec![S("sl.photoAlbum")]],
            },
            Group {
                label: "slideNative.illustrations",
                large: vec![L("sl.shapes"), L("sl.icons")],
                rows: vec![
                    vec![S("sl.model3d")],
                    vec![S("sl.smartart")],
                    vec![S("sl.chart")],
                ],
            },
            Group {
                label: COLLAPSED,
                large: vec![L("sl.links")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.comments",
                large: vec![L("sl.comment")],
                rows: vec![],
            },
            Group {
                label: "slideNative.text",
                large: vec![L("sl.textBox"), L("sl.headerFooter"), L("sl.textStyles")],
                rows: vec![
                    vec![B("sl.dateTime")],
                    vec![B("sl.slideNumber")],
                    vec![B("sl.object")],
                ],
            },
            Group {
                label: COLLAPSED,
                large: vec![L("sl.symbols")],
                rows: vec![],
            },
            Group {
                label: COLLAPSED,
                large: vec![L("sl.media")],
                rows: vec![],
            },
        ],
    }
}

pub(super) fn size_group(crop: bool) -> Group {
    use Item::{Labeled as S, Large as L};
    Group {
        label: "slideNative.size",
        large: if crop { vec![L("sl.crop")] } else { vec![] },
        rows: vec![vec![S("sl.height")], vec![S("sl.width")]],
    }
}

/// Table Layout (a table selected).
pub(super) fn table_layout_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "slideNative.tableLayout",
        groups: vec![
            Group {
                label: "slideNative.rowsColumns",
                large: vec![L("sl.tblDelete")],
                rows: vec![
                    vec![S("sl.insAbove"), S("sl.insLeft")],
                    vec![S("sl.insBelow"), S("sl.insRight")],
                ],
            },
            Group {
                label: "slideNative.merge",
                large: vec![],
                rows: vec![vec![S("sl.merge")], vec![S("sl.split")]],
            },
            super::fmt::arrange_group(),
        ],
    }
}

/// Contextual tabs of a selection.
pub(super) fn context_tabs(c: Context) -> Vec<Tab> {
    match c {
        Context::None => vec![],
        Context::Shape => vec![super::fmt::shape_format_tab()],
        Context::Picture => vec![super::pic::picture_format_tab()],
        Context::Table => vec![super::fmt::table_design_tab(), table_layout_tab()],
        Context::Chart => super::chart::chart_tabs(),
    }
}

/// Own icons of these commands (own drawings), and aliases of shared ones.
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    for (id, existing) in [
        ("sl.table", "insert.table"),
        ("sl.pictures", "insert.picture"),
        ("sl.icons", "insert.icon"),
        ("sl.chart", "insert.chart"),
        ("sl.photoAlbum", "insert.photoAlbum"),
        ("sl.smartart", "insert.smartart"),
        ("sl.links", "insert.link"),
        ("sl.comment", "review.newComment"),
        ("sl.textBox", "insert.textBox"),
        ("sl.headerFooter", "insert.header"),
        ("sl.slideNumber", "insert.pageNumber"),
        ("sl.dateTime", "insert.field"),
        ("sl.symbols", "insert.symbol"),
        ("sl.media", "insert.media"),
        ("sl.editing", "edit.find"),
        ("sl.find", "edit.find"),
        ("sl.replace", "edit.replace"),
        ("sl.painter", "format.painter"),
        ("sl.fill", "format.fillColor"),
        ("sl.shading", "format.fillColor"),
        ("sl.outline", "format.line"),
        ("sl.picBorder", "format.line"),
        ("sl.forward", "arrange.forward"),
        ("sl.backward", "arrange.backward"),
        ("sl.group", "arrange.group"),
        ("sl.align", "arrange.align.left"),
        ("sl.tblDelete", "table.deleteTable"),
        ("sl.insAbove", "table.insertRowAbove"),
        ("sl.insBelow", "table.insertRowBelow"),
        ("sl.insLeft", "table.insertColLeft"),
        ("sl.insRight", "table.insertColRight"),
        ("sl.merge", "table.mergeCells"),
        ("sl.split", "table.splitCell"),
        ("sl.resetPicture", "picture.position"),
    ] {
        icons.alias(id, existing);
    }
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    let pale = Rgba(0xcf, 0xe0, 0xf3, 255);
    let orange = Rgba(0xed, 0x7d, 0x31, 255);
    let white = Rgba(255, 255, 255, 255);
    // Shapes: a square overlapped by a circle (own drawing in the reference's colours).
    icons.add_solid(
        "sl.shapes",
        &[
            ("M3 9h11v11h-11Z", Some(pale)),
            ("M3 9h11v11h-11Z", None),
            ("M15 3a6 6 0 1 0 0.01 0Z", Some(blue)),
        ],
    );
    // Arrange: two overlapping squares, the front one orange.
    icons.add_solid(
        "sl.arrange",
        &[
            ("M3 3h11v11h-11Z", Some(white)),
            ("M3 3h11v11h-11Z", None),
            ("M10 10h11v11h-11Z", Some(orange)),
        ],
    );
    // Quick Styles: a rounded tile with a brush.
    icons.add_solid(
        "sl.quickStyles",
        &[
            (
                "M4 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-14a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z",
                Some(pale),
            ),
            (
                "M4 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-14a2 2 0 0 1-2-2v-12a2 2 0 0 1 2-2Z",
                None,
            ),
            ("M13 13l8-8l2 2l-8 8Z", Some(Rgba(0x72, 0x72, 0x72, 255))),
        ],
    );
    icons.add_solid(
        "sl.effects",
        &[
            ("M5 5h13v13h-13Z", Some(Rgba(0xbf, 0xbf, 0xbf, 255))),
            ("M3 3h13v13h-13Z", Some(pale)),
            ("M3 3h13v13h-13Z", None),
        ],
    );
    icons.add_solid(
        "sl.rotate",
        &[("M6 12a6 6 0 1 1 6 6", None), ("M12 15v6l4-3Z", Some(blue))],
    );
    icons.add_solid("sl.crop", &[("M6 2v16h16", None), ("M2 6h16v16", None)]);
    icons.alias("sl.height", "sl.crop");
    // 3-D Models: a cube; Text Styles: an outlined letter A (own drawings).
    icons.add_solid(
        "sl.model3d",
        &[
            ("M12 3l8 4.5v9l-8 4.5l-8-4.5v-9Z", Some(pale)),
            ("M12 3l8 4.5v9l-8 4.5l-8-4.5v-9Z", None),
            ("M4 7.5l8 4.5l8-4.5M12 12v9", None),
        ],
    );
    icons.add_solid(
        "sl.textStyles",
        &[(
            "M10 3h4l7 18h-4l-1.6-4.5h-6.8l-1.6 4.5h-4ZM9.8 13h4.4l-2.2-6Z",
            Some(blue),
        )],
    );
    icons.alias("sl.width", "sl.crop");
}

/// Label of a group the reference shrinks to its one button at this width: the button
/// carries the group's name and the label row stays empty.
pub(super) const COLLAPSED: &str = "slideNative.collapsedGroup";

/// Name the commands of these tabs.
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    s.set(COLLAPSED, "");
    for (id, key) in LABELS {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
    for (id, _, _) in STYLE_TILES {
        s.set(&format!("commands.{id}"), "");
    }
    // The Size group's slots show an icon beside the spin box the window draws (no label
    // at the reference's width); the name is the tooltip.
    for id in ["sl.height", "sl.width"] {
        let v = s
            .get(&format!("commands.{id}"))
            .trim_end_matches(':')
            .to_owned();
        s.set(&format!("commands.tip.{id}"), &v);
        s.set(&format!("commands.{id}"), "");
    }
}

/// UTF-16 length of `s`.
fn u16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Byte index of `needle` in `hay` at or after byte `from`, ignoring case (ASCII and
/// simple case mappings).
fn find_ci(hay: &str, needle: &str, from: usize) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let lower = |s: &str| s.to_lowercase();
    let n = lower(needle);
    let mut i = from;
    while i <= hay.len() {
        if !hay.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let rest = &hay[i..];
        let take: String = rest.chars().take(needle.chars().count()).collect();
        if lower(&take) == n {
            return Some(i);
        }
        match rest.chars().next() {
            Some(c) => i += c.len_utf8(),
            None => break,
        }
    }
    None
}

/// Replace every case-insensitive occurrence of `what` in `text` by `with`; the count.
pub(super) fn replace_ci(text: &str, what: &str, with: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut n = 0;
    while let Some(j) = find_ci(text, what, i) {
        out.push_str(&text[i..j]);
        out.push_str(with);
        let len: usize = text[j..]
            .chars()
            .take(what.chars().count())
            .map(char::len_utf8)
            .sum();
        i = j + len;
        n += 1;
    }
    out.push_str(&text[i..]);
    (out, n)
}

impl SlideWindow {
    /// The contextual tabs the selection asks for.
    fn selection_context(&self) -> Context {
        let sel = self.view.selection();
        match sel.first().map(|s| s.kind) {
            None => Context::None,
            Some("picture") if sel.len() == 1 => Context::Picture,
            Some("table") if sel.len() == 1 => Context::Table,
            Some(_) if self.chart_selected() => Context::Chart,
            Some(_) => Context::Shape,
        }
    }

    /// Show the selection's contextual tabs (after the fixed ones, as on the reference);
    /// `true` when the ribbon changed.
    pub(super) fn sync_context(&mut self) -> bool {
        // The Background Removal tab stays until the mode ends.
        if self.bg_mode.is_some() {
            return false;
        }
        let c = self.selection_context();
        if c == self.context {
            if std::mem::take(&mut self.activate_context) && c != Context::None {
                let i = self.base_tabs;
                if self.ribbon.active_tab() != i {
                    self.ribbon.set_active_tab(i);
                    return true;
                }
            }
            return false;
        }
        let active = self.ribbon.active_tab();
        if active < self.base_tabs {
            self.last_base_tab = active;
        }
        let mut tabs = self.fixed_tabs();
        tabs.extend(context_tabs(c));
        self.ribbon.set_tabs(tabs);
        self.context = c;
        let activate = std::mem::take(&mut self.activate_context);
        if c != Context::None && activate {
            self.ribbon.set_active_tab(self.base_tabs);
        } else if active >= self.base_tabs {
            // A contextual tab was shown: back to the tab before it, or to the new
            // selection's first contextual tab.
            if c == Context::None {
                self.ribbon.set_active_tab(self.last_base_tab);
            } else {
                self.ribbon.set_active_tab(self.base_tabs);
            }
        }
        self.relayout_pending = true;
        true
    }

    /// The selected table's cells: table commands need a cell selection, so a table
    /// selected as a whole gets its first cell selected (row and column commands then act
    /// at the first cell, as the reference's do at the current cell).
    fn ensure_table_cells(&mut self) {
        if self.view.state().is_some_and(|s| s.table.is_some()) {
            return;
        }
        let Some(id) = self
            .view
            .selection()
            .first()
            .filter(|s| s.kind == "table")
            .map(|s| s.id)
        else {
            return;
        };
        self.view.run(SlideCommand::TableSelect {
            id: Some(id),
            r0: 0,
            c0: 0,
            r1: None,
            c1: None,
        });
    }

    /// Header Row and Banded Rows of the selected table (`a:tblPr` firstRow, bandRow).
    pub(super) fn table_flags(&self) -> Option<(bool, bool)> {
        if let Some(t) = self.view.state().and_then(|s| s.table.as_ref()) {
            return Some((t.first_row, t.band_row));
        }
        let id = self
            .view
            .selection()
            .first()
            .filter(|s| s.kind == "table")?
            .id;
        let slide = self
            .view
            .engine()
            .presentation()?
            .slides
            .get(self.view.current())?;
        slide.data.shapes.iter().find_map(|n| match n {
            bo_model::slide::ShapeNode::Frame(f) if f.nv.id == id => match &f.content {
                bo_model::slide::GraphicContent::Table(t) => {
                    let (flags, _) = bo_pptx::table_style::table_props(t.props.as_ref());
                    Some((flags.first_row, flags.band_row))
                }
                bo_model::slide::GraphicContent::Other(_) => None,
            },
            _ => None,
        })
    }

    /// Theme accent colours 1–6, dark 1 and light 1 of the deck.
    pub(super) fn theme_colors(&self) -> Vec<Rgba> {
        use bo_engine::slide_edit::master::COLOR_SETS;
        let design = self.view.state().and_then(|s| s.design.as_ref());
        let set = design
            .and_then(|d| COLOR_SETS.iter().find(|c| c.name == d.colors))
            .or_else(|| COLOR_SETS.first());
        let Some(set) = set else {
            return Vec::new();
        };
        let mut v: Vec<Rgba> = set.c[4..10].iter().map(|c| pk::rgb(*c)).collect();
        v.push(pk::rgb(set.c[0]));
        v.push(pk::rgb(set.c[1]));
        v
    }

    pub(super) fn label(&self, key: &str) -> String {
        self.strings.get(&format!("slideNative.{key}")).to_owned()
    }

    /// Open a picker under ribbon command `anchor`.
    pub(super) fn open_picker(&mut self, kind: PickKind, anchor: &'static str) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(anchor)) else {
            return;
        };
        let labels: Vec<String> = match kind {
            PickKind::Table => vec![self.label("insertTable"), self.label("tableSize")],
            PickKind::Shapes => pk::SHAPE_SECTIONS
                .iter()
                .map(|(k, _)| self.strings.get(k).to_owned())
                .collect(),
            PickKind::Icons => vec![self.label("icons")],
            PickKind::Symbols => vec![self.label("symbols")],
            PickKind::Styles => vec![self.label("themeStyles")],
            PickKind::Fill => vec![
                self.label("noFill"),
                self.label("themeColors"),
                self.label("standardColors"),
            ],
            PickKind::Outline => vec![
                self.label("noOutline"),
                self.label("themeColors"),
                self.label("standardColors"),
            ],
        };
        if kind == PickKind::Icons && self.library.is_empty() {
            self.library = pk::load_library();
        }
        let accents = self.theme_colors();
        self.menu = None;
        self.picker = Some((
            Picker::open(
                kind,
                r,
                self.size,
                self.scale,
                &labels,
                &accents,
                &self.library,
            ),
            anchor,
        ));
    }

    /// Input while a picker is open.
    pub(super) fn picker_input(&mut self, input: &Input) -> Option<bool> {
        let (p, anchor) = self.picker.as_mut()?;
        let anchor = *anchor;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => p.hover(x, y),
            Input::Wheel(dy) => p.wheel(dy),
            Input::Press { x, y, .. } => {
                let v = p.press(x, y);
                let inside = p.contains(x, y);
                if v.is_some() || !inside {
                    self.picker = None;
                    self.ribbon.clear_pointer();
                }
                if let Some(v) = v {
                    self.pick(&v, anchor);
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.picker = None;
                true
            }
            Input::Release { .. } | Input::Text(_) | Input::Key(..) => true,
            _ => return None,
        })
    }

    /// Apply what a picker cell offers.
    fn pick(&mut self, v: &str, anchor: &str) -> bool {
        let Some((kind, value)) = v.split_once(':') else {
            return false;
        };
        match kind {
            "table" => {
                let (c, r) = value.split_once('x').unwrap_or(("2", "2"));
                let (cols, rows) = (c.parse().unwrap_or(2), r.parse().unwrap_or(2));
                self.activate_context = true;
                self.view.run(SlideCommand::InsertTable {
                    rows,
                    cols,
                    rect: None,
                })
            }
            "shape" => {
                self.activate_context = true;
                self.view.run(SlideCommand::InsertShape {
                    preset: value.to_owned(),
                    rect: None,
                })
            }
            "icon" => {
                let Some(icon) = self.library.iter().find(|i| i.id == value) else {
                    return false;
                };
                let data = base64::engine::general_purpose::STANDARD.encode(icon.svg.as_bytes());
                self.activate_context = true;
                self.view
                    .run_picture(bo_engine::picture_tools::PictureCommand::PictureInsertSvg {
                        data,
                        name: format!("{value}.svg"),
                    })
            }
            "sym" => self.insert_text(value),
            "style" => {
                let mut it = value.split(':').filter_map(|n| n.parse::<usize>().ok());
                let (a, r) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
                let (fill, line, _) = pk::quick_style(&self.theme_colors(), a, r);
                let f = self.view.run(SlideCommand::SetFill {
                    color: Some(pk::hex(fill)),
                });
                let l = self.view.run(SlideCommand::SetLine {
                    color: Some(pk::hex(line)),
                    width: None,
                });
                f || l
            }
            "color" => {
                let color = (!value.is_empty()).then(|| value.to_owned());
                if let Some(r) = self.pick_text_color(anchor, color.clone()) {
                    return r;
                }
                match anchor {
                    "sl.outline" | "sl.picBorder" => {
                        self.view.run(SlideCommand::SetLine { color, width: None })
                    }
                    "sl.shading" => self.view.run(SlideCommand::TableFill { color }),
                    _ => self.view.run(SlideCommand::SetFill { color }),
                }
            }
            _ => false,
        }
    }

    /// Type `s` at the caret (text editing), else put it in a new text box.
    fn insert_text(&mut self, s: &str) -> bool {
        if self.view.text_edit().is_some() {
            return self.view.handle(Input::Text(s.to_owned()));
        }
        self.view.run(SlideCommand::InsertTextBox {
            text: s.to_owned(),
            rect: None,
        })
    }

    /// Run one of these tabs' commands; `None` when `id` is not one of them.
    pub(super) fn insert_action(&mut self, id: &str) -> Option<bool> {
        let picker = |w: &mut Self, kind, anchor: &'static str| {
            w.open_picker(kind, anchor);
            true
        };
        let menu = |w: &mut Self, items: Vec<(String, String)>, anchor: &'static str| {
            w.open_list(items, anchor);
            true
        };
        Some(match id {
            "sl.table" => picker(self, PickKind::Table, "sl.table"),
            "sl.shapes" => picker(self, PickKind::Shapes, "sl.shapes"),
            "sl.icons" => picker(self, PickKind::Icons, "sl.icons"),
            "sl.symbols" => picker(self, PickKind::Symbols, "sl.symbols"),
            "sl.quickStyles" => picker(self, PickKind::Styles, "sl.quickStyles"),
            "sl.fill" => picker(self, PickKind::Fill, "sl.fill"),
            "sl.shading" => picker(self, PickKind::Fill, "sl.shading"),
            "sl.outline" => picker(self, PickKind::Outline, "sl.outline"),
            "sl.picBorder" => picker(self, PickKind::Outline, "sl.picBorder"),
            // Insert Picture From / Photo Album / 3D Models menus (reference).
            "sl.pictures" => menu(
                self,
                vec![(self.label("fromDevice"), "ins:picture".into())],
                "sl.pictures",
            ),
            "sl.photoAlbum" => menu(
                self,
                vec![(self.label("newPhotoAlbum"), "ins:album".into())],
                "sl.photoAlbum",
            ),
            "sl.model3d" => menu(
                self,
                vec![(self.label("fromDevice"), "ins:model3d".into())],
                "sl.model3d",
            ),
            // Insert Object: a new embedded workbook or one from a file.
            "sl.object" => menu(
                self,
                vec![
                    (self.label("newWorkbook"), "ins:workbook".into()),
                    (self.label("objectFromFile"), "ins:object".into()),
                ],
                "sl.object",
            ),
            "sl.smartart" => menu(
                self,
                [
                    "list",
                    "process",
                    "cycle",
                    "hierarchy",
                    "relationship",
                    "matrix",
                    "pyramid",
                ]
                .iter()
                .map(|k| {
                    (
                        self.strings
                            .get(&format!("slideNative.smartart_{k}"))
                            .to_owned(),
                        format!("smart:{k}"),
                    )
                })
                .collect(),
                "sl.smartart",
            ),
            "sl.links" => menu(
                self,
                [
                    ("link", "lnk:url"),
                    ("actionNext", "lnk:next"),
                    ("actionPrev", "lnk:prev"),
                    ("actionFirst", "lnk:first"),
                    ("actionLast", "lnk:last"),
                ]
                .iter()
                .map(|(k, c)| (self.label(k), (*c).to_owned()))
                .collect(),
                "sl.links",
            ),
            "sl.textStyles" => menu(
                self,
                TEXT_STYLES
                    .iter()
                    .enumerate()
                    .map(|(i, (k, ..))| (self.label(k), format!("tstyle:{i}")))
                    .collect(),
                "sl.textStyles",
            ),
            "sl.media" => menu(
                self,
                vec![
                    (self.label("video"), "ins:video".into()),
                    (self.label("audio"), "ins:audio".into()),
                ],
                "sl.media",
            ),
            "sl.chart" => {
                self.activate_context = true;
                self.view.run(SlideCommand::InsertChart {
                    kind: "column".into(),
                    title: None,
                    data: None,
                    legend: None,
                    rect: None,
                    template: None,
                    texts: bo_sheet::chart_style::LayoutTexts::default(),
                })
            }
            "sl.comment" => self.action("review.newComment"),
            "sl.textBox" => {
                self.activate_context = true;
                self.view.action("insert.textBox")
            }
            "sl.headerFooter" | "sl.dateTime" | "sl.slideNumber" => {
                self.open_dialog(SlideDialog::HeaderFooter);
                true
            }
            "sl.editing" => menu(
                self,
                vec![
                    (self.label("find"), "ins:find".into()),
                    (self.label("replace"), "ins:replace".into()),
                    (self.label("selectAll"), "ins:selectAll".into()),
                    (self.label("selectionPane"), "side:selPane".into()),
                ],
                "sl.editing",
            ),
            "sl.find" => {
                self.open_dialog(SlideDialog::Find);
                true
            }
            "sl.replace" => {
                self.open_dialog(SlideDialog::Replace);
                true
            }
            "sl.painter" => {
                if self.fmt_painter.take().is_some() {
                    return Some(true);
                }
                let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
                let copied = !ids.is_empty() && self.view.run(SlideCommand::CopyFormat);
                if copied {
                    self.fmt_painter = Some(ids);
                }
                copied
            }
            "sl.arrange" => menu(
                self,
                [
                    ("bringFront", "arr:front"),
                    ("sendBack", "arr:back"),
                    ("bringForward", "arr:forward"),
                    ("sendBackward", "arr:backward"),
                    ("group", "arr:group"),
                    ("ungroup", "arr:ungroup"),
                    ("alignLeft", "aln:left"),
                    ("alignCenter", "aln:center"),
                    ("alignRight", "aln:right"),
                    ("alignTop", "aln:top"),
                    ("alignMiddle", "aln:middle"),
                    ("alignBottom", "aln:bottom"),
                    ("rotateRight", "rot:90"),
                    ("rotateLeft", "rot:-90"),
                    ("selectionPane", "side:selPane"),
                ]
                .iter()
                .map(|(k, c)| (self.label(k), (*c).to_owned()))
                .collect(),
                "sl.arrange",
            ),
            "sl.align" => menu(
                self,
                [
                    ("alignLeft", "aln:left"),
                    ("alignCenter", "aln:center"),
                    ("alignRight", "aln:right"),
                    ("alignTop", "aln:top"),
                    ("alignMiddle", "aln:middle"),
                    ("alignBottom", "aln:bottom"),
                    ("distributeH", "aln:dh"),
                    ("distributeV", "aln:dv"),
                ]
                .iter()
                .map(|(k, c)| (self.label(k), (*c).to_owned()))
                .collect(),
                "sl.align",
            ),
            "sl.group" => menu(
                self,
                vec![
                    (self.label("group"), "arr:group".into()),
                    (self.label("ungroup"), "arr:ungroup".into()),
                ],
                "sl.group",
            ),
            "sl.rotate" => menu(
                self,
                vec![
                    (self.label("rotateRight"), "rot:90".into()),
                    (self.label("rotateLeft"), "rot:-90".into()),
                ],
                "sl.rotate",
            ),
            "sl.forward" => self.view.action("arrange.forward"),
            "sl.backward" => self.view.action("arrange.backward"),
            "sl.effects" => menu(
                self,
                [
                    ("reflection", "fx:reflection"),
                    ("softEdges", "fx:soft"),
                    ("rotation3d", "fx:3d"),
                    ("noEffects", "fx:none"),
                ]
                .iter()
                .map(|(k, c)| (self.label(k), (*c).to_owned()))
                .collect(),
                "sl.effects",
            ),
            "sl.crop" => menu(
                self,
                [
                    ("cropOval", "crop:ellipse"),
                    ("cropRounded", "crop:roundRect"),
                    ("cropTriangle", "crop:triangle"),
                    ("ratio11", "ratio:1:1"),
                    ("ratio43", "ratio:4:3"),
                    ("ratio169", "ratio:16:9"),
                    ("cropFill", "pic:fill"),
                    ("cropFit", "pic:fit"),
                ]
                .iter()
                .map(|(k, c)| (self.label(k), (*c).to_owned()))
                .collect(),
                "sl.crop",
            ),
            "sl.resetPicture" => {
                self.view
                    .run_picture(bo_engine::picture_tools::PictureCommand::PictureReset {
                        size: false,
                    })
            }
            "sl.headerRow" | "sl.bandedRows" => {
                self.ensure_table_cells();
                let (first, band) = self.table_flags().unwrap_or((false, false));
                self.view.run(if id == "sl.headerRow" {
                    SlideCommand::TableOptions {
                        first_row: Some(!first),
                        band_row: None,
                    }
                } else {
                    SlideCommand::TableOptions {
                        first_row: None,
                        band_row: Some(!band),
                    }
                })
            }
            id if STYLE_TILES.iter().any(|(t, _, _)| *t == id) => {
                let Some((_, a, r)) = STYLE_TILES.iter().find(|(t, _, _)| *t == id) else {
                    return Some(false);
                };
                self.pick(&format!("style:{a}:{r}"), "sl.quickStyles")
            }
            "sl.tblDelete" => menu(
                self,
                vec![
                    (self.label("deleteColumns"), "tbl:cols".into()),
                    (self.label("deleteRows"), "tbl:rows".into()),
                    (self.label("deleteTable"), "tbl:table".into()),
                ],
                "sl.tblDelete",
            ),
            "sl.insAbove" => {
                self.ensure_table_cells();
                self.view.run(SlideCommand::TableInsertRow { below: false })
            }
            "sl.insBelow" => {
                self.ensure_table_cells();
                self.view.run(SlideCommand::TableInsertRow { below: true })
            }
            "sl.insLeft" => {
                self.ensure_table_cells();
                self.view.run(SlideCommand::TableInsertCol { right: false })
            }
            "sl.insRight" => {
                self.ensure_table_cells();
                self.view.run(SlideCommand::TableInsertCol { right: true })
            }
            "sl.merge" => {
                self.ensure_table_cells();
                self.view.run(SlideCommand::TableMerge)
            }
            "sl.split" => {
                self.ensure_table_cells();
                self.view.run(SlideCommand::TableSplit)
            }
            "sl.height" | "sl.width" => {
                self.size_edit = Some(sb::SpinEdit {
                    id: if id == "sl.height" {
                        "sl.height"
                    } else {
                        "sl.width"
                    },
                    text: String::new(),
                });
                true
            }
            _ => return None,
        })
    }

    /// Open a plain list menu of (label, command) under ribbon command `anchor`.
    pub(super) fn open_list(&mut self, items: Vec<(String, String)>, anchor: &'static str) {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(anchor)) else {
            return;
        };
        let items = items
            .into_iter()
            .map(|(label, c)| MenuItem {
                label,
                choice: Choice::Command(c),
                checked: false,
                dim: false,
            })
            .collect();
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, r, self.size, self.scale));
    }

    /// Menu choices of these tabs (`kind:value`); `None` for other menus.
    pub(super) fn choose_insert(&mut self, kind: &str, v: &str) -> Option<bool> {
        use bo_engine::picture_tools::PictureCommand as P;
        use bo_engine::slide_edit::effects::EffectOp;
        Some(match kind {
            "smart" => {
                self.activate_context = true;
                let items: Vec<bo_engine::slide_edit::smartart::SmartItem> = (0..3)
                    .map(|_| bo_engine::slide_edit::smartart::SmartItem {
                        text: self.label("smartText"),
                        ..Default::default()
                    })
                    .collect();
                self.view.run(SlideCommand::InsertSmartArt {
                    layout: v.to_owned(),
                    items,
                    colors: None,
                    style: None,
                    rect: None,
                })
            }
            "lnk" => {
                use bo_engine::slide_edit::present::LinkAction;
                let target = match v {
                    "next" => LinkAction::Next,
                    "prev" => LinkAction::Prev,
                    "first" => LinkAction::First,
                    "last" => LinkAction::Last,
                    _ => {
                        self.open_dialog(SlideDialog::Link);
                        return Some(true);
                    }
                };
                self.set_link(target)
            }
            "tstyle" => {
                let Some((_, fill, outline)) =
                    v.parse::<usize>().ok().and_then(|i| TEXT_STYLES.get(i))
                else {
                    return Some(false);
                };
                self.insert_text_style(*fill, *outline)
            }
            "ins" => match v {
                "picture" => {
                    self.file_request = Some(InsertFile::Picture);
                    true
                }
                "album" => {
                    self.file_request = Some(InsertFile::AlbumPicture);
                    true
                }
                "model3d" => {
                    self.file_request = Some(InsertFile::Model3d);
                    true
                }
                "object" => {
                    self.file_request = Some(InsertFile::Object);
                    true
                }
                "workbook" => {
                    self.activate_context = true;
                    self.view.run_ole(bo_engine::ole::OleCommand::OleInsert {
                        data: None,
                        range: None,
                        cells: None,
                        link: None,
                        auto_update: false,
                    })
                }
                "video" => {
                    self.file_request = Some(InsertFile::Video);
                    true
                }
                "audio" => {
                    self.file_request = Some(InsertFile::Audio);
                    true
                }
                "find" => {
                    self.open_dialog(SlideDialog::Find);
                    true
                }
                "replace" => {
                    self.open_dialog(SlideDialog::Replace);
                    true
                }
                "slideSize" => {
                    self.open_dialog(SlideDialog::SlideSize);
                    true
                }
                "formatBg" => {
                    self.bg_pane.shown = true;
                    self.comments.shown = false;
                    self.anim_pane.shown = false;
                    self.relayout_pending = true;
                    true
                }
                "hideGraphics" => self.toggle_bg_graphics(),
                _ => self.view.action("edit.selectAll"),
            },
            "noop" => false,
            "arr" => match v {
                "group" => self.view.action("arrange.group"),
                "ungroup" => self.view.action("arrange.ungroup"),
                other => self.view.action(&format!("arrange.{other}")),
            },
            "aln" => match v {
                "dh" => self.view.run(SlideCommand::Distribute {
                    axis: Axis::Horizontal,
                }),
                "dv" => self.view.run(SlideCommand::Distribute {
                    axis: Axis::Vertical,
                }),
                other => self.view.run(SlideCommand::Align {
                    to: match other {
                        "center" => AlignTo::Center,
                        "right" => AlignTo::Right,
                        "top" => AlignTo::Top,
                        "middle" => AlignTo::Middle,
                        "bottom" => AlignTo::Bottom,
                        _ => AlignTo::Left,
                    },
                }),
            },
            "rot" => self.rotate_by(v.parse().unwrap_or(90.0)),
            "fx" => {
                let ops = match v {
                    "reflection" => vec![EffectOp::Reflection {
                        size: Some(0.5),
                        distance: 0.0,
                    }],
                    "soft" => vec![EffectOp::SoftEdge { radius: Some(5.0) }],
                    "3d" => vec![EffectOp::Rotation3d {
                        preset: Some("perspectiveRight".into()),
                    }],
                    _ => vec![
                        EffectOp::Reflection {
                            size: None,
                            distance: 0.0,
                        },
                        EffectOp::SoftEdge { radius: None },
                        EffectOp::Rotation3d { preset: None },
                    ],
                };
                let mut done = false;
                for op in ops {
                    done |= self.view.run(SlideCommand::ShapeEffects { op });
                }
                done
            }
            "crop" => self.view.run_picture(P::PictureCropShape {
                preset: v.to_owned(),
            }),
            "ratio" => self.view.run_picture(P::PictureCropAspect {
                ratio: v.to_owned(),
            }),
            "pic" => self.view.run_picture(if v == "fill" {
                P::PictureFill
            } else {
                P::PictureFit
            }),
            "tbl" => match v {
                "cols" => self.view.run(SlideCommand::TableDeleteCols),
                "rows" => self.view.run(SlideCommand::TableDeleteRows),
                _ => self.view.run(SlideCommand::Delete),
            },
            _ => return None,
        })
    }

    /// Rotate the selection by `deg` degrees (clockwise), through the rotation handle's
    /// command: the pointer placed where the handle points after turning.
    fn rotate_by(&mut self, deg: f64) -> bool {
        let Some(b) = self.view.selection().first().map(|s| s.rect) else {
            return false;
        };
        let to = (b.rot + deg).to_radians();
        let r = 100.0;
        self.view.run(SlideCommand::Transform {
            transform: Transform::Rotate {
                x: b.cx + r * to.sin(),
                y: b.cy - r * to.cos(),
                step: false,
            },
            snap: false,
            tolerance: 0.0,
            preview: false,
        })
    }

    /// Height or width of the selection in the window's unit (inches or centimetres).
    fn size_value(&self, id: &str) -> Option<f64> {
        let b = self.view.selection().first()?.rect;
        let pt = if id == "sl.height" { b.h } else { b.w };
        Some(if self.metric {
            pt / 72.0 * 2.54
        } else {
            pt / 72.0
        })
    }

    fn fmt_size(&self, v: f64) -> String {
        if self.metric {
            format!("{v:.2} cm")
        } else {
            format!("{v:.2}\"")
        }
    }

    /// Set the selection's height or width (window unit), keeping its centre's corner.
    fn set_size(&mut self, id: &str, v: f64) -> bool {
        let Some(s) = self.view.selection().first() else {
            return false;
        };
        let b = s.rect;
        let pt = if self.metric {
            v / 2.54 * 72.0
        } else {
            v * 72.0
        }
        .max(1.0);
        let (handle, dx, dy) = if id == "sl.height" {
            (HandleKind::S, 0.0, pt - b.h)
        } else {
            (HandleKind::E, pt - b.w, 0.0)
        };
        self.view.run(SlideCommand::Transform {
            transform: Transform::Resize {
                handle,
                dx,
                dy,
                keep_aspect: false,
            },
            snap: false,
            tolerance: 0.0,
            preview: false,
        })
    }

    /// Spin box of the Size group over slot `id` (right part of the slot).
    pub(super) fn size_box(&self, id: &'static str) -> Option<[f32; 4]> {
        let r = self.ribbon.rect_of(RibbonHit::Command(id))?;
        let w = self.px(89.6);
        let h = self.px(25.6);
        Some([r[0] + r[2] - w, r[1] + (r[3] - h) / 2.0, w, h])
    }

    /// A press on the Size group's spin boxes: arrows step by 0.1 (inch or cm).
    pub(super) fn size_press(&mut self, x: f32, y: f32) -> Option<bool> {
        for id in ["sl.height", "sl.width"] {
            let Some(r) = self.size_box(id) else { continue };
            let Some(hit) = sb::spin_hit(r, self.scale, x, y) else {
                continue;
            };
            let Some(v) = self.size_value(id) else {
                return Some(false);
            };
            return Some(match hit {
                SpinHit::Up => self.set_size(id, v + 0.1),
                SpinHit::Down => self.set_size(id, (v - 0.1).max(0.01)),
                SpinHit::Field => {
                    self.size_edit = Some(sb::SpinEdit {
                        id,
                        text: String::new(),
                    });
                    true
                }
            });
        }
        if self.size_edit.is_some() {
            self.size_edit = None;
        }
        None
    }

    /// Typing in a Size spin box.
    pub(super) fn size_key(&mut self, input: &Input) -> Option<bool> {
        let edit = self.size_edit.as_mut()?;
        Some(match input {
            Input::Text(t) => edit.type_text(t),
            Input::Key(k, _) => match edit.key(*k) {
                sb::SpinKey::Commit => {
                    let e = self.size_edit.take()?;
                    let v = e
                        .text
                        .replace(',', ".")
                        .trim_end_matches('"')
                        .parse::<f64>();
                    match v {
                        Ok(v) if v > 0.0 => self.set_size(e.id, v),
                        _ => true,
                    }
                }
                sb::SpinKey::Cancel => {
                    self.size_edit = None;
                    true
                }
                sb::SpinKey::Up | sb::SpinKey::Down => {
                    let id = edit.id;
                    let up = edit.key(*k) == sb::SpinKey::Up;
                    self.size_edit = None;
                    let v = self.size_value(id).unwrap_or(0.0);
                    self.set_size(id, if up { v + 0.1 } else { (v - 0.1).max(0.01) })
                }
                sb::SpinKey::Edited => true,
                sb::SpinKey::Ignored => return None,
            },
            _ => return None,
        })
    }

    /// Open one of these tabs' dialogs.
    pub(super) fn open_dialog(&mut self, kind: SlideDialog) {
        let t = |w: &Self, k: &str| w.strings.get(k).to_owned();
        let d = match kind {
            SlideDialog::Show(k) => {
                self.open_show_dialog(k);
                return;
            }
            SlideDialog::Compress => {
                self.open_compress();
                return;
            }
            SlideDialog::Section(i) => {
                self.open_section_dialog(i);
                return;
            }
            SlideDialog::Paragraph => {
                self.paragraph_action("format.paragraphSettings");
                return;
            }
            SlideDialog::HeaderFooter => {
                let info = {
                    self.view.run(SlideCommand::HeaderFooterInfo);
                    self.view.state().and_then(|s| s.header_footer.clone())
                };
                let (date, num, footer, not_title) = info
                    .map_or((false, false, None, false), |i| {
                        (i.date.is_some(), i.slide_number, i.footer, i.not_on_title)
                    });
                Dialog::new(
                    DialogKind::Comment,
                    self.label("hfTitle"),
                    vec![(
                        self.label("hfFooterText"),
                        footer.clone().unwrap_or_default(),
                    )],
                    vec![
                        (self.label("applyAll"), DialogButton::Ok),
                        (self.label("apply"), DialogButton::Replace),
                        (self.label("cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                )
                .with_checks(vec![
                    (self.label("hfDate"), date),
                    (self.label("hfNumber"), num),
                    (self.label("hfFooter"), footer.is_some()),
                    (self.label("hfNotTitle"), not_title),
                ])
            }
            // A message box like the reference's after a show with ink: Keep / Discard.
            SlideDialog::KeepInk => Dialog::new(
                DialogKind::WordCount,
                // The product name (ADR-0006), as the title-bar shows it.
                "BlueSlide".to_owned(),
                Vec::new(),
                vec![
                    (t(self, "showTools.keep"), DialogButton::Ok),
                    (t(self, "showTools.discard"), DialogButton::Cancel),
                ],
                vec![t(self, "slideNative.keepInkQuestion")],
            ),
            SlideDialog::Link => Dialog::new(
                DialogKind::Link,
                self.label("linkTitle"),
                vec![(self.label("linkAddress"), "https://".into())],
                vec![
                    (self.label("ok"), DialogButton::Ok),
                    (self.label("cancel"), DialogButton::Cancel),
                ],
                Vec::new(),
            ),
            SlideDialog::SlideSize => {
                let (cx, cy) = self
                    .view
                    .state()
                    .and_then(|s| s.design.as_ref())
                    .map_or((12_192_000, 6_858_000), |d| (d.cx, d.cy));
                // EMU to inches (or centimetres).
                let unit = |v: i64| {
                    let inch = v as f64 / 914_400.0;
                    let v = if self.metric { inch * 2.54 } else { inch };
                    format!("{v:.2}")
                };
                Dialog::new(
                    DialogKind::Comment,
                    self.label("slideSizeTitle"),
                    vec![
                        (
                            self.label(if self.metric { "widthCm" } else { "widthIn" }),
                            unit(cx),
                        ),
                        (
                            self.label(if self.metric { "heightCm" } else { "heightIn" }),
                            unit(cy),
                        ),
                    ],
                    vec![
                        (self.label("ok"), DialogButton::Ok),
                        (self.label("cancel"), DialogButton::Cancel),
                    ],
                    Vec::new(),
                )
            }
            SlideDialog::Find => Dialog::new(
                DialogKind::Find,
                t(self, "slideEditor.find.title"),
                vec![(t(self, "slideEditor.find.what"), self.find_what.clone())],
                vec![
                    (t(self, "slideEditor.find.next"), DialogButton::FindNext),
                    (self.label("close"), DialogButton::Cancel),
                ],
                Vec::new(),
            ),
            SlideDialog::Replace => Dialog::new(
                DialogKind::Replace,
                t(self, "slideEditor.find.replaceTitle"),
                vec![
                    (t(self, "slideEditor.find.what"), self.find_what.clone()),
                    (t(self, "slideEditor.find.with"), String::new()),
                ],
                vec![
                    (t(self, "slideEditor.find.next"), DialogButton::FindNext),
                    (t(self, "slideEditor.find.replace"), DialogButton::Replace),
                    (
                        t(self, "slideEditor.find.replaceAll"),
                        DialogButton::ReplaceAll,
                    ),
                    (self.label("close"), DialogButton::Cancel),
                ],
                Vec::new(),
            ),
        };
        self.menu = None;
        self.picker = None;
        self.dialog = Some((d, kind));
        self.dialog_layout = true;
    }

    /// Input while a dialog is open (it is modal).
    pub(super) fn dialog_input(&mut self, input: &Input, text: &dyn TextMeasure) -> Option<bool> {
        let (d, kind) = self.dialog.as_mut()?;
        let kind = *kind;
        let button = match input {
            Input::Text(s) => {
                d.text(s);
                None
            }
            Input::Key(k, m) => d.key(*k, *m),
            Input::Press { x, y, .. } => d.press(*x, *y),
            Input::Move { x, y } => return Some(d.hover(*x, *y)),
            Input::Resize { .. } | Input::Scale(_) | Input::Tick | Input::Blink => {
                self.dialog_layout = true;
                return None;
            }
            _ => return Some(false),
        };
        if let Some(b) = button {
            let values = d.values();
            self.dialog_button(kind, b, &values);
        }
        if let Some((d, _)) = self.dialog.as_mut() {
            d.layout(text, self.size, self.scale);
        }
        Some(true)
    }

    pub(super) fn dialog_button(&mut self, kind: SlideDialog, b: DialogButton, values: &[String]) {
        if let SlideDialog::Show(k) = kind {
            self.show_dialog_button(k, b, values);
            return;
        }
        if kind == SlideDialog::KeepInk {
            let name = self.strings.get("showTools.inkName").to_owned();
            self.view.resolve_pending_ink(b == DialogButton::Ok, &name);
            self.dialog = None;
            return;
        }
        if b == DialogButton::Cancel {
            self.dialog = None;
            return;
        }
        let on = |i: usize| values.get(i).is_some_and(|v| v == "1");
        match kind {
            SlideDialog::Show(_) | SlideDialog::KeepInk => {}
            SlideDialog::Paragraph => self.paragraph_button(values),
            SlideDialog::Compress => self.compress_button(b, values),
            SlideDialog::Section(i) => self.section_button(i, b, values),
            SlideDialog::Link => {
                let url = values.first().cloned().unwrap_or_default();
                if !url.trim().is_empty() {
                    self.set_link(bo_engine::slide_edit::present::LinkAction::Url {
                        url: url.trim().to_owned(),
                    });
                }
                self.dialog = None;
            }
            SlideDialog::SlideSize => {
                let emu = |i: usize| {
                    let v: f64 = values.get(i)?.replace(',', ".").trim().parse().ok()?;
                    let inch = if self.metric { v / 2.54 } else { v };
                    // ECMA-376 §19.2.1.39 sldSz: 914400 to 51206400 EMU (1 to 56 inches).
                    (1.0..=56.0)
                        .contains(&inch)
                        .then(|| (inch * 914_400.0).round() as i64)
                };
                if let (Some(cx), Some(cy)) = (emu(0), emu(1)) {
                    self.view.run_master(
                        bo_engine::slide_edit::master::MasterCommand::SetSlideSize {
                            cx,
                            cy,
                            kind: Some("custom".into()),
                            mode: bo_engine::slide_edit::master::SizeMode::Fit,
                        },
                    );
                    self.view.set_zoom(None);
                    self.dialog = None;
                } else if let Some((d, _)) = self.dialog.as_mut() {
                    d.set_message(vec![self.strings.get("slideNative.sizeRange").to_owned()]);
                }
            }
            SlideDialog::HeaderFooter => {
                let footer = values.first().cloned().unwrap_or_default();
                let spec = HfSpec {
                    date: on(1).then(|| HfDate {
                        text: today(self.metric),
                        auto: true,
                    }),
                    slide_number: on(2),
                    footer: on(3).then_some(footer),
                    not_on_title: on(4),
                };
                self.view.run(SlideCommand::HeaderFooter {
                    spec,
                    slides: None,
                    all: b == DialogButton::Ok,
                });
                self.dialog = None;
            }
            SlideDialog::Find | SlideDialog::Replace => {
                let what = values.first().cloned().unwrap_or_default();
                let with = values.get(1).cloned().unwrap_or_default();
                self.find_what.clone_from(&what);
                let msg = match b {
                    DialogButton::Replace => {
                        self.replace_current(&what, &with);
                        self.find_next(&what)
                    }
                    DialogButton::ReplaceAll => {
                        let n = self.replace_all(&what, &with);
                        Some(
                            self.strings
                                .get("slideEditor.find.replaced")
                                .replace("{{count}}", &n.to_string()),
                        )
                    }
                    _ => self.find_next(&what),
                };
                if let Some((d, _)) = self.dialog.as_mut() {
                    d.set_message(msg.into_iter().collect());
                }
            }
        }
    }

    /// Select the next occurrence of `what` after the caret (wrapping round the deck);
    /// a message when there is none.
    pub(super) fn find_next(&mut self, what: &str) -> Option<String> {
        let not_found = || Some(self.label("notFound"));
        if what.is_empty() {
            return not_found();
        }
        let n = self.view.slide_count();
        let cur = self.view.current();
        let (cur_id, cur_end) = self
            .view
            .text_edit()
            .map_or((None, 0), |t| (Some(t.id), t.start.max(t.end)));
        let pres = self.view.engine().presentation()?;
        let mut hit = None;
        'slides: for k in 0..=n {
            let slide = (cur + k) % n.max(1);
            let shapes = bo_engine::slide_edit::a11y::slide_shapes(pres, slide);
            // On the starting slide, shapes before the caret's shape come last (k == n).
            let mut past = k > 0 || cur_id.is_none();
            for s in shapes {
                let Some(text) = s.text.as_deref() else {
                    if Some(s.id) == cur_id {
                        past = true;
                    }
                    continue;
                };
                let from = if k == 0 && Some(s.id) == cur_id {
                    past = true;
                    // Byte index of the caret's UTF-16 offset.
                    let mut u = 0;
                    text.char_indices()
                        .find(|(_, c)| {
                            let before = u;
                            u += c.len_utf16();
                            before >= cur_end
                        })
                        .map_or(text.len(), |(i, _)| i)
                } else if (k == 0 && !past) || (k == n && past && Some(s.id) != cur_id) {
                    if k == n {
                        break 'slides;
                    }
                    continue;
                } else {
                    0
                };
                if let Some(i) = find_ci(text, what, from) {
                    let len: usize = text[i..]
                        .chars()
                        .take(what.chars().count())
                        .map(char::len_utf8)
                        .sum();
                    let start = u16_len(&text[..i]);
                    hit = Some((slide, s.id, start, start + u16_len(&text[i..i + len])));
                    break 'slides;
                }
            }
        }
        let Some((slide, id, start, end)) = hit else {
            return not_found();
        };
        if slide != self.view.current() {
            self.view.go_to(slide);
        }
        self.view.run(SlideCommand::Select { ids: vec![id] });
        self.view.run(SlideCommand::TextSelect {
            id: Some(id),
            start,
            end,
            cell: None,
        });
        None
    }

    /// Replace the selected text when it is `what`.
    fn replace_current(&mut self, what: &str, with: &str) -> bool {
        let Some(t) = self.view.text_edit() else {
            return false;
        };
        let (a, b) = (t.start.min(t.end), t.start.max(t.end));
        let units: Vec<u16> = t.text.encode_utf16().collect();
        let Some(sel) = units.get(a..b).map(String::from_utf16_lossy) else {
            return false;
        };
        if sel.to_lowercase() != what.to_lowercase() {
            return false;
        }
        let before = String::from_utf16_lossy(&units[..a]);
        let after = String::from_utf16_lossy(&units[b..]);
        let id = t.id;
        let caret = a + u16_len(with);
        self.view.run(SlideCommand::SetText {
            id: Some(id),
            text: format!("{before}{with}{after}"),
            merge: false,
            sel: Some([caret, caret]),
            cell: None,
        })
    }

    /// Replace every occurrence in the deck; the number replaced.
    fn replace_all(&mut self, what: &str, with: &str) -> usize {
        if what.is_empty() {
            return 0;
        }
        let back = self.view.current();
        let mut total = 0;
        for slide in 0..self.view.slide_count() {
            let edits: Vec<(u32, String)> = self
                .view
                .engine()
                .presentation()
                .map(|p| {
                    bo_engine::slide_edit::a11y::slide_shapes(p, slide)
                        .into_iter()
                        .filter_map(|s| {
                            let (t, n) = replace_ci(s.text.as_deref()?, what, with);
                            (n > 0).then(|| {
                                total += n;
                                (s.id, t)
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if edits.is_empty() {
                continue;
            }
            self.view.go_to(slide);
            for (id, text) in edits {
                self.view.run(SlideCommand::SetText {
                    id: Some(id),
                    text,
                    merge: false,
                    sel: None,
                    cell: None,
                });
            }
        }
        self.view.go_to(back);
        total
    }

    /// Format Painter: give the next shape chosen the copied formatting; in text, the
    /// characters selected next (a click takes the word under the pointer, as the
    /// reference does). `release` is where the mouse button was let go, if it was.
    pub(super) fn paste_format_painter(&mut self, release: Option<(f32, f32)>) -> bool {
        let Some(from) = self.fmt_painter.as_ref() else {
            return false;
        };
        let c = self.view.canvas_rect();
        let on_canvas = release
            .filter(|(x, y)| *x >= c[0] && *x < c[0] + c[2] && *y >= c[1] && *y < c[1] + c[3]);
        if let (Some((x, y)), Some(te)) = (on_canvas, self.view.text_edit().cloned()) {
            if te.start == te.end {
                // A click: the word under the pointer (double click selects it).
                self.view.handle(Input::Press {
                    x,
                    y,
                    count: 2,
                    shift: false,
                });
                self.view.handle(Input::Release { x, y });
            }
            if self.view.text_edit().is_some_and(|t| t.start != t.end) {
                self.fmt_painter = None;
                return self.view.run(SlideCommand::PasteFormat);
            }
            return false;
        }
        let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
        if ids.is_empty() || &ids == from {
            return false;
        }
        self.fmt_painter = None;
        self.view.run(SlideCommand::PasteFormat)
    }

    /// A hyperlink / action on the selection (text range or shapes).
    fn set_link(&mut self, target: bo_engine::slide_edit::present::LinkAction) -> bool {
        use bo_engine::slide_edit::action::{ActionCommand, ActionSpec, Scope, SoundSpec, Trigger};
        self.view.run_action_cmd(ActionCommand::ActionSet {
            on: Trigger::Click,
            spec: ActionSpec {
                target,
                tooltip: None,
                highlight: false,
                sound: SoundSpec::default(),
            },
            scope: Scope::Auto,
        })
    }

    /// A text box in large letters with a text style (fill and outline in theme colours).
    fn insert_text_style(&mut self, fill: Option<usize>, outline: Option<usize>) -> bool {
        use bo_engine::slide_edit::effects::EffectOp;
        use bo_engine::slide_edit::text::TextFormat;
        let colors = self.theme_colors();
        let hex = |i: usize| colors.get(i).map(|c| pk::hex(*c));
        self.activate_context = true;
        if !self.view.run(SlideCommand::InsertTextBox {
            text: self.label("tsText"),
            rect: None,
        }) {
            return false;
        }
        self.view.format_text(TextFormat::Size { points: 54.0 });
        if let Some(c) = fill.and_then(hex) {
            self.view.format_text(TextFormat::Color { hex: Some(c) });
        }
        if let Some(c) = outline.and_then(hex) {
            self.view.run(SlideCommand::ShapeEffects {
                op: EffectOp::TextOutline {
                    color: Some(c),
                    width: Some(1.0),
                },
            });
        }
        true
    }

    /// Hide or show the current slide's background graphics.
    fn toggle_bg_graphics(&mut self) -> bool {
        let hide = !self.bg_pane.hide_graphics;
        let done = self.view.run_master(
            bo_engine::slide_edit::master::MasterCommand::HideBackgroundGraphics { hide },
        );
        if done {
            self.bg_pane.hide_graphics = hide;
        }
        done
    }

    /// Input for the Format Background pane.
    pub(super) fn bg_input(&mut self, input: &Input) -> Option<bool> {
        use crate::slide_bgpane::BgHit;
        use bo_engine::slide_edit::master::MasterCommand;
        let Input::Press { x, y, .. } = *input else {
            return None;
        };
        if !self.bg_pane.contains(x, y) {
            return None;
        }
        self.area_press = false;
        Some(match self.bg_pane.press(x, y) {
            Some(BgHit::Close) => {
                self.bg_pane.shown = false;
                self.relayout_pending = true;
                true
            }
            Some(BgHit::Color(c)) => {
                self.bg_pane.current = Some(c.clone());
                self.view.run_master(MasterCommand::SetBackground {
                    color: Some(c),
                    all: false,
                })
            }
            Some(BgHit::HideGraphics) => self.toggle_bg_graphics(),
            Some(BgHit::ApplyAll) => self.view.run_master(MasterCommand::SetBackground {
                color: self.bg_pane.current.clone(),
                all: true,
            }),
            Some(BgHit::Reset) => {
                self.bg_pane.current = None;
                self.view.run_master(MasterCommand::SetBackground {
                    color: None,
                    all: false,
                })
            }
            None => false,
        })
    }

    /// Paint the Format Background pane.
    pub(super) fn paint_bg_pane(&self, p: &mut dyn Painter) {
        let labels = [
            self.label("formatBackground"),
            self.label("fill"),
            self.label("solidFill"),
            self.label("hideGraphics"),
            self.label("applyAll"),
            self.strings
                .get("slideEditor.design.resetBackground")
                .to_owned(),
        ];
        self.bg_pane.paint(p, &labels, self.theme.text);
    }

    /// File the host should pick for an Insert command (taken once).
    pub fn take_file_request(&mut self) -> Option<InsertFile> {
        self.file_request.take()
    }

    /// Insert a picked picture, video or audio file on the current slide.
    pub fn insert_file(&mut self, bytes: Vec<u8>, name: &str) -> bool {
        self.insert_file_as(bytes, name, false)
    }

    /// Insert a picked file; `album`: on a new slide after the current one (Photo Album).
    pub fn insert_file_as(&mut self, bytes: Vec<u8>, name: &str, album: bool) -> bool {
        if album {
            self.view.new_slide(None);
        }
        let svg = name.to_ascii_lowercase().ends_with(".svg");
        self.activate_context = true;
        let done = if svg {
            let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            self.view
                .run_picture(bo_engine::picture_tools::PictureCommand::PictureInsertSvg {
                    data,
                    name: name.to_owned(),
                })
        } else {
            self.view.insert_media(bytes, name)
        };
        let changed = self.sync_context();
        done || changed
    }

    /// Paint pickers, Size spin boxes and dialogs of these tabs.
    pub(super) fn paint_insert(&self, p: &mut dyn Painter) {
        for id in ["sl.height", "sl.width"] {
            let Some(r) = self.size_box(id) else { continue };
            let editing = self.size_edit.as_ref().filter(|e| e.id == id);
            let value = match editing {
                Some(e) => e.text.clone(),
                None => self
                    .size_value(id)
                    .map(|v| self.fmt_size(v))
                    .unwrap_or_default(),
            };
            let style = self.control_style();
            // Cover the slot's own label area right of the label (the ribbon draws none).
            p.fill_rect(
                r[0] - 1.0,
                r[1] - 1.0,
                r[2] + 2.0,
                r[3] + 2.0,
                Rgba(0xf3, 0xf3, 0xf3, 255),
            );
            sb::paint_spin(
                p,
                r,
                self.scale,
                &BoxLook {
                    text: &value,
                    style,
                    ink: self.theme.text,
                    enabled: !self.view.selection().is_empty(),
                    caret: editing.map(|e| pk::estimate(&e.text, style)),
                    hover: None,
                },
            );
        }
        self.paint_style_tiles(p);

        if let Some((pick, _)) = &self.picker {
            pick.paint(p, &self.theme, &self.library);
        }
    }

    /// Shape Format's style gallery: a white box with three "Abc" tiles in the theme's
    /// quick styles (reference at 125 %: box y 106-197, rounded tiles 59 px on an 83 px
    /// pitch).
    fn paint_style_tiles(&self, p: &mut dyn Painter) {
        let rect = |id: &'static str| self.ribbon.rect_of(RibbonHit::Command(id));
        let (Some(first), Some(last)) = (rect(STYLE_TILES[0].0), rect(STYLE_TILES[2].0)) else {
            return;
        };
        let grey = |v: u8| Rgba(v, v, v, 255);
        let top = first[1] + self.px(1.6);
        let h = self.px(73.6);
        let left = first[0] - self.px(3.2);
        let right = last[0] + last[2];
        p.fill_rect(left, first[1], right - left + 2.0, first[3], grey(0xf3));
        p.fill_rect(left, top, right - left + 1.0, h, grey(0xd2));
        p.fill_rect(
            left + 1.0,
            top + 1.0,
            right - left - 1.0,
            h - 2.0,
            grey(0xff),
        );
        let accents = self.theme_colors();
        let style = crate::TextStyle {
            size: self.px(10.4),
            bold: false,
        };
        let hovered = self.ribbon.hovered();
        for (id, a, row) in STYLE_TILES {
            let Some(r) = rect(id) else { continue };
            if hovered == Some(RibbonHit::Command(id)) {
                p.fill_rect(r[0], top + self.px(3.2), r[2], h - self.px(6.4), grey(0xe6));
            }
            let (fill, line, ink) = if a == 6 {
                let dark = accents.get(6).copied().unwrap_or(Rgba(0, 0, 0, 255));
                (dark, dark, Rgba(255, 255, 255, 255))
            } else {
                pk::quick_style(&accents, a, row)
            };
            let s = self.px(47.2);
            let (x, y) = (
                (r[0] + (r[2] - s) / 2.0).round(),
                (top + (h - s) / 2.0).round(),
            );
            let k = self.px(6.4);
            let segs = {
                use bo_render::PathSeg as S;
                vec![
                    S::M(x + k, y),
                    S::L(x + s - k, y),
                    S::Q(x + s, y, x + s, y + k),
                    S::L(x + s, y + s - k),
                    S::Q(x + s, y + s, x + s - k, y + s),
                    S::L(x + k, y + s),
                    S::Q(x, y + s, x, y + s - k),
                    S::L(x, y + k),
                    S::Q(x, y, x + k, y),
                    S::Z,
                ]
            };
            p.path(&segs, Some(fill), Some((line, self.px(1.6))));
            let tw = pk::estimate("Abc", style);
            p.text(
                "Abc",
                (x + (s - tw) / 2.0).round(),
                (y + (s - style.size * 1.33) / 2.0).round(),
                style,
                ink,
            );
        }
    }
}

/// Today's date as the Header and Footer dialog writes it (`M/D/YYYY`, or `D/M/YYYY`).
fn today(day_first: bool) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    if day_first {
        format!("{d}/{m}/{y}")
    } else {
        format!("{m}/{d}/{y}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_ignores_case_and_counts() {
        let (t, n) = replace_ci("Cat cat CAT dog", "cat", "fox");
        assert_eq!((t.as_str(), n), ("fox fox fox dog", 3));
        let (t, n) = replace_ci("ăn Ăn", "ăn", "x");
        assert_eq!((t.as_str(), n), ("x x", 2));
    }

    #[test]
    fn today_is_a_plausible_date() {
        let d = today(false);
        let parts: Vec<u32> = d.split('/').filter_map(|p| p.parse().ok()).collect();
        assert_eq!(parts.len(), 3);
        assert!((1..=12).contains(&parts[0]) && (1..=31).contains(&parts[1]));
        assert!(parts[2] >= 2024);
    }
}
