//! SmartArt corpus of BlueDoc written by the editor's own SmartArt commands (`insertSmartArt`,
//! `setSmartArt`; CORE-SMART-004): assistant shapes (ST_PtType `asst`, ECMA-376 Part 1, §21.4.3
//! `dgm:pt`) and diagrams laid out right to left (the layout variable `dgm:dir val="rev"`). All
//! text is invented.

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

/// A generated file: name, description, bytes.
type File = (&'static str, &'static str, Vec<u8>);

/// The documents.
pub fn documents() -> Result<Vec<File>, Box<dyn std::error::Error>> {
    let mut e = Engine::with_bundled_fonts();
    e.open(
        "Sơ đồ tổ chức\n\nQuy trình tiếp nhận hồ sơ (từ phải sang trái)\n\nVòng đời dự án"
            .as_bytes(),
        "new.txt",
    )?;
    // An organisation chart with assistants of the top shape and of a second-level shape.
    caret(&mut e, 1)?;
    run(
        &mut e,
        &json!({"cmd": "insertSmartArt", "layout": "hierarchy", "width": 420, "height": 220,
        "items": [
            {"text": "Giám đốc", "level": 0},
            {"text": "Thư ký", "level": 1, "assistant": true},
            {"text": "Kỹ thuật", "level": 1},
            {"text": "Trợ lý kỹ thuật", "level": 2, "assistant": true},
            {"text": "Phát triển", "level": 2},
            {"text": "Kinh doanh", "level": 1}
        ]}),
    )?;
    // A process laid out right to left.
    caret(&mut e, 3)?;
    run(
        &mut e,
        &json!({"cmd": "insertSmartArt", "layout": "process", "colors": "green", "width": 420,
        "height": 120, "items": [
            {"text": "Tiếp nhận"}, {"text": "Xử lý"}, {"text": "Trả kết quả"}
        ]}),
    )?;
    run(&mut e, &json!({"cmd": "setSmartArt", "rtl": true}))?;
    // A cycle right to left, outline look.
    caret(&mut e, 5)?;
    run(
        &mut e,
        &json!({"cmd": "insertSmartArt", "layout": "cycle", "colors": "orange", "style": "outline",
        "width": 300, "height": 220, "items": [
            {"text": "Lập kế hoạch"}, {"text": "Thực hiện"}, {"text": "Kiểm tra"}, {"text": "Cải tiến"}
        ]}),
    )?;
    run(&mut e, &json!({"cmd": "setSmartArt", "rtl": true}))?;
    Ok(vec![(
        "smartart-06-assistant-rtl.docx",
        "sơ đồ tổ chức có trợ lý (dgm:pt type=\"asst\") ở cấp 1 và cấp 2; quy trình và vòng tròn từ phải sang trái (dgm:dir val=\"rev\"), viết bằng lệnh SmartArt của BlueDoc",
        e.save_docx()?,
    )])
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // source: ECMA-376 Part 1, §21.4.3 dgm:pt type `asst` and the layout variable `dgm:dir`:
    // the written document has two assistants and two diagrams right to left, and reads back so.
    #[test]
    fn assistants_and_right_to_left_read_back() {
        let (_, _, bytes) = documents().unwrap().remove(0);
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let data: Vec<String> = pkg
            .parts()
            .iter()
            .filter(|p| p.name.starts_with("/word/diagrams/data"))
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .collect();
        assert_eq!(data.len(), 3);
        assert_eq!(
            data.iter()
                .map(|d| d.matches(r#"type="asst""#).count())
                .sum::<usize>(),
            2
        );
        assert_eq!(
            data.iter()
                .filter(|d| d.contains(r#"<dgm:dir val="rev"/>"#))
                .count(),
            2
        );
        let mut e = Engine::with_bundled_fonts();
        e.open(&bytes, "smartart.docx").unwrap();
        caret(&mut e, 1).unwrap();
        let info = e.doc_smartart().unwrap();
        assert!(!info.rtl);
        assert_eq!(info.items.iter().filter(|i| i.assistant).count(), 2);
        caret(&mut e, 3).unwrap();
        assert!(e.doc_smartart().unwrap().rtl);
    }
}
