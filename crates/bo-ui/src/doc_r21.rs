//! BlueDoc parity round 21 (docs/ui/REPORT.md "Vòng 111"), the window's side of File ›
//! Options › Customize Ribbon / Quick Access Toolbar (`doc_options21`, model
//! `doc_ribbon21`): the customised tabs laid into the ribbon (with their scaling), names of
//! renamed / custom tabs and groups, the Quick Access Toolbar's commands and position, and
//! the customisation file's import / export through the host. A child module of the shell.
use super::{DocWindow, Request};
use crate::doc_fmtpane21 as fp;
use crate::doc_forms::options6::r21::Out;
use crate::doc_forms::options6::DocPrefs;
use crate::doc_ribbon21 as rb;
use crate::popup::{Choice, Menu, MenuItem};
use std::collections::HashMap;

/// Round-21 state of the window.
#[derive(Default)]
pub(super) struct R21 {
    /// Strings replaced by names the user gave built-in tabs and groups (original text).
    originals: HashMap<String, String>,
    /// The customisation shown (to skip rebuilding an unchanged ribbon).
    shown: Option<(rb::Layout, Vec<String>)>,
    /// Protected View: the file came from the Internet (Mark of the Web) and opens
    /// read-only until Enable Editing.
    protected: bool,
    /// The Enable Editing button of the Protected View bar (window pixels).
    enable_rect: [f32; 4],
    /// The Format Shape / Format Picture pane.
    fmt: Option<fp::FmtPane>,
}

impl DocWindow {
    /// Make the ribbon customisation and Quick Access Toolbar position of `p` take effect.
    pub(super) fn r21_apply(&mut self, p: &DocPrefs) {
        let mut layout = p.extra.ribbon.full();
        for t in &mut layout.tabs {
            if p.extra.hidden_tabs.contains(&t.id) {
                t.shown = false;
            }
        }
        self.r21_set_below(p.extra.qat_below);
        let key = (layout.clone(), p.extra.hidden_tabs.clone());
        if self.r21.shown.as_ref() == Some(&key) {
            return;
        }
        let first = self.r21.shown.is_none();
        self.r21.shown = Some(key);
        if first && layout.is_default() {
            // Nothing customised: BlueDoc's own ribbon stays as built.
            return;
        }
        rb::install(&layout);
        // Names: restore what an earlier customisation replaced, then set the new ones.
        for (k, v) in std::mem::take(&mut self.r21.originals) {
            self.strings.set(&k, &v);
        }
        for (k, v) in rb::names(&layout) {
            if !k.starts_with(rb::TAB_PREFIX)
                && !k.starts_with(rb::GROUP_PREFIX)
                && !k.starts_with("commands.c21b.")
            {
                let old = self.strings.get(&k).to_owned();
                self.r21.originals.entry(k.clone()).or_insert(old);
            }
            self.strings.set(&k, &v);
        }
        // Collapsed custom groups show their first command's icon.
        for (button, _, _, of) in rb::group_buttons() {
            let of = crate::ribbon::icon_of(of);
            if self.icons.has(of) {
                self.icons.alias(button, of);
            }
        }
        self.ribbon.set_scaling(rb::scaling, rb::shrink, rb::holder);
        let tabs = rb::build_main(&layout);
        if !tabs.is_empty() {
            self.ribbon.set_fixed_tabs(tabs);
        }
        // The contextual tabs shown now, as customised.
        let tools = rb::build_tools(super::doc_tools::tabs(self.context));
        self.ribbon.set_contextual(tools, false);
        self.relayout_pending = true;
    }

    /// The Quick Access Toolbar above (title bar) or below the ribbon.
    fn r21_set_below(&mut self, below: bool) {
        if self.ribbon.qat_below() != below {
            self.ribbon.set_qat_below(below);
            self.relayout_pending = true;
        }
    }

    /// Place the quick access buttons below the ribbon when the option asks for it (and the
    /// ribbon is shown), else in the title bar.
    pub(super) fn r21_place_quick(&mut self) {
        let strip = self.ribbon.qat_strip_h();
        let below = (strip > 0.0 && self.read_mode.is_none() && !self.r4.focus)
            .then(|| (self.ribbon.bottom() - strip, strip));
        self.title.set_quick_below(below);
    }

    /// What the Options dialog asked for.
    pub(super) fn r21_options_out(&mut self, out: Out) -> bool {
        match out {
            Out::Import => {
                self.requests.push(Request::PickFile {
                    purpose: "ribbon21".into(),
                    filter: "customization".into(),
                });
                false
            }
            Out::Export(text) => {
                self.requests.push(Request::SaveText {
                    name: "BlueDoc Customizations.bocustom".into(),
                    filter: "customization".into(),
                    text,
                });
                false
            }
            Out::Keyboard => {
                // Customize Keyboard (round 17's dialog) over the Options dialog.
                self.r3_open_form(crate::doc_forms::DocFormKind::CustomizeKeyboard)
            }
        }
    }

    /// A customisation file the host read for Import.
    pub(super) fn r21_file_picked(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes);
        if let Some(o) = self.r6.options.as_mut() {
            return o.r21_import(&text);
        }
        // Without the dialog (scripted): apply it at once.
        let Some(f) = rb::import(&text) else {
            return false;
        };
        let mut p = self.r6.prefs.clone();
        p.extra.hidden_tabs = f.ribbon.hidden_main();
        p.extra.ribbon = f.ribbon;
        p.quick = f.qat;
        p.extra.qat_below = f.qat_below;
        self.r6_set_quick(&p.quick);
        self.r21_apply(&p);
        self.r6_store_prefs(p);
        true
    }
}

/// Keys that only move or copy (allowed in Protected View).
fn reading_key(input: &crate::Input) -> bool {
    use crate::Key;
    match input {
        crate::Input::Key(k, m) => match k {
            Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
            | Key::Escape
            | Key::F(_) => true,
            Key::Letter(c) => m.ctrl && matches!(c, 'c' | 'a' | 'f' | 'p' | 'o' | 'n' | 'w'),
            _ => false,
        },
        _ => true,
    }
}

/// Commands that change nothing in the document (allowed in Protected View).
fn reading_command(id: &str) -> bool {
    id.starts_with("file.")
        || id.starts_with("view.")
        || id.starts_with("bs6")
        || id.starts_with("file:")
        || id.starts_with("options:")
        || matches!(
            id,
            "edit.copy"
                | "edit.find"
                | "edit.selectAll"
                | "review.wordCount"
                | "review.next"
                | "review.previous"
        )
}

impl DocWindow {
    /// The host's Mark of the Web for the opened file (`ZoneId` of its `Zone.Identifier`
    /// stream: 3 Internet, 4 Restricted; 0 when there is none): files from the Internet
    /// open in Protected View when File › Options › Trust Center asks for it.
    pub fn set_download_zone(&mut self, zone: u32) {
        let on = self.r6.prefs.extra.protected(zone, false);
        if on != self.r21.protected {
            self.r21.protected = on;
            self.relayout_pending = true;
        }
    }

    /// Whether the document is in Protected View.
    pub fn protected_view(&self) -> bool {
        self.r21.protected
    }

    /// Height of the Protected View bar below the ribbon (0 when not protected).
    pub(super) fn r21_bar_h(&self) -> f32 {
        if self.r21.protected && self.read_mode.is_none() {
            (40.0 * self.scale).round()
        } else {
            0.0
        }
    }

    /// Input while protected: Enable Editing on the bar, editing keys and text ignored.
    pub(super) fn r21_input(&mut self, input: &crate::Input) -> Option<bool> {
        if self.menu.is_none() {
            if let Some(r) = self.r21_pane_input(input) {
                return Some(r);
            }
        }
        if !self.r21.protected {
            return None;
        }
        match input {
            crate::Input::Press { x, y, .. } => {
                let r = self.r21.enable_rect;
                if *x >= r[0] && *x < r[0] + r[2] && *y >= r[1] && *y < r[1] + r[3] {
                    self.r21.protected = false;
                    self.relayout_pending = true;
                    return Some(true);
                }
                let top = self.ribbon.bottom();
                (*y >= top && *y < top + self.r21_bar_h()).then_some(false)
            }
            crate::Input::Text(_) | crate::Input::Context { .. } => Some(false),
            k @ crate::Input::Key(..) if !reading_key(k) => Some(false),
            _ => None,
        }
    }

    /// A command while protected: only those that change nothing run.
    pub(super) fn r21_guard(&mut self, id: &str) -> Option<bool> {
        if self.r21.protected && !reading_command(id) {
            return Some(false);
        }
        self.r21_pane_action(id)
    }

    /// Paint the Protected View bar (light yellow, as the reference's message bar).
    pub(super) fn r21_paint_bar(&self, p: &mut dyn crate::Painter) {
        use bo_render::{PathSeg, Rgba};
        let h = self.r21_bar_h();
        if h <= 0.0 {
            return;
        }
        let k = self.scale;
        let y = self.ribbon.bottom();
        let w = self.size.0;
        p.fill_rect(0.0, y, w, h, Rgba(0xFF, 0xF4, 0xCE, 255));
        p.fill_rect(0.0, y + h - 1.0, w, 1.0, Rgba(0xE1, 0xC8, 0x7A, 255));
        let size = (12.0 * k).round();
        let line = size * 1.33;
        let ty = (y + (h - line) / 2.0).round();
        let ink = Rgba(0x26, 0x26, 0x26, 255);
        // Shield mark (own drawing).
        let (sx, sy) = ((16.0 * k).round(), y + h / 2.0);
        p.path(
            &[
                PathSeg::M(sx, sy - 8.0 * k),
                PathSeg::L(sx + 7.0 * k, sy - 5.0 * k),
                PathSeg::L(sx + 6.0 * k, sy + 3.0 * k),
                PathSeg::L(sx, sy + 8.0 * k),
                PathSeg::L(sx - 6.0 * k, sy + 3.0 * k),
                PathSeg::L(sx - 7.0 * k, sy - 5.0 * k),
                PathSeg::Z,
            ],
            Some(Rgba(0xC1, 0x8C, 0x00, 255)),
            None,
        );
        let title = tr(&self.strings, "doc21.pvTitle", "PROTECTED VIEW");
        let body = tr(
            &self.strings,
            "doc21.pvBody",
            "Be careful \u{2014} files from the Internet can contain viruses. Unless you need to edit, it's safer to stay in Protected View.",
        );
        let x0 = (32.0 * k).round();
        p.text(&title, x0, ty, crate::TextStyle { size, bold: true }, ink);
        let tw = title.chars().count() as f32 * size * 0.66;
        p.text(
            &body,
            (x0 + tw + 14.0 * k).round(),
            ty,
            crate::TextStyle { size, bold: false },
            ink,
        );
        let r = self.r21.enable_rect;
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x8A, 0x8A, 0x8A, 255));
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(0xFD, 0xFD, 0xFD, 255),
        );
        let label = tr(&self.strings, "doc21.enableEditing", "Enable Editing");
        let lw = label.chars().count() as f32 * size * 0.52;
        p.text(
            &label,
            (r[0] + (r[2] - lw) / 2.0).round(),
            (r[1] + (r[3] - line) / 2.0).round(),
            crate::TextStyle { size, bold: false },
            ink,
        );
    }

    /// Place the Enable Editing button (at layout).
    pub(super) fn r21_layout_bar(&mut self) {
        let h = self.r21_bar_h();
        let k = self.scale;
        let (bw, bh) = ((120.0 * k).round(), (28.0 * k).round());
        let y = self.ribbon.bottom();
        self.r21.enable_rect = if h > 0.0 {
            [
                (self.size.0 - bw - 40.0 * k).round(),
                (y + (h - bh) / 2.0).round(),
                bw,
                bh,
            ]
        } else {
            [0.0; 4]
        };
    }
}

/// Standard colours of the pane's colour drop-downs (hex, key, English): the round-18 /
/// round-19 keys of the ribbon's colour menus.
const PANE_COLORS: [(&str, &str, &str); 12] = [
    ("000000", "doc19.cBlack", "Black"),
    ("FFFFFF", "doc19.cWhite", "White"),
    ("C00000", "doc18.cDarkRed", "Dark Red"),
    ("FF0000", "doc18.cRed", "Red"),
    ("FFC000", "doc18.cOrange", "Orange"),
    ("FFFF00", "doc18.cYellow", "Yellow"),
    ("92D050", "doc18.cLightGreen", "Light Green"),
    ("00B050", "doc18.cGreen", "Green"),
    ("00B0F0", "doc18.cLightBlue", "Light Blue"),
    ("0070C0", "doc18.cBlue", "Blue"),
    ("002060", "doc18.cDarkBlue", "Dark Blue"),
    ("7030A0", "doc18.cPurple", "Purple"),
];

impl DocWindow {
    /// The selected shape or picture and what the pane formats.
    fn r21_selected(&self) -> Option<(fp::Target, bo_model::Image)> {
        let s = self.view.engine().session()?;
        if let Some(img) = s.tools_shape() {
            return Some((fp::Target::Shape, img));
        }
        s.tools_picture().map(|img| (fp::Target::Picture, img))
    }

    /// Open the Format Shape / Format Picture pane on `page` with `section` expanded
    /// (`text`: the Text Options tab).
    pub(super) fn r21_open_pane(
        &mut self,
        page: fp::Page,
        section: Option<&'static str>,
        text: bool,
    ) -> bool {
        let Some((target, img)) = self.r21_selected() else {
            return false;
        };
        let page = if target == fp::Target::Picture && page == fp::Page::Layout {
            fp::Page::Picture
        } else {
            page
        };
        let mut pane = fp::FmtPane::new(target, page, section);
        pane.text_tab = text && target == fp::Target::Shape;
        pane.vals = fp::Vals::of(&img);
        self.menu = None;
        self.r21.fmt = Some(pane);
        self.relayout_pending = true;
        true
    }

    /// Width the pane takes at the right.
    pub(super) fn r21_pane_w(&self) -> f32 {
        self.r21
            .fmt
            .as_ref()
            .map_or(0.0, |p| p.pane_width(self.scale))
    }

    /// Place the pane (at layout): the leftmost of the panes at the right.
    pub(super) fn r21_layout_pane(&mut self) {
        if self.r21.fmt.is_none() {
            return;
        }
        let x = self.doc_right();
        let y = self.chrome_bottom();
        let h = (self.size.1 - self.status_h() - y).max(1.0);
        let scale = self.scale;
        let w = self.r21_pane_w();
        if let Some(p) = self.r21.fmt.as_mut() {
            p.layout([x, y, w, h], scale);
        }
    }

    /// Re-read the selection into the pane after each input (the pane closes when nothing
    /// it can format is selected).
    pub(super) fn r21_sync(&mut self) {
        if self.r21.fmt.is_none() {
            return;
        }
        let Some((target, img)) = self.r21_selected() else {
            self.r21.fmt = None;
            self.relayout_pending = true;
            return;
        };
        let vals = fp::Vals::of(&img);
        if let Some(p) = self.r21.fmt.as_mut() {
            if p.target != target {
                p.target = target;
                p.text_tab = false;
                p.page = fp::Page::FillLine;
            }
            let stop = p.vals.stop;
            if p.vals != vals {
                p.vals = vals;
                p.vals.stop = stop.min(p.vals.stops.len().saturating_sub(1));
            }
        }
    }

    /// Run the pane's commands (one undo step each) on the selection.
    fn r21_run(&mut self, cmds: &[serde_json::Value]) -> bool {
        let mut ok = false;
        for c in cmds {
            ok |= self.view.picture_command(c);
        }
        self.sync_context();
        self.r21_sync();
        ok || !cmds.is_empty()
    }

    /// Input to the pane (`None`: not for it).
    fn r21_pane_input(&mut self, input: &crate::Input) -> Option<bool> {
        let pointer = self.pointer;
        let p = self.r21.fmt.as_mut()?;
        match *input {
            crate::Input::Move { x, y } if p.contains(x, y) => Some(p.hover(x, y)),
            crate::Input::Wheel(dy) if p.contains(pointer.0, pointer.1) => Some(p.wheel(dy)),
            crate::Input::Release { x, y } | crate::Input::Drag { x, y } if p.contains(x, y) => {
                Some(false)
            }
            crate::Input::Press { x, y, .. } if p.contains(x, y) => {
                let act = p.press(x, y);
                Some(self.r21_pane_act(act))
            }
            _ => None,
        }
    }

    fn r21_pane_act(&mut self, act: fp::PaneAct) -> bool {
        match act {
            fp::PaneAct::None => false,
            fp::PaneAct::Redraw => true,
            fp::PaneAct::Close => {
                self.r21.fmt = None;
                self.relayout_pending = true;
                true
            }
            fp::PaneAct::Run(cmds) => self.r21_run(&cmds),
            fp::PaneAct::PickPicture => {
                self.requests.push(Request::PickFile {
                    purpose: "fp21pic".into(),
                    filter: "pictures".into(),
                });
                false
            }
            fp::PaneAct::Menu(id, anchor) => {
                let Some(p) = self.r21.fmt.as_ref() else {
                    return false;
                };
                let items: Vec<MenuItem> = if id.ends_with("Color") {
                    PANE_COLORS
                        .iter()
                        .map(|(hex, k, en)| MenuItem {
                            label: tr(&self.strings, k, en),
                            choice: Choice::Command(format!("fp21.color:{id}:{hex}")),
                            checked: false,
                            dim: false,
                        })
                        .collect()
                } else {
                    let t = |k: &str, en: &str| tr(&self.strings, k, en);
                    p.menu_labels(id, &t)
                        .into_iter()
                        .map(|(choice, label, on)| MenuItem {
                            label,
                            choice: Choice::Command(choice),
                            checked: on,
                            dim: false,
                        })
                        .collect()
                };
                self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
                true
            }
        }
    }

    /// Commands of the pane and of the buttons that open it (`None`: not one of them).
    fn r21_pane_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "fp21.shapeStyles" | "fp21.formatShape" => {
                self.r21_open_pane(fp::Page::FillLine, Some("fill"), false)
            }
            "fp21.wordArt" => self.r21_open_pane(fp::Page::FillLine, Some("tfill"), true),
            "fp21.pictureStyles" => self.r21_open_pane(fp::Page::Effects, Some("shadow"), false),
            // Shape Fill › Gradient › More Gradients… / Texture › More Textures… open the
            // pane with the fill chosen, as the reference.
            "r20.fillEffects:0" | "r20.fillEffects:1" if self.r21_selected().is_some() => {
                self.r21_open_pane(fp::Page::FillLine, Some("fill"), false);
                let grad = id.ends_with('0');
                let act = self
                    .r21
                    .fmt
                    .as_mut()
                    .map(|p| p.choose_fill(if grad { 2 } else { 3 }));
                match act {
                    Some(a) => {
                        self.r21_pane_act(a);
                        true
                    }
                    None => false,
                }
            }
            "fp21.paneClose" => {
                self.menu = None;
                self.r21.fmt = None;
                self.relayout_pending = true;
                true
            }
            "fp21.paneSize" => {
                self.menu = None;
                if let Some(p) = self.r21.fmt.as_mut() {
                    p.toggle_size();
                }
                self.relayout_pending = true;
                true
            }
            _ if id.starts_with("fp21.color:") => {
                let rest = &id["fp21.color:".len()..];
                let Some((target, hex)) = rest.split_once(':') else {
                    return Some(false);
                };
                let rgb = u32::from_str_radix(hex, 16).unwrap_or(0);
                let cmds = match self.r21.fmt.as_mut() {
                    Some(p) => p.set_color(target, rgb),
                    None => return Some(false),
                };
                self.menu = None;
                self.r21_run(&cmds)
            }
            _ if id.starts_with("fp21.") => {
                let cmds = match self.r21.fmt.as_mut() {
                    Some(p) => p.choose(id),
                    None => return Some(false),
                };
                self.menu = None;
                self.r21_run(&cmds)
            }
            _ => return None,
        })
    }

    /// A picture the host read for Picture or texture fill › Insert….
    pub(super) fn r21_pane_picture(&mut self, bytes: &[u8]) -> bool {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD.encode(bytes);
        self.r21_run(&[serde_json::json!({
            "cmd": "shapePictureFill", "data": data, "tile": false
        })])
    }

    /// Paint the pane.
    pub(super) fn r21_paint_pane(&self, p: &mut dyn crate::Painter) {
        if let Some(pane) = &self.r21.fmt {
            let t = |k: &str, en: &str| tr(&self.strings, k, en);
            pane.paint(p, &t);
        }
    }
}

fn tr(s: &crate::strings::Strings, key: &str, fallback: &str) -> String {
    let v = s.get(key);
    if v == key {
        fallback.to_owned()
    } else {
        v.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DocView, Input, TextMeasure, TextStyle};

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
    fn custom_tab_appears_and_collapses_with_the_window() {
        let mut engine = bo_engine::Engine::with_bundled_fonts();
        engine.new_document("a.docx").expect("new");
        let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
        let m = Fixed;
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 900.0,
            },
            &m,
        );
        let mut p = DocPrefs::default();
        let mut l = rb::Layout::default().full();
        let tid = l.new_id(rb::TAB_PREFIX);
        let gid = l.new_id(rb::GROUP_PREFIX);
        l.tabs.insert(
            0,
            rb::CTab {
                id: tid.clone(),
                name: "Mine".into(),
                shown: true,
                tool: false,
                groups: vec![rb::CGroup {
                    id: gid.clone(),
                    name: "Fast".into(),
                    commands: vec![
                        "format.bold".into(),
                        "edit.find".into(),
                        "insert.table".into(),
                        "format.italic".into(),
                    ],
                }],
            },
        );
        l.tabs.iter_mut().for_each(|t| {
            if t.id == "ribbon.tabs.review" {
                "Proof".clone_into(&mut t.name);
            }
        });
        p.extra.ribbon = l;
        w.r21_apply(&p);
        w.relayout(&m);
        let labels = w.ribbon.fixed_labels();
        assert_eq!(labels[0], tid.as_str());
        assert_eq!(w.strings.get(&tid), "Mine");
        assert_eq!(w.strings.get("ribbon.tabs.review"), "Proof");
        assert!(w.ribbon.select_tab(&tid));
        w.relayout(&m);
        assert!(w
            .ribbon
            .rect_of(crate::ribbon::RibbonHit::Command("format.bold"))
            .is_some());
        // Narrow: the group's buttons lose their labels, then the group is one drop-down
        // button that opens the whole group.
        let wide = w
            .ribbon
            .rect_of(crate::ribbon::RibbonHit::Command("format.bold"))
            .map_or(0.0, |r| r[2]);
        w.handle(
            Input::Resize {
                width: 160.0,
                height: 900.0,
            },
            &m,
        );
        let narrow = w
            .ribbon
            .rect_of(crate::ribbon::RibbonHit::Command("format.bold"))
            .map_or(0.0, |r| r[2]);
        assert!(narrow > 0.0 && narrow < wide, "{narrow} {wide}");
        let button = rb::button_of(&gid);
        let last = rb::shrink(&gid, 2).expect("button stage");
        assert_eq!(last.large, vec![crate::ribbon::Item::Large(button)]);
        assert!(crate::doc_scale18::is_group(button));
        assert!(crate::ribbon::doc_group(&tid, &gid).is_some());
        assert!(rb::holder("edit.find").is_some());
        // Back to the default ribbon: the name comes back.
        w.r21_apply(&DocPrefs::default());
        assert_eq!(w.strings.get("ribbon.tabs.review"), "Review");
        assert_ne!(w.ribbon.fixed_labels()[0], tid.as_str());
    }

    #[test]
    fn more_gradients_opens_the_format_shape_pane_and_its_settings_apply() {
        let mut engine = bo_engine::Engine::with_bundled_fonts();
        engine.new_document("a.docx").expect("new");
        let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
        w.handle(Input::Scale(1.25), &Fixed);
        w.handle(
            Input::Resize {
                width: 1500.0,
                height: 1102.0,
            },
            &Fixed,
        );
        w.run_interface_command("text:Shape here", &Fixed);
        w.r3_run(vec![serde_json::json!({
            "cmd": "insertShape", "preset": "rect", "width": 120.0, "height": 60.0
        })]);
        w.view.exec(bo_engine::edit::Command::Move {
            motion: bo_engine::nav::Motion::Left,
            extend: true,
        });
        w.handle(Input::Tick, &Fixed);
        // The launchers can be pressed (enabled on their tabs).
        let f = bo_edit::FormatState::default();
        assert!(crate::commands::enabled("fp21.shapeStyles", &f));
        assert!(w.action("r20.fillEffects:0"));
        let shape = |w: &DocWindow| w.r21_selected().and_then(|s| s.1.shape).expect("shape");
        assert_eq!(w.r21.fmt.as_ref().map(|p| p.vals.fill), Some(2));
        assert!(shape(&w).style.and_then(|s| s.gradient).is_some());
        // The pane takes room at the right of the document.
        assert!(w.r21_pane_w() > 0.0);
        // A shadow setting from the pane: one command, the shape's shadow follows it.
        let cmds = w.r21.fmt.as_mut().expect("pane").set_spin("shBlur", 7.0);
        assert!(w.r21_run(&cmds));
        let blur = shape(&w)
            .style
            .and_then(|s| s.effects)
            .and_then(|e| e.shadow)
            .map(|s| s.blur);
        assert_eq!(blur, Some(7 * 12_700));
        // Text box margins.
        let cmds = w.r21.fmt.as_mut().expect("pane").set_spin("insL", 18.0);
        assert!(w.r21_run(&cmds));
        assert_eq!(shape(&w).insets[0].0, 18 * 12_700);
        // Close.
        let act = fp::PaneAct::Close;
        w.r21_pane_act(act);
        assert!(w.r21.fmt.is_none());
    }

    #[test]
    fn internet_files_open_read_only_until_enable_editing() {
        let mut engine = bo_engine::Engine::with_bundled_fonts();
        engine.new_document("a.docx").expect("new");
        let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
        let m = Fixed;
        w.handle(
            Input::Resize {
                width: 1200.0,
                height: 800.0,
            },
            &m,
        );
        w.set_download_zone(0);
        assert!(!w.protected_view());
        w.set_download_zone(3);
        assert!(w.protected_view());
        w.handle(Input::Tick, &m);
        let top = w.ribbon.bottom();
        let frame_top = w.chrome_bottom();
        assert!(frame_top > top, "{frame_top} {top}");
        let before = w.view.modified();
        w.handle(Input::Text("abc".into()), &m);
        assert_eq!(w.view.modified(), before);
        assert_eq!(w.r21_guard("format.bold"), Some(false));
        assert_eq!(w.r21_guard("view.zoom"), None);
        let r = w.r21.enable_rect;
        w.handle(
            Input::Press {
                x: r[0] + 4.0,
                y: r[1] + 4.0,
                count: 1,
                shift: false,
            },
            &m,
        );
        assert!(!w.protected_view());
        w.handle(Input::Text("abc".into()), &m);
        assert!(w.view.modified());
    }
}
