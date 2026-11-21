//! More tests of the BlueSheet window (headless, fixed-width text measure, CPU raster).
use super::*;
use crate::sheet_view::{GridMove, GridStore};
use bo_engine::SheetEngine;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.5,
            style.size * 1.3,
        )
    }
}

fn window_with(e: SheetEngine) -> SheetWindow {
    let mut w = SheetWindow::new(SheetView::new(e), "en-US", "Book1.xlsx");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1200.0,
            height: 800.0,
        },
        &Fixed,
    );
    w
}

#[allow(dead_code)]
fn window() -> SheetWindow {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").expect("workbook");
    window_with(e)
}

fn ctrl(k: Key) -> Input {
    Input::Key(
        k,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    )
}

/// Grid store rasterising on the CPU: scrolling applies the moves to the previous image and
/// copies only the bands from a full render, so the result can be compared with drawing the
/// whole image.
#[derive(Default)]
struct Raster {
    img: Vec<u8>,
    size: (u32, u32),
    scrolls: usize,
    fulls: usize,
    band_px: u64,
    worst: u8,
    /// Pixels differing by more than 4 in any channel.
    off: usize,
    pixels: usize,
}

fn rasterise(list: &bo_render::PageList, e: &SheetEngine, scale: f32) -> (Vec<u8>, (u32, u32)) {
    let pm = bo_raster::render_page(list, e.fonts(), scale).expect("raster");
    (pm.data().to_vec(), (pm.width(), pm.height()))
}

impl GridStore for Raster {
    fn update(
        &mut self,
        list: &bo_render::PageList,
        e: &SheetEngine,
        scale: f32,
    ) -> Result<(), String> {
        (self.img, self.size) = rasterise(list, e, scale);
        self.fulls += 1;
        Ok(())
    }

    fn scroll(
        &mut self,
        list: &bo_render::PageList,
        e: &SheetEngine,
        scale: f32,
        moves: &[GridMove],
        bands: &[[u32; 4]],
    ) -> Result<(), String> {
        let (full, size) = rasterise(list, e, scale);
        assert_eq!(size, self.size, "same image size when scrolling");
        let (w, h) = size;
        let mut out = vec![0_u8; full.len()];
        let copy = |dst: &mut [u8], src: &[u8], r: [u32; 4], at: [u32; 2]| {
            for y in 0..r[3] {
                let (sy, dy) = (r[1] + y, at[1] + y);
                if sy >= h || dy >= h {
                    continue;
                }
                for x in 0..r[2] {
                    let (sx, dx) = (r[0] + x, at[0] + x);
                    if sx >= w || dx >= w {
                        continue;
                    }
                    let s = ((sy * w + sx) * 4) as usize;
                    let d = ((dy * w + dx) * 4) as usize;
                    dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
                }
            }
        };
        for m in moves {
            copy(&mut out, &self.img, m.src, m.dst);
        }
        for b in bands {
            copy(&mut out, &full, *b, [b[0], b[1]]);
            self.band_px += u64::from(b[2]) * u64::from(b[3]);
        }
        let mut first = None;
        for (i, (a, b)) in out.chunks(4).zip(full.chunks(4)).enumerate() {
            let d = a
                .iter()
                .zip(b)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0);
            self.worst = self.worst.max(d);
            if d > 4 {
                self.off += 1;
                first.get_or_insert((i as u32 % w, i as u32 / w, d));
            }
        }
        if let Some(f) = first {
            eprintln!("scroll {}: first differing pixel {f:?}", self.scrolls);
        }
        self.pixels += out.len() / 4;
        self.img = out;
        self.scrolls += 1;
        Ok(())
    }
}

#[test]
fn scrolling_moves_the_grid_image_and_draws_only_new_rows() {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").expect("workbook");
    let cells: Vec<(u32, u32, String)> = (0..400)
        .flat_map(|r| {
            (0..12).map(move |c| {
                let v = if c % 3 == 0 {
                    format!("Row {r} text that overflows")
                } else {
                    format!("{}", r * 13 + c)
                };
                (
                    r,
                    c,
                    if c % 3 == 0 && r % 2 == 1 {
                        String::new()
                    } else {
                        v
                    },
                )
            })
        })
        .collect();
    e.set_inputs(0, &cells).expect("cells");
    let mut w = window_with(e);
    let mut store = Raster::default();
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.fulls, 1);
    let (iw, ih) = store.size;
    // Wheel down and up, arrow scroll right and left.
    for _ in 0..6 {
        w.handle(Input::Wheel(75.0), &Fixed);
        w.prepare(&mut store).expect("grid");
    }
    for _ in 0..2 {
        w.handle(Input::Wheel(-75.0), &Fixed);
        w.prepare(&mut store).expect("grid");
    }
    for _ in 0..3 {
        assert!(w.view_mut().scroll_cols(1));
        w.prepare(&mut store).expect("grid");
    }
    assert!(w.view_mut().scroll_cols(-1));
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.fulls, 1, "no full redraw while scrolling");
    assert_eq!(store.scrolls, 12);
    // Glyph edges may differ by a few levels where a glyph sits on a half pixel; a wrong move
    // would shift whole gridlines and text.
    assert!(
        store.off * 20_000 < store.pixels && store.worst <= 32,
        "composited image: {} of {} pixels differ (by up to {})",
        store.off,
        store.pixels,
        store.worst
    );
    // Each wheel notch draws three rows (3 × 24 px), not the whole image.
    let per = store.band_px / 12;
    assert!(
        per < u64::from(iw) * u64::from(ih) / 4,
        "{per} px per scroll of {iw}×{ih}"
    );
    // An edit draws the whole image again.
    w.handle(Input::Text("x".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.fulls, 2);
}

/// Grid store keeping its CPU image as a ring ([`GridRing`]): scrolling turns it and draws
/// only the bands where the ring stores them; showing it puts the pieces together.
#[derive(Default)]
struct RingRaster {
    phys: Vec<u8>,
    ring: crate::GridRing,
    size: (u32, u32),
    turns: usize,
    /// Turns drawn from per-band display lists.
    band_turns: usize,
    fulls: usize,
    moved: usize,
    band_px: u64,
    worst: u8,
    off: usize,
    pixels: usize,
}

fn blit(dst: &mut [u8], src: &[u8], (w, h): (u32, u32), r: [u32; 4], at: [u32; 2]) {
    for y in 0..r[3] {
        let (sy, dy) = (r[1] + y, at[1] + y);
        assert!(sy < h && dy < h, "piece inside the image");
        for x in 0..r[2] {
            let (sx, dx) = (r[0] + x, at[0] + x);
            assert!(sx < w && dx < w, "piece inside the image");
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((dy * w + dx) * 4) as usize;
            dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
}

impl GridStore for RingRaster {
    fn update(
        &mut self,
        list: &bo_render::PageList,
        e: &SheetEngine,
        scale: f32,
    ) -> Result<(), String> {
        (self.phys, self.size) = rasterise(list, e, scale);
        self.ring = crate::GridRing::new(self.size.0, self.size.1);
        self.fulls += 1;
        Ok(())
    }

    fn scroll(
        &mut self,
        list: &bo_render::PageList,
        e: &SheetEngine,
        scale: f32,
        _: &[GridMove],
        _: &[[u32; 4]],
    ) -> Result<(), String> {
        self.moved += 1;
        self.update(list, e, scale)
    }

    fn turn(
        &mut self,
        list: &bo_render::PageList,
        e: &SheetEngine,
        scale: f32,
        shift: [i64; 2],
        bands: &[[u32; 4]],
    ) -> Result<bool, String> {
        let (full, size) = rasterise(list, e, scale);
        assert_eq!(size, self.size, "same image size when scrolling");
        self.ring.turn(shift);
        for b in bands {
            for (r, at) in self.ring.split(*b) {
                // Piece r of the new image, stored at `at`.
                blit(&mut self.phys, &full, size, r, at);
            }
            self.band_px += u64::from(b[2]) * u64::from(b[3]);
        }
        // Show: the pieces put back in image order.
        let mut out = vec![0_u8; full.len()];
        for (r, at) in self.ring.pieces() {
            blit(
                &mut out,
                &self.phys,
                size,
                [at[0], at[1], r[2], r[3]],
                [r[0], r[1]],
            );
        }
        for (a, b) in out.chunks(4).zip(full.chunks(4)) {
            let d = a
                .iter()
                .zip(b)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0);
            self.worst = self.worst.max(d);
            if d > 4 {
                self.off += 1;
            }
        }
        self.pixels += out.len() / 4;
        self.turns += 1;
        Ok(true)
    }

    fn turn_bands(
        &mut self,
        e: &SheetEngine,
        scale: f32,
        shift: [i64; 2],
        bands: &[crate::sheet_view::GridBand],
        size: (u32, u32),
    ) -> Result<bool, String> {
        assert_eq!(size, self.size, "same image size when scrolling");
        self.ring.turn(shift);
        for b in bands {
            let (img, (bw, bh)) = rasterise(&b.list, e, scale);
            for (r, at) in self.ring.split(b.rect) {
                for y in 0..r[3] {
                    let sy = r[1] + y - b.origin[1];
                    let dy = at[1] + y;
                    for x in 0..r[2] {
                        let sx = r[0] + x - b.origin[0];
                        let dx = at[0] + x;
                        if sx >= bw || sy >= bh {
                            continue;
                        }
                        let s = ((sy * bw + sx) * 4) as usize;
                        let d = ((dy * size.0 + dx) * 4) as usize;
                        self.phys[d..d + 4].copy_from_slice(&img[s..s + 4]);
                    }
                }
            }
            self.band_px += u64::from(b.rect[2]) * u64::from(b.rect[3]);
        }
        self.band_turns += 1;
        Ok(true)
    }
}

impl RingRaster {
    /// The image in display order (the ring's pieces put back).
    fn shown(&self) -> Vec<u8> {
        let mut out = vec![0_u8; self.phys.len()];
        for (r, at) in self.ring.pieces() {
            blit(
                &mut out,
                &self.phys,
                self.size,
                [at[0], at[1], r[2], r[3]],
                [r[0], r[1]],
            );
        }
        out
    }
}

#[test]
fn grid_ring_splits_wrapped_rectangles() {
    let mut r = crate::GridRing::new(100, 50);
    assert_eq!(r.pieces(), vec![([0, 0, 100, 50], [0, 0])]);
    r.turn([0, 30]);
    r.turn([0, 30]);
    assert_eq!(r.phase(), [0, 10]);
    // Image rows 0..40 are stored at 10..50, rows 40..50 at 0..10.
    assert_eq!(
        r.pieces(),
        vec![([0, 0, 100, 40], [0, 10]), ([0, 40, 100, 10], [0, 0])]
    );
    r.turn([-20, -15]);
    assert_eq!(r.phase(), [80, 45]);
    assert_eq!(r.split([10, 0, 20, 10]).len(), 4);
    assert_eq!(
        r.split([30, 10, 5, 5]),
        vec![([30, 10, 5, 5], [10, 5])],
        "inside one piece"
    );
}

#[test]
fn scrolling_turns_a_ring_image_without_copying() {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").expect("workbook");
    let cells: Vec<(u32, u32, String)> = (0..400)
        .flat_map(|r| {
            (0..12).map(move |c| {
                let v = if c % 3 == 0 {
                    format!("Row {r} text that overflows")
                } else {
                    format!("{}", r * 13 + c)
                };
                (r, c, v)
            })
        })
        .collect();
    e.set_inputs(0, &cells).expect("cells");
    let mut w = window_with(e);
    let mut store = RingRaster::default();
    w.prepare(&mut store).expect("grid");
    for _ in 0..9 {
        w.handle(Input::Wheel(75.0), &Fixed);
        w.prepare(&mut store).expect("grid");
    }
    for _ in 0..4 {
        w.handle(Input::Wheel(-75.0), &Fixed);
        w.prepare(&mut store).expect("grid");
    }
    for _ in 0..3 {
        assert!(w.view_mut().scroll_cols(1));
        w.prepare(&mut store).expect("grid");
    }
    assert!(w.view_mut().scroll_cols(-1));
    w.prepare(&mut store).expect("grid");
    assert_eq!(
        (store.fulls, store.moved, store.turns + store.band_turns),
        (1, 0, 17)
    );
    // Wheel notches lay out only the exposed rows (round 27).
    assert_eq!(store.band_turns, 13);
    assert_ne!(store.ring.phase(), [0, 0]);
    assert!(
        store.off * 20_000 < store.pixels && store.worst <= 32,
        "ring image: {} of {} pixels differ (by up to {})",
        store.off,
        store.pixels,
        store.worst
    );
    let (iw, ih) = store.size;
    assert!(store.band_px / 17 < u64::from(iw) * u64::from(ih) / 4);
    // Frozen rows: pixels are moved again (after a fresh image).
    assert!(w
        .view_mut()
        .run(json!({ "cmd": "freeze", "rows": 2, "cols": 0 })));
    w.prepare(&mut store).expect("grid");
    w.handle(Input::Wheel(75.0), &Fixed);
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.moved, 1);
}

#[test]
fn scrolling_below_frozen_rows_keeps_them() {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").expect("workbook");
    let cells: Vec<(u32, u32, String)> = (0..300)
        .flat_map(|r| (0..6).map(move |c| (r, c, format!("{r}:{c}"))))
        .collect();
    e.set_inputs(0, &cells).expect("cells");
    let mut w = window_with(e);
    assert!(w
        .view_mut()
        .run(json!({ "cmd": "freeze", "rows": 2, "cols": 1 })));
    let mut store = Raster::default();
    w.prepare(&mut store).expect("grid");
    for _ in 0..4 {
        w.handle(Input::Wheel(75.0), &Fixed);
        w.prepare(&mut store).expect("grid");
    }
    w.handle(Input::Wheel(-75.0), &Fixed);
    w.prepare(&mut store).expect("grid");
    assert!(w.view_mut().scroll_cols(2));
    w.prepare(&mut store).expect("grid");
    assert_eq!((store.fulls, store.scrolls), (1, 6));
    // Glyph edges may differ by a few levels where a glyph sits on a half pixel; a wrong move
    // would shift whole gridlines and text.
    assert!(
        store.off * 20_000 < store.pixels && store.worst <= 32,
        "composited image: {} of {} pixels differ (by up to {})",
        store.off,
        store.pixels,
        store.worst
    );
    let _ = ctrl(Key::Home);
}

fn put(w: &mut SheetWindow, cells: &[(u32, u32, &str)]) {
    let c: Vec<(u32, u32, String)> = cells
        .iter()
        .map(|(r, c, s)| (*r, *c, (*s).to_owned()))
        .collect();
    w.view_mut().engine_mut().set_inputs(0, &c).expect("cells");
    w.view_mut().changed();
}

fn input(w: &SheetWindow, r: u32, c: u32) -> String {
    w.view().engine().cell(0, r, c).expect("cell").input
}

fn shown(w: &SheetWindow, r: u32, c: u32) -> String {
    w.view().engine().cell(0, r, c).expect("cell").display
}

#[test]
fn number_format_box_menu_and_category() {
    let mut w = window();
    put(&mut w, &[(0, 0, "0,5")]);
    // The box shows the category and opens the list with it checked.
    assert!(w.action("sheet.numberFormat", &Fixed));
    assert!(w.menu.is_some(), "the number format list opens");
    w.menu = None;
    assert!(w.action("numfmt.percent2", &Fixed));
    assert_eq!(shown(&w, 0, 0), "50,00%");
    assert_eq!(
        sheet_ribbon::number_category(&w.view().format().num_fmt),
        "sheetMenu.numFmt.percentage"
    );
    assert!(w.action("numfmt.general", &Fixed));
    assert_eq!(shown(&w, 0, 0), "0,5");
}

#[test]
fn auto_sum_writes_totals_and_edits_a_single_cell() {
    let mut w = window();
    put(&mut w, &[(0, 0, "1"), (1, 0, "2"), (2, 0, "3")]);
    // One empty cell under the numbers: the formula opens in the editor.
    w.view_mut().select((3, 0), (3, 0), (3, 0));
    assert!(w.action("autosum.sum", &Fixed));
    assert!(w.view().editing());
    assert_eq!(w.view().formula_text(), "=SUM(A1:A3)");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(shown(&w, 3, 0), "6");
    // A selected column gets its total at once.
    put(&mut w, &[(0, 1, "4"), (1, 1, "5")]);
    w.view_mut().select((0, 1), (1, 1), (0, 1));
    assert!(w.action("autosum.max", &Fixed));
    assert_eq!(input(&w, 2, 1), "=MAX(B1:B2)");
}

#[test]
fn clear_fill_sort_and_styles_from_the_menus() {
    let mut w = window();
    put(&mut w, &[(0, 0, "3"), (1, 0, "1"), (2, 0, "2")]);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("sort.asc", &Fixed));
    assert_eq!((input(&w, 0, 0), input(&w, 2, 0)), ("1".into(), "3".into()));
    assert!(w.action("sort.desc", &Fixed));
    assert_eq!(input(&w, 0, 0), "3");
    // Fill down copies the first row.
    w.view_mut().select((0, 0), (2, 0), (0, 0));
    assert!(w.action("fill.down", &Fixed));
    assert_eq!(input(&w, 2, 0), "3");
    // Cell style, then clear formats keeps the values; clear all removes them.
    assert!(w.action("style.Good", &Fixed));
    assert!(
        w.view().format().color.len() == 6,
        "the style colours the text"
    );
    assert!(w.action("clear.formats", &Fixed));
    assert_eq!(w.view().format().color, "");
    assert_eq!(input(&w, 0, 0), "3");
    assert!(w.action("clear.all", &Fixed));
    assert_eq!(input(&w, 0, 0), "");
    assert!(w.action("edit.undo", &Fixed));
    assert_eq!(input(&w, 0, 0), "3", "clear all is one undo step");
}

#[test]
fn conditional_formats_tables_and_find() {
    let mut w = window();
    put(
        &mut w,
        &[(0, 0, "Name"), (1, 0, "a"), (2, 0, "b"), (3, 0, "a")],
    );
    w.view_mut().select((1, 0), (3, 0), (1, 0));
    assert!(w.action("cf.duplicates", &Fixed));
    assert!(w.action("cf.barBlue", &Fixed));
    let rules = |w: &SheetWindow| {
        w.view()
            .engine()
            .workbook()
            .map_or(0, |wb| wb.sheets[0].conditional_formats.len())
    };
    assert!(rules(&w) >= 1);
    assert!(w.action("cf.clearSheet", &Fixed));
    assert_eq!(rules(&w), 0);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("table.TableStyleMedium2", &Fixed));
    assert_eq!(
        w.view()
            .engine()
            .workbook()
            .map_or(0, |wb| wb.sheets[0].tables.len()),
        1
    );
    // Find selects the next match.
    assert!(w.action("find.find", &Fixed));
    w.handle(Input::Text("b".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.view().active(), (2, 0));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.dialog.is_none());
}

#[test]
fn view_toggles_and_more_tabs() {
    let mut w = window();
    let mut store = Raster::default();
    w.prepare(&mut store).expect("grid");
    assert!(w.action("sheet.gridlines", &Fixed));
    assert!(!w.view().gridlines());
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.fulls, 2, "gridlines redraw the grid");
    assert!(w.action("sheet.formulaBar", &Fixed));
    assert!(!w.show_fbar);
    assert!(w.action("freeze.row", &Fixed));
    let frozen = |w: &SheetWindow| {
        w.view().engine().workbook().map_or(0, |wb| {
            wb.sheets[0]
                .views
                .first()
                .and_then(|v| v.pane.as_ref())
                .map_or(0.0, |p| p.y_split) as u32
        })
    };
    assert_eq!(frozen(&w), 1);
    assert!(w.action("freeze.none", &Fixed));
    assert_eq!(frozen(&w), 0);
    // Function library: a function starts a formula in the editor.
    assert!(w.action("fn.VLOOKUP", &Fixed));
    assert_eq!(w.view().formula_text(), "=VLOOKUP(");
    // Every tab lays out.
    for i in 0..5 {
        let [x, y, ww, hh] = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("tab");
        w.handle(
            Input::Press {
                x: x + ww / 2.0,
                y: y + hh / 2.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(
            Input::Release {
                x: x + ww / 2.0,
                y: y + hh / 2.0,
            },
            &Fixed,
        );
    }
}

/// Window centre of cell (`r`, `c`) when it is on screen.
fn cell_xy(w: &SheetWindow, r: u32, c: u32) -> (f32, f32) {
    let rr = w
        .view()
        .rows_for_test()
        .iter()
        .find(|s| s.0 == r)
        .copied()
        .expect("row shown");
    let cc = w
        .view()
        .cols_for_test()
        .iter()
        .find(|s| s.0 == c)
        .copied()
        .expect("column shown");
    (f32::midpoint(cc.1, cc.2), f32::midpoint(rr.1, rr.2))
}

fn press(w: &mut SheetWindow, (x, y): (f32, f32), count: u8) {
    w.handle(
        Input::Press {
            x,
            y,
            count,
            shift: false,
        },
        &Fixed,
    );
}

fn release(w: &mut SheetWindow, (x, y): (f32, f32)) {
    w.handle(Input::Release { x, y }, &Fixed);
}

#[test]
fn formula_references_are_found_outside_strings() {
    let refs = crate::sheet_view::formula_refs("=A1+SUM($B$2:C3)*\"D4\"+LOG10(E5)+Sheet1!F6");
    let b: Vec<_> = refs.iter().map(|r| r.1).collect();
    assert_eq!(b, vec![(0, 0, 0, 0), (1, 1, 2, 2), (4, 4, 4, 4)]);
    assert_eq!(crate::sheet_view::formula_refs("A1"), Vec::new());
}

#[test]
fn pointing_at_cells_while_typing_a_formula_inserts_references() {
    let mut w = window();
    put(&mut w, &[(0, 0, "2"), (1, 0, "3"), (2, 0, "4")]);
    w.view_mut().select((0, 2), (0, 2), (0, 2));
    type_in(&mut w, "=SUM(");
    let a1 = cell_xy(&w, 0, 0);
    let a3 = cell_xy(&w, 2, 0);
    press(&mut w, a1, 1);
    assert_eq!(w.view().formula_text(), "=SUM(A1");
    // Dragging extends the pointed reference to a range.
    w.handle(Input::Drag { x: a3.0, y: a3.1 }, &Fixed);
    release(&mut w, a3);
    assert_eq!(w.view().formula_text(), "=SUM(A1:A3");
    assert!(w.view().editing(), "pointing keeps the formula open");
    type_in(&mut w, ")+");
    let a2 = cell_xy(&w, 1, 0);
    press(&mut w, a2, 1);
    release(&mut w, a2);
    assert_eq!(w.view().formula_text(), "=SUM(A1:A3)+A2");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(shown(&w, 0, 2), "12");
    assert_eq!(w.view().active(), (1, 2), "Enter commits and moves down");
}

#[test]
fn fill_handle_drag_fills_a_series() {
    let mut w = window();
    put(&mut w, &[(0, 0, "1"), (1, 0, "2")]);
    w.view_mut().select((0, 0), (1, 0), (0, 0));
    let [_, _, r, b] = w.view().sel_rect_for_test().expect("selection on screen");
    press(&mut w, (r, b), 1);
    let to = cell_xy(&w, 4, 0);
    w.handle(Input::Drag { x: to.0, y: to.1 }, &Fixed);
    release(&mut w, to);
    assert_eq!(input(&w, 4, 0), "5");
    assert_eq!(w.view().range_bounds(), (0, 0, 4, 0));
}

#[test]
fn fill_handle_with_ctrl_toggles_copy_and_series() {
    // One number: a plain drag copies it, with Ctrl it counts up (and the reverse for two).
    for (ctrl, want) in [(false, "7"), (true, "10")] {
        let mut w = window();
        put(&mut w, &[(0, 0, "7")]);
        w.view_mut().select((0, 0), (0, 0), (0, 0));
        let [_, _, r, b] = w.view().sel_rect_for_test().expect("selection on screen");
        press(&mut w, (r, b), 1);
        let to = cell_xy(&w, 3, 0);
        w.handle(Input::Drag { x: to.0, y: to.1 }, &Fixed);
        w.view_mut().set_mouse_ctrl(ctrl);
        release(&mut w, to);
        assert_eq!(input(&w, 3, 0), want, "ctrl {ctrl}");
    }
}

#[test]
fn double_click_on_a_column_edge_fits_it_and_context_menu_opens() {
    let mut w = window();
    put(&mut w, &[(0, 1, "a rather long text that does not fit")]);
    let before = w
        .view()
        .cols_for_test()
        .iter()
        .find(|s| s.0 == 1)
        .map(|s| s.2 - s.1)
        .expect("B");
    let edge = w
        .view()
        .cols_for_test()
        .iter()
        .find(|s| s.0 == 1)
        .map(|s| s.2)
        .expect("B");
    let y = w.view().frame_for_test()[1] + 5.0;
    press(&mut w, (edge, y), 1);
    release(&mut w, (edge, y));
    press(&mut w, (edge, y), 2);
    release(&mut w, (edge, y));
    let after = w
        .view()
        .cols_for_test()
        .iter()
        .find(|s| s.0 == 1)
        .map(|s| s.2 - s.1)
        .expect("B");
    assert!(
        after > before + 20.0,
        "AutoFit widened B from {before} to {after}"
    );
    // Right click on a cell: the context menu, then Clear contents from it.
    let at = cell_xy(&w, 0, 1);
    w.handle(Input::Context { x: at.0, y: at.1 }, &Fixed);
    assert!(w.menu.is_some());
    w.menu = None;
    assert!(w.action("clear.contents", &Fixed));
    assert_eq!(input(&w, 0, 1), "");
}

fn type_in(w: &mut SheetWindow, s: &str) {
    for c in s.chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
}

/// Painter recording filled rectangles and text runs.
#[derive(Default)]
struct Rec {
    rects: Vec<([f32; 4], Rgba)>,
    texts: Vec<(String, Rgba)>,
}

impl Painter for Rec {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Rgba) {
        self.rects.push(([x, y, w, h], color));
    }
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, text: &str, _: f32, _: f32, _: TextStyle, color: Rgba) {
        self.texts.push((text.to_owned(), color));
    }
    fn text_font(&mut self, text: &str, _: f32, _: f32, _: &Font<'_>, color: Rgba) {
        self.texts.push((text.to_owned(), color));
    }
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

#[test]
fn arrow_keys_point_at_cells_while_typing_a_formula() {
    let mut w = window();
    w.view_mut().select((4, 2), (4, 2), (4, 2));
    type_in(&mut w, "=");
    let key = |w: &mut SheetWindow, k: Key, shift: bool| {
        w.handle(
            Input::Key(
                k,
                Mods {
                    shift,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
    };
    // The first arrow points next to the active cell (C5 → C4), later ones move it.
    key(&mut w, Key::Up, false);
    assert_eq!(w.view().formula_text(), "=C4");
    assert_eq!(w.view().edit_mode(), Some("point"));
    key(&mut w, Key::Left, false);
    assert_eq!(w.view().formula_text(), "=B4");
    // Shift extends the pointed cell to a range.
    key(&mut w, Key::Up, true);
    key(&mut w, Key::Up, true);
    assert_eq!(w.view().formula_text(), "=B2:B4");
    // After an operator the next arrow starts from the active cell again.
    type_in(&mut w, "+");
    assert_eq!(w.view().edit_mode(), Some("enter"));
    key(&mut w, Key::Right, false);
    assert_eq!(w.view().formula_text(), "=B2:B4+D5");
    assert!(w.view().editing());
    // Without a place for a reference, an arrow commits and moves (Enter mode).
    type_in(&mut w, "1");
    key(&mut w, Key::Down, false);
    assert!(!w.view().editing());
    assert_eq!(w.view().active(), (5, 2));
}

#[test]
fn reference_frames_use_the_measured_colours_tint_and_corner_squares() {
    let mut w = window();
    w.view_mut().select((1, 7), (1, 7), (1, 7));
    type_in(&mut w, "=A1+B2:C3+A5+D2+E6+F8+B10+D11");
    let mut rec = Rec::default();
    w.paint(&mut rec);
    let blue = Rgba(0x32, 0x6a, 0xc7, 255);
    let red = Rgba(0xc0, 0x35, 0x3e, 255);
    // Text of the references in their frame colour; the eighth wraps to the first colour.
    let colour_of = |t: &str| rec.texts.iter().find(|(s, _)| s == t).map(|x| x.1);
    assert_eq!(colour_of("A1"), Some(blue));
    assert_eq!(colour_of("B2:C3"), Some(red));
    assert_eq!(colour_of("D11"), Some(blue));
    assert_eq!(colour_of("+"), Some(Rgba(0, 0, 0, 255)));
    // 10 % tints and 5 px corner squares (4 per reference) in the frame colours.
    assert!(rec
        .rects
        .iter()
        .any(|(_, c)| *c == Rgba(0xc0, 0x35, 0x3e, 26)));
    let squares = rec
        .rects
        .iter()
        .filter(|(r, c)| *c == red && r[2] == 5.0 && r[3] == 5.0)
        .count();
    assert_eq!(squares, 4);
}

#[test]
fn name_box_goes_to_typed_references_and_lists_defined_names() {
    let mut w = window();
    assert!(w
        .view_mut()
        .query(serde_json::json!({
            "cmd": "defineName", "name": "Prices", "refersTo": "=Sheet1!$B$2:$C$4",
        }))
        .is_some());
    // A click in the name box selects its text; typing replaces it, Enter goes there.
    let n = w.name_rect;
    press(&mut w, (n[0] + 10.0, n[1] + n[3] / 2.0), 1);
    type_in(&mut w, "D7");
    assert!(
        !w.view().editing(),
        "typing goes to the name box, not the cell"
    );
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.view().active(), (6, 3));
    // The arrow lists the names; choosing one selects its range.
    let a = w.name_arrow_rect();
    press(&mut w, (a[0] + a[2] / 2.0, a[1] + a[3] / 2.0), 1);
    assert!(w.menu.is_some());
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.view().range_bounds(), (1, 1, 3, 2));
    // Escape leaves the name box without moving.
    press(&mut w, (n[0] + 10.0, n[1] + n[3] / 2.0), 1);
    type_in(&mut w, "Z99");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.view().range_bounds(), (1, 1, 3, 2));
    type_in(&mut w, "x");
    assert!(w.view().editing(), "typing goes to the cell again");
}

#[test]
fn expand_button_shows_three_formula_lines() {
    let mut w = window();
    put(&mut w, &[(0, 0, "one")]);
    let top = w.grid_top();
    let e = w.expand_rect();
    press(&mut w, (e[0] + e[2] / 2.0, e[1] + e[3] / 2.0), 1);
    // Two more lines of 24 px at 125 %, as measured on the reference.
    assert_eq!(w.grid_top(), top + 48.0);
    assert_eq!(w.fx_rect[3], 30.0 + 48.0);
    let e = w.expand_rect();
    press(&mut w, (e[0] + e[2] / 2.0, e[1] + e[3] / 2.0), 1);
    assert_eq!(w.grid_top(), top);
}

#[test]
fn home_tab_is_wide_or_compact_with_the_window_as_on_the_reference() {
    let mut w = window();
    let resize = |w: &mut SheetWindow, width: f32| {
        w.handle(
            Input::Resize {
                width,
                height: 800.0,
            },
            &Fixed,
        );
    };
    let rect = |w: &SheetWindow, id: &'static str| {
        w.ribbon
            .rect_of(RibbonHit::Command(id))
            .expect("placed command")
    };
    // Wide: number format box 196 px, Sort & Filter and Find & Select as large buttons after
    // the editing icon column, styles as large buttons.
    resize(&mut w, 2400.0);
    assert_eq!(rect(&w, "sheet.numberFormat")[2], 196.0);
    let sum = rect(&w, "sheet.autoSum");
    let sort = rect(&w, "sheet.sortFilter");
    let find = rect(&w, "sheet.findSelect");
    assert!(sort[0] > sum[0] && find[0] > sort[0]);
    assert!(sort[3] > 2.0 * sum[3], "large button spans the band");
    assert!(rect(&w, "sheet.conditional")[3] > 2.0 * sum[3]);
    // Compact: a 121 px box, the editing buttons in two small columns.
    resize(&mut w, 1100.0);
    assert_eq!(rect(&w, "sheet.numberFormat")[2], 121.0);
    let sum = rect(&w, "sheet.autoSum");
    let sort = rect(&w, "sheet.sortFilter");
    assert_eq!(sort[3], sum[3]);
}

/// Click the form's control `id` at (dx, dy) reference pixels inside it.
fn click_form(w: &mut SheetWindow, id: &str, dx: f32, dy: f32) {
    let (x, y) = {
        let f = w.form.as_ref().expect("form");
        let r = f.get(id).expect("control").rect;
        let d = f.dev([r[0] + dx, r[1] + dy, 0.0, 0.0]);
        (d[0], d[1])
    };
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
}

#[test]
fn format_cells_general_description_starts_right_of_the_category_list() {
    let mut w = window();
    assert!(w.action("sheet.formatCellsDialog", &Fixed));
    let f = w.form.as_ref().expect("form");
    let cat = f.get("cat").expect("cat").rect;
    let desc = f.get("desc").expect("desc").rect;
    assert!(desc[0] >= cat[0] + cat[2], "{desc:?} over {cat:?}");
}

#[test]
fn format_cells_dialog_has_pages_like_the_reference() {
    let mut w = window();
    put(&mut w, &[(0, 0, "1234,5")]);
    assert!(w.action("sheet.formatCellsDialog", &Fixed));
    let tab = |w: &SheetWindow| w.form.as_ref().map(|f| f.tab);
    assert_eq!(tab(&w), Some(0));
    let ctrl_tab = |w: &mut SheetWindow, shift: bool| {
        w.handle(
            Input::Key(
                Key::Tab,
                Mods {
                    shift,
                    ctrl: true,
                    ..Mods::default()
                },
            ),
            &Fixed,
        );
    };
    // Number: category Number (second row), 1 decimal, thousands separator; the sample
    // follows.
    click_form(&mut w, "cat", 20.0, 2.0 + 17.0 + 5.0);
    {
        let f = w.form.as_mut().expect("form");
        assert_eq!(f.sel("cat"), Some(1));
        assert!(f.get("dec").is_some_and(|c| c.visible));
        assert!(f.get("type").is_some_and(|c| !c.visible));
        f.set_num("dec", 1);
        f.set_on("sep", true);
    }
    assert_eq!(w.fc_sample(), "1.234,5");
    // Date shows the type list.
    click_form(&mut w, "cat", 20.0, 2.0 + 4.0 * 17.0 + 5.0);
    assert!(w
        .form
        .as_ref()
        .and_then(|f| f.get("type"))
        .is_some_and(|c| c.visible));
    click_form(&mut w, "cat", 20.0, 2.0 + 17.0 + 5.0);
    // Alignment: centre (third item of the drop-down), wrap, 45 degrees from the dial.
    ctrl_tab(&mut w, false);
    assert_eq!(tab(&w), Some(1));
    click_form(&mut w, "h", 20.0, 10.0);
    click_form(&mut w, "h", 20.0, 26.0 + 1.0 + 2.0 * 17.0 + 5.0);
    click_form(&mut w, "wrap", 5.0, 10.0);
    click_form(&mut w, "dial", 6.0 + 40.0, 75.0 - 40.0);
    assert_eq!(w.form.as_ref().map(|f| f.num("deg")), Some(45));
    // Font: Bold from the style list.
    ctrl_tab(&mut w, false);
    click_form(&mut w, "fstyle", 20.0, 2.0 + 2.0 * 17.0 + 5.0);
    assert_eq!(
        w.form.as_ref().map(|f| f.text("styleName")),
        Some("Bold".into())
    );
    // Border: thick line, Outline preset.
    ctrl_tab(&mut w, false);
    click_form(&mut w, "lines", 80.0, 14.0 + 5.0 * 20.5);
    assert_eq!(fcells_impl::LINE_STYLES[w.fc.line], "thick");
    click_form(&mut w, "presets", 80.0 + 30.0, 20.0);
    // Fill: the standard red.
    ctrl_tab(&mut w, false);
    click_form(&mut w, "bg", 22.0 + 8.0, 6.0 * 21.0 + 20.0 + 8.0);
    // Protection: clear Locked.
    ctrl_tab(&mut w, false);
    assert_eq!(tab(&w), Some(5));
    click_form(&mut w, "locked", 5.0, 10.0);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    let f = w.view().format().clone();
    assert_eq!(f.h_align, "center");
    assert!(f.wrap && f.bold);
    assert_eq!(f.num_fmt, "#,##0.0");
    let wb = w.view().engine().workbook().expect("workbook");
    let xf = wb
        .styles
        .xf(wb.sheets[0].cell(0, 0).expect("cell").style)
        .expect("xf");
    assert_eq!(
        xf.alignment.as_ref().and_then(|a| a.text_rotation),
        Some(45)
    );
    assert_eq!(xf.protection.as_ref().and_then(|p| p.locked), Some(false));
    let b = &wb.styles.borders[xf.border_id as usize];
    assert_eq!(b.top.style.as_deref(), Some("thick"));
    assert_eq!(b.left.style.as_deref(), Some("thick"));
    match &wb.styles.fills[xf.fill_id as usize] {
        bo_model::sheet::Fill::Pattern(p) => assert_eq!(
            p.fg.as_ref().and_then(|c| c.rgb.as_deref()),
            Some("FFFF0000")
        ),
        other @ bo_model::sheet::Fill::Gradient(_) => panic!("solid fill expected: {other:?}"),
    }
}

#[test]
fn page_layout_tab_sets_up_printing() {
    let mut w = window();
    assert!(w.action("orient.landscape", &Fixed));
    assert!(w.action("margins.narrow", &Fixed));
    assert!(w.action("paper.8", &Fixed));
    w.view_mut().select((0, 0), (4, 2), (0, 0));
    assert!(w.action("printArea.set", &Fixed));
    assert!(w.action("fitw.1", &Fixed));
    let s = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "pageSetup" }))
        .expect("page setup");
    assert_eq!(s["orientation"], "landscape");
    assert_eq!(s["paperSize"], 8);
    assert_eq!(s["margins"][0], 0.25);
    assert_eq!(s["fit"][0], 1);
    assert!(s["printArea"]
        .as_str()
        .is_some_and(|a| a.contains("$A$1:$C$5")));
    assert!(w.action("sheet.printGridlines", &Fixed));
    let s = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "pageSetup" }))
        .expect("page setup");
    assert_eq!(s["gridLines"], true);
}

#[test]
fn review_tab_comments_and_sheet_protection() {
    let mut w = window();
    w.view_mut().select((2, 1), (2, 1), (2, 1));
    assert!(w.action("sheet.newComment", &Fixed));
    type_in(&mut w, "Check this");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.commented_cells(), vec![(2, 1)]);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("sheet.nextComment", &Fixed));
    assert_eq!(w.view().active(), (2, 1));
    assert_eq!(w.status, "Check this");
    assert!(w.action("sheet.deleteComment", &Fixed));
    assert_eq!(w.commented_cells(), Vec::new());
    // Protect, then a locked cell refuses typing; Unprotect with the same button.
    assert!(w.action("sheet.protectSheet", &Fixed));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.view_mut().select((5, 5), (5, 5), (5, 5));
    type_in(&mut w, "1");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(
        input(&w, 5, 5),
        "",
        "locked cells of a protected sheet keep their value"
    );
    assert!(w.status.contains("protected"), "{}", w.status);
    assert!(w.action("sheet.protectSheet", &Fixed));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    put(&mut w, &[(5, 5, "1")]);
    assert_eq!(input(&w, 5, 5), "1");
}

#[test]
fn manual_calculation_waits_for_f9() {
    let mut w = window();
    put(&mut w, &[(0, 0, "1"), (0, 1, "=A1+1")]);
    assert!(w.action("calc.manual", &Fixed));
    put(&mut w, &[(0, 0, "10")]);
    assert_eq!(shown(&w, 0, 1), "2", "not recalculated in manual mode");
    assert!(w.calc_state.1, "the status bar says Calculate");
    w.handle(Input::Key(Key::F(9), Mods::default()), &Fixed);
    assert_eq!(shown(&w, 0, 1), "11");
    assert!(!w.calc_state.1);
    assert!(w.action("calc.auto", &Fixed));
    put(&mut w, &[(0, 0, "20")]);
    assert_eq!(shown(&w, 0, 1), "21");
}

#[test]
fn text_to_columns_wizard_previews_then_splits() {
    let mut w = window();
    put(&mut w, &[(0, 0, "a;b;c"), (1, 0, "d;e")]);
    w.view_mut().select((0, 0), (1, 0), (0, 0));
    assert!(w.action("sheet.textToColumns", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::TextToColumns);
    // Step 1: the guess is Delimited.
    assert!(w
        .form
        .as_ref()
        .is_some_and(|f| f.on("delimited") && !f.on("fixed")));
    click_form(&mut w, "next", 10.0, 10.0);
    // Step 2: Tab only, as the reference; tick Semicolon.
    let f = w.form.as_ref().expect("step 2");
    assert!(f.on("dTab") && !f.on("dSemi") && !f.on("dComma"));
    click_form(&mut w, "dSemi", 5.0, 10.0);
    click_form(&mut w, "next", 10.0, 10.0);
    // Step 3: second column as text, then pick the destination C1 on the sheet.
    assert_eq!(w.ttc.as_ref().map(|t| t.step), Some(2));
    click_form(&mut w, "preview", 20.0, 60.0);
    assert_eq!(w.ttc.as_ref().map(|t| t.col), Some(1));
    click_form(&mut w, "fText", 5.0, 10.0);
    click_form(&mut w, "pickDest", 5.0, 5.0);
    assert_eq!(w.form_kind, form_impl::FormKind::RangePick);
    let r = w.view.cell_rect(0, 2).expect("C1 on screen");
    w.handle(
        Input::Press {
            x: r[0] + 5.0,
            y: r[1] + 5.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: r[0] + 5.0,
            y: r[1] + 5.0,
        },
        &Fixed,
    );
    assert_eq!(w.form.as_ref().map(|f| f.text("ref")), Some("$C$1".into()));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.form_kind, form_impl::FormKind::TextToColumns);
    assert_eq!(w.form.as_ref().map(|f| f.text("dest")), Some("$C$1".into()));
    click_form(&mut w, "finish", 10.0, 10.0);
    assert!(w.form.is_none());
    assert_eq!(
        (
            input(&w, 0, 0),
            input(&w, 0, 2),
            input(&w, 0, 3),
            input(&w, 0, 4),
            input(&w, 1, 3)
        ),
        (
            "a;b;c".into(),
            "a".into(),
            "b".into(),
            "c".into(),
            "e".into()
        )
    );
}

#[test]
fn text_to_columns_fixed_width_breaks_click_drag_and_double_click() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Alice     Hanoi     2024"),
            (1, 0, "Bob       Hue       2023"),
        ],
    );
    w.view_mut().select((0, 0), (1, 0), (0, 0));
    assert!(w.action("sheet.textToColumns", &Fixed));
    assert!(w.form.as_ref().is_some_and(|f| f.on("fixed")));
    click_form(&mut w, "next", 10.0, 10.0);
    assert_eq!(w.ttc.as_ref().map(|t| t.breaks.clone()), Some(vec![10, 20]));
    // Click at character 5 adds a break; drag it to 6; double-click removes the one at 20.
    let press = |w: &mut SheetWindow, cx: f32, count: u8| {
        let (x, y) = {
            let f = w.form.as_ref().expect("form");
            let d = f.dev([32.0 + 1.0 + cx * 8.0, 315.0 + 60.0, 0.0, 0.0]);
            (d[0], d[1])
        };
        w.handle(
            Input::Press {
                x,
                y,
                count,
                shift: false,
            },
            &Fixed,
        );
        (x, y)
    };
    let (_, y) = press(&mut w, 5.0, 1);
    let x6 = w
        .form
        .as_ref()
        .expect("form")
        .dev([32.0 + 1.0 + 48.0, 0.0, 0.0, 0.0])[0];
    w.handle(Input::Drag { x: x6, y }, &Fixed);
    w.handle(Input::Release { x: x6, y }, &Fixed);
    assert_eq!(
        w.ttc.as_ref().map(|t| t.breaks.clone()),
        Some(vec![6, 10, 20])
    );
    press(&mut w, 20.0, 1);
    w.handle(Input::Release { x: 0.0, y }, &Fixed);
    press(&mut w, 20.0, 2);
    assert_eq!(w.ttc.as_ref().map(|t| t.breaks.clone()), Some(vec![6, 10]));
    click_form(&mut w, "finish", 10.0, 10.0);
    assert_eq!(
        (
            input(&w, 0, 0),
            input(&w, 0, 1),
            input(&w, 0, 2),
            input(&w, 1, 2)
        ),
        (
            "Alice".into(),
            String::new(),
            "Hanoi     2024".into(),
            "Hue       2023".into()
        )
    );
}

/// Timing of wheel steps on a large workbook (BLUESHEET_BIG=<xlsx>; run with --ignored
/// --nocapture): input handling, display list, header labels.
#[test]
#[ignore = "manual timing"]
fn wheel_step_timing() {
    struct Null;
    impl GridStore for Null {
        fn update(
            &mut self,
            _: &bo_render::PageList,
            _: &SheetEngine,
            _: f32,
        ) -> Result<(), String> {
            Ok(())
        }
    }
    let Some(path) = std::env::var_os("BLUESHEET_BIG") else {
        return;
    };
    let bytes = std::fs::read(path).expect("file");
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&bytes, "big.xlsx").expect("open");
    let mut w = window_with(e);
    w.prepare(&mut Null).expect("grid");
    let mut t = [0.0_f64; 6];
    let n = 40;
    for i in 0..n {
        let d: i64 = if i % 10 < 5 { 3 } else { -3 };
        let t0 = std::time::Instant::now();
        w.view.scroll_rows(d);
        let t1 = std::time::Instant::now();
        w.update_bars();
        let t2 = std::time::Instant::now();
        w.view.measure_labels(&Fixed);
        let t3 = std::time::Instant::now();
        w.prepare(&mut Null).expect("grid");
        let t4 = std::time::Instant::now();
        w.handle(Input::Wheel(if d > 0 { -1.0 } else { 1.0 }), &Fixed);
        let t5 = std::time::Instant::now();
        w.prepare(&mut Null).expect("grid");
        let t6 = std::time::Instant::now();
        for (k, (a, b)) in [(t0, t1), (t1, t2), (t2, t3), (t3, t4), (t4, t5), (t5, t6)]
            .iter()
            .enumerate()
        {
            t[k] += (*b - *a).as_secs_f64() * 1e3 / f64::from(n);
        }
    }
    eprintln!(
        "per wheel step (ms): scroll {:.2}, bars {:.2}, labels {:.2}, display list {:.2}; handle(wheel) {:.2}, list {:.2}",
        t[0], t[1], t[2], t[3], t[4], t[5]
    );
}

/// Time of each step of `handle` for an arrow key and a wheel notch on a large sheet
/// (BLUESHEET_BIG=<file.xlsx>; round 27 latency work).
#[test]
fn handle_step_timing() {
    let Some(path) = std::env::var_os("BLUESHEET_BIG") else {
        return;
    };
    let bytes = std::fs::read(path).expect("file");
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&bytes, "big.xlsx").expect("open");
    let mut w = window_with(e);
    for (name, make) in [
        ("key", Input::Key(Key::Down, Mods::default())),
        ("wheel", Input::Wheel(-75.0)),
    ] {
        let mut t = [0.0_f64; 9];
        let n = 40;
        for i in 0..n {
            let input = match (&make, i % 2) {
                (Input::Key(..), 0) => Input::Key(Key::Down, Mods::default()),
                (Input::Key(..), _) => Input::Key(Key::Up, Mods::default()),
                (_, 0) => Input::Wheel(-75.0),
                _ => Input::Wheel(75.0),
            };
            let mut at = vec![std::time::Instant::now()];
            w.route(input, &Fixed);
            at.push(std::time::Instant::now());
            w.refresh_chart_cache();
            w.sync_table_tab();
            at.push(std::time::Instant::now());
            w.layout_cards(&Fixed);
            at.push(std::time::Instant::now());
            w.update_bars();
            at.push(std::time::Instant::now());
            w.view.measure_labels(&Fixed);
            at.push(std::time::Instant::now());
            w.layout_form(&Fixed);
            w.measure_name(&Fixed);
            at.push(std::time::Instant::now());
            let _ = w.on_ids();
            at.push(std::time::Instant::now());
            let mut hash = PaintHash::default();
            w.paint_top(&mut hash);
            at.push(std::time::Instant::now());
            let mut hash = PaintHash::default();
            w.paint_overlay(&mut hash);
            at.push(std::time::Instant::now());
            for k in 0..at.len() - 1 {
                t[k] += (at[k + 1] - at[k]).as_secs_f64() * 1e3 / f64::from(n);
            }
        }
        eprintln!(
            "{name} per step (ms): route {:.3}, chart+table {:.3}, cards {:.3}, bars {:.3}, labels {:.3}, form+name {:.3}, on_ids {:.3}, paint_top(hash) {:.3}, paint_overlay(hash) {:.3}",
            t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8]
        );
    }
    // Inside the view: the engine's layout of the viewport and the active cell's details.
    let vp = w.view.viewport();
    let sheet = w.view.sheet();
    let (r, c) = w.view.active();
    let n = 40;
    let t0 = std::time::Instant::now();
    for _ in 0..n {
        let _ = w.view.engine_mut().view_layout(sheet, &vp);
    }
    let t1 = std::time::Instant::now();
    for _ in 0..n {
        let _ = w.view.engine().cell(sheet, r, c);
    }
    let t2 = std::time::Instant::now();
    for _ in 0..n {
        w.view.engine_mut().set_display_scale(Some(1.25));
    }
    let t3 = std::time::Instant::now();
    let ms =
        |a: std::time::Instant, b: std::time::Instant| (b - a).as_secs_f64() * 1e3 / f64::from(n);
    eprintln!(
        "view_layout {:.3} ms, cell details {:.3} ms, set_display_scale {:.3} ms",
        ms(t0, t1),
        ms(t1, t2),
        ms(t2, t3)
    );
}

#[test]
fn data_and_formulas_dialogs_sort_validate_seek_name_and_trace() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Name"),
            (0, 1, "Score"),
            (1, 0, "b"),
            (1, 1, "2"),
            (2, 0, "a"),
            (2, 1, "2"),
            (3, 0, "c"),
            (3, 1, "1"),
        ],
    );
    // Sort by Score (descending), then by Name.
    w.view_mut().select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.sortDialog", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Sort);
    {
        let f = w.form.as_mut().expect("sort form");
        assert!(f.on("header"), "header row guessed");
        // No key chosen yet, as the reference opens.
        assert_eq!(f.text("col0"), "");
        f.set_sel("col0", Some(2));
        f.set_sel("ord0", Some(1));
    }
    click_form(&mut w, "add", 10.0, 10.0);
    assert!(w
        .form
        .as_ref()
        .and_then(|f| f.get("col1"))
        .is_some_and(|c| c.visible));
    if let Some(f) = w.form.as_mut() {
        f.set_sel("col1", Some(1));
    }
    let ok = w.fc_ok_point();
    w.handle(
        Input::Press {
            x: ok.0,
            y: ok.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.form.is_none());
    let col0: Vec<String> = (1..4).map(|r| input(&w, r, 0)).collect();
    assert_eq!(col0, ["a", "b", "c"]);
    // Data validation: whole number between 1 and 5.
    w.view_mut().select((5, 1), (5, 1), (5, 1));
    assert!(w.action("sheet.dataValidation", &Fixed));
    {
        let f = w.form.as_mut().expect("validation form");
        f.set_sel("allow", Some(1));
        f.set_text("f1", "1");
        f.set_text("f2", "5");
    }
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.form.is_none(), "Escape cancels");
    assert!(w.action("sheet.dataValidation", &Fixed));
    {
        let f = w.form.as_mut().expect("validation form");
        f.set_sel("allow", Some(1));
        f.set_text("f1", "1");
        f.set_text("f2", "5");
    }
    let ok = w.fc_ok_point();
    w.handle(
        Input::Press {
            x: ok.0,
            y: ok.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let sheet = w.view().sheet();
    let rule = w
        .view_mut()
        .query(json!({ "cmd": "validation", "sheet": sheet, "row": 5, "col": 1 }))
        .unwrap_or(serde_json::Value::Null);
    assert_eq!(rule["kind"], json!("whole"));
    // Goal seek: B8 = B7 * 2 should reach 10 by changing B7.
    put(&mut w, &[(6, 1, "1"), (7, 1, "=B7*2")]);
    w.view_mut().select((7, 1), (7, 1), (7, 1));
    assert!(w.action("sheet.goalSeek", &Fixed));
    {
        let f = w.form.as_mut().expect("goal seek form");
        assert_eq!(f.text("set"), "B8");
        f.set_text("to", "10");
        f.set_text("by", "B7");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    assert_eq!(shown(&w, 6, 1), "5");
    // Name Manager: New… opens New Name; define Rate = 0,1.
    assert!(w.action("sheet.nameManager", &Fixed));
    click_form(&mut w, "new", 10.0, 10.0);
    assert_eq!(w.form_kind, form_impl::FormKind::NewName);
    {
        let f = w.form.as_mut().expect("new name form");
        f.set_text("name", "Rate");
        f.set_text("ref", "=0,1");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.form_kind, form_impl::FormKind::Names);
    let rate = w
        .name_table
        .iter()
        .find(|r| r.name == "Rate")
        .expect("Rate");
    assert_eq!(rate.value, "0.1");
    // Filter ▾ is a button with a menu: Table Names leaves no defined name.
    click_form(&mut w, "filterBtn", 10.0, 10.0);
    assert!(w
        .form
        .as_ref()
        .and_then(|f| f.get("fmenu"))
        .is_some_and(|c| c.visible));
    w.form_event(
        form_impl::FormKind::Names,
        &form_impl::Event::Canvas("fmenu", 10.0, 6.0 * 26.0 + 10.0),
        &Fixed,
    );
    assert!(w.name_table.iter().all(|r| r.table));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // Insert Function: search IF, pick it, OK starts the formula.
    w.view_mut().select((9, 0), (9, 0), (9, 0));
    assert!(w.action("sheet.insertFunction", &Fixed));
    type_in(&mut w, "IFERROR");
    assert_eq!(
        w.form.as_ref().map(|f| f.text("fns")),
        Some("IFERROR".into())
    );
    assert!(matches!(
        w.form.as_ref().and_then(|f| f.get("sig")).map(|c| &c.kind),
        Some(form_impl::Kind::Label(s)) if s == "IFERROR(value, value_if_error)"
    ));
    // OK opens Function Arguments (round 52) on its two arguments.
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.form_kind, form_impl::FormKind::FnArgs);
    assert_eq!(w.fnargs.as_ref().map(|s| s.args.len()), Some(2));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    // Trace precedents of B8: one arrow from B7.
    w.view_mut().select((7, 1), (7, 1), (7, 1));
    assert!(w.action("sheet.tracePrecedents", &Fixed));
    assert_eq!(w.arrows.len(), 1);
    assert_eq!(w.arrows[0].from, (6, 1, 6, 1));
    assert!(w.action("sheet.removeArrows", &Fixed));
    assert_eq!(w.arrows.len(), 0);
    assert!(w.action("sheet.showFormulas", &Fixed));
    assert!(w.view().show_formulas());
}

#[test]
fn comment_cards_show_on_hover_and_all_at_once() {
    let mut w = window();
    put(&mut w, &[(1, 1, "x"), (4, 2, "y")]);
    for (r, c, t) in [(1, 1, "Check this value before sending"), (4, 2, "Second")] {
        w.view_mut().select((r, c), (r, c), (r, c));
        let sheet = w.view().sheet();
        w.view_mut().query(json!({
            "cmd": "addComment", "sheet": sheet, "row": r, "col": c, "text": t, "author": "An",
        }));
        w.view_mut().changed();
    }
    assert_eq!(w.commented_cells().len(), 2);
    let (x, y) = cell_xy(&w, 1, 1);
    w.handle(Input::Move { x, y }, &Fixed);
    assert_eq!(w.hover_comment, Some((1, 1)));
    assert_eq!(w.cards.len(), 1);
    assert!(w.cards[0].lines.join(" ").contains("Check this value"));
    let (x, y) = cell_xy(&w, 3, 3);
    w.handle(Input::Move { x, y }, &Fixed);
    assert_eq!(w.cards.len(), 0);
    assert!(w.action("sheet.showComments", &Fixed));
    w.handle(Input::Blink, &Fixed);
    assert_eq!(w.cards.len(), 2);
}

#[test]
fn print_titles_help_tab_and_option_check_boxes() {
    let mut w = window();
    assert!(w.action("sheet.printTitles", &Fixed));
    type_in(&mut w, "$1:$2");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    let titles = w
        .view()
        .engine()
        .workbook()
        .and_then(|wb| wb.sheets[0].print_titles.clone())
        .unwrap_or_default();
    assert!(titles.contains("$1:$2"), "{titles}");
    assert!(w.action("sheet.shortcuts", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Help);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    assert!(sheet_ribbon::tab_ids().contains(&"sheet.help"));
    // Gridlines / headings / print options are check boxes, not toggled buttons.
    assert!(!w.on_ids().contains(&"sheet.gridlines"));
    let checks = w.check_items();
    assert!(checks
        .iter()
        .any(|(id, on)| *id == "sheet.gridlines" && *on));
}

#[test]
fn filter_drop_down_checks_values_and_reports_records_found() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Name"),
            (0, 1, "Region"),
            (1, 0, "An"),
            (1, 1, "North"),
            (2, 0, "Binh"),
            (2, 1, "South"),
            (3, 0, "Chi"),
            (3, 1, "North"),
        ],
    );
    assert!(w.action("sheet.filter", &Fixed));
    assert_eq!(w.auto_filter(), Some([0, 0, 3, 1]));
    // The button sits at the right end of the header cell.
    let b = w.view().cell_box(0, 1).expect("header");
    let at = (b[2] - 4.0, f32::midpoint(b[1], b[3]));
    assert_eq!(w.filter_button_at(at.0, at.1), Some((1, None)));
    assert_eq!(w.filter_button_at(b[0] + 4.0, at.1), None);
    press(&mut w, at, 1);
    let f = w.filter.as_ref().expect("drop-down open");
    assert_eq!(f.header, "Region");
    assert_eq!(f.kind, filter_impl::ColumnKind::Text);
    let texts: Vec<&str> = f.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["North", "South"]);
    // Untick South (list row 2 after Select All) and press OK.
    w.filter.as_mut().expect("open").toggle(2);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.filter.is_none());
    assert_eq!(w.status, "2 of 3 records found");
    let info = w.view().engine().info().expect("info");
    assert_eq!(info.sheets[0].hidden_rows, vec![(2, 2)]);
    // Clear Filter From "Region" shows every row again.
    press(&mut w, at, 1);
    assert!(w.filter.as_ref().is_some_and(|f| f.active));
    assert!(w.filter_item(3));
    let info = w.view().engine().info().expect("info");
    assert_eq!(info.sheets[0].hidden_rows.len(), 0);
    // Number Filters > Greater Than opens the Custom AutoFilter form.
    put(&mut w, &[(1, 1, "5"), (2, 1, "7"), (3, 1, "9")]);
    press(&mut w, at, 1);
    assert_eq!(
        w.filter.as_ref().map(|f| f.kind),
        Some(filter_impl::ColumnKind::Number)
    );
    assert!(w.filter_item(5));
    assert!(w.menu.is_some(), "submenu");
    w.menu = None;
    assert!(w.filter_choice("op.2"));
    let f = w.form.as_mut().expect("custom filter");
    assert_eq!(f.sel("op1"), Some(2));
    f.set_text("v1", "6");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.status, "2 of 3 records found");
    // A press in the grid clears the message.
    let at = cell_xy(&w, 5, 3);
    press(&mut w, at, 1);
    assert_eq!(w.status, "");
}

#[test]
fn insert_tab_forms_create_tables_pivots_sparklines_links_and_headers() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Region"),
            (0, 1, "Amount"),
            (1, 0, "North"),
            (1, 1, "5"),
            (2, 0, "South"),
            (2, 1, "7"),
            (3, 0, "North"),
            (3, 1, "9"),
        ],
    );
    // Create Table: the region and the header guess are filled in.
    assert!(w.action("sheet.table", &Fixed));
    assert_eq!(
        w.form.as_ref().map(|f| f.text("range")).as_deref(),
        Some("$A$1:$B$4")
    );
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    let tables = w.view.query(json!({ "cmd": "tables" })).expect("tables");
    assert_eq!(tables["tables"].as_array().map(Vec::len), Some(1));
    // Recommended PivotTable: Region as rows, Sum of Amount, on a new sheet.
    assert!(w.action("sheet.recPivot", &Fixed));
    {
        let f = w.form.as_ref().expect("pivot form");
        assert_eq!(f.sel("rows"), Some(0));
        assert_eq!(f.sel("values"), Some(1));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.view().sheet(), 1, "the report's new sheet is shown");
    let shown_cells: Vec<String> = (0..8).map(|r| shown_on(&w, 1, r, 1)).collect();
    assert!(shown_cells.iter().any(|s| s == "21"), "{shown_cells:?}");
    w.view_mut().set_sheet(0);
    // Sparklines in D2:D4 from B2:B4.
    w.view_mut().select((1, 1), (3, 1), (1, 1));
    assert!(w.action("sheet.sparkColumn", &Fixed));
    w.form.as_mut().expect("form").set_text("loc", "D2:D4");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let sp = w
        .view
        .query(json!({ "cmd": "sparklines" }))
        .expect("sparklines");
    assert!(sp.to_string().contains("column"), "{sp}");
    // Link: a HYPERLINK formula in the active cell.
    w.view_mut().select((5, 0), (5, 0), (5, 0));
    assert!(w.action("sheet.link", &Fixed));
    {
        let f = w.form.as_mut().expect("link");
        f.set_text("text", "Site");
        f.set_text("addr", "https://example.com");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    // A cell hyperlink (§18.3.1.47) showing the text.
    assert_eq!(input(&w, 5, 0), "Site");
    let link = w
        .view
        .query(json!({ "cmd": "hyperlink", "sheet": 0, "row": 5, "col": 0 }))
        .expect("link");
    assert_eq!(link["target"], "https://example.com");
    // Header & Footer sections.
    assert!(w.action("sheet.headerFooter", &Fixed));
    w.form.as_mut().expect("hf").set_text("hc", "Report");
    w.form.as_mut().expect("hf").set_text("fr", "Page &P");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let ps = &w.view().engine().workbook().expect("wb").sheets[0].page_setup;
    assert_eq!(ps.odd_header.as_deref(), Some("&CReport"));
    assert_eq!(ps.odd_footer.as_deref(), Some("&RPage &P"));
    // Symbol: the chosen character goes into the cell editor.
    assert!(w.action("sheet.symbol", &Fixed));
    w.symbol = (0, 2);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.view().editing());
    assert_eq!(w.view().formula_text(), "#");
}

fn shown_on(w: &SheetWindow, sheet: usize, r: u32, c: u32) -> String {
    w.view()
        .engine()
        .cell(sheet, r, c)
        .map(|d| d.display)
        .unwrap_or_default()
}

#[test]
fn format_cells_diagonal_borders_reach_the_engine_and_come_back() {
    let mut w = window();
    put(&mut w, &[(0, 0, "x")]);
    assert!(w.action("sheet.formatCellsDialog", &Fixed));
    w.fc.edges[7] = (6, "FF0000".into());
    w.fc.border_changed = true;
    let ok = w.fc_ok_point();
    press(&mut w, ok, 1);
    assert!(w.form.is_none());
    let wb = w.view().engine().workbook().expect("wb");
    let xf = wb
        .styles
        .xf(wb.sheets[0].cell(0, 0).expect("cell").style)
        .expect("xf");
    let b = &wb.styles.borders[xf.border_id as usize];
    assert!(b.diagonal_down && !b.diagonal_up);
    assert!(w.action("sheet.formatCellsDialog", &Fixed));
    assert_eq!(w.fc.edges[7], (6, "FF0000".to_owned()));
    assert_eq!(w.fc.edges[6].0, 0);
}

#[test]
fn insert_shapes_pictures_pivot_charts_and_slicers() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Region"),
            (0, 1, "Amount"),
            (1, 0, "North"),
            (1, 1, "5"),
            (2, 0, "South"),
            (2, 1, "7"),
        ],
    );
    // Shapes menu entry: an oval at the active cell.
    assert_eq!(
        sheet_ribbon::menu("sheet.shapes", &[]).len(),
        sheet_ribbon::SHAPES.len()
    );
    w.view_mut().select((4, 1), (4, 1), (4, 1));
    assert!(w.action("shape.ellipse", &Fixed));
    // A picture from the host (1 × 1 PNG): one column, one row.
    let png = [
        0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0,
    ];
    assert!(w.insert_picture(&png, "dot.png"));
    {
        let ws = &w.view().engine().workbook().expect("wb").sheets[0];
        assert_eq!(ws.shape_groups.len(), 2);
        assert!(ws.shape_groups[0].grp_sp.contains("ellipse"));
        assert_eq!(ws.shape_groups[1].to.map(|t| (t.row, t.col)), Some((5, 2)));
    }
    // Pictures asks the host for files.
    assert!(w.action("sheet.pictures", &Fixed));
    assert!(w.take_requests().contains(&Request::PickPicture));
    // Slicer outside a table: a hint; inside a table: a slicer.
    w.view_mut().select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.slicer", &Fixed));
    assert!(w.status.contains("table"), "{}", w.status);
    w.view_mut().select((0, 0), (2, 1), (0, 0));
    assert!(w.action("table.TableStyleMedium2", &Fixed));
    w.view_mut().select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.slicer", &Fixed));
    assert_eq!(
        w.view().engine().workbook().expect("wb").sheets[0]
            .slicers
            .len(),
        1
    );
    // PivotChart: a pivot table and its chart on a new sheet.
    w.view_mut().select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.pivotChart", &Fixed));
    let wb = w.view().engine().workbook().expect("wb");
    let last = wb.sheets.last().expect("sheet");
    assert_eq!((last.pivots.len(), last.charts.len()), (1, 1));
}

#[test]
fn name_box_lists_functions_while_a_formula_is_typed() {
    let mut w = window();
    assert_eq!(w.name_box_text(), "A1");
    type_text_in(&mut w, "=round(");
    assert_eq!(w.name_box_text(), "ROUND");
    let a = w.name_arrow_rect();
    press(&mut w, (a[0] + 2.0, a[1] + 2.0), 1);
    let items = w.menu.as_ref().expect("function list").items().len();
    assert_eq!(items, fbar_impl::RECENT_FUNCTIONS.len() + 1);
    w.menu = None;
    assert!(w.choose(Choice::Command("fnin.SUM".into()), &Fixed));
    assert_eq!(w.view().formula_text(), "=round(SUM(");
    assert_eq!(w.name_box_text(), "SUM");
}

fn type_text_in(w: &mut SheetWindow, s: &str) {
    w.handle(Input::Text(s.into()), &Fixed);
}

#[test]
fn colour_drop_downs_open_the_palette_and_more_colors() {
    let mut w = window();
    put(&mut w, &[(0, 0, "x")]);
    assert!(w.action("sheet.formatCellsDialog", &Fixed));
    {
        let f = w.form.as_mut().expect("form");
        f.set_tab(2);
        assert_eq!(f.color("color"), "");
    }
    // Click the Font page's colour box, then a standard colour (red, second cell).
    let click = |w: &mut SheetWindow, x: f32, y: f32| {
        let d = w.form.as_ref().expect("form").dev([x, y, 0.0, 0.0]);
        press(w, (d[0], d[1]), 1);
    };
    let r = w
        .form
        .as_ref()
        .expect("form")
        .get("color")
        .expect("color")
        .rect;
    click(&mut w, r[0] + 10.0, r[1] + 10.0);
    click(&mut w, r[0] + 8.0 + 18.0 + 5.0, r[1] + r[3] + 166.0 + 5.0);
    assert_eq!(w.form.as_ref().expect("form").color("color"), "FF0000");
    // More Colors…: a Colors form over Format Cells; its hex code comes back.
    click(&mut w, r[0] + 10.0, r[1] + 10.0);
    click(&mut w, r[0] + 40.0, r[1] + r[3] + 200.0);
    assert_eq!(w.form_kind, form_impl::FormKind::MoreColors);
    w.form.as_mut().expect("colors").set_num("green", 128);
    w.more_colors_event(&form_impl::Event::Changed("green"));
    assert_eq!(w.form.as_ref().expect("colors").text("hex"), "FF8000");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.form_kind, form_impl::FormKind::FormatCells);
    assert_eq!(w.form.as_ref().expect("form").color("color"), "FF8000");
    let ok = w.fc_ok_point();
    press(&mut w, ok, 1);
    assert_eq!(w.view().format().color.to_uppercase(), "FF8000");
}

#[test]
fn insert_icons_shapes_smartart_equation_and_timeline() {
    let mut w = window();
    w.relayout(&Fixed);
    let groups = |w: &SheetWindow| {
        w.view
            .engine()
            .workbook()
            .map_or(0, |wb| wb.sheets[0].shape_groups.len())
    };
    // Icons: the library gallery; choosing an icon inserts its drawing (SVG stored as PNG).
    let [x, y, ww, hh] = w.ribbon.rect_of(RibbonHit::Tab(1)).expect("insert tab");
    press(&mut w, (x + ww / 2.0, y + hh / 2.0), 1);
    w.handle(
        Input::Release {
            x: x + ww / 2.0,
            y: y + hh / 2.0,
        },
        &Fixed,
    );
    assert!(w.action("sheet.icons", &Fixed));
    assert!(w.picker.is_some() && !w.library.is_empty());
    let id = w.library[0].id.clone();
    w.picker = None;
    assert!(w.pick_drawing(&format!("icon:{id}")));
    assert_eq!(groups(&w), 1);
    // Shapes: the shapes gallery.
    assert!(w.action("sheet.shapes", &Fixed));
    assert!(w.picker.is_some());
    w.picker = None;
    assert!(w.pick_drawing("shape:ellipse"));
    assert_eq!(groups(&w), 2);
    // Equation: a text box with the equation's linear form.
    assert!(w.action("equation.eqPythagoras", &Fixed));
    assert_eq!(groups(&w), 3);
    // SmartArt by family.
    assert!(w.action("smartart.process", &Fixed));
    assert_eq!(
        w.view
            .engine()
            .workbook()
            .map(|wb| wb.sheets[0].diagrams.len()),
        Some(1)
    );
    // Timeline without a pivot table: a hint.
    assert!(w.action("sheet.timeline", &Fixed));
    assert!(w.status.contains("PivotTable"));
    // Screenshot and 3D Models are shown dimmed.
    assert!(!sheet_enabled("sheet.screenshot", &w.format_state()));
}

#[test]
fn charts_select_move_resize_and_design() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Q"),
            (0, 1, "Sales"),
            (1, 0, "Q1"),
            (1, 1, "5"),
            (2, 0, "Q2"),
            (2, 1, "7"),
        ],
    );
    w.relayout(&Fixed);
    assert!(w.action("chart.column", &Fixed));
    // The new chart is selected and Chart Design is shown.
    assert_eq!(w.chart_sel, Some(0));
    assert_eq!(w.ribbon.active_label(), "sheetChart.tabDesign");
    assert_eq!(w.name_box_text(), "Chart 1");
    // The cell selection is not drawn while the chart is selected.
    let mut with = PaintHash::default();
    w.view.paint(&mut with);
    w.view.set_object_selected(false);
    let mut without = PaintHash::default();
    w.view.paint(&mut without);
    assert_ne!(with.finish(), without.finish());
    w.view.set_object_selected(true);
    // The chart tab is laid out after the input (its buttons have places), not left blank.
    w.handle(Input::Move { x: 1.0, y: 1.0 }, &Fixed);
    assert!(w
        .ribbon
        .rect_of(RibbonHit::Command("sheet.chartQuickLayout"))
        .is_some());
    let charts = |w: &mut SheetWindow| w.view.query(json!({ "cmd": "charts" })).expect("charts");
    let from = charts(&mut w)[0]["from"].clone();
    // Drag its body two rows down.
    let f = [
        from[0].as_u64().unwrap() as u32,
        from[1].as_u64().unwrap() as u32,
    ];
    let r = w.view.cell_rect(f[0] + 1, f[1] + 1).expect("cell");
    let b = w.view.cell_rect(f[0] + 3, f[1] + 1).expect("cell");
    press(&mut w, (r[0] + 3.0, r[1] + 3.0), 1);
    w.handle(
        Input::Drag {
            x: b[0] + 3.0,
            y: b[1] + 3.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: b[0] + 3.0,
            y: b[1] + 3.0,
        },
        &Fixed,
    );
    assert_eq!(
        charts(&mut w)[0]["from"][0].as_u64(),
        Some(u64::from(f[0]) + 2)
    );
    // Design commands reach the chart.
    assert!(w.action("chartEl.title", &Fixed));
    assert_eq!(charts(&mut w)[0]["title"], "Chart Title");
    assert!(w.action("chartEl.legendBottom", &Fixed));
    assert_eq!(charts(&mut w)[0]["legend"], "b");
    assert!(w.action("chartType.line", &Fixed));
    assert_eq!(charts(&mut w)[0]["kind"], "line");
    let layout = format!("chartLayout.{}", bo_sheet::chart_style::LAYOUTS[1]);
    assert!(w.action(&layout, &Fixed));
    assert!(w.status.is_empty(), "{}", w.status);
    assert_ne!(sheet_ribbon::menu("sheet.chartColors", &[]), vec![]);
    // Pressing a cell deselects; Delete on a selected chart removes it.
    let far = w.view.cell_rect(0, 0).expect("a1");
    press(&mut w, (far[0] + 2.0, far[1] + 2.0), 1);
    w.handle(
        Input::Release {
            x: far[0] + 2.0,
            y: far[1] + 2.0,
        },
        &Fixed,
    );
    assert_eq!(w.chart_sel, None);
    w.set_chart_selection(Some(0), false);
    w.handle(Input::Key(Key::Delete, Mods::default()), &Fixed);
    assert_eq!(charts(&mut w).as_array().map(Vec::len), Some(0));
}

#[test]
fn keyboard_shortcuts_as_the_reference() {
    use crate::sheet_view::cycle_reference;
    // F4 cycles $A$1 → A$1 → $A1 → A1 → $A$1, ranges as a whole.
    assert_eq!(cycle_reference("=A1+2", 2), Some(("=$A$1+2".into(), 5)));
    assert_eq!(cycle_reference("=$A$1", 5), Some(("=A$1".into(), 4)));
    assert_eq!(cycle_reference("=A$1", 4), Some(("=$A1".into(), 4)));
    assert_eq!(cycle_reference("=$A1", 4), Some(("=A1".into(), 3)));
    assert_eq!(
        cycle_reference("=SUM(B2:C3)", 9),
        Some(("=SUM($B$2:$C$3)".into(), 14))
    );
    assert_eq!(cycle_reference("=SUM(", 5), None);
    let mut w = window();
    put(&mut w, &[(0, 0, "1"), (1, 0, "2"), (0, 1, "x")]);
    let ctrl = |c: char| {
        Input::Key(
            Key::Letter(c),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        )
    };
    // F2 then F4 in the editor.
    w.view_mut().select((5, 0), (5, 0), (5, 0));
    type_text_in(&mut w, "=A1");
    w.handle(Input::Key(Key::F(4), Mods::default()), &Fixed);
    assert_eq!(w.view().formula_text(), "=$A$1");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // Ctrl+D fills the top cell down the selection, Ctrl+R the left cell right.
    w.view_mut().select((0, 0), (2, 0), (0, 0));
    w.handle(ctrl('d'), &Fixed);
    assert_eq!(
        w.view
            .engine()
            .cell(0, 2, 0)
            .map(|c| c.display)
            .ok()
            .as_deref(),
        Some("1")
    );
    w.view_mut().select((0, 1), (0, 2), (0, 1));
    w.handle(ctrl('r'), &Fixed);
    assert_eq!(
        w.view
            .engine()
            .cell(0, 0, 2)
            .map(|c| c.display)
            .ok()
            .as_deref(),
        Some("x")
    );
    // Ctrl+1 opens Format Cells.
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    w.handle(ctrl('1'), &Fixed);
    assert_eq!(w.form_kind, form_impl::FormKind::FormatCells);
    w.form = None;
    // Ctrl+; types today's date into the editor.
    w.view_mut().select((7, 0), (7, 0), (7, 0));
    w.handle(ctrl(';'), &Fixed);
    assert!(w.view().editing());
    assert_eq!(w.view().formula_text().matches('/').count(), 2);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // Alt+= puts AutoSum under a column of numbers.
    w.view_mut().select((3, 0), (3, 0), (3, 0));
    w.handle(
        Input::Key(
            Key::Letter('='),
            Mods {
                alt: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert!(
        w.view().formula_text().starts_with("=SUM("),
        "{}",
        w.view().formula_text()
    );
}

#[test]
fn data_formulas_and_view_tabs_round_6() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Name"),
            (0, 1, "Amount"),
            (1, 0, "A"),
            (1, 1, "5"),
            (2, 0, "A"),
            (2, 1, "7"),
            (3, 0, "B"),
            (3, 1, "9"),
        ],
    );
    w.relayout(&Fixed);
    // The reference's Data tab commands are on the ribbon (some dimmed).
    let ids = sheet_ribbon::tab_ids();
    for id in [
        "sheet.getData",
        "sheet.refreshAll",
        "sheet.flashFill",
        "sheet.consolidate",
        "sheet.whatIf",
        "sheet.outline",
        "sheet.definedNames",
        "sheet.fnRecent",
        "sheet.customViews",
        "sheet.zoomDialog",
    ] {
        assert!(ids.contains(&id), "{id}");
    }
    assert!(!sheet_enabled("sheet.split", &w.format_state()));
    // Get Data from Text/CSV: a text file becomes a query table on a new sheet.
    assert!(w.action("sheet.fromText", &Fixed));
    assert!(w.take_requests().contains(&Request::PickMergeData));
    assert!(w.pick_filters()[0].1.contains("csv"));
    assert!(w.picked_file(b"City,Count\nHue,3\nVinh,4\n", "cities.csv"));
    assert_eq!(w.view.sheet_names().len(), 2);
    assert_eq!(
        w.view
            .engine()
            .cell(1, 1, 0)
            .map(|c| c.display)
            .ok()
            .as_deref(),
        Some("Hue")
    );
    assert!(w.action("sheet.refreshAllNow", &Fixed));
    assert!(w.status.contains('1'), "{}", w.status);
    w.view.set_sheet(0);
    // Outline: group rows 2–3, then subtotals.
    w.view_mut().select((1, 0), (2, 1), (1, 0));
    assert!(w.action("outline.group", &Fixed));
    assert!(w.action("outline.clear", &Fixed));
    // Custom views: add one, then the list shows it.
    assert!(w.action("sheet.customViews", &Fixed));
    w.form.as_mut().expect("views").set_text("name", "Mine");
    w.data_form_event(&form_impl::Event::Clicked("add"), &Fixed);
    assert_eq!(w.custom_view_names().len(), 1);
    w.data_form_event(&form_impl::Event::Cancel, &Fixed);
    // Zoom: 75 %.
    assert!(w.action("sheet.zoomDialog", &Fixed));
    click_form(&mut w, "z2", 5.0, 10.0);
    w.data_form_event(&form_impl::Event::Ok, &Fixed);
    assert!((w.view.zoom() - 0.75).abs() < 0.01);
    // Consolidate the list by name into D1.
    w.view_mut().select((0, 3), (0, 3), (0, 3));
    assert!(w.action("sheet.consolidate", &Fixed));
    w.form.as_mut().expect("cons").set_text("ref", "$A$1:$B$4");
    w.data_form_event(&form_impl::Event::Ok, &Fixed);
    assert_eq!(
        w.view
            .engine()
            .cell(0, 1, 4)
            .map(|c| c.display)
            .ok()
            .as_deref(),
        Some("12"),
        "{}",
        w.status
    );
}

#[test]
fn cell_style_and_table_galleries_apply_their_choice() {
    let mut w = window();
    put(
        &mut w,
        &[(0, 0, "A"), (0, 1, "B"), (1, 0, "1"), (1, 1, "2")],
    );
    w.relayout(&Fixed);
    assert!(w.action("sheet.cellStyles", &Fixed));
    let g = w.gallery.as_ref().expect("gallery");
    // Good / Bad / Neutral, Data and Model, Titles, Themed, Number Format; menu lines.
    assert_eq!(g.sections.len(), 5);
    assert_eq!(g.sections[4].1.len(), 5);
    assert_eq!(g.footer.len(), 2);
    assert_eq!(
        w.gallery_choice_at(4, 1).as_deref(),
        Some("style.Comma [0]")
    );
    // Calculation shows its four edges (the reference frames it), Heading 1 a thick rule.
    let edges = |s: usize, i: usize| match &w.gallery.as_ref().expect("g").sections[s].1[i].1 {
        gallery_impl::Tile::Cell { edges, .. } => edges.map(|e| e.map(|(l, _)| l)),
        gallery_impl::Tile::Table { .. } => [None; 4],
    };
    assert!(edges(1, 0).iter().all(Option::is_some));
    assert_eq!(edges(2, 0)[2], Some(gallery_impl::Line::Thick));
    assert_eq!(edges(1, 4)[2], Some(gallery_impl::Line::Double));
    assert_eq!(w.gallery_choice_at(0, 2).as_deref(), Some("style.Good"));
    // Clicking the Good tile styles the selection.
    let r = w.gallery.as_ref().expect("g").rect();
    let hit = (0..400)
        .flat_map(|dx| (0..200).map(move |dy| (dx as f32, dy as f32)))
        .map(|(dx, dy)| (r[0] + dx, r[1] + dy))
        .find(|(x, y)| w.gallery.as_ref().and_then(|g| g.hit(*x, *y)) == Some((0, 2)))
        .expect("Good tile");
    press(&mut w, hit, 1);
    assert!(w.gallery.is_none());
    let st = w
        .view
        .query(json!({ "cmd": "formatState", "row": 0, "col": 0 }))
        .expect("st");
    assert_eq!(st["styleName"], "Good");
    // New Cell Style…: the Style form takes the active cell's format under a new name, and
    // the gallery then shows a Custom group with it.
    assert!(w.action("sheet.newCellStyle", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::CellStyle);
    assert_eq!(w.form.as_ref().expect("form").text("name"), "Style 1");
    w.cell_style_event(&form_impl::Event::Ok);
    assert!(w.form.is_none());
    assert!(w.action("sheet.cellStyles", &Fixed));
    let g = w.gallery.take().expect("gallery");
    assert_eq!(g.sections.len(), 6);
    assert_eq!(g.sections[0].1[0].0, "style.Style 1");
    // Merge Styles… asks the host for a workbook; a file that is not one is reported.
    assert!(w.action("sheet.mergeStyles", &Fixed));
    assert!(w.take_requests().contains(&Request::PickMergeData));
    assert!(w.pick_filters()[0].1.contains("xlsx"));
    assert!(w.picked_file(b"not a workbook", "x.xlsx"));
    assert!(w.status.contains("not a workbook"));
    // Format as Table: the reference's 60 styles (Light 21, Medium 28, Dark 11).
    assert!(w.action("sheet.formatTable", &Fixed));
    let g = w.gallery.as_ref().expect("gallery");
    assert_eq!(g.sections.iter().map(|s| s.1.len()).sum::<usize>(), 60);
    let c = w.gallery_choice_at(1, 1).expect("medium 2");
    assert_eq!(c, "table.TableStyleMedium2");
    w.gallery = None;
    assert!(w.action(&c, &Fixed));
    let tables = w.view.query(json!({ "cmd": "tables" })).expect("tables");
    assert_eq!(tables["tables"].as_array().map(Vec::len), Some(1));
}

// ---------------------------------------------------------------- round 27

#[test]
fn insert_tab_fits_at_1500_px() {
    let mut w = window();
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 900.0,
        },
        &Fixed,
    );
    assert!(w.ribbon.select_tab("ribbon.tabs.insert"));
    w.relayout(&Fixed);
    assert!(!w.ribbon.overflows(), "Insert fits without scroll arrows");
    // Folded groups show no group label (the button carries the name).
    let t = sheet_ribbon::tabs();
    let ins = &t[1];
    assert!(ins.groups.iter().any(|g| g.label.is_empty()
        && g.large
            .contains(&crate::ribbon::Item::Large("sheet.illustrations"))));
}

fn chart_window() -> SheetWindow {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 1, "'2025"),
            (0, 2, "'2026"),
            (1, 0, "Q1"),
            (1, 1, "5"),
            (1, 2, "6"),
            (2, 0, "Q2"),
            (2, 1, "7"),
            (2, 2, "8"),
        ],
    );
    assert!(w.view_mut().run(serde_json::json!({
        "cmd": "insertChart",
        "range": { "r0": 0, "c0": 0, "r1": 2, "c1": 2 },
        "kind": "column"
    })));
    w.set_chart_selection(Some(0), true);
    w
}

#[test]
fn chart_design_switch_select_data_and_move_chart() {
    let mut w = chart_window();
    let data = |w: &mut SheetWindow| {
        w.view_mut()
            .query(serde_json::json!({ "cmd": "chartData", "index": 0 }))
            .expect("data")
    };
    assert_eq!(data(&mut w)["byRow"], false);
    assert!(w.action("sheet.chartSwitch", &Fixed));
    assert_eq!(data(&mut w)["byRow"], true);
    // Select Data: the series list, remove one, OK replots.
    assert!(w.action("sheet.chartSelectData", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::ChartData);
    let n = w.chart_data.as_ref().map_or(0, |s| s.series.len());
    assert_eq!(n, 2);
    w.chart_data_event(
        form_impl::FormKind::ChartData,
        &form_impl::Event::Clicked("remove"),
    );
    w.chart_data_event(form_impl::FormKind::ChartData, &form_impl::Event::Ok);
    assert!(w.form.is_none());
    assert_eq!(data(&mut w)["series"].as_array().map_or(0, Vec::len), 1);
    // Move Chart to a new sheet: the sheet comes before, the chart is selected there.
    assert!(w.action("sheet.chartMove", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::MoveChart);
    if let Some(f) = w.form.as_mut() {
        f.set_on("newSheet", true);
        f.set_on("objectIn", false);
    }
    w.chart_data_event(form_impl::FormKind::MoveChart, &form_impl::Event::Ok);
    let names: Vec<String> = w.view().sheet_names().into_iter().flatten().collect();
    assert_eq!(names, vec!["Chart1".to_owned(), "Sheet1".to_owned()]);
    assert_eq!(w.view().sheet(), 0);
    assert_eq!(w.chart_sel, Some(0));
}

#[test]
fn table_design_tab_options_and_quick_styles() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Name"),
            (0, 1, "Score"),
            (1, 0, "a"),
            (1, 1, "1"),
            (2, 0, "b"),
            (2, 1, "2"),
        ],
    );
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("table.TableStyleMedium2", &Fixed));
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    assert!(w.table_tab, "the Table Design tab shows in a table");
    assert!(w
        .ribbon
        .contextual_labels()
        .contains(&"sheetTable.tabDesign"));
    let checks = |w: &SheetWindow| w.table_checks();
    assert!(checks(&w).contains(&("sheet.tblBandedRows", true)));
    assert!(w.action("sheet.tblTotalRow", &Fixed));
    assert!(checks(&w).contains(&("sheet.tblTotalRow", true)));
    assert!(w.action("sheet.tblBandedRows", &Fixed));
    assert!(checks(&w).contains(&("sheet.tblBandedRows", false)));
    // The gallery restyles the table the active cell is in (no second table).
    assert!(w.action("table.TableStyleLight9", &Fixed));
    let tables = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "tables" }))
        .expect("tables");
    let list = tables["tables"].as_array().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["style"], "TableStyleLight9");
    // Leaving the table hides the tab.
    w.view_mut().select((10, 5), (10, 5), (10, 5));
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    assert!(!w.table_tab);
}

#[test]
fn table_style_gallery_has_60_reference_tiles() {
    let mut w = window();
    assert!(w.open_gallery(false, &Fixed));
    let g = w.gallery.as_ref().expect("gallery");
    let n: usize = g.sections.iter().map(|(_, t)| t.len()).sum();
    assert_eq!(n, 60);
    assert_eq!(g.sections[0].1.len(), 21);
    assert_eq!(g.sections[1].1.len(), 28);
    assert_eq!(g.sections[2].1.len(), 11);
    // Tiles are 77 x 56 px at 125 %.
    assert_eq!((g.tile.0 * 1.25).round(), 77.0);
    assert_eq!((g.tile.1 * 1.25).round(), 56.0);
}

#[test]
fn conditional_formatting_menu_rules_and_manager() {
    let mut w = window();
    put(&mut w, &[(0, 0, "1"), (1, 0, "5"), (2, 0, "9")]);
    w.view_mut().select((0, 0), (2, 0), (0, 0));
    assert!(w.action("sheet.conditional", &Fixed));
    assert!(w.cf_menu.is_some(), "the cascading menu opens");
    w.cf_menu = None;
    // Greater Than… with the first preset.
    w.open_cf_dialog("greaterThan");
    assert_eq!(w.form_kind, form_impl::FormKind::CfRule);
    if let Some(f) = w.form.as_mut() {
        f.set_text("v1", "4");
    }
    w.cf_form_event(form_impl::FormKind::CfRule, &form_impl::Event::Ok);
    assert!(w.form.is_none());
    // A colour scale tile and a data bar tile.
    w.cf_choose("cfs.0", &Fixed);
    w.cf_choose("cfb.s.638EC6", &Fixed);
    let rules = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "listCf" }))
        .expect("rules");
    let kinds: Vec<&str> = rules["rules"]
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|r| r["kind"].as_str())
        .collect();
    assert_eq!(kinds.len(), 3, "{kinds:?}");
    assert!(kinds.contains(&"cellIs"));
    // The manager lists them for the selection; Delete removes the chosen one.
    w.open_cf_manager();
    assert_eq!(w.cf_rules.len(), 3);
    w.cf_form_event(
        form_impl::FormKind::CfManager,
        &form_impl::Event::Clicked("delete"),
    );
    assert_eq!(w.cf_rules.len(), 2);
    w.cf_form_event(form_impl::FormKind::CfManager, &form_impl::Event::Ok);
    assert!(w.form.is_none());
}

#[test]
fn autocomplete_suggests_an_entry_of_the_column() {
    let mut w = window();
    put(&mut w, &[(0, 0, "Apple"), (1, 0, "Banana")]);
    w.view_mut().select((2, 0), (2, 0), (2, 0));
    type_in(&mut w, "a");
    assert_eq!(w.view().formula_text(), "apple");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(input(&w, 2, 0), "Apple");
    // Backspace drops the suggestion; a second match stops suggesting.
    w.view_mut().select((3, 0), (3, 0), (3, 0));
    type_in(&mut w, "b");
    w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    assert_eq!(w.view().formula_text(), "b");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(input(&w, 3, 0), "b");
}

#[test]
fn drag_moves_cells_and_double_click_fills_down() {
    let mut w = window();
    put(&mut w, &[(0, 0, "7"), (0, 2, "=A1*2")]);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    let [l, t, _, b] = w.view().sel_rect_for_test().expect("selection");
    // Grab the left border, drop two rows lower.
    let grab = (l, f32::midpoint(t, b));
    press(&mut w, grab, 1);
    let to = cell_xy(&w, 2, 0);
    w.handle(Input::Drag { x: grab.0, y: to.1 }, &Fixed);
    release(&mut w, (grab.0, to.1));
    assert_eq!(input(&w, 2, 0), "7");
    assert_eq!(input(&w, 0, 0), "");
    assert_eq!(input(&w, 0, 2), "=A3*2", "references follow the moved cell");
    // Double click on the fill handle: down as far as the column to the left has data.
    put(
        &mut w,
        &[(0, 3, "1"), (1, 3, "1"), (2, 3, "1"), (0, 4, "x")],
    );
    w.view_mut().select((0, 4), (0, 4), (0, 4));
    let [_, _, r, b] = w.view().sel_rect_for_test().expect("selection");
    press(&mut w, (r, b), 2);
    release(&mut w, (r, b));
    assert_eq!(input(&w, 2, 4), "x");
    assert_eq!(input(&w, 3, 4), "");
}

#[test]
fn paste_special_values_and_transpose() {
    let mut w = window();
    put(&mut w, &[(0, 0, "2"), (0, 1, "=A1*3")]);
    w.view_mut().select((0, 0), (0, 1), (0, 0));
    assert!(w.action("edit.copy", &Fixed));
    w.view_mut().select((2, 0), (2, 0), (2, 0));
    assert!(w.action("sheet.pasteSpecial", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::PasteSpecial);
    w.paste_special_event(&form_impl::Event::Changed("pValues"));
    if let Some(f) = w.form.as_mut() {
        f.set_on("pValues", true);
        f.set_on("transpose", true);
    }
    w.paste_special_event(&form_impl::Event::Ok);
    assert!(w.form.is_none());
    assert_eq!(input(&w, 2, 0), "2");
    assert_eq!(input(&w, 3, 0), "6", "values, transposed");
}

#[test]
fn paste_special_operation_multiplies() {
    let mut w = window();
    put(&mut w, &[(0, 0, "2"), (0, 2, "5"), (1, 2, "7")]);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("edit.copy", &Fixed));
    w.view_mut().select((0, 2), (1, 2), (0, 2));
    assert!(w.action("sheet.pasteSpecial", &Fixed));
    w.paste_special_event(&form_impl::Event::Changed("pValues"));
    w.paste_special_event(&form_impl::Event::Changed("oMultiply"));
    if let Some(f) = w.form.as_mut() {
        f.set_on("pValues", true);
        f.set_on("oMultiply", true);
    }
    assert!(w.form.as_ref().is_some_and(|f| !f.on("oNone")));
    w.paste_special_event(&form_impl::Event::Ok);
    // One copied cell fills the two selected cells, each multiplied.
    assert_eq!(input(&w, 0, 2), "10");
    assert_eq!(input(&w, 1, 2), "14");
}

#[test]
fn page_layout_themes_apply_and_save() {
    let mut w = window();
    assert_eq!(
        sheet_ribbon::menu("sheet.themes", &[]).len(),
        5,
        "4 themes and Browse"
    );
    assert!(w.action("theme.ocean", &Fixed));
    let saved = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "saveThemeFile" }))
        .expect("theme");
    let xml = bo_engine::theme_file::theme_xml_of_b64(saved["file"].as_str().expect("file"))
        .expect("xml");
    let text = String::from_utf8_lossy(&xml);
    assert!(text.contains("BlueOffice Ocean"));
    assert!(text.contains("1F6FB2"));
    // Fonts only: the Bamboo font pair with Ocean's colours.
    assert!(w.action("themeFonts.bamboo", &Fixed));
    let saved = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "saveThemeFile" }))
        .expect("theme");
    let xml = bo_engine::theme_file::theme_xml_of_b64(saved["file"].as_str().expect("file"))
        .expect("xml");
    let text = String::from_utf8_lossy(&xml);
    assert!(text.contains("Times New Roman") && text.contains("1F6FB2"));
    // Saved with the workbook.
    let bytes = w.view_mut().engine_mut().save("xlsx", 0).expect("save");
    let mut back = SheetEngine::with_bundled_fonts();
    back.open(&bytes, "t.xlsx").expect("open");
    let b64 = back.exec_json(r#"{"cmd":"saveThemeFile"}"#).expect("theme");
    assert!(b64.contains("file"));
}

#[test]
fn band_display_lists_give_the_same_image_as_a_full_layout() {
    let mut e = SheetEngine::with_bundled_fonts();
    e.new_workbook("Book1.xlsx").expect("workbook");
    let cells: Vec<(u32, u32, String)> = (0..300)
        .flat_map(|r| {
            (0..10).map(move |c| {
                let v = if c % 4 == 0 {
                    format!("Row {r} text that overflows its cell")
                } else {
                    format!("{}", r * 7 + c)
                };
                (r, c, v)
            })
        })
        .collect();
    e.set_inputs(0, &cells).expect("cells");
    let mut w = window_with(e);
    // Borders and a fill on some rows: their edges must match too.
    assert!(w.view_mut().run(
        json!({ "cmd": "borders", "range": { "r0": 5, "c0": 0, "r1": 40, "c1": 3 },
        "spec": { "preset": "all" } })
    ));
    let mut store = RingRaster::default();
    w.prepare(&mut store).expect("grid");
    for i in 0..15 {
        w.handle(Input::Wheel(if i % 4 == 3 { -75.0 } else { 75.0 }), &Fixed);
        w.prepare(&mut store).expect("grid");
    }
    assert!(store.band_turns >= 14, "{} band turns", store.band_turns);
    let shown = store.shown();
    let mut full = Raster::default();
    w.view_mut().invalidate();
    w.prepare(&mut full).expect("grid");
    assert_eq!(full.size, store.size);
    let (mut off, mut worst) = (0, 0_u8);
    let (iw, ih) = full.size;
    for (i, (a, b)) in shown.chunks(4).zip(full.img.chunks(4)).enumerate() {
        // A border straddling an edge of the image is clipped in half by a fresh layout but
        // kept as drawn before in a turned image: the image's edge pixels may differ.
        let (x, y) = (i as u32 % iw, i as u32 / iw);
        if y == 0 || y + 1 == ih || x == 0 || x + 1 == iw {
            continue;
        }
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        worst = worst.max(d);
        if d > 4 {
            if off < 12 {
                eprintln!(
                    "band diff at ({}, {}) by {d}: {a:?} vs {b:?}",
                    i as u32 % full.size.0,
                    i as u32 / full.size.0
                );
            }
            off += 1;
        }
    }
    assert!(
        off * 20_000 < shown.len() / 4 && worst <= 32,
        "band image: {off} pixels differ (by up to {worst})"
    );
}

/// A grid store keeping only the last full image's list (page views).
#[derive(Default)]
struct ListStore {
    lists: usize,
    commands: usize,
}

impl GridStore for ListStore {
    fn update(
        &mut self,
        list: &bo_render::PageList,
        _: &SheetEngine,
        _: f32,
    ) -> Result<(), String> {
        self.lists += 1;
        self.commands = list.commands.len();
        Ok(())
    }
}

fn long_sheet(w: &mut SheetWindow) {
    let cells: Vec<(u32, u32, String)> = (0..120)
        .flat_map(|r| (0..14).map(move |c| (r, c, format!("{}", r * 14 + c))))
        .collect();
    w.view_mut()
        .engine_mut()
        .set_inputs(0, &cells)
        .expect("cells");
    w.view_mut().changed();
}

#[test]
fn page_break_preview_from_the_status_bar() {
    let mut w = window();
    long_sheet(&mut w);
    assert_eq!(w.view_mode(), ViewMode::Normal);
    // Third status bar view button: Page Break Preview at 60 %, kept in the sheet.
    let [_, _, pbp] = w.view_button_rects();
    press(&mut w, (pbp[0] + 5.0, pbp[1] + 5.0), 1);
    assert_eq!(w.view_mode(), ViewMode::PageBreak);
    assert!((w.view().zoom() - 0.6).abs() < 1e-3);
    // The breaks are those of the sheet at 100 % (the zoom does not move them).
    let at_60 = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "pageBreakPreview" }))
        .expect("breaks");
    assert!(w.view_mut().set_zoom(1.0));
    let at_100 = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "pageBreakPreview" }))
        .expect("breaks");
    assert_eq!(
        at_60["areas"][0]["rowBreaks"],
        at_100["areas"][0]["rowBreaks"]
    );
    assert!(w.view_mut().set_zoom(0.6));
    let state = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "sheetViewState" }))
        .expect("state");
    assert_eq!(state["view"], "pageBreakPreview");
    let mut store = ListStore::default();
    w.prepare(&mut store).expect("grid");
    assert!(store.lists >= 1);
    // A horizontal automatic break lies on screen; dragging it up makes it manual there.
    let breaks = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "pageBreakPreview" }))
        .expect("breaks");
    let first = breaks["areas"][0]["rowBreaks"][0]["at"]
        .as_u64()
        .expect("a row break") as u32;
    w.view_mut().scroll_rows(i64::from(first) - 10);
    let x = w.view().frame_for_test()[0] + 45.0;
    let (row, y0) = w
        .view()
        .rows_for_test()
        .iter()
        .find_map(|&(r, y0, _)| {
            w.view()
                .break_at(x, y0 - 1.0)
                .filter(|b| b.1)
                .map(|_| (r, y0))
        })
        .expect("a row break on screen");
    let target = w
        .view()
        .rows_for_test()
        .iter()
        .find(|s| s.0 == row - 5)
        .map(|s| s.1)
        .expect("row above");
    press(&mut w, (x, y0 - 1.0), 1);
    w.handle(Input::Drag { x, y: target }, &Fixed);
    release(&mut w, (x, target));
    let err = w.view_mut().take_error();
    let ws = &w.view().engine().workbook().expect("wb").sheets[0];
    assert!(
        // The model keeps the last row before a break (§18.3.1.14 brk/@id, 0-based here).
        ws.page_setup.row_breaks.contains(&(row - 6)),
        "{:?} row {row} error {err:?}",
        ws.page_setup.row_breaks
    );
    // Back to Normal with the View tab.
    assert!(w.action("sheet.viewNormal", &Fixed));
    assert_eq!(w.view_mode(), ViewMode::Normal);
    assert!((w.view().zoom() - 1.0).abs() < 1e-3);
}

#[test]
fn page_layout_view_shows_pages_and_selects_on_them() {
    let mut w = window();
    long_sheet(&mut w);
    assert!(w.action("sheet.viewPageLayout", &Fixed));
    assert_eq!(w.view_mode(), ViewMode::PageLayout);
    let mut store = ListStore::default();
    w.prepare(&mut store).expect("grid");
    assert_eq!(store.lists, 1);
    assert!(store.commands > 100, "pages drawn: {}", store.commands);
    // Cell B3 of the first page: press on it.
    let (x, y) = w.view().layout_cell_center(2, 1).expect("B3 on screen");
    press(&mut w, (x, y), 1);
    assert_eq!(w.view().active(), (2, 1));
    // Moving far down scrolls the pages.
    let before = w.view_mut().scroll_info(true).0;
    for _ in 0..60 {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    }
    assert_eq!(w.view().active(), (62, 1));
    assert!(w.view_mut().scroll_info(true).0 > before);
    // The wheel scrolls too.
    let s = w.view_mut().scroll_info(true).0;
    w.handle(Input::Wheel(120.0), &Fixed);
    assert!(w.view_mut().scroll_info(true).0 > s);
    // The view is kept in the file's sheet view.
    let state = w
        .view_mut()
        .query(serde_json::json!({ "cmd": "sheetViewState" }))
        .expect("state");
    assert_eq!(state["view"], "pageLayout");
    // The status bar names the page of the active cell.
    let (n, of) = w
        .view()
        .layout_page_of_active()
        .expect("page of the active cell");
    assert!(n >= 1 && of >= n, "{n} of {of}");
    let mut rec = Rec::default();
    w.paint(&mut rec);
    let want = format!("Page: {n} of {of}");
    assert!(rec.texts.iter().any(|t| t.0 == want), "{want}");
}

fn tab_xy(w: &SheetWindow, sheet: usize) -> (f32, f32) {
    let (_, x, tw) = *w
        .tab_rects
        .iter()
        .find(|t| t.0 == sheet)
        .expect("tab shown");
    let y = w.size.1 - w.status_h() - w.tabs_h() / 2.0;
    (x + tw / 2.0, y)
}

#[test]
fn sheet_tabs_rename_move_colour_hide() {
    let mut w = window();
    assert!(w.action("tab.insert", &Fixed));
    assert!(w.action("tab.insert", &Fixed));
    let names = |w: &SheetWindow| -> Vec<String> {
        w.view()
            .engine()
            .workbook()
            .expect("wb")
            .sheets
            .iter()
            .map(|s| s.name.clone())
            .collect()
    };
    assert_eq!(names(&w).len(), 3);
    // Double-click renames in place: the name is selected, typing replaces it.
    let at = tab_xy(&w, 0);
    press(&mut w, at, 1);
    release(&mut w, at);
    press(&mut w, at, 2);
    release(&mut w, at);
    assert!(w.tab_edit.is_some());
    w.handle(Input::Text("Data".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(names(&w)[0], "Data");
    // Drag the first tab past the last one.
    let from = tab_xy(&w, 0);
    let (lx, ly) = tab_xy(&w, 2);
    press(&mut w, from, 1);
    w.handle(
        Input::Drag {
            x: lx + 30.0,
            y: ly,
        },
        &Fixed,
    );
    release(&mut w, (lx + 30.0, ly));
    assert_eq!(names(&w)[2], "Data", "{:?}", names(&w));
    assert_eq!(w.view().sheet(), 2);
    // Tab colour from the palette.
    assert!(w.action("tab.color", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::TabColor);
    if let Some(f) = w.form.as_mut() {
        f.set_sel("swatches", Some(5));
    }
    w.tabs_form_event(
        form_impl::FormKind::TabColor,
        &form_impl::Event::Changed("swatches"),
    );
    assert!(w.form.is_none());
    assert!(w.tab_color(2).is_some());
    // Hide, then Unhide from the list.
    assert!(w.action("tab.hide", &Fixed));
    assert_eq!(w.view().sheet_names().iter().flatten().count(), 2);
    assert!(w.action("tab.unhide", &Fixed));
    w.tabs_form_event(form_impl::FormKind::Unhide, &form_impl::Event::Ok);
    assert_eq!(w.view().sheet_names().iter().flatten().count(), 3);
    assert_eq!(w.view().sheet(), 2);
}

#[test]
fn status_bar_statistics_are_chosen_in_its_menu() {
    let mut w = window();
    put(&mut w, &[(0, 0, "2"), (1, 0, "8"), (2, 0, "x")]);
    w.view_mut().select((0, 0), (2, 0), (0, 0));
    let st = w.view().stats();
    assert_eq!((st.min, st.max), (Some(2.0), Some(8.0)));
    assert!(w.status_item_on("status.sum"));
    assert!(!w.status_item_on("status.max"));
    let y = w.size.1 - w.status_h() / 2.0;
    w.handle(Input::Context { x: 600.0, y }, &Fixed);
    assert!(w.menu.is_some());
    w.menu = None;
    assert!(w.action("status.max", &Fixed));
    assert!(w.action("status.sum", &Fixed));
    assert!(w.status_item_on("status.max"));
    assert!(!w.status_item_on("status.sum"));
    // Shown in the reference's order with the workbook's decimal separator, right-aligned
    // 41 px before the view buttons.
    put(&mut w, &[(1, 0, "8,5")]);
    w.view_mut().select((0, 0), (2, 0), (0, 0));
    w.handle(Input::Blink, &Fixed);
    let texts: Vec<&str> = w.stat_widths.iter().map(|t| t.0.as_str()).collect();
    assert_eq!(texts, ["Average: 5,25", "Count: 3", "Maximum: 8,5"]);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t.0 == "Maximum: 8,5"));
}

#[test]
fn totals_row_drop_down_picks_the_function() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Item"),
            (0, 1, "Price"),
            (1, 0, "a"),
            (1, 1, "10"),
            (2, 0, "b"),
            (2, 1, "30"),
        ],
    );
    assert!(w.view_mut().run(serde_json::json!({
        "cmd": "createTable", "range": { "r0": 0, "c0": 0, "r1": 2, "c1": 1 },
    })));
    assert!(w.view_mut().run(serde_json::json!({
        "cmd": "tableUpdate", "table": 0, "totalsRow": true,
    })));
    // No button outside the totals row.
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    assert!(w.totals_button().is_none());
    w.view_mut().select((3, 1), (3, 1), (3, 1));
    let [x, y, bw, bh] = w.totals_button().expect("button beside the total");
    press(&mut w, (x + bw / 2.0, y + bh / 2.0), 1);
    // A compact list: 20 px rows at 125 %.
    let l = w.totals_list.clone().expect("function list");
    assert_eq!(l.row, 20.0);
    // Max is the fifth row.
    press(&mut w, (l.rect[0] + 10.0, l.rect[1] + 1.0 + 4.5 * l.row), 1);
    assert!(w.totals_list.is_none());
    assert_eq!(shown(&w, 3, 1), "30");
    assert!(w.action("total.max", &Fixed));
    assert_eq!(shown(&w, 3, 1), "30");
    assert!(w.action("total.average", &Fixed));
    assert_eq!(shown(&w, 3, 1), "20");
}

#[test]
fn new_table_style_dialog_makes_a_custom_style() {
    use form_impl::Event;
    let mut w = window();
    put(
        &mut w,
        &[(0, 0, "Item"), (0, 1, "Price"), (1, 0, "a"), (1, 1, "10")],
    );
    assert!(w.view_mut().run(serde_json::json!({
        "cmd": "createTable", "range": { "r0": 0, "c0": 0, "r1": 1, "c1": 1 },
    })));
    assert!(w.action("sheet.newTableStyle", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::TableStyle);
    assert_eq!(w.form.as_ref().expect("form").text("name"), "Table Style 1");
    // Header Row: a dark fill, white bold text.
    w.form.as_mut().expect("form").set_sel("elements", Some(7));
    w.tstyle_event(&Event::Changed("elements"));
    {
        let f = w.form.as_mut().expect("form");
        f.set_color("fill", "1F4E79");
        f.set_color("text", "FFFFFF");
        f.set_on("bold", true);
    }
    w.tstyle_event(&Event::Changed("fill"));
    w.tstyle_event(&Event::Changed("bold"));
    let desc = |w: &SheetWindow| match w.form.as_ref().and_then(|f| f.get("desc")).map(|c| &c.kind)
    {
        Some(form_impl::Kind::Note(s)) => s.clone(),
        _ => String::new(),
    };
    assert_eq!(desc(&w), "Shaded; Bold; Font color");
    // Whole Table shows its own (empty) format; Header Row keeps its own.
    w.form.as_mut().expect("form").set_sel("elements", Some(0));
    w.tstyle_event(&Event::Changed("elements"));
    assert_eq!(w.form.as_ref().expect("form").color("fill"), "-");
    assert_eq!(desc(&w), "");
    // The preview draws the header fill.
    w.form.as_mut().expect("form").set_sel("elements", Some(7));
    w.tstyle_event(&Event::Changed("elements"));
    w.handle(Input::Blink, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.rects.iter().any(|r| r.1 == Rgba(0x1F, 0x4E, 0x79, 255)));
    w.tstyle_event(&Event::Ok);
    assert!(w.form.is_none(), "{}", w.status);
    let wb = w.view().engine().workbook().expect("wb");
    assert_eq!(wb.styles.table_styles.len(), 1);
    assert_eq!(wb.styles.table_styles[0].elements[0].kind, "headerRow");
    // The gallery lists it under Custom, and choosing it styles the table.
    assert!(w.action("table.Table Style 1", &Fixed));
    let wb = w.view().engine().workbook().expect("wb");
    let t = &wb.sheets[0].tables[0];
    let l = bo_sheet::table_style::look_in(wb, t, 0, 0).expect("header look");
    assert_eq!(
        (l.fill.as_deref(), l.font.as_deref(), l.bold),
        (Some("1F4E79"), Some("FFFFFF"), true)
    );
    // A second style gets the next name.
    assert!(w.action("sheet.newTableStyle", &Fixed));
    assert_eq!(w.form.as_ref().expect("form").text("name"), "Table Style 2");
}

#[test]
fn tab_then_enter_returns_to_the_first_column_and_enter_direction_is_an_option() {
    let mut w = window();
    let key = |w: &mut SheetWindow, k: Key| {
        w.handle(Input::Key(k, Mods::default()), &Fixed);
    };
    w.view_mut().select((0, 1), (0, 1), (0, 1));
    for s in ["a", "b", "c"] {
        type_in(&mut w, s);
        key(&mut w, Key::Tab);
    }
    assert_eq!(w.view().active(), (0, 4));
    type_in(&mut w, "d");
    key(&mut w, Key::Enter);
    // Measured on the reference: Tab, Tab, …, Enter goes to the next row of the first column.
    assert_eq!(w.view().active(), (1, 1));
    assert_eq!(shown(&w, 0, 3), "c");
    // An arrow ends the run: Enter then goes straight down.
    key(&mut w, Key::Tab);
    key(&mut w, Key::Right);
    key(&mut w, Key::Enter);
    assert_eq!(w.view().active(), (2, 3));
    // "After pressing Enter, move selection": right, or not at all.
    w.view_mut().set_enter_move(Some((0, 1)));
    key(&mut w, Key::Enter);
    assert_eq!(w.view().active(), (2, 4));
    w.view_mut().set_enter_move(None);
    type_in(&mut w, "5");
    key(&mut w, Key::Enter);
    assert_eq!(w.view().active(), (2, 4));
    assert_eq!(shown(&w, 2, 4), "5");
    // Each entry is one undo step, as the reference.
    w.view_mut().undo();
    assert_eq!(shown(&w, 2, 4), "");
    assert_eq!(shown(&w, 0, 4), "d");
}

#[test]
fn table_header_buttons_open_the_table_filter() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Name"),
            (0, 1, "Region"),
            (1, 0, "An"),
            (1, 1, "North"),
            (2, 0, "Binh"),
            (2, 1, "South"),
            (3, 0, "Chi"),
            (3, 1, "North"),
        ],
    );
    w.view.run(serde_json::json!({ "cmd": "createTable", "range": { "r0": 0, "c0": 0, "r1": 3, "c1": 1 } }));
    assert_eq!(w.auto_filter(), None);
    let b = w.view().cell_box(0, 1).expect("header");
    let at = (b[2] - 4.0, f32::midpoint(b[1], b[3]));
    assert_eq!(
        w.filter_button_at(at.0, at.1),
        Some((1, Some([0, 0, 3, 1])))
    );
    press(&mut w, at, 1);
    let f = w.filter.as_ref().expect("drop-down open");
    assert_eq!(f.header, "Region");
    w.filter.as_mut().expect("open").toggle(2);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.status, "2 of 3 records found");
    let info = w.view().engine().info().expect("info");
    assert_eq!(info.sheets[0].hidden_rows, vec![(2, 2)]);
    // Sort Z to A from the table's drop-down sorts the table's rows.
    press(&mut w, at, 1);
    assert!(w.filter_item(1));
    assert_eq!(input(&w, 1, 1), "South");
}

#[test]
fn options_dialog_sets_enter_direction_r1c1_and_iteration() {
    let mut w = window();
    assert!(w.action("file.options", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Options);
    {
        let f = w.form.as_mut().expect("options");
        f.set_on("aEnter", true);
        f.set_sel("aDir", Some(1));
        f.set_on("fR1c1", true);
        f.set_on("fIterate", true);
        f.set_num("fCount", 50);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    let text = w.take_preferences().expect("changed");
    assert!(
        text.contains("enterDir=1") && text.contains("r1c1=1"),
        "{text}"
    );
    assert_eq!(w.view().enter_move(), Some((0, 1)));
    assert!(w.view().r1c1());
    let calc = w
        .view
        .query(serde_json::json!({ "cmd": "calcOptions" }))
        .expect("calc");
    assert_eq!(calc["options"]["iterate"], true);
    assert_eq!(calc["options"]["iterateCount"], 50);
    // R1C1: the name box and formulas read and type in R1C1 notation.
    put(&mut w, &[(0, 0, "2"), (1, 0, "=A1*3")]);
    w.view.select((1, 0), (1, 0), (1, 0));
    assert_eq!(w.view().name_box(), "R2C1");
    assert_eq!(w.view().formula_text(), "=R[-1]C*3");
    w.view.select((1, 1), (1, 1), (1, 1));
    w.handle(Input::Text("=R[-1]C[-1]+1".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(
        w.view().engine().cell(0, 1, 1).expect("cell").input,
        "=A1+1"
    );
}

fn region_list(w: &mut SheetWindow) {
    put(
        w,
        &[
            (0, 0, "Region"),
            (0, 1, "Sales"),
            (1, 0, "North"),
            (1, 1, "5"),
            (2, 0, "North"),
            (2, 1, "7"),
            (3, 0, "South"),
            (3, 1, "4"),
            (4, 0, "South"),
            (4, 1, "4"),
        ],
    );
    w.view.select((1, 0), (1, 0), (1, 0));
}

#[test]
fn subtotal_dialog_adds_and_removes_subtotals() {
    let mut w = window();
    region_list(&mut w);
    assert!(w.action("outline.subtotal", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Subtotal);
    {
        let f = w.form.as_ref().expect("subtotal");
        // The last column is ticked, as the reference.
        assert!(!f.on("c0") && f.on("c1"));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    // North 5 + 7 = 12 under the North rows.
    assert_eq!(shown(&w, 3, 1), "12");
    assert!(w.action("outline.subtotal", &Fixed));
    w.form_event(
        form_impl::FormKind::Subtotal,
        &form_impl::Event::Clicked("removeAll"),
        &Fixed,
    );
    assert_eq!(input(&w, 3, 0), "South");
}

#[test]
fn remove_duplicates_dialog_compares_the_ticked_columns() {
    let mut w = window();
    region_list(&mut w);
    assert!(w.action("sheet.removeDuplicates", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::RemoveDups);
    {
        let f = w.form.as_mut().expect("dups");
        assert!(f.on("header") && f.on("c0") && f.on("c1"));
        // Only Region: one North and one South stay.
        f.set_on("c1", false);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    assert_eq!(input(&w, 1, 0), "North");
    assert_eq!(input(&w, 2, 0), "South");
    assert_eq!(input(&w, 3, 0), "");
    assert!(w.status.contains('2'), "{}", w.status);
}

#[test]
fn advanced_filter_dialog_filters_in_place() {
    let mut w = window();
    region_list(&mut w);
    put(&mut w, &[(0, 4, "Region"), (1, 4, "South")]);
    w.view.select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.filterAdvanced", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::AdvancedFilter);
    {
        let f = w.form.as_mut().expect("advanced");
        assert_eq!(f.text("list"), "$A$1:$B$5");
        f.set_text("crit", "E1:E2");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    let info = w.view().engine().info().expect("info");
    assert_eq!(info.sheets[0].hidden_rows, vec![(1, 2)]);
}

#[test]
fn evaluate_formula_steps_like_the_reference() {
    let mut w = window();
    put(&mut w, &[(0, 0, "2"), (0, 1, "3"), (1, 0, "=A1+B1*2")]);
    w.view.select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.evaluateFormula", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Evaluate);
    let first = w.eval.as_ref().expect("state").pieces();
    assert!(!first.1.is_empty(), "an underlined part: {first:?}");
    let n = w.eval.as_ref().map_or(0, |e| e.steps.len());
    for _ in 0..n {
        w.form_event(
            form_impl::FormKind::Evaluate,
            &form_impl::Event::Clicked("evaluate"),
            &Fixed,
        );
    }
    // Every step done: the result, in the box as the last value.
    assert_eq!(w.eval.as_ref().expect("state").pieces().3, "8");
    w.form_event(
        form_impl::FormKind::Evaluate,
        &form_impl::Event::Clicked("cancel"),
        &Fixed,
    );
    assert!(w.form.is_none());
}

#[test]
fn sort_dialog_moves_levels_and_sorts_with_options() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Name"),
            (0, 1, "Score"),
            (1, 0, "B"),
            (1, 1, "2"),
            (2, 0, "b"),
            (2, 1, "2"),
            (3, 0, "a"),
            (3, 1, "1"),
        ],
    );
    w.view_mut().select((1, 0), (1, 0), (1, 0));
    assert!(w.action("sheet.sortDialog", &Fixed));
    // Level 1: Score; level 2: Name; then move Name up: Name first, Score second.
    w.form.as_mut().expect("sort").set_sel("col0", Some(2));
    w.form_event(
        form_impl::FormKind::Sort,
        &form_impl::Event::Clicked("add"),
        &Fixed,
    );
    w.form.as_mut().expect("sort").set_sel("col1", Some(1));
    assert_eq!(w.sort_sel, 1);
    w.form_event(
        form_impl::FormKind::Sort,
        &form_impl::Event::Clicked("up"),
        &Fixed,
    );
    {
        let f = w.form.as_ref().expect("sort");
        assert_eq!(
            (f.text("col0").as_str(), f.text("col1").as_str()),
            ("Name", "Score")
        );
    }
    assert_eq!(w.sort_sel, 0);
    // Options: case sensitive (lower case first, as the reference sorts).
    w.form_event(
        form_impl::FormKind::Sort,
        &form_impl::Event::Clicked("opts"),
        &Fixed,
    );
    assert_eq!(w.form_kind, form_impl::FormKind::SortOptions);
    w.form.as_mut().expect("options").set_on("case", true);
    w.form_event(
        form_impl::FormKind::SortOptions,
        &form_impl::Event::Ok,
        &Fixed,
    );
    assert_eq!(w.form_kind, form_impl::FormKind::Sort);
    assert!(w.sort_case);
    // Alt+D deletes the chosen level (Name): Score stays.
    w.handle(
        Input::Key(
            Key::Letter('d'),
            Mods {
                alt: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert_eq!(w.sort_levels, 1);
    assert_eq!(w.form.as_ref().expect("sort").text("col0"), "Score");
    w.form_event(
        form_impl::FormKind::Sort,
        &form_impl::Event::Clicked("add"),
        &Fixed,
    );
    w.form.as_mut().expect("sort").set_sel("col1", Some(1));
    w.form_event(form_impl::FormKind::Sort, &form_impl::Event::Ok, &Fixed);
    assert!(w.form.is_none());
    let names: Vec<String> = (1..4).map(|r| input(&w, r, 0)).collect();
    // Score, then Name case sensitive: lower case before upper case.
    assert_eq!(names, ["a", "b", "B"]);
}

#[test]
fn insert_function_recent_list_descriptions_and_search() {
    let mut w = window();
    assert!(w.action("sheet.insertFunction", &Fixed));
    {
        let f = w.form.as_ref().expect("insert function");
        // Most Recently Used, seeded as the reference.
        assert_eq!(f.sel("cat"), Some(0));
        assert_eq!(f.text("fns"), "SUM");
    }
    let help = |w: &SheetWindow| match w
        .form
        .as_ref()
        .and_then(|f| f.get("help"))
        .map(|c| c.kind.clone())
    {
        Some(form_impl::Kind::Note(s)) => s,
        _ => String::new(),
    };
    assert_eq!(help(&w), w.function_description("SUM"));
    assert_ne!(help(&w), String::new());
    // Go searches the descriptions too.
    if let Some(f) = w.form.as_mut() {
        f.set_text("search", "remainder");
    }
    w.form_event(
        form_impl::FormKind::Function,
        &form_impl::Event::Clicked("go"),
        &Fixed,
    );
    assert_eq!(w.form.as_ref().expect("form").text("fns"), "MOD");
    w.form_event(form_impl::FormKind::Function, &form_impl::Event::Ok, &Fixed);
    assert_eq!(
        w.prefs.recent_functions.first().map(String::as_str),
        Some("MOD")
    );
}

#[test]
fn subtotal_defaults_to_count_for_a_text_column() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Region"),
            (0, 1, "Note"),
            (1, 0, "N"),
            (1, 1, "x"),
            (2, 0, "S"),
            (2, 1, "y"),
        ],
    );
    w.view.select((1, 0), (1, 0), (1, 0));
    assert!(w.action("outline.subtotal", &Fixed));
    assert_eq!(w.form.as_ref().and_then(|f| f.sel("fn")), Some(1));
}

#[test]
fn data_validation_dialog_greys_data_applies_to_all_and_circles_invalid_cells() {
    let mut w = window();
    put(&mut w, &[(0, 0, "5"), (1, 0, "12"), (2, 0, "3")]);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("sheet.dataValidation", &Fixed));
    {
        // Any value: Data shown but greyed, Ignore blank greyed, Apply… greyed (no rule).
        let f = w.form.as_ref().expect("dialog");
        let op = f.get("op").expect("data box");
        assert!(op.visible && !op.enabled);
        assert!(!f.get("blank").expect("blank").enabled);
        assert!(!f.get("apply").expect("apply").enabled);
        assert_eq!(f.size, (604.0, 420.0));
    }
    // Whole number between 1 and 10 on A1:A3.
    w.form = None;
    w.view_mut().select((0, 0), (2, 0), (0, 0));
    assert!(w.action("sheet.dataValidation", &Fixed));
    if let Some(f) = w.form.as_mut() {
        f.set_sel("allow", Some(1));
    }
    w.validation_refresh();
    if let Some(f) = w.form.as_mut() {
        f.set_text("f1", "1");
        f.set_text("f2", "10");
        assert!(f.get("op").is_some_and(|c| c.enabled));
    }
    click_form(&mut w, "ok", 10.0, 10.0);
    assert!(w.form.is_none());
    // Circle Invalid Data finds A2 (12).
    assert!(w.action("sheet.circleInvalid", &Fixed));
    assert_eq!(w.invalid_circles, vec![(1, 0)]);
    assert!(w.action("sheet.clearCircles", &Fixed));
    assert_eq!(w.invalid_circles, Vec::new());
    // From A1 alone, Apply to all other cells changes A1:A3 to a maximum of 20.
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("sheet.dataValidation", &Fixed));
    if let Some(f) = w.form.as_mut() {
        assert_eq!(f.text("f2"), "10");
        assert!(f.get("apply").is_some_and(|c| c.enabled));
        f.set_text("f2", "20");
        f.set_on("apply", true);
    }
    click_form(&mut w, "ok", 10.0, 10.0);
    assert!(w.action("sheet.circleInvalid", &Fixed));
    assert!(
        w.invalid_circles.is_empty(),
        "12 is valid up to 20 on A2 too"
    );
}

#[test]
fn protect_sheet_dialog_permissions_password_and_unprotect() {
    let mut w = window();
    assert!(w.action("sheet.protectSheet", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Protect);
    {
        let f = w.form.as_ref().expect("dialog");
        assert!(f.on("p0") && f.on("p1") && !f.on("p2"));
        assert!(
            !f.get("p10").expect("sort").visible,
            "rows past 9 scrolled away"
        );
    }
    assert!(w.prot_wheel(120.0));
    assert!(w
        .form
        .as_ref()
        .is_some_and(|f| f.get("p9").is_some_and(|c| c.visible)));
    if let Some(f) = w.form.as_mut() {
        f.set_on("p10", true);
    }
    type_in(&mut w, "abc");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    // Confirm Password.
    assert!(w.form.as_ref().is_some_and(|f| f.text("pw").is_empty()));
    type_in(&mut w, "abc");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    let info = w
        .view
        .query(serde_json::json!({ "cmd": "protection", "sheet": 0 }))
        .expect("info");
    assert_eq!(info["sheet"]["protected"], true);
    assert_eq!(info["sheet"]["hasPassword"], true);
    assert_eq!(info["sheet"]["allow"]["sort"], true);
    // Unprotect asks for the password.
    assert!(w.action("sheet.protectSheet", &Fixed));
    type_in(&mut w, "wrong");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.status.contains("not correct"), "{}", w.status);
    assert!(w.action("sheet.protectSheet", &Fixed));
    type_in(&mut w, "abc");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let info = w
        .view
        .query(serde_json::json!({ "cmd": "protection", "sheet": 0 }))
        .expect("info");
    assert_eq!(info["sheet"]["protected"], false);
}

#[test]
fn review_tab_spelling_changes_words_and_greys_what_is_missing() {
    let mut w = window();
    put(&mut w, &[(0, 0, "Helo world"), (2, 1, "fine")]);
    w.view_mut().select((0, 0), (0, 0), (0, 0));
    assert!(w.action("sheet.spelling", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Spelling);
    assert_eq!(w.form.as_ref().map(|f| f.text("word")), Some("Helo".into()));
    if let Some(f) = w.form.as_mut() {
        f.set_text("word", "Hello");
    }
    click_form(&mut w, "change", 10.0, 10.0);
    assert!(w.form.is_none(), "nothing else to check");
    assert_eq!(input(&w, 0, 0), "Hello world");
    assert!(w.status.contains("complete"), "{}", w.status);
    // Greyed commands do nothing; statistics and accessibility open.
    assert!(!w.action("sheet.thesaurus", &Fixed));
    assert!(w.action("sheet.workbookStats", &Fixed));
    assert!(w.form.is_some());
    w.form = None;
    // Check Accessibility is a task pane (round 52), not a dialog.
    assert!(w.action("sheet.checkAccessibility", &Fixed));
    assert!(w.form.is_none() && w.acc.is_some());
}

#[test]
fn scripted_form_clicks_tick_wizard_delimiters() {
    let mut w = window();
    put(&mut w, &[(0, 0, "a,b"), (1, 0, "c,d")]);
    for id in [
        "goto.A1:A2",
        "sheet.textToColumns",
        "form.click.next",
        "form.click.dComma",
    ] {
        w.run_command(id, &Fixed);
    }
    let f = w.form.as_ref().expect("wizard");
    assert!(f.on("dComma"));
    assert!(w.ttc.as_ref().is_some_and(|t| t.delims[2]));
}

/// BLUESHEET_FILE=<xlsx> BLUESHEET_ACTIONS="a;b": run scripted actions and paint (debugging).
#[test]
fn scripted_actions_on_a_file_paint() {
    let Some(path) = std::env::var_os("BLUESHEET_FILE") else {
        return;
    };
    let bytes = std::fs::read(path).expect("file");
    let mut e = SheetEngine::with_bundled_fonts();
    e.open(&bytes, "t.xlsx").expect("open");
    let mut w = window_with(e);
    let acts = std::env::var("BLUESHEET_ACTIONS").unwrap_or_default();
    for id in acts.split(';').filter(|s| !s.is_empty()) {
        w.run_command(id, &Fixed);
    }
    let mut hash = PaintHash::default();
    w.paint_top(&mut hash);
    w.paint_overlay(&mut hash);
}

#[test]
fn review_comment_buttons_follow_the_sheets_comments() {
    let mut w = window();
    let f = w.format_state();
    assert!(!sheet_enabled("sheet.deleteComment", &f));
    assert!(!sheet_enabled("sheet.nextComment", &f));
    assert!(!sheet_enabled("sheet.showHideComment", &f));
    assert!(!sheet_enabled("sheet.unshareWorkbook", &f));
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    assert!(w.action("sheet.newComment", &Fixed));
    type_in(&mut w, "Look");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    let f = w.format_state();
    assert!(sheet_enabled("sheet.deleteComment", &f));
    assert!(sheet_enabled("sheet.nextComment", &f));
    w.view_mut().select((4, 4), (4, 4), (4, 4));
    let f = w.format_state();
    assert!(!sheet_enabled("sheet.deleteComment", &f));
    assert!(sheet_enabled("sheet.prevComment", &f));
    assert!(sheet_ribbon::is_menu("sheet.checkAccessibility"));
    assert!(sheet_ribbon::is_menu("sheet.hideInk"));
}

#[test]
fn hyperlink_browser_lists_the_folder_and_creates_a_new_document() {
    let dir = std::env::temp_dir().join(format!("bo-link-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).expect("dir");
    std::fs::write(dir.join("report.xlsx"), b"x").expect("file");
    let mut w = window();
    w.set_folder(&dir);
    assert!(w.action("sheet.link", &Fixed));
    // Folders first, then files.
    assert_eq!(
        w.link_entries,
        vec![("sub".to_owned(), true), ("report.xlsx".to_owned(), false)]
    );
    w.form.as_mut().expect("link").set_sel("used", Some(1));
    w.link_event(&form_impl::Event::Changed("used"));
    assert_eq!(
        w.form.as_ref().map(|f| f.text("addr")),
        Some("report.xlsx".into())
    );
    // Bookmark… adds a place in that workbook.
    w.link_event(&form_impl::Event::Clicked("bookmark"));
    w.form.as_mut().expect("bm").set_text("bmCell", "B2");
    w.link_event(&form_impl::Event::Ok);
    assert_eq!(
        w.form.as_ref().map(|f| f.text("addr")),
        Some("report.xlsx#Sheet1!B2".into())
    );
    w.link_event(&form_impl::Event::Ok);
    let link = w
        .view
        .query(json!({ "cmd": "hyperlink", "sheet": 0, "row": 0, "col": 0 }))
        .expect("link");
    assert_eq!(link["target"], "report.xlsx");
    assert_eq!(link["location"], "Sheet1!B2");
    // Create New Document writes an empty workbook beside and links it.
    w.view_mut().select((2, 0), (2, 0), (2, 0));
    assert!(w.action("sheet.link", &Fixed));
    w.link_event(&form_impl::Event::Canvas("bar", 10.0, 2.5 * 78.0));
    assert_eq!(w.form.as_ref().map(|f| f.tab), Some(2));
    w.form.as_mut().expect("new").set_text("newName", "Budget");
    w.link_event(&form_impl::Event::Ok);
    let made = dir.join("Budget.xlsx");
    assert!(made.exists());
    let link = w
        .view
        .query(json!({ "cmd": "hyperlink", "sheet": 0, "row": 2, "col": 0 }))
        .expect("link");
    assert_eq!(link["target"], made.display().to_string());
    let _ = std::fs::remove_dir_all(&dir);
}

#[path = "sheet_r11_tests.rs"]
mod r11;

#[path = "sheet_r12_tests.rs"]
mod r12;

#[path = "sheet_r13_tests.rs"]
mod r13;

#[path = "sheet_r13b_tests.rs"]
mod r13b;
