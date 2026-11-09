//! Drawing pictures in cells, `IMAGE` results and checkbox controls on the grid
//! (`bo_sheet::render`, `bo_sheet::cell_image`).
#![allow(clippy::unwrap_used)]

use bo_model::sheet::{Cell, CellError, CellImage, CellValue, CellXf, Workbook};
use bo_render::Command;
use bo_sheet::cell_image::{placement, ImageState, ImageStates};
use bo_sheet::geometry::{SheetGeometry, SheetMetrics};
use bo_sheet::render::{render_viewport, RenderOptions};
use bo_sheet::viewport::Viewport;
use bo_text::FontManager;
use std::sync::Arc;

fn fonts() -> FontManager {
    let mut fm = FontManager::new();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts");
    fm.add_fonts_from_dir(&dir);
    fm
}

/// PNG header of a 4×2 picture (IHDR width 4, height 2); enough to be measured.
fn png_4x2() -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    v.extend_from_slice(&4u32.to_be_bytes());
    v.extend_from_slice(&2u32.to_be_bytes());
    v
}

fn render(wb: &mut Workbook, opts: &RenderOptions) -> Vec<Command> {
    bo_sheet::live::LiveCalc::new().rebuild(wb);
    let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
    let vp = Viewport::new(600.0, 300.0);
    render_viewport(wb, 0, &geom, &vp, opts, &mut fonts()).commands
}

fn images(cmds: &[Command]) -> Vec<(&str, [f32; 4])> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Image {
                image, x, y, w, h, ..
            } => Some((image.as_str(), [*x, *y, *w, *h])),
            _ => None,
        })
        .collect()
}

fn texts(cmds: &[Command]) -> Vec<&str> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Glyphs { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn stored_picture_fits_its_cell_and_hides_the_fallback() {
    let mut wb = Workbook::new();
    let img = CellImage::local(png_4x2(), "png", Some("Ảnh".into()));
    let id = img.media_id();
    wb.sheets[0].set_cell(
        1,
        1,
        Cell {
            value: CellValue::Error(CellError::Value),
            image: Some(Box::new(img)),
            ..Cell::default()
        },
    );
    let cmds = render(&mut wb, &RenderOptions::default());
    let geom = SheetGeometry::with_metrics(&wb.sheets[0], SheetMetrics::default());
    let vp = Viewport::new(600.0, 300.0);
    // B2 on screen = its sheet rectangle shifted by the header sizes.
    let mut cell = geom.cell_rect(1, 1);
    cell.x += vp.header_width;
    cell.y += vp.header_height;
    let want = placement(
        cell,
        (4.0, 2.0),
        bo_model::sheet::ImageSizing::Fit,
        None,
        None,
    )
    .unwrap();
    let got = images(&cmds);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, id);
    let [x, y, w, h] = got[0].1;
    assert!((f64::from(x) - want.x).abs() < 1e-3 && (f64::from(w) - want.w).abs() < 1e-3);
    assert!((f64::from(y) - want.y).abs() < 1e-3 && (f64::from(h) - want.h).abs() < 1e-3);
    // Fit keeps the 2:1 ratio.
    assert!((w / h - 2.0).abs() < 1e-3);
    assert!(!texts(&cmds).contains(&"#VALUE!"));
}

#[test]
fn image_function_waits_draws_or_shows_the_host_error() {
    let mut wb = Workbook::new();
    wb.sheets[0].set_a1("A1", CellValue::Empty);
    wb.sheets[0].set_cell(
        0,
        0,
        Cell::default().with_formula("IMAGE(\"https://example.vn/a.png\",\"Bản đồ\",1)"),
    );
    // Pending: nothing drawn, no fallback text.
    let cmds = render(&mut wb, &RenderOptions::default());
    assert_eq!(images(&cmds).len(), 0);
    assert!(!texts(&cmds).contains(&"#VALUE!"));
    let cell = wb.sheets[0].cell(0, 0).unwrap();
    assert_eq!(cell.value, CellValue::Error(CellError::Value));
    let img = cell.image.as_deref().unwrap();
    assert_eq!(img.alt_text.as_deref(), Some("Bản đồ"));
    let id = img.media_id();
    assert_eq!(id, "url:https://example.vn/a.png");
    // Loaded by the host: drawn (fill sizing).
    let mut states = ImageStates::new();
    states.insert(
        id.clone(),
        ImageState::Ready {
            width: 10.0,
            height: 10.0,
        },
    );
    let opts = RenderOptions {
        images: Arc::new(states.clone()),
        ..RenderOptions::default()
    };
    assert_eq!(images(&render(&mut wb, &opts)).len(), 1);
    // Failed: the host's error literal is shown as the cell text.
    states.insert(id, ImageState::Failed("#CONNECT!".into()));
    let opts = RenderOptions {
        images: Arc::new(states),
        ..RenderOptions::default()
    };
    let cmds = render(&mut wb, &opts);
    assert_eq!(images(&cmds).len(), 0);
    if !fonts().is_empty() {
        assert!(texts(&cmds).contains(&"#CONNECT!"), "{:?}", texts(&cmds));
    }
}

#[test]
fn checkbox_cells_draw_a_box_instead_of_true_false() {
    let mut wb = Workbook::new();
    let cb = wb.styles.add_xf(CellXf {
        checkbox: true,
        ..CellXf::default()
    });
    let ws = &mut wb.sheets[0];
    ws.set_cell(0, 0, Cell::new(CellValue::Bool(true)).with_style(cb));
    ws.set_cell(1, 0, Cell::new(CellValue::Bool(false)).with_style(cb));
    ws.set_cell(2, 0, Cell::new(CellValue::Empty).with_style(cb));
    // A number keeps its text even with the checkbox format.
    ws.set_cell(3, 0, Cell::new(CellValue::Number(7.0)).with_style(cb));
    let cmds = render(&mut wb, &RenderOptions::default());
    let paths = cmds
        .iter()
        .filter(|c| matches!(c, Command::Path { .. }))
        .count();
    // Checked: square + tick; unchecked and empty: square each.
    assert_eq!(paths, 4);
    let t = texts(&cmds);
    assert!(!t.contains(&"TRUE") && !t.contains(&"FALSE"));
    if !fonts().is_empty() {
        assert!(t.contains(&"7"));
    }
}
