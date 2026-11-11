//! SmartArt in BlueDoc (CORE-SMART-005): insert a diagram from the BlueOffice layout library (the
//! same as BlueSlide's, `bo_pptx::diagram_layout`) inline or floating at the selection, read the
//! selected one's text outline, layout, colours and look (the text pane), and change them. Every
//! edit writes all five parts (ECMA-376 Part 1, §21.4 data model, layout definition, quick style,
//! colours; [MS-ODRAWXML] drawing) under fresh names and points the frame at them, so it is one
//! undo step and the frame's picture changes with it. A diagram read from the file keeps its
//! parts verbatim until it is edited.

use crate::edit::EditResult;
use crate::slide_edit::smartart::SmartItem;
use crate::smartart::Pictures;
use crate::{EngineError, Result};
use bo_model::diagram::{self, DRAWING_REL_PLACEHOLDER};
use bo_model::{Anchor, Emu, Image, Wrap};
use bo_pptx::diagram_layout::{self as dl, Fill, Layout, Look, Palette};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DIR: &str = "/word/diagrams";
const EMU_PT: f64 = 12_700.0;

/// SmartArt commands of BlueDoc (`cmd` = `insertSmartArt` | `setSmartArt` | `smartArtInfo`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum DocSmartArtCommand {
    /// Insert a SmartArt at the selection.
    InsertSmartArt {
        /// Layout of the BlueOffice library (`list`, `processChevron`, `picture`, …).
        layout: String,
        /// Text outline.
        items: Vec<SmartItem>,
        /// Colours (`colorful`, `blue`, …).
        #[serde(default)]
        colors: Option<String>,
        /// Look (`fill` | `outline`).
        #[serde(default)]
        style: Option<String>,
        /// Width in points (default: the text width).
        #[serde(default)]
        width: Option<f64>,
        /// Height in points (default: 9/16 of the width).
        #[serde(default)]
        height: Option<f64>,
        /// Float with square wrapping (§20.4.2.3 anchor) instead of inline (§20.4.2.8).
        #[serde(default)]
        floating: bool,
    },
    /// Change the SmartArt at the selection (`None` keeps a value).
    SetSmartArt {
        /// New layout.
        #[serde(default)]
        layout: Option<String>,
        /// New text outline.
        #[serde(default)]
        items: Option<Vec<SmartItem>>,
        /// New colours.
        #[serde(default)]
        colors: Option<String>,
        /// New look.
        #[serde(default)]
        style: Option<String>,
        /// Lay out right to left (`None` keeps the direction).
        #[serde(default)]
        rtl: Option<bool>,
    },
    /// The SmartArt at the selection (for the text pane).
    SmartArtInfo,
    /// Replace the SmartArt at the selection by a group of ordinary shapes drawn as it shows
    /// (a `wpg:wgp` when saved), one undo step.
    SmartArtToShapes,
    /// Thumbnails of every layout of the library: answers `{ previews: [{ layout, svg }] }`.
    SmartArtPreviews {
        /// Colours (default `colorful`).
        #[serde(default)]
        colors: Option<String>,
        /// Look (default `fill`).
        #[serde(default)]
        style: Option<String>,
        /// Thumbnail width in points (default 96).
        #[serde(default)]
        width: Option<f64>,
        /// Thumbnail height in points (default 64).
        #[serde(default)]
        height: Option<f64>,
        /// Draw this text outline instead of sample nodes (live preview of a layout).
        #[serde(default)]
        items: Option<Vec<SmartItem>>,
        /// Right to left.
        #[serde(default)]
        rtl: Option<bool>,
    },
}

impl DocSmartArtCommand {
    /// Parses a JSON command when it is a BlueDoc SmartArt command.
    pub fn parse(v: &serde_json::Value) -> Option<DocSmartArtCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !matches!(
            c,
            "insertSmartArt"
                | "setSmartArt"
                | "smartArtInfo"
                | "smartArtToShapes"
                | "smartArtPreviews"
        ) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// The SmartArt at the selection as the text pane shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocSmartArt {
    /// Layout.
    pub layout: String,
    /// Text outline.
    pub items: Vec<SmartItem>,
    /// Colours.
    pub colors: String,
    /// Look.
    pub style: String,
    /// Laid out right to left.
    pub rtl: bool,
    /// Frame width in points.
    pub width: f64,
    /// Frame height in points.
    pub height: f64,
    /// Floating (anchored) frame.
    pub floating: bool,
}

fn look(
    base: Look,
    colors: Option<&str>,
    style: Option<&str>,
) -> std::result::Result<Look, String> {
    Ok(Look {
        palette: match colors {
            Some(c) => Palette::parse(c).ok_or("badSmartArt")?,
            None => base.palette,
        },
        fill: match style {
            Some(s) => Fill::parse(s).ok_or("badSmartArt")?,
            None => base.fill,
        },
        ..base
    })
}

/// Alternative text of a diagram: its node texts.
fn alt_text(nodes: &[dl::DiagramNode]) -> String {
    dl::outline_of(nodes)
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The five parts of a diagram under `names` (in [`diagram::DiagramPart::ALL`] order).
fn parts_of(
    names: &[String; 5],
    p: dl::DiagramParts,
) -> impl Iterator<Item = (String, Arc<Vec<u8>>)> + '_ {
    names
        .iter()
        .cloned()
        .zip([p.data, p.layout, p.style, p.colors, p.drawing].map(Arc::new))
}

/// A diagram as read from its parts: outline, layout, look and the parts themselves.
struct Read {
    nodes: Vec<dl::DiagramNode>,
    layout: Layout,
    look: Look,
    src: bo_docx::diagram::DiagramSource,
}

/// Read the outline, layout and look of a diagram's data model.
fn read(doc: &bo_model::Document, img: &Image) -> Option<Read> {
    let src = bo_docx::diagram::diagram_source(doc, img)?;
    let (name, data) = &src.data;
    let (nodes, lo) = dl::parse_data_model(data, name)?;
    let layout = dl::layout_of(lo.as_deref(), src.layout.as_ref().map(|l| l.1.as_slice()));
    Some(Read {
        nodes,
        layout,
        look: dl::look_of(data, name),
        src,
    })
}

/// New image parts of a command: names no media part or part of the source package has.
struct NewMedia {
    taken: Vec<String>,
    start: usize,
    added: Vec<(String, Arc<Vec<u8>>)>,
}

impl NewMedia {
    fn new(s: &bo_edit::Session) -> NewMedia {
        let mut taken = bo_docx::diagram::source_part_names(s.doc());
        taken.extend(s.doc().media.keys().cloned());
        NewMedia {
            start: s.part_number_start(s.doc().media.len() + 1),
            taken,
            added: Vec::new(),
        }
    }

    fn store(&mut self, pic: crate::smartart::NewPicture) -> String {
        let mut n = self.start;
        let name = loop {
            let cand = format!("/word/media/image{n}.{}", pic.ext);
            if !self.taken.contains(&cand) {
                break cand;
            }
            n += 1;
        };
        self.taken.push(name.clone());
        self.added.push((name.clone(), Arc::new(pic.data)));
        name
    }

    fn bytes(&self, name: &str) -> Option<&Arc<Vec<u8>>> {
        self.added.iter().find(|(n, _)| n == name).map(|(_, b)| b)
    }
}

/// The relationships parts (as media) of the data model and drawing parts `names[0]` /
/// `names[4]` for `pictures`.
fn rels_parts(names: &[String; 5], pictures: &Pictures) -> Vec<(String, Arc<Vec<u8>>)> {
    if pictures.is_empty() {
        return Vec::new();
    }
    [&names[0], &names[4]]
        .iter()
        .map(|part| {
            (
                bo_docx::diagram::rels_name(part),
                Arc::new(bo_pptx::diagram::image_rels_xml(part, pictures)),
            )
        })
        .collect()
}

impl crate::Engine {
    /// The SmartArt at the selection of the open document.
    pub fn doc_smartart(&self) -> Option<DocSmartArt> {
        let s = self.session.as_ref()?;
        let img = s.smartart()?;
        let r = read(s.doc(), &img)?;
        Some(DocSmartArt {
            layout: r.layout.name().to_owned(),
            items: crate::smartart::items_of(&r.nodes),
            colors: r.look.palette.name().to_owned(),
            style: r.look.fill.name().to_owned(),
            rtl: r.look.rtl,
            width: img.width.0 as f64 / EMU_PT,
            height: img.height.0 as f64 / EMU_PT,
            floating: img.anchor.is_some(),
        })
    }

    /// Run a BlueDoc SmartArt command; the result is the edit result plus `smartArt` (the SmartArt
    /// at the selection afterwards). `smartArtPreviews` answers `{ previews }` only.
    #[allow(clippy::too_many_lines)]
    pub fn doc_smartart_command(&mut self, cmd: DocSmartArtCommand) -> Result<serde_json::Value> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        if let DocSmartArtCommand::SmartArtPreviews {
            colors,
            style,
            width,
            height,
            items,
            rtl,
        } = &cmd
        {
            let mut lk = look(Look::default(), colors.as_deref(), style.as_deref())
                .map_err(EngineError::BadCommand)?;
            lk.rtl = rtl.unwrap_or(false);
            return Ok(serde_json::json!({
                "previews": crate::smartart::layout_previews_of(
                    lk,
                    items.as_deref(),
                    width.unwrap_or(96.0),
                    height.unwrap_or(64.0),
                    &mut self.fonts,
                ),
            }));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let res: std::result::Result<(), String> = match cmd {
            DocSmartArtCommand::SmartArtInfo | DocSmartArtCommand::SmartArtPreviews { .. } => {
                Ok(())
            }
            _ if read_only => Err(crate::security::READ_ONLY.into()),
            DocSmartArtCommand::InsertSmartArt {
                layout,
                items,
                colors,
                style,
                width,
                height,
                floating,
            } => (|| {
                let cat = Layout::parse(&layout).ok_or("badSmartArt")?;
                let lk = look(Look::default(), colors.as_deref(), style.as_deref())?;
                if items.iter().all(|i| i.text.trim().is_empty()) {
                    return Err("badSmartArt".to_owned());
                }
                let mut media = NewMedia::new(s);
                let (nodes, pictures) =
                    crate::smartart::nodes_of(&items, &Vec::new(), &mut |p| Ok(media.store(p)))?;
                if nodes.is_empty() {
                    return Err("badSmartArt".to_owned());
                }
                // Default size: the text width of the first section, 9:16.
                let text_w = s.doc().sections.first().map_or(453.0, |sec| {
                    (sec.page.width.0 - sec.page.margin_left.0 - sec.page.margin_right.0) as f64
                        / 20.0
                });
                let w = width.unwrap_or(text_w).clamp(36.0, 1_500.0);
                let h = height.unwrap_or(w * 9.0 / 16.0).clamp(36.0, 1_500.0);
                let (cx, cy) = ((w * EMU_PT).round() as i64, (h * EMU_PT).round() as i64);
                let taken = bo_docx::diagram::source_diagram_parts(s.doc());
                let names = diagram::part_names(DIR, s.new_diagram_number(DIR, &taken));
                let size = |rid: &str| {
                    let (_, name) = pictures.iter().find(|(id, _)| id == rid)?;
                    crate::edit::image_size(media.bytes(name)?)
                };
                let styling = dl::Styling {
                    look: lk,
                    defs: None,
                    picture_size: Some(&size),
                };
                let built =
                    dl::build_parts_with(&nodes, cat, &styling, (cx, cy), DRAWING_REL_PLACEHOLDER);
                let img = Image {
                    media: names[4].clone(),
                    width: Emu(cx),
                    height: Emu(cy),
                    // §20.4.2.3 anchor with §20.4.2.17 wrapSquare.
                    anchor: floating.then(|| Anchor {
                        h_relative: "column".into(),
                        v_relative: "paragraph".into(),
                        wrap: Wrap::Square,
                        dist: [Emu(0), Emu(0), Emu(114_300), Emu(114_300)],
                        ..Anchor::default()
                    }),
                    crop: None,
                    rotation: 0,
                    description: alt_text(&nodes),
                    shape: None,
                    format: None,
                    src: None,
                    ole: None,
                };
                let mut parts: Vec<(String, Arc<Vec<u8>>)> = parts_of(&names, built).collect();
                parts.extend(rels_parts(&names, &pictures));
                parts.extend(media.added.iter().cloned());
                s.insert_smartart(parts, img)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })(),
            DocSmartArtCommand::SetSmartArt {
                layout,
                items,
                colors,
                style,
                rtl,
            } => (|| {
                let img = s.smartart().ok_or("noSmartArt")?;
                let old = read(s.doc(), &img).ok_or("noSmartArt")?;
                let cat = match layout.as_deref() {
                    Some(l) => Layout::parse(l).ok_or("badSmartArt")?,
                    None => old.layout,
                };
                let mut lk = look(old.look, colors.as_deref(), style.as_deref())?;
                lk.rtl = rtl.unwrap_or(lk.rtl);
                let mut media = NewMedia::new(s);
                let (nodes, pictures) = if let Some(i) = items.as_deref() {
                    crate::smartart::nodes_of(i, &old.src.pictures, &mut |p| Ok(media.store(p)))?
                } else {
                    let used = crate::smartart::used_pictures(&old.nodes, &old.src.pictures);
                    (old.nodes.clone(), used)
                };
                if nodes.is_empty() {
                    return Err("badSmartArt".to_owned());
                }
                let image = |name: &str| {
                    media
                        .bytes(name)
                        .or_else(|| s.doc().media.get(name))
                        .or_else(|| {
                            old.src
                                .images
                                .iter()
                                .find(|(n, _)| n == name)
                                .map(|(_, b)| b)
                        })
                        .cloned()
                };
                let size = |rid: &str| {
                    let (_, name) = pictures.iter().find(|(id, _)| id == rid)?;
                    crate::edit::image_size(&image(name)?)
                };
                let defs = dl::Defs::parse(
                    old.src.style.as_ref().map(|p| p.1.as_slice()),
                    old.src.colors.as_ref().map(|p| p.1.as_slice()),
                );
                let styling = dl::Styling {
                    look: lk,
                    defs: Some(&defs),
                    picture_size: Some(&size),
                };
                let taken = bo_docx::diagram::source_diagram_parts(s.doc());
                let names = diagram::part_names(DIR, s.new_diagram_number(DIR, &taken));
                let built = dl::build_parts_with(
                    &nodes,
                    cat,
                    &styling,
                    (img.width.0, img.height.0),
                    DRAWING_REL_PLACEHOLDER,
                );
                let mut parts: Vec<(String, Option<Arc<Vec<u8>>>)> =
                    parts_of(&names, built).map(|(n, b)| (n, Some(b))).collect();
                parts.extend(
                    rels_parts(&names, &pictures)
                        .into_iter()
                        .map(|(n, b)| (n, Some(b))),
                );
                // Pictures kept from the file become media of the document; new ones are added.
                for (_, name) in &pictures {
                    if !s.doc().media.contains_key(name) {
                        parts.push((name.clone(), image(name)));
                    }
                }
                let added: Vec<String> = media.added.iter().map(|(n, _)| n.clone()).collect();
                parts.extend(old_parts(s.doc(), &img, &old.src, &pictures, &added));
                let media_name = names[4].clone();
                let alt = alt_text(&nodes);
                s.update_smartart(parts, move |i| {
                    i.media = media_name;
                    i.src = None;
                    i.description = alt;
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
            })(),
            DocSmartArtCommand::SmartArtToShapes => (|| {
                let img = s.smartart().ok_or("noSmartArt")?;
                let old = read(s.doc(), &img).ok_or("noSmartArt")?;
                let size = (img.width.0, img.height.0);
                // The drawing as it shows: the drawing part, else the data model laid out.
                let is_drawing = diagram::split_name(&img.media)
                    .is_some_and(|(k, _, _)| k == diagram::DiagramPart::Drawing);
                let (drawing, rels) = if is_drawing {
                    let d = s.doc().media.get(&img.media).ok_or("noSmartArt")?;
                    (d.as_ref().clone(), &old.src.drawing_pictures)
                } else {
                    let defs = dl::Defs::parse(
                        old.src.style.as_ref().map(|p| p.1.as_slice()),
                        old.src.colors.as_ref().map(|p| p.1.as_slice()),
                    );
                    let styling = dl::Styling {
                        look: old.look,
                        defs: Some(&defs),
                        picture_size: None,
                    };
                    (
                        dl::drawing_xml_with(&old.nodes, old.layout, &styling, size.0, size.1)
                            .into_bytes(),
                        &old.src.pictures,
                    )
                };
                let resolve = |rid: &str| {
                    rels.iter()
                        .find(|(id, _)| id == rid)
                        .map(|(_, name)| name.clone())
                };
                let tree =
                    bo_pptx::diagram::group_tree(&drawing, size, &resolve).ok_or("noSmartArt")?;
                let mut n = s.part_number_start(1);
                let name = loop {
                    let cand = diagram::group_name("/word", n);
                    if !s.doc().media.contains_key(&cand) {
                        break cand;
                    }
                    n += 1;
                };
                let mut parts: Vec<(String, Option<Arc<Vec<u8>>>)> =
                    vec![(name.clone(), Some(Arc::new(tree)))];
                // The pictures of the shapes are media of the document.
                for (_, image) in rels {
                    if !s.doc().media.contains_key(image) {
                        let bytes = old.src.images.iter().find(|(n, _)| n == image);
                        parts.push((image.clone(), bytes.map(|(_, b)| b.clone())));
                    }
                }
                parts.extend(old_parts(s.doc(), &img, &old.src, rels, &[]));
                s.update_smartart(parts, move |i| {
                    i.media = name;
                    i.src = None;
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
            })(),
        };
        let dirty = self.relayout()?;
        let mut r: EditResult = self.story_result(dirty, res.err())?;
        r.collab = self.collab_after_edit();
        let mut v = serde_json::to_value(&r).unwrap_or_default();
        if let Some(o) = v.as_object_mut() {
            o.insert(
                "smartArt".into(),
                serde_json::to_value(self.doc_smartart()).unwrap_or_default(),
            );
        }
        Ok(v)
    }
}

/// The parts of the previous BlueOffice version of diagram `img` that go when it is rewritten or
/// converted (a file diagram's stay as read): its five parts, their relationships parts and the
/// pictures BlueOffice added for it that neither `keep` nor another picture of the document uses.
fn old_parts(
    doc: &bo_model::Document,
    img: &Image,
    src: &bo_docx::diagram::DiagramSource,
    keep: &Pictures,
    new_names: &[String],
) -> Vec<(String, Option<Arc<Vec<u8>>>)> {
    let Some(old) = bo_docx::diagram::own_parts(doc, img) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Option<Arc<Vec<u8>>>)> = old.into_iter().map(|n| (n, None)).collect();
    out.extend(
        bo_docx::diagram::own_rels(doc, img)
            .into_iter()
            .map(|n| (n, None)),
    );
    let shown = crate::media::sizes(doc);
    let in_file = bo_docx::diagram::source_part_names(doc);
    for (_, name) in &src.pictures {
        if keep.iter().all(|(_, n)| n != name)
            && doc.media.contains_key(name)
            && !shown.contains_key(name)
            && !in_file.contains(name)
            && !new_names.contains(name)
            && out.iter().all(|(n, _)| n != name)
        {
            out.push((name.clone(), None));
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Engine;
    use serde_json::json;

    fn engine() -> Engine {
        let mut e = Engine::new();
        for f in crate::bundled::FONTS {
            e.fonts.add_font(f.to_vec());
        }
        e.open("Sơ đồ".as_bytes(), "new.txt").unwrap();
        e
    }

    #[allow(clippy::needless_pass_by_value)]
    fn run(e: &mut Engine, v: serde_json::Value) -> serde_json::Value {
        let out = e.command_json(&v.to_string()).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    fn items(v: &[(&str, usize)]) -> serde_json::Value {
        json!(v
            .iter()
            .map(|(t, l)| json!({"text": t, "level": l}))
            .collect::<Vec<_>>())
    }

    // source: black-box Office SmartArt Design tab (CORE-SMART-004): Add Assistant, Right to Left
    // and the live preview of a layout on the diagram's own text, each one undo step, kept when
    // saved (ST_PtType `asst`, layout variable `dgm:dir`, ECMA-376 Part 1, §21.4).
    #[test]
    fn design_actions_rtl_assistant_and_preview() {
        let mut e = engine();
        run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "hierarchy", "items": items(&[("Một", 0), ("Hai", 1)])}),
        );
        let r = run(
            &mut e,
            json!({"cmd": "setSmartArt", "rtl": true, "items": [
                {"text": "Một", "level": 0},
                {"text": "Trợ lý", "level": 1, "assistant": true},
                {"text": "Hai", "level": 1}
            ]}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["rtl"], true);
        assert_eq!(r["smartArt"]["items"][1]["assistant"], true);
        // One undo step takes both back.
        run(&mut e, json!({"cmd": "undo"}));
        let info = e.doc_smartart().unwrap();
        assert!(!info.rtl && info.items.len() == 2);
        run(&mut e, json!({"cmd": "redo"}));
        let bytes = e.save_docx().unwrap();
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let data = pkg
            .parts()
            .iter()
            .find(|p| p.name.starts_with("/word/diagrams/data"))
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
            .unwrap();
        assert!(data.contains(r#"type="asst""#) && data.contains(r#"<dgm:dir val="rev"/>"#));
        // Reopened: the direction and the assistant read back.
        let mut back = engine();
        back.open(&bytes, "back.docx").unwrap();
        let (para, off) = image_pos(back.document().unwrap());
        run(
            &mut back,
            json!({"cmd": "setSelection", "selection": {"anchor": {"p": para, "o": off}, "focus": {"p": para, "o": off + 1}}}),
        );
        let info = back.doc_smartart().unwrap();
        assert!(info.rtl && info.items[1].assistant);
        // Live preview: the layouts drawn with the outline given.
        let r = run(
            &mut e,
            json!({"cmd": "smartArtPreviews", "items": items(&[("Một", 0)]), "width": 300, "height": 200}),
        );
        let svg = r["previews"][0]["svg"].as_str().unwrap();
        assert!(svg.contains(r#"viewBox="0 0 300 200""#));
    }

    #[test]
    fn insert_edit_undo_and_save() {
        let mut e = engine();
        let r = run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "process", "items": items(&[("Một", 0), ("Hai", 0), ("Ba", 0)])}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["layout"], "process");
        assert_eq!(r["smartArt"]["items"][2]["text"], "Ba");
        let doc = e.document().unwrap();
        for k in diagram::DiagramPart::ALL {
            assert!(doc
                .media
                .contains_key(&format!("/word/diagrams/{}1.xml", k.stem())));
        }
        // The frame is shown as a PNG picture of its drawing part.
        assert_eq!(
            &e.media("/word/diagrams/drawing1.xml").unwrap()[..4],
            b"\x89PNG"
        );
        // Text pane: demote "Hai" under "Một", change colours and layout.
        let r = run(
            &mut e,
            json!({"cmd": "setSmartArt", "layout": "hierarchy", "colors": "green", "items": items(&[("Một", 0), ("Hai", 1), ("Ba", 0)])}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["layout"], "hierarchy");
        assert_eq!(r["smartArt"]["colors"], "green");
        assert_eq!(r["smartArt"]["items"][1]["level"], 1);
        // New parts replace the previous BlueOffice ones.
        let doc = e.document().unwrap();
        assert!(doc.media.contains_key("/word/diagrams/drawing2.xml"));
        assert!(!doc.media.contains_key("/word/diagrams/drawing1.xml"));
        // One undo step brings the process back.
        run(&mut e, json!({"cmd": "undo"}));
        let info = e.doc_smartart().unwrap();
        assert_eq!(info.layout, "process");
        assert_eq!(info.items.iter().map(|i| i.level).max(), Some(0));
        run(&mut e, json!({"cmd": "redo"}));
        // Saved and reopened: the same outline, five parts with content types and relationships.
        let bytes = e.save_docx().unwrap();
        // BO_SMARTART_OUT=<dir>: keep the file for `bo-cli validate`.
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/smartart-doc.docx"), &bytes).unwrap();
        }
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let ct = pkg.content_types().unwrap();
        let rels = pkg.relationships("/word/document.xml").unwrap();
        for k in diagram::DiagramPart::ALL {
            let n = format!("/word/diagrams/{}2.xml", k.stem());
            assert_eq!(ct.get(&n), Some(k.content_type().as_str()), "{n}");
            assert!(
                rels.iter().any(|r| r.rel_type == k.rel_type()
                    && bo_opc::resolve_target("/word/document.xml", &r.target) == n),
                "{n}"
            );
        }
        assert!(pkg.part("/word/diagrams/drawing1.xml").is_none());
        let data =
            String::from_utf8(pkg.part("/word/diagrams/data2.xml").unwrap().data.clone()).unwrap();
        let drawing_rel = rels
            .iter()
            .find(|r| r.rel_type == diagram::DiagramPart::Drawing.rel_type())
            .unwrap();
        assert!(data.contains(&format!("relId=\"{}\"", drawing_rel.id)));
        let mut back = engine();
        back.open(&bytes, "back.docx").unwrap();
        let doc = back.document().unwrap();
        let img = first_image(doc);
        assert_eq!(img.media, "/word/diagrams/drawing2.xml");
        let r = read(doc, &img).unwrap();
        assert_eq!(r.layout, Layout::Hierarchy);
        assert_eq!(r.look.palette, Palette::Green);
        assert_eq!(
            dl::outline_of(&r.nodes),
            vec![("Một".into(), 0), ("Hai".into(), 1), ("Ba".into(), 0)]
        );
    }

    fn first_image(doc: &bo_model::Document) -> Image {
        doc.sections
            .iter()
            .flat_map(|s| &s.blocks)
            .find_map(|b| match b {
                bo_model::Block::Paragraph(p) => {
                    p.runs
                        .iter()
                        .flat_map(|r| &r.content)
                        .find_map(|c| match c {
                            bo_model::RunContent::Image(i) => Some((**i).clone()),
                            _ => None,
                        })
                }
                _ => None,
            })
            .unwrap()
    }

    // source: ECMA-376 Part 1, §21.4.2.15 layoutDef — the layouts of the library beyond the first
    // of each category (CORE-SMART-003), in a document.
    #[test]
    fn library_layouts_in_a_document() {
        let mut e = engine();
        let r = run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "processChevron", "items": items(&[("Một", 0), ("Hai", 0)])}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["layout"], "processChevron");
        for layout in Layout::ALL {
            let r = run(
                &mut e,
                json!({"cmd": "setSmartArt", "layout": layout.name()}),
            );
            assert!(r["error"].is_null(), "{r}");
            assert_eq!(r["smartArt"]["layout"], layout.name());
            assert_eq!(r["smartArt"]["items"][1]["text"], "Hai");
            // The frame shows the picture of the new drawing.
            let media = first_image(e.document().unwrap()).media;
            assert_eq!(&e.media(&media).unwrap()[..4], b"\x89PNG", "{layout:?}");
        }
        let bytes = e.save_docx().unwrap();
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/smartart-doc-picture.docx"), &bytes).unwrap();
        }
        let mut back = engine();
        back.open(&bytes, "back.docx").unwrap();
        let doc = back.document().unwrap();
        let r = read(doc, &first_image(doc)).unwrap();
        assert_eq!(r.layout, Layout::PictureStrip);
        assert_eq!(r.layout.category(), dl::Category::Picture);
        assert_eq!(r.nodes.len(), 2);
    }

    #[test]
    fn floating_and_refusals() {
        let mut e = engine();
        let r = run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "nope", "items": items(&[("A", 0)])}),
        );
        assert_eq!(r["error"], "badSmartArt");
        let r = run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "cycle", "items": items(&[("  ", 0)])}),
        );
        assert_eq!(r["error"], "badSmartArt");
        let r = run(&mut e, json!({"cmd": "setSmartArt", "colors": "blue"}));
        assert_eq!(r["error"], "noSmartArt");
        let r = run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "cycle", "floating": true, "width": 300, "height": 200, "items": items(&[("A", 0), ("B", 0)])}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["floating"], true);
        assert_eq!(r["smartArt"]["width"], 300.0);
        assert_eq!(r["format"]["smartArt"], true);
    }

    // source: corpus smartart-04-datamodel.docx (hand-built: data model, layout, style, colours).
    #[test]
    fn file_diagram_is_read_and_edited_its_parts_kept_until_then() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/smartart-04-datamodel.docx"
        );
        let src = std::fs::read(path).unwrap();
        let mut e = engine();
        e.open(&src, "smart.docx").unwrap();
        // Unchanged: every diagram part of the file is written as read.
        let saved = e.save_docx().unwrap();
        let (a, b) = (
            bo_opc::Package::read(&src, &bo_opc::Limits::default()).unwrap(),
            bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap(),
        );
        let file_parts: Vec<String> = a
            .parts()
            .iter()
            .filter(|p| p.name.contains("/diagrams/"))
            .map(|p| p.name.clone())
            .collect();
        assert!(file_parts.len() >= 4);
        for n in &file_parts {
            assert_eq!(a.part(n).unwrap().data, b.part(n).unwrap().data, "{n}");
        }
        // Select the frame and edit it through the text pane.
        let doc = e.document().unwrap();
        let img = first_image(doc);
        let (para, off) = image_pos(doc);
        assert!(bo_docx::diagram::is_diagram(doc, &img));
        run(
            &mut e,
            json!({"cmd": "setSelection", "selection": {"anchor": {"p": para, "o": off}, "focus": {"p": para, "o": off + 1}}}),
        );
        let info = e.doc_smartart().unwrap();
        assert_ne!(info.items, vec![]);
        let r = run(&mut e, json!({"cmd": "setSmartArt", "style": "outline"}));
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["style"], "outline");
        assert_eq!(r["smartArt"]["items"], json!(info.items));
        let saved = e.save_docx().unwrap();
        let pkg = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
        // New parts beside the file's, which stay as read.
        let new_drawing = first_image(e.document().unwrap()).media;
        assert!(pkg.part(&new_drawing).is_some());
        for n in &file_parts {
            assert_eq!(a.part(n).unwrap().data, pkg.part(n).unwrap().data, "{n}");
        }
    }

    fn image_pos(doc: &bo_model::Document) -> (usize, usize) {
        let paths = bo_model::index::body_paragraphs(doc);
        for (id, path) in paths.iter().enumerate() {
            let p = bo_model::index::para_at(doc, path).unwrap();
            let mut off = 0;
            for (_, item) in bo_edit::para::flatten(p) {
                if matches!(item, bo_model::RunContent::Image(_)) {
                    return (id, off);
                }
                off += bo_edit::para::atom_len(&item);
            }
        }
        panic!("no image")
    }

    /// A `w` × `h` PNG of one colour as a `data:` URL.
    fn png_url(w: u32, h: u32, rgb: [u8; 3]) -> String {
        use base64::Engine as _;
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
            .collect();
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(bo_model3d::scene::encode_png(w, h, &rgba))
        )
    }

    fn save(e: &Engine, name: &str) -> Vec<u8> {
        let bytes = e.save_docx().unwrap();
        // BO_SMARTART_OUT=<dir>: keep the file for `bo-cli validate`.
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/{name}"), &bytes).unwrap();
        }
        bytes
    }

    // source: ECMA-376 Part 1, §20.1.8.14 blipFill in `dgm:pt/dgm:spPr` and in the drawing part's
    // shapes; Part 2 relationships of the data model and drawing parts to the image part.
    #[test]
    fn picture_nodes_in_a_document() {
        let mut e = engine();
        let (red, blue) = (png_url(40, 10, [200, 30, 30]), png_url(8, 8, [30, 30, 200]));
        let r = run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "pictureStrip", "items": [
                {"text": "Hà Nội", "level": 0, "picture": red},
                {"text": "Huế", "level": 0},
                {"text": "Đà Nẵng", "level": 0, "picture": blue},
            ]}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["items"][0]["picture"], "rIdImg1");
        assert!(r["smartArt"]["items"][1].get("picture").is_none());
        assert_eq!(r["smartArt"]["items"][2]["picture"], "rIdImg2");
        let doc = e.document().unwrap();
        let images: Vec<&String> = doc.media.keys().filter(|k| k.contains("/media/")).collect();
        assert_eq!(images.len(), 2);
        for part in ["data1", "drawing1"] {
            let rels = &doc.media[&format!("/word/diagrams/_rels/{part}.xml.rels")];
            let x = String::from_utf8_lossy(rels);
            assert!(
                x.contains("Id=\"rIdImg1\"") && x.contains("Target=\"../media/image"),
                "{x}"
            );
        }
        // Cropped to its round frame (40 × 10 → 3/8 from each side); drawn as a picture.
        let drawing =
            String::from_utf8_lossy(&doc.media["/word/diagrams/drawing1.xml"]).into_owned();
        assert!(drawing.contains(r#"<a:blip r:embed="rIdImg1"/><a:srcRect l="37500" r="37500"/>"#));
        assert_eq!(
            &e.media("/word/diagrams/drawing1.xml").unwrap()[..4],
            b"\x89PNG"
        );
        // Replace one, move one, remove one; a layout change keeps them.
        let green = png_url(4, 4, [30, 200, 30]);
        let r = run(
            &mut e,
            json!({"cmd": "setSmartArt", "layout": "picture", "items": [
                {"text": "Hà Nội", "level": 0, "picture": green},
                {"text": "Huế", "level": 0, "picture": "rIdImg1"},
                {"text": "Đà Nẵng", "level": 0},
            ]}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["items"][0]["picture"], "rIdImg3");
        assert_eq!(r["smartArt"]["items"][1]["picture"], "rIdImg1");
        // The picture no node shows any more is gone from the media; the old parts too.
        let doc = e.document().unwrap();
        assert_eq!(
            doc.media.keys().filter(|k| k.contains("/media/")).count(),
            2
        );
        assert!(!doc
            .media
            .contains_key("/word/diagrams/_rels/data1.xml.rels"));
        assert!(doc
            .media
            .contains_key("/word/diagrams/_rels/data2.xml.rels"));
        // Saved: both parts have their relationships; no relationships part is left as media.
        let bytes = save(&e, "smartart-doc-pictures.docx");
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        for part in ["/word/diagrams/data2.xml", "/word/diagrams/drawing2.xml"] {
            let rels = pkg.relationships(part).unwrap();
            assert_eq!(rels.len(), 2, "{part}");
            for r in rels {
                assert!(pkg.part(&bo_opc::resolve_target(part, &r.target)).is_some());
            }
        }
        assert!(pkg.part("/word/diagrams/_rels/data1.xml.rels").is_none());
        let ct = pkg.content_types().unwrap();
        for p in pkg
            .parts()
            .iter()
            .filter(|p| p.name.starts_with("/word/media/"))
        {
            assert_eq!(ct.get(&p.name), Some("image/png"), "{}", p.name);
        }
        // Reopened: the same pictures on the same nodes, drawn; then edited again (file diagram).
        let mut back = engine();
        back.open(&bytes, "back.docx").unwrap();
        let (para, off) = image_pos(back.document().unwrap());
        run(
            &mut back,
            json!({"cmd": "setSelection", "selection": {"anchor": {"p": para, "o": off}, "focus": {"p": para, "o": off + 1}}}),
        );
        let info = back.doc_smartart().unwrap();
        assert_eq!(
            info.items
                .iter()
                .map(|i| i.picture.as_deref())
                .collect::<Vec<_>>(),
            [Some("rIdImg3"), Some("rIdImg1"), None]
        );
        let media = first_image(back.document().unwrap()).media;
        assert_eq!(&back.media(&media).unwrap()[..4], b"\x89PNG");
        let r = run(&mut back, json!({"cmd": "setSmartArt", "colors": "green"}));
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["items"][1]["picture"], "rIdImg1");
        let again = back.save_docx().unwrap();
        let pkg = bo_opc::Package::read(&again, &bo_opc::Limits::default()).unwrap();
        let new_data = pkg
            .parts()
            .iter()
            .filter(|p| p.name.starts_with("/word/diagrams/data"))
            .map(|p| p.name.clone())
            .max()
            .unwrap();
        assert_eq!(pkg.relationships(&new_data).unwrap().len(), 2);
        // Refused: a picture that is no PNG / JPEG.
        let r = run(
            &mut e,
            json!({"cmd": "setSmartArt", "items": [{"text": "A", "level": 0, "picture": "data:image/png;base64,QUJD"}]}),
        );
        assert_eq!(r["error"], "badPicture");
    }

    // source: [MS-ODRAWXML] wordprocessingGroup (`wpg:wgp` in a §17.3.3.9 drawing whose
    // graphicData@uri is the group namespace) — CORE-SMART-004 "Convert to shapes" in BlueDoc.
    #[test]
    fn a_diagram_becomes_a_group_of_shapes() {
        let mut e = engine();
        let red = png_url(6, 6, [200, 30, 30]);
        run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "picture", "items": [
                {"text": "Một", "level": 0, "picture": red},
                {"text": "Hai", "level": 0},
            ]}),
        );
        let frame = first_image(e.document().unwrap());
        let r = run(&mut e, json!({"cmd": "smartArtToShapes"}));
        assert!(r["error"].is_null(), "{r}");
        // No diagram at the selection any more; the frame shows a group of the same size.
        assert!(r["smartArt"].is_null());
        assert!(r["format"]["smartArt"].is_null() || r["format"]["smartArt"] == false);
        let doc = e.document().unwrap();
        let img = first_image(doc);
        assert!(diagram::is_group_name(&img.media), "{}", img.media);
        assert_eq!((img.width, img.height), (frame.width, frame.height));
        assert!(
            doc.media.keys().all(|k| !k.contains("/diagrams/")),
            "diagram parts are gone"
        );
        let tree = String::from_utf8_lossy(&doc.media[&img.media]).into_owned();
        assert!(tree.contains(&format!(
            r#"<a:chExt cx="{}" cy="{}"/>"#,
            frame.width.0, frame.height.0
        )));
        assert!(tree.contains(r#"r:embed="/word/media/image"#), "{tree}");
        assert_eq!(&e.media(&img.media).unwrap()[..4], b"\x89PNG");
        // Saved: a wpg:wgp with one wps:wsp per shape, text as WordprocessingML, the picture by a
        // relationship of the document part; no diagram or group part in the package.
        let bytes = save(&e, "smartart-doc-converted.docx");
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        let body =
            String::from_utf8_lossy(&pkg.part("/word/document.xml").unwrap().data).into_owned();
        assert!(body.contains("wordprocessingGroup\"><wpg:wgp "), "{body}");
        // Two cards: a frame and a caption each, plus the two marks of the empty frame.
        assert_eq!(body.matches("<wps:wsp>").count(), 6);
        assert!(body.contains("<w:t xml:space=\"preserve\">Một</w:t>"));
        assert!(pkg
            .parts()
            .iter()
            .all(|p| !p.name.contains("/diagrams/") && !p.name.contains("shapegroups")));
        let rels = pkg.relationships("/word/document.xml").unwrap();
        let image = rels
            .iter()
            .find(|r| r.rel_type.ends_with("/image"))
            .unwrap();
        assert!(body.contains(&format!("r:embed=\"{}\"", image.id)));
        // Reopened: read as a group again (not a diagram), drawn; saved again byte-for-byte equal
        // in its drawing (the source XML is kept).
        let mut back = engine();
        back.open(&bytes, "back.docx").unwrap();
        let doc = back.document().unwrap();
        let img2 = first_image(doc);
        assert!(diagram::is_group_name(&img2.media));
        assert!(!bo_docx::diagram::is_diagram(doc, &img2));
        assert_eq!(&back.media(&img2.media).unwrap()[..4], b"\x89PNG");
        let read = String::from_utf8_lossy(&doc.media[&img2.media]).into_owned();
        assert_eq!(read.matches("<dsp:sp ").count(), 6);
        assert!(read.contains("<a:t>Một</a:t>") && read.contains(r#"r:embed="/word/media/image"#));
        let again = back.save_docx().unwrap();
        let pkg2 = bo_opc::Package::read(&again, &bo_opc::Limits::default()).unwrap();
        let body2 =
            String::from_utf8_lossy(&pkg2.part("/word/document.xml").unwrap().data).into_owned();
        let wgp =
            |s: &str| s[s.find("<wpg:wgp").unwrap()..s.find("</wpg:wgp>").unwrap()].to_owned();
        assert_eq!(wgp(&body), wgp(&body2));
        // One undo step brings the diagram back.
        run(&mut e, json!({"cmd": "undo"}));
        let info = e.doc_smartart().unwrap();
        assert_eq!(info.layout, "picture");
        assert_eq!(info.items[0].picture.as_deref(), Some("rIdImg1"));
        // Nothing to convert without a diagram at the selection.
        let mut empty = engine();
        let r = run(&mut empty, json!({"cmd": "smartArtToShapes"}));
        assert_eq!(r["error"], "noSmartArt");
    }

    const FILE_COLORS: &str = r#"<dgm:colorsDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" uniqueId="urn:example/colors/test1"><dgm:styleLbl name="node1"><dgm:fillClrLst meth="repeat"><a:srgbClr val="AA1122"/><a:srgbClr val="11AA22"/></dgm:fillClrLst><dgm:linClrLst meth="repeat"><a:srgbClr val="333333"/></dgm:linClrLst></dgm:styleLbl></dgm:colorsDef>"#;

    /// `bytes` with the first colours part replaced by [`FILE_COLORS`] and named on the document
    /// point of the first data model: a diagram as another application writes it.
    fn with_file_colors(bytes: &[u8]) -> Vec<u8> {
        let mut pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
        let name = |stem: &str| {
            pkg.parts()
                .iter()
                .find(|p| p.name.contains(&format!("/diagrams/{stem}")))
                .map(|p| p.name.clone())
                .unwrap()
        };
        let (colors, data) = (name("colors"), name("data"));
        pkg.put_part(&colors, FILE_COLORS.as_bytes().to_vec());
        let d = String::from_utf8(pkg.part(&data).unwrap().data.clone()).unwrap();
        let at = d.find("csTypeId=\"").unwrap() + 10;
        let end = at + d[at..].find('"').unwrap();
        let d = format!("{}urn:example/colors/test1{}", &d[..at], &d[end..]);
        pkg.put_part(&data, d.into_bytes());
        pkg.write().unwrap()
    }

    // source: ECMA-376 Part 1, §21.4 colorsDef (styleLbl / fillClrLst meth="repeat") — the file's
    // own colours are used when BlueOffice lays the diagram out, and kept until others are chosen.
    #[test]
    fn the_files_own_colours_in_a_document() {
        let mut e = engine();
        run(
            &mut e,
            json!({"cmd": "insertSmartArt", "layout": "process", "items": items(&[("Một", 0), ("Hai", 0), ("Ba", 0)])}),
        );
        let bytes = with_file_colors(&e.save_docx().unwrap());
        let mut f = engine();
        f.open(&bytes, "file.docx").unwrap();
        let (para, off) = image_pos(f.document().unwrap());
        run(
            &mut f,
            json!({"cmd": "setSelection", "selection": {"anchor": {"p": para, "o": off}, "focus": {"p": para, "o": off + 1}}}),
        );
        let info = f.doc_smartart().unwrap();
        assert_eq!(
            (info.colors.as_str(), info.style.as_str()),
            ("file", "fill")
        );
        // Editing the text lays the diagram out with the file's colours and keeps its part.
        let r = run(
            &mut f,
            json!({"cmd": "setSmartArt", "items": items(&[("Một", 0), ("Hai", 0), ("Ba", 0), ("Bốn", 0)])}),
        );
        assert!(r["error"].is_null(), "{r}");
        assert_eq!(r["smartArt"]["colors"], "file");
        let doc = f.document().unwrap();
        let img = first_image(doc);
        let drawing = String::from_utf8_lossy(&doc.media[&img.media]).into_owned();
        assert_eq!(drawing.matches(r#"<a:solidFill><a:srgbClr val="AA1122"/></a:solidFill><a:ln w="12700"><a:solidFill><a:srgbClr val="333333"/>"#).count(), 2);
        assert_eq!(
            drawing
                .matches(r#"<a:solidFill><a:srgbClr val="11AA22"/></a:solidFill><a:ln"#)
                .count(),
            2
        );
        assert!(!drawing.contains("1F6FB2"), "no default palette colour");
        let colors = diagram::sibling(&img.media, diagram::DiagramPart::Colors).unwrap();
        assert_eq!(doc.media[&colors].as_slice(), FILE_COLORS.as_bytes());
        // Choosing a BlueOffice palette replaces the part.
        let r = run(&mut f, json!({"cmd": "setSmartArt", "colors": "blue"}));
        assert_eq!(r["smartArt"]["colors"], "blue");
        let doc = f.document().unwrap();
        let img = first_image(doc);
        let colors = diagram::sibling(&img.media, diagram::DiagramPart::Colors).unwrap();
        assert!(String::from_utf8_lossy(&doc.media[&colors]).contains("blueoffice/colors/blue"));
        assert!(String::from_utf8_lossy(&doc.media[&img.media]).contains("1F6FB2"));
        // Layout thumbnails for the dialog.
        let p = run(
            &mut f,
            json!({"cmd": "smartArtPreviews", "style": "outline"}),
        );
        assert_eq!(p["previews"].as_array().unwrap().len(), Layout::ALL.len());
        assert!(p["previews"][0]["svg"]
            .as_str()
            .unwrap()
            .starts_with("<svg "));
    }
}
