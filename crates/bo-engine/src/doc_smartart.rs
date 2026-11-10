//! SmartArt in BlueDoc (CORE-SMART-005): insert a diagram from the BlueOffice layout library (the
//! same as BlueSlide's, `bo_pptx::diagram_layout`) inline or floating at the selection, read the
//! selected one's text outline, layout, colours and look (the text pane), and change them. Every
//! edit writes all five parts (ECMA-376 Part 1, §21.4 data model, layout definition, quick style,
//! colours; [MS-ODRAWXML] drawing) under fresh names and points the frame at them, so it is one
//! undo step and the frame's picture changes with it. A diagram read from the file keeps its
//! parts verbatim until it is edited.

use crate::edit::EditResult;
use crate::slide_edit::smartart::SmartItem;
use crate::{EngineError, Result};
use bo_model::diagram::{self, DRAWING_REL_PLACEHOLDER};
use bo_model::{Anchor, Emu, Image, Wrap};
use bo_pptx::diagram_layout::{self as dl, Category, Fill, Look, Palette};
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
        /// Layout category (`list`, `process`, `cycle`, `hierarchy`, `matrix`, `pyramid`,
        /// `relationship`).
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
        /// New layout category.
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
    },
    /// The SmartArt at the selection (for the text pane).
    SmartArtInfo,
}

impl DocSmartArtCommand {
    /// Parses a JSON command when it is a BlueDoc SmartArt command.
    pub fn parse(v: &serde_json::Value) -> Option<DocSmartArtCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !matches!(c, "insertSmartArt" | "setSmartArt" | "smartArtInfo") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

/// The SmartArt at the selection as the text pane shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocSmartArt {
    /// Layout category.
    pub layout: String,
    /// Text outline.
    pub items: Vec<SmartItem>,
    /// Colours.
    pub colors: String,
    /// Look.
    pub style: String,
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
    })
}

fn outline(items: &[SmartItem]) -> Vec<(String, usize)> {
    items
        .iter()
        .filter(|i| !i.text.trim().is_empty())
        .map(|i| (i.text.trim().to_owned(), i.level))
        .collect()
}

/// Alternative text of a diagram: its node texts.
fn alt_text(lines: &[(String, usize)]) -> String {
    lines
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

/// Read the outline, category and look of a diagram's data model.
fn read(doc: &bo_model::Document, img: &Image) -> Option<(Vec<dl::DiagramNode>, Category, Look)> {
    let src = bo_docx::diagram::diagram_source(doc, img)?;
    let (name, data) = &src.data;
    let (nodes, lo) = dl::parse_data_model(data, name)?;
    let cat = dl::category(lo.as_deref(), src.layout.as_ref().map(|l| l.1.as_slice()));
    Some((nodes, cat, dl::look_of(data, name)))
}

impl crate::Engine {
    /// The SmartArt at the selection of the open document.
    pub fn doc_smartart(&self) -> Option<DocSmartArt> {
        let s = self.session.as_ref()?;
        let img = s.smartart()?;
        let (nodes, cat, lk) = read(s.doc(), &img)?;
        Some(DocSmartArt {
            layout: cat.name().to_owned(),
            items: dl::outline_of(&nodes)
                .into_iter()
                .map(|(text, level)| SmartItem { text, level })
                .collect(),
            colors: lk.palette.name().to_owned(),
            style: lk.fill.name().to_owned(),
            width: img.width.0 as f64 / EMU_PT,
            height: img.height.0 as f64 / EMU_PT,
            floating: img.anchor.is_some(),
        })
    }

    /// Run a BlueDoc SmartArt command; the result is the edit result plus `smartArt` (the SmartArt
    /// at the selection afterwards).
    pub fn doc_smartart_command(&mut self, cmd: DocSmartArtCommand) -> Result<serde_json::Value> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let res: std::result::Result<(), String> = match cmd {
            DocSmartArtCommand::SmartArtInfo => Ok(()),
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
                let cat = Category::parse(&layout).ok_or("badSmartArt")?;
                let lk = look(Look::default(), colors.as_deref(), style.as_deref())?;
                let lines = outline(&items);
                let nodes = dl::nodes_from_outline(&lines);
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
                let built = dl::build_parts(&nodes, cat, lk, (cx, cy), DRAWING_REL_PLACEHOLDER);
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
                    description: alt_text(&lines),
                    shape: None,
                    format: None,
                    src: None,
                    ole: None,
                };
                s.insert_smartart(parts_of(&names, built).collect(), img)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })(),
            DocSmartArtCommand::SetSmartArt {
                layout,
                items,
                colors,
                style,
            } => (|| {
                let img = s.smartart().ok_or("noSmartArt")?;
                let (old_nodes, old_cat, old_look) = read(s.doc(), &img).ok_or("noSmartArt")?;
                let cat = match layout.as_deref() {
                    Some(l) => Category::parse(l).ok_or("badSmartArt")?,
                    None => old_cat,
                };
                let lk = look(old_look, colors.as_deref(), style.as_deref())?;
                let (nodes, lines) = match items.as_deref() {
                    Some(i) => {
                        let lines = outline(i);
                        (dl::nodes_from_outline(&lines), lines)
                    }
                    None => (old_nodes.clone(), dl::outline_of(&old_nodes)),
                };
                if nodes.is_empty() {
                    return Err("badSmartArt".to_owned());
                }
                let taken = bo_docx::diagram::source_diagram_parts(s.doc());
                let names = diagram::part_names(DIR, s.new_diagram_number(DIR, &taken));
                let built = dl::build_parts(
                    &nodes,
                    cat,
                    lk,
                    (img.width.0, img.height.0),
                    DRAWING_REL_PLACEHOLDER,
                );
                let mut parts: Vec<(String, Option<Arc<Vec<u8>>>)> =
                    parts_of(&names, built).map(|(n, b)| (n, Some(b))).collect();
                // The parts of the previous BlueOffice version go (a file diagram's stay as read).
                if let Some(old) = bo_docx::diagram::own_parts(s.doc(), &img) {
                    parts.extend(old.into_iter().map(|n| (n, None)));
                }
                let media = names[4].clone();
                let alt = alt_text(&lines);
                s.update_smartart(parts, move |i| {
                    i.media = media;
                    i.src = None;
                    i.description = alt;
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
            })(),
        };
        let dirty = self.relayout()?;
        let mut r: EditResult = self.edit_result(dirty, (None, None, None), res.err())?;
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
        let (nodes, cat, lk) = read(doc, &img).unwrap();
        assert_eq!(cat, Category::Hierarchy);
        assert_eq!(lk.palette, Palette::Green);
        assert_eq!(
            dl::outline_of(&nodes),
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
}
