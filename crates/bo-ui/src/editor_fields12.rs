//! BlueEditor round 12: the field-mode context menu as the reference editor's (black-box at
//! 125 %: rows 28 px from x 36, rules 8 px apart in #E0E0E0, the highlighted row a #D9D9D9 band
//! 2 px in from the frame, sub-menus marked with a solid triangle) — Properties… | Cut / Copy /
//! Paste | Delete | Align ▸ / Center ▸ / Distribute ▸ / Size ▸ | Create Multiple Copies… /
//! Set Tab Order / Duplicate… / Use Current Properties as New Defaults — with the field
//! clipboard, the "Create Multiple Copies of Fields" dialog (545 × 585 px) and the Duplicate
//! dialog, through `bo_pdfread::FieldClip`, `field_copies` and `duplicate_fields`.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{
    button, check, group, label, radio, rlabel, spin, text_w, Form, On, Val,
};
use crate::editor_window::EditorWindow;
use crate::popup::{Choice, MenuItem, MenuLook};
use crate::{Input, Key, Painter};
use bo_pdfread::{FieldClip, FieldKind, FieldProps};
use bo_render::Rgba;

/// The field context menu at 100 % (measured at 125 %: rows 28 px, labels from x 36 with an
/// 11 px cap height, 2 px above the first row, white with a #E0E0E0 frame).
pub(crate) const FIELD_MENU12: MenuLook = MenuLook {
    item_h: 22.4,
    text: 12.0,
    text_x: (28.8, 28.8),
    icon_x: 4.0,
    icon: 14.4,
    pad: 1.6,
    min_w: 96.0,
    char_w: 4.0,
    fill: Some(Rgba(255, 255, 255, 255)),
    frame: Some(Rgba(0xE0, 0xE0, 0xE0, 255)),
};

/// Band under the highlighted row (#D9D9D9 on the reference).
const HOVER12: Rgba = Rgba(0xD9, 0xD9, 0xD9, 255);

/// Space a rule takes between two groups of rows (8 px at 125 %).
const SEP_GAP12: f32 = 6.4;

/// Space right of the longest label (50 px at 125 % with the label estimate: room for the
/// sub-menu mark).
const RIGHT12: f32 = 28.0;

/// Dialogs of round 12.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Dlg12 {
    /// Create Multiple Copies of Fields.
    Copies,
    /// Duplicate the selected fields to other pages.
    Duplicate,
    /// A value of the File › Properties page (`editor_bsprops12`).
    Value(crate::editor_bsprops12::Value12),
    /// Split Document › Output Options.
    Output,
    /// Create Custom Stamp (`true`: dynamic) (`editor_stamps12`).
    Stamp(bool),
    /// Manage Stamps.
    ManageStamps,
    /// Rights-management servers are not set up.
    Rms,
}

/// Field clipboard and new-field defaults (round 12).
#[derive(Debug, Clone, Default)]
pub(crate) struct Fields12 {
    /// Fields copied or cut.
    pub clip: Vec<FieldClip>,
    /// "Use Current Properties as New Defaults": the look new fields of a kind get.
    pub defaults: Vec<(FieldKind, FieldProps)>,
    /// Overall position of Create Multiple Copies (points, user space).
    pub shift: (f32, f32),
    /// Split Document › Output Options (kept for the session).
    pub out: SplitOut,
    /// Access keys of the next menu12 (one per row) and whether to underline them (opened
    /// from the keyboard).
    pub mn_next: Option<(Vec<char>, bool)>,
    /// Access keys of the open field menu: (lower-case letter, command, dimmed).
    pub mnem: Vec<(char, String, bool)>,
    /// Where the open field menu underlines its access keys (keyboard-opened only).
    pub underlines: Vec<Option<(f32, f32)>>,
}

/// Split Document › Output Options: where the parts go and how they are named (the
/// reference's defaults: same folder, label and number after the name, "Part", "_").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitOut {
    /// Folder typed for "In" (empty: same as the source).
    pub dir: String,
    /// 0 bookmark names, 1 label and number before, 2 after the original name.
    pub naming: usize,
    pub use_label: bool,
    pub label: String,
    pub use_sep: bool,
    pub sep: String,
    pub no_overwrite: bool,
}

impl Default for SplitOut {
    fn default() -> Self {
        Self {
            dir: String::new(),
            naming: 2,
            use_label: true,
            label: "Part".into(),
            use_sep: true,
            sep: "_".into(),
            no_overwrite: true,
        }
    }
}

impl SplitOut {
    /// File name of part `k` (1-based) of `stem`; `bookmark` is the part's top-level
    /// bookmark title when splitting by bookmarks.
    pub(crate) fn name(&self, stem: &str, k: usize, bookmark: Option<&str>) -> String {
        let sep = if self.use_sep { self.sep.as_str() } else { "" };
        let label = if self.use_label {
            self.label.as_str()
        } else {
            ""
        };
        let base = match (self.naming, bookmark) {
            (0, Some(b)) if !b.trim().is_empty() => b
                .chars()
                .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
                .collect(),
            (1, _) => format!("{label}{k}{sep}{stem}"),
            _ => format!("{stem}{sep}{label}{k}"),
        };
        let name = format!("{base}.pdf");
        if self.dir.trim().is_empty() {
            name
        } else {
            std::path::Path::new(self.dir.trim())
                .join(name)
                .to_string_lossy()
                .into_owned()
        }
    }
}

/// Access keys of the field menu's rows, as the reference underlines them.
const FIELD_KEYS12: [char; 13] = [
    't', 't', 'C', 'P', 'D', 'l', 'n', 'b', 'z', 'M', 'O', 'e', 'U',
];

/// One menu row: command, label key, dimmed, opens a sub-menu.
type Row12 = (&'static str, &'static str, bool, bool);

impl EditorWindow {
    /// A context menu with the reference's field-menu look at window point `at`: rules before
    /// the rows in `seps`, a sub-menu mark on the rows set in `arrows`.
    pub(crate) fn menu12(&mut self, rows: &[Row12], seps: Vec<usize>, at: [f32; 2]) {
        let items: Vec<MenuItem> = rows
            .iter()
            .map(|(cmd, key, dim, _)| MenuItem {
                label: self.t(key),
                choice: Choice::Command((*cmd).to_owned()),
                checked: false,
                dim: *dim,
            })
            .collect();
        let arrows = rows.iter().map(|r| r.3).collect();
        self.menu12_items(items, seps, arrows, at);
    }

    /// A sub-menu of the field menu (Align, Center, Distribute, Size) with the same look.
    pub(crate) fn submenu12(&mut self, items: Vec<MenuItem>, at: [f32; 2]) {
        let items = items
            .into_iter()
            .map(|mut i| {
                i.label = i.label.trim_end_matches(['\u{203A}', ' ']).to_owned();
                i
            })
            .collect();
        self.menu12_items(items, Vec::new(), Vec::new(), at);
    }

    fn menu12_items(
        &mut self,
        items: Vec<MenuItem>,
        seps: Vec<usize>,
        arrows: Vec<bool>,
        at: [f32; 2],
    ) {
        let s = self.ui_scale();
        // Labels are drawn 1.1 × larger by the host (text size factor).
        let size = FIELD_MENU12.text * s * 1.1;
        let widest = items
            .iter()
            .map(|i| text_w(&i.label, size) * 0.93)
            .fold(0.0_f32, f32::max);
        let keys = vec![(String::new(), 0.0); items.len()];
        // Access keys (the reference's Proper_t_ies, Cu_t_, _C_opy …): the first matching
        // letter of each label; underlined only when the menu came from the keyboard.
        let (letters, show) = self.f12.mn_next.take().unwrap_or_default();
        self.f12.mnem.clear();
        let mut lines = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let hit = letters.get(i).and_then(|&c| {
                item.label
                    .char_indices()
                    .find(|&(_, l)| l == c)
                    .or_else(|| {
                        item.label
                            .char_indices()
                            .find(|&(_, l)| l.to_lowercase().eq(c.to_lowercase()))
                    })
            });
            let line = hit.map(|(at, l)| {
                if let Choice::Command(cmd) = &item.choice {
                    self.f12.mnem.push((
                        l.to_lowercase().next().unwrap_or(l),
                        cmd.clone(),
                        item.dim,
                    ));
                }
                let x = text_w(&item.label[..at], size) * 0.93;
                let w = text_w(&l.to_string(), size) * 0.93;
                (x, w)
            });
            lines.push(if show { line } else { None });
        }
        let gap = (2.0 * s).round();
        let view = self.window_size();
        let menu = crate::popup::Menu::open_with(
            items,
            [at[0], at[1] - gap - 1.0, 1.0, 1.0],
            view,
            s,
            FIELD_MENU12,
        )
        .with_soft_shadow()
        .with_decor(Vec::new(), seps)
        .with_keys(keys, widest, RIGHT12, SEP_GAP12, view)
        .with_arrows(arrows, HOVER12);
        // The underlines wait for the shared popup's access-key support (BlueSheet round):
        // kept here, (x from the label start, width) per row.
        self.f12.underlines = lines;
        self.r.set_menu9(menu, "field");
    }

    /// The context menu of the selected fields, as the reference's (enable rules measured with
    /// one field: Align, Distribute and Size need several, Duplicate another page, Paste a
    /// copied field).
    pub(crate) fn field_menu12(&mut self, x: f32, y: f32) {
        self.field_menu12_from(x, y, false);
    }

    /// [`Self::field_menu12`]; `keyboard`: opened with Shift+F10 / the menu key, so the access
    /// keys are underlined as the reference shows them.
    pub(crate) fn field_menu12_from(&mut self, x: f32, y: f32, keyboard: bool) {
        self.f12.mn_next = Some((FIELD_KEYS12.to_vec(), keyboard));
        self.f11.menu_at = [x, y];
        let n = self.f11.selected.len();
        let pages = self.view().map_or(0, crate::ReaderView::page_count);
        let rows: [Row12; 13] = [
            ("fd11.props", "pdfEditor12.properties", n == 0, false),
            ("fd12.cut", "pdfEditor12.cut", n == 0, false),
            ("fd12.copy", "pdfEditor12.copy", n == 0, false),
            (
                "fd12.paste",
                "pdfEditor12.paste",
                self.f12.clip.is_empty(),
                false,
            ),
            ("fd11.delete", "pdfEditor12.delete", n == 0, false),
            ("fd11.menu.align", "pdfEditor12.align", n < 2, true),
            ("fd11.menu.center", "pdfEditor12.center", n == 0, true),
            (
                "fd11.menu.distribute",
                "pdfEditor12.distribute",
                n < 3,
                true,
            ),
            ("fd11.menu.size", "pdfEditor12.size", n < 2, true),
            ("fd12.copies", "pdfEditor12.copies", n == 0, false),
            // Greyed for one field, as the reference (ordering needs several).
            ("fd11.tabManual", "pdfEditor12.setTabOrder", n < 2, false),
            (
                "fd12.duplicate",
                "pdfEditor12.duplicate",
                n == 0 || pages < 2,
                false,
            ),
            ("fd12.defaults", "pdfEditor12.defaults", n != 1, false),
        ];
        self.menu12(&rows, vec![1, 4, 5, 9], [x, y]);
    }

    /// The context menu over the page (no field) in field mode: Paste.
    fn page_menu12(&mut self, x: f32, y: f32) {
        self.f11.menu_at = [x, y];
        let rows: [Row12; 1] = [(
            "fd12.paste",
            "pdfEditor12.paste",
            self.f12.clip.is_empty(),
            false,
        )];
        self.menu12(&rows, Vec::new(), [x, y]);
    }

    /// Keyboard of the field menu, before any other input: a letter runs the row with that
    /// access key while the menu is open; Shift+F10 opens the menu (keys underlined) at the
    /// first selected field. True when used.
    pub(crate) fn field_keys12(&mut self, input: &Input) -> bool {
        if self.r.open_menu_id() == Some("field") {
            let c = match input {
                Input::Text(t) if t.chars().count() == 1 => t.chars().next(),
                Input::Key(Key::Letter(c), m) if !m.ctrl && !m.alt => Some(*c),
                _ => None,
            };
            let Some(c) = c.and_then(|c| c.to_lowercase().next()) else {
                return false;
            };
            let hit = self
                .f12
                .mnem
                .iter()
                .find(|(k, _, dim)| *k == c && !dim)
                .map(|m| m.1.clone());
            if let Some(cmd) = hit {
                self.r.close_menu();
                self.run_field_command12(&cmd);
            }
            // Letters without a row are swallowed, as a menu does.
            return true;
        }
        if let Input::Key(Key::F(10), m) = input {
            if m.shift && self.design.tool.is_some() && !self.r.modal_open() {
                let at = self.f11.selected.first().cloned().and_then(|f| {
                    self.view()
                        .and_then(|v| v.doc_rect_to_window(f.page, f.rect))
                });
                if let Some(r) = at {
                    self.field_menu12_from(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, true);
                    return true;
                }
            }
        }
        false
    }

    /// Run a row of the field menu chosen by its access key.
    fn run_field_command12(&mut self, cmd: &str) {
        if !self.fields12_command(cmd) {
            self.fields11_command(cmd);
        }
    }

    /// Field-mode input of round 12 (before round 11's): Ctrl+X / C / V and the context menu
    /// over the page; true when used.
    pub(crate) fn fields12_input(&mut self, input: &Input) -> bool {
        match *input {
            Input::Key(Key::Letter(c), m) if m.ctrl && !m.alt => match c {
                'c' if !self.f11.selected.is_empty() => self.fields12_command("fd12.copy"),
                'x' if !self.f11.selected.is_empty() => self.fields12_command("fd12.cut"),
                'v' if !self.f12.clip.is_empty() => {
                    self.f11.menu_at = [f32::NAN, f32::NAN];
                    self.fields12_command("fd12.paste")
                }
                _ => false,
            },
            Input::Context { x, y }
                if crate::editor_window::inside(self.r.doc_area(), x, y)
                    && self.field_at11(x, y).is_none() =>
            {
                self.page_menu12(x, y);
                true
            }
            _ => false,
        }
    }

    /// Commands of round 12's field menu (`fd12.*`, `prot.drm`); false when `cmd` is not one.
    pub(crate) fn fields12_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "fd12.copy" | "fd12.cut" => {
                let clips: Vec<FieldClip> = self
                    .view()
                    .map(|v| {
                        let d = v.doc();
                        self.f11
                            .selected
                            .iter()
                            .filter_map(|f| d.copy_field(f))
                            .collect()
                    })
                    .unwrap_or_default();
                if !clips.is_empty() {
                    self.f12.clip = clips;
                }
                if cmd == "fd12.cut" {
                    self.fields11_command("fd11.delete");
                }
            }
            "fd12.paste" => self.paste12(),
            "fd12.copies" => self.open_copies12(),
            "fd12.duplicate" => self.open_duplicate12(),
            "fd12.defaults" => {
                let Some(f) = self.f11.selected.first().cloned() else {
                    return true;
                };
                if let Some(p) = self.view().map(|v| v.doc().field_props(&f)) {
                    self.f12.defaults.retain(|d| d.0 != f.kind);
                    self.f12.defaults.push((f.kind, p));
                }
            }
            "prot.drm12" => self.open_rms12(),
            _ => return false,
        }
        true
    }

    /// Paste the clipboard fields: at the point the context menu opened (their top-left
    /// corner there), or 10 pt down and right of where they were (Ctrl+V).
    fn paste12(&mut self) {
        let clips = self.f12.clip.clone();
        let Some(first) = clips.first() else {
            return;
        };
        let x0 = clips
            .iter()
            .map(|c| c.rect[0])
            .fold(first.rect[0], f32::min);
        let y1 = clips
            .iter()
            .map(|c| c.rect[3])
            .fold(first.rect[3], f32::max);
        let [mx, my] = self.f11.menu_at;
        let target = (!mx.is_nan())
            .then(|| self.view().and_then(|v| v.window_to_doc(mx, my)))
            .flatten();
        let (page, dx, dy) = if let Some((page, px, py)) = target {
            let Some((ux, uy)) = self.view().and_then(|v| v.doc().page_to_user(page, px, py))
            else {
                return;
            };
            (page, ux - x0, uy - y1)
        } else {
            let page = self.f11.selected.first().map_or_else(
                || self.view().map_or(0, crate::ReaderView::current_page),
                |f| f.page,
            );
            (page, 10.0, -10.0)
        };
        match self.doc_edit(|d| d.paste_fields(page, &clips, dx, dy)) {
            Ok(names) => {
                self.after_fields12(page);
                self.f11.selected = self
                    .fields()
                    .into_iter()
                    .filter(|f| names.contains(&f.name))
                    .collect();
            }
            Err(e) => self.set_status(&e),
        }
    }

    fn after_fields12(&mut self, page: usize) {
        let pages = self.view().map_or(0, crate::ReaderView::page_count);
        if let Some(v) = self.r.view_mut() {
            for p in page..pages {
                v.invalidate_page(p);
            }
            if page > 0 {
                v.invalidate_page(page - 1);
            }
        }
        self.after_doc_edit();
    }

    /// "Create Multiple Copies of Fields" (545 × 585 px at 125 %, measured on the reference).
    fn open_copies12(&mut self) {
        self.f12.shift = (0.0, 0.0);
        let t = |k: &str| self.t(k);
        let c = vec![
            group(t("pdfEditor12.numberOfFields"), [30.0, 50.0, 485.0, 106.0]),
            rlabel(349.0, 80.0, t("pdfEditor12.copyDown"), On::Always),
            spin(
                "down",
                [361.0, 63.0, 122.0, 33.0],
                1.0,
                (1.0, 99.0),
                On::Always,
            ),
            rlabel(349.0, 124.0, t("pdfEditor12.copyAcross"), On::Always),
            spin(
                "across",
                [361.0, 107.0, 122.0, 33.0],
                1.0,
                (1.0, 99.0),
                On::Always,
            ),
            group(t("pdfEditor12.overallSize"), [30.0, 179.0, 485.0, 119.0]),
            rlabel(326.0, 218.0, t("pdfEditor12.hSpacing"), On::Always),
            spin(
                "hgap",
                [337.0, 201.0, 121.0, 33.0],
                0.01,
                (0.0, 20.0),
                On::Always,
            ),
            rlabel(326.0, 265.0, t("pdfEditor12.vSpacing"), On::Always),
            spin(
                "vgap",
                [337.0, 248.0, 121.0, 33.0],
                0.01,
                (0.0, 20.0),
                On::Always,
            ),
            group(
                t("pdfEditor12.overallPosition"),
                [30.0, 318.0, 485.0, 152.0],
            ),
            button(
                "up12",
                [223.0, 339.0, 108.0, 31.0],
                t("pdfEditor12.up"),
                On::Always,
            ),
            button(
                "left12",
                [55.0, 379.0, 109.0, 31.0],
                t("pdfEditor12.left"),
                On::Always,
            ),
            button(
                "right12",
                [385.0, 379.0, 109.0, 31.0],
                t("pdfEditor12.right"),
                On::Always,
            ),
            button(
                "down12",
                [223.0, 424.0, 108.0, 31.0],
                t("pdfEditor12.down"),
                On::Always,
            ),
            check("preview", 29.0, 503.0, t("pdfEditor12.preview")),
            check("same", 155.0, 503.0, t("pdfEditor12.sameName")),
            button(
                "ok",
                [247.0, 538.0, 122.0, 31.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [392.0, 538.0, 122.0, 31.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor12.copiesTitle"), (545.0, 585.0), c)
            .with("down", Val::Text("2".into()))
            .with("across", Val::Text("2".into()))
            .with("hgap", Val::Text("0.04".into()))
            .with("vgap", Val::Text("0.04".into()))
            .with("preview", Val::Pick(1))
            .with("same", Val::Pick(0))
            .focused("down");
        self.form8.push((f, FormWhat::R12(Dlg12::Copies)));
    }

    /// Duplicate (to pages): the reference's command is greyed on a one-page document, so its
    /// dialog was not measured; the layout is ours in the same family.
    fn open_duplicate12(&mut self) {
        let t = |k: &str| self.t(k);
        let n = self.view().map_or(1, crate::ReaderView::page_count).max(1);
        #[allow(clippy::cast_precision_loss)]
        let max = n as f32;
        let c = vec![
            group(t("pdfEditor12.duplicateTo"), [21.0, 53.0, 406.0, 128.0]),
            radio("which", 0, 38.0, 92.0, t("pdfEditor12.allPages")),
            radio("which", 1, 38.0, 140.0, t("pdfEditor12.fromPage")),
            spin(
                "from",
                [120.0, 125.0, 75.0, 29.0],
                1.0,
                (1.0, max),
                On::If("which", 1),
            ),
            rlabel(238.0, 140.0, t("pdfEditor12.toPage"), On::If("which", 1)),
            spin(
                "to",
                [248.0, 125.0, 75.0, 29.0],
                1.0,
                (1.0, max),
                On::If("which", 1),
            ),
            label(338.0, 140.0, format!("{} {n}", t("pdfEditor9.ofShort"))),
            button(
                "ok",
                [243.0, 203.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [343.0, 203.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor12.duplicateTitle"), (453.0, 249.0), c)
            .with("which", Val::Pick(0))
            .with("from", Val::Text("1".into()))
            .with("to", Val::Text(n.to_string()));
        self.form8.push((f, FormWhat::R12(Dlg12::Duplicate)));
    }

    /// Rights-management servers: none is set up (the reference connects to a vendor service).
    fn open_rms12(&mut self) {
        let t = |k: &str| self.t(k);
        let c = vec![
            Ctl12::info(),
            label(78.0, 70.0, t("pdfEditor12.rmsNone")),
            label(78.0, 98.0, t("pdfEditor12.rmsHint")),
            button(
                "ok",
                [343.0, 138.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor12.rmsTitle"), (453.0, 184.0), c);
        self.form8.push((f, FormWhat::R12(Dlg12::Rms)));
    }

    /// Split Document › Output Options (553 × 458 px at 125 %, measured on the reference).
    fn open_output12(&mut self) {
        let t = |k: &str| self.t(k);
        let o = self.f12.out.clone();
        let c = vec![
            group(t("pdfEditor12.outputFolder"), [17.0, 55.0, 520.0, 106.0]),
            radio("folder", 0, 28.0, 94.0, t("pdfEditor12.sameAsSource")),
            radio("folder", 1, 28.0, 132.0, t("pdfEditor12.inFolder")),
            crate::editor_form8::edit("dir", [97.0, 118.0, 328.0, 28.0], On::If("folder", 1)),
            button(
                "browse",
                [432.0, 118.0, 93.0, 28.0],
                t("pdfEditor12.browse"),
                On::Never,
            ),
            group(String::new(), [17.0, 178.0, 520.0, 226.0]),
            Ctl12::radio_off("naming", 0, 28.0, 215.0, t("pdfEditor12.bookmarkNames")),
            radio("naming", 1, 28.0, 246.0, t("pdfEditor12.labelBefore")),
            radio("naming", 2, 28.0, 279.0, t("pdfEditor12.labelAfter")),
            check("uselabel", 28.0, 312.0, t("pdfEditor12.useLabel")),
            crate::editor_form8::edit("label", [160.0, 298.0, 100.0, 27.0], On::If("uselabel", 1)),
            check("usesep", 28.0, 346.0, t("pdfEditor12.useSeparator")),
            crate::editor_form8::edit("sep", [378.0, 333.0, 147.0, 26.0], On::If("usesep", 1)),
            check("nooverwrite", 28.0, 380.0, t("pdfEditor12.noOverwrite")),
            button(
                "ok",
                [345.0, 414.0, 91.0, 27.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [445.0, 414.0, 91.0, 27.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let f = Form::new(t("pdfEditor12.outputTitle"), (553.0, 458.0), c)
            .with("folder", Val::Pick(usize::from(!o.dir.trim().is_empty())))
            .with("dir", Val::Text(o.dir))
            .with("naming", Val::Pick(o.naming.max(1)))
            .with("uselabel", Val::Pick(usize::from(o.use_label)))
            .with("label", Val::Text(o.label))
            .with("usesep", Val::Pick(usize::from(o.use_sep)))
            .with("sep", Val::Text(o.sep))
            .with("nooverwrite", Val::Pick(usize::from(o.no_overwrite)));
        self.form8.push((f, FormWhat::R12(Dlg12::Output)));
    }

    /// Round-12 dialog buttons other than OK / Cancel; true when used.
    pub(crate) fn r12_button(&mut self, id: &str) -> bool {
        if id == "delete12" {
            self.delete_stamp12();
            return true;
        }
        if id == "output" && matches!(self.form8.last(), Some((_, FormWhat::Split))) {
            self.open_output12();
            return true;
        }
        let step = 1.0;
        let d = match id {
            "up12" => (0.0, step),
            "down12" => (0.0, -step),
            "left12" => (-step, 0.0),
            "right12" => (step, 0.0),
            _ => return false,
        };
        self.f12.shift.0 += d.0;
        self.f12.shift.1 += d.1;
        true
    }

    /// OK of a round-12 dialog.
    pub(crate) fn r12_ok(&mut self, f: &Form, what: &Dlg12) {
        let sel = self.f11.selected.clone();
        match what {
            Dlg12::Copies => {
                let down = f.num("down", 2.0).clamp(1.0, 99.0).round() as usize;
                let across = f.num("across", 2.0).clamp(1.0, 99.0).round() as usize;
                let gap = (
                    f.num("hgap", 0.04).max(0.0) * 72.0,
                    f.num("vgap", 0.04).max(0.0) * 72.0,
                );
                let same = f.pick("same") == 1;
                let shift = self.f12.shift;
                let page = sel.first().map_or(0, |s| s.page);
                match self.doc_edit(|d| d.field_copies(&sel, down, across, gap, shift, same)) {
                    Ok(_) => self.after_fields12(page),
                    Err(e) => self.set_status(&e),
                }
                self.f12.shift = (0.0, 0.0);
                self.refresh_selection12();
            }
            Dlg12::Duplicate => {
                let n = self.view().map_or(0, crate::ReaderView::page_count);
                let pages: Vec<usize> = if f.pick("which") == 0 {
                    (0..n).collect()
                } else {
                    let a = f.num("from", 1.0).max(1.0) as usize;
                    let b = f.num("to", n as f32).max(1.0) as usize;
                    (a.min(b).saturating_sub(1)..b.max(a).min(n)).collect()
                };
                match self.doc_edit(|d| d.duplicate_fields(&sel, &pages)) {
                    Ok(_) => self.after_fields12(0),
                    Err(e) => self.set_status(&e),
                }
                self.refresh_selection12();
            }
            Dlg12::Rms | Dlg12::ManageStamps => {}
            Dlg12::Stamp(dynamic) => self.create12_ok(f, *dynamic),
            Dlg12::Output => {
                self.f12.out = SplitOut {
                    dir: if f.pick("folder") == 1 {
                        f.text("dir").trim().to_owned()
                    } else {
                        String::new()
                    },
                    naming: f.pick("naming"),
                    use_label: f.pick("uselabel") == 1,
                    label: f.text("label"),
                    use_sep: f.pick("usesep") == 1,
                    sep: f.text("sep"),
                    no_overwrite: f.pick("nooverwrite") == 1,
                };
            }
            Dlg12::Value(v) => self.value12_ok(f, *v),
        }
    }

    /// Keep the selected widgets selected after their fields changed.
    fn refresh_selection12(&mut self) {
        let now = self.fields();
        let old = std::mem::take(&mut self.f11.selected);
        self.f11.selected = old
            .into_iter()
            .filter_map(|w| now.iter().find(|f| f.same_widget(&w)).cloned())
            .collect();
    }

    /// A field just drawn takes the look saved with "Use Current Properties as New Defaults"
    /// for its kind (appearance, font, options; not its name, box or actions).
    pub(crate) fn apply_defaults12(&mut self, page: usize, name: &str) {
        let Some(f) = self
            .fields()
            .into_iter()
            .find(|f| f.page == page && f.name == name)
        else {
            return;
        };
        let Some((_, d)) = self.f12.defaults.iter().find(|d| d.0 == f.kind).cloned() else {
            return;
        };
        let Some(mut p) = self.view().map(|v| v.doc().field_props(&f)) else {
            return;
        };
        p.visibility = d.visibility;
        p.rotation = d.rotation;
        p.read_only = d.read_only;
        p.required = d.required;
        p.line_style = d.line_style;
        p.thickness = d.thickness;
        p.border_color = d.border_color;
        p.fill_color = d.fill_color;
        p.font_size = d.font_size;
        p.text_color = d.text_color;
        p.font = d.font.clone();
        p.rtl = d.rtl;
        p.align = d.align;
        p.scroll = d.scroll;
        p.rich = d.rich;
        p.max_len = d.max_len;
        p.password = d.password;
        p.spell_check = d.spell_check;
        p.comb = d.comb;
        p.multiline = d.multiline;
        p.format = d.format.clone();
        if let Err(e) = self.doc_edit(|doc| doc.set_field_props(&f, &p)) {
            self.set_status(&e);
        }
    }

    /// While Create Multiple Copies is open with Preview on: the copies' boxes, dashed.
    pub(crate) fn paint_fields12(&mut self, p: &mut dyn Painter) {
        let Some((f, FormWhat::R12(Dlg12::Copies))) = self.form8.last() else {
            return;
        };
        if f.pick("preview") != 1 {
            return;
        }
        let down = f.num("down", 2.0).clamp(1.0, 99.0).round() as usize;
        let across = f.num("across", 2.0).clamp(1.0, 99.0).round() as usize;
        let gap = (f.num("hgap", 0.04) * 72.0, f.num("vgap", 0.04) * 72.0);
        let sel = self.f11.selected.clone();
        let shift = self.f12.shift;
        let Some(v) = self.view() else { return };
        let boxes: Vec<(usize, [f32; 4])> = sel.iter().map(|s| (s.page, s.rect)).collect();
        let Some(&(page, first)) = boxes.first() else {
            return;
        };
        // Page points (y down): the selection's bounding box is one cell.
        let x0 = boxes.iter().map(|b| b.1[0]).fold(first[0], f32::min);
        let x1 = boxes.iter().map(|b| b.1[2]).fold(first[2], f32::max);
        let y0 = boxes.iter().map(|b| b.1[1]).fold(first[1], f32::min);
        let y1 = boxes.iter().map(|b| b.1[3]).fold(first[3], f32::max);
        let (cw, ch) = (x1 - x0 + gap.0, y1 - y0 + gap.1);
        let ink = Rgba(0x2E, 0x6D, 0xAD, 255);
        for row in 0..down {
            for col in 0..across {
                for (_, b) in &boxes {
                    let dx = shift.0 + col as f32 * cw;
                    let dy = -shift.1 + row as f32 * ch;
                    let r = [b[0] + dx, b[1] + dy, b[2] + dx, b[3] + dy];
                    let Some(w) = v.doc_rect_to_window(page, r) else {
                        continue;
                    };
                    dashed_box(p, w, ink);
                }
            }
        }
    }
}

/// A 1 px dashed frame (4 on, 3 off) around window rectangle `r` (x0, y0, x1, y1).
fn dashed_box(p: &mut dyn Painter, r: [f32; 4], ink: Rgba) {
    let (x0, y0, x1, y1) = (r[0].round(), r[1].round(), r[2].round(), r[3].round());
    let mut x = x0;
    while x < x1 {
        let w = 4.0_f32.min(x1 - x);
        p.fill_rect(x, y0, w, 1.0, ink);
        p.fill_rect(x, y1 - 1.0, w, 1.0, ink);
        x += 7.0;
    }
    let mut y = y0;
    while y < y1 {
        let h = 4.0_f32.min(y1 - y);
        p.fill_rect(x0, y, 1.0, h, ink);
        p.fill_rect(x1 - 1.0, y, 1.0, h, ink);
        y += 7.0;
    }
}

/// Controls of round 12 not in the round-8 set.
struct Ctl12;

impl Ctl12 {
    /// A radio button that is never available (the reference's bookmark-name naming, which
    /// applies only when splitting by bookmarks).
    fn radio_off(
        id: &'static str,
        k: usize,
        x: f32,
        y: f32,
        text: String,
    ) -> crate::editor_form8::Ctl {
        let mut c = radio(id, k, x, y, text);
        if let crate::editor_form8::Ctl::Radio { on, .. } = &mut c {
            *on = On::Never;
        }
        c
    }

    /// The information mark at the top left of a message.
    fn info() -> crate::editor_form8::Ctl {
        crate::editor_form8::Ctl::Info { x: 40.0, y: 84.0 }
    }
}

#[cfg(test)]
#[path = "editor_fields12_tests.rs"]
mod tests;
