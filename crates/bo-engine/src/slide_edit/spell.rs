//! Spelling and grammar in BlueSlide (`docs/parity/BACKLOG.md` CORE-SPELL-006), through the checker
//! BlueDoc uses (`crate::spell`, `crate::proof_text`).
//!
//! Text is checked paragraph by paragraph (§21.1.2.2.6 `a:p`), each run in its own language:
//! `a:rPr/@lang` (§21.1.2.3.9), else the paragraph's `a:pPr/a:defRPr/@lang`; runs marked
//! `a:rPr/@noProof` are not checked, nor are fields (§21.1.2.2.4 `a:fld`: slide numbers, dates).
//! The text units are the shapes with text (groups included), table cells (§21.1.3.16 `a:tc`),
//! SmartArt nodes (the text outline of `slide_edit::smartart`) and the speaker notes
//! (§19.3.1.26 notes, body placeholder).
//!
//! Commands (JSON, before [`super::SlideCommand`]):
//! * `slideSpelling { slide? }` — wavy-line marks under the issues of a slide's shapes;
//! * `slideSpellAt { x, y, slide? }` — the issue under a point, with corrections;
//! * `slideSpellCheck` — every issue of the deck in checking order (the F7 dialog);
//! * `slideSpellReplace { edits }` — corrections as one undoable transaction (replace all);
//! * `slideSpellIgnore`, `slideSpellAddWord`, `slideGrammarIgnore`, `slideSpellOptions`.
//!
//! Queries answer `{ "slideSpell": … }`; `slideSpellReplace` answers a [`super::SlideEditResult`].

use super::geom::{find, place_slide, BoxR, Placed};
use super::ops::{node_at, Tx};
use super::smartart::{self, SmartItem};
use super::{node_clone, notes_body_index, refuse, rich, text, Outcome, SlideSel, R};
use crate::proof_text::{self, OptionsIn, Replace, TextIssue};
use crate::spell::Span;
use crate::{Engine, EngineError};
use bo_model::slide::{
    GraphicContent, Paragraph, Presentation, RunProps, ShapeNode, TextBody, TextRun,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Where a text unit lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SpellTarget {
    /// A shape's text body (`p:sp/p:txBody`), by shape id (`cNvPr@id`).
    Shape {
        /// Slide index.
        slide: usize,
        /// Shape id.
        id: u32,
    },
    /// A table cell (graphic frame `id`, grid row and column).
    Cell {
        /// Slide index.
        slide: usize,
        /// Graphic frame id.
        id: u32,
        /// Row.
        row: usize,
        /// Grid column.
        col: usize,
    },
    /// The speaker notes of a slide.
    Notes {
        /// Slide index.
        slide: usize,
    },
    /// A SmartArt diagram (paragraph = node of its text outline).
    SmartArt {
        /// Slide index.
        slide: usize,
        /// Graphic frame id.
        id: u32,
    },
}

/// A correction of a paragraph of a text unit.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SlideReplace {
    /// The text unit.
    pub target: SpellTarget,
    /// Paragraph index in the unit.
    pub para: usize,
    /// Byte range and replacement.
    #[serde(flatten)]
    pub edit: Replace,
}

/// A spelling command of BlueSlide.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SlideSpellCommand {
    /// Marks of a slide (default: the current one).
    SlideSpelling {
        /// Slide index.
        #[serde(default)]
        slide: Option<usize>,
    },
    /// The issue at slide point (x, y).
    SlideSpellAt {
        /// Slide index (default: the current one).
        #[serde(default)]
        slide: Option<usize>,
        /// Slide x in points.
        x: f64,
        /// Slide y in points.
        y: f64,
    },
    /// Every issue of the deck.
    SlideSpellCheck,
    /// Apply corrections as one undoable transaction.
    SlideSpellReplace {
        /// Corrections.
        edits: Vec<SlideReplace>,
    },
    /// Ignore a word everywhere (this session).
    SlideSpellIgnore {
        /// The word.
        word: String,
    },
    /// Add a word to the user dictionary.
    SlideSpellAddWord {
        /// The word.
        word: String,
    },
    /// Ignore a grammar issue (rule id, flagged text).
    SlideGrammarIgnore {
        /// Rule id.
        rule: String,
        /// Flagged text.
        text: String,
    },
    /// Read or change proofing options.
    SlideSpellOptions {
        /// New option values (absent: unchanged).
        #[serde(flatten)]
        options: OptionsIn,
    },
}

impl SlideSpellCommand {
    /// Parses a spelling command (`None`: another command).
    pub fn parse(json: &str) -> Option<SlideSpellCommand> {
        let v: Value = serde_json::from_str(json).ok()?;
        let name = v.get("cmd")?.as_str()?;
        if !name.starts_with("slideSpell") && name != "slideGrammarIgnore" {
            return None;
        }
        serde_json::from_value(v).ok()
    }
}

/// A wavy line under an issue on the slide (slide points).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellMarkOut {
    /// Line start `[x, y]`.
    pub from: [f64; 2],
    /// Line end `[x, y]`.
    pub to: [f64; 2],
    /// Height of the text line above it (points).
    pub h: f64,
    /// A grammar or style issue (blue) rather than a misspelt word (red).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub grammar: bool,
    /// Shape id.
    pub id: u32,
}

/// An issue of the deck, or the one under the pointer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideIssue {
    /// The text unit.
    pub target: SpellTarget,
    /// Slide index.
    pub slide: usize,
    /// Position of the unit in checking order.
    pub unit: usize,
    /// Paragraph index in the unit.
    pub para: usize,
    /// Offsets of the flagged text in the unit's editing text (paragraphs joined by `\n`),
    /// UTF-16 code units: what the text editor of a shape selects.
    pub body_from: usize,
    /// End of the flagged text in the editing text (UTF-16).
    pub body_to: usize,
    /// The issue (byte offsets in the paragraph text).
    #[serde(flatten)]
    pub issue: TextIssue,
}

/// A text unit: where it is and its paragraphs (editing text, language spans).
struct Unit {
    target: SpellTarget,
    paras: Vec<(String, Vec<Span>)>,
}

/// §21.1.2.3.9 rPr `noProof`: the run is not proofed.
fn no_proof(p: &RunProps) -> bool {
    p.attrs
        .iter()
        .any(|(k, v)| k == "noProof" && (v == "1" || v == "true"))
}

fn lang_of(p: &RunProps) -> Option<String> {
    p.lang
        .as_deref()
        .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("x-none"))
        .map(str::to_owned)
}

/// Editing text of a paragraph (as [`text::para_text`]) with its language spans.
fn para_unit(p: &Paragraph) -> (String, Vec<Span>) {
    let def = p.props.as_ref().and_then(|pp| pp.def_rpr.as_ref());
    let def_lang = def.and_then(lang_of);
    let def_skip = def.is_some_and(no_proof);
    let mut s = String::new();
    let mut spans: Vec<Span> = Vec::new();
    for r in &p.runs {
        let (piece, lang, skip) = match r {
            TextRun::Run { props, text } => (
                text.as_str(),
                lang_of(props).or_else(|| def_lang.clone()),
                def_skip || no_proof(props),
            ),
            // Field text (slide number, date) is not the author's; a line break is no word.
            TextRun::Field { text, .. } => (text.as_str(), None, true),
            TextRun::Break { .. } => ("\u{b}", None, true),
            TextRun::Other(_) => continue,
        };
        let start = s.len();
        s.push_str(piece);
        if piece.is_empty() {
            continue;
        }
        match spans.last_mut() {
            Some(sp) if sp.range.end == start && sp.lang == lang && sp.skip == skip => {
                sp.range.end = s.len();
            }
            _ => spans.push(Span {
                range: start..s.len(),
                lang,
                skip,
            }),
        }
    }
    debug_assert_eq!(s, text::para_text(p));
    (s, spans)
}

fn body_unit(target: SpellTarget, tb: &TextBody) -> Unit {
    Unit {
        target,
        paras: tb.paragraphs.iter().map(para_unit).collect(),
    }
}

fn node_units(pres: &Presentation, slide: usize, nodes: &[ShapeNode], out: &mut Vec<Unit>) {
    for n in nodes {
        match n {
            ShapeNode::Shape(s) => {
                if let Some(tb) = &s.text {
                    out.push(body_unit(SpellTarget::Shape { slide, id: s.nv.id }, tb));
                }
            }
            ShapeNode::Group(g) => node_units(pres, slide, &g.children, out),
            ShapeNode::Frame(f) => match &f.content {
                GraphicContent::Table(t) => {
                    for (row, r) in t.rows.iter().enumerate() {
                        for (col, c) in r.cells.iter().enumerate() {
                            // Cells merged into another hold no text of their own.
                            if c.h_merge == Some(true) || c.v_merge == Some(true) {
                                continue;
                            }
                            if let Some(tb) = &c.text {
                                let target = SpellTarget::Cell {
                                    slide,
                                    id: f.nv.id,
                                    row,
                                    col,
                                };
                                out.push(body_unit(target, tb));
                            }
                        }
                    }
                }
                GraphicContent::Other(_) => {
                    if smartart::is_smartart(n) {
                        if let Some(sa) = smartart::smartart_out(pres, slide, f.nv.id) {
                            out.push(Unit {
                                target: SpellTarget::SmartArt { slide, id: f.nv.id },
                                paras: sa
                                    .items
                                    .into_iter()
                                    .map(|i| {
                                        let spans = proof_text::whole(&i.text, None);
                                        (i.text, spans)
                                    })
                                    .collect(),
                            });
                        }
                    }
                }
            },
            ShapeNode::Picture(_) | ShapeNode::Connector(_) | ShapeNode::Other(_) => {}
        }
    }
}

/// Text units of a slide in checking order: shapes (tree order), then the notes.
fn slide_units(pres: &Presentation, slide: usize) -> Vec<Unit> {
    let Some(s) = pres.slides.get(slide) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    node_units(pres, slide, &s.data.shapes, &mut out);
    if let Some(n) = &s.notes {
        let body = notes_body_index(n).and_then(|i| n.data.shapes.get(i));
        if let Some(ShapeNode::Shape(sh)) = body {
            if let Some(tb) = &sh.text {
                out.push(body_unit(SpellTarget::Notes { slide }, tb));
            }
        }
    }
    out
}

fn deck_units(pres: &Presentation) -> Vec<Unit> {
    (0..pres.slides.len())
        .flat_map(|i| slide_units(pres, i))
        .collect()
}

/// Character offset in the unit's editing text of byte `at` of paragraph `para`.
fn body_chars(unit: &Unit, para: usize, at: usize) -> usize {
    let before: usize = unit.paras[..para]
        .iter()
        .map(|(t, _)| t.chars().count() + 1)
        .sum();
    before
        + unit.paras[para]
            .0
            .get(..at)
            .map_or(0, |s| s.chars().count())
}

fn body_utf16(unit: &Unit, para: usize, at: usize) -> usize {
    let before: usize = unit.paras[..para]
        .iter()
        .map(|(t, _)| t.encode_utf16().count() + 1)
        .sum();
    before + proof_text::utf16_at(&unit.paras[para].0, at)
}

/// Corrections of one paragraph, keeping run formatting; fields and line breaks stay.
fn replace_para(p: &mut Paragraph, edits: &[&Replace]) -> R<()> {
    let text = text::para_text(p);
    let Some(plan) = proof_text::plan(&text, edits) else {
        return refuse("spellStale");
    };
    let fixed: Vec<bool> = p
        .runs
        .iter()
        .map(|r| !matches!(r, TextRun::Run { .. }))
        .collect();
    let mut pieces: Vec<String> = p
        .runs
        .iter()
        .map(|r| match r {
            TextRun::Run { text, .. } | TextRun::Field { text, .. } => text.clone(),
            TextRun::Break { .. } => "\u{b}".to_owned(),
            TextRun::Other(_) => String::new(),
        })
        .collect();
    for e in plan {
        let mut parts: Vec<&mut String> = pieces.iter_mut().collect();
        if !proof_text::splice_parts(&mut parts, &|i| !fixed[i], e.start, e.end, &e.text) {
            return refuse("spellStale");
        }
    }
    for (r, s) in p.runs.iter_mut().zip(pieces) {
        if let TextRun::Run { text, .. } = r {
            *text = s;
        }
    }
    // Runs emptied by a correction go, as long as text remains.
    let keep = p
        .runs
        .iter()
        .any(|r| matches!(r, TextRun::Run { text, .. } if !text.is_empty()));
    if keep {
        p.runs
            .retain(|r| !matches!(r, TextRun::Run { text, .. } if text.is_empty()));
    }
    Ok(())
}

fn replace_body(tb: &mut TextBody, edits: &[&SlideReplace]) -> R<()> {
    let mut paras: Vec<usize> = edits.iter().map(|e| e.para).collect();
    paras.sort_unstable();
    paras.dedup();
    for i in paras {
        let list: Vec<&Replace> = edits
            .iter()
            .filter(|e| e.para == i)
            .map(|e| &e.edit)
            .collect();
        let Some(p) = tb.paragraphs.get_mut(i) else {
            return refuse("spellStale");
        };
        replace_para(p, &list)?;
    }
    Ok(())
}

fn path_of(pres: &Presentation, slide: usize, id: u32) -> R<Vec<usize>> {
    let placed = place_slide(pres, slide);
    find(&placed, id)
        .map(|p| p.path.clone())
        .ok_or(super::EditError::Refused("spellStale"))
}

/// Apply the corrections of one text unit inside a transaction.
fn replace_unit(tx: &mut Tx<'_>, target: &SpellTarget, edits: &[&SlideReplace]) -> R<()> {
    match *target {
        SpellTarget::Shape { slide, id } | SpellTarget::Cell { slide, id, .. } => {
            let path = path_of(tx.pres, slide, id)?;
            let mut n = node_clone(tx.pres, slide, &path)?;
            let tb = match (&mut n, target) {
                (ShapeNode::Shape(s), SpellTarget::Shape { .. }) => s.text.as_mut(),
                (ShapeNode::Frame(f), SpellTarget::Cell { row, col, .. }) => match &mut f.content {
                    GraphicContent::Table(t) => t
                        .rows
                        .get_mut(*row)
                        .and_then(|r| r.cells.get_mut(*col))
                        .and_then(|c| c.text.as_mut()),
                    GraphicContent::Other(_) => None,
                },
                _ => None,
            };
            let Some(tb) = tb else {
                return refuse("spellStale");
            };
            replace_body(tb, edits)?;
            tx.replace_node(slide, &path, n)?;
        }
        SpellTarget::Notes { slide } => {
            let Some(mut notes) = tx.pres.slides.get(slide).and_then(|s| s.notes.clone()) else {
                return refuse("spellStale");
            };
            let body = notes_body_index(&notes).and_then(|i| notes.data.shapes.get_mut(i));
            let Some(ShapeNode::Shape(sh)) = body else {
                return refuse("spellStale");
            };
            let Some(tb) = sh.text.as_mut() else {
                return refuse("spellStale");
            };
            replace_body(tb, edits)?;
            tx.fields(slide, |f| f.notes = Some(notes))?;
        }
        SpellTarget::SmartArt { slide, id } => {
            let Some(sa) = smartart::smartart_out(tx.pres, slide, id) else {
                return refuse("spellStale");
            };
            let mut items: Vec<SmartItem> = sa.items;
            for (i, item) in items.iter_mut().enumerate() {
                let list: Vec<&Replace> = edits
                    .iter()
                    .filter(|e| e.para == i)
                    .map(|e| &e.edit)
                    .collect();
                if list.is_empty() {
                    continue;
                }
                let Some(t) = proof_text::apply(&item.text, &list) else {
                    return refuse("spellStale");
                };
                item.text = t;
            }
            if edits.iter().any(|e| e.para >= items.len()) {
                return refuse("spellStale");
            }
            let sel = SlideSel {
                slide,
                ids: vec![id],
            };
            smartart::cmd_set_smartart(tx, &sel, Some(id), None, Some(&items), (None, None, None))?;
        }
    }
    Ok(())
}

/// Underline of a text box: its bottom edge (slide points).
fn underline(b: &BoxR) -> ([f64; 2], [f64; 2]) {
    let (x1, y1) = b.to_slide(-b.w / 2.0, b.h / 2.0);
    let (x2, y2) = b.to_slide(b.w / 2.0, b.h / 2.0);
    ([x1, y1], [x2, y2])
}

/// Placed shapes of a tree, groups opened.
fn flatten(placed: &[Placed], out: &mut Vec<Placed>) {
    for p in placed {
        if p.children.is_empty() {
            out.push(p.clone());
        } else {
            flatten(&p.children, out);
        }
    }
}

impl Engine {
    /// Runs a BlueSlide spelling command; returns its JSON answer.
    pub(crate) fn slide_spell(&mut self, cmd: SlideSpellCommand) -> crate::Result<String> {
        let current = self.slides.sel.slide;
        let out = match cmd {
            SlideSpellCommand::SlideSpelling { slide } => {
                let marks = self.slide_spell_marks(slide.unwrap_or(current));
                json!({ "marks": marks.into_iter().map(|(m, _, _)| m).collect::<Vec<_>>() })
            }
            SlideSpellCommand::SlideSpellAt { slide, x, y } => {
                let hit = self.slide_spell_at(slide.unwrap_or(current), x, y);
                json!({ "hit": hit })
            }
            SlideSpellCommand::SlideSpellCheck => self.slide_spell_check(),
            SlideSpellCommand::SlideSpellReplace { edits } => {
                let r = self.slide_spell_replace(&edits)?;
                return Ok(serde_json::to_string(&r).unwrap_or_default());
            }
            SlideSpellCommand::SlideSpellIgnore { word } => {
                self.spell.learn(&word, true);
                json!({ "ok": true })
            }
            SlideSpellCommand::SlideSpellAddWord { word } => {
                self.spell.learn(&word, false);
                json!({ "ok": true })
            }
            SlideSpellCommand::SlideGrammarIgnore { rule, text } => {
                self.spell.ignore_issue(&rule, &text);
                json!({ "ok": true })
            }
            SlideSpellCommand::SlideSpellOptions { options } => {
                json!({ "options": self.spell.set_options(&options) })
            }
        };
        Ok(json!({ "slideSpell": out }).to_string())
    }

    /// The tone placement most used in the deck.
    fn deck_style(&mut self, units: &[Unit]) -> Option<bo_spell::vi::ToneStyle> {
        let texts: Vec<(String, Vec<Span>)> =
            units.iter().flat_map(|u| u.paras.iter().cloned()).collect();
        proof_text::style_of(&mut self.spell, &texts)
    }

    /// Marks of a slide's shapes with the issue each one belongs to.
    fn slide_spell_marks(&mut self, slide: usize) -> Vec<(SpellMarkOut, BoxR, SlideIssue)> {
        let Some(pres) = self.pres.as_ref() else {
            return Vec::new();
        };
        if !self.spell.enabled || slide >= pres.slides.len() {
            return Vec::new();
        }
        let units = slide_units(pres, slide);
        let all = deck_units(pres);
        let style = {
            let texts: Vec<(String, Vec<Span>)> =
                all.iter().flat_map(|u| u.paras.iter().cloned()).collect();
            proof_text::style_of(&mut self.spell, &texts)
        };
        let mut placed = Vec::new();
        flatten(&place_slide(pres, slide), &mut placed);
        let mut out = Vec::new();
        for (ui, unit) in units.iter().enumerate() {
            let SpellTarget::Shape { id, .. } = unit.target else {
                continue;
            };
            let Some(p) = placed.iter().find(|p| p.id == id) else {
                continue;
            };
            let Some(ShapeNode::Shape(s)) = node_at(pres, slide, &p.path) else {
                continue;
            };
            let node = ShapeNode::Shape(s.clone());
            let b = rich::text_box(p, &node);
            let mut found = Vec::new();
            for (pi, (t, sp)) in unit.paras.iter().enumerate() {
                let (issues, _) = proof_text::check(&mut self.spell, t, sp, style, false);
                found.extend(issues.into_iter().map(|i| (pi, i)));
            }
            if found.is_empty() {
                continue;
            }
            let Some(g) = rich::geometry(pres, slide, s, &b, &mut self.fonts) else {
                continue;
            };
            for (pi, issue) in found {
                let cs = body_chars(unit, pi, issue.start);
                let ce = body_chars(unit, pi, issue.end);
                let (_, rects) = rich::selection_shapes(&g, &b, cs, ce);
                let si = SlideIssue {
                    target: unit.target.clone(),
                    slide,
                    unit: ui,
                    para: pi,
                    body_from: body_utf16(unit, pi, issue.start),
                    body_to: body_utf16(unit, pi, issue.end),
                    issue,
                };
                for r in rects {
                    let (from, to) = underline(&r);
                    let m = SpellMarkOut {
                        from,
                        to,
                        h: r.h,
                        grammar: si.issue.grammar,
                        id,
                    };
                    out.push((m, r, si.clone()));
                }
            }
        }
        out
    }

    /// The issue under slide point (x, y), with corrections.
    fn slide_spell_at(&mut self, slide: usize, x: f64, y: f64) -> Option<SlideIssue> {
        const TOL: f64 = 2.0;
        let (_, _, mut hit) = self
            .slide_spell_marks(slide)
            .into_iter()
            .find(|(_, b, _)| {
                let (lx, ly) = b.to_local(x, y);
                lx.abs() <= b.w / 2.0 + TOL && ly.abs() <= b.h / 2.0 + TOL
            })?;
        if !hit.issue.grammar {
            hit.issue.suggestions = self
                .spell
                .suggest(&hit.issue.word, hit.issue.lang.as_deref());
        }
        Some(hit)
    }

    /// Every issue of the deck in checking order.
    fn slide_spell_check(&mut self) -> Value {
        let Some(pres) = self.pres.as_ref() else {
            return json!({ "issues": [], "unchecked": [], "languages": [] });
        };
        let units = deck_units(pres);
        let style = self.deck_style(&units);
        let mut issues = Vec::new();
        let mut unchecked = std::collections::BTreeSet::new();
        for (ui, unit) in units.iter().enumerate() {
            let slide = match unit.target {
                SpellTarget::Shape { slide, .. }
                | SpellTarget::Cell { slide, .. }
                | SpellTarget::Notes { slide }
                | SpellTarget::SmartArt { slide, .. } => slide,
            };
            for (pi, (t, sp)) in unit.paras.iter().enumerate() {
                let (found, none) = proof_text::check(&mut self.spell, t, sp, style, true);
                unchecked.extend(none);
                issues.extend(found.into_iter().map(|issue| SlideIssue {
                    target: unit.target.clone(),
                    slide,
                    unit: ui,
                    para: pi,
                    body_from: body_utf16(unit, pi, issue.start),
                    body_to: body_utf16(unit, pi, issue.end),
                    issue,
                }));
            }
        }
        json!({
            "issues": issues,
            "unchecked": unchecked,
            "languages": proof_text::languages(&self.spell),
        })
    }

    /// Corrections as one undoable transaction.
    fn slide_spell_replace(
        &mut self,
        edits: &[SlideReplace],
    ) -> crate::Result<super::SlideEditResult> {
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            if ed.in_master_view() {
                out.error = Some("masterView".into());
            } else {
                let mut targets: Vec<&SpellTarget> = Vec::new();
                for e in edits {
                    if !targets.contains(&&e.target) {
                        targets.push(&e.target);
                    }
                }
                ed.mutate(pres, &mut out, |tx, _| {
                    for t in targets {
                        let list: Vec<&SlideReplace> =
                            edits.iter().filter(|e| &e.target == t).collect();
                        replace_unit(tx, t, &list)?;
                    }
                    Ok(())
                });
            }
            // The text being edited follows the corrected text (offsets kept in range).
            ed.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slide_edit::SlideCommand;

    /// A corpus deck whose first slide is emptied (no shapes, no notes).
    fn deck() -> Engine {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-edit-arrange.pptx");
        let mut e = Engine::with_bundled_fonts();
        e.open(&std::fs::read(p).unwrap(), "deck.pptx").unwrap();
        let pres = e.pres.as_mut().unwrap();
        pres.slides[0].data.shapes.clear();
        pres.slides[0].notes = None;
        e.slide_command(SlideCommand::GoTo { slide: 0 }).unwrap();
        e
    }

    /// Issues of slide 0.
    fn first_slide(v: &Value) -> Vec<Value> {
        v["slideSpell"]["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["slide"] == 0)
            .cloned()
            .collect()
    }

    fn spell(e: &mut Engine, json: &str) -> Value {
        let s = e.slide_command_json(json).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    fn run(text: &str, lang: Option<&str>, no_proof: bool) -> TextRun {
        TextRun::Run {
            props: RunProps {
                lang: lang.map(str::to_owned),
                attrs: if no_proof {
                    vec![("noProof".into(), "1".into())]
                } else {
                    vec![]
                },
                ..RunProps::default()
            },
            text: text.into(),
        }
    }

    /// Adds a text box to slide 0 holding `paras`; returns its id.
    fn set_paras(e: &mut Engine, paras: Vec<Paragraph>) -> u32 {
        let r = e
            .slide_command(SlideCommand::InsertTextBox {
                text: "x".into(),
                rect: None,
            })
            .unwrap();
        let id = r.selection[0].id;
        let pres = e.pres.as_mut().unwrap();
        let shape = pres.slides[0].data.shapes.iter_mut().find_map(|n| match n {
            ShapeNode::Shape(s) if s.nv.id == id => Some(s),
            _ => None,
        });
        shape.unwrap().text.as_mut().unwrap().paragraphs = paras;
        id
    }

    fn words(v: &Value) -> Vec<String> {
        first_slide(v)
            .iter()
            .filter(|i| i["grammar"].as_bool() != Some(true))
            .map(|i| i["word"].as_str().unwrap().to_owned())
            .collect()
    }

    // source: misspellings of common Vietnamese and English words written for this test.
    #[test]
    fn per_run_language_and_no_proof() {
        let mut e = deck();
        let p = Paragraph {
            runs: vec![
                // "the" is a Vietnamese syllable: checked as English only by its run language.
                run("Xin chàoo ", Some("vi-VN"), false),
                run("the documnet ", Some("en-US"), false),
                run("Blueofise ", Some("en-US"), true),
                run("hello", Some("vi-VN"), false),
            ],
            ..Paragraph::default()
        };
        set_paras(&mut e, vec![p]);
        let r = spell(&mut e, r#"{"cmd":"slideSpellCheck"}"#);
        // "chàoo" (vi), "documnet" (en), not "Blueofise" (noProof); "hello" is no Vietnamese
        // syllable ("he" + "llo") and its run says vi-VN.
        assert_eq!(words(&r), ["chàoo", "documnet", "hello"], "{r}");
        let issues = first_slide(&r);
        let i = issues.iter().find(|i| i["word"] == "documnet").unwrap();
        assert_eq!(i["lang"], "en-US");
        assert_eq!(i["target"]["kind"], "shape");
        assert!(i["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "document"));
    }

    #[test]
    fn notes_and_tables_are_checked_and_replace_all_undoes_at_once() {
        let mut e = deck();
        let id = set_paras(
            &mut e,
            vec![Paragraph {
                runs: vec![run("one documnet", Some("en-US"), false)],
                ..Paragraph::default()
            }],
        );
        e.slide_command(SlideCommand::SetNotes {
            slide: Some(0),
            text: "documnet in the notes".into(),
        })
        .unwrap();
        e.slide_command_json(r#"{"cmd":"insertTable","rows":2,"cols":2}"#)
            .unwrap();
        for n in &mut e.pres.as_mut().unwrap().slides[0].data.shapes {
            if let ShapeNode::Frame(f) = n {
                if let GraphicContent::Table(t) = &mut f.content {
                    let tb = t.rows[1].cells[0]
                        .text
                        .get_or_insert_with(TextBody::default);
                    text::set_text(tb, "a documnet");
                }
            }
        }
        let r = spell(&mut e, r#"{"cmd":"slideSpellCheck"}"#);
        let all = first_slide(&r);
        let issues: Vec<&Value> = all
            .iter()
            .filter(|i| i["word"] == "documnet" && i["grammar"].as_bool() != Some(true))
            .collect();
        assert_eq!(issues.len(), 3, "{r}");
        assert_eq!(issues[0]["target"]["kind"], "shape");
        assert_eq!(issues[1]["target"]["kind"], "cell");
        assert_eq!(
            (
                issues[1]["target"]["row"].as_u64(),
                issues[1]["target"]["col"].as_u64()
            ),
            (Some(1), Some(0))
        );
        // The notes come after the slide's shapes.
        assert_eq!(issues[2]["target"]["kind"], "notes");
        assert!(issues[2]["unit"].as_u64() > issues[1]["unit"].as_u64());
        let edits: Vec<String> = issues
            .iter()
            .map(|i| {
                format!(
                    r#"{{"target":{},"para":{},"start":{},"end":{},"text":"document","word":"documnet"}}"#,
                    i["target"], i["para"], i["start"], i["end"]
                )
            })
            .collect();
        let r = spell(
            &mut e,
            &format!(
                r#"{{"cmd":"slideSpellReplace","edits":[{}]}}"#,
                edits.join(",")
            ),
        );
        assert!(r.get("error").is_none(), "{r}");
        let text_of = |e: &Engine| {
            let pres = e.pres.as_ref().unwrap();
            let body = pres.slides[0].data.shapes.iter().find_map(|n| match n {
                ShapeNode::Shape(s) if s.nv.id == id => s.text.as_ref().map(text::body_text),
                _ => None,
            });
            (
                body.unwrap_or_default(),
                pres.slides[0].notes.as_ref().unwrap().text(),
            )
        };
        assert_eq!(
            text_of(&e),
            (
                "one document".to_owned(),
                "document in the notes".to_owned()
            )
        );
        let cell = |e: &Engine| {
            let pres = e.pres.as_ref().unwrap();
            pres.slides[0].data.shapes.iter().find_map(|n| match n {
                ShapeNode::Frame(f) => match &f.content {
                    GraphicContent::Table(t) => {
                        t.rows[1].cells[0].text.as_ref().map(text::body_text)
                    }
                    GraphicContent::Other(_) => None,
                },
                _ => None,
            })
        };
        assert_eq!(cell(&e).as_deref(), Some("a document"));
        // One undo takes back both corrections.
        e.slide_command(SlideCommand::Undo).unwrap();
        assert_eq!(
            text_of(&e),
            (
                "one documnet".to_owned(),
                "documnet in the notes".to_owned()
            )
        );
        assert_eq!(cell(&e).as_deref(), Some("a documnet"));
    }

    #[test]
    fn marks_and_the_issue_under_the_pointer() {
        let mut e = deck();
        set_paras(
            &mut e,
            vec![Paragraph {
                runs: vec![run("Good documnet", Some("en-US"), false)],
                ..Paragraph::default()
            }],
        );
        let r = spell(&mut e, r#"{"cmd":"slideSpelling"}"#);
        let marks = r["slideSpell"]["marks"].as_array().unwrap();
        assert_eq!(marks.len(), 1, "{r}");
        let m = &marks[0];
        let (x1, y1, x2) = (
            m["from"][0].as_f64().unwrap(),
            m["from"][1].as_f64().unwrap(),
            m["to"][0].as_f64().unwrap(),
        );
        assert!(x2 > x1);
        // Just above the line, half way along: the misspelt word.
        let at = format!(
            r#"{{"cmd":"slideSpellAt","x":{},"y":{}}}"#,
            f64::midpoint(x1, x2),
            y1 - 3.0
        );
        let r = spell(&mut e, &at);
        let hit = &r["slideSpell"]["hit"];
        assert_eq!(hit["word"], "documnet", "{r}");
        // "Good " is 5 UTF-16 units of the editing text.
        assert_eq!(
            (hit["bodyFrom"].as_u64(), hit["bodyTo"].as_u64()),
            (Some(5), Some(13))
        );
        assert!(hit["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "document"));
        // Added to the dictionary: no more marks.
        spell(&mut e, r#"{"cmd":"slideSpellAddWord","word":"documnet"}"#);
        let r = spell(&mut e, r#"{"cmd":"slideSpelling"}"#);
        assert_eq!(r["slideSpell"]["marks"].as_array().map(Vec::len), Some(0));
    }
}
