//! Chart corpus of BlueDoc (`tests/corpus/doc/doc-chart-{1,2,3}.docx`, CORE-CHART-007 /
//! CORE-CHART-004): documents built with the editor's own chart commands (`insertChart`,
//! `setChart`), so the files are what BlueDoc writes. All text and numbers are invented.

use bo_engine::Engine;
use serde_json::{json, Value};

fn run(e: &mut Engine, v: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    let r: Value = serde_json::from_str(&e.command_json(&v.to_string())?)?;
    if let Some(err) = r.get("error").and_then(Value::as_str) {
        return Err(format!("{v}: {err}").into());
    }
    Ok(r)
}

/// Put the caret at the start of paragraph `p`.
fn caret(e: &mut Engine, p: usize) -> Result<(), Box<dyn std::error::Error>> {
    run(
        e,
        &json!({"cmd": "setSelection", "selection": {"anchor": {"p": p, "o": 0}, "focus": {"p": p, "o": 0}}}),
    )?;
    Ok(())
}

fn grid(categories: &[&str], series: &[(&str, &[f64])]) -> Value {
    json!({
        "categories": categories,
        "series": series.iter().map(|(n, v)| json!({"name": n, "values": v})).collect::<Vec<_>>(),
    })
}

fn document(text: &str) -> Result<Engine, Box<dyn std::error::Error>> {
    let mut e = Engine::with_bundled_fonts();
    e.open(text.as_bytes(), "new.txt")?;
    Ok(e)
}

/// A generated file: name, description, bytes.
type File = (&'static str, &'static str, Vec<u8>);

/// The three chart documents.
pub fn documents() -> Result<Vec<File>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();

    // 1. A column chart in line with the text, its data edited, then a line chart.
    let mut e = document(
        "Doanh thu theo quý\n\nBiểu đồ trên tổng hợp doanh thu hai chi nhánh.\n\nXu hướng cả năm.",
    )?;
    caret(&mut e, 1)?;
    run(
        &mut e,
        &json!({"cmd": "insertChart", "kind": "column", "title": "Doanh thu (tỷ đồng)",
            "data": grid(&["Quý 1", "Quý 2", "Quý 3", "Quý 4"],
                &[("Hà Nội", &[12.5, 14.0, 13.2, 16.8]), ("Đà Nẵng", &[8.1, 9.4, 10.0, 11.3])])}),
    )?;
    run(
        &mut e,
        &json!({"cmd": "setChart", "data": grid(&["Quý 1", "Quý 2", "Quý 3", "Quý 4"],
            &[("Hà Nội", &[12.5, 15.5, 13.2, 16.8]), ("Đà Nẵng", &[8.1, 9.4, 10.0, 12.6])])}),
    )?;
    caret(&mut e, 3)?;
    run(
        &mut e,
        &json!({"cmd": "insertChart", "kind": "line", "title": "Tăng trưởng", "legend": "r",
            "data": grid(&["T1", "T2", "T3", "T4", "T5", "T6"],
                &[("Chỉ số", &[100.0, 104.0, 103.0, 108.0, 112.0, 115.0])])}),
    )?;
    run(
        &mut e,
        &json!({"cmd": "setChart", "style": "soft", "colors": "colorfulAlt"}),
    )?;
    out.push((
        "doc-chart-1.docx",
        "biểu đồ cột trên dòng (wp:inline, c:chart + workbook nhúng, dữ liệu đã sửa) và biểu đồ đường có kiểu/màu",
        e.save_docx()?,
    ));

    // 2. Floating charts: square, tight, behind and in front of the text.
    let filler = "Nội dung minh hoạ cho việc ngắt dòng quanh khung biểu đồ đặt nổi trên trang, \
                  các dòng chữ chạy vòng qua khung theo kiểu bao quanh đã chọn. ";
    let body = [
        "Báo cáo thị phần".to_owned(),
        format!("Biểu đồ tròn, bao kiểu hình vuông. {}", filler.repeat(3)),
        format!("Biểu đồ thanh, bao sát. {}", filler.repeat(3)),
        format!("Biểu đồ vùng nằm sau chữ. {}", filler.repeat(3)),
        format!("Biểu đồ vành khuyên nằm trước chữ. {}", filler.repeat(3)),
    ]
    .join("\n");
    let mut e = document(&body)?;
    for (p, kind, wrap, x) in [
        (1, "pie", "square", 0.0),
        (2, "bar", "tight", 260.0),
        (3, "area", "behind", 0.0),
        (4, "doughnut", "inFront", 300.0),
    ] {
        caret(&mut e, p)?;
        run(
            &mut e,
            &json!({"cmd": "insertChart", "kind": kind, "wrap": wrap, "width": 200, "height": 130,
                "data": grid(&["Miền Bắc", "Miền Trung", "Miền Nam"], &[("Thị phần", &[42.0, 18.0, 40.0])])}),
        )?;
        // §20.4.2.10 positionH / §20.4.2.11 positionV offsets of the floating frame.
        run(
            &mut e,
            &json!({"cmd": "setPicturePosition", "x": x, "y": 0}),
        )?;
    }
    out.push((
        "doc-chart-2.docx",
        "biểu đồ nổi (wp:anchor): bao vuông, bao sát (wrapPolygon), sau chữ (behindDoc), trước chữ",
        e.save_docx()?,
    ));

    // 3. Chartex layouts (mc:AlternateContent) and a volume stock chart (secondary axis).
    let mut e = document("Phân tích dòng tiền\n\nPhễu bán hàng\n\nGiá và khối lượng giao dịch\n\nNguồn: số liệu minh hoạ.")?;
    caret(&mut e, 1)?;
    run(
        &mut e,
        &json!({"cmd": "insertChart", "kind": "waterfall", "title": "Dòng tiền",
            "data": grid(&["Đầu kỳ", "Thu", "Chi", "Đầu tư", "Cuối kỳ"], &[("Tỷ đồng", &[50.0, 30.0, -20.0, -15.0, 45.0])])}),
    )?;
    caret(&mut e, 3)?;
    run(
        &mut e,
        &json!({"cmd": "insertChart", "kind": "funnel",
            "data": grid(&["Tiếp cận", "Quan tâm", "Báo giá", "Ký hợp đồng"], &[("Khách hàng", &[1200.0, 640.0, 210.0, 75.0])])}),
    )?;
    caret(&mut e, 5)?;
    run(
        &mut e,
        &json!({"cmd": "insertChart", "kind": "stockVolumeUpDown", "title": "Phiên giao dịch",
        "data": grid(&["P1", "P2", "P3", "P4"], &[
            ("Khối lượng", &[1500.0, 2300.0, 1800.0, 2600.0]),
            ("Mở", &[20.0, 21.0, 22.5, 21.8]),
            ("Cao", &[22.0, 23.0, 23.0, 24.1]),
            ("Thấp", &[19.0, 20.5, 20.0, 21.2]),
            ("Đóng", &[21.0, 22.5, 20.5, 23.6]),
        ])}),
    )?;
    out.push((
        "doc-chart-3.docx",
        "biểu đồ mở rộng cx: thác nước và phễu trong mc:AlternateContent (Requires cx1/cx2), biểu đồ chứng khoán có khối lượng (giá trên trục phụ tự co giãn)",
        e.save_docx()?,
    ));
    Ok(out)
}
