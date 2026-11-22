//! BlueDoc dialogs of parity round 12 (docs/ui/REPORT.md "Vòng 102"), laid out from the
//! reference word processor's dialogs captured at 125 % (window less its 8 px invisible
//! border): Table Positioning (ECMA-376 Part 1, §17.4.58 tblpPr, §17.4.56 tblOverlap) and
//! Borders and Shading with its Borders, Page Border and Shading tabs (§17.3.1.24 pBdr,
//! §17.3.2.4 bdr, §17.4.39 tblBorders, §17.4.67 tcBorders, §17.6.10 pgBorders, §17.3.1.31 /
//! §17.3.2.32 / §17.4.33 shd). Pictures (setting icons, edge buttons, preview) are own
//! drawings. A child module of `doc_forms`.
use super::tprops11::{len, parse};
use super::{t, DocForm, DocFormKind, FormContext, FormOut};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};

/// Line styles of the Style list (§17.18.2 ST_Border), in the reference's order.
pub(crate) const LINE_STYLES: [&str; 22] = [
    "single",
    "dotted",
    "dashSmallGap",
    "dashed",
    "dotDash",
    "dotDotDash",
    "double",
    "triple",
    "thinThickSmallGap",
    "thickThinSmallGap",
    "thinThickThinSmallGap",
    "thinThickMediumGap",
    "thickThinMediumGap",
    "thinThickThinMediumGap",
    "thinThickLargeGap",
    "thickThinLargeGap",
    "wave",
    "doubleWave",
    "dashDotStroked",
    "threeDEmboss",
    "threeDEngrave",
    "outset",
];

/// Shading patterns of the Shading tab in the reference's order: `w:val` (§17.18.78
/// `ST_Shd`) and the name (a key of `doc13` for the named ones, else the percentage).
pub(crate) const PATTERNS: [(&str, &str); 37] = [
    ("clear", "doc12.clear"),
    ("solid", "doc12.solid"),
    ("pct5", "5%"),
    ("pct10", "10%"),
    ("pct12", "12.5%"),
    ("pct15", "15%"),
    ("pct20", "20%"),
    ("pct25", "25%"),
    ("pct30", "30%"),
    ("pct35", "35%"),
    ("pct37", "37.5%"),
    ("pct40", "40%"),
    ("pct45", "45%"),
    ("pct50", "50%"),
    ("pct55", "55%"),
    ("pct60", "60%"),
    ("pct62", "62.5%"),
    ("pct65", "65%"),
    ("pct70", "70%"),
    ("pct75", "75%"),
    ("pct80", "80%"),
    ("pct85", "85%"),
    ("pct87", "87.5%"),
    ("pct90", "90%"),
    ("pct95", "95%"),
    ("horzStripe", "doc13.pat_dkHorz"),
    ("vertStripe", "doc13.pat_dkVert"),
    ("reverseDiagStripe", "doc13.pat_dkDown"),
    ("diagStripe", "doc13.pat_dkUp"),
    ("horzCross", "doc13.pat_dkGrid"),
    ("diagCross", "doc13.pat_dkTrellis"),
    ("thinHorzStripe", "doc13.pat_ltHorz"),
    ("thinVertStripe", "doc13.pat_ltVert"),
    ("thinReverseDiagStripe", "doc13.pat_ltDown"),
    ("thinDiagStripe", "doc13.pat_ltUp"),
    ("thinHorzCross", "doc13.pat_ltGrid"),
    ("thinDiagCross", "doc13.pat_ltTrellis"),
];

/// Widths offered (eighths of a point) and their names.
const WIDTHS: [(u32, &str); 9] = [
    (2, "¼ pt"),
    (4, "½ pt"),
    (6, "¾ pt"),
    (8, "1 pt"),
    (12, "1 ½ pt"),
    (18, "2 ¼ pt"),
    (24, "3 pt"),
    (36, "4 ½ pt"),
    (48, "6 pt"),
];

/// Rows of the Style list (reference pixels).
const STYLE_ROW: f32 = 33.0;
/// Visible rows of the Style list.
const STYLE_ROWS: usize = 5;

/// Edges: top, left, bottom, right, inside horizontal (between paragraphs), inside vertical,
/// and the cell diagonals tl2br (§17.4.80) and tr2bl (§17.4.79).
const EDGES: usize = 8;

/// One edge's line: style (index into [`LINE_STYLES`]), width (eighths of a point), colour
/// (`""` automatic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    style: usize,
    size: u32,
    color: String,
}

impl Line {
    fn json(&self) -> Value {
        let color = (!self.color.is_empty() && self.color != super::form::NO_COLOR)
            .then(|| self.color.clone());
        json!({ "style": LINE_STYLES[self.style], "size": self.size, "color": color })
    }
}

/// State of the Borders and Shading dialog.
#[derive(Debug, Clone, Default)]
pub(crate) struct Bs {
    /// Apply-to choices of the Borders and Shading tabs: `text`, `paragraph`, `cell`, `table`.
    scopes: Vec<&'static str>,
    /// Edges of the Borders tab (0) and the Page Border tab (1).
    edges: [[Option<Line>; EDGES]; 2],
    /// Setting chosen (0 none, 1 box, 2 shadow / all, 3 3-D / grid, 4 custom).
    setting: [usize; 2],
    /// Style row chosen and the list's first visible row.
    style: [usize; 2],
    top: [usize; 2],
    /// Distances from the text of paragraph borders (points): top, left, bottom, right.
    space: [u32; 4],
    /// The selection spans paragraphs (the line between them is offered).
    multi_para: bool,
    /// The selection covers several cells (one cell is set like a paragraph, as the
    /// reference: Shadow / 3-D and its four edges).
    multi_cell: bool,
}

fn style_index(s: &str) -> usize {
    LINE_STYLES.iter().position(|x| *x == s).unwrap_or(0)
}

fn width_index(size: u32) -> usize {
    WIDTHS
        .iter()
        .position(|w| w.0 >= size)
        .unwrap_or(WIDTHS.len() - 1)
}

/// A line of the `extra` JSON (`{style, size, color}`), `None` for no line / `nil`.
fn line_of(v: &Value) -> Option<Line> {
    let style = v["style"].as_str()?;
    if matches!(style, "nil" | "none") {
        return None;
    }
    Some(Line {
        style: style_index(style),
        size: v["size"].as_u64().map_or(4, |s| s as u32),
        color: v["color"].as_str().unwrap_or("").to_owned(),
    })
}

/// The setting the edges make: None, Box (four equal edges), All (every edge equal, cells and
/// tables), Grid (equal outer edges, ½ pt single inside), else Custom.
fn setting_of(e: &[Option<Line>; EDGES], table: bool) -> usize {
    if e.iter().all(Option::is_none) {
        return 0;
    }
    if e[6].is_some() || e[7].is_some() {
        return 4;
    }
    let outer = &e[0];
    let box_ = outer.is_some() && e[..4].iter().all(|x| x == outer);
    if !box_ {
        return 4;
    }
    if e[4].is_none() && e[5].is_none() {
        return 1;
    }
    if table && e[4] == *outer && e[5] == *outer {
        return 2;
    }
    let thin = |x: &Option<Line>| x.as_ref().is_some_and(|l| l.style == 0 && l.size == 4);
    if table && thin(&e[4]) && thin(&e[5]) {
        return 3;
    }
    4
}

impl DocForm {
    /// The scope has the table settings (All / Grid and inside lines).
    fn bs_tabular(&self, scope: &str) -> bool {
        scope == "table" || (scope == "cell" && self.bs().multi_cell)
    }

    fn bs(&self) -> &Bs {
        static EMPTY: std::sync::OnceLock<Bs> = std::sync::OnceLock::new();
        self.bs
            .as_deref()
            .unwrap_or_else(|| EMPTY.get_or_init(Bs::default))
    }

    fn bs_mut(&mut self) -> &mut Bs {
        self.bs.get_or_insert_with(Box::default)
    }

    /// The scope of the Borders tab.
    fn bs_scope(&self) -> &'static str {
        let i = self.sel("bApply").unwrap_or(0);
        self.bs().scopes.get(i).copied().unwrap_or("paragraph")
    }

    /// The pen of tab `tab` (0 Borders, 1 Page Border) from its style, colour and width.
    fn bs_pen(&self, tab: usize) -> Line {
        let (w, c) = if tab == 0 {
            ("bWidth", "bColor")
        } else {
            ("pWidth", "pColor")
        };
        Line {
            style: self.bs().style[tab],
            size: WIDTHS[self.sel(w).unwrap_or(1).min(WIDTHS.len() - 1)].0,
            color: self.form.color(c),
        }
    }

    /// Edges of setting `s` with the pen (None, Box, Shadow / All, 3-D / Grid; Custom keeps).
    fn bs_preset(&mut self, tab: usize, s: usize) {
        let pen = self.bs_pen(tab);
        let grid = tab == 0 && self.bs_tabular(self.bs_scope());
        let thin = Line {
            style: 0,
            size: 4,
            color: pen.color.clone(),
        };
        let e = &mut self.bs_mut().edges[tab];
        match s {
            0 => *e = Default::default(),
            1..=3 => {
                for edge in e.iter_mut().take(4) {
                    *edge = Some(pen.clone());
                }
                let inner = match (grid, s) {
                    (true, 2) => Some(pen.clone()),
                    (true, 3) => Some(thin),
                    _ => None,
                };
                e[4].clone_from(&inner);
                e[5] = if grid { inner } else { None };
                e[6] = None;
                e[7] = None;
            }
            _ => {}
        }
        self.bs_mut().setting[tab] = s;
    }

    /// The style, colour or width of tab `tab` changed: a preset takes the new pen; with no
    /// border yet the setting becomes Box (as the reference).
    fn bs_pen_changed(&mut self, tab: usize) {
        match self.bs().setting[tab] {
            0 => self.bs_preset(tab, 1),
            s @ 1..=3 => self.bs_preset(tab, s),
            _ => {}
        }
    }

    /// Toggle edge `i` of tab `tab` with the pen; the setting becomes Custom.
    fn bs_toggle(&mut self, tab: usize, i: usize) {
        let pen = self.bs_pen(tab);
        let b = self.bs_mut();
        b.edges[tab][i] = if b.edges[tab][i].as_ref() == Some(&pen) {
            None
        } else {
            Some(pen)
        };
        b.setting[tab] = if b.edges[tab].iter().all(Option::is_none) {
            0
        } else {
            4
        };
    }

    /// Edges the preview offers buttons for on tab `tab` (indices into the edge array).
    fn bs_edges_offered(&self, tab: usize) -> Vec<usize> {
        if tab == 1 {
            return vec![0, 2, 1, 3];
        }
        match self.bs_scope() {
            "text" => Vec::new(),
            "paragraph" if self.bs().multi_para => vec![0, 4, 2, 1, 3],
            "paragraph" => vec![0, 2, 1, 3],
            // Diagonals for cells (the selected cells' tl2br / tr2bl).
            "cell" if self.bs().multi_cell => vec![0, 4, 2, 1, 5, 3, 7, 6],
            "cell" => vec![0, 2, 1, 3, 7, 6],
            _ => vec![0, 4, 2, 1, 5, 3],
        }
    }

    /// Edge buttons of the preview canvas (local reference pixels): edge index and rectangle.
    fn bs_buttons(&self, tab: usize) -> Vec<(usize, [f32; 4])> {
        let offered = self.bs_edges_offered(tab);
        let left: Vec<usize> = offered
            .iter()
            .copied()
            .filter(|e| matches!(e, 0 | 2 | 4))
            .collect();
        let below: Vec<usize> = offered
            .iter()
            .copied()
            .filter(|e| matches!(e, 1 | 3 | 5))
            .collect();
        let mut out = Vec::new();
        // Left column beside the diagram (56…211 local), bottom row under it (76…270).
        let ys: &[f32] = if left.len() == 3 {
            &[88.0, 120.0, 151.0]
        } else {
            &[88.0, 151.0]
        };
        let mut sorted_left = left;
        sorted_left.sort_by_key(|e| match e {
            0 => 0,
            4 => 1,
            _ => 2,
        });
        for (k, e) in sorted_left.into_iter().enumerate() {
            out.push((e, [14.0, ys[k.min(ys.len() - 1)], 27.0, 27.0]));
        }
        let xs: &[f32] = if below.len() == 3 {
            &[116.0, 159.0, 202.0]
        } else {
            &[116.0, 202.0]
        };
        let mut sorted_below = below;
        sorted_below.sort_by_key(|e| match e {
            1 => 0,
            5 => 1,
            _ => 2,
        });
        for (k, e) in sorted_below.into_iter().enumerate() {
            out.push((e, [xs[k.min(xs.len() - 1)], 231.0, 27.0, 27.0]));
        }
        // The diagonals at the row's ends (measured: 16 and 304).
        if offered.contains(&7) {
            out.push((7, [16.0, 231.0, 27.0, 27.0]));
        }
        if offered.contains(&6) {
            out.push((6, [304.0, 231.0, 27.0, 27.0]));
        }
        out
    }

    /// A click in a round-12 canvas: `true` when handled.
    pub(super) fn r12_canvas(&mut self, id: &str, x: f32, y: f32) -> bool {
        if self.kind != DocFormKind::BordersShading {
            return false;
        }
        let tab = usize::from(id.starts_with('p'));
        if let Some(n) = id
            .strip_prefix("bSet")
            .or_else(|| id.strip_prefix("pSet"))
            .and_then(|n| n.parse::<usize>().ok())
        {
            if tab == 0 && self.bs_scope() == "text" && n == 4 {
                return true;
            }
            self.bs_preset(tab, n);
            return true;
        }
        if matches!(id, "bStyle" | "pStyle") {
            let first = self.bs().top[tab];
            let w = self.form.get(id).map_or(201.0, |c| c.rect[2]);
            if x > w - 18.0 {
                // The list's scroll arrows.
                let up = y < 87.0;
                let max = LINE_STYLES.len() - STYLE_ROWS;
                let b = self.bs_mut();
                b.top[tab] = if up {
                    first.saturating_sub(1)
                } else {
                    (first + 1).min(max)
                };
                return true;
            }
            let row = first + ((y - 2.0) / STYLE_ROW).floor().max(0.0) as usize;
            if row < LINE_STYLES.len() {
                self.bs_mut().style[tab] = row;
                self.bs_pen_changed(tab);
            }
            return true;
        }
        if matches!(id, "bPreview" | "pPreview") {
            if let Some((e, _)) = self
                .bs_buttons(tab)
                .into_iter()
                .find(|(_, r)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            {
                self.bs_toggle(tab, e);
                return true;
            }
            // A click on the diagram's edge toggles that edge.
            let (dx, dy, dw, dh) = (76.0, 56.0, 194.0, 155.0);
            if x >= dx && x < dx + dw && y >= dy && y < dy + dh {
                let (lx, ly) = (x - dx, y - dy);
                let near = 18.0;
                let offered = self.bs_edges_offered(tab);
                let e = if ly < near {
                    0
                } else if ly > dh - near {
                    2
                } else if lx < near {
                    1
                } else if lx > dw - near {
                    3
                } else if (ly - dh / 2.0).abs() < near && offered.contains(&4) {
                    4
                } else if (lx - dw / 2.0).abs() < near && offered.contains(&5) {
                    5
                } else {
                    return true;
                };
                if offered.contains(&e) {
                    self.bs_toggle(tab, e);
                }
            }
            return true;
        }
        false
    }

    /// A round-12 control changed.
    pub(super) fn r12_changed(&mut self, id: &str) {
        match (self.kind, id) {
            (DocFormKind::BordersShading, "bColor" | "bWidth") => self.bs_pen_changed(0),
            (DocFormKind::BordersShading, "pColor" | "pWidth") => self.bs_pen_changed(1),
            (DocFormKind::BordersShading, "bApply") => {
                let i = self.sel("bApply");
                self.form.set_sel("sApply", i);
                self.bs_scope_changed();
            }
            (DocFormKind::BordersShading, "sApply") => {
                let i = self.sel("sApply");
                self.form.set_sel("bApply", i);
                self.bs_scope_changed();
            }
            (DocFormKind::BordersShading, "sStyle") => {
                let solid = self.sel("sStyle").unwrap_or(0) > 0;
                self.form.enable("sColor", solid);
                self.form.enable("sColorL", solid);
            }
            (DocFormKind::NewBlock, "name") => {
                let ok = !self.text("name").trim().is_empty();
                self.form.enable("ok", ok);
            }
            (DocFormKind::TablePosition, "hPick") => {
                let v = self.text("hPick");
                self.form.set_text("hPos", &v);
            }
            (DocFormKind::TablePosition, "vPick") => {
                let v = self.text("vPick");
                self.form.set_text("vPos", &v);
            }
            (DocFormKind::TablePosition, "move") => {
                // Move with text: the vertical position is measured from the paragraph.
                let on = self.on("move");
                self.form.set_sel("vRel", Some(if on { 2 } else { 0 }));
            }
            (DocFormKind::TablePosition, "vRel") => {
                let para = self.sel("vRel") == Some(2);
                self.set_on("move", para);
            }
            _ => {}
        }
    }

    /// Apply to changed: the settings' names and the edges follow the new scope.
    fn bs_scope_changed(&mut self) {
        let s = self.bs().setting[0];
        if s > 0 && s < 4 {
            self.bs_preset(0, s);
        }
    }

    /// OK of a round-12 dialog.
    pub(super) fn r12_ok(&mut self) -> FormOut {
        match self.kind {
            DocFormKind::TablePosition => self.position_ok(),
            DocFormKind::NewBlock => self.new_block_ok(),
            _ => self.bs_ok(),
        }
    }

    /// Create New Building Block OK: `saveBuildingBlock` in the user's file (through the
    /// window, which stores the file).
    fn new_block_ok(&self) -> FormOut {
        let name = self.text("name");
        if name.trim().is_empty() {
            return FormOut::Redraw;
        }
        let gallery = NEW_BLOCK_GALLERIES[self.sel("gallery").unwrap_or(0).min(6)].0;
        let behavior = ["content", "p", "pg"][self.sel("options").unwrap_or(1).min(2)];
        let spec = json!({
            "name": name.trim(), "gallery": gallery, "category": self.text("category"),
            "description": self.text("desc"), "behavior": behavior,
        });
        FormOut::Run(
            vec![json!({ "cmd": "ui.action", "id": format!("blocks12.save:{spec}") })],
            true,
        )
    }

    fn position_ok(&self) -> FormOut {
        let inches = self.preview.first().is_some_and(|u| u == "in");
        let tw = |id: &str| parse(&self.text(id), inches).unwrap_or(0).max(0);
        // A typed name (Left, Center, …) is an alignment, anything else a length.
        let pick = |edit: &str, list: &str, values: &[&str]| -> (Option<String>, i32) {
            let text = self.text(edit);
            let items = self
                .form
                .get(list)
                .map(|c| match &c.kind {
                    super::Kind::Combo { items, .. } => items.clone(),
                    _ => Vec::new(),
                })
                .unwrap_or_default();
            match items
                .iter()
                .position(|i| i.eq_ignore_ascii_case(text.trim()))
            {
                Some(k) => (values.get(k).map(|v| (*v).to_owned()), 0),
                None => (None, parse(&text, inches).unwrap_or(0)),
            }
        };
        let (x_align, x) = pick(
            "hPos",
            "hPick",
            &["left", "center", "right", "inside", "outside"],
        );
        let (y_align, y) = pick(
            "vPos",
            "vPick",
            &["top", "center", "bottom", "inside", "outside"],
        );
        let h_anchor = ["margin", "page", "text"][self.sel("hRel").unwrap_or(2).min(2)];
        let v_anchor = ["margin", "page", "text"][self.sel("vRel").unwrap_or(2).min(2)];
        FormOut::Run(
            vec![json!({ "cmd": "tableEdit", "edit": {
                "kind": "position",
                "overlap": self.on("overlap"),
                "float": {
                    "left_from_text": tw("dLeft"), "right_from_text": tw("dRight"),
                    "top_from_text": tw("dTop"), "bottom_from_text": tw("dBottom"),
                    "horz_anchor": h_anchor, "vert_anchor": v_anchor,
                    "x": x, "x_align": x_align, "y": y, "y_align": y_align,
                },
            } })],
            true,
        )
    }

    /// Borders and Shading OK: the edits of the tab shown last decide nothing — every tab's
    /// choices are applied (borders, page borders when changed, shading).
    fn bs_ok(&mut self) -> FormOut {
        let mut cmds = Vec::new();
        let scope = self.bs_scope();
        let e = self.bs().edges[0].clone();
        // Shadow / 3-D of a paragraph or one cell: §17.18.2 w:shadow / w:frame on the edges.
        let tabular = self.bs_tabular(scope);
        let setting = self.bs().setting[0];
        let (shadow, frame) = (!tabular && setting == 2, !tabular && setting == 3);
        let pen = |i: usize| {
            e[i].as_ref().map(|l| {
                let mut v = l.json();
                if i < 4 {
                    v["shadow"] = json!(shadow);
                    v["frame"] = json!(frame);
                }
                v
            })
        };
        match scope {
            "text" => {
                let border = e.iter().take(4).flatten().next().map(Line::json);
                cmds.push(json!({ "cmd": "formatText", "format": { "kind": "advanced",
                    "patch": { "border": border } } }));
            }
            "paragraph" => {
                let sp = self.bs().space;
                cmds.push(json!({ "cmd": "formatParagraph", "format": {
                    "kind": "setBorders", "top": pen(0), "left": pen(1), "bottom": pen(2),
                    "right": pen(3), "between": pen(4), "space": sp,
                } }));
            }
            s => {
                cmds.push(json!({ "cmd": "tableEdit", "edit": {
                    "kind": "setBorders", "cells": s == "cell",
                    "top": pen(0), "left": pen(1), "bottom": pen(2), "right": pen(3),
                    "insideH": pen(4), "insideV": pen(5),
                    "tl2br": pen(6), "tr2bl": pen(7),
                } }));
            }
        }
        // Shading: the fill, and the pattern in the pattern colour (§17.18.78); text has no
        // pattern of its own here, so Solid there takes the pattern colour as before.
        let pat_i = self.sel("sStyle").unwrap_or(0).min(PATTERNS.len() - 1);
        let pcol = self.form.color("sColor");
        let pcol = (!pcol.is_empty() && pcol != super::form::NO_COLOR).then_some(pcol);
        let pattern = (pat_i > 0).then(|| json!({ "val": PATTERNS[pat_i].0, "color": pcol }));
        let fill = if pat_i == 1
            && self
                .bs()
                .scopes
                .get(self.sel("sApply").unwrap_or(0))
                .copied()
                .unwrap_or(scope)
                == "text"
        {
            self.form.color("sColor")
        } else {
            self.form.color("sFill")
        };
        let hex = (!fill.is_empty() && fill != super::form::NO_COLOR).then_some(fill);
        let pat_changed = self
            .words
            .iter()
            .find(|w| w.0 == "shdPat")
            .is_none_or(|w| w.1 != pat_i.to_string())
            || pat_i > 0;
        let shading_scope = self
            .bs()
            .scopes
            .get(self.sel("sApply").unwrap_or(0))
            .copied()
            .unwrap_or(scope);
        if Some(hex.clone())
            != self
                .words
                .iter()
                .find(|w| w.0 == "shd")
                .map(|w| (!w.1.is_empty()).then(|| w.1.clone()))
            || shading_scope != scope
            || pat_changed
        {
            cmds.push(match shading_scope {
                "text" => json!({ "cmd": "formatText", "format": { "kind": "advanced",
                    "patch": { "shading": hex } } }),
                "paragraph" => json!({ "cmd": "formatParagraph",
                    "format": { "kind": "shading", "hex": hex, "pattern": pattern } }),
                s => json!({ "cmd": "tableEdit",
                    "edit": { "kind": "shading", "hex": hex, "whole": s == "table",
                        "pattern": pattern } }),
            });
        }
        // Page borders only when the Page Border tab was changed.
        if self.bs().edges[1] != self.bs_page_initial()
            || self.words.iter().any(|w| w.0 == "pageTouched")
        {
            cmds.push(self.bs_page_command());
        }
        FormOut::Run(cmds, true)
    }

    fn bs_page_initial(&self) -> [Option<Line>; EDGES] {
        self.bs.as_ref().map_or_else(Default::default, |b| {
            let mut e: [Option<Line>; EDGES] = Default::default();
            for (i, w) in self
                .words
                .iter()
                .filter(|w| w.0.starts_with("pageEdge"))
                .enumerate()
            {
                if i < 4 && !w.1.is_empty() {
                    e[i] = Some(Line {
                        style: style_index(&w.1),
                        size: b.edges[1].iter().flatten().next().map_or(4, |l| l.size),
                        color: String::new(),
                    });
                }
            }
            e
        })
    }

    /// `setPageBorders` for the Page Border tab.
    fn bs_page_command(&self) -> Value {
        let e = &self.bs().edges[1];
        let scope = if self.sel("pApply") == Some(1) {
            "section"
        } else {
            "document"
        };
        let Some(first) = e.iter().take(4).flatten().next() else {
            return json!({ "cmd": "setPageBorders", "borders": null, "scope": scope });
        };
        let color = (!first.color.is_empty() && first.color != super::form::NO_COLOR)
            .then(|| first.color.clone());
        json!({ "cmd": "setPageBorders", "scope": scope, "borders": {
            "style": LINE_STYLES[first.style], "size": first.size, "color": color, "space": 24,
            "sides": [e[0].is_some(), e[1].is_some(), e[2].is_some(), e[3].is_some()],
            "shadow": self.bs().setting[1] == 2,
        } })
    }

    /// Distances of paragraph borders from the text (points: top, left, bottom, right).
    pub(crate) fn bs_space(&self) -> [u32; 4] {
        self.bs.as_ref().map_or([1, 4, 1, 4], |b| b.space)
    }

    /// Set the distances (Border and Shading Options).
    pub(crate) fn set_bs_space(&mut self, s: [u32; 4]) {
        if let Some(b) = self.bs.as_mut() {
            b.space = s;
        }
    }

    /// Edge `i` of tab `tab` (tests).
    #[cfg(test)]
    pub(crate) fn bs_edge_for_test(&self, tab: usize, i: usize) -> Option<(String, u32)> {
        self.bs().edges[tab][i]
            .as_ref()
            .map(|l| (LINE_STYLES[l.style].to_owned(), l.size))
    }

    /// Click canvas `id` at local point (x, y) (tests).
    #[cfg(test)]
    pub(crate) fn canvas_for_test(&mut self, id: &str, x: f32, y: f32) -> bool {
        self.r12_canvas(id, x, y) || self.tp_canvas(id)
    }

    /// Edge buttons of the preview of tab `tab` (tests).
    #[cfg(test)]
    pub(crate) fn bs_buttons_for_test(&self, tab: usize) -> Vec<(usize, [f32; 4])> {
        self.bs_buttons(tab)
    }

    /// Change control `id` as the user would (tests).
    #[cfg(test)]
    pub(crate) fn changed_for_test(&mut self, id: &str) {
        self.r12_changed(id);
    }
}

/// Whether `kind` is a round-12 dialog.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::TablePosition | DocFormKind::BordersShading | DocFormKind::NewBlock
    )
}

/// Build a round-12 dialog.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    match kind {
        DocFormKind::TablePosition => open_position(s, c),
        DocFormKind::NewBlock => open_new_block(s, c),
        _ => open_borders(s, c),
    }
}

/// An editable drop-down: an edit box with a narrow drop-down button over its right end
/// whose list opens under the whole box (the reference's combo boxes).
fn edit_combo(
    f: &mut DocForm,
    edit: &'static str,
    list: &'static str,
    r: [f32; 4],
    value: &str,
    items: Vec<String>,
) {
    f.edit(edit, [r[0], r[1], r[2] - 22.0, r[3]], value);
    f.form.add(
        list,
        None,
        [r[0] + r[2] - 23.0, r[1], 23.0, r[3]],
        super::Kind::Combo { items, sel: 0 },
    );
    f.form.drop_spans.push((list, r[0], r[2]));
}

/// Galleries a new building block can go to (§17.18.15 ST_DocPartGallery) and their names.
const NEW_BLOCK_GALLERIES: [(&str, &str, &str); 7] = [
    ("tbls", "doc12.galTables", "Tables"),
    ("docParts", "doc12.galQuickParts", "Quick Parts"),
    ("autoTxt", "doc12.galAutoText", "AutoText"),
    ("coverPg", "doc12.galCover", "Cover Pages"),
    ("hdrs", "doc12.galHeaders", "Headers"),
    ("ftrs", "doc12.galFooters", "Footers"),
    ("txtBox", "doc12.galTextBoxes", "Text Boxes"),
];

/// Create New Building Block, measured on the reference at 125 %: 409 × 317 (name, gallery,
/// category, description, save in, options; OK grey until a name is typed).
fn open_new_block(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let mut f = DocForm::new(
        DocFormKind::NewBlock,
        &tr("doc12.newBlockTitle", "Create New Building Block"),
        (409.0, 317.0),
        vec![],
    );
    let rows: [(&'static str, &str, &str, f32); 6] = [
        ("nameL", "doc12.nameC", "Name:", 47.0),
        ("galleryL", "doc12.galleryC", "Gallery:", 78.0),
        ("categoryL", "doc12.categoryC", "Category:", 110.0),
        ("descL", "doc12.descriptionC", "Description:", 141.0),
        ("saveL", "doc12.saveInC", "Save in:", 203.0),
        ("optionsL", "doc12.optionsC", "Options:", 234.0),
    ];
    for (id, k, fb, y) in rows {
        f.label(id, [10.0, y + 4.0, 82.0, 18.0], &tr(k, fb));
    }
    // A name from the selection's first words, as the reference suggests one.
    let name: String = c
        .selection
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ");
    f.edit("name", [93.0, 47.0, 306.0, 26.0], &name);
    f.combo(
        "gallery",
        [93.0, 78.0, 306.0, 26.0],
        NEW_BLOCK_GALLERIES
            .iter()
            .map(|(_, k, fb)| tr(k, fb))
            .collect(),
        0,
    );
    f.combo(
        "category",
        [93.0, 110.0, 306.0, 26.0],
        vec![tr("doc12.general", "General")],
        0,
    );
    f.edit("desc", [93.0, 141.0, 306.0, 56.0], "");
    f.combo(
        "saveIn",
        [93.0, 203.0, 306.0, 26.0],
        vec![tr("doc12.userBlocks", "BlueOffice Building Blocks.dotx")],
        0,
    );
    f.combo(
        "options",
        [93.0, 234.0, 306.0, 26.0],
        vec![
            tr("doc12.insContent", "Insert content only"),
            tr("doc12.insParagraph", "Insert content in its own paragraph"),
            tr("doc12.insPage", "Insert content in its own page"),
        ],
        1,
    );
    f.button(
        "ok",
        [161.0, 277.0, 112.0, 29.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [287.0, 277.0, 112.0, 29.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.form.enable("ok", !name.is_empty());
    f
}

/// Table Positioning, measured on the reference at 125 %: 340 × 449.
fn open_position(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let mut f = DocForm::new(
        DocFormKind::TablePosition,
        &tr("doc12.posTitle", "Table Positioning"),
        (340.0, 449.0),
        vec![],
    );
    let inches = c.inches;
    f.preview = vec![if inches { "in" } else { "cm" }.to_owned()];
    let tbl = c.table.clone().unwrap_or_default();
    // An inline table gets the reference's first position: Left of the column, 0" below
    // the paragraph, 0.13" from the text on either side, no overlap.
    let fl = tbl
        .float
        .as_deref()
        .cloned()
        .unwrap_or(bo_model::TableFloat {
            left_from_text: bo_model::Twips(180),
            right_from_text: bo_model::Twips(180),
            horz_anchor: Some("text".into()),
            vert_anchor: Some("text".into()),
            x_align: Some("left".into()),
            ..Default::default()
        });
    let overlap = tbl.float.is_some() && tbl.overlap;
    let h_items = vec![
        tr("doc12.left", "Left"),
        tr("doc12.center", "Center"),
        tr("doc12.right", "Right"),
        tr("doc12.inside", "Inside"),
        tr("doc12.outside", "Outside"),
    ];
    let v_items = vec![
        tr("doc12.top", "Top"),
        tr("doc12.center", "Center"),
        tr("doc12.bottom", "Bottom"),
        tr("doc12.inside", "Inside"),
        tr("doc12.outside", "Outside"),
    ];
    let shown = |align: &Option<String>, values: &[&str], items: &[String], off: i32| {
        align
            .as_deref()
            .and_then(|a| values.iter().position(|v| *v == a))
            .and_then(|k| items.get(k).cloned())
            .unwrap_or_else(|| len(off, inches))
    };
    let hv = shown(
        &fl.x_align,
        &["left", "center", "right", "inside", "outside"],
        &h_items,
        fl.x.0,
    );
    let vv = shown(
        &fl.y_align,
        &["top", "center", "bottom", "inside", "outside"],
        &v_items,
        fl.y.0,
    );
    let anchor = |a: &Option<String>| match a.as_deref() {
        Some("margin") => 0,
        Some("page") => 1,
        _ => 2,
    };
    f.group(
        "hG",
        [10.0, 47.0, 320.0, 18.0],
        &tr("doc12.horizontal", "Horizontal"),
    );
    f.label(
        "hPosL",
        [24.0, 72.0, 140.0, 18.0],
        &tr("doc12.position", "Position:"),
    );
    edit_combo(
        &mut f,
        "hPos",
        "hPick",
        [24.0, 95.0, 152.0, 26.0],
        &hv,
        h_items,
    );
    f.label(
        "hRelL",
        [190.0, 72.0, 140.0, 18.0],
        &tr("doc12.relativeTo", "Relative to:"),
    );
    f.combo(
        "hRel",
        [190.0, 95.0, 140.0, 26.0],
        vec![
            tr("doc12.margin", "Margin"),
            tr("doc12.page", "Page"),
            tr("doc12.column", "Column"),
        ],
        anchor(&fl.horz_anchor),
    );
    f.group(
        "vG",
        [10.0, 132.0, 320.0, 18.0],
        &tr("doc12.vertical", "Vertical"),
    );
    f.label(
        "vPosL",
        [24.0, 157.0, 140.0, 18.0],
        &tr("doc12.position", "Position:"),
    );
    edit_combo(
        &mut f,
        "vPos",
        "vPick",
        [24.0, 180.0, 152.0, 26.0],
        &vv,
        v_items,
    );
    f.label(
        "vRelL",
        [190.0, 157.0, 140.0, 18.0],
        &tr("doc12.relativeTo", "Relative to:"),
    );
    let v_anchor = anchor(&fl.vert_anchor);
    f.combo(
        "vRel",
        [190.0, 180.0, 140.0, 26.0],
        vec![
            tr("doc12.margin", "Margin"),
            tr("doc12.page", "Page"),
            tr("doc12.paragraph", "Paragraph"),
        ],
        v_anchor,
    );
    f.group(
        "dG",
        [10.0, 212.0, 320.0, 18.0],
        &tr("doc12.distance", "Distance from surrounding text"),
    );
    for (lid, id, key, fb, x, y, v) in [
        (
            "dTopL",
            "dTop",
            "doc12.topC",
            "Top:",
            24.0,
            243.0,
            fl.top_from_text.0,
        ),
        (
            "dBottomL",
            "dBottom",
            "doc12.bottomC",
            "Bottom:",
            24.0,
            273.0,
            fl.bottom_from_text.0,
        ),
        (
            "dLeftL",
            "dLeft",
            "doc12.leftC",
            "Left:",
            190.0,
            243.0,
            fl.left_from_text.0,
        ),
        (
            "dRightL",
            "dRight",
            "doc12.rightC",
            "Right:",
            190.0,
            273.0,
            fl.right_from_text.0,
        ),
    ] {
        // Boxes measured at 83 (left column) and 237 (right column), 93 × 26.
        let bx = if x > 100.0 { 237.0 } else { 83.0 };
        f.label(lid, [x, y + 4.0, 56.0, 18.0], &tr(key, fb));
        f.edit(id, [bx, y, 93.0, 26.0], &len(v, inches));
    }
    f.group(
        "oG",
        [10.0, 309.0, 320.0, 18.0],
        &tr("doc12.options", "Options"),
    );
    f.check(
        "move",
        [24.0, 335.0, 220.0, 22.0],
        &tr("doc12.moveWithText", "Move with text"),
        v_anchor == 2,
    );
    f.check(
        "overlap",
        [24.0, 360.0, 220.0, 22.0],
        &tr("doc12.allowOverlap", "Allow overlap"),
        overlap,
    );
    f.button(
        "ok",
        [91.0, 410.0, 112.0, 29.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [218.0, 410.0, 112.0, 29.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f
}

/// Borders and Shading, measured on the reference at 125 %: 755 × 536 with the tabs
/// Borders, Page Border and Shading.
#[allow(clippy::too_many_lines)]
fn open_borders(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, f: &str| t(s, k, f);
    let mut f = DocForm::new(
        DocFormKind::BordersShading,
        &tr("doc12.bsTitle", "Borders and Shading"),
        (755.0, 536.0),
        vec![
            tr("doc12.tabBorders", "Borders"),
            tr("doc12.tabPage", "Page Border"),
            tr("doc12.tabShading", "Shading"),
        ],
    );
    let x = &c.extra;
    // Apply to: text (a selection in one paragraph), paragraph, and in a table cell / table.
    let mut scopes: Vec<&'static str> = Vec::new();
    if x["hasSelection"].as_bool().unwrap_or(false) {
        scopes.push("text");
    }
    scopes.push("paragraph");
    if c.table.is_some() {
        scopes.push("cell");
        scopes.push("table");
    }
    let first = if c.table.is_some() {
        if x["multiCell"].as_bool().unwrap_or(false) {
            scopes.iter().position(|s| *s == "table")
        } else {
            scopes.iter().position(|s| *s == "cell")
        }
    } else {
        scopes.iter().position(|s| *s == "paragraph")
    }
    .unwrap_or(0);
    let scope_name = |sc: &str| match sc {
        "text" => tr("doc12.applyText", "Text"),
        "cell" => tr("doc12.applyCell", "Cell"),
        "table" => tr("doc12.applyTable", "Table"),
        _ => tr("doc12.applyParagraph", "Paragraph"),
    };
    let scope_items: Vec<String> = scopes.iter().map(|s| scope_name(s)).collect();
    let mut bs = Bs {
        scopes: scopes.clone(),
        space: [1, 4, 1, 4],
        multi_para: x["multiPara"].as_bool().unwrap_or(false),
        multi_cell: x["multiCell"].as_bool().unwrap_or(false),
        ..Bs::default()
    };
    // Current borders of the scope shown first.
    let edges_of = |v: &Value| -> [Option<Line>; EDGES] {
        let mut e: [Option<Line>; EDGES] = Default::default();
        for (i, k) in [
            "top", "left", "bottom", "right", "insideH", "insideV", "tl2br", "tr2bl",
        ]
        .iter()
        .enumerate()
        {
            e[i] = line_of(&v[*k]);
        }
        e
    };
    let key = match scopes.get(first).copied().unwrap_or("paragraph") {
        "text" => "runBorders",
        "cell" => "cellBorders",
        "table" => "tableBorders",
        _ => "paraBorders",
    };
    bs.edges[0] = edges_of(&x[key]);
    if let Some(sp) = x["paraSpace"].as_array() {
        for (i, v) in sp.iter().take(4).enumerate() {
            bs.space[i] = v.as_u64().map_or(bs.space[i], |n| n as u32);
        }
    }
    let tabular = match scopes.get(first).copied().unwrap_or("") {
        "table" => true,
        "cell" => x["multiCell"].as_bool().unwrap_or(false),
        _ => false,
    };
    bs.setting[0] = setting_of(&bs.edges[0], tabular);
    // A box whose edges carry §17.18.2 w:shadow / w:frame is the Shadow / 3-D setting.
    if !tabular && bs.setting[0] == 1 {
        let top = &x[key]["top"];
        if top["shadow"].as_bool() == Some(true) {
            bs.setting[0] = 2;
        } else if top["frame"].as_bool() == Some(true) {
            bs.setting[0] = 3;
        }
    }
    let pb = &x["pageBorders"];
    let mut page_words = Vec::new();
    if let Some(style) = pb["style"].as_str() {
        let line = Line {
            style: style_index(style),
            size: pb["size"].as_u64().map_or(4, |s| s as u32),
            color: pb["color"].as_str().unwrap_or("").to_owned(),
        };
        let sides = pb["sides"].as_array().cloned().unwrap_or_default();
        for i in 0..4 {
            let on = sides.get(i).and_then(Value::as_bool).unwrap_or(true);
            bs.edges[1][i] = on.then(|| line.clone());
            page_words.push((
                format!("pageEdge{i}"),
                if on { style.to_owned() } else { String::new() },
            ));
        }
        bs.setting[1] = if pb["shadow"].as_bool().unwrap_or(false) {
            2
        } else if sides.iter().all(|s| s.as_bool().unwrap_or(true)) {
            1
        } else {
            4
        };
        bs.style[1] = line.style;
    }
    let pen0 = bs.edges[0].iter().flatten().next().cloned();
    if let Some(p) = &pen0 {
        bs.style[0] = p.style;
    }
    let page_pen = bs.edges[1].iter().flatten().next().cloned();
    let palette = x["palette"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    f.form.palette_labels = vec![
        tr("doc12.automatic", "Automatic"),
        tr("doc12.themeColors", "Theme Colors"),
        tr("doc12.standardColors", "Standard Colors"),
        tr("doc12.moreColors", "More Colors..."),
    ];
    let widths: Vec<String> = WIDTHS.iter().map(|w| w.1.to_owned()).collect();
    for (page, pre) in [(0usize, 'b'), (1, 'p')] {
        let id = |n: &str| -> &'static str { leak(&format!("{pre}{n}")) };
        f.label_on(
            page,
            id("SetL"),
            [24.0, 87.0, 120.0, 18.0],
            &tr("doc12.setting", "Setting:"),
        );
        for i in 0..5 {
            f.form.add(
                id(&format!("Set{i}")),
                Some(page),
                [24.0, 108.0 + 62.0 * i as f32, 124.0, 50.0],
                super::Kind::Canvas,
            );
        }
        f.form.add(
            id("Sep1"),
            Some(page),
            [152.0, 88.0, 2.0, 384.0],
            super::Kind::Canvas,
        );
        f.form.add(
            id("Sep2"),
            Some(page),
            [382.0, 88.0, 2.0, 384.0],
            super::Kind::Canvas,
        );
        f.label_on(
            page,
            id("StyleL"),
            [169.0, 87.0, 120.0, 18.0],
            &tr("doc12.style", "Style:"),
        );
        f.form.add(
            id("Style"),
            Some(page),
            [168.0, 108.0, 201.0, 174.0],
            super::Kind::Canvas,
        );
        f.label_on(
            page,
            id("ColorL"),
            [169.0, 298.0, 120.0, 18.0],
            &tr("doc12.color", "Color:"),
        );
        let pen = if page == 0 { &pen0 } else { &page_pen };
        f.form.add(
            id("Color"),
            Some(page),
            [168.0, 318.0, 201.0, 26.0],
            super::Kind::ColorPick {
                hex: pen.as_ref().map(|p| p.color.clone()).unwrap_or_default(),
                palette: palette.clone(),
            },
        );
        f.label_on(
            page,
            id("WidthL"),
            [169.0, 348.0, 120.0, 18.0],
            &tr("doc12.width", "Width:"),
        );
        f.form.add(
            id("Width"),
            Some(page),
            [168.0, 368.0, 201.0, 41.0],
            super::Kind::Combo {
                items: widths.clone(),
                sel: pen.as_ref().map_or(1, |p| width_index(p.size)),
            },
        );
        f.form.add(
            id("PrevG"),
            Some(page),
            [399.0, 88.0, 331.0, 18.0],
            super::Kind::Group(tr("doc12.preview", "Preview")),
        );
        f.form.add(
            id("Preview"),
            Some(page),
            [399.0, 110.0, 331.0, 270.0],
            super::Kind::Canvas,
        );
        f.label_on(
            page,
            id("ApplyL"),
            [399.0, 389.0, 120.0, 18.0],
            &tr("doc12.applyTo", "Apply to:"),
        );
        f.button_on(
            page,
            id("Options"),
            [618.0, 444.0, 112.0, 29.0],
            &tr("doc12.optionsB", "Options..."),
        );
    }
    f.form.add(
        "bApply",
        Some(0),
        [399.0, 410.0, 332.0, 28.0],
        super::Kind::Combo {
            items: scope_items.clone(),
            sel: first,
        },
    );
    f.form.add(
        "pApply",
        Some(1),
        [399.0, 410.0, 332.0, 28.0],
        super::Kind::Combo {
            items: vec![
                tr("doc12.wholeDocument", "Whole document"),
                tr("doc12.thisSection", "This section"),
            ],
            sel: 0,
        },
    );
    f.label_on(
        1,
        "pArtL",
        [169.0, 413.0, 120.0, 18.0],
        &tr("doc12.art", "Art:"),
    );
    f.form.add(
        "pArt",
        Some(1),
        [168.0, 434.0, 201.0, 40.0],
        super::Kind::Combo {
            items: vec![tr("doc12.artNone", "(none)")],
            sel: 0,
        },
    );
    f.form.enable("pArt", false);
    f.form.enable("pOptions", false);
    // Shading.
    let shd = x[match scopes.get(first).copied().unwrap_or("paragraph") {
        "text" => "runShading",
        "cell" => "cellShading",
        "table" => "cellShading",
        _ => "paraShading",
    }]
    .as_str()
    .unwrap_or("")
    .to_owned();
    f.words.push(("shd".into(), shd.clone()));
    // The shading pattern of a paragraph or cell (§17.18.78), its colour.
    let pat = &x["r13"]["shadingPattern"];
    let pat_i = PATTERNS
        .iter()
        .position(|p| Some(p.0) == pat["val"].as_str())
        .unwrap_or(0);
    f.words.push(("shdPat".into(), pat_i.to_string()));
    f.words.extend(page_words);
    f.form.add(
        "sFillG",
        Some(2),
        [24.0, 88.0, 345.0, 18.0],
        super::Kind::Group(tr("doc12.fill", "Fill")),
    );
    f.form.add(
        "sFill",
        Some(2),
        [38.0, 113.0, 143.0, 26.0],
        super::Kind::ColorPick {
            hex: if shd.is_empty() {
                super::form::NO_COLOR.to_owned()
            } else {
                shd
            },
            palette: palette.clone(),
        },
    );
    f.form.add(
        "sPatG",
        Some(2),
        [24.0, 154.0, 345.0, 18.0],
        super::Kind::Group(tr("doc12.patterns", "Patterns")),
    );
    f.label_on(
        2,
        "sStyleL",
        [24.0, 184.0, 60.0, 18.0],
        &tr("doc12.style", "Style:"),
    );
    f.form.add(
        "sStyle",
        Some(2),
        [87.0, 180.0, 166.0, 27.0],
        super::Kind::Combo {
            items: PATTERNS
                .iter()
                .map(|(_, name)| match *name {
                    "doc12.clear" => tr(name, "Clear"),
                    "doc12.solid" => tr(name, "Solid (100%)"),
                    n if n.starts_with("doc13.") => tr(n, n),
                    n => n.to_owned(),
                })
                .collect(),
            sel: pat_i,
        },
    );
    f.label_on(
        2,
        "sColorL",
        [24.0, 215.0, 60.0, 18.0],
        &tr("doc12.color", "Color:"),
    );
    f.form.add(
        "sColor",
        Some(2),
        [87.0, 211.0, 166.0, 27.0],
        super::Kind::ColorPick {
            hex: pat["color"].as_str().unwrap_or("").to_owned(),
            palette,
        },
    );
    f.form.enable("sColor", pat_i > 0);
    f.form.enable("sColorL", pat_i > 0);
    f.form.add(
        "sSep",
        Some(2),
        [382.0, 88.0, 2.0, 384.0],
        super::Kind::Canvas,
    );
    f.form.add(
        "sPrevG",
        Some(2),
        [399.0, 88.0, 331.0, 18.0],
        super::Kind::Group(tr("doc12.preview", "Preview")),
    );
    f.form.add(
        "sPreview",
        Some(2),
        [420.0, 130.0, 290.0, 240.0],
        super::Kind::Canvas,
    );
    f.label_on(
        2,
        "sApplyL",
        [399.0, 389.0, 120.0, 18.0],
        &tr("doc12.applyTo", "Apply to:"),
    );
    f.form.add(
        "sApply",
        Some(2),
        [399.0, 410.0, 332.0, 28.0],
        super::Kind::Combo {
            items: scope_items,
            sel: first,
        },
    );
    f.button(
        "ok",
        [505.0, 496.0, 112.0, 29.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [632.0, 496.0, 112.0, 29.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    // Texts the canvases draw (measured at layout time).
    let hint = [
        tr("doc12.hint1", "Click on diagram below or use"),
        tr("doc12.hint2", "buttons to apply borders"),
    ];
    let names = [
        tr("doc12.none", "None"),
        tr("doc12.box", "Box"),
        tr("doc12.shadow", "Shadow"),
        tr("doc12.threeD", "3-D"),
        tr("doc12.custom", "Custom"),
        tr("doc12.all", "All"),
        tr("doc12.grid", "Grid"),
        tr("doc12.sample", "Sample text Sample text"),
    ];
    for (i, h) in hint.iter().enumerate() {
        f.words.push((format!("hint{i}"), h.clone()));
    }
    for (i, n) in names.iter().enumerate() {
        f.words.push((format!("name{i}"), n.clone()));
    }
    let points: Vec<f32> = WIDTHS.iter().map(|w| w.0 as f32 / 8.0).collect();
    f.form.combo_lines.push(("bWidth", points.clone()));
    f.form.combo_lines.push(("pWidth", points));
    let auto = tr("doc12.automatic", "Automatic");
    let no_color = tr("doc12.noColor", "No Color");
    for id in ["bColor", "pColor", "sColor"] {
        f.form.swatch_text.push((id, auto.clone(), String::new()));
    }
    f.form
        .swatch_text
        .push(("sFill", no_color.clone(), no_color.clone()));
    f.words.push(("auto".into(), auto));
    f.words.push(("noColor".into(), no_color));
    f.bs = Some(Box::new(bs));
    if c.extra["pageTab"].as_bool().unwrap_or(false) {
        f.form.set_tab(1);
    }
    f
}

/// A `&'static str` for a generated control id (a few dozen per dialog, kept for the life of
/// the program like the literal ids).
fn leak(s: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static IDS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let set = IDS.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut g) = set.lock() else {
        return "";
    };
    if let Some(v) = g.get(s) {
        return v;
    }
    let v: &'static str = Box::leak(s.to_owned().into_boxed_str());
    g.insert(v);
    v
}

fn word<'a>(f: &'a DocForm, key: &str) -> &'a str {
    f.words
        .iter()
        .find(|w| w.0 == key)
        .map_or("", |w| w.1.as_str())
}

/// `a` mixed with `b` by `t` (0 = `a`), as `RRGGBB`.
fn mix_hex(a: &str, b: &str, t: f32) -> String {
    let pa = u32::from_str_radix(a, 16).unwrap_or(0xFF_FF_FF);
    let pb = u32::from_str_radix(b, 16).unwrap_or(0);
    let ch = |s: u32| {
        let x = ((pa >> s) & 0xFF) as f32;
        let y = ((pb >> s) & 0xFF) as f32;
        (x + (y - x) * t.clamp(0.0, 1.0)).round() as u32
    };
    format!("{:02X}{:02X}{:02X}", ch(16), ch(8), ch(0))
}

fn rgb(hex: &str) -> Rgba {
    if hex.is_empty() || hex == super::form::NO_COLOR {
        return Rgba(0, 0, 0, 255);
    }
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

const WHITE: Rgba = Rgba(255, 255, 255, 255);
const INK: Rgba = Rgba(0, 0, 0, 255);
const EDGE: Rgba = Rgba(0xab, 0xad, 0xb3, 255);
const SEL_FILL: Rgba = Rgba(0xcc, 0xe8, 0xff, 255);
const SEL_EDGE: Rgba = Rgba(0x00, 0x78, 0xd4, 255);
const TEXT_GREY: Rgba = Rgba(0xa6, 0xa6, 0xa6, 255);
const MARK: Rgba = Rgba(0x70, 0x70, 0x70, 255);

/// Draw a horizontal line of `style` from x to x + w at y (device pixels), `t` one line's
/// thickness.
fn hline(p: &mut dyn Painter, x: f32, y: f32, w: f32, style: &str, t: f32, ink: Rgba) {
    let dash = |p: &mut dyn Painter, on: &[f32], y: f32| {
        let mut i = 0.0;
        let mut k = 0;
        while i < w {
            let seg = on[k % on.len()] * t;
            if k % 2 == 0 {
                p.fill_rect(x + i, y, seg.min(w - i), t, ink);
            }
            i += seg;
            k += 1;
        }
    };
    match style {
        "dotted" => dash(p, &[1.0, 2.0], y),
        "dashSmallGap" => dash(p, &[4.0, 2.0], y),
        "dashed" => dash(p, &[6.0, 3.0], y),
        "dotDash" => dash(p, &[6.0, 2.0, 1.0, 2.0], y),
        "dotDotDash" => dash(p, &[6.0, 2.0, 1.0, 2.0, 1.0, 2.0], y),
        "dashDotStroked" => dash(p, &[8.0, 2.0, 2.0, 2.0], y),
        "double" => {
            p.fill_rect(x, y - t, w, t, ink);
            p.fill_rect(x, y + t, w, t, ink);
        }
        "triple" => {
            for k in [-2.0, 0.0, 2.0] {
                p.fill_rect(x, y + k * t, w, t, ink);
            }
        }
        s if s.starts_with("thinThickThin") => {
            p.fill_rect(x, y - 3.0 * t, w, t, ink);
            p.fill_rect(x, y - t, w, 2.0 * t, ink);
            p.fill_rect(x, y + 2.0 * t, w, t, ink);
        }
        s if s.starts_with("thinThick") => {
            let gap = if s.ends_with("LargeGap") { 3.0 } else { 1.0 };
            p.fill_rect(x, y - t - gap * t, w, t, ink);
            p.fill_rect(x, y, w, 2.0 * t, ink);
        }
        s if s.starts_with("thickThin") => {
            let gap = if s.ends_with("LargeGap") { 3.0 } else { 1.0 };
            p.fill_rect(x, y - t, w, 2.0 * t, ink);
            p.fill_rect(x, y + t + gap * t, w, t, ink);
        }
        "wave" | "doubleWave" => {
            let rows: &[f32] = if style == "wave" {
                &[0.0]
            } else {
                &[-1.5, 1.5]
            };
            for dy in rows {
                let mut segs = vec![PathSeg::M(x, y + dy * t)];
                let mut i = 0.0;
                while i < w {
                    let a = (i / (3.0 * t)).sin() * 1.5 * t;
                    segs.push(PathSeg::L(x + i, y + dy * t + a));
                    i += t;
                }
                p.path(&segs, None, Some((ink, t)));
            }
        }
        "threeDEmboss" | "threeDEngrave" | "outset" | "inset" => {
            let (a, b) = if matches!(style, "threeDEmboss" | "outset") {
                (Rgba(0xc0, 0xc0, 0xc0, 255), ink)
            } else {
                (ink, Rgba(0xc0, 0xc0, 0xc0, 255))
            };
            p.fill_rect(x, y - t, w, 2.0 * t, a);
            p.fill_rect(x, y + t, w, 2.0 * t, b);
        }
        _ => p.fill_rect(x, y, w, t, ink),
    }
}

/// A vertical line (the same patterns, solid parts only for the dashed families).
fn vline(p: &mut dyn Painter, x: f32, y: f32, h: f32, style: &str, t: f32, ink: Rgba) {
    match style {
        "double" => {
            p.fill_rect(x - t, y, t, h, ink);
            p.fill_rect(x + t, y, t, h, ink);
        }
        "triple" => {
            for k in [-2.0, 0.0, 2.0] {
                p.fill_rect(x + k * t, y, t, h, ink);
            }
        }
        "dotted" | "dashSmallGap" | "dashed" | "dotDash" | "dotDotDash" | "dashDotStroked" => {
            let (on, off) = if style == "dotted" {
                (1.0, 2.0)
            } else {
                (5.0, 3.0)
            };
            let mut i = 0.0;
            while i < h {
                p.fill_rect(x, y + i, t, (on * t).min(h - i), ink);
                i += (on + off) * t;
            }
        }
        s if s.starts_with("thinThick") || s.starts_with("thickThin") => {
            p.fill_rect(x - 2.0 * t, y, t, h, ink);
            p.fill_rect(x, y, 2.0 * t, h, ink);
        }
        _ => p.fill_rect(x, y, t, h, ink),
    }
}

/// Paint a round-12 canvas: `true` when `id` is one.
#[allow(clippy::too_many_lines)]
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
) -> bool {
    if f.kind != DocFormKind::BordersShading {
        return false;
    }
    let k = f.form.scale_k();
    let t1 = k.round().max(1.0);
    let line_h = font.size * 1.33;
    let tab = usize::from(id.starts_with('p'));
    if id.ends_with("Sep1") || id.ends_with("Sep2") || id == "sSep" {
        p.fill_rect(r[0], r[1], t1, r[3], Rgba(0xa0, 0xa0, 0xa0, 255));
        return true;
    }
    if let Some(n) = id
        .strip_prefix("bSet")
        .or_else(|| id.strip_prefix("pSet"))
        .and_then(|n| n.parse::<usize>().ok())
    {
        let scope = if tab == 0 { f.bs_scope() } else { "page" };
        let on = f.bs().setting[tab] == n;
        let icon = [r[0], r[1], (54.0 * k).round(), (50.0 * k).round()];
        // Measured on the reference (round 13): a #828790 frame on white; the chosen setting
        // adds a dotted focus line and a 2 px #0078D4 frame inside it, the picture stays white.
        p.fill_rect(
            icon[0],
            icon[1],
            icon[2],
            icon[3],
            Rgba(0x82, 0x87, 0x90, 255),
        );
        p.fill_rect(
            icon[0] + t1,
            icon[1] + t1,
            icon[2] - 2.0 * t1,
            icon[3] - 2.0 * t1,
            WHITE,
        );
        if on {
            let (x0, y0, w0, h0) = (
                icon[0] + 2.0 * t1,
                icon[1] + 2.0 * t1,
                icon[2] - 4.0 * t1,
                icon[3] - 4.0 * t1,
            );
            let mut i = 0.0;
            while i < w0 {
                p.fill_rect(x0 + i, y0, t1, t1, INK);
                p.fill_rect(x0 + i, y0 + h0 - t1, t1, t1, INK);
                i += 2.0 * t1;
            }
            let mut j = 0.0;
            while j < h0 {
                p.fill_rect(x0, y0 + j, t1, t1, INK);
                p.fill_rect(x0 + w0 - t1, y0 + j, t1, t1, INK);
                j += 2.0 * t1;
            }
            let blue = Rgba(0x00, 0x78, 0xd4, 255);
            let b = 2.0 * t1;
            let (x1, y1, w1, h1) = (x0 + t1, y0 + t1, w0 - 2.0 * t1, h0 - 2.0 * t1);
            p.fill_rect(x1, y1, w1, b, blue);
            p.fill_rect(x1, y1 + h1 - b, w1, b, blue);
            p.fill_rect(x1, y1, b, h1, blue);
            p.fill_rect(x1 + w1 - b, y1, b, h1, blue);
        }
        paint_setting_icon(p, icon, n, f.bs_tabular(scope), k);
        let name = match (n, f.bs_tabular(scope)) {
            (0, _) => word(f, "name0"),
            (1, _) => word(f, "name1"),
            (2, false) => word(f, "name2"),
            (3, false) => word(f, "name3"),
            (2, true) => word(f, "name5"),
            (3, true) => word(f, "name6"),
            _ => word(f, "name4"),
        };
        let dim = tab == 0 && scope == "text" && n == 4;
        p.text_font(
            name,
            r[0] + (68.0 * k).round(),
            (r[1] + (r[3] - line_h) / 2.0).round(),
            font,
            if dim { TEXT_GREY } else { INK },
        );
        return true;
    }
    if matches!(id, "bStyle" | "pStyle") {
        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x7a, 0x7a, 0x7a, 255));
        p.fill_rect(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, WHITE);
        let sb = (18.0 * k).round();
        let first = f.bs().top[tab];
        p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - sb - 2.0, r[3] - 2.0);
        for (row, style) in LINE_STYLES
            .iter()
            .enumerate()
            .skip(first)
            .take(STYLE_ROWS + 1)
        {
            let y = r[1] + ((2.0 + (row - first) as f32 * STYLE_ROW) * k).round();
            let rh = (STYLE_ROW * k).round() - 2.0;
            if row == f.bs().style[tab] {
                p.fill_rect(r[0] + 3.0 * k, y, r[2] - sb - 8.0 * k, rh, SEL_EDGE);
                p.fill_rect(
                    r[0] + 3.0 * k + 1.0,
                    y + 1.0,
                    r[2] - sb - 8.0 * k - 2.0,
                    rh - 2.0,
                    WHITE,
                );
            }
            hline(
                p,
                r[0] + (21.0 * k).round(),
                (y + rh / 2.0).round(),
                (140.0 * k).round(),
                style,
                t1,
                INK,
            );
        }
        p.pop_clip();
        // Scroll bar: track, thumb, arrows (own drawing).
        let sx = r[0] + r[2] - sb - 1.0;
        p.fill_rect(sx, r[1] + 1.0, sb, r[3] - 2.0, Rgba(0xf0, 0xf0, 0xf0, 255));
        let max = (LINE_STYLES.len() - STYLE_ROWS) as f32;
        let track = r[3] - 2.0 * sb;
        let th = (track * STYLE_ROWS as f32 / LINE_STYLES.len() as f32).max(12.0 * k);
        let ty = r[1] + sb + (track - th) * (first as f32 / max);
        p.fill_rect(
            sx + 5.0 * k,
            ty,
            sb - 10.0 * k,
            th,
            Rgba(0x85, 0x85, 0x85, 255),
        );
        for (cy, up) in [(r[1] + sb / 2.0, true), (r[1] + r[3] - sb / 2.0, false)] {
            let cx = sx + sb / 2.0;
            let d = 3.0 * k;
            let dy = if up { -d / 2.0 } else { d / 2.0 };
            p.path(
                &[
                    PathSeg::M(cx - d, cy - dy),
                    PathSeg::L(cx, cy + dy),
                    PathSeg::L(cx + d, cy - dy),
                    PathSeg::Z,
                ],
                Some(Rgba(0x60, 0x60, 0x60, 255)),
                None,
            );
        }
        return true;
    }
    if matches!(id, "bPreview" | "pPreview") {
        let scope = if tab == 0 { f.bs_scope() } else { "page" };
        // The hint in two centred lines over the diagram.
        for i in 0..2 {
            let s = word(f, &format!("hint{i}"));
            let w = f.form.width(s);
            p.text_font(
                s,
                (r[0] + r[2] / 2.0 - w / 2.0).round(),
                r[1] + ((4.0 + 17.0 * i as f32) * k).round(),
                font,
                INK,
            );
        }
        let at = |x: f32, y: f32, w: f32, h: f32| {
            [
                r[0] + (x * k).round(),
                r[1] + (y * k).round(),
                (w * k).round(),
                (h * k).round(),
            ]
        };
        let d = at(76.0, 56.0, 194.0, 155.0);
        p.fill_rect(d[0], d[1], d[2], d[3], WHITE);
        let edges = &f.bs().edges[tab];
        let m = (20.0 * k).round();
        let (bx, by, bw, bh) = (d[0] + m, d[1] + m, d[2] - 2.0 * m, d[3] - 2.0 * m);
        // Content: text lines (paragraph / page / text) or a 2 × 2 table.
        if f.bs_tabular(scope) {
            for row in 0..2 {
                for col in 0..2 {
                    let cx = bx + col as f32 * bw / 2.0 + 6.0 * k;
                    let cy = by + row as f32 * bh / 2.0 + 8.0 * k;
                    for l in 0..4 {
                        p.fill_rect(
                            cx,
                            cy + l as f32 * 9.0 * k,
                            bw / 2.0 - 12.0 * k,
                            (4.0 * k).round(),
                            TEXT_GREY,
                        );
                    }
                }
            }
        } else {
            let n = 10;
            for l in 0..n {
                let short = l == 0 || l == n - 1;
                let lx = if l == 0 { bx + bw * 0.35 } else { bx + 4.0 * k };
                let lw = if short { bw * 0.6 } else { bw - 8.0 * k };
                p.fill_rect(
                    lx,
                    by + 4.0 * k + l as f32 * (bh - 8.0 * k) / n as f32,
                    lw,
                    (4.0 * k).round(),
                    TEXT_GREY,
                );
            }
        }
        // Corner marks where no border is drawn.
        if edges.iter().all(Option::is_none) {
            let c = (8.0 * k).round();
            for (x, y, sx, sy) in [
                (bx, by, -1.0, -1.0),
                (bx + bw, by, 1.0, -1.0),
                (bx, by + bh, -1.0, 1.0),
                (bx + bw, by + bh, 1.0, 1.0),
            ] {
                p.fill_rect(if sx < 0.0 { x - c } else { x }, y, c, t1, MARK);
                p.fill_rect(x, if sy < 0.0 { y - c } else { y }, t1, c, MARK);
            }
        }
        let lw = |l: &Line| (l.size as f32 / 8.0 * k).max(1.0).round();
        let ink = |l: &Line| rgb(&l.color);
        if let Some(l) = &edges[0] {
            hline(p, bx, by, bw, LINE_STYLES[l.style], lw(l), ink(l));
        }
        if let Some(l) = &edges[2] {
            hline(p, bx, by + bh, bw, LINE_STYLES[l.style], lw(l), ink(l));
        }
        if let Some(l) = &edges[1] {
            vline(p, bx, by, bh, LINE_STYLES[l.style], lw(l), ink(l));
        }
        if let Some(l) = &edges[3] {
            vline(p, bx + bw, by, bh, LINE_STYLES[l.style], lw(l), ink(l));
        }
        if let Some(l) = &edges[4] {
            hline(
                p,
                bx,
                by + bh / 2.0,
                bw,
                LINE_STYLES[l.style],
                lw(l),
                ink(l),
            );
        }
        if let Some(l) = &edges[5] {
            vline(
                p,
                bx + bw / 2.0,
                by,
                bh,
                LINE_STYLES[l.style],
                lw(l),
                ink(l),
            );
        }
        // Diagonals of the cell.
        for (i, from_left) in [(6, true), (7, false)] {
            if let Some(l) = &edges[i] {
                let (x0, x1) = if from_left {
                    (bx, bx + bw)
                } else {
                    (bx + bw, bx)
                };
                p.path(
                    &[PathSeg::M(x0, by), PathSeg::L(x1, by + bh)],
                    None,
                    Some((ink(l), lw(l))),
                );
            }
        }
        // Shadow (paragraph Shadow / page Shadow setting).
        if f.bs().setting[tab] == 2 && !f.bs_tabular(scope) && edges.iter().any(Option::is_some) {
            let s = (4.0 * k).round();
            p.fill_rect(bx + s, by + bh, bw, s, Rgba(0x80, 0x80, 0x80, 255));
            p.fill_rect(bx + bw, by + s, s, bh, Rgba(0x80, 0x80, 0x80, 255));
        }
        for (e, b) in f.bs_buttons(tab) {
            let br = at(b[0], b[1], b[2], b[3]);
            let on = edges[e].is_some();
            p.fill_rect(br[0], br[1], br[2], br[3], if on { SEL_EDGE } else { EDGE });
            p.fill_rect(
                br[0] + 1.0,
                br[1] + 1.0,
                br[2] - 2.0,
                br[3] - 2.0,
                if on {
                    SEL_FILL
                } else {
                    Rgba(0xfd, 0xfd, 0xfd, 255)
                },
            );
            paint_edge_icon(p, br, e, k);
        }
        return true;
    }
    if id == "sPreview" {
        // The fill with the pattern colour mixed in by the pattern's share (§17.18.78).
        let pat_i = f.sel("sStyle").unwrap_or(0).min(PATTERNS.len() - 1);
        let base = f.form.color("sFill");
        let pc = f.form.color("sColor");
        let pc = if pc.is_empty() || pc == super::form::NO_COLOR {
            "000000".to_owned()
        } else {
            pc
        };
        let cover = bo_model::run_fx::ShadingPattern {
            val: PATTERNS[pat_i].0.to_owned(),
            color: None,
        }
        .coverage();
        let fill = if pat_i == 0 {
            base
        } else {
            let b = if base.is_empty() || base == super::form::NO_COLOR {
                "FFFFFF".to_owned()
            } else {
                base
            };
            mix_hex(&b, &pc, cover)
        };
        if !fill.is_empty() && fill != super::form::NO_COLOR {
            let m = (30.0 * k).round();
            p.fill_rect(
                r[0] + m,
                r[1] + m,
                r[2] - 2.0 * m,
                r[3] - 2.0 * m,
                rgb(&fill),
            );
        }
        // The reference's preview stays empty without a fill or a pattern.
        let none = f.form.color("sFill");
        if pat_i == 0 && (none.is_empty() || none == super::form::NO_COLOR) {
            return true;
        }
        let s = word(f, "name7");
        let w = f.form.width(s);
        p.text_font(
            s,
            (r[0] + r[2] / 2.0 - w / 2.0).round(),
            (r[1] + r[3] / 2.0 - line_h / 2.0).round(),
            font,
            INK,
        );
        return true;
    }
    false
}

/// A setting picture (own drawing): a small page of text lines, or a 3 × 3 table for cells
/// and tables, with the setting's border.
fn paint_setting_icon(p: &mut dyn Painter, r: [f32; 4], n: usize, table: bool, k: f32) {
    let t = k.round().max(1.0);
    let (x, y, w, h) = (
        r[0] + 9.0 * k,
        r[1] + 7.0 * k,
        r[2] - 18.0 * k,
        r[3] - 14.0 * k,
    );
    let grey = Rgba(0xaa, 0xaa, 0xaa, 255);
    if table {
        // Cell text bars.
        for row in 0..3 {
            for col in 0..3 {
                p.fill_rect(
                    x + col as f32 * w / 3.0 + 2.0 * k,
                    y + row as f32 * h / 3.0 + h / 6.0 - t,
                    w / 3.0 - 4.0 * k,
                    2.0 * t,
                    grey,
                );
            }
        }
    } else {
        // Text lines: a short first line at the right, full lines, a short last line.
        for l in 0..7 {
            let (lx, lw) = match l {
                0 => (x + w * 0.45, w * 0.55 - 3.0 * k),
                6 => (x + 3.0 * k, w * 0.6),
                _ => (x + 3.0 * k, w - 6.0 * k),
            };
            p.fill_rect(
                lx,
                y + 3.0 * k + l as f32 * (h - 6.0 * k) / 7.0,
                lw,
                t,
                grey,
            );
        }
    }
    let frame = |p: &mut dyn Painter, th: f32| {
        p.fill_rect(x, y, w, th, INK);
        p.fill_rect(x, y + h - th, w, th, INK);
        p.fill_rect(x, y, th, h, INK);
        p.fill_rect(x + w - th, y, th, h, INK);
    };
    match (n, table) {
        (1, _) => frame(p, 2.0 * t),
        (2, false) => {
            frame(p, t);
            p.fill_rect(x + 3.0 * t, y + h, w, 3.0 * t, INK);
            p.fill_rect(x + w, y + 3.0 * t, 3.0 * t, h, INK);
        }
        (3, false) => {
            frame(p, t);
            p.fill_rect(
                x + 2.0 * t,
                y + 2.0 * t,
                w - 4.0 * t,
                t,
                Rgba(0x60, 0x60, 0x60, 255),
            );
            p.fill_rect(
                x + 2.0 * t,
                y + 2.0 * t,
                t,
                h - 4.0 * t,
                Rgba(0x60, 0x60, 0x60, 255),
            );
            p.fill_rect(x, y + h - 2.0 * t, w, 2.0 * t, INK);
            p.fill_rect(x + w - 2.0 * t, y, 2.0 * t, h, INK);
        }
        (2 | 3, true) => {
            frame(p, if n == 3 { 2.0 * t } else { t });
            for i in 1..3 {
                p.fill_rect(x + i as f32 * w / 3.0, y, t, h, INK);
                p.fill_rect(x, y + i as f32 * h / 3.0, w, t, INK);
            }
        }
        (4, _) => {
            // Custom: two edges in different lines.
            p.fill_rect(x, y + h - 2.0 * t, w, 2.0 * t, INK);
            let mut i = 0.0;
            while i < h {
                p.fill_rect(x, y + i, t, 2.0 * t, INK);
                i += 4.0 * t;
            }
        }
        _ => {}
    }
}

/// An edge button picture (own drawing): a dotted square with the edge solid.
fn paint_edge_icon(p: &mut dyn Painter, r: [f32; 4], e: usize, k: f32) {
    let t = k.round().max(1.0);
    let s = (15.0 * k).round();
    let (x, y) = (
        (r[0] + (r[2] - s) / 2.0).round(),
        (r[1] + (r[3] - s) / 2.0).round(),
    );
    let dots = |p: &mut dyn Painter, x0: f32, y0: f32, horizontal: bool| {
        let mut i = 0.0;
        while i < s {
            if horizontal {
                p.fill_rect(x0 + i, y0, t, t, MARK);
            } else {
                p.fill_rect(x0, y0 + i, t, t, MARK);
            }
            i += 2.0 * t;
        }
    };
    dots(p, x, y, true);
    dots(p, x, y + s - t, true);
    dots(p, x, y, false);
    dots(p, x + s - t, y, false);
    dots(p, x, y + (s / 2.0).floor(), true);
    dots(p, x + (s / 2.0).floor(), y, false);
    let blue = Rgba(0x1f, 0x4e, 0x99, 255);
    match e {
        0 => p.fill_rect(x, y, s, 2.0 * t, blue),
        2 => p.fill_rect(x, y + s - 2.0 * t, s, 2.0 * t, blue),
        1 => p.fill_rect(x, y, 2.0 * t, s, blue),
        3 => p.fill_rect(x + s - 2.0 * t, y, 2.0 * t, s, blue),
        4 => p.fill_rect(x, y + (s / 2.0).floor() - t / 2.0, s, 2.0 * t, blue),
        6 | 7 => {
            let (x0, x1) = if e == 6 { (x, x + s) } else { (x + s, x) };
            p.path(
                &[PathSeg::M(x0, y), PathSeg::L(x1, y + s)],
                None,
                Some((blue, 1.5 * t)),
            );
        }
        _ => p.fill_rect(x + (s / 2.0).floor() - t / 2.0, y, 2.0 * t, s, blue),
    }
}
