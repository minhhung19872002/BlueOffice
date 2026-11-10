//! Headless BlueScript runtime (CORE-SCRIPT-004): the object API on the native engine
//! for the three applications, and the sandbox limits.

use bo_script::{run_script, App, Bridge, Document, ErrorKind, Limits, RunOptions, RunReport};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// Runs `f` on a thread with a roomy stack (unoptimised engine builds need it on Windows).
fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .expect("thread")
        .join()
        .expect("join")
}

fn corpus(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel)
}

fn blank_docx() -> Vec<u8> {
    let mut e = bo_engine::Engine::with_bundled_fonts();
    e.new_document("script.docx").expect("new document");
    e.save_docx().expect("save")
}

fn blank_xlsx() -> Vec<u8> {
    let mut e = bo_engine::SheetEngine::with_bundled_fonts();
    e.new_workbook("script.xlsx").expect("new workbook");
    e.save("xlsx", 0).expect("save")
}

fn open(bytes: &[u8], name: &str) -> Rc<RefCell<Document>> {
    Rc::new(RefCell::new(
        Document::open(bytes, name, "vi-VN").expect("open"),
    ))
}

fn run(doc: &Rc<RefCell<Document>>, code: &str, limits: Limits) -> RunReport {
    let app = doc.borrow().app();
    let mut opts = RunOptions::new(app);
    opts.limits = limits;
    let bridge: Rc<RefCell<dyn Bridge>> = doc.clone();
    run_script(code, bridge, &opts)
}

fn ok(doc: &Rc<RefCell<Document>>, code: &str) -> RunReport {
    let r = run(doc, code, Limits::default());
    assert!(r.ok, "script failed: {:?}", r.error);
    r
}

#[test]
fn doc_api_writes_formats_finds_and_replaces() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = ok(
            &doc,
            r"
            const first = await doc.appendParagraph('Báo cáo quý', 'Heading1');
            await doc.appendParagraph('Doanh thu tăng 10%. Doanh thu ổn định.');
            const paras = await doc.paragraphs;
            await paras[1].format({ bold: true, size: 14 });
            const style = await paras[0].getStyle();
            const hits = await doc.find('Doanh thu');
            const n = await doc.replace('Doanh thu', 'Lợi nhuận');
            console.log('đã thay', n);
            return { first, count: paras.length, style, hits, n, text: await doc.text };
            ",
        );
        let v = r.value.expect("value");
        assert_eq!(v["first"], 0);
        assert_eq!(v["count"], 2);
        assert_eq!(v["style"], "Heading1");
        // Offsets are UTF-16 indexes, as in the browser.
        assert_eq!(
            v["hits"],
            serde_json::json!([
                { "paragraph": 1, "start": 0, "endParagraph": 1, "end": 9 },
                { "paragraph": 1, "start": 20, "endParagraph": 1, "end": 29 },
            ])
        );
        assert_eq!(v["n"], 2);
        assert_eq!(
            v["text"],
            "Báo cáo quý\nLợi nhuận tăng 10%. Lợi nhuận ổn định."
        );
        assert_eq!(r.logs.len(), 1);
        assert_eq!(r.logs[0].text, "đã thay 2");
        assert!(r.calls >= 7, "calls {}", r.calls);

        // The change is in the saved package.
        let bytes = doc.borrow_mut().save("docx").expect("save");
        let again = open(&bytes, "b.docx");
        let r = ok(&again, "return await doc.text;");
        assert_eq!(
            r.value.expect("text"),
            "Báo cáo quý\nLợi nhuận tăng 10%. Lợi nhuận ổn định."
        );
    });
}

#[test]
fn api_errors_reach_the_script() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = ok(
            &doc,
            r"
            const out = [];
            try { await doc.select(99); } catch (e) { out.push(e.message); }
            try { await doc.format({ bold: 'yes' }); } catch (e) { out.push(e.message); }
            try { await doc.command({ cmd: 'noSuchCommand' }); } catch (e) { out.push('engine'); }
            return out;
            ",
        );
        assert_eq!(
            r.value.expect("value"),
            serde_json::json!([
                "no paragraph 99",
                "format.bold: expected a boolean",
                "engine"
            ])
        );
    });
}

#[test]
fn sheet_api_values_formulas_and_sheets() {
    big_stack(|| {
        let doc = open(&blank_xlsx(), "a.xlsx");
        let r = ok(
            &doc,
            r"
            const r = sheet.range('A1:B2');
            await r.setValues([[1, 2.5], ['Tổng', null]]);
            await sheet.range('C1').setFormulas([['=A1+B1']]);
            await sheet.range('A1').format({ bold: true });
            const s2 = await workbook.addSheet('Dữ liệu');
            await s2.range('A1').setValue('x');
            const names = (await workbook.sheets).map((s) => s.name);
            return { values: await sheet.range('A1:C2').values, f: await sheet.range('C1').formulas, names };
            ",
        );
        let v = r.value.expect("value");
        assert_eq!(
            v["values"],
            serde_json::json!([[1, 2.5, 3.5], ["Tổng", null, null]])
        );
        assert_eq!(v["f"], serde_json::json!([["=A1+B1"]]));
        assert_eq!(v["names"][1], "Dữ liệu");
        let bytes = doc.borrow_mut().save("xlsx").expect("save");
        assert!(bytes.starts_with(b"PK"));
    });
}

#[test]
fn slide_api_shapes_text_and_notes() {
    big_stack(|| {
        let bytes = std::fs::read(corpus("slide/slide-edit-arrange.pptx")).expect("corpus deck");
        let doc = open(&bytes, "deck.pptx");
        assert_eq!(doc.borrow().app(), App::Slide);
        let r = ok(
            &doc,
            r"
            const info = await presentation.info;
            const s = presentation.slide(0);
            const box = await s.addTextBox('Xin chào', { x: 50, y: 60, w: 200, h: 40 });
            await s.setNotes('Ghi chú');
            const shapes = await s.shapes;
            return { count: info.slideCount, box: box.text, notes: await s.notes,
                     has: shapes.some((x) => x.id === box.id) };
            ",
        );
        let v = r.value.expect("value");
        assert!(v["count"].as_u64().unwrap_or(0) >= 1);
        assert_eq!(v["box"], "Xin chào");
        assert_eq!(v["notes"], "Ghi chú");
        assert_eq!(v["has"], true);
        let out = doc.borrow_mut().save("pptx").expect("save");
        assert!(out.starts_with(b"PK"));
    });
}

#[test]
fn infinite_loop_times_out() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let limits = Limits {
            timeout: Duration::from_millis(300),
            ..Limits::default()
        };
        let r = run(&doc, "while (true) {}", limits.clone());
        assert!(!r.ok);
        assert_eq!(r.error.expect("error").kind, ErrorKind::Timeout);
        // Also when the loop awaits API calls in between.
        let r = run(&doc, "for (;;) { await doc.info; }", limits);
        assert_eq!(r.error.expect("error").kind, ErrorKind::Timeout);
    });
}

#[test]
fn big_allocation_hits_the_memory_limit() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let limits = Limits {
            memory_bytes: 8 << 20,
            ..Limits::default()
        };
        let r = run(
            &doc,
            "const a = []; for (;;) a.push(new Array(100000).fill(1));",
            limits.clone(),
        );
        assert!(!r.ok);
        assert_eq!(r.error.expect("error").kind, ErrorKind::Memory);
        let r = run(&doc, "return 'x'.repeat(64 * 1024 * 1024).length;", limits);
        assert_eq!(r.error.expect("error").kind, ErrorKind::Memory);
    });
}

#[test]
fn deep_recursion_hits_the_stack_limit() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = run(
            &doc,
            "const f = (n) => f(n + 1) + 1; f(0);",
            Limits::default(),
        );
        let e = r.error.expect("error");
        assert_eq!(e.kind, ErrorKind::Error);
        assert!(
            e.message.contains("Maximum call stack size exceeded"),
            "{}",
            e.message
        );
    });
}

#[test]
fn no_module_loader_network_filesystem_or_timers() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = ok(
            &doc,
            r"
            const names = ['require', 'fetch', 'process', 'std', 'os', 'setTimeout', 'setInterval',
              'XMLHttpRequest', 'WebSocket', 'importScripts', 'print', 'scriptArgs', 'Deno', 'Bun',
              '__bo_call', '__bo_log', '__bo_app', '__bo_headless'];
            const present = names.filter((n) => typeof globalThis[n] !== 'undefined');
            let imported = 'no';
            try { await import('fs'); imported = 'yes'; } catch (e) { imported = 'refused'; }
            return { present, imported, own: Object.getOwnPropertyNames(globalThis).filter((n) => n.startsWith('__')) };
            ",
        );
        let v = r.value.expect("value");
        assert_eq!(v["present"], serde_json::json!([]));
        assert_eq!(v["imported"], "refused");
        assert_eq!(v["own"], serde_json::json!([]));
    });
}

#[test]
fn errors_carry_the_script_line_and_column() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = run(&doc, "const a = 1;\nconst b = ;\n", Limits::default());
        let e = r.error.expect("error");
        assert_eq!(e.kind, ErrorKind::Syntax);
        assert_eq!(e.line, Some(2), "{e:?}");

        let r = run(
            &doc,
            "const a = 1;\n\n  throw new TypeError('hỏng');\n",
            Limits::default(),
        );
        let e = r.error.expect("error");
        assert_eq!(e.kind, ErrorKind::Error);
        assert_eq!(e.message, "TypeError: hỏng");
        assert_eq!(e.line, Some(3), "{e:?}");
        // QuickJS points at the constructor of the thrown error: `TypeError` is column 13.
        assert_eq!(e.column, Some(13), "{e:?}");
    });
}

#[test]
fn call_and_output_limits() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = run(
            &doc,
            "for (let i = 0; i < 100; i++) await doc.info;",
            Limits {
                max_calls: 10,
                ..Limits::default()
            },
        );
        assert_eq!(r.error.expect("error").kind, ErrorKind::Limit);
        assert_eq!(r.calls, 11);

        let r = run(
            &doc,
            "for (let i = 0; i < 50; i++) console.log('dòng ' + i); return 1;",
            Limits {
                max_log_lines: 5,
                ..Limits::default()
            },
        );
        assert!(r.ok);
        assert_eq!(r.logs.len(), 5);
        assert_eq!(r.logs[4].text, "…");
        assert!(r.logs_truncated);

        let r = run(
            &doc,
            "return 'x'.repeat(5000);",
            Limits {
                max_output_bytes: 1000,
                ..Limits::default()
            },
        );
        assert_eq!(r.error.expect("error").kind, ErrorKind::Limit);
    });
}

#[test]
fn never_settling_promise_is_an_error_not_a_hang() {
    big_stack(|| {
        let doc = open(&blank_docx(), "a.docx");
        let r = run(&doc, "await new Promise(() => {});", Limits::default());
        assert_eq!(r.error.expect("error").kind, ErrorKind::Error);
    });
}
