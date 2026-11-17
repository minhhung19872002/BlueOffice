//! BlueDoc parity round 7 (docs/ui/REPORT.md rounds 93+): the commands of the contextual tabs
//! (`doc_tabs7`) — the Borders pen and Border Painter, View Gridlines, Formula, the picture
//! Adjust group (Remove Background, Corrections / Color galleries, Compress, Change, Reset),
//! Alt Text, picture size boxes, shape styles, the Header & Footer navigation, options and
//! position boxes, and the Equation tab. A child module of the shell so the window's state
//! is at hand.
use super::DocWindow;
use crate::doc_forms::DocFormKind;

#[path = "doc_card7.rs"]
mod card;
use crate::doc_tabs7::{self as tabs, Pen};
use crate::ribbon::RibbonHit;
use crate::slide_window::pic::{GalleryKind, PicGallery};
use crate::spin_box::{BoxLook, SpinHit};
use crate::{Input, Key, PageStore, Painter, TextStyle};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// Round-7 state of the window.
#[derive(Default)]
pub(super) struct R7 {
    /// The Borders group's pen.
    pub(super) pen: Pen,
    /// Border Painter armed: the next press in a table applies the pen to it.
    pub(super) painter: bool,
    /// Table gridlines shown (View Gridlines).
    pub(super) gridlines: bool,
    /// Header & Footer › Show Document Text (on by default, as the reference).
    pub(super) hide_body: bool,
    /// Corrections / Color gallery, when open.
    pub(super) gallery: Option<PicGallery>,
    /// Equation input form: LaTeX instead of Unicode (the reference's Conversions toggle).
    pub(super) latex: bool,
    /// The next picture the host reads replaces the selected one (Change Picture).
    pub(super) change_pending: bool,
    /// Document Inspector groups inspected last.
    pub(super) inspected: Vec<String>,
    /// The Accessibility pane, when shown.
    pub(super) a11y: Option<crate::doc_reveal::InfoPane>,
    /// Comment card in the markup area (Contextual view).
    pub(super) card: card::Card,
}

/// Size boxes of Picture / Shape Format and position boxes of Header & Footer.
const BOXES: [&str; 4] = [
    "picture.height",
    "picture.width",
    "hf.headerPos",
    "hf.footerPos",
];

/// Shape style tiles: fill, outline, text colour (own colours: outline only, dark, accent).
const SHAPE_STYLES: [(Option<&str>, &str, &str); 3] = [
    (Some("FFFFFF"), "70AD47", "000000"),
    (Some("000000"), "000000", "FFFFFF"),
    (Some("4472C4"), "2F528F", "FFFFFF"),
];

fn hex(s: &str) -> Rgba {
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

impl DocWindow {
    /// A round-7 command; `None` when `id` is not one.
    #[allow(clippy::too_many_lines)]
    pub(super) fn r7_action(&mut self, id: &str) -> Option<bool> {
        if let Some(p) = tabs::pen_entry(id, &self.r7.pen) {
            self.r7.pen = p;
            return Some(true);
        }
        if let Some((_, preset, _)) = tabs::BORDERS.iter().find(|b| b.0 == id) {
            return Some(self.r7_borders(preset));
        }
        if let Some(i) = tabs::SYMBOL_IDS.iter().position(|s| *s == id) {
            return Some(self.r7_math_text(&tabs::SYMBOLS[i].to_string()));
        }
        if let Some(t) = tabs::structure_text(id) {
            return Some(self.r7_math_text(t));
        }
        Some(match id {
            // Review › New Comment: a card with a text box in the comments pane, Post /
            // Cancel (the reference's in-card editor) instead of a dialog.
            "review.newComment" => {
                // Contextual view: a card beside the selection in the markup area; the List
                // view (Comments pane open) writes in the pane.
                if self.comments_pane.is_none() && self.r7_card_new() {
                    return Some(true);
                }
                if self.comments_pane.is_none() {
                    self.spell_pane = None;
                    self.comments_pane = Some(crate::doc_comments::CommentsPane::new());
                    self.relayout_pending = true;
                }
                let author = self.r6.prefs.user_name.trim().to_owned();
                if let Some(cp) = self.comments_pane.as_mut() {
                    cp.author = author;
                    cp.start_new();
                }
                true
            }
            "table.borderPainter" => {
                self.r7.painter = !self.r7.painter;
                true
            }
            "table.viewGridlines" => {
                self.r7.gridlines = !self.r7.gridlines;
                true
            }
            // §17.16.5.19 = (formula): the sum of the numbers above, as the reference
            // proposes in a cell under numbers.
            "table.formula" => self.r3_run(vec![
                json!({ "cmd": "insertField", "instr": "=SUM(ABOVE)" }),
                json!({ "cmd": "updateFields" }),
            ]),
            "picture.removeBackground" => self.view.picture_command(&json!({
                "cmd": "pictureRemoveBackground", "marks": [],
            })),
            "picture.corrections" => self.r7_open_gallery(GalleryKind::Corrections, id),
            "picture.color" => self.r7_open_gallery(GalleryKind::Color, id),
            "picture.compress" => self.view.picture_command(&json!({
                "cmd": "pictureCompress", "ppi": 150.0, "dropCropped": true,
            })),
            "picture.change.file" => {
                self.r7.change_pending = true;
                self.requests.push(super::Request::PickPicture);
                false
            }
            "picture.resetSize" => self.view.picture_command(&json!({
                "cmd": "pictureReset", "size": true,
            })),
            "picture.altText" => self.r7_alt_text(),
            "file.inspect" => {
                self.r6.bs = None;
                self.r3_open_form(DocFormKind::Inspect)
            }
            "file.compatibility" => {
                self.r6.bs = None;
                self.r7_compatibility()
            }
            "file.readOnlyRecommended" | "file.markFinal" => {
                let flag = if id == "file.markFinal" {
                    "markedFinal"
                } else {
                    "readOnlyRecommended"
                };
                let on = !self
                    .view
                    .run_json(&json!({ "cmd": "security" }))
                    .is_some_and(|v| v["security"][flag].as_bool().unwrap_or(false));
                let cmd = if id == "file.markFinal" {
                    "markFinal"
                } else {
                    "setReadOnlyRecommended"
                };
                self.r3_run(vec![json!({ "cmd": cmd, "on": on })])
            }
            "review.checkAccessibility" => {
                if self.r7.a11y.take().is_some() {
                    self.relayout_pending = true;
                    return Some(true);
                }
                self.r7_open_accessibility()
            }
            "file.accessibility" => {
                self.r6.bs = None;
                self.r7_open_accessibility()
            }
            "file.encrypt" => {
                self.r6.bs = None;
                self.r3_open_form(DocFormKind::Encrypt)
            }
            "hf.pictures" => {
                self.requests.push(super::Request::PickPicture);
                false
            }
            "hf.previous" | "hf.next" => self.r7_hf_step(id == "hf.next"),
            "hf.showDocumentText" => {
                self.r7.hide_body = !self.r7.hide_body;
                true
            }
            "eq.unicode" | "eq.latex" => {
                self.r7.latex = id == "eq.latex";
                true
            }
            "eq.text" => self.r7_math_text("\"\""),
            "eq.convert.professional" | "eq.convert.linear" => self.r3_run(vec![json!({
                "cmd": "equationFormat", "linear": id == "eq.convert.linear",
            })]),
            _ if BOXES.contains(&id) => self.r7_box_press(id),
            _ => return None,
        })
    }

    /// Apply border preset `preset` with the pen to the table at the caret.
    fn r7_borders(&mut self, preset: &str) -> bool {
        let p = &self.r7.pen;
        self.r3_run(vec![json!({
            "cmd": "tableEdit",
            "edit": {
                "kind": "borders", "preset": preset,
                "pen": { "style": p.style, "size": p.size, "color": p.color },
            },
        })])
    }

    /// Type math text into the equation being edited, or insert a new equation holding it
    /// (as the reference inserts a structure at the caret outside an equation).
    fn r7_math_text(&mut self, text: &str) -> bool {
        if self.view.engine().math_active() {
            return self.r3_run(vec![json!({ "cmd": "mathInsert", "text": text })]);
        }
        self.r3_run(vec![json!({
            "cmd": "insertEquation", "text": text, "latex": self.r7.latex, "display": false,
        })])
    }

    /// Previous / Next header or footer: the same story of the section before or after.
    fn r7_hf_step(&mut self, next: bool) -> bool {
        let Some(story) = self.view.story() else {
            return false;
        };
        let header = story.kind == "header";
        let page = self.view.current_page();
        let sec = self.view.format().section.index;
        let sections: Vec<usize> = self
            .view
            .engine()
            .layout()
            .map(|l| l.pages.iter().map(|p| p.section).collect())
            .unwrap_or_default();
        // The first page of the next / previous section.
        let target = if next {
            (page + 1..sections.len()).find(|p| sections[*p] > sec)
        } else {
            (0..page).rev().find(|p| sections[*p] < sec)
        };
        let Some(target) = target else {
            return false;
        };
        self.r3_run(vec![
            json!({ "cmd": "editHeaderFooter", "page": target, "header": header }),
        ])
    }

    /// Open the Corrections / Color gallery under its button.
    fn r7_open_gallery(&mut self, kind: GalleryKind, id: &str) -> bool {
        if self.r7.gallery.take().is_some() {
            return true;
        }
        let Some(sid) = tabs::static_id(id) else {
            return false;
        };
        let Some(anchor) = self.ribbon.rect_of(RibbonHit::Command(sid)) else {
            return false;
        };
        let Some(info) = self
            .view
            .run_json(&json!({ "cmd": "pictureInfo" }))
            .and_then(|v| v.get("picture").cloned())
        else {
            return false;
        };
        let current: bo_model::picture::PictureFx =
            serde_json::from_value(info["fx"].clone()).unwrap_or_default();
        let media = info["media"].as_str().unwrap_or("").to_owned();
        let pw = info["pixelWidth"].as_u64().unwrap_or(1) as u32;
        let ph = info["pixelHeight"].as_u64().unwrap_or(1) as u32;
        let strings = &self.strings;
        let title = |k: &str| {
            let key = format!("slideNative.{k}");
            strings.get(&key).to_owned()
        };
        let accents = [
            Rgba(0x44, 0x72, 0xc4, 255),
            Rgba(0xed, 0x7d, 0x31, 255),
            Rgba(0xa5, 0xa5, 0xa5, 255),
            Rgba(0xff, 0xc0, 0x00, 255),
            Rgba(0x5b, 0x9b, 0xd5, 255),
            Rgba(0x70, 0xad, 0x47, 255),
        ];
        self.menu = None;
        self.r7.gallery = Some(PicGallery::open(
            kind,
            sid,
            anchor,
            self.size,
            self.scale,
            (pw, ph),
            current,
            media,
            &title,
            &accents,
        ));
        true
    }

    /// A press on a size / position box: the arrows step it (0.1" / 0.1 cm).
    fn r7_box_press(&mut self, id: &str) -> bool {
        let Some(sid) = BOXES.iter().find(|b| **b == id).copied() else {
            return false;
        };
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(sid)) else {
            return false;
        };
        let rect = crate::doc_table::spin_rect(r, self.scale);
        let up = match crate::spin_box::spin_hit(rect, self.scale, self.pointer.0, self.pointer.1) {
            Some(SpinHit::Up) => true,
            Some(SpinHit::Down) => false,
            _ => return false,
        };
        let step = self.units.step();
        let delta = if up { step } else { -step };
        let Some(v) = self.r7_box_value(id) else {
            return false;
        };
        self.r7_set_box(id, (v + delta).max(0))
    }

    /// Current value (twips) of a size / position box.
    fn r7_box_value(&self, id: &str) -> Option<i32> {
        let sec = &self.view.format().section;
        match id {
            "hf.headerPos" => Some(sec.header),
            "hf.footerPos" => Some(sec.footer),
            _ => {
                let img = self.view.engine().session()?.tools_picture()?;
                // EMU → twips (914 400 per inch, 1 440 twips per inch).
                let emu = if id == "picture.height" {
                    img.height.0
                } else {
                    img.width.0
                };
                Some((emu as f64 / 635.0).round() as i32)
            }
        }
    }

    /// Set a size / position box to `twips`.
    fn r7_set_box(&mut self, id: &str, twips: i32) -> bool {
        match id {
            "hf.headerPos" | "hf.footerPos" => {
                let s = self.view.format().section;
                let mut setup = json!({
                    "width": s.width, "height": s.height,
                    "marginTop": s.margin_top, "marginBottom": s.margin_bottom,
                    "marginLeft": s.margin_left, "marginRight": s.margin_right,
                    "header": s.header, "footer": s.footer,
                    "columns": s.columns, "columnSpace": s.column_space,
                });
                setup[if id == "hf.headerPos" {
                    "header"
                } else {
                    "footer"
                }] = json!(twips);
                self.r3_run(vec![json!({ "cmd": "setPageSetup", "setup": setup })])
            }
            _ => {
                let Some(s) = self.view.engine().session() else {
                    return false;
                };
                let Some(img) = s.tools_picture() else {
                    return false;
                };
                let Some((para, at)) = self.r7_object_at() else {
                    return false;
                };
                let pt = twips as f32 / 20.0;
                // The other side keeps the picture's proportions (Lock aspect ratio, as the
                // reference's default for pictures).
                let (w, h) = (img.width.0 as f32 / 12700.0, img.height.0 as f32 / 12700.0);
                let (width, height) = if id == "picture.height" {
                    (w * pt / h.max(0.1), pt)
                } else {
                    (pt, h * pt / w.max(0.1))
                };
                self.r3_run(vec![json!({
                    "cmd": "setDrawingFrame", "para": para, "at": at,
                    "width": width.max(1.0), "height": height.max(1.0),
                })])
            }
        }
    }

    /// Input for round-7 parts (the open gallery, the armed Border Painter); `None` lets
    /// it through.
    pub(super) fn r7_input(&mut self, input: &Input) -> Option<bool> {
        // Typing goes to the comment being written in a card.
        if let Some(cp) = self.comments_pane.as_mut().filter(|c| c.draft.is_some()) {
            match input {
                Input::Text(s) => return Some(cp.type_text(s)),
                Input::Key(k, m) => {
                    if let Some(a) = cp.key(*k, m.ctrl) {
                        return Some(self.comments_action(a));
                    }
                }
                _ => {}
            }
        }
        if let Some(r) = self.r7_pane_input(input) {
            return Some(r);
        }
        if let Some(r) = self.r7_card_input(input) {
            return Some(r);
        }
        if let Some(g) = self.r7.gallery.as_mut() {
            match *input {
                Input::Move { x, y } if g.contains(x, y) => {
                    let h = g.cell_at(x, y);
                    let changed = h != g.hover;
                    g.hover = h;
                    return Some(changed);
                }
                Input::Press { x, y, .. } => {
                    let pick = g.cell_at(x, y).and_then(|i| g.cells.get(i).cloned());
                    let inside = g.contains(x, y);
                    if pick.is_some() || !inside {
                        self.r7.gallery = None;
                    }
                    if let Some((_, fx)) = pick {
                        return Some(
                            self.view
                                .picture_command(&json!({ "cmd": "pictureFx", "fx": fx })),
                        );
                    }
                    if inside {
                        return Some(false);
                    }
                    // A press on the button again only closes the gallery.
                    let anchor = g_anchor_hit(&self.ribbon, x, y);
                    if anchor {
                        return Some(true);
                    }
                }
                Input::Key(Key::Escape, _) => {
                    self.r7.gallery = None;
                    return Some(true);
                }
                _ => {}
            }
        }
        if self.r7.painter {
            match *input {
                Input::Key(Key::Escape, _) => {
                    self.r7.painter = false;
                    return Some(true);
                }
                // The press places the caret (the click goes on); the release paints the
                // table now holding the caret.
                Input::Release { .. } if self.view.format().table.is_some() => {
                    self.r7_borders("all");
                    return None;
                }
                _ => {}
            }
        }
        None
    }

    /// Bring the gallery's preview images up to date.
    pub(super) fn r7_prepare(&mut self, store: &mut dyn PageStore) -> Result<(), String> {
        if let Some(g) = self.r7.gallery.as_mut() {
            g.prepare(store, self.view.engine())?;
        }
        Ok(())
    }

    /// Ids drawn pressed / ticked by round 7.
    pub(super) fn r7_on_ids(&self) -> Vec<&'static str> {
        let mut on = Vec::new();
        if self.r7.painter {
            on.push("table.borderPainter");
        }
        if self.r7.gridlines {
            on.push("table.viewGridlines");
        }
        on.push(if self.r7.latex {
            "eq.latex"
        } else {
            "eq.unicode"
        });
        on
    }

    /// Draw the round-7 controls over their ribbon placeholders: the pen boxes, the pen
    /// colour bar, the Show Document Text check box, the size / position boxes and the
    /// shape style tiles.
    pub(super) fn r7_paint_ribbon(&self, p: &mut dyn Painter) {
        let s = self.scale;
        let px = |v: f32| (v * s).round();
        let t = &self.theme;
        let style = TextStyle {
            size: px(12.0),
            bold: false,
        };
        let rect = |id: &'static str| self.ribbon.rect_of(RibbonHit::Command(id));
        // Equation › Symbols: a white gallery of framed cells, each with its symbol (the
        // reference's Basic Math set: frame 198 grey, cells 38 × 30 px at 125 %).
        if let (Some(first), Some(last)) = (rect(tabs::SYMBOL_IDS[0]), rect(tabs::SYMBOL_IDS[15])) {
            let (l, top) = (first[0] - px(2.0), first[1] - px(2.0));
            let (r, bottom) = (last[0] + last[2] + px(2.0), last[1] + last[3] + px(2.0));
            p.fill_rect(l, top, r - l, bottom - top, Rgba(198, 198, 198, 255));
            p.fill_rect(
                l + 1.0,
                top + 1.0,
                r - l - 2.0,
                bottom - top - 2.0,
                Rgba(255, 255, 255, 255),
            );
            let hovered = self.ribbon.hovered();
            let glyph = TextStyle {
                size: px(14.0),
                bold: false,
            };
            for (i, id) in tabs::SYMBOL_IDS.iter().enumerate() {
                let Some(c) = rect(id) else { continue };
                let k = px(1.0);
                let cell = [c[0] + k, c[1] + k, c[2] - 2.0 * k, c[3] - 2.0 * k];
                p.fill_rect(cell[0], cell[1], cell[2], cell[3], Rgba(171, 171, 171, 255));
                let bg = if hovered == Some(RibbonHit::Command(id)) {
                    t.hover
                } else {
                    Rgba(255, 255, 255, 255)
                };
                p.fill_rect(
                    cell[0] + 1.0,
                    cell[1] + 1.0,
                    cell[2] - 2.0,
                    cell[3] - 2.0,
                    bg,
                );
                let sym = tabs::SYMBOLS[i].to_string();
                p.text(
                    &sym,
                    (cell[0] + cell[2] / 2.0 - glyph.size * 0.3).round(),
                    (cell[1] + (cell[3] - glyph.size * 1.33) / 2.0).round(),
                    glyph,
                    t.text,
                );
            }
        }
        // Conversions: the Unicode / LaTeX / Text marks drawn as text (own lettering).
        for (id, mark) in [("eq.unicode", "/"), ("eq.latex", "{ }"), ("eq.text", "abc")] {
            let Some(c) = rect(id) else { continue };
            let icon = px(16.0);
            let x = c[0] + px(4.0);
            let y = (c[1] + (c[3] - icon) / 2.0).round();
            let on = (id == "eq.latex") == self.r7.latex && id != "eq.text";
            let bg = if on { t.hover } else { t.ribbon };
            p.fill_rect(x, y, icon, icon, bg);
            let st = TextStyle {
                size: px(if mark.len() > 2 { 9.0 } else { 12.0 }),
                bold: false,
            };
            p.text(
                mark,
                x,
                (y + (icon - st.size * 1.33) / 2.0).round(),
                st,
                t.text,
            );
        }
        let in_table = self.view.format().table.is_some();
        // Line style and weight boxes: a drop-down box showing the line (and the weight).
        for id in ["table.penStyle", "table.penWeight"] {
            let Some(r) = rect(id) else { continue };
            p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
            let h = px(22.0).min(r[3]);
            let b = [
                r[0] + px(2.0),
                (r[1] + (r[3] - h) / 2.0).round(),
                r[2] - px(4.0),
                h,
            ];
            let label = if id == "table.penWeight" {
                tabs::weight_name(self.r7.pen.size)
            } else {
                String::new()
            };
            crate::spin_box::paint_combo(
                p,
                b,
                s,
                &BoxLook {
                    text: &label,
                    style,
                    ink: t.text,
                    enabled: in_table,
                    caret: None,
                    hover: (self.ribbon.hovered() == Some(RibbonHit::Command(id)))
                        .then_some(SpinHit::Field),
                },
            );
            // The line sample: after the weight's name, or across the style box.
            let x0 = if label.is_empty() {
                b[0] + px(6.0)
            } else {
                b[0] + px(44.0)
            };
            let x1 = b[0] + b[2] - px(22.0);
            let y = (b[1] + b[3] / 2.0).round();
            let ink = if in_table {
                Rgba(0x20, 0x20, 0x20, 255)
            } else {
                Rgba(0xa6, 0xa6, 0xa6, 255)
            };
            paint_line_sample(p, self.r7.pen.style, [x0, y, x1 - x0], s, ink);
        }
        // Pen Color: the bar under the pencil takes the pen's colour.
        if let Some(r) = rect("table.penColor") {
            let c = self.r7.pen.color.map_or(Rgba(0, 0, 0, 255), hex);
            let icon = px(16.0);
            let x = r[0] + px(4.0);
            let y = (r[1] + (r[3] - icon) / 2.0).round() + icon - px(3.0);
            p.fill_rect(x, y, icon, px(3.0), c);
        }
        // Show Document Text: a check box like the other options.
        if self.view.story().is_some() {
            crate::doc_table::paint_checks(
                p,
                &self.ribbon,
                &[("hf.showDocumentText", !self.r7.hide_body)],
                t,
                &self.strings,
                s,
            );
        }
        // Size / position boxes: an icon, then the value in a spin box.
        for (i, id) in BOXES.iter().enumerate() {
            let Some(r) = rect(id) else { continue };
            p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
            let value = self.r7_box_value(id);
            let ink = Rgba(0x66, 0x69, 0x6e, 255);
            let (cx, cy, k) = (r[0] + px(9.0), r[1] + r[3] / 2.0, px(6.0));
            let vertical = i % 2 == 0;
            let segs = if vertical {
                vec![
                    PathSeg::M(cx, cy - k),
                    PathSeg::L(cx, cy + k),
                    PathSeg::M(cx - k * 0.6, cy - k),
                    PathSeg::L(cx + k * 0.6, cy - k),
                    PathSeg::M(cx - k * 0.6, cy + k),
                    PathSeg::L(cx + k * 0.6, cy + k),
                ]
            } else {
                vec![
                    PathSeg::M(cx - k, cy),
                    PathSeg::L(cx + k, cy),
                    PathSeg::M(cx - k, cy - k * 0.6),
                    PathSeg::L(cx - k, cy + k * 0.6),
                    PathSeg::M(cx + k, cy - k * 0.6),
                    PathSeg::L(cx + k, cy + k * 0.6),
                ]
            };
            p.path(&segs, None, Some((ink, s.max(1.0))));
            let text = value.map_or_else(String::new, |v| self.units.fmt(v));
            crate::spin_box::paint_spin(
                p,
                crate::doc_table::spin_rect(r, s),
                s,
                &BoxLook {
                    text: &text,
                    style,
                    ink: t.text,
                    enabled: value.is_some(),
                    caret: None,
                    hover: None,
                },
            );
        }
        // Shape style tiles: "Abc" on the style's fill inside its outline.
        for (i, (fill, line, ink)) in SHAPE_STYLES.iter().enumerate() {
            let Some(r) = rect(["shape.style.0", "shape.style.1", "shape.style.2"][i]) else {
                continue;
            };
            p.fill_rect(r[0], r[1], r[2], r[3], t.ribbon);
            let (w, h) = (r[2] - px(10.0), px(48.0).min(r[3] - px(8.0)));
            let (x, y) = (r[0] + px(5.0), (r[1] + (r[3] - h) / 2.0).round());
            let k = px(6.0);
            let round = rounded(x, y, w, h, k);
            p.path(&round, fill.map(hex), Some((hex(line), (1.5 * s).max(1.0))));
            let st = TextStyle {
                size: px(11.0),
                bold: false,
            };
            p.text(
                "Abc",
                (x + w / 2.0 - px(10.0)).round(),
                (y + (h - st.size * 1.33) / 2.0).round(),
                st,
                hex(ink),
            );
        }
    }

    /// Paint the open gallery (over the document, under dialogs).
    pub(super) fn r7_paint(&self, p: &mut dyn Painter) {
        self.r7_paint_card(p);
        self.r7_paint_panes(p);
        if let Some(g) = &self.r7.gallery {
            g.paint(p, self.theme.text, self.scale);
        }
    }

    /// Where the selected drawing object is: body paragraph id and index offset (the
    /// object character at or just before the selection's start).
    fn r7_object_at(&self) -> Option<(usize, usize)> {
        let s = self.view.engine().session()?;
        let (a, b) = s.selection().ordered();
        let p = bo_model::index::para_by_id(s.doc(), a.para)?;
        let text = bo_model::index::para_index_text(p);
        let end = if b.para == a.para {
            b.offset
        } else {
            text.len()
        };
        text.char_indices()
            .filter(|(i, c)| *c == bo_model::index::OBJECT && *i + c.len_utf8() >= a.offset)
            .find(|(i, _)| *i <= end)
            .map(|(i, _)| (a.para, i))
    }

    /// Alt Text for the selected picture or shape: the dialog with its current text.
    fn r7_alt_text(&mut self) -> bool {
        let Some(desc) = self
            .view
            .engine()
            .session()
            .and_then(bo_edit::Session::tools_picture)
            .map(|i| i.description)
        else {
            return false;
        };
        let Some((para, at)) = self.r7_object_at() else {
            return false;
        };
        self.r3_open_form(DocFormKind::AltText);
        if let Some(f) = self.r3.form.as_mut() {
            f.ids = vec![para.to_string(), at.to_string()];
            f.set_text("alt", &desc);
        }
        true
    }

    /// The engine's inspection report.
    fn r7_report(&mut self) -> Value {
        self.view
            .run_json(&json!({ "cmd": "inspect" }))
            .map_or(Value::Null, |v| v["inspection"].clone())
    }

    /// Document Inspector: inspect `cats` (dialog groups), or remove everything of group
    /// `remove`, then show the results in the dialog.
    pub(super) fn r7_inspect(&mut self, cats: &[String], remove: Option<&str>) -> bool {
        if let Some(group) = remove {
            let report = self.r7_report();
            let mut categories = Vec::new();
            let mut accept = false;
            for f in report["inspector"].as_array().into_iter().flatten() {
                let cat = f["category"].as_str().unwrap_or("");
                if !group_categories(group).contains(&cat) {
                    continue;
                }
                match f["action"].as_str() {
                    Some("remove" | "onSave") => categories.push(cat.to_owned()),
                    Some("acceptAll") => accept = true,
                    _ => {}
                }
            }
            let mut cmds = Vec::new();
            if accept {
                cmds.push(json!({ "cmd": "acceptAllRevisions" }));
            }
            if !categories.is_empty() {
                cmds.push(json!({ "cmd": "inspectRemove", "categories": categories }));
            }
            self.r3_run(cmds);
        }
        let groups: Vec<String> = if remove.is_some() {
            self.r7.inspected.clone()
        } else {
            cats.to_vec()
        };
        self.r7.inspected.clone_from(&groups);
        let report = self.r7_report();
        let mut found = Vec::new();
        for (i, (name, ..)) in crate::doc_forms::INSPECT.iter().enumerate() {
            if !groups.iter().any(|g| g == name) {
                continue;
            }
            let mut parts = Vec::new();
            let mut removable = false;
            for f in report["inspector"].as_array().into_iter().flatten() {
                let cat = f["category"].as_str().unwrap_or("");
                let n = f["count"].as_u64().unwrap_or(0);
                if !group_categories(name).contains(&cat) || n == 0 {
                    continue;
                }
                removable |= f["action"].as_str().is_some_and(|a| a != "none");
                let label = self.t(&format!("docR7.cat.{cat}"), cat);
                parts.push(format!("{label}: {n}"));
            }
            let what = (!parts.is_empty()).then(|| {
                format!(
                    "{} {}",
                    self.t("docR7.ins.found", "The following items were found:"),
                    parts.join(", ")
                )
            });
            found.push((i, what, removable));
        }
        let strings = &self.strings;
        if let Some(f) = self.r3.form.as_mut() {
            f.r7_results(strings, &found);
        }
        true
    }

    /// Check for Issues › Check Compatibility: the dialog listing what older formats lose.
    fn r7_compatibility(&mut self) -> bool {
        let report = self.r7_report();
        let issues: Vec<String> = report["compatibility"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                let feature = c["feature"].as_str().unwrap_or("");
                let name = self.t(&format!("docR7.feat.{feature}"), feature);
                format!("{name}  ({})", c["count"].as_u64().unwrap_or(0))
            })
            .collect();
        self.r3_open_form(DocFormKind::Compatibility);
        let strings = &self.strings;
        if let Some(f) = self.r3.form.as_mut() {
            f.r7_issues(strings, issues);
        }
        true
    }
}

/// Engine inspector categories of a Document Inspector group.
fn group_categories(group: &str) -> &'static [&'static str] {
    match group {
        "comments" => &["comments", "revisions"],
        "properties" => &["properties", "customProperties"],
        "customXml" => &["customXml", "serverProperties"],
        "headersFooters" => &["headersFooters", "watermarks"],
        "hiddenText" => &["hiddenText"],
        "macros" => &["macros", "embedded"],
        _ => &[],
    }
}

/// Whether (`x`, `y`) is on a ribbon button (a second press on the gallery's button).
fn g_anchor_hit(ribbon: &crate::ribbon::Ribbon, x: f32, y: f32) -> bool {
    matches!(
        ribbon.hit(x, y),
        Some(RibbonHit::Command("picture.corrections" | "picture.color"))
    )
}

/// A rounded rectangle path.
fn rounded(x: f32, y: f32, w: f32, h: f32, k: f32) -> Vec<PathSeg> {
    vec![
        PathSeg::M(x + k, y),
        PathSeg::L(x + w - k, y),
        PathSeg::Q(x + w, y, x + w, y + k),
        PathSeg::L(x + w, y + h - k),
        PathSeg::Q(x + w, y + h, x + w - k, y + h),
        PathSeg::L(x + k, y + h),
        PathSeg::Q(x, y + h, x, y + h - k),
        PathSeg::L(x, y + k),
        PathSeg::Q(x, y, x + k, y),
        PathSeg::Z,
    ]
}

/// A horizontal sample of border style `style` from (`x`, `y`) over `w` px.
fn paint_line_sample(p: &mut dyn Painter, style: &str, [x, y, w]: [f32; 3], s: f32, ink: Rgba) {
    let one = s.max(1.0).round();
    let dashes = |p: &mut dyn Painter, on: f32, off: f32, y: f32| {
        let mut cx = x;
        while cx < x + w {
            p.fill_rect(cx, y, on.min(x + w - cx), one, ink);
            cx += on + off;
        }
    };
    match style {
        "dotted" => dashes(p, one, one * 2.0, y),
        "dashSmallGap" => dashes(p, 4.0 * one, 2.0 * one, y),
        "dashed" => dashes(p, 6.0 * one, 3.0 * one, y),
        "dotDash" => {
            let mut cx = x;
            while cx < x + w {
                p.fill_rect(cx, y, (6.0 * one).min(x + w - cx), one, ink);
                cx += 8.0 * one;
                if cx < x + w {
                    p.fill_rect(cx, y, one, one, ink);
                }
                cx += 3.0 * one;
            }
        }
        "double" => {
            p.fill_rect(x, y - one, w, one, ink);
            p.fill_rect(x, y + one, w, one, ink);
        }
        "triple" => {
            for k in [-2.0, 0.0, 2.0] {
                p.fill_rect(x, y + k * one, w, one, ink);
            }
        }
        "thickThinSmallGap" => {
            p.fill_rect(x, y - 2.0 * one, w, 2.0 * one, ink);
            p.fill_rect(x, y + one, w, one, ink);
        }
        _ => p.fill_rect(x, y, w, one, ink),
    }
}

/// English names of the accessibility rules (translations under `docR7.rule.*`).
fn rule_name(rule: &str) -> &'static str {
    match rule {
        "altText" => "Missing alternative text",
        "tableHeader" => "Missing table header",
        "headingOrder" => "Skipped heading level",
        "noHeadings" => "No headings in a long document",
        "contrast" => "Hard-to-read text contrast",
        "linkText" => "Unclear hyperlink text",
        "mergedCells" => "Merged cells",
        _ => "Other issue",
    }
}

impl DocWindow {
    /// Width of the Accessibility pane.
    pub(super) fn r7_pane_w(&self) -> f32 {
        if self.r7.a11y.is_some() {
            crate::doc_reveal::InfoPane::width(self.scale)
        } else {
            0.0
        }
    }

    /// Review › Check Accessibility / File › Check for Issues: the Accessibility pane with
    /// the inspection results by severity (errors, warnings, tips).
    pub(super) fn r7_open_accessibility(&mut self) -> bool {
        let title = self.t("docR7.acc.title", "Accessibility");
        self.r7.a11y = Some(crate::doc_reveal::InfoPane::new(title));
        self.r7_refresh_accessibility();
        self.relayout_pending = true;
        true
    }

    /// The pane's sections from the engine's report.
    pub(super) fn r7_refresh_accessibility(&mut self) {
        if self.r7.a11y.is_none() {
            return;
        }
        let report = self.r7_report();
        let issues: Vec<&Value> = report["accessibility"]
            .as_array()
            .into_iter()
            .flatten()
            .collect();
        // Every rule under its category with the number of issues, or a tick when there
        // are none, as the reference's pane lists them.
        let mut sections = Vec::new();
        for (key, fallback, rules) in [
            (
                "docR7.acc.colorContrast",
                "Color and Contrast",
                &["contrast"][..],
            ),
            (
                "docR7.acc.media",
                "Media and Illustrations",
                &["altText"][..],
            ),
            (
                "docR7.acc.tables",
                "Tables",
                &["tableHeader", "mergedCells"][..],
            ),
            (
                "docR7.acc.structure",
                "Document Structure",
                &["noHeadings", "headingOrder", "linkText"][..],
            ),
        ] {
            let lines = rules
                .iter()
                .map(|rule| {
                    let name = self.t(&format!("docR7.rule.{rule}"), rule_name(rule));
                    let n = issues.iter().filter(|i| i["rule"] == *rule).count();
                    if n == 0 {
                        format!("{name}  \u{2713}")
                    } else {
                        format!("{name}  {n}")
                    }
                })
                .collect();
            sections.push(crate::doc_reveal::InfoSection {
                header: self.t(key, fallback),
                lines,
                clear: None,
            });
        }
        if issues.is_empty() {
            sections.insert(
                0,
                crate::doc_reveal::InfoSection {
                    header: self.t("docR7.acc.ok", "No accessibility issues found"),
                    lines: vec![self.t(
                        "docR7.acc.okHint",
                        "People with disabilities should not have difficulty reading this document.",
                    )],
                    clear: None,
                },
            );
        }
        if let Some(p) = self.r7.a11y.as_mut() {
            p.sections = sections;
        }
    }

    /// Lay the Accessibility pane out (left of the round-3 / round-5 panes); dim what the
    /// reference dims on the contextual tabs (large buttons included).
    pub(super) fn r7_layout(&mut self, text: &dyn crate::TextMeasure) {
        self.r7_layout_card(text);
        let active = self.ribbon.active_label();
        if [
            crate::doc_tools::TABLE_DESIGN,
            crate::doc_tools::TABLE_LAYOUT,
            crate::doc_tools::PICTURE_FORMAT,
            crate::doc_tools::HEADER_FOOTER,
            tabs::SHAPE_FORMAT,
            tabs::EQUATION,
        ]
        .contains(&active)
        {
            let mut dim: Vec<&'static str> = tabs::GREYED.to_vec();
            if let Some(story) = self.view.story() {
                // The story being edited: its Go to button is dimmed, and the first
                // section has nothing to link to.
                dim.push(if story.kind == "header" {
                    "hf.gotoHeader"
                } else {
                    "hf.gotoFooter"
                });
                if self.view.format().section.index == 0 {
                    dim.push("hf.linkToPrevious");
                }
            }
            self.ribbon.set_extra_dim(dim);
        }
        let right = self.size.0 - self.r3_pane_w() + self.r7_pane_w();
        let (top, bottom, scale) = (
            self.chrome_bottom(),
            self.size.1 - self.status_h(),
            self.scale,
        );
        if let Some(pane) = self.r7.a11y.as_mut() {
            pane.layout(right, top, bottom, scale);
        }
    }

    /// Input on the Accessibility pane.
    pub(super) fn r7_pane_input(&mut self, input: &Input) -> Option<bool> {
        let pane = self.r7.a11y.as_mut()?;
        match *input {
            Input::Press { x, y, .. } if pane.contains(x, y) => {
                if pane.press(x, y) == crate::doc_reveal::InfoHit::Close {
                    self.r7.a11y = None;
                    self.relayout_pending = true;
                    return Some(true);
                }
                Some(false)
            }
            Input::Wheel(dy) if pane.contains(self.pointer.0, self.pointer.1) => {
                Some(pane.wheel(dy))
            }
            Input::Move { x, y } | Input::Drag { x, y } | Input::Release { x, y }
                if pane.contains(x, y) =>
            {
                Some(false)
            }
            _ => None,
        }
    }

    /// Paint the Accessibility pane.
    pub(super) fn r7_paint_panes(&self, p: &mut dyn Painter) {
        if let Some(pane) = &self.r7.a11y {
            pane.paint(p);
        }
    }
}

#[cfg(test)]
#[path = "doc_r7_tests.rs"]
mod tests;
