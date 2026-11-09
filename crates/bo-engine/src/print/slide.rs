//! BlueSlide pages of a print job: full slides, notes pages and handouts
//! (drawn by [`crate::slide_export`]), or an outline of the slide titles and
//! body text. Hidden slides (`p:sld/@show="0"`, ECMA-376 Part 1, §19.3.1.38)
//! are left out unless asked for.
//!
//! Outline layout (our own choice): 54 pt margins, "N  title" in 14 pt, body
//! paragraphs in 11 pt indented 18 pt per level, a new page when the next line
//! does not fit.

use super::{choose, Logical, PaperSize, PrintOptions, SlideLayout};
use crate::slide_export::{text, HandoutOptions, NotesOptions, Paper};
use crate::{Engine, EngineError, Result};
use bo_render::{Command, PageList, Rgba};

const MARGIN: f32 = 54.0;
const TITLE: f32 = 14.0;
const BODY: f32 = 11.0;
const INDENT: f32 = 18.0;

/// Paper of handouts and notes pages for the job's paper choice.
fn export_paper(p: Option<PaperSize>, default: Paper) -> Paper {
    match p {
        Some(PaperSize::Letter | PaperSize::Legal) => Paper::Letter,
        Some(_) => Paper::A4,
        None => default,
    }
}

impl Engine {
    /// Slides of the job: chosen, hidden ones dropped unless asked for.
    fn print_slide_list(&self, o: &PrintOptions) -> Result<Vec<usize>> {
        let mut v = choose(o, self.slide_count(), None)?;
        if !o.slide.hidden {
            let deck = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
            v.retain(|&i| deck.slides.get(i).is_some_and(|s| s.show != Some(false)));
        }
        Ok(v)
    }

    /// BlueSlide pages of a print job and whether the generic frame is drawn.
    pub(crate) fn slide_print_pages(&mut self, o: &PrintOptions) -> Result<(Vec<Logical>, bool)> {
        let list = self.print_slide_list(o)?;
        Ok(match o.slide.layout {
            SlideLayout::Slides => {
                let mut out = Vec::with_capacity(list.len());
                for i in list {
                    out.push((i + 1, self.slide(i)?.clone()));
                }
                (out, o.frame)
            }
            SlideLayout::Notes => {
                let pages = self
                    .notes_pages(&NotesOptions {
                        paper: export_paper(o.paper, Paper::Notes),
                        slides: Some(list.clone()),
                        ..NotesOptions::default()
                    })?
                    .pages;
                (list.iter().map(|i| i + 1).zip(pages).collect(), false)
            }
            SlideLayout::Handouts => {
                let pages = self
                    .handout_pages(&HandoutOptions {
                        per_page: o.slide.handout_per_page,
                        order: o.slide.handout_order,
                        frame: o.frame,
                        paper: export_paper(o.paper, Paper::A4),
                        slides: Some(list),
                        ..HandoutOptions::default()
                    })?
                    .pages;
                ((1..).zip(pages).collect(), false)
            }
            SlideLayout::Outline => {
                let size = o.paper.unwrap_or(PaperSize::A4).size();
                ((1..).zip(self.outline_pages(&list, size)?).collect(), false)
            }
        })
    }

    /// Outline pages of `slides` on paper `(w, h)`.
    fn outline_pages(&mut self, slides: &[usize], (w, h): (f32, f32)) -> Result<Vec<PageList>> {
        let items: Vec<(usize, crate::slide_edit::outline::OutlineSlide)> = {
            let deck = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
            slides
                .iter()
                .filter_map(|&i| {
                    deck.slides
                        .get(i)
                        .map(|s| (i, crate::slide_edit::outline::slide_outline(s)))
                })
                .collect()
        };
        let Some(id) = text::face(&mut self.fonts) else {
            return Ok(Vec::new());
        };
        let mut pages = Vec::new();
        let mut cmds: Vec<Command> = Vec::new();
        let mut y = MARGIN;
        let black = Rgba(0, 0, 0, 255);
        let width = w - 2.0 * MARGIN;
        for (i, s) in items {
            let mut lines: Vec<(f32, f32, String)> = Vec::new(); // (indent, size, text)
            let title = s.title.replace(['\u{b}', '\n'], " ");
            for (k, l) in text::wrap(
                &mut self.fonts,
                id,
                &format!("{}  {title}", i + 1),
                TITLE,
                width,
            )
            .into_iter()
            .enumerate()
            {
                lines.push((if k == 0 { 0.0 } else { INDENT }, TITLE, l));
            }
            for b in &s.bullets {
                let indent = INDENT * (1.0 + f32::from(b.level));
                let t = b.text.replace('\u{b}', " ");
                for l in text::wrap(&mut self.fonts, id, &t, BODY, (width - indent).max(BODY)) {
                    lines.push((indent, BODY, l));
                }
            }
            for (k, (indent, size, l)) in lines.into_iter().enumerate() {
                let pitch = size * 1.3;
                if y + pitch > h - MARGIN && !cmds.is_empty() {
                    pages.push(PageList {
                        width: w,
                        height: h,
                        commands: std::mem::take(&mut cmds),
                    });
                    y = MARGIN;
                }
                if k == 0 && y > MARGIN {
                    y += BODY * 0.6; // space between slides
                }
                y += pitch;
                if !l.is_empty() {
                    let (mut c, _) = text::run(&mut self.fonts, id, &l, size, MARGIN + indent, y);
                    if let Command::Glyphs { color, .. } = &mut c {
                        *color = black;
                    }
                    cmds.push(c);
                }
            }
        }
        if !cmds.is_empty() {
            pages.push(PageList {
                width: w,
                height: h,
                commands: cmds,
            });
        }
        Ok(pages)
    }
}
