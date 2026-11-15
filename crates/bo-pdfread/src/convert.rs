//! What BlueEditor's conversions to Office formats read from a page: paragraph blocks with
//! their character styles ([`StyledBlock`]), and the page drawn without its text (graphics
//! and pictures only) so a slide can show it behind editable text boxes.
use crate::objects::{deleted_run, ObjectKind};
use crate::paragraph::{text_blocks, Align};
use crate::PdfDocument;
use lopdf::{Dictionary, Object, ObjectId, Stream};

/// A run of one style in a [`StyledBlock`].
#[derive(Debug, Clone, PartialEq)]
pub struct StyledRun {
    /// Its text.
    pub text: String,
    /// Font family (subset tag removed).
    pub family: String,
    /// Size in points.
    pub size: f32,
    /// Bold face.
    pub bold: bool,
    /// Italic face.
    pub italic: bool,
    /// Fill colour.
    pub color: [u8; 3],
}

/// A paragraph of a page with styled runs, for conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledBlock {
    /// Left, top, right, bottom in page points.
    pub bbox: [f32; 4],
    /// Alignment of its lines.
    pub align: Align,
    /// Number of lines on the page.
    pub lines: usize,
    /// First-line indent in points (along the baseline).
    pub indent: f32,
    /// Baseline-to-baseline distance in points (0 for one line).
    pub pitch: f32,
    /// Runs in reading order.
    pub runs: Vec<StyledRun>,
}

/// Page contents swapped out while the page is drawn without text.
#[derive(Debug, Default)]
pub(crate) struct HiddenText {
    /// Page dictionary and its `/Contents` before.
    contents: Vec<(ObjectId, Option<Object>)>,
    /// Streams added for it.
    added: Vec<ObjectId>,
    changed: usize,
}

impl PdfDocument {
    /// Paragraph blocks of page `index` with their runs' styles.
    pub fn styled_blocks(&mut self, index: usize) -> Result<Vec<StyledBlock>, String> {
        let objs = self.page_objects(index)?;
        let blocks = text_blocks(&objs);
        let mut out = Vec::with_capacity(blocks.len());
        for b in blocks {
            let mut runs: Vec<StyledRun> = Vec::new();
            let mut last_run = usize::MAX;
            let mut style: Option<crate::paragraph::RunStyle> = None;
            for (c, &r) in b.text.chars().zip(&b.runs) {
                if r != last_run {
                    last_run = r;
                    style = objs.get(r).and_then(|o| self.style_of_run(index, &o.rec));
                }
                let Some(st) = &style else { continue };
                let same = runs.last().is_some_and(|l| {
                    l.family == st.family
                        && (l.size - st.size).abs() < 0.05
                        && l.bold == st.bold
                        && l.italic == st.italic
                        && l.color == st.color
                });
                if same {
                    if let Some(l) = runs.last_mut() {
                        l.text.push(c);
                    }
                } else {
                    runs.push(StyledRun {
                        text: c.to_string(),
                        family: st.family.clone(),
                        size: (st.size * 2.0).round() / 2.0,
                        bold: st.bold,
                        italic: st.italic,
                        color: st.color,
                    });
                }
            }
            // Indent and pitch from the character boxes: the first character against the
            // block's left edge, and the step between the first two lines.
            let first = b.char_boxes.first().copied().unwrap_or(b.bbox);
            let indent = if b.lines > 1 {
                (first[0] - b.bbox[0]).max(0.0)
            } else {
                0.0
            };
            let pitch = b
                .char_boxes
                .iter()
                .map(|r| r[3])
                .find(|&y| y - first[3] > 0.5 * b.font_size.max(1.0))
                .map_or(0.0, |y| y - first[3]);
            out.push(StyledBlock {
                bbox: b.bbox,
                align: b.align,
                lines: b.lines,
                indent,
                pitch,
                runs,
            });
        }
        Ok(out)
    }

    /// Draw pages without their text (for slides showing the page behind text boxes) until
    /// [`PdfDocument::show_text`]: every page gets content with its text runs neutralised.
    /// Not an edit: undo, the edit count and the next save do not see it.
    pub fn hide_text(&mut self) -> Result<(), String> {
        if self.edit.hidden_text.is_some() {
            return Ok(());
        }
        let mut h = HiddenText {
            changed: self.changed.len(),
            ..HiddenText::default()
        };
        for i in 0..self.pages.len() {
            let content = self.virtual_content(i)?;
            let objs = self.objects_in(i, Some(&content))?;
            let mut edits: Vec<((usize, usize), Vec<u8>)> = objs
                .iter()
                .filter(|o| o.kind == ObjectKind::Text)
                .map(|o| (o.rec.span, deleted_run(&o.rec).into_bytes()))
                .collect();
            edits.sort_by_key(|e| std::cmp::Reverse(e.0 .0));
            let mut c = content;
            for ((a, b), with) in edits {
                if a <= b && b <= c.len() {
                    c.splice(a..b, with);
                }
            }
            let id = self
                .doc
                .add_object(Object::Stream(Stream::new(Dictionary::new(), c)));
            h.added.push(id);
            let page = self.pages[i].id;
            let before = self
                .doc
                .get_dictionary(page)
                .ok()
                .and_then(|d| d.get(b"Contents").ok().cloned());
            h.contents.push((page, before));
            if let Ok(d) = self.doc.get_dictionary_mut(page) {
                d.set("Contents", Object::Reference(id));
            }
        }
        self.edit.hidden_text = Some(h);
        Ok(())
    }

    /// Undo [`PdfDocument::hide_text`].
    pub fn show_text(&mut self) {
        let Some(h) = self.edit.hidden_text.take() else {
            return;
        };
        for (page, before) in h.contents {
            if let Ok(d) = self.doc.get_dictionary_mut(page) {
                match before {
                    Some(c) => d.set("Contents", c),
                    None => {
                        d.remove(b"Contents");
                    }
                }
            }
        }
        for id in h.added {
            self.doc.objects.remove(&id);
        }
        self.changed.truncate(h.changed);
    }
}
