//! The BlueSlide window (N6): title bar, ribbon, the editing area of [`SlideView`] and the
//! status bar, with input routing. Laid out like the reference presentation app (thumbnails
//! on the left, notes under the slide, slide counter, notes toggle and zoom in the status
//! bar), drawn with BlueDoc's chrome (title bar, ribbon, menus, File screen).
use crate::backstage::{Backstage, FileAction};
use crate::icons::{IconColors, Icons};
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, Ribbon, RibbonHit, Tab};
use crate::slide_comments::{now_iso, CommentsPane, PaneEdit, PaneHit};
use crate::slide_ribbon as sr;
use crate::slide_view::{Focus, SlideView, ViewMode};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{Input, Key, Mods, PageStore, Painter, Request, TextMeasure, TextStyle};
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
    use Item::{Button as B, FontBox, Labeled as S, Large as L, SizeBox};
    Tab {
        label: "ribbon.tabs.home",
        groups: vec![
            Group {
                label: "ribbon.groups.clipboard",
                large: vec![L("edit.paste")],
                rows: vec![vec![B("edit.cut")], vec![B("edit.copy")]],
            },
            Group {
                label: "slideEditor.groupSlides",
                large: vec![L("slide.new")],
                rows: vec![
                    vec![S("slide.layout")],
                    vec![S("slide.duplicate")],
                    vec![S("slide.delete")],
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
                    ],
                    vec![
                        B("format.bold"),
                        B("format.italic"),
                        B("format.underline"),
                        B("format.strike"),
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
                label: "slideEditor.groupDrawing",
                large: vec![L("insert.textBox")],
                rows: vec![
                    vec![
                        B("insert.rect"),
                        B("insert.ellipse"),
                        B("insert.triangle"),
                        B("insert.arrow"),
                    ],
                    vec![
                        B("arrange.front"),
                        B("arrange.back"),
                        B("arrange.group"),
                        B("arrange.ungroup"),
                    ],
                ],
            },
            Group {
                label: "ribbon.groups.editing",
                large: vec![],
                rows: vec![vec![S("edit.selectAll")]],
            },
        ],
    }
}

fn insert_tab() -> Tab {
    use Item::{Button as B, Large as L};
    Tab {
        label: "ribbon.tabs.insert",
        groups: vec![
            Group {
                label: "slideEditor.groupSlides",
                large: vec![L("slide.new")],
                rows: vec![],
            },
            Group {
                label: "slideEditor.groupShapes",
                large: vec![L("insert.textBox")],
                rows: vec![
                    vec![B("insert.rect"), B("insert.ellipse")],
                    vec![B("insert.triangle"), B("insert.arrow")],
                ],
            },
        ],
    }
}

fn show_tab() -> Tab {
    use Item::Large as L;
    Tab {
        label: "slideEditor.showTab",
        groups: vec![Group {
            label: "slideEditor.groupShow",
            large: vec![L("show.fromStart"), L("show.fromCurrent")],
            rows: vec![],
        }],
    }
}

fn view_tab() -> Tab {
    use Item::{Labeled as S, Large as L};
    Tab {
        label: "slideEditor.viewTab",
        groups: vec![
            Group {
                label: "ribbon.groups.show",
                large: vec![L("view.notes")],
                rows: vec![],
            },
            Group {
                label: "ribbon.groups.zoom",
                large: vec![L("view.fitSlide")],
                rows: vec![vec![S("view.zoomIn")], vec![S("view.zoomOut")]],
            },
        ],
    }
}

/// The slide formatting as the ribbon's format state (toggles, font and size boxes, undo).
fn ribbon_format(view: &SlideView) -> bo_edit::FormatState {
    let f = view.format();
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
        ..bo_edit::FormatState::default()
    }
}

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
    AnimMore,
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

fn slide_enabled(id: &str, f: &bo_edit::FormatState) -> bool {
    crate::commands::enabled(id, f) || sr::is_command(id)
}

impl SlideWindow {
    /// Wrap the presentation `name` shown by `view` with BlueSlide's interface in `locale`.
    pub fn new(mut view: SlideView, locale: &str, name: &str) -> Self {
        let mut strings = Strings::load(locale);
        sr::install_strings(&mut strings);
        let mut icons = Icons::load();
        sr::install_icons(&mut icons);
        view.set_labels(
            strings.get("slideEditor.notesPlaceholder"),
            strings.get("slideEditor.end"),
        );
        view.set_prompts(
            strings.get("slideEditor.promptTitle"),
            strings.get("slideEditor.promptSubtitle"),
            strings.get("slideEditor.promptText"),
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
            gallery_w: Vec::new(),
            ribbon: Ribbon::with_tabs(vec![
                home(),
                insert_tab(),
                sr::design_tab(),
                sr::transitions_tab(),
                sr::animations_tab(),
                show_tab(),
                sr::review_tab(),
                view_tab(),
            ])
            .with_band_extra(RIBBON_EXTRA)
            .with_large_widths(sr::large_width)
            .with_enabled(slide_enabled),
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
        }
    }

    /// The editing area.
    pub fn view(&self) -> &SlideView {
        &self.view
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
        self.view.prepare(store)
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
        let pane = if self.comments.shown && self.view.mode() != ViewMode::Reading {
            CommentsPane::width(self.scale)
        } else {
            0.0
        };
        let area_h = (self.size.1 - top - sh).max(1.0);
        self.view
            .set_frame(0.0, top, (self.size.0 - pane).max(1.0), area_h);
        self.comments
            .layout([self.size.0 - pane, top, pane, area_h], self.scale);
        self.view.set_window(self.size.0, self.size.1);
        if let Some(b) = self.backstage.as_mut() {
            b.layout(self.title.height(), self.size, self.scale);
        }
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
        let style = self.status_style();
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
        let redraw = self.route(input, text);
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
            ];
            self.comments
                .record(text, &threads, &labels, self.theme.text);
        }
        self.view.measure_notes(text, self.text_size);
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
            "review.comments" => {
                self.comments.shown = !self.comments.shown;
                self.comments.draft = None;
                self.relayout_pending = true;
                true
            }
            "review.newComment" => {
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
                    "design.fonts" => (MenuKind::Fonts, "design.fonts"),
                    "design.slideSize" => (MenuKind::SlideSize, "design.slideSize"),
                    "design.background" => (MenuKind::Background, "design.background"),
                    "transition.effect" => (MenuKind::TransitionEffect, "transition.effect"),
                    "transition.duration" => (MenuKind::Duration, "transition.duration"),
                    "transition.more" => (MenuKind::TransitionMore, "transition.more"),
                    "anim.more" => (MenuKind::AnimMore, "anim.more"),
                    _ => (MenuKind::AnimEffect, "anim.effect"),
                };
                self.open_menu(kind, RibbonHit::Command(anchor));
                true
            }
            "transition.applyAll" => self.set_transition(None, None, None, true),
            "transition.preview" => self.view.start_preview(false),
            "anim.preview" => self.view.start_preview(true),
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
                        true
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
            PaneEdit::Post(text) => {
                let author = std::env::var("USERNAME")
                    .or_else(|_| std::env::var("USER"))
                    .unwrap_or_else(|_| "BlueSlide".to_owned());
                self.view.run(SlideCommand::AddComment {
                    text,
                    author,
                    date: now_iso(),
                    parent: None,
                    mentions: Vec::new(),
                    task: None,
                })
            }
        })
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
        let token = format!("BlueSlide/{:p}/{}", &raw const *self, self.clip_seq);
        self.clip_token = Some(token.clone());
        self.requests.push(Request::SetClipboard {
            text,
            html,
            internal: token,
        });
        true
    }

    /// Paste what the system clipboard holds: this window's own copy from the engine's
    /// clipboard, else another program's text as a new text box (as the reference does).
    pub fn paste(
        &mut self,
        internal: Option<String>,
        _html: Option<String>,
        text: Option<String>,
    ) -> bool {
        let ours = internal.is_some_and(|i| Some(&i) == self.clip_token.as_ref());
        let text = text.unwrap_or_default();
        if ours || text.trim().is_empty() {
            return self.view.run(SlideCommand::Paste { keep_source: false });
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
        let anchor = self.ribbon.rect_of(hit).unwrap_or([0.0, 0.0, 1.0, 1.0]);
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
            MenuKind::TransitionMore | MenuKind::AnimMore => Vec::new(),
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
            MenuKind::AnimMore => gallery(&mut sr::EFFECTS.iter().map(|(c, _)| *c)),
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
                    // The Layout menu adds a slide with the chosen layout, as New Slide's
                    // menu does.
                    Some(layout) if self.menu_kind == MenuKind::Layout => {
                        self.view.new_slide(Some(layout))
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
            "tdir" => self.set_transition(None, Some(v.to_owned()), None, false),
            "tdur" => self.set_transition(None, None, v.parse().ok(), false),
            "adir" => self.apply_effect(None, Some(v.to_owned())),
            _ => return None,
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
                Input::Release { .. } | Input::Move { .. } | Input::Blink => false,
                other => self.view.handle(other),
            };
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
        if self.menu.is_some() {
            if let Some(r) = self.menu_input(&input) {
                return r;
            }
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
            Input::Tick => self.tick(text),
            Input::Maximized(m) => self.title.set_maximized(m),
            Input::Leave => {
                let a = self.title.hover(-1.0, -1.0);
                self.ribbon.clear_pointer() || a
            }
            Input::Move { x, y } => {
                let a = self.title.hover(x, y);
                self.ribbon.hover(x, y) || a
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
                    Some(0) => self.action("view.notes"),
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
                    _ => false,
                }
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
                    Some(RibbonHit::File) => {
                        let mut b = Backstage::default();
                        b.layout(self.title.height(), self.size, self.scale);
                        self.backstage = Some(b);
                        true
                    }
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
            Input::Context { .. } => false,
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
            return;
        }
        p.clear(self.theme.bg);
        self.view.paint(p, self.text_size);
        let format = ribbon_format(&self.view);
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
        }
        self.comments.paint(p);
        self.paint_status(p);
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
    }

    /// Ribbon commands shown as on: notes, the current slide's transition, the selection's
    /// effect.
    fn toggled(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.view.notes_shown() {
            on.push("view.notes");
        }
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
                    .take(sr::GALLERY_TILES)
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
            let right = m[0] + px(27.2);
            // Cover what the ribbon drew for these buttons (toggled / hover fills).
            p.fill_rect(
                first[0] - 1.0,
                first[1],
                right - first[0] + 2.0,
                first[3],
                rgb(0xf3),
            );
            p.fill_rect(first[0] - 1.0, top, right - first[0] + 1.0, h, rgb(0xd2));
            p.fill_rect(
                first[0],
                top + 1.0,
                right - first[0] - 1.0,
                h - 2.0,
                rgb(0xff),
            );
            for id in &tiles {
                let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
                    continue;
                };
                let (x, w) = (r[0], r[2]);
                if on.contains(id) {
                    p.fill_rect(x, top + 1.0, w, h - 2.0, rgb(0x88));
                    p.fill_rect(x + 1.0, top + 2.0, w - 2.0, h - 4.0, rgb(0xd2));
                } else if hovered == Some(RibbonHit::Command(id)) {
                    p.fill_rect(x, top + 1.0, w, h - 2.0, rgb(0xe6));
                }
                let size = px(38.4);
                self.icons.draw(
                    p,
                    id,
                    (x + (w - size) / 2.0).round(),
                    top + px(4.0),
                    size,
                    IconColors {
                        ink: self.theme.text,
                        accent: self.theme.accent,
                        on: self.theme.accent_on,
                        tinted: true,
                    },
                );
                let key = format!("commands.{id}");
                let label = self.strings.get(&key);
                let style = self.status_style();
                let tw = self
                    .gallery_w
                    .iter()
                    .find(|(g, _)| g == id)
                    .map_or(0.0, |(_, w)| *w);
                p.text(
                    label,
                    (x + (w - tw) / 2.0).round(),
                    top + px(46.4),
                    style,
                    self.theme.text,
                );
            }
            // "More" column: its own frame with the arrow in the middle.
            let mx = m[0];
            let mw = right - mx;
            p.fill_rect(mx, top, 1.0, h, rgb(0xd2));
            if hovered == Some(RibbonHit::Command(more)) {
                p.fill_rect(mx + 1.0, top + 1.0, mw - 2.0, h - 2.0, rgb(0xe6));
            }
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
        let (Some(first), Some(last)) = (rect(sr::THEMES[0].0), rect(sr::THEMES[4].0)) else {
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
            let current = design.is_some_and(|d| d.theme.as_deref() == Some(def.name));
            sr::paint_theme_tile(p, r, top, self.scale, heading, &colors, current);
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
        let counter = self
            .strings
            .get("slideEditor.slideOf")
            // Reading view counts the slide it shows.
            .replace(
                "{{n}}",
                &(self.view.show_slide().unwrap_or(self.view.current()) + 1).to_string(),
            )
            .replace("{{total}}", &self.view.slide_count().to_string());
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
        if self.view.mode() == ViewMode::Normal {
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
            PathSeg::M(cx - r + self.px(3.0), cy),
            PathSeg::L(cx + r - self.px(3.0), cy),
            PathSeg::M(cx, cy - r + self.px(3.0)),
            PathSeg::L(cx, cy + r - self.px(3.0)),
        ];
        p.path(&segs, None, Some((ink, line)));
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
