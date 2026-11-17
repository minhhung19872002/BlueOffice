//! The BlueSlide window (N6): title bar, ribbon, the editing area of [`SlideView`] and the
//! status bar, with input routing. Laid out like the reference presentation app (thumbnails
//! on the left, notes under the slide, slide counter, notes toggle and zoom in the status
//! bar), drawn with BlueDoc's chrome (title bar, ribbon, menus, File screen).
use crate::backstage::{Backstage, FileAction};
use crate::icons::{IconColors, Icons};
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, Ribbon, RibbonHit, Tab};
use crate::slide_anim_pane::{AnimHit, AnimKey, AnimPane};
use crate::slide_comments::{now_iso, CommentsPane, PaneEdit, PaneHit};
use crate::slide_ribbon as sr;
use crate::slide_view::{Focus, SlideView, ViewMode};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{Input, Key, Mods, PageStore, Painter, Request, TextMeasure, TextStyle};
use bo_engine::slide_edit::clip_xfer::Copied;
use bo_engine::slide_edit::master::{MasterCommand, SizeMode, COLOR_SETS, FONT_SETS};
use bo_engine::slide_edit::show::{AnimSpec, TransitionSpec};
use bo_engine::slide_edit::text::{Align, TextFormat};
use bo_engine::slide_edit::SlideCommand;
use bo_render::PathSeg;

/// Slide commands the ribbon offers beyond the shared formatting ones (always enabled).
pub const COMMANDS: [&str; 19] = [
    "slide.new",
    "slide.layout",
    "slide.duplicate",
    "slide.delete",
    "show.fromStart",
    "show.fromCurrent",
    "insert.rect",
    "insert.ellipse",
    "insert.triangle",
    "insert.arrow",
    "arrange.front",
    "arrange.back",
    "arrange.forward",
    "arrange.backward",
    "arrange.group",
    "arrange.ungroup",
    "edit.selectAll",
    "view.notes",
    "view.fitSlide",
];

/// Font sizes of the size box (points), as presentation apps offer them.
const SIZES: [f64; 24] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 44.0,
    48.0, 54.0, 60.0, 66.0, 72.0, 80.0, 88.0, 96.0,
];

/// Families offered by the font box besides the selection's.
const FONTS: [&str; 10] = [
    "Calibri",
    "Calibri Light",
    "Arial",
    "Cambria",
    "Georgia",
    "Segoe UI",
    "Tahoma",
    "Times New Roman",
    "Trebuchet MS",
    "Verdana",
];

/// Ticks (about 100 ms each) of rest before a tooltip shows.
const TOOLTIP_TICKS: u32 = 5;

fn home() -> Tab {
    use Item::{Button as B, FontBox, Large as L, SizeBox};
    Tab {
        label: "ribbon.tabs.home",
        groups: vec![
            Group {
                label: "ribbon.groups.clipboard",
                large: vec![L("edit.paste")],
                rows: vec![
                    vec![B("edit.cut")],
                    vec![B("edit.copy")],
                    vec![B("sl.painter")],
                ],
            },
            sections::slides_group(),
            Group {
                label: "ribbon.groups.font",
                large: vec![],
                rows: vec![
                    vec![
                        FontBox,
                        SizeBox,
                        B("format.growStep"),
                        B("format.shrinkStep"),
                        B("sl.clearFmt"),
                    ],
                    // Reference row: B I U S abc AV▾ Aa▾ | highlight▾ colour▾.
                    vec![
                        B("format.bold"),
                        B("format.italic"),
                        B("format.underline"),
                        B("sl.shadow"),
                        B("format.strike"),
                        B("sl.charSpacing"),
                        B("sl.changeCase"),
                        B("sl.highlight"),
                        B("sl.fontColor"),
                    ],
                ],
            },
            Group {
                label: "ribbon.groups.paragraph",
                large: vec![],
                // Two rows, then Text Direction / Align Text in a column on the right.
                rows: vec![
                    vec![
                        B("format.bullets"),
                        B("format.numbering"),
                        B("format.outdent"),
                        B("format.indent"),
                        B("format.lineSpacing"),
                        B("sl.textDir"),
                    ],
                    vec![
                        B("format.alignLeft"),
                        B("format.alignCenter"),
                        B("format.alignRight"),
                        B("format.justify"),
                        B("sl.columns"),
                        B("sl.alignText"),
                    ],
                    // Right-hand column, third button (reference).
                    vec![B("sl.toSmartArt")],
                ],
            },
            ins::drawing_group(),
            ins::editing_group(),
        ],
    }
}

/// The slide formatting as the ribbon's format state (toggles, font and size boxes, undo).
fn ribbon_format(view: &SlideView, anim: bool) -> bo_edit::FormatState {
    let f = view.format();
    let mut flags = String::new();
    if !view.selection().is_empty() {
        flags.push_str("sel ");
    }
    if anim {
        flags.push_str("anim ");
    }
    // Preview (Transitions / Animations) and Delete comment are dimmed on the reference
    // while the slide has nothing to play or delete.
    if let Some(st) = view.state() {
        if st.transition.is_some() {
            flags.push_str("trans ");
        }
        if !st.animations.is_empty() {
            flags.push_str("anims ");
        }
        if !st.comments.is_empty() {
            flags.push_str("cmt ");
        }
    }
    // Hide Ink is dimmed while the deck holds no ink (as on the reference); Ink Replay
    // while the slide has none.
    if view.engine().slide_has_ink() {
        flags.push_str("ink ");
        if view.current_slide_has_ink() {
            flags.push_str("slink ");
        }
    }
    let (can_undo, can_redo) = view.history();
    bo_edit::FormatState {
        bold: f.bold,
        italic: f.italic,
        underline: f.underline,
        strike: f.strike,
        font: f.font.clone(),
        size_half_points: f.size.map(|s| (s * 2.0).round() as u32),
        color: f.color.clone(),
        highlight: f.highlight.clone(),
        align: match f.align {
            Some(Align::Center) => bo_model::Alignment::Center,
            Some(Align::Right) => bo_model::Alignment::Right,
            Some(Align::Justify) => bo_model::Alignment::Justify,
            _ => bo_model::Alignment::Left,
        },
        bullets: f.bullets,
        numbering: f.numbering,
        can_undo,
        can_redo,
        style: Some(flags),
        ..bo_edit::FormatState::default()
    }
}

/// Plain text of shape `id` among `nodes` (groups searched).
fn shape_text(nodes: &[bo_model::slide::ShapeNode], id: u32) -> Option<String> {
    use bo_model::slide::ShapeNode;
    nodes.iter().find_map(|n| match n {
        ShapeNode::Shape(s) if s.nv.id == id => s.text.as_ref().map(|t| {
            t.paragraphs
                .iter()
                .map(bo_model::slide::Paragraph::text)
                .collect::<Vec<_>>()
                .join(" ")
        }),
        ShapeNode::Group(g) => shape_text(&g.children, id),
        _ => None,
    })
}

/// A line of Outline View: its top and pieces (text, x, bold, italic, underline).
type OutlineLine = (f32, Vec<(String, f32, bool, bool, bool)>);

/// Window contents of BlueSlide.
pub struct SlideWindow {
    view: SlideView,
    title: TitleBar,
    name: String,
    requests: Vec<Request>,
    /// What this window last put on the system clipboard (its internal form), and a count.
    clip_token: Option<String>,
    clip_seq: u32,
    /// Comments pane on the right of the editing area.
    comments: CommentsPane,
    /// Animation Pane (same place; one of the two at a time).
    anim_pane: AnimPane,
    /// The last press was in the Animation Pane (its keys go to it).
    anim_focus: bool,
    /// Animation Painter: the effect to give the next shape chosen, and the shape it came
    /// from.
    painter: Option<(AnimSpec, u32)>,
    /// Round 17 dialogs (Set Up Show, Custom Shows, Zoom, Grid and Guides): the open one,
    /// whether it needs laying out, the pointer's last position over it.
    form8: Option<forms8::Open>,
    form8_layout: bool,
    form8_ptr: (f32, f32),
    /// Set Up Show > Disable hardware graphics acceleration (kept for the session).
    no_hw_accel: bool,
    /// Outline View: the chosen line (slide, body paragraph or `None` for the title).
    outline_sel: Option<outline8::Line>,
    /// Draw tab: the pens of the gallery and the one in use.
    pens: Vec<draw::PenSlot>,
    active_pen: Option<&'static str>,
    /// Measured widths of the gallery tiles' names.
    gallery_w: Vec<(&'static str, f32)>,
    ribbon: Ribbon,
    theme: Theme,
    icons: Icons,
    strings: Strings,
    size: (f32, f32),
    scale: f32,
    menu: Option<Menu>,
    /// What the open menu chooses.
    menu_kind: MenuKind,
    rest: u32,
    tooltip: Option<(String, f32, [f32; 4])>,
    backstage: Option<Backstage>,
    /// Status bar: notes toggle, zoom minus, slider track, plus, percentage, fit button.
    status: [[f32; 4]; 10],
    zoom_drag: bool,
    /// The pointer went down in the editing area.
    area_press: bool,
    relayout_pending: bool,
    /// UI text size (device pixels) for slide numbers and the notes.
    text_size: f32,
    /// Controls drawn over ribbon slots (timing boxes, Effect Options).
    controls: controls::Controls,
    /// Open picker (table grid, shapes, icons, colours…) and the command it belongs to.
    picker: Option<(crate::slide_pickers::Picker, &'static str)>,
    /// Icon library (loaded when the icon picker first opens).
    library: Vec<crate::slide_pickers::LibIcon>,
    /// Open dialog (Header and Footer, Find, Replace).
    dialog: Option<(crate::dialog::Dialog, ins::SlideDialog)>,
    dialog_layout: bool,
    /// A file the host should pick for Insert.
    file_request: Option<InsertFile>,
    /// Typing in a Size spin box.
    size_edit: Option<crate::spin_box::SpinEdit>,
    /// Last searched text.
    find_what: String,
    /// Contextual tabs shown, and whether the next ones appear active (after inserting).
    context: ins::Context,
    activate_context: bool,
    /// Number of fixed tabs; the fixed tab active before a contextual one.
    base_tabs: usize,
    last_base_tab: usize,
    /// Sizes in centimetres (else inches, as the reference in en-US).
    metric: bool,
    /// Format Painter holds the formatting of these shapes.
    fmt_painter: Option<Vec<u32>>,
    /// Format Background pane (same place as the comments pane; one at a time).
    bg_pane: crate::slide_bgpane::BgPane,
    /// Slide Show tab: Show Media Controls, Use Presenter View.
    media_controls: bool,
    use_presenter: bool,
    /// Rehearsed timings waiting for the user's answer.
    pending_timings: Option<Vec<bo_engine::slide_edit::present::SlideTiming>>,
    /// View tab: Outline View / Notes Page drawn by the window, Slide Master view,
    /// gridlines and guides, outline rows (top, slide) for hit testing.
    pane: viewtab::Pane,
    master_view: bool,
    gridlines: bool,
    guides: bool,
    outline_rows: Vec<(f32, usize)>,
    /// Outline View: each listed paragraph line's top and pieces (text, x, bold, italic,
    /// underline).
    outline_lines: Vec<OutlineLine>,
    /// Outline View: the caret and listed lines (round 19, editing in place).
    outline9: outline9::Caret,
    /// Accessibility pane (round 19) and the status bar's extra items.
    a11y9: a11y9::A11yPane,
    status9: status9::Status9,
    /// Selection Pane / Alt Text pane (same place as the comments pane; one at a time).
    side: selpane::SidePane,
    /// Picture Format gallery of previews (Corrections, Color, Transparency).
    gallery: Option<pic::PicGallery>,
    /// Remove Background mode.
    bg_mode: Option<pic::BgMode>,
    /// Draw Borders pen of Table Design.
    pen: fmt::Pen,
    /// Notes / Handout Master view open, and its placeholder check boxes.
    page_master: Option<bo_engine::slide_edit::page_master::PageKind>,
    pm_flags: [bool; 6],
    /// View > Ruler.
    ruler: bool,
    /// A window operation for the host (View > Window).
    window_request: Option<WindowRequest>,
    /// Draw Table / Eraser in use.
    table_tool: tabletools::TableTool,
    /// The spelling issue the open context menu corrects.
    ctx_issue: Option<serde_json::Value>,
    /// Where the next menu opens instead of under its ribbon button (a context menu's
    /// sub-menu).
    menu_anchor: Option<[f32; 4]>,
    /// Where the last context menu opened.
    ctx_at: [f32; 4],
    /// The Spelling pane's walk through the deck.
    spelling: spelling::SpellRun,
    /// Text the host should read aloud.
    speak: Option<String>,
    /// File screen (round 10 of the slide series).
    f10: file10_win::F10,
}

#[path = "slide_controls.rs"]
mod controls;

#[path = "slide_insert.rs"]
mod ins;

#[path = "slide_format.rs"]
mod fmt;

#[path = "slide_selpane.rs"]
mod selpane;

#[path = "slide_picture.rs"]
mod pic;

#[path = "slide_views5.rs"]
mod views5;

#[path = "slide_sections.rs"]
mod sections;

#[path = "slide_tabletools.rs"]
mod tabletools;

#[path = "slide_showtab.rs"]
mod showtab;

#[path = "slide_viewtab.rs"]
mod viewtab;

#[path = "slide_paragraph.rs"]
mod paragraph;

#[path = "slide_chart.rs"]
mod chart;

#[path = "slide_context.rs"]
mod context;

#[path = "slide_spelling.rs"]
mod spelling;

#[path = "slide_insert_icons.rs"]
mod insert_icons;

#[path = "slide_review.rs"]
mod review;

#[path = "slide_draw.rs"]
mod draw;

#[path = "slide_anim8.rs"]
mod anim8;

#[path = "sheet_form.rs"]
#[allow(dead_code, clippy::duplicate_mod)] // BlueSheet includes the same form
mod sform;

#[path = "slide_forms8.rs"]
mod forms8;

#[path = "slide_outline8.rs"]
mod outline8;

#[path = "slide_outline9.rs"]
mod outline9;

#[path = "slide_a11y9.rs"]
mod a11y9;

#[path = "slide_status9.rs"]
mod status9;

#[path = "slide_lang9.rs"]
mod lang9;

#[path = "slide_timing9.rs"]
mod timing9;

#[path = "slide_thesaurus.rs"]
mod thesaurus;

#[path = "slide_file10.rs"]
mod file10;

#[path = "slide_file10_win.rs"]
mod file10_win;

#[path = "slide_showmenu10.rs"]
mod showmenu10;
pub use file10_win::{FileRequest, SlidePrintJob};
pub use ins::InsertFile;
pub use views5::WindowRequest;

impl SlideWindow {
    /// The fixed tabs now shown: Slide Master's tab first while the master is edited.
    fn fixed_tabs(&self) -> Vec<Tab> {
        let mut tabs = base_tabs();
        // Pens added to the Draw tab's gallery.
        if self.pens.len() != 6 {
            if let Some(t) = tabs.iter_mut().find(|t| t.label == "slideNative.draw") {
                *t = draw::draw_tab(self.pens.len());
            }
        }
        if self.master_view {
            tabs.insert(0, viewtab::master_tab());
        }
        if let Some(t) = self.page_master_tab() {
            tabs.insert(0, t);
        }
        tabs
    }
}

/// The fixed tabs, in the reference's order.
fn base_tabs() -> Vec<Tab> {
    vec![
        home(),
        ins::insert_tab(),
        draw::draw_tab(6),
        sr::design_tab(),
        sr::transitions_tab(),
        sr::animations_tab(),
        showtab::show_tab(),
        sr::review_tab(),
        viewtab::view_tab(),
    ]
}

/// Fixed widths of gallery tiles (themes, transitions, effects, shape styles).
fn large_widths(id: &str) -> Option<f32> {
    sr::large_width(id)
        .or_else(|| draw::large_width(id))
        .or_else(|| ins::large_width(id))
        .or_else(|| fmt::large_width(id))
}

/// Fixed widths of row slots drawn over by the window (timing and size boxes).
fn row_widths(id: &str) -> Option<f32> {
    sr::row_width(id)
        .or_else(|| ins::row_width(id))
        .or_else(|| tabletools::row_width(id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    Font,
    Size,
    Layout,
    /// Design: colour variants, font variants, slide size, background colour.
    Colors,
    Fonts,
    SlideSize,
    Background,
    /// Transitions: effect options (direction), duration.
    TransitionEffect,
    Duration,
    /// Animations: effect options (direction).
    AnimEffect,
    /// Every transition / effect of the galleries.
    TransitionMore,
    /// Design: theme effects; every theme (the gallery's "more" column).
    Effects,
    Themes,
}

/// Extra height of BlueSlide's command band (logical px): measured 2026-11-15 at 125 %, the
/// reference presentation app's ribbon body (y 99-235) is 1 device pixel taller than
/// BlueDoc's (the 8 px of round 1 came from another state of the reference window).
const RIBBON_EXTRA: f32 = 0.8;

/// Durations offered for a transition (ms).
const DURATIONS: [u32; 8] = [250, 500, 750, 1000, 1500, 2000, 3000, 5000];

/// Background colours offered by Format Background (`RRGGBB`; `None` resets).
const BACKGROUNDS: [Option<&str>; 9] = [
    None,
    Some("FFFFFF"),
    Some("F2F2F2"),
    Some("D9D9D9"),
    Some("000000"),
    Some("DEEAF6"),
    Some("E2F0D9"),
    Some("FFF2CC"),
    Some("FBE5D6"),
];

/// Ribbon state the enabling rule reads, carried in the format state's `style` (BlueSlide
/// has no paragraph style gallery): `sel` with a selection, `anim` when it has an effect.
fn slide_flags(f: &bo_edit::FormatState) -> &str {
    f.style.as_deref().unwrap_or("")
}

fn slide_enabled(id: &str, f: &bo_edit::FormatState) -> bool {
    let flags = slide_flags(f);
    if ins::needs_selection(id) || fmt::needs_selection(id) {
        return flags.contains("sel");
    }
    // Trigger and Animation Painter need an effect on the selection (dimmed otherwise, as
    // on the reference).
    if matches!(id, "anim.trigger" | "anim.painter") {
        return flags.contains("anim ");
    }
    if flags.contains("outl ")
        && matches!(
            id,
            "view.ruler"
                | "view.gridlines"
                | "view.guides"
                | "view.notes"
                | "view.color"
                | "view.grayscale"
                | "view.blackWhite"
        )
    {
        return false;
    }
    match id {
        "transition.preview" => return flags.contains("trans "),
        "anim.preview" => return flags.contains("anims "),
        "review.deleteComment" => return flags.contains("cmt "),
        "sl.hideInk" => return flags.contains("ink "),
        "draw.replay" => return flags.contains("slink "),
        // Screenshot needs other windows' pictures from the host: not available yet.
        "draw.inkToMath" | "draw.inkHelp" | "sl.screenshot" => return false,
        id if review::GREYED.contains(&id) => return false,
        _ => {}
    }
    crate::commands::enabled(id, f)
        || sr::is_command(id)
        || ins::COMMANDS.contains(&id)
        || showtab::COMMANDS.contains(&id)
        || viewtab::COMMANDS.contains(&id)
        || draw::COMMANDS.contains(&id)
        || draw::PEN_IDS.contains(&id)
        || fmt::COMMANDS.contains(&id)
        || pic::COMMANDS.contains(&id)
        || pic::BG_COMMANDS.contains(&id)
        || views5::COMMANDS.contains(&id)
        || sections::COMMANDS.contains(&id)
        || paragraph::COMMANDS.contains(&id)
        || chart::COMMANDS.contains(&id)
        || review::COMMANDS.contains(&id)
}

impl SlideWindow {
    /// Wrap the presentation `name` shown by `view` with BlueSlide's interface in `locale`.
    pub fn new(mut view: SlideView, locale: &str, name: &str) -> Self {
        let mut strings = Strings::load(locale);
        sr::install_strings(&mut strings);
        ins::install_strings(&mut strings);
        showtab::install_strings(&mut strings);
        viewtab::install_strings(&mut strings);
        fmt::install_strings(&mut strings);
        pic::install_strings(&mut strings);
        views5::install_strings(&mut strings);
        sections::install_strings(&mut strings);
        paragraph::install_strings(&mut strings);
        review::install_strings(&mut strings);
        draw::install_strings(&mut strings);
        let mut icons = Icons::load();
        sr::install_icons(&mut icons);
        ins::install_icons(&mut icons);
        showtab::install_icons(&mut icons);
        viewtab::install_icons(&mut icons);
        fmt::install_icons(&mut icons);
        pic::install_icons(&mut icons);
        views5::install_icons(&mut icons);
        sections::install_icons(&mut icons);
        crate::sheet_window::add_chart_icons(&mut icons);
        paragraph::install_icons(&mut icons);
        insert_icons::install(&mut icons);
        review::install_icons(&mut icons);
        draw::install_icons(&mut icons);
        view.set_labels(
            strings.get("slideEditor.notesPlaceholder"),
            strings.get("slideEditor.end"),
        );
        view.set_present_labels(
            strings.get("slideNative.nextSlide"),
            strings.get("slideEditor.slideOf"),
        );
        view.set_prompts(
            strings.get("slideEditor.promptTitle"),
            strings.get("slideEditor.promptSubtitle"),
            strings.get("slideEditor.promptText"),
        );
        view.set_hf_prompts(
            strings.get("slideNative.phHeader"),
            strings.get("slideNative.phFooter"),
        );
        let mut title = TitleBar::new();
        title.set_app("BlueSlide", "app.slide");
        Self {
            view,
            title,
            name: name.to_owned(),
            requests: Vec::new(),
            clip_token: None,
            clip_seq: 0,
            comments: CommentsPane::default(),
            anim_pane: AnimPane::default(),
            anim_focus: false,
            painter: None,
            pens: draw::default_pens(),
            form8: None,
            form8_layout: false,
            form8_ptr: (0.0, 0.0),
            no_hw_accel: false,
            outline_sel: None,
            active_pen: None,
            gallery_w: Vec::new(),
            ribbon: Ribbon::with_tabs(base_tabs())
                .with_band_extra(RIBBON_EXTRA)
                .with_large_widths(large_widths)
                .with_row_widths(row_widths)
                .with_two_line_labels()
                .with_mark_on_own_line()
                .with_large_dim()
                .with_late_rows(paragraph::late_row)
                // Reference at 125 %: font box 213 px, size box 67.
                .with_box_widths(170.4, 53.6)
                .with_enabled(slide_enabled)
                .with_trailing(|id| {
                    matches!(
                        id,
                        "view.notes" | "win.switch" | "sl.drawTable" | "sl.eraser"
                    )
                }),
            theme: Theme::light("slide"),
            icons,
            strings,
            size: (800.0, 600.0),
            scale: 1.0,
            menu: None,
            menu_kind: MenuKind::Font,
            rest: 0,
            tooltip: None,
            backstage: None,
            status: [[0.0; 4]; 10],
            zoom_drag: false,
            area_press: false,
            relayout_pending: false,
            text_size: 12.0,
            controls: controls::Controls::default(),
            picker: None,
            library: Vec::new(),
            dialog: None,
            dialog_layout: false,
            file_request: None,
            size_edit: None,
            find_what: String::new(),
            context: ins::Context::None,
            activate_context: false,
            base_tabs: base_tabs().len(),
            last_base_tab: 0,
            metric: locale.starts_with("vi"),
            fmt_painter: None,
            bg_pane: crate::slide_bgpane::BgPane::default(),
            media_controls: true,
            // Use Presenter View is on by default, as on the reference.
            use_presenter: true,
            pending_timings: None,
            pane: viewtab::Pane::Normal,
            master_view: false,
            gridlines: false,
            guides: false,
            outline_rows: Vec::new(),
            outline_lines: Vec::new(),
            outline9: outline9::Caret::default(),
            a11y9: a11y9::A11yPane::default(),
            status9: status9::Status9::default(),
            side: selpane::SidePane::default(),
            gallery: None,
            bg_mode: None,
            pen: fmt::Pen::default(),
            page_master: None,
            pm_flags: [true; 6],
            ruler: false,
            window_request: None,
            table_tool: tabletools::TableTool::Off,
            ctx_issue: None,
            menu_anchor: None,
            ctx_at: [0.0; 4],
            spelling: spelling::SpellRun::default(),
            speak: None,
            f10: file10_win::F10::default(),
        }
    }

    /// The editing area.
    pub fn view(&self) -> &SlideView {
        &self.view
    }

    /// Shift / Ctrl held with the mouse button (constrained moves, Ctrl + drag copies,
    /// marquee extends); the host sets them before each mouse event.
    pub fn set_pointer_mods(&mut self, m: crate::Mods) {
        self.view.set_pointer_mods(m);
    }

    /// The editing area, for the host's file operations.
    pub fn view_mut(&mut self) -> &mut SlideView {
        &mut self.view
    }

    /// The presentation now has this file name (after Save As).
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
        self.relayout_pending = true;
    }

    /// Requests for the host made since the last call.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// What a window position is for the window manager.
    pub fn nc_hit(&self, x: f32, y: f32) -> NcHit {
        if self.menu.is_some() || self.view.showing() {
            return NcHit::Client;
        }
        self.title.hit(x, y)
    }

    /// Caret rectangle in window pixels (input method placement).
    pub fn caret_rect(&self) -> Option<[f32; 4]> {
        self.view.caret_rect()
    }

    /// Forget page images when the host changes how document text is rendered.
    pub fn invalidate_page_images(&mut self) {
        self.view.invalidate_page_images();
    }

    /// Update the images the next frame shows.
    pub fn prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        self.view.prepare(store)?;
        self.prepare_picture(store)?;
        self.prepare_file10(store)
    }

    /// Draw one image ahead of need; `true` while more remains (call when idle).
    pub fn prefetch(&mut self, store: &mut dyn PageStore) -> Result<bool, String> {
        self.view.prefetch(store)
    }

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    fn status_h(&self) -> f32 {
        // Reference: line 33 device pixels above the client bottom at 125 % (re-measured).
        (26.4 * self.scale).round()
    }

    fn t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    fn relayout(&mut self, text: &dyn TextMeasure) {
        self.title
            .layout(text, &self.strings, &self.name, self.size.0, self.scale);
        let top = self.title.height();
        self.ribbon
            .layout(text, &self.strings, self.size.0, self.scale, top);
        // Reading view: no ribbon, the slide fills the window between title and status bar.
        let top = if self.view.mode() == ViewMode::Reading {
            top
        } else {
            self.ribbon.bottom()
        };
        let sh = self.status_h();
        let pane = if (self.comments.shown
            || self.anim_pane.shown
            || self.bg_pane.shown
            || self.side.shown())
            && self.view.mode() != ViewMode::Reading
        {
            if self.side.kind == selpane::SideKind::Spelling {
                // The reference's Spelling pane is wider: 437 device px at 125 %.
                (349.6 * self.scale).round()
            } else if self.anim_pane.shown && !self.side.shown() {
                // The reference's Animation Pane: 275 device px at 125 % (round 19).
                (220.0 * self.scale).round()
            } else if self.side.kind == selpane::SideKind::Accessibility {
                // The reference's Accessibility pane: 400 device px at 125 % (round 19).
                (a11y9::PANE_W * self.scale / 1.25).round()
            } else if self.side.kind == selpane::SideKind::Thesaurus {
                // The reference's Thesaurus pane is narrower: 245 device px at 125 %.
                (196.0 * self.scale).round()
            } else {
                CommentsPane::width(self.scale)
            }
        } else {
            0.0
        };
        let area_h = (self.size.1 - top - sh).max(1.0);
        self.view
            .set_frame(0.0, top, (self.size.0 - pane).max(1.0), area_h);
        self.comments
            .layout([self.size.0 - pane, top, pane, area_h], self.scale);
        self.anim_pane
            .layout([self.size.0 - pane, top, pane, area_h], self.scale);
        self.side
            .layout([self.size.0 - pane, top, pane, area_h], self.scale);
        let colors = self.theme_colors();
        self.bg_pane
            .layout([self.size.0 - pane, top, pane, area_h], self.scale, &colors);
        self.view.set_window(self.size.0, self.size.1);
        if let Some(b) = self.backstage.as_mut() {
            b.layout(self.title.height(), self.size, self.scale);
        }
        self.file10_relayout(text);
        // Status bar, right to left: fit button, percentage, plus, slider, minus; the notes
        // toggle before them.
        let (w, h) = self.size;
        let y = h - sh + 1.0;
        let sh = sh - 1.0;
        // Positions from the window's right edge measured on the reference at 125 %
        // (logical px): fit button centre 13.6, percentage 61.6, plus centre 80, slider track
        // 89.6–211.2, minus centre 220, view buttons 37.6 wide ending at 253.6.
        let at = |v: f32| w - v * self.scale;
        let box_at =
            |c: f32, half: f32| [at(c) - half * self.scale, y, 2.0 * half * self.scale, sh];
        let fit = box_at(13.6, 11.0);
        let pct = [at(61.6), y, at(13.6 + 11.0) - at(61.6), sh];
        let plus = box_at(80.0, 9.0);
        let track = [at(211.2).round(), y, (121.6 * self.scale).round(), sh];
        let minus = box_at(220.0, 9.0);
        let bw = 37.6 * self.scale;
        let views: Vec<[f32; 4]> = (0..4)
            .map(|i| [(at(253.6) - (4 - i) as f32 * bw).round(), y, bw.round(), sh])
            .collect();
        let label = self.t("slideEditor.notes", "Notes");
        let style = self.status_style();
        let nw = text.measure(&label, style).0 + self.px(30.0);
        let notes = [views[0][0] - self.px(150.0) - nw, y, nw, sh];
        self.status = [
            notes, minus, track, plus, pct, fit, views[0], views[1], views[2], views[3],
        ];
        self.text_size = (12.0 * self.scale).round();
        let style = self.control_style();
        self.gallery_w = sr::TRANSITIONS
            .iter()
            .map(|(c, _)| *c)
            .chain(sr::EFFECTS.iter().map(|(c, _)| *c))
            .map(|id| {
                let key = format!("commands.{id}");
                let label = self.strings.get(&key);
                (id, text.measure(label, style).0)
            })
            .collect();
        self.view.measure_notes(text, self.text_size);
        self.measure_controls(text);
    }

    /// Text of the reference's ribbon controls (gallery names, timing boxes): measured at
    /// 125 %, 8 % larger than group labels (cap height 13–14 px against 11).
    fn control_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(13.0),
            bold: false,
        }
    }

    fn status_style(&self) -> TextStyle {
        TextStyle {
            size: self.px(12.0),
            bold: false,
        }
    }

    /// Handle an input event; `true` when a new frame is needed.
    pub fn handle(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        let showing = self.view.showing();
        let mode = self.view.mode();
        let released = match input {
            Input::Release { x, y } => Some((x, y)),
            _ => None,
        };
        let redraw = self.route(input, text)
            | self.paint_effect()
            | self.paste_format_painter(released)
            | self.sync_context()
            | self.after_rehearsal();
        // A show with ink ended: ask whether to keep it (reference message box).
        if !self.view.showing() && self.view.has_pending_ink() && self.dialog.is_none() {
            self.open_dialog(ins::SlideDialog::KeepInk);
        }
        if std::mem::take(&mut self.dialog_layout) {
            if let Some((d, _)) = self.dialog.as_mut() {
                d.layout(text, self.size, self.scale);
            }
        }
        if std::mem::take(&mut self.form8_layout) {
            if let Some(o) = self.form8.as_mut() {
                o.form.layout(text, self.size, self.scale);
                let labels: Vec<String> = o
                    .form
                    .ctls
                    .iter()
                    .filter_map(|c| match &c.kind {
                        sform::Kind::Check { label, .. } | sform::Kind::Label(label) => {
                            Some(label.clone())
                        }
                        _ => None,
                    })
                    .collect();
                o.form.measure(text, &labels);
            }
        }
        if mode != self.view.mode() {
            self.relayout_pending = true;
        }
        if showing != self.view.showing() {
            self.requests.push(Request::FullScreen(self.view.showing()));
        }
        if self.relayout_pending {
            self.relayout_pending = false;
            self.relayout(text);
        }
        if self.comments.shown {
            let threads = self
                .view
                .state()
                .map(|s| s.comments.clone())
                .unwrap_or_default();
            let labels = [
                self.t("slideEditor.comments.title", "Comments"),
                self.t("slideEditor.comments.new", "New comment"),
                self.t("slideNative.noComments", "No comments"),
                self.t("slideEditor.comments.resolve", "Resolve"),
                self.t("slideEditor.comments.reopen", "Reopen"),
                self.t("slideEditor.comments.delete", "Delete"),
                self.t("slideEditor.comments.newPlaceholder", ""),
                self.t("slideNative.reply", "Reply..."),
            ];
            self.comments
                .record(text, &threads, &labels, self.theme.text);
        }
        if self.anim_pane.shown {
            let mut effects = self
                .view
                .state()
                .map(|s| s.animations.clone())
                .unwrap_or_default();
            // The row of the shape selected on the slide is the chosen one (reference).
            let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
            let keep = self
                .anim_pane
                .selected
                .and_then(|i| effects.get(i))
                .is_some_and(|a| ids.contains(&a.shape_id));
            if !keep && !ids.is_empty() {
                self.anim_pane.selected = effects.iter().position(|a| ids.contains(&a.shape_id));
            }
            // Rows name the shape and the start of its text ("Title 1: Báo cáo…"), as the
            // reference.
            if let Some(slide) = self
                .view
                .engine()
                .presentation()
                .and_then(|p| p.slides.get(self.view.current()))
            {
                for e in &mut effects {
                    if let Some(t) = shape_text(&slide.data.shapes, e.shape_id) {
                        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !t.is_empty() {
                            e.shape_name = format!("{}: {t}", e.shape_name);
                        }
                    }
                }
            }
            let labels = [
                self.t("slideNative.animationPane", "Animation Pane"),
                self.t("slideNative.playFrom", "Play From"),
                self.t("slideNative.playAll", "Play All"),
                self.t("slideNative.playSelected", "Play Selected"),
            ];
            self.anim_pane
                .record(text, &effects, &labels, self.theme.text);
        }
        if self.side.shown() {
            self.record_side(text);
        }
        self.view.measure_notes(text, self.text_size);
        self.layout_outline(text);
        self.layout_status9(text);
        redraw
    }

    /// Zoom of a slider position: the left half covers 10–100 %, the right half 100–400 %.
    fn slider_zoom(&self, x: f32) -> f32 {
        let [sx, _, sw, _] = self.status[2];
        let t = ((x - sx) / sw).clamp(0.0, 1.0);
        let z = if t < 0.5 {
            0.1 + t / 0.5 * 0.9
        } else {
            1.0 + (t - 0.5) / 0.5 * 3.0
        };
        if (z - 1.0).abs() < 0.04 {
            1.0
        } else {
            (z * 100.0).round() / 100.0
        }
    }

    fn slider_pos(&self) -> f32 {
        let [sx, _, sw, _] = self.status[2];
        let z = self.view.zoom();
        let t = if z <= 1.0 {
            (z - 0.1) / 0.9 * 0.5
        } else {
            0.5 + (z - 1.0) / 3.0 * 0.5
        };
        sx + t.clamp(0.0, 1.0) * sw
    }

    /// Zoom steps of the reference (10 % below 100 %, then larger).
    fn step_zoom(&mut self, up: bool) -> bool {
        let z = (self.view.zoom() * 100.0).round();
        let z = if up {
            ((z / 10.0).floor() + 1.0) * 10.0
        } else {
            ((z / 10.0).ceil() - 1.0) * 10.0
        };
        self.view.set_zoom(Some(z.clamp(10.0, 400.0) / 100.0))
    }

    fn file_action(&mut self, a: FileAction) -> bool {
        if a != FileAction::Back {
            self.requests.push(match a {
                FileAction::New => Request::New,
                FileAction::Open => Request::Open,
                FileAction::Save => Request::Save,
                FileAction::SaveAs => Request::SaveAs,
                FileAction::Print => Request::Print,
                FileAction::ExportPdf => Request::ExportPdf,
                _ => Request::Close,
            });
        }
        self.backstage = None;
        true
    }

    /// Run interface command `id` (ribbon, title bar, menu, shortcut).
    fn action(&mut self, id: &str) -> bool {
        if let Some(r) = self.draw_action(id) {
            return r;
        }
        if let Some(r) = self.table_tool_action(id) {
            return r;
        }
        if let Some(r) = self.review_action(id) {
            return r;
        }
        if let Some(r) = self.sections_action(id) {
            return r;
        }
        if let Some(r) = self.picture_action(id) {
            return r;
        }
        if let Some(r) = self.format_action(id) {
            return r;
        }
        if let Some(r) = self.paragraph_action(id) {
            return r;
        }
        if let Some(r) = self.chart_action(id) {
            return r;
        }
        if let Some(r) = self.insert_action(id) {
            return r;
        }
        if let Some(r) = self.show_action(id) {
            return r;
        }
        if let Some(r) = self.view_action(id) {
            return r;
        }
        match id {
            "file.save" => {
                self.requests.push(Request::Save);
                true
            }
            "edit.copy" => self.copy(false),
            "edit.cut" => self.copy(true),
            "edit.paste" => {
                // The host reads the system clipboard and calls `paste`.
                self.requests.push(Request::Paste);
                true
            }
            "draw.inkToShape" => self.view.ink_to_shapes(),
            id if sr::DRAW_TOOLS.contains(&id) => {
                self.active_pen = None;
                self.view.set_ink_tool(crate::slide_view::InkTool::of(id));
                true
            }
            "anim.painter" => {
                if self.painter.take().is_some() {
                    return true;
                }
                let Some(i) = self.anim_index() else {
                    return false;
                };
                let Some(a) = self.view.state().and_then(|s| s.animations.get(i).cloned()) else {
                    return false;
                };
                self.painter = Some((
                    AnimSpec {
                        effect: Some(a.effect.clone()),
                        class: Some(a.class.to_owned()),
                        direction: a.direction.clone(),
                        trigger: Some(a.trigger.to_owned()),
                        dur_ms: Some(a.dur_ms),
                        delay_ms: Some(a.delay_ms),
                        ..AnimSpec::default()
                    },
                    a.shape_id,
                ));
                true
            }
            "anim.trigger" => {
                self.open_trigger_menu();
                true
            }
            "show.hideSlide" => self.view.run(SlideCommand::HideSlides {
                slides: Some(self.view.selected_slides()),
                hidden: None,
            }),
            "review.deleteComment" => {
                let id = self.comments.active.clone().or_else(|| {
                    self.view
                        .state()
                        .and_then(|s| s.comments.first())
                        .map(|t| t.id.clone())
                });
                match id {
                    Some(id) => {
                        self.comments.active = None;
                        self.view.run(SlideCommand::DeleteComment { id })
                    }
                    None => false,
                }
            }
            "review.spellingDialog" => self.start_spelling(),
            "review.prevComment" | "review.nextComment" => {
                self.step_comment(id == "review.nextComment")
            }
            "anim.pane" => {
                self.side.close();
                self.bg_pane.shown = false;
                self.anim_pane.shown = !self.anim_pane.shown;
                self.comments.shown = false;
                self.relayout_pending = true;
                true
            }
            "anim.earlier" | "anim.later" => {
                let Some(i) = self.anim_index() else {
                    return false;
                };
                let n = self.view.state().map_or(0, |s| s.animations.len());
                let to = if id == "anim.earlier" {
                    i.checked_sub(1)
                } else {
                    (i + 1 < n).then_some(i + 1)
                };
                let Some(to) = to else { return false };
                let moved = self.view.run(SlideCommand::MoveAnimation { from: i, to });
                if moved && self.anim_pane.selected.is_some() {
                    self.anim_pane.selected = Some(to);
                }
                moved
            }
            "review.comments" => {
                self.side.close();
                self.anim_pane.shown = false;
                self.bg_pane.shown = false;
                self.comments.shown = !self.comments.shown;
                self.comments.draft = None;
                self.relayout_pending = true;
                true
            }
            "review.newComment" => {
                self.side.close();
                self.anim_pane.shown = false;
                self.bg_pane.shown = false;
                self.comments.shown = true;
                self.comments.draft = Some(String::new());
                self.relayout_pending = true;
                true
            }
            "view.normal" | "view.sorter" | "view.reading" => {
                let mode = match id {
                    "view.sorter" => ViewMode::Sorter,
                    "view.reading" => ViewMode::Reading,
                    _ => ViewMode::Normal,
                };
                let changed = self.view.set_mode(mode);
                self.relayout_pending = true;
                changed
            }
            "slide.layout" => {
                self.open_menu(MenuKind::Layout, RibbonHit::Command("slide.layout"));
                true
            }
            "view.notes" => {
                let on = !self.view.notes_shown();
                self.view.set_notes_shown(on);
                true
            }
            "view.fitSlide" => {
                self.view.set_zoom(None);
                true
            }
            "view.zoomIn" => self.step_zoom(true),
            "view.zoomOut" => self.step_zoom(false),
            id if sr::is_menu(id) => {
                let (kind, anchor) = match id {
                    "design.colors" => (MenuKind::Colors, "design.colors"),
                    "design.variants" => (MenuKind::Colors, "design.variants"),
                    "design.effects" => (MenuKind::Effects, "design.effects"),
                    "design.layout" => (MenuKind::Layout, "design.layout"),
                    "design.more" => (MenuKind::Themes, "design.more"),
                    "design.fonts" => (MenuKind::Fonts, "design.fonts"),
                    "design.slideSize" => (MenuKind::SlideSize, "design.slideSize"),
                    "design.background" => (MenuKind::Background, "design.background"),
                    "transition.effect" => (MenuKind::TransitionEffect, "transition.effect"),
                    "transition.duration" => (MenuKind::Duration, "transition.duration"),
                    "transition.more" => (MenuKind::TransitionMore, "transition.more"),
                    // The full gallery: effects by class with their pictures (round 17).
                    "anim.more" | "anim.add" => {
                        let anchor = if id == "anim.add" {
                            "anim.add"
                        } else {
                            "anim.none"
                        };
                        self.open_effects(anchor, id == "anim.add");
                        return true;
                    }
                    _ => (MenuKind::AnimEffect, "anim.effect"),
                };
                self.open_menu(kind, RibbonHit::Command(anchor));
                true
            }
            "transition.applyAll" => self.set_transition(None, None, None, true),
            "transition.preview" => self.view.start_preview(false),
            "anim.preview" => self.view.start_preview(true),
            sr::CURRENT_THEME => true,
            id => {
                if let Some((_, theme)) = sr::THEMES.iter().find(|(c, _)| *c == id) {
                    return self.view.run_master(MasterCommand::ApplyTheme {
                        builtin: Some((*theme).to_owned()),
                        file: None,
                        all: true,
                    });
                }
                if let Some((_, kind)) = sr::TRANSITIONS.iter().find(|(c, _)| *c == id) {
                    // The reference plays a transition as soon as it is chosen.
                    let set = self.set_transition(Some(kind), None, None, false);
                    if set {
                        self.view.start_preview(false);
                    }
                    return set;
                }
                if let Some((_, effect)) = sr::EFFECTS.iter().find(|(c, _)| *c == id) {
                    let set = self.apply_effect(*effect, None);
                    if set && effect.is_some() {
                        self.view.start_preview(true);
                    }
                    return set;
                }
                self.view.action(id)
            }
        }
    }

    /// The effect the Animations tab's timing applies to: the one chosen in the Animation
    /// Pane, else the first effect of the selected shapes.
    pub(super) fn anim_index(&self) -> Option<usize> {
        if let Some(i) = self.anim_pane.selected.filter(|_| self.anim_pane.shown) {
            return Some(i);
        }
        let st = self.view.state()?;
        let ids: Vec<u32> = st.selection.iter().map(|s| s.id).collect();
        st.animations
            .iter()
            .find(|a| ids.contains(&a.shape_id))
            .map(|a| a.index)
    }

    /// Trigger: start the effect on a click of another shape of the slide, or with the
    /// sequence.
    fn open_trigger_menu(&mut self) {
        let Some(i) = self.anim_index() else { return };
        let Some(a) = self.view.state().and_then(|s| s.animations.get(i).cloned()) else {
            return;
        };
        let slide = self.view.current();
        let shapes: Vec<(u32, String)> = self
            .view
            .engine()
            .presentation()
            .and_then(|p| p.slides.get(slide))
            .map(|s| {
                s.data
                    .shapes
                    .iter()
                    .filter_map(|n| n.nv().map(|nv| (nv.id, nv.name.clone())))
                    .filter(|(id, _)| *id != a.shape_id)
                    .collect()
            })
            .unwrap_or_default();
        let on_click = self.t("slideNative.onClickOf", "On Click of {{name}}");
        let mut items = vec![MenuItem {
            label: self.t("slideNative.onSequence", "On Sequence"),
            choice: Choice::Command("tshape:0".into()),
            checked: a.trigger_shape.is_none(),
            dim: false,
        }];
        items.extend(shapes.into_iter().map(|(id, name)| MenuItem {
            label: on_click.replace("{{name}}", &name),
            choice: Choice::Command(format!("tshape:{id}")),
            checked: a.trigger_shape == Some(id),
            dim: false,
        }));
        let anchor = self
            .ribbon
            .rect_of(RibbonHit::Command("anim.trigger"))
            .unwrap_or([0.0, 0.0, 1.0, 1.0]);
        self.menu_kind = MenuKind::Duration;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    /// Previous / Next comment: the neighbouring thread on this slide, else the first (last)
    /// thread of the next (previous) slide with comments; the pane opens on it.
    fn step_comment(&mut self, forward: bool) -> bool {
        let threads: Vec<String> = self
            .view
            .state()
            .map(|s| s.comments.iter().map(|t| t.id.clone()).collect())
            .unwrap_or_default();
        let at = self
            .comments
            .active
            .as_ref()
            .and_then(|a| threads.iter().position(|t| t == a));
        let next = match (at, forward) {
            (Some(i), true) => threads.get(i + 1).cloned(),
            (Some(i), false) => i.checked_sub(1).and_then(|j| threads.get(j).cloned()),
            (None, _) => None,
        };
        if let Some(id) = next {
            self.comments.shown = true;
            self.comments.active = Some(id);
            self.relayout_pending = true;
            return true;
        }
        let cur = self.view.current();
        let n = self.view.slide_count();
        let has = |w: &Self, i: usize| {
            w.view
                .engine()
                .presentation()
                .map(|p| bo_engine::slide_edit::comments::comments_out(p, i))
                .unwrap_or_default()
        };
        let order: Vec<usize> = if forward {
            (cur + 1..n).collect()
        } else {
            (0..cur).rev().collect()
        };
        for i in order {
            let list = has(self, i);
            let pick = if forward { list.first() } else { list.last() };
            if let Some(t) = pick {
                let id = t.id.clone();
                self.view.go_to(i);
                self.comments.shown = true;
                self.comments.active = Some(id);
                self.relayout_pending = true;
                return true;
            }
        }
        false
    }

    /// Give the shape just chosen the effect the Animation Painter holds.
    fn paint_effect(&mut self) -> bool {
        let Some((spec, from)) = self.painter.clone() else {
            return false;
        };
        let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
        if ids.is_empty() || ids == [from] {
            return false;
        }
        self.painter = None;
        self.view.run(SlideCommand::AddAnimation { spec })
    }

    /// Input for the Animation Pane (clicks in it; arrows and Delete after a click in it).
    fn anim_input(&mut self, input: &Input) -> Option<bool> {
        if !self.anim_pane.shown {
            return None;
        }
        match input {
            Input::Press { x, y, shift, .. } if self.anim_pane.contains(*x, *y) => {
                self.area_press = false;
                self.anim_focus = true;
                if *shift {
                    if let Some(AnimHit::Select(i)) = self.anim_pane.press(*x, *y) {
                        // Shift+click: a range of rows ("Play Selected").
                        self.anim_pane.extend_to(i);
                        self.relayout_pending = true;
                        return Some(true);
                    }
                }
                Some(match self.anim_pane.press(*x, *y) {
                    Some(AnimHit::Close) => {
                        self.anim_pane.shown = false;
                        self.relayout_pending = true;
                        true
                    }
                    Some(AnimHit::Play) => self.view.start_preview(true),
                    Some(AnimHit::Earlier) => self.action("anim.earlier"),
                    Some(AnimHit::Later) => self.action("anim.later"),
                    Some(AnimHit::Select(i)) => self.choose_effect(i),
                    Some(AnimHit::Menu(i)) => self.open_effect_menu(i),
                    None => false,
                })
            }
            Input::Context { x, y } if self.anim_pane.contains(*x, *y) => {
                match self.anim_pane.press(*x, *y) {
                    Some(AnimHit::Select(i) | AnimHit::Menu(i)) => {
                        self.choose_effect(i);
                        Some(self.open_effect_menu(i))
                    }
                    _ => Some(false),
                }
            }
            Input::Press { .. } => {
                self.anim_focus = false;
                None
            }
            Input::Key(k, m) if self.anim_focus && !m.ctrl => match self.anim_pane.key(*k)? {
                AnimKey::Select(i) => Some(self.choose_effect(i)),
                AnimKey::Remove(i) => {
                    self.anim_pane.selected = None;
                    Some(self.view.run(SlideCommand::RemoveAnimation { index: i }))
                }
            },
            _ => None,
        }
    }

    /// Choose effect `i` in the Animation Pane and select its shape on the slide.
    fn choose_effect(&mut self, i: usize) -> bool {
        self.anim_pane.selected = Some(i);
        self.anim_pane.extra.clear();
        let id = self
            .view
            .state()
            .and_then(|s| s.animations.get(i))
            .map(|a| a.shape_id);
        if let Some(id) = id {
            self.view.run(SlideCommand::Select { ids: vec![id] });
        }
        true
    }

    /// Input for the Comments pane (clicks in it, typing a new comment).
    fn comments_input(&mut self, input: &Input) -> Option<bool> {
        if !self.comments.shown {
            return None;
        }
        let edit = match input {
            Input::Press { x, y, .. } if self.comments.contains(*x, *y) => {
                self.area_press = false;
                let hit = self.comments.press(*x, *y);
                return Some(match hit {
                    Some(PaneHit::New) => {
                        self.comments.draft = Some(String::new());
                        self.comments.reply = None;
                        true
                    }
                    Some(PaneHit::Reply(id)) => {
                        self.comments.draft = None;
                        if self.comments.reply.as_ref().map(|r| &r.0) != Some(&id) {
                            self.comments.reply = Some((id.clone(), String::new()));
                        }
                        self.comments.active = Some(id);
                        true
                    }
                    Some(PaneHit::Select(id)) => {
                        let changed = self.comments.active.as_ref() != Some(&id);
                        self.comments.active = Some(id);
                        changed
                    }
                    Some(PaneHit::Close) => {
                        self.comments.shown = false;
                        self.relayout_pending = true;
                        true
                    }
                    Some(PaneHit::Resolve(id, done)) => {
                        self.view.run(SlideCommand::ResolveComment { id, done })
                    }
                    Some(PaneHit::Delete(id)) => self.view.run(SlideCommand::DeleteComment { id }),
                    None => false,
                });
            }
            Input::Text(t) => self.comments.text(t),
            Input::Key(k, m) if !m.ctrl => self.comments.key(*k),
            _ => None,
        }?;
        Some(match edit {
            PaneEdit::Changed | PaneEdit::Cancel => true,
            PaneEdit::Post(text) => self.post_comment(text, None),
            PaneEdit::PostReply(id, text) => self.post_comment(text, Some(id)),
        })
    }

    /// Add a comment (a reply to thread `parent`) by the signed-in user, dated now.
    fn post_comment(&mut self, text: String, parent: Option<String>) -> bool {
        let author = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "BlueSlide".to_owned());
        self.view.run(SlideCommand::AddComment {
            text,
            author,
            date: now_iso(),
            parent,
            mentions: Vec::new(),
            task: None,
        })
    }

    /// Comment markers on the slide (Normal view): one bubble per thread along the slide's
    /// top-left corner, as the reference marks comments not tied to a shape.
    fn comment_markers(&self) -> Vec<([f32; 4], String)> {
        if self.view.mode() != ViewMode::Normal || self.view.showing() || self.view.previewing() {
            return Vec::new();
        }
        let Some(st) = self.view.state() else {
            return Vec::new();
        };
        let (ox, oy) = self.view.slide_origin();
        let size = self.px(24.0);
        let k = self.view.scale();
        let mut corner = 0;
        let placed = self
            .view
            .engine()
            .presentation()
            .map(|p| bo_engine::slide_edit::geom::place_slide(p, self.view.current()));
        // Top-right corner of an anchored shape (points).
        let shape_corner = |id: u32| {
            let p = bo_engine::slide_edit::geom::find(placed.as_deref()?, id)?;
            let b = p.b.bounds();
            Some((b.x + b.w, b.y))
        };
        st.comments
            .iter()
            .map(|t| {
                // At the comment's anchor point when the file has one (p188:pos), at the
                // top-right corner of the shape it is anchored to, else along the slide's
                // top-left corner.
                let at_shape = t.shape.and_then(shape_corner);
                let (x, y) = if let Some((px, py)) = t.pos {
                    (ox + px as f32 * k, oy + py as f32 * k)
                } else if let Some((sx, sy)) = at_shape {
                    (ox + sx as f32 * k + self.px(4.0), oy + sy as f32 * k)
                } else {
                    corner += 1;
                    (
                        ox + self.px(6.0) + (corner - 1) as f32 * (size + self.px(4.0)),
                        oy + self.px(6.0),
                    )
                };
                ([x.round(), y.round(), size, size], t.id.clone())
            })
            .collect()
    }

    /// A press on a comment marker opens the pane at its thread.
    fn marker_press(&mut self, x: f32, y: f32) -> bool {
        let Some((_, id)) = self
            .comment_markers()
            .into_iter()
            .find(|(r, _)| crate::spin_box::inside(*r, x, y))
        else {
            return false;
        };
        self.comments.shown = true;
        self.comments.active = Some(id);
        self.relayout_pending = true;
        true
    }

    /// Speech bubbles marking the slide's comment threads (own drawing).
    fn paint_markers(&self, p: &mut dyn Painter) {
        let markers = self.comment_markers();
        if markers.is_empty() {
            return;
        }
        let area = self.view.canvas_rect();
        p.push_clip(area[0], area[1], area[2], area[3]);
        for ([x, y, w, h], id) in markers {
            let active =
                self.comments.shown && self.comments.active.as_deref() == Some(id.as_str());
            let (fill, ink) = if active {
                (self.theme.accent, bo_render::Rgba(255, 255, 255, 255))
            } else {
                (bo_render::Rgba(255, 255, 255, 255), self.theme.accent)
            };
            let bh = h * 0.78;
            let r = h * 0.18;
            let segs = [
                PathSeg::M(x + r, y),
                PathSeg::L(x + w - r, y),
                PathSeg::Q(x + w, y, x + w, y + r),
                PathSeg::L(x + w, y + bh - r),
                PathSeg::Q(x + w, y + bh, x + w - r, y + bh),
                PathSeg::L(x + w * 0.45, y + bh),
                PathSeg::L(x + w * 0.22, y + h),
                PathSeg::L(x + w * 0.25, y + bh),
                PathSeg::L(x + r, y + bh),
                PathSeg::Q(x, y + bh, x, y + bh - r),
                PathSeg::L(x, y + r),
                PathSeg::Q(x, y, x + r, y),
                PathSeg::Z,
            ];
            p.path(
                &segs,
                Some(fill),
                Some((self.theme.accent, self.scale.max(1.0).round())),
            );
            // Three text lines inside.
            for k in 0..3 {
                let ly = (y + bh * (0.28 + 0.2 * k as f32)).round();
                let lw = if k == 2 { w * 0.35 } else { w * 0.56 };
                p.fill_rect((x + w * 0.22).round(), ly, lw.round(), 1.0, ink);
            }
        }
        p.pop_clip();
    }

    /// Copy (or cut) the selected shapes, or the chosen slides in the sorter / thumbnails,
    /// into the engine's clipboard and put text, a picture (HTML) and this window's token
    /// on the system clipboard.
    fn copy(&mut self, cut: bool) -> bool {
        let slides = self.view.mode() == ViewMode::Sorter || self.view.focus() == Focus::Thumbs;
        let indices = Some(self.view.selected_slides());
        let cmd = match (slides, cut) {
            (true, false) => SlideCommand::CopySlides { indices },
            (true, true) => SlideCommand::CutSlides { indices },
            (false, false) => SlideCommand::Copy,
            (false, true) => SlideCommand::Cut,
        };
        if !self.view.run(cmd) || self.view.state().is_some_and(|s| s.error.is_some()) {
            return false;
        }
        let out = self.view.state().and_then(|s| s.clipboard.clone());
        let text = out.as_ref().map(|c| c.text.clone()).unwrap_or_default();
        let html = out
            .as_ref()
            .and_then(|c| c.png.as_ref())
            .map(|png| format!(r#"<img src="data:image/png;base64,{png}">"#))
            .unwrap_or_default();
        self.clip_seq += 1;
        let token = format!(
            "BlueSlide/{}/{:p}/{}",
            std::process::id(),
            &raw const *self,
            self.clip_seq
        );
        self.clip_token = Some(token.clone());
        // What another BlueSlide window needs to paste it: a small deck holding only the
        // copied slides or shapes (with their media, layouts and theme), not the whole deck.
        let deck = self.view.engine().slide_clip_deck();
        let internal = match deck.and_then(|(b, c)| serde_json::to_string(&c).ok().map(|w| (b, w)))
        {
            Some((bytes, what)) => {
                use base64::Engine as _;
                format!(
                    "{token}\n{what}\n{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                )
            }
            _ => token,
        };
        self.requests.push(Request::SetClipboard {
            text,
            html,
            internal,
        });
        true
    }

    /// Paste what the system clipboard holds: this window's own copy from the engine's
    /// clipboard, else another program's text as a new text box (as the reference does).
    #[allow(clippy::needless_pass_by_value)] // the host hands over what it read
    pub fn paste(
        &mut self,
        internal: Option<String>,
        _html: Option<String>,
        text: Option<String>,
    ) -> bool {
        let token = internal
            .as_deref()
            .and_then(|i| i.lines().next())
            .map(str::to_owned);
        let ours = token.is_some() && token == self.clip_token;
        let text = text.unwrap_or_default();
        if ours || (internal.is_none() && text.trim().is_empty()) {
            return self.view.run(SlideCommand::Paste { keep_source: false });
        }
        // Copied in another BlueSlide window: its deck travels with it.
        if let Some(i) = internal.as_deref().filter(|i| i.starts_with("BlueSlide/")) {
            let mut parts = i.splitn(3, '\n');
            let (_, what, data) = (parts.next(), parts.next(), parts.next());
            let what: Option<Copied> = what.and_then(|w| serde_json::from_str(w).ok());
            let bytes = data.and_then(bo_engine::merge::base64_decode);
            if let (Some(what), Some(bytes)) = (what, bytes) {
                if self.view.engine_mut().copy_from_deck(&bytes, &what) {
                    // Our own copy is no longer what the engine holds.
                    self.clip_token = None;
                    return self.view.run(SlideCommand::Paste { keep_source: false });
                }
            }
        }
        if text.trim().is_empty() {
            return false;
        }
        self.view.run(SlideCommand::InsertTextBox {
            text: text.replace("\r\n", "\n"),
            rect: None,
        })
    }

    /// Set the current slide's transition (`kind` `None` keeps the effect; `all`: every
    /// slide gets the current slide's transition, as Apply To All).
    fn set_transition(
        &mut self,
        kind: Option<&str>,
        dir: Option<String>,
        dur_ms: Option<u32>,
        all: bool,
    ) -> bool {
        let cur = self.view.state().and_then(|s| s.transition.clone());
        let transition = if all {
            let Some(t) = cur else {
                return self.view.run(SlideCommand::SetTransition {
                    slides: None,
                    all: true,
                    transition: TransitionSpec {
                        kind: "none".into(),
                        ..TransitionSpec::default()
                    },
                });
            };
            TransitionSpec {
                kind: t.kind,
                dir: t.dir,
                dur_ms: Some(t.dur_ms),
                ..TransitionSpec::default()
            }
        } else {
            TransitionSpec {
                kind: kind.unwrap_or("keep").to_owned(),
                dir,
                dur_ms,
                ..TransitionSpec::default()
            }
        };
        self.view.run(SlideCommand::SetTransition {
            slides: None,
            all,
            transition,
        })
    }

    /// Give the selected shapes `effect` (engine effect id and class) in place of their
    /// effects, as the reference's animation gallery does (`None` removes them); `dir`
    /// changes the direction of their effects instead.
    fn apply_effect(&mut self, effect: Option<(&str, &str)>, dir: Option<String>) -> bool {
        let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
        if ids.is_empty() {
            return false;
        }
        let mine: Vec<usize> = self
            .view
            .state()
            .map(|s| {
                s.animations
                    .iter()
                    .filter(|a| ids.contains(&a.shape_id))
                    .map(|a| a.index)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(d) = dir {
            let mut done = false;
            for index in mine {
                done |= self.view.run(SlideCommand::SetAnimation {
                    index,
                    spec: AnimSpec {
                        direction: Some(d.clone()),
                        ..AnimSpec::default()
                    },
                });
            }
            return done;
        }
        let mut done = false;
        for index in mine.into_iter().rev() {
            done |= self.view.run(SlideCommand::RemoveAnimation { index });
        }
        if let Some((e, class)) = effect {
            done |= self.view.run(SlideCommand::AddAnimation {
                spec: AnimSpec {
                    effect: Some(e.to_owned()),
                    class: Some(class.to_owned()),
                    ..AnimSpec::default()
                },
            });
        }
        done
    }

    fn open_menu(&mut self, kind: MenuKind, hit: RibbonHit) {
        let anchor = self
            .menu_anchor
            .take()
            .or_else(|| self.ribbon.rect_of(hit))
            .unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let f = self.view.format();
        let design = self.view.state().and_then(|s| s.design.clone());
        let items: Vec<MenuItem> = match kind {
            MenuKind::Font => {
                let mut names: Vec<String> = FONTS.iter().map(|s| (*s).to_owned()).collect();
                if let Some(cur) = f.font.as_ref().filter(|c| !names.contains(c)) {
                    names.insert(0, cur.clone());
                }
                names
                    .into_iter()
                    .map(|n| MenuItem {
                        checked: f.font.as_deref() == Some(n.as_str()),
                        dim: false,
                        label: n.clone(),
                        choice: Choice::Font(n),
                    })
                    .collect()
            }
            MenuKind::Size => SIZES
                .iter()
                .map(|pt| {
                    let half = (pt * 2.0).round() as u32;
                    MenuItem {
                        label: if half.is_multiple_of(2) {
                            (half / 2).to_string()
                        } else {
                            format!("{},5", half / 2)
                        },
                        checked: f.size.is_some_and(|s| (s - pt).abs() < 0.01),
                        dim: false,
                        choice: Choice::Size(half),
                    }
                })
                .collect(),
            MenuKind::Layout => self
                .view
                .layouts()
                .into_iter()
                .map(|(i, name)| MenuItem {
                    label: name,
                    choice: Choice::Command(format!("layout:{i}")),
                    checked: false,
                    dim: false,
                })
                .collect(),
            MenuKind::Effects => vec![MenuItem {
                // The built-in themes share one effect style (§20.1.4.1.12 fmtScheme).
                label: self.t("slideNative.officeEffects", "Office"),
                choice: Choice::Command("noop:".into()),
                checked: true,
                dim: false,
            }],
            MenuKind::Themes => std::iter::once((sr::CURRENT_THEME, String::new()))
                .chain(sr::THEMES.iter().map(|(c, _)| {
                    (
                        *c,
                        self.strings.get(&format!("commands.tip.{c}")).to_owned(),
                    )
                }))
                .map(|(c, label)| MenuItem {
                    label: if label.is_empty() {
                        self.strings.get(&format!("commands.tip.{c}")).to_owned()
                    } else {
                        label
                    },
                    choice: Choice::Command(c.to_owned()),
                    checked: false,
                    dim: false,
                })
                .collect(),
            MenuKind::Colors => COLOR_SETS
                .iter()
                .map(|c| MenuItem {
                    label: c.name.trim_start_matches("BlueOffice ").to_owned(),
                    choice: Choice::Command(format!("colors:{}", c.id)),
                    checked: design.as_ref().is_some_and(|d| d.colors == c.name),
                    dim: false,
                })
                .collect(),
            MenuKind::Fonts => FONT_SETS
                .iter()
                .map(|f| MenuItem {
                    label: format!("{} / {}", f.major, f.minor),
                    choice: Choice::Command(format!("fonts:{}", f.id)),
                    checked: design.as_ref().is_some_and(|d| d.fonts == f.name),
                    dim: false,
                })
                .collect(),
            MenuKind::SlideSize => [
                ("slideNative.standard", "size:43", 9_144_000, 6_858_000),
                ("slideNative.widescreen", "size:169", 12_192_000, 6_858_000),
            ]
            .iter()
            .map(|(key, c, cx, cy)| MenuItem {
                label: self.strings.get(key).to_owned(),
                choice: Choice::Command((*c).to_owned()),
                checked: design.as_ref().is_some_and(|d| d.cx == *cx && d.cy == *cy),
                dim: false,
            })
            .chain(std::iter::once(MenuItem {
                label: self.t("slideNative.customSize", "Custom Slide Size..."),
                choice: Choice::Command("ins:slideSize".into()),
                checked: false,
                dim: false,
            }))
            .collect(),
            MenuKind::Background => BACKGROUNDS
                .iter()
                .map(|c| MenuItem {
                    label: match c {
                        Some(hex) => format!("#{hex}"),
                        None => self
                            .strings
                            .get("slideEditor.design.resetBackground")
                            .to_owned(),
                    },
                    choice: Choice::Command(format!("bg:{}", c.unwrap_or(""))),
                    checked: false,
                    dim: false,
                })
                .chain([
                    MenuItem {
                        label: self.t("slideNative.formatBackgroundMenu", "Format Background..."),
                        choice: Choice::Command("ins:formatBg".into()),
                        checked: false,
                        dim: false,
                    },
                    MenuItem {
                        label: self.t("slideNative.hideGraphics", "Hide Background Graphics"),
                        choice: Choice::Command("ins:hideGraphics".into()),
                        checked: self.bg_pane.hide_graphics,
                        dim: false,
                    },
                ])
                .collect(),
            MenuKind::TransitionEffect => {
                let t = self.view.state().and_then(|s| s.transition.clone());
                t.map(|t| {
                    t.dirs
                        .iter()
                        .map(|d| MenuItem {
                            label: self.dir_label(d),
                            choice: Choice::Command(format!("tdir:{d}")),
                            checked: t.dir.as_deref() == Some(*d),
                            dim: false,
                        })
                        .collect()
                })
                .unwrap_or_default()
            }
            MenuKind::AnimEffect => {
                let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
                let a = self.view.state().and_then(|s| {
                    s.animations
                        .iter()
                        .find(|a| ids.contains(&a.shape_id))
                        .cloned()
                });
                a.map(|a| {
                    a.dirs
                        .iter()
                        .map(|d| MenuItem {
                            label: self.dir_label(d),
                            choice: Choice::Command(format!("adir:{d}")),
                            checked: a.direction.as_deref() == Some(*d),
                            dim: false,
                        })
                        .collect()
                })
                .unwrap_or_default()
            }
            MenuKind::TransitionMore => Vec::new(),
            MenuKind::Duration => {
                let cur = self
                    .view
                    .state()
                    .and_then(|s| s.transition.as_ref())
                    .map(|t| t.dur_ms);
                DURATIONS
                    .iter()
                    .map(|ms| MenuItem {
                        label: self.strings.get("slideNative.seconds").replace(
                            "{{n}}",
                            &format!("{:.2}", f64::from(*ms) / 1000.0).replace('.', ","),
                        ),
                        choice: Choice::Command(format!("tdur:{ms}")),
                        checked: cur == Some(*ms),
                        dim: false,
                    })
                    .collect()
            }
        };
        let on = self.toggled();
        let gallery = |list: &mut dyn Iterator<Item = &'static str>| -> Vec<MenuItem> {
            list.map(|id| MenuItem {
                label: self.strings.get(&format!("commands.{id}")).to_owned(),
                choice: Choice::Command(id.to_owned()),
                checked: on.contains(&id),
                dim: false,
            })
            .collect()
        };
        let items = match kind {
            MenuKind::TransitionMore => gallery(&mut sr::TRANSITIONS.iter().map(|(c, _)| *c)),
            _ => items,
        };
        if items.is_empty() {
            return;
        }
        self.menu_kind = kind;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn choose(&mut self, choice: Choice) -> bool {
        match choice {
            Choice::Font(typeface) => self.view.format_text(TextFormat::Font { typeface }),
            Choice::Size(half) => self.view.format_text(TextFormat::Size {
                points: f64::from(half) / 2.0,
            }),
            Choice::Command(c) => {
                if let Some(r) = self.choose_design(&c) {
                    return r;
                }
                match c.strip_prefix("layout:").and_then(|i| i.parse().ok()) {
                    // The Layout menus (Home, Design) give the selected slides the chosen
                    // layout, as the reference does.
                    Some(layout) if self.menu_kind == MenuKind::Layout => {
                        self.view.run_master(MasterCommand::ApplyLayout {
                            layout,
                            slides: Some(self.view.selected_slides()),
                        })
                    }
                    _ => self.action(&c),
                }
            }
            Choice::Style(_) => false,
        }
    }

    /// Menu choices of the Design, Transitions and Animations tabs.
    fn choose_design(&mut self, c: &str) -> Option<bool> {
        let (kind, v) = c.split_once(':')?;
        Some(match kind {
            "colors" => self.view.run_master(MasterCommand::ThemeVariant {
                colors: Some(v.to_owned()),
                fonts: None,
                all: true,
            }),
            "fonts" => self.view.run_master(MasterCommand::ThemeVariant {
                colors: None,
                fonts: Some(v.to_owned()),
                all: true,
            }),
            "size" => {
                let (cx, kind) = if v == "43" {
                    (9_144_000, Some("screen4x3".to_owned()))
                } else {
                    (12_192_000, None)
                };
                let changed = self.view.run_master(MasterCommand::SetSlideSize {
                    cx,
                    cy: 6_858_000,
                    kind,
                    mode: SizeMode::Fit,
                });
                self.view.set_zoom(None);
                changed
            }
            "bg" => self.view.run_master(MasterCommand::SetBackground {
                color: (!v.is_empty()).then(|| v.to_owned()),
                all: false,
            }),
            "lspc" => self.choose_line_spacing(v),
            "case" => self.change_case(v),
            "chsp" => self.choose_spacing(v),
            "tosmart" => self.convert_to_smartart(v),
            "ctxsp" => self.choose_context(v),
            "rv" if v == "closepane" => {
                self.side.close();
                self.relayout_pending = true;
                true
            }
            "rv" => self.choose_review(v),
            "deras" | "dadd" | "dpen" => return self.choose_draw(kind, v),
            "arow" => return self.choose_effect_menu(v),
            "hl" | "fc" | "body" => self.choose_home(kind, v),
            "slchart" => self.chart_action(c).unwrap_or(false),
            "tdir" => self.set_transition(None, Some(v.to_owned()), None, false),
            "tdur" => self.set_transition(None, None, v.parse().ok(), false),
            "adir" => self.apply_effect(None, Some(v.to_owned())),
            "snd" | "trig" => return self.choose_control(kind, v),
            "tshape" => {
                let Some(index) = self.anim_index() else {
                    return Some(false);
                };
                self.view.run(SlideCommand::SetAnimation {
                    index,
                    spec: AnimSpec {
                        trigger_shape: v.parse().ok(),
                        ..AnimSpec::default()
                    },
                })
            }
            "add" => {
                let Some((_, Some((e, class)))) = sr::EFFECTS.iter().find(|(c, _)| *c == v) else {
                    return Some(false);
                };
                let ids: Vec<u32> = self.view.selection().iter().map(|s| s.id).collect();
                if ids.is_empty() {
                    return Some(false);
                }
                let added = self.view.run(SlideCommand::AddAnimation {
                    spec: AnimSpec {
                        effect: Some((*e).to_owned()),
                        class: Some((*class).to_owned()),
                        ..AnimSpec::default()
                    },
                });
                if added {
                    self.view.start_preview(true);
                }
                added
            }
            "side" => self.choose_format(v),
            "sect" => self.choose_section(v),
            "cshow" | "rec" => return self.choose_show(kind, v),
            "fbs" => self.choose_file10(v),
            "shw" => self.view.show_command(v),
            "zoom" | "pm" => return self.choose_view(kind, v),
            _ => return self.choose_insert(kind, v),
        })
    }

    /// Name of an effect variant (direction, ...) from the editor's strings.
    fn dir_label(&self, d: &str) -> String {
        let key = match d {
            "u" => "slideEditor.animDir.up".to_owned(),
            "d" => "slideEditor.animDir.down".to_owned(),
            other => format!("slideEditor.animDir.{other}"),
        };
        self.t(&key, d)
    }

    fn menu_input(&mut self, input: &Input) -> Option<bool> {
        let menu = self.menu.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => menu.hover(x, y),
            Input::Press { x, y, .. } => {
                if menu.contains(x, y) {
                    menu.press(x, y);
                } else {
                    self.menu = None;
                    self.ribbon.clear_pointer();
                }
                true
            }
            Input::Release { x, y } => {
                if let Some(choice) = menu.release(x, y) {
                    self.menu = None;
                    self.choose(choice);
                }
                true
            }
            Input::Wheel(dy) => menu.wheel(dy),
            Input::Key(Key::Escape, _) => {
                self.menu = None;
                true
            }
            Input::Key(Key::Up, _) => {
                menu.step(-1);
                true
            }
            Input::Key(Key::Down, _) => {
                menu.step(1);
                true
            }
            Input::Key(Key::Enter, _) => {
                let c = menu.current();
                self.menu = None;
                if let Some(c) = c {
                    self.choose(c);
                }
                true
            }
            Input::Text(_) | Input::Key(..) => true,
            _ => return None,
        })
    }

    fn tick(&mut self, text: &dyn TextMeasure) -> bool {
        let Some(RibbonHit::Command(id)) = self.ribbon.hovered() else {
            return false;
        };
        self.rest += 1;
        if self.rest != TOOLTIP_TICKS || self.menu.is_some() {
            return false;
        }
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let mut name = self.strings.get(&format!("commands.{id}")).to_owned();
        if name.is_empty() {
            // Theme tiles: the theme's name.
            name = self.strings.get(&format!("commands.tip.{id}")).to_owned();
        }
        let keys = match id {
            "slide.new" => Some("Ctrl+M"),
            "show.fromStart" => Some("F5"),
            "show.fromCurrent" => Some("Shift+F5"),
            _ => crate::commands::shortcut(id),
        };
        let label = match keys {
            Some(k) => format!("{name} ({k})"),
            None => name,
        };
        let (w, _) = text.measure(&label, self.status_style());
        self.tooltip = Some((label, w, anchor));
        true
    }

    fn status_hit(&self, x: f32, y: f32) -> Option<usize> {
        self.status
            .iter()
            .position(|[rx, ry, rw, rh]| x >= *rx && x < rx + rw && y >= *ry && y < ry + rh)
    }

    #[allow(clippy::too_many_lines)] // one arm per input kind, like DocWindow::route
    fn route(&mut self, input: Input, text: &dyn TextMeasure) -> bool {
        // The slide show takes every input.
        if self.view.showing() {
            return match input {
                Input::Resize { width, height } => {
                    self.size = (width.max(1.0), height.max(1.0));
                    self.relayout(text);
                    true
                }
                Input::Scale(s) => {
                    self.scale = s;
                    self.view.set_display_scale(s);
                    self.relayout(text);
                    true
                }
                Input::Tick => self.view.show_tick(),
                Input::Blink => false,
                other => {
                    if let Some(r) = self.show_menu_input(&other) {
                        return r;
                    }
                    self.view.handle(other)
                }
            };
        }
        if self.form8.is_some() {
            if let Some(r) = self.form8_input(&input) {
                return r;
            }
        }
        if let Some(r) = self.file10_input(&input) {
            return r;
        }
        if let Input::Key(k, m) = input {
            if let Some(r) = self.outline_key(k, m) {
                return r;
            }
        }
        if let Input::Text(t) = &input {
            if let Some(r) = self.outline_text(t) {
                return r;
            }
        }
        if let Some(b) = self.backstage.as_mut() {
            match input {
                Input::Move { x, y } if y >= self.title.height() => return b.hover(x, y),
                Input::Press { x, y, .. } if y >= self.title.height() => {
                    return match b.press(x, y) {
                        Some(a) => self.file_action(a),
                        None => false,
                    };
                }
                Input::Key(Key::Escape, _) => {
                    self.backstage = None;
                    return true;
                }
                Input::Text(_)
                | Input::Key(..)
                | Input::Wheel(_)
                | Input::Context { .. }
                | Input::Preedit(_) => return false,
                _ => {}
            }
        }
        if self.dialog.is_some() {
            if let Some(r) = self.dialog_input(&input, text) {
                return r;
            }
        }
        if self.gallery.is_some() {
            if let Some(r) = self.gallery_input(&input) {
                return r;
            }
        }
        if self.picker.is_some() {
            if let Some(r) = self.picker_input(&input) {
                return r;
            }
        }
        if self.menu.is_some() {
            if let Some(r) = self.menu_input(&input) {
                return r;
            }
        }
        if let Some(r) = self.size_key(&input) {
            return r;
        }
        let tip_before = self.tooltip.is_some();
        if matches!(
            input,
            Input::Move { .. }
                | Input::Press { .. }
                | Input::Wheel(_)
                | Input::Key(..)
                | Input::Text(_)
        ) {
            self.rest = 0;
            self.tooltip = None;
        }
        if let Input::Key(Key::Letter('s'), Mods { ctrl: true, .. }) = input {
            return self.action("file.save");
        }
        // Ctrl+P: the File screen's Print page, as on the reference.
        if let Input::Key(
            Key::Letter('p'),
            Mods {
                ctrl: true,
                shift: false,
                alt: false,
                ..
            },
        ) = input
        {
            if self.view.text_edit().is_none() {
                return self.open_file_screen(file10::BsPage::Print);
            }
        }
        if let Some(r) = self.spelling_key(&input) {
            return r;
        }
        if let Input::Key(Key::F(7), m) = input {
            if !m.ctrl && !m.shift && !m.alt && !self.view.showing() {
                return self.start_spelling();
            }
            // Shift+F7: Thesaurus, as on the reference.
            if m.shift && !m.ctrl && !m.alt && !self.view.showing() {
                return self.open_thesaurus();
            }
        }
        // Clipboard shortcuts outside text editing go to the engine's slide clipboard.
        if let Input::Key(Key::Letter(c @ ('c' | 'x' | 'v')), Mods { ctrl: true, .. }) = input {
            if self.view.text_edit().is_none() && self.view.focus() != Focus::Notes {
                let id = match c {
                    'c' => "edit.copy",
                    'x' => "edit.cut",
                    _ => "edit.paste",
                };
                return self.action(id);
            }
        }
        if let Input::Press { x, y, .. } = input {
            if let Some(r) = self.controls_press(x, y) {
                return r;
            }
            if let Some(r) = self.size_press(x, y) {
                return r;
            }
            if let Input::Press { count, .. } = input {
                if let Some(r) = self.section_press(x, y, count) {
                    self.area_press = false;
                    return r;
                }
            }
            if let Some(r) = self.pane_press(x, y) {
                self.area_press = false;
                return r;
            }
        }
        if let Some(r) = self.controls_key(&input, text) {
            return r;
        }
        if let Some(r) = self.table_tool_input(&input) {
            return r;
        }
        if let Some(r) = self.removal_input(&input) {
            return r;
        }
        if let Some(r) = self.thesaurus_pointer(&input) {
            return r || tip_before != self.tooltip.is_some();
        }
        if let Some(r) = self.a9_input(&input) {
            return r || tip_before != self.tooltip.is_some();
        }
        if let Some(r) = self.side_input(&input) {
            return r || tip_before != self.tooltip.is_some();
        }
        if let Some(r) = self.anim_input(&input) {
            return r || tip_before != self.tooltip.is_some();
        }
        if let Some(r) = self.bg_input(&input) {
            return r || tip_before != self.tooltip.is_some();
        }
        if let Some(r) = self.comments_input(&input) {
            return r || tip_before != self.tooltip.is_some();
        }
        let status_top = self.size.1 - self.status_h();
        let in_title = |w: &Self, y: f32| y < w.title.height();
        let in_ribbon = |w: &Self, y: f32| {
            w.view.mode() != ViewMode::Reading && y >= w.ribbon.top() && y < w.ribbon.bottom()
        };
        let redraw = match input {
            Input::Resize { width, height } => {
                self.size = (width.max(1.0), height.max(1.0));
                self.menu = None;
                self.relayout(text);
                true
            }
            Input::Scale(s) => {
                self.scale = s;
                self.menu = None;
                self.view.set_display_scale(s);
                self.relayout(text);
                true
            }
            Input::Tick => {
                let spell = self.view.spell_tick();
                self.tick(text) || spell
            }
            Input::Maximized(m) => self.title.set_maximized(m),
            Input::Leave => {
                let a = self.title.hover(-1.0, -1.0);
                self.ribbon.clear_pointer() || a
            }
            Input::Move { x, y } => {
                let a = self.title.hover(x, y);
                let b = self.controls_hover(x, y);
                self.ribbon.hover(x, y) || a || b
            }
            Input::Zoom(steps) => {
                if steps > 0.0 {
                    self.step_zoom(true)
                } else {
                    self.step_zoom(false)
                }
            }
            Input::Press { x, y, .. } if in_title(self, y) => {
                self.area_press = false;
                self.title.press(x, y)
            }
            Input::Press { x, y, .. } if in_ribbon(self, y) => {
                self.area_press = false;
                self.ribbon.press(x, y)
            }
            Input::Press { x, y, .. } if y >= status_top => {
                self.area_press = false;
                match self.status_hit(x, y) {
                    Some(0) if self.page_master.is_none() => self.action("view.notes"),
                    Some(1) => self.step_zoom(false),
                    Some(2) => {
                        self.zoom_drag = true;
                        let z = self.slider_zoom(x);
                        self.view.set_zoom(Some(z))
                    }
                    Some(3) => self.step_zoom(true),
                    Some(4 | 5) => {
                        self.view.set_zoom(None);
                        true
                    }
                    Some(6) => self.action("view.normal"),
                    Some(7) => self.action("view.sorter"),
                    Some(8) => self.action("view.reading"),
                    Some(9) => self.action("show.fromCurrent"),
                    _ => self.status9_press(x, y),
                }
            }
            Input::Press { x, y, .. } if self.marker_press(x, y) => {
                self.area_press = false;
                true
            }
            Input::Press { x, y, .. } => {
                self.area_press = true;
                let cleared = self.ribbon.clear_pointer();
                match self.view.scrollbar_hit(x, y) {
                    -1 => self.view.go_to(self.view.current().saturating_sub(1)) || cleared,
                    1 => self.view.go_to(self.view.current() + 1) || cleared,
                    _ => self.view.handle(input) || cleared,
                }
            }
            Input::Drag { x, .. } if self.zoom_drag => {
                let z = self.slider_zoom(x);
                self.view.set_zoom(Some(z))
            }
            Input::Drag { .. } if !self.area_press => false,
            Input::Release { x, y } => {
                if self.zoom_drag {
                    self.zoom_drag = false;
                    return false;
                }
                let area = std::mem::take(&mut self.area_press);
                let mut redraw = area && self.view.handle(Input::Release { x, y });
                if let Some(id) = self.title.release(x, y) {
                    redraw |= self.action(id);
                }
                let (fired, changed) = self.ribbon.release(x, y);
                redraw |= match fired {
                    Some(RibbonHit::File) => self.open_file_screen(file10::BsPage::Info),
                    Some(RibbonHit::Command(id)) => self.action(id),
                    Some(RibbonHit::FontBox) => {
                        self.open_menu(MenuKind::Font, RibbonHit::FontBox);
                        true
                    }
                    Some(RibbonHit::SizeBox) => {
                        self.open_menu(MenuKind::Size, RibbonHit::SizeBox);
                        true
                    }
                    _ => false,
                };
                if self.ribbon.needs_layout() {
                    self.relayout(text);
                }
                redraw || changed || fired.is_some()
            }
            Input::Wheel(dy) => {
                // The thumbnails pane scrolls under the pointer's last position is unknown
                // here; the view decides by its focus.
                self.view.handle(Input::Wheel(dy))
            }
            Input::Context { x, y } => self.open_context(x, y),
            Input::Text(_)
            | Input::Key(..)
            | Input::Blink
            | Input::Preedit(_)
            | Input::Drag { .. } => self.view.handle(input),
        };
        redraw || tip_before != self.tooltip.is_some()
    }

    /// Paint the whole window.
    pub fn paint(&self, p: &mut dyn Painter) {
        if self.view.showing() {
            self.view.paint(p, self.text_size);
            if let Some(menu) = &self.menu {
                menu.paint(p, &self.theme, &self.icons);
            }
            return;
        }
        p.clear(self.theme.bg);
        self.view.paint(p, self.text_size);
        self.paint_removal(p);
        self.paint_view_extras(p);
        self.paint_review_extras(p);
        let mut format = ribbon_format(&self.view, self.anim_index().is_some());
        if self.pane == viewtab::Pane::Outline {
            // Outline View dims Ruler, Gridlines, Guides, Notes and Color/Grayscale (reference).
            if let Some(s) = format.style.as_mut() {
                s.push_str("outl ");
            }
        }
        self.title.paint(
            p,
            &self.theme,
            &self.icons,
            &self.strings,
            &self.name,
            self.view.modified(),
            &format,
        );
        let on = self.toggled();
        if self.view.mode() != ViewMode::Reading {
            self.ribbon.paint(
                p,
                &self.theme,
                &self.icons,
                &self.strings,
                &format,
                &[],
                &on,
            );
            self.paint_theme_tiles(p);
            self.paint_galleries(p, &on);
            self.paint_pens(p);
            self.paint_controls(p);
            self.paint_insert(p);
            self.paint_table_design(p);
            self.paint_table_tools(p);
            self.paint_show_checks(p);
            self.paint_view_checks(p);
        }
        self.paint_markers(p);
        self.paint_anim_tags(p);
        self.comments.paint(p);
        self.anim_pane.paint(p);
        self.paint_bg_pane(p);
        self.side.paint(p);
        self.paint_a9(p);
        self.paint_status(p);
        self.paint_status9(p);
        self.paint_gallery(p);
        if let Some(menu) = &self.menu {
            menu.paint(p, &self.theme, &self.icons);
        } else if let Some((label, tw, anchor)) = &self.tooltip {
            paint_tooltip(p, &self.theme, label, *tw, *anchor, self.size.0, self.scale);
        }
        if let Some(b) = &self.backstage {
            b.paint(
                p,
                &self.theme,
                &self.strings,
                &self.name,
                self.view.slide_count(),
                0,
                self.view.modified(),
            );
        }
        self.paint_file10(p);
        if let Some((d, _)) = &self.dialog {
            d.paint(p, &self.theme, self.size, true);
        }
        self.paint_form8(p);
    }

    /// Ribbon commands shown as on: notes, the current slide's transition, the selection's
    /// effect.
    fn toggled(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.view.notes_shown() {
            on.push("view.notes");
        }
        if self.painter.is_some() {
            on.push("anim.painter");
        }
        if self.fmt_painter.is_some() {
            on.push("sl.painter");
        }
        if self.view.format().shadow {
            on.push("sl.shadow");
        }
        if self.view.engine().slide_hide_ink() {
            on.push("sl.hideInk");
        }
        self.view_toggled(&mut on);
        self.picture_toggled(&mut on);
        self.table_tool_toggled(&mut on);
        // The Draw tool in use, the ruler.
        self.draw_toggled(&mut on);
        let Some(st) = self.view.state() else {
            return on;
        };
        let kind = st.transition.as_ref().map_or("none", |t| t.kind.as_str());
        if let Some((id, _)) = sr::TRANSITIONS.iter().find(|(_, k)| *k == kind) {
            on.push(id);
        }
        let ids: Vec<u32> = st.selection.iter().map(|s| s.id).collect();
        if !ids.is_empty() {
            let a = st.animations.iter().find(|a| ids.contains(&a.shape_id));
            let found = sr::EFFECTS.iter().find(|(_, e)| match (e, a) {
                (None, None) => true,
                (Some((effect, class)), Some(a)) => a.effect == *effect && a.class == *class,
                _ => false,
            });
            if let Some((id, _)) = found {
                on.push(id);
            }
        }
        on
    }

    /// The transition and effect galleries drawn as the reference draws them: one white box
    /// with a light frame (y 106-197 at 125 %), 48 px pictures over the names, the current
    /// tile framed dark grey on light grey, and the "more" column at the right edge.
    fn paint_galleries(&self, p: &mut dyn Painter, on: &[&'static str]) {
        let lists: [(Vec<&'static str>, &'static str); 2] = [
            (
                sr::TRANSITIONS
                    .iter()
                    .take(sr::GALLERY_TILES)
                    .map(|(c, _)| *c)
                    .collect(),
                "transition.more",
            ),
            (
                sr::EFFECTS
                    .iter()
                    .take(sr::ANIM_TILES)
                    .map(|(c, _)| *c)
                    .collect(),
                "anim.more",
            ),
        ];
        let px = |v: f32| (v * self.scale).round();
        let rgb = |v: u8| bo_render::Rgba(v, v, v, 255);
        let hovered = self.ribbon.hovered();
        for (tiles, more) in lists {
            let Some(first) = self.ribbon.rect_of(RibbonHit::Command(tiles[0])) else {
                continue;
            };
            let Some(m) = self.ribbon.rect_of(RibbonHit::Command(more)) else {
                continue;
            };
            let top = first[1] + px(1.6);
            let h = px(73.6);
            // Reference at 125 %: white box from x 104 (first slot 106) to the "more"
            // column, which has its own darker frame (x 851-886).
            let left = first[0] - 2.0;
            let right = m[0] + 1.0;
            let gap = px(2.0);
            // Cover what the ribbon drew for these buttons (toggled / hover fills).
            p.fill_rect(left, first[1], m[0] + m[2] - left, first[3], rgb(0xf3));
            p.fill_rect(left, top, right - left, h, rgb(0xd2));
            p.fill_rect(
                left + 1.0,
                top + 1.0,
                right - left - 1.0,
                h - 2.0,
                rgb(0xff),
            );
            let transitions = more == "transition.more";
            let enabled = transitions || !self.view.selection().is_empty();
            for id in &tiles {
                let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                    continue;
                };
                let (x, w) = (r[0], r[2] + gap);
                // Current / hovered band: 4 px inside the box (reference y 110-193).
                let inset = px(3.2);
                if on.contains(id) {
                    p.fill_rect(x, top + inset, w, h - 2.0 * inset, rgb(0x88));
                    p.fill_rect(
                        x + 1.0,
                        top + inset + 1.0,
                        w - 2.0,
                        h - 2.0 * inset - 2.0,
                        rgb(0xd2),
                    );
                } else if hovered == Some(RibbonHit::Command(id)) && enabled {
                    p.fill_rect(x, top + inset, w, h - 2.0 * inset, rgb(0xe6));
                }
                if transitions {
                    let kind = sr::TRANSITIONS
                        .iter()
                        .find(|(c, _)| c == id)
                        .map_or("none", |(_, k)| *k);
                    let tw = (40.0 * self.scale).round();
                    sr::paint_transition_tile(
                        p,
                        kind,
                        (x + (w - tw) / 2.0).round(),
                        top + px(8.8),
                        self.scale,
                        true,
                    );
                } else {
                    // Shaded stars in the class colour; without a selection the reference
                    // greys every star.
                    let effect = sr::EFFECTS
                        .iter()
                        .find(|(c, _)| c == id)
                        .and_then(|(_, e)| *e);
                    let base = match effect {
                        None => bo_render::Rgba(0xa6, 0xa6, 0xa6, 255),
                        Some((_, "emph")) => bo_render::Rgba(0xe8, 0xb3, 0x1e, 255),
                        Some((_, "exit")) => bo_render::Rgba(0xd2, 0x4a, 0x3c, 255),
                        _ => bo_render::Rgba(0x68, 0xa4, 0x90, 255),
                    };
                    let size = px(38.4);
                    sr::paint_star(
                        p,
                        (x + w / 2.0).round(),
                        top + px(4.0) + size / 2.0,
                        size * 0.42,
                        base,
                        enabled,
                    );
                }
                let key = format!("commands.{id}");
                let label = self.strings.get(&key);
                let style = self.control_style();
                let tw = self
                    .gallery_w
                    .iter()
                    .find(|(g, _)| g == id)
                    .map_or(0.0, |(_, w)| *w);
                p.text(
                    label,
                    (x + (w - tw) / 2.0).round(),
                    top + px(45.6),
                    style,
                    if enabled { self.theme.text } else { rgb(0xa6) },
                );
            }
            // "More" column: its own frame (grey 128 at 125 %) with the arrow in the middle.
            let mx = m[0] + 1.0;
            let mw = px(28.8);
            p.fill_rect(mx, top, mw, h, rgb(0x80));
            p.fill_rect(
                mx + 1.0,
                top + 1.0,
                mw - 2.0,
                h - 2.0,
                if hovered == Some(RibbonHit::Command(more)) {
                    rgb(0xe6)
                } else {
                    rgb(0xf3)
                },
            );
            let (cx, cy) = (mx + mw / 2.0, top + h / 2.0);
            let k = px(3.2);
            p.fill_rect(
                (cx - k).round(),
                (cy - k - px(2.4)).round(),
                2.0 * k,
                1.0,
                self.theme.text,
            );
            p.path(
                &[
                    PathSeg::M(cx - k, cy - k / 2.0),
                    PathSeg::L(cx + k, cy - k / 2.0),
                    PathSeg::L(cx, cy + k / 2.0),
                    PathSeg::Z,
                ],
                Some(self.theme.text),
                None,
            );
        }
    }

    /// Theme previews over the Design tab's theme buttons.
    fn paint_theme_tiles(&self, p: &mut dyn Painter) {
        let design = self.view.state().and_then(|s| s.design.as_ref());
        let rect = |id: &'static str| self.ribbon.rect_of(RibbonHit::Command(id));
        let (Some(first), Some(last)) = (rect(sr::CURRENT_THEME), rect(sr::THEMES[4].0)) else {
            return;
        };
        // One white gallery box with a light frame, as the transition gallery (y 106-197).
        let px = |v: f32| (v * self.scale).round();
        let top = first[1] + px(1.6);
        let right = last[0] + last[2];
        let grey = |v: u8| bo_render::Rgba(v, v, v, 255);
        p.fill_rect(
            first[0] - 1.0,
            first[1],
            right - first[0] + 2.0,
            first[3],
            grey(0xf3),
        );
        // The box starts 4 px left of the first slot (reference: x 20, first card 27).
        let left = first[0] - px(3.2);
        p.fill_rect(left, top, right - left + 1.0, px(73.6), grey(0xd2));
        p.fill_rect(
            left + 1.0,
            top + 1.0,
            right - left - 1.0,
            px(73.6) - 2.0,
            grey(0xff),
        );
        // First card: this presentation's theme (its colours and heading font).
        if let Some(d) = design {
            let colors: Vec<bo_render::Rgba> = self.theme_colors().into_iter().take(6).collect();
            let heading = FONT_SETS
                .iter()
                .find(|f| f.name == d.fonts)
                .map_or("Calibri Light", |f| f.major);
            // The reference always marks this card as the current theme.
            sr::paint_theme_tile(p, first, top, self.scale, heading, &colors, true);
        }
        // The gallery's "more" column (frame 128 on 243, as the transition gallery's).
        if let Some(m) = rect("design.more") {
            let mx = m[0] + 1.0;
            let mw = px(28.8);
            let h = px(73.6);
            p.fill_rect(mx, top, mw, h, grey(0x80));
            p.fill_rect(mx + 1.0, top + 1.0, mw - 2.0, h - 2.0, grey(0xf3));
            let (cx, cy) = (mx + mw / 2.0, top + h / 2.0);
            let k = px(3.2);
            p.fill_rect(
                (cx - k).round(),
                (cy - k - px(2.4)).round(),
                2.0 * k,
                1.0,
                self.theme.text,
            );
            p.path(
                &[
                    PathSeg::M(cx - k, cy - k / 2.0),
                    PathSeg::L(cx + k, cy - k / 2.0),
                    PathSeg::L(cx, cy + k / 2.0),
                    PathSeg::Z,
                ],
                Some(self.theme.text),
                None,
            );
        }
        for (id, theme) in sr::THEMES {
            let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                continue;
            };
            let Some(def) = bo_engine::slide_edit::master::THEMES
                .iter()
                .find(|t| t.id == theme)
            else {
                continue;
            };
            let colors: Vec<bo_render::Rgba> = COLOR_SETS
                .iter()
                .find(|c| c.id == def.colors)
                .map(|c| {
                    c.c[4..10]
                        .iter()
                        .map(|v| bo_render::Rgba((v >> 16) as u8, (v >> 8) as u8, *v as u8, 255))
                        .collect()
                })
                .unwrap_or_default();
            let heading = FONT_SETS
                .iter()
                .find(|f| f.id == def.fonts)
                .map_or("Calibri Light", |f| f.major);
            sr::paint_theme_tile(p, r, top, self.scale, heading, &colors, false);
        }
    }

    fn paint_status(&self, p: &mut dyn Painter) {
        let (w, h) = self.size;
        let sh = self.status_h();
        let y = h - sh;
        // Light grey bar under a grey line (reference: 243 under 198).
        p.fill_rect(0.0, y, w, sh, bo_render::Rgba(0xf3, 0xf3, 0xf3, 255));
        p.fill_rect(0.0, y, w, 1.0, bo_render::Rgba(0xc6, 0xc6, 0xc6, 255));
        let style = self.status_style();
        let ink = self.theme.text;
        let ty = (y + 1.0 + (sh - 1.0 - style.size * 1.33) / 2.0).round();
        // A master view names itself instead of counting slides (reference: "Handout
        // Master", "Notes Master", "Slide Master").
        let counter = match self.status_mode_text() {
            Some(t) => t,
            // Outline View names itself, as the reference ("Outline").
            None if self.pane == viewtab::Pane::Outline => {
                self.t("slideNative.statusOutline", "Outline")
            }
            None => self
                .strings
                // Notes Page counts notes pages, as the reference ("Notes 1 of 4").
                .get(if self.pane == viewtab::Pane::NotesPage {
                    "slideNative.notesOf"
                } else {
                    "slideEditor.slideOf"
                })
                // Reading view counts the slide it shows.
                .replace(
                    "{{n}}",
                    &(self.view.show_slide().unwrap_or(self.view.current()) + 1).to_string(),
                )
                .replace("{{total}}", &self.view.slide_count().to_string()),
        };
        p.text(&counter, self.px(11.0), ty, style, ink);
        let [notes, minus, track, plus, pct, fit, normal, ..] = self.status;
        let icon = self.px(16.0);
        let colors = IconColors {
            ink,
            accent: self.theme.accent,
            on: self.theme.accent_on,
            tinted: true,
        };
        // View buttons: the current one on a grey band, as on the reference.
        let current = match self.view.mode() {
            ViewMode::Normal => normal,
            ViewMode::Sorter => self.status[7],
            ViewMode::Reading => self.status[8],
        };
        p.fill_rect(
            current[0],
            current[1],
            current[2],
            current[3],
            bo_render::Rgba(0xc6, 0xc6, 0xc6, 255),
        );
        for (i, name) in [
            "view.normal",
            "view.sorter",
            "view.reading",
            "show.fromStart",
        ]
        .iter()
        .enumerate()
        {
            let r = self.status[6 + i];
            if *name == "show.fromStart" {
                // Slide Show: a projector screen on its stand (own drawing, the
                // reference's meaning; round 19).
                status9::paint_screen(p, r, self.scale, ink);
                continue;
            }
            self.icons.draw(
                p,
                name,
                (r[0] + (r[2] - icon) / 2.0).round(),
                (r[1] + (r[3] - icon) / 2.0).round(),
                icon,
                colors,
            );
        }
        // The notes toggle is a plain icon and label (no band), as on the reference; the
        // sorter has no notes, so no toggle.
        if self.view.mode() == ViewMode::Normal
            && self.page_master.is_none()
            && self.pane == viewtab::Pane::Normal
        {
            self.icons.draw(
                p,
                "format.lineSpacing",
                notes[0] + self.px(4.0),
                (notes[1] + (notes[3] - icon) / 2.0).round(),
                icon,
                colors,
            );
            p.text(
                &self.t("slideEditor.notes", "Notes"),
                notes[0] + self.px(24.0),
                ty,
                style,
                ink,
            );
        }
        let k = (4.0 * self.scale).round();
        let line = self.scale.max(1.0).round();
        let (mx, my) = (minus[0] + minus[2] / 2.0, minus[1] + minus[3] / 2.0);
        p.fill_rect((mx - k).round(), my.round(), 2.0 * k, line, ink);
        let (qx, qy) = (plus[0] + plus[2] / 2.0, plus[1] + plus[3] / 2.0);
        p.fill_rect((qx - k).round(), qy.round(), 2.0 * k, line, ink);
        p.fill_rect(qx.round(), (qy - k).round(), line, 2.0 * k, ink);
        let ty2 = (track[1] + track[3] / 2.0).round();
        // Track and 100 % tick in the text colour (reference: 38).
        let groove = bo_render::Rgba(0x26, 0x26, 0x26, 255);
        p.fill_rect(track[0], ty2, track[2], line, groove);
        p.fill_rect(
            (track[0] + track[2] / 2.0).round(),
            ty2 - k / 2.0,
            line,
            k + line,
            groove,
        );
        let thumb = self.slider_pos();
        p.fill_rect(
            (thumb - k / 2.0).round(),
            ty2 - k - line,
            k,
            2.0 * k + 3.0 * line,
            ink,
        );
        // The sorter shows its own zoom (its images at 100 %).
        let zoom = if self.view.mode() == ViewMode::Sorter {
            "100%".to_owned()
        } else if self.pane == viewtab::Pane::NotesPage {
            format!("{}%", (self.notes_page_zoom() * 100.0).round() as i32)
        } else {
            format!("{}%", (self.view.zoom() * 100.0).round() as i32)
        };
        p.text(&zoom, pct[0], ty, style, ink);
        // Fit to window: a frame with arrows out to its corners (own drawing).
        let (cx, cy) = (fit[0] + fit[2] / 2.0, fit[1] + fit[3] / 2.0);
        let r = self.px(6.0);
        let segs = [
            PathSeg::M(cx - r, cy - r),
            PathSeg::L(cx + r, cy - r),
            PathSeg::L(cx + r, cy + r),
            PathSeg::L(cx - r, cy + r),
            PathSeg::Z,
        ];
        p.path(&segs, None, Some((ink, line)));
        // Four small arrowheads at the middle of the sides pointing out: fit the slide to
        // the frame (round 19, the reference's meaning in own shapes).
        let a = self.px(2.0);
        let e = r - self.px(1.5);
        for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
            let (tx, ty) = (cx + dx * e, cy + dy * e);
            let (bx, by) = (cx + dx * (e - 2.0 * a), cy + dy * (e - 2.0 * a));
            p.path(
                &[
                    PathSeg::M(tx, ty),
                    PathSeg::L(bx - dy * a, by - dx * a),
                    PathSeg::L(bx + dy * a, by + dx * a),
                    PathSeg::Z,
                ],
                Some(ink),
                None,
            );
        }
    }

    /// The host's clock (ms) before a frame: transitions and effects advance with it;
    /// `true` when the frame changed.
    pub fn set_clock(&mut self, ms: f64) -> bool {
        self.view.set_clock(ms)
    }

    /// A transition or effects are playing: the host draws every frame.
    pub fn animating(&self) -> bool {
        self.view.animating()
    }

    /// Whether the slide show is running (the host shows the window full screen).
    pub fn showing(&self) -> bool {
        self.view.showing()
    }

    /// Run a command id as the ribbon would (tests and automation).
    pub fn run_action(&mut self, id: &str) -> bool {
        self.action(id)
    }

    /// Ribbon rectangle of command `id`.
    pub fn command_rect(&self, id: &'static str) -> Option<[f32; 4]> {
        self.ribbon.rect_of(RibbonHit::Command(id))
    }
}

#[cfg(test)]
#[path = "slide_window_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "slide_round17_tests.rs"]
mod round17_tests;

#[cfg(test)]
#[path = "slide_round19_tests.rs"]
mod round19_tests;

#[cfg(test)]
#[path = "slide_round20_tests.rs"]
mod round20_tests;

#[cfg(test)]
#[path = "slide_round7_tests.rs"]
mod round7_tests;

#[cfg(test)]
#[path = "slide_round8_tests.rs"]
mod round8_tests;

#[cfg(test)]
#[path = "slide_round5_tests.rs"]
mod round5_tests;
