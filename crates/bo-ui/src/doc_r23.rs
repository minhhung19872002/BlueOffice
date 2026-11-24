//! BlueDoc parity round 23 (docs/ui/REPORT.md "Vòng 113"): the entry-by-entry inventory of
//! every drop-down and context menu against the reference word processor, and the Home tab
//! drop-downs that were missing: Bullets ▾ / Numbering ▾ / Multilevel List ▾ libraries with
//! Change List Level ▸, Define New…, Set Numbering Value…; Text Highlight Color ▾; Shading ▾
//! as a colour palette; the full Borders ▾ list; Line Spacing Options…; Sort… as the Sort
//! Text dialog; Editing › Find ▾ and Select ▾. List libraries are §17.9.1 abstractNum /
//! §17.9.6 lvl definitions (`defineList`), highlight colours §17.3.2.15 / §17.18.40
//! `ST_HighlightColor`, shading §17.3.1.31 shd. A child module of the shell.
use super::DocWindow;
use crate::catalog::Entry;
use crate::doc_thumbs22::{Cell, Section, Thumb, ThumbGallery};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::Input;
use bo_engine::edit::Command;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

#[cfg(test)]
#[path = "doc_r23_inv_tests.rs"]
pub(super) mod inv_tests;

#[cfg(test)]
#[path = "doc_r23_tests.rs"]
mod tests;

/// Round-23 state of the window.
#[derive(Default)]
pub(super) struct R23 {
    /// Text Highlight Color: the colour the button applies (`RRGGBB`; none: yellow).
    pub(super) highlight: Option<String>,
    /// Where the last list gallery opened (Change List Level ▸ opens beside it).
    gallery_at: [f32; 4],
    /// The open Shading palette.
    pub(super) shading: Option<Box<crate::doc_wm14::PagePalette>>,
    /// Select ▾ › Select Objects: presses on the page select drawing objects.
    pub(super) select_objects: bool,
    /// Where the last press was (a split button's part is told by it; taken once).
    pub(super) press: Option<(f32, f32)>,
}

/// Menu entries of round 23 that the window runs itself (no engine command of their own).
#[cfg(test)]
pub(crate) const WINDOW: [&str; 2] = ["layout.lineNumbers.suppress", "layout.lineNumbers.options"];

/// Split buttons of Home: the left part runs the command, the arrow part opens its menu.
pub(crate) const SPLIT: [&str; 5] = [
    "format.underline",
    "format.bullets",
    "format.numbering",
    "format.highlight",
    "edit.find",
];

/// The reference's 15 highlight colours (§17.18.40 `ST_HighlightColor`), in its 5 × 3 grid
/// order: (hex, key, English).
pub(crate) const HIGHLIGHTS: [(&str, &str, &str); 15] = [
    ("FFFF00", "doc23.hlYellow", "Yellow"),
    ("00FF00", "doc23.hlBrightGreen", "Bright Green"),
    ("00FFFF", "doc23.hlTurquoise", "Turquoise"),
    ("FF00FF", "doc23.hlPink", "Pink"),
    ("0000FF", "doc23.hlBlue", "Blue"),
    ("FF0000", "doc23.hlRed", "Red"),
    ("000080", "doc23.hlDarkBlue", "Dark Blue"),
    ("008080", "doc23.hlTeal", "Teal"),
    ("008000", "doc23.hlGreen", "Green"),
    ("800080", "doc23.hlViolet", "Violet"),
    ("800000", "doc23.hlDarkRed", "Dark Red"),
    ("808000", "doc23.hlDarkYellow", "Dark Yellow"),
    ("808080", "doc23.hlGray50", "Gray-50%"),
    ("C0C0C0", "doc23.hlGray25", "Gray-25%"),
    ("000000", "doc23.hlBlack", "Black"),
];

/// Bullet Library: (bullet character, key, English) after None.
pub(crate) const BULLETS: [(&str, &str, &str); 6] = [
    ("\u{25CF}", "doc23.bDot", "Filled Round Bullet"),
    ("\u{25CB}", "doc23.bCircle", "Hollow Round Bullet"),
    ("\u{25A0}", "doc23.bSquare", "Filled Square Bullet"),
    ("\u{2756}", "doc23.bDiamonds", "Four Diamonds Bullet"),
    ("\u{27A2}", "doc23.bArrow", "Arrow Bullet"),
    ("\u{2713}", "doc23.bCheck", "Check Mark Bullet"),
];

/// Numbering Library: (§17.18.59 numFmt, level text, sample) after None.
pub(crate) const NUMBERS: [(&str, &str, [&str; 3]); 7] = [
    ("decimal", "%1.", ["1.", "2.", "3."]),
    ("decimal", "%1)", ["1)", "2)", "3)"]),
    ("upperRoman", "%1.", ["I.", "II.", "III."]),
    ("upperLetter", "%1.", ["A.", "B.", "C."]),
    ("lowerLetter", "%1)", ["a)", "b)", "c)"]),
    ("lowerLetter", "%1.", ["a.", "b.", "c."]),
    ("lowerRoman", "%1.", ["i.", "ii.", "iii."]),
];

/// List Library of Multilevel List ▾: (id, samples of levels 1–3).
pub(crate) const MULTILEVEL: [(&str, [&str; 3]); 4] = [
    ("outline", ["1.", "1.1.", "1.1.1."]),
    ("mixed", ["1)", "a)", "i)"]),
    ("bullets", ["\u{2022}", "\u{25E6}", "\u{25AA}"]),
    (
        "headings",
        ["1 Heading 1", "1.1 Heading 2", "1.1.1 Heading 3"],
    ),
];

/// Text effect presets of Text Effects and Typography ▾ (BlueOffice's own combinations):
/// fill, outline, halo (black: a shadow; other: a glow).
pub(crate) const TEXT_PRESETS: [(&str, Option<&str>, Option<&str>); 15] = [
    ("000000", None, Some("000000")),
    ("4472C4", None, Some("000000")),
    ("ED7D31", None, Some("000000")),
    ("FFFFFF", Some("4472C4"), None),
    ("FFFFFF", Some("ED7D31"), None),
    ("FFFFFF", Some("000000"), Some("000000")),
    ("4472C4", Some("1F3864"), None),
    ("ED7D31", Some("843C0C"), None),
    ("A5A5A5", Some("404040"), None),
    ("FFC000", None, Some("FFC000")),
    ("5B9BD5", None, Some("5B9BD5")),
    ("70AD47", None, Some("70AD47")),
    ("1F3864", None, Some("8FAADC")),
    ("C00000", None, Some("F4B183")),
    ("FFFFFF", Some("7F7F7F"), Some("A5A5A5")),
];

/// Entries of a round-23 menu button (the catalog lists them so the ribbon draws the
/// drop-down mark); `None` for other ids.
pub(crate) fn menu(id: &str) -> Option<Vec<Entry>> {
    const fn e(id: &'static str, label: &'static str) -> Entry {
        Entry { id, label }
    }
    Some(match id {
        "format.bullets" => vec![
            e("r23.bullet:none", "doc23.none"),
            e("r23.levels", "doc23.changeLevel"),
            e("r23.defineBullet", "doc23.defineBullet"),
        ],
        "format.numbering" => vec![
            e("r23.number:none", "doc23.none"),
            e("r23.levels", "doc23.changeLevel"),
            e("r23.defineNumber", "doc23.defineNumber"),
            e("r23.setValue", "doc23.setValue"),
        ],
        "format.highlight" => vec![e("r23.hl:none", "doc23.noColor")],
        "format.underline" => vec![
            e("r23.ul:single", "doc23.uSingle"),
            e("r23.ulMore", "doc23.moreUnderlines"),
        ],
        "edit.find" => vec![
            e("edit.find", "doc23.find"),
            e("r23.advancedFind", "doc23.advancedFind"),
            e("edit.goTo", "doc23.goTo"),
        ],
        "edit.select" => vec![
            e("edit.selectAll", "doc23.selectAll"),
            e("r23.selectObjects", "doc23.selectObjects"),
            e("r23.selectSimilar", "doc23.selectSimilar"),
            e("layout.selectionPane", "doc23.selectionPane"),
        ],
        _ => return None,
    })
}

fn rgb(h: &str) -> Rgba {
    let v = u32::from_str_radix(h, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// A filled square swatch in the unit box.
fn swatch(hex: &str) -> Thumb {
    let edge = Rgba(0x80, 0x80, 0x80, 255);
    Thumb::Paths(vec![(
        vec![
            PathSeg::M(0.0, 0.0),
            PathSeg::L(1.0, 0.0),
            PathSeg::L(1.0, 1.0),
            PathSeg::L(0.0, 1.0),
            PathSeg::Z,
        ],
        Some(rgb(hex)),
        Some((edge, 1.0)),
    )])
}

/// The thin frame of a library cell (the reference draws one around each entry).
fn frame() -> crate::doc_thumbs22::ThumbPath {
    (
        vec![
            PathSeg::M(-0.06, -0.06),
            PathSeg::L(1.06, -0.06),
            PathSeg::L(1.06, 1.06),
            PathSeg::L(-0.06, 1.06),
            PathSeg::Z,
        ],
        None,
        Some((Rgba(0xc8, 0xc8, 0xc8, 255), 1.0)),
    )
}

/// A list sample cell: three markers, each followed by a grey line (BlueOffice's drawing of
/// a list), with the markers drawn as text.
fn list_thumb(markers: [&str; 3], indent: bool) -> Thumb {
    let grey = Rgba(0x9a, 0x9a, 0x9a, 255);
    let ink = Rgba(0x26, 0x26, 0x26, 255);
    let mut texts = Vec::new();
    let mut paths = Vec::new();
    for (i, m) in markers.iter().enumerate() {
        let y = 0.18 + i as f32 * 0.32;
        let x = if indent { 0.02 + i as f32 * 0.12 } else { 0.02 };
        let wide = m.chars().count() > 4;
        texts.push((
            (*m).to_owned(),
            x,
            y - 0.13,
            if wide { 0.2 } else { 0.26 },
            ink,
        ));
        if !wide {
            let lx = x + 0.08 + 0.07 * m.chars().count() as f32;
            paths.push((
                vec![PathSeg::M(lx, y), PathSeg::L(0.98, y)],
                None,
                Some((grey, 1.0)),
            ));
        }
    }
    paths.push(frame());
    Thumb::Text { texts, paths }
}

impl DocWindow {
    fn r23_t(&self, key: &str, en: &str) -> String {
        self.t(key, en)
    }

    /// The ribbon rectangle of `id` (the window's middle top when it is not shown).
    fn r23_anchor(&self, id: &'static str) -> [f32; 4] {
        self.ribbon.rect_of(RibbonHit::Command(id)).unwrap_or([
            self.size.0 / 3.0,
            120.0,
            40.0,
            30.0,
        ])
    }

    /// The press that ran `id` was on the drop-down part of its ribbon split button.
    fn r23_on_arrow(&mut self, id: &'static str) -> bool {
        let Some((x, y)) = self.r23.press.take() else {
            return false;
        };
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(id)) else {
            return false;
        };
        let inside = x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        inside && x >= r[0] + r[2] * 0.6
    }

    fn r23_gallery(
        &mut self,
        owner: &'static str,
        sections: Vec<Section>,
        footer: Vec<(String, String)>,
        grid: (usize, (f32, f32)),
    ) -> bool {
        self.menu = None;
        let anchor = self.r23_anchor(owner);
        // Change List Level and Set Numbering Value are greyed outside a list (reference).
        let in_list = self.r23_list_level().is_some();
        let dim = footer
            .iter()
            .enumerate()
            .filter(|(_, (_, c))| {
                (!in_list && (c == "r23.levels" || c == "r23.setValue"))
                    // Round 24: Stop Highlighting only while the highlighter pen is on.
                    || (c == "r23.hl:stop" && !self.r24.pen)
            })
            .map(|(i, _)| i)
            .collect();
        let g = ThumbGallery::open(sections, footer, grid, anchor, self.size, self.scale)
            .with_dim_footer(dim);
        self.r23.gallery_at = g.rect();
        self.r22.gallery = Some(g);
        true
    }

    /// A menu of `(label, command, dim)` rows under `owner` (or at `at`), with separators
    /// before the rows listed.
    fn r23_menu(
        &mut self,
        rows: Vec<(String, String, bool)>,
        at: [f32; 4],
        seps: Vec<usize>,
    ) -> bool {
        let n = rows.len();
        let items = rows
            .into_iter()
            .map(|(label, cmd, dim)| MenuItem {
                label,
                choice: Choice::Command(cmd),
                checked: false,
                dim,
            })
            .collect();
        self.menu =
            Some(Menu::open(items, at, self.size, self.scale).with_decor(vec![None; n], seps));
        true
    }

    /// Round-23 commands; `None` passes `id` on to the earlier rounds.
    pub(super) fn r23_action(&mut self, id: &str) -> Option<bool> {
        use crate::doc_forms::DocFormKind as K;
        if let Some(&split) = SPLIT.iter().find(|s| **s == id) {
            if self.r23_on_arrow(split) {
                return Some(match split {
                    "format.bullets" => self.r23_bullets(),
                    "format.numbering" => self.r23_numbering(),
                    "format.highlight" => self.r23_highlights(),
                    "format.underline" => self.r23_underlines(),
                    _ => self.r23_find_menu(),
                });
            }
            return match split {
                // The button applies the colour last picked (the reference's split button);
                // on text with that colour it takes it off.
                "format.highlight" => {
                    let hex = self
                        .r23
                        .highlight
                        .clone()
                        .unwrap_or_else(|| "FFFF00".to_owned());
                    let on = self.view.format().highlight.as_deref() == Some(hex.as_str());
                    Some(self.r23_highlight(if on { None } else { Some(hex) }))
                }
                // The catalog lists these as menus (for the drop-down mark): run the
                // button's own command here.
                "format.underline" | "format.bullets" | "format.numbering" => {
                    crate::commands::command(split).map(|c| self.view.run_command(c))
                }
                _ => None,
            };
        }
        if let Some(v) = id.strip_prefix("r23.bullet:") {
            return Some(self.r23_apply_bullet(v));
        }
        if let Some(v) = id.strip_prefix("r23.number:") {
            return Some(self.r23_apply_number(v));
        }
        if let Some(v) = id.strip_prefix("r23.level:") {
            return Some(self.r23_set_level(v.parse().unwrap_or(0)));
        }
        if let Some(v) = id.strip_prefix("r23.ul:") {
            self.menu = None;
            let patch = json!({ "underline": v });
            return Some(self.r3_run(vec![json!({ "cmd": "formatText",
                "format": { "kind": "advanced", "patch": patch } })]));
        }
        if let Some(v) = id.strip_prefix("r23.tefxMenu:") {
            let v = v.to_owned();
            return Some(self.r23_text_effects_menu(&v));
        }
        if let Some(v) = id.strip_prefix("r23.tefx:") {
            let v = v.to_owned();
            return Some(self.r23_apply_text_effect(&v));
        }
        // The drop-down part of a split button by command (keyboard access, scripts).
        if let Some(v) = id.strip_prefix("r23.open:") {
            return Some(match v {
                "format.bullets" => self.r23_bullets(),
                "format.numbering" => self.r23_numbering(),
                "format.highlight" => self.r23_highlights(),
                "format.underline" => self.r23_underlines(),
                "edit.find" => self.r23_find_menu(),
                _ => false,
            });
        }
        if let Some(v) = id.strip_prefix("r23.selObj:") {
            let (p, a) = v.split_once(':').unwrap_or(("0", "0"));
            let (p, a) = (p.parse().unwrap_or(0), a.parse().unwrap_or(0));
            let sel = self
                .view
                .engine()
                .session()
                .and_then(|s| s.drawing_selection(p, a));
            return Some(sel.is_some_and(|selection| {
                self.view.run_command(Command::SetSelection { selection })
            }));
        }
        if let Some(v) = id.strip_prefix("r23.qatAdd:") {
            self.menu = None;
            let mut p = self.r6.prefs.clone();
            if p.quick.iter().any(|q| q == v) {
                return Some(false);
            }
            p.quick.push(v.to_owned());
            self.r6_set_quick(&p.quick);
            self.r6_store_prefs(p);
            return Some(true);
        }
        if let Some(v) = id.strip_prefix("r23.tb:") {
            let v = v.to_owned();
            return Some(self.r23_table_borders(&v));
        }
        if let Some(v) = id.strip_prefix("r23.hl:") {
            return Some(match v {
                "none" => self.r23_highlight(None),
                "stop" => {
                    self.menu = None;
                    true
                }
                hex => {
                    self.r23.highlight = Some(hex.to_owned());
                    self.r23_highlight(Some(hex.to_owned()))
                }
            });
        }
        if let Some(v) = id.strip_prefix("r23.numfmt:") {
            let (fmt, text) = v.split_once('|').unwrap_or(("decimal", "%1."));
            let (fmt, text) = (fmt.to_owned(), text.to_owned());
            return Some(self.r23_define(&fmt, &text));
        }
        if let Some(v) = id.strip_prefix("r23.sort:") {
            let v = v.to_owned();
            return Some(self.r23_sort(&v));
        }
        Some(match id {
            "format.fontSettings" | "format.fontDialog" | "r23.ulMore" => {
                self.r23_open_form(K::Font23)
            }
            "format.sortParagraphs" => self.r23_open_form(K::SortText),
            "r23.defineBullet" => self.r23_open_form(K::DefineBullet),
            "r23.defineNumber" => self.r23_open_form(K::DefineNumber),
            "r23.setValue" => self.r23_open_form(K::SetNumbering),
            "r23.defineMultilevel" => self.r23_open_form(K::DefineMultilevel),
            "r23.defineListStyle" => self.r23_open_form(K::NewStyle),
            "layout.selectionPane" => self.r23_open_form(K::SelPane23),
            "layout.lineNumbers.options" => self.r23_open_form(K::LineNumbers),
            "layout.lineNumbers.suppress" => {
                self.menu = None;
                let on = !self.r23_suppressed();
                self.view.run_command(Command::FormatParagraph {
                    format: bo_edit::ParaFormat::SuppressLineNumbers { on },
                })
            }
            "insert.quickParts.field" => self.r23_open_form(K::Field23),
            "r23.qatCustomize" | "r23.ribbonCustomize" => {
                self.menu = None;
                self.r6_open_options();
                if let Some(o) = self.r6.options.as_mut() {
                    o.set_page(if id == "r23.qatCustomize" { 8 } else { 7 });
                }
                true
            }
            "r23.qatBelow" => {
                self.menu = None;
                let mut p = self.r6.prefs.clone();
                p.extra.qat_below = !p.extra.qat_below;
                self.r21_apply(&p);
                self.r6_store_prefs(p);
                true
            }
            // Size and Position… of a picture or shape: the Format pane's Layout &
            // Properties page (BlueDoc's Layout options live there).
            "picture.layoutOptions" => {
                self.menu = None;
                self.r21_open_pane(crate::doc_fmtpane21::Page::Layout, None, false)
            }
            // Alt Text of a shape (pictures: round 7).
            // Round 24: also when the right-click moved the caret off the shape (no picture
            // selected): the paragraph's shape.
            "picture.altText" if self.context.shape || !self.context.picture => {
                self.r23_shape_alt_text()
            }
            "r23.fontDefault" => {
                self.r3_run(vec![json!({ "cmd": "styleUpdateToMatch", "id": "Normal" })])
            }
            "r23.fieldUpdate" => {
                self.menu = None;
                self.r3_run(vec![json!({ "cmd": "updateFields", "selection": true })])
            }
            "r23.fieldToggle" => {
                self.menu = None;
                self.view.run_command(Command::ToggleFieldCodes)
            }
            "r23.listIndents" => {
                self.menu = None;
                self.r3_open_form(K::Paragraph)
            }
            "r23.restart1" | "r23.continue" => {
                self.menu = None;
                let numbering = if id == "r23.continue" {
                    bo_edit::ListNumbering::Continue
                } else {
                    bo_edit::ListNumbering::Restart { value: 1 }
                };
                self.view
                    .run_command(Command::SetListNumbering { numbering })
            }
            "format.textEffects" => self.r23_text_effects(),
            "format.multilevel" => self.r23_multilevel(),
            "format.lineSpacing" => self.r23_line_spacing(),
            "format.paraBorders" => self.r23_borders(),
            "format.shading" => self.r23_open_shading(),
            "edit.select" => self.r23_select_menu(),
            "r23.levels" => self.r23_levels_menu(),
            "r23.lineOptions" => {
                self.menu = None;
                self.r3_open_form(crate::doc_forms::DocFormKind::Paragraph)
            }
            "r23.advancedFind" => {
                self.menu = None;
                self.open_find_dialog(0)
            }
            "edit.selectAll" => {
                self.menu = None;
                self.view.run_command(Command::SelectAll)
            }
            "r23.selectObjects" => {
                self.menu = None;
                self.r23.select_objects = !self.r23.select_objects;
                true
            }
            "r23.selectSimilar" => self.r23_select_similar(),
            "format.paraBorders.insideH" => {
                self.r23_border_preset(bo_edit::ParaBorderPreset::Inside)
            }
            "format.paraBorders.horizontalLine" => self.r23_horizontal_line(),
            _ => return None,
        })
    }

    /// Right-click on the ribbon: Add to Quick Access Toolbar (on a command), Customize
    /// Quick Access Toolbar…, Show Quick Access Toolbar Below / Above the Ribbon, Customize
    /// the Ribbon… — the reference's ribbon menu.
    pub(super) fn r23_ribbon_menu(&mut self, x: f32, y: f32) -> bool {
        let mut rows: Vec<(String, String, bool)> = Vec::new();
        if let Some(RibbonHit::Command(id)) = self.ribbon.hit(x, y) {
            let on = self.r6.prefs.quick.iter().any(|q| q == id);
            rows.push((
                self.r23_t("doc23.qatAdd", "Add to Quick Access Toolbar"),
                format!("r23.qatAdd:{id}"),
                on,
            ));
        }
        rows.push((
            self.r23_t("doc23.qatCustomize", "Customize Quick Access Toolbar..."),
            "r23.qatCustomize".into(),
            false,
        ));
        rows.push((
            if self.r6.prefs.extra.qat_below {
                self.r23_t(
                    "doc23.qatAbove",
                    "Show Quick Access Toolbar Above the Ribbon",
                )
            } else {
                self.r23_t(
                    "doc23.qatBelow",
                    "Show Quick Access Toolbar Below the Ribbon",
                )
            },
            "r23.qatBelow".into(),
            false,
        ));
        let sep = rows.len();
        rows.push((
            self.r23_t("doc23.ribbonCustomize", "Customize the Ribbon..."),
            "r23.ribbonCustomize".into(),
            false,
        ));
        // Round 24: Collapse the Ribbon (Ctrl+F1) — checked while collapsed.
        rows.push((
            self.r23_t("doc24.collapseRibbon", "Collapse the Ribbon"),
            "r24.ribbon:toggle".into(),
            false,
        ));
        self.r23_menu(rows, [x, y, 1.0, 0.0], vec![sep])
    }

    /// Context menu rows of round 23 (label, command): `group` 0 a field under the caret,
    /// 1 a list paragraph, 2 a tracked change or a comment.
    pub(super) fn r23_ctx_rows(&self, group: u8) -> Vec<(String, String)> {
        let row = |k: &str, en: &str, id: &str| (self.r23_t(k, en), id.to_owned());
        let Some(s) = self.view.engine().session() else {
            return Vec::new();
        };
        match group {
            0 if s.field_at().is_some() => vec![
                row("doc23.updateField", "Update Field", "r23.fieldUpdate"),
                row("doc23.toggleCodes", "Toggle Field Codes", "r23.fieldToggle"),
            ],
            1 if self.r23_list_level().is_some() => {
                let mut v = vec![row(
                    "doc23.adjustIndents",
                    "Adjust List Indents...",
                    "r23.listIndents",
                )];
                if self.view.format().numbering {
                    v.push(row("doc23.restart1", "Restart at 1", "r23.restart1"));
                    v.push(row(
                        "doc23.continueNumbering",
                        "Continue Numbering",
                        "r23.continue",
                    ));
                    v.push(row(
                        "doc23.setValue",
                        "Set Numbering Value...",
                        "r23.setValue",
                    ));
                }
                v
            }
            2 => {
                let mut v = Vec::new();
                if !s.revisions_at_selection().is_empty() {
                    v.push(row("doc23.acceptChange", "Accept Change", "review.accept"));
                    v.push(row("doc23.rejectChange", "Reject Change", "review.reject"));
                }
                if self.view.comment_at_caret().is_some() {
                    v.push(row(
                        "doc23.deleteComment",
                        "Delete Comment",
                        "review.deleteComment",
                    ));
                }
                v
            }
            _ => Vec::new(),
        }
    }

    /// Alt Text… of the selected shape (§20.4.2.5 docPr descr), as for pictures.
    fn r23_shape_alt_text(&mut self) -> bool {
        self.menu = None;
        // Round 24: from inside a text box's text, Alt Text is the text box's.
        if self.view.story().is_some() {
            self.view.run_command(Command::CloseTextBox);
        }
        let Some(s) = self.view.engine().session() else {
            return false;
        };
        let (a, b) = s.selection().ordered();
        let old = s.selection();
        let Some(p) = bo_model::index::para_by_id(s.doc(), a.para) else {
            return false;
        };
        let text = bo_model::index::para_index_text(p);
        let end = if b.para == a.para {
            b.offset
        } else {
            text.len()
        };
        let object = bo_model::index::OBJECT;
        let hit = text
            .char_indices()
            .filter(|(i, c)| *c == object && *i + c.len_utf8() >= a.offset)
            .find(|(i, _)| *i <= end)
            .map(|(i, _)| i);
        let objects: Vec<usize> = text
            .char_indices()
            .filter(|(_, c)| *c == object)
            .map(|(i, _)| i)
            .collect();
        let para = a.para;
        // Round 24: else the paragraph's first object that is a shape (a right-click beside
        // a selected text box moves the caret off it): each object tried as the selection.
        let at = hit.or_else(|| {
            let found = objects.into_iter().find(|&i| {
                self.view.run_command(Command::SetSelection {
                    selection: bo_edit::Selection {
                        anchor: bo_edit::Pos::new(para, i),
                        focus: bo_edit::Pos::new(para, i + object.len_utf8()),
                    },
                });
                self.view
                    .engine()
                    .session()
                    .is_some_and(|s| s.tools_shape().is_some())
            });
            if found.is_none() {
                self.view
                    .run_command(Command::SetSelection { selection: old });
            }
            found
        });
        let Some(at) = at else {
            return false;
        };
        let desc = self
            .view
            .engine()
            .session()
            .and_then(bo_edit::Session::tools_shape)
            .map(|i| i.description)
            .unwrap_or_default();
        self.r3_open_form(crate::doc_forms::DocFormKind::AltText);
        if let Some(f) = self.r3.form.as_mut() {
            f.ids = vec![para.to_string(), at.to_string()];
            f.set_text("alt", &desc);
        }
        true
    }

    /// The caret's paragraph suppresses line numbers (§17.3.1.34 suppressLineNumbers).
    fn r23_suppressed(&self) -> bool {
        self.view
            .engine()
            .session()
            .and_then(|s| bo_model::index::para_by_id(s.doc(), s.selection().focus.para))
            .is_some_and(|p| p.props.suppress_line_numbers == Some(true))
    }

    /// The document's drawing objects: (name, "para:offset"), named by kind and number as
    /// the reference's Selection Pane does ("Picture 1", "Text Box 2", "Rectangle 3").
    fn r23_objects(&self) -> Vec<(String, String)> {
        let Some(s) = self.view.engine().session() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for para in 0..s.paragraph_count() {
            let Some(p) = bo_model::index::para_by_id(s.doc(), para) else {
                continue;
            };
            let text = bo_model::index::para_index_text(p);
            for (at, ch) in text.char_indices() {
                if ch != bo_model::index::OBJECT {
                    continue;
                }
                let Some(img) = s.drawing(para, at) else {
                    continue;
                };
                let kind = match &img.shape {
                    Some(sh) if sh.text.is_some() => self.r23_t("doc23.objTextBox", "Text Box"),
                    Some(_) => self.r23_t("doc23.objShape", "Shape"),
                    None => self.r23_t("doc23.objPicture", "Picture"),
                };
                let name = format!("{kind} {}", out.len() + 1);
                out.push((name, format!("{para}:{at}")));
            }
        }
        out
    }

    /// Open round-23 dialog `kind` with the selection's state.
    fn r23_open_form(&mut self, kind: crate::doc_forms::DocFormKind) -> bool {
        self.menu = None;
        self.r22.gallery = None;
        let mut c = self.form_context();
        c.extra["r23"] = crate::doc_forms::r23::extra(self.view.format());
        if kind == crate::doc_forms::DocFormKind::SelPane23 {
            c.extra["r23"]["objects"] = json!(self.r23_objects());
        }
        if let Some(s) = self.view.engine().session() {
            let t = &s.doc().theme_fonts;
            c.extra["r23"]["themeBody"] = json!(t.minor_latin);
            c.extra["r23"]["themeHeadings"] = json!(t.major_latin);
            // Round 24: the layout mode (OpenType features need 14 or later).
            c.extra["r23"]["compat"] = json!(s.doc().compat_mode);
        }
        if kind == crate::doc_forms::DocFormKind::Font23 {
            c.fonts = self.view.font_list().into_iter().map(|f| f.name).collect();
        }
        let f = crate::doc_forms::open(kind, &self.strings, &c);
        self.r3.form = Some(f);
        self.relayout_pending = true;
        true
    }

    /// Sort Text OK: the selected paragraphs (less the first with a header row) sorted.
    fn r23_sort(&mut self, v: &str) -> bool {
        let descending = v.starts_with("desc");
        if v.contains(":header") {
            if let Some(s) = self.view.engine().session() {
                let sel = s.selection();
                let (a, b) = if sel.anchor.para <= sel.focus.para {
                    (sel.anchor, sel.focus)
                } else {
                    (sel.focus, sel.anchor)
                };
                if b.para > a.para {
                    let selection = bo_edit::Selection {
                        anchor: bo_edit::Pos::new(a.para + 1, 0),
                        focus: b,
                    };
                    self.view.run_command(Command::SetSelection { selection });
                }
            }
        }
        // Round 24: Type Number / Date.
        let by = if v.ends_with(":number") {
            bo_edit::sort_by::SortType::Number
        } else if v.ends_with(":date") {
            bo_edit::sort_by::SortType::Date
        } else {
            return self
                .view
                .run_command(Command::SortParagraphs { descending });
        };
        // Numeric dates read day first in the Vietnamese interface (as its short date).
        let day_first = self.locale_day_first();
        self.view.run_command(Command::SortParagraphsBy {
            descending,
            by,
            day_first,
        })
    }

    // ----- Bullets / Numbering / Multilevel List -----

    fn r23_bullets(&mut self) -> bool {
        let none = self.r23_t("doc23.none", "None");
        let mut cells = vec![Cell {
            thumb: Thumb::Text {
                texts: vec![(none.clone(), 0.18, 0.32, 0.3, Rgba(0x26, 0x26, 0x26, 255))],
                paths: Vec::new(),
            },
            command: "r23.bullet:none".into(),
            tip: none,
            checked: !self.view.format().bullets,
        }];
        for (ch, k, en) in BULLETS {
            cells.push(Cell {
                thumb: Thumb::Text {
                    texts: vec![(ch.to_owned(), 0.3, 0.12, 0.6, Rgba(0x26, 0x26, 0x26, 255))],
                    paths: vec![frame()],
                },
                command: format!("r23.bullet:{ch}"),
                tip: self.r23_t(k, en),
                checked: false,
            });
        }
        let sections = vec![Section {
            title: self.r23_t("doc23.bulletLibrary", "Bullet Library"),
            cells,
        }];
        let footer = vec![
            (
                format!(
                    "{}  \u{25B8}",
                    self.r23_t("doc23.changeLevel", "Change List Level")
                ),
                "r23.levels".to_owned(),
            ),
            (
                self.r23_t("doc23.defineBullet", "Define New Bullet..."),
                "r23.defineBullet".to_owned(),
            ),
        ];
        self.r23_gallery("format.bullets", sections, footer, (6, (57.0, 57.0)))
    }

    fn r23_numbering(&mut self) -> bool {
        let none = self.r23_t("doc23.none", "None");
        let mut cells = vec![Cell {
            thumb: Thumb::Text {
                texts: vec![(none.clone(), 0.3, 0.36, 0.2, Rgba(0x26, 0x26, 0x26, 255))],
                paths: Vec::new(),
            },
            command: "r23.number:none".into(),
            tip: none,
            checked: !self.view.format().numbering,
        }];
        for (i, (_, _, sample)) in NUMBERS.iter().enumerate() {
            cells.push(Cell {
                thumb: list_thumb(*sample, false),
                command: format!("r23.number:{i}"),
                tip: sample.join(" "),
                checked: false,
            });
        }
        let sections = vec![Section {
            title: self.r23_t("doc23.numberingLibrary", "Numbering Library"),
            cells,
        }];
        let footer = vec![
            (
                format!(
                    "{}  \u{25B8}",
                    self.r23_t("doc23.changeLevel", "Change List Level")
                ),
                "r23.levels".to_owned(),
            ),
            (
                self.r23_t("doc23.defineNumber", "Define New Number Format..."),
                "r23.defineNumber".to_owned(),
            ),
            (
                self.r23_t("doc23.setValue", "Set Numbering Value..."),
                "r23.setValue".to_owned(),
            ),
        ];
        self.r23_gallery("format.numbering", sections, footer, (3, (100.0, 100.0)))
    }

    fn r23_multilevel(&mut self) -> bool {
        let none = self.r23_t("doc23.none", "None");
        let mut cells = vec![Cell {
            thumb: Thumb::Text {
                texts: vec![(none.clone(), 0.3, 0.36, 0.2, Rgba(0x26, 0x26, 0x26, 255))],
                paths: Vec::new(),
            },
            command: "r23.number:none".into(),
            tip: none,
            checked: false,
        }];
        for (id, sample) in MULTILEVEL {
            cells.push(Cell {
                thumb: list_thumb(sample, true),
                command: format!("format.multilevel.{id}"),
                tip: sample.join(" "),
                checked: false,
            });
        }
        let sections = vec![Section {
            title: self.r23_t("doc23.listLibrary", "List Library"),
            cells,
        }];
        let footer = vec![
            (
                format!(
                    "{}  \u{25B8}",
                    self.r23_t("doc23.changeLevel", "Change List Level")
                ),
                "r23.levels".to_owned(),
            ),
            (
                self.r23_t("doc23.defineMultilevel", "Define New Multilevel List..."),
                "r23.defineMultilevel".to_owned(),
            ),
            (
                self.r23_t("doc23.defineListStyle", "Define New List Style..."),
                "r23.defineListStyle".to_owned(),
            ),
        ];
        self.r23_gallery("format.multilevel", sections, footer, (3, (100.0, 100.0)))
    }

    /// The list level (0-based) of the paragraph at the caret, if it is in a list.
    pub(super) fn r23_list_level(&self) -> Option<u8> {
        let s = self.view.engine().session()?;
        let para = s.selection().focus.para;
        let p = bo_model::index::para_by_id(s.doc(), para)?;
        p.props.num.as_ref().map(|n| n.ilvl)
    }

    /// Change List Level ▸: the nine levels beside the gallery's last rows.
    fn r23_levels_menu(&mut self) -> bool {
        let current = self.r23_list_level();
        let rows: Vec<(String, String, bool)> = (0..9u8)
            .map(|l| {
                (
                    format!(
                        "{}{} {}",
                        "   ".repeat(usize::from(l.min(4))),
                        self.r23_t("doc23.level", "Level"),
                        l + 1
                    ),
                    format!("r23.level:{l}"),
                    current.is_none(),
                )
            })
            .collect();
        let g = self.r23.gallery_at;
        let at = [g[0] + g[2] - 4.0, g[1] + g[3] - 80.0, 1.0, 0.0];
        self.r22.gallery = None;
        let open = self.r23_menu(rows, at, Vec::new());
        if let (Some(m), Some(c)) = (self.menu.as_mut(), current) {
            m.set_hovered(Some(usize::from(c)));
        }
        open
    }

    fn r23_set_level(&mut self, level: u8) -> bool {
        self.menu = None;
        let Some(now) = self.r23_list_level() else {
            return false;
        };
        let (n, up) = if level > now {
            (level - now, true)
        } else {
            (now - level, false)
        };
        let format = if up {
            bo_edit::ParaFormat::IncreaseIndent
        } else {
            bo_edit::ParaFormat::DecreaseIndent
        };
        let mut done = false;
        for _ in 0..n {
            done |= self.view.run_command(Command::FormatParagraph {
                format: format.clone(),
            });
        }
        done
    }

    /// A one-level list definition applied to the selected paragraphs: the other eight
    /// levels follow the reference's defaults (§17.9.6 lvl).
    fn r23_define(&mut self, format: &str, text: &str) -> bool {
        self.menu = None;
        self.r22.gallery = None;
        let levels: Vec<serde_json::Value> = (0..9)
            .map(|i| {
                if i == 0 {
                    json!({ "format": format, "text": text, "indent": 720, "hanging": 360 })
                } else if format == "bullet" {
                    let b = ["\u{25CB}", "\u{25A0}", "\u{25CF}"][(i - 1) % 3];
                    json!({ "format": "bullet", "text": b,
                        "indent": 720 * (i + 1), "hanging": 360 })
                } else {
                    let f = ["lowerLetter", "lowerRoman", "decimal"][(i - 1) % 3];
                    json!({ "format": f, "text": format!("%{}.", i + 1),
                        "indent": 720 * (i + 1), "hanging": 360 })
                }
            })
            .collect();
        self.r3_run(vec![
            json!({ "cmd": "defineList", "levels": levels, "apply": true }),
        ])
    }

    fn r23_apply_bullet(&mut self, v: &str) -> bool {
        if v == "none" {
            self.r22.gallery = None;
            return self.view.format().bullets && self.action("format.bullets")
                || self.view.format().numbering && self.action("format.numbering");
        }
        self.r23_define("bullet", v)
    }

    fn r23_apply_number(&mut self, v: &str) -> bool {
        if v == "none" {
            self.r22.gallery = None;
            let f = self.view.format().clone();
            return if f.numbering {
                self.view.run_command(Command::FormatParagraph {
                    format: bo_edit::ParaFormat::Numbering,
                })
            } else if f.bullets {
                self.view.run_command(Command::FormatParagraph {
                    format: bo_edit::ParaFormat::Bullets,
                })
            } else {
                false
            };
        }
        let Some((fmt, text, _)) = v.parse::<usize>().ok().and_then(|i| NUMBERS.get(i)) else {
            return false;
        };
        self.r23_define(fmt, text)
    }

    /// Underline ▾: the reference's eight line styles (§17.18.99), then More Underlines….
    fn r23_underlines(&mut self) -> bool {
        let anchor = self.r23_anchor("format.underline");
        let current = self.view.format().advanced.underline.clone();
        let rows: Vec<(String, String, bool)> = [
            ("single", "doc23.uSingle", "Single"),
            ("double", "doc23.uDouble", "Double"),
            ("thick", "doc23.uThick", "Thick"),
            ("dotted", "doc23.uDotted", "Dotted"),
            ("dash", "doc23.uDash", "Dash"),
            ("dotDash", "doc23.uDotDash", "Dot dash"),
            ("dotDotDash", "doc23.uDotDotDash", "Dot dot dash"),
            ("wave", "doc23.uWave", "Wave"),
        ]
        .iter()
        .map(|(v, k, en)| (self.r23_t(k, en), format!("r23.ul:{v}"), false))
        .chain([(
            self.r23_t("doc23.moreUnderlines", "More Underlines..."),
            "r23.ulMore".to_owned(),
            false,
        )])
        .collect();
        let open = self.r23_menu(rows, anchor, vec![8]);
        if let Some(m) = self.menu.as_mut() {
            let mut items = m.items().to_vec();
            for it in &mut items {
                it.checked = matches!(&it.choice,
                    Choice::Command(c) if c.strip_prefix("r23.ul:") == Some(current.as_str()));
            }
            *m =
                Menu::open(items, anchor, self.size, self.scale).with_decor(vec![None; 9], vec![8]);
        }
        open
    }

    // ----- Text Effects and Typography -----

    /// Home › Text Effects and Typography ▾: fifteen effect presets (BlueOffice's own
    /// combinations of fill, outline, shadow and glow), then Outline ▸, Shadow ▸,
    /// Reflection ▸, Glow ▸, Number Styles ▸, Ligatures ▸, Stylistic Sets ▸.
    fn r23_text_effects(&mut self) -> bool {
        let cells = TEXT_PRESETS
            .iter()
            .enumerate()
            .map(|(i, (fill, outline, halo))| Cell {
                thumb: Thumb::Letter {
                    fill: rgb(fill),
                    outline: outline.map(rgb),
                    halo: halo.map(rgb),
                },
                command: format!("r23.tefx:preset={i}"),
                tip: format!("{} {}", self.r23_t("doc23.effect", "Effect"), i + 1),
                checked: false,
            })
            .collect();
        let sub = |w: &Self, k: &str, en: &str, id: &str| {
            (
                format!("{}  \u{25B8}", w.r23_t(k, en)),
                format!("r23.tefxMenu:{id}"),
            )
        };
        let footer = vec![
            sub(self, "doc23.teOutline", "Outline", "outline"),
            sub(self, "doc23.teShadow", "Shadow", "shadow"),
            sub(self, "doc23.teReflection", "Reflection", "reflection"),
            sub(self, "doc23.teGlow", "Glow", "glow"),
            sub(self, "doc23.teNumberStyles", "Number Styles", "numbers"),
            sub(self, "doc23.teLigatures", "Ligatures", "ligatures"),
            sub(self, "doc23.teSets", "Stylistic Sets", "sets"),
        ];
        self.r23_gallery(
            "format.textEffects",
            vec![Section {
                title: String::new(),
                cells,
            }],
            footer,
            (5, (48.0, 48.0)),
        )
    }

    fn r23_text_effects_menu(&mut self, which: &str) -> bool {
        let rows: Vec<(String, String, bool)> = match which {
            "outline" => {
                let mut v = vec![(
                    self.r23_t("doc23.noOutline", "No Outline"),
                    "r23.tefx:outline=none".to_owned(),
                    false,
                )];
                for (hex, k, en) in &HIGHLIGHTS[..12] {
                    v.push((self.r23_t(k, en), format!("r23.tefx:outline={hex}"), false));
                }
                v
            }
            "shadow" => [
                ("none", "doc23.noShadow", "No Shadow"),
                ("br", "doc23.shOffsetBR", "Offset: Bottom Right"),
                ("b", "doc23.shOffsetB", "Offset: Bottom"),
                ("r", "doc23.shOffsetR", "Offset: Right"),
                ("c", "doc23.shOffsetC", "Offset: Center"),
            ]
            .iter()
            .map(|(id, k, en)| (self.r23_t(k, en), format!("r23.tefx:shadow={id}"), false))
            .collect(),
            "reflection" => [
                ("none", "doc23.noReflection", "No Reflection"),
                ("tight", "doc23.rfTight", "Tight Reflection: Touching"),
                ("half", "doc23.rfHalf", "Half Reflection: Touching"),
                ("full", "doc23.rfFull", "Full Reflection: Touching"),
            ]
            .iter()
            .map(|(id, k, en)| {
                (
                    self.r23_t(k, en),
                    format!("r23.tefx:reflection={id}"),
                    false,
                )
            })
            .collect(),
            "glow" => {
                let mut v = vec![(
                    self.r23_t("doc23.noGlow", "No Glow"),
                    "r23.tefx:glow=none".to_owned(),
                    false,
                )];
                for hex in ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"] {
                    v.push((
                        format!("{}: 5 pt, #{hex}", self.r23_t("doc23.glow", "Glow")),
                        format!("r23.tefx:glow={hex}"),
                        false,
                    ));
                }
                v
            }
            "numbers" => [
                ("default", "doc23.default", "Default"),
                ("propLining", "doc23.nsPropLining", "Proportional Lining"),
                ("tabLining", "doc23.nsTabLining", "Tabular Lining"),
                ("propOld", "doc23.nsPropOld", "Proportional Old-style"),
                ("tabOld", "doc23.nsTabOld", "Tabular Old-style"),
            ]
            .iter()
            .map(|(id, k, en)| (self.r23_t(k, en), format!("r23.tefx:numbers={id}"), false))
            .collect(),
            "ligatures" => [
                ("", "doc23.ligNone", "None"),
                ("standard", "doc23.ligStandard", "Standard Only"),
                (
                    "standardContextual",
                    "doc23.ligStdCtx",
                    "Standard and Contextual",
                ),
                (
                    "historicalDiscretional",
                    "doc23.ligHistDisc",
                    "Historical and Discretionary",
                ),
                ("all", "doc23.ligAll", "All"),
            ]
            .iter()
            .map(|(id, k, en)| (self.r23_t(k, en), format!("r23.tefx:ligatures={id}"), false))
            .collect(),
            _ => std::iter::once((
                self.r23_t("doc23.default", "Default"),
                "r23.tefx:sets=0".to_owned(),
                false,
            ))
            .chain((1..=20).map(|i| {
                (
                    format!("{} {i}", self.r23_t("doc23.set", "Set")),
                    format!("r23.tefx:sets={i}"),
                    false,
                )
            }))
            .collect(),
        };
        let g = self.r23.gallery_at;
        let at = [g[0] + g[2] - 4.0, g[1] + g[3] * 0.55, 1.0, 0.0];
        self.r22.gallery = None;
        self.r23_menu(rows, at, Vec::new())
    }

    fn r23_apply_text_effect(&mut self, v: &str) -> bool {
        self.menu = None;
        self.r22.gallery = None;
        let (kind, val) = v.split_once('=').unwrap_or((v, ""));
        let glow = |hex: &str| json!({ "color": hex, "radius": 5.0, "transparency": 60 });
        let shadow = |angle: f32, distance: f32| {
            json!({
                "color": "000000", "blur": 4.0, "distance": distance, "angle": angle,
                "transparency": 60,
            })
        };
        let mut color: Option<String> = None;
        let patch = match kind {
            "outline" if val == "none" => json!({ "textOutline": null }),
            "outline" => json!({ "textOutline": { "color": val, "width": 0.75 } }),
            "shadow" => match val {
                "br" => json!({ "softShadow": shadow(45.0, 3.0) }),
                "b" => json!({ "softShadow": shadow(90.0, 3.0) }),
                "r" => json!({ "softShadow": shadow(0.0, 3.0) }),
                "c" => json!({ "softShadow": shadow(0.0, 0.0) }),
                _ => json!({ "softShadow": null }),
            },
            "reflection" => match val {
                "tight" => {
                    json!({ "reflection": { "size": 33, "transparency": 50, "distance": 0.0 } })
                }
                "half" => {
                    json!({ "reflection": { "size": 50, "transparency": 50, "distance": 0.0 } })
                }
                "full" => {
                    json!({ "reflection": { "size": 100, "transparency": 50, "distance": 0.0 } })
                }
                _ => json!({ "reflection": null }),
            },
            "glow" if val == "none" => json!({ "glow": null }),
            "glow" => json!({ "glow": glow(val) }),
            "numbers" => {
                let (sp, form) = match val {
                    "propLining" => ("proportional", "lining"),
                    "tabLining" => ("tabular", "lining"),
                    "propOld" => ("proportional", "oldStyle"),
                    "tabOld" => ("tabular", "oldStyle"),
                    _ => ("", ""),
                };
                json!({ "numSpacing": sp, "numForm": form })
            }
            "ligatures" => json!({ "ligatures": val }),
            "sets" => json!({ "stylisticSet": val.parse::<u8>().unwrap_or(0) }),
            "preset" => {
                let i: usize = val.parse().unwrap_or(0);
                let (fill, outline, halo) = TEXT_PRESETS[i.min(TEXT_PRESETS.len() - 1)];
                color = Some(fill.to_owned());
                let dark = halo.is_some_and(|h| h == "000000");
                json!({
                    "textOutline": outline.map(|o| json!({ "color": o, "width": 0.75 })),
                    "softShadow": if dark { shadow(45.0, 3.0) } else { Value::Null },
                    "glow": match halo { Some(h) if h != "000000" => glow(h), _ => Value::Null },
                })
            }
            _ => return false,
        };
        let mut cmds = Vec::new();
        if let Some(hex) = color {
            cmds.push(json!({ "cmd": "formatText",
                "format": serde_json::to_value(bo_edit::CharFormat::Color { hex: Some(hex) })
                    .unwrap_or(Value::Null) }));
        }
        cmds.push(json!({ "cmd": "formatText",
            "format": { "kind": "advanced", "patch": patch } }));
        self.r3_run(cmds)
    }

    // ----- Highlight -----

    fn r23_highlights(&mut self) -> bool {
        let current = self.view.format().highlight.clone();
        let cells = HIGHLIGHTS
            .iter()
            .map(|(hex, k, en)| Cell {
                thumb: swatch(hex),
                command: format!("r23.hl:{hex}"),
                tip: self.r23_t(k, en),
                checked: current.as_deref() == Some(*hex),
            })
            .collect();
        let footer = vec![
            (
                self.r23_t("doc23.noColor", "No Color"),
                "r23.hl:none".to_owned(),
            ),
            (
                self.r23_t("doc23.stopHighlighting", "Stop Highlighting"),
                "r23.hl:stop".to_owned(),
            ),
        ];
        self.r23_gallery(
            "format.highlight",
            vec![Section {
                title: String::new(),
                cells,
            }],
            footer,
            (5, (30.0, 30.0)),
        )
    }

    fn r23_highlight(&mut self, hex: Option<String>) -> bool {
        self.menu = None;
        self.r22.gallery = None;
        self.view.run_command(Command::FormatText {
            format: bo_edit::CharFormat::Highlight { hex },
        })
    }

    // ----- Paragraph menus -----

    fn r23_line_spacing(&mut self) -> bool {
        let anchor = self.r23_anchor("format.lineSpacing");
        self.open_command_menu("format.lineSpacing", anchor);
        if let Some(m) = self.menu.take() {
            let mut items: Vec<MenuItem> = m.items().to_vec();
            items.insert(
                6,
                MenuItem {
                    label: self.r23_t("doc23.lineOptions", "Line Spacing Options..."),
                    choice: Choice::Command("r23.lineOptions".into()),
                    checked: false,
                    dim: false,
                },
            );
            let n = items.len();
            self.menu = Some(
                Menu::open(items, anchor, self.size, self.scale)
                    .with_decor(vec![None; n], vec![6, 7]),
            );
        }
        true
    }

    fn r23_borders(&mut self) -> bool {
        let anchor = self.r23_anchor("format.paraBorders");
        let in_table = self.view.format().table.is_some();
        // In a table the edges are the cells' (§17.4.39 tblBorders presets), else the
        // paragraphs' (§17.3.1.24 pBdr).
        let edge = |para: &str, table: &str| {
            if in_table {
                format!("r23.tb:{table}")
            } else {
                format!("format.paraBorders.{para}")
            }
        };
        let rows: Vec<(String, String, bool)> = vec![
            (
                "doc23.bBottom",
                "Bottom Border",
                edge("bottom", "bottom"),
                false,
            ),
            ("doc23.bTop", "Top Border", edge("top", "top"), false),
            ("doc23.bLeft", "Left Border", edge("left", "left"), false),
            (
                "doc23.bRight",
                "Right Border",
                edge("right", "right"),
                false,
            ),
            ("doc23.bNone", "No Border", edge("none", "none"), false),
            ("doc23.bAll", "All Borders", edge("all", "all"), false),
            (
                "doc23.bOutside",
                "Outside Borders",
                edge("outside", "outside"),
                false,
            ),
            (
                "doc23.bInside",
                "Inside Borders",
                edge("inside", "inside"),
                false,
            ),
            (
                "doc23.bInsideH",
                "Inside Horizontal Border",
                edge("insideH", "insideH"),
                false,
            ),
            // Inside vertical belongs to table cells; the reference dims it for paragraphs.
            (
                "doc23.bInsideV",
                "Inside Vertical Border",
                "r23.tb:insideV".to_owned(),
                !in_table,
            ),
            // Diagonal cell borders (§17.4.80 tl2br / tr2bl, round 24): table cells only.
            (
                "doc23.bDiagDown",
                "Diagonal Down Border",
                "r24.diag:down".to_owned(),
                !in_table,
            ),
            (
                "doc23.bDiagUp",
                "Diagonal Up Border",
                "r24.diag:up".to_owned(),
                !in_table,
            ),
            (
                "doc23.bHorizontalLine",
                "Horizontal Line",
                "format.paraBorders.horizontalLine".to_owned(),
                false,
            ),
            (
                "doc23.drawTable",
                "Draw Table",
                "table.drawTable".to_owned(),
                false,
            ),
            (
                "doc23.viewGridlines",
                "View Gridlines",
                "table.viewGridlines".to_owned(),
                false,
            ),
            (
                "doc12.bsMenu",
                "Borders and Shading...",
                "format.bordersShading".to_owned(),
                false,
            ),
        ]
        .into_iter()
        .map(|(k, en, cmd, dim)| (self.r23_t(k, en), cmd, dim))
        .collect();
        self.r23_menu(rows, anchor, vec![4, 12, 15])
    }

    fn r23_border_preset(&mut self, preset: bo_edit::ParaBorderPreset) -> bool {
        self.menu = None;
        self.view.run_command(Command::FormatParagraph {
            format: bo_edit::ParaFormat::Borders { preset },
        })
    }

    fn r23_table_borders(&mut self, preset: &str) -> bool {
        self.menu = None;
        crate::catalog::from_json(json!({
            "cmd": "tableEdit", "edit": { "kind": "borders", "preset": preset }
        }))
        .is_some_and(|c| self.view.run_command(c))
    }
    /// Horizontal Line: an empty paragraph with a bottom border after the caret's paragraph
    /// (BlueOffice's reading of the reference's line; §17.3.1.7 bottom).
    fn r23_horizontal_line(&mut self) -> bool {
        self.menu = None;
        let mut done = self.view.run_command(Command::SplitParagraph);
        done |= self.r23_border_preset(bo_edit::ParaBorderPreset::Bottom);
        done
    }

    fn r23_open_shading(&mut self) -> bool {
        self.menu = None;
        let anchor = self.r23_anchor("format.shading");
        let doc = self.view.engine().document();
        let theme = doc
            .map(|d| d.design.theme_colors.clone())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                doc.and_then(|d| d.design.theme_xml.clone())
                    .map(|x| bo_engine::theme_file::theme_colors(x.as_bytes()))
            })
            .unwrap_or_default();
        let labels = [
            self.t("doc14.pcHighContrast", "High-contrast only"),
            self.t("doc14.pcTheme", "Theme Colors"),
            self.t("doc14.pcStandard", "Standard Colors"),
            self.t("doc14.pcNone", "No Color"),
            self.t("doc14.pcMore", "More Colors..."),
            String::new(),
        ];
        let current = self.view.format().shading.clone();
        let mut pal = crate::doc_wm14::PagePalette::open(
            anchor,
            self.size,
            self.scale,
            super::r12::palette(&theme),
            labels,
            current,
        );
        pal.recent.clone_from(&self.r15.recent);
        pal.recent_label = self.t("doc15.recentColors", "Recent Colors");
        self.r23.shading = Some(Box::new(pal));
        true
    }

    fn r23_shade(&mut self, hex: Option<String>) -> bool {
        self.r23.shading = None;
        // In a table the cells are shaded (§17.4.33 shd), else the paragraphs.
        if self.view.format().table.is_some() {
            let id = format!("table.shade.{}", hex.as_deref().unwrap_or("none"));
            return self.action(&id);
        }
        self.view.run_command(Command::FormatParagraph {
            format: bo_edit::ParaFormat::Shading { hex, pattern: None },
        })
    }

    // ----- Editing -----

    fn r23_find_menu(&mut self) -> bool {
        let anchor = self.r23_anchor("edit.find");
        let rows = vec![
            (
                self.r23_t("doc23.find", "Find"),
                "edit.find".to_owned(),
                false,
            ),
            (
                self.r23_t("doc23.advancedFind", "Advanced Find..."),
                "r23.advancedFind".to_owned(),
                false,
            ),
            (
                self.r23_t("doc23.goTo", "Go To..."),
                "edit.goTo".to_owned(),
                false,
            ),
        ];
        self.r23_menu(rows, anchor, Vec::new())
    }

    fn r23_select_menu(&mut self) -> bool {
        let anchor = self.r23_anchor("edit.select");
        let rows = vec![
            (
                self.r23_t("doc23.selectAll", "Select All"),
                "edit.selectAll".to_owned(),
                false,
            ),
            (
                self.r23_t("doc23.selectObjects", "Select Objects"),
                "r23.selectObjects".to_owned(),
                false,
            ),
            (
                self.r23_t(
                    "doc23.selectSimilar",
                    "Select All Text With Similar Formatting",
                ),
                "r23.selectSimilar".to_owned(),
                !self.view.has_selection(),
            ),
            (
                self.r23_t("doc23.selectionPane", "Selection Pane..."),
                "layout.selectionPane".to_owned(),
                false,
            ),
        ];
        let open = self.r23_menu(rows, anchor, Vec::new());
        if let Some(m) = self.menu.as_mut() {
            if self.r23.select_objects {
                let mut items = m.items().to_vec();
                items[1].checked = true;
                *m = Menu::open(items, anchor, self.size, self.scale);
            }
        }
        open
    }

    /// Select All Text With Similar Formatting: from the first to the last paragraph whose
    /// text has the selection's font, size, bold and italic (BlueDoc has one selection, so the
    /// similar text is the span that holds all of it).
    fn r23_select_similar(&mut self) -> bool {
        self.menu = None;
        let f = self.view.format().clone();
        let Some(s) = self.view.engine().session() else {
            return false;
        };
        let n = s.paragraph_count();
        let doc = s.doc();
        let mut first: Option<(usize, usize)> = None;
        let mut last: Option<(usize, usize)> = None;
        for id in 0..n {
            let Some(p) = bo_model::index::para_by_id(doc, id) else {
                continue;
            };
            let text_len = bo_model::index::para_index_text(p).len();
            let similar = p.runs.iter().any(|r| {
                let rp = &r.props;
                let has_text = r
                    .content
                    .iter()
                    .any(|c| matches!(c, bo_model::RunContent::Text(t) if !t.trim().is_empty()));
                has_text
                    && rp.bold.unwrap_or(false) == f.bold
                    && rp.italic.unwrap_or(false) == f.italic
                    && rp.underline.is_some() == f.underline
            });
            if similar {
                if first.is_none() {
                    first = Some((id, 0));
                }
                last = Some((id, text_len));
            }
        }
        let (Some(a), Some(b)) = (first, last) else {
            return false;
        };
        self.view.run_command(Command::SetSelection {
            selection: bo_edit::Selection {
                anchor: bo_edit::Pos::new(a.0, a.1),
                focus: bo_edit::Pos::new(b.0, b.1),
            },
        })
    }

    /// Select Objects: a press on a drawing object selects all of it (text boxes too).
    fn r23_select_object_at(&mut self, x: f32, y: f32) -> Option<bool> {
        let (page, px, py) = self.r8_page_point(x, y)?;
        let layout = self.view.engine().layout()?;
        let s = self.view.engine().session()?;
        let sel = layout.pages.get(page)?.items.iter().rev().find_map(|it| {
            let bo_layout::PageItem::Shape {
                x: sx,
                y: sy,
                w,
                h,
                source: bo_layout::LineSource::TextBox { para, at, .. },
                ..
            } = it
            else {
                return None;
            };
            let inside = px >= *sx && px <= sx + w && py >= *sy && py <= sy + h;
            inside
                .then(|| s.drawing_selection(*para as usize, *at as usize))
                .flatten()
        })?;
        Some(
            self.view
                .run_command(Command::SetSelection { selection: sel }),
        )
    }

    /// Input of round 23: the Shading palette, Select Objects presses.
    pub(super) fn r23_input(&mut self, input: &Input) -> Option<bool> {
        use crate::doc_wm14::PcPick;
        if let Input::Press { x, y, .. } = *input {
            self.r23.press = Some((x, y));
        }
        if let Some(p) = self.r23.shading.as_mut() {
            return Some(match *input {
                Input::Move { x, y } | Input::Drag { x, y } => p.hover(x, y),
                Input::Press { x, y, .. } => match p.press(x, y) {
                    Some(PcPick::Nothing | PcPick::Fill) => true,
                    Some(PcPick::Color(hex)) => self.r23_shade(Some(hex)),
                    Some(PcPick::None) => self.r23_shade(None),
                    Some(PcPick::More) => {
                        self.r23.shading = None;
                        self.r3_open_form(crate::doc_forms::DocFormKind::Colors)
                    }
                    None => {
                        self.r23.shading = None;
                        true
                    }
                },
                Input::Key(crate::Key::Escape, _) => {
                    self.r23.shading = None;
                    true
                }
                Input::Release { .. } | Input::Wheel(_) => false,
                _ => return None,
            });
        }
        if self.r23.select_objects {
            if let Input::Press { x, y, count: 1, .. } = *input {
                if y > self.view_top() {
                    return self.r23_select_object_at(x, y);
                }
            }
        }
        None
    }

    pub(super) fn r23_paint(&self, p: &mut dyn crate::Painter) {
        if let Some(pal) = &self.r23.shading {
            pal.paint(p);
        }
    }
}

/// Labels and icons of the round-23 buttons (BlueOffice's own drawings).
pub(crate) fn install(icons: &mut crate::icons::Icons, s: &mut crate::strings::Strings) {
    for (key, cmd, en) in [
        ("doc23.select", "commands.edit.select", "Select"),
        ("doc23.find", "commands.edit.find", "Find"),
    ] {
        let v = s.get(key).to_owned();
        if s.get(cmd) == cmd || key == "doc23.select" {
            s.set(cmd, if v == key { en } else { v.as_str() });
        }
    }
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    // Select: an arrow pointer over a dashed frame (own drawing).
    icons.add_solid(
        "edit.select",
        &[
            (
                "M3 3h3v1.5h-1.5v1.5h-1.5ZM8 3h3v1.5h-3ZM13 3h3v3h-1.5v-1.5h-1.5ZM3 8h1.5v3h-1.5ZM3 13h1.5v1.5h1.5v1.5h-3Z",
                rgb(0x60, 0x60, 0x60),
            ),
            (
                "M9 8l11 5l-4.5 1.2l3 4.3l-1.8 1.2l-3-4.3l-3.2 3.2Z",
                rgb(0x26, 0x26, 0x26),
            ),
        ],
    );
}
