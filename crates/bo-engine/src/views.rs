//! BlueDoc views: print / web / draft layouts, formatting marks, the document
//! outline (outline view and navigation pane) and find results with context.
//!
//! Web and draft layouts lay out a *view copy* of the document: the same blocks
//! (so paragraph ids, carets and edits are unchanged) in sections whose page
//! setup is adjusted — web: the page is as wide as the window and has no
//! headers, footers, page borders or columns; draft: the print page size without
//! headers, footers, page borders or columns. Saving and the model are not affected.

use crate::edit::{Command, EditResult};
use crate::{Engine, Result};
use bo_edit::outline::OutlineEntry;
use bo_edit::Match;
use bo_layout::{Line, LineSource};
use bo_model::index::{blocks_at, body_paragraphs, para_at, para_index, OBJECT};
use bo_model::{Columns, Document, HeaderFooter, RunContent, SectionBreak, Twips};
use serde::{Deserialize, Serialize};

/// Layout used by the BlueDoc view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    /// Pages as printed.
    #[default]
    Print,
    /// Text reflows to the window width; no page decorations.
    Web,
    /// Print page width, no headers/footers or page decorations.
    Draft,
    /// Read mode: text reflows into screen-sized column pages (`width` × `height`),
    /// independent of the page setup; no headers/footers or page decorations.
    Read,
}

/// The view layout and, for the web layout, the window width.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewLayout {
    /// Layout.
    pub mode: ViewMode,
    /// Width of the web layout page (read mode: of a column page) in points.
    pub width: f32,
    /// Height of a read mode column page in points (0 otherwise).
    pub height: f32,
}

/// Narrowest and widest web layout page (points).
const WEB_WIDTH: (f32, f32) = (144.0, 4000.0);
/// Side margins of the web layout (points).
const WEB_SIDE: f32 = 24.0;
/// Top/bottom margins of the web layout (points).
const WEB_TOP: f32 = 12.0;
/// Height of one web layout strip (points): the view stacks strips without gaps.
const WEB_HEIGHT: f32 = 1440.0;
/// Lowest and highest read mode column page (points).
const READ_HEIGHT: (f32, f32) = (144.0, 4000.0);
/// Margins of a read mode column page (points).
const READ_MARGIN: f32 = 24.0;

fn twips(pt: f32) -> Twips {
    Twips((pt * 20.0).round() as i32)
}

/// The view copy laid out for `view` (`None`: the print layout lays out `doc` itself).
pub fn view_document(doc: &Document, view: ViewLayout) -> Option<Document> {
    if view.mode == ViewMode::Print {
        return None;
    }
    let mut d = doc.clone();
    let width = view.width.clamp(WEB_WIDTH.0, WEB_WIDTH.1);
    for (i, s) in d.sections.iter_mut().enumerate() {
        // ECMA-376 Part 1, §17.10 headers/footers and §17.6.10 pgBorders are print decorations.
        s.headers = HeaderFooter::default();
        s.footers = HeaderFooter::default();
        s.title_page = false;
        s.page_borders = None;
        // §17.6.4 cols: one column.
        s.columns = Columns::default();
        if view.mode == ViewMode::Web {
            let p = &mut s.page;
            p.width = twips(width);
            p.height = twips(WEB_HEIGHT);
            p.margin_left = twips(WEB_SIDE.min(width / 8.0));
            p.margin_right = p.margin_left;
            p.margin_top = twips(WEB_TOP);
            p.margin_bottom = twips(WEB_TOP);
            p.header = Twips(0);
            p.footer = Twips(0);
            // §17.6.22 type: sections follow each other without new pages.
            if i > 0 {
                s.break_type = SectionBreak::Continuous;
            }
        }
        if view.mode == ViewMode::Read {
            // §17.6.13 pgSz / §17.6.11 pgMar replaced by the reading column: the text
            // reflows to the screen whatever the printed page size.
            let height = view.height.clamp(READ_HEIGHT.0, READ_HEIGHT.1);
            let margin = READ_MARGIN.min(width / 8.0).min(height / 8.0);
            let p = &mut s.page;
            p.width = twips(width);
            p.height = twips(height);
            p.margin_left = twips(margin);
            p.margin_right = p.margin_left;
            p.margin_top = twips(margin);
            p.margin_bottom = twips(margin);
            p.header = Twips(0);
            p.footer = Twips(0);
            if i > 0 {
                s.break_type = SectionBreak::Continuous;
            }
        }
    }
    Some(d)
}

/// Kind of a formatting mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FormatMarkKind {
    /// End of a paragraph (¶).
    Paragraph,
    /// Space (·).
    Space,
    /// No-break space (°).
    NoBreakSpace,
    /// Tab (→).
    Tab,
    /// Line break (↵, §17.3.3.1 br).
    LineBreak,
    /// Page break (§17.3.3.1 br type="page").
    PageBreak,
    /// Column break.
    ColumnBreak,
    /// Anchor of a floating object (§20.4.2.3 anchor).
    Anchor,
    /// End of a table cell (¤ in place of ¶ after the cell's last paragraph,
    /// §17.4.66 tc).
    CellEnd,
    /// End of a table row (¤ right of the row, §17.4.79 tr).
    RowEnd,
}

/// A formatting mark on a page (points), drawn by the view only (never printed).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatMark {
    /// What the mark shows.
    pub kind: FormatMarkKind,
    /// Left edge.
    pub x: f32,
    /// Width of the marked character (0 for the paragraph mark).
    pub w: f32,
    /// Top of the line.
    pub top: f32,
    /// Height of the line.
    pub h: f32,
    /// Baseline.
    pub baseline: f32,
    /// Font size of the line's text (points).
    pub size: f32,
}

/// An outline entry with the page where the paragraph starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItem {
    /// The entry.
    #[serde(flatten)]
    pub entry: OutlineEntry,
    /// 0-based page index (of the current layout).
    pub page: usize,
}

/// A find match with its context (navigation pane results).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindResult {
    /// The match (select it with `selectMatch`).
    #[serde(rename = "match")]
    pub m: Match,
    /// Text before the match.
    pub before: String,
    /// The matched text (its first paragraph).
    pub text: String,
    /// Text after the match.
    pub after: String,
    /// 0-based page index.
    pub page: usize,
}

/// Gap between a table row's right edge and its end-of-row mark (points).
const ROW_END_GAP: f32 = 2.0;

/// Characters of context on each side of a find result.
const CONTEXT: usize = 32;

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || c == '\u{2028}' {
                ' '
            } else {
                c
            }
        })
        .filter(|&c| c != OBJECT)
        .collect()
}

fn mark(line: &Line, kind: FormatMarkKind, from: usize, to: usize) -> FormatMark {
    let x = crate::nav::caret_x(line, from);
    FormatMark {
        kind,
        x,
        w: (crate::nav::caret_x(line, to) - x).max(0.0),
        top: line.top,
        h: line.height,
        baseline: line.baseline,
        size: line.fragments.last().map_or(10.0, |f| f.size),
    }
}

impl Engine {
    /// The view layout.
    pub fn view_layout(&self) -> ViewLayout {
        self.view_layout
    }

    /// Formatting marks are drawn in the view.
    pub fn shows_formatting_marks(&self) -> bool {
        self.show_marks
    }

    /// Formatting marks of a laid-out page (body text, table cells included).
    pub fn formatting_marks(&self, index: usize) -> Vec<FormatMark> {
        let (Some(s), Some(page)) = (
            self.session.as_ref(),
            self.layout.document().pages.get(index),
        ) else {
            return Vec::new();
        };
        let doc = s.doc();
        let paths = body_paragraphs(doc);
        let mut out = Vec::new();
        let mut cached: Option<(u32, String, Vec<usize>)> = None;
        for line in &page.lines {
            if line.source == LineSource::RowEnd {
                if let Some((_, x)) = line.carets.first() {
                    out.push(FormatMark {
                        kind: FormatMarkKind::RowEnd,
                        x: x + ROW_END_GAP,
                        w: 0.0,
                        top: line.top,
                        h: line.height,
                        baseline: line.baseline,
                        size: (line.height * 0.75).max(6.0),
                    });
                }
                continue;
            }
            let LineSource::Body(id) = line.source else {
                continue;
            };
            if line.rotation.is_some() {
                continue;
            }
            if cached.as_ref().is_none_or(|c| c.0 != id) {
                let Some(p) = paths.get(id as usize).and_then(|path| para_at(doc, path)) else {
                    continue;
                };
                let (text, spans) = para_index(p);
                // Offsets of floating pictures and shapes (anchors).
                let anchors = spans
                    .iter()
                    .filter(|sp| {
                        matches!(
                            p.runs.get(sp.run).and_then(|r| r.content.get(sp.item)),
                            Some(RunContent::Image(img)) if img.anchor.is_some()
                        )
                    })
                    .map(|sp| sp.start)
                    .collect();
                cached = Some((id, text, anchors));
            }
            let Some((_, text, anchors)) = cached.as_ref() else {
                continue;
            };
            let (start, end) = (line.start as usize, (line.end as usize).min(text.len()));
            for (off, ch) in text.get(start..end).unwrap_or("").char_indices() {
                let off = start + off;
                let kind = match ch {
                    ' ' => FormatMarkKind::Space,
                    '\u{00A0}' => FormatMarkKind::NoBreakSpace,
                    '\t' => FormatMarkKind::Tab,
                    '\u{2028}' => FormatMarkKind::LineBreak,
                    '\u{000C}' => FormatMarkKind::PageBreak,
                    '\u{000B}' => FormatMarkKind::ColumnBreak,
                    OBJECT if anchors.contains(&off) => FormatMarkKind::Anchor,
                    _ => continue,
                };
                out.push(mark(line, kind, off, off + ch.len_utf8()));
            }
            if line.end as usize >= text.len() {
                // The last paragraph of a table cell ends the cell: ¤ instead of ¶.
                let cell_end = paths.get(id as usize).is_some_and(|path| {
                    path.len() >= 4
                        && blocks_at(doc, &path[..path.len() - 1])
                            .is_some_and(|b| b.len() == path[path.len() - 1] + 1)
                });
                let kind = if cell_end {
                    FormatMarkKind::CellEnd
                } else {
                    FormatMarkKind::Paragraph
                };
                let mut m = mark(line, kind, text.len(), text.len());
                m.w = 0.0;
                out.push(m);
            }
        }
        out
    }

    /// The outline with the page of each paragraph.
    pub fn outline_items(&self) -> Vec<OutlineItem> {
        let Some(s) = self.session.as_ref() else {
            return Vec::new();
        };
        let mut first_page = std::collections::HashMap::new();
        for (pi, p) in self.layout.document().pages.iter().enumerate() {
            for l in &p.lines {
                if let LineSource::Body(id) = l.source {
                    first_page.entry(id as usize).or_insert(pi);
                }
            }
        }
        s.outline()
            .into_iter()
            .map(|entry| OutlineItem {
                page: first_page.get(&entry.para).copied().unwrap_or(0),
                entry,
            })
            .collect()
    }

    /// Find results with context.
    fn find_results(&self, matches: &[Match]) -> Vec<FindResult> {
        let Some(s) = self.session.as_ref() else {
            return Vec::new();
        };
        let layout = self.layout.document();
        matches
            .iter()
            .map(|m| {
                let t = s.para_text(m.para).unwrap_or_default();
                let start = m.start.min(t.len());
                let end = if m.end_para.is_some() {
                    t.len()
                } else {
                    m.end.clamp(start, t.len())
                };
                let before: String = {
                    let b = t.get(..start).unwrap_or("");
                    let n = b.chars().count();
                    b.chars().skip(n.saturating_sub(CONTEXT)).collect()
                };
                let after: String = t.get(end..).unwrap_or("").chars().take(CONTEXT).collect();
                let page = layout
                    .lines_of(m.para as u32)
                    .find(|(_, l)| (l.start as usize) <= start && start <= l.end as usize)
                    .map_or(0, |(pi, _)| pi);
                FindResult {
                    m: *m,
                    before: clean(&before).trim_start().to_owned(),
                    text: clean(t.get(start..end).unwrap_or("")),
                    after: clean(&after).trim_end().to_owned(),
                    page,
                }
            })
            .collect()
    }

    /// View commands (`None`: not one).
    pub(crate) fn view_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        match cmd {
            Command::SetViewLayout {
                mode,
                width,
                height,
            } => {
                let next = ViewLayout {
                    mode: *mode,
                    width: match mode {
                        ViewMode::Web | ViewMode::Read => width
                            .unwrap_or(self.view_layout.width)
                            .clamp(WEB_WIDTH.0, WEB_WIDTH.1)
                            .round(),
                        _ => 0.0,
                    },
                    height: match mode {
                        ViewMode::Read => height
                            .unwrap_or(self.view_layout.height)
                            .clamp(READ_HEIGHT.0, READ_HEIGHT.1)
                            .round(),
                        _ => 0.0,
                    },
                };
                let mut dirty = Vec::new();
                if next != self.view_layout {
                    self.close_header_footer()?;
                    self.close_note()?;
                    self.close_text_box()?;
                    self.view_layout = next;
                    self.view_changed = true;
                    dirty = self.relayout()?;
                }
                Ok(Some(self.edit_result(dirty, (None, None, None), None)?))
            }
            Command::ShowFormattingMarks { on } => {
                let dirty = if self.show_marks == *on {
                    Vec::new()
                } else {
                    self.show_marks = *on;
                    // Hidden text (§17.3.2.41 vanish) comes and goes with the marks.
                    self.view_changed = true;
                    let mut d = self.relayout()?;
                    d.extend(0..self.layout.document().pages.len());
                    d.sort_unstable();
                    d.dedup();
                    d
                };
                Ok(Some(self.edit_result(dirty, (None, None, None), None)?))
            }
            // Headers and footers are edited on printed pages only.
            Command::EditHeaderFooter { .. } if self.view_layout.mode != ViewMode::Print => {
                Ok(Some(self.edit_result(
                    Vec::new(),
                    (None, None, None),
                    Some("header/footer: print layout only".into()),
                )?))
            }
            Command::Outline => {
                let mut r = self.query_result()?;
                r.outline = Some(self.outline_items());
                Ok(Some(r))
            }
            Command::SetOutlineLevel { .. }
            | Command::MoveOutline { .. }
            | Command::MoveOutlineTo { .. }
            | Command::SetOutlineText { .. }
            | Command::SplitOutlineText { .. } => {
                self.close_header_footer()?;
                self.close_note()?;
                self.close_text_box()?;
                let s = self
                    .session
                    .as_mut()
                    .ok_or(crate::EngineError::NoDocument)?;
                let here = s.selection().focus.para;
                let res = match cmd {
                    Command::SetOutlineLevel { para, level } => {
                        s.set_outline_level(para.unwrap_or(here), *level)
                    }
                    Command::MoveOutline { para, up } => s.move_outline(para.unwrap_or(here), *up),
                    Command::MoveOutlineTo { para, before } => s.move_outline_to(*para, *before),
                    Command::SetOutlineText { para, text } => s.set_outline_text(*para, text),
                    Command::SplitOutlineText { para, text, at } => {
                        s.split_outline_text(*para, text, *at)
                    }
                    _ => Ok(s.selection()),
                };
                let error = res.err().map(|e| e.to_string());
                let dirty = self.relayout()?;
                let mut r = self.edit_result(dirty, (None, None, None), error)?;
                r.outline = Some(self.outline_items());
                r.collab = self.collab_after_edit();
                if let Some(s) = self.session.as_ref() {
                    r.review = Some(crate::review::review_state(s, self.markup_options()));
                }
                Ok(Some(r))
            }
            Command::FindResults { query, options } => {
                let s = self
                    .session
                    .as_ref()
                    .ok_or(crate::EngineError::NoDocument)?;
                let (matches, error) = match s.find(query, options) {
                    Ok(m) => (m, None),
                    Err(e) => (Vec::new(), Some(e.to_string())),
                };
                let mut r = self.query_result()?;
                r.error = error;
                r.highlights = Some(self.match_rects(&matches));
                r.results = Some(self.find_results(&matches));
                r.count = Some(matches.len());
                r.matches = Some(matches);
                Ok(Some(r))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Command;

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(crate::plain_text_document(text), "t.docx").unwrap();
        e
    }

    fn cmd(e: &mut Engine, json: &str) -> serde_json::Value {
        serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
    }

    #[test]
    fn web_layout_reflows_to_the_window_width() {
        let long = "lorem ipsum dolor sit amet consectetur ".repeat(40);
        let mut e = engine(&long);
        let print_lines = e.layout().unwrap().pages[0].lines.len();
        let r = cmd(
            &mut e,
            r#"{"cmd":"setViewLayout","mode":"web","width":300}"#,
        );
        assert_eq!(r["info"]["pages"][0]["width"].as_f64(), Some(300.0));
        let page = &e.layout().unwrap().pages[0];
        // Narrower than A4: more lines, every one inside the window (side margins 24 pt).
        assert!(page.lines.len() > print_lines);
        for l in &page.lines {
            for f in &l.fragments {
                assert!(
                    f.x >= 23.0 && f.x + f.width <= 300.0 - 23.0,
                    "{} {}",
                    f.x,
                    f.width
                );
            }
        }
        // Wider window: fewer lines. Print layout restores the A4 page.
        cmd(
            &mut e,
            r#"{"cmd":"setViewLayout","mode":"web","width":900}"#,
        );
        let wide = e.layout().unwrap().pages[0].lines.len();
        assert!(wide < page_lines(&mut e, 300.0));
        cmd(&mut e, r#"{"cmd":"setViewLayout","mode":"print"}"#);
        assert_eq!(e.layout().unwrap().pages[0].lines.len(), print_lines);
        assert!((e.layout().unwrap().pages[0].width - 595.3).abs() < 1.0);
    }

    /// Read mode reflows the text into column pages of the given size, whatever the
    /// printed page setup; the print layout comes back unchanged.
    #[test]
    fn read_mode_reflows_to_a_column() {
        let long = "lorem ipsum dolor sit amet consectetur ".repeat(200);
        let mut e = engine(&long);
        let print_pages = e.layout().unwrap().pages.len();
        let print_lines = e.layout().unwrap().pages[0].lines.len();
        let r = cmd(
            &mut e,
            r#"{"cmd":"setViewLayout","mode":"read","width":360,"height":480}"#,
        );
        assert_eq!(r["info"]["pages"][0]["width"].as_f64(), Some(360.0));
        assert_eq!(r["info"]["pages"][0]["height"].as_f64(), Some(480.0));
        let layout = e.layout().unwrap();
        // Smaller pages than A4: more of them, every line inside the column.
        assert!(layout.pages.len() > print_pages);
        for page in &layout.pages {
            assert!((page.width - 360.0).abs() < 0.1);
            for l in &page.lines {
                for f in &l.fragments {
                    assert!(f.x >= 23.0 && f.x + f.width <= 360.0 - 23.0);
                }
            }
        }
        let short = layout.pages.len();
        // A taller column holds more lines per page; print layout restores A4.
        cmd(
            &mut e,
            r#"{"cmd":"setViewLayout","mode":"read","width":360,"height":900}"#,
        );
        assert!(e.layout().unwrap().pages.len() < short);
        cmd(&mut e, r#"{"cmd":"setViewLayout","mode":"print"}"#);
        assert_eq!(e.layout().unwrap().pages.len(), print_pages);
        assert_eq!(e.layout().unwrap().pages[0].lines.len(), print_lines);
    }

    fn page_lines(e: &mut Engine, width: f32) -> usize {
        cmd(
            e,
            &format!(r#"{{"cmd":"setViewLayout","mode":"web","width":{width}}}"#),
        );
        e.layout().unwrap().pages[0].lines.len()
    }

    #[test]
    fn web_and_draft_drop_headers_and_keep_editing() {
        let mut e = engine("one two");
        e.command(Command::EditHeaderFooter {
            page: 0,
            header: true,
        })
        .unwrap();
        e.command(Command::InsertText {
            text: "HEAD".into(),
        })
        .unwrap();
        e.command(Command::CloseHeaderFooter).unwrap();
        assert!(e.layout().unwrap().page_text(0).contains("HEAD"));
        cmd(&mut e, r#"{"cmd":"setViewLayout","mode":"draft"}"#);
        assert!(!e.layout().unwrap().page_text(0).contains("HEAD"));
        // Typing in draft view edits the document.
        cmd(&mut e, r#"{"cmd":"insertText","text":"X"}"#);
        assert!(e.layout().unwrap().page_text(0).contains('X'));
        let r = cmd(
            &mut e,
            r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#,
        );
        assert!(r["error"].is_string());
        // The header is still in the document.
        cmd(&mut e, r#"{"cmd":"setViewLayout","mode":"print"}"#);
        assert!(e.layout().unwrap().page_text(0).contains("HEAD"));
    }

    #[test]
    fn formatting_marks_in_the_page_display_list() {
        let mut e = engine("");
        for c in [
            r#"{"cmd":"insertText","text":"a b"}"#,
            r#"{"cmd":"tab"}"#,
            r#"{"cmd":"insertText","text":"c"}"#,
            r#"{"cmd":"splitParagraph"}"#,
            r#"{"cmd":"insertText","text":"second"}"#,
        ] {
            cmd(&mut e, c);
        }
        let page: serde_json::Value = serde_json::from_str(&e.page_json(0).unwrap()).unwrap();
        assert!(page.get("marks").is_none());
        let r = cmd(&mut e, r#"{"cmd":"showFormattingMarks","on":true}"#);
        assert_eq!(r["dirty"], serde_json::json!([0]));
        let page: serde_json::Value = serde_json::from_str(&e.page_json(0).unwrap()).unwrap();
        let kinds: Vec<&str> = page["marks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["space", "tab", "paragraph", "paragraph"]);
        let marks = e.formatting_marks(0);
        // The space mark sits between "a" and "b"; the tab is wider than a space.
        assert!(marks[0].x > 0.0 && marks[0].w > 0.0);
        assert!(marks[1].x > marks[0].x && marks[1].w > marks[0].w);
        // The second paragraph's mark is on the next line.
        assert!(marks[3].top > marks[2].top);
        cmd(&mut e, r#"{"cmd":"showFormattingMarks","on":false}"#);
        let page: serde_json::Value = serde_json::from_str(&e.page_json(0).unwrap()).unwrap();
        assert!(page.get("marks").is_none());
    }

    /// Hidden text (§17.3.2.41 vanish) shows with the formatting marks, underlined with
    /// dots, and takes room on the line; printing leaves it out.
    #[test]
    fn hidden_text_shows_with_the_formatting_marks() {
        let mut e = engine("Hiện ẩn cuối");
        let text = |e: &Engine| e.layout().unwrap().page_text(0);
        // "ẩn " (4 bytes + 1) hidden.
        cmd(
            &mut e,
            r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":6},"focus":{"p":0,"o":11}}}"#,
        );
        let r = cmd(
            &mut e,
            r#"{"cmd":"formatText","format":{"kind":"advanced","patch":{"hidden":true}}}"#,
        );
        assert!(!text(&e).contains("ẩn"), "{}", text(&e));
        assert_eq!(r["format"]["advanced"]["hidden"], true, "{}", r["format"]);
        cmd(&mut e, r#"{"cmd":"showFormattingMarks","on":true}"#);
        assert!(text(&e).contains("ẩn"), "{}", text(&e));
        let dotted = e.layout().unwrap().pages[0]
            .lines
            .iter()
            .flat_map(|l| l.fragments.iter())
            .filter(|f| f.text.contains("ẩn"))
            .all(|f| f.underline == bo_model::Underline::Dotted);
        assert!(dotted);
        // Not printed even while shown.
        let sheets = e
            .print_sheets(&crate::print::PrintOptions::default())
            .unwrap();
        let printed: String = sheets[0]
            .commands
            .iter()
            .filter_map(|c| match c {
                bo_render::Command::Glyphs { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            printed.contains("Hiện") && !printed.contains("ẩn"),
            "{printed}"
        );
        assert!(text(&e).contains("ẩn"), "the view keeps showing it");
        cmd(&mut e, r#"{"cmd":"showFormattingMarks","on":false}"#);
        assert!(!text(&e).contains("ẩn"));
    }

    /// With the formatting marks, a table cell's last paragraph ends with ¤ (not ¶)
    /// and each row has a ¤ right of its last cell.
    #[test]
    fn cell_and_row_end_marks() {
        let mut e = engine("");
        cmd(&mut e, r#"{"cmd":"insertTable","rows":2,"cols":3}"#);
        cmd(&mut e, r#"{"cmd":"insertText","text":"a"}"#);
        cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
        cmd(&mut e, r#"{"cmd":"insertText","text":"b"}"#);
        cmd(&mut e, r#"{"cmd":"showFormattingMarks","on":true}"#);
        let marks = e.formatting_marks(0);
        let count = |k: FormatMarkKind| marks.iter().filter(|m| m.kind == k).count();
        assert_eq!(count(FormatMarkKind::CellEnd), 6);
        assert_eq!(count(FormatMarkKind::RowEnd), 2);
        // The first cell's first paragraph ("a") keeps its ¶; the paragraph after the
        // table too.
        assert_eq!(count(FormatMarkKind::Paragraph), 2);
        let cells: Vec<&FormatMark> = marks
            .iter()
            .filter(|m| m.kind == FormatMarkKind::CellEnd)
            .collect();
        let rows: Vec<&FormatMark> = marks
            .iter()
            .filter(|m| m.kind == FormatMarkKind::RowEnd)
            .collect();
        // Each row mark is right of every cell mark of its row, one row below the other.
        let right = cells.iter().map(|m| m.x).fold(0.0, f32::max);
        assert!(rows.iter().all(|r| r.x > right), "{rows:?} {right}");
        assert!(rows[1].top > rows[0].top);
        let page: serde_json::Value = serde_json::from_str(&e.page_json(0).unwrap()).unwrap();
        let kinds: Vec<&str> = page["marks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["kind"].as_str().unwrap())
            .collect();
        assert!(kinds.contains(&"cellEnd") && kinds.contains(&"rowEnd"));
    }

    #[test]
    fn outline_commands_move_headings_and_undo() {
        let mut e = engine("A\na\nB\nb");
        cmd(&mut e, r#"{"cmd":"setOutlineLevel","para":0,"level":0}"#);
        let r = cmd(&mut e, r#"{"cmd":"setOutlineLevel","para":2,"level":0}"#);
        let levels: Vec<Option<u64>> = r["outline"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["level"].as_u64())
            .collect();
        assert_eq!(levels, [Some(0), None, Some(0), None]);
        assert_eq!(r["outline"][2]["page"], 0);
        let r = cmd(&mut e, r#"{"cmd":"moveOutline","para":2,"up":true}"#);
        let texts: Vec<&str> = r["outline"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["text"].as_str().unwrap())
            .collect();
        assert_eq!(texts, ["B", "b", "A", "a"]);
        assert_eq!(r["selection"]["focus"]["p"], 0);
        cmd(&mut e, r#"{"cmd":"undo"}"#);
        let r = cmd(&mut e, r#"{"cmd":"outline"}"#);
        assert_eq!(r["outline"][0]["text"], "A");
        // Drag A (with "a") to the end.
        let r = cmd(&mut e, r#"{"cmd":"moveOutlineTo","para":0}"#);
        assert_eq!(r["outline"][0]["text"], "B");
        assert_eq!(r["outline"][3]["text"], "a");
    }

    #[test]
    fn find_results_carry_context_and_pages() {
        let mut e = engine("the quick brown fox\njumps over the lazy dog");
        let r = cmd(&mut e, r#"{"cmd":"findResults","query":"the"}"#);
        let res = r["results"].as_array().unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0]["text"], "the");
        assert_eq!(res[0]["after"], " quick brown fox");
        assert_eq!(res[1]["before"], "jumps over ");
        assert_eq!(res[1]["match"]["p"], 1);
        assert_eq!(res[1]["page"], 0);
        assert_eq!(r["highlights"].as_array().unwrap().len(), 2);
    }
}
