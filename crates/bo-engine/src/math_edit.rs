//! Equations edited in place in BlueDoc (`docs/parity/BACKLOG.md` CORE-MATH-010, ADR-0015).
//!
//! The document keeps an equation as one inline object (an Office Math zone, ECMA-376 Part 1,
//! §22.1.2.77 oMath / §22.1.2.78 oMathPara) at a (paragraph, byte offset) position. The *math caret*
//! adds a position inside it: the equation's start, a slot path and an offset in that slot
//! ([`bo_math::edit::MathPos`]). While it is set, the main selection stays collapsed just after the
//! equation (the equation commands keep reporting it) and the editing keys — typing, Backspace /
//! Delete, arrows, Home / End, Tab — act inside the equation. Every change rewrites the zone as one
//! reversible edit ([`bo_edit::Session::replace_equation`]), so undo / redo and collaboration see an
//! ordinary paragraph change. The caret, a selected structure and the empty slots are drawn from the
//! slot geometry of the laid-out zone ([`bo_layout::equation_geometry`]).

use crate::edit::{Command, EditResult};
use crate::nav::{self, CaretRect, Motion, SelRect};
use bo_edit::{Pos, Selection};
use bo_math::edit::{self as me, Deleted, MathPos, Step};
use bo_math::Node;
use serde::{Deserialize, Serialize};

/// Length in the index text of an equation (one object placeholder, U+FFFC).
const OBJECT_LEN: usize = '\u{FFFC}'.len_utf8();

/// The caret inside an equation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MathCaret {
    /// Start of the equation in the body.
    pub at: Pos,
    /// Position inside it.
    pub pos: MathPos,
    /// The structure just before `pos` is selected (the first Backspace / Delete on a structure).
    pub selected: bool,
}

/// Caret motions inside an equation (`mathMove`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MathMotion {
    /// One position left in reading order (leaves the zone before its start).
    Left,
    /// One position right (leaves the zone after its end).
    Right,
    /// Start of the zone.
    Home,
    /// End of the zone.
    End,
    /// Next slot (Tab).
    Next,
    /// Previous slot (Shift+Tab).
    Prev,
    /// Leave the zone, the caret after it (Esc).
    Exit,
}

/// The math caret as reported to the host (`math` of a result).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MathState {
    /// Slot path: `[structure index, slot index]` per level.
    pub path: Vec<[usize; 2]>,
    /// Offset in the slot.
    pub offset: usize,
    /// A structure is selected (the next Backspace / Delete removes it).
    pub selected: bool,
    /// Box of the whole zone on its page.
    pub zone: SelRect,
    /// Empty slots of the zone (drawn as placeholders).
    pub placeholders: Vec<SelRect>,
}

/// Error code of a refused `mathEnter` / click: the equation holds markup the in-place model cannot
/// keep (ADR-0015 addendum); it is selected instead, for the equation dialog.
pub const MATH_UNSUPPORTED: &str = "mathUnsupported";

/// What is around the tree of a zone: inline (`m:oMath`) or display (`m:oMathPara`), and the
/// display properties (`m:oMathParaPr`, ECMA-376 Part 1, §22.1.2.79) to keep.
#[derive(Debug, Clone)]
struct Zone {
    display: bool,
    para_pr: Option<String>,
}

impl Zone {
    fn of(xml: &str) -> Zone {
        Zone {
            display: crate::equation::is_display(xml),
            para_pr: bo_math::read::para_props(xml),
        }
    }

    /// The zone holding `nodes`.
    fn xml(&self, nodes: &[Node]) -> String {
        crate::equation::wrap_with(
            &bo_math::to_omml(nodes),
            self.display,
            self.para_pr.as_deref(),
        )
    }
}

/// The equation at `at`: its tree and its zone.
fn read(s: &bo_edit::Session, at: Pos) -> Option<(Vec<Node>, Zone)> {
    let xml = s.equation_at(at)?;
    Some((bo_math::from_omml(&xml)?, Zone::of(&xml)))
}

/// The zone is written back unchanged by the in-place model: reading it into the tree and writing
/// the tree again (as the first in-place edit does) gives the same markup (`bo_math::read::
/// same_omml`). The tree keeps run properties (`m:rPr`, `w:rPr`), structure properties (with
/// `m:ctrlPr`) and the display properties verbatim (ADR-0015 addendum C); zones with what it still
/// does not hold — several `m:oMath` in one `m:oMathPara`, tracked changes or bookmarks inside the
/// zone, argument properties (`m:argPr`), runs with other content than `m:t`, a hidden degree or
/// limit with content — are not.
#[must_use]
pub fn lossless(xml: &str) -> bool {
    let Some(mut nodes) = bo_math::from_omml(xml) else {
        return false;
    };
    me::normalize(&mut nodes);
    bo_math::read::same_omml(xml, &Zone::of(xml).xml(&nodes))
}

/// The equation at `at` can be edited in place (`Some(false)`: there is one, but not editable —
/// unsupported markup, or a tracked deletion).
fn editable(s: &bo_edit::Session, at: Pos) -> Option<bool> {
    let xml = s.equation_at(at)?;
    Some(!s.equation_deleted(at) && lossless(&xml))
}

impl crate::Engine {
    fn main_story(&self) -> bool {
        self.hf.is_none() && self.note.is_none() && self.textbox.is_none()
    }

    /// Editing commands while the caret is in an equation, and the ways into one (an arrow onto it,
    /// a click on it, `mathEnter`). `None`: run the command normally (after leaving the equation
    /// unless the command keeps it, e.g. undo).
    pub(crate) fn math_command(&mut self, cmd: &Command) -> crate::Result<Option<EditResult>> {
        // Read-only files and restricted editing edit equations as other text: not in place.
        let restricted = self.sec.read_only
            || self.sec.regions_enforced()
            || self.session.as_ref().is_none_or(|s| {
                crate::forms::gate(Command::InsertText { text: "x".into() }, s).is_err()
            });
        if restricted || !self.main_story() {
            self.math = None;
            return Ok(None);
        }
        // Typing just after a refusal does not replace the selected equation: it still cannot be
        // edited in place (the dialog edits it; Delete / Backspace remove it).
        if let (Some((at, version)), Some(s)) = (self.math_refused, self.session.as_ref()) {
            let range = Selection {
                anchor: at,
                focus: Pos::new(at.para, at.offset + OBJECT_LEN),
            };
            let still = s.version() == version && s.selection() == range;
            if !still {
                self.math_refused = None;
            } else if matches!(cmd, Command::InsertText { .. } | Command::MathInsert { .. }) {
                return Ok(Some(self.edit_result(
                    Vec::new(),
                    (None, None, None),
                    Some(MATH_UNSUPPORTED.into()),
                )?));
            }
        }
        // The equation may be gone (an edit elsewhere, another document).
        if let Some(m) = &self.math {
            let ok = self
                .session
                .as_ref()
                .is_some_and(|s| read(s, m.at).is_some() && !s.equation_deleted(m.at));
            if !ok {
                self.math = None;
            }
        }
        if self.math.is_none() {
            return self.math_entry(cmd);
        }
        let r = match cmd {
            Command::MathMove { motion } => self.math_move(*motion)?,
            Command::MathInsert { text } => self.math_type(text)?,
            Command::InsertText { text } if !text.contains(['\n', '\r', '\t']) => {
                self.math_type(text)?
            }
            Command::MathDelete { back } => self.math_delete(*back)?,
            Command::DeleteBackward => self.math_delete(true)?,
            Command::DeleteForward => self.math_delete(false)?,
            Command::MathAutobuild => self.math_build()?,
            Command::Tab { back } => self.math_move(if *back {
                MathMotion::Prev
            } else {
                MathMotion::Next
            })?,
            Command::Move {
                motion,
                extend: false,
            } => match motion {
                Motion::Left | Motion::WordLeft => self.math_move(MathMotion::Left)?,
                Motion::Right | Motion::WordRight => self.math_move(MathMotion::Right)?,
                Motion::LineStart => self.math_move(MathMotion::Home)?,
                Motion::LineEnd => self.math_move(MathMotion::End)?,
                _ => {
                    self.math_leave(true);
                    return Ok(None);
                }
            },
            Command::MathEnter { end } => {
                let at = self.math.as_ref().map(|m| m.at);
                if let (Some(at), Some(s)) = (at, self.session.as_ref()) {
                    if let Some((nodes, _)) = read(s, at) {
                        self.set_math(at, me::zone_end(&nodes, *end));
                    }
                }
                self.math_result(Vec::new())?
            }
            // Commands that keep the caret in the equation.
            Command::State
            | Command::Equation
            | Command::EquationTemplates
            | Command::HistoryLen
            | Command::Spelling { .. }
            | Command::Undo
            | Command::Redo => return Ok(None),
            Command::Click {
                page,
                x,
                y,
                extend: false,
                count: 1,
            } => {
                let (page, x, y) = (*page, *x, *y);
                self.math_leave(true);
                return self.math_click(page, x, y);
            }
            // A double click on an equation selects it (for the equation dialog).
            Command::Click {
                page,
                x,
                y,
                extend: false,
                count: 2,
            } => {
                let (page, x, y) = (*page, *x, *y);
                self.math_leave(true);
                return self.math_select_click(page, x, y);
            }
            _ => {
                self.math_leave(true);
                return Ok(None);
            }
        };
        Ok(Some(r))
    }

    /// Commands that put the caret into an equation when it is outside one.
    fn math_entry(&mut self, cmd: &Command) -> crate::Result<Option<EditResult>> {
        let Some(s) = self.session.as_ref() else {
            return Ok(None);
        };
        let sel = s.selection();
        match cmd {
            Command::MathEnter { end } => {
                let (a, b) = sel.ordered();
                // The equation after the caret (or the selected one), else the one before it.
                let found = [a, Pos::new(b.para, b.offset.saturating_sub(OBJECT_LEN))]
                    .into_iter()
                    .find_map(|at| read(s, at).map(|(n, _)| (at, n)));
                let Some((at, nodes)) = found else {
                    return Ok(None);
                };
                if editable(s, at) != Some(true) {
                    return self.math_refuse(at).map(Some);
                }
                self.set_math(at, me::zone_end(&nodes, *end));
                Ok(Some(self.math_result(Vec::new())?))
            }
            // An arrow onto an equation enters it (one that cannot be edited in place is passed
            // over as before).
            Command::Move {
                motion: Motion::Right,
                extend: false,
            } if sel.is_collapsed() => {
                let at = sel.focus;
                match read(s, at) {
                    Some((nodes, _)) if editable(s, at) == Some(true) => {
                        self.set_math(at, me::zone_end(&nodes, false));
                        Ok(Some(self.math_result(Vec::new())?))
                    }
                    _ => Ok(None),
                }
            }
            Command::Move {
                motion: Motion::Left,
                extend: false,
            } if sel.is_collapsed() && sel.focus.offset >= OBJECT_LEN => {
                let at = Pos::new(sel.focus.para, sel.focus.offset - OBJECT_LEN);
                match read(s, at) {
                    Some((nodes, _)) if editable(s, at) == Some(true) => {
                        self.set_math(at, me::zone_end(&nodes, true));
                        Ok(Some(self.math_result(Vec::new())?))
                    }
                    _ => Ok(None),
                }
            }
            Command::Click {
                page,
                x,
                y,
                extend: false,
                count: 1,
            } => self.math_click(*page, *x, *y),
            Command::Click {
                page,
                x,
                y,
                extend: false,
                count: 2,
            } => self.math_select_click(*page, *x, *y),
            Command::MathMove { .. }
            | Command::MathInsert { .. }
            | Command::MathDelete { .. }
            | Command::MathAutobuild => Ok(Some(self.query_result()?)),
            _ => Ok(None),
        }
    }

    fn set_math(&mut self, at: Pos, pos: MathPos) {
        self.math = Some(MathCaret {
            at,
            pos,
            selected: false,
        });
        if let Some(s) = self.session.as_mut() {
            s.end_math_group();
            // The main caret waits just after the equation.
            s.set_selection(Selection::caret(Pos::new(at.para, at.offset + OBJECT_LEN)));
        }
    }

    /// Leave the equation: the main caret after it (`after`) or before it.
    fn math_leave(&mut self, after: bool) {
        if let (Some(m), Some(s)) = (self.math.take(), self.session.as_mut()) {
            let off = if after {
                m.at.offset + OBJECT_LEN
            } else {
                m.at.offset
            };
            s.set_selection(Selection::caret(Pos::new(m.at.para, off)));
        }
    }

    /// A click: on an equation, the caret goes to the slot under the pointer; an equation that
    /// cannot be edited in place is selected instead, with [`MATH_UNSUPPORTED`].
    fn math_click(&mut self, page: usize, x: f32, y: f32) -> crate::Result<Option<EditResult>> {
        let Some((at, pos)) = self.equation_hit(page, x, y) else {
            return Ok(None);
        };
        if self.session.as_ref().and_then(|s| editable(s, at)) != Some(true) {
            return self.math_refuse(at).map(Some);
        }
        self.set_math(at, pos);
        Ok(Some(self.math_result(Vec::new())?))
    }

    /// A double click on an equation selects the whole equation.
    fn math_select_click(
        &mut self,
        page: usize,
        x: f32,
        y: f32,
    ) -> crate::Result<Option<EditResult>> {
        let Some((at, _)) = self.equation_hit(page, x, y) else {
            return Ok(None);
        };
        self.select_equation(at);
        Ok(Some(self.edit_result(
            Vec::new(),
            (None, None, None),
            None,
        )?))
    }

    /// The equation under a point and the position in it under the point.
    fn equation_hit(&mut self, page: usize, x: f32, y: f32) -> Option<(Pos, MathPos)> {
        let layout = self.layout.document();
        let p = nav::hit_test(layout, page, x, y)?;
        // The nearest caret stop to a point on an equation is its start or its end.
        let candidates = [p, Pos::new(p.para, p.offset.saturating_sub(OBJECT_LEN))];
        for at in candidates {
            let Some((nodes, display)) = self.session.as_ref().and_then(|s| read(s, at)) else {
                continue;
            };
            let Some((gpage, ox, base)) = nav::object_origin(self.layout.document(), at) else {
                continue;
            };
            let Some(geo) = self.math_geometry(at, &nodes, &display) else {
                continue;
            };
            let (lx, ly) = (x - ox, y - base);
            if gpage != page || lx < 0.0 || lx > geo.w || ly < -geo.asc || ly > geo.desc {
                continue;
            }
            return Some((at, hit_slot(&geo, &nodes, lx, ly)));
        }
        None
    }

    fn select_equation(&mut self, at: Pos) {
        self.math = None;
        if let Some(s) = self.session.as_mut() {
            s.end_math_group();
            s.set_selection(Selection {
                anchor: at,
                focus: Pos::new(at.para, at.offset + OBJECT_LEN),
            });
        }
    }

    /// In-place editing refused (ADR-0015 addendum): the equation is selected as before in-place
    /// editing existed (the equation dialog edits it) and [`MATH_UNSUPPORTED`] tells the host why.
    fn math_refuse(&mut self, at: Pos) -> crate::Result<EditResult> {
        self.select_equation(at);
        self.math_refused = self.session.as_ref().map(|s| (at, s.version()));
        self.edit_result(
            Vec::new(),
            (None, None, None),
            Some(MATH_UNSUPPORTED.into()),
        )
    }

    /// Slot geometry of the equation at `at` as laid out from its tree.
    fn math_geometry(
        &mut self,
        at: Pos,
        nodes: &[Node],
        display: &Zone,
    ) -> Option<bo_layout::EquationGeometry> {
        let s = self.session.as_ref()?;
        let doc = s.doc();
        let paths = bo_model::index::body_paragraphs(doc);
        let para = bo_model::index::para_at(doc, paths.get(at.para)?)?;
        let xml = display.xml(nodes);
        bo_layout::equation_geometry(doc, &mut self.fonts, para, at.offset, &xml)
    }

    fn math_move(&mut self, motion: MathMotion) -> crate::Result<EditResult> {
        let Some(m) = self.math.clone() else {
            return self.query_result();
        };
        let Some((nodes, _)) = self.session.as_ref().and_then(|s| read(s, m.at)) else {
            return self.query_result();
        };
        let pos = me::clamp(&nodes, &m.pos);
        let next = match motion {
            MathMotion::Left => me::left(&nodes, &pos),
            MathMotion::Right => me::right(&nodes, &pos),
            MathMotion::Home => Some(me::zone_end(&nodes, false)),
            MathMotion::End => Some(me::zone_end(&nodes, true)),
            MathMotion::Next => Some(me::tab(&nodes, &pos, false)),
            MathMotion::Prev => Some(me::tab(&nodes, &pos, true)),
            MathMotion::Exit => None,
        };
        match next {
            Some(p) => self.set_math(m.at, p),
            None => self.math_leave(motion != MathMotion::Left),
        }
        self.math_result(Vec::new())
    }

    /// Write the edited tree as the equation and lay out again.
    fn math_write(
        &mut self,
        mut nodes: Vec<Node>,
        display: &Zone,
        pos: &MathPos,
        group: bool,
    ) -> crate::Result<EditResult> {
        let Some(m) = self.math.clone() else {
            return self.query_result();
        };
        me::normalize(&mut nodes);
        let xml = display.xml(&nodes);
        let res = self
            .session
            .as_mut()
            .ok_or(crate::EngineError::NoDocument)?
            .replace_equation(m.at, &xml, group);
        // With track changes the edited equation follows the deleted original.
        let (at, error) = match res {
            Ok(at) => (at, None),
            Err(e) => (m.at, Some(e.to_string())),
        };
        // The caret in the tree as it reads back.
        let back = bo_math::from_omml(&xml).unwrap_or_default();
        self.math = Some(MathCaret {
            at,
            pos: me::clamp(&back, pos),
            selected: false,
        });
        let dirty = self.relayout()?;
        let mut r = self.math_result_with(dirty, error)?;
        r.collab = self.collab_after_edit();
        if let Some(s) = self.session.as_ref() {
            r.review = Some(crate::review::review_state(s, &self.markup));
        }
        Ok(r)
    }

    fn math_type(&mut self, text: &str) -> crate::Result<EditResult> {
        let Some(m) = self.math.clone() else {
            return self.query_result();
        };
        let Some((mut nodes, display)) = self.session.as_ref().and_then(|s| read(s, m.at)) else {
            return self.query_result();
        };
        let mut pos = me::clamp(&nodes, &m.pos);
        // Typing replaces a selected structure.
        if m.selected {
            if let Some(p) = me::delete_selected(&mut nodes, &pos) {
                pos = p;
            }
        }
        let single = text.chars().count() == 1;
        if text == " " {
            // A space builds up the linear text before it; nothing to build: a space.
            if let Some(p) = me::autobuild(&mut nodes, &pos) {
                if let Some(s) = self.session.as_mut() {
                    s.end_math_group();
                }
                return self.math_write(nodes, &display, &p, false);
            }
        }
        let Some(p) = me::insert_text(&mut nodes, &pos, text) else {
            return self.query_result();
        };
        let group = single && text != " " && !m.selected;
        self.math_write(nodes, &display, &p, group)
    }

    fn math_build(&mut self) -> crate::Result<EditResult> {
        let Some(m) = self.math.clone() else {
            return self.query_result();
        };
        let Some((mut nodes, display)) = self.session.as_ref().and_then(|s| read(s, m.at)) else {
            return self.query_result();
        };
        let pos = me::clamp(&nodes, &m.pos);
        match me::autobuild(&mut nodes, &pos) {
            Some(p) => self.math_write(nodes, &display, &p, false),
            None => self.math_result(Vec::new()),
        }
    }

    fn math_delete(&mut self, back: bool) -> crate::Result<EditResult> {
        let Some(m) = self.math.clone() else {
            return self.query_result();
        };
        let Some((mut nodes, display)) = self.session.as_ref().and_then(|s| read(s, m.at)) else {
            return self.query_result();
        };
        let pos = me::clamp(&nodes, &m.pos);
        if m.selected {
            return match me::delete_selected(&mut nodes, &pos) {
                Some(p) => self.math_write(nodes, &display, &p, false),
                None => self.math_result(Vec::new()),
            };
        }
        match me::delete(&mut nodes, &pos, back) {
            Some(Deleted::Char(p)) => self.math_write(nodes, &display, &p, true),
            Some(Deleted::Select(p)) => {
                self.set_math(m.at, p);
                if let Some(c) = self.math.as_mut() {
                    c.selected = true;
                }
                self.math_result(Vec::new())
            }
            Some(Deleted::Outside) => {
                // Backspace at the start / Delete at the end of the zone: leave it.
                self.math_leave(!back);
                self.math_result(Vec::new())
            }
            None => self.math_result(Vec::new()),
        }
    }

    fn math_result(&mut self, dirty: Vec<usize>) -> crate::Result<EditResult> {
        self.math_result_with(dirty, None)
    }

    fn math_result_with(
        &mut self,
        dirty: Vec<usize>,
        error: Option<String>,
    ) -> crate::Result<EditResult> {
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        self.math_decorate(&mut r);
        Ok(r)
    }

    /// The math caret drawn into a result: caret, selected structure, zone and placeholders; the
    /// caret is kept valid after undo / redo changed the equation under it.
    pub(crate) fn math_decorate(&mut self, r: &mut EditResult) {
        let Some(m) = self.math.clone() else {
            return;
        };
        let Some((nodes, display)) = self.session.as_ref().and_then(|s| read(s, m.at)) else {
            self.math = None;
            return;
        };
        let pos = me::clamp(&nodes, &m.pos);
        let selected = m.selected && me::structure_before(&nodes, &pos).is_some();
        if pos != m.pos || selected != m.selected {
            self.math = Some(MathCaret {
                at: m.at,
                pos: pos.clone(),
                selected,
            });
        }
        let Some((page, ox, base)) = nav::object_origin(self.layout.document(), m.at) else {
            return;
        };
        let Some(geo) = self.math_geometry(m.at, &nodes, &display) else {
            return;
        };
        let Some(slot) = me::slot_index(&nodes, &pos.path).and_then(|i| geo.slots.get(i)) else {
            return;
        };
        let x = slot
            .stops
            .get(pos.offset)
            .or(slot.stops.last())
            .copied()
            .unwrap_or(slot.x);
        let rect = |x0: f32, x1: f32, t: f32, b: f32| SelRect {
            page,
            x: ox + x0,
            y: base + t,
            w: (x1 - x0).max(0.5),
            h: (b - t).max(0.5),
        };
        r.caret = Some(CaretRect {
            page,
            x: ox + x,
            y: base + slot.top,
            h: slot.bottom - slot.top,
        });
        r.rects = if selected {
            me::structure_before(&nodes, &pos)
                .and_then(|i| slot.items.get(i))
                .map(|b| vec![rect(b[0], b[1], b[2], b[3])])
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let placeholders = geo
            .slots
            .iter()
            .filter(|s| s.stops.len() == 1)
            .map(|s| rect(s.x, s.x + s.w, s.top, s.bottom))
            .collect();
        r.math = Some(MathState {
            path: pos.path.iter().map(|s| [s.item, s.slot]).collect(),
            offset: pos.offset,
            selected,
            zone: rect(0.0, geo.w, -geo.asc, geo.desc),
            placeholders,
        });
    }
}

/// The position under a point of an equation (coordinates from its origin): the smallest slot around
/// the point (the innermost), its nearest caret stop.
fn hit_slot(geo: &bo_layout::EquationGeometry, nodes: &[Node], x: f32, y: f32) -> MathPos {
    let paths = me::all_slots(nodes);
    let mut best: Option<(usize, f32)> = None;
    for (i, s) in geo.slots.iter().enumerate() {
        let inside = x >= s.x - 0.5 && x <= s.x + s.w + 0.5 && y >= s.top && y <= s.bottom;
        let area = s.w.max(0.5) * (s.bottom - s.top);
        if inside && best.is_none_or(|b| area < b.1) {
            best = Some((i, area));
        }
    }
    let id = best.map_or(0, |b| b.0);
    let Some(s) = geo.slots.get(id) else {
        return MathPos::default();
    };
    let offset = s
        .stops
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
        .map_or(0, |(k, _)| k);
    let path: Vec<Step> = paths.get(id).cloned().unwrap_or_default();
    MathPos::new(path, offset)
}

#[cfg(test)]
#[path = "math_edit_tests.rs"]
mod tests;
