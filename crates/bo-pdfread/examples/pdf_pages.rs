//! Rasterise every page of a PDF and dump its shown characters (layout-fidelity tooling,
//! `docs/ui/REPORT.md` "Fidelity round 1").
//!
//! ```text
//! cargo run -p bo-pdfread --example pdf_pages -- in.pdf out-dir [scale]
//! ```
//!
//! Writes `out-dir/page-<n>.png` (1-based, `scale` pixels per point, default 96/72) and
//! `out-dir/chars.tsv` with one shown character per row: page, left, top, right, bottom,
//! baseline x, baseline y, size, text (points from the page's top-left corner).
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use bo_pdfread::{PdfDocument, RenderOptions};

fn png_of(img: &bo_pdfread::Image) -> Option<Vec<u8>> {
    // Straight alpha from the premultiplied pixels.
    let mut px = img.pixels.clone();
    for p in px.chunks_exact_mut(4) {
        let a = u32::from(p[3]);
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = u8::try_from((u32::from(*c) * 255 / a).min(255)).unwrap_or(255);
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&px).ok()?;
    }
    Some(out)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(input), Some(out)) = (args.first(), args.get(1)) else {
        return Err("usage: pdf_pages <in.pdf> <out-dir> [scale]".into());
    };
    let scale: f32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(96.0 / 72.0);
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
    let mut doc = PdfDocument::open(bytes, bo_pdfread::standard_fonts())?;
    let mut tsv = String::new();
    for i in 0..doc.page_count() {
        let page = doc.render(i, RenderOptions::default())?;
        for c in &page.chars {
            let t: String = c
                .text
                .chars()
                .map(|ch| {
                    if ch == '\t' || ch == '\n' || ch == '\r' {
                        ' '
                    } else {
                        ch
                    }
                })
                .collect();
            let _ = writeln!(
                tsv,
                "{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{t}",
                i + 1,
                c.rect[0],
                c.rect[1],
                c.rect[2],
                c.rect[3],
                c.origin[0],
                c.origin[1],
                c.size
            );
        }
        let images: BTreeMap<String, Vec<u8>> = page
            .images
            .iter()
            .filter_map(|(k, v)| png_of(v).map(|b| (k.clone(), b)))
            .collect();
        let png = bo_raster::render_page_png_with(&page.list, doc.fonts(), scale, &images)
            .map_err(|e| e.to_string())?;
        std::fs::write(out.join(format!("page-{}.png", i + 1)), png).map_err(|e| e.to_string())?;
    }
    std::fs::write(out.join("chars.tsv"), tsv).map_err(|e| e.to_string())?;
    println!("{}", doc.page_count());
    Ok(())
}
