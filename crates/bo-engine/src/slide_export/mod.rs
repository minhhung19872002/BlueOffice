//! BlueSlide exports beyond the slide-per-page PDF: handouts (1/2/3/4/6/9
//! slides per page, header/footer from the handout master, ECMA-376 Part 1,
//! §19.3.1.24 `handoutMaster`), notes pages (slide + speaker notes, from the
//! notes master, §19.3.1.27 `notesMaster`), slide images (PNG, JPEG, SVG) and
//! a presentation package (zip of the `.pptx`, its media clips and the
//! embeddable bundled fonts).
//!
//! Handouts and notes pages are composed as display lists: every slide's own
//! display list is drawn into its frame with `PushClip` + `PushTransform`, so
//! the PDF, the raster and the SVG backends all draw the same pages.

pub mod handout;
pub(crate) mod masters;
mod package;
pub mod svg;
pub(crate) mod text;

use crate::{Engine, EngineError, Result};
use bo_render::{Command, DisplayList, PageList, PathSeg, Rgba, Stroke};
use handout::{Frame, Order};
use masters::{HfKind, MasterInfo};
use serde::Deserialize;

/// EMU per point (ECMA-376 Part 1, §20.1.2.1: 12 700 EMU = 1 pt).
const EMU_PER_PT: f32 = 12_700.0;

/// Paper of handout and notes pages (portrait).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Paper {
    /// ISO 216 A4, 210 × 297 mm.
    #[default]
    A4,
    /// US Letter, 8.5 × 11 in.
    Letter,
    /// The presentation's notes size (§19.2.1.23 `notesSz`).
    Notes,
}

impl Paper {
    /// Page size in points.
    pub fn size(self, notes_size: (f32, f32)) -> (f32, f32) {
        match self {
            Paper::A4 => (210.0 / 25.4 * 72.0, 297.0 / 25.4 * 72.0),
            Paper::Letter => (612.0, 792.0),
            Paper::Notes => notes_size,
        }
    }
}

/// Header / footer overrides. `None` takes the item from the master (when the
/// master has the placeholder and its `p:hf` flag is on); `Some("")` hides it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeaderFooter {
    /// Header text (top left).
    pub header: Option<String>,
    /// Footer text (bottom left).
    pub footer: Option<String>,
    /// Date text (top right), already formatted by the host.
    pub date: Option<String>,
    /// Page numbers (bottom right).
    pub page_numbers: Option<bool>,
}

/// Options of a handout export.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HandoutOptions {
    /// Slides per page: 1, 2, 3, 4, 6 or 9.
    pub per_page: u8,
    /// Reading order of the slides on a page.
    pub order: Order,
    /// Draw a thin frame around every slide.
    pub frame: bool,
    /// Paper size.
    pub paper: Paper,
    /// Header and footer.
    #[serde(flatten)]
    pub hf: HeaderFooter,
    /// 0-based slides to put on the pages, in order (`None`: every slide).
    pub slides: Option<Vec<usize>>,
}

impl Default for HandoutOptions {
    fn default() -> Self {
        HandoutOptions {
            per_page: 6,
            order: Order::Horizontal,
            frame: true,
            paper: Paper::A4,
            hf: HeaderFooter::default(),
            slides: None,
        }
    }
}

/// Options of a notes pages export.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotesOptions {
    /// Paper size.
    pub paper: Paper,
    /// Header and footer.
    #[serde(flatten)]
    pub hf: HeaderFooter,
    /// 0-based slides to print, in order (`None`: every slide).
    pub slides: Option<Vec<usize>>,
}

/// Picture format of a slide image export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFormat {
    /// PNG (lossless).
    #[default]
    Png,
    /// Baseline JPEG.
    Jpg,
    /// SVG 1.1 (vector; glyphs as outlines, pictures as data URIs).
    Svg,
}

impl ImageFormat {
    /// File extension.
    pub fn ext(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpg => "jpg",
            ImageFormat::Svg => "svg",
        }
    }
}

/// Options of a slide image export.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageOptions {
    /// Picture format.
    pub format: ImageFormat,
    /// 0-based slides to export; `None` = every slide. One slide gives the
    /// picture itself, several give a zip (`slide1.png`, `slide2.png`, …).
    pub slides: Option<Vec<usize>>,
    /// Width in pixels (the height follows the slide's aspect ratio).
    pub width: u32,
    /// JPEG quality 1..=100.
    pub quality: u8,
}

impl Default for ImageOptions {
    fn default() -> Self {
        ImageOptions {
            format: ImageFormat::Png,
            slides: None,
            width: 1280,
            quality: 90,
        }
    }
}

/// What to export (JSON: `{"kind":"handout","perPage":6,…}`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SlideExport {
    /// Handout PDF.
    Handout(HandoutOptions),
    /// Notes pages PDF.
    Notes(NotesOptions),
    /// Slide pictures.
    Image(ImageOptions),
    /// Zip of the deck, its media clips and embeddable fonts.
    Package,
}

/// Thin black outline of a rectangle.
fn outline(f: &Frame, width: f32) -> Command {
    Command::Path {
        segs: vec![
            PathSeg::M(f.x, f.y),
            PathSeg::L(f.x + f.w, f.y),
            PathSeg::L(f.x + f.w, f.y + f.h),
            PathSeg::L(f.x, f.y + f.h),
            PathSeg::Z,
        ],
        fill: None,
        stroke: Some(Stroke {
            width,
            color: Rgba(0, 0, 0, 255),
            dash: Vec::new(),
            cap: "butt".into(),
            join: "miter".into(),
        }),
    }
}

/// Draw `slide` scaled into `frame` (same aspect ratio), clipped to it.
fn place_slide(out: &mut Vec<Command>, slide: &PageList, frame: &Frame) {
    let s = if slide.width > 0.0 {
        frame.w / slide.width
    } else {
        1.0
    };
    out.push(Command::PushClip {
        x: frame.x,
        y: frame.y,
        w: frame.w,
        h: frame.h,
    });
    out.push(Command::PushTransform {
        m: [s, 0.0, 0.0, s, frame.x, frame.y],
    });
    out.extend(slide.commands.iter().cloned());
    out.push(Command::PopTransform);
    out.push(Command::PopClip);
}

impl Engine {
    fn deck(&self) -> Result<&bo_model::slide::Presentation> {
        self.pres.as_ref().ok_or(EngineError::NoDocument)
    }

    /// Slide size in points.
    fn slide_size_pt(&self) -> Result<(f32, f32)> {
        let p = self.deck()?;
        Ok((
            p.slide_size.cx as f32 / EMU_PER_PT,
            p.slide_size.cy as f32 / EMU_PER_PT,
        ))
    }

    /// Notes page size in points (§19.2.1.23 `notesSz`; 7.5 × 10 in when absent).
    fn notes_size_pt(&self) -> Result<(f32, f32)> {
        let p = self.deck()?;
        let (cx, cy) = (p.notes_size.cx, p.notes_size.cy);
        Ok(if cx > 0 && cy > 0 {
            (cx as f32 / EMU_PER_PT, cy as f32 / EMU_PER_PT)
        } else {
            (540.0, 720.0)
        })
    }

    /// Handout pages as a display list (one page per `per_page` slides).
    pub fn handout_pages(&mut self, o: &HandoutOptions) -> Result<DisplayList> {
        let (sw, sh) = self.slide_size_pt()?;
        let notes_size = self.notes_size_pt()?;
        let (pw, ph) = o.paper.size(notes_size);
        let layout = handout::layout(o.per_page, pw, ph, sw / sh.max(1.0)).ok_or_else(|| {
            EngineError::BadCommand(format!(
                "slides per page must be 1, 2, 3, 4, 6 or 9, not {}",
                o.per_page
            ))
        })?;
        let master = masters::handout_master(self.deck()?);
        let n = self.slide_count();
        let list: Vec<usize> = o.slides.as_ref().map_or_else(
            || (0..n).collect(),
            |v| v.iter().copied().filter(|&i| i < n).collect(),
        );
        let k = usize::from(o.per_page);
        let mut pages = Vec::with_capacity(list.len().div_ceil(k));
        for (page_no, chunk) in list.chunks(k).enumerate() {
            let mut cmds = Vec::new();
            for (pos, &i) in chunk.iter().enumerate() {
                let slot = handout::slot(pos, o.per_page, o.order);
                let Some(frame) = layout.frames.get(slot) else {
                    continue;
                };
                place_slide(&mut cmds, self.page(i)?, frame);
                if o.frame {
                    cmds.push(outline(frame, 0.75));
                }
                for l in layout.lines.get(slot).into_iter().flatten() {
                    cmds.push(Command::Line {
                        x1: l.x1,
                        y1: l.y,
                        x2: l.x2,
                        y2: l.y,
                        width: 0.5,
                        color: Rgba(128, 128, 128, 255),
                        dash: None,
                    });
                }
            }
            self.header_footer(
                &mut cmds,
                master.as_ref(),
                &o.hf,
                (pw, ph),
                notes_size,
                page_no + 1,
            );
            pages.push(PageList {
                width: pw,
                height: ph,
                commands: cmds,
            });
        }
        Ok(DisplayList { pages })
    }

    /// Notes pages as a display list: one page per slide, the slide in the
    /// notes master's slide image placeholder (`sldImg`), the speaker notes in
    /// its body placeholder (§19.7.10 `ST_PlaceholderType`).
    pub fn notes_pages(&mut self, o: &NotesOptions) -> Result<DisplayList> {
        let (sw, sh) = self.slide_size_pt()?;
        let notes_size = self.notes_size_pt()?;
        let (pw, ph) = o.paper.size(notes_size);
        let (sx, sy) = (pw / notes_size.0, ph / notes_size.1);
        let master = masters::notes_master(self.deck()?);
        let mut pages = Vec::new();
        let n = self.slide_count();
        let list: Vec<usize> = o.slides.as_ref().map_or_else(
            || (0..n).collect(),
            |v| v.iter().copied().filter(|&i| i < n).collect(),
        );
        for i in list {
            let own = self
                .deck()?
                .slides
                .get(i)
                .and_then(|s| s.notes.as_ref())
                .map(masters::notes_boxes)
                .unwrap_or_default();
            let scale = |f: Frame| Frame {
                x: f.x * sx,
                y: f.y * sy,
                w: f.w * sx,
                h: f.h * sy,
            };
            let img_box = own
                .slide_image
                .or(master.as_ref().and_then(|m| m.slide_image))
                .map_or_else(|| handout::default_notes_image(pw, ph), scale);
            let body_box = own
                .body
                .or(master.as_ref().and_then(|m| m.body))
                .map_or_else(|| handout::default_notes_body(pw, ph, &img_box), scale);
            let frame = handout::fit(&img_box, sw / sh.max(1.0));
            let mut cmds = Vec::new();
            place_slide(&mut cmds, self.page(i)?, &frame);
            cmds.push(outline(&frame, 0.75));
            let notes = self.slide_notes(i)?;
            let size = master
                .as_ref()
                .and_then(|m| m.notes_size_pt)
                .unwrap_or(12.0);
            text::wrap_into(&mut self.fonts, &mut cmds, &notes, &body_box, size);
            self.header_footer(
                &mut cmds,
                master.as_ref(),
                &o.hf,
                (pw, ph),
                notes_size,
                i + 1,
            );
            pages.push(PageList {
                width: pw,
                height: ph,
                commands: cmds,
            });
        }
        Ok(DisplayList { pages })
    }

    /// Header, footer, date and page number of a handout or notes page.
    fn header_footer(
        &mut self,
        out: &mut Vec<Command>,
        master: Option<&MasterInfo>,
        hf: &HeaderFooter,
        (pw, ph): (f32, f32),
        notes_size: (f32, f32),
        number: usize,
    ) {
        let (sx, sy) = (pw / notes_size.0, ph / notes_size.1);
        let page_no = number.to_string();
        for kind in HfKind::ALL {
            let explicit = match kind {
                HfKind::Header => hf.header.clone(),
                HfKind::Footer => hf.footer.clone(),
                HfKind::Date => hf.date.clone(),
                HfKind::SlideNumber => {
                    hf.page_numbers
                        .map(|on| if on { page_no.clone() } else { String::new() })
                }
            };
            let placeholder = master.and_then(|m| m.placeholder(kind));
            let text = match explicit {
                Some(t) => t,
                None => match (master, placeholder) {
                    (Some(m), Some(p)) if m.shows(kind) => p.text(&page_no),
                    _ => String::new(),
                },
            };
            if text.trim().is_empty() {
                continue;
            }
            let (frame, align, size) = match placeholder {
                Some(p) => (
                    Frame {
                        x: p.frame.x * sx,
                        y: p.frame.y * sy,
                        w: p.frame.w * sx,
                        h: p.frame.h * sy,
                    },
                    p.align.unwrap_or(kind.default_align()),
                    p.size_pt.unwrap_or(10.0),
                ),
                None => (
                    handout::default_hf_box(kind, pw, ph),
                    kind.default_align(),
                    10.0,
                ),
            };
            text::line_in_box(
                &mut self.fonts,
                out,
                &text,
                &frame,
                align,
                kind.at_bottom(),
                size,
            );
        }
    }

    /// Run an export described by `opts`; returns the file bytes (PDF, picture,
    /// SVG text or zip).
    pub fn export_slides(&mut self, opts: &SlideExport) -> Result<Vec<u8>> {
        match opts {
            SlideExport::Handout(o) => {
                let list = self.handout_pages(o)?;
                self.write_pdf_list(&list)
            }
            SlideExport::Notes(o) => {
                let list = self.notes_pages(o)?;
                self.write_pdf_list(&list)
            }
            SlideExport::Image(o) => self.export_images(o),
            SlideExport::Package => package::build(self),
        }
    }

    /// JSON variant of [`Engine::export_slides`] (the WASM / Tauri bridge).
    pub fn export_slides_json(&mut self, json: &str) -> Result<Vec<u8>> {
        let opts: SlideExport =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        self.export_slides(&opts)
    }

    fn write_pdf_list(&self, list: &DisplayList) -> Result<Vec<u8>> {
        let title = self
            .name
            .rsplit_once('.')
            .map_or(self.name.clone(), |(b, _)| b.to_owned());
        Ok(bo_pdf::write_pdf_with_images(
            list,
            &self.fonts,
            &bo_pdf::PdfMeta { title: Some(title) },
            &self.image_source(),
        )?)
    }

    /// One slide as a picture in `format`, `width` pixels wide.
    pub fn slide_image(
        &self,
        index: usize,
        format: ImageFormat,
        width: u32,
        quality: u8,
    ) -> Result<Vec<u8>> {
        let page = self.slide(index)?;
        let width = width.clamp(16, bo_raster::MAX_DIMENSION);
        let scale = width as f32 / page.width.max(1.0);
        Ok(match format {
            ImageFormat::Png => {
                bo_raster::render_page_png_with(page, &self.fonts, scale, &self.image_source())?
            }
            ImageFormat::Jpg => bo_raster::render_page_jpeg_with(
                page,
                &self.fonts,
                scale,
                &self.image_source(),
                quality.clamp(1, 100),
            )?,
            ImageFormat::Svg => {
                svg::page_svg(page, &self.fonts, &self.image_source(), width).into_bytes()
            }
        })
    }

    fn export_images(&self, o: &ImageOptions) -> Result<Vec<u8>> {
        let all: Vec<usize> = (0..self.slide_count()).collect();
        let list = o.slides.as_ref().unwrap_or(&all);
        if let [one] = list.as_slice() {
            return self.slide_image(*one, o.format, o.width, o.quality);
        }
        if list.is_empty() {
            return Err(EngineError::NoDocument);
        }
        let mut files = Vec::with_capacity(list.len());
        for &i in list {
            files.push((
                format!("slide{}.{}", i + 1, o.format.ext()),
                self.slide_image(i, o.format, o.width, o.quality)?,
            ));
        }
        package::zip(&files)
    }
}
