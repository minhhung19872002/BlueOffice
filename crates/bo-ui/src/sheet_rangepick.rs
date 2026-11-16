//! Range pickers of BlueSheet's dialogs (round 45): the button at the right of a reference
//! box collapses the dialog to a strip holding only that box (at the dialog's top-left
//! corner, the window not dimmed); cells clicked or dragged on the sheet are written into
//! it as an absolute reference; the expand button, Enter or Escape bring the dialog back
//! with the reference (Escape keeps the old text), as the reference spreadsheet's
//! collapsed dialogs do.
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Input, Key, Painter};
use bo_render::{PathSeg, Rgba};

/// How a picked range is written into a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RefStyle {
    /// `$A$1:$B$4`.
    Plain,
    /// `Sheet1!$A$1:$B$4`.
    Sheet,
    /// `=Sheet1!$A$1:$B$4` (Refers to boxes).
    Formula,
    /// `=$A$1:$B$4` (Data Validation sources).
    Eq,
}

/// A dialog waiting while cells are picked: the form, its kind, the box being filled, its
/// text before picking and how the reference is written.
pub(super) struct PickState {
    form: Form,
    kind: FormKind,
    ctl: &'static str,
    before: String,
    style: RefStyle,
}

/// Picker buttons: (form kind, button id, the box it fills, reference style).
const PICKERS: [(FormKind, &str, &str, RefStyle); 10] = [
    (FormKind::Names, "refPick", "ref", RefStyle::Formula),
    (FormKind::NewName, "refPick", "ref", RefStyle::Formula),
    (FormKind::TextToColumns, "pickDest", "dest", RefStyle::Plain),
    (
        FormKind::AdvancedFilter,
        "pickList",
        "list",
        RefStyle::Plain,
    ),
    (
        FormKind::AdvancedFilter,
        "pickCrit",
        "crit",
        RefStyle::Sheet,
    ),
    (
        FormKind::AdvancedFilter,
        "pickDest",
        "dest",
        RefStyle::Sheet,
    ),
    (FormKind::Validation, "pick1", "f1", RefStyle::Eq),
    (FormKind::Validation, "pick2", "f2", RefStyle::Eq),
    (FormKind::CreatePivot, "pickTable", "table", RefStyle::Sheet),
    (FormKind::Sparklines, "pickData", "data", RefStyle::Plain),
];

fn col_letters(mut c: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Sheet name as a reference writes it (quoted when it is not a plain word).
fn sheet_ref(name: &str) -> String {
    if name.chars().all(|c| c.is_alphanumeric() || c == '_') && !name.is_empty() {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

impl SheetWindow {
    /// The box picker button `id` of the open form fills, if it is one.
    pub(super) fn picker_target(&self, id: &str) -> Option<(&'static str, RefStyle)> {
        PICKERS
            .iter()
            .find(|(k, b, ..)| *k == self.form_kind && *b == id)
            .map(|(_, _, c, s)| (*c, *s))
    }

    /// Collapse the open form to box `ctl` and let the sheet take pointer input.
    pub(super) fn start_range_pick(&mut self, ctl: &'static str) -> bool {
        let style = PICKERS
            .iter()
            .find(|(k, _, c, _)| *k == self.form_kind && *c == ctl)
            .map_or(RefStyle::Plain, |p| p.3);
        let Some(form) = self.form.take() else {
            return false;
        };
        let before = form.text(ctl);
        let w = form.size.0;
        let mut mini = Form::new(&form.title, (w, 86.0), vec![]);
        mini.add(
            "ref",
            None,
            [10.0, 48.0, w - 50.0, 26.0],
            Kind::Edit {
                value: before.chars().collect(),
                caret: before.chars().count(),
            },
        );
        mini.add("expand", None, [w - 36.0, 48.0, 26.0, 26.0], Kind::Canvas);
        mini.focus("ref");
        mini.pos = Some(form.origin());
        self.pick_state = Some(PickState {
            form,
            kind: self.form_kind,
            ctl,
            before,
            style,
        });
        self.form = Some(mini);
        self.form_kind = FormKind::RangePick;
        true
    }

    /// Bring the dialog back; `keep` writes the picked reference into its box.
    pub(super) fn end_range_pick(&mut self, keep: bool) {
        let Some(st) = self.pick_state.take() else {
            return;
        };
        let text = self
            .form
            .as_ref()
            .map_or_else(|| st.before.clone(), |f| f.text("ref"));
        let mut form = st.form;
        form.set_text(st.ctl, if keep { &text } else { &st.before });
        form.focus(st.ctl);
        self.form = Some(form);
        self.form_kind = st.kind;
        // The wizard keeps its destination in its own state.
        if st.kind == FormKind::TextToColumns && keep {
            if let Some(t) = self.ttc.as_mut() {
                t.dest = text;
            }
        }
    }

    /// The selection written in the style of the box being filled.
    fn picked_text(&self, style: RefStyle) -> String {
        let (r0, c0, r1, c1) = {
            let v = self.view.rng();
            let n = |k: &str| v.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0) as u32;
            (n("r0"), n("c0"), n("r1"), n("c1"))
        };
        let mut s = format!("${}${}", col_letters(c0), r0 + 1);
        if (r0, c0) != (r1, c1) {
            s.push_str(&format!(":${}${}", col_letters(c1), r1 + 1));
        }
        let sheet = self
            .view
            .sheet_names()
            .get(self.view.sheet())
            .cloned()
            .flatten()
            .unwrap_or_default();
        match style {
            RefStyle::Plain => s,
            RefStyle::Sheet => format!("{}!{s}", sheet_ref(&sheet)),
            RefStyle::Formula => format!("={}!{s}", sheet_ref(&sheet)),
            RefStyle::Eq => format!("={s}"),
        }
    }

    /// Input while a dialog is collapsed: `None` when the strip's form handles it,
    /// `Some(None)` when the sheet should (pointer input on the grid), `Some(Some(redraw))`
    /// when it was handled here.
    #[allow(clippy::option_option)]
    pub(super) fn range_pick_input(&mut self, input: &Input) -> Option<Option<bool>> {
        let f = self.form.as_ref()?;
        let on_grid =
            |w: &Self, y: f32| y >= w.grid_top() && y < w.size.1 - w.status_h() - w.px(30.0);
        match input {
            Input::Press { x, y, .. } | Input::Move { x, y } if !f.contains(*x, *y) => {
                Some(if on_grid(self, *y) { None } else { Some(false) })
            }
            Input::Drag { .. } | Input::Release { .. } | Input::Wheel(_) => Some(None),
            Input::Key(Key::Enter, _) => {
                self.end_range_pick(true);
                Some(Some(true))
            }
            Input::Key(Key::Escape, _) => {
                self.end_range_pick(false);
                Some(Some(true))
            }
            _ => None,
        }
    }

    /// After the sheet took pointer input: write the selection into the strip's box.
    pub(super) fn range_pick_follow(&mut self) {
        let Some(style) = self.pick_state.as_ref().map(|s| s.style) else {
            return;
        };
        let text = self.picked_text(style);
        if let Some(f) = self.form.as_mut() {
            f.set_text("ref", &text);
        }
    }

    /// Events of the collapsed strip.
    pub(super) fn range_pick_event(&mut self, ev: &Event) {
        match ev {
            Event::Canvas("expand", ..) | Event::Ok => self.end_range_pick(true),
            Event::Cancel => self.end_range_pick(false),
            _ => {}
        }
    }

    /// The expand button of the strip (a box with an arrow down into it; own drawing).
    pub(super) fn paint_range_pick(f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        if id != "expand" {
            return;
        }
        let k = f.scale_k();
        let ink = Rgba(0, 0, 0, 255);
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x7a, 0x7a, 0x7a, 255));
        p.fill_rect(
            r[0],
            r[1] + 1.0,
            r[2] - 1.0,
            r[3] - 2.0,
            Rgba(255, 255, 255, 255),
        );
        let (cx, cy) = ((r[0] + r[2] / 2.0).round(), (r[1] + r[3] / 2.0).round());
        let a = (4.0 * k).round();
        p.fill_rect(
            cx - a,
            cy - a - (2.0 * k).round(),
            2.0 * a,
            (1.0 * k).round().max(1.0),
            ink,
        );
        p.fill_rect(cx - (1.0 * k).round(), cy - a, (2.0 * k).round(), a, ink);
        p.path(
            &[
                PathSeg::M(cx, cy + a + 1.0),
                PathSeg::L(cx + a, cy),
                PathSeg::L(cx - a, cy),
                PathSeg::Z,
            ],
            Some(ink),
            None,
        );
    }
}
