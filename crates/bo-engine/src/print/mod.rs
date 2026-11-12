//! One print pipeline for BlueDoc, BlueSlide and BlueSheet (CORE-PRINT-001).
//!
//! A print job turns the document into *logical pages* (document pages,
//! slides, notes pages, handouts, outline pages or printed sheet pages), then:
//! 1. keeps the chosen pages (all / current / selection / a range text, see
//!    [`range`]) and the odd or even ones;
//! 2. applies the colour mode ([`color`]);
//! 3. puts them on sheets of paper, several per sheet or scaled to a paper
//!    size ([`nup`]);
//! 4. reverses the sheets and repeats them for the copies (collated: the whole
//!    set again; not collated: each sheet several times in a row).
//!
//! The PDF that is printed and the print panel's preview come from the same
//! sheet list: the preview shows sheet `i` of exactly the pages the PDF holds.

pub mod color;
mod doc;
pub mod nup;
pub mod range;
mod slide;

use crate::{Engine, EngineError, Result};
use bo_render::{DisplayList, PageList};
use color::ColorMode;
use nup::NupOrder;
use range::PageKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Which pages to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageChoice {
    /// Every page.
    #[default]
    All,
    /// The page (slide) shown in the editor: [`PrintOptions::current`].
    Current,
    /// The host's selection: [`PrintOptions::selected`] (slides); BlueSheet
    /// prints the selected cells instead ([`SheetPrint::selection`]).
    Selection,
    /// The pages of [`PrintOptions::range`].
    Range,
}

/// Odd or even pages only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Subset {
    /// Every page.
    #[default]
    All,
    /// Pages 1, 3, 5, …
    Odd,
    /// Pages 2, 4, 6, …
    Even,
}

/// Paper sizes of the "scale to paper" choice (portrait; turned as needed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaperSize {
    /// ISO 216 A3, 297 × 420 mm.
    A3,
    /// ISO 216 A4, 210 × 297 mm.
    A4,
    /// ISO 216 A5, 148 × 210 mm.
    A5,
    /// JIS B5, 182 × 257 mm.
    B5,
    /// US Letter, 8.5 × 11 in.
    Letter,
    /// US Legal, 8.5 × 14 in.
    Legal,
}

impl PaperSize {
    /// Width and height in points (1 in = 72 pt = 25.4 mm).
    pub fn size(self) -> (f32, f32) {
        let mm = |w: f32, h: f32| (w / 25.4 * 72.0, h / 25.4 * 72.0);
        match self {
            PaperSize::A3 => mm(297.0, 420.0),
            PaperSize::A4 => mm(210.0, 297.0),
            PaperSize::A5 => mm(148.0, 210.0),
            PaperSize::B5 => mm(182.0, 257.0),
            PaperSize::Letter => (612.0, 792.0),
            PaperSize::Legal => (612.0, 1008.0),
        }
    }
}

/// BlueDoc choices.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocPrint {
    /// Print tracked changes and comments as the view shows them (change bars
    /// and balloons in a markup area right of every page); off prints the
    /// text with every change accepted and no marks.
    pub markup: bool,
    /// Add a page listing the document properties after the pages.
    pub properties: bool,
    /// Labels of the properties page by key (`title`, `author`, …), in the
    /// host's language.
    pub labels: BTreeMap<String, String>,
    /// Print the page colour (ECMA-376 Part 1, §17.2.1 background). Off by
    /// default, as word processors leave background colours out of print jobs.
    pub background: bool,
}

impl Default for DocPrint {
    fn default() -> Self {
        Self {
            markup: true,
            properties: false,
            labels: BTreeMap::new(),
            background: false,
        }
    }
}

/// What a BlueSlide job prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlideLayout {
    /// One slide per page.
    #[default]
    Slides,
    /// Notes pages (slide + speaker notes).
    Notes,
    /// Titles and body text of the slides.
    Outline,
    /// Handouts with several slides per page.
    Handouts,
}

/// BlueSlide choices.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SlidePrint {
    /// What to print.
    pub layout: SlideLayout,
    /// Slides per handout page (1, 2, 3, 4, 6 or 9).
    pub handout_per_page: u8,
    /// Order of the slides on a handout page.
    pub handout_order: crate::slide_export::handout::Order,
    /// Print hidden slides (`p:sld/@show="0"`, ECMA-376 Part 1, §19.3.1.38).
    pub hidden: bool,
}

impl Default for SlidePrint {
    fn default() -> Self {
        Self {
            layout: SlideLayout::Slides,
            handout_per_page: 6,
            handout_order: crate::slide_export::handout::Order::Horizontal,
            hidden: false,
        }
    }
}

/// Which sheets a BlueSheet job prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SheetScope {
    /// The sheets in [`SheetPrint::sheets`] (the active one).
    #[default]
    Active,
    /// Every visible sheet.
    Workbook,
    /// The selected cells of the first sheet in [`SheetPrint::sheets`].
    Selection,
}

/// BlueSheet choices; `None` keeps the sheet's own page setup.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetPrint {
    /// Which sheets.
    pub scope: SheetScope,
    /// 0-based sheets of `Active` / `Selection`.
    pub sheets: Vec<usize>,
    /// Selected cells (`B2:D10`) for `Selection`.
    pub selection: Option<String>,
    /// Print gridlines (§18.3.1.70 printOptions@gridLines).
    pub gridlines: Option<bool>,
    /// Print row and column headings (§18.3.1.70 printOptions@headings).
    pub headings: Option<bool>,
    /// Rows repeated on every page (`1:2`; empty = none), §18.2.6 `_xlnm.Print_Titles`.
    pub title_rows: Option<String>,
    /// Columns repeated on every page (`A:B`; empty = none).
    pub title_cols: Option<String>,
    /// Print the used cells instead of the print areas (`_xlnm.Print_Area`).
    pub ignore_print_area: bool,
}

/// A print job (JSON from the print panel).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PrintOptions {
    /// Which pages.
    pub pages: PageChoice,
    /// Range text for [`PageChoice::Range`].
    pub range: String,
    /// 0-based current page or slide.
    pub current: usize,
    /// 0-based selected pages or slides.
    pub selected: Vec<usize>,
    /// Odd / even pages only.
    pub subset: Subset,
    /// Number of copies (1–999).
    pub copies: u32,
    /// Collate the copies.
    pub collate: bool,
    /// Last sheet first.
    pub reverse: bool,
    /// Pages per sheet: 1, 2, 4, 6, 8 or 16.
    pub per_sheet: u8,
    /// Order of the pages on a sheet.
    pub order: NupOrder,
    /// Scale to this paper (`None`: one page per sheet keeps its size; several
    /// per sheet use the first page's size).
    pub paper: Option<PaperSize>,
    /// Thin frame around every placed page (BlueSlide: frame slides).
    pub frame: bool,
    /// Colour mode.
    pub color: ColorMode,
    /// BlueDoc choices.
    pub doc: DocPrint,
    /// BlueSlide choices.
    pub slide: SlidePrint,
    /// BlueSheet choices.
    pub sheet: SheetPrint,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            pages: PageChoice::All,
            range: String::new(),
            current: 0,
            selected: Vec::new(),
            subset: Subset::All,
            copies: 1,
            collate: true,
            reverse: false,
            per_sheet: 1,
            order: NupOrder::Across,
            paper: None,
            frame: false,
            color: ColorMode::Color,
            doc: DocPrint::default(),
            slide: SlidePrint::default(),
            sheet: SheetPrint::default(),
        }
    }
}

impl PrintOptions {
    /// Options from the bridge JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))
    }

    fn copies(&self) -> usize {
        self.copies.clamp(1, 999) as usize
    }
}

/// A page before it is put on paper, with its number (1-based) for odd / even.
pub(crate) type Logical = (usize, PageList);

/// 0-based indices among `total` pages chosen by `o` (`keys`: page numbers
/// and sections, BlueDoc).
pub(crate) fn choose(
    o: &PrintOptions,
    total: usize,
    keys: Option<&[PageKey]>,
) -> Result<Vec<usize>> {
    Ok(match o.pages {
        PageChoice::All => (0..total).collect(),
        PageChoice::Current => (o.current < total)
            .then_some(o.current)
            .into_iter()
            .collect(),
        PageChoice::Selection => o.selected.iter().copied().filter(|&i| i < total).collect(),
        PageChoice::Range => range::parse_ranges(&o.range, total, keys)
            .map_err(|e| EngineError::BadCommand(e.to_string()))?,
    })
}

/// Steps 1–3 of the pipeline (after the page choice) and the reversal: the
/// sheets of one copy.
pub(crate) fn finish(pages: Vec<Logical>, o: &PrintOptions, frame: bool) -> Vec<PageList> {
    let pages: Vec<PageList> = pages
        .into_iter()
        .filter(|(n, _)| match o.subset {
            Subset::All => true,
            Subset::Odd => n % 2 == 1,
            Subset::Even => n % 2 == 0,
        })
        .map(|(_, mut p)| {
            color::apply(o.color, &mut p.commands);
            p
        })
        .collect();
    let mut sheets = nup::compose(
        pages,
        o.per_sheet,
        o.order,
        o.paper.map(PaperSize::size),
        frame,
    );
    if o.reverse {
        sheets.reverse();
    }
    sheets
}

/// Every printed sheet: the copies of one set.
pub(crate) fn with_copies(sheets: Vec<PageList>, o: &PrintOptions) -> Vec<PageList> {
    let n = o.copies();
    if n == 1 {
        return sheets;
    }
    let mut out = Vec::with_capacity(sheets.len() * n);
    if o.collate {
        for _ in 0..n {
            out.extend(sheets.iter().cloned());
        }
    } else {
        for s in sheets {
            for _ in 1..n {
                out.push(s.clone());
            }
            out.push(s);
        }
    }
    out
}

/// One sheet of the print preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSheet<'a> {
    /// Sheets in one copy.
    pub sheets: usize,
    /// Copies.
    pub copies: usize,
    /// Index of `page`.
    pub index: usize,
    /// The sheet (`None` when nothing is printed).
    pub page: Option<&'a PageList>,
}

/// Sheets of the last previewed job, keyed by its options JSON (the panel
/// adds a job number that changes whenever it opens).
#[derive(Debug, Default)]
pub(crate) struct PreviewCache {
    key: String,
    sheets: Vec<PageList>,
}

impl PreviewCache {
    /// Preview JSON of sheet `index`, building the sheets with `build` when the
    /// options changed.
    pub(crate) fn preview(
        &mut self,
        json: &str,
        index: usize,
        build: impl FnOnce(&PrintOptions) -> Result<Vec<PageList>>,
    ) -> Result<String> {
        let o = PrintOptions::from_json(json)?;
        if self.key != json {
            self.sheets = build(&o)?;
            self.key = json.to_owned();
        }
        let n = self.sheets.len();
        let index = index.min(n.saturating_sub(1));
        serde_json::to_string(&PreviewSheet {
            sheets: n,
            copies: o.copies(),
            index,
            page: self.sheets.get(index),
        })
        .map_err(|e| EngineError::BadCommand(e.to_string()))
    }

    /// Forget the cached sheets.
    pub(crate) fn clear(&mut self) {
        self.key.clear();
        self.sheets.clear();
    }
}

/// Error of a job that prints nothing.
pub(crate) fn nothing() -> EngineError {
    EngineError::BadCommand("print: no pages selected".into())
}

impl Engine {
    /// The sheets of one copy of a print job (BlueDoc or BlueSlide).
    pub fn print_sheets(&mut self, o: &PrintOptions) -> Result<Vec<PageList>> {
        let (pages, frame) = if self.pres.is_some() {
            self.slide_print_pages(o)?
        } else {
            (self.doc_print_pages(o)?, o.frame)
        };
        Ok(finish(pages, o, frame))
    }

    /// PDF of a print job: every sheet of every copy.
    pub fn print_pdf(&mut self, o: &PrintOptions) -> Result<Vec<u8>> {
        let sheets = with_copies(self.print_sheets(o)?, o);
        if sheets.is_empty() {
            return Err(nothing());
        }
        let title = self
            .name
            .rsplit_once('.')
            .map_or(self.name.clone(), |(b, _)| b.to_owned());
        Ok(bo_pdf::write_pdf_with_images(
            &DisplayList { pages: sheets },
            &self.fonts,
            &bo_pdf::PdfMeta { title: Some(title) },
            &self.image_source(),
        )?)
    }

    /// JSON variant of [`Engine::print_pdf`] (the WASM / Tauri bridge).
    pub fn print_pdf_json(&mut self, json: &str) -> Result<Vec<u8>> {
        self.print_pdf(&PrintOptions::from_json(json)?)
    }

    /// Print preview: sheet `index` of the job in `json` with the sheet count
    /// (JSON `{"sheets","copies","index","page"}`).
    pub fn print_preview_json(&mut self, json: &str, index: usize) -> Result<String> {
        let mut cache = std::mem::take(&mut self.print_cache);
        let r = cache.preview(json, index, |o| self.print_sheets(o));
        if r.is_err() {
            cache.clear();
        }
        self.print_cache = cache;
        r
    }
}

#[cfg(test)]
mod tests;
