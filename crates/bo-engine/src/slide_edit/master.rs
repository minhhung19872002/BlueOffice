//! Slide masters, layouts, themes and slide size (SLIDE-MASTER-001).
//!
//! * **Master view.** While it is open the deck's real slides are set aside and
//!   the slide list holds one *view slide* per master (§19.3.1.42 sldMaster) and
//!   per layout (§19.3.1.39 sldLayout), in master order. Every editing command of
//!   the normal view (move, resize, text, fill, insert…) therefore works on them
//!   unchanged; after each command the edited view slides are copied back into
//!   their master or layout by an extra [`SlideOp::Pres`] in the same
//!   transaction, so undo stays exact and layouts follow their master live.
//!   Closing the view folds the whole session into one undoable step.
//! * Layout and master structure: insert / duplicate / rename / delete layouts,
//!   preserve a master, insert placeholders (§19.3.1.36 ph), title and footer
//!   placeholders, hide background graphics (§19.3.1.39 `showMasterSp`), add a
//!   master.
//! * Design: backgrounds (§19.3.1.1 bg), master text styles (§19.3.1.52
//!   txStyles), themes and their colour / font variants (§20.1.6.9 theme,
//!   §20.1.6.2 clrScheme, §20.1.4.1.18 fontScheme), applying a layout to slides,
//!   slide size (§19.2.1.39 sldSz) with "scale" or "fit" of the content.

use super::build;
use super::geom::{Rect, EMU_PT};
use super::ops::{PresFields, SlideOp, Tx};
use super::{next_shape_id, refuse, EditError, Outcome, SizeFit, SlideEditor, SlideSel, Txn, R};
use crate::{Engine, EngineError};
use bo_model::slide::{
    Background, Color, Fill, ListStyle, ParaProps, Presentation, RunProps, ShapeNode, Slide,
    SlideLayout, SlideMaster, SlideSize, TextFont, Theme,
};
use bo_pptx::inherit::{chain, effective_xfrm, find_placeholder, Level};
use bo_render::{Command, PageList};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// View state
// ---------------------------------------------------------------------------

/// What the normal view had while the master view is open.
pub(crate) struct MasterView {
    /// The deck's slides, set aside.
    slides: Vec<Slide>,
    /// Normal-view selection and history.
    sel: SlideSel,
    undo: Vec<Txn>,
    redo: Vec<Txn>,
    /// Masters, layouts and themes when the view opened.
    entry: PresFields,
    /// Part names (lower case) of the layouts when the view opened: they identify
    /// the layouts of the set-aside slides across inserts and deletions.
    entry_names: Vec<String>,
    /// The deck had unsaved changes when the view opened (cleared by a save).
    modified: bool,
    /// View history state last saved (0: none).
    saved: u64,
}

impl MasterView {
    /// Layout part names used by the deck's slides.
    fn used(&self) -> Vec<String> {
        self.slides
            .iter()
            .filter_map(|s| self.entry_names.get(s.layout).cloned())
            .collect()
    }
    /// Unsaved changes (the deck's or this session's), given the view's history state.
    pub(crate) fn modified(&self, state: u64) -> bool {
        self.modified || state != self.saved
    }
}

/// What a view slide stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A slide master (index into `Presentation::masters`).
    Master(usize),
    /// A slide layout (index into `Presentation::layouts`).
    Layout(usize),
}

impl Target {
    fn level(self) -> Level {
        match self {
            Target::Master(m) => Level::Master(m),
            Target::Layout(l) => Level::Layout(l),
        }
    }
}

/// Number of real layouts while the view is open (one helper layout per master
/// follows them).
fn real_count(pres: &Presentation) -> usize {
    pres.layouts.len().saturating_sub(pres.masters.len())
}

/// View order: each master followed by its layouts (§19.3.1.40 sldLayoutIdLst
/// order, then layouts that point at the master without being listed).
fn targets_of(layouts: &[SlideLayout], masters: &[SlideMaster]) -> Vec<Target> {
    let mut out = Vec::new();
    let mut seen = vec![false; layouts.len()];
    for (mi, m) in masters.iter().enumerate() {
        out.push(Target::Master(mi));
        for r in &m.layouts {
            if layouts.get(r.layout).is_some_and(|l| l.master == mi) && !seen[r.layout] {
                seen[r.layout] = true;
                out.push(Target::Layout(r.layout));
            }
        }
        for (li, l) in layouts.iter().enumerate() {
            if l.master == mi && !seen[li] {
                seen[li] = true;
                out.push(Target::Layout(li));
            }
        }
    }
    out
}

/// Targets of the open view.
fn view_targets(pres: &Presentation) -> Vec<Target> {
    let n = real_count(pres);
    targets_of(&pres.layouts[..n], &pres.masters)
}

/// Helper layouts (one empty layout per master, so a master's view slide
/// inherits from its master only) and the view slides.
fn view_slides(layouts: &[SlideLayout], masters: &[SlideMaster]) -> (Vec<SlideLayout>, Vec<Slide>) {
    let n = layouts.len();
    let helpers = (0..masters.len())
        .map(|m| SlideLayout {
            master: m,
            show_master_sp: Some(false),
            ..SlideLayout::default()
        })
        .collect();
    let slides = targets_of(layouts, masters)
        .into_iter()
        .map(|t| match t {
            Target::Master(m) => Slide {
                info: masters[m].info.clone(),
                data: masters[m].data.clone(),
                layout: n + m,
                ..Slide::default()
            },
            Target::Layout(l) => Slide {
                info: layouts[l].info.clone(),
                data: layouts[l].data.clone(),
                layout: l,
                clr_map_ovr: layouts[l].clr_map_ovr.clone(),
                ..Slide::default()
            },
        })
        .collect();
    (helpers, slides)
}

/// The op copying edited view slides back into their masters / layouts.
fn sync_op(pres: &Presentation) -> Option<SlideOp> {
    let targets = view_targets(pres);
    if targets.len() != pres.slides.len() {
        return None;
    }
    let differs = |s: &Slide, t: Target| match t {
        Target::Master(m) => pres
            .masters
            .get(m)
            .is_some_and(|x| x.data != s.data || x.info.rels != s.info.rels),
        Target::Layout(l) => pres
            .layouts
            .get(l)
            .is_some_and(|x| x.data != s.data || x.info.rels != s.info.rels),
    };
    if !pres
        .slides
        .iter()
        .zip(&targets)
        .any(|(s, t)| differs(s, *t))
    {
        return None;
    }
    let before = PresFields::of(pres);
    let mut after = before.clone();
    for (s, t) in pres.slides.iter().zip(targets) {
        let (data, info) = match t {
            Target::Master(m) => {
                let x = &mut after.masters[m];
                (&mut x.data, &mut x.info)
            }
            Target::Layout(l) => {
                let x = &mut after.layouts[l];
                (&mut x.data, &mut x.info)
            }
        };
        data.clone_from(&s.data);
        info.rels.clone_from(&s.info.rels);
    }
    Some(SlideOp::Pres {
        before: Box::new(before),
        after: Box::new(after),
    })
}

/// Slides re-pointed at the layouts' new indices (by part name).
fn remap(slides: &[Slide], entry_names: &[String], layouts: &[SlideLayout]) -> Vec<Slide> {
    let names: Vec<String> = layouts
        .iter()
        .map(|l| l.info.part_name.to_ascii_lowercase())
        .collect();
    slides
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.layout = entry_names
                .get(s.layout)
                .and_then(|n| names.iter().position(|x| x == n))
                .unwrap_or(0);
            s
        })
        .collect()
}

impl SlideEditor {
    /// A save while the master view is open.
    pub(super) fn master_saved(&mut self) {
        let state = self.state_id();
        if let Some(mv) = &mut self.master {
            mv.modified = false;
            mv.saved = state;
        }
    }

    /// Undo-history position (detects whether a command changed something).
    pub(super) fn history_mark(&self) -> (usize, usize, usize) {
        (
            self.undo.len(),
            self.undo.last().map_or(0, |t| t.ops.len()),
            self.redo.len(),
        )
    }

    /// The master view is open.
    pub(crate) fn in_master_view(&self) -> bool {
        self.master.is_some()
    }

    /// After a command in the master view: copy the edited view slides into
    /// their masters / layouts (same transaction) and re-render every view slide.
    pub(super) fn master_sync(
        &mut self,
        pres: &mut Presentation,
        out: &mut Outcome,
        mark: (usize, usize, usize),
        history: bool,
    ) {
        if self.master.is_none() || self.history_mark() == mark {
            return;
        }
        out.all = true;
        if history {
            return;
        }
        if let Some(op) = sync_op(pres) {
            if op.apply(pres).is_ok() {
                if let Some(last) = self.undo.last_mut() {
                    last.ops.push(op);
                }
            }
        }
    }
}

fn enter(ed: &mut SlideEditor, pres: &mut Presentation, out: &mut Outcome) {
    if ed.master.is_some() {
        return;
    }
    if ed.read_only {
        out.error = Some(crate::security::READ_ONLY.into());
        return;
    }
    if pres.masters.is_empty() {
        out.error = Some("noMaster".into());
        return;
    }
    // Part names identify layouts while the view is open.
    for i in 0..pres.layouts.len() {
        if pres.layouts[i].info.part_name.is_empty() {
            let n = build::unique_part(pres, "/ppt/slideLayouts/slideLayout", "xml");
            pres.layouts[i].info.part_name = n;
        }
    }
    let entry = PresFields::of(pres);
    let entry_names = pres
        .layouts
        .iter()
        .map(|l| l.info.part_name.to_ascii_lowercase())
        .collect();
    let current = pres.slides.get(ed.sel.slide).map(|s| s.layout);
    let slides = std::mem::take(&mut pres.slides);
    let targets = targets_of(&pres.layouts, &pres.masters);
    let (helpers, view) = view_slides(&pres.layouts, &pres.masters);
    pres.layouts.extend(helpers);
    pres.slides = view;
    let slide = current
        .and_then(|l| targets.iter().position(|t| *t == Target::Layout(l)))
        .unwrap_or(0);
    let modified = ed.state_id() != ed.saved;
    let sel = std::mem::replace(&mut ed.sel, SlideSel { slide, ids: vec![] });
    ed.master = Some(MasterView {
        slides,
        sel,
        undo: std::mem::take(&mut ed.undo),
        redo: std::mem::take(&mut ed.redo),
        entry,
        entry_names,
        modified,
        saved: 0,
    });
    out.all = true;
}

fn exit(ed: &mut SlideEditor, pres: &mut Presentation, out: &mut Outcome) {
    let Some(mv) = ed.master.take() else { return };
    // Saved while the view was open, in its final state: the deck stays saved.
    let saved_now = !mv.modified(ed.state_id());
    let n = real_count(pres);
    pres.layouts.truncate(n);
    let slides = remap(&mv.slides, &mv.entry_names, &pres.layouts);
    pres.slides.clone_from(&slides);
    ed.sel = mv.sel;
    ed.undo = mv.undo;
    ed.redo = mv.redo;
    let after = PresFields::of(pres);
    let mut ops = Vec::new();
    if after != mv.entry {
        ops.push(SlideOp::Pres {
            before: Box::new(mv.entry),
            after: Box::new(after),
        });
    }
    if slides != mv.slides {
        ops.push(SlideOp::Slides {
            index: 0,
            remove: mv.slides,
            insert: slides,
        });
    }
    if !ops.is_empty() {
        ed.next_txn += 1;
        ed.undo.push(Txn {
            id: ed.next_txn,
            ops,
            before: ed.sel.clone(),
            after: ed.sel.clone(),
            typing: None,
        });
        ed.redo.clear();
    }
    if saved_now {
        ed.saved = ed.state_id();
    }
    out.all = true;
}

/// The real presentation while the master view is open (for saving).
pub(crate) fn assemble(pres: &Presentation, ed: &SlideEditor) -> Option<Presentation> {
    let mv = ed.master.as_ref()?;
    let n = real_count(pres);
    let mut p = pres.clone();
    p.layouts.truncate(n);
    p.slides = remap(&mv.slides, &mv.entry_names, &p.layouts);
    Some(p)
}

/// Display list of slide `i` (a master or layout while the view is open).
pub(crate) fn render_page(
    pres: &Presentation,
    in_view: bool,
    i: usize,
    fonts: &mut bo_text::FontManager,
    images: &mut std::collections::BTreeMap<String, Vec<u8>>,
) -> bo_pptx::Result<PageList> {
    if in_view {
        if let Some(t) = view_targets(pres).get(i) {
            return bo_pptx::render_level(pres, t.level(), fonts, images);
        }
    }
    bo_pptx::render_slide(pres, i, fonts, images)
}

/// `page` scaled to `width` points (thumbnail).
pub(crate) fn scaled(page: PageList, width: f32) -> PageList {
    let k = if page.width > 0.0 {
        width / page.width
    } else {
        1.0
    };
    let mut commands = Vec::with_capacity(page.commands.len() + 4);
    commands.push(Command::PushClip {
        x: 0.0,
        y: 0.0,
        w: width,
        h: page.height * k,
    });
    commands.push(Command::PushTransform {
        m: [k, 0.0, 0.0, k, 0.0, 0.0],
    });
    commands.extend(page.commands);
    commands.push(Command::PopTransform);
    commands.push(Command::PopClip);
    PageList {
        width,
        height: page.height * k,
        commands,
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Kind of a placeholder inserted into a layout (§19.7.10 ST_PlaceholderType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PhKind {
    /// Any content (`obj`: no `type` attribute).
    Content,
    /// Text (`body`).
    Text,
    /// Picture (`pic`).
    Picture,
    /// Chart (`chart`).
    Chart,
    /// Table (`tbl`).
    Table,
    /// Diagram (`dgm`).
    Diagram,
    /// Video or audio (`media`).
    Media,
    /// Picture from the web (`clipArt`; BlueOffice's choice of type).
    OnlineImage,
}

impl PhKind {
    fn spec(self) -> (Option<&'static str>, &'static str) {
        match self {
            PhKind::Content => (None, "Content Placeholder"),
            PhKind::Text => (Some("body"), "Text Placeholder"),
            PhKind::Picture => (Some("pic"), "Picture Placeholder"),
            PhKind::Chart => (Some("chart"), "Chart Placeholder"),
            PhKind::Table => (Some("tbl"), "Table Placeholder"),
            PhKind::Diagram => (Some("dgm"), "Diagram Placeholder"),
            PhKind::Media => (Some("media"), "Media Placeholder"),
            PhKind::OnlineImage => (Some("clipArt"), "Online Image Placeholder"),
        }
    }
}

/// Which master text style (§19.3.1.52 txStyles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextStyleKind {
    /// `titleStyle`.
    Title,
    /// `bodyStyle`.
    Body,
    /// `otherStyle`.
    Other,
}

/// How content follows a slide size change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeMode {
    /// Uniform scale by the larger ratio, centred: content fills the new size
    /// (and may extend past its edges).
    Scale,
    /// Uniform scale by the smaller ratio, centred: everything stays visible.
    Fit,
}

/// Commands of the master view and the Design tab (JSON bridge, ADR-0008).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MasterCommand {
    /// Open (`on`) or close the master view.
    MasterView {
        /// Open the view.
        on: bool,
    },
    /// New layout after the current one, in the current master.
    InsertLayout {
        /// Layout name (`cSld@name`).
        name: String,
    },
    /// Copy of the current layout after it.
    DuplicateLayout {
        /// Name of the copy (default: the original's).
        #[serde(default)]
        name: Option<String>,
    },
    /// Rename the current master or layout (`cSld@name`).
    RenameMasterItem {
        /// New name.
        name: String,
    },
    /// Delete the current layout or master (refused while slides use it).
    DeleteMasterItem,
    /// Keep the current master even when no slide uses it (§19.3.1.42 `preserve`).
    PreserveMaster {
        /// Preserve.
        on: bool,
    },
    /// Insert a placeholder into the current layout.
    InsertPlaceholder {
        /// Placeholder kind.
        kind: PhKind,
        /// Box in slide points (default: centred, half the slide).
        #[serde(default)]
        rect: Option<Rect>,
    },
    /// Show or remove the title / footer placeholders of the current layout.
    LayoutElements {
        /// Title placeholder present.
        #[serde(default)]
        title: Option<bool>,
        /// Date, footer and slide number placeholders present.
        #[serde(default)]
        footers: Option<bool>,
    },
    /// Hide the master's graphics behind the current layout or slide (§19.3.1.39 /
    /// §19.3.1.38 `showMasterSp`).
    HideBackgroundGraphics {
        /// Hide them.
        hide: bool,
    },
    /// Solid background of the current slide / master / layout; `None` removes
    /// it (the level inherits again). `all`: every slide (normal view).
    SetBackground {
        /// `RRGGBB`.
        #[serde(default)]
        color: Option<String>,
        /// Apply to every slide.
        #[serde(default)]
        all: bool,
    },
    /// Change a text style of the current master.
    MasterTextStyle {
        /// Which style.
        style: TextStyleKind,
        /// 1-based level (`None`: every level).
        #[serde(default)]
        level: Option<u8>,
        /// Latin typeface.
        #[serde(default)]
        font: Option<String>,
        /// Size in points.
        #[serde(default)]
        size: Option<f64>,
        /// Bold.
        #[serde(default)]
        bold: Option<bool>,
        /// Italic.
        #[serde(default)]
        italic: Option<bool>,
        /// Colour `RRGGBB`.
        #[serde(default)]
        color: Option<String>,
    },
    /// Add a new master (with its theme and three layouts) after the last one.
    AddMaster {
        /// Master name (`cSld@name`).
        name: String,
    },
    /// Apply a BlueOffice theme (`builtin`) or the theme of a `.thmx` / `.pptx`
    /// file (`file`, base64) to the current master (`all`: every master).
    ApplyTheme {
        /// Built-in theme id.
        #[serde(default)]
        builtin: Option<String>,
        /// File bytes, base64.
        #[serde(default)]
        file: Option<String>,
        /// Every master.
        #[serde(default)]
        all: bool,
    },
    /// Replace the colour and/or font scheme of the current master's theme.
    ThemeVariant {
        /// Built-in colour scheme id.
        #[serde(default)]
        colors: Option<String>,
        /// Built-in font scheme id.
        #[serde(default)]
        fonts: Option<String>,
        /// Every master.
        #[serde(default)]
        all: bool,
    },
    /// Change the slide size (EMU) and move / scale the content.
    SetSlideSize {
        /// Width in EMU.
        cx: i64,
        /// Height in EMU.
        cy: i64,
        /// §19.7.16 ST_SlideSizeType (`screen4x3`, …); `None` for custom sizes.
        #[serde(default)]
        kind: Option<String>,
        /// Scale or fit.
        mode: SizeMode,
    },
    /// Base slides on another layout (default: the current slide).
    ApplyLayout {
        /// Layout index.
        layout: usize,
        /// Slide indices.
        #[serde(default)]
        slides: Option<Vec<usize>>,
    },
}

/// Command names of [`MasterCommand`] (routing in the JSON bridge).
pub const COMMANDS: &[&str] = &[
    "masterView",
    "insertLayout",
    "duplicateLayout",
    "renameMasterItem",
    "deleteMasterItem",
    "preserveMaster",
    "insertPlaceholder",
    "layoutElements",
    "hideBackgroundGraphics",
    "setBackground",
    "masterTextStyle",
    "addMaster",
    "applyTheme",
    "themeVariant",
    "setSlideSize",
    "applyLayout",
];

/// Facts about the view a command needs besides the deck.
struct Ctx {
    view: bool,
    used: Vec<String>,
}

/// Masters, layouts and themes edited as a whole (view structure commands).
struct Real {
    layouts: Vec<bo_model::slide::SlideLayout>,
    masters: Vec<SlideMaster>,
    themes: Vec<Theme>,
}

fn current_target(pres: &Presentation, sel: &SlideSel) -> Option<Target> {
    view_targets(pres).get(sel.slide).copied()
}

/// Rebuild masters / layouts / themes through `f` and the view slides after
/// them; `f` returns the target to select.
fn restructure(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    f: impl FnOnce(&mut Real, Option<Target>, &Presentation) -> R<Option<Target>>,
) -> R<()> {
    let pres = &*tx.pres;
    let n = real_count(pres);
    let cur = current_target(pres, sel);
    let mut real = Real {
        layouts: pres.layouts[..n].to_vec(),
        masters: pres.masters.clone(),
        themes: pres.themes.clone(),
    };
    let want = f(&mut real, cur, pres)?;
    let (helpers, view) = view_slides(&real.layouts, &real.masters);
    let targets = targets_of(&real.layouts, &real.masters);
    let before = PresFields::of(pres);
    let mut after = before.clone();
    after.layouts = real.layouts;
    after.layouts.extend(helpers);
    after.masters = real.masters;
    after.themes = real.themes;
    let old = pres.slides.clone();
    if after != before {
        tx.op(SlideOp::Pres {
            before: Box::new(before),
            after: Box::new(after),
        })?;
    }
    if view != old {
        tx.op(SlideOp::Slides {
            index: 0,
            remove: old,
            insert: view,
        })?;
    }
    let slide = want
        .and_then(|w| targets.iter().position(|t| *t == w))
        .unwrap_or_else(|| sel.slide.min(targets.len().saturating_sub(1)));
    *sel = SlideSel { slide, ids: vec![] };
    Ok(())
}

/// Change presentation-level fields (both views).
fn pres_edit(tx: &mut Tx<'_>, f: impl FnOnce(&mut PresFields) -> R<()>) -> R<()> {
    let before = PresFields::of(tx.pres);
    let mut after = before.clone();
    f(&mut after)?;
    if after != before {
        tx.op(SlideOp::Pres {
            before: Box::new(before),
            after: Box::new(after),
        })?;
    }
    Ok(())
}

/// Master of the current slide (normal view) or view slide.
fn current_master(pres: &Presentation, sel: &SlideSel, view: bool) -> Option<usize> {
    if view {
        return match current_target(pres, sel)? {
            Target::Master(m) => Some(m),
            Target::Layout(l) => pres.layouts.get(l).map(|l| l.master),
        };
    }
    match pres.slides.get(sel.slide) {
        Some(s) => pres.layouts.get(s.layout).map(|l| l.master),
        None => (!pres.masters.is_empty()).then_some(0),
    }
}

fn need_view(ctx: &Ctx) -> R<()> {
    if ctx.view {
        Ok(())
    } else {
        refuse("notMasterView")
    }
}

fn need_normal(ctx: &Ctx) -> R<()> {
    if ctx.view {
        refuse("masterView")
    } else {
        Ok(())
    }
}

/// Run a master / design command inside a transaction.
fn run(tx: &mut Tx<'_>, sel: &mut SlideSel, ctx: &Ctx, cmd: MasterCommand) -> R<()> {
    match cmd {
        MasterCommand::MasterView { .. } => Ok(()),
        MasterCommand::InsertLayout { name } => {
            need_view(ctx)?;
            insert_layout(tx, sel, &name)
        }
        MasterCommand::DuplicateLayout { name } => {
            need_view(ctx)?;
            duplicate_layout(tx, sel, name)
        }
        MasterCommand::RenameMasterItem { name } => {
            need_view(ctx)?;
            restructure(tx, sel, |real, cur, _| {
                let data = match cur {
                    Some(Target::Master(m)) => &mut real.masters[m].data,
                    Some(Target::Layout(l)) => &mut real.layouts[l].data,
                    None => return refuse("noSlide"),
                };
                data.name = Some(name);
                Ok(cur)
            })
        }
        MasterCommand::DeleteMasterItem => {
            need_view(ctx)?;
            delete_item(tx, sel, &ctx.used)
        }
        MasterCommand::PreserveMaster { on } => {
            need_view(ctx)?;
            restructure(tx, sel, |real, cur, pres| {
                let m = match cur {
                    Some(Target::Master(m)) => m,
                    Some(Target::Layout(l)) => pres.layouts[l].master,
                    None => return refuse("noSlide"),
                };
                // §19.3.1.42 sldMaster@preserve.
                let attrs = &mut real.masters[m].info.root_attrs;
                attrs.retain(|(k, _)| k != "preserve");
                if on {
                    if attrs.is_empty() {
                        attrs.extend(default_ns());
                    }
                    attrs.push(("preserve".into(), "1".into()));
                }
                Ok(cur)
            })
        }
        MasterCommand::InsertPlaceholder { kind, rect } => {
            need_view(ctx)?;
            insert_placeholder(tx, sel, kind, rect)
        }
        MasterCommand::LayoutElements { title, footers } => {
            need_view(ctx)?;
            layout_elements(tx, sel, title, footers)
        }
        MasterCommand::HideBackgroundGraphics { hide } => hide_graphics(tx, sel, ctx, hide),
        MasterCommand::SetBackground { color, all } => {
            set_background(tx, sel, ctx, color.as_deref(), all)
        }
        MasterCommand::MasterTextStyle {
            style,
            level,
            font,
            size,
            bold,
            italic,
            color,
        } => {
            let m = current_master(tx.pres, sel, ctx.view).ok_or(EditError::Refused("noMaster"))?;
            let rgb = match color.as_deref() {
                Some(h) => Some(super::text::parse_hex(h).ok_or(EditError::Refused("badColor"))?),
                None => None,
            };
            let props = StyleProps {
                font,
                size,
                bold,
                italic,
                rgb,
            };
            pres_edit(tx, |p| {
                let ts = &mut p.masters[m].text_styles;
                let ls = match style {
                    TextStyleKind::Title => &mut ts.title,
                    TextStyleKind::Body => &mut ts.body,
                    TextStyleKind::Other => &mut ts.other,
                };
                apply_style(ls.get_or_insert_with(ListStyle::default), level, &props)
            })
        }
        MasterCommand::AddMaster { name } => {
            need_view(ctx)?;
            add_master(tx, sel, &name)
        }
        MasterCommand::ApplyTheme { builtin, file, all } => {
            let theme = match (builtin, file) {
                (Some(id), _) => builtin_theme(&id).ok_or(EditError::Refused("noTheme"))?,
                (None, Some(b64)) => theme_from_file(&b64).ok_or(EditError::Refused("noTheme"))?,
                (None, None) => return refuse("noTheme"),
            };
            let m = current_master(tx.pres, sel, ctx.view).ok_or(EditError::Refused("noMaster"))?;
            pres_edit(tx, |p| {
                for t in themes_to_edit(p, m, all) {
                    let old = &p.themes[t];
                    let mut new = theme.clone();
                    // The part keeps its name and relationships; the new theme's
                    // unmodelled children (and their namespaces) come with it.
                    new.info.part_name.clone_from(&old.info.part_name);
                    new.info.rels.clone_from(&old.info.rels);
                    p.themes[t] = new;
                }
                Ok(())
            })
        }
        MasterCommand::ThemeVariant { colors, fonts, all } => {
            let cs = match colors.as_deref() {
                Some(id) => Some(
                    COLOR_SETS
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or(EditError::Refused("noTheme"))?,
                ),
                None => None,
            };
            let fs = match fonts.as_deref() {
                Some(id) => Some(
                    FONT_SETS
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or(EditError::Refused("noTheme"))?,
                ),
                None => None,
            };
            let m = current_master(tx.pres, sel, ctx.view).ok_or(EditError::Refused("noMaster"))?;
            pres_edit(tx, |p| {
                for t in themes_to_edit(p, m, all) {
                    let th = &mut p.themes[t];
                    if let Some(c) = cs {
                        // §20.1.6.2 clrScheme: the twelve slots in schema order.
                        th.colors.name = c.name.into();
                        th.colors.colors = SLOTS
                            .iter()
                            .zip(c.c)
                            .map(|(s, v)| ((*s).to_owned(), Color::srgb(v)))
                            .collect();
                    }
                    if let Some(f) = fs {
                        // §20.1.4.1.18 fontScheme: Latin typefaces of the major / minor fonts.
                        th.fonts.name = f.name.into();
                        th.fonts.major.latin.typeface = f.major.into();
                        th.fonts.minor.latin.typeface = f.minor.into();
                    }
                }
                Ok(())
            })
        }
        MasterCommand::SetSlideSize { cx, cy, kind, mode } => {
            need_normal(ctx)?;
            set_slide_size(tx, cx, cy, kind, mode)
        }
        MasterCommand::ApplyLayout { layout, slides } => {
            need_normal(ctx)?;
            let slides = slides.unwrap_or_else(|| vec![sel.slide]);
            apply_layout(tx, layout, &slides)
        }
    }
}

fn default_ns() -> Vec<(String, String)> {
    vec![
        (
            "xmlns:a".into(),
            "http://schemas.openxmlformats.org/drawingml/2006/main".into(),
        ),
        (
            "xmlns:r".into(),
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships".into(),
        ),
        (
            "xmlns:p".into(),
            "http://schemas.openxmlformats.org/presentationml/2006/main".into(),
        ),
    ]
}

/// Placeholder kinds of a shape tree, normalised (`title` covers `ctrTitle`).
fn has_ph(nodes: &[ShapeNode], kinds: &[&str]) -> bool {
    nodes.iter().any(|n| {
        n.nv()
            .and_then(|nv| nv.placeholder.as_ref())
            .is_some_and(|p| kinds.contains(&p.kind.as_deref().unwrap_or("obj")))
    })
}

fn max_id(nodes: &[ShapeNode]) -> u32 {
    let mut max = 1;
    bo_pptx::visit(nodes, &mut |n| {
        if let Some(nv) = n.nv() {
            max = max.max(nv.id);
        }
    });
    max
}

/// Position of the target in its master's `sldLayoutIdLst` (insert after it).
fn insert_layout_ref(real: &mut Real, m: usize, after: Option<usize>, new: usize) {
    let refs = &mut real.masters[m].layouts;
    let at = after
        .and_then(|a| refs.iter().position(|r| r.layout == a))
        .map_or(0, |p| p + 1);
    refs.insert(
        at.min(refs.len()),
        bo_model::slide::LayoutRef {
            layout: new,
            id: 0,
            rel_id: String::new(),
        },
    );
}

fn new_layout_name(pres: &Presentation, real: &Real) -> String {
    let also: Vec<String> = real
        .layouts
        .iter()
        .map(|l| l.info.part_name.clone())
        .collect();
    build::unique_part_except(pres, "/ppt/slideLayouts/slideLayout", "xml", &also)
}

fn insert_layout(tx: &mut Tx<'_>, sel: &mut SlideSel, name: &str) -> R<()> {
    restructure(tx, sel, |real, cur, pres| {
        let (m, after) = match cur {
            Some(Target::Master(m)) => (m, None),
            Some(Target::Layout(l)) => (real.layouts[l].master, Some(l)),
            None => return refuse("noSlide"),
        };
        // A title and the footer placeholders, inheriting their boxes from the
        // master (§19.3.1.36: matched by type).
        let mut shapes = Vec::new();
        let master_shapes = &real.masters[m].data.shapes;
        let mut id = 2;
        for (kind, label) in [
            ("title", "Title"),
            ("dt", "Date Placeholder"),
            ("ftr", "Footer Placeholder"),
            ("sldNum", "Slide Number Placeholder"),
        ] {
            if kind != "title" && !has_ph(master_shapes, &[kind]) {
                continue;
            }
            if let Some(p) =
                build::placeholder(id, Some(kind), None, &format!("{label} {}", id - 1))
            {
                shapes.push(p);
                id += 1;
            }
        }
        let mut root = default_ns();
        // §19.3.1.39 sldLayout@preserve, @userDrawn.
        root.push(("preserve".into(), "1".into()));
        root.push(("userDrawn".into(), "1".into()));
        let layout = SlideLayout {
            info: bo_model::slide::PartInfo {
                part_name: new_layout_name(pres, real),
                rels: Vec::new(),
                root_attrs: root,
            },
            data: bo_model::slide::CommonSlideData {
                name: Some(name.to_owned()),
                tree_nv: bo_model::slide::NonVisual {
                    id: 1,
                    ..Default::default()
                },
                shapes,
                ..Default::default()
            },
            master: m,
            // §19.7.15 ST_SlideLayoutType `cust`.
            kind: Some("cust".into()),
            clr_map_ovr: Some(bo_model::slide::ColorMapOverride::Master),
            ..SlideLayout::default()
        };
        real.layouts.push(layout);
        let new = real.layouts.len() - 1;
        insert_layout_ref(real, m, after, new);
        Ok(Some(Target::Layout(new)))
    })
}

fn duplicate_layout(tx: &mut Tx<'_>, sel: &mut SlideSel, name: Option<String>) -> R<()> {
    restructure(tx, sel, |real, cur, pres| {
        let Some(Target::Layout(l)) = cur else {
            return refuse("noLayout");
        };
        let mut copy = real.layouts[l].clone();
        copy.info.part_name = new_layout_name(pres, real);
        if let Some(n) = name {
            copy.data.name = Some(n);
        }
        let m = copy.master;
        real.layouts.push(copy);
        let new = real.layouts.len() - 1;
        insert_layout_ref(real, m, Some(l), new);
        Ok(Some(Target::Layout(new)))
    })
}

fn delete_item(tx: &mut Tx<'_>, sel: &mut SlideSel, used: &[String]) -> R<()> {
    restructure(tx, sel, |real, cur, _| {
        let is_used = |l: &SlideLayout| used.contains(&l.info.part_name.to_ascii_lowercase());
        let (gone_layouts, gone_master): (Vec<usize>, Option<usize>) = match cur {
            Some(Target::Layout(l)) => {
                if is_used(&real.layouts[l]) {
                    return refuse("layoutInUse");
                }
                (vec![l], None)
            }
            Some(Target::Master(m)) => {
                if real.masters.len() < 2 {
                    return refuse("lastMaster");
                }
                let mine: Vec<usize> = (0..real.layouts.len())
                    .filter(|&i| real.layouts[i].master == m)
                    .collect();
                if mine.iter().any(|&i| is_used(&real.layouts[i])) {
                    return refuse("masterInUse");
                }
                (mine, Some(m))
            }
            None => return refuse("noSlide"),
        };
        // Old → new layout indices.
        let mut map: Vec<Option<usize>> = Vec::with_capacity(real.layouts.len());
        let mut k = 0;
        for i in 0..real.layouts.len() {
            if gone_layouts.contains(&i) {
                map.push(None);
            } else {
                map.push(Some(k));
                k += 1;
            }
        }
        let mut i = 0;
        real.layouts.retain(|_| {
            i += 1;
            !gone_layouts.contains(&(i - 1))
        });
        if let Some(m) = gone_master {
            let theme = real.masters[m].theme;
            real.masters.remove(m);
            for l in &mut real.layouts {
                if l.master > m {
                    l.master -= 1;
                }
            }
            // Its theme goes too when no other master uses it.
            if !real.masters.iter().any(|x| x.theme == theme) && theme < real.themes.len() {
                real.themes.remove(theme);
                for x in &mut real.masters {
                    if x.theme > theme {
                        x.theme -= 1;
                    }
                }
            }
        }
        for x in &mut real.masters {
            x.layouts
                .retain_mut(|r| match map.get(r.layout).copied().flatten() {
                    Some(n) => {
                        r.layout = n;
                        true
                    }
                    None => false,
                });
        }
        Ok(None)
    })
}

fn insert_placeholder(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    kind: PhKind,
    rect: Option<Rect>,
) -> R<()> {
    let Some(Target::Layout(l)) = current_target(tx.pres, sel) else {
        return refuse("noLayout");
    };
    let slide = sel.slide;
    let (w, h) = super::slide_size(tx.pres);
    let r = rect.unwrap_or(Rect {
        x: w / 4.0,
        y: h / 4.0,
        w: w / 2.0,
        h: h / 2.0,
    });
    if !(r.w > 0.0 && r.h > 0.0) {
        return refuse("badRect");
    }
    // A fresh `idx` (§19.3.1.36) above every idx of the layout and its master.
    let mut idx = 9;
    let master = tx.pres.layouts.get(l).map(|x| x.master);
    let mut scan = |nodes: &[ShapeNode]| {
        bo_pptx::visit(nodes, &mut |n| {
            if let Some(i) = n
                .nv()
                .and_then(|nv| nv.placeholder.as_ref())
                .and_then(|p| p.idx)
            {
                idx = idx.max(i);
            }
        });
    };
    scan(&tx.pres.slides[slide].data.shapes);
    if let Some(m) = master.and_then(|m| tx.pres.masters.get(m)) {
        scan(&m.data.shapes);
    }
    let idx = idx + 1;
    let id = next_shape_id(tx.pres, slide);
    let (ty, label) = kind.spec();
    let ty_attr = ty.map(|t| format!(r#" type="{t}""#)).unwrap_or_default();
    let emu = |v: f64| (v * EMU_PT).round() as i64;
    let xml = format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label} {n}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph{ty} idx="{idx}"/></p:nvPr></p:nvSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm></p:spPr>"#,
            r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="{lang}"/></a:p></p:txBody></p:sp>"#
        ),
        id = id,
        label = label,
        n = id - 1,
        ty = ty_attr,
        idx = idx,
        x = emu(r.x),
        y = emu(r.y),
        cx = emu(r.w),
        cy = emu(r.h),
        lang = build::LANG,
    );
    let node = build::node(&xml).ok_or(EditError::Refused("badShape"))?;
    let at = tx.pres.slides[slide].data.shapes.len();
    tx.insert_nodes(slide, &[], at, vec![node])?;
    sel.ids = vec![id];
    Ok(())
}

fn layout_elements(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    title: Option<bool>,
    footers: Option<bool>,
) -> R<()> {
    restructure(tx, sel, |real, cur, _| {
        let Some(Target::Layout(l)) = cur else {
            return refuse("noLayout");
        };
        let m = real.layouts[l].master;
        let master_shapes = real.masters[m].data.shapes.clone();
        let shapes = &mut real.layouts[l].data.shapes;
        let mut toggle = |on: bool, kinds: &[&str], add: &[(&str, &str)]| {
            if on {
                for (kind, label) in add {
                    if has_ph(shapes, &[kind])
                        || (*kind != "title" && !has_ph(&master_shapes, &[kind]))
                    {
                        continue;
                    }
                    let id = max_id(shapes) + 1;
                    if let Some(p) =
                        build::placeholder(id, Some(kind), None, &format!("{label} {}", id - 1))
                    {
                        shapes.push(p);
                    }
                }
            } else {
                shapes.retain(|n| {
                    !n.nv()
                        .and_then(|nv| nv.placeholder.as_ref())
                        .is_some_and(|p| kinds.contains(&p.kind.as_deref().unwrap_or("obj")))
                });
            }
        };
        if let Some(on) = title {
            toggle(on, &["title", "ctrTitle"], &[("title", "Title")]);
        }
        if let Some(on) = footers {
            toggle(
                on,
                &["dt", "ftr", "sldNum"],
                &[
                    ("dt", "Date Placeholder"),
                    ("ftr", "Footer Placeholder"),
                    ("sldNum", "Slide Number Placeholder"),
                ],
            );
        }
        Ok(cur)
    })
}

fn hide_graphics(tx: &mut Tx<'_>, sel: &mut SlideSel, ctx: &Ctx, hide: bool) -> R<()> {
    if ctx.view {
        return restructure(tx, sel, |real, cur, _| {
            let Some(Target::Layout(l)) = cur else {
                return refuse("noLayout");
            };
            // §19.3.1.39 sldLayout@showMasterSp (default true).
            real.layouts[l].show_master_sp = hide.then_some(false);
            Ok(cur)
        });
    }
    let i = sel.slide;
    let old = tx
        .pres
        .slides
        .get(i)
        .cloned()
        .ok_or(EditError::Refused("noSlide"))?;
    let mut s = old.clone();
    // §19.3.1.38 sld@showMasterSp (default true).
    s.info.root_attrs.retain(|(k, _)| k != "showMasterSp");
    if hide {
        if s.info.root_attrs.is_empty() {
            s.info.root_attrs = default_ns();
        }
        s.info.root_attrs.push(("showMasterSp".into(), "0".into()));
    }
    if s != old {
        tx.op(SlideOp::Slides {
            index: i,
            remove: vec![old],
            insert: vec![s],
        })?;
    }
    Ok(())
}

fn set_background(
    tx: &mut Tx<'_>,
    sel: &mut SlideSel,
    ctx: &Ctx,
    color: Option<&str>,
    all: bool,
) -> R<()> {
    // §19.3.1.1 bg / §19.3.1.2 bgPr with a solid fill (§20.1.8.54).
    let bg = match color {
        Some(h) => Some(Background::Props {
            fill: Fill::Solid(Color::srgb(
                super::text::parse_hex(h).ok_or(EditError::Refused("badColor"))?,
            )),
            shade_to_title: None,
            extra: Vec::new(),
        }),
        None => None,
    };
    let indices: Vec<usize> = if all && !ctx.view {
        (0..tx.pres.slides.len()).collect()
    } else {
        vec![sel.slide]
    };
    for i in indices {
        let Some(old) = tx.pres.slides.get(i).cloned() else {
            return refuse("noSlide");
        };
        if old.data.background == bg {
            continue;
        }
        let mut s = old.clone();
        s.data.background.clone_from(&bg);
        tx.op(SlideOp::Slides {
            index: i,
            remove: vec![old],
            insert: vec![s],
        })?;
    }
    Ok(())
}

struct StyleProps {
    font: Option<String>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    rgb: Option<u32>,
}

/// Set run defaults (§21.1.2.3.2 defRPr) of list style levels (§21.1.2.4.13 lvlNpPr).
fn apply_style(ls: &mut ListStyle, level: Option<u8>, p: &StyleProps) -> R<()> {
    let levels: Vec<usize> = match level {
        Some(n @ 1..=9) => vec![usize::from(n) - 1],
        Some(_) => return refuse("badLevel"),
        None => {
            let have: Vec<usize> = (0..9).filter(|&i| ls.levels[i].is_some()).collect();
            if have.is_empty() {
                vec![0]
            } else {
                have
            }
        }
    };
    if let Some(sz) = p.size {
        // §20.1.10.72 ST_TextFontSize: 1..4000 pt in 1/100 pt.
        if !(1.0..=4000.0).contains(&sz) {
            return refuse("badSize");
        }
    }
    for i in levels {
        let ppr = ls.levels[i].get_or_insert_with(ParaProps::default);
        let r = ppr.def_rpr.get_or_insert_with(RunProps::default);
        if let Some(sz) = p.size {
            r.sz = Some((sz * 100.0).round() as i32);
        }
        if let Some(b) = p.bold {
            r.b = Some(b);
        }
        if let Some(it) = p.italic {
            r.i = Some(it);
        }
        if let Some(f) = &p.font {
            r.latin = Some(TextFont {
                typeface: f.clone(),
                attrs: Vec::new(),
            });
        }
        if let Some(c) = p.rgb {
            r.fill = Some(Fill::Solid(Color::srgb(c)));
        }
    }
    Ok(())
}

/// Themes to change: the current master's (copied first when other masters
/// share it), or every master's.
fn themes_to_edit(p: &mut PresFields, m: usize, all: bool) -> Vec<usize> {
    if all {
        let mut v: Vec<usize> = p.masters.iter().map(|x| x.theme).collect();
        v.sort_unstable();
        v.dedup();
        v.retain(|&t| t < p.themes.len());
        return v;
    }
    let Some(t) = p.masters.get(m).map(|x| x.theme) else {
        return Vec::new();
    };
    if t >= p.themes.len() {
        return Vec::new();
    }
    let shared = p
        .masters
        .iter()
        .enumerate()
        .any(|(i, x)| i != m && x.theme == t);
    if !shared {
        return vec![t];
    }
    let mut copy = p.themes[t].clone();
    let mut used: Vec<String> = p
        .themes
        .iter()
        .map(|x| x.info.part_name.to_ascii_lowercase())
        .collect();
    used.sort_unstable();
    copy.info.part_name = (1..=used.len() + 1)
        .map(|n| format!("/ppt/theme/theme{n}.xml"))
        .find(|n| used.binary_search(&n.to_ascii_lowercase()).is_err())
        .unwrap_or_default();
    copy.info.rels.clear();
    p.themes.push(copy);
    let nt = p.themes.len() - 1;
    p.masters[m].theme = nt;
    vec![nt]
}

fn add_master(tx: &mut Tx<'_>, sel: &mut SlideSel, name: &str) -> R<()> {
    let size = tx.pres.slide_size.clone();
    let bytes = bo_pptx::build::build_pptx(&bo_pptx::build::DeckSpec {
        width: if size.cx > 0 { size.cx } else { 12_192_000 },
        height: if size.cy > 0 { size.cy } else { 6_858_000 },
        slides: Vec::new(),
        title: None,
    })
    .map_err(|_| EditError::Refused("badMaster"))?;
    let src = bo_pptx::read_pptx(&bytes).map_err(|_| EditError::Refused("badMaster"))?;
    restructure(tx, sel, |real, _, pres| {
        let Some(sm) = src.masters.first() else {
            return refuse("badMaster");
        };
        let mut taken: Vec<String> = real
            .layouts
            .iter()
            .map(|l| l.info.part_name.clone())
            .chain(real.masters.iter().map(|m| m.info.part_name.clone()))
            .chain(real.themes.iter().map(|t| t.info.part_name.clone()))
            .collect();
        let mut unique = |prefix: &str| {
            let n = build::unique_part_except(pres, prefix, "xml", &taken);
            taken.push(n.clone());
            n
        };
        let mut theme = src
            .themes
            .get(sm.theme)
            .cloned()
            .ok_or(EditError::Refused("badMaster"))?;
        theme.info.part_name = unique("/ppt/theme/theme");
        real.themes.push(theme);
        let theme_idx = real.themes.len() - 1;
        let mi = real.masters.len();
        let base = real.layouts.len();
        let mut master = sm.clone();
        master.info.part_name = unique("/ppt/slideMasters/slideMaster");
        master.data.name = Some(name.to_owned());
        master.theme = theme_idx;
        master.master_id = 0;
        master.pres_rel_id = String::new();
        for r in &mut master.layouts {
            r.layout += base;
            r.id = 0;
        }
        for l in &src.layouts {
            let mut l = l.clone();
            l.info.part_name = unique("/ppt/slideLayouts/slideLayout");
            l.master = mi;
            real.layouts.push(l);
        }
        real.masters.push(master);
        Ok(Some(Target::Master(mi)))
    })
}

/// §19.7.16 ST_SlideSizeType values.
const SIZE_KINDS: &[&str] = &[
    "screen4x3",
    "letter",
    "A4",
    "35mm",
    "overhead",
    "banner",
    "custom",
    "ledger",
    "A3",
    "B4ISO",
    "B5ISO",
    "B4JIS",
    "B5JIS",
    "hagakiCard",
    "screen16x9",
    "screen16x10",
];

fn set_slide_size(
    tx: &mut Tx<'_>,
    cx: i64,
    cy: i64,
    kind: Option<String>,
    mode: SizeMode,
) -> R<()> {
    // §19.2.1.39 sldSz: ST_SlideSizeCoordinate is 914 400 – 51 206 400 EMU.
    let range = 914_400..=51_206_400;
    if !range.contains(&cx) || !range.contains(&cy) {
        return refuse("badSize");
    }
    let kind = kind.filter(|k| SIZE_KINDS.contains(&k.as_str()));
    let old = tx.pres.slide_size.clone();
    let from = if old.cx > 0 && old.cy > 0 {
        old.clone()
    } else {
        SlideSize {
            cx: 12_192_000,
            cy: 6_858_000,
            kind: None,
        }
    };
    let to = SlideSize { cx, cy, kind };
    let fit = size_fit(&from, &to, mode);
    let slides: Vec<Slide> = tx
        .pres
        .slides
        .iter()
        .map(|s| {
            let mut s = s.clone();
            fit.apply(&mut s.data.shapes);
            s
        })
        .collect();
    pres_edit(tx, |p| {
        for l in &mut p.layouts {
            fit.apply(&mut l.data.shapes);
        }
        for m in &mut p.masters {
            fit.apply(&mut m.data.shapes);
        }
        p.slide_size = to.clone();
        Ok(())
    })?;
    if slides != tx.pres.slides {
        let old = tx.pres.slides.clone();
        tx.op(SlideOp::Slides {
            index: 0,
            remove: old,
            insert: slides,
        })?;
    }
    Ok(())
}

/// Uniform scale `k` and centring offsets for a size change: `k` is the larger
/// (scale) or smaller (fit) of the width and height ratios; content stays
/// centred: `x' = x·k + (W' − W·k)/2`, `y' = y·k + (H' − H·k)/2`.
fn size_fit(from: &SlideSize, to: &SlideSize, mode: SizeMode) -> SizeFit {
    let (fx, fy) = (from.cx as f64, from.cy as f64);
    let (tx, ty) = (to.cx as f64, to.cy as f64);
    let (sx, sy) = (tx / fx, ty / fy);
    let scale = match mode {
        SizeMode::Scale => sx.max(sy),
        SizeMode::Fit => sx.min(sy),
    };
    SizeFit {
        scale,
        dx: (tx - fx * scale) / 2.0,
        dy: (ty - fy * scale) / 2.0,
    }
}

/// Base slides on `layout` (§19.3.1.38 sld → sldLayout relationship). Placeholders
/// that match the new layout (§19.3.1.36: idx, then type) follow it; others keep
/// their current box, or are dropped when empty; content placeholders the
/// layout adds appear empty.
fn apply_layout(tx: &mut Tx<'_>, layout: usize, slides: &[usize]) -> R<()> {
    let Some(lay) = tx.pres.layouts.get(layout).cloned() else {
        return refuse("noLayout");
    };
    for &i in slides {
        let Some(old) = tx.pres.slides.get(i).cloned() else {
            return refuse("noSlide");
        };
        if old.layout == layout {
            continue;
        }
        let mut s = old.clone();
        s.layout = layout;
        let mut shapes = Vec::with_capacity(old.data.shapes.len());
        for n in &old.data.shapes {
            let ShapeNode::Shape(sh) = n else {
                shapes.push(n.clone());
                continue;
            };
            let Some(ph) = &sh.nv.placeholder else {
                shapes.push(n.clone());
                continue;
            };
            if find_placeholder(&lay.data.shapes, ph, false).is_some() {
                shapes.push(n.clone());
                continue;
            }
            let empty = sh.text.as_ref().is_none_or(|t| t.text().trim().is_empty());
            if empty {
                continue;
            }
            let mut sh = sh.clone();
            if sh.props.xfrm.is_none() {
                sh.props.xfrm = effective_xfrm(&chain(tx.pres, Level::Slide(i), &sh));
            }
            shapes.push(ShapeNode::Shape(sh));
        }
        // Content placeholders of the layout the slide does not have yet.
        let mut next = max_id(&shapes).max(old.data.tree_nv.id) + 1;
        for n in &lay.data.shapes {
            let ShapeNode::Shape(lp) = n else { continue };
            let Some(ph) = &lp.nv.placeholder else {
                continue;
            };
            if matches!(ph.kind.as_deref(), Some("dt" | "ftr" | "sldNum" | "hdr")) {
                continue;
            }
            let present = shapes.iter().any(|x| {
                let ShapeNode::Shape(xs) = x else {
                    return false;
                };
                xs.nv
                    .placeholder
                    .as_ref()
                    .is_some_and(|p| find_placeholder(std::slice::from_ref(n), p, false).is_some())
            });
            if present {
                continue;
            }
            if let Some(p) = build::placeholder(next, ph.kind.as_deref(), ph.idx, &lp.nv.name) {
                shapes.push(p);
                next += 1;
            }
        }
        s.data.shapes = shapes;
        tx.op(SlideOp::Slides {
            index: i,
            remove: vec![old],
            insert: vec![s],
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// BlueOffice themes (own designs)
// ---------------------------------------------------------------------------

/// §20.1.6.2 clrScheme slots in schema order.
const SLOTS: [&str; 12] = [
    "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

/// A colour scheme: the twelve slots of [`SLOTS`].
pub struct ColorSet {
    /// Stable id (UI label key).
    pub id: &'static str,
    /// Scheme name written to the file.
    pub name: &'static str,
    /// `0xRRGGBB` per slot.
    pub c: [u32; 12],
}

/// A font scheme (Latin typefaces of headings and body text).
pub struct FontSet {
    /// Stable id.
    pub id: &'static str,
    /// Scheme name written to the file.
    pub name: &'static str,
    /// Heading typeface (`majorFont`).
    pub major: &'static str,
    /// Body typeface (`minorFont`).
    pub minor: &'static str,
}

/// Colour variants offered by BlueSlide.
// `0xRRGGBB` reads best without digit separators.
#[allow(clippy::unreadable_literal)]
pub const COLOR_SETS: &[ColorSet] = &[
    ColorSet {
        id: "harbor",
        name: "BlueOffice Harbor",
        c: [
            0x000000, 0xFFFFFF, 0x14324F, 0xE8EEF4, 0x1B6CA8, 0x2BA3C4, 0xF2A541, 0x5C946E,
            0x8E6C8A, 0xD9534F, 0x1B6CA8, 0x6F5A8C,
        ],
    },
    ColorSet {
        id: "lotus",
        name: "BlueOffice Lotus",
        c: [
            0x000000, 0xFFFFFF, 0x4A2040, 0xF7EEF3, 0xC2185B, 0xF48FB1, 0x7CB342, 0xFFB300,
            0x5C6BC0, 0x8D6E63, 0xAD1457, 0x6A1B9A,
        ],
    },
    ColorSet {
        id: "bamboo",
        name: "BlueOffice Bamboo",
        c: [
            0x000000, 0xFFFFFF, 0x1E3B2A, 0xEEF4EC, 0x2E7D32, 0x9CCC65, 0x00897B, 0xFBC02D,
            0x6D4C41, 0x558B2F, 0x00695C, 0x4E342E,
        ],
    },
    ColorSet {
        id: "ember",
        name: "BlueOffice Ember",
        c: [
            0x000000, 0xFFFFFF, 0x3E2723, 0xFBEFE6, 0xD84315, 0xFF8F00, 0x6D4C41, 0xC62828,
            0x8D6E63, 0xFFCA28, 0xBF360C, 0x795548,
        ],
    },
    ColorSet {
        id: "slate",
        name: "BlueOffice Slate",
        c: [
            0x000000, 0xFFFFFF, 0x263238, 0xECEFF1, 0x455A64, 0x78909C, 0x26A69A, 0xEF6C00,
            0x5E35B1, 0x9E9D24, 0x0277BD, 0x5E35B1,
        ],
    },
    ColorSet {
        id: "mono",
        name: "BlueOffice Mono",
        c: [
            0x000000, 0xFFFFFF, 0x212121, 0xEEEEEE, 0x424242, 0x616161, 0x757575, 0x9E9E9E,
            0xBDBDBD, 0x303030, 0x424242, 0x757575,
        ],
    },
];

/// Font variants (bundled typefaces, ADR-0003).
pub const FONT_SETS: &[FontSet] = &[
    FontSet {
        id: "carlito",
        name: "BlueOffice Carlito",
        major: "Carlito",
        minor: "Carlito",
    },
    FontSet {
        id: "caladea",
        name: "BlueOffice Caladea",
        major: "Caladea",
        minor: "Carlito",
    },
    FontSet {
        id: "liberation",
        name: "BlueOffice Liberation",
        major: "Liberation Sans",
        minor: "Liberation Sans",
    },
    FontSet {
        id: "tinos",
        name: "BlueOffice Tinos",
        major: "Tinos",
        minor: "Liberation Sans",
    },
];

/// A BlueOffice theme: a colour and a font scheme.
pub struct ThemeDef {
    /// Stable id.
    pub id: &'static str,
    /// Theme name written to the file.
    pub name: &'static str,
    /// [`COLOR_SETS`] id.
    pub colors: &'static str,
    /// [`FONT_SETS`] id.
    pub fonts: &'static str,
}

/// Themes offered on the Design tab.
pub const THEMES: &[ThemeDef] = &[
    ThemeDef {
        id: "harbor",
        name: "BlueOffice Harbor",
        colors: "harbor",
        fonts: "carlito",
    },
    ThemeDef {
        id: "lotus",
        name: "BlueOffice Lotus",
        colors: "lotus",
        fonts: "caladea",
    },
    ThemeDef {
        id: "bamboo",
        name: "BlueOffice Bamboo",
        colors: "bamboo",
        fonts: "liberation",
    },
    ThemeDef {
        id: "ember",
        name: "BlueOffice Ember",
        colors: "ember",
        fonts: "tinos",
    },
    ThemeDef {
        id: "slate",
        name: "BlueOffice Slate",
        colors: "slate",
        fonts: "liberation",
    },
];

/// Theme part markup (§20.1.6.9 theme) of a built-in theme.
fn theme_xml(name: &str, c: &ColorSet, f: &FontSet) -> String {
    let mut clr = String::new();
    for (slot, v) in SLOTS.iter().zip(c.c) {
        clr.push_str(&format!(
            r#"<a:{slot}><a:srgbClr val="{v:06X}"/></a:{slot}>"#
        ));
    }
    let ln = |w: u32| {
        format!(
            r#"<a:ln w="{w}" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln>"#
        )
    };
    let font = |tag: &str, face: &str| {
        format!(
            r#"<a:{tag}><a:latin typeface="{face}"/><a:ea typeface=""/><a:cs typeface=""/></a:{tag}>"#
        )
    };
    format!(
        concat!(
            r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}">"#,
            r#"<a:themeElements><a:clrScheme name="{cn}">{clr}</a:clrScheme>"#,
            r#"<a:fontScheme name="{fname}">{major}{minor}</a:fontScheme>"#,
            r#"<a:fmtScheme name="{name}"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
            r#"<a:solidFill><a:schemeClr val="phClr"><a:tint val="50000"/></a:schemeClr></a:solidFill>"#,
            r#"<a:solidFill><a:schemeClr val="phClr"><a:shade val="80000"/></a:schemeClr></a:solidFill></a:fillStyleLst>"#,
            r#"<a:lnStyleLst>{l1}{l2}{l3}</a:lnStyleLst>"#,
            r#"<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>"#,
            r#"<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
            r#"<a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/></a:schemeClr></a:solidFill>"#,
            r#"<a:solidFill><a:schemeClr val="phClr"><a:shade val="90000"/></a:schemeClr></a:solidFill></a:bgFillStyleLst></a:fmtScheme>"#,
            r#"</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#
        ),
        name = build::esc(name),
        cn = build::esc(c.name),
        clr = clr,
        fname = build::esc(f.name),
        major = font("majorFont", f.major),
        minor = font("minorFont", f.minor),
        l1 = ln(6350),
        l2 = ln(12700),
        l3 = ln(19050),
    )
}

/// A built-in theme as a model theme.
pub fn builtin_theme(id: &str) -> Option<Theme> {
    let d = THEMES.iter().find(|t| t.id == id)?;
    let c = COLOR_SETS.iter().find(|c| c.id == d.colors)?;
    let f = FONT_SETS.iter().find(|f| f.id == d.fonts)?;
    bo_drawingml::theme::parse_theme(theme_xml(d.name, c, f).as_bytes(), "/ppt/theme/theme1.xml")
        .ok()
}

/// The theme of a `.thmx` package (§14.2.7 theme part as main part), of the
/// first master of a `.pptx`, or of a bare theme part.
pub fn theme_from_bytes(bytes: &[u8]) -> Option<Theme> {
    if bytes.first() == Some(&b'<') {
        return bo_drawingml::theme::parse_theme(bytes, "/theme/theme/theme1.xml").ok();
    }
    if bo_pptx::is_pptx(bytes) {
        let p = bo_pptx::read_pptx(bytes).ok()?;
        let m = p.masters.first()?;
        return p.themes.get(m.theme).cloned();
    }
    // Theme part as main part, or behind a theme manager (§20.1.6.11, CORE-FMT-001).
    let xml = crate::variant::theme_of_thmx(bytes)?;
    bo_drawingml::theme::parse_theme(&xml, "/theme/theme/theme1.xml").ok()
}

fn theme_from_file(b64: &str) -> Option<Theme> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    theme_from_bytes(&bytes)
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// A built-in choice (theme or variant) with its preview colours.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    /// Stable id.
    pub id: &'static str,
    /// Preview colours `RRGGBB` (dk2, lt2, accent1–4), or empty for fonts.
    pub colors: Vec<String>,
    /// Heading / body typefaces (font schemes and themes).
    pub fonts: Option<[&'static str; 2]>,
}

/// A layout in the Design / Layout menus.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutItem {
    /// Layout index (for `applyLayout`, `addSlide`).
    pub index: usize,
    /// Layout name.
    pub name: String,
}

/// A master and its layouts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterItem {
    /// Master index.
    pub index: usize,
    /// Master name.
    pub name: String,
    /// Theme name.
    pub theme: Option<String>,
    /// Its layouts in view order.
    pub layouts: Vec<LayoutItem>,
}

/// Design state of the deck.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignOut {
    /// Slide width (EMU).
    pub cx: i64,
    /// Slide height (EMU).
    pub cy: i64,
    /// Preset size type.
    pub size_kind: Option<String>,
    /// Theme name of the current master.
    pub theme: Option<String>,
    /// Colour scheme name of the current master's theme.
    pub colors: String,
    /// Font scheme name of the current master's theme.
    pub fonts: String,
    /// Masters and layouts (normal view only).
    pub masters: Vec<MasterItem>,
    /// Built-in themes.
    pub themes: Vec<Choice>,
    /// Built-in colour variants.
    pub color_variants: Vec<Choice>,
    /// Built-in font variants.
    pub font_variants: Vec<Choice>,
    /// The theme as a `.thmx` file, base64 (`saveThemeFile` only, CORE-FMT-001).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_file: Option<String>,
}

/// One entry of the master view's slide pane.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewItem {
    /// `master` | `layout`.
    pub kind: &'static str,
    /// Name (`cSld@name`).
    pub name: String,
    /// Master index.
    pub master: usize,
    /// Number of slides based on it (layouts: directly; masters: through their layouts).
    pub uses: usize,
    /// Master kept when unused (§19.3.1.42 `preserve`).
    pub preserved: bool,
    /// Master graphics hidden (layouts, §19.3.1.39 `showMasterSp`).
    pub hide_graphics: bool,
    /// Has a title placeholder.
    pub title: bool,
    /// Has date / footer / slide number placeholders.
    pub footers: bool,
}

/// The open master view.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterViewOut {
    /// One item per view slide.
    pub items: Vec<ViewItem>,
}

fn choices() -> (Vec<Choice>, Vec<Choice>, Vec<Choice>) {
    let preview = |c: &ColorSet| {
        [2, 3, 4, 5, 6, 7]
            .iter()
            .map(|&i| format!("{:06X}", c.c[i]))
            .collect::<Vec<_>>()
    };
    let themes = THEMES
        .iter()
        .map(|t| {
            let c = COLOR_SETS.iter().find(|c| c.id == t.colors);
            let f = FONT_SETS.iter().find(|f| f.id == t.fonts);
            Choice {
                id: t.id,
                colors: c.map(preview).unwrap_or_default(),
                fonts: f.map(|f| [f.major, f.minor]),
            }
        })
        .collect();
    let colors = COLOR_SETS
        .iter()
        .map(|c| Choice {
            id: c.id,
            colors: preview(c),
            fonts: None,
        })
        .collect();
    let fonts = FONT_SETS
        .iter()
        .map(|f| Choice {
            id: f.id,
            colors: Vec::new(),
            fonts: Some([f.major, f.minor]),
        })
        .collect();
    (themes, colors, fonts)
}

/// Design state for the result.
pub(crate) fn design_out(pres: &Presentation, ed: &SlideEditor) -> DesignOut {
    let view = ed.master.is_some();
    let m = current_master(pres, &ed.sel, view);
    let theme = m
        .and_then(|m| pres.masters.get(m))
        .and_then(|m| pres.themes.get(m.theme));
    let n = if view {
        real_count(pres)
    } else {
        pres.layouts.len()
    };
    let targets = targets_of(&pres.layouts[..n], &pres.masters);
    let masters = if view {
        Vec::new()
    } else {
        pres.masters
            .iter()
            .enumerate()
            .map(|(mi, x)| MasterItem {
                index: mi,
                name: x.data.name.clone().unwrap_or_default(),
                theme: pres.themes.get(x.theme).and_then(|t| t.name.clone()),
                layouts: targets
                    .iter()
                    .filter_map(|t| match t {
                        Target::Layout(l) if pres.layouts[*l].master == mi => Some(LayoutItem {
                            index: *l,
                            name: pres.layouts[*l].data.name.clone().unwrap_or_default(),
                        }),
                        _ => None,
                    })
                    .collect(),
            })
            .collect()
    };
    let (themes, color_variants, font_variants) = choices();
    DesignOut {
        cx: pres.slide_size.cx,
        cy: pres.slide_size.cy,
        size_kind: pres.slide_size.kind.clone(),
        theme: theme.and_then(|t| t.name.clone()),
        colors: theme.map(|t| t.colors.name.clone()).unwrap_or_default(),
        fonts: theme.map(|t| t.fonts.name.clone()).unwrap_or_default(),
        masters,
        themes,
        color_variants,
        font_variants,
        theme_file: None,
    }
}

/// Master view pane state.
pub(crate) fn view_out(pres: &Presentation, ed: &SlideEditor) -> Option<MasterViewOut> {
    let mv = ed.master.as_ref()?;
    let used = mv.used();
    let count = |l: &SlideLayout| {
        let n = l.info.part_name.to_ascii_lowercase();
        used.iter().filter(|u| **u == n).count()
    };
    let items = view_targets(pres)
        .into_iter()
        .map(|t| match t {
            Target::Master(m) => {
                let x = &pres.masters[m];
                let n = real_count(pres);
                ViewItem {
                    kind: "master",
                    name: x.data.name.clone().unwrap_or_default(),
                    master: m,
                    uses: pres.layouts[..n]
                        .iter()
                        .filter(|l| l.master == m)
                        .map(count)
                        .sum(),
                    preserved: x
                        .info
                        .root_attrs
                        .iter()
                        .any(|(k, v)| k == "preserve" && (v == "1" || v == "true")),
                    hide_graphics: false,
                    title: has_ph(&x.data.shapes, &["title", "ctrTitle"]),
                    footers: has_ph(&x.data.shapes, &["dt", "ftr", "sldNum"]),
                }
            }
            Target::Layout(l) => {
                let x = &pres.layouts[l];
                ViewItem {
                    kind: "layout",
                    name: x.data.name.clone().unwrap_or_default(),
                    master: x.master,
                    uses: count(x),
                    preserved: false,
                    hide_graphics: x.show_master_sp == Some(false),
                    title: has_ph(&x.data.shapes, &["title", "ctrTitle"]),
                    footers: has_ph(&x.data.shapes, &["dt", "ftr", "sldNum"]),
                }
            }
        })
        .collect();
    Some(MasterViewOut { items })
}

// ---------------------------------------------------------------------------
// Engine entry point
// ---------------------------------------------------------------------------

impl Engine {
    /// Run a master view / Design command.
    pub fn slide_master_command(
        &mut self,
        cmd: MasterCommand,
    ) -> crate::Result<super::SlideEditResult> {
        let collab = self.slide_collab.is_some();
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.table = None;
            let mark = ed.history_mark();
            match cmd {
                // The notes / handout master view is open (SLIDE-VIEW-001).
                _ if ed.in_page_master_view() => out.error = Some("masterView".into()),
                MasterCommand::MasterView { on: true } => {
                    if collab {
                        out.error = Some("masterViewCollab".into());
                    } else {
                        enter(ed, pres, &mut out);
                    }
                }
                MasterCommand::MasterView { on: false } => exit(ed, pres, &mut out),
                cmd => {
                    let ctx = Ctx {
                        view: ed.master.is_some(),
                        used: ed.master.as_ref().map(MasterView::used).unwrap_or_default(),
                    };
                    ed.mutate(pres, &mut out, |tx, sel| run(tx, sel, &ctx, cmd));
                    // Background / theme / text-style changes reach every slide.
                    out.all = true;
                }
            }
            ed.master_sync(pres, &mut out, mark, false);
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}
