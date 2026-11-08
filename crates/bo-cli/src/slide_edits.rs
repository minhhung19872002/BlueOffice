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
    // Adjust handles and connectors attached to connection sites, re-routed
    // when a shape moves; marquee selection.
    Script {
        base: "slide-shapes-presets.pptx",
        name: "slide-edit-connectors.pptx",
        commands: &[
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"insertShape","preset":"roundRect","rect":{"x":60,"y":400,"w":160,"h":90}}"#,
            r#"{"cmd":"adjust","index":0,"x":100,"y":400}"#,
            r#"{"cmd":"insertShape","preset":"rightArrow","rect":{"x":300,"y":400,"w":160,"h":90}}"#,
            r#"{"cmd":"adjust","index":1,"x":380,"y":400}"#,
            r#"{"cmd":"insertShape","preset":"rect","rect":{"x":80,"y":250,"w":120,"h":60}}"#,
            r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":500,"y":250,"w":140,"h":70}}"#,
            r#"{"cmd":"selectRect","rect":{"x":70,"y":240,"w":580,"h":90}}"#,
            r#"{"cmd":"insertConnector"}"#,
            r#"{"cmd":"click","x":570,"y":285}"#,
            r#"{"cmd":"transform","transform":{"kind":"move","dx":40,"dy":90},"snap":false}"#,
            r#"{"cmd":"selectRect","rect":{"x":50,"y":390,"w":420,"h":110}}"#,
            r#"{"cmd":"insertConnector","preset":"bentConnector3"}"#,
        ],
    },
    // Connectors keep their rotation when re-routed (elbow connector turned
    // 90°, bend recomputed) and connectors inside a group follow their shapes.
    Script {
        base: "slide-shapes-presets.pptx",
        name: "slide-edit-connectors-rotated.pptx",
        commands: &[
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"insertShape","preset":"rect","rect":{"x":80,"y":250,"w":100,"h":60}}"#,
            r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":400,"y":350,"w":100,"h":60}}"#,
            r#"{"cmd":"selectRect","rect":{"x":70,"y":240,"w":440,"h":180}}"#,
            r#"{"cmd":"insertConnector","preset":"bentConnector3"}"#,
            r#"{"cmd":"transform","transform":{"kind":"rotate","x":2000,"y":330,"step":true}}"#,
            r#"{"cmd":"click","x":450,"y":380}"#,
            r#"{"cmd":"transform","transform":{"kind":"move","dx":40,"dy":60},"snap":false}"#,
            r#"{"cmd":"insertShape","preset":"rect","rect":{"x":560,"y":100,"w":100,"h":60}}"#,
            r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":760,"y":220,"w":100,"h":60}}"#,
            r#"{"cmd":"selectRect","rect":{"x":550,"y":90,"w":320,"h":200}}"#,
            r#"{"cmd":"insertConnector","preset":"curvedConnector3"}"#,
            r#"{"cmd":"selectRect","rect":{"x":550,"y":90,"w":320,"h":200}}"#,
            r#"{"cmd":"group"}"#,
            r#"{"cmd":"click","x":810,"y":250,"deep":true}"#,
            r#"{"cmd":"transform","transform":{"kind":"move","dx":0,"dy":80},"snap":false}"#,
        ],
    },
    // Tables: cell text, rows/columns, merge, cell fill, style options.
    Script {
        base: "slide-tables-basic.pptx",
        name: "slide-edit-tables.pptx",
        commands: &[
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"insertTable","rows":3,"cols":3,"rect":{"x":80,"y":150,"w":600,"h":90}}"#,
            r#"{"cmd":"tableSelect","r0":0,"c0":0}"#,
            r#"{"cmd":"setCellText","text":"Tỉnh"}"#,
            r#"{"cmd":"tableSelect","r0":0,"c0":1}"#,
            r#"{"cmd":"setCellText","text":"Dân số"}"#,
            r#"{"cmd":"tableSelect","r0":1,"c0":0}"#,
            r#"{"cmd":"setCellText","text":"Đà Nẵng"}"#,
            r#"{"cmd":"tableInsertRow","below":true}"#,
            r#"{"cmd":"tableInsertCol","right":true}"#,
            r#"{"cmd":"tableSelect","r0":2,"c0":0,"r1":3,"c1":1}"#,
            r#"{"cmd":"tableMerge"}"#,
            r#"{"cmd":"tableFill","color":"FFF2CC"}"#,
            r#"{"cmd":"tableSelect","r0":0,"c0":3}"#,
            r#"{"cmd":"tableDeleteCols"}"#,
            r#"{"cmd":"tableOptions","bandRow":false}"#,
            r#"{"cmd":"tableExit"}"#,
        ],
    },
    // Slides pasted from another deck (pictures copied, layout matched by name or
    // placeholder types): the clipboard is filled from `slide-images-crop.pptx`
    // and `slide-tables-basic.pptx` first (see `open_script`).
    Script {
        base: "slide-text-bullets.pptx",
        name: PASTE_DECK,
        commands: &[r#"{"cmd":"goTo","slide":0}"#, r#"{"cmd":"paste"}"#],
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
    // Transitions (§19.3.1.50): per slide, for every slide, timing only.
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-edit-transitions.pptx",
        commands: &[
            r#"{"cmd":"setTransition","all":true,"transition":{"kind":"fade","durMs":700}}"#,
            r#"{"cmd":"setTransition","slides":[0],"transition":{"kind":"push","dir":"u","durMs":900}}"#,
            r#"{"cmd":"goTo","slide":1}"#,
            r#"{"cmd":"setTransition","transition":{"kind":"wipe","dir":"r","advanceOnClick":false,"advanceAfterMs":3000}}"#,
            r#"{"cmd":"setTransition","slides":[0],"transition":{"kind":"keep","advanceAfterMs":5000}}"#,
        ],
    },
    // Main sequence (§19.5): appear, fade, fly-in, wipe; on click / with / after previous.
    Script {
        base: "slide-shapes-presets.pptx",
        name: "slide-edit-animations.pptx",
        commands: &[
            r#"{"cmd":"select","ids":[10]}"#,
            r#"{"cmd":"addAnimation","effect":"appear"}"#,
            r#"{"cmd":"select","ids":[11]}"#,
            r#"{"cmd":"addAnimation","effect":"fade","trigger":"withPrevious","durMs":800}"#,
            r#"{"cmd":"select","ids":[12,13]}"#,
            r#"{"cmd":"addAnimation","effect":"flyIn","direction":"l","trigger":"afterPrevious"}"#,
            r#"{"cmd":"select","ids":[14]}"#,
            r#"{"cmd":"addAnimation","effect":"wipe","direction":"t","delayMs":300}"#,
            r#"{"cmd":"select","ids":[15]}"#,
            r#"{"cmd":"addAnimation","effect":"fade","class":"exit"}"#,
            r#"{"cmd":"setAnimation","index":1,"durMs":1200}"#,
            r#"{"cmd":"moveAnimation","from":5,"to":4}"#,
            r#"{"cmd":"setTransition","transition":{"kind":"cover","dir":"r"}}"#,
        ],
    },
    // Effects BlueSlide does not model stay as read while the sequence changes.
    Script {
        base: "slide-anim-unknown.pptx",
        name: "slide-edit-anim-unknown.pptx",
        commands: &[
            r#"{"cmd":"setAnimation","index":0,"durMs":900}"#,
            r#"{"cmd":"setAnimation","index":1,"trigger":"afterPrevious","delayMs":200}"#,
            r#"{"cmd":"moveAnimation","from":2,"to":0}"#,
            r#"{"cmd":"setTransition","transition":{"kind":"keep","durMs":1100}}"#,
        ],
    },
    // Charts: insert column / line / pie with embedded workbooks, edit them.
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-edit-charts.pptx",
        commands: &[
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"insertChart","kind":"column","title":"Doanh thu theo quý","data":{"categories":["Quý 1","Quý 2","Quý 3","Quý 4"],"series":[{"name":"2025","values":[12,15,9,18]},{"name":"2026","values":[14,17,null,21]}]}}"#,
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"insertChart","kind":"line","title":"Khách hàng","data":{"categories":["T1","T2","T3"],"series":[{"name":"Hà Nội","values":[3,5,8]}]}}"#,
            r#"{"cmd":"setChart","legend":"t","data":{"categories":["T1","T2","T3","T4"],"series":[{"name":"Hà Nội","values":[3,5,8,9]},{"name":"Đà Nẵng","values":[2,2,4,6]}]}}"#,
            r#"{"cmd":"addSlide"}"#,
            r#"{"cmd":"insertChart","kind":"bar"}"#,
            r#"{"cmd":"setChart","kind":"pie","title":"Thị phần","legend":"r","data":{"categories":["A","B","C"],"series":[{"name":"Phần","values":[50,30,20]}]}}"#,
        ],
    },
    // A chart read from a file: data, title, legend and type changed.
    Script {
        base: "slide-charts-column.pptx",
        name: "slide-edit-charts-existing.pptx",
        commands: &[
            r#"{"cmd":"selectAll"}"#,
            r#"{"cmd":"select","ids":[4]}"#,
            r#"{"cmd":"setChart","kind":"bar","title":"Đã sửa dữ liệu","legend":"b","data":{"categories":["Bắc","Trung","Nam"],"series":[{"name":"Năm nay","values":[7,4,9]},{"name":"Năm trước","values":[5,3,8]}]}}"#,
        ],
    },
    // Video and audio inserted (see `decks`): clips, posters, relationships.
    Script {
        base: "slide-text-bullets.pptx",
        name: MEDIA_DECK,
        commands: &[r#"{"cmd":"addSlide"}"#],
    },
];

/// Deck receiving a video and an audio clip.
const MEDIA_DECK: &str = "slide-edit-media.pptx";

/// Deck receiving slides copied from other decks.
const PASTE_DECK: &str = "slide-edit-paste-other-deck.pptx";

/// Open a script's base deck; for [`PASTE_DECK`] the slide clipboard is first filled
/// with slides of other decks (one engine keeps its clipboard across decks).
fn open_script(e: &mut Engine, s: &Script, base: &[(&'static str, Vec<u8>)]) -> bool {
    if s.name == PASTE_DECK {
        let Some((_, src)) = base.iter().find(|(n, _)| *n == "slide-images-crop.pptx") else {
            return false;
        };
        if e.open(src, "src.pptx").is_err() {
            return false;
        }
        let _ = e.slide_command_json(r#"{"cmd":"copySlides","indices":[0,1]}"#);
    }
    let Some((_, bytes)) = base.iter().find(|(n, _)| *n == s.base) else {
        return false;
    };
    e.open(bytes, s.base).is_ok()
}

/// Placeholder clips (headers only, like the media base decks).
fn media_clips() -> (Vec<u8>, Vec<u8>) {
    let mut mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    mp4.extend(std::iter::repeat_n(0u8, 64));
    let mut mp3 = b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec();
    mp3.extend(std::iter::repeat_n(0u8, 32));
    (mp4, mp3)
}

/// Edited decks, built from the generated base decks.
pub fn decks(base: &[(&'static str, Vec<u8>)], picture: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let mut out = Vec::new();
    for s in SCRIPTS {
        let mut e = Engine::with_bundled_fonts();
        if !open_script(&mut e, s, base) {
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
        if s.name == MEDIA_DECK {
            let (mp4, mp3) = media_clips();
            let _ = e.slide_insert_picture(mp4, "gioi-thieu.mp4");
            let _ = e.slide_insert_picture(mp3, "nhac-nen.mp3");
            let _ = e.slide_command_json(r#"{"cmd":"addAnimation","effect":"fade"}"#);
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
            let mut e = Engine::with_bundled_fonts();
            assert!(open_script(&mut e, s, &base), "{}", s.name);
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
