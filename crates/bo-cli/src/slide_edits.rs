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
    // Slides from a 4:3 deck pasted into a 16:9 one: positions and sizes are
    // scaled to fit and centred (document approach).
    Script {
        base: "slide-shapes-presets.pptx",
        name: PASTE_RESIZED,
        commands: &[r#"{"cmd":"goTo","slide":0}"#, r#"{"cmd":"paste"}"#],
    },
    // A chart slide pasted into another deck: the chart part, its embedded
    // workbook, colour and style parts come along under new names.
    Script {
        base: "slide-text-bullets.pptx",
        name: PASTE_CHART,
        commands: &[r#"{"cmd":"goTo","slide":0}"#, r#"{"cmd":"paste"}"#],
    },
    // Slides with slide → slide hyperlinks: a link to a slide pasted along is
    // re-pointed at its copy, a link to a slide left behind is removed.
    Script {
        base: "slide-shapes-presets.pptx",
        name: PASTE_LINKS,
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
    // Zooms (SLIDE-ZOOM-001, [MS-PPTX] §2.8–§2.11): slide zooms with options, section zooms,
    // a summary zoom creating its slide and sections.
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-zoom-slide.pptx",
        commands: &[
            r#"{"cmd":"goTo","slide":0}"#,
            r#"{"cmd":"zoomInsertSlides","slides":[1,2]}"#,
            r#"{"cmd":"zoomOptions","returnToParent":false,"transition":true,"durationMs":1500}"#,
        ],
    },
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-zoom-section.pptx",
        commands: &[
            r#"{"cmd":"addSection","name":"Mục tiêu","slide":1,"firstName":"Mở đầu"}"#,
            r#"{"cmd":"addSection","name":"Kết luận","slide":2}"#,
            r#"{"cmd":"goTo","slide":0}"#,
            r#"{"cmd":"zoomInsertSections","sections":[1,2]}"#,
            r#"{"cmd":"zoomOptions","showBg":false}"#,
        ],
    },
    Script {
        base: "slide-text-bullets.pptx",
        name: "slide-zoom-summary.pptx",
        commands: &[
            r#"{"cmd":"zoomInsertSummary","slides":[1,2],"names":{"title":"Tổng quan","summary":"Phần tổng quan","section":"Phần {n}","first":"Mở đầu"}}"#,
            r#"{"cmd":"zoomOptions","transition":true,"durationMs":800}"#,
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

const PASTE_RESIZED: &str = "slide-edit-paste-resized.pptx";
const PASTE_CHART: &str = "slide-edit-paste-chart.pptx";
const PASTE_LINKS: &str = "slide-edit-paste-links.pptx";

const REL_SLIDE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

/// Add a click action jumping to slide `to` on the first shape of slide `from`
/// (§21.1.2.3.5 hlinkClick, `ppaction://hlinksldjump`, relationship to the slide).
fn link_slides(p: &mut bo_model::slide::Presentation, from: usize, to: usize) {
    use bo_model::slide::{Rel, XmlElement, XmlNode};
    let target = p.slides[to].info.part_name.clone();
    let owner = p.slides[from].info.part_name.clone();
    let rels = &mut p.slides[from].info.rels;
    let id = (1..=rels.len() + 1)
        .map(|n| format!("rIdL{n}"))
        .find(|i| rels.iter().all(|r| &r.id != i))
        .unwrap_or_default();
    rels.push(Rel {
        id: id.clone(),
        rel_type: REL_SLIDE.into(),
        target: bo_engine::slide_edit::clip::relative_target(&owner, &target),
        external: false,
    });
    let Some(n) = p.slides[from].data.shapes.first_mut() else {
        return;
    };
    let mut el = bo_pptx::write_node(n);
    let add = |e: &mut XmlElement| {
        let mut h = XmlElement::new("a:hlinkClick");
        h.attrs = vec![
            ("r:id".into(), id.clone()),
            ("action".into(), "ppaction://hlinksldjump".into()),
        ];
        e.children.push(XmlNode::Element(h));
    };
    // cNvPr is the first child of the nv*Pr element.
    if let Some(XmlNode::Element(nv)) = el.children.first_mut() {
        if let Some(XmlNode::Element(cnv)) = nv.children.first_mut() {
            add(cnv);
        }
    }
    *n = bo_pptx::parse_node(&el);
}

/// A chart deck whose chart has an embedded workbook (§21.2.2.63 externalData)
/// and colour / style parts ([MS-ODRAWXML] chartColorStyle, chartStyle).
fn chart_source(bytes: &[u8]) -> Option<Vec<u8>> {
    use bo_model::slide::RawPart;
    let mut p = bo_pptx::read_pptx(bytes).ok()?;
    let chart = p
        .parts
        .iter_mut()
        .find(|x| x.name == "/ppt/charts/chart1.xml")?;
    let xml = String::from_utf8(chart.data.clone()).ok()?;
    let close = xml.rfind("</c:chartSpace>")?;
    let mut x = xml[..close].to_owned();
    if !x.contains("xmlns:r=") {
        x = x.replacen(
            "<c:chartSpace ",
            "<c:chartSpace xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
            1,
        );
    }
    x.push_str(
        "<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/></c:externalData></c:chartSpace>",
    );
    chart.data = x.into_bytes();
    let mut wb = bo_model::sheet::Workbook::new();
    wb.sheets[0].set_cell(
        0,
        0,
        bo_model::sheet::Cell::new(bo_model::sheet::CellValue::Text("Doanh thu".into())),
    );
    let book = bo_xlsx::write_xlsx(&wb).ok()?;
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/package\" Target=\"../embeddings/Microsoft_Excel_Worksheet1.xlsx\"/></Relationships>";
    p.parts.push(RawPart {
        name: "/ppt/embeddings/Microsoft_Excel_Worksheet1.xlsx".into(),
        content_type: Some(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
        ),
        data: book,
    });
    p.parts.push(RawPart {
        name: "/ppt/charts/_rels/chart1.xml.rels".into(),
        content_type: Some("application/vnd.openxmlformats-package.relationships+xml".into()),
        data: rels.as_bytes().to_vec(),
    });
    bo_pptx::write_pptx(&p).ok()
}

/// The source deck of a paste script, prepared for its feature.
fn paste_source(name: &str, base: &[(&'static str, Vec<u8>)]) -> Option<(Vec<u8>, &'static str)> {
    let get = |n: &str| base.iter().find(|(b, _)| *b == n).map(|(_, d)| d.clone());
    match name {
        PASTE_DECK => Some((
            get("slide-images-crop.pptx")?,
            r#"{"cmd":"copySlides","indices":[0,1]}"#,
        )),
        PASTE_RESIZED => {
            let mut p = bo_pptx::read_pptx(&get("slide-text-bullets.pptx")?).ok()?;
            // 4:3 at 10 × 7.5 in (§19.2.1.39 sldSz).
            p.slide_size.cx = 9_144_000;
            p.slide_size.cy = 6_858_000;
            p.slide_size.kind = Some("screen4x3".into());
            Some((
                bo_pptx::write_pptx(&p).ok()?,
                r#"{"cmd":"copySlides","indices":[0,1]}"#,
            ))
        }
        PASTE_CHART => Some((
            chart_source(&get("slide-charts-column.pptx")?)?,
            r#"{"cmd":"copySlides","indices":[0]}"#,
        )),
        PASTE_LINKS => {
            let mut p = bo_pptx::read_pptx(&get("slide-text-bullets.pptx")?).ok()?;
            if p.slides.len() < 3 {
                return None;
            }
            link_slides(&mut p, 0, 2);
            link_slides(&mut p, 1, 0);
            Some((
                bo_pptx::write_pptx(&p).ok()?,
                r#"{"cmd":"copySlides","indices":[0,1]}"#,
            ))
        }
        _ => None,
    }
}

/// Open a script's base deck; for the paste scripts the slide clipboard is first
/// filled with slides of other decks (one engine keeps its clipboard across decks).
fn open_script(e: &mut Engine, s: &Script, base: &[(&'static str, Vec<u8>)]) -> bool {
    if let Some((src, copy)) = paste_source(s.name, base) {
        if e.open(&src, "src.pptx").is_err() {
            return false;
        }
        let _ = e.slide_command_json(copy);
    } else if [PASTE_DECK, PASTE_RESIZED, PASTE_CHART, PASTE_LINKS].contains(&s.name) {
        return false;
    }

    let Some((_, bytes)) = base.iter().find(|(n, _)| *n == s.base) else {
        return false;
    };
    e.open(bytes, s.base).is_ok()
}

/// Connectors inside a flipped group follow their shapes (§20.1.7.6 flipH /
/// flipV of the group): `(name, flipH, flipV)`.
const FLIPPED: &[(&str, bool, bool)] = &[
    ("slide-edit-connectors-flip-h.pptx", true, false),
    ("slide-edit-connectors-flip-v.pptx", false, true),
    ("slide-edit-connectors-flip-hv.pptx", true, true),
];

/// A group of two shapes joined by a connector, flipped, then one shape moved.
fn flipped_connectors(base: &[u8], flip_h: bool, flip_v: bool) -> Option<Vec<u8>> {
    let mut e = Engine::with_bundled_fonts();
    e.open(base, "base.pptx").ok()?;
    for c in [
        r#"{"cmd":"addSlide"}"#,
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":100,"y":200,"w":120,"h":70}}"#,
        r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":420,"y":320,"w":120,"h":70}}"#,
        r#"{"cmd":"selectRect","rect":{"x":90,"y":190,"w":460,"h":210}}"#,
        r#"{"cmd":"insertConnector","preset":"bentConnector3"}"#,
        r#"{"cmd":"selectRect","rect":{"x":90,"y":190,"w":460,"h":210}}"#,
        r#"{"cmd":"group"}"#,
    ] {
        e.slide_command_json(c).ok()?;
    }
    let mut p = bo_pptx::read_pptx(&e.save_pptx().ok()?).ok()?;
    let last = p.slides.len() - 1;
    for n in &mut p.slides[last].data.shapes {
        if let bo_model::slide::ShapeNode::Group(g) = n {
            if let Some(x) = g.props.xfrm.as_mut() {
                x.flip_h = flip_h.then_some(true);
                x.flip_v = flip_v.then_some(true);
            }
        }
    }
    let mut e = Engine::with_bundled_fonts();
    e.open(&bo_pptx::write_pptx(&p).ok()?, "flipped.pptx")
        .ok()?;
    // The ellipse after the flips (group box 100–540 × 200–390, mirrored).
    let x = if flip_h { 160.0 } else { 480.0 };
    let y = if flip_v { 235.0 } else { 355.0 };
    for c in [
        format!(r#"{{"cmd":"goTo","slide":{last}}}"#),
        format!(r#"{{"cmd":"click","x":{x},"y":{y},"deep":true}}"#),
        r#"{"cmd":"transform","transform":{"kind":"move","dx":40,"dy":30},"snap":false}"#.into(),
    ] {
        e.slide_command_json(&c).ok()?;
    }
    e.save_pptx().ok()
}

/// Placeholder clips (headers only, like the media base decks).
fn media_clips() -> (Vec<u8>, Vec<u8>) {
    let mut mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    mp4.extend(std::iter::repeat_n(0u8, 64));
    let mut mp3 = b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec();
    mp3.extend(std::iter::repeat_n(0u8, 32));
    (mp4, mp3)
}

/// A short WAV clip (PCM, 8 kHz mono, a tenth of a second of a square wave).
pub fn sample_wav() -> Vec<u8> {
    let samples: Vec<u8> = (0..800u32)
        .map(|i| if (i / 10) % 2 == 0 { 96 } else { 160 })
        .collect();
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&8000u32.to_le_bytes());
    w.extend_from_slice(&8000u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&8u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    w.extend(samples);
    w
}

/// Recorded slide shows (SLIDE-REC-001): narration on every slide; camera, timed ink and
/// recorded clicks; rehearsed timings only (with "advance on click" off on one slide).
pub const RECORDING_DECKS: [&str; 3] = [
    "slide-rec-narration.pptx",
    "slide-rec-camera-ink.pptx",
    "slide-rec-timings.pptx",
];

fn recording_decks(base: &[(&'static str, Vec<u8>)]) -> Vec<(&'static str, Vec<u8>)> {
    use base64::Engine as _;
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let wav = b64(&sample_wav());
    let (mp4, _) = media_clips();
    let mp4 = b64(&mp4);
    let names = r#""names":{"narration":"Lời thuyết minh","camera":"Camera","ink":"Mực"}"#;
    let narration = |slide: usize, ms: u32| {
        format!(
            r#"{{"slide":{slide},"durationMs":{ms},"narration":{{"data":"{wav}","mime":"audio/wav"}}}}"#
        )
    };
    let mut out = Vec::new();
    let open = |name: &str| {
        let (_, b) = base.iter().find(|(n, _)| *n == name)?;
        let mut e = Engine::with_bundled_fonts();
        e.open(b, "base.pptx").ok()?;
        Some(e)
    };
    if let Some(mut e) = open("slide-text-bullets.pptx") {
        let slides = format!("{},{}", narration(0, 6500), narration(1, 4200));
        let _ = e.slide_command_json(&format!(
            r#"{{"cmd":"saveRecording","slides":[{slides}],{names}}}"#
        ));
        if let Ok(b) = e.save_pptx() {
            out.push((RECORDING_DECKS[0], b));
        }
    }
    if let Some(mut e) = open("slide-shapes-presets.pptx") {
        let _ = e.slide_command_json(r#"{"cmd":"select","ids":[10]}"#);
        let _ = e.slide_command_json(r#"{"cmd":"addAnimation","effect":"fade"}"#);
        let _ = e.slide_command_json(r#"{"cmd":"select","ids":[11]}"#);
        let _ = e.slide_command_json(r#"{"cmd":"addAnimation","effect":"wipe","direction":"l"}"#);
        let ink = concat!(
            r#"[{"points":[[120,300],[160,320],[200,300],[240,320]],"times":[2500,2540,2580,2620],"color":"D7261E","width":2},"#,
            r#"{"points":[[300,120],[420,120]],"times":[5200,5400],"color":"FFE600","width":14,"highlighter":true}]"#
        );
        let _ = e.slide_command_json(&format!(
            concat!(
                r#"{{"cmd":"saveRecording","slides":[{{"slide":0,"durationMs":9000,"clicks":[1500],"#,
                r#""ink":{ink},"narration":{{"data":"{wav}","mime":"audio/wav"}},"#,
                r#""camera":{{"data":"{mp4}","mime":"video/mp4","corner":"bottomLeft"}}}}],{names}}}"#
            ),
            ink = ink,
            wav = wav,
            mp4 = mp4,
            names = names
        ));
        if let Ok(b) = e.save_pptx() {
            out.push((RECORDING_DECKS[1], b));
        }
    }
    if let Some(mut e) = open("slide-text-bullets.pptx") {
        let _ = e.slide_command_json(r#"{"cmd":"goTo","slide":1}"#);
        let _ = e.slide_command_json(
            r#"{"cmd":"setTransition","transition":{"kind":"fade","advanceOnClick":false}}"#,
        );
        let _ = e.slide_command_json(
            r#"{"cmd":"setTimings","timings":[{"slide":0,"ms":3200},{"slide":1,"ms":7450}],"useTimings":true}"#,
        );
        if let Ok(b) = e.save_pptx() {
            out.push((RECORDING_DECKS[2], b));
        }
    }
    out
}

/// Every transition of the catalog (SLIDE-ANIM-002), one slide each with its last variant, a sound
/// on the first slide and "stop the sound" on the second.
pub const TRANSITIONS_DECK: &str = "slide-edit-transitions-all.pptx";

fn transitions_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    for (i, d) in bo_pptx::transitions::CATALOG.iter().enumerate() {
        e.slide_command_json(r#"{"cmd":"addSlide"}"#).ok()?;
        let dir = bo_pptx::transitions::variants(d)
            .last()
            .map_or(String::new(), |v| format!(r#","dir":"{v}""#));
        let c = format!(
            r#"{{"cmd":"setTransition","transition":{{"kind":"{}"{dir},"durMs":{}}}}}"#,
            d.id,
            800 + 50 * (i % 10)
        );
        e.slide_command_json(&c).ok()?;
    }
    let wav = base64::engine::general_purpose::STANDARD.encode(sample_wav());
    e.slide_command_json(r#"{"cmd":"goTo","slide":1}"#).ok()?;
    e.slide_command_json(&format!(
        r#"{{"cmd":"setTransition","transition":{{"kind":"keep","soundData":"{wav}","soundName":"chuông.wav"}}}}"#
    ))
    .ok()?;
    e.slide_command_json(r#"{"cmd":"goTo","slide":2}"#).ok()?;
    e.slide_command_json(r#"{"cmd":"setTransition","transition":{"kind":"keep","sound":"stop"}}"#)
        .ok()?;
    e.save_pptx().ok()
}

/// Every animation effect of the catalog (SLIDE-ANIM-004), one slide each: a shape with the
/// effect (its last variant), and for entrance effects the same effect as an exit after it.
pub const ANIMATIONS_DECK: &str = "slide-edit-animations-all.pptx";

fn animations_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    use bo_pptx::anim_catalog::{Class, EFFECTS};
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    for d in EFFECTS {
        e.slide_command_json(r#"{"cmd":"addSlide"}"#).ok()?;
        e.slide_command_json(r#"{"cmd":"insertShape","preset":"roundRect"}"#)
            .ok()?;
        let dir = d
            .dirs
            .last()
            .map_or(String::new(), |(v, _)| format!(r#","direction":"{v}""#));
        e.slide_command_json(&format!(
            r#"{{"cmd":"addAnimation","effect":"{}"{dir},"durMs":900}}"#,
            d.id
        ))
        .ok()?;
        if d.class == Class::EntranceExit {
            e.slide_command_json(&format!(
                r#"{{"cmd":"addAnimation","effect":"{}","class":"exit"{dir},"trigger":"afterPrevious"}}"#,
                d.id
            ))
            .ok()?;
        }
    }
    e.save_pptx().ok()
}

/// Text, drawn paths, colour and Morph by word (SLIDE-ANIM-003/004): slide 1 has a title faded
/// in word by word, a line flown in letter by letter, a shape on a motion path drawn by the
/// author (reversed and locked) and a fill colour change; slide 2 morphs into slide 3 word by word.
pub const ANIM_TEXT_DECK: &str = "slide-edit-anim-text-paths.pptx";

fn anim_text_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    let cmds = [
        r#"{"cmd":"addSlide"}"#,
        r#"{"cmd":"insertTextBox","text":"Doanh thu quý ba tăng mạnh","rect":{"x":60,"y":60,"w":600,"h":60}}"#,
        r#"{"cmd":"addAnimation","effect":"fade","textBy":"word","durMs":600}"#,
        r#"{"cmd":"insertTextBox","text":"Từng chữ một","rect":{"x":60,"y":160,"w":400,"h":50}}"#,
        r#"{"cmd":"addAnimation","effect":"flyIn","direction":"b","textBy":"letter","textDelayPct":20,"durMs":400,"trigger":"afterPrevious"}"#,
        r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":80,"y":300,"w":80,"h":80}}"#,
        r#"{"cmd":"addAnimation","effect":"customPath","path":"M 0 0 L 0.2 0.05 C 0.3 0.1 0.35 0.2 0.4 0.1 L 0.5 0 E","durMs":1500}"#,
        r#"{"cmd":"setAnimation","index":2,"reversePath":true,"lockPath":true}"#,
        r#"{"cmd":"insertShape","preset":"roundRect","rect":{"x":600,"y":300,"w":160,"h":90}}"#,
        r#"{"cmd":"addAnimation","effect":"fillColor","direction":"00B050"}"#,
        r#"{"cmd":"addSlide"}"#,
        r#"{"cmd":"insertTextBox","text":"Kế hoạch năm mới","rect":{"x":80,"y":80,"w":500,"h":60}}"#,
        r#"{"cmd":"addSlide"}"#,
        r#"{"cmd":"insertTextBox","text":"Năm mới kế hoạch lớn","rect":{"x":120,"y":200,"w":500,"h":60}}"#,
        r#"{"cmd":"setTransition","transition":{"kind":"morph","dir":"byWord"}}"#,
    ];
    for c in cmds {
        e.slide_command_json(c).ok()?;
    }
    e.save_pptx().ok()
}

/// Builds and effect options (SLIDE-ANIM-003/004): slide 2's bullets built by 2nd-level
/// paragraphs (in reverse, dimmed after), a column chart by series with its background first, a
/// SmartArt one node by one node, a shape hidden after its effect with a sound and another
/// rewinding and hidden on the next click; slide 6 morphs a rectangle into an ellipse.
pub const ANIM_BUILD_DECK: &str = "slide-edit-anim-build-elements.pptx";

fn anim_build_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    let wav = base64::engine::general_purpose::STANDARD.encode(sample_wav());
    let cmds = [
        r#"{"cmd":"goTo","slide":1}"#.to_owned(),
        r#"{"cmd":"select","ids":[3]}"#.into(),
        r#"{"cmd":"addAnimation","effect":"fade","durMs":400}"#.into(),
        r#"{"cmd":"setAnimation","index":0,"build":"p2","buildReverse":true}"#.into(),
        r#"{"cmd":"setAnimation","index":0,"after":"dim","dimColor":"A6A6A6"}"#.into(),
        r#"{"cmd":"addSlide"}"#.into(),
        r#"{"cmd":"insertChart","kind":"column","title":"Doanh thu","data":{"categories":["Q1","Q2","Q3"],"series":[{"name":"2025","values":[3,5,4]},{"name":"2026","values":[4,6,2]}]},"rect":{"x":60,"y":40,"w":560,"h":380}}"#.into(),
        r#"{"cmd":"addAnimation","effect":"wipe","direction":"b"}"#.into(),
        r#"{"cmd":"setAnimation","index":0,"build":"series","buildBg":true}"#.into(),
        r#"{"cmd":"addSlide"}"#.into(),
        r#"{"cmd":"insertSmartArt","layout":"process","items":[{"text":"Tiếp nhận"},{"text":"Xử lý"},{"text":"Trả kết quả"}]}"#.into(),
        r#"{"cmd":"addAnimation","effect":"zoom"}"#.into(),
        r#"{"cmd":"setAnimation","index":0,"build":"one","buildReverse":true}"#.into(),
        r#"{"cmd":"addSlide"}"#.into(),
        r#"{"cmd":"insertShape","preset":"roundRect","rect":{"x":80,"y":120,"w":200,"h":120}}"#.into(),
        r#"{"cmd":"addAnimation","effect":"flyIn","direction":"l"}"#.into(),
        format!(
            r#"{{"cmd":"setAnimation","index":0,"after":"hide","soundData":"{wav}","soundName":"chime.wav"}}"#
        ),
        r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":420,"y":120,"w":160,"h":160}}"#.into(),
        r#"{"cmd":"addAnimation","effect":"spin"}"#.into(),
        r#"{"cmd":"setAnimation","index":1,"rewind":true,"after":"hideOnClick"}"#.into(),
        r#"{"cmd":"addSlide"}"#.into(),
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":60,"y":60,"w":160,"h":100}}"#.into(),
        r#"{"cmd":"setText","text":"Hộp"}"#.into(),
        r#"{"cmd":"addSlide"}"#.into(),
        r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":300,"y":200,"w":200,"h":200}}"#.into(),
        r#"{"cmd":"setText","text":"Hộp"}"#.into(),
        r#"{"cmd":"setTransition","transition":{"kind":"morph","durMs":2500}}"#.into(),
    ];
    for c in &cmds {
        let r = e.slide_command_json(c).ok()?;
        if r.contains("\"error\":\"") {
            return None;
        }
    }
    e.save_pptx().ok()
}

/// The SmartArt layout library (CORE-SMART-003): one slide per layout of
/// `bo_pptx::diagram_layout::Layout`, each inserted through `insertSmartArt` (data model, layout
/// definition, quick style, colours and drawing parts), with sub-items, and the palettes and looks
/// in turn.
pub const SMARTART_LIBRARY_DECK: &str = "slide-edit-smartart-library.pptx";

fn smartart_library_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    use bo_pptx::diagram_layout::{Category, Layout};
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    let colors = ["colorful", "blue", "green", "orange", "purple", "gray"];
    for (i, layout) in Layout::ALL.into_iter().enumerate() {
        let items = match layout.category() {
            Category::Hierarchy => {
                r#"[{"text":"Giám đốc"},{"text":"Kỹ thuật","level":1},{"text":"Phát triển","level":2},{"text":"Kiểm thử","level":2},{"text":"Kinh doanh","level":1},{"text":"Bán hàng","level":2}]"#
            }
            Category::Picture => {
                r#"[{"text":"Hà Nội"},{"text":"Thủ đô","level":1},{"text":"Huế"},{"text":"Đà Nẵng"},{"text":"Cần Thơ"}]"#
            }
            _ => {
                r#"[{"text":"Khảo sát"},{"text":"Phỏng vấn","level":1},{"text":"Số liệu","level":1},{"text":"Thiết kế"},{"text":"Xây dựng"},{"text":"Kiểm thử"},{"text":"Vận hành"}]"#
            }
        };
        let style = if i % 5 == 4 { "outline" } else { "fill" };
        let cmds = [
            r#"{"cmd":"addSlide"}"#.to_owned(),
            format!(
                r#"{{"cmd":"insertSmartArt","layout":"{}","items":{items},"colors":"{}","style":"{style}"}}"#,
                layout.name(),
                colors[i % colors.len()]
            ),
        ];
        for c in &cmds {
            let r = e.slide_command_json(c).ok()?;
            if r.contains("\"error\":\"") {
                return None;
            }
        }
    }
    e.save_pptx().ok()
}

/// Diagram nodes with pictures (CORE-SMART-003): the two picture layouts with a wide, a tall and
/// a square picture and a node without one (its frame shows the placeholder mark), a layout of
/// another category whose nodes keep their pictures in the data model, and a picture layout
/// converted to shapes (picture fills with relationships of the slide). Each picture is a
/// picture fill (ECMA-376 Part 1, §20.1.8.14 blipFill) of the node's `dgm:spPr` and of its shape
/// in the drawing part, both parts with their own relationship to the image part.
pub const SMARTART_PICTURES_DECK: &str = "slide-edit-smartart-pictures.pptx";

fn smartart_pictures_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    let url = |w: u32, h: u32, tint: [u8; 3]| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(crate::slide_samples::sample_png_of(w, h, tint))
        )
    };
    let items = format!(
        r#"[{{"text":"Hà Nội","picture":"{}"}},{{"text":"Thủ đô","level":1}},{{"text":"Huế","picture":"{}"}},{{"text":"Đà Nẵng","picture":"{}"}},{{"text":"Cần Thơ"}}]"#,
        url(96, 32, [46, 117, 182]),
        url(32, 64, [192, 80, 30]),
        url(40, 40, [84, 130, 53]),
    );
    let mut cmds = Vec::new();
    for (layout, colors, style) in [
        ("picture", "colorful", "fill"),
        ("pictureStrip", "green", "fill"),
        ("pictureStrip", "purple", "outline"),
        // Not a picture layout: the pictures stay in the data model, the drawing shows none.
        ("process", "blue", "fill"),
        ("picture", "orange", "fill"),
    ] {
        cmds.push(r#"{"cmd":"addSlide"}"#.to_owned());
        cmds.push(format!(
            r#"{{"cmd":"insertSmartArt","layout":"{layout}","items":{items},"colors":"{colors}","style":"{style}"}}"#
        ));
    }
    // The last diagram becomes a group of shapes.
    cmds.push(r#"{"cmd":"smartArtToShapes"}"#.to_owned());
    for c in &cmds {
        let r = e.slide_command_json(c).ok()?;
        if r.contains("\"error\":\"") {
            return None;
        }
    }
    e.save_pptx().ok()
}

/// Assistant shapes and right-to-left diagrams (CORE-SMART-004): an organisation chart with an
/// assistant of the top shape and of a second-level shape (ST_PtType `asst`, ECMA-376 Part 1,
/// §21.4.3 `dgm:pt`), the same chart right to left, then a process, a cycle and a picture strip
/// laid out right to left (the layout variable `dgm:dir val="rev"`), each made with the Design
/// tab commands (`insertSmartArt`, `setSmartArt`).
pub const SMARTART_ASSISTANT_RTL_DECK: &str = "slide-edit-smartart-assistant-rtl.pptx";

fn smartart_assistant_rtl_deck(base: &[(&'static str, Vec<u8>)]) -> Option<Vec<u8>> {
    let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
    let mut e = Engine::with_bundled_fonts();
    e.open(b, "base.pptx").ok()?;
    let org = r#"[{"text":"Giám đốc"},{"text":"Thư ký","level":1,"assistant":true},{"text":"Kỹ thuật","level":1},{"text":"Trợ lý kỹ thuật","level":2,"assistant":true},{"text":"Phát triển","level":2},{"text":"Kinh doanh","level":1}]"#;
    let steps =
        r#"[{"text":"Tiếp nhận"},{"text":"Xử lý"},{"text":"Phê duyệt"},{"text":"Trả kết quả"}]"#;
    let mut cmds = Vec::new();
    for (layout, items, colors, rtl) in [
        ("hierarchy", org, "colorful", false),
        ("hierarchy", org, "blue", true),
        ("process", steps, "green", true),
        ("cycle", steps, "orange", true),
        ("pictureStrip", steps, "purple", true),
    ] {
        cmds.push(r#"{"cmd":"addSlide"}"#.to_owned());
        cmds.push(format!(
            r#"{{"cmd":"insertSmartArt","layout":"{layout}","items":{items},"colors":"{colors}","style":"fill"}}"#
        ));
        if rtl {
            cmds.push(r#"{"cmd":"setSmartArt","rtl":true}"#.to_owned());
        }
    }
    for c in &cmds {
        let r = e.slide_command_json(c).ok()?;
        if r.contains("\"error\":\"") {
            return None;
        }
    }
    e.save_pptx().ok()
}

/// Hyperlinks and actions (SLIDE-ACTION-001): linked text (slide, URL, mail) with a
/// custom show, a file link and a hover sound; action buttons on every slide;
/// actions BlueSlide never runs (program, macro, file, other deck, unknown).
pub const ACTION_DECKS: [&str; 3] = [
    "slide-action-links.pptx",
    "slide-action-buttons.pptx",
    "slide-action-unsafe.pptx",
];

/// Text of the linked text box of the first action deck and its three ranges
/// (characters): a slide link, a web link, a mail link.
pub const ACTION_TEXT: &str = "Xem trang 3 · Trang web Bluestar · Gửi thư";

fn action_decks(base: &[(&'static str, Vec<u8>)]) -> Vec<(&'static str, Vec<u8>)> {
    use base64::Engine as _;
    use bo_model::slide::{Rel, XmlElement, XmlNode};
    let wav = base64::engine::general_purpose::STANDARD.encode(sample_wav());
    let open = || {
        let (_, b) = base.iter().find(|(n, _)| *n == "slide-text-bullets.pptx")?;
        let mut e = Engine::with_bundled_fonts();
        e.open(b, "base.pptx").ok()?;
        // Four slides at least: the links jump between them.
        while e.presentation().map_or(4, |p| p.slides.len()) < 4 {
            e.slide_command_json(r#"{"cmd":"addSlide"}"#).ok()?;
        }
        e.slide_command_json(r#"{"cmd":"goTo","slide":0}"#).ok()?;
        Some(e)
    };
    let mut out = Vec::new();
    // 1. Linked text, a custom show, a file link, a hover sound.
    if let Some(mut e) = open() {
        let cmds = [
            format!(
                r#"{{"cmd":"insertTextBox","text":"{ACTION_TEXT}","rect":{{"x":60,"y":330,"w":560,"h":40}}}}"#
            ),
            r#"{"cmd":"textSelect","start":0,"end":11}"#.into(),
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"slide","slide":2},"tooltip":"Đi tới trang 3"}}"#.into(),
            r#"{"cmd":"textSelect","start":14,"end":32}"#.into(),
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"url","url":"https://example.vn/blueoffice"},"tooltip":"Mở trang web"}}"#.into(),
            r#"{"cmd":"textSelect","start":35,"end":42}"#.into(),
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"url","url":"mailto:lienhe@example.vn?subject=BlueSlide"}}}"#.into(),
            r#"{"cmd":"setCustomShow","name":"Phụ lục","slides":[2,3]}"#.into(),
            r#"{"cmd":"insertShape","preset":"roundRect","rect":{"x":60,"y":400,"w":150,"h":50}}"#.into(),
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"customShow","id":0,"return":true},"tooltip":"Xem phụ lục rồi quay lại"}}"#.into(),
            r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":240,"y":400,"w":150,"h":50}}"#.into(),
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"file","path":"bao-cao.docx"}}}"#.into(),
            r#"{"cmd":"insertShape","preset":"rect","rect":{"x":420,"y":400,"w":150,"h":50}}"#.into(),
            format!(
                r#"{{"cmd":"actionSet","on":"hover","spec":{{"target":{{"kind":"none"}},"highlight":true,"sound":{{"kind":"file","name":"chuông.wav","data":"{wav}"}}}}}}"#
            ),
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"lastViewed"}}}"#.into(),
        ];
        for c in &cmds {
            let _ = e.slide_command_json(c);
        }
        if let Ok(b) = e.save_pptx() {
            out.push((ACTION_DECKS[0], b));
        }
    }
    // 2. Action buttons with their usual jumps on every slide; a sound button.
    if let Some(mut e) = open() {
        let n = e.presentation().map_or(0, |p| p.slides.len());
        let buttons = [
            "actionButtonBeginning",
            "actionButtonBackPrevious",
            "actionButtonForwardNext",
            "actionButtonEnd",
            "actionButtonHome",
            "actionButtonReturn",
        ];
        for slide in 0..n {
            let _ = e.slide_command_json(&format!(r#"{{"cmd":"goTo","slide":{slide}}}"#));
            for (i, b) in buttons.iter().enumerate() {
                let _ = e.slide_command_json(&format!(
                    r#"{{"cmd":"actionButton","preset":"{b}","rect":{{"x":{},"y":470,"w":44,"h":44}}}}"#,
                    60 + 54 * i
                ));
            }
        }
        let _ = e.slide_command_json(r#"{"cmd":"goTo","slide":0}"#);
        let _ = e.slide_command_json(
            r#"{"cmd":"actionButton","preset":"actionButtonSound","rect":{"x":400,"y":470,"w":44,"h":44}}"#,
        );
        let _ = e.slide_command_json(&format!(
            r#"{{"cmd":"actionSet","spec":{{"target":{{"kind":"none"}},"highlight":true,"sound":{{"kind":"file","name":"bấm.wav","data":"{wav}"}}}}}}"#
        ));
        let _ = e.slide_command_json(
            r#"{"cmd":"actionButton","preset":"actionButtonInformation","rect":{"x":454,"y":470,"w":44,"h":44}}"#,
        );
        let _ = e.slide_command_json(
            r#"{"cmd":"actionSet","spec":{"target":{"kind":"url","url":"https://example.vn/huong-dan"},"sound":{"kind":"stop"}}}"#,
        );
        if let Ok(b) = e.save_pptx() {
            out.push((ACTION_DECKS[1], b));
        }
    }
    // 3. Actions that are kept and never run. The editor refuses to create them,
    // so they are written into the model (§21.1.2.3.5 hlinkClick, §21.1.2.3.6 hlinkHover).
    if let Some(mut e) = open() {
        for i in 0..6 {
            let _ = e.slide_command_json(&format!(
                r#"{{"cmd":"insertShape","preset":"rect","rect":{{"x":{},"y":400,"w":80,"h":50}}}}"#,
                40 + 90 * i
            ));
        }
        let made = e.save_pptx().ok().and_then(|b| bo_pptx::read_pptx(&b).ok());
        if let Some(mut p) = made {
            const HYPERLINK: &str =
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
            // (element, action, external target, tooltip)
            let actions: [(&str, Option<&str>, Option<&str>, &str); 6] = [
                (
                    "a:hlinkClick",
                    Some("ppaction://program"),
                    Some("file:///C:/Windows/System32/calc.exe"),
                    "Chương trình",
                ),
                (
                    "a:hlinkClick",
                    Some("ppaction://macro?name=TuDongChay"),
                    None,
                    "Macro",
                ),
                (
                    "a:hlinkClick",
                    Some("ppaction://hlinkfile"),
                    Some("bao-cao.xlsx"),
                    "Tệp",
                ),
                (
                    "a:hlinkClick",
                    Some("ppaction://hlinkpres?slideindex=2&slidetitle=Mở đầu"),
                    Some("bai-khac.pptx"),
                    "Bài trình chiếu khác",
                ),
                (
                    "a:hlinkHover",
                    Some("ppaction://frobnicate?x=1"),
                    None,
                    "Không rõ",
                ),
                (
                    "a:hlinkClick",
                    None,
                    Some("file:///C:/Windows/System32/cmd.exe"),
                    "URL file:",
                ),
            ];
            let slide = &mut p.slides[0];
            let first = slide.data.shapes.len().saturating_sub(actions.len());
            for (k, (name, action, target, tip)) in actions.iter().enumerate() {
                let mut rid = String::new();
                if let Some(t) = target {
                    rid = format!("rIdA{}", k + 1);
                    slide.info.rels.push(Rel {
                        id: rid.clone(),
                        rel_type: HYPERLINK.into(),
                        target: (*t).into(),
                        external: true,
                    });
                }
                let mut h = XmlElement::new(name);
                h.attrs.push(("r:id".into(), rid));
                if let Some(a) = action {
                    h.attrs.push(("action".into(), (*a).into()));
                }
                h.attrs.push(("tooltip".into(), (*tip).into()));
                let Some(n) = slide.data.shapes.get_mut(first + k) else {
                    continue;
                };
                let mut el = bo_pptx::write_node(n);
                // cNvPr is the first child of the nv*Pr element.
                if let Some(XmlNode::Element(nv)) = el.children.first_mut() {
                    if let Some(XmlNode::Element(cnv)) = nv.children.first_mut() {
                        cnv.children.push(XmlNode::Element(h));
                    }
                }
                *n = bo_pptx::parse_node(&el);
            }
            if let Ok(b) = bo_pptx::write_pptx(&p) {
                out.push((ACTION_DECKS[2], b));
            }
        }
    }
    out
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
    if let Some(d) = transitions_deck(base) {
        out.push((TRANSITIONS_DECK, d));
    }
    if let Some(d) = animations_deck(base) {
        out.push((ANIMATIONS_DECK, d));
    }
    if let Some(d) = anim_text_deck(base) {
        out.push((ANIM_TEXT_DECK, d));
    }
    if let Some(d) = anim_build_deck(base) {
        out.push((ANIM_BUILD_DECK, d));
    }
    if let Some(d) = smartart_library_deck(base) {
        out.push((SMARTART_LIBRARY_DECK, d));
    }
    if let Some(d) = smartart_pictures_deck(base) {
        out.push((SMARTART_PICTURES_DECK, d));
    }
    if let Some(d) = smartart_assistant_rtl_deck(base) {
        out.push((SMARTART_ASSISTANT_RTL_DECK, d));
    }
    out.extend(recording_decks(base));
    out.extend(action_decks(base));
    if let Some((_, b)) = base.iter().find(|(n, _)| *n == "slide-shapes-presets.pptx") {
        for (name, h, v) in FLIPPED {
            if let Some(d) = flipped_connectors(b, *h, *v) {
                out.push((name, d));
            }
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
        assert_eq!(
            decks.len(),
            SCRIPTS.len() + FLIPPED.len() + 6 + RECORDING_DECKS.len() + ACTION_DECKS.len()
        );
        for (name, bytes) in decks {
            let p = bo_pptx::read_pptx(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!p.slides.is_empty(), "{name}");
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    // source: ECMA-376 Part 1, §19.3.1.50 transition, §19.3.1.42 sndAc; [MS-PPTX] 2.3 p14 / p15 —
    // every catalog transition with its last variant is read back as itself after saving.
    // source: ECMA-376 Part 1, §21.4 (dgm:relIds, dataModel, layoutDef) — CORE-SMART-003: every
    // layout of the library is written with its five parts and reads back as the same layout.
    #[test]
    fn every_smartart_layout_survives_saving() {
        use bo_pptx::diagram_layout::{self as dl, Layout};
        let base = crate::slide_samples::base_decks();
        let bytes = smartart_library_deck(&base).expect("deck");
        let mut e = Engine::with_bundled_fonts();
        e.open(&bytes, "library.pptx").unwrap();
        let p = e.presentation().unwrap();
        let mut seen = Vec::new();
        for s in &p.slides {
            let rel = |ty: &str| {
                s.info
                    .rels
                    .iter()
                    .find(|r| r.rel_type.ends_with(ty))
                    .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
            };
            let Some(data) = rel("/diagramData") else {
                continue;
            };
            let layout = rel("/diagramLayout").unwrap();
            for ty in ["/diagramQuickStyle", "/diagramColors", "/diagramDrawing"] {
                assert!(p.part_data(&rel(ty).unwrap()).is_some(), "{ty}");
            }
            let (nodes, lo) = dl::parse_data_model(p.part_data(&data).unwrap(), &data).unwrap();
            assert_ne!(nodes.len(), 0);
            let l = dl::layout_of(lo.as_deref(), p.part_data(&layout));
            // The layout part names the category (§21.4.2.3 cat) for other applications.
            assert_eq!(dl::category(None, p.part_data(&layout)), l.category());
            let drawing = rel("/diagramDrawing").unwrap();
            let shapes = bo_pptx::diagram::drawing_shapes(p.part_data(&drawing).unwrap(), &drawing);
            assert!(shapes.unwrap().len() >= nodes.len(), "{l:?}");
            seen.push(l);
        }
        assert_eq!(seen, Layout::ALL);
    }

    // source: ECMA-376 Part 1, §21.4.3 dgm:pt (ST_PtType `asst`) and the layout variable
    // `dgm:dir` (`rev`): the assistant and right-to-left deck reads back with two assistants on
    // each organisation chart and four diagrams right to left, the outline unchanged.
    #[test]
    fn assistant_and_right_to_left_deck_reads_back() {
        use bo_pptx::diagram_layout as dl;
        /// (nodes, assistants) of an outline.
        fn count(n: &[dl::DiagramNode]) -> (usize, usize) {
            n.iter().fold((0, 0), |(all, asst), x| {
                let (a, b) = count(&x.children);
                (all + 1 + a, asst + usize::from(x.assistant) + b)
            })
        }
        let base = crate::slide_samples::base_decks();
        let bytes = smartart_assistant_rtl_deck(&base).expect("deck");
        let mut e = Engine::with_bundled_fonts();
        e.open(&bytes, "assistant-rtl.pptx").unwrap();
        let p = e.presentation().unwrap();
        let mut got = Vec::new();
        for s in &p.slides {
            let Some(data) = s
                .info
                .rels
                .iter()
                .find(|r| r.rel_type.ends_with("/diagramData"))
                .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
            else {
                continue;
            };
            let bytes = p.part_data(&data).unwrap();
            let (nodes, _) = dl::parse_data_model(bytes, &data).unwrap();
            let (all, assistants) = count(&nodes);
            got.push((all, assistants, dl::look_of(bytes, &data).rtl));
        }
        assert_eq!(
            got,
            [
                (6, 2, false),
                (6, 2, true),
                (4, 0, true),
                (4, 0, true),
                (4, 0, true)
            ]
        );
    }

    #[test]
    fn every_transition_survives_saving() {
        let base = crate::slide_samples::base_decks();
        let b = transitions_deck(&base).expect("deck");
        let mut e = Engine::with_bundled_fonts();
        e.open(&b, "transitions.pptx").unwrap();
        // Each added slide follows the current one: the transitions are slides 1 to N.
        let first = 1;
        for (i, d) in bo_pptx::transitions::CATALOG.iter().enumerate() {
            let r: serde_json::Value = serde_json::from_str(
                &e.slide_command_json(&format!(r#"{{"cmd":"goTo","slide":{}}}"#, first + i))
                    .unwrap(),
            )
            .unwrap();
            let t = &r["transition"];
            assert_eq!(t["kind"], d.id, "{}", d.id);
            assert_eq!(t["known"], true, "{}", d.id);
            let last = bo_pptx::transitions::variants(d).last().copied();
            assert_eq!(t["dir"].as_str(), last, "{}", d.id);
            assert_eq!(t["durMs"], 800 + 50 * (i % 10) as u64, "{}", d.id);
        }
        let r: serde_json::Value =
            serde_json::from_str(&e.slide_command_json(r#"{"cmd":"goTo","slide":1}"#).unwrap())
                .unwrap();
        let snd = &r["transition"]["sound"];
        assert_eq!(snd["name"], "chuông.wav");
        let part = snd["part"].as_str().unwrap();
        assert!(e
            .presentation()
            .unwrap()
            .part_data(part)
            .is_some_and(|d| d.starts_with(b"RIFF")));
        let r: serde_json::Value =
            serde_json::from_str(&e.slide_command_json(r#"{"cmd":"goTo","slide":2}"#).unwrap())
                .unwrap();
        assert_eq!(r["transition"]["sound"]["stop"], true);
        // A sound that is not WAV is refused.
        let r: serde_json::Value = serde_json::from_str(
            &e.slide_command_json(
                r#"{"cmd":"setTransition","transition":{"kind":"keep","soundData":"SUQzBAAAAAAA"}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(r["error"], "badSound");
    }
}

#[cfg(test)]
mod animation_tests {
    use super::*;

    // source: ECMA-376 Part 1, §19.5 — every catalog effect is read back as itself after saving,
    // with its variant and class, and plays (the engine reports its tracks).
    #[test]
    fn every_effect_survives_saving() {
        use bo_pptx::anim_catalog::{Class, EFFECTS};
        let base = crate::slide_samples::base_decks();
        let b = animations_deck(&base).expect("deck");
        let mut e = Engine::with_bundled_fonts();
        e.open(&b, "animations.pptx").unwrap();
        for (i, d) in EFFECTS.iter().enumerate() {
            let r: serde_json::Value = serde_json::from_str(
                &e.slide_command_json(&format!(r#"{{"cmd":"goTo","slide":{}}}"#, i + 1))
                    .unwrap(),
            )
            .unwrap();
            let a = &r["animations"];
            let want = match d.class {
                Class::EntranceExit => vec!["entr", "exit"],
                Class::Emphasis => vec!["emph"],
                Class::Path => vec!["path"],
            };
            assert_eq!(a.as_array().map(Vec::len), Some(want.len()), "{}", d.id);
            for (k, cls) in want.iter().enumerate() {
                assert_eq!(a[k]["effect"], d.id, "{} {cls}", d.id);
                assert_eq!(a[k]["class"], *cls, "{}", d.id);
                assert_eq!(
                    a[k]["direction"].as_str(),
                    d.dirs.last().map(|v| v.0),
                    "{}",
                    d.id
                );
                assert!(
                    a[k]["play"]["tracks"]
                        .as_array()
                        .is_some_and(|t| !t.is_empty())
                        || d.id == "appear",
                    "{}",
                    d.id
                );
            }
        }
    }

    // source: ECMA-376 Part 1, §19.5 bldLst (bldP build p / bldLvl 2 / rev, bldGraphic with
    // bldChart series + animBg and bldDgm one + rev), spTgt sub-targets, after effects, fill
    // remove and sndTgt — the deck reads back with every option; slide 2 has 5 groups of
    // paragraphs (levels 1 2 2 3 1 2 4 5), the chart 2 series after its background.
    #[test]
    fn builds_and_effect_options_survive_saving() {
        let base = crate::slide_samples::base_decks();
        let b = anim_build_deck(&base).expect("deck");
        let mut e = Engine::with_bundled_fonts();
        e.open(&b, "anim-build.pptx").unwrap();
        let p = e.presentation().unwrap();
        let t = |i: usize| p.slides[i].timing.clone().unwrap();
        let s1 = t(1);
        assert_eq!(s1.animations.len(), 5);
        assert!(s1.animations.iter().all(|a| matches!(
            a.opts.after,
            Some(bo_model::slide::AnimAfter::Dim {
                rgb: 0xA6_A6_A6,
                ..
            })
        )));
        assert!(
            matches!(&s1.builds[..], [bo_model::slide::Build::Paragraph(pb)]
            if pb.build == "p" && pb.level == 2 && pb.reverse)
        );
        let s2 = t(2);
        assert_eq!(s2.animations.len(), 3);
        assert!(matches!(
            &s2.animations[0].opts.target,
            Some(bo_model::slide::AnimTarget::Chart { step, .. }) if step == "gridLegend"
        ));
        assert_eq!(t(3).animations.len(), 3);
        let s4 = t(4);
        assert_eq!(
            s4.animations[0].opts.after,
            Some(bo_model::slide::AnimAfter::Hide)
        );
        assert_eq!(
            s4.animations[0].opts.sound.as_ref().unwrap().name,
            "chime.wav"
        );
        assert!(s4.animations[1].opts.rewind);
        assert_eq!(
            s4.animations[1].opts.after,
            Some(bo_model::slide::AnimAfter::HideOnClick)
        );
        // 2 clicks of effects and one hiding the ellipse.
        assert_eq!(s4.click_count(), 3);
        let r: serde_json::Value =
            serde_json::from_str(&e.slide_command_json(r#"{"cmd":"show","slide":6}"#).unwrap())
                .unwrap();
        let pairs = r["show"]["morph"]["pairs"].as_array().unwrap();
        assert!(pairs.iter().any(|p| p["outline"].is_object()));
    }

    // source: ECMA-376 Part 1, §19.5 iterate (word / letter, tmPct), §19.5.4 animMotion (path,
    // pathEditMode), §19.5.2 animClr; [MS-PPTX] p159:morph option byWord. Expected values: the
    // text has 6 words / 10 letters; a step is tmPct × duration.
    #[test]
    fn text_paths_colours_and_morph_by_word_survive_saving() {
        let base = crate::slide_samples::base_decks();
        let b = anim_text_deck(&base).expect("deck");
        let mut e = Engine::with_bundled_fonts();
        e.open(&b, "anim-text.pptx").unwrap();
        let cmd = |e: &mut Engine, c: &str| -> serde_json::Value {
            serde_json::from_str(&e.slide_command_json(c).unwrap()).unwrap()
        };
        let r = cmd(&mut e, r#"{"cmd":"goTo","slide":1}"#);
        let a = r["animations"].as_array().unwrap().clone();
        assert_eq!(a.len(), 4, "{a:?}");
        // "Doanh thu quý ba tăng mạnh": 6 words, 10 % of 600 ms apart → 600 + 5 × 60 ms.
        assert_eq!(a[0]["textBy"], "wd");
        assert_eq!(a[0]["textStepMs"], 60);
        assert_eq!(
            a[0]["endMs"].as_u64().unwrap() - a[0]["startMs"].as_u64().unwrap(),
            900
        );
        // "Từng chữ một": 10 letters, 20 % of 400 ms apart → 400 + 9 × 80 ms, after the title.
        assert_eq!(a[1]["textBy"], "lt");
        assert_eq!(a[1]["textStepMs"], 80);
        assert_eq!(a[1]["startMs"], 900);
        assert_eq!(a[1]["endMs"], 900 + 400 + 720);
        // The drawn path, reversed (from its end back to the shape) and locked.
        assert_eq!(a[2]["effect"], "customPath");
        assert_eq!(a[2]["class"], "path");
        assert_eq!(a[2]["pathLocked"], true);
        assert_eq!(
            a[2]["path"],
            "M 0.5 0 L 0.4 0.1 C 0.35 0.2 0.3 0.1 0.2 0.05 L 0 0 E"
        );
        assert_eq!(a[2]["durMs"], 1500);
        assert_eq!(a[3]["effect"], "fillColor");
        // The show: letters for the iterated layers, the green drawing for the colour change.
        let s = cmd(&mut e, r#"{"cmd":"show","slide":1}"#)["show"].clone();
        let layer =
            |i: usize| s["layers"][s["animations"][i]["layer"].as_u64().unwrap() as usize].clone();
        assert_eq!(layer(0)["text"]["words"], 6);
        assert_eq!(layer(1)["text"]["pieces"].as_array().unwrap().len(), 10);
        assert!(layer(2)["text"].is_null());
        let tint = s["animations"][3]["tint"]["commands"].to_string();
        // 00B050 = (0, 176, 80).
        assert!(tint.contains("[0,176,80,255]"), "{tint}");
        assert!(!layer(3)["page"]["commands"]
            .to_string()
            .contains("[0,176,80,255]"));
        // Morph by word: the words found again move on their own; the others fade.
        let s = cmd(&mut e, r#"{"cmd":"show","slide":3}"#)["show"].clone();
        let m = &s["morph"];
        assert_eq!(m["option"], "byWord");
        let words = m["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| !p["clip"].is_null())
            .count();
        // "Kế hoạch năm mới" → "Năm mới kế hoạch lớn": "hoạch", "mới" in reading order.
        assert_eq!(words, 2, "{}", m["pairs"]);
        assert_eq!(m["goneText"].as_array().map(Vec::len), Some(2));
        assert_eq!(m["newText"].as_array().map(Vec::len), Some(3));
        // Saved and read back: the same iterate, path and lock.
        let back = Engine::with_bundled_fonts();
        let mut back = back;
        back.open(&e.save_pptx().unwrap(), "back.pptx").unwrap();
        let r = cmd(&mut back, r#"{"cmd":"goTo","slide":1}"#);
        let b = r["animations"].as_array().unwrap();
        for k in [
            "textBy",
            "textStepMs",
            "path",
            "pathLocked",
            "effect",
            "endMs",
        ] {
            for i in 0..4 {
                assert_eq!(a[i][k], b[i][k], "{k} {i}");
            }
        }
        // Back to one object; a new duration scales the drawn path's behaviour.
        let r = cmd(
            &mut back,
            r#"{"cmd":"setAnimation","index":0,"textBy":"all"}"#,
        );
        assert!(r["animations"][0]["textBy"].is_null());
        let r = cmd(
            &mut back,
            r#"{"cmd":"setAnimation","index":2,"durMs":3000}"#,
        );
        assert_eq!(r["animations"][2]["play"]["durMs"], 3000.0);
        let r = cmd(
            &mut back,
            r#"{"cmd":"setAnimation","index":2,"path":"M 0 0 L x"}"#,
        );
        assert_eq!(r["error"], "badPath");
        // Locked: moving the shape a tenth of the slide right leaves the path where it was on the
        // slide (its points a tenth to the left); unlocked, the path moves with the shape.
        let id = a[2]["shapeId"].as_u64().unwrap();
        cmd(&mut back, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
        let mv = r#"{"cmd":"transform","transform":{"kind":"move","dx":96,"dy":0},"snap":false}"#;
        let r = cmd(&mut back, mv);
        assert_eq!(
            r["animations"][2]["path"],
            "M 0.4 0 L 0.3 0.1 C 0.25 0.2 0.2 0.1 0.1 0.05 L -0.1 0 E"
        );
        cmd(
            &mut back,
            r#"{"cmd":"setAnimation","index":2,"lockPath":false}"#,
        );
        let r = cmd(&mut back, mv);
        assert_eq!(r["animations"][2]["pathLocked"], serde_json::Value::Null);
        assert_eq!(
            r["animations"][2]["path"],
            "M 0.4 0 L 0.3 0.1 C 0.25 0.2 0.2 0.1 0.1 0.05 L -0.1 0 E"
        );
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;

    fn deck_named(name: &str) -> bo_model::slide::Presentation {
        let base = crate::slide_samples::base_decks();
        let decks = decks(&base, &crate::slide_samples::sample_png());
        let (_, b) = decks.iter().find(|(n, _)| *n == name).expect(name);
        bo_pptx::read_pptx(b).unwrap()
    }

    // source: ECMA-376 Part 1, §19.2.1.39 sldSz; Part 2, §9.3 relationships
    #[test]
    fn pasted_slides_fit_carry_charts_and_remap_links() {
        // 4:3 → target size: everything inside the target slide.
        let p = deck_named(PASTE_RESIZED);
        let (w, h) = (p.slide_size.cx, p.slide_size.cy);
        for n in &p.slides[1].data.shapes {
            if let bo_model::slide::ShapeNode::Shape(s) = n {
                if let Some(x) = &s.props.xfrm {
                    let (ox, oy) = x.off.unwrap_or_default();
                    let (cx, cy) = x.ext.unwrap_or_default();
                    assert!(ox >= 0 && oy >= 0 && ox + cx <= w && oy + cy <= h, "{x:?}");
                }
            }
        }
        // Chart: the chart part, its rels and the workbook were copied.
        let p = deck_named(PASTE_CHART);
        let charts: Vec<&str> = p
            .parts
            .iter()
            .map(|x| x.name.as_str())
            .filter(|n| n.starts_with("/ppt/charts/chart"))
            .collect();
        assert_eq!(charts.len(), 1, "{charts:?}");
        let rels = p
            .parts
            .iter()
            .find(|x| x.name == bo_opc::rels_part_name(charts[0]))
            .expect("chart rels");
        let r = bo_engine::slide_edit::clip::rels_of(&rels.data);
        let wb = bo_opc::resolve_target(charts[0], &r[0].2);
        assert!(p.parts.iter().any(|x| x.name == wb), "{wb}");
        // Links: 1 → 0 retargeted, 0 → 2 (not pasted) removed.
        let p = deck_named(PASTE_LINKS);
        let slide_rels = |i: usize| {
            p.slides[i]
                .info
                .rels
                .iter()
                .filter(|r| r.rel_type.ends_with("/slide"))
                .count()
        };
        assert_eq!(slide_rels(1), 0);
        let link = p.slides[2]
            .info
            .rels
            .iter()
            .find(|r| r.rel_type.ends_with("/slide"))
            .expect("link kept");
        assert_eq!(
            bo_opc::resolve_target(&p.slides[2].info.part_name, &link.target),
            p.slides[1].info.part_name
        );
    }
}
