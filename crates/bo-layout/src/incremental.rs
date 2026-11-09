//! Incremental layout (BlueOffice-PROMPT §5.4): after an edit only the changed
//! blocks are re-shaped and the pages are re-paginated from the first affected
//! one until the flow re-synchronises with the previous layout.
//!
//! Pagination records a [`Checkpoint`] before every top-level block: the flow
//! state (position, column, notes on the page, list counters…) without the pages.
//! An edit of blocks `start..end` of a section restarts the flow from the last
//! checkpoint at or before `start` and compares each checkpoint at or after `end`
//! with the previous layout's checkpoint of the same block. When they are equal,
//! the rest of the document lays out exactly as before: the previous pages are
//! reused — moved by the difference in page count, renumbered and with paragraph
//! ids shifted when needed — instead of being laid out again. Pure paragraphs
//! (no list label, fields or note references) keep their built lines between
//! layouts, so re-pagination does not even look them up in [`LayoutCache`].
//!
//! Headers and footers are applied afterwards per page; a reused page keeps its
//! header/footer unless its page fields (number, total) changed. Editing a header
//! or footer re-decorates the pages from its section on without re-paginating, as
//! long as the header/footer heights (which push the body) stay the same. The pages
//! whose content changed are reported so hosts redraw only those.
//!
//! When the body shows the total page count (NUMPAGES, SECTIONPAGES), a full layout
//! paginates once with an empty total to count the pages, then again with that
//! count. Both passes are kept incrementally: a probe layout (no headers/footers)
//! gives the count, and the main layout is updated incrementally while the count
//! stays the same.
//!
//! Whatever changes, the result is identical to a full layout of the new
//! document (property-tested in `tests/incremental.rs`).

use crate::blocks::{stack_blocks, Ctx, Stack};
use crate::flow::{float_item, hf_blocks, CachedPara, Checkpoint, Flow};
use crate::style::ListCounters;
use crate::types::{LayoutDocument, NoteKey, Page};
use crate::{content_width, number_notes, FieldContext, LayoutCache, LayoutError, LayoutOptions};
use bo_model::change::BlockChanges;
use bo_model::{Block, Document, RunContent, SectionBreak, Twips};
use bo_text::FontManager;
use std::collections::HashMap;

/// What a page's header and footer depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct HfKey {
    section: usize,
    variant: u8,
    /// Page fields (0 when the headers and footers have no fields).
    number: u32,
    total: u32,
    section_pages: u32,
}

/// Header/footer content added to a page after pagination.
#[derive(Debug, Clone, Default)]
struct PageMeta {
    /// Header lines at the start of `lines`, footer lines at the end, header and
    /// footer items at the end of `items`.
    head: usize,
    foot: usize,
    items: usize,
    key: HfKey,
    /// List counters of headers/footers/notes before and after this page's header/footer.
    lists_before: ListCounters,
    lists_after: ListCounters,
}

/// Where a page of the new layout comes from.
#[derive(Debug, Clone, Copy)]
enum Origin {
    /// Laid out again (or merged with the previous layout at the re-synchronisation point).
    Fresh,
    /// Page `old` of the previous layout; `moved`: at another index or renumbered.
    Reused { old: usize, moved: bool },
}

/// Everything kept between layouts.
struct State {
    opts: LayoutOptions,
    layout: LayoutDocument,
    meta: Vec<PageMeta>,
    cps: Vec<Vec<Option<Checkpoint>>>,
    built: Vec<Vec<Option<CachedPara>>>,
    foot_cache: HashMap<NoteKey, Stack>,
    foot_order: Vec<NoteKey>,
    /// Header/footer/note list counters at the end of pagination.
    end_other_lists: ListCounters,
    section_start: Vec<Option<usize>>,
    hf_heights: Vec<(f32, f32)>,
    /// Note numbers the layout used (per page with §17.11.19 numRestart eachPage).
    notes: HashMap<NoteKey, String>,
    /// Note numbers of the scan of the document (before per-page numbering).
    scan_notes: HashMap<NoteKey, String>,
    endnote_order: Vec<i32>,
    /// Total page count shown by body fields (0: none, or the probe layout).
    total: u32,
    /// Headers or footers contain fields (their content depends on page numbers).
    hf_fields: bool,
}

/// How the last [`IncrementalLayout::update`] was done (diagnostics and tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateKind {
    /// Nothing to do.
    #[default]
    None,
    /// The whole document was laid out.
    Full,
    /// Re-paginated from the first changed block; `resynced`: stopped early and
    /// reused the previous layout's later pages.
    Incremental {
        /// Pagination stopped early and reused the previous later pages.
        resynced: bool,
    },
}

/// A layout kept up to date across edits.
#[derive(Default)]
pub struct IncrementalLayout {
    st: Option<State>,
    last: UpdateKind,
    /// Never stop early (benchmarks of the worst case: every later page moves).
    no_resync: bool,
    /// This is the probe layout: body fields show no total, no headers/footers.
    probe_mode: bool,
    /// Probe layout counting the pages when the body shows the total.
    probe: Option<Box<IncrementalLayout>>,
}

/// Analysis of the main story done before every layout.
struct Scan {
    notes: HashMap<NoteKey, String>,
    endnote_order: Vec<i32>,
    needs_total: bool,
    /// The body has fields (their text may depend on the page number).
    body_fields: bool,
}

fn scan(doc: &Document) -> Scan {
    let (notes, endnote_order, needs_total, body_fields) = number_notes(doc);
    Scan {
        notes,
        endnote_order,
        needs_total,
        body_fields,
    }
}

fn blocks_have_fields(blocks: &[Block]) -> bool {
    blocks.iter().any(|b| match b {
        Block::Paragraph(p) => p.runs.iter().any(|r| {
            r.content
                .iter()
                .any(|c| matches!(c, RunContent::Field { .. }))
        }),
        Block::Table(t) => t
            .rows
            .iter()
            .any(|r| r.cells.iter().any(|c| blocks_have_fields(&c.blocks))),
        Block::Opaque(_) => false,
    })
}

/// Header/footer variant shown on page `pi`: 0 default, 1 first, 2 even (§17.10.5, §17.10.6 titlePg).
fn variant(doc: &Document, pages: &[Page], section_start: &[Option<usize>], pi: usize) -> u8 {
    let page = &pages[pi];
    let sec = &doc.sections[page.section.min(doc.sections.len() - 1)];
    if sec.title_page && section_start.get(page.section).copied().flatten() == Some(pi) {
        1
    } else if doc.even_and_odd_headers && page.number & 1 == 0 {
        2
    } else {
        0
    }
}

/// Remove the header/footer content described by `m`.
fn strip(page: &mut Page, m: &PageMeta) {
    page.lines.drain(..m.head.min(page.lines.len()));
    let n = page.lines.len().saturating_sub(m.foot);
    page.lines.truncate(n);
    let n = page.items.len().saturating_sub(m.items);
    page.items.truncate(n);
}

/// Shift the body paragraph ids of `page` by `delta`.
fn shift_ids(page: &mut Page, delta: i64) {
    if delta == 0 {
        return;
    }
    for l in &mut page.lines {
        l.source.shift(delta);
    }
    for i in &mut page.items {
        if let Some(s) = i.source_mut() {
            s.shift(delta);
        }
    }
}

/// Add the header and footer of page `pi` (ECMA-376 Part 1, §17.10).
fn decorate(
    ctx: &mut Ctx<'_>,
    page: &mut Page,
    variant: u8,
    fields: FieldContext,
) -> Result<(usize, usize, usize), LayoutError> {
    let doc = ctx.doc;
    let si = page.section;
    let sec = &doc.sections[si.min(doc.sections.len() - 1)];
    let setup = sec.page;
    ctx.fields = fields;
    let left = setup.margin_left.to_pt();
    let w = content_width(sec);
    let (mut head, mut foot) = (0, 0);
    let items0 = page.items.len();
    for header in [true, false] {
        let Some(blocks) = hf_blocks(doc, si, header, variant) else {
            continue;
        };
        let st = stack_blocks(ctx, blocks, w, false, None)?;
        let y0 = if header {
            setup.header.to_pt()
        } else {
            setup.height.to_pt() - setup.footer.to_pt() - st.h
        };
        let mut lines = Vec::new();
        for (top, band) in &st.bands {
            let b = band.translated(left, y0 + top);
            lines.extend(b.lines);
            page.items.extend(b.items);
        }
        for (top, img) in &st.floats {
            let (items, flines, _) = float_item(img, &setup, left, y0 + top);
            page.items.extend(items);
            lines.extend(flines);
        }
        // Keep reading order: header lines first, footer lines last.
        if header {
            head = lines.len();
            page.lines.splice(0..0, lines);
        } else {
            foot = lines.len();
            page.lines.extend(lines);
        }
    }
    Ok((head, foot, page.items.len() - items0))
}

/// Measure a table of `doc` for the table tools (distribute rows, AutoFit to the
/// contents) as laid out in a box `avail` points wide.
pub fn measure_table(
    doc: &Document,
    fonts: &mut FontManager,
    cache: &mut LayoutCache,
    table: &bo_model::Table,
    avail: f32,
) -> Result<crate::blocks::TableMeasure, LayoutError> {
    if fonts.is_empty() {
        return Err(LayoutError::NoFonts);
    }
    let sc = scan(doc);
    let mut ctx = new_ctx(doc, fonts, cache, LayoutOptions::default(), &sc);
    crate::blocks::measure_table(&mut ctx, table, avail)
}

fn new_ctx<'a>(
    doc: &'a Document,
    fonts: &'a mut FontManager,
    cache: &'a mut LayoutCache,
    opts: LayoutOptions,
    sc: &Scan,
) -> Ctx<'a> {
    Ctx {
        doc,
        fonts,
        cache,
        lists: ListCounters::default(),
        other_lists: ListCounters::default(),
        notes: sc.notes.clone(),
        fields: FieldContext {
            codes: opts.field_codes,
            ..FieldContext::default()
        },
        default_tab: doc.default_tab_stop.unwrap_or(Twips(720)).to_pt(),
        next_id: 0,
        id_origin: 0,
        endnote_order: sc.endnote_order.clone(),
        view: opts.view,
        note: None,
        textbox: None,
    }
}

/// A flow with one (empty) checkpoint and built-block slot per top-level block.
fn new_flow(doc: &Document, hf: Vec<(f32, f32)>) -> Flow {
    let mut flow = Flow::new(hf, doc.sections.len());
    flow.cps = doc
        .sections
        .iter()
        .map(|s| {
            std::iter::repeat_with(|| None)
                .take(s.blocks.len())
                .collect()
        })
        .collect();
    flow.built = doc
        .sections
        .iter()
        .map(|s| {
            std::iter::repeat_with(|| None)
                .take(s.blocks.len())
                .collect()
        })
        .collect();
    flow
}

/// Replace the entries of `v` for the changed range by empty ones.
fn splice_slots<T>(v: &mut Vec<Option<T>>, start: usize, old_end: usize, end: usize) {
    let s = start.min(v.len());
    let e = old_end.clamp(s, v.len());
    v.splice(s..e, std::iter::repeat_with(|| None).take(end - start));
}

impl IncrementalLayout {
    /// No layout yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current layout (empty before the first [`IncrementalLayout::update`]).
    pub fn document(&self) -> &LayoutDocument {
        static EMPTY: LayoutDocument = LayoutDocument { pages: Vec::new() };
        self.st.as_ref().map_or(&EMPTY, |s| &s.layout)
    }

    /// Take the layout out (the next update is a full layout).
    pub fn into_document(self) -> LayoutDocument {
        self.st.map(|s| s.layout).unwrap_or_default()
    }

    /// Forget the previous layout (fonts changed, another document…).
    pub fn reset(&mut self) {
        self.st = None;
        self.probe = None;
    }

    /// How the last update was done.
    pub fn last_update(&self) -> UpdateKind {
        self.last
    }

    /// Allow (default) or forbid re-synchronising with the previous layout; when
    /// forbidden every update re-paginates to the end (worst-case benchmarks).
    pub fn set_resync(&mut self, enabled: bool) {
        self.no_resync = !enabled;
        if let Some(p) = self.probe.as_mut() {
            p.no_resync = !enabled;
        }
    }

    /// Lay `doc` out again after `changes` (as recorded by the editing session since
    /// the previous update). Returns the indexes of the pages whose content changed.
    pub fn update(
        &mut self,
        doc: &Document,
        fonts: &mut FontManager,
        cache: &mut LayoutCache,
        opts: LayoutOptions,
        changes: &BlockChanges,
    ) -> Result<Vec<usize>, LayoutError> {
        if fonts.is_empty() {
            return Err(LayoutError::NoFonts);
        }
        if changes.is_empty() && self.st.as_ref().is_some_and(|s| s.opts == opts) {
            self.last = UpdateKind::None;
            return Ok(Vec::new());
        }
        let sc = scan(doc);
        // Body fields showing the total page count: the count of a layout with an
        // empty total (the first pass of a full layout), kept by the probe layout.
        let total = if sc.needs_total && !self.probe_mode && !doc.sections.is_empty() {
            let no_resync = self.no_resync;
            let probe = self.probe.get_or_insert_with(|| {
                Box::new(IncrementalLayout {
                    no_resync,
                    probe_mode: true,
                    ..IncrementalLayout::default()
                })
            });
            probe.update(doc, fonts, cache, opts, changes)?;
            probe.document().pages.len() as u32
        } else {
            self.probe = None;
            0
        };
        let mut incremental = self.st.as_ref().is_some_and(|st| {
            !changes.is_all()
                && st.opts == opts
                && st.total == total
                && st.scan_notes == sc.notes
                && st.endnote_order == sc.endnote_order
                && st.cps.len() == doc.sections.len()
                && doc.sections.iter().enumerate().all(|(si, s)| {
                    let delta = changes.section(si).map_or(0, |c| c.delta);
                    st.cps[si].len() as isize + delta == s.blocks.len() as isize
                })
        });
        // Edited headers/footers: the body is paginated as before while their heights
        // (which push the body, see `body_area`) stay the same.
        let mut hf_fields = None;
        if incremental && changes.headers().is_some() {
            let mut ctx = new_ctx(doc, fonts, cache, opts, &sc);
            let (heights, fields) = hf_heights(&mut ctx, doc)?;
            incremental = self.st.as_ref().is_some_and(|st| st.hf_heights == heights);
            hf_fields = Some(fields);
        }
        if incremental {
            if let Some(mut st) = self.st.take() {
                if let Some(f) = hf_fields {
                    st.hf_fields = f;
                }
                // `None`: no usable checkpoint, lay everything out.
                if let Some(dirty) = self.incremental(doc, fonts, cache, opts, changes, &sc, st)? {
                    return Ok(dirty);
                }
            }
        }
        self.full(doc, fonts, cache, opts, &sc, total)?;
        self.last = UpdateKind::Full;
        Ok((0..self.document().pages.len()).collect())
    }

    fn full(
        &mut self,
        doc: &Document,
        fonts: &mut FontManager,
        cache: &mut LayoutCache,
        opts: LayoutOptions,
        sc: &Scan,
        total: u32,
    ) -> Result<(), LayoutError> {
        let mut ctx = new_ctx(doc, fonts, cache, opts, sc);
        let (hf, hf_fields) = hf_heights(&mut ctx, doc)?;
        ctx.other_lists = ListCounters::default();

        // Pagination with the total page count found by the probe layout.
        ctx.fields.pages = total;
        let mut flow = new_flow(doc, hf.clone());
        flow.run(&mut ctx, None)?;
        // §17.11.19 numRestart eachPage: number the footnotes per page, lay out again.
        let each_page = if self.probe_mode {
            None
        } else {
            crate::page_note_numbers(doc, &flow.pages, &ctx.notes)
        };
        if let Some(map) = each_page {
            if map != ctx.notes {
                ctx.notes = map;
                ctx.lists = ListCounters::default();
                ctx.other_lists = ListCounters::default();
                ctx.next_id = 0;
                flow = new_flow(doc, hf.clone());
                flow.run(&mut ctx, None)?;
            }
        }
        let end_other_lists = ctx.other_lists.clone();
        let origins = vec![Origin::Fresh; flow.pages.len()];
        let mut st = State {
            opts,
            layout: LayoutDocument { pages: Vec::new() },
            meta: Vec::new(),
            cps: std::mem::take(&mut flow.cps),
            built: std::mem::take(&mut flow.built),
            foot_cache: std::mem::take(&mut flow.foot_cache),
            foot_order: std::mem::take(&mut flow.foot_order),
            end_other_lists,
            section_start: std::mem::take(&mut flow.section_start),
            hf_heights: hf,
            // Differs from the scan with per-page numbering (§17.11.19 eachPage).
            notes: ctx.notes.clone(),
            scan_notes: sc.notes.clone(),
            endnote_order: sc.endnote_order.clone(),
            total,
            hf_fields,
        };
        let pages = std::mem::take(&mut flow.pages);
        let (pages, meta, _) = self.decorate(&mut ctx, &st, pages, &origins, Vec::new(), None)?;
        st.layout.pages = pages;
        st.meta = meta;
        self.st = Some(st);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn incremental(
        &mut self,
        doc: &Document,
        fonts: &mut FontManager,
        cache: &mut LayoutCache,
        opts: LayoutOptions,
        changes: &BlockChanges,
        sc: &Scan,
        mut st: State,
    ) -> Result<Option<Vec<usize>>, LayoutError> {
        let Some(first) = changes
            .sections()
            .iter()
            .min_by_key(|c| (c.section, c.start))
        else {
            // Only headers/footers changed: decorate the pages again.
            let Some(from) = changes.headers() else {
                self.st = Some(st);
                return Ok(Some(Vec::new()));
            };
            let mut ctx = new_ctx(doc, fonts, cache, opts, sc);
            let pages = std::mem::take(&mut st.layout.pages);
            let origins: Vec<Origin> = (0..pages.len())
                .map(|old| Origin::Reused { old, moved: false })
                .collect();
            let old_meta = std::mem::take(&mut st.meta);
            let (pages, meta, dirty) =
                self.decorate(&mut ctx, &st, pages, &origins, old_meta, Some(from))?;
            st.layout.pages = pages;
            st.meta = meta;
            self.st = Some(st);
            self.last = UpdateKind::Incremental { resynced: true };
            return Ok(Some(dirty));
        };
        let Some(last) = changes.sections().iter().max_by_key(|c| c.section) else {
            return Ok(None);
        };
        // Restart from the last checkpoint at or before the first changed block (a
        // keepNext lookahead leaves no checkpoint, so this also covers the paragraph
        // kept with the changed one).
        let mut restart = None;
        'find: for si in (0..=first.section).rev() {
            let v = &st.cps[si];
            let hi = if si == first.section {
                first.start.min(v.len().saturating_sub(1))
            } else {
                v.len().saturating_sub(1)
            };
            for b in (0..=hi).rev() {
                if let Some(Some(cp)) = v.get(b) {
                    restart = Some((si, b, cp.clone()));
                    break 'find;
                }
            }
        }
        let Some((rs, rb, cp)) = restart else {
            return Ok(None);
        };
        for c in changes.sections() {
            splice_slots(&mut st.cps[c.section], c.start, c.old_end(), c.end);
            splice_slots(&mut st.built[c.section], c.start, c.old_end(), c.end);
        }

        // Pages before the restart point stay; the restart page is cut back to the checkpoint.
        let mut old: Vec<Option<Page>> = std::mem::take(&mut st.layout.pages)
            .into_iter()
            .map(Some)
            .collect();
        if cp.page >= old.len() {
            return Ok(None);
        }
        let mut origins = Vec::with_capacity(old.len() + 4);
        let mut pages = Vec::with_capacity(old.len() + 4);
        for (i, slot) in old.iter_mut().enumerate().take(cp.page) {
            if let Some(p) = slot.take() {
                pages.push(p);
                origins.push(Origin::Reused {
                    old: i,
                    moved: false,
                });
            }
        }
        let Some(mut cur) = old[cp.page].clone() else {
            return Ok(None);
        };
        strip(&mut cur, &st.meta[cp.page]);
        cur.lines.truncate(cp.lines);
        cur.items.truncate(cp.items);
        pages.push(cur);

        let mut ctx = new_ctx(doc, fonts, cache, opts, sc);
        ctx.fields.pages = st.total;
        // Per-page footnote numbers (§17.11.19 numRestart eachPage) of the previous
        // layout: kept when the pages' notes stay the same (checked below).
        ctx.notes.clone_from(&st.notes);
        let mut flow = Flow::new(st.hf_heights.clone(), doc.sections.len());
        flow.pages = pages;
        for (s, v) in st.section_start.iter().enumerate().take(rs + 1) {
            flow.section_start[s] = *v;
        }
        let keys = &st.foot_order[..cp.snap.foot_keys.min(st.foot_order.len())];
        flow.foot_order = keys.to_vec();
        flow.foot_cache = keys
            .iter()
            .filter_map(|k| st.foot_cache.get(k).map(|s| (*k, s.clone())))
            .collect();
        flow.old_foot_order = std::mem::take(&mut st.foot_order);
        flow.built = std::mem::take(&mut st.built);
        flow.cps = std::mem::take(&mut st.cps);
        flow.resync_from = (0..doc.sections.len())
            .map(|si| match si.cmp(&last.section) {
                std::cmp::Ordering::Less => usize::MAX,
                std::cmp::Ordering::Equal => last.end,
                std::cmp::Ordering::Greater => 0,
            })
            .collect();
        // Renumbering reused pages is exact unless page numbers show in the body or
        // decide blank pages (odd/even section breaks, §17.18.77).
        flow.allow_renumber = !sc.body_fields
            && !doc
                .sections
                .iter()
                .any(|s| matches!(s.break_type, SectionBreak::OddPage | SectionBreak::EvenPage));
        if self.no_resync {
            flow.resync_from = vec![usize::MAX; doc.sections.len()];
        }
        flow.restore(&mut ctx, &cp);
        flow.run(&mut ctx, Some((rs, rb)))?;

        let mut pages = std::mem::take(&mut flow.pages);
        origins.resize(pages.len(), Origin::Fresh);
        let mut cps = std::mem::take(&mut flow.cps);
        let mut section_start = std::mem::take(&mut flow.section_start);
        let end_other_lists;
        self.last = UpdateKind::Incremental {
            resynced: flow.resynced.is_some(),
        };
        if let Some(rsync) = flow.resynced.take() {
            let (o, n) = (&rsync.old, &rsync.new);
            let off = n.page as i64 - o.page as i64;
            let dn = i64::from(n.number) - i64::from(o.number);
            let did = i64::from(n.next_id) - i64::from(o.next_id);
            let dl = n.lines as i64 - o.lines as i64;
            let di = n.items as i64 - o.items as i64;
            // The page where the flows meet: new content so far + the previous rest.
            let Some(mut tail) = old[o.page].take() else {
                return Ok(None);
            };
            strip(&mut tail, &st.meta[o.page]);
            shift_ids(&mut tail, did);
            if let Some(p) = pages.last_mut() {
                p.lines
                    .extend(tail.lines.drain(o.lines.min(tail.lines.len())..));
                p.items
                    .extend(tail.items.drain(o.items.min(tail.items.len())..));
            }
            // Later pages are reused, renumbered until a section restarts numbering.
            let restarts: HashMap<usize, bool> = st
                .section_start
                .iter()
                .enumerate()
                .filter_map(|(s, v)| v.map(|p| (p, doc.sections[s].page_number_start.is_some())))
                .collect();
            let mut renumber = dn != 0;
            let mut renumbered = vec![false; old.len()];
            for (oi, slot) in old.iter_mut().enumerate().skip(o.page + 1) {
                let Some(mut p) = slot.take() else { continue };
                if restarts.get(&oi) == Some(&true) {
                    renumber = false;
                }
                if renumber {
                    p.number = (i64::from(p.number) + dn) as u32;
                    renumbered[oi] = true;
                }
                shift_ids(&mut p, did);
                pages.push(p);
                origins.push(Origin::Reused {
                    old: oi,
                    moved: off != 0 || renumbered[oi],
                });
            }
            for (s, v) in section_start.iter_mut().enumerate().skip(rsync.section + 1) {
                *v = st.section_start[s].map(|p| (p as i64 + off) as usize);
            }
            // Checkpoints after the meeting point still describe the previous layout.
            for (si, v) in cps.iter_mut().enumerate().skip(rsync.section) {
                let from = if si == rsync.section {
                    rsync.block + 1
                } else {
                    0
                };
                for c in v.iter_mut().skip(from).flatten() {
                    if c.page == o.page {
                        c.lines = (c.lines as i64 + dl) as usize;
                        c.items = (c.items as i64 + di) as usize;
                    }
                    if renumbered.get(c.page).copied().unwrap_or(false)
                        || (c.page == o.page && dn != 0)
                    {
                        c.number = (i64::from(c.number) + dn) as u32;
                    }
                    c.page = (c.page as i64 + off) as usize;
                    c.next_id = (i64::from(c.next_id) + did) as u32;
                }
            }
            st.foot_order = std::mem::take(&mut flow.old_foot_order);
            end_other_lists = st.end_other_lists.clone();
        } else {
            st.foot_cache = std::mem::take(&mut flow.foot_cache);
            st.foot_order = std::mem::take(&mut flow.foot_order);
            end_other_lists = ctx.other_lists.clone();
        }
        // Footnotes moved to other pages renumber with eachPage: lay out again.
        if !self.probe_mode {
            if let Some(map) = crate::page_note_numbers(doc, &pages, &ctx.notes) {
                if map != ctx.notes {
                    return Ok(None);
                }
            }
        }
        st.cps = cps;
        st.built = std::mem::take(&mut flow.built);
        st.section_start = section_start;
        st.end_other_lists = end_other_lists;
        let old_meta = std::mem::take(&mut st.meta);
        let (pages, meta, dirty) =
            self.decorate(&mut ctx, &st, pages, &origins, old_meta, changes.headers())?;
        st.layout.pages = pages;
        st.meta = meta;
        self.st = Some(st);
        Ok(Some(dirty))
    }
}

impl IncrementalLayout {
    /// [`decorate_all`], or nothing for the probe layout (no headers/footers).
    fn decorate(
        &self,
        ctx: &mut Ctx<'_>,
        st: &State,
        pages: Vec<Page>,
        origins: &[Origin],
        old_meta: Vec<PageMeta>,
        force_from: Option<usize>,
    ) -> Result<Decorated, LayoutError> {
        if self.probe_mode {
            let dirty = origins
                .iter()
                .enumerate()
                .filter(|(_, o)| !matches!(o, Origin::Reused { moved: false, .. }))
                .map(|(i, _)| i)
                .collect();
            let meta = vec![PageMeta::default(); pages.len()];
            return Ok((pages, meta, dirty));
        }
        decorate_all(ctx, st, pages, origins, old_meta, force_from)
    }
}

/// Header/footer heights per section (they push the body when taller than the
/// margin, ECMA-376 Part 1, §17.6.11 pgMar header/footer) and whether any header
/// or footer has fields.
fn hf_heights(ctx: &mut Ctx<'_>, doc: &Document) -> Result<(Vec<(f32, f32)>, bool), LayoutError> {
    let mut hf = Vec::with_capacity(doc.sections.len());
    let mut hf_fields = false;
    for (si, s) in doc.sections.iter().enumerate() {
        let w = content_width(s);
        let mut heights = (0.0f32, 0.0f32);
        for header in [true, false] {
            for v in 0..3u8 {
                if let Some(blocks) = hf_blocks(doc, si, header, v) {
                    hf_fields |= blocks_have_fields(blocks);
                    let h = stack_blocks(ctx, blocks, w, false, None)?.h;
                    if header {
                        heights.0 = heights.0.max(h);
                    } else {
                        heights.1 = heights.1.max(h);
                    }
                }
            }
        }
        hf.push(heights);
    }
    Ok((hf, hf_fields))
}

/// Pages with their header/footer metadata and the indexes of the pages that changed.
type Decorated = (Vec<Page>, Vec<PageMeta>, Vec<usize>);

/// Apply headers and footers to every page whose header/footer inputs changed, and
/// to every page of section `force_from` and later (edited headers/footers, which
/// later sections may inherit). Returns the pages, their metadata and the indexes
/// of the pages that changed.
fn decorate_all(
    ctx: &mut Ctx<'_>,
    st: &State,
    mut pages: Vec<Page>,
    origins: &[Origin],
    mut old_meta: Vec<PageMeta>,
    force_from: Option<usize>,
) -> Result<Decorated, LayoutError> {
    let doc = ctx.doc;
    let total = pages.len() as u32;
    let mut section_pages: HashMap<usize, u32> = HashMap::new();
    for p in &pages {
        *section_pages.entry(p.section).or_default() += 1;
    }
    let mut running = st.end_other_lists.clone();
    let mut meta = Vec::with_capacity(pages.len());
    let mut dirty = Vec::new();
    for pi in 0..pages.len() {
        if doc.sections.is_empty() {
            meta.push(PageMeta::default());
            dirty.push(pi);
            continue;
        }
        let (si, number) = (pages[pi].section, pages[pi].number);
        let fields = FieldContext {
            page: number,
            pages: total,
            section_pages: section_pages.get(&si).copied().unwrap_or(0),
            codes: ctx.fields.codes,
        };
        let key = HfKey {
            section: si,
            variant: variant(doc, &pages, &st.section_start, pi),
            number: if st.hf_fields { number } else { 0 },
            total: if st.hf_fields { total } else { 0 },
            section_pages: if st.hf_fields {
                fields.section_pages
            } else {
                0
            },
        };
        let origin = origins.get(pi).copied().unwrap_or(Origin::Fresh);
        if let Origin::Reused { old, moved } = origin {
            let m = old_meta
                .get_mut(old)
                .map(std::mem::take)
                .unwrap_or_default();
            let forced = force_from.is_some_and(|f| si >= f);
            if !forced && m.key == key && m.lists_before == running {
                running.clone_from(&m.lists_after);
                meta.push(m);
                if moved {
                    dirty.push(pi);
                }
                continue;
            }
            strip(&mut pages[pi], &m);
        }
        ctx.other_lists.clone_from(&running);
        let (head, foot, items) = decorate(ctx, &mut pages[pi], key.variant, fields)?;
        meta.push(PageMeta {
            head,
            foot,
            items,
            key,
            lists_before: std::mem::replace(&mut running, ctx.other_lists.clone()),
            lists_after: ctx.other_lists.clone(),
        });
        dirty.push(pi);
    }
    Ok((pages, meta, dirty))
}
