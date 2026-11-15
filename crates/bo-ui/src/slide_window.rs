//! The BlueSlide window (N6): title bar, ribbon, the editing area of [`SlideView`] and the
//! status bar, with input routing. Laid out like the reference presentation app (thumbnails
//! on the left, notes under the slide, slide counter, notes toggle and zoom in the status
//! bar), drawn with BlueDoc's chrome (title bar, ribbon, menus, File screen).
use crate::backstage::{Backstage, FileAction};
use crate::icons::{IconColors, Icons};
use crate::popup::{paint_tooltip, Choice, Menu, MenuItem};
use crate::ribbon::{Group, Item, Ribbon, RibbonHit, Tab};
use crate::slide_view::{Focus, SlideView};
use crate::strings::Strings;
use crate::theme::Theme;
use crate::titlebar::{NcHit, TitleBar};
use crate::{Input, Key, Mods, PageStore, Painter, Request, TextMeasure, TextStyle};
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
}

impl SlideWindow {
    /// Wrap the presentation `name` shown by `view` with BlueSlide's interface in `locale`.
    pub fn new(mut view: SlideView, locale: &str, name: &str) -> Self {
        let strings = Strings::load(locale);
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
            ribbon: Ribbon::with_tabs(vec![home(), insert_tab(), show_tab(), view_tab()]),
            theme: Theme::light("slide"),
            icons: Icons::load(),
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
        // Reference: line 34 device pixels above the client bottom at 125 %.
        (27.2 * self.scale).round()
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
        let top = self.ribbon.bottom();
        let sh = self.status_h();
        self.view
            .set_frame(0.0, top, self.size.0, (self.size.1 - top - sh).max(1.0));
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
        let redraw = self.route(input, text);
        if showing != self.view.showing() {
            self.requests.push(Request::FullScreen(self.view.showing()));
        }
        if self.relayout_pending {
            self.relayout_pending = false;
            self.relayout(text);
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
            "edit.copy" => self.view.run(SlideCommand::Copy),
            "edit.cut" => self.view.run(SlideCommand::Cut),
            "edit.paste" => self.view.run(SlideCommand::Paste { keep_source: false }),
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
            _ => self.view.action(id),
        }
    }

    fn open_menu(&mut self, kind: MenuKind, hit: RibbonHit) {
        let anchor = self.ribbon.rect_of(hit).unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let f = self.view.format();
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
        };
        self.menu_kind = kind;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
    }

    fn choose(&mut self, choice: Choice) -> bool {
        match choice {
            Choice::Font(typeface) => self.view.format_text(TextFormat::Font { typeface }),
            Choice::Size(half) => self.view.format_text(TextFormat::Size {
                points: f64::from(half) / 2.0,
            }),
            Choice::Command(c) => match c.strip_prefix("layout:").and_then(|i| i.parse().ok()) {
                // The Layout menu adds a slide with the chosen layout, as New Slide's menu does.
                Some(layout) if self.menu_kind == MenuKind::Layout => {
                    self.view.new_slide(Some(layout))
                }
                _ => self.action(&c),
            },
            Choice::Style(_) => false,
        }
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
        let name = self.strings.get(&format!("commands.{id}")).to_owned();
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
                Input::Release { .. } | Input::Move { .. } | Input::Tick | Input::Blink => false,
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
        let status_top = self.size.1 - self.status_h();
        let in_title = |w: &Self, y: f32| y < w.title.height();
        let in_ribbon = |w: &Self, y: f32| y >= w.ribbon.top() && y < w.ribbon.bottom();
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
                    // Normal view is the only one so far; the last button presents.
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
        let on: Vec<&str> = if self.view.notes_shown() {
            vec!["view.notes"]
        } else {
            vec![]
        };
        self.ribbon.paint(
            p,
            &self.theme,
            &self.icons,
            &self.strings,
            &format,
            &[],
            &on,
        );
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
            .replace("{{n}}", &(self.view.current() + 1).to_string())
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
        // View buttons: the current one (normal) on a grey band, as on the reference.
        p.fill_rect(
            normal[0],
            normal[1],
            normal[2],
            normal[3],
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
        // The notes toggle is a plain icon and label (no band), as on the reference.
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
        let zoom = format!("{}%", (self.view.zoom() * 100.0).round() as i32);
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
