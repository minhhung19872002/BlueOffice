//! Formatting marks of headers and footers (BlueDoc round 9): with Show ¶ on, the desktop word
//! processor draws ¶ after each header / footer paragraph and · for its spaces, like the body's
//! (lighter while the body is edited: the view's veil over the margins covers them too). Header
//! and footer lines carry no paragraph id in the page layout (`LineSource::Other`); a line whose
//! covered range starts at 0 begins a paragraph (ECMA-376 Part 1, §17.10.3 hdr / §17.10.2 ftr
//! content flows as ordinary paragraphs), so the line before the next such line ends one.
use crate::views::{FormatMark, FormatMarkKind};
use bo_layout::{Line, LineSource, Page};

/// Lines of the page's header and footer, in placement order, each zone on its own. The header
/// starts above the top margin and may grow past it (the body then starts lower); the footer
/// ends below the bottom margin and may grow upwards. Repeated table header rows (also
/// `Other`) sit inside the body area and are left out.
fn zones(page: &Page, margin_top: f32, margin_bottom: f32) -> [Vec<&Line>; 2] {
    let body = |l: &&Line| matches!(l.source, LineSource::Body(_));
    let first_body = page
        .lines
        .iter()
        .filter(body)
        .map(|l| l.top)
        .reduce(f32::min);
    let last_body = page
        .lines
        .iter()
        .filter(body)
        .map(|l| l.top + l.height)
        .reduce(f32::max);
    let mut other: Vec<&Line> = page
        .lines
        .iter()
        .filter(|l| l.source == LineSource::Other && l.rotation.is_none())
        .collect();
    other.sort_by(|a, b| a.top.total_cmp(&b.top));
    // Header: from the lines starting above the top margin, down through the lines that
    // continue them (no larger gap than a blank line) above the body.
    let mut header: Vec<&Line> = Vec::new();
    for l in &other {
        let continues = header.last().is_some_and(|p| {
            l.top <= p.top + 2.0 * p.height.max(12.0) && first_body.is_none_or(|b| l.top < b)
        });
        if l.top < margin_top - 0.5 || continues {
            header.push(l);
        } else if !header.is_empty() {
            break;
        }
    }
    // Footer: the same from the bottom up.
    let bottom = page.height - margin_bottom;
    let mut footer: Vec<&Line> = Vec::new();
    for l in other.iter().rev() {
        let continues = footer.last().is_some_and(|n| {
            l.top + l.height >= n.top - n.height.max(12.0) && last_body.is_none_or(|b| l.top >= b)
        });
        if (l.top >= bottom - 0.5 && !header.iter().any(|h| std::ptr::eq(*h, *l))) || continues {
            footer.push(l);
        } else if !footer.is_empty() {
            break;
        }
    }
    footer.reverse();
    [header, footer]
}

/// Marks of the header and footer lines of `page` (`margin_top` / `margin_bottom` of its
/// section, points).
pub(crate) fn hf_marks(page: &Page, margin_top: f32, margin_bottom: f32) -> Vec<FormatMark> {
    let mut out = Vec::new();
    for lines in zones(page, margin_top, margin_bottom) {
        for (i, l) in lines.iter().enumerate() {
            let size = l.fragments.last().map_or(10.0, |f| f.size);
            // Spaces of the line's runs.
            for f in &l.fragments {
                for (gi, g) in f.glyphs.iter().enumerate() {
                    let at = g.cluster as usize;
                    if f.text.get(at..).is_some_and(|t| t.starts_with(' ')) {
                        let x0 = f.x + g.x;
                        let x1 = f.glyphs.get(gi + 1).map_or(f.x + f.width, |n| f.x + n.x);
                        out.push(FormatMark {
                            kind: FormatMarkKind::Space,
                            x: x0,
                            w: (x1 - x0).max(0.0),
                            top: l.top,
                            h: l.height,
                            baseline: l.baseline,
                            size: f.size,
                        });
                    }
                }
            }
            let ends = lines.get(i + 1).is_none_or(|n| n.start == 0);
            if ends {
                let text_end = l.fragments.iter().map(|f| f.x + f.width).reduce(f32::max);
                let caret_end = l.carets.last().map(|c| c.1);
                let x = match (text_end, caret_end) {
                    (Some(t), Some(c)) => t.max(c),
                    (Some(t), None) => t,
                    (None, Some(c)) => c,
                    (None, None) => continue,
                };
                out.push(FormatMark {
                    kind: FormatMarkKind::Paragraph,
                    x,
                    w: 0.0,
                    top: l.top,
                    h: l.height,
                    baseline: l.baseline,
                    size,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::edit::Command;
    use crate::views::FormatMarkKind;
    use crate::Engine;

    #[test]
    fn header_paragraphs_end_with_a_mark_and_spaces_show() {
        let mut e = Engine::with_bundled_fonts();
        e.new_document("a.docx").expect("new");
        e.command(Command::InsertText {
            text: "Body".into(),
        })
        .expect("body");
        e.command(Command::EditHeaderFooter {
            page: 0,
            header: true,
        })
        .expect("open header");
        e.command(Command::InsertText {
            text: "Top line".into(),
        })
        .expect("type");
        e.command(Command::SplitParagraph).expect("enter");
        e.command(Command::InsertText {
            text: "Second".into(),
        })
        .expect("type");
        e.command(Command::CloseHeaderFooter).expect("close");
        e.command(Command::ShowFormattingMarks {
            on: true,
            kinds: None,
        })
        .expect("marks on");
        let marks = e.formatting_marks(0);
        // Header lines at 36 and 51 pt, the body below.
        let header: Vec<_> = marks.iter().filter(|m| m.top < 60.0).collect();
        let paras = header
            .iter()
            .filter(|m| m.kind == FormatMarkKind::Paragraph)
            .count();
        let spaces = header
            .iter()
            .filter(|m| m.kind == FormatMarkKind::Space)
            .count();
        assert_eq!((paras, spaces), (2, 1), "{marks:?}");
        // The body's own mark is still there.
        assert!(
            marks
                .iter()
                .any(|m| m.top >= 60.0 && m.kind == FormatMarkKind::Paragraph),
            "{marks:?}"
        );
    }
}
