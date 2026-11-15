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
    assert_eq!((store.fulls, store.moved, store.turns), (1, 0, 17));
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
    // Step 1: delimiters field (second), type a semicolon list.
    w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
    for _ in 0..12 {
        w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    }
    type_in(&mut w, "semicolon");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(matches!(
        w.dialog.as_ref().map(|d| d.0),
        Some(commands_impl::SheetDialog::TextToColumns(1))
    ));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.dialog.is_none());
    assert_eq!(
        (
            input(&w, 0, 0),
            input(&w, 0, 1),
            input(&w, 0, 2),
            input(&w, 1, 1)
        ),
        ("a".into(), "b".into(), "c".into(), "e".into())
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
        assert_eq!(f.text("col0"), "Name");
        f.set_sel("col0", Some(1));
        f.set_sel("ord0", Some(1));
    }
    click_form(&mut w, "add", 10.0, 10.0);
    assert!(w
        .form
        .as_ref()
        .and_then(|f| f.get("col1"))
        .is_some_and(|c| c.visible));
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
    // Name Manager: define Rate = 0,1.
    assert!(w.action("sheet.nameManager", &Fixed));
    {
        let f = w.form.as_mut().expect("names form");
        f.set_text("name", "Rate");
        f.set_text("ref", "=0,1");
    }
    click_form(&mut w, "new", 10.0, 10.0);
    assert!(w.name_rows.iter().any(|(n, _)| n == "Rate"));
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
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.view().editing());
    assert_eq!(w.view().formula_text(), "=IFERROR(");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
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
