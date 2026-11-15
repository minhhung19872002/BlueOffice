//! Insert tab, Illustrations / Filters / Symbols groups of BlueSheet (round 6): the Shapes and
//! Icons galleries (the picker BlueSlide uses, with the shared icon library of our own
//! drawings), SmartArt by layout family, Timeline and Equation, as on the reference's
//! Insert tab.
use super::SheetWindow;
use crate::ribbon::RibbonHit;
use crate::slide_pickers::{self as pk, PickKind, Picker};
use crate::{Input, Key, Painter};
use bo_render::Rgba;
use serde_json::{json, Value};

/// Built-in equations of the Equation menu: label key, fallback, linear text.
pub(crate) const EQUATIONS: [(&str, &str, &str); 5] = [
    ("eqCircle", "Area of Circle", "A = πr²"),
    (
        "eqBinomial",
        "Binomial Theorem",
        "(x + a)ⁿ = ∑ₖ₌₀ⁿ (ⁿₖ) xᵏ aⁿ⁻ᵏ",
    ),
    ("eqPythagoras", "Pythagorean Theorem", "a² + b² = c²"),
    (
        "eqQuadratic",
        "Quadratic Formula",
        "x = (−b ± √(b² − 4ac)) / 2a",
    ),
    ("eqNew", "Insert New Equation", "Type equation here."),
];

/// SmartArt layout families of the engine's library (`insertSmartArt` layout).
pub(crate) const SMARTART: [&str; 7] = [
    "list",
    "process",
    "cycle",
    "hierarchy",
    "relationship",
    "matrix",
    "pyramid",
];

/// Office default theme accents 1–6, dark 1, light 1 (the shapes gallery's colours).
const ACCENTS: [u32; 8] = [
    0x44_72C4, 0xED_7D31, 0xA5_A5A5, 0xFF_C000, 0x5B_9BD5, 0x70_AD47, 0x00_0000, 0xFF_FFFF,
];

impl SheetWindow {
    /// Illustrations / Filters / Symbols commands; `None` for other ids.
    pub(super) fn illus_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            "sheet.shapes" => self.open_picker(PickKind::Shapes, "sheet.shapes"),
            "sheet.icons" => self.open_picker(PickKind::Icons, "sheet.icons"),
            "sheet.timeline" => self.insert_timeline(),
            _ if id.starts_with("smartart.") => self.insert_smartart(&id["smartart.".len()..]),
            _ if id.starts_with("equation.") => self.insert_equation(&id["equation.".len()..]),
            _ => return None,
        })
    }

    fn open_picker(&mut self, kind: PickKind, anchor: &'static str) -> bool {
        let Some(r) = self.ribbon.rect_of(RibbonHit::Command(anchor)) else {
            return false;
        };
        let labels: Vec<String> = match kind {
            PickKind::Shapes => pk::SHAPE_SECTIONS
                .iter()
                .map(|(k, _)| self.strings.get(k).to_owned())
                .collect(),
            _ => vec![self.t("sheetInsert.icons", "Icons")],
        };
        if kind == PickKind::Icons && self.library.is_empty() {
            self.library = pk::load_library();
        }
        let accents: Vec<Rgba> = ACCENTS.iter().map(|v| pk::rgb(*v)).collect();
        self.menu = None;
        self.picker = Some(Picker::open(
            kind,
            r,
            self.size,
            self.scale,
            &labels,
            &accents,
            &self.library,
        ));
        true
    }

    /// Input while a picker is open: `Some(redraw)` when it took the input.
    pub(super) fn picker_input(&mut self, input: &Input) -> Option<bool> {
        let p = self.picker.as_mut()?;
        Some(match *input {
            Input::Move { x, y } | Input::Drag { x, y } => p.hover(x, y),
            Input::Wheel(dy) => p.wheel(dy),
            Input::Press { x, y, .. } => {
                let v = p.press(x, y);
                let inside = p.contains(x, y);
                if v.is_some() || !inside {
                    self.picker = None;
                    self.ribbon.clear_pointer();
                }
                if let Some(v) = v {
                    self.pick_drawing(&v);
                }
                true
            }
            Input::Key(Key::Escape, _) => {
                self.picker = None;
                true
            }
            Input::Release { .. } | Input::Text(_) | Input::Key(..) => true,
            _ => return None,
        })
    }

    pub(super) fn paint_picker(&self, p: &mut dyn Painter) {
        if let Some(pick) = &self.picker {
            pick.paint(p, &self.theme, &self.library);
        }
    }

    /// Anchor cells of a new drawing at the active cell (`rows` × `cols` cells).
    fn drawing_cells(&self, rows: u32, cols: u32) -> ([u32; 2], [u32; 2]) {
        let (r, c) = self.view.active();
        ([r, c], [r + rows, c + cols])
    }

    /// Insert what a picker cell offers (`shape:<preset>`, `icon:<id>`).
    pub(super) fn pick_drawing(&mut self, v: &str) -> bool {
        let Some((kind, value)) = v.split_once(':') else {
            return false;
        };
        match kind {
            "shape" => {
                let (from, to) = self.drawing_cells(6, 3);
                self.view.run(json!({
                    "cmd": "insertDrawing", "from": from, "to": to, "shape": value,
                }))
            }
            "icon" => {
                use base64::Engine as _;
                let Some(icon) = self.library.iter().find(|i| i.id == value) else {
                    return false;
                };
                // The reference inserts an icon about 1 inch square: 5 rows × 2 columns.
                let data = base64::engine::general_purpose::STANDARD.encode(icon.svg.as_bytes());
                let (from, to) = self.drawing_cells(5, 2);
                self.view.run(json!({
                    "cmd": "insertDrawing", "from": from, "to": to, "image": data,
                }))
            }
            _ => false,
        }
    }

    /// SmartArt of a layout family with three placeholder nodes, over 15 rows × 8 columns.
    fn insert_smartart(&mut self, layout: &str) -> bool {
        let text = self.t("sheetInsert.smartText", "[Text]");
        let (from, to) = self.drawing_cells(15, 8);
        let items: Vec<Value> = (0..3)
            .map(|_| json!({ "text": text, "level": 0 }))
            .collect();
        // SmartArt commands are outside `SheetCommand`: sent as JSON, then marked changed.
        let done = self
            .view
            .query(json!({
                "cmd": "insertSmartArt", "layout": layout, "items": items, "from": from, "to": to,
            }))
            .is_some();
        self.view.changed();
        done
    }

    /// An equation as a text box with its linear form (the engine's sheets have no math zones).
    fn insert_equation(&mut self, key: &str) -> bool {
        let Some((_, _, text)) = EQUATIONS.iter().find(|(k, _, _)| *k == key) else {
            return false;
        };
        let (from, to) = self.drawing_cells(2, 4);
        self.view.run(json!({
            "cmd": "insertDrawing", "from": from, "to": to, "shape": "rect", "text": text,
        }))
    }

    /// Timeline over the first date field of the sheet's first pivot table; without one, the
    /// status bar says what is needed (the reference asks for a PivotTable with a date field).
    fn insert_timeline(&mut self) -> bool {
        let sheet = self.view.sheet();
        for field in 0..32u32 {
            if self.view.run(json!({
                "cmd": "insertTimeline", "pivot": 0, "field": field, "pivotSheet": sheet,
            })) {
                return true;
            }
            let _ = self.view.take_error();
        }
        self.status = self.t(
            "sheetInsert.timelineNeedsPivot",
            "Select a PivotTable with a date field to insert a timeline.",
        );
        true
    }
}
