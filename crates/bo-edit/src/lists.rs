//! Multilevel lists: define a new list, link its levels to paragraph styles and set
//! the numbering value (restart / continue).
//!
//! ECMA-376 Part 1: §17.9.1 abstractNum, §17.9.6 lvl, §17.9.11 lvlText, §17.9.17
//! numFmt, §17.9.25 start, §17.9.23 pStyle, §17.9.15 num, §17.9.8 lvlOverride,
//! §17.9.26 startOverride; paragraph and style numbering §17.3.1.19 numPr.

use crate::{builtin_style, style_para_props, EditError, Op, Result, Selection, Session};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{
    AbstractNum, Alignment, Document, Level, Num, NumFormat, NumRef, ParaProps, Paragraph,
    RunProps, StyleKind, Twips,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One level of a new multilevel list, as sent by hosts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLevelSpec {
    /// Number format (§17.9.17 numFmt, `ST_NumberFormat` §17.18.59): `decimal`,
    /// `decimalZero`, `lowerLetter`, `upperLetter`, `lowerRoman`, `upperRoman`,
    /// `bullet` or `none`.
    pub format: String,
    /// Level text (§17.9.11 lvlText): `%1` … `%9` stand for the levels' numbers,
    /// e.g. `%1.%2.`; for a bullet the bullet character.
    pub text: String,
    /// Start value (§17.9.25 start).
    #[serde(default = "one")]
    pub start: i32,
    /// Left indent of the level's paragraphs in twips (§17.3.1.12 ind @w:left).
    #[serde(default)]
    pub indent: i32,
    /// Hanging indent in twips (§17.3.1.12 ind @w:hanging): the number sits this far
    /// left of the text.
    #[serde(default)]
    pub hanging: i32,
    /// Paragraph style linked to the level (§17.9.23 pStyle), e.g. `Heading1`.
    #[serde(default)]
    pub style: Option<String>,
}

fn one() -> i32 {
    1
}

/// "Set numbering value" for the list paragraph at the caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ListNumbering {
    /// Start a new list here whose current level counts from `value`
    /// (a new §17.9.15 num with a §17.9.26 startOverride).
    Restart {
        /// Number the paragraph shows.
        value: i32,
    },
    /// Join the nearest list above (of the same kind: numbered or bulleted).
    Continue,
}

/// Number formats a new list level may use (§17.18.59 subset BlueDoc can draw).
const FORMATS: [&str; 8] = [
    "decimal",
    "decimalZero",
    "lowerLetter",
    "upperLetter",
    "lowerRoman",
    "upperRoman",
    "bullet",
    "none",
];

/// Largest start value accepted (Word-compatible readers cap it at 32767).
const MAX_START: i32 = 32767;

/// Default level `i` of a new multilevel list: `1.`, `1.1.`, `1.1.1.` …
pub fn default_level(i: u8) -> ListLevelSpec {
    let mut text = String::new();
    for k in 1..=i + 1 {
        text.push('%');
        text.push_str(&k.to_string());
        text.push('.');
    }
    ListLevelSpec {
        format: "decimal".into(),
        text,
        start: 1,
        indent: 720 * (i32::from(i) + 1),
        // Longer labels (`1.1.1.`) need a wider hanging indent.
        hanging: 360 + 144 * i32::from(i),
        style: None,
    }
}

/// The list a paragraph is numbered with: its own `numPr`, else its style's with the
/// level linked to that style (§17.9.23 pStyle). `numId` 0 means "not numbered".
pub fn effective_num(doc: &Document, p: &Paragraph) -> Option<NumRef> {
    if let Some(n) = p.props.num {
        return (n.num_id != 0).then_some(n);
    }
    let n = style_para_props(doc, &p.props)
        .num
        .filter(|n| n.num_id != 0)?;
    let style = p.props.style.clone().or_else(|| {
        doc.styles
            .default_of(StyleKind::Paragraph)
            .map(|s| s.id.clone())
    });
    Some(match style {
        Some(id) => doc.numbering.level_for_style(n, &id),
        None => n,
    })
}

fn is_bullet(doc: &Document, n: NumRef) -> bool {
    doc.numbering
        .level(n)
        .is_some_and(|l| l.format == NumFormat::Bullet)
}

/// Build the 9 levels of a new abstract numbering from `specs` (missing levels get
/// [`default_level`]); a style is linked to its first level only.
fn build_levels(specs: &[ListLevelSpec]) -> Result<Vec<Level>> {
    if specs.len() > 9 {
        return Err(EditError::Unsupported("a list has at most 9 levels".into()));
    }
    let mut linked = BTreeSet::new();
    (0u8..9)
        .map(|i| {
            let spec = specs
                .get(usize::from(i))
                .cloned()
                .unwrap_or_else(|| default_level(i));
            if !FORMATS.contains(&spec.format.as_str()) {
                return Err(EditError::Unsupported(format!(
                    "number format {}",
                    spec.format
                )));
            }
            let indent = spec.indent.clamp(0, 31680);
            let hanging = spec.hanging.clamp(0, indent);
            let pstyle = spec
                .style
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty() && linked.insert(s.clone()));
            Ok(Level {
                ilvl: i,
                start: spec.start.clamp(0, MAX_START),
                format: NumFormat::parse(&spec.format),
                // §17.9.11 lvlText: at most 255 characters is plenty for a label.
                text: spec.text.chars().take(255).collect(),
                align: Alignment::Left,
                // §17.9.6 lvl/pPr: ind of the level's paragraphs.
                ppr: ParaProps {
                    indent_left: Some(Twips(indent)),
                    indent_first_line: Some(Twips(-hanging)),
                    ..ParaProps::default()
                },
                rpr: RunProps::default(),
                restart: None,
                suffix: "tab".into(),
                pstyle,
            })
        })
        .collect()
}

impl Session {
    /// Define a new multilevel list (§17.9.1 abstractNum with 9 §17.9.6 lvl and a
    /// §17.9.15 num) and return its `numId`. Levels with a `style` link that paragraph
    /// style (§17.9.23 pStyle + the style's §17.3.1.19 numPr; missing built-in heading
    /// styles are added), so paragraphs of the style are numbered at that level; a
    /// style linked elsewhere before is unlinked there. With `apply`, the touched
    /// paragraphs join the list: paragraphs of a linked style through their style,
    /// others at their current list level (or level 1). One undo step.
    pub fn define_list(&mut self, specs: &[ListLevelSpec], apply: bool) -> Result<i32> {
        let levels = build_levels(specs)?;
        let links: Vec<(String, u8)> = levels
            .iter()
            .filter_map(|l| l.pstyle.clone().map(|s| (s, l.ilvl)))
            .collect();
        let old_num = self.doc.numbering.clone();
        let mut new_num = old_num.clone();
        let abs_id = new_num.next_abstract_id();
        let num_id = new_num.next_num_id();
        // A style is linked to one list level only: drop older links.
        for a in &mut new_num.abstracts {
            for l in &mut a.levels {
                if l.pstyle
                    .as_ref()
                    .is_some_and(|s| links.iter().any(|(id, _)| id == s))
                {
                    l.pstyle = None;
                }
            }
        }
        new_num.abstracts.push(AbstractNum { id: abs_id, levels });
        new_num.nums.push(Num {
            id: num_id,
            abstract_id: abs_id,
            start_overrides: Vec::new(),
        });
        let old_styles = self.doc.styles.clone();
        let mut new_styles = old_styles.clone();
        for (id, ilvl) in &links {
            if !new_styles.styles.iter().any(|s| &s.id == id) {
                let Some(b) = builtin_style(id) else {
                    return Err(EditError::Unsupported(format!("unknown style {id}")));
                };
                if b.based_on.as_deref() == Some("Normal")
                    && !new_styles.styles.iter().any(|s| s.id == "Normal")
                {
                    if let Some(n) = builtin_style("Normal") {
                        new_styles.styles.push(n);
                    }
                }
                new_styles.styles.push(b);
            }
            if let Some(st) = new_styles.styles.iter_mut().find(|s| &s.id == id) {
                if st.kind != StyleKind::Paragraph {
                    return Err(EditError::Unsupported(format!(
                        "{id} is not a paragraph style"
                    )));
                }
                // §17.3.1.19 numPr in the style's pPr (ilvl written for clarity; the
                // linked level decides, §17.9.23).
                st.ppr.num = Some(NumRef {
                    num_id,
                    ilvl: *ilvl,
                });
            }
        }
        let ids: Vec<usize> = if apply {
            let (a, b) = self.selection.ordered();
            (a.para..=b.para).collect()
        } else {
            Vec::new()
        };
        self.edit(move |tx, sel| {
            tx.push(Op::Numbering {
                old: Box::new(old_num),
                new: Box::new(new_num),
            })?;
            if new_styles != old_styles {
                tx.push(Op::Styles {
                    old: Box::new(old_styles),
                    new: Box::new(new_styles),
                })?;
            }
            for id in ids {
                let (path, p, _) = tx.atoms(id)?;
                let style = p.props.style.clone().or_else(|| {
                    tx.doc
                        .styles
                        .default_of(StyleKind::Paragraph)
                        .map(|s| s.id.clone())
                });
                let mut q = p.clone();
                if style.is_some_and(|s| links.iter().any(|(id, _)| *id == s)) {
                    // Numbered through the linked style.
                    q.props.num = None;
                } else {
                    let ilvl = p.props.num.filter(|n| n.num_id != 0).map_or(0, |n| n.ilvl);
                    q.props.num = Some(NumRef { num_id, ilvl });
                }
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })?;
        Ok(num_id)
    }

    /// Set the numbering value of the list paragraph at the caret (Restart: a new
    /// §17.9.15 num on the same abstract with a §17.9.8 lvlOverride / §17.9.26
    /// startOverride; Continue: join the nearest list above). The paragraph and the
    /// following paragraphs of its list move to that list. One undo step.
    pub fn set_list_numbering(&mut self, how: ListNumbering) -> Result<Selection> {
        let at = self.selection.ordered().0.para;
        let paths = body_paragraphs(&self.doc);
        let nums: Vec<Option<NumRef>> = paths
            .iter()
            .map(|p| para_at(&self.doc, p).and_then(|q| effective_num(&self.doc, q)))
            .collect();
        let Some(cur) = nums.get(at).copied().flatten() else {
            return Err(EditError::Unsupported("not in a list".into()));
        };
        let old_numbering = self.doc.numbering.clone();
        let mut new_numbering = old_numbering.clone();
        let target = match how {
            ListNumbering::Restart { value } => {
                let abs = old_numbering
                    .abstract_of(cur.num_id)
                    .ok_or_else(|| EditError::Unsupported("unknown list".into()))?;
                let id = new_numbering.next_num_id();
                new_numbering.nums.push(Num {
                    id,
                    abstract_id: abs,
                    start_overrides: vec![(cur.ilvl, value.clamp(0, MAX_START))],
                });
                id
            }
            ListNumbering::Continue => {
                let bullet = is_bullet(&self.doc, cur);
                nums[..at]
                    .iter()
                    .rev()
                    .flatten()
                    .find(|n| n.num_id != cur.num_id && is_bullet(&self.doc, **n) == bullet)
                    .map(|n| n.num_id)
                    .ok_or_else(|| EditError::Unsupported("no list above to continue".into()))?
            }
        };
        let moved: Vec<(usize, u8)> = nums
            .iter()
            .enumerate()
            .skip(at)
            .filter_map(|(i, n)| n.filter(|n| n.num_id == cur.num_id).map(|n| (i, n.ilvl)))
            .collect();
        self.edit(move |tx, sel| {
            if new_numbering != old_numbering {
                tx.push(Op::Numbering {
                    old: Box::new(old_numbering),
                    new: Box::new(new_numbering),
                })?;
            }
            for (id, ilvl) in moved {
                let (path, p, _) = tx.atoms(id)?;
                let mut q = p.clone();
                q.props.num = Some(NumRef {
                    num_id: target,
                    ilvl,
                });
                tx.replace_para(&path, q)?;
            }
            Ok(sel)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pos, Selection};
    use bo_model::{Block, Run, Section};

    fn doc(texts: &[(&str, Option<&str>)]) -> Document {
        let blocks = texts
            .iter()
            .map(|(t, st)| {
                let props = ParaProps {
                    style: st.map(str::to_owned),
                    ..ParaProps::default()
                };
                Block::Paragraph(Paragraph::new(
                    props,
                    vec![Run::text(*t, RunProps::default())],
                ))
            })
            .collect();
        Document {
            sections: vec![Section::new(blocks)],
            ..Default::default()
        }
    }

    fn para(s: &Session, i: usize) -> Paragraph {
        let paths = body_paragraphs(s.doc());
        para_at(s.doc(), &paths[i]).cloned().unwrap_or_default()
    }

    #[test]
    fn defines_nine_levels_and_links_heading_styles() {
        let mut s = Session::new(doc(&[("A", Some("Heading1")), ("B", None)]));
        let specs = vec![
            ListLevelSpec {
                style: Some("Heading1".into()),
                ..default_level(0)
            },
            ListLevelSpec {
                style: Some("Heading2".into()),
                ..default_level(1)
            },
            ListLevelSpec {
                format: "lowerLetter".into(),
                text: "%3)".into(),
                ..default_level(2)
            },
        ];
        let id = s.define_list(&specs, false).unwrap();
        let d = s.doc();
        let abs = d.numbering.abstracts.last().unwrap();
        assert_eq!(abs.levels.len(), 9);
        assert_eq!(abs.levels[1].text, "%1.%2.");
        assert_eq!(abs.levels[2].format, NumFormat::LowerLetter);
        assert_eq!(abs.levels[0].pstyle.as_deref(), Some("Heading1"));
        assert_eq!(abs.levels[8].text, "%1.%2.%3.%4.%5.%6.%7.%8.%9.");
        // Heading2 was missing: added as a built-in style, linked through numPr.
        let h2 = d.style("Heading2").unwrap();
        assert_eq!(
            h2.ppr.num,
            Some(NumRef {
                num_id: id,
                ilvl: 1
            })
        );
        assert_eq!(
            effective_num(d, &para(&s, 0)),
            Some(NumRef {
                num_id: id,
                ilvl: 0
            })
        );
        assert_eq!(effective_num(d, &para(&s, 1)), None);
        // One undo step restores numbering and styles.
        assert!(s.undo());
        assert_eq!(s.doc().numbering.abstracts.len(), 0);
        assert!(s.doc().style("Heading2").is_none());
    }

    #[test]
    fn rejects_bad_formats_and_too_many_levels() {
        let mut s = Session::new(doc(&[("A", None)]));
        let bad = ListLevelSpec {
            format: "cardinalText".into(),
            ..default_level(0)
        };
        assert!(s.define_list(&[bad], true).is_err());
        assert!(s.define_list(&vec![default_level(0); 10], true).is_err());
        assert_eq!(s.doc().numbering.nums.len(), 0);
    }

    #[test]
    fn relinking_a_style_drops_the_old_link() {
        let mut s = Session::new(doc(&[("A", Some("Heading1"))]));
        let link = |st: &str| ListLevelSpec {
            style: Some(st.into()),
            ..default_level(0)
        };
        let first = s.define_list(&[link("Heading1")], false).unwrap();
        let second = s.define_list(&[link("Heading1")], false).unwrap();
        assert_ne!(first, second);
        let d = s.doc();
        assert!(d.numbering.abstracts[0].levels[0].pstyle.is_none());
        assert_eq!(
            d.numbering.abstracts[1].levels[0].pstyle.as_deref(),
            Some("Heading1")
        );
        assert_eq!(
            effective_num(d, &para(&s, 0)).map(|n| n.num_id),
            Some(second)
        );
    }

    #[test]
    fn applies_to_selection_and_restarts_then_continues() {
        let mut s = Session::new(doc(&[("a", None), ("b", None), ("c", None), ("d", None)]));
        s.set_selection(Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(3, 0),
        });
        let id = s.define_list(&[], true).unwrap();
        for i in 0..4 {
            assert_eq!(
                para(&s, i).props.num,
                Some(NumRef {
                    num_id: id,
                    ilvl: 0
                })
            );
        }
        s.set_selection(Selection::caret(Pos::new(2, 0)));
        s.set_list_numbering(ListNumbering::Restart { value: 5 })
            .unwrap();
        let d = s.doc();
        let new = d.numbering.nums.last().unwrap();
        assert_eq!(new.start_overrides, vec![(0, 5)]);
        assert_eq!(para(&s, 1).props.num.map(|n| n.num_id), Some(id));
        assert_eq!(para(&s, 2).props.num.map(|n| n.num_id), Some(new.id));
        assert_eq!(para(&s, 3).props.num.map(|n| n.num_id), Some(new.id));
        // Continue joins the list above again.
        s.set_list_numbering(ListNumbering::Continue).unwrap();
        for i in 0..4 {
            assert_eq!(para(&s, i).props.num.map(|n| n.num_id), Some(id));
        }
        assert!(s.undo());
        assert!(s.undo());
        assert_eq!(s.doc().numbering.nums.len(), 1);
        // Not in a list / nothing to continue.
        assert!(s.set_list_numbering(ListNumbering::Continue).is_err());
        assert!(s.undo());
        assert!(s
            .set_list_numbering(ListNumbering::Restart { value: 1 })
            .is_err());
    }
}
