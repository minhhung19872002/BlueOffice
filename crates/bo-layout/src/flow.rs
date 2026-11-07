//! Pagination of the main story: sections, columns, keep rules, widow/orphan
//! control, tables split across pages, anchored pictures and footnotes.

use crate::blocks::{
    build_para, deco_items, layout_table, row_band, row_notes, split_row, stack_blocks, Band, Brk,
    Ctx, ParaBlock, Stack, TableBox,
};
use crate::types::{Layer, NoteKey, Page, PageItem};
use crate::LayoutError;
use bo_model::{Block, Color, Image, PageSetup, Section, SectionBreak, Wrap};
use std::collections::HashMap;

const EPS: f32 = 0.01;
/// Space reserved above the footnote area for the separator (ECMA-376 Part 1, §17.11.23 separator).
const NOTE_SEPARATOR: f32 = 12.0;

struct Prev {
    style: Option<String>,
    contextual: bool,
    after: f32,
}

/// Pagination state.
pub(crate) struct Flow {
    pub pages: Vec<Page>,
    setup: PageSetup,
    section: usize,
    number: u32,
    /// (x, width) of each text column.
    cols: Vec<(f32, f32)>,
    col: usize,
    top: f32,
    bottom: f32,
    y: f32,
    col_has_content: bool,
    page_has_content: bool,
    notes_on_page: Vec<NoteKey>,
    foot_h: f32,
    /// Vertical ranges on the current page occupied by top-and-bottom pictures.
    excl: Vec<(f32, f32)>,
    foot_cache: HashMap<NoteKey, Stack>,
    prev: Option<Prev>,
    /// Page index at which each section started on a fresh page.
    pub section_start: Vec<Option<usize>>,
    /// (header height, footer height) per section, used to push the body.
    pub hf_heights: Vec<(f32, f32)>,
}

/// Geometry of a section's body area.
fn body_area(setup: &PageSetup, hf: (f32, f32)) -> (f32, f32) {
    let page_h = setup.height.to_pt();
    let mt = setup.margin_top.to_pt();
    let mb = setup.margin_bottom.to_pt();
    // §17.6.11 pgMar: a negative top/bottom margin is exact (the header does not push the body).
    let top = if mt < 0.0 {
        -mt
    } else {
        mt.max(setup.header.to_pt() + hf.0)
    };
    let bottom = if mb < 0.0 {
        page_h + mb
    } else {
        page_h - mb.max(setup.footer.to_pt() + hf.1)
    };
    (top, bottom.max(top + 36.0))
}

/// Column boxes (§17.6.4 cols): equal widths separated by `space`.
pub(crate) fn columns(setup: &PageSetup, sec: &Section) -> Vec<(f32, f32)> {
    let left = setup.margin_left.to_pt();
    let width =
        (setup.width.to_pt() - setup.margin_left.to_pt() - setup.margin_right.to_pt()).max(36.0);
    let n = usize::from(sec.columns.count.clamp(1, 45));
    let space = sec.columns.space.to_pt().max(0.0);
    let w = ((width - space * (n - 1) as f32) / n as f32).max(18.0);
    (0..n).map(|i| (left + i as f32 * (w + space), w)).collect()
}

/// Position of an anchored picture (§20.4.2.3 anchor, §20.4.2.10 positionH, §20.4.2.11 positionV).
pub(crate) fn float_item(
    img: &Image,
    setup: &PageSetup,
    col_x: f32,
    para_top: f32,
) -> (PageItem, Option<(f32, f32)>) {
    let Some(a) = &img.anchor else {
        return (
            PageItem::Image {
                x: col_x,
                y: para_top,
                w: img.width.to_pt(),
                h: img.height.to_pt(),
                media: img.media.clone(),
                crop: crate::blocks::crop_of(img),
                rotation: crate::blocks::rotation_of(img),
                layer: Layer::Front,
            },
            None,
        );
    };
    let x = match a.h_relative.as_str() {
        "page" | "leftMargin" | "insideMargin" => a.x.to_pt(),
        "rightMargin" | "outsideMargin" => {
            setup.width.to_pt() - setup.margin_right.to_pt() + a.x.to_pt()
        }
        "margin" => setup.margin_left.to_pt() + a.x.to_pt(),
        _ => col_x + a.x.to_pt(),
    };
    let y = match a.v_relative.as_str() {
        "page" | "topMargin" | "insideMargin" => a.y.to_pt(),
        "margin" => setup.margin_top.to_pt().abs() + a.y.to_pt(),
        "bottomMargin" | "outsideMargin" => {
            setup.height.to_pt() - setup.margin_bottom.to_pt().abs() + a.y.to_pt()
        }
        _ => para_top + a.y.to_pt(),
    };
    let w = img.width.to_pt();
    let h = img.height.to_pt();
    let item = PageItem::Image {
        x,
        y,
        w,
        h,
        media: img.media.clone(),
        crop: crate::blocks::crop_of(img),
        rotation: crate::blocks::rotation_of(img),
        layer: if a.behind { Layer::Back } else { Layer::Front },
    };
    // Square/tight wrapping is approximated as top-and-bottom in this version (see phase-2 plan).
    let excl = match a.wrap {
        Wrap::None => None,
        Wrap::Square | Wrap::Tight | Wrap::TopAndBottom => Some((y, y + h)),
    };
    (item, excl)
}

enum Built {
    Para(Box<ParaBlock>),
    Table(TableBox),
    Skip,
}

impl Flow {
    pub fn new(hf_heights: Vec<(f32, f32)>, sections: usize) -> Flow {
        Flow {
            pages: Vec::new(),
            setup: PageSetup::default(),
            section: 0,
            number: 0,
            cols: vec![(0.0, 100.0)],
            col: 0,
            top: 0.0,
            bottom: 0.0,
            y: 0.0,
            col_has_content: false,
            page_has_content: false,
            notes_on_page: Vec::new(),
            foot_h: 0.0,
            excl: Vec::new(),
            foot_cache: HashMap::new(),
            prev: None,
            section_start: vec![None; sections],
            hf_heights,
        }
    }

    fn col_x(&self) -> f32 {
        self.cols[self.col].0
    }

    fn col_w(&self) -> f32 {
        self.cols[self.col].1
    }

    fn push_page(&mut self) {
        self.pages.push(Page {
            width: self.setup.width.to_pt(),
            height: self.setup.height.to_pt(),
            lines: Vec::new(),
            items: Vec::new(),
            number: self.number,
            section: self.section,
        });
        self.col = 0;
        self.y = self.top;
        self.col_has_content = false;
        self.page_has_content = false;
        self.notes_on_page.clear();
        self.foot_h = 0.0;
        self.excl.clear();
        self.prev = None;
    }

    /// Place the footnotes of the current page at its bottom.
    fn finish_page(&mut self) {
        if self.notes_on_page.is_empty() {
            return;
        }
        let x = self.cols[0].0;
        let mut y = self.bottom - self.foot_h;
        let Some(page) = self.pages.last_mut() else {
            return;
        };
        page.items.push(PageItem::Rule {
            x1: x,
            y1: y + NOTE_SEPARATOR / 2.0,
            x2: x + 144.0,
            y2: y + NOTE_SEPARATOR / 2.0,
            width: 0.5,
            color: Color::BLACK,
            style: "single".into(),
        });
        y += NOTE_SEPARATOR;
        for key in &self.notes_on_page {
            if let Some(st) = self.foot_cache.get(key) {
                for (top, band) in &st.bands {
                    let mut b = band.translated(x, y + top);
                    b.set_other();
                    page.lines.extend(b.lines);
                    page.items.extend(b.items);
                }
                y += st.h;
            }
        }
    }

    fn new_page(&mut self) {
        self.finish_page();
        self.number += 1;
        self.push_page();
    }

    fn next_column(&mut self) {
        if self.col + 1 < self.cols.len() {
            self.col += 1;
            self.y = self.top;
            self.col_has_content = false;
            self.prev = None;
        } else {
            self.new_page();
        }
    }

    /// Height added to the footnote area by placing content that references `notes`.
    fn notes_need(&mut self, ctx: &mut Ctx<'_>, notes: &[NoteKey]) -> Result<f32, LayoutError> {
        let mut add = 0.0;
        let mut seen: Vec<NoteKey> = Vec::new();
        for key in notes {
            if key.0 || self.notes_on_page.contains(key) || seen.contains(key) {
                continue;
            }
            seen.push(*key);
            add += self.note_stack(ctx, *key)?;
        }
        if add > 0.0 && self.foot_h <= 0.0 {
            add += NOTE_SEPARATOR;
        }
        Ok(add)
    }

    fn note_stack(&mut self, ctx: &mut Ctx<'_>, key: NoteKey) -> Result<f32, LayoutError> {
        if let Some(s) = self.foot_cache.get(&key) {
            return Ok(s.h);
        }
        let blocks = if key.0 {
            ctx.doc.endnotes.get(&key.1)
        } else {
            ctx.doc.footnotes.get(&key.1)
        };
        let mark = ctx.notes.get(&key).cloned();
        let st = match blocks {
            Some(b) => {
                let width = (self.setup.width.to_pt()
                    - self.setup.margin_left.to_pt()
                    - self.setup.margin_right.to_pt())
                .max(36.0);
                stack_blocks(ctx, b, width, false, mark.as_deref())?
            }
            None => Stack::default(),
        };
        let h = st.h;
        self.foot_cache.insert(key, st);
        Ok(h)
    }

    fn register_notes(&mut self, ctx: &mut Ctx<'_>, notes: &[NoteKey]) -> Result<(), LayoutError> {
        for key in notes {
            if key.0 || self.notes_on_page.contains(key) {
                continue;
            }
            let h = self.note_stack(ctx, *key)?;
            if self.notes_on_page.is_empty() {
                self.foot_h += NOTE_SEPARATOR;
            }
            self.foot_h += h;
            self.notes_on_page.push(*key);
        }
        Ok(())
    }

    fn skip_excl(&self, mut y: f32, h: f32) -> f32 {
        for _ in 0..16 {
            let mut moved = false;
            for &(a, b) in &self.excl {
                if y < b - EPS && y + h > a + EPS {
                    y = b;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        y
    }

    /// Emit a band at (current column x, `y`).
    fn put(&mut self, ctx: &mut Ctx<'_>, band: &Band, y: f32) -> Result<(), LayoutError> {
        self.register_notes(ctx, &band.notes)?;
        let b = band.translated(self.col_x(), y);
        if let Some(page) = self.pages.last_mut() {
            page.lines.extend(b.lines);
            page.items.extend(b.items);
        }
        self.y = y + band.h;
        self.col_has_content = true;
        self.page_has_content = true;
        Ok(())
    }

    fn fits(&self, y: f32, h: f32, notes_add: f32) -> bool {
        y + h <= self.bottom - self.foot_h - notes_add + EPS
    }

    /// Place an unsplittable band, moving to the next column/page when needed.
    fn place_band(&mut self, ctx: &mut Ctx<'_>, band: &Band) -> Result<(), LayoutError> {
        loop {
            let add = self.notes_need(ctx, &band.notes)?;
            let y = self.skip_excl(self.y, band.h);
            if self.fits(y, band.h, add) || !self.col_has_content {
                self.put(ctx, band, y)?;
                break;
            }
            self.next_column();
        }
        match band.brk {
            Brk::Page => self.new_page(),
            Brk::Column => self.next_column(),
            Brk::None => {}
        }
        Ok(())
    }

    /// How many of `lines` fit from the current position.
    fn fit_count(&mut self, ctx: &mut Ctx<'_>, lines: &[Band]) -> Result<usize, LayoutError> {
        let mut y = self.y;
        let mut foot = self.foot_h;
        let mut seen: Vec<NoteKey> = self.notes_on_page.clone();
        for (i, l) in lines.iter().enumerate() {
            for key in &l.notes {
                if !key.0 && !seen.contains(key) {
                    if seen.is_empty() {
                        foot += NOTE_SEPARATOR;
                    }
                    foot += self.note_stack(ctx, *key)?;
                    seen.push(*key);
                }
            }
            let ly = self.skip_excl(y, l.h);
            if ly + l.h > self.bottom - foot + EPS {
                return Ok(i);
            }
            y = ly + l.h;
            if l.brk != Brk::None {
                return Ok(i + 1);
            }
        }
        Ok(lines.len())
    }

    fn full_height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Place a paragraph applying spacing, keep and widow rules
    /// (ECMA-376 Part 1, §17.3.1.14 keepNext, §17.3.1.15 keepLines, §17.3.1.44 widowControl).
    fn place_para(
        &mut self,
        ctx: &mut Ctx<'_>,
        pb: &ParaBlock,
        keep_with: f32,
    ) -> Result<(), LayoutError> {
        if pb.page_break_before && self.page_has_content {
            self.new_page();
        }
        let mut before = pb.space_before;
        if let Some(p) = &self.prev {
            if p.style == pb.style {
                if pb.contextual {
                    before = 0.0;
                }
                if p.contextual {
                    self.y -= p.after;
                }
            }
        }
        if self.col_has_content {
            self.y += before;
        }
        let total = pb.height();
        let full = self.full_height();
        let keep_h = if pb.keep_next {
            total + keep_with
        } else {
            total
        };
        if (pb.keep_lines || pb.keep_next)
            && self.col_has_content
            && keep_h <= full
            && !self.fits(self.y, keep_h, 0.0)
        {
            self.next_column();
        }

        // Anchored pictures are positioned relative to the paragraph's first line.
        let mut float_items = Vec::new();
        for img in &pb.floats {
            let (item, excl) = float_item(img, &self.setup, self.col_x(), self.y);
            float_items.push(item);
            if let Some(e) = excl {
                self.excl.push(e);
            }
        }
        if let Some(page) = self.pages.last_mut() {
            page.items.extend(float_items);
        }

        let n = pb.lines.len();
        let mut i = 0;
        let mut segments: Vec<(usize, f32, f32, f32)> = Vec::new(); // (page, col x, y0, y1)
        while i < n {
            let fit = self.fit_count(ctx, &pb.lines[i..])?;
            let remaining = n - i;
            let mut take = fit.min(remaining);
            let breaks_inside = pb.lines[i..i + take].iter().any(|l| l.brk != Brk::None);
            if take < remaining && !breaks_inside {
                if pb.widow_control && remaining >= 2 {
                    if remaining - take == 1 && take >= 1 {
                        take -= 1; // widow: never leave the last line alone
                    }
                    if take == 1 && i == 0 {
                        take = 0; // orphan: never leave the first line alone
                    }
                }
                if take == 0 && self.col_has_content {
                    self.next_column();
                    continue;
                }
                take = take.max(1);
            }
            let page_idx = self.pages.len() - 1;
            let col_x = self.col_x();
            let mut y0 = None;
            for line in &pb.lines[i..i + take] {
                let y = self.skip_excl(self.y, line.h);
                y0.get_or_insert(y);
                self.put(ctx, line, y)?;
                i += 1;
                if line.brk != Brk::None {
                    break;
                }
            }
            if let Some(y0) = y0 {
                segments.push((page_idx, col_x, y0, self.y));
            }
            let last = &pb.lines[i - 1];
            match last.brk {
                Brk::Page => self.new_page(),
                Brk::Column => self.next_column(),
                Brk::None if i < n => self.next_column(),
                Brk::None => {}
            }
        }
        if let Some(d) = &pb.deco {
            for (pi, cx, y0, y1) in segments {
                let mut items = Vec::new();
                deco_items(d, y0, y1, &mut items);
                if let Some(page) = self.pages.get_mut(pi) {
                    for mut it in items {
                        it.translate(cx, 0.0);
                        page.items.push(it);
                    }
                }
            }
        }
        self.y += pb.space_after;
        self.prev = Some(Prev {
            style: pb.style.clone(),
            contextual: pb.contextual,
            after: pb.space_after,
        });
        Ok(())
    }

    fn place_row(
        &mut self,
        ctx: &mut Ctx<'_>,
        mut band: Band,
        repeat: bool,
    ) -> Result<(), LayoutError> {
        if repeat {
            band.set_other();
            band.notes.clear();
        }
        let y = self.y;
        self.put(ctx, &band, y)
    }

    /// Place a table row by row; rows split across pages unless `cantSplit`, and
    /// `tblHeader` rows repeat at the top of each page (§17.4.49 tblHeader).
    fn place_table(&mut self, ctx: &mut Ctx<'_>, tb: &TableBox) -> Result<(), LayoutError> {
        let header_count = tb.rows.iter().take_while(|r| r.header).count();
        let mut idx = 0;
        while idx < tb.rows.len() {
            let mut row = tb.rows[idx].clone();
            let mut continuation = false;
            loop {
                let notes = row_notes(&row);
                let add = self.notes_need(ctx, &notes)?;
                self.y = self.skip_excl(self.y, row.h);
                if self.fits(self.y, row.h, add) {
                    self.place_row(ctx, row_band(&row, row.h, tb.x, continuation, false), false)?;
                    break;
                }
                let avail = self.bottom - self.foot_h - add - self.y;
                if !row.cant_split && avail > 6.0 {
                    if let Some((a, b)) = split_row(&row, avail) {
                        self.place_row(ctx, row_band(&a, a.h, tb.x, continuation, true), false)?;
                        self.next_column();
                        self.repeat_headers(ctx, tb, header_count, idx)?;
                        row = b;
                        continuation = true;
                        continue;
                    }
                }
                if !self.col_has_content {
                    self.place_row(ctx, row_band(&row, row.h, tb.x, continuation, false), false)?;
                    break;
                }
                self.next_column();
                self.repeat_headers(ctx, tb, header_count, idx)?;
                continuation = continuation || idx > 0;
            }
            idx += 1;
        }
        self.prev = None;
        Ok(())
    }

    fn repeat_headers(
        &mut self,
        ctx: &mut Ctx<'_>,
        tb: &TableBox,
        header_count: usize,
        idx: usize,
    ) -> Result<(), LayoutError> {
        if idx < header_count || header_count == 0 {
            return Ok(());
        }
        for r in &tb.rows[..header_count] {
            self.place_row(ctx, row_band(r, r.h, tb.x, false, false), true)?;
        }
        Ok(())
    }

    fn build(&mut self, ctx: &mut Ctx<'_>, b: &Block) -> Result<Built, LayoutError> {
        ctx.fields.page = self.number;
        Ok(match b {
            Block::Paragraph(p) => {
                Built::Para(Box::new(build_para(ctx, p, self.col_w(), true, None)?))
            }
            Block::Table(t) => Built::Table(layout_table(ctx, t, self.col_w(), true)?),
            Block::Opaque(_) => Built::Skip,
        })
    }

    /// Start section `si`.
    fn start_section(&mut self, si: usize, sec: &Section, first: bool) {
        let same_size = self.setup.width == sec.page.width && self.setup.height == sec.page.height;
        let continuous = !first
            && matches!(
                sec.break_type,
                SectionBreak::Continuous | SectionBreak::NextColumn
            )
            && same_size;
        if !first && !continuous {
            self.finish_page();
        }
        self.setup = sec.page;
        let hf = self.hf_heights.get(si).copied().unwrap_or((0.0, 0.0));
        let (top, bottom) = body_area(&sec.page, hf);
        self.cols = columns(&sec.page, sec);
        self.top = top;
        self.bottom = bottom;
        if continuous {
            self.section = si;
            self.col = 0;
            if sec.break_type == SectionBreak::NextColumn && self.col_has_content {
                self.next_column();
            }
            return;
        }
        self.section = si;
        self.number = match sec.page_number_start {
            Some(n) => n,
            None => self.number + 1,
        };
        // §17.18.77 oddPage / evenPage: insert a blank page to reach the right parity.
        let want_odd = match sec.break_type {
            SectionBreak::OddPage => Some(true),
            SectionBreak::EvenPage => Some(false),
            _ => None,
        };
        if let Some(odd) = want_odd {
            if !first && (self.number % 2 == 1) != odd {
                self.push_page();
                self.number += 1;
            }
        }
        self.push_page();
        self.section_start[si] = Some(self.pages.len() - 1);
    }

    /// Paginate all sections.
    pub fn run(&mut self, ctx: &mut Ctx<'_>) -> Result<(), LayoutError> {
        let doc = ctx.doc;
        for (si, sec) in doc.sections.iter().enumerate() {
            self.start_section(si, sec, si == 0);
            let mut next: Option<Built> = None;
            let mut k = 0;
            while k < sec.blocks.len() {
                let cur = match next.take() {
                    Some(b) => b,
                    None => self.build(ctx, &sec.blocks[k])?,
                };
                // One-block lookahead for keepNext.
                let mut keep_with = 0.0;
                if let Built::Para(pb) = &cur {
                    if pb.keep_next && k + 1 < sec.blocks.len() {
                        let nb = self.build(ctx, &sec.blocks[k + 1])?;
                        keep_with = match &nb {
                            Built::Para(n) => n.space_before + n.lines.first().map_or(0.0, |l| l.h),
                            Built::Table(t) => t.rows.first().map_or(0.0, |r| r.h),
                            Built::Skip => 0.0,
                        };
                        next = Some(nb);
                    }
                }
                match &cur {
                    Built::Para(pb) => self.place_para(ctx, pb, keep_with)?,
                    Built::Table(tb) => self.place_table(ctx, tb)?,
                    Built::Skip => {}
                }
                k += 1;
            }
        }
        if self.pages.is_empty() {
            self.number = 1;
            self.push_page();
        }
        self.place_endnotes(ctx)?;
        self.finish_page();
        Ok(())
    }

    /// Endnotes at the end of the document (§17.11.4 endnotePr default position docEnd).
    fn place_endnotes(&mut self, ctx: &mut Ctx<'_>) -> Result<(), LayoutError> {
        let keys: Vec<NoteKey> = ctx
            .endnote_order
            .iter()
            .filter(|id| ctx.doc.endnotes.contains_key(id))
            .map(|id| (true, *id))
            .collect();
        if keys.is_empty() {
            return Ok(());
        }
        let mut sep = Band::empty(NOTE_SEPARATOR);
        sep.items.push(PageItem::Rule {
            x1: 0.0,
            y1: NOTE_SEPARATOR / 2.0,
            x2: 144.0,
            y2: NOTE_SEPARATOR / 2.0,
            width: 0.5,
            color: Color::BLACK,
            style: "single".into(),
        });
        self.place_band(ctx, &sep)?;
        for key in keys {
            let blocks = ctx.doc.endnotes.get(&key.1).cloned().unwrap_or_default();
            let mark = ctx.notes.get(&key).cloned();
            let st = stack_blocks(ctx, &blocks, self.col_w(), false, mark.as_deref())?;
            let mut last_bottom = 0.0f32;
            for (top, band) in &st.bands {
                // Keep the vertical gaps between bands (paragraph spacing).
                self.y += (top - last_bottom).max(0.0);
                let mut b = band.clone();
                b.set_other();
                self.place_band(ctx, &b)?;
                last_bottom = top + band.h;
            }
            self.y += (st.h - last_bottom).max(0.0);
        }
        Ok(())
    }
}

/// Header/footer lookup with inheritance from previous sections (§17.10.5 headerReference).
pub(crate) fn hf_blocks(
    doc: &bo_model::Document,
    si: usize,
    header: bool,
    variant: u8,
) -> Option<&Vec<Block>> {
    for s in doc.sections[..=si.min(doc.sections.len().saturating_sub(1))]
        .iter()
        .rev()
    {
        let set = if header { &s.headers } else { &s.footers };
        let v = match variant {
            1 => &set.first,
            2 => &set.even,
            _ => &set.default,
        };
        if let Some(b) = v {
            return Some(b);
        }
    }
    None
}
