//! Layout tab: text columns (ECMA-376 Part 1, §17.6.4 cols, §17.6.3 col), line
//! numbering (§17.6.8 lnNumType), automatic hyphenation (§17.15.1.10
//! autoHyphenation and related settings), the text direction of a section
//! (§17.6.20 textDirection) and drop caps (§17.3.1.11 framePr `w:dropCap`). Every
//! change is one undo step.

use crate::{effective_run, EditError, Op, Pos, Result, Selection, Session, SetupScope};
use bo_model::index::body_paragraphs;
use bo_model::{
    Block, ColumnDef, Columns, DropCap, FrameProps, HalfPoints, Hyphenation, LineNumbering,
    Paragraph, Run, RunContent, Section, SectionBreak, TextDirection, Twips,
};
use serde::{Deserialize, Serialize};

/// Width and following gap of one text column (twips).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnWidth {
    /// Column width.
    pub width: i32,
    /// Space after the column (ignored for the last one).
    #[serde(default)]
    pub space: i32,
}

/// Text columns sent by hosts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnsSpec {
    /// Number of columns (1–45).
    pub count: u16,
    /// Gap between equal columns (twips).
    #[serde(default = "half_inch")]
    pub space: i32,
    /// Draw a line between the columns (§17.6.4 `w:sep`).
    #[serde(default)]
    pub separator: bool,
    /// One entry per column for columns of unequal width; empty = equal columns.
    #[serde(default)]
    pub widths: Vec<ColumnWidth>,
}

fn half_inch() -> i32 {
    720
}

/// Where a column layout applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LayoutScope {
    /// The caret's section.
    #[default]
    Section,
    /// Every section.
    Document,
    /// From the caret's paragraph to the end: a continuous section break is
    /// inserted before it (§17.18.77 `continuous`).
    Forward,
}

/// A drop cap sent by hosts (§17.3.1.11 framePr).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropCapSpec {
    /// In the text or in the margin.
    pub position: DropCap,
    /// Font of the letter (`None`: the paragraph's).
    #[serde(default)]
    pub font: Option<String>,
    /// Height in lines (1–10).
    #[serde(default = "three")]
    pub lines: u8,
    /// Distance from the text (twips).
    #[serde(default)]
    pub distance: i32,
}

fn three() -> u8 {
    3
}

/// Layout tab state of the caret's section and paragraph.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageLayoutState {
    /// Columns of the caret's section.
    pub columns: ColumnsSpec,
    /// Width of the text area (twips), for the column widths.
    pub text_width: i32,
    /// Line numbering of the caret's section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_numbers: Option<LineNumbering>,
    /// Text direction of the caret's section (`tbRl`, `btLr`; absent = horizontal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<String>,
    /// Document hyphenation settings.
    pub hyphenation: Hyphenation,
    /// Drop cap of the caret's paragraph (or of the one just before it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_cap: Option<DropCapSpec>,
    /// The caret's paragraph is left out of line numbering.
    pub suppress_line_numbers: bool,
    /// The caret's paragraph is not hyphenated.
    pub suppress_auto_hyphens: bool,
}

/// Model columns for `spec` (unequal widths only when they differ).
fn columns_of(spec: &ColumnsSpec) -> Columns {
    let n = usize::from(spec.count.clamp(1, 45));
    let space = Twips(spec.space.clamp(0, 31_680));
    // Unequal when a width, or a gap between columns, differs from the first.
    let unequal = n > 1
        && spec.widths.len() == n
        && (spec.widths.iter().any(|w| w.width != spec.widths[0].width)
            || spec.widths[..n - 1]
                .iter()
                .any(|w| w.space != spec.widths[0].space));
    let mut c = Columns {
        separator: spec.separator && n > 1,
        ..Columns::equal(spec.count, space)
    };
    if unequal {
        c.cols = spec
            .widths
            .iter()
            .enumerate()
            .map(|(i, w)| ColumnDef {
                width: Twips(w.width.clamp(144, 31_680)),
                space: Twips(if i + 1 < n {
                    w.space.clamp(0, 31_680)
                } else {
                    0
                }),
            })
            .collect();
    }
    c
}

/// Text width of a section (twips).
fn text_width(s: &Section) -> i32 {
    (s.page.width.0 - s.page.margin_left.0 - s.page.margin_right.0).max(720)
}

/// Spec of the columns of `s` (widths of equal columns spelled out).
fn spec_of(s: &Section) -> ColumnsSpec {
    let c = &s.columns;
    let n = c.len();
    let widths = if c.unequal() {
        c.cols
            .iter()
            .map(|d| ColumnWidth {
                width: d.width.0,
                space: d.space.0,
            })
            .collect()
    } else {
        let n32 = i32::try_from(n).unwrap_or(1);
        let w = (text_width(s) - c.space.0 * (n32 - 1)) / n32;
        (0..n)
            .map(|i| ColumnWidth {
                width: w,
                space: if i + 1 < n { c.space.0 } else { 0 },
            })
            .collect()
    };
    ColumnsSpec {
        count: u16::try_from(n).unwrap_or(1),
        space: c.space.0,
        separator: c.separator,
        widths,
    }
}

impl Session {
    /// Top-level path `(section, block)` of the caret's paragraph.
    fn caret_block(&self) -> Option<(usize, usize, usize)> {
        let at = self.selection.ordered().0;
        let paths = body_paragraphs(&self.doc);
        match paths.get(at.para).map(Vec::as_slice) {
            Some(&[si, bi]) => Some((si, bi, at.para)),
            _ => None,
        }
    }

    /// Drop cap paragraph of the caret: (section, block, paragraph id) of the caret's
    /// paragraph when it is one, else of the paragraph just before it.
    fn drop_cap_block(&self) -> Option<(usize, usize, usize)> {
        let (si, bi, id) = self.caret_block()?;
        let blocks = &self.doc.sections.get(si)?.blocks;
        let is_cap =
            |b: Option<&Block>| matches!(b, Some(Block::Paragraph(p)) if p.props.frame.is_some());
        if is_cap(blocks.get(bi)) {
            Some((si, bi, id))
        } else if bi > 0 && is_cap(blocks.get(bi - 1)) {
            Some((si, bi - 1, id - 1))
        } else {
            None
        }
    }

    /// Layout tab state.
    pub fn page_layout_state(&self) -> PageLayoutState {
        let si = self.section_state().index;
        let sec = self.doc.sections.get(si);
        let caret_para = self.caret_block().and_then(|(si, bi, _)| {
            match self.doc.sections.get(si)?.blocks.get(bi)? {
                Block::Paragraph(p) => Some(p),
                _ => None,
            }
        });
        let drop_cap = self.drop_cap_block().and_then(|(si, bi, _)| {
            let Block::Paragraph(p) = self.doc.sections.get(si)?.blocks.get(bi)? else {
                return None;
            };
            let f = p.props.frame?;
            Some(DropCapSpec {
                position: f.drop_cap,
                font: p.runs.iter().find_map(|r| r.props.font.clone()),
                lines: f.lines,
                distance: f.h_space.0,
            })
        });
        PageLayoutState {
            columns: sec.map_or_else(|| spec_of(&Section::default()), spec_of),
            text_width: sec.map_or(9360, text_width),
            line_numbers: sec.and_then(|s| s.line_numbers),
            text_direction: sec
                .and_then(|s| s.text_direction)
                .filter(|d| d.rotation() != 0)
                .map(|d| d.as_str().to_owned()),
            hyphenation: self.doc.hyphenation,
            drop_cap,
            suppress_line_numbers: caret_para
                .is_some_and(|p| p.props.suppress_line_numbers == Some(true)),
            suppress_auto_hyphens: caret_para
                .is_some_and(|p| p.props.suppress_auto_hyphens == Some(true)),
        }
    }

    /// Replace sections `targets` by `f` applied to a copy of each (one undo step).
    fn update_sections(&mut self, targets: Vec<usize>, f: impl Fn(&mut Section)) -> Result<()> {
        self.edit(|tx, sel| {
            for si in targets {
                let old = tx.doc.sections.get(si).cloned().ok_or(EditError::BadPath)?;
                let mut new = old.clone();
                f(&mut new);
                if new != old {
                    tx.push(Op::Sections {
                        index: si,
                        remove: vec![old],
                        insert: vec![new],
                    })?;
                }
            }
            Ok(sel)
        })?;
        Ok(())
    }

    fn scope_sections(&self, scope: SetupScope) -> Vec<usize> {
        match scope {
            SetupScope::Section => vec![self.section_state().index],
            SetupScope::Document => (0..self.doc.sections.len()).collect(),
        }
    }

    /// Text columns of the caret's section, every section, or from the caret's
    /// paragraph on (a continuous section break goes before it).
    pub fn set_columns(&mut self, spec: &ColumnsSpec, scope: LayoutScope) -> Result<()> {
        let cols = columns_of(spec);
        match scope {
            LayoutScope::Section => self.update_sections(vec![self.section_state().index], |s| {
                s.columns = cols.clone();
            }),
            LayoutScope::Document => {
                self.update_sections((0..self.doc.sections.len()).collect(), |s| {
                    s.columns = cols.clone();
                })
            }
            LayoutScope::Forward => {
                let (si, bi, _) = self.caret_block().ok_or_else(|| {
                    EditError::Unsupported("columns from a paragraph inside a table".into())
                })?;
                let n = self.doc.sections.len();
                if bi == 0 {
                    return self.update_sections((si..n).collect(), |s| s.columns = cols.clone());
                }
                self.edit(|tx, sel| {
                    let old: Vec<Section> = tx.doc.sections[si..].to_vec();
                    let sec = &old[0];
                    let mut first = sec.clone();
                    first.blocks = sec.blocks[..bi].to_vec();
                    first.src = None;
                    // §17.6.17: the first part's sectPr goes in its last paragraph.
                    let mut shift = 0;
                    if !matches!(first.blocks.last(), Some(Block::Paragraph(_))) {
                        first.blocks.push(Block::Paragraph(Paragraph::default()));
                        shift = 1;
                    }
                    let mut second = sec.clone();
                    second.blocks = sec.blocks[bi..].to_vec();
                    second.break_type = SectionBreak::Continuous;
                    second.columns = cols.clone();
                    let mut insert = vec![first, second];
                    for s in &old[1..] {
                        let mut s = s.clone();
                        s.columns = cols.clone();
                        insert.push(s);
                    }
                    tx.push(Op::Sections {
                        index: si,
                        remove: old,
                        insert,
                    })?;
                    let (a, b) = sel.ordered();
                    let mv = |p: Pos| Pos {
                        para: p.para + shift,
                        offset: p.offset,
                    };
                    Ok(Selection {
                        anchor: mv(a),
                        focus: mv(b),
                    })
                })?;
                Ok(())
            }
        }
    }

    /// Line numbering (`None`: off) of the caret's section or every section
    /// (§17.6.8 lnNumType).
    pub fn set_line_numbering(
        &mut self,
        numbering: Option<LineNumbering>,
        scope: SetupScope,
    ) -> Result<()> {
        let ln = numbering.map(|mut l| {
            l.count_by = l.count_by.clamp(1, 100);
            l.distance = l.distance.map(|d| Twips(d.0.clamp(0, 31_680)));
            l
        });
        self.update_sections(self.scope_sections(scope), |s| s.line_numbers = ln)
    }

    /// Text direction of the caret's section or every section (§17.6.20
    /// textDirection; `None` or a horizontal direction: horizontal).
    pub fn set_section_text_direction(
        &mut self,
        dir: Option<TextDirection>,
        scope: SetupScope,
    ) -> Result<()> {
        let dir = dir.filter(|d| d.rotation() != 0);
        self.update_sections(self.scope_sections(scope), |s| s.text_direction = dir)
    }

    /// Document hyphenation settings (§17.15.1.10 autoHyphenation, §17.15.1.52
    /// hyphenationZone, §17.15.1.21 consecutiveHyphenLimit, §17.15.1.40
    /// doNotHyphenateCaps).
    pub fn set_hyphenation(&mut self, h: Hyphenation) -> Result<()> {
        let new = Hyphenation {
            zone: h.zone.map(|z| Twips(z.0.clamp(0, 31_680))),
            consecutive_limit: h.consecutive_limit.filter(|n| *n > 0),
            ..h
        };
        let old = self.doc.hyphenation;
        if new == old {
            return Ok(());
        }
        self.edit(|tx, sel| {
            tx.push(Op::Hyphenation { old, new })?;
            Ok(sel)
        })?;
        Ok(())
    }

    /// Make the first letter of the caret's paragraph a drop cap, change the drop
    /// cap of the caret's paragraph (or of the one before it), or remove it
    /// (`None`): the letter goes back to the start of the paragraph.
    pub fn set_drop_cap(&mut self, spec: Option<&DropCapSpec>) -> Result<()> {
        let existing = self.drop_cap_block();
        let frame = spec.map(|s| FrameProps {
            drop_cap: s.position,
            lines: s.lines.clamp(1, 10),
            h_space: Twips(s.distance.clamp(0, 31_680)),
        });
        let font = spec
            .and_then(|s| s.font.clone())
            .filter(|f| !f.trim().is_empty());
        match (existing, frame) {
            (None, None) => Ok(()),
            // Remove: the cap's runs start the next paragraph again.
            (Some((si, bi, id)), None) => {
                let blocks = &self.doc.sections[si].blocks;
                let (Some(Block::Paragraph(cap)), Some(Block::Paragraph(next))) =
                    (blocks.get(bi), blocks.get(bi + 1))
                else {
                    return Err(EditError::Unsupported(
                        "drop cap without its paragraph".into(),
                    ));
                };
                let base = next
                    .runs
                    .first()
                    .map(|r| r.props.clone())
                    .unwrap_or_default();
                let mut merged = next.clone();
                merged.src = None;
                let runs: Vec<Run> = cap
                    .runs
                    .iter()
                    .map(|r| Run {
                        props: base.clone(),
                        content: r.content.clone(),
                    })
                    .collect();
                merged.runs.splice(0..0, runs);
                let remove = vec![
                    Block::Paragraph(cap.clone()),
                    Block::Paragraph(next.clone()),
                ];
                self.edit(|tx, _| {
                    tx.push(Op::Blocks {
                        path: vec![si],
                        index: bi,
                        remove,
                        insert: vec![Block::Paragraph(merged)],
                    })?;
                    Ok(Selection::caret(Pos {
                        para: id,
                        offset: 0,
                    }))
                })?;
                Ok(())
            }
            // Change the existing cap's frame and font.
            (Some((si, bi, _)), Some(f)) => {
                let Some(Block::Paragraph(cap)) = self.doc.sections[si].blocks.get(bi) else {
                    return Err(EditError::BadPath);
                };
                let mut new = cap.clone();
                new.src = None;
                new.props.frame = Some(f);
                for r in &mut new.runs {
                    r.props.font.clone_from(&font);
                }
                let remove = vec![Block::Paragraph(cap.clone())];
                self.edit(|tx, sel| {
                    tx.push(Op::Blocks {
                        path: vec![si],
                        index: bi,
                        remove,
                        insert: vec![Block::Paragraph(new)],
                    })?;
                    Ok(sel)
                })?;
                Ok(())
            }
            // New: the first letter moves into a drop cap paragraph before.
            (None, Some(f)) => {
                let (si, bi, id) = self
                    .caret_block()
                    .ok_or_else(|| EditError::Unsupported("drop cap inside a table".into()))?;
                let Some(Block::Paragraph(p)) = self.doc.sections[si].blocks.get(bi) else {
                    return Err(EditError::BadPath);
                };
                let (cap, rest) = split_first_letter(&self.doc, p, f, font)?;
                let remove = vec![Block::Paragraph(p.clone())];
                self.edit(|tx, _| {
                    tx.push(Op::Blocks {
                        path: vec![si],
                        index: bi,
                        remove,
                        insert: vec![Block::Paragraph(cap), Block::Paragraph(rest)],
                    })?;
                    Ok(Selection::caret(Pos {
                        para: id + 1,
                        offset: 0,
                    }))
                })?;
                Ok(())
            }
        }
    }
}

/// Split paragraph `p` into a drop cap paragraph holding its first letter (as
/// large as `lines` of its text: its size times the lines times 1.1, roughly the
/// height of single-spaced lines) and the rest of the paragraph.
fn split_first_letter(
    doc: &bo_model::Document,
    p: &Paragraph,
    f: FrameProps,
    font: Option<String>,
) -> Result<(Paragraph, Paragraph)> {
    let no_text = || EditError::Unsupported("the paragraph does not start with text".into());
    let ri = p
        .runs
        .iter()
        .position(|r| !r.content.is_empty())
        .ok_or_else(no_text)?;
    let run = &p.runs[ri];
    let Some(RunContent::Text(t)) = run.content.first() else {
        return Err(no_text());
    };
    let first = t.trim_start().chars().next().ok_or_else(no_text)?;
    let lead = t.len() - t.trim_start().len();
    let rest_text = t[lead + first.len_utf8()..].to_owned();
    let size = effective_run(doc, p, &run.props).size.map_or(22, |s| s.0);
    let cap_size = (size.clamp(2, 3276) as f32 * f32::from(f.lines) * 1.1).round() as u32;
    let mut cap_props = run.props.clone();
    cap_props.size = Some(HalfPoints(cap_size.clamp(2, 3276)));
    if font.is_some() {
        cap_props.font = font;
    }
    let cap = Paragraph::new(
        bo_model::ParaProps {
            style: p.props.style.clone(),
            frame: Some(f),
            keep_next: Some(true),
            space_before: Some(Twips(0)),
            space_after: Some(Twips(0)),
            ..bo_model::ParaProps::default()
        },
        vec![Run::text(first.to_string(), cap_props)],
    );
    let mut rest = p.clone();
    rest.src = None;
    let mut content = run.content.clone();
    if rest_text.is_empty() {
        content.remove(0);
    } else {
        content[0] = RunContent::Text(rest_text);
    }
    if content.is_empty() {
        rest.runs.remove(ri);
    } else {
        rest.runs[ri].content = content;
    }
    Ok((cap, rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Document, ParaProps, RunProps};

    fn session(paras: &[&str]) -> Session {
        let blocks = paras
            .iter()
            .map(|t| {
                Block::Paragraph(Paragraph::new(
                    ParaProps::default(),
                    vec![Run::text(*t, RunProps::default())],
                ))
            })
            .collect();
        Session::new(Document {
            sections: vec![Section::new(blocks)],
            ..Document::default()
        })
    }

    fn text(s: &Session, i: usize) -> String {
        match &s.doc().sections[0].blocks[i] {
            Block::Paragraph(p) => p.plain_text(),
            Block::Table(_) | Block::Opaque(_) => String::new(),
        }
    }

    #[test]
    fn drop_cap_create_change_remove_and_undo() {
        let mut s = session(&["Ngày xưa có một ông vua.", "Đoạn hai."]);
        let spec = DropCapSpec {
            position: DropCap::Drop,
            font: Some("Cambria".into()),
            lines: 3,
            distance: 72,
        };
        s.set_drop_cap(Some(&spec)).unwrap();
        assert_eq!(s.doc().sections[0].blocks.len(), 3);
        assert_eq!(text(&s, 0), "N");
        assert_eq!(text(&s, 1), "gày xưa có một ông vua.");
        let Block::Paragraph(cap) = &s.doc().sections[0].blocks[0] else {
            panic!()
        };
        assert_eq!(
            cap.props.frame,
            Some(FrameProps {
                drop_cap: DropCap::Drop,
                lines: 3,
                h_space: Twips(72)
            })
        );
        // 11 pt × 3 lines × 1.1 = 36.5 pt (73 half-points).
        assert_eq!(cap.runs[0].props.size, Some(HalfPoints(73)));
        assert_eq!(cap.runs[0].props.font.as_deref(), Some("Cambria"));
        assert_eq!(s.selection().focus.para, 1);
        let st = s.page_layout_state();
        assert_eq!(st.drop_cap.as_ref().map(|d| d.lines), Some(3));
        // In the margin, 2 lines: the same paragraph changes.
        s.set_drop_cap(Some(&DropCapSpec {
            position: DropCap::Margin,
            lines: 2,
            ..spec.clone()
        }))
        .unwrap();
        assert_eq!(s.doc().sections[0].blocks.len(), 3);
        assert_eq!(
            s.page_layout_state().drop_cap.unwrap().position,
            DropCap::Margin
        );
        s.set_drop_cap(None).unwrap();
        assert_eq!(s.doc().sections[0].blocks.len(), 2);
        assert_eq!(text(&s, 0), "Ngày xưa có một ông vua.");
        // Each change is one undo step.
        assert!(s.undo());
        assert_eq!(s.doc().sections[0].blocks.len(), 3);
        assert!(s.undo());
        assert!(s.undo());
        assert_eq!(text(&s, 0), "Ngày xưa có một ông vua.");
        assert_eq!(s.doc().sections[0].blocks.len(), 2);
    }

    #[test]
    fn columns_scopes() {
        let mut s = session(&["Một", "Hai", "Ba"]);
        let spec = ColumnsSpec {
            count: 2,
            space: 720,
            separator: true,
            widths: vec![
                ColumnWidth {
                    width: 3000,
                    space: 500,
                },
                ColumnWidth {
                    width: 5000,
                    space: 0,
                },
            ],
        };
        s.set_columns(&spec, LayoutScope::Section).unwrap();
        let c = &s.doc().sections[0].columns;
        assert!(c.separator && c.unequal());
        assert_eq!(c.cols[1].width, Twips(5000));
        // Equal widths are stored as equal columns.
        let eq = ColumnsSpec {
            count: 3,
            space: 360,
            separator: false,
            widths: Vec::new(),
        };
        s.set_columns(&eq, LayoutScope::Document).unwrap();
        assert_eq!(s.doc().sections[0].columns, Columns::equal(3, Twips(360)));
        // From the second paragraph on: a continuous section starts there.
        s.set_selection(Selection::caret(Pos { para: 1, offset: 0 }));
        s.set_columns(&spec, LayoutScope::Forward).unwrap();
        let d = s.doc();
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[0].blocks.len(), 1);
        assert_eq!(d.sections[1].break_type, SectionBreak::Continuous);
        assert!(d.sections[1].columns.unequal());
        assert_eq!(d.sections[0].columns.len(), 3);
        assert!(s.undo());
        assert_eq!(s.doc().sections.len(), 1);
    }

    #[test]
    fn line_numbers_direction_and_hyphenation() {
        let mut s = session(&["Một"]);
        s.set_line_numbering(
            Some(LineNumbering {
                count_by: 0,
                ..LineNumbering::default()
            }),
            SetupScope::Section,
        )
        .unwrap();
        assert_eq!(s.doc().sections[0].line_numbers.unwrap().count_by, 1);
        s.set_section_text_direction(Some(TextDirection::BtLr), SetupScope::Document)
            .unwrap();
        assert_eq!(
            s.page_layout_state().text_direction.as_deref(),
            Some("btLr")
        );
        s.set_section_text_direction(Some(TextDirection::LrTb), SetupScope::Document)
            .unwrap();
        assert_eq!(s.doc().sections[0].text_direction, None);
        s.set_hyphenation(Hyphenation {
            auto: true,
            consecutive_limit: Some(0),
            ..Hyphenation::default()
        })
        .unwrap();
        assert!(s.doc().hyphenation.auto);
        assert_eq!(s.doc().hyphenation.consecutive_limit, None);
        assert!(s.undo());
        assert!(!s.doc().hyphenation.auto);
        assert!(s.redo());
        assert!(s.doc().hyphenation.auto);
    }
}
