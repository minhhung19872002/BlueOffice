//! Incremental repaints are pixel-identical to full repaints after real edits.
#![cfg(windows)]

use bo_engine::Engine;
use bo_gpu_win::{changed_region, Device, PageImage};

fn full(device: &Device, engine: &Engine, scale: f32) -> PageImage {
    device
        .render_page(engine.page(0).unwrap(), engine.fonts(), scale, &|_| None)
        .unwrap()
}

/// Paint the changed band of the new page over the old image, as the WebView does.
fn incremental(
    device: &Device,
    engine: &Engine,
    old_list: &bo_render::PageList,
    old: &PageImage,
    scale: f32,
) -> (Vec<u8>, u64) {
    let new_list = engine.page(0).unwrap();
    let region = changed_region(old_list, new_list).device_band(scale);
    let band = device
        .render_page_region(new_list, engine.fonts(), scale, &|_| None, region)
        .unwrap();
    assert_eq!((band.page_width, band.page_height), (old.width, old.height));
    let mut pixels = old.pixels.clone();
    let row = band.width as usize * 4;
    for y in 0..band.height as usize {
        let at = ((band.top as usize + y) * old.width as usize + band.left as usize) * 4;
        pixels[at..at + row].copy_from_slice(&band.pixels[y * row..(y + 1) * row]);
    }
    (pixels, u64::from(band.width) * u64::from(band.height))
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)] // `as_chunks` exceeds the workspace MSRV.
fn typed_text_repaints_a_band_identical_to_a_full_repaint() {
    let device = Device::new().expect("Direct2D hardware or WARP");
    device.set_text_rendering(bo_gpu_win::TextRendering::document_default());
    for (dpi, zoom) in [(1.0, 1.0), (1.25, 1.1), (1.5, 0.75), (2.0, 1.0)] {
        let scale = dpi * 96.0 / 72.0 * zoom;
        let mut engine = Engine::with_bundled_fonts();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/phase1-sample.docx"
        );
        engine
            .open(&std::fs::read(path).unwrap(), "phase1-sample.docx")
            .unwrap();
        let mut old_list = engine.page(0).unwrap().clone();
        let mut old = full(&device, &engine, scale);
        let page_pixels = u64::from(old.width) * u64::from(old.height);
        let edits = [
            r#"{"cmd":"insertText","text":"a"}"#,
            r#"{"cmd":"deleteBackward"}"#,
            r#"{"cmd":"insertText","text":"ấ"}"#,
            "{\"cmd\":\"insertText\",\"text\":\"a\u{0306}\u{0301}\"}",
            r#"{"cmd":"deleteBackward"}"#,
            r#"{"cmd":"insertText","text":"Việt"}"#,
            r#"{"cmd":"undo"}"#,
            r#"{"cmd":"redo"}"#,
        ];
        for (i, edit) in edits.iter().enumerate() {
            engine.command_json(edit).unwrap();
            let started = std::time::Instant::now();
            let (pixels, band_pixels) = incremental(&device, &engine, &old_list, &old, scale);
            let band_time = started.elapsed();
            let started = std::time::Instant::now();
            let expected = full(&device, &engine, scale);
            eprintln!(
                "edit {i}: band {:.1}% of the page ({:.1} KB), {:.1} ms vs full {:.1} ms",
                band_pixels as f64 * 100.0 / page_pixels as f64,
                band_pixels as f64 * 4.0 / 1024.0,
                band_time.as_secs_f64() * 1e3,
                started.elapsed().as_secs_f64() * 1e3,
            );
            let differing = pixels
                .chunks_exact(4)
                .zip(expected.pixels.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(
                differing, 0,
                "DPI {dpi}, zoom {zoom}, edit {i}: {differing} pixels differ"
            );
            // One typed character repaints a small part of the page.
            assert!(
                band_pixels * 4 < page_pixels,
                "edit {i}: band {band_pixels} of {page_pixels}"
            );
            old_list = engine.page(0).unwrap().clone();
            old = expected;
        }
    }
}
