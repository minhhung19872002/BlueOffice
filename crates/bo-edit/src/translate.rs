//! Whole-document translation (CORE-LANG-001): the text the host sends to its translation
//! provider and the write-back of the translated text **keeping the run formatting**.
//!
//! A body paragraph (§17.3.1.22 p) is cut into *segments*: maximal stretches of text
//! (§17.3.3.31 t) that are not interrupted by non-text run content — tabs (§17.3.3.32), breaks
//! (§17.3.3.1), pictures (§17.3.3.9), fields (§17.16), note references (§17.11.14 / §17.11.7),
//! content control boundaries (§17.5.2.29), form fields and content kept verbatim. Zero-width
//! markup (bookmarks, comment ranges, `w:proofErr`) and soft / non-breaking hyphens do not
//! interrupt a segment; deleted runs (§17.13.5.14 del) are not part of any segment and stay as
//! they are. Each segment is translated as one unit, so a sentence whose words carry different
//! formatting (bold, italic, a hyperlink colour…) is translated whole.
//!
//! Writing back, the translated text of a segment is shared among the runs of that segment **in
//! proportion to their original length** (characters), each cut moved to the nearest word
//! boundary, so a run that held a third of the sentence keeps its formatting on about a third of
//! the translation. Paragraph properties (style, numbering, alignment, §17.3.1.26 pPr) are never
//! touched; run properties (§17.3.2.28 rPr) stay on their runs. Our own rule: word order differs
//! between languages, so formatting inside a sentence lands on roughly — not exactly — the
//! matching words.

use crate::{para_clone, Result, Session};
use bo_model::index::body_paragraphs;
use bo_model::{Paragraph, RunContent};
use serde::{Deserialize, Serialize};

/// Text of one segment of a body paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSegment {
    /// Body paragraph id (as `bo_model::index::body_paragraphs`).
    pub p: usize,
    /// Segment index inside the paragraph.
    pub s: usize,
    /// The segment's text.
    pub text: String,
}

/// Does this run content interrupt a segment?
fn breaks_segment(c: &RunContent) -> bool {
    !matches!(
        c,
        RunContent::Text(_)
            | RunContent::Markup(_)
            | RunContent::CommentStart(_)
            | RunContent::CommentEnd(_)
            | RunContent::CommentRef(_)
            | RunContent::SoftHyphen
            | RunContent::NoBreakHyphen
    )
}

/// `(run, content)` positions of the text items of each segment of `p`.
fn segment_pieces(p: &Paragraph) -> Vec<Vec<(usize, usize)>> {
    let mut out: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut cur: Vec<(usize, usize)> = Vec::new();
    for (ri, r) in p.runs.iter().enumerate() {
        // Deleted text stays as it is and is not translated.
        if r.props.del.is_some() {
            continue;
        }
        for (ci, c) in r.content.iter().enumerate() {
            match c {
                RunContent::Text(t) if !t.is_empty() => cur.push((ri, ci)),
                c if breaks_segment(c) && !cur.is_empty() => {
                    out.push(std::mem::take(&mut cur));
                }
                _ => {}
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn text_of(p: &Paragraph, at: (usize, usize)) -> &str {
    match p.runs.get(at.0).and_then(|r| r.content.get(at.1)) {
        Some(RunContent::Text(t)) => t,
        _ => "",
    }
}

/// Segments of every body paragraph, in document order (blank segments included).
pub fn text_segments(doc: &bo_model::Document) -> Vec<TextSegment> {
    let mut out = Vec::new();
    for (id, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = bo_model::index::para_at(doc, path) else {
            continue;
        };
        for (s, pieces) in segment_pieces(p).iter().enumerate() {
            let text: String = pieces.iter().map(|&at| text_of(p, at)).collect();
            out.push(TextSegment { p: id, s, text });
        }
    }
    out
}

/// Share `new` among pieces of the given lengths (characters): each piece gets a part in
/// proportion to its length, cut at the nearest word boundary (after white space) within a small
/// window; the parts concatenate to `new`. One piece takes everything.
pub fn distribute(new: &str, lens: &[usize]) -> Vec<String> {
    if lens.len() <= 1 {
        return vec![new.to_owned(); lens.len()];
    }
    let chars: Vec<char> = new.chars().collect();
    let n = chars.len();
    let total: usize = lens.iter().sum::<usize>().max(1);
    // Search this far (characters) for a word boundary around the proportional cut.
    let window = (n / (lens.len() * 2)).clamp(2, 12);
    let boundary = |j: usize| j == 0 || j == n || chars[j - 1].is_whitespace();
    let mut cuts = Vec::with_capacity(lens.len() + 1);
    cuts.push(0usize);
    let mut cum = 0usize;
    for &len in &lens[..lens.len() - 1] {
        cum += len;
        let prev = *cuts.last().unwrap_or(&0);
        let ideal = ((n * cum + total / 2) / total).clamp(prev, n);
        let mut best = ideal;
        if !boundary(ideal) {
            for d in 1..=window {
                if ideal + d <= n && boundary(ideal + d) {
                    best = ideal + d;
                    break;
                }
                if ideal >= prev + d && boundary(ideal - d) {
                    best = ideal - d;
                    break;
                }
            }
        }
        cuts.push(best.max(prev));
    }
    cuts.push(n);
    cuts.windows(2)
        .map(|w| chars[w[0]..w[1]].iter().collect())
        .collect()
}

/// Replace the text of the segment made of `pieces` in `p` by `text` (shared by proportion).
fn with_segment(p: &mut Paragraph, pieces: &[(usize, usize)], text: &str) {
    let lens: Vec<usize> = pieces
        .iter()
        .map(|&at| text_of(p, at).chars().count())
        .collect();
    for (&(ri, ci), part) in pieces.iter().zip(distribute(text, &lens)) {
        if let Some(c) = p.runs.get_mut(ri).and_then(|r| r.content.get_mut(ci)) {
            *c = RunContent::Text(part);
        }
    }
}

impl Session {
    /// Replace the text of segments (see the module documentation), one undo step; returns how
    /// many segments changed. Unknown paragraph / segment indices are ignored. The change is not
    /// tracked: the translated document is a new copy.
    pub fn apply_translation(&mut self, segments: &[TextSegment]) -> Result<usize> {
        let mut changed = 0;
        self.edit(|tx, _| {
            let paths = tx.paths();
            let mut by_para: std::collections::BTreeMap<usize, Vec<&TextSegment>> =
                std::collections::BTreeMap::new();
            for seg in segments {
                by_para.entry(seg.p).or_default().push(seg);
            }
            for (id, segs) in by_para {
                let Some(path) = paths.get(id) else {
                    continue;
                };
                let old = para_clone(tx.doc, path)?;
                let all = segment_pieces(&old);
                let mut np = old.clone();
                let mut touched = std::collections::BTreeSet::new();
                for seg in segs {
                    let Some(pieces) = all.get(seg.s) else {
                        continue;
                    };
                    let before: String = pieces.iter().map(|&at| text_of(&old, at)).collect();
                    if before == seg.text {
                        continue;
                    }
                    with_segment(&mut np, pieces, &seg.text);
                    touched.extend(pieces.iter().map(|&(r, _)| r));
                    changed += 1;
                }
                if touched.is_empty() {
                    continue;
                }
                // Runs whose text went to their neighbours disappear.
                for &r in &touched {
                    if let Some(run) = np.runs.get_mut(r) {
                        run.content
                            .retain(|c| !matches!(c, RunContent::Text(t) if t.is_empty()));
                    }
                }
                let mut i = 0;
                np.runs.retain(|run| {
                    let keep = !touched.contains(&i) || !run.content.is_empty();
                    i += 1;
                    keep
                });
                tx.replace_para(path, np)?;
            }
            Ok(crate::Selection::caret(crate::Pos::new(0, 0)))
        })?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::{Block, Document, Run, RunProps, Section};

    fn bold() -> RunProps {
        RunProps {
            bold: Some(true),
            ..RunProps::default()
        }
    }

    fn doc(paras: Vec<Paragraph>) -> Document {
        Document {
            sections: vec![Section::new(
                paras.into_iter().map(Block::Paragraph).collect(),
            )],
            ..Default::default()
        }
    }

    #[test]
    fn distribute_keeps_all_text_and_cuts_at_words() {
        let parts = distribute("hello brave new world", &[6, 6]);
        assert_eq!(parts.concat(), "hello brave new world");
        assert_eq!(parts.len(), 2);
        assert!(parts[0].ends_with(' '), "{parts:?}");
        assert_eq!(distribute("abc", &[3]), vec!["abc"]);
        assert_eq!(distribute("x", &[]), Vec::<String>::new());
        // More pieces than characters: some parts are empty, nothing is lost.
        let p = distribute("ab", &[1, 1, 1, 1]);
        assert_eq!(p.concat(), "ab");
        assert_eq!(p.len(), 4);
        // Multi-byte text is cut at character boundaries.
        let p = distribute("xin chào thế giới", &[1, 3]);
        assert_eq!(p.concat(), "xin chào thế giới");
    }

    #[test]
    fn segments_stop_at_tabs_and_skip_deleted_runs() {
        let deleted = RunProps {
            del: Some(bo_model::RevMark::default()),
            ..RunProps::default()
        };
        let p = Paragraph::new(
            bo_model::ParaProps::default(),
            vec![
                Run::text("Tên ", RunProps::default()),
                Run::text("đầy đủ", bold()),
                Run {
                    props: RunProps::default(),
                    content: vec![RunContent::Tab, RunContent::Text("Giá trị".into())],
                },
                Run::text("cũ", deleted),
            ],
        );
        let segs = text_segments(&doc(vec![p]));
        let texts: Vec<_> = segs.iter().map(|s| (s.p, s.s, s.text.as_str())).collect();
        assert_eq!(texts, vec![(0, 0, "Tên đầy đủ"), (0, 1, "Giá trị")]);
    }

    #[test]
    fn apply_keeps_runs_formatting_and_paragraph_count() {
        let p0 = Paragraph::new(
            bo_model::ParaProps {
                style: Some("Heading1".into()),
                ..bo_model::ParaProps::default()
            },
            vec![Run::text("Báo cáo", RunProps::default())],
        );
        let p1 = Paragraph::new(
            bo_model::ParaProps::default(),
            vec![
                Run::text("Xin chào ", RunProps::default()),
                Run::text("thế giới", bold()),
            ],
        );
        let mut s = Session::new(doc(vec![p0, p1]));
        let n = s
            .apply_translation(&[
                TextSegment {
                    p: 0,
                    s: 0,
                    text: "Report".into(),
                },
                TextSegment {
                    p: 1,
                    s: 0,
                    text: "Hello world".into(),
                },
                // Unknown paragraph / segment: ignored.
                TextSegment {
                    p: 9,
                    s: 0,
                    text: "x".into(),
                },
                TextSegment {
                    p: 1,
                    s: 5,
                    text: "x".into(),
                },
            ])
            .unwrap();
        assert_eq!(n, 2);
        assert_eq!(s.paragraph_count(), 2);
        let segs = text_segments(s.doc());
        assert_eq!(segs[0].text, "Report");
        assert_eq!(segs[1].text, "Hello world");
        let Block::Paragraph(h) = &s.doc().sections[0].blocks[0] else {
            panic!("paragraph")
        };
        assert_eq!(h.props.style.as_deref(), Some("Heading1"));
        let Block::Paragraph(b) = &s.doc().sections[0].blocks[1] else {
            panic!("paragraph")
        };
        assert_eq!(b.runs.len(), 2);
        assert_eq!(b.runs[0].plain_text(), "Hello ");
        assert_eq!(b.runs[1].plain_text(), "world");
        assert_eq!(b.runs[1].props.bold, Some(true));
        // One undo step restores the original.
        assert!(s.undo());
        assert_eq!(text_segments(s.doc())[1].text, "Xin chào thế giới");
    }

    #[test]
    fn runs_left_without_text_are_dropped() {
        let p = Paragraph::new(
            bo_model::ParaProps::default(),
            vec![
                Run::text("aaaaaaaaaa", RunProps::default()),
                Run::text("b", bold()),
            ],
        );
        let mut s = Session::new(doc(vec![p]));
        s.apply_translation(&[TextSegment {
            p: 0,
            s: 0,
            text: "z".into(),
        }])
        .unwrap();
        let Block::Paragraph(b) = &s.doc().sections[0].blocks[0] else {
            panic!("paragraph")
        };
        assert_eq!(b.runs.len(), 1);
        assert_eq!(b.runs[0].plain_text(), "z");
    }
}
