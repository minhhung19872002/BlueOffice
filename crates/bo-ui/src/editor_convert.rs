//! BlueEditor conversions that keep the page's layout editable:
//!
//! - PowerPoint: one slide per page, the page drawn without its text as the slide picture and
//!   every paragraph an editable text box at its place, with its runs' font, size, bold, italic
//!   and colour (PresentationML through `bo_pptx::build`);
//! - Word: the paragraphs in reading order with their alignment, first-line indent, line pitch
//!   (exact), space before, runs' styles, the page size and margins of the PDF, a page break
//!   between pages (ECMA-376 Part 1 §17.3 paragraphs and runs, §17.6.13 `pgSz`);
//! - Excel: text cells placed in columns found per page from the left edges of the cells.
use bo_opc::xml::escape;
use bo_opc::{ContentTypes, Package};
use bo_pdfread::{Align, PdfDocument, StyledBlock};

/// One page for a conversion: size in points, its blocks and (slides) its picture.
pub(crate) struct PageText {
    pub size: (f32, f32),
    pub blocks: Vec<StyledBlock>,
    pub picture: Option<Vec<u8>>,
}

/// Pages of `doc` with their styled paragraphs.
pub(crate) fn page_texts(doc: &mut PdfDocument) -> Result<Vec<PageText>, String> {
    (0..doc.page_count())
        .map(|i| {
            let size = doc
                .page_info(i)
                .map_or((612.0, 792.0), |p| (p.width, p.height));
            Ok(PageText {
                size,
                blocks: doc.styled_blocks(i)?,
                picture: None,
            })
        })
        .collect()
}

fn hex(c: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// A deck with one slide per page: the page picture (without text) and a text box per
/// paragraph.
pub(crate) fn pages_to_pptx(pages: Vec<PageText>) -> Result<Vec<u8>, String> {
    use bo_pptx::build::{DeckSpec, ParaSpec, Picture, RunSpec, ShapeSpec, SlideSpec};
    let emu = |pt: f32| (pt * 12_700.0).round() as i64;
    let size = pages.first().map_or((612.0, 792.0), |p| p.size);
    // Text box insets (0.1 in × 0.05 in) are added around the paragraph's box.
    let (ix, iy) = (7.2, 3.6);
    let slides = pages
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut shapes = Vec::new();
            if let Some(data) = p.picture {
                shapes.push(ShapeSpec {
                    name: format!("Page {}", i + 1),
                    cx: emu(p.size.0),
                    cy: emu(p.size.1),
                    geometry: "rect".into(),
                    picture: Some(Picture {
                        data,
                        ext: "png".into(),
                    }),
                    ..ShapeSpec::default()
                });
            }
            for (k, b) in p.blocks.iter().enumerate() {
                let runs = b
                    .runs
                    .iter()
                    .map(|r| RunSpec {
                        text: r.text.clone(),
                        bold: r.bold,
                        italic: r.italic,
                        size: Some((r.size * 100.0).round().max(100.0) as u32),
                        color: Some(hex(r.color)),
                        font: Some(r.family.clone()),
                        ..RunSpec::default()
                    })
                    .collect();
                let align = match b.align {
                    Align::Left => None,
                    Align::Center => Some("ctr".to_owned()),
                    Align::Right => Some("r".to_owned()),
                    Align::Justify => Some("just".to_owned()),
                };
                shapes.push(ShapeSpec {
                    name: format!("Text {}", k + 1),
                    x: emu(b.bbox[0] - ix),
                    y: emu(b.bbox[1] - iy),
                    cx: emu(b.bbox[2] - b.bbox[0] + 2.0 * ix + 1.0),
                    cy: emu(b.bbox[3] - b.bbox[1] + 2.0 * iy),
                    geometry: "rect".into(),
                    text: vec![ParaSpec {
                        runs,
                        align,
                        ..ParaSpec::default()
                    }],
                    anchor: Some("t".into()),
                    ..ShapeSpec::default()
                });
            }
            SlideSpec {
                shapes,
                ..SlideSpec::default()
            }
        })
        .collect();
    let deck = DeckSpec {
        width: emu(size.0),
        height: emu(size.1),
        slides,
        title: None,
    };
    bo_pptx::build::build_pptx(&deck).map_err(|e| e.to_string())
}

/// A Word document with the pages' paragraphs.
pub(crate) fn pages_to_docx(pages: &[PageText]) -> Result<Vec<u8>, String> {
    let tw = |pt: f32| (pt * 20.0).round().max(0.0) as i64;
    let first = pages.first().ok_or("no pages")?;
    // Margins from the text of the first page (at least 0.25 in).
    let all: Vec<[f32; 4]> = first.blocks.iter().map(|b| b.bbox).collect();
    let (w, h) = first.size;
    let left = all
        .iter()
        .map(|b| b[0])
        .fold(w, f32::min)
        .clamp(18.0, w / 3.0);
    let right = all
        .iter()
        .map(|b| w - b[2])
        .fold(w, f32::min)
        .clamp(18.0, w / 3.0);
    let top = all
        .iter()
        .map(|b| b[1])
        .fold(h, f32::min)
        .clamp(18.0, h / 3.0);
    let mut body = String::new();
    for (pi, page) in pages.iter().enumerate() {
        let mut prev_bottom = top;
        for (bi, b) in page.blocks.iter().enumerate() {
            let jc = match b.align {
                Align::Left => "left",
                Align::Center => "center",
                Align::Right => "right",
                Align::Justify => "both",
            };
            let before = (b.bbox[1] - prev_bottom).max(0.0);
            prev_bottom = b.bbox[3];
            let size = b.runs.first().map_or(12.0, |r| r.size);
            let line = if b.pitch > 0.0 { b.pitch } else { size * 1.15 };
            let ind_left = (b.bbox[0] - left).max(0.0);
            body.push_str("<w:p><w:pPr>");
            if pi > 0 && bi == 0 {
                body.push_str("<w:pageBreakBefore/>");
            }
            body.push_str(&format!(
                r#"<w:spacing w:before="{}" w:after="0" w:line="{}" w:lineRule="exact"/><w:ind w:left="{}" w:firstLine="{}"/><w:jc w:val="{jc}"/></w:pPr>"#,
                tw(before),
                tw(line),
                tw(ind_left),
                tw(b.indent)
            ));
            for r in &b.runs {
                let mut rpr = format!(
                    r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}" w:cs="{f}"/>"#,
                    f = escape(&r.family)
                );
                if r.bold {
                    rpr.push_str("<w:b/>");
                }
                if r.italic {
                    rpr.push_str("<w:i/>");
                }
                if r.color != [0, 0, 0] {
                    rpr.push_str(&format!(r#"<w:color w:val="{}"/>"#, hex(r.color)));
                }
                let hp = (r.size * 2.0).round().max(2.0) as i64;
                rpr.push_str(&format!(r#"<w:sz w:val="{hp}"/><w:szCs w:val="{hp}"/>"#));
                body.push_str(&format!(
                    r#"<w:r><w:rPr>{rpr}</w:rPr><w:t xml:space="preserve">{}</w:t></w:r>"#,
                    escape(&r.text)
                ));
            }
            body.push_str("</w:p>");
        }
    }
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="{}" w:h="{}"/><w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="0" w:footer="0" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        tw(w),
        tw(h),
        tw(top),
        tw(right),
        tw(top),
        tw(left)
    );
    let mut pkg = Package::new();
    let mut ct = ContentTypes {
        defaults: vec![
            (
                "rels".into(),
                "application/vnd.openxmlformats-package.relationships+xml".into(),
            ),
            ("xml".into(), "application/xml".into()),
        ],
        overrides: Vec::new(),
    };
    ct.set_override(
        "/word/document.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
    );
    pkg.set_content_types(&ct);
    pkg.put_part(
        "/_rels/.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
    );
    pkg.put_part("/word/document.xml", doc.into_bytes());
    pkg.write().map_err(|e| e.to_string())
}

/// Columns of a page: left edges of its cells merged when closer than `gap` points.
fn columns(mut xs: Vec<f32>, gap: f32) -> Vec<f32> {
    xs.sort_by(f32::total_cmp);
    let mut out: Vec<f32> = Vec::new();
    for x in xs {
        if out.last().is_none_or(|l| x - l > gap) {
            out.push(x);
        }
    }
    out
}

/// A workbook with one sheet's rows per page: text cells in the columns detected from their
/// left edges, a blank row between pages.
pub(crate) fn to_xlsx(doc: &mut PdfDocument) -> Result<Vec<u8>, String> {
    let mut sheet = bo_engine::SheetEngine::with_bundled_fonts();
    sheet
        .new_workbook("Converted.xlsx")
        .map_err(|e| e.to_string())?;
    let cells = pdf_cells(doc)?;
    sheet.set_inputs(0, &cells).map_err(|e| e.to_string())?;
    sheet.save("xlsx", 0).map_err(|e| e.to_string())
}

/// The pages' text lines as worksheet cells (row, column, text): columns from the lines' left
/// edges, rows from their baselines, two blank rows between pages.
pub(crate) fn pdf_cells(doc: &mut PdfDocument) -> Result<Vec<(u32, u32, String)>, String> {
    let mut cells = Vec::new();
    let mut row = 0_u32;
    for i in 0..doc.page_count() {
        let objs = doc.page_objects(i)?;
        let mut lines = bo_pdfread::text_lines(&objs);
        lines.retain(|l| !l.text.trim().is_empty());
        lines.sort_by(|a, b| {
            (a.bbox[3] / 4.0)
                .round()
                .total_cmp(&(b.bbox[3] / 4.0).round())
                .then(a.bbox[0].total_cmp(&b.bbox[0]))
        });
        let cols = columns(lines.iter().map(|l| l.bbox[0]).collect(), 6.0);
        let col_of = |x: f32| cols.iter().rposition(|c| *c <= x + 0.5).unwrap_or(0) as u32;
        let mut last_y: Option<f32> = None;
        for l in lines {
            let y = l.bbox[3];
            if last_y.is_some_and(|ly| (ly - y).abs() > l.font_size.max(4.0) * 0.5) {
                row += 1;
            }
            last_y = Some(y);
            cells.push((row, col_of(l.bbox[0]), l.text.trim().to_owned()));
        }
        row += 2;
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_merge_close_edges() {
        assert_eq!(
            columns(vec![72.0, 300.0, 74.0, 180.0, 302.5], 6.0),
            [72.0, 180.0, 300.0]
        );
    }
}
