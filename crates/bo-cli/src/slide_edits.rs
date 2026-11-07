//! Corpus decks edited through the slide edit API (`bo_engine::slide_edit`):
//! generated decks are opened, edited with the same JSON commands the UI
//! sends, and saved. The fidelity report validates them with
//! OpenXmlValidator like every other corpus file.

use bo_engine::Engine;

/// A deck to edit: base corpus deck, output name, JSON commands.
struct Script {
    base: &'static str,
    name: &'static str,
    commands: &'static [&'static str],
}

const SCRIPTS: &[Script] = &[
    // Arrange: move, resize, rotate, group, align, distribute, z-order, duplicate, delete.
    Script {
        base: "slide-shapes-presets.pptx",
        name: "slide-edit-arrange.pptx",
        commands: &[
            r#"{"cmd":"select","ids":[10]}"#,
            r#"{"cmd":"transform","transform":{"kind":"move","dx":24,"dy":12},"snap":false}"#,
            r#"{"cmd":"select","ids":[11]}"#,
            r#"{"cmd":"transform","transform":{"kind":"resize","handle":"se","dx":30,"dy":10,"keepAspect":true},"snap":false}"#,
            r#"{"cmd":"select","ids":[12]}"#,
            r#"{"cmd":"transform","transform":{"kind":"rotate","x":2000,"y":0,"step":true}}"#,
            r#"{"cmd":"select","ids":[13,14,15]}"#,
            r#"{"cmd":"group"}"#,
            r#"{"cmd":"transform","transform":{"kind":"move","dx":0,"dy":20},"snap":false}"#,
            r#"{"cmd":"select","ids":[16,17,18,19]}"#,
            r#"{"cmd":"align","to":"top"}"#,
            r#"{"cmd":"distribute","axis":"horizontal"}"#,
            r#"{"cmd":"select","ids":[21]}"#,
            r#"{"cmd":"zOrder","to":"back"}"#,
            r#"{"cmd":"duplicate"}"#,
            r#"{"cmd":"setFill","color":"7030A0"}"#,
            r#"{"cmd":"setLine","color":"FFC000","width":3}"#,
            r#"{"cmd":"select","ids":[22]}"#,
            r#"{"cmd":"delete"}"#,
            r#"{"cmd":"goTo","slide":1}"#,
            r#"{"cmd":"selectAll"}"#,
            r#"{"cmd":"transform","transform":{"kind":"resize","handle":"e","dx":-60,"dy":0},"snap":false}"#,
        ],
    },
    // Text: edit placeholder text, format runs and paragraphs, insert text box,
    // shapes and a table, add a slide from a layout and write notes.
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-edit-text.pptx",
        commands: &[
            r#"{"cmd":"select","ids":[2]}"#,
            r#"{"cmd":"setText","text":"Báo cáo quý IV – đã chỉnh sửa"}"#,
            r#"{"cmd":"formatText","format":{"kind":"color","hex":"C00000"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"bold"}}"#,
            r#"{"cmd":"insertTextBox","text":"Hộp văn bản mới\nDòng thứ hai","rect":{"x":60,"y":420,"w":360,"h":60}}"#,
            r#"{"cmd":"formatText","format":{"kind":"italic"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"size","points":20}}"#,
            r#"{"cmd":"formatText","format":{"kind":"align","align":"center"}}"#,
            r#"{"cmd":"insertShape","preset":"rightArrow","rect":{"x":520,"y":430,"w":160,"h":60}}"#,
            r#"{"cmd":"setText","text":"Tiếp"}"#,
            r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":720,"y":420,"w":120,"h":80}}"#,
            r#"{"cmd":"setFill","color":"00B050"}"#,
            r#"{"cmd":"goTo","slide":1}"#,
            r#"{"cmd":"select","ids":[3]}"#,
            r#"{"cmd":"setText","text":"Mục tiêu năm 2026\nTăng doanh thu 20%\nMở rộng thị trường miền Trung\nĐà Nẵng, Huế, Quảng Nam\nKế hoạch triển khai"}"#,
            r#"{"cmd":"formatText","format":{"kind":"bullets"}}"#,
            r#"{"cmd":"setNotes","text":"Nhấn mạnh mục tiêu doanh thu.\nCâu hỏi dự kiến."}"#,
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"select","ids":[2]}"#,
            r#"{"cmd":"setText","text":"Slide mới từ bố cục"}"#,
            r#"{"cmd":"insertTable","rows":3,"cols":4,"rect":{"x":80,"y":160,"w":800,"h":120}}"#,
        ],
    },
    // Rich text inside shapes: formatting character ranges, paragraph formats,
    // list levels, line spacing, typing with auto-fit.
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-edit-rich.pptx",
        commands: &[
            r#"{"cmd":"insertTextBox","text":"Định dạng một phần đoạn chữ\nDòng hai có gạch đầu dòng\nDòng ba","rect":{"x":60,"y":300,"w":520,"h":60}}"#,
            r#"{"cmd":"textSelect","start":0,"end":9}"#,
            r#"{"cmd":"formatText","format":{"kind":"bold"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"color","hex":"1F4E79"}}"#,
            r#"{"cmd":"textSelect","start":10,"end":13}"#,
            r#"{"cmd":"formatText","format":{"kind":"italic"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"strike"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"highlight","hex":"FFFF00"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"font","typeface":"Arial"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"size","points":28}}"#,
            r#"{"cmd":"textSelect","start":30,"end":30}"#,
            r#"{"cmd":"formatText","format":{"kind":"bullets"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"level","delta":1}}"#,
            r#"{"cmd":"formatText","format":{"kind":"lineSpacing","value":1.5}}"#,
            r#"{"cmd":"formatText","format":{"kind":"align","align":"center"}}"#,
            r#"{"cmd":"formatText","format":{"kind":"numbering"},"range":[54,61]}"#,
            r#"{"cmd":"setText","text":"Định dạng một phần đoạn chữ\nDòng hai có gạch đầu dòng\nDòng ba\nDòng bốn gõ thêm","merge":true,"sel":[78,78]}"#,
            r#"{"cmd":"textExit"}"#,
        ],
    },
    // Clipboard: copy/paste shapes with pictures on the same and another
    // slide, cut, copy/paste slides.
    Script {
        base: "slide-images-crop.pptx",
        name: "slide-edit-clipboard.pptx",
        commands: &[
            r#"{"cmd":"selectAll"}"#,
            r#"{"cmd":"copy"}"#,
            r#"{"cmd":"paste"}"#,
            r#"{"cmd":"transform","transform":{"kind":"resize","handle":"se","dx":-40,"dy":-30,"keepAspect":true},"snap":false}"#,
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"paste"}"#,
            r#"{"cmd":"cut"}"#,
            r#"{"cmd":"paste"}"#,
            r#"{"cmd":"copySlides","indices":[0]}"#,
            r#"{"cmd":"paste"}"#,
        ],
    },
    // Slides: duplicate, reorder, delete, notes and a picture.
    Script {
        base: "slide-notes-basic.pptx",
        name: "slide-edit-slides.pptx",
        commands: &[
            r#"{"cmd":"duplicateSlide","index":0}"#,
            r#"{"cmd":"setNotes","text":"Ghi chú của bản sao"}"#,
            r#"{"cmd":"moveSlide","from":1,"to":0}"#,
            r#"{"cmd":"addSlide","after":1}"#,
            r#"{"cmd":"setNotes","text":"Ghi chú slide mới"}"#,
            r#"{"cmd":"deleteSlide","index":3}"#,
            r#"{"cmd":"goTo","slide":0}"#,
            r#"{"cmd":"setNotes","slide":2,"text":"Ghi chú đã sửa\nDòng hai"}"#,
        ],
    },
];

/// Edited decks, built from the generated base decks.
pub fn decks(base: &[(&'static str, Vec<u8>)], picture: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let mut out = Vec::new();
    for s in SCRIPTS {
        let Some((_, bytes)) = base.iter().find(|(n, _)| *n == s.base) else {
            continue;
        };
        let mut e = Engine::with_bundled_fonts();
        if e.open(bytes, s.base).is_err() {
            continue;
        }
        for c in s.commands {
            // A refused command leaves the deck unchanged; the tests below catch it.
            let _ = e.slide_command_json(c);
        }
        if s.name == "slide-edit-slides.pptx" {
            let _ = e.slide_command_json(r#"{"cmd":"goTo","slide":1}"#);
            let _ = e.slide_insert_picture(picture.to_vec(), "logo.png");
        }
        if let Ok(b) = e.save_pptx() {
            out.push((s.name, b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scripted_command_applies() {
        let base = crate::slide_samples::base_decks();
        for s in SCRIPTS {
            let (_, bytes) = base.iter().find(|(n, _)| *n == s.base).unwrap();
            let mut e = Engine::with_bundled_fonts();
            e.open(bytes, s.base).unwrap();
            for c in s.commands {
                let r: serde_json::Value =
                    serde_json::from_str(&e.slide_command_json(c).unwrap()).unwrap();
                assert!(r.get("error").is_none(), "{}: {c}: {}", s.name, r["error"]);
            }
        }
        let decks = decks(&base, &crate::slide_samples::sample_png());
        assert_eq!(decks.len(), SCRIPTS.len());
        for (name, bytes) in decks {
            let p = bo_pptx::read_pptx(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!p.slides.is_empty(), "{name}");
        }
    }
}
