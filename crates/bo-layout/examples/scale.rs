//! Layout scaling probe: `cargo run --release -p bo-layout --example scale -- big.docx...`.

use bo_layout::{layout_document, LayoutCache};
use bo_text::FontManager;
use std::time::Instant;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    for n in [4300usize, 8600, 17200, 4300] {
        use bo_model::{Block, Document, HalfPoints, ParaProps, Paragraph, Run, RunProps, Section};
        let text = "Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015; Căn cứ Luật sửa đổi, bổ sung một số điều của Luật Tổ chức Chính phủ và Luật Tổ chức chính quyền địa phương.";
        let blocks = (0..n)
            .map(|i| {
                Block::Paragraph(Paragraph::new(
                    ParaProps::default(),
                    vec![Run::text(format!("{i}. {text}"), RunProps::default())],
                ))
            })
            .collect();
        let doc = Document {
            default_run: RunProps {
                font: Some("Times New Roman".into()),
                size: Some(HalfPoints(26)),
                ..Default::default()
            },
            sections: vec![Section::new(blocks)],
            ..Default::default()
        };
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            let _ = layout_document(&doc, &mut fm, &mut LayoutCache::new());
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{n} paragraphs: {best:.1} ms");
    }
    for path in std::env::args().skip(1) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let t = Instant::now();
        let Ok(doc) = bo_docx::read_docx(&bytes) else {
            continue;
        };
        let read = t.elapsed();
        let mut best = f64::MAX;
        let mut pages = 0;
        for _ in 0..3 {
            let t = Instant::now();
            let l = layout_document(&doc, &mut fm, &mut LayoutCache::new());
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
            pages = l.map_or(0, |l| l.pages.len());
        }
        println!(
            "{path}: read {:.1} ms, layout {best:.1} ms, {pages} pages",
            read.as_secs_f64() * 1e3
        );
    }
}
