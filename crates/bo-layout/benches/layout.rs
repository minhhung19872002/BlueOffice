//! Layout benchmark: ~300 A4 pages of justified Vietnamese text.
// Benchmarks may unwrap.
#![allow(clippy::unwrap_used)]

use bo_layout::{layout_document, LayoutCache};
use bo_model::{
    Alignment, Block, Document, HalfPoints, ParaProps, Paragraph, Run, RunProps, Section, Twips,
};
use bo_text::FontManager;
use criterion::{criterion_group, criterion_main, Criterion};

const TEXT: &str = "Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015; Căn cứ Luật sửa đổi, bổ sung một số điều của Luật Tổ chức Chính phủ và Luật Tổ chức chính quyền địa phương ngày 22 tháng 11 năm 2019; Theo đề nghị của Bộ trưởng Bộ Nội vụ.";

fn big_doc(paragraphs: usize) -> Document {
    let blocks = (0..paragraphs)
        .map(|i| {
            Block::Paragraph(Paragraph::new(
                ParaProps {
                    align: Some(Alignment::Justify),
                    indent_first_line: Some(Twips(567)),
                    space_after: Some(Twips(120)),
                    ..Default::default()
                },
                vec![Run::text(format!("{i}. {TEXT}"), RunProps::default())],
            ))
        })
        .collect();
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],

        ..Default::default()
    }
}

fn bench(c: &mut Criterion) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    let mut fm = FontManager::new();
    fm.add_fonts_from_dir(&dir);
    let doc = big_doc(4300);
    let pages = layout_document(&doc, &mut fm, &mut LayoutCache::new())
        .unwrap()
        .pages
        .len();
    println!("benchmark document: {pages} pages");
    let mut g = c.benchmark_group("layout");
    g.sample_size(10);
    g.bench_function("cold_300_pages", |b| {
        b.iter(|| layout_document(&doc, &mut fm, &mut LayoutCache::new()).unwrap());
    });
    let mut cache = LayoutCache::new();
    layout_document(&doc, &mut fm, &mut cache).unwrap();
    let mut edited = doc.clone();
    g.bench_function("edit_one_paragraph_relayout", |b| {
        let mut n = 0u32;
        b.iter(|| {
            n += 1;
            let Block::Paragraph(p) = &mut edited.sections[0].blocks[2000] else {
                unreachable!("benchmark document has only paragraphs")
            };
            p.runs[0] = Run::text(format!("{n} {TEXT}"), RunProps::default());
            layout_document(&edited, &mut fm, &mut cache).unwrap()
        });
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
