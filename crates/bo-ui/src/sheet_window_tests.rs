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
