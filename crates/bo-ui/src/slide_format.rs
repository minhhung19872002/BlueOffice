//! BlueSlide's contextual Shape Format and Table Design tabs as the reference presentation app
//! lays them out at 1500 px (N6, round 5): the shape style gallery with its "more" column, a
//! text style group (WordArt-free: quick text styles, text fill, outline and transform),
//! Accessibility (Alt Text), Arrange with Selection Pane; Table Design with the six table
//! style options and a gallery of the built-in table styles drawn as small tables. Also the
//! window side of the Selection Pane and the Alt Text pane (`slide_selpane`).
use super::ins::{self, STYLE_TILES, TEXT_STYLES};
use super::selpane::{SelRow, SideEdit, SideHit, SideKind};
use super::{spelling, SlideWindow};
use crate::ribbon::{Group, Item, RibbonHit, Tab};
use crate::slide_pickers::{self as pk, PickKind};
use crate::{Input, Painter, TextMeasure};
use bo_engine::slide_edit::shape_props::{is_decorative, ShapePropsCommand as Props};
use bo_engine::slide_edit::SlideCommand;
use bo_model::slide::{GraphicContent, ShapeNode};
use bo_render::Rgba;

/// Table style tiles shown in the ribbon (the reference shows five at 1500 px); the sixth
/// built-in style is in the gallery's "more" menu.
pub const TABLE_TILES: [&str; 5] = ["sl.tbl0", "sl.tbl1", "sl.tbl2", "sl.tbl3", "sl.tbl4"];

/// Table style option check boxes: id, `tblPr` attribute (§21.1.3.15).
pub const TABLE_FLAGS: [(&str, &str); 6] = [
    ("sl.headerRow", "firstRow"),
    ("sl.firstCol", "firstCol"),
    ("sl.totalRow", "lastRow"),
    ("sl.lastCol", "lastCol"),
    ("sl.bandedRows", "bandRow"),
    ("sl.bandedCols", "bandCol"),
];

/// Commands of these tabs.
pub const COMMANDS: [&str; 25] = [
    "sl.drawTable",
    "sl.eraser",
    "sl.tblBorders",
    "sl.tblEffects",
    "sl.penStyle",
    "sl.penWeight",
    "sl.penColor",
    "sl.picQuick",
    "sl.firstCol",
    "sl.totalRow",
    "sl.lastCol",
    "sl.bandedCols",
    "sl.tbl0",
    "sl.tbl1",
    "sl.tbl2",
    "sl.tbl3",
    "sl.tbl4",
    "sl.tblMore",
    "sl.stylesMore",
    "sl.textQuick",
    "sl.textFill",
    "sl.textOutline",
    "sl.textEffects",
    "sl.altText",
    "sl.selPane",
];

/// Label keys (`slideNative.*`).
const LABELS: [(&str, &str); 18] = [
    ("sl.drawTable", "drawTable"),
    ("sl.eraser", "eraser"),
    ("sl.tblBorders", "borders"),
    ("sl.tblEffects", "effects"),
    ("sl.penStyle", "penStyle"),
    ("sl.penWeight", "penWeight"),
    ("sl.penColor", "penColor"),
    ("sl.picQuick", "quickStyles"),
    ("sl.firstCol", "firstColumn"),
    ("sl.totalRow", "totalRow"),
    ("sl.lastCol", "lastColumn"),
    ("sl.bandedCols", "bandedColumns"),
    ("sl.textQuick", "quickStyles"),
    ("sl.textFill", "textFill"),
    ("sl.textOutline", "textOutline"),
    ("sl.textEffects", "textEffects"),
    ("sl.altText", "altText"),
    ("sl.selPane", "selectionPane"),
];

/// Commands that act on the selection (dimmed without one).
pub fn needs_selection(id: &str) -> bool {
    matches!(
        id,
        "sl.textQuick" | "sl.textFill" | "sl.textOutline" | "sl.textEffects" | "sl.altText"
    )
}

/// Widths (logical px) of the gallery slots: table tiles on the reference's 108 device px
/// pitch at 125 % (86.4 with the ribbon's gap), "more" columns 29.6 as the other galleries.
pub fn large_width(id: &str) -> Option<f32> {
    if TABLE_TILES.contains(&id) {
        return Some(84.0);
    }
    matches!(id, "sl.tblMore" | "sl.stylesMore").then_some(29.6)
}

/// Shape Format (a shape, text box or group selected).
pub(super) fn shape_format_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "slideNative.shapeFormat",
        groups: vec![
            Group {
                label: "slideNative.insertShapes",
                large: vec![L("sl.shapes"), L("sl.textBox")],
                rows: vec![],
            },
            Group {
                label: "slideNative.shapeStyles",
                large: STYLE_TILES
                    .iter()
                    .map(|(id, _, _)| L(id))
                    .chain([L("sl.stylesMore")])
                    .collect(),
                rows: vec![
                    vec![S("sl.fill")],
                    vec![S("sl.outline")],
                    vec![S("sl.effects")],
                ],
            },
            text_styles_group(),
            accessibility_group(),
            arrange_group(),
            ins::size_group(false),
        ],
    }
}

/// Accessibility: Alt Text.
pub(super) fn accessibility_group() -> Group {
    Group {
        label: "slideNative.accessibility",
        large: vec![Item::Large("sl.altText")],
        rows: vec![],
    }
}

/// Arrange as the reference shows it at 1500 px: Bring Forward, Send Backward and Selection
/// Pane with their names, Align, Group and Rotate as icons.
pub(super) fn arrange_group() -> Group {
    use Item::{Button as B, Labeled as S};
    Group {
        label: "slideNative.arrange",
        large: vec![],
        rows: vec![
            vec![S("sl.forward"), B("sl.align")],
            vec![S("sl.backward"), B("sl.group")],
            vec![S("sl.selPane"), B("sl.rotate")],
        ],
    }
}

/// Table Design (a table selected): style options, the style gallery and shading.
pub(super) fn table_design_tab() -> Tab {
    use Item::{Button as B, Labeled as S, Large as L};
    Tab {
        label: "slideNative.tableDesign",
        groups: vec![
            Group {
                label: "slideNative.tableStyleOptions",
                large: vec![],
                rows: vec![
                    vec![S("sl.headerRow"), S("sl.firstCol")],
                    vec![S("sl.totalRow"), S("sl.lastCol")],
                    vec![S("sl.bandedRows"), S("sl.bandedCols")],
                ],
            },
            Group {
                label: "slideNative.tableStyles",
                large: TABLE_TILES
                    .iter()
                    .map(|id| L(id))
                    .chain([L("sl.tblMore")])
                    .collect(),
                rows: vec![
                    vec![B("sl.shading")],
                    vec![B("sl.tblBorders")],
                    vec![B("sl.tblEffects")],
                ],
            },
            text_styles_group(),
            Group {
                label: "slideNative.drawBorders",
                large: vec![L("sl.drawTable"), L("sl.eraser")],
                rows: vec![
                    vec![S("sl.penStyle")],
                    vec![S("sl.penWeight")],
                    vec![S("sl.penColor")],
                ],
            },
        ],
    }
}

/// Text styles (WordArt-free): quick text styles, text fill, outline and transform.
pub(super) fn text_styles_group() -> Group {
    use Item::{Button as B, Large as L};
    Group {
        label: "slideNative.textStylesGroup",
        large: vec![L("sl.textQuick")],
        rows: vec![
            vec![B("sl.textFill")],
            vec![B("sl.textOutline")],
            vec![B("sl.textEffects")],
        ],
    }
}

/// The pen of Draw Borders: dash, width (points), colour.
#[derive(Debug, Clone, PartialEq)]
pub struct Pen {
    /// §20.1.10.48 preset dash (`solid`, `sysDash`, `sysDot`, `dash`).
    pub dash: &'static str,
    /// Width in points.
    pub width: f64,
    /// Colour `RRGGBB`.
    pub color: String,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            dash: "solid",
            width: 1.0,
            color: "000000".into(),
        }
    }
}

/// Pen dashes offered by Pen Style.
const DASHES: [(&str, &str); 4] = [
    ("dashSolid", "solid"),
    ("dashDash", "dash"),
    ("dashRound", "sysDot"),
    ("dashSquare", "sysDash"),
];

/// Pen weights offered (points).
const WEIGHTS: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5];

/// Border choices of Borders.
const BORDERS: [(&str, &str); 8] = [
    ("borderAll", "all"),
    ("borderOutside", "outside"),
    ("borderInside", "inside"),
    ("borderNone", "none"),
    ("borderTop", "top"),
    ("borderBottom", "bottom"),
    ("borderLeft", "left"),
    ("borderRight", "right"),
];

/// Picture quick styles (own set): label key, line colour / width, effect.
const PIC_STYLES: [(&str, &str); 5] = [
    ("pqWhite", "white"),
    ("pqBlack", "black"),
    ("pqSoft", "soft"),
    ("pqReflect", "reflect"),
    ("pqPerspective", "perspective"),
];

/// Own icons (own drawings) and aliases.
pub(super) fn install_icons(icons: &mut crate::icons::Icons) {
    for id in ["sl.tblMore", "sl.stylesMore"] {
        icons.alias(id, "transition.more");
    }
    icons.alias("sl.textQuick", "sl.textStyles");
    icons.alias("sl.picQuick", "sl.quickStyles");
    icons.alias("sl.tblEffects", "sl.effects");
    // Draw Table: a pencil over a grid; Eraser: the Draw tab's eraser.
    icons.add_solid(
        "sl.drawTable",
        &[
            ("M2 5h16v14h-16Z", Some(Rgba(255, 255, 255, 255))),
            ("M2 5h16v14h-16ZM2 12h16M10 5v14", None),
            (
                "M19 2l3 3l-10 10l-4 1l1-4Z",
                Some(Rgba(0x44, 0x80, 0xc0, 255)),
            ),
        ],
    );
    icons.alias("sl.eraser", "draw.eraser");
    icons.add_solid(
        "sl.tblBorders",
        &[
            ("M3 3h18v18h-18Z", Some(Rgba(255, 255, 255, 255))),
            ("M3 3h18v18h-18ZM12 3v18M3 12h18", None),
        ],
    );
    icons.add_solid(
        "sl.penStyle",
        &[("M2 12h4M9 12h4M16 12h4", None), ("M2 6h20", None)],
    );
    icons.add_solid(
        "sl.penWeight",
        &[
            ("M2 5h20v1h-20Z", Some(Rgba(0x26, 0x26, 0x26, 255))),
            ("M2 10h20v2h-20Z", Some(Rgba(0x26, 0x26, 0x26, 255))),
            ("M2 16h20v4h-20Z", Some(Rgba(0x26, 0x26, 0x26, 255))),
        ],
    );
    icons.add_solid(
        "sl.penColor",
        &[
            (
                "M15 2l4 4l-10 10l-5 1l1-5Z",
                Some(Rgba(0xf4, 0xf4, 0xf4, 255)),
            ),
            ("M15 2l4 4l-10 10l-5 1l1-5Z", None),
            ("M2 19h20v4h-20Z", Some(Rgba(0x26, 0x26, 0x26, 255))),
        ],
    );
    let blue = Rgba(0x44, 0x80, 0xc0, 255);
    let dark = Rgba(0x26, 0x26, 0x26, 255);
    let letter = "M10 2h4l6 14h-3l-1.4-3.6h-6.2l-1.4 3.6h-3ZM10.4 10h3.2l-1.6-4.4Z";
    icons.add_solid(
        "sl.textFill",
        &[(letter, Some(dark)), ("M3 19h18v4h-18Z", Some(dark))],
    );
    icons.add_solid(
        "sl.textOutline",
        &[
            (letter, Some(Rgba(255, 255, 255, 255))),
            (letter, None),
            ("M3 19h18v4h-18Z", Some(dark)),
        ],
    );
    icons.add_solid(
        "sl.textEffects",
        &[
            (
                "M12 1a11 11 0 1 0 0.01 0Z",
                Some(Rgba(0xd6, 0xe6, 0xf6, 255)),
            ),
            (letter, Some(blue)),
        ],
    );
    // Alt Text: a picture with a caption box (own drawing).
    icons.add_solid(
        "sl.altText",
        &[
            ("M2 3h16v13h-16Z", Some(Rgba(0xdd, 0xec, 0xf8, 255))),
            ("M2 3h16v13h-16Z", None),
            ("M3 15l5-6l4 4l2-2l4 4Z", Some(blue)),
            ("M10 14h12v8h-12Z", Some(Rgba(255, 255, 255, 255))),
            ("M10 14h12v8h-12Z", None),
            ("M12 17h8M12 19.5h6", None),
        ],
    );
    // Selection Pane: two stacked squares and a pointer.
    icons.add_solid(
        "sl.selPane",
        &[
            ("M2 2h10v10h-10Z", Some(Rgba(0xf0, 0xc0, 0x80, 255))),
            ("M2 2h10v10h-10Z", None),
            ("M6 8h10v10h-10Z", Some(Rgba(255, 255, 255, 255))),
            ("M6 8h10v10h-10Z", None),
            ("M14 12l7 7h-3.5l2 3.5l-1.5 1l-2-3.5l-2 2Z", Some(dark)),
        ],
    );
}

/// Name the commands; the gallery tiles show pictures (names are tooltips).
pub(super) fn install_strings(s: &mut crate::strings::Strings) {
    for (id, key) in LABELS {
        let v = s.get(&format!("slideNative.{key}")).to_owned();
        s.set(&format!("commands.{id}"), &v);
    }
    let accent = s.get("slideNative.tableAccent").to_owned();
    for (i, id) in TABLE_TILES.iter().enumerate() {
        s.set(
            &format!("commands.tip.{id}"),
            &accent.replace("{{n}}", &(i + 1).to_string()),
        );
        s.set(&format!("commands.{id}"), "");
    }
    for id in ["sl.tblMore", "sl.stylesMore"] {
        s.set(&format!("commands.{id}"), "");
    }
    // Icons only at this width (the name is the tooltip), as the reference.
    for id in [
        "sl.align",
        "sl.group",
        "sl.rotate",
        "sl.textFill",
        "sl.textOutline",
        "sl.textEffects",
        "sl.tblBorders",
        "sl.tblEffects",
        "sl.penStyle",
        "sl.penWeight",
    ] {
        let v = s.get(&format!("commands.{id}")).to_owned();
        if !v.is_empty() {
            s.set(&format!("commands.tip.{id}"), &v);
        }
    }
    // The pen boxes show the line itself (the name is the tooltip).
    for id in ["sl.penStyle", "sl.penWeight"] {
        s.set(&format!("commands.{id}"), "");
    }
}

/// Text transforms offered by Text Effects (§20.1.10.76 ST_TextShapeType).
const WARPS: [(&str, &str); 6] = [
    ("warpNone", "textNoShape"),
    ("warpArchUp", "textArchUp"),
    ("warpArchDown", "textArchDown"),
    ("warpWave", "textWave1"),
    ("warpInflate", "textInflate"),
    ("warpTriangle", "textTriangle"),
];

impl SlideWindow {
    /// Table style option flags and the built-in style index of the selected table.
    pub(super) fn table_state(&self) -> Option<(bo_pptx::table_style::Flags, Option<usize>)> {
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
        let path = bo_engine::slide_edit::shape_props::path_of(&slide.data.shapes, id)?;
        let mut nodes = &slide.data.shapes;
        let mut node = None;
        for (k, i) in path.iter().enumerate() {
            let n = nodes.get(*i)?;
            if k + 1 == path.len() {
                node = Some(n);
            } else if let ShapeNode::Group(g) = n {
                nodes = &g.children;
            }
        }
        match node? {
            ShapeNode::Frame(f) => match &f.content {
                GraphicContent::Table(t) => {
                    let (flags, style) = bo_pptx::table_style::table_props(t.props.as_ref());
                    let n = style.and_then(|s| {
                        bo_pptx::table_style::BUILTIN_IDS
                            .iter()
                            .position(|b| *b == s)
                    });
                    Some((flags, n))
                }
                GraphicContent::Other(_) => None,
            },
            _ => None,
        }
    }

    /// The shapes of the current slide for the Selection Pane, front to back (group members
    /// after their group, indented).
    pub(super) fn selection_rows(&self) -> Vec<SelRow> {
        fn walk(nodes: &[ShapeNode], depth: usize, sel: &[u32], out: &mut Vec<SelRow>) {
            for n in nodes.iter().rev() {
                let Some(nv) = n.nv() else { continue };
                out.push(SelRow {
                    id: nv.id,
                    name: nv.name.clone(),
                    hidden: nv.hidden == Some(true),
                    depth,
                    selected: sel.contains(&nv.id),
                });
                if let ShapeNode::Group(g) = n {
                    walk(&g.children, depth + 1, sel, out);
                }
            }
        }
        let sel: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
        let mut out = Vec::new();
        if let Some(s) = self
            .view
            .engine()
            .presentation()
            .and_then(|p| p.slides.get(self.view.current()))
        {
            walk(&s.data.shapes, 0, &sel, &mut out);
        }
        out
    }

    /// Alternative text and "decorative" of the only selected shape.
    pub(super) fn selected_alt(&self) -> Option<(u32, String, bool)> {
        let [one] = self.view.selection() else {
            return None;
        };
        let slide = self
            .view
            .engine()
            .presentation()?
            .slides
            .get(self.view.current())?;
        let path = bo_engine::slide_edit::shape_props::path_of(&slide.data.shapes, one.id)?;
        let mut nodes = &slide.data.shapes;
        for (k, i) in path.iter().enumerate() {
            let n = nodes.get(*i)?;
            if k + 1 == path.len() {
                let nv = n.nv()?;
                return Some((
                    nv.id,
                    nv.descr.clone().unwrap_or_default(),
                    is_decorative(nv),
                ));
            }
            if let ShapeNode::Group(g) = n {
                nodes = &g.children;
            }
        }
        None
    }

    /// Show the Selection Pane or the Alt Text pane (another pane on the right closes).
    pub(super) fn open_side(&mut self, kind: SideKind) -> bool {
        self.commit_alt();
        self.side.toggle(kind);
        if self.side.shown() {
            self.comments.shown = false;
            self.anim_pane.shown = false;
            self.bg_pane.shown = false;
        }
        self.relayout_pending = true;
        true
    }

    /// Store the alternative text being typed.
    fn commit_alt(&mut self) -> bool {
        let Some(text) = self.side.alt_edit.take() else {
            return false;
        };
        let Some((id, old, _)) = self.selected_alt() else {
            return false;
        };
        if text.trim() == old {
            return false;
        }
        self.view.run_props(Props::SetShapeProps {
            id,
            name: None,
            descr: Some(text),
            hidden: None,
            decorative: None,
        })
    }

    fn set_hidden(&mut self, id: u32, hidden: bool) -> bool {
        self.view.run_props(Props::SetShapeProps {
            id,
            name: None,
            descr: None,
            hidden: Some(hidden),
            decorative: None,
        })
    }

    /// Show or hide every shape of the slide (one undo step).
    fn show_all(&mut self, hidden: bool) -> bool {
        let ids: Vec<u32> = self
            .selection_rows()
            .into_iter()
            .filter(|r| r.hidden != hidden)
            .map(|r| r.id)
            .collect();
        if ids.is_empty() {
            return false;
        }
        let since = self.history_len();
        let mut done = false;
        for id in &ids {
            done |= self.set_hidden(*id, hidden);
        }
        // One undo step for the whole change.
        if let Some(since) = since.filter(|_| ids.len() > 1) {
            self.view.run(SlideCommand::MergeHistory { since });
        }
        done
    }

    /// Length of the undo history.
    fn history_len(&mut self) -> Option<usize> {
        self.view.run(SlideCommand::HistoryLen);
        self.view.state().and_then(|s| s.history)
    }

    /// Input for the Selection / Alt Text panes.
    pub(super) fn side_input(&mut self, input: &Input) -> Option<bool> {
        if !self.side.shown() {
            return None;
        }
        match input {
            Input::Press { x, y, count, .. } if self.side.contains(*x, *y) => {
                self.area_press = false;
                let hit = self.side.press(*x, *y);
                if hit != Some(SideHit::AltBox) {
                    self.commit_alt();
                }
                Some(match hit {
                    Some(SideHit::Close) => {
                        self.side.close();
                        self.relayout_pending = true;
                        true
                    }
                    Some(SideHit::ShowAll) => self.show_all(false),
                    Some(SideHit::HideAll) => self.show_all(true),
                    Some(SideHit::Forward | SideHit::Backward) => {
                        let forward = hit == Some(SideHit::Forward);
                        let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
                        match ids[..] {
                            [id] => self.view.run_props(Props::ShapeOrder { id, forward }),
                            [] => false,
                            _ => self.view.action(if forward {
                                "arrange.forward"
                            } else {
                                "arrange.backward"
                            }),
                        }
                    }
                    Some(SideHit::Select(id)) => {
                        if *count >= 2 {
                            let name = self
                                .selection_rows()
                                .into_iter()
                                .find(|r| r.id == id)
                                .map(|r| r.name)
                                .unwrap_or_default();
                            self.side.start_rename(id, &name);
                            return Some(true);
                        }
                        self.side.rename = None;
                        self.view.run(SlideCommand::Select { ids: vec![id] })
                    }
                    Some(SideHit::Toggle(id)) => {
                        let hidden = self.selection_rows().iter().any(|r| r.id == id && r.hidden);
                        self.set_hidden(id, !hidden)
                    }
                    Some(SideHit::AltBox) => {
                        if self.side.alt_edit.is_none() {
                            self.side.alt_edit = self.selected_alt().map(|(_, t, _)| t);
                        }
                        true
                    }
                    Some(
                        h @ (SideHit::IgnoreOnce
                        | SideHit::IgnoreAll
                        | SideHit::AddWord
                        | SideHit::Change
                        | SideHit::ChangeAll
                        | SideHit::Suggestion(_)),
                    ) => self.spelling_hit(&h),
                    Some(
                        h @ (SideHit::ThesBack | SideHit::ThesLookup(_) | SideHit::ThesToggle(_)),
                    ) => self.thesaurus_hit(&h, *count),
                    Some(h @ (SideHit::Goto(..) | SideHit::PaneMenu | SideHit::Speak)) => {
                        self.review_side_hit(&h).unwrap_or(false)
                    }
                    Some(SideHit::Decorative) => match self.selected_alt() {
                        Some((id, _, on)) => self.view.run_props(Props::SetShapeProps {
                            id,
                            name: None,
                            descr: None,
                            hidden: None,
                            decorative: Some(!on),
                        }),
                        None => false,
                    },
                    None => false,
                })
            }
            Input::Press { .. } => {
                // A press elsewhere ends editing in the pane; the press goes on to the
                // rest of the window.
                self.side.rename = None;
                self.commit_alt();
                None
            }
            Input::Text(t) => self.side.text(t).map(|_| true),
            Input::Key(k, m) if !m.ctrl => {
                let edit = self.side.key(*k)?;
                Some(match edit {
                    SideEdit::Changed | SideEdit::Cancel => true,
                    SideEdit::Rename(id, name) => self.view.run_props(Props::SetShapeProps {
                        id,
                        name: Some(name),
                        descr: None,
                        hidden: None,
                        decorative: None,
                    }),
                    SideEdit::AltText(text) => {
                        self.side.alt_edit = Some(text);
                        self.commit_alt()
                    }
                })
            }
            _ => None,
        }
    }

    /// Lay the shown pane out for the current slide.
    pub(super) fn record_side(&mut self, text: &dyn TextMeasure) {
        match self.side.kind {
            SideKind::None => {}
            SideKind::Selection => {
                let rows = self.selection_rows();
                let labels = [
                    self.t("slideNative.selection", "Selection"),
                    self.t("slideNative.showAll", "Show All"),
                    self.t("slideNative.hideAll", "Hide All"),
                ];
                self.side
                    .record_selection(text, &rows, &labels, self.theme.text);
            }
            SideKind::Accessibility => {
                let rows = self.a11y_rows();
                let labels = [
                    self.t("slideNative.accessibility", "Accessibility"),
                    self.t("slideNative.a11yNone", "No accessibility issues found."),
                    self.t("slideNative.a11yErrors", "Errors"),
                ];
                self.side.record_a11y(text, &rows, &labels, self.theme.text);
            }
            SideKind::Spelling => {
                let shown = self.spell_shown();
                let labels = [
                    self.t("slideNative.spelling", "Spelling"),
                    spelling::access(&self.t("slideNative.ignoreOnce", "Ignore Once"), 'i'),
                    spelling::access(&self.t("slideNative.ignoreAll", "Ignore All"), 'g'),
                    spelling::access(&self.t("slideNative.addWord", "Add"), 'a'),
                    spelling::access(&self.t("slideNative.changeWord", "Change"), 'c'),
                    spelling::access(&self.t("slideNative.changeAll", "Change All"), 'l'),
                    self.t("slideNative.spellDone", "Spell check complete."),
                    self.strings.get("spell.noSuggestions").to_owned(),
                ];
                self.side
                    .record_spelling(text, &shown, &labels, self.theme.text);
            }
            SideKind::Thesaurus => {
                let meanings = self.thesaurus_meanings();
                let labels = [
                    self.t("slideNative.thesaurus", "Thesaurus"),
                    self.t("slideNative.thesNoResults", "No results found."),
                    self.t("slideNative.thesLang", "English (United States)"),
                    self.t("slideNative.thesAntonym", "Antonym"),
                ];
                self.side
                    .record_thesaurus(text, &meanings, &labels, self.theme.text);
            }
            SideKind::AltText => {
                let alt = self.selected_alt();
                let labels = [
                    self.t("slideNative.altText", "Alt Text"),
                    self.t("slideNative.altPrompt", ""),
                    self.t("slideNative.altHint", ""),
                    self.t("slideNative.decorative", "Mark as decorative"),
                ];
                self.side.record_alt(
                    text,
                    alt.as_ref().map(|(_, t, d)| (t.as_str(), *d)),
                    &labels,
                    self.theme.text,
                );
            }
        }
    }

    /// Give the selection one of the quick text styles (fill and outline in theme colours).
    fn apply_text_style(&mut self, i: usize) -> bool {
        use bo_engine::slide_edit::effects::EffectOp;
        use bo_engine::slide_edit::text::TextFormat;
        let Some((_, fill, outline)) = TEXT_STYLES.get(i).copied() else {
            return false;
        };
        let colors = self.theme_colors();
        let hex = |i: usize| colors.get(i).map(|c| pk::hex(*c));
        let mut done = false;
        if let Some(c) = fill.and_then(hex) {
            done |= self.view.format_text(TextFormat::Color { hex: Some(c) });
        }
        done |= self.view.run(SlideCommand::ShapeEffects {
            op: EffectOp::TextOutline {
                color: outline.and_then(hex),
                width: outline.map(|_| 1.0),
            },
        });
        done
    }

    /// Run one of these tabs' commands; `None` when `id` is not one of them.
    pub(super) fn format_action(&mut self, id: &str) -> Option<bool> {
        let flag = TABLE_FLAGS.iter().find(|(c, _)| *c == id).map(|(_, f)| *f);
        if let Some(f) = flag {
            let (flags, _) = self.table_state()?;
            let cur = match f {
                "firstRow" => flags.first_row,
                "firstCol" => flags.first_col,
                "lastRow" => flags.last_row,
                "lastCol" => flags.last_col,
                "bandRow" => flags.band_row,
                _ => flags.band_col,
            };
            let set = |name: &str| (name == f).then_some(!cur);
            return Some(self.view.run_props(Props::SetTableFlags {
                first_row: set("firstRow"),
                last_row: set("lastRow"),
                first_col: set("firstCol"),
                last_col: set("lastCol"),
                band_row: set("bandRow"),
                band_col: set("bandCol"),
            }));
        }
        if let Some(i) = TABLE_TILES.iter().position(|t| *t == id) {
            return Some(self.view.run_props(Props::SetTableStyle { style: i }));
        }
        let list = |w: &mut Self, items: Vec<(String, String)>, anchor: &'static str| {
            w.open_list(items, anchor);
            true
        };
        Some(match id {
            "sl.tblMore" => {
                let accent = self.t("slideNative.tableAccent", "Accent {{n}}");
                let items = (0..bo_pptx::table_style::BUILTIN_IDS.len())
                    .map(|i| {
                        (
                            accent.replace("{{n}}", &(i + 1).to_string()),
                            format!("side:tbl{i}"),
                        )
                    })
                    .collect();
                list(self, items, "sl.tblMore")
            }
            "sl.stylesMore" => {
                self.open_picker(PickKind::Styles, "sl.stylesMore");
                true
            }
            "sl.textQuick" => {
                let items = TEXT_STYLES
                    .iter()
                    .enumerate()
                    .map(|(i, (k, ..))| (self.label(k), format!("side:text{i}")))
                    .collect();
                list(self, items, "sl.textQuick")
            }
            "sl.textFill" => {
                self.open_picker(PickKind::Fill, "sl.textFill");
                true
            }
            "sl.textOutline" => {
                self.open_picker(PickKind::Outline, "sl.textOutline");
                true
            }
            "sl.textEffects" => {
                let items = WARPS
                    .iter()
                    .map(|(k, w)| (self.label(k), format!("side:warp{w}")))
                    .collect();
                list(self, items, "sl.textEffects")
            }
            "sl.tblBorders" => {
                let items = BORDERS
                    .iter()
                    .map(|(k, v)| (self.label(k), format!("side:bd{v}")))
                    .collect();
                list(self, items, "sl.tblBorders")
            }
            "sl.tblEffects" => {
                let items = [
                    ("reflection", "fx:reflection"),
                    ("softEdges", "fx:soft"),
                    ("noEffects", "fx:none"),
                ]
                .iter()
                .map(|(k, c)| (self.label(k), (*c).to_owned()))
                .collect();
                list(self, items, "sl.tblEffects")
            }
            "sl.penStyle" => {
                let items = DASHES
                    .iter()
                    .map(|(k, v)| (self.label(k), format!("side:dash{v}")))
                    .collect();
                list(self, items, "sl.penStyle")
            }
            "sl.penWeight" => {
                let items = WEIGHTS
                    .iter()
                    .map(|w| (format!("{w} pt"), format!("side:pw{w}")))
                    .collect();
                list(self, items, "sl.penWeight")
            }
            "sl.penColor" => {
                self.open_picker(PickKind::Outline, "sl.penColor");
                true
            }
            "sl.picQuick" => {
                let items = PIC_STYLES
                    .iter()
                    .map(|(k, v)| (self.label(k), format!("side:pq{v}")))
                    .collect();
                list(self, items, "sl.picQuick")
            }
            "sl.altText" => self.open_side(SideKind::AltText),
            "sl.selPane" => self.open_side(SideKind::Selection),
            _ => return None,
        })
    }

    /// Menu choices of these tabs (`side:…`).
    pub(super) fn choose_format(&mut self, v: &str) -> bool {
        use bo_engine::slide_edit::effects::EffectOp;
        if let Some(i) = v.strip_prefix("tbl").and_then(|n| n.parse().ok()) {
            return self.view.run_props(Props::SetTableStyle { style: i });
        }
        if let Some(i) = v.strip_prefix("text").and_then(|n| n.parse().ok()) {
            return self.apply_text_style(i);
        }
        if let Some(w) = v.strip_prefix("warp") {
            return self.view.run(SlideCommand::ShapeEffects {
                op: EffectOp::TextWarp {
                    preset: (w != "textNoShape").then(|| w.to_owned()),
                },
            });
        }
        if let Some(which) = v.strip_prefix("bd") {
            return self.view.run_props(Props::SetTableBorders {
                which: which.to_owned(),
                color: Some(self.pen.color.clone()),
                width: self.pen.width,
                dash: Some(self.pen.dash.to_owned()),
            });
        }
        if let Some(d) = v.strip_prefix("dash") {
            if let Some((_, d)) = DASHES.iter().find(|(_, x)| *x == d) {
                self.pen.dash = d;
            }
            return true;
        }
        if let Some(w) = v.strip_prefix("pw").and_then(|w| w.parse().ok()) {
            self.pen.width = w;
            return true;
        }
        if let Some(q) = v.strip_prefix("pq") {
            return self.picture_style(q);
        }
        match v {
            "selPane" => self.open_side(SideKind::Selection),
            _ => false,
        }
    }

    /// A picture quick style: a frame and / or an effect.
    fn picture_style(&mut self, q: &str) -> bool {
        use bo_engine::slide_edit::effects::EffectOp;
        let line = |w: &mut Self, c: &str, width: f64| {
            w.view.run(SlideCommand::SetLine {
                color: Some(c.to_owned()),
                width: Some(width),
            })
        };
        match q {
            "white" => line(self, "FFFFFF", 6.0),
            "black" => line(self, "000000", 4.5),
            "soft" => self.view.run(SlideCommand::ShapeEffects {
                op: EffectOp::SoftEdge { radius: Some(8.0) },
            }),
            "reflect" => self.view.run(SlideCommand::ShapeEffects {
                op: EffectOp::Reflection {
                    size: Some(0.35),
                    distance: 0.0,
                },
            }),
            "perspective" => self.view.run(SlideCommand::ShapeEffects {
                op: EffectOp::Rotation3d {
                    preset: Some("perspectiveRight".into()),
                },
            }),
            _ => false,
        }
    }

    /// A colour picked for the text fill or outline (`None` = no colour).
    pub(super) fn pick_text_color(&mut self, anchor: &str, color: Option<String>) -> Option<bool> {
        use bo_engine::slide_edit::effects::EffectOp;
        use bo_engine::slide_edit::text::TextFormat;
        Some(match anchor {
            "sl.penColor" => {
                if let Some(c) = color {
                    self.pen.color = c;
                }
                true
            }
            "sl.textFill" => self.view.format_text(TextFormat::Color { hex: color }),
            "sl.textOutline" => self.view.run(SlideCommand::ShapeEffects {
                op: EffectOp::TextOutline {
                    width: color.as_ref().map(|_| 1.0),
                    color,
                },
            }),
            _ => return None,
        })
    }

    /// Table Design: the option check boxes and the style gallery (a white box with tables
    /// drawn in each style's colours, the table's own style framed, the "more" column).
    pub(super) fn paint_table_design(&self, p: &mut dyn Painter) {
        // Shape Format's style gallery "more" column, framed like the gallery box.
        if let Some(r) = self.ribbon.rect_of(RibbonHit::Command("sl.stylesMore")) {
            frame_more(p, [r[0], r[1] + self.px(1.6), r[2], self.px(73.6)]);
        }
        let Some((flags, current)) = self.table_state() else {
            return;
        };
        let on = |f: &str| match f {
            "firstRow" => flags.first_row,
            "firstCol" => flags.first_col,
            "lastRow" => flags.last_row,
            "lastCol" => flags.last_col,
            "bandRow" => flags.band_row,
            _ => flags.band_col,
        };
        let grey = |v: u8| Rgba(v, v, v, 255);
        for (id, f) in TABLE_FLAGS {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let b = self.px(14.4);
            let (x, y) = (r[0] + self.px(4.0), (r[1] + (r[3] - b) / 2.0).round());
            p.fill_rect(r[0], r[1], self.px(24.0), r[3], grey(0xf3));
            crate::spin_box::paint_check(p, [x, y, b, b], on(f), true, self.theme.text);
        }
        let rect = |id: &'static str| self.ribbon.rect_of(RibbonHit::Command(id));
        let (Some(first), Some(more)) = (rect(TABLE_TILES[0]), rect("sl.tblMore")) else {
            return;
        };
        // Reference at 125 %: box y 106-197, 13 px left of the first tile.
        let top = first[1] + self.px(1.6);
        let h = self.px(73.6);
        let left = first[0] - self.px(3.2);
        let right = more[0];
        p.fill_rect(left, top, right - left + 1.0, h, grey(0xd2));
        p.fill_rect(
            left + 1.0,
            top + 1.0,
            right - left - 1.0,
            h - 2.0,
            grey(0xff),
        );
        frame_more(p, [more[0], top, more[2], h]);
        let accents = self.theme_colors();
        let hovered = self.ribbon.hovered();
        for (i, id) in TABLE_TILES.iter().enumerate() {
            let Some(r) = rect(id) else { continue };
            let band = [
                r[0] + self.px(1.6),
                top + self.px(3.2),
                r[2] - self.px(3.2),
                h - self.px(6.4),
            ];
            if current.unwrap_or(0) == i {
                p.fill_rect(band[0], band[1], band[2], band[3], grey(0x88));
                p.fill_rect(
                    band[0] + 1.0,
                    band[1] + 1.0,
                    band[2] - 2.0,
                    band[3] - 2.0,
                    grey(0xd2),
                );
                // A white mat inside the grey frame, as the reference's chosen tile.
                let m = self.px(4.0);
                p.fill_rect(
                    band[0] + m,
                    band[1] + m,
                    band[2] - 2.0 * m,
                    band[3] - 2.0 * m,
                    grey(0xff),
                );
            } else if hovered == Some(RibbonHit::Command(id)) {
                p.fill_rect(band[0], band[1], band[2], band[3], grey(0xe6));
            }
            let accent = accents
                .get(i)
                .copied()
                .unwrap_or(Rgba(0x44, 0x72, 0xc4, 255));
            let tw = self.px(68.8);
            let th = self.px(51.2);
            let x = (r[0] + (r[2] - tw) / 2.0).round();
            let y = (top + (h - th) / 2.0).round();
            paint_table_tile(p, [x, y, tw, th], accent, flags, self.scale);
        }
        // The "more" mark is the ribbon's icon; nothing else to draw here.
        let _ = more;
    }
}

/// The frame of a gallery's "more" column (edges only: the ribbon draws its mark).
pub(super) fn frame_more(p: &mut dyn Painter, r: [f32; 4]) {
    let c = Rgba(0xd2, 0xd2, 0xd2, 255);
    let [x, y, w, h] = r;
    p.fill_rect(x, y, w, 1.0, c);
    p.fill_rect(x, y + h - 1.0, w, 1.0, c);
    p.fill_rect(x, y, 1.0, h, c);
    p.fill_rect(x + w - 1.0, y, 1.0, h, c);
}

/// White mixed with `c` at `t` (DrawingML `tint`: `t` of the colour).
fn tint(c: Rgba, t: f32) -> Rgba {
    let l = |v: u8| (255.0 - (255.0 - f32::from(v)) * t).round() as u8;
    Rgba(l(c.0), l(c.1), l(c.2), 255)
}

/// A small table in a built-in style (own drawing in the reference's manner): five rows of
/// five cells inside a thin accent border, the header row in the accent with white text
/// dashes, banded rows lightly tinted, cells parted by light rules, a dark dash of "text"
/// filling most of each cell.
fn paint_table_tile(
    p: &mut dyn Painter,
    r: [f32; 4],
    accent: Rgba,
    f: bo_pptx::table_style::Flags,
    scale: f32,
) {
    let [x, y, w, h] = r;
    let rows: usize = 5;
    let cols: usize = 5;
    let rh = (h / rows as f32).floor();
    let cw = (w / cols as f32).floor();
    let (tw, th) = (cw * cols as f32, rh * rows as f32);
    let band = tint(accent, 0.15);
    let rule = tint(accent, 0.45);
    let ink = Rgba(0x10, 0x10, 0x10, 255);
    let white = Rgba(255, 255, 255, 255);
    let k = (scale * 0.8).round().max(1.0);
    p.fill_rect(x, y, tw, th, white);
    for rr in 0..rows {
        let header = rr == 0 && f.first_row;
        let total = rr == rows - 1 && f.last_row;
        let body = rr.saturating_sub(usize::from(f.first_row));
        let ry = y + rr as f32 * rh;
        if header || total {
            p.fill_rect(x, ry, tw, rh, accent);
        } else if f.band_row && body % 2 == 0 {
            p.fill_rect(x, ry, tw, rh, band);
        }
        for cc in 0..cols {
            let cx = x + cc as f32 * cw;
            let emph = (cc == 0 && f.first_col) || (cc == cols - 1 && f.last_col);
            if emph && !(header || total) {
                p.fill_rect(cx, ry, cw, rh, tint(accent, 0.35));
            }
            let dash = if header || total { white } else { ink };
            p.fill_rect(
                cx + k + 1.0,
                (ry + rh / 2.0).round(),
                cw - 2.0 * k - 2.0,
                (2.0 * k).min(rh / 3.0).round().max(1.0),
                dash,
            );
            if cc > 0 {
                let c = if header || total { white } else { rule };
                p.fill_rect(cx, ry, 1.0, rh, c);
            }
        }
        if rr > 0 {
            p.fill_rect(x, ry, tw, 1.0, rule);
        }
    }
    // Thin border in the accent's tint.
    for (bx, by, bw, bh) in [
        (x, y, tw, 1.0),
        (x, y + th - 1.0, tw, 1.0),
        (x, y, 1.0, th),
        (x + tw - 1.0, y, 1.0, th),
    ] {
        p.fill_rect(bx, by, bw, bh, rule);
    }
}
