//! Header/footer editing (ECMA-376 Part 1, §17.10).
//!
//! Double-clicking the header or footer area of a page opens that page's
//! header/footer variant (§17.10.5: first page with `titlePg`, even pages with
//! `evenAndOddHeaders`, else default) in a *story session*: a [`bo_edit::Session`]
//! over a one-section document holding only the header/footer blocks, laid out
//! with the section's text width so its lines line up with the header drawn on
//! the page. Every change is mirrored into the main document without history
//! (live display); closing the header records one undoable change in the main
//! session.

use crate::edit::{Command, EditResult};
use crate::nav::{self, CaretRect, Motion, SelRect};
use crate::{Engine, EngineError, Result};
use bo_edit::section::{effective_hf, HfVariant};
use bo_edit::{Pos, Selection, Session};
use bo_layout::{layout_document_with, LayoutDocument, LayoutOptions, LineSource};
use bo_model::{Block, Document, PageSetup, Paragraph, Section, Twips};
use serde::Serialize;

/// The open header/footer editor.
pub(crate) struct HfEdit {
    /// Section of the page the header/footer is edited on.
    pub section: usize,
    /// Section whose header/footer is changed: `section` itself, or the one it is
    /// linked to (§17.10.5 "link to previous").
    pub target: usize,
    pub header: bool,
    pub variant: HfVariant,
    /// Page the header/footer is edited on.
    pub page: usize,
    pub sub: Session,
    layout: LayoutDocument,
    /// Main page y = story y + `dy`.
    dy: f32,
    /// The variant's own content before editing (`None` = not defined).
    original: Option<Vec<Block>>,
    version: u64,
}

/// Header/footer editing state reported to hosts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryState {
    /// `header` or `footer`.
    pub kind: &'static str,
    /// 0-based section index.
    pub section: usize,
    /// Which header/footer of the section (default, first, even).
    pub variant: HfVariant,
    /// 0-based page the header/footer is edited on.
    pub page: usize,
    /// The section has a different first page (§17.10.6 `titlePg`).
    pub title_page: bool,
    /// The document has different odd and even pages (§17.15.1.35).
    pub even_and_odd: bool,
    /// Linked to the previous section's header/footer (§17.10.5).
    pub linked: bool,
    /// The section has a previous section to link to.
    pub can_link: bool,
    /// Id of the footnote/endnote being edited (`kind` footnote/endnote).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<i32>,
}

/// Height (twips) of the scratch page the story is laid out on.
const STORY_PAGE: i32 = 31_680;

fn story_doc(main: &Document, si: usize, blocks: Vec<Block>, top: Twips) -> Document {
    let page = main.sections.get(si).map(|s| s.page).unwrap_or_default();
    let sec = Section {
        page: PageSetup {
            height: Twips(STORY_PAGE),
            margin_top: top,
            margin_bottom: Twips(0),
            header: Twips(0),
            footer: Twips(0),
            ..page
        },
        ..Section::new(blocks)
    };
    Document {
        sections: vec![sec],
        default_run: main.default_run.clone(),
        default_para: main.default_para.clone(),
        styles: main.styles.clone(),
        numbering: main.numbering.clone(),
        media: main.media.clone(),
        default_tab_stop: main.default_tab_stop,
        ..Document::default()
    }
}

/// Blocks of the story document.
fn story_blocks(s: &Session) -> Vec<Block> {
    s.doc()
        .sections
        .first()
        .map(|s| s.blocks.clone())
        .unwrap_or_default()
}

impl Engine {
    /// Variant of the header/footer shown on page `pi` (§17.10.5, §17.10.6).
    fn variant_of(&self, pi: usize) -> Option<(usize, HfVariant)> {
        let layout = self.layout()?;
        let doc = self.session.as_ref()?.doc();
        let page = layout.pages.get(pi)?;
        let si = page.section.min(doc.sections.len().saturating_sub(1));
        let sec = doc.sections.get(si)?;
        let first = layout.pages.iter().position(|p| p.section == page.section) == Some(pi);
        let v = if sec.title_page && first {
            HfVariant::First
        } else if doc.even_and_odd_headers && page.number % 2 == 0 {
            HfVariant::Even
        } else {
            HfVariant::Default
        };
        Some((si, v))
    }

    /// `Some(true)` = header area, `Some(false)` = footer area of page `pi` at `y`.
    pub(crate) fn hf_zone(&self, pi: usize, y: f32) -> Option<bool> {
        let layout = self.layout()?;
        let doc = self.session.as_ref()?.doc();
        let page = layout.pages.get(pi)?;
        let setup = doc.sections.get(page.section)?.page;
        if y < setup.margin_top.to_pt() {
            Some(true)
        } else if y > page.height - setup.margin_bottom.to_pt() {
            Some(false)
        } else {
            None
        }
    }

    /// Open the header (`header`) or footer of page `pi` for editing.
    pub fn open_header_footer(&mut self, pi: usize, header: bool) -> Result<()> {
        self.close_header_footer()?;
        let (si, variant) = self.variant_of(pi).ok_or(EngineError::NoDocument)?;
        let s = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let doc = s.doc();
        let target = bo_edit::section::hf_source(doc, si, header, variant);
        let set = if header {
            &doc.sections[target].headers
        } else {
            &doc.sections[target].footers
        };
        let original = bo_edit::section::hf_slot(set, variant).cloned();
        let mut blocks = effective_hf(doc, si, header, variant).unwrap_or_default();
        if !blocks.iter().any(|b| matches!(b, Block::Paragraph(_))) {
            blocks.push(Block::Paragraph(Paragraph::default()));
        }
        let top = if header {
            doc.sections[si].page.header
        } else {
            Twips(0)
        };
        let mut sub = Session::new(story_doc(doc, si, blocks, top));
        sub.set_author(s.author());
        sync_tracking(&mut sub, s);
        let mut hf = HfEdit {
            section: si,
            target,
            header,
            variant,
            page: pi,
            sub,
            layout: LayoutDocument::default(),
            dy: 0.0,
            original,
            version: u64::MAX,
        };
        self.hf_layout(&mut hf)?;
        self.hf = Some(hf);
        Ok(())
    }

    /// Lay the story out and place it on the page.
    fn hf_layout(&mut self, hf: &mut HfEdit) -> Result<()> {
        let opts = LayoutOptions {
            view: self.view,
            field_codes: self.field_codes,
        };
        hf.layout = layout_document_with(hf.sub.doc(), &mut self.fonts, &mut self.cache, opts)?;
        let sub_top = hf
            .layout
            .pages
            .first()
            .and_then(|p| p.lines.first())
            .map(|l| l.top);
        let sub_bottom = hf
            .layout
            .pages
            .first()
            .and_then(|p| p.lines.iter().map(|l| l.top + l.height).reduce(f32::max))
            .unwrap_or(0.0);
        let Some(doc) = self.session.as_ref().map(Session::doc) else {
            return Ok(());
        };
        let setup = doc
            .sections
            .get(hf.section)
            .map(|s| s.page)
            .unwrap_or_default();
        // Align with the lines drawn on the page when there are any.
        let main_top = self
            .layout()
            .and_then(|l| l.pages.get(hf.page))
            .and_then(|p| {
                let (mt, mb) = (
                    setup.margin_top.to_pt(),
                    p.height - setup.margin_bottom.to_pt(),
                );
                p.lines
                    .iter()
                    .filter(|l| l.source == LineSource::Other)
                    .find(|l| {
                        if hf.header {
                            l.top < mt
                        } else {
                            l.top >= mb - 0.5
                        }
                    })
                    .map(|l| l.top)
            });
        hf.dy = match (main_top, sub_top) {
            (Some(m), Some(s)) => m - s,
            _ if hf.header => 0.0,
            _ => setup.height.to_pt() - setup.footer.to_pt() - sub_bottom,
        };
        hf.version = hf.sub.version();
        Ok(())
    }

    /// Close the header/footer editor, recording its change (one undo step).
    pub fn close_header_footer(&mut self) -> Result<bool> {
        let Some(hf) = self.hf.take() else {
            return Ok(false);
        };
        let blocks = story_blocks(&hf.sub);
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        // Back to the state before editing, then the real (undoable) change.
        s.preview_header_footer(hf.target, hf.header, hf.variant, hf.original.clone());
        let unchanged_default = hf.original.is_none()
            && effective_hf(s.doc(), hf.target, hf.header, hf.variant).as_ref() == Some(&blocks);
        let empty = blocks.iter().all(|b| match b {
            Block::Paragraph(p) => p.runs.is_empty(),
            _ => false,
        });
        if hf.original.as_ref() != Some(&blocks)
            && !unchanged_default
            && !(hf.original.is_none() && empty)
        {
            s.set_header_footer(hf.target, hf.header, hf.variant, blocks)
                .map_err(|e| EngineError::BadCommand(e.to_string()))?;
        }
        self.relayout()?;
        Ok(true)
    }

    /// State of the open header/footer editor.
    pub(crate) fn story_state(&self) -> Option<StoryState> {
        let hf = self.hf.as_ref()?;
        let doc = self.session.as_ref()?.doc();
        Some(StoryState {
            kind: if hf.header { "header" } else { "footer" },
            section: hf.section,
            variant: hf.variant,
            page: hf.page,
            title_page: doc.sections.get(hf.section).is_some_and(|s| s.title_page),
            even_and_odd: doc.even_and_odd_headers,
            linked: bo_edit::section::hf_linked(doc, hf.section, hf.header, hf.variant),
            can_link: hf.section > 0,
            note: None,
        })
    }

    /// Run `cmd` in the header/footer editor. Returns `None` when the command is not
    /// a story command (the editor is then closed and the command runs on the body).
    pub(crate) fn story_command(&mut self, cmd: &Command) -> Result<Option<EditResult>> {
        if let Command::Click { page, y, count, .. } = cmd {
            // A click in the body closes the header/footer.
            if self.hf_zone(*page, *y).is_none() {
                self.close_header_footer()?;
                return Ok(None);
            }
            if let (Some(zone), Some(hf)) = (self.hf_zone(*page, *y), self.hf.as_ref()) {
                let target = self.variant_of(*page);
                if zone != hf.header || target != Some((hf.section, hf.variant)) {
                    if *count >= 2 {
                        self.open_header_footer(*page, zone)?;
                        return self.story_result(Vec::new(), None).map(Some);
                    }
                    self.close_header_footer()?;
                    return Ok(None);
                }
            }
        }
        if let Command::SetHeaderOptions {
            title_page,
            even_and_odd,
        } = cmd
        {
            let (page, header) = self.hf.as_ref().map_or((0, true), |h| (h.page, h.header));
            self.close_header_footer()?;
            let si = self.variant_of(page).map_or(0, |v| v.0);
            if let Some(s) = self.session.as_mut() {
                s.set_header_options(si, *title_page, *even_and_odd)
                    .map_err(|e| EngineError::BadCommand(e.to_string()))?;
            }
            let dirty = self.relayout()?;
            self.open_header_footer(page.min(self.page_count().saturating_sub(1)), header)?;
            return self.story_result(dirty, None).map(Some);
        }
        if let Command::SetHeaderLink { linked } = cmd {
            let Some((page, header, si, v)) = self
                .hf
                .as_ref()
                .map(|h| (h.page, h.header, h.section, h.variant))
            else {
                return Ok(None);
            };
            self.close_header_footer()?;
            let mut error = None;
            if let Some(s) = self.session.as_mut() {
                if let Err(e) = s.set_header_link(si, header, v, *linked) {
                    error = Some(e.to_string());
                }
            }
            let dirty = self.relayout()?;
            self.open_header_footer(page.min(self.page_count().saturating_sub(1)), header)?;
            return self.story_result(dirty, error).map(Some);
        }
        let Some(mut hf) = self.hf.take() else {
            return Ok(None);
        };
        let mut error = None;
        let mut clip = None;
        let mut cur_page = hf.page;
        if let Some(s) = self.session.as_ref() {
            sync_tracking(&mut hf.sub, s);
        }
        let res = sub_command(
            &mut hf.sub,
            &hf.layout,
            (hf.dy, Some(0)),
            cmd,
            &mut clip,
            &mut cur_page,
        );
        hf.page = cur_page;
        let handled = match res {
            Ok(h) => h,
            Err(e) => {
                error = Some(e.to_string());
                true
            }
        };
        if !handled {
            self.hf = Some(hf);
            self.close_header_footer()?;
            return Ok(None);
        }
        let mut dirty = Vec::new();
        if hf.sub.version() != hf.version {
            let blocks = story_blocks(&hf.sub);
            if let Some(s) = self.session.as_mut() {
                s.preview_header_footer(hf.target, hf.header, hf.variant, Some(blocks));
            }
            dirty = self.relayout()?;
            self.hf_layout(&mut hf)?;
        }
        self.hf = Some(hf);
        let mut r = self.story_result(dirty, error)?;
        r.clip = clip;
        Ok(Some(r))
    }

    /// Edit result while the header/footer is open: its selection mapped onto the page.
    pub(crate) fn story_result(
        &self,
        dirty: Vec<usize>,
        error: Option<String>,
    ) -> Result<EditResult> {
        let mut r = self.edit_result(dirty, (None, None, None), error)?;
        if let Some(hf) = self.hf.as_ref() {
            let sel = hf.sub.selection();
            r.selection = sel;
            r.caret = nav::caret_rect(&hf.layout, sel.focus).map(|c| CaretRect {
                page: hf.page,
                y: c.y + hf.dy,
                ..c
            });
            r.rects = nav::selection_rects(&hf.layout, sel)
                .into_iter()
                .map(|q| SelRect {
                    page: hf.page,
                    y: q.y + hf.dy,
                    ..q
                })
                .collect();
            let section = r.format.section;
            r.format = hf.sub.format_state();
            r.format.section = section;
            r.format.table = None;
        }
        r.story = self.story_state();
        Ok(r)
    }

    fn page_count(&self) -> usize {
        self.layout().map_or(0, |l| l.pages.len())
    }
}

/// A header/footer or note session records tracked changes like the main session
/// (ECMA-376 Part 1, §17.13.5 in header, footer and note parts): same mode, author
/// and clock, revision ids above every id of the document.
pub(crate) fn sync_tracking(sub: &mut Session, main: &Session) {
    if sub.track_changes() != main.track_changes() {
        sub.set_track_changes(main.track_changes());
    }
    sub.set_author(main.author());
    sub.set_clock(main.clock().map(str::to_owned));
    sub.set_revision_floor(bo_edit::revisions::max_id(main.doc()));
}

/// Run an editing command in a story session laid out as `layout`: story y = page y
/// − `dy`; hit tests use page `hit_page` (`None`: the clicked page). Returns `false`
/// for commands that are not story commands.
pub(crate) fn sub_command(
    s: &mut Session,
    layout: &LayoutDocument,
    (dy, hit_page): (f32, Option<usize>),
    cmd: &Command,
    clip: &mut Option<crate::edit::ClipOut>,
    cur_page: &mut usize,
) -> bo_edit::Result<bool> {
    let sel = s.selection();
    match cmd {
        Command::State => {}
        Command::Click {
            page,
            x,
            y,
            extend,
            count,
        } => {
            *cur_page = *page;
            if let Some(p) = nav::hit_test(layout, hit_page.unwrap_or(*page), *x, *y - dy) {
                s.set_selection(match count {
                    2 => s.word_at(p),
                    3.. => s.paragraph_at(p),
                    _ if *extend => Selection {
                        anchor: sel.anchor,
                        focus: p,
                    },
                    _ => Selection::caret(p),
                });
            }
        }
        Command::DragTo { page, x, y, .. } => {
            if let Some(p) = nav::hit_test(layout, hit_page.unwrap_or(*page), *x, *y - dy) {
                s.set_selection(Selection {
                    anchor: sel.anchor,
                    focus: p,
                });
            }
        }
        Command::Move { motion, extend } => {
            let from = sel.focus;
            let to = match motion {
                Motion::Left if !extend && !sel.is_collapsed() => sel.ordered().0,
                Motion::Right if !extend && !sel.is_collapsed() => sel.ordered().1,
                Motion::Left => s.char_step(from, false),
                Motion::Right => s.char_step(from, true),
                Motion::WordLeft => s.word_boundary(from, false),
                Motion::WordRight => s.word_boundary(from, true),
                Motion::DocStart => Pos::new(0, 0),
                Motion::DocEnd => {
                    let last = s.paragraph_count().saturating_sub(1);
                    Pos::new(last, s.para_text(last).map_or(0, |t| t.len()))
                }
                _ => nav::vertical(layout, from, *motion, None).map_or(from, |v| v.0),
            };
            s.set_selection(if *extend {
                Selection {
                    anchor: sel.anchor,
                    focus: to,
                }
            } else {
                Selection::caret(to)
            });
        }
        Command::SetSelection { selection } => s.set_selection(*selection),
        Command::SelectAll => {
            s.select_all();
        }
        Command::InsertText { text } => {
            s.insert_text(text)?;
        }
        Command::Tab { .. } => {
            s.insert_text("\t")?;
        }
        Command::DeleteBackward => {
            s.delete_backward()?;
        }
        Command::DeleteForward => {
            s.delete_forward()?;
        }
        Command::SplitParagraph => {
            s.split_paragraph()?;
        }
        Command::FormatText { format } => {
            s.format_text(format)?;
        }
        Command::FormatParagraph { format } => {
            s.format_paragraphs(format)?;
        }
        Command::ApplyStyle { id } => {
            s.apply_style(id)?;
        }
        Command::ClearFormatting => {
            s.clear_formatting()?;
        }
        Command::InsertField { instr } => {
            s.insert_field(instr, "1")?;
        }
        Command::Undo => {
            s.undo();
        }
        Command::Redo => {
            s.redo();
        }
        Command::Copy | Command::Cut => {
            let c = if matches!(cmd, Command::Cut) {
                s.cut()?
            } else {
                s.copy()
            };
            *clip = Some(crate::edit::ClipOut {
                text: c.text,
                html: c.html,
                internal: c.internal,
            });
        }
        Command::Paste {
            internal,
            html,
            text,
            mode,
        } => {
            s.paste_with(
                mode.unwrap_or_default(),
                internal.as_deref(),
                html.as_deref(),
                text.as_deref(),
            )?;
        }
        Command::PickFormat => {
            s.pick_format();
        }
        Command::ApplyFormat => {
            s.apply_picked_format()?;
        }
        Command::CancelFormat => s.clear_picked_format(),
        _ => return Ok(false),
    }
    Ok(true)
}
