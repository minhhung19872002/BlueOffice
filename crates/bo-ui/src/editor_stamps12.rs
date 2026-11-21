//! BlueEditor round 12: Comment › Stamp ▾ as the reference editor's stamp palette (black-box at
//! 125 %: 599 px wide, a bold section title, then cells 137 × 40 px four to a row 144 px apart,
//! rows 69 px apart) with stamp designs of our own — Dynamic (title and who / when), Sign Here
//! (filled bands), Standard (ISO 32000-1 Table 181 names) and the user's Custom stamps — and
//! Comment › Create ▾ with the reference's six rows (Create Custom Stamp, Create Custom Dynamic
//! Stamp, Manage Stamps, Paste Clipboard Image as Stamp Tool, Show Stamps Palette, Set
//! Favorite Stamps). A chosen stamp is placed where the page is clicked
//! (`bo_pdfread::PdfDocument::add_stamp12`).
use crate::editor_comment::CommentTool;
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::{button, combo, edit, label, Form, On, Val};
use crate::editor_window::{inside, EditorWindow};
use crate::{Input, Painter, TextStyle};
use bo_pdfread::StampLook;
use bo_render::Rgba;

/// Palette size at 125 % (the reference's: 599 px wide).
const PAL_W: f32 = 599.0;
const CELL_W: f32 = 137.0;
const CELL_H: f32 = 40.0;
const CELL_X0: f32 = 5.0;
const PITCH_X: f32 = 144.0;
const PITCH_Y: f32 = 69.0;
/// From a section title's top to its first row.
const HEAD_H: f32 = 28.0;
/// From the last row of a section to the next title.
const GAP_Y: f32 = 26.0;

const GREEN: [u8; 3] = [0x1F, 0x7A, 0x4D];
const BLUE: [u8; 3] = [0x1E, 0x50, 0xB4];
const RED: [u8; 3] = [0xB8, 0x2E, 0x2E];
const TEAL: [u8; 3] = [0x1B, 0x8A, 0x78];

/// A palette stamp owned: title, colour, filled, dynamic.
pub(crate) type Own = (String, [u8; 3], bool, bool);

/// A stamp of the palette: title, colour, filled, dynamic (who and when on a second line).
type Def = (&'static str, [u8; 3], bool, bool);

const DYNAMIC: [Def; 5] = [
    ("Approved", GREEN, false, true),
    ("Revised", BLUE, false, true),
    ("Reviewed", BLUE, false, true),
    ("Received", BLUE, false, true),
    ("Confidential", RED, false, true),
];

const SIGN_HERE: [Def; 5] = [
    ("Accepted", TEAL, true, false),
    ("Witness", TEAL, true, false),
    ("Sign Here", TEAL, true, false),
    ("Rejected", RED, true, false),
    ("Initial", TEAL, true, false),
];

const STANDARD: [Def; 12] = [
    ("Approved", GREEN, false, false),
    ("Not Approved", RED, false, false),
    ("Draft", BLUE, false, false),
    ("Final", GREEN, false, false),
    ("Confidential", RED, false, false),
    ("For Comment", BLUE, false, false),
    ("Expired", RED, false, false),
    ("Experimental", BLUE, false, false),
    ("As Is", BLUE, false, false),
    ("Sold", BLUE, false, false),
    ("Departmental", BLUE, false, false),
    ("Top Secret", RED, false, false),
];

/// Colours offered for custom stamps.
const COLORS: [[u8; 3]; 4] = [BLUE, GREEN, RED, [0x55, 0x55, 0x55]];

/// Stamp palette state (round 12).
#[derive(Debug, Clone, Default)]
pub(crate) struct Stamps12 {
    /// The open palette (window rectangle).
    pub palette: Option<[f32; 4]>,
    /// Cell under the pointer: section and index.
    pub hover: Option<(usize, usize)>,
    /// The stamp the Stamp tool places.
    pub chosen: Option<(String, [u8; 3], bool, bool)>,
    /// The user's own stamps (title, colour, filled, dynamic), kept for the session.
    pub custom: Vec<(String, [u8; 3], bool, bool)>,
}

impl EditorWindow {
    /// Sections of the palette: title key and stamps.
    fn sections12(&self) -> Vec<(&'static str, Vec<Own>)> {
        let own = |d: &[Def]| {
            d.iter()
                .map(|(t, c, s, y)| ((*t).to_owned(), *c, *s, *y))
                .collect::<Vec<_>>()
        };
        let mut v = vec![
            ("pdfEditor12.dynamicStamps", own(&DYNAMIC)),
            ("pdfEditor12.signHere", own(&SIGN_HERE)),
            ("pdfEditor12.standardStamps", own(&STANDARD)),
        ];
        if !self.st12.custom.is_empty() {
            v.push(("pdfEditor12.customStamps", self.st12.custom.clone()));
        }
        v
    }

    /// Height of the palette at 125 %.
    fn palette_h12(&self) -> f32 {
        let mut h = 6.0;
        for (_, s) in self.sections12() {
            let rows = s.len().div_ceil(4) as f32;
            h += HEAD_H + rows * PITCH_Y - (PITCH_Y - CELL_H) + GAP_Y;
        }
        h
    }

    /// Cells of the palette (window rectangles) with their section and index.
    fn cells12(&self) -> Vec<([f32; 4], usize, usize)> {
        let Some(g) = self.st12.palette else {
            return Vec::new();
        };
        let s = self.ui_scale() / 1.25;
        let mut out = Vec::new();
        let mut y = 6.0;
        for (k, (_, stamps)) in self.sections12().iter().enumerate() {
            y += HEAD_H;
            for (i, _) in stamps.iter().enumerate() {
                let (col, row) = ((i % 4) as f32, (i / 4) as f32);
                out.push((
                    [
                        g[0] + ((CELL_X0 + col * PITCH_X) * s).round(),
                        g[1] + ((y + row * PITCH_Y) * s).round(),
                        (CELL_W * s).round(),
                        (CELL_H * s).round(),
                    ],
                    k,
                    i,
                ));
            }
            y += stamps.len().div_ceil(4) as f32 * PITCH_Y - (PITCH_Y - CELL_H) + GAP_Y;
        }
        out
    }

    /// Open (or close) the palette under the Stamp button.
    pub(crate) fn toggle_palette12(&mut self) {
        if self.st12.palette.take().is_some() {
            return;
        }
        let b = self.r.control_rect("cm.stamp").unwrap_or([
            self.window_size().0 / 2.0,
            120.0,
            10.0,
            10.0,
        ]);
        let s = self.ui_scale() / 1.25;
        let (w, h) = ((PAL_W * s).round(), (self.palette_h12() * s).round());
        let win = self.window_size();
        let x = b[0].min(win.0 - w - 2.0).max(0.0);
        let y = (b[1] + b[3]).min((win.1 - h).max(0.0));
        self.st12.palette = Some([x, y, w, h]);
        self.st12.hover = None;
    }

    /// Commands of round 12's stamps; false when not one.
    pub(crate) fn stamps12_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "cm.stamp" | "st12.palette" => self.toggle_palette12(),
            "cm.createMenu" => self.menu10(
                &[
                    (
                        "st12.create",
                        "pdfEditor12.createStamp",
                        Some("customStamp"),
                        false,
                    ),
                    (
                        "st12.createDynamic",
                        "pdfEditor12.createDynamic",
                        Some("customStamp"),
                        false,
                    ),
                    (
                        "st12.manage",
                        "pdfEditor12.manageStamps",
                        Some("stamp"),
                        self.st12.custom.is_empty(),
                    ),
                    ("st12.paste", "pdfEditor12.pasteStamp", Some("paste"), true),
                    (
                        "st12.palette",
                        "pdfEditor12.showPalette",
                        Some("stamp"),
                        false,
                    ),
                    (
                        "st12.favorites",
                        "pdfEditor12.favoriteStamps",
                        Some("stamp"),
                        true,
                    ),
                ],
                Vec::new(),
                "cm.createMenu",
            ),
            "st12.create" => self.open_create12(false),
            "st12.createDynamic" => self.open_create12(true),
            "st12.manage" => self.open_manage12(),
            _ => return false,
        }
        true
    }

    fn open_create12(&mut self, dynamic: bool) {
        let t = |k: &str| self.t(k);
        let c = vec![
            label(24.0, 70.0, t("pdfEditor12.stampTitle")),
            edit("title", [24.0, 90.0, 405.0, 29.0], On::Always),
            label(24.0, 145.0, t("pdfEditor12.stampColor")),
            combo(
                "color",
                [130.0, 131.0, 160.0, 28.0],
                vec![
                    t("pdfEditor12.blue"),
                    t("pdfEditor12.green"),
                    t("pdfEditor12.red"),
                    t("pdfEditor12.grey"),
                ],
                On::Always,
            ),
            label(24.0, 185.0, t("pdfEditor12.stampStyle")),
            combo(
                "style",
                [130.0, 171.0, 160.0, 28.0],
                vec![t("pdfEditor12.frame"), t("pdfEditor12.filled")],
                On::Always,
            ),
            button(
                "ok",
                [243.0, 226.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
            button(
                "cancel",
                [343.0, 226.0, 91.0, 26.0],
                t("pdfReader.cancel"),
                On::Always,
            ),
        ];
        let title = if dynamic {
            t("pdfEditor12.createDynamic")
        } else {
            t("pdfEditor12.createStamp")
        };
        let f = Form::new(title, (453.0, 272.0), c)
            .with("color", Val::Pick(0))
            .with("style", Val::Pick(0))
            .focused("title");
        self.form8.push((
            f,
            FormWhat::R12(crate::editor_fields12::Dlg12::Stamp(dynamic)),
        ));
    }

    fn open_manage12(&mut self) {
        let t = |k: &str| self.t(k);
        let names: Vec<String> = self.st12.custom.iter().map(|c| c.0.clone()).collect();
        let c = vec![
            label(24.0, 70.0, t("pdfEditor12.customStamps")),
            combo("which", [24.0, 90.0, 300.0, 28.0], names, On::Always),
            button(
                "delete12",
                [338.0, 90.0, 91.0, 28.0],
                t("pdfEditor12.delete"),
                On::Always,
            ),
            button(
                "ok",
                [343.0, 150.0, 91.0, 26.0],
                t("pdfReader.ok"),
                On::Always,
            ),
        ];
        let f =
            Form::new(t("pdfEditor12.manageStamps"), (453.0, 196.0), c).with("which", Val::Pick(0));
        self.form8.push((
            f,
            FormWhat::R12(crate::editor_fields12::Dlg12::ManageStamps),
        ));
    }

    /// OK of Create Custom (Dynamic) Stamp: the stamp joins the palette's Custom section and
    /// becomes the Stamp tool's.
    pub(crate) fn create12_ok(&mut self, f: &Form, dynamic: bool) {
        let title = f.text("title").trim().to_owned();
        if title.is_empty() {
            return;
        }
        let color = COLORS.get(f.pick("color")).copied().unwrap_or(BLUE);
        let def = (title, color, f.pick("style") == 1, dynamic);
        self.st12.custom.retain(|c| c.0 != def.0);
        self.st12.custom.push(def.clone());
        self.choose_stamp12(def);
    }

    /// Manage Stamps › Delete.
    pub(crate) fn delete_stamp12(&mut self) {
        let Some((f, _)) = self.form8.last_mut() else {
            return;
        };
        let k = f.pick("which");
        if k < self.st12.custom.len() {
            self.st12.custom.remove(k);
        }
        let names: Vec<String> = self.st12.custom.iter().map(|c| c.0.clone()).collect();
        f.set_items("which", &names);
        f.set("which", Val::Pick(0));
    }

    fn choose_stamp12(&mut self, def: (String, [u8; 3], bool, bool)) {
        self.st12.chosen = Some(def);
        self.set_tool(None);
        self.design.tool = None;
        self.r.set_tool(crate::reader_window::Tool::Hand);
        self.comment.tool = Some(CommentTool::Stamp);
        self.comment.points = None;
    }

    /// Input of the palette and of the Stamp tool with a palette stamp; true when used.
    pub(crate) fn stamps12_input(&mut self, input: &Input) -> bool {
        if let Some(g) = self.st12.palette {
            match *input {
                Input::Move { x, y } => {
                    let h = self
                        .cells12()
                        .into_iter()
                        .find(|c| inside(c.0, x, y))
                        .map(|c| (c.1, c.2));
                    let changed = h != self.st12.hover;
                    self.st12.hover = h;
                    return changed || inside(g, x, y);
                }
                Input::Press { x, y, .. } => {
                    let hit = self.cells12().into_iter().find(|c| inside(c.0, x, y));
                    self.st12.palette = None;
                    if let Some((_, k, i)) = hit {
                        if let Some(def) = self.sections12().get(k).and_then(|s| s.1.get(i)) {
                            self.choose_stamp12(def.clone());
                        }
                        return true;
                    }
                    return self
                        .r
                        .control_rect("cm.stamp")
                        .is_some_and(|b| inside(b, x, y))
                        || inside(g, x, y);
                }
                Input::Key(crate::Key::Escape, _) => {
                    self.st12.palette = None;
                    return true;
                }
                Input::Release { x, y } | Input::Drag { x, y } => return inside(g, x, y),
                _ => return false,
            }
        }
        if self.comment.tool != Some(CommentTool::Stamp) || !self.r.showing_doc() {
            return false;
        }
        let Some(def) = self.st12.chosen.clone() else {
            return false;
        };
        let Input::Press { x, y, .. } = *input else {
            return false;
        };
        if !inside(self.r.doc_area(), x, y) {
            return false;
        }
        let Some((page, px, py)) = self.view().and_then(|v| v.window_to_doc(x, y)) else {
            return false;
        };
        let author = self.prefs.author.clone();
        let sub = if def.3 {
            let who = if author.is_empty() {
                "BlueEditor".to_owned()
            } else {
                author.clone()
            };
            format!("{who}, {}", now_text())
        } else {
            String::new()
        };
        let look = StampLook {
            title: def.0,
            sub,
            color: def.1,
            solid: def.2,
        };
        let h = if look.sub.is_empty() { 28.0 } else { 36.0 };
        match self.doc_edit(|d| d.add_stamp12(page, [px, py - h / 2.0], h, &look, &author)) {
            Ok(_) => {
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(page);
                }
                self.after_doc_edit();
            }
            Err(e) => self.set_status(&e),
        }
        true
    }

    /// The open palette (over the ribbon and the page).
    pub(crate) fn paint_stamps12(&mut self, p: &mut dyn Painter) {
        let Some(g) = self.st12.palette else { return };
        let k = self.ui_scale();
        let s = k / 1.25;
        let q = |v: f32| (v * s).round();
        for (d, a) in [(3.0, 14u8), (2.0, 22), (1.0, 32)] {
            p.fill_rect(
                g[0] - d,
                g[1] - d + 1.0,
                g[2] + 2.0 * d,
                g[3] + 2.0 * d,
                Rgba(0, 0, 0, a),
            );
        }
        p.fill_rect(g[0], g[1], g[2], g[3], Rgba(0xE0, 0xE0, 0xE0, 255));
        p.fill_rect(
            g[0] + 1.0,
            g[1] + 1.0,
            g[2] - 2.0,
            g[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let head = TextStyle {
            size: (15.0 * s).round(),
            bold: true,
        };
        let sections = self.sections12();
        let cells = self.cells12();
        let mut y = 6.0;
        for (k2, (key, stamps)) in sections.iter().enumerate() {
            p.text(
                &self.t(key),
                g[0] + q(13.0),
                g[1] + q(y + 2.0),
                head,
                Rgba(0x3B, 0x3B, 0x3B, 255),
            );
            y += HEAD_H + stamps.len().div_ceil(4) as f32 * PITCH_Y - (PITCH_Y - CELL_H) + GAP_Y;
            for (c, sk, i) in cells.iter().filter(|c| c.1 == k2) {
                let Some((title, color, solid, dynamic)) = stamps.get(*i) else {
                    continue;
                };
                if self.st12.hover == Some((*sk, *i)) {
                    p.fill_rect(
                        c[0] - 3.0,
                        c[1] - 3.0,
                        c[2] + 6.0,
                        c[3] + 6.0,
                        Rgba(0xE2, 0xE2, 0xE2, 255),
                    );
                }
                paint_stamp_cell(p, *c, title, *color, *solid, *dynamic, s);
            }
        }
    }
}

/// "YYYY-MM-DD HH:MM" now (UTC; the host shows local time in its own dialogs).
fn now_text() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let (h, m) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = yoe + era * 400 + i64::from(mo <= 2);
    format!("{yr:04}-{mo:02}-{d:02} {h:02}:{m:02}")
}

/// One palette cell: our own stamp drawing (a rounded frame, or a filled band) with the title
/// in capitals and, for dynamic stamps, a small "name, date" line.
fn paint_stamp_cell(
    p: &mut dyn Painter,
    c: [f32; 4],
    title: &str,
    color: [u8; 3],
    solid: bool,
    dynamic: bool,
    s: f32,
) {
    let col = Rgba(color[0], color[1], color[2], 255);
    let r = (6.0 * s).round();
    let path = crate::reader_window::rounded(c[0] + 1.0, c[1] + 1.0, c[2] - 2.0, c[3] - 2.0, r);
    if solid {
        p.path(&path, Some(col), None);
    } else {
        p.path(
            &path,
            Some(Rgba(255, 255, 255, 255)),
            Some((col, (2.0 * s).max(1.5))),
        );
    }
    let ink = if solid { Rgba(255, 255, 255, 255) } else { col };
    let upper = title.to_uppercase();
    let mut size = if dynamic {
        (13.0 * s).round()
    } else {
        (16.0 * s).round()
    };
    // Long titles ("NOT APPROVED", "DEPARTMENTAL") shrink to stay inside the frame with 6 px
    // on each side (the estimate runs 10-18 % wide on capitals: APPROVED 87 px drawn,
    // 96 px estimated).
    let room = c[2] - (12.0 * s).round();
    let est = |size: f32| crate::editor_form8::text_w(&upper, size * 1.1) * 0.88;
    while est(size) > room && size > 8.0 {
        size -= 1.0;
    }
    let st = TextStyle { size, bold: false };
    let tw = est(size);
    let ty = if dynamic {
        c[1] + (3.0 * s).round()
    } else {
        c[1] + ((c[3] - size * 1.33) / 2.0).round()
    };
    p.push_clip(c[0], c[1], c[2], c[3]);
    p.text(&upper, (c[0] + (c[2] - tw) / 2.0).round(), ty, st, ink);
    if dynamic {
        let small = TextStyle {
            size: (9.0 * s).round(),
            bold: true,
        };
        let line = "NAME, DATE TIME";
        let lw = crate::editor_form8::text_w(line, small.size * 1.1);
        p.text(
            line,
            (c[0] + (c[2] - lw) / 2.0).round(),
            c[1] + c[3] - (15.0 * s).round(),
            small,
            ink,
        );
    }
    p.pop_clip();
}

#[cfg(test)]
mod tests {
    use crate::editor_tests::{window, Fixed, Rec};
    use crate::Input;

    #[test]
    fn palette_places_a_stamp_where_the_page_is_clicked() {
        let mut w = window(1);
        w.run("cm.stamp", &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        for l in [
            "Dynamic Stamps",
            "Sign Here",
            "Standard Stamps",
            "SIGN HERE",
            "APPROVED",
        ] {
            assert!(rec.texts.iter().any(|t| t == l), "{l}: {:?}", rec.texts);
        }
        // The third Sign Here cell.
        let cell = w
            .cells12()
            .into_iter()
            .find(|c| c.1 == 1 && c.2 == 2)
            .expect("cell")
            .0;
        w.handle(
            Input::Press {
                x: cell[0] + 5.0,
                y: cell[1] + 5.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        assert!(w.st12.palette.is_none());
        assert_eq!(
            w.st12.chosen.as_ref().map(|c| c.0.as_str()),
            Some("Sign Here")
        );
        let a = w.r.doc_area();
        let (x, y) = (a[0] + a[2] / 2.0, a[1] + a[3] / 2.0);
        w.handle(
            Input::Press {
                x,
                y,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        let n = w.view().expect("view").doc().annotations().len();
        assert_eq!(n, 1);
    }

    #[test]
    fn create_menu_has_six_rows_and_custom_stamps_join_the_palette() {
        let mut w = window(1);
        // The Comment tab shows the Create button the menu opens under.
        w.show_tab(4, &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        w.run("cm.createMenu", &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        for l in [
            "Create Custom Stamp",
            "Create Custom Dynamic Stamp",
            "Manage Stamps",
            "Paste Clipboard Image as Stamp Tool",
            "Show Stamps Palette",
            "Set Favorite Stamps",
        ] {
            assert!(rec.texts.iter().any(|t| t == l), "{l}: {:?}", rec.texts);
        }
        w.run("st12.create", &Fixed);
        {
            let f = &mut w.form8.last_mut().expect("dialog").0;
            f.set("title", crate::editor_form8::Val::Text("Paid".into()));
        }
        w.handle(
            Input::Key(crate::Key::Enter, crate::Mods::default()),
            &Fixed,
        );
        assert_eq!(w.st12.custom.len(), 1);
        w.run("cm.stamp", &Fixed);
        let mut rec = Rec::default();
        w.paint(&mut rec);
        assert!(rec.texts.iter().any(|t| t == "PAID"));
    }
}
