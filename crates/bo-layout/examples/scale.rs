//! Layout scaling probe: `cargo run --release -p bo-layout --example scale -- big.docx...`.
//!
//! `--repeat file.docx...` instead repeats the body of each (feature-rich corpus)
//! document 1×, 2×, 4× and 8× and prints the layout time per page, so super-linear
//! growth (lists, notes, tables, fields) shows directly.

use bo_layout::{layout_document, LayoutCache};
use bo_text::FontManager;
use std::time::Instant;

/// Shift the note references and list instances (`numId`) of `blocks` by `off`.
fn renumber_notes(blocks: &mut [bo_model::Block], off: i32) {
    use bo_model::{Block, RunContent};
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                if let Some(n) = &mut p.props.num {
                    if n.num_id > 0 {
                        n.num_id += off;
                    }
                }
                for r in &mut p.runs {
                    for c in &mut r.content {
                        if let RunContent::FootnoteRef(id) | RunContent::EndnoteRef(id) = c {
                            *id += off;
                        }
                    }
                }
            }
            Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        renumber_notes(&mut cell.blocks, off);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Lay out the body of `path` repeated until about `pages` pages, at 1×, 2×, 4×, 8×.
fn repeat(fm: &mut FontManager, path: &str) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let Ok(base) = bo_docx::read_docx(&bytes) else {
        return;
    };
    let Ok(one) = layout_document(&base, fm, &mut LayoutCache::new()) else {
        return;
    };
    // About 150 pages at 1×.
    let unit = (150 / one.pages.len().max(1)).max(1);
    for k in [1usize, 2, 4, 8] {
        let mut doc = base.clone();
        let (feet, ends) = (base.footnotes.clone(), base.endnotes.clone());
        for si in 0..doc.sections.len() {
            let blocks = doc.sections[si].blocks.clone();
            for copy in 1..unit * k {
                // Every copy references notes of its own (distinct ids).
                let off = i32::try_from(copy).unwrap_or(0) * 10_000;
                let mut more = blocks.clone();
                renumber_notes(&mut more, off);
                doc.sections[si].blocks.extend(more);
                for (id, b) in &feet {
                    doc.footnotes.insert(id + off, b.clone());
                }
                for (id, b) in &ends {
                    doc.endnotes.insert(id + off, b.clone());
                }
                for n in &base.numbering.nums {
                    let mut m = n.clone();
                    m.id += off;
                    doc.numbering.nums.push(m);
                }
            }
        }
        let mut best = f64::MAX;
        let mut pages = 0;
        for _ in 0..3 {
            let t = Instant::now();
            let l = layout_document(&doc, fm, &mut LayoutCache::new());
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
            pages = l.map_or(0, |l| l.pages.len());
        }
        println!(
            "{path} ×{}: {pages} pages, {best:.1} ms, {:.2} ms/page",
            unit * k,
            best / pages.max(1) as f64
        );
    }
}

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--repeat") {
        for path in &args[1..] {
            repeat(&mut fm, path);
        }
        return;
    }
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
