//! Round 11 tests of the BlueSheet window: Text to Columns replace question and preview
//! scroll thumbs, Check Accessibility pane, Formulas tab dialogs.
use super::*;

fn press(w: &mut SheetWindow, x: f32, y: f32) {
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

/// Window point of (x, y) reference pixels inside control `id` of the open form.
fn form_point(w: &SheetWindow, id: &str, x: f32, y: f32) -> (f32, f32) {
    let f = w.form.as_ref().expect("form");
    let r = f.get(id).expect("control").rect;
    let d = f.dev([r[0] + x, r[1] + y, 0.0, 0.0]);
    (d[0], d[1])
}

#[test]
fn text_to_columns_asks_before_replacing_cells_and_scroll_thumbs_drag() {
    let mut w = window();
    let mut cells: Vec<(u32, u32, String)> = (0..30)
        .map(|r| (r, 0, format!("{r};{}", "x".repeat(150))))
        .collect();
    cells.push((0, 1, "keep".into()));
    w.view_mut()
        .engine_mut()
        .set_inputs(0, &cells)
        .expect("cells");
    w.view_mut().changed();
    w.view_mut().select((0, 0), (29, 0), (0, 0));
    assert!(w.action("sheet.textToColumns", &Fixed));
    // Step 1 preview: the vertical thumb drags to the end, the track pages back.
    let (h, v) = w.ttc.as_ref().expect("wizard").bars();
    let v = v.expect("rows overflow");
    assert!(h.max > 0, "long lines scroll sideways");
    let (x, y) = form_point(&w, "preview", 697.0, v.thumb + 2.0);
    press(&mut w, x, y);
    let (x2, y2) = form_point(&w, "preview", 697.0, v.track + v.len);
    w.handle(Input::Drag { x: x2, y: y2 }, &Fixed);
    w.handle(Input::Release { x: x2, y: y2 }, &Fixed);
    assert_eq!(w.ttc.as_ref().map(|t| t.top), Some(v.max));
    let (x, y) = form_point(&w, "preview", 697.0, v.track + 1.0);
    press(&mut w, x, y);
    w.handle(Input::Release { x, y }, &Fixed);
    assert!(w.ttc.as_ref().is_some_and(|t| t.top < v.max));
    // Horizontal: a click on the track right of the thumb pages one box width.
    let (x, y) = form_point(&w, "preview", h.thumb + h.size + 20.0, 147.0);
    press(&mut w, x, y);
    w.handle(Input::Release { x, y }, &Fixed);
    assert_eq!(w.ttc.as_ref().map(|t| t.left), Some(h.max));
    click_form(&mut w, "next", 10.0, 10.0);
    click_form(&mut w, "dSemi", 5.0, 10.0);
    click_form(&mut w, "next", 10.0, 10.0);
    // B1 holds data: Finish asks first; Cancel goes back to the wizard untouched.
    click_form(&mut w, "finish", 10.0, 10.0);
    assert_eq!(w.form_kind, form_impl::FormKind::TtcConfirm);
    click_form(&mut w, "cancelb", 10.0, 10.0);
    assert_eq!(w.form_kind, form_impl::FormKind::TextToColumns);
    assert_eq!(input(&w, 0, 1), "keep");
    click_form(&mut w, "finish", 10.0, 10.0);
    click_form(&mut w, "okb", 10.0, 10.0);
    assert!(w.form.is_none());
    assert_eq!(
        (input(&w, 0, 0), input(&w, 0, 1)),
        ("0".into(), "x".repeat(150))
    );
}

fn type_into(w: &mut SheetWindow, s: &str) {
    w.handle(Input::Text(s.into()), &Fixed);
}

#[test]
fn function_arguments_show_live_values_grow_and_write_the_formula() {
    let mut w = window();
    put(
        &mut w,
        &[(0, 0, "4"), (1, 0, "24"), (2, 0, "25"), (0, 1, "1")],
    );
    w.view_mut().select((5, 0), (5, 0), (5, 0));
    assert!(w.action("sheet.insertFunction", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::Function);
    // Choose SUM in All and press OK: Function Arguments opens on number1.
    let all = 1;
    w.form.as_mut().expect("form").set_sel("cat", Some(all));
    w.form_event(
        form_impl::FormKind::Function,
        &form_impl::Event::Changed("cat"),
        &Fixed,
    );
    let items = w.form.as_ref().map(|f| f.text("fns")).unwrap_or_default();
    assert_ne!(items, "");
    let f = w.form.as_mut().expect("form");
    if let Some(form_impl::Kind::List { items, sel, .. }) = f.get_mut("fns").map(|c| &mut c.kind) {
        *sel = items.iter().position(|n| n == "SUM");
    }
    w.form_event(form_impl::FormKind::Function, &form_impl::Event::Ok, &Fixed);
    assert_eq!(w.form_kind, form_impl::FormKind::FnArgs);
    let st = w.fnargs.as_ref().expect("args");
    assert_eq!((st.name.as_str(), st.args.len()), ("SUM", 2));
    // Typing a range shows its values and the result; a third box appears.
    w.form.as_mut().expect("form").focus("a0");
    type_into(&mut w, "A1:A3");
    let st = w.fnargs.as_ref().expect("args");
    assert_eq!(st.values[0].as_deref(), Some("{4;24;25}"));
    assert_eq!(st.result.as_deref(), Some("53"));
    w.form.as_mut().expect("form").focus("a1");
    type_into(&mut w, "B1");
    assert_eq!(w.fnargs.as_ref().map(|s| s.args.len()), Some(3));
    assert_eq!(
        w.fnargs.as_ref().and_then(|s| s.result.clone()).as_deref(),
        Some("54")
    );
    w.form_event(form_impl::FormKind::FnArgs, &form_impl::Event::Ok, &Fixed);
    assert!(w.form.is_none());
    assert_eq!(input(&w, 5, 0), "=SUM(A1:A3,B1)");
    // Insert Function on that cell opens its arguments directly.
    assert!(w.action("sheet.insertFunction", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::FnArgs);
    assert_eq!(
        w.fnargs.as_ref().map(|s| s.args[..2].to_vec()),
        Some(vec!["A1:A3".to_owned(), "B1".to_owned()])
    );
    // The first box opens with its text selected: typing replaces it; the help is our own
    // text for this function's argument.
    assert!(w.form.as_ref().is_some_and(form_impl::Form::all_selected));
    type_into(&mut w, "A2");
    assert_eq!(
        w.fnargs.as_ref().map(|s| s.args[0].clone()),
        Some("A2".into())
    );
    let help = w
        .fnargs
        .as_ref()
        .map(|s| w.fnargs_help(s))
        .unwrap_or_default();
    assert!(help.contains("added up"), "{help}");
}

#[test]
fn error_checking_dialog_steps_ignores_and_edits() {
    let mut w = window();
    put(&mut w, &[(0, 0, "1"), (1, 0, "=A1/0"), (2, 0, "=NA()")]);
    assert!(w.action("sheet.errorCheck", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::ErrorCheck);
    assert_eq!(w.view.active(), (1, 0));
    let group = |w: &SheetWindow, id: &str| match w
        .form
        .as_ref()
        .and_then(|f| f.get(id))
        .map(|c| &c.kind)
    {
        Some(form_impl::Kind::Group(s)) => s.clone(),
        _ => String::new(),
    };
    assert_eq!(group(&w, "cellG"), "Error in cell A2");
    assert_eq!(group(&w, "ruleG"), "Divide by Zero Error");
    click_form(&mut w, "next", 10.0, 10.0);
    assert_eq!(w.view.active(), (2, 0));
    assert_eq!(group(&w, "ruleG"), "Value Not Available Error");
    // Ignore Error records it; the dialog keeps the remaining finding.
    click_form(&mut w, "ignore", 10.0, 10.0);
    let left = w.errc.as_ref().map(|e| e.list.len());
    assert_eq!(left, Some(1));
    click_form(&mut w, "edit", 10.0, 10.0);
    assert!(w.form.is_none() && w.view.editing());
}

#[test]
fn accessibility_pane_docks_lists_issues_and_fixes_them() {
    let mut w = window();
    put(&mut w, &[(0, 0, "Region"), (1, 0, "North")]);
    let grid_w = w.view.grid_frame()[2];
    assert!(w.action("sheet.checkAccessibility", &Fixed));
    w.handle(Input::Blink, &Fixed);
    assert!(w.acc.is_some());
    // The grid gives the pane its width.
    assert!(w.view.grid_frame()[2] < grid_w - 300.0);
    let p = w.acc.as_ref().expect("pane");
    // Default sheet name ("Sheet1" with data) is the only issue.
    assert_eq!(p.issues[6].len(), 1);
    assert_eq!(p.issues.iter().map(Vec::len).sum::<usize>(), 1);
    assert!(w.acc_status_text().contains("Investigate"));
    // Details, then Rename Sheet starts renaming the tab.
    w.pane_detail(6);
    assert_eq!(w.acc.as_ref().and_then(|p| p.detail), Some((6, 0)));
    let r = w.pane_rect();
    let k = w.scale / 1.25;
    press(&mut w, r[0] + 100.0 * k, r[1] + 360.0 * k);
    assert!(w.tab_edit.is_some());
    // The close button closes the pane and gives the width back.
    press(&mut w, r[0] + 370.0 * k, r[1] + 28.0 * k);
    assert!(w.acc.is_none());
    assert!((w.view.grid_frame()[2] - grid_w).abs() < 1.0);
}

#[test]
fn watch_window_adds_and_deletes_watches() {
    let mut w = window();
    put(&mut w, &[(0, 0, "2"), (0, 1, "=A1*3")]);
    assert!(w.action("sheet.watchWindow", &Fixed));
    assert!(w.watch.as_ref().is_some_and(|x| x.rows.is_empty()));
    w.view_mut().select((0, 1), (0, 1), (0, 1));
    let r = w.watch_rect().expect("window");
    let k = w.scale / 1.25;
    press(&mut w, r[0] + 40.0 * k, r[1] + 66.0 * k);
    let rows = w.watch.as_ref().map(|x| x.rows.clone()).unwrap_or_default();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (
            rows[0][3].as_str(),
            rows[0][4].as_str(),
            rows[0][5].as_str()
        ),
        ("B1", "6", "=A1*3")
    );
    // Values follow edits.
    put(&mut w, &[(0, 0, "5")]);
    w.handle(Input::Blink, &Fixed);
    assert_eq!(
        w.watch.as_ref().map(|x| x.rows[0][4].clone()),
        Some("15".into())
    );
    // Select the row, Delete Watch.
    press(&mut w, r[0] + 40.0 * k, r[1] + 112.0 * k);
    press(&mut w, r[0] + 170.0 * k, r[1] + 66.0 * k);
    assert!(w.watch.as_ref().is_some_and(|x| x.rows.is_empty()));
}

#[test]
fn find_and_replace_options_find_all_and_replace() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "apple"),
            (0, 1, "Apple pie"),
            (1, 0, "pear"),
            (2, 1, "apple"),
        ],
    );
    assert!(w.action("find.find", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::FindReplace);
    assert_eq!(w.form.as_ref().map(|f| f.size), Some((683.0, 258.0)));
    type_into(&mut w, "apple");
    // By rows: B1 after A1, then B3; Options >> then By Columns: A1, B1, B3 → from A1 goes
    // to B1 still; Match case leaves out "Apple pie".
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.view().active(), (0, 1));
    click_form(&mut w, "options", 10.0, 10.0);
    assert_eq!(w.form.as_ref().map(|f| f.size), Some((683.0, 320.0)));
    assert_eq!(
        w.form.as_ref().map(|f| f.text("what")),
        Some("apple".into())
    );
    click_form(&mut w, "matchCase", 5.0, 10.0);
    click_form(&mut w, "findAll", 10.0, 10.0);
    assert_eq!(w.form.as_ref().map(|f| f.size), Some((683.0, 445.0)));
    assert_eq!(w.find_state.found.clone(), Some(vec![(0, 0, 0), (0, 2, 1)]));
    assert_eq!(w.view().active(), (0, 0));
    // A click on the second row goes to B3.
    click_form(&mut w, "results", 30.0, 24.0 + 21.0 + 5.0);
    assert_eq!(w.view().active(), (2, 1));
    // Replace tab: Replace changes the active match, Replace All the rest.
    w.form.as_mut().expect("form").set_tab(1);
    w.form.as_mut().expect("form").focus("with");
    type_into(&mut w, "plum");
    click_form(&mut w, "replaceOne", 10.0, 10.0);
    assert_eq!(input(&w, 2, 1), "plum");
    click_form(&mut w, "replaceAll", 10.0, 10.0);
    assert_eq!(
        (input(&w, 0, 0), input(&w, 0, 1)),
        ("plum".into(), "Apple pie".into())
    );
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    // The settings stay for the next time.
    assert!(w.action("find.replace", &Fixed));
    assert!(w
        .form
        .as_ref()
        .is_some_and(|f| f.tab == 1 && f.on("matchCase")));
}

#[test]
fn new_pivot_table_opens_its_fields_pane_and_ticks_build_the_report() {
    let mut w = window();
    put(
        &mut w,
        &[
            (0, 0, "Region"),
            (0, 1, "Product"),
            (0, 2, "Sales"),
            (1, 0, "North"),
            (1, 1, "Rice"),
            (1, 2, "10"),
            (2, 0, "South"),
            (2, 1, "Rice"),
            (2, 2, "5"),
            (3, 0, "North"),
            (3, 1, "Tea"),
            (3, 2, "7"),
        ],
    );
    w.view_mut().select((1, 1), (1, 1), (1, 1));
    assert!(w.action("sheet.pivotTable", &Fixed));
    assert_eq!(w.form_kind, form_impl::FormKind::PivotNew);
    assert_eq!(
        w.form.as_ref().map(|f| f.text("table")),
        Some("Sheet1!$A$1:$C$4".into())
    );
    assert!(w.form.as_ref().is_some_and(|f| !f.on("model")));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.form.is_none());
    // A new sheet, in front of the source sheet (round 58).
    assert_eq!(w.view().sheet(), 0, "a new sheet");
    assert_eq!(w.view().sheet_names().len(), 2);
    let pane = w.pivot.clone().expect("fields pane");
    assert_eq!(
        pane.fields.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
        vec!["Region", "Product", "Sales"]
    );
    assert!(w.view.grid_frame()[2] < 1200.0 - 400.0);
    // Region → Rows, Sales (numbers) → Values as a Sum.
    w.pivot_toggle(0);
    w.pivot_toggle(2);
    let pane = w.pivot.clone().expect("pane");
    assert_eq!((pane.rows.clone(), pane.values.len()), (vec![0], 1));
    let shown: Vec<String> = (0..6)
        .flat_map(|r| (0..3).map(move |c| (r, c)))
        .map(|(r, c)| {
            w.view()
                .engine()
                .cell(0, r, c)
                .map(|d| d.display)
                .unwrap_or_default()
        })
        .collect();
    assert!(
        shown.contains(&"North".to_owned()) && shown.contains(&"17".to_owned()),
        "{shown:?}"
    );
    // Clearing Region takes it out again.
    w.pivot_toggle(0);
    assert!(w.pivot.as_ref().is_some_and(|p| p.rows.is_empty()));
}

/// Where typing in a cell spends its time on a big sheet (`cargo test --release -- --ignored
/// typing_phases --nocapture`).
#[test]
#[ignore = "timing probe"]
fn typing_phases_on_a_big_sheet() {
    let mut w = window();
    let cells: Vec<(u32, u32, String)> = (0..100_000u32)
        .flat_map(|r| (0..6u32).map(move |c| (r, c, format!("{}", r * 7 + c))))
        .chain((0..10_000u32).flat_map(|r| {
            [
                (r, 6, format!("=SUM(A{0}:F{0})", r + 1)),
                (
                    r,
                    7,
                    if r == 0 {
                        "=G1".to_owned()
                    } else {
                        format!("=H{}+G{}", r, r + 1)
                    },
                ),
                (r, 9, format!("=VLOOKUP({r},$A$1:$B$1000,2,FALSE)")),
            ]
        }))
        .collect();
    w.view_mut()
        .engine_mut()
        .set_inputs(0, &cells)
        .expect("cells");
    w.view_mut().changed();
    w.handle(Input::Blink, &Fixed);
    w.view_mut().select((5, 8), (5, 8), (5, 8));
    w.handle(Input::Text("a".into()), &Fixed);
    let t = |name: &str, f: &mut dyn FnMut()| {
        let s = std::time::Instant::now();
        for _ in 0..20 {
            f();
        }
        eprintln!("{name}: {:.3} ms", s.elapsed().as_secs_f64() * 1e3 / 20.0);
    };
    t("handle(Text)", &mut || {
        w.handle(Input::Text("b".into()), &Fixed);
    });
    t("view.handle", &mut || {
        w.view.handle(Input::Text("b".into()), &Fixed);
    });
    t("status_stat_parts", &mut || {
        let _ = w.status_stat_parts();
    });
    t("acc_issue_count", &mut || {
        let _ = w.acc_issue_count();
    });
    t("sync_pivot_pane", &mut || w.sync_pivot_pane());
    t("update_bars", &mut || w.update_bars());
    t("measure_labels", &mut || w.view.measure_labels(&Fixed));
    t("format_state", &mut || {
        let _ = w.format_state();
    });
    t("refresh_calc_state", &mut || w.refresh_calc_state());
    t("measure_name", &mut || w.measure_name(&Fixed));
    t("layout_cards", &mut || w.layout_cards(&Fixed));
    let one = |name: &str, f: &mut dyn FnMut()| {
        let s = std::time::Instant::now();
        f();
        eprintln!("{name}: {:.3} ms", s.elapsed().as_secs_f64() * 1e3);
    };
    one("commit (Enter)", &mut || {
        w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    });
    one("engine set_input", &mut || {
        w.view.engine_mut().set_input(0, 7, 8, "5").expect("input");
    });
    one("content_size", &mut || {
        let _ = w.view.engine_mut().content_size(0);
    });
    one("view.changed + bars", &mut || {
        w.view.changed();
        w.update_bars();
    });
    one("acc_issue_count", &mut || {
        let _ = w.acc_issue_count();
    });
    one("refresh_calc_state", &mut || w.refresh_calc_state());
    let mut store = super::Raster::default();
    one("prepare (grid image)", &mut || {
        w.prepare(&mut store).expect("grid");
    });
}
