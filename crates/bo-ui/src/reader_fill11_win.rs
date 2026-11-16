//! BlueReader round 11 (a child of `reader_window`), measured on the reference reader at 125 %:
//!
//! - Comment tools: a shape is selected once drawn; the rectangle tool draws 2 pt; NoZoom notes
//!   are hit and framed by their drawn icon (dotted frame, no handles).
//! - Fill & Sign: the reference's tab (signature gallery, Manage Signatures, Keep Tool
//!   Selected, Add Text typed in place, Predefined Text, check / cross / dot / line / box), the
//!   Create Signature dialog (Type / Draw / Import, initials), signatures placed by a click,
//!   resized by one corner, saved for later, flattened on save when asked.
//! - Forms: fields drawn on the reference's highlight (221, 228, 255) under their appearance,
//!   text typed in the field with a dotted focus frame, Tab / Shift+Tab order, Space for check
//!   boxes and radio buttons, list box rows, calculated fields (AFSimple_Calculate subset).
//! - Read Aloud: the word being spoken is highlighted on the page.
//! - Accessibility: every ribbon command has a name for assistive technology.
use super::{inside, ReaderRequest, ReaderWindow, Tool};
use crate::popup::{Choice, MenuItem};
use crate::{Input, Key, Painter, TextMeasure, TextStyle};
use bo_pdfread::{Annot, FieldKind, FormField, Markup, MarkupKind, SignatureInk};
use bo_render::{PathSeg, Rgba};

/// The reference's field highlight (measured: 221, 228, 255 on white).
pub(crate) const FIELD_TINT: Rgba = Rgba(221, 228, 255, 255);
/// The Add Text box while typing (measured 1 px, 31, 123, 204).
const TYPING_EDGE: Rgba = Rgba(31, 123, 204, 255);
/// The signature gallery's width (506 device px at 125 %).
const GALLERY_W: f32 = 404.8;
/// A gallery tile (145 × 68 device px) and the gap between tiles.
const TILE_W: f32 = 116.0;
const TILE_H: f32 = 54.4;
const TILE_GAP: f32 = 6.4;
const TILE_SEL: Rgba = Rgba(230, 233, 240, 255);
const EDGE: Rgba = Rgba(199, 204, 217, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const SELECT_BG: Rgba = Rgba(0, 51, 102, 255);
/// Read Aloud's word highlight.
const WORD_BG: Rgba = Rgba(255, 214, 0, 110);
/// Bundled italic faces offered for a typed signature: name, face index.
const FACES: [(&str, usize); 4] = [
    ("Tinos Italic", 2),
    ("Caladea Italic", 14),
    ("Carlito Italic", 10),
    ("Liberation Sans Italic", 6),
];
/// Signature colours offered: black, blue, red.
const COLOURS: [[u8; 3]; 3] = [[0, 0, 0], [0, 0, 255], [255, 0, 0]];
/// Create Signature dialog (869 × 483 logical px: 1086 × 604 device px at 125 %).
const DLG_W: f32 = 868.8;
const DLG_H: f32 = 483.2;

/// How the selected comment's handles look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GripMode {
    /// Eight handles and the rotate handle (round 10).
    Full,
    /// Only a dotted frame round the drawn icon (NoZoom notes).
    Frame,
    /// One square handle at the bottom right corner, proportions kept (Fill & Sign).
    Corner,
}

/// A saved signature or initials.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SavedSig {
    pub(crate) ink: SignatureInk,
    pub(crate) initials: bool,
    pub(crate) color: [u8; 3],
}

/// What a click on the page places (besides the round 5 marks).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Mark11 {
    /// A square box mark.
    Box,
    /// Text typed in place.
    Text,
    /// Predefined text (the author's name, today's date).
    Predef(String),
}

/// A text field being typed in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldEdit {
    pub(crate) f: FormField,
    pub(crate) text: String,
    /// Caret as a character index.
    pub(crate) caret: usize,
    /// All the text is selected (entered with Tab).
    pub(crate) all: bool,
}

/// The Create Signature dialog.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct SigDialog {
    /// 0 Type, 1 Draw, 2 Import.
    pub(crate) tab: u8,
    pub(crate) text: String,
    pub(crate) face: usize,
    pub(crate) colour: usize,
    /// Strokes in dialog-relative device pixels.
    pub(crate) strokes: Vec<Vec<[f32; 2]>>,
    drawing: bool,
    pub(crate) image: Option<Vec<u8>>,
    pub(crate) initials: bool,
    /// The font list or the colour row is open.
    fonts_open: bool,
    colours_open: bool,
}

/// State of round 11.
#[derive(Debug, Default)]
pub(crate) struct Eleven {
    init: bool,
    pub(crate) sigs: Vec<SavedSig>,
    /// The saved signature being placed.
    pub(crate) placing: Option<usize>,
    pub(crate) keep: bool,
    pub(crate) flatten_on_save: bool,
    pub(crate) mark: Option<Mark11>,
    /// Add Text being typed: page, point (page points), text.
    pub(crate) typing: Option<(usize, [f32; 2], String)>,
    pub(crate) dialog: Option<SigDialog>,
    /// The form field with the keyboard and the text field being typed in.
    pub(crate) focus: Option<FormField>,
    pub(crate) edit: Option<FieldEdit>,
    /// Gallery tiles of the last frame: a saved signature, or `None` for "+".
    gallery: Vec<(Option<usize>, [f32; 4])>,
    /// Read Aloud: for each page read, its first UTF-16 offset in the spoken text and the page
    /// character of each spoken character; the word being spoken (page, first, last char).
    reading: Vec<(usize, usize, Vec<usize>, Vec<usize>)>,
    pub(crate) word: Option<(usize, usize, usize)>,
}

/// Today's date as the reference writes it (M/D/YYYY).
fn today11() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{m}/{d}/{y}")
}

/// Saved signatures as text lines (`T|initials|rrggbb|face|text`, `D|…|x,y x,y;…`,
/// `I|…|base64`).
pub(crate) fn encode_sigs(sigs: &[SavedSig]) -> String {
    use base64::Engine as _;
    let mut out = String::new();
    for s in sigs {
        let head = |k: &str| {
            format!(
                "{k}|{}|{:02x}{:02x}{:02x}|",
                u8::from(s.initials),
                s.color[0],
                s.color[1],
                s.color[2]
            )
        };
        match &s.ink {
            SignatureInk::Typed { text, face } => {
                let t = text.replace('\\', "\\\\").replace('\n', "\\n");
                out.push_str(&format!("{}{face}|{t}", head("T")));
            }
            SignatureInk::Strokes(st) => {
                let body: Vec<String> = st
                    .iter()
                    .map(|v| {
                        v.iter()
                            .map(|p| format!("{:.4},{:.4}", p[0], p[1]))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect();
                out.push_str(&format!("{}{}", head("D"), body.join(";")));
            }
            SignatureInk::Image(b) => {
                out.push_str(&head("I"));
                out.push_str(&base64::engine::general_purpose::STANDARD.encode(b));
            }
        }
        out.push('\n');
    }
    out
}

/// Read saved signatures written by [`encode_sigs`] (bad lines are skipped).
pub(crate) fn decode_sigs(text: &str) -> Vec<SavedSig> {
    use base64::Engine as _;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut p = line.splitn(4, '|');
        let (Some(k), Some(i), Some(c), Some(rest)) = (p.next(), p.next(), p.next(), p.next())
        else {
            continue;
        };
        let hex = |a: usize| {
            c.get(a..a + 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .unwrap_or(0)
        };
        let color = [hex(0), hex(2), hex(4)];
        let ink = match k {
            "T" => {
                let Some((face, t)) = rest.split_once('|') else {
                    continue;
                };
                let mut text = String::new();
                let mut esc = false;
                for ch in t.chars() {
                    match (esc, ch) {
                        (true, 'n') => {
                            text.push('\n');
                            esc = false;
                        }
                        (true, c) => {
                            text.push(c);
                            esc = false;
                        }
                        (false, '\\') => esc = true,
                        (false, c) => text.push(c),
                    }
                }
                SignatureInk::Typed {
                    text,
                    face: face.parse().unwrap_or(2),
                }
            }
            "D" => SignatureInk::Strokes(
                rest.split(';')
                    .map(|s| {
                        s.split(' ')
                            .filter_map(|xy| {
                                let (x, y) = xy.split_once(',')?;
                                Some([x.parse().ok()?, y.parse().ok()?])
                            })
                            .collect::<Vec<[f32; 2]>>()
                    })
                    .filter(|v| !v.is_empty())
                    .collect(),
            ),
            "I" => match base64::engine::general_purpose::STANDARD.decode(rest) {
                Ok(b) => SignatureInk::Image(b),
                Err(_) => continue,
            },
            _ => continue,
        };
        out.push(SavedSig {
            ink,
            initials: i == "1",
            color,
        });
    }
    out
}

/// Strokes moved into a unit box (0..1 both ways, proportions kept, top left); and the box's
/// width over height.
fn unit_strokes(strokes: &[Vec<[f32; 2]>]) -> (Vec<Vec<[f32; 2]>>, f32) {
    let pts = strokes.iter().flatten();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    if x0 > x1 {
        return (Vec::new(), 1.0);
    }
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let v = strokes
        .iter()
        .map(|s| {
            s.iter()
                .map(|p| [(p[0] - x0) / w, (p[1] - y0) / h])
                .collect()
        })
        .collect();
    (v, w / h)
}

/// Default size of a placed signature (page points): typed 24 pt high, drawn 30 pt, picture
/// 36 pt; initials smaller; the width follows the ink.
pub(crate) fn sig_size(s: &SavedSig) -> (f32, f32) {
    let k = if s.initials { 0.7 } else { 1.0 };
    match &s.ink {
        SignatureInk::Typed { text, face } => {
            let h = 24.0 * k;
            let em = bo_pdfread::typed_width(text, *face).max(0.5);
            (em * h * 0.72 / 0.96, h)
        }
        SignatureInk::Strokes(st) => {
            let (_, ratio) = unit_strokes(st);
            let h = 30.0 * k;
            ((h * ratio).clamp(h * 0.3, h * 8.0), h)
        }
        SignatureInk::Image(b) => {
            let (w, hh) = bo_pdfread::image_size(b).unwrap_or((4, 3));
            let h = 36.0 * k;
            (h * w.max(1) as f32 / hh.max(1) as f32, h)
        }
    }
}

impl ReaderWindow {
    /// Round 11 behaviour is BlueReader's (BlueEditor keeps its own).
    fn eleven_on(&self) -> bool {
        self.profile.is_none()
    }

    /// Highlight Fields is on.
    #[cfg(test)]
    pub(crate) fn tabs4_highlight(&self) -> bool {
        self.tabs4.highlight_fields
    }

    /// The form highlight is drawn with the page (BlueReader).
    pub(crate) fn tint11_on(&self) -> bool {
        self.eleven_on()
    }

    /// First use: fields highlighted as the reference does by default.
    fn init11(&mut self) {
        if self.eleven.init || !self.eleven_on() {
            return;
        }
        self.eleven.init = true;
        self.tabs4.highlight_fields = true;
    }

    /// A document was opened: its fields are drawn highlighted.
    pub(crate) fn opened11(&mut self) {
        self.init11();
        self.sync_tint11();
    }

    /// Keep every open view's field tint in step with Highlight Fields.
    fn sync_tint11(&mut self) {
        if !self.eleven_on() {
            return;
        }
        let tint = self.tabs4.highlight_fields.then_some(FIELD_TINT);
        for t in &mut self.tabs {
            t.view.set_field_tint(tint);
        }
    }

    /// A comment's box as drawn: NoZoom notes keep their icon's size on screen.
    pub(crate) fn shown_rect11(&self, a: &Annot) -> [f32; 4] {
        let Some(v) = self.view() else {
            return a.rect;
        };
        if !v.rotation().is_multiple_of(360) || !v.doc().annotation_no_zoom(a) {
            return a.rect;
        }
        let k = 1.0 / (v.zoom() * 96.0 / 72.0).max(0.01);
        let r = a.rect;
        [
            r[0],
            r[1],
            r[0] + (r[2] - r[0]) * k,
            r[1] + (r[3] - r[1]) * k,
        ]
    }

    /// The selected comment's handles.
    pub(crate) fn grip_mode11(&self, a: &Annot) -> GripMode {
        let Some(v) = self.view() else {
            return GripMode::Full;
        };
        if v.doc().annotation_no_zoom(a) {
            GripMode::Frame
        } else if v.doc().is_fill_sign(a) {
            GripMode::Corner
        } else {
            GripMode::Full
        }
    }

    /// Draw the frame / handle of a NoZoom note or a Fill & Sign mark; true when drawn.
    pub(crate) fn paint_grips11(&self, p: &mut dyn Painter, a: &Annot, b: [f32; 4]) -> bool {
        match self.grip_mode11(a) {
            GripMode::Full => false,
            GripMode::Frame => {
                // Measured: a black dotted frame (1 on, 1 off) round the icon, a red one inside.
                let (l, t, r, bo) = (b[0] - 2.0, b[1] - 2.0, b[0] + b[2] + 2.0, b[1] + b[3] + 2.0);
                dotted(p, [l, t, r - l, bo - t], INK, 1.0);
                let red = Rgba(255, 0, 0, 255);
                for e in [
                    [l + 1.0, t + 1.0, r - l - 2.0, 1.0],
                    [l + 1.0, bo - 2.0, r - l - 2.0, 1.0],
                    [l + 1.0, t + 1.0, 1.0, bo - t - 2.0],
                    [r - 2.0, t + 1.0, 1.0, bo - t - 2.0],
                ] {
                    p.fill_rect(e[0], e[1], e[2], e[3], red);
                }
                true
            }
            GripMode::Corner => {
                // Measured: a dotted frame and one blue square (9 device px) at the corner.
                dotted(p, b, Rgba(51, 142, 244, 255), 2.0);
                let s = self.px(7.2);
                let (x, y) = (b[0] + b[2], b[1] + b[3]);
                p.fill_rect(
                    (x - s / 2.0).round(),
                    (y - s / 2.0).round(),
                    s,
                    s,
                    Rgba(38, 128, 235, 255),
                );
                true
            }
        }
    }

    /// Select the comment just added to `page` (the last of its comments).
    pub(crate) fn select_new11(&mut self, page: usize) {
        if !self.eleven_on() {
            return;
        }
        self.extra.selected = self.view().and_then(|v| {
            v.doc()
                .annotations()
                .into_iter()
                .rev()
                .find(|a| a.page == page)
        });
    }

    /// The rectangle tool: a 2 pt rectangle in `color`, selected (false: not BlueReader).
    pub(crate) fn add_rect11(&mut self, page: usize, rect: [f32; 4], color: [u8; 3]) -> bool {
        if !self.eleven_on() {
            return false;
        }
        let style = bo_pdfread::DrawStyle {
            color,
            width: 2.0,
            ..bo_pdfread::DrawStyle::default()
        };
        let author = self.author.clone();
        let unit = self.measure_unit();
        let Some(t) = self.tabs.get_mut(self.active) else {
            return true;
        };
        match t.view.doc_mut().add_drawing_in(
            page,
            &bo_pdfread::Shape::Rect(rect),
            &style,
            &author,
            "",
            &unit,
        ) {
            Ok(()) => {
                self.document_changed();
                self.select_new11(page);
            }
            Err(e) => self.status = e,
        }
        true
    }

    /// Fill & Sign: the gallery is wider than a button; later controls move right.
    pub(crate) fn relayout11(&mut self) {
        let Some(k) = self.controls.iter().position(|c| c.id == "fill.gallery") else {
            return;
        };
        let want = self.px(GALLERY_W);
        let r = self.controls[k].r;
        let dx = want - r[2];
        let th = self.title_h();
        self.controls[k].r[2] = want;
        for c in self.controls.iter_mut().skip(k + 1) {
            if c.r[0] > r[0] && c.r[1] > th {
                c.r[0] += dx;
            }
        }
    }

    /// Controls of round 11 shown pressed.
    pub(crate) fn on11(&self, id: &str) -> bool {
        match id {
            "fill.keep" => self.eleven.keep,
            "fill.box" => self.eleven.mark == Some(Mark11::Box),
            "fill.text" => self.eleven.mark == Some(Mark11::Text),
            _ => false,
        }
    }

    fn clear_fill11(&mut self) {
        self.eleven.mark = None;
        self.eleven.placing = None;
        self.commit_typing11();
        for id in ["fill.box", "fill.text"] {
            self.on.remove(id);
        }
    }

    fn set_mark11(&mut self, m: Mark11) {
        let same = self.eleven.mark.as_ref() == Some(&m);
        self.clear_fill11();
        self.tabs5.fill = None;
        if !same {
            self.eleven.mark = Some(m);
            self.status = self.t("pdfFill.place");
        }
        for id in ["fill.box", "fill.text"] {
            if self.on11(id) {
                self.on.insert(id);
            }
        }
    }

    fn save_sigs11(&mut self) {
        let text = encode_sigs(&self.eleven.sigs);
        self.requests
            .push(ReaderRequest::Command(format!("host.sigs11:{text}")));
    }

    /// Saved signatures read by the host at start (`sigs11`), a picture for the dialog
    /// (`sig11img`); true when the purpose was one of these.
    pub(crate) fn file_picked11(&mut self, purpose: &str, bytes: &[u8]) -> bool {
        match purpose {
            "sigs11" => {
                self.eleven.sigs = decode_sigs(&String::from_utf8_lossy(bytes));
                true
            }
            "sig11img" => {
                if bo_pdfread::image_size(bytes).is_some() {
                    if let Some(d) = self.eleven.dialog.as_mut() {
                        d.image = Some(bytes.to_vec());
                    }
                } else {
                    self.status = self.t("pdfFill.badImage");
                }
                true
            }
            _ => false,
        }
    }

    fn open_sig_dialog11(&mut self, initials: bool) {
        self.eleven.dialog = Some(SigDialog {
            initials,
            ..SigDialog::default()
        });
    }

    /// Commands of round 11; true when `cmd` was one.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn command_eleven(&mut self, cmd: &str, m: &dyn TextMeasure) -> bool {
        if !self.eleven_on() {
            return false;
        }
        self.init11();
        match cmd {
            "fill.check" | "fill.cross" | "fill.dot" | "fill.line" => {
                // The round 5 marks; ours are cleared first.
                self.clear_fill11();
                return false;
            }
            "fill.box" => self.set_mark11(Mark11::Box),
            "fill.text" => self.set_mark11(Mark11::Text),
            "fill.keep" => {
                self.eleven.keep = !self.eleven.keep;
                if self.eleven.keep {
                    self.on.insert("fill.keep");
                } else {
                    self.on.remove("fill.keep");
                }
            }
            "fill.close" | "prot.fillSign" => {
                self.clear_fill11();
                return false;
            }
            "fill.newSig" => self.open_sig_dialog11(false),
            "fill.newInitials" => self.open_sig_dialog11(true),
            "fill.flattenSave" => self.eleven.flatten_on_save = !self.eleven.flatten_on_save,
            "fill.manage" => {
                let mut items = vec![
                    MenuItem {
                        label: self.t("pdfFill.newSig"),
                        choice: Choice::Command("fill.newSig".into()),
                        checked: false,
                        dim: false,
                    },
                    MenuItem {
                        label: self.t("pdfFill.newInitials"),
                        choice: Choice::Command("fill.newInitials".into()),
                        checked: false,
                        dim: false,
                    },
                ];
                for (i, s) in self.eleven.sigs.iter().enumerate() {
                    let name = match &s.ink {
                        SignatureInk::Typed { text, .. } => text.clone(),
                        SignatureInk::Strokes(_) => self.t("pdfFill.drawn"),
                        SignatureInk::Image(_) => self.t("pdfFill.picture"),
                    };
                    items.push(MenuItem {
                        label: self.strings.fmt("pdfFill.delete", &[("name", &name)]),
                        choice: Choice::Command(format!("fill.delSig.{i}")),
                        checked: false,
                        dim: false,
                    });
                }
                items.push(MenuItem {
                    label: self.t("pdfFill.flattenSave"),
                    choice: Choice::Command("fill.flattenSave".into()),
                    checked: self.eleven.flatten_on_save,
                    dim: false,
                });
                let r = self.control_rect("fill.manage").unwrap_or([0.0; 4]);
                let n = items.len();
                let menu = self
                    .reader_menu(items, r)
                    .with_decor(Vec::new(), vec![2, n - 1]);
                self.menu = Some((menu, "fill.manage"));
            }
            "menu.fillPredef" => {
                let items = vec![
                    MenuItem {
                        label: self.author.clone(),
                        choice: Choice::Command("fill.predef.name".into()),
                        checked: false,
                        dim: false,
                    },
                    MenuItem {
                        label: today11(),
                        choice: Choice::Command("fill.predef.date".into()),
                        checked: false,
                        dim: false,
                    },
                ];
                let r = self.control_rect("menu.fillPredef").unwrap_or([0.0; 4]);
                self.menu = Some((self.reader_menu(items, r), "menu.fillPredef"));
            }
            "fill.predef.name" => {
                let a = self.author.clone();
                self.set_mark11(Mark11::Predef(a));
            }
            "fill.predef.date" => self.set_mark11(Mark11::Predef(today11())),
            "form.highlight" => {
                self.tabs4.highlight_fields = !self.tabs4.highlight_fields;
                self.sync_tint11();
            }
            "form.reset" => {
                self.eleven.edit = None;
                self.eleven.focus = None;
                return false;
            }
            "file.save" | "file.saveAs" => {
                self.commit_field11();
                if self.eleven.flatten_on_save {
                    let r = self
                        .tabs
                        .get_mut(self.active)
                        .map(|t| t.view.doc_mut().flatten_fill_sign());
                    if let Some(Err(e)) = r {
                        self.status = e;
                    }
                    self.document_changed();
                }
                return false;
            }
            "read.page" | "read.from" => {
                self.start_reading11();
                return false;
            }
            "read.stop" => {
                self.eleven.word = None;
                self.eleven.reading.clear();
                return false;
            }
            c if c.starts_with("fill.delSig.") => {
                if let Ok(i) = c["fill.delSig.".len()..].parse::<usize>() {
                    if i < self.eleven.sigs.len() {
                        self.eleven.sigs.remove(i);
                        self.eleven.placing = None;
                        self.save_sigs11();
                    }
                }
            }
            _ => return false,
        }
        let _ = m;
        true
    }

    /// Input of round 11 first: `Some(redraw)` when handled.
    pub(crate) fn route_eleven(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if !self.eleven_on() {
            return None;
        }
        self.init11();
        if matches!(input, Input::Press { .. } | Input::Move { .. }) {
            self.sync_tint11();
        }
        if self.eleven.dialog.is_some() {
            return Some(self.dialog_input11(input));
        }
        if self.menu.is_some() || self.dialog.is_some() {
            return None;
        }
        // Ctrl+S with Flatten on save.
        if let Input::Key(Key::Letter('s'), mods) = input {
            if mods.ctrl {
                self.command_eleven("file.save", m);
                return None;
            }
        }
        if let Some(r) = self.typing_input11(input) {
            return Some(r);
        }
        if let Some(r) = self.fill_input11(input, m) {
            return Some(r);
        }
        if let Some(r) = self.form_input11(input, m) {
            return Some(r);
        }
        // A press elsewhere with a drawing tool drops the selection of the shape just drawn.
        if let Input::Press { x, y, .. } = *input {
            if inside(self.doc_rect, x, y)
                && (self.draw.tool.is_some() || self.tool == Tool::Rectangle)
                && self.extra.selected.is_some()
            {
                self.extra.selected = None;
            }
        }
        if let Input::Key(Key::Delete, _) = input {
            if self.draw.tool.is_some() || self.tool == Tool::Rectangle {
                if let Some(a) = self.extra.selected.take() {
                    if let Some(t) = self.tabs.get_mut(self.active) {
                        if let Err(e) = t.view.doc_mut().delete_annotation(&a) {
                            self.status = e;
                        }
                    }
                    self.document_changed();
                    return Some(true);
                }
            }
        }
        None
    }

    // ----- Fill & Sign -----------------------------------------------------------------

    fn page_point11(&self, x: f32, y: f32) -> Option<(usize, f32, f32)> {
        self.tabs.get(self.active)?.view.window_to_doc(x, y)
    }

    fn fill_input11(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        // The gallery.
        if let Input::Press { x, y, .. } = *input {
            if let Some(&(tile, _)) = self.eleven.gallery.iter().find(|(_, r)| inside(*r, x, y)) {
                if let Some(i) = tile {
                    self.clear_fill11();
                    self.tabs5.fill = None;
                    self.eleven.placing = Some(i);
                    self.status = self.t("pdfFill.placeSig");
                } else {
                    let items = vec![
                        MenuItem {
                            label: self.t("pdfFill.newSig"),
                            choice: Choice::Command("fill.newSig".into()),
                            checked: false,
                            dim: false,
                        },
                        MenuItem {
                            label: self.t("pdfFill.newInitials"),
                            choice: Choice::Command("fill.newInitials".into()),
                            checked: false,
                            dim: false,
                        },
                    ];
                    let r = self
                        .eleven
                        .gallery
                        .iter()
                        .find(|(t, _)| t.is_none())
                        .map_or([x, y, 0.0, 0.0], |g| g.1);
                    self.menu = Some((self.reader_menu(items, r), "fill.plus"));
                }
                return Some(true);
            }
        }
        let placing = self.eleven.placing.is_some();
        let marking = self.eleven.mark.is_some();
        let round5 = self.tabs5.fill.is_some();
        if !(placing || marking || round5) {
            return None;
        }
        match *input {
            Input::Press { x, y, .. } if inside(self.doc_rect, x, y) => {
                let (page, px, py) = self.page_point11(x, y)?;
                self.extra.selected = None;
                if let Some(i) = self.eleven.placing {
                    self.place_sig11(i, page, px, py);
                    if !self.eleven.keep {
                        self.eleven.placing = None;
                        self.status.clear();
                    }
                    return Some(true);
                }
                let before = self.view().map_or(0, |v| v.doc().annot_count(page));
                match self.eleven.mark.clone() {
                    Some(Mark11::Text) => {
                        self.commit_typing11();
                        self.eleven.typing = Some((page, [px, py], String::new()));
                        return Some(true);
                    }
                    Some(Mark11::Box) => {
                        let s = 18.0;
                        let mk = Markup {
                            kind: MarkupKind::Rectangle,
                            page,
                            rect: [px - s / 2.0, py - s / 2.0, px + s / 2.0, py + s / 2.0],
                            points: Vec::new(),
                            color: [0, 0, 0],
                            opacity: 1.0,
                            width: 2.0,
                            author: self.author.clone(),
                            contents: String::new(),
                            font_size: 12.0,
                        };
                        if let Some(t) = self.tabs.get_mut(self.active) {
                            if let Err(e) = t.view.doc_mut().add_markup(&mk) {
                                self.status = e;
                            }
                        }
                    }
                    Some(Mark11::Predef(text)) => self.add_fill_text11(page, [px, py], &text),
                    None => {
                        if let Some(f) = self.tabs5.fill {
                            self.place_fill(f, page, px, py, m);
                        }
                    }
                }
                self.mark_new11(page, before);
                Some(true)
            }
            Input::Move { x, y } if placing => {
                self.pointer = (x, y);
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.clear_fill11();
                self.tabs5.fill = None;
                self.status.clear();
                Some(true)
            }
            _ => None,
        }
    }

    /// Mark what was just added to `page` as Fill & Sign and select it (its corner handle).
    fn mark_new11(&mut self, page: usize, before: usize) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            if t.view.doc().annot_count(page) > before {
                if let Err(e) = t.view.doc_mut().mark_fill_sign(page, before) {
                    self.status = e;
                }
            }
        }
        self.document_changed();
        self.select_new11(page);
    }

    fn add_fill_text11(&mut self, page: usize, p: [f32; 2], text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let mk = Markup {
            kind: MarkupKind::Typewriter,
            page,
            // The first line sits where the box was opened (measured: the box's top left).
            rect: [p[0] + 2.0, p[1] + 2.0, p[0] + 3.0, p[1] + 3.0],
            points: Vec::new(),
            color: [0, 0, 0],
            opacity: 1.0,
            width: 1.0,
            author: self.author.clone(),
            contents: text.to_owned(),
            font_size: 12.0,
        };
        if let Some(t) = self.tabs.get_mut(self.active) {
            if let Err(e) = t.view.doc_mut().add_markup(&mk) {
                self.status = e;
            }
        }
    }

    fn commit_typing11(&mut self) {
        let Some((page, p, text)) = self.eleven.typing.take() else {
            return;
        };
        let before = self.view().map_or(0, |v| v.doc().annot_count(page));
        self.add_fill_text11(page, p, &text);
        self.mark_new11(page, before);
    }

    fn typing_input11(&mut self, input: &Input) -> Option<bool> {
        self.eleven.typing.as_ref()?;
        match input {
            Input::Text(s) => {
                if let Some((_, _, t)) = self.eleven.typing.as_mut() {
                    t.push_str(s);
                }
                Some(true)
            }
            Input::Key(Key::Backspace, _) => {
                if let Some((_, _, t)) = self.eleven.typing.as_mut() {
                    t.pop();
                }
                Some(true)
            }
            Input::Key(Key::Enter, mods) if !mods.shift => {
                if let Some((_, _, t)) = self.eleven.typing.as_mut() {
                    t.push('\n');
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.commit_typing11();
                Some(true)
            }
            Input::Press { x, y, .. } => {
                // A click elsewhere ends the text; with Add Text still chosen it starts another.
                let _ = (x, y);
                self.commit_typing11();
                None
            }
            _ => None,
        }
    }

    fn place_sig11(&mut self, i: usize, page: usize, px: f32, py: f32) {
        let Some(s) = self.eleven.sigs.get(i).cloned() else {
            return;
        };
        let (w, h) = sig_size(&s);
        let rect = [px - w / 2.0, py - h / 2.0, px + w / 2.0, py + h / 2.0];
        let ink = match &s.ink {
            SignatureInk::Strokes(st) => SignatureInk::Strokes(unit_strokes(st).0),
            other => other.clone(),
        };
        let author = self.author.clone();
        if let Some(t) = self.tabs.get_mut(self.active) {
            if let Err(e) = t
                .view
                .doc_mut()
                .add_signature_stamp(page, rect, &ink, s.color, &author)
            {
                self.status = e;
            }
        }
        self.document_changed();
    }

    // ----- Create Signature dialog ----------------------------------------------------------

    fn dlg_rect11(&self) -> [f32; 4] {
        let (w, h) = (self.px(DLG_W), self.px(DLG_H));
        let (ww, wh) = self.size;
        [((ww - w) / 2.0).round(), ((wh - h) / 2.0).round(), w, h]
    }

    /// Parts of the dialog (window pixels).
    fn dlg_parts11(&self) -> DlgParts {
        let d = self.dlg_rect11();
        let at = |x: f32, y: f32, w: f32, h: f32| {
            [d[0] + self.px(x), d[1] + self.px(y), self.px(w), self.px(h)]
        };
        DlgParts {
            close: at(DLG_W - 32.0, 0.0, 32.0, 25.6),
            tabs: [
                at(62.4, 46.4, 76.8, 28.8),
                at(138.4, 46.4, 76.8, 28.8),
                at(214.4, 46.4, 76.8, 28.8),
            ],
            panel: at(24.0, 88.0, 821.6, 336.0),
            edit: at(96.8, 98.4, 252.8, 27.2),
            font: at(359.2, 98.4, 193.6, 27.2),
            colour: at(565.6, 101.6, 20.8, 20.8),
            ok: at(667.2, 439.2, 84.8, 27.2),
            cancel: at(761.6, 439.2, 84.8, 27.2),
            browse: at(361.6, 230.4, 145.6, 27.2),
            clear: at(817.6, 96.0, 20.8, 20.8),
        }
    }

    fn dlg_ready11(d: &SigDialog) -> bool {
        match d.tab {
            0 => !d.text.trim().is_empty(),
            1 => d.strokes.iter().any(|s| s.len() > 1),
            _ => d.image.is_some(),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn dialog_input11(&mut self, input: &Input) -> bool {
        let parts = self.dlg_parts11();
        let Some(d) = self.eleven.dialog.as_mut() else {
            return false;
        };
        match *input {
            Input::Press { x, y, .. } => {
                if d.fonts_open {
                    d.fonts_open = false;
                    for (k, _) in FACES.iter().enumerate() {
                        let r = font_row(parts.font, k);
                        if inside(r, x, y) {
                            d.face = k;
                        }
                    }
                    return true;
                }
                if d.colours_open {
                    d.colours_open = false;
                    for k in 0..COLOURS.len() {
                        if inside(colour_cell(parts.colour_for(d.tab), k), x, y) {
                            d.colour = k;
                        }
                    }
                    return true;
                }
                if inside(parts.close, x, y) || inside(parts.cancel, x, y) {
                    self.eleven.dialog = None;
                    return true;
                }
                for (k, r) in parts.tabs.iter().enumerate() {
                    if inside(*r, x, y) {
                        d.tab = k as u8;
                        return true;
                    }
                }
                if inside(parts.ok, x, y) {
                    if Self::dlg_ready11(d) {
                        let d = d.clone();
                        self.finish_dialog11(&d, parts.panel);
                    }
                    return true;
                }
                match d.tab {
                    0 if inside(parts.font, x, y) => d.fonts_open = true,
                    0 | 1 if inside(parts.colour_for(d.tab), x, y) => d.colours_open = true,
                    1 if inside(parts.clear, x, y) => d.strokes.clear(),
                    1 if inside(parts.panel, x, y) => {
                        d.drawing = true;
                        d.strokes
                            .push(vec![[x - parts.panel[0], y - parts.panel[1]]]);
                    }
                    2 if inside(parts.browse, x, y) => {
                        let f = self.t("pdfComment.images");
                        self.requests.push(ReaderRequest::PickFile {
                            purpose: "sig11img".into(),
                            filters: vec![(f, "*.png;*.jpg;*.jpeg".into())],
                        });
                    }
                    _ => {}
                }
                true
            }
            Input::Drag { x, y } => {
                if d.drawing {
                    let p = [
                        (x - parts.panel[0]).clamp(0.0, parts.panel[2]),
                        (y - parts.panel[1]).clamp(0.0, parts.panel[3]),
                    ];
                    if let Some(s) = d.strokes.last_mut() {
                        if s.last()
                            .is_none_or(|q| (q[0] - p[0]).abs() + (q[1] - p[1]).abs() > 1.5)
                        {
                            s.push(p);
                        }
                    }
                }
                true
            }
            Input::Release { .. } => {
                d.drawing = false;
                true
            }
            Input::Text(ref s) if d.tab == 0 => {
                d.text.push_str(&s.replace(['\r', '\n'], ""));
                true
            }
            Input::Key(Key::Backspace, _) if d.tab == 0 => {
                d.text.pop();
                true
            }
            Input::Key(Key::Escape, _) => {
                self.eleven.dialog = None;
                true
            }
            Input::Key(Key::Enter, _) => {
                if Self::dlg_ready11(d) {
                    let d = d.clone();
                    self.finish_dialog11(&d, parts.panel);
                }
                true
            }
            Input::Key(Key::Tab, mods) => {
                d.tab = if mods.shift {
                    (d.tab + 2) % 3
                } else {
                    (d.tab + 1) % 3
                };
                true
            }
            Input::Resize { .. } | Input::Scale(_) => false,
            _ => true,
        }
    }

    /// OK: the signature is saved (first in the gallery) and ready to place.
    fn finish_dialog11(&mut self, d: &SigDialog, panel: [f32; 4]) {
        let ink = match d.tab {
            0 => SignatureInk::Typed {
                text: d.text.trim().to_owned(),
                face: FACES[d.face.min(FACES.len() - 1)].1,
            },
            1 => {
                let _ = panel;
                SignatureInk::Strokes(unit_strokes(&d.strokes).0)
            }
            _ => match &d.image {
                Some(b) => SignatureInk::Image(b.clone()),
                None => return,
            },
        };
        self.eleven.sigs.insert(
            0,
            SavedSig {
                ink,
                initials: d.initials,
                color: COLOURS[d.colour.min(COLOURS.len() - 1)],
            },
        );
        self.eleven.dialog = None;
        self.clear_fill11();
        self.tabs5.fill = None;
        self.eleven.placing = Some(0);
        self.status = self.t("pdfFill.placeSig");
        self.save_sigs11();
    }

    // ----- Forms ------------------------------------------------------------------------

    fn field_under11(&self, x: f32, y: f32) -> Option<FormField> {
        let (page, px, py) = self.page_point11(x, y)?;
        self.view()?.doc().form_fields().into_iter().find(|f| {
            f.page == page
                && !f.read_only
                && px >= f.rect[0]
                && px <= f.rect[2]
                && py >= f.rect[1]
                && py <= f.rect[3]
        })
    }

    /// Write the text being typed into its field (and run the calculations).
    pub(crate) fn commit_field11(&mut self) {
        let Some(e) = self.eleven.edit.take() else {
            return;
        };
        if e.text != e.f.value {
            self.set_field(&e.f, Some(&e.text), None);
        }
    }

    fn focus_field11(&mut self, f: FormField, by_tab: bool, m: &dyn TextMeasure) {
        self.commit_field11();
        let _ = m;
        if f.kind == FieldKind::Text {
            let text = f.value.clone();
            let caret = text.chars().count();
            self.eleven.edit = Some(FieldEdit {
                f: f.clone(),
                text,
                caret,
                all: by_tab,
            });
        }
        // Show the field when Tab reaches one out of view.
        if by_tab {
            let shown = self
                .view()
                .and_then(|v| v.doc_rect_to_window(f.page, f.rect))
                .is_some_and(|r| {
                    let d = self.doc_rect;
                    r[1] >= d[1] && r[1] + r[3] <= d[1] + d[3]
                });
            if !shown {
                if let Some(t) = self.tabs.get_mut(self.active) {
                    t.view.go_to(f.page, Some((f.rect[1] - 40.0).max(0.0)));
                }
            }
        }
        self.eleven.focus = Some(f);
    }

    fn next_field11(&mut self, back: bool, m: &dyn TextMeasure) {
        let order = self
            .view()
            .map(|v| v.doc().form_tab_order())
            .unwrap_or_default();
        if order.is_empty() {
            return;
        }
        let cur = self
            .eleven
            .focus
            .as_ref()
            .and_then(|f| order.iter().position(|o| o.same_field(f)));
        let n = order.len();
        let k = match (cur, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        // The order was read before the commit; take the field again after it.
        self.commit_field11();
        let f = self
            .view()
            .map(|v| v.doc().form_tab_order())
            .and_then(|o| o.into_iter().nth(k));
        if let Some(f) = f {
            self.focus_field11(f, true, m);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn form_input11(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        match *input {
            Input::Press { x, y, count, .. } => {
                if !inside(self.doc_rect, x, y) || self.tool != Tool::Hand {
                    self.commit_field11();
                    self.eleven.focus = None;
                    return None;
                }
                let Some(f) = self.field_under11(x, y) else {
                    if self.eleven.focus.is_some() {
                        self.commit_field11();
                        self.eleven.focus = None;
                    }
                    return None;
                };
                let _ = count;
                match f.kind {
                    FieldKind::Text => {
                        let same = self
                            .eleven
                            .edit
                            .as_ref()
                            .is_some_and(|e| e.f.same_widget(&f));
                        if !same {
                            self.focus_field11(f.clone(), false, m);
                        }
                        // Caret at the click: the nearest character boundary by width.
                        let caret = self.caret_at11(&f, x);
                        if let Some(e) = self.eleven.edit.as_mut() {
                            e.caret = caret.min(e.text.chars().count());
                            e.all = false;
                        }
                    }
                    FieldKind::List => {
                        self.focus_field11(f.clone(), false, m);
                        let Some((_, _, py)) = self.page_point11(x, y) else {
                            return Some(true);
                        };
                        // Rows of the list box: its font's line height from the top.
                        let row = 13.0;
                        let k = ((py - f.rect[1] - 2.0) / row).floor().max(0.0) as usize;
                        if let Some(v) = f.options.get(k).cloned() {
                            self.set_field(&f, Some(&v), None);
                            self.refocus11(&f);
                        }
                    }
                    FieldKind::CheckBox | FieldKind::Radio => {
                        self.focus_field11(f.clone(), false, m);
                        let on = f.kind == FieldKind::Radio || !f.checked;
                        self.set_field(&f, None, Some(on));
                        self.refocus11(&f);
                    }
                    FieldKind::Combo => {
                        self.focus_field11(f.clone(), false, m);
                        self.fill_field(f, m);
                    }
                    FieldKind::Button | FieldKind::Signature => return None,
                }
                Some(true)
            }
            Input::Key(Key::Tab, mods) if self.eleven.focus.is_some() || self.has_form9() => {
                if self.eleven.focus.is_none()
                    && !inside(self.doc_rect, self.pointer.0, self.pointer.1)
                {
                    return None;
                }
                self.next_field11(mods.shift, m);
                Some(true)
            }
            _ => self.field_keys11(input, m),
        }
    }

    /// The field again after a change (its value and state are new).
    fn refocus11(&mut self, f: &FormField) {
        let again = self
            .view()
            .and_then(|v| v.doc().form_fields().into_iter().find(|g| g.same_widget(f)));
        if let Some(g) = again {
            self.eleven.focus = Some(g);
        }
    }

    fn caret_at11(&self, f: &FormField, x: f32) -> usize {
        let Some(r) = self
            .view()
            .and_then(|v| v.doc_rect_to_window(f.page, f.rect))
        else {
            return 0;
        };
        let text = self
            .eleven
            .edit
            .as_ref()
            .filter(|e| e.f.same_widget(f))
            .map_or_else(|| f.value.clone(), |e| e.text.clone());
        let size = self.field_font11(f);
        let adv = size * 0.52;
        let k = ((x - r[0] - self.px(2.0)) / adv).round().max(0.0) as usize;
        k.min(text.chars().count())
    }

    /// Text size of a field being typed in (device pixels): its box height, as its appearance.
    fn field_font11(&self, f: &FormField) -> f32 {
        let s = self.view().map_or(1.0, super::ReaderView::px_per_pt);
        let h = f.rect[3] - f.rect[1];
        (h * 0.7).clamp(4.0, 12.0) * s
    }

    #[allow(clippy::too_many_lines)]
    fn field_keys11(&mut self, input: &Input, m: &dyn TextMeasure) -> Option<bool> {
        if let Some(e) = self.eleven.edit.as_mut() {
            let n = e.text.chars().count();
            let byte = |t: &str, c: usize| t.char_indices().nth(c).map_or(t.len(), |(b, _)| b);
            match input {
                Input::Text(s) => {
                    if e.all {
                        e.text.clear();
                        e.caret = 0;
                        e.all = false;
                    }
                    let clean: String = if e.f.multiline {
                        s.clone()
                    } else {
                        s.replace(['\r', '\n'], "")
                    };
                    let at = byte(&e.text, e.caret);
                    e.text.insert_str(at, &clean);
                    e.caret += clean.chars().count();
                    return Some(true);
                }
                Input::Key(k, mods) => {
                    match k {
                        Key::Backspace => {
                            if e.all {
                                e.text.clear();
                                e.caret = 0;
                                e.all = false;
                            } else if e.caret > 0 {
                                let a = byte(&e.text, e.caret - 1);
                                let b = byte(&e.text, e.caret);
                                e.text.replace_range(a..b, "");
                                e.caret -= 1;
                            }
                        }
                        Key::Delete => {
                            if e.all {
                                e.text.clear();
                                e.caret = 0;
                                e.all = false;
                            } else if e.caret < n {
                                let a = byte(&e.text, e.caret);
                                let b = byte(&e.text, e.caret + 1);
                                e.text.replace_range(a..b, "");
                            }
                        }
                        Key::Left => {
                            e.caret = if e.all { 0 } else { e.caret.saturating_sub(1) };
                            e.all = false;
                        }
                        Key::Right => {
                            e.caret = if e.all { n } else { (e.caret + 1).min(n) };
                            e.all = false;
                        }
                        Key::Home => {
                            e.caret = 0;
                            e.all = false;
                        }
                        Key::End => {
                            e.caret = n;
                            e.all = false;
                        }
                        Key::Letter('a') if mods.ctrl => e.all = true,
                        Key::Enter if e.f.multiline && !mods.ctrl => {
                            let at = byte(&e.text, e.caret);
                            e.text.insert(at, '\n');
                            e.caret += 1;
                        }
                        Key::Enter => {
                            self.commit_field11();
                            let f = self.eleven.focus.clone();
                            if let Some(f) = f {
                                self.refocus11(&f);
                            }
                        }
                        Key::Escape => {
                            // Typing is dropped, the field keeps its value.
                            self.eleven.edit = None;
                            self.eleven.focus = None;
                        }
                        Key::Letter(_) if mods.ctrl => return None,
                        _ => return Some(true),
                    }
                    return Some(true);
                }
                _ => return None,
            }
        }
        let f = self.eleven.focus.clone()?;
        match input {
            Input::Text(s) if s == " " => {
                match f.kind {
                    FieldKind::CheckBox | FieldKind::Radio => {
                        let on = f.kind == FieldKind::Radio || !f.checked;
                        self.set_field(&f, None, Some(on));
                        self.refocus11(&f);
                    }
                    FieldKind::Combo => self.fill_field(f, m),
                    _ => {}
                }
                Some(true)
            }
            Input::Key(Key::Up | Key::Down, mods)
                if matches!(f.kind, FieldKind::Combo | FieldKind::List) =>
            {
                if mods.alt && f.kind == FieldKind::Combo {
                    self.fill_field(f, m);
                    return Some(true);
                }
                let cur = f.options.iter().position(|o| *o == f.value);
                let down = matches!(input, Input::Key(Key::Down, _));
                let k = match (cur, down) {
                    (None, _) => 0,
                    (Some(i), true) => (i + 1).min(f.options.len().saturating_sub(1)),
                    (Some(i), false) => i.saturating_sub(1),
                };
                if let Some(v) = f.options.get(k).cloned() {
                    self.set_field(&f, Some(&v), None);
                    self.refocus11(&f);
                }
                Some(true)
            }
            Input::Key(Key::Escape, _) => {
                self.eleven.focus = None;
                Some(true)
            }
            _ => None,
        }
    }

    // ----- Read Aloud -------------------------------------------------------------------

    /// Remember how the spoken text maps to page characters (the same text View › Read
    /// speaks).
    fn start_reading11(&mut self) {
        self.eleven.reading.clear();
        self.eleven.word = None;
        let Some(t) = self.tabs.get_mut(self.active) else {
            return;
        };
        let first = t.view.current_page();
        let last = t.view.page_count().saturating_sub(1);
        let mut at = 0usize;
        for i in first..=last {
            if let Some(layer) = t.view.text(i) {
                let text = layer.text();
                let units: Vec<usize> = text.chars().map(char::len_utf16).collect();
                self.eleven
                    .reading
                    .push((i, at, layer.reading_chars(), units));
                at += text.encode_utf16().count() + 1;
            }
        }
    }

    /// Reading aloud is on.
    pub(crate) fn reading11(&self) -> bool {
        self.view4.reading && !self.eleven.reading.is_empty()
    }

    /// The voice is saying the word at UTF-16 offset `pos`, `len` long: highlight it. True
    /// when the highlight moved.
    pub(crate) fn speech_word11(&mut self, pos: usize, len: usize) -> bool {
        let Some((page, start, chars, units)) = self
            .eleven
            .reading
            .iter()
            .rev()
            .find(|r| r.1 <= pos)
            .cloned()
        else {
            return false;
        };
        // UTF-16 offset in the page's text → its character index.
        let mut u = start;
        let mut first = None;
        let mut last = None;
        for (k, n) in units.iter().enumerate() {
            if u >= pos && first.is_none() {
                first = Some(k);
            }
            if u < pos + len.max(1) {
                last = Some(k);
            }
            u += n;
        }
        let (Some(a), Some(b)) = (first, last) else {
            return false;
        };
        let pick = |k: usize| chars.get(k).copied().filter(|c| *c != usize::MAX);
        let (first, last) = (a, b);
        let a = (first..=last).find_map(pick);
        let b = (first..=last).rev().find_map(pick);
        let (Some(a), Some(b)) = (a, b) else {
            return false;
        };
        let w = Some((page, a.min(b), a.max(b)));
        if w == self.eleven.word {
            return false;
        }
        self.eleven.word = w;
        // Follow the voice to the next page.
        if let Some(t) = self.tabs.get_mut(self.active) {
            if !t.view.visible_pages().contains(&page) {
                t.view.go_to(page, None);
            }
        }
        true
    }

    // ----- Accessibility ----------------------------------------------------------------

    /// Every command of the ribbon and title bar with the name assistive technology reads
    /// (its label, else its tooltip) and its box.
    pub fn accessible_names11(&self) -> Vec<(&'static str, String, [f32; 4])> {
        self.controls
            .iter()
            .filter(|c| !matches!(c.kind, super::Kind::Sep))
            .map(|c| {
                let name = match c.kind {
                    super::Kind::Win(_) => self.t(match c.id {
                        "win.min" => "pdfA11y.minimize",
                        "win.max" => "pdfA11y.maximize",
                        _ => "pdfA11y.close",
                    }),
                    _ if c.id == "fill.gallery" => self.t("pdfFill.gallery"),
                    _ => self.tip_text9(c.id).unwrap_or_default(),
                };
                (c.id, name, c.r)
            })
            .collect()
    }

    // ----- Painting ---------------------------------------------------------------------

    /// Round 11 on top of the window: the gallery, the field being typed in and its focus
    /// frame, the Add Text box, the signature at the pointer, the spoken word, the dialog.
    pub(crate) fn paint_eleven(&mut self, p: &mut dyn Painter) {
        if !self.eleven_on() {
            return;
        }
        self.init11();
        self.sync_tint11();
        self.paint_gallery11(p);
        let d = self.doc_rect;
        if self.showing_doc() {
            p.push_clip(d[0], d[1], d[2], d[3]);
            self.paint_word11(p);
            self.paint_field11(p);
            self.paint_typing11(p);
            self.paint_placing11(p);
            p.pop_clip();
        }
        self.paint_dialog11(p);
    }

    fn paint_word11(&self, p: &mut dyn Painter) {
        let Some((page, a, b)) = self.eleven.word else {
            return;
        };
        let Some(v) = self.view() else { return };
        let Some(layer) = v.text_if_ready(page) else {
            return;
        };
        for r in layer.rects(a, b) {
            if let Some(w) = v.doc_rect_to_window(page, r) {
                p.fill_rect(w[0], w[1], w[2], w[3], WORD_BG);
            }
        }
    }

    fn paint_field11(&self, p: &mut dyn Painter) {
        let Some(f) = self.eleven.focus.as_ref() else {
            return;
        };
        let Some(v) = self.view() else { return };
        let Some(r) = v.doc_rect_to_window(f.page, f.rect) else {
            return;
        };
        let r = r.map(f32::round);
        p.raw_colours(true);
        if let Some(e) = self.eleven.edit.as_ref().filter(|e| e.f.same_widget(f)) {
            // Measured: white while typing, the text from 2 px in, a 1 px caret.
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
            let size = self.field_font11(f);
            let st = TextStyle { size, bold: false };
            let lines: Vec<&str> = if f.multiline {
                e.text.split('\n').collect()
            } else {
                vec![e.text.as_str()]
            };
            let lh = (size * 1.2).round();
            let top = if f.multiline {
                r[1] + self.px(1.6)
            } else {
                (r[1] + (r[3] - lh) / 2.0).round()
            };
            let x0 = r[0] + self.px(2.0);
            let adv = size * 0.52;
            p.push_clip(r[0], r[1], r[2], r[3]);
            for (k, l) in lines.iter().enumerate() {
                let y = top + lh * k as f32;
                if e.all && !l.is_empty() {
                    let w = l.chars().count() as f32 * adv;
                    p.fill_rect(x0, y, w, lh, SELECT_BG);
                    p.text(l, x0, y, st, Rgba(255, 255, 255, 255));
                } else {
                    p.text(l, x0, y, st, INK);
                }
            }
            if !e.all {
                // Caret: count characters on its line.
                let before: String = e.text.chars().take(e.caret).collect();
                let line = before.matches('\n').count();
                let col = before.rsplit('\n').next().map_or(0, |s| s.chars().count());
                let cx = (x0 + col as f32 * adv).round();
                p.fill_rect(cx, top + lh * line as f32, 1.0, lh, INK);
            }
            p.pop_clip();
        }
        // The focus frame: 1 px dotted, one pixel outside the field (measured).
        dotted(
            p,
            [r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0],
            INK,
            1.0,
        );
        p.raw_colours(false);
    }

    fn paint_typing11(&self, p: &mut dyn Painter) {
        let Some((page, pt, text)) = self.eleven.typing.as_ref() else {
            return;
        };
        let Some(v) = self.view() else { return };
        let s = v.px_per_pt();
        let Some(o) = v.doc_rect_to_window(*page, [pt[0], pt[1], pt[0], pt[1]]) else {
            return;
        };
        let size = 12.0 * s;
        let st = TextStyle { size, bold: false };
        let lines: Vec<&str> = text.split('\n').collect();
        let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
        let lh = (size * 1.25).round();
        let w = (longest * size * 0.52 + self.px(12.0))
            .max(self.px(22.0))
            .round();
        let h = (lh * lines.len() as f32 + self.px(6.0)).round();
        let (x, y) = (o[0].round(), o[1].round());
        p.raw_colours(true);
        for e in [
            [x, y, w, 1.0],
            [x, y + h - 1.0, w, 1.0],
            [x, y, 1.0, h],
            [x + w - 1.0, y, 1.0, h],
        ] {
            p.fill_rect(e[0], e[1], e[2], e[3], TYPING_EDGE);
        }
        let tx = x + self.px(5.0);
        for (k, l) in lines.iter().enumerate() {
            p.text(l, tx, y + self.px(3.0) + lh * k as f32, st, INK);
        }
        let last = lines.last().map_or(0, |l| l.chars().count()) as f32;
        let cx = (tx + last * size * 0.52).round();
        p.fill_rect(
            cx,
            y + self.px(3.0) + lh * (lines.len() - 1) as f32,
            1.0,
            lh,
            INK,
        );
        p.raw_colours(false);
    }

    /// The signature being placed follows the pointer at its placed size.
    fn paint_placing11(&self, p: &mut dyn Painter) {
        let Some(s) = self.eleven.placing.and_then(|i| self.eleven.sigs.get(i)) else {
            return;
        };
        let (x, y) = self.pointer;
        if !inside(self.doc_rect, x, y) {
            return;
        }
        let k = self.view().map_or(1.0, super::ReaderView::px_per_pt);
        let (w, h) = sig_size(s);
        let b = [x - w * k / 2.0, y - h * k / 2.0, w * k, h * k];
        p.raw_colours(true);
        paint_ink(p, s, b, 160);
        p.raw_colours(false);
    }

    fn paint_gallery11(&mut self, p: &mut dyn Painter) {
        self.eleven.gallery.clear();
        let Some(r) = self.control_rect("fill.gallery") else {
            return;
        };
        if self.ribbon_tab() != super::tabs5::FILL_TAB {
            return;
        }
        // Measured: a white box with a 1 px edge, tiles from 7 px in, a column of arrows.
        let r = [r[0], r[1] + self.px(1.6), r[2], r[3] - self.px(3.2)];
        p.fill_rect(r[0], r[1], r[2], r[3], EDGE);
        p.fill_rect(
            r[0] + 1.0,
            r[1] + 1.0,
            r[2] - 2.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let (tw, th, gap) = (self.px(TILE_W), self.px(TILE_H), self.px(TILE_GAP));
        let ty = (r[1] + (r[3] - th) / 2.0).round();
        let mut x = r[0] + self.px(5.6);
        let right = r[0] + r[2] - self.px(16.0);
        let sigs = self.eleven.sigs.clone();
        for (i, s) in sigs.iter().enumerate() {
            if x + tw > right - self.px(40.0) {
                break;
            }
            let tile = [x, ty, tw, th];
            if self.eleven.placing == Some(i) {
                p.fill_rect(tile[0], tile[1], tile[2], tile[3], TILE_SEL);
            }
            let pad = self.px(4.0);
            let inner = [
                tile[0] + pad,
                tile[1] + pad,
                tile[2] - 2.0 * pad,
                tile[3] - 2.0 * pad,
            ];
            p.push_clip(tile[0], tile[1], tile[2], tile[3]);
            paint_ink(p, s, fit_box(inner, sig_size(s)), 255);
            p.pop_clip();
            self.eleven.gallery.push((Some(i), tile));
            x += tw + gap;
        }
        // "+": a 30 px cross, 4 px strokes (measured), centred in a tile.
        let plus = [x, ty, (tw * 0.7).round(), th];
        let (cx, cy) = (plus[0] + plus[2] / 2.0, plus[1] + plus[3] / 2.0);
        let (len, wd) = (self.px(12.8), self.px(3.2));
        let ink = Rgba(102, 102, 102, 255);
        p.fill_rect(
            (cx - len).round(),
            (cy - wd / 2.0).round(),
            2.0 * len,
            wd,
            ink,
        );
        p.fill_rect(
            (cx - wd / 2.0).round(),
            (cy - len).round(),
            wd,
            2.0 * len,
            ink,
        );
        self.eleven.gallery.push((None, plus));
        // Scroll arrows (drawn; a long list scrolls in Manage Signatures).
        let ax = r[0] + r[2] - self.px(9.6);
        for (k, up) in [(0, true), (1, false), (2, false)] {
            let ay = r[1] + self.px(10.4) + self.px(21.6) * k as f32;
            let s = self.px(3.2);
            let segs = if up {
                vec![
                    PathSeg::M(ax - s, ay + s / 2.0),
                    PathSeg::L(ax + s, ay + s / 2.0),
                    PathSeg::L(ax, ay - s / 2.0),
                    PathSeg::Z,
                ]
            } else {
                vec![
                    PathSeg::M(ax - s, ay - s / 2.0),
                    PathSeg::L(ax + s, ay - s / 2.0),
                    PathSeg::L(ax, ay + s / 2.0),
                    PathSeg::Z,
                ]
            };
            p.path(&segs, Some(Rgba(110, 110, 110, 255)), None);
            if k == 2 {
                p.fill_rect(ax - s, ay - s, 2.0 * s, 1.0, Rgba(110, 110, 110, 255));
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint_dialog11(&self, p: &mut dyn Painter) {
        let Some(d) = self.eleven.dialog.as_ref() else {
            return;
        };
        let parts = self.dlg_parts11();
        let r = self.dlg_rect11();
        let (w, h) = self.size;
        // Shade the window behind, then the dialog with its title row.
        p.fill_rect(0.0, 0.0, w, h, Rgba(0, 0, 0, 40));
        p.fill_rect(
            r[0] - 1.0,
            r[1] - 1.0,
            r[2] + 2.0,
            r[3] + 2.0,
            Rgba(130, 130, 130, 255),
        );
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
        let title_h = self.px(25.6);
        p.fill_rect(r[0], r[1], r[2], title_h, Rgba(243, 243, 243, 255));
        let ts = self.style(17.0, false);
        let title = self.t(if d.initials {
            "pdfFill.createInitials"
        } else {
            "pdfFill.createSig"
        });
        p.text(&title, r[0] + self.px(6.4), r[1] + self.px(4.0), ts, INK);
        // Close box: an x.
        let c = parts.close;
        let (cx, cy, s) = (c[0] + c[2] / 2.0, c[1] + c[3] / 2.0, self.px(5.6));
        p.path(
            &[
                PathSeg::M(cx - s, cy - s),
                PathSeg::L(cx + s, cy + s),
                PathSeg::M(cx + s, cy - s),
                PathSeg::L(cx - s, cy + s),
            ],
            None,
            Some((INK, 1.0)),
        );
        // Tabs: the chosen one bold with a notch into the panel below.
        let tab_style = self.style(17.0, false);
        let bold = self.style(17.0, true);
        for (k, tr) in parts.tabs.iter().enumerate() {
            let key = ["pdfFill.type", "pdfFill.draw", "pdfFill.import"][k];
            let label = self.t(key);
            let on = d.tab as usize == k;
            let st = if on { bold } else { tab_style };
            let tw = label.chars().count() as f32 * st.size * 0.5;
            p.text(
                &label,
                (tr[0] + (tr[2] - tw) / 2.0).round(),
                tr[1] + self.px(4.0),
                st,
                Rgba(51, 51, 51, 255),
            );
        }
        let pr = parts.panel;
        p.fill_rect(pr[0], pr[1], pr[2], pr[3], EDGE);
        p.fill_rect(
            pr[0] + 1.0,
            pr[1] + 1.0,
            pr[2] - 2.0,
            pr[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let at = parts.tabs[d.tab as usize];
        let (nx, ny, ns) = (at[0] + at[2] / 2.0, pr[1], self.px(8.0));
        p.path(
            &[
                PathSeg::M(nx - ns, ny + 0.5),
                PathSeg::L(nx, ny - ns + 0.5),
                PathSeg::L(nx + ns, ny + 0.5),
                PathSeg::Z,
            ],
            Some(Rgba(255, 255, 255, 255)),
            Some((EDGE, 1.0)),
        );
        p.fill_rect(
            nx - ns + 1.0,
            ny,
            2.0 * ns - 2.0,
            1.0,
            Rgba(255, 255, 255, 255),
        );
        let ui = self.style(15.0, false);
        let colour = COLOURS[d.colour.min(COLOURS.len() - 1)];
        let preview = SavedSig {
            ink: SignatureInk::Strokes(Vec::new()),
            initials: d.initials,
            color: colour,
        };
        match d.tab {
            0 => {
                p.text(
                    &self.t("pdfFill.signature"),
                    pr[0] + self.px(12.8),
                    parts.edit[1] + self.px(5.6),
                    ui,
                    INK,
                );
                field_box(p, parts.edit, Rgba(31, 123, 204, 255));
                p.text(
                    &d.text,
                    parts.edit[0] + self.px(4.0),
                    parts.edit[1] + self.px(5.6),
                    ui,
                    INK,
                );
                let cw = d.text.chars().count() as f32 * ui.size * 0.5;
                p.fill_rect(
                    (parts.edit[0] + self.px(4.0) + cw).round(),
                    parts.edit[1] + self.px(4.8),
                    1.0,
                    parts.edit[3] - self.px(9.6),
                    INK,
                );
                field_box(p, parts.font, Rgba(122, 122, 122, 255));
                let face = FACES[d.face.min(FACES.len() - 1)];
                p.text(
                    face.0,
                    parts.font[0] + self.px(4.0),
                    parts.font[1] + self.px(5.6),
                    ui,
                    INK,
                );
                chevron(
                    p,
                    parts.font[0] + parts.font[2] - self.px(12.0),
                    parts.font[1] + parts.font[3] / 2.0,
                    self.px(4.0),
                );
                colour_dot(p, parts.colour, colour);
                if d.text.trim().is_empty() {
                    let hint = self.t("pdfFill.typeHint");
                    let hw = hint.chars().count() as f32 * ui.size * 0.48;
                    p.text(
                        &hint,
                        pr[0] + (pr[2] - hw) / 2.0,
                        pr[1] + pr[3] * 0.55,
                        ui,
                        INK,
                    );
                } else {
                    let s = SavedSig {
                        ink: SignatureInk::Typed {
                            text: d.text.clone(),
                            face: face.1,
                        },
                        ..preview
                    };
                    let area = [
                        pr[0] + self.px(12.0),
                        pr[1] + self.px(48.0),
                        pr[2] - self.px(24.0),
                        pr[3] - self.px(60.0),
                    ];
                    paint_ink(p, &s, fit_box(area, sig_size(&s)), 255);
                }
                if d.fonts_open {
                    for (k, (name, _)) in FACES.iter().enumerate() {
                        let rr = font_row(parts.font, k);
                        p.fill_rect(
                            rr[0],
                            rr[1],
                            rr[2],
                            rr[3],
                            if k == d.face {
                                TILE_SEL
                            } else {
                                Rgba(255, 255, 255, 255)
                            },
                        );
                        p.text(name, rr[0] + self.px(4.0), rr[1] + self.px(4.0), ui, INK);
                    }
                    let last = font_row(parts.font, FACES.len() - 1);
                    let top = font_row(parts.font, 0);
                    field_box_outline(
                        p,
                        [top[0], top[1], top[2], last[1] + last[3] - top[1]],
                        EDGE,
                    );
                }
            }
            1 => {
                colour_dot(p, parts.colour_for(1), colour);
                // Eraser: our own drawing (a tilted block over a base line).
                let e = parts.clear;
                let (ex, ey) = (e[0], e[1]);
                let k = e[2] / 24.0;
                let pts = [(6.0, 15.0), (14.0, 5.0), (20.0, 10.0), (12.0, 20.0)];
                let mut segs: Vec<PathSeg> = pts
                    .iter()
                    .enumerate()
                    .map(|(i, (x, y))| {
                        let (x, y) = (ex + x * k, ey + y * k);
                        if i == 0 {
                            PathSeg::M(x, y)
                        } else {
                            PathSeg::L(x, y)
                        }
                    })
                    .collect();
                segs.push(PathSeg::Z);
                segs.push(PathSeg::M(ex + 4.0 * k, ey + 22.0 * k));
                segs.push(PathSeg::L(ex + 20.0 * k, ey + 22.0 * k));
                p.path(&segs, None, Some((Rgba(51, 51, 51, 255), 1.0)));
                for st in &d.strokes {
                    let segs: Vec<PathSeg> = st
                        .iter()
                        .enumerate()
                        .map(|(i, q)| {
                            let (x, y) = (pr[0] + q[0], pr[1] + q[1]);
                            if i == 0 {
                                PathSeg::M(x, y)
                            } else {
                                PathSeg::L(x, y)
                            }
                        })
                        .collect();
                    let [cr, cg, cb] = colour;
                    p.path(&segs, None, Some((Rgba(cr, cg, cb, 255), self.px(2.4))));
                }
                let hint = self.t("pdfFill.drawHint");
                let hw = hint.chars().count() as f32 * ui.size * 0.48;
                p.text(
                    &hint,
                    pr[0] + (pr[2] - hw) / 2.0,
                    pr[1] + pr[3] - self.px(20.0),
                    ui,
                    Rgba(150, 150, 151, 255),
                );
            }
            _ => {
                field_box(p, parts.browse, Rgba(122, 122, 122, 255));
                let b = self.t("pdfFill.browse");
                let bw = b.chars().count() as f32 * ui.size * 0.48;
                p.text(
                    &b,
                    parts.browse[0] + (parts.browse[2] - bw) / 2.0,
                    parts.browse[1] + self.px(5.6),
                    ui,
                    INK,
                );
                let msg = match &d.image {
                    Some(img) => {
                        let (iw, ih) = bo_pdfread::image_size(img).unwrap_or((0, 0));
                        self.strings.fmt(
                            "pdfFill.imageLoaded",
                            &[("w", &iw.to_string()), ("h", &ih.to_string())],
                        )
                    }
                    None => self.t("pdfFill.importHint"),
                };
                let mw = msg.chars().count() as f32 * ui.size * 0.48;
                p.text(
                    &msg,
                    pr[0] + (pr[2] - mw) / 2.0,
                    parts.browse[1] + self.px(44.0),
                    ui,
                    INK,
                );
            }
        }
        // OK (accent, pale until there is a signature) and Cancel.
        let ready = Self::dlg_ready11(d);
        let accent = self.theme.accent;
        let ok_fill = if ready {
            accent
        } else {
            let m = |v: u8| ((u16::from(v) + 3 * 255) / 4) as u8;
            Rgba(m(accent.0), m(accent.1), m(accent.2), 255)
        };
        let ob = parts.ok;
        p.path(&rounded_rect(ob, self.px(3.2)), Some(ok_fill), None);
        let ok = self.t("pdfReader.ok");
        let okw = ok.chars().count() as f32 * ui.size * 0.5;
        p.text(
            &ok,
            ob[0] + (ob[2] - okw) / 2.0,
            ob[1] + self.px(5.6),
            ui,
            Rgba(255, 255, 255, 255),
        );
        let cb = parts.cancel;
        p.path(
            &rounded_rect(cb, self.px(3.2)),
            Some(Rgba(255, 255, 255, 255)),
            Some((Rgba(190, 190, 190, 255), 1.0)),
        );
        let cancel = self.t("pdfPrint.cancel");
        let cw = cancel.chars().count() as f32 * ui.size * 0.5;
        p.text(
            &cancel,
            cb[0] + (cb[2] - cw) / 2.0,
            cb[1] + self.px(5.6),
            ui,
            INK,
        );
        if d.colours_open {
            for (k, col) in COLOURS.iter().enumerate() {
                let cc = colour_cell(parts.colour_for(d.tab), k);
                p.fill_rect(
                    cc[0] - 2.0,
                    cc[1] - 2.0,
                    cc[2] + 4.0,
                    cc[3] + 4.0,
                    Rgba(255, 255, 255, 255),
                );
                colour_dot(p, cc, *col);
            }
        }
    }
}

/// Boxes of the Create Signature dialog.
struct DlgParts {
    close: [f32; 4],
    tabs: [[f32; 4]; 3],
    panel: [f32; 4],
    edit: [f32; 4],
    font: [f32; 4],
    colour: [f32; 4],
    ok: [f32; 4],
    cancel: [f32; 4],
    browse: [f32; 4],
    clear: [f32; 4],
}

impl DlgParts {
    /// The colour dot: beside the font on Type, top right of the panel on Draw.
    fn colour_for(&self, tab: u8) -> [f32; 4] {
        if tab == 1 {
            [
                self.clear[0] - self.colour[2] - 8.0,
                self.clear[1],
                self.colour[2],
                self.colour[3],
            ]
        } else {
            self.colour
        }
    }
}

fn font_row(font: [f32; 4], k: usize) -> [f32; 4] {
    [
        font[0],
        font[1] + font[3] * (k as f32 + 1.0),
        font[2],
        font[3],
    ]
}

fn colour_cell(dot: [f32; 4], k: usize) -> [f32; 4] {
    [
        dot[0],
        dot[1] + (dot[3] + 6.0) * (k as f32 + 1.0),
        dot[2],
        dot[3],
    ]
}

fn field_box(p: &mut dyn Painter, r: [f32; 4], edge: Rgba) {
    p.fill_rect(r[0], r[1], r[2], r[3], edge);
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
}

fn field_box_outline(p: &mut dyn Painter, r: [f32; 4], edge: Rgba) {
    for e in [
        [r[0], r[1], r[2], 1.0],
        [r[0], r[1] + r[3] - 1.0, r[2], 1.0],
        [r[0], r[1], 1.0, r[3]],
        [r[0] + r[2] - 1.0, r[1], 1.0, r[3]],
    ] {
        p.fill_rect(e[0], e[1], e[2], e[3], edge);
    }
}

fn chevron(p: &mut dyn Painter, x: f32, y: f32, s: f32) {
    p.path(
        &[
            PathSeg::M(x - s, y - s / 2.0),
            PathSeg::L(x, y + s / 2.0),
            PathSeg::L(x + s, y - s / 2.0),
        ],
        None,
        Some((Rgba(90, 90, 90, 255), 1.0)),
    );
}

fn colour_dot(p: &mut dyn Painter, r: [f32; 4], c: [u8; 3]) {
    let (cx, cy, rad) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, r[2] / 2.0);
    p.path(
        &circle(cx, cy, rad),
        Some(Rgba(255, 255, 255, 255)),
        Some((Rgba(160, 160, 160, 255), 1.0)),
    );
    p.path(
        &circle(cx, cy, rad - 2.0),
        Some(Rgba(c[0], c[1], c[2], 255)),
        None,
    );
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<PathSeg> {
    let k = 0.552_284_8 * r;
    vec![
        PathSeg::M(cx + r, cy),
        PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathSeg::Z,
    ]
}

fn rounded_rect(b: [f32; 4], r: f32) -> Vec<PathSeg> {
    let (x0, y0, x1, y1) = (b[0], b[1], b[0] + b[2], b[1] + b[3]);
    vec![
        PathSeg::M(x0 + r, y0),
        PathSeg::L(x1 - r, y0),
        PathSeg::Q(x1, y0, x1, y0 + r),
        PathSeg::L(x1, y1 - r),
        PathSeg::Q(x1, y1, x1 - r, y1),
        PathSeg::L(x0 + r, y1),
        PathSeg::Q(x0, y1, x0, y1 - r),
        PathSeg::L(x0, y0 + r),
        PathSeg::Q(x0, y0, x0 + r, y0),
        PathSeg::Z,
    ]
}

/// A dotted outline (`step` px on, `step` px off) on the edges of box `b` (x, y, w, h).
fn dotted(p: &mut dyn Painter, b: [f32; 4], c: Rgba, step: f32) {
    let (l, t, r, bo) = (
        b[0].round(),
        b[1].round(),
        (b[0] + b[2]).round(),
        (b[1] + b[3]).round(),
    );
    let mut x = l;
    while x < r {
        let w = step.min(r - x);
        p.fill_rect(x, t, w, 1.0, c);
        p.fill_rect(x, bo - 1.0, w, 1.0, c);
        x += 2.0 * step;
    }
    let mut y = t;
    while y < bo {
        let h = step.min(bo - y);
        p.fill_rect(l, y, 1.0, h, c);
        p.fill_rect(r - 1.0, y, 1.0, h, c);
        y += 2.0 * step;
    }
}

/// The largest box of proportions `size` (w, h) centred in `area` (x, y, w, h).
fn fit_box(area: [f32; 4], size: (f32, f32)) -> [f32; 4] {
    let (w, h) = (size.0.max(0.01), size.1.max(0.01));
    let k = (area[2] / w).min(area[3] / h);
    let (bw, bh) = (w * k, h * k);
    [
        area[0] + (area[2] - bw) / 2.0,
        area[1] + (area[3] - bh) / 2.0,
        bw,
        bh,
    ]
}

/// Draw a signature's ink in box `b` (x, y, w, h) at `alpha`, as it is placed on the page.
fn paint_ink(p: &mut dyn Painter, s: &SavedSig, b: [f32; 4], alpha: u8) {
    let [r, g, bl] = s.color;
    let c = Rgba(r, g, bl, alpha);
    match &s.ink {
        SignatureInk::Typed { text, face } => {
            // As placed: size 0.72 × the box height, 96 % of its width at most, centred.
            let em = bo_pdfread::typed_width(text, *face).max(0.1);
            let size = (b[3] * 0.72).min(b[2] * 0.96 / em);
            let x0 = b[0] + (b[2] - em * size) / 2.0;
            let base = b[1] + b[3] - (b[3] - size * 0.7) / 2.0;
            let segs: Vec<PathSeg> = bo_pdfread::typed_outline(text, *face)
                .into_iter()
                .map(|sg| match sg {
                    PathSeg::M(x, y) => PathSeg::M(x0 + x * size, base + y * size),
                    PathSeg::L(x, y) => PathSeg::L(x0 + x * size, base + y * size),
                    PathSeg::Q(a, bb, x, y) => PathSeg::Q(
                        x0 + a * size,
                        base + bb * size,
                        x0 + x * size,
                        base + y * size,
                    ),
                    PathSeg::C(a, bb, cc, d, x, y) => PathSeg::C(
                        x0 + a * size,
                        base + bb * size,
                        x0 + cc * size,
                        base + d * size,
                        x0 + x * size,
                        base + y * size,
                    ),
                    PathSeg::Z => PathSeg::Z,
                })
                .collect();
            if !segs.is_empty() {
                p.path(&segs, Some(c), None);
            }
        }
        SignatureInk::Strokes(st) => {
            let (u, _) = unit_strokes(st);
            let wd = (b[3] * 0.035).max(1.0);
            for v in &u {
                let segs: Vec<PathSeg> = v
                    .iter()
                    .enumerate()
                    .map(|(i, q)| {
                        let (x, y) = (b[0] + q[0] * b[2], b[1] + q[1] * b[3]);
                        if i == 0 {
                            PathSeg::M(x, y)
                        } else {
                            PathSeg::L(x, y)
                        }
                    })
                    .collect();
                p.path(&segs, None, Some((c, wd)));
            }
        }
        SignatureInk::Image(_) => {
            // A picture: its frame with a mountain mark (the page shows the picture itself).
            dotted(p, b, Rgba(120, 120, 120, alpha), 2.0);
            let (cx, cy, k) = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0, b[3].min(b[2]) / 4.0);
            p.path(
                &[
                    PathSeg::M(cx - 1.5 * k, cy + k),
                    PathSeg::L(cx - 0.4 * k, cy - 0.4 * k),
                    PathSeg::L(cx + 0.3 * k, cy + 0.4 * k),
                    PathSeg::L(cx + 0.8 * k, cy - 0.1 * k),
                    PathSeg::L(cx + 1.5 * k, cy + k),
                    PathSeg::Z,
                ],
                Some(Rgba(150, 150, 150, alpha)),
                None,
            );
        }
    }
}

#[cfg(test)]
#[path = "reader_fill11_tests.rs"]
mod tests;
