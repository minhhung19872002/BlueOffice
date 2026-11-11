//! SmartArt in BlueSheet (CORE-SMART-005): insert a diagram from the BlueOffice layout library (the
//! same as BlueSlide's and BlueDoc's, `bo_pptx::diagram_layout`) over a range of cells, read a
//! diagram's text outline, layout, colours and look (the text pane), change them, move and delete
//! it. A diagram is an `xdr:graphicFrame` (ECMA-376 Part 1, §20.5.2.16) in a two-cell anchor
//! (§20.5.2.33) whose `dgm:relIds` (§21.4.2.6) name its data model, layout, quick style and colours
//! parts; every edit rebuilds all of them plus the drawing part ([MS-ODRAWXML] `dsp:drawing`) into
//! [`SheetDiagram::xml`], so it is one undo step through [`SheetEngine::edit`]. A diagram read from
//! the file keeps its parts verbatim until it is edited, then they are rewritten in place.

use super::cmd::Recalc;
use super::{SheetEngine, MAX_COLS, MAX_ROWS};
use crate::slide_edit::smartart::SmartItem;
use crate::smartart::Pictures;
use crate::{EngineError, Result};
use bo_model::diagram::{self, DiagramXml, DRAWING_REL_PLACEHOLDER};
use bo_model::sheet::{CellRef, SheetDiagram, Workbook};
use bo_pptx::diagram_layout::{self as dl, Fill, Layout, Look, Palette};
use serde::Deserialize;
use serde_json::{json, Value};

const DIR: &str = "/xl/diagrams";
const EMU_PT: f64 = 12_700.0;

fn bad(msg: impl Into<String>) -> EngineError {
    EngineError::Sheet(msg.into())
}

/// SmartArt commands of BlueSheet.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SmartArtCommand {
    /// Insert a SmartArt over the cells `from` … `to` (`[row, col]`; default: 10 rows × 6
    /// columns from `from`); the result is `{ index, smartArt }`.
    InsertSmartArt {
        /// 0-based sheet index.
        sheet: usize,
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
        /// Top-left anchor cell.
        #[serde(default)]
        from: [u32; 2],
        /// Bottom-right anchor cell.
        #[serde(default)]
        to: Option<[u32; 2]>,
    },
    /// Change diagram `index` (`None` keeps a value).
    SetSmartArt {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the diagram in the sheet.
        index: usize,
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
        /// Lay out right to left (`None` keeps the direction).
        #[serde(default)]
        rtl: Option<bool>,
    },
    /// Diagram `index` as the text pane shows it.
    SmartArtInfo {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the diagram in the sheet.
        index: usize,
    },
    /// Move / resize diagram `index` to the cells `from` … `to`.
    MoveSmartArt {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the diagram in the sheet.
        index: usize,
        /// Top-left anchor cell `[row, col]`.
        from: [u32; 2],
        /// Bottom-right anchor cell `[row, col]`.
        to: [u32; 2],
    },
    /// Delete diagram `index` (a file diagram loses its anchor, relationships and parts).
    DeleteSmartArt {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the diagram in the sheet.
        index: usize,
    },
    /// Replace diagram `index` by a group of ordinary shapes drawn as it shows (an `xdr:grpSp`
    /// in the same cells), one undo step.
    SmartArtToShapes {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the diagram in the sheet.
        index: usize,
    },
    /// The shape of diagram `index` under point (`x`, `y`) — shares (0..1) of its frame — or,
    /// with `node`, that node's text box: the result is `{ node, text, x, y, w, h }` (box in
    /// shares of the frame) or `null`.
    SmartArtNodeAt {
        /// 0-based sheet index.
        sheet: usize,
        /// 0-based index of the diagram in the sheet.
        index: usize,
        /// Point across the frame (0 left … 1 right).
        #[serde(default)]
        x: Option<f64>,
        /// Point down the frame (0 top … 1 bottom).
        #[serde(default)]
        y: Option<f64>,
        /// Index of a node in the text outline (instead of a point).
        #[serde(default)]
        node: Option<usize>,
    },
    /// Thumbnails of every layout of the library: the result is `{ previews: [{ layout, svg }] }`.
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

impl SmartArtCommand {
    /// Parses a JSON command when it is a BlueSheet SmartArt command.
    pub fn parse(v: &Value) -> Option<SmartArtCommand> {
        const NAMES: &[&str] = &[
            "insertSmartArt",
            "setSmartArt",
            "smartArtInfo",
            "moveSmartArt",
            "deleteSmartArt",
            "smartArtToShapes",
            "smartArtPreviews",
            "smartArtNodeAt",
        ];
        let name = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&name) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    fn sheet(&self) -> usize {
        match self {
            SmartArtCommand::InsertSmartArt { sheet, .. }
            | SmartArtCommand::SetSmartArt { sheet, .. }
            | SmartArtCommand::SmartArtInfo { sheet, .. }
            | SmartArtCommand::MoveSmartArt { sheet, .. }
            | SmartArtCommand::DeleteSmartArt { sheet, .. }
            | SmartArtCommand::SmartArtToShapes { sheet, .. }
            | SmartArtCommand::SmartArtNodeAt { sheet, .. } => *sheet,
            SmartArtCommand::SmartArtPreviews { .. } => 0,
        }
    }
}

fn look(base: Look, colors: Option<&str>, style: Option<&str>) -> Result<Look> {
    Ok(Look {
        palette: match colors {
            Some(c) => Palette::parse(c).ok_or_else(|| bad("badSmartArt"))?,
            None => base.palette,
        },
        fill: match style {
            Some(s) => Fill::parse(s).ok_or_else(|| bad("badSmartArt"))?,
            None => base.fill,
        },
        ..base
    })
}

fn cell([r, c]: [u32; 2]) -> CellRef {
    CellRef::new(r.min(MAX_ROWS - 1), c.min(MAX_COLS - 1))
}

/// Anchor cells at least one cell wide and tall.
fn span(from: CellRef, to: CellRef) -> (CellRef, CellRef) {
    (
        from,
        CellRef::new(
            to.row.max(from.row + 1).min(MAX_ROWS - 1),
            to.col.max(from.col + 1).min(MAX_COLS - 1),
        ),
    )
}

/// Bytes of a workbook part (the file's, kept in the opaque parts).
fn opaque<'a>(wb: &'a Workbook, name: &str) -> Option<&'a [u8]> {
    if name.is_empty() {
        return None;
    }
    wb.opaque
        .parts
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .map(|p| p.data.as_slice())
}

/// Bytes of an image part: the file's, or one a diagram or a group of shapes adds.
pub(crate) fn image_bytes<'a>(wb: &'a Workbook, name: &str) -> Option<&'a [u8]> {
    opaque(wb, name).or_else(|| {
        wb.sheets.iter().find_map(|ws| {
            ws.diagrams
                .iter()
                .filter_map(|d| d.xml.as_ref())
                .flat_map(|x| &x.images)
                .chain(ws.shape_groups.iter().flat_map(|g| &g.images))
                .find(|(n, _)| n == name)
                .map(|(_, b)| b.as_slice())
        })
    })
}

/// The parts a diagram is read from: its own once edited, else the file's.
pub(crate) struct Sources<'a> {
    /// Data model part name.
    pub name: &'a str,
    /// Data model.
    pub data: &'a [u8],
    /// Layout definition.
    pub layout: Option<&'a [u8]>,
    /// Quick style.
    pub style: Option<&'a [u8]>,
    /// Colours.
    pub colors: Option<&'a [u8]>,
    /// Image relationships of the data model part (the pictures of the nodes).
    pub pictures: Pictures,
    /// Image relationships of the drawing part.
    pub drawing_pictures: Pictures,
}

impl Sources<'_> {
    /// The diagram's own quick style and colours.
    pub fn defs(&self) -> dl::Defs {
        dl::Defs::parse(self.style, self.colors)
    }
}

/// Data model, definitions and pictures of a diagram: its own parts once edited, else the file's.
pub(crate) fn sources<'a>(wb: &'a Workbook, d: &'a SheetDiagram) -> Option<Sources<'a>> {
    let rels = |bytes: Option<&[u8]>, part: &str| {
        bytes
            .filter(|b| !b.is_empty())
            .map(|b| bo_pptx::diagram::image_rels(b, part))
            .unwrap_or_default()
    };
    if let Some(x) = &d.xml {
        return Some(Sources {
            name: d.data.as_str(),
            data: x.data.as_slice(),
            layout: Some(x.layout.as_slice()),
            style: Some(x.style.as_slice()),
            colors: Some(x.colors.as_slice()),
            pictures: rels(Some(&x.rels), &d.data),
            drawing_pictures: rels(Some(&x.rels), &d.part),
        });
    }
    Some(Sources {
        name: d.data.as_str(),
        data: opaque(wb, &d.data)?,
        layout: opaque(wb, &d.layout),
        style: opaque(wb, &d.style),
        colors: opaque(wb, &d.colors),
        pictures: rels(opaque(wb, &bo_opc::rels_part_name(&d.data)), &d.data),
        drawing_pictures: rels(opaque(wb, &bo_opc::rels_part_name(&d.part)), &d.part),
    })
}

/// Outline, layout and look of a diagram's data model.
fn read(wb: &Workbook, d: &SheetDiagram) -> Option<(Vec<dl::DiagramNode>, Layout, Look)> {
    let src = sources(wb, d)?;
    let (nodes, lo) = dl::parse_data_model(src.data, src.name)?;
    Some((
        nodes,
        dl::layout_of(lo.as_deref(), src.layout),
        dl::look_of(src.data, src.name),
    ))
}

fn info_json(wb: &Workbook, index: usize, d: &SheetDiagram) -> Value {
    let Some((nodes, cat, lk)) = read(wb, d) else {
        return Value::Null;
    };
    let items: Vec<SmartItem> = crate::smartart::items_of(&nodes);
    let to =
        d.to.unwrap_or(CellRef::new(d.from.row + 10, d.from.col + 6));
    json!({
        "index": index,
        "name": d.name,
        "layout": cat.name(),
        "items": items,
        "colors": lk.palette.name(),
        "style": lk.fill.name(),
        "rtl": lk.rtl,
        "from": [d.from.row, d.from.col],
        "to": [to.row, to.col],
    })
}

/// A diagram number `n` none of whose parts `<dir>/<kind><n>.xml` exists in the workbook.
fn fresh_number(wb: &Workbook) -> usize {
    let taken = |n: &str| {
        opaque(wb, n).is_some()
            || wb.sheets.iter().flat_map(|s| &s.diagrams).any(|d| {
                [&d.part, &d.data, &d.layout, &d.style, &d.colors]
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(n))
            })
    };
    let mut n = 1;
    while diagram::part_names(DIR, n).iter().any(|p| taken(p)) {
        n += 1;
    }
    n
}

/// New image parts of a command: names no part of the workbook has.
struct NewImages {
    taken: Vec<String>,
    added: Vec<(String, Vec<u8>)>,
}

impl NewImages {
    fn new(wb: &Workbook) -> NewImages {
        let mut taken: Vec<String> = wb.opaque.parts.iter().map(|p| p.name.clone()).collect();
        for ws in &wb.sheets {
            taken.extend(
                ws.diagrams
                    .iter()
                    .filter_map(|d| d.xml.as_ref())
                    .flat_map(|x| &x.images)
                    .chain(ws.shape_groups.iter().flat_map(|g| &g.images))
                    .map(|(n, _)| n.clone()),
            );
        }
        NewImages {
            taken,
            added: Vec::new(),
        }
    }

    fn store(&mut self, pic: crate::smartart::NewPicture) -> String {
        let mut n = 1;
        let name = loop {
            let cand = format!("/xl/media/image{n}.{}", pic.ext);
            if !self.taken.iter().any(|t| t.eq_ignore_ascii_case(&cand)) {
                break cand;
            }
            n += 1;
        };
        self.taken.push(name.clone());
        self.added.push((name.clone(), pic.data));
        name
    }

    fn bytes(&self, name: &str) -> Option<&[u8]> {
        self.added
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.as_slice())
    }
}

/// The relationships part the data model part and the drawing part of a sheet diagram share
/// (targets are relative to `/xl/diagrams`); empty without pictures.
fn rels_xml(pictures: &Pictures) -> Vec<u8> {
    if pictures.is_empty() {
        Vec::new()
    } else {
        bo_pptx::diagram::image_rels_xml(&format!("{DIR}/data.xml"), pictures)
    }
}

/// The parts of a diagram as the model holds them, with its pictures: the images it added
/// before (`prev`) or adds now (`new`) that its nodes still use.
fn diagram_xml(
    built: dl::DiagramParts,
    pictures: &Pictures,
    prev: Option<&DiagramXml>,
    new: &NewImages,
) -> DiagramXml {
    let images = prev
        .into_iter()
        .flat_map(|x| x.images.iter())
        .chain(&new.added)
        .filter(|(n, _)| pictures.iter().any(|(_, p)| p == n))
        .cloned()
        .collect();
    DiagramXml {
        data: built.data,
        layout: built.layout,
        style: built.style,
        colors: built.colors,
        drawing: built.drawing,
        rels: rels_xml(pictures),
        images,
    }
}

impl SheetEngine {
    /// Frame size (EMU) of the cells `from` … `to`.
    fn frame_emu(&mut self, sheet: usize, from: CellRef, to: CellRef) -> Result<(i64, i64)> {
        let g = self.geometry(sheet)?;
        let w = g.col_x(to.col) - g.col_x(from.col);
        let h = g.row_y(to.row) - g.row_y(from.row);
        #[allow(clippy::cast_possible_truncation)]
        Ok((
            (w.max(36.0) * EMU_PT).round() as i64,
            (h.max(36.0) * EMU_PT).round() as i64,
        ))
    }

    /// Runs a BlueSheet SmartArt command.
    pub(super) fn smartart_command(&mut self, cmd: SmartArtCommand) -> Result<Value> {
        let sheet = cmd.sheet();
        let ws = self.sheet(sheet)?;
        if !matches!(
            cmd,
            SmartArtCommand::SmartArtInfo { .. }
                | SmartArtCommand::SmartArtPreviews { .. }
                | SmartArtCommand::SmartArtNodeAt { .. }
        ) {
            if self.sec.read_only {
                return Err(bad(crate::security::READ_ONLY));
            }
            super::protect::check(ws, super::protect::Action::Objects)?;
        }
        let count = ws.diagrams.len();
        let check_index = |index: usize| {
            if index < count {
                Ok(())
            } else {
                Err(bad("noSmartArt"))
            }
        };
        match cmd {
            SmartArtCommand::SmartArtInfo { index, .. } => {
                check_index(index)?;
                let wb = self.wb()?;
                Ok(info_json(wb, index, &wb.sheets[sheet].diagrams[index]))
            }
            SmartArtCommand::SmartArtNodeAt {
                index, x, y, node, ..
            } => {
                check_index(index)?;
                let d = self.wb()?.sheets[sheet].diagrams[index].clone();
                let to =
                    d.to.unwrap_or(CellRef::new(d.from.row + 10, d.from.col + 6));
                let ext = self.frame_emu(sheet, d.from, to)?;
                let wb = self.wb()?;
                let Some(src) = sources(wb, &d) else {
                    return Ok(Value::Null);
                };
                // The drawing as it shows: the diagram's own once edited, else the file's.
                let drawing = match &d.xml {
                    Some(x) => Some(x.drawing.as_slice()),
                    None => (d.part != d.data).then(|| opaque(wb, &d.part)).flatten(),
                };
                let at = x
                    .zip(y)
                    .filter(|(x, y)| (0.0..=1.0).contains(x) && (0.0..=1.0).contains(y));
                if node.is_none() && at.is_none() {
                    return Ok(Value::Null);
                }
                Ok(crate::smartart::frame_node(
                    (src.data, src.name),
                    src.layout,
                    drawing.map(|b| (b, d.part.as_str())),
                    ext,
                    at,
                    node,
                )
                .map_or(Value::Null, |n| json!(n)))
            }
            SmartArtCommand::InsertSmartArt {
                layout,
                items,
                colors,
                style,
                from,
                to,
                ..
            } => {
                let cat = Layout::parse(&layout).ok_or_else(|| bad("badSmartArt"))?;
                let lk = look(Look::default(), colors.as_deref(), style.as_deref())?;
                if items.iter().all(|i| i.text.trim().is_empty()) {
                    return Err(bad("badSmartArt"));
                }
                let mut new = NewImages::new(self.wb()?);
                let (nodes, pictures) =
                    crate::smartart::nodes_of(&items, &Vec::new(), &mut |p| Ok(new.store(p)))
                        .map_err(bad)?;
                if nodes.is_empty() {
                    return Err(bad("badSmartArt"));
                }
                let from = cell(from);
                let to = to.map_or(
                    CellRef::new(from.row.saturating_add(10), from.col.saturating_add(6)),
                    cell,
                );
                let (from, to) = span(from, to);
                let ext = self.frame_emu(sheet, from, to)?;
                let size = |rid: &str| {
                    let (_, name) = pictures.iter().find(|(id, _)| id == rid)?;
                    crate::edit::image_size(new.bytes(name)?)
                };
                let styling = dl::Styling {
                    look: lk,
                    defs: None,
                    picture_size: Some(&size),
                };
                let built =
                    dl::build_parts_with(&nodes, cat, &styling, ext, DRAWING_REL_PLACEHOLDER);
                let xml = diagram_xml(built, &pictures, None, &new);
                let index = self.edit(sheet, Recalc::No, |wb, _| {
                    let names = diagram::part_names(DIR, fresh_number(wb));
                    let ws = &mut wb.sheets[sheet];
                    ws.diagrams.push(SheetDiagram {
                        from,
                        to: Some(to),
                        part: names[4].clone(),
                        name: format!("Diagram {}", ws.diagrams.len() + 1),
                        data: names[0].clone(),
                        layout: names[1].clone(),
                        style: names[2].clone(),
                        colors: names[3].clone(),
                        rel_dm: String::new(),
                        xml: Some(Box::new(xml)),
                        moved: false,
                    });
                    Ok(ws.diagrams.len() - 1)
                })?;
                let wb = self.wb()?;
                Ok(json!({
                    "index": index,
                    "smartArt": info_json(wb, index, &wb.sheets[sheet].diagrams[index]),
                }))
            }
            SmartArtCommand::SetSmartArt {
                index,
                layout,
                items,
                colors,
                style,
                rtl,
                ..
            } => {
                check_index(index)?;
                let d = self.wb()?.sheets[sheet].diagrams[index].clone();
                let to =
                    d.to.unwrap_or(CellRef::new(d.from.row + 10, d.from.col + 6));
                let ext = self.frame_emu(sheet, d.from, to)?;
                let xml = {
                    let wb = self.wb()?;
                    let (old_nodes, old_cat, old_look) =
                        read(wb, &d).ok_or_else(|| bad("noSmartArt"))?;
                    let src = sources(wb, &d).ok_or_else(|| bad("noSmartArt"))?;
                    // A file diagram keeps the drawing relationship its data model names (none:
                    // the data model names no drawing); a new one gets it when written.
                    let rel = if d.rel_dm.is_empty() {
                        DRAWING_REL_PLACEHOLDER.to_owned()
                    } else {
                        bo_pptx::diagram::drawing_rel_id(src.data, &d.data).unwrap_or_default()
                    };
                    let cat = match layout.as_deref() {
                        Some(l) => Layout::parse(l).ok_or_else(|| bad("badSmartArt"))?,
                        None => old_cat,
                    };
                    let mut lk = look(old_look, colors.as_deref(), style.as_deref())?;
                    lk.rtl = rtl.unwrap_or(lk.rtl);
                    let mut new = NewImages::new(wb);
                    let (nodes, pictures) = if let Some(i) = items.as_deref() {
                        crate::smartart::nodes_of(i, &src.pictures, &mut |p| Ok(new.store(p)))
                            .map_err(bad)?
                    } else {
                        let used = crate::smartart::used_pictures(&old_nodes, &src.pictures);
                        (old_nodes, used)
                    };
                    if nodes.is_empty() {
                        return Err(bad("badSmartArt"));
                    }
                    let size = |rid: &str| {
                        let (_, name) = pictures.iter().find(|(id, _)| id == rid)?;
                        crate::edit::image_size(new.bytes(name).or_else(|| image_bytes(wb, name))?)
                    };
                    let defs = src.defs();
                    let styling = dl::Styling {
                        look: lk,
                        defs: Some(&defs),
                        picture_size: Some(&size),
                    };
                    let built = dl::build_parts_with(&nodes, cat, &styling, ext, &rel);
                    diagram_xml(built, &pictures, d.xml.as_deref(), &new)
                };
                self.edit(sheet, Recalc::No, |wb, _| {
                    wb.sheets[sheet].diagrams[index].xml = Some(Box::new(xml));
                    Ok(())
                })?;
                let wb = self.wb()?;
                Ok(json!({
                    "index": index,
                    "smartArt": info_json(wb, index, &wb.sheets[sheet].diagrams[index]),
                }))
            }
            SmartArtCommand::MoveSmartArt {
                index, from, to, ..
            } => {
                check_index(index)?;
                let (from, to) = span(cell(from), cell(to));
                // The drawing is laid out again for the new frame size (a file diagram is then
                // written as edited).
                let ext = self.frame_emu(sheet, from, to)?;
                let drawing = {
                    let wb = self.wb()?;
                    let d = &wb.sheets[sheet].diagrams[index];
                    d.xml
                        .as_ref()
                        .and_then(|_| Some((read(wb, d)?, sources(wb, d)?)))
                        .map(|((nodes, cat, lk), src)| {
                            let size = |rid: &str| {
                                let (_, name) = src.pictures.iter().find(|(id, _)| id == rid)?;
                                crate::edit::image_size(image_bytes(wb, name)?)
                            };
                            let defs = src.defs();
                            let styling = dl::Styling {
                                look: lk,
                                defs: Some(&defs),
                                picture_size: Some(&size),
                            };
                            dl::drawing_xml_with(&nodes, cat, &styling, ext.0, ext.1).into_bytes()
                        })
                };
                self.edit(sheet, Recalc::No, |wb, _| {
                    let d = &mut wb.sheets[sheet].diagrams[index];
                    d.from = from;
                    d.to = Some(to);
                    d.moved = true;
                    if let (Some(x), Some(drawing)) = (d.xml.as_mut(), drawing) {
                        x.drawing = drawing;
                    }
                    Ok(())
                })?;
                Ok(json!({ "ok": true }))
            }
            SmartArtCommand::DeleteSmartArt { index, .. } => {
                check_index(index)?;
                self.edit(sheet, Recalc::No, |wb, _| {
                    if bo_xlsx::diagram_write::remove_diagram(wb, sheet, index) {
                        Ok(json!({ "ok": true }))
                    } else {
                        Err(bad("noSmartArt"))
                    }
                })
            }
            SmartArtCommand::SmartArtToShapes { index, .. } => {
                check_index(index)?;
                let d = self.wb()?.sheets[sheet].diagrams[index].clone();
                let to =
                    d.to.unwrap_or(CellRef::new(d.from.row + 10, d.from.col + 6));
                let ext = self.frame_emu(sheet, d.from, to)?;
                let (grp_sp, images) = {
                    let wb = self.wb()?;
                    let src = sources(wb, &d).ok_or_else(|| bad("noSmartArt"))?;
                    // The drawing as it shows: the drawing part, else the data model laid out.
                    let own = d.xml.as_ref().map(|x| x.drawing.clone());
                    let file = (d.part != d.data)
                        .then(|| opaque(wb, &d.part).map(<[u8]>::to_vec))
                        .flatten();
                    let (drawing, rels) = if let Some(x) = own.or(file) {
                        (x, &src.drawing_pictures)
                    } else {
                        let (nodes, cat, lk) = read(wb, &d).ok_or_else(|| bad("noSmartArt"))?;
                        let defs = src.defs();
                        let styling = dl::Styling {
                            look: lk,
                            defs: Some(&defs),
                            picture_size: None,
                        };
                        (
                            dl::drawing_xml_with(&nodes, cat, &styling, ext.0, ext.1).into_bytes(),
                            &src.pictures,
                        )
                    };
                    let resolve = |rid: &str| {
                        rels.iter()
                            .find(|(id, _)| id == rid)
                            .map(|(_, name)| name.clone())
                    };
                    let grp = crate::smartart::sheet_group_xml(&drawing, ext, &resolve)
                        .ok_or_else(|| bad("noSmartArt"))?;
                    // The pictures the diagram added go with the group; the file's stay parts.
                    let images: Vec<(String, Vec<u8>)> = rels
                        .iter()
                        .filter(|(_, name)| opaque(wb, name).is_none())
                        .filter_map(|(_, name)| {
                            Some((name.clone(), image_bytes(wb, name)?.to_vec()))
                        })
                        .collect();
                    (grp, images)
                };
                self.edit(sheet, Recalc::No, |wb, _| {
                    let mut n = 1;
                    let key = loop {
                        let cand = diagram::group_name("/xl", n);
                        if wb
                            .sheets
                            .iter()
                            .flat_map(|s| &s.shape_groups)
                            .all(|g| g.key != cand)
                        {
                            break cand;
                        }
                        n += 1;
                    };
                    if !bo_xlsx::diagram_write::remove_diagram(wb, sheet, index) {
                        return Err(bad("noSmartArt"));
                    }
                    wb.sheets[sheet]
                        .shape_groups
                        .push(bo_model::sheet::SheetShapeGroup {
                            from: d.from,
                            to: Some(to),
                            key,
                            grp_sp,
                            images,
                            in_file: false,
                        });
                    Ok(json!({ "ok": true }))
                })
            }
            SmartArtCommand::SmartArtPreviews {
                colors,
                style,
                width,
                height,
                items,
                rtl,
            } => {
                let mut lk = look(Look::default(), colors.as_deref(), style.as_deref())?;
                lk.rtl = rtl.unwrap_or(false);
                Ok(json!({
                    "previews": crate::smartart::layout_previews_of(
                        lk,
                        items.as_deref(),
                        width.unwrap_or(96.0),
                        height.unwrap_or(64.0),
                        &mut self.fonts,
                    ),
                }))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::SheetEngine;
    use serde_json::{json, Value};

    fn run(e: &mut SheetEngine, v: &Value) -> Value {
        serde_json::from_str(&e.exec_json(&v.to_string()).unwrap()).unwrap()
    }

    fn items(v: &[(&str, usize)]) -> Value {
        json!(v
            .iter()
            .map(|(t, l)| json!({"text": t, "level": l}))
            .collect::<Vec<_>>())
    }

    // source: black-box Office SmartArt: a click on a shape inside a diagram over the cells picks
    // that shape; a hierarchy puts its top shape above the ones under it. Not an undo step.
    #[test]
    fn shape_under_a_point_of_the_frame() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        run(
            &mut e,
            &json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "hierarchy", "from": [1, 1],
                "items": items(&[("Giám đốc", 0), ("Kỹ thuật", 1), ("Kinh doanh", 1)])}),
        );
        let depth = e.history_len();
        let at = |e: &mut SheetEngine, v: Value| {
            let mut v = v;
            v["cmd"] = json!("smartArtNodeAt");
            v["sheet"] = json!(0);
            v["index"] = json!(0);
            run(e, &v)
        };
        let top = at(&mut e, json!({ "node": 0 }));
        let left = at(&mut e, json!({ "node": 1 }));
        assert_eq!(top["text"], "Giám đốc");
        let f = |b: &Value, k: &str| b[k].as_f64().unwrap();
        for b in [&top, &left] {
            assert!(
                f(b, "x") >= 0.0 && f(b, "x") + f(b, "w") <= 1.0 + 1e-9,
                "{b}"
            );
            assert!(
                f(b, "y") >= 0.0 && f(b, "y") + f(b, "h") <= 1.0 + 1e-9,
                "{b}"
            );
        }
        assert!(
            f(&top, "y") + f(&top, "h") <= f(&left, "y") + 1e-9,
            "{top} {left}"
        );
        // The middle of a shape picks it; empty room or a point off the frame picks nothing.
        let hit = at(
            &mut e,
            json!({ "x": f(&left, "x") + f(&left, "w") / 2.0, "y": f(&left, "y") + f(&left, "h") / 2.0 }),
        );
        assert_eq!(
            (hit["node"].clone(), hit["text"].clone()),
            (json!(1), json!("Kỹ thuật"))
        );
        assert!(at(&mut e, json!({ "x": 1.5, "y": 0.5 })).is_null());
        assert!(at(&mut e, json!({})).is_null());
        assert_eq!(e.history_len(), depth);
        assert!(exec_err(
            &mut e,
            &json!({"cmd": "smartArtNodeAt", "sheet": 0, "index": 4, "node": 0})
        ));
    }

    fn exec_err(e: &mut SheetEngine, v: &Value) -> bool {
        e.exec_json(&v.to_string()).is_err()
    }

    // source: black-box Office SmartArt Design tab (CORE-SMART-004): Right to Left and Add
    // Assistant on a sheet, one undo step each (layout variable `dgm:dir`, ST_PtType `asst`).
    #[test]
    fn design_actions_rtl_and_assistant() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        run(
            &mut e,
            &json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "hierarchy", "from": [1, 1], "items": items(&[("Một", 0), ("Hai", 1)])}),
        );
        let r = run(
            &mut e,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "rtl": true}),
        );
        assert_eq!(r["smartArt"]["rtl"], true);
        let r = run(
            &mut e,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "items": [
                {"text": "Một", "level": 0},
                {"text": "Trợ lý", "level": 1, "assistant": true},
                {"text": "Hai", "level": 1}
            ]}),
        );
        assert_eq!(r["smartArt"]["items"][1]["assistant"], true);
        assert_eq!(r["smartArt"]["rtl"], true, "the direction stays");
        e.undo().unwrap();
        let r = run(
            &mut e,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(r["items"].as_array().unwrap().len(), 2);
        assert_eq!(r["rtl"], true);
        e.undo().unwrap();
        let r = run(
            &mut e,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(r["rtl"], false);
    }

    #[test]
    fn insert_edit_move_undo_save_delete() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        let r = run(
            &mut e,
            &json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "process", "from": [1, 1], "items": items(&[("Một", 0), ("Hai", 0)])}),
        );
        assert_eq!(r["index"], 0);
        assert_eq!(r["smartArt"]["layout"], "process");
        assert_eq!(r["smartArt"]["to"], json!([11, 7]));
        // Text pane: add a child, change colours and look.
        let r = run(
            &mut e,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "layout": "hierarchy", "colors": "orange", "style": "outline", "items": items(&[("Một", 0), ("Con", 1), ("Hai", 0)])}),
        );
        assert_eq!(r["smartArt"]["items"][1]["level"], 1);
        assert_eq!(r["smartArt"]["colors"], "orange");
        assert_eq!(r["smartArt"]["style"], "outline");
        run(
            &mut e,
            &json!({"cmd": "moveSmartArt", "sheet": 0, "index": 0, "from": [3, 2], "to": [12, 8]}),
        );
        // Undo the move, then redo it.
        e.undo().unwrap();
        let i = run(
            &mut e,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["from"], json!([1, 1]));
        e.redo().unwrap();
        let bytes = e.save("xlsx", 0).unwrap();
        // BO_SMARTART_OUT=<dir>: keep the file for `bo-cli validate`.
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/smartart-sheet.xlsx"), &bytes).unwrap();
        }
        let mut back = SheetEngine::new();
        back.open(&bytes, "back.xlsx").unwrap();
        let i = run(
            &mut back,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["layout"], "hierarchy");
        assert_eq!(i["colors"], "orange");
        assert_eq!(i["from"], json!([3, 2]));
        assert_eq!(i["items"][1]["text"], "Con");
        // Edit the file diagram (rewritten in place), then delete it.
        let r = run(
            &mut back,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "colors": "green"}),
        );
        assert_eq!(r["smartArt"]["colors"], "green");
        let again = back.save("xlsx", 0).unwrap();
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/smartart-sheet-edited.xlsx"), &again).unwrap();
        }
        let mut third = SheetEngine::new();
        third.open(&again, "third.xlsx").unwrap();
        let i = run(
            &mut third,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["colors"], "green");
        assert_eq!(i["items"][1]["text"], "Con");
        run(
            &mut third,
            &json!({"cmd": "deleteSmartArt", "sheet": 0, "index": 0}),
        );
        let gone = third.save("xlsx", 0).unwrap();
        let pkg = bo_opc::Package::read(&gone, &bo_opc::Limits::default()).unwrap();
        assert!(pkg.parts().iter().all(|p| !p.name.contains("/diagrams/")));
        let mut fourth = SheetEngine::new();
        fourth.open(&gone, "fourth.xlsx").unwrap();
        assert!(fourth
            .exec_json(&json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}).to_string())
            .is_err());
    }

    // source: ECMA-376 Part 1, §21.4.2.15 layoutDef — the layouts of the library beyond the first
    // of each category (CORE-SMART-003), on a sheet.
    #[test]
    fn library_layouts_on_a_sheet() {
        use bo_pptx::diagram_layout::Layout;
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        let r = run(
            &mut e,
            &json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "cycleRadial", "from": [1, 1], "items": items(&[("Tâm", 0), ("A", 0), ("B", 0)])}),
        );
        assert_eq!(r["smartArt"]["layout"], "cycleRadial");
        for layout in Layout::ALL {
            let r = run(
                &mut e,
                &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "layout": layout.name()}),
            );
            assert_eq!(r["smartArt"]["layout"], layout.name());
            assert_eq!(r["smartArt"]["items"][2]["text"], "B");
        }
        let bytes = e.save("xlsx", 0).unwrap();
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/smartart-sheet-picture.xlsx"), &bytes).unwrap();
        }
        let mut back = SheetEngine::new();
        back.open(&bytes, "back.xlsx").unwrap();
        let i = run(
            &mut back,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["layout"], "pictureStrip");
        assert_eq!(i["items"], items(&[("Tâm", 0), ("A", 0), ("B", 0)]));
    }

    #[test]
    fn refusals() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        let bad = |e: &mut SheetEngine, v: Value| e.exec_json(&v.to_string()).unwrap_err();
        bad(
            &mut e,
            json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "nope", "items": items(&[("A", 0)])}),
        );
        bad(
            &mut e,
            json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "list", "items": items(&[(" ", 0)])}),
        );
        bad(
            &mut e,
            json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "colors": "blue"}),
        );
        bad(
            &mut e,
            json!({"cmd": "deleteSmartArt", "sheet": 0, "index": 3}),
        );
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

    fn save(e: &mut SheetEngine, name: &str) -> Vec<u8> {
        let bytes = e.save("xlsx", 0).unwrap();
        // BO_SMARTART_OUT=<dir>: keep the file for `bo-cli validate`.
        if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
            std::fs::write(format!("{dir}/{name}"), &bytes).unwrap();
        }
        bytes
    }

    fn view() -> crate::sheet::ViewRequest {
        crate::sheet::ViewRequest {
            sheet: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            width: 900.0,
            height: 500.0,
        }
    }

    // source: ECMA-376 Part 1, §20.1.8.14 blipFill in `dgm:pt/dgm:spPr` and the drawing part;
    // Part 2 relationships of both parts; §20.5.2.17 grpSp for the converted diagram.
    #[test]
    fn picture_nodes_and_convert_to_shapes_on_a_sheet() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        let (red, blue) = (png_url(40, 10, [200, 30, 30]), png_url(8, 8, [30, 30, 200]));
        let r = run(
            &mut e,
            &json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "pictureStrip", "from": [1, 1], "items": [
                {"text": "Hà Nội", "level": 0, "picture": red},
                {"text": "Huế", "level": 0},
                {"text": "Đà Nẵng", "level": 0, "picture": blue},
            ]}),
        );
        assert_eq!(r["smartArt"]["items"][0]["picture"], "rIdImg1");
        assert_eq!(r["smartArt"]["items"][2]["picture"], "rIdImg2");
        // Drawn over the cells with both pictures; their bytes are served to the canvas.
        let page = e.render(&view()).unwrap().to_json();
        assert_eq!(page.matches("/xl/media/image").count(), 2, "{page}");
        let m = run(
            &mut e,
            &json!({"cmd": "cellMedia", "id": "/xl/media/image1.png"}),
        );
        assert!(m.as_str().is_some_and(|b| b.starts_with("iVBOR")), "{m}");
        // Replace, move and remove pictures; the layout change keeps them.
        let green = png_url(4, 4, [30, 200, 30]);
        let r = run(
            &mut e,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "layout": "picture", "items": [
                {"text": "Hà Nội", "level": 0, "picture": green},
                {"text": "Huế", "level": 0, "picture": "rIdImg1"},
                {"text": "Đà Nẵng", "level": 0},
            ]}),
        );
        assert_eq!(r["smartArt"]["items"][0]["picture"], "rIdImg3");
        assert_eq!(r["smartArt"]["items"][1]["picture"], "rIdImg1");
        let bytes = save(&mut e, "smartart-sheet-pictures.xlsx");
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        assert_eq!(
            pkg.parts()
                .iter()
                .filter(|p| p.name.starts_with("/xl/media/"))
                .count(),
            2,
            "the removed picture is not written"
        );
        for part in ["/xl/diagrams/data1.xml", "/xl/diagrams/drawing1.xml"] {
            let rels = pkg.relationships(part).unwrap();
            assert_eq!(rels.len(), 2, "{part}");
            for r in rels {
                assert!(pkg.part(&bo_opc::resolve_target(part, &r.target)).is_some());
            }
        }
        // Reopened: the pictures are read from the file; edited in place they are kept.
        let mut back = SheetEngine::new();
        back.open(&bytes, "back.xlsx").unwrap();
        let i = run(
            &mut back,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["items"][0]["picture"], "rIdImg3");
        assert_eq!(i["items"][1]["picture"], "rIdImg1");
        assert_eq!(
            back.render(&view())
                .unwrap()
                .to_json()
                .matches("/xl/media/image")
                .count(),
            2
        );
        let r = run(
            &mut back,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "colors": "green"}),
        );
        assert_eq!(r["smartArt"]["items"][1]["picture"], "rIdImg1");
        let again = back.save("xlsx", 0).unwrap();
        let pkg = bo_opc::Package::read(&again, &bo_opc::Limits::default()).unwrap();
        assert_eq!(
            pkg.relationships("/xl/diagrams/data1.xml").unwrap().len(),
            2
        );
        // Convert to shapes: the diagram is gone, a group of shapes takes its cells.
        let r = run(
            &mut e,
            &json!({"cmd": "smartArtToShapes", "sheet": 0, "index": 0}),
        );
        assert_eq!(r["ok"], true);
        assert!(e
            .exec_json(&json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}).to_string())
            .is_err());
        let page = e.render(&view()).unwrap().to_json();
        assert_eq!(
            page.matches("/xl/media/image").count(),
            2,
            "still drawn with its pictures"
        );
        let bytes = save(&mut e, "smartart-sheet-converted.xlsx");
        let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        assert!(pkg.parts().iter().all(|p| !p.name.contains("/diagrams/")));
        let drawing = pkg
            .parts()
            .iter()
            .find(|p| p.name.starts_with("/xl/drawings/drawing"))
            .unwrap();
        let x = String::from_utf8_lossy(&drawing.data).into_owned();
        assert_eq!(
            x.matches("<xdr:grpSp ").count() + x.matches("<xdr:grpSp>").count(),
            1,
            "{x}"
        );
        // Three cards: a frame and a caption each, plus the two marks of the empty frame.
        assert_eq!(x.matches("<xdr:sp macro=\"\" textlink=\"\">").count(), 8);
        assert!(x.contains("<a:t>Hà Nội</a:t>"));
        let rels = pkg.relationships(&drawing.name).unwrap();
        assert_eq!(
            rels.iter()
                .filter(|r| r.rel_type.ends_with("/image"))
                .count(),
            2
        );
        for r in rels.iter().filter(|r| r.rel_type.ends_with("/image")) {
            assert!(x.contains(&format!("r:embed=\"{}\"", r.id)));
            assert!(pkg
                .part(&bo_opc::resolve_target(&drawing.name, &r.target))
                .is_some());
        }
        // Reopened: the group is read from the drawing and drawn; saving keeps the drawing part.
        let mut back = SheetEngine::new();
        back.open(&bytes, "back.xlsx").unwrap();
        assert_eq!(
            back.render(&view())
                .unwrap()
                .to_json()
                .matches("/xl/media/image")
                .count(),
            2
        );
        let again = back.save("xlsx", 0).unwrap();
        let pkg2 = bo_opc::Package::read(&again, &bo_opc::Limits::default()).unwrap();
        assert_eq!(pkg2.part(&drawing.name).unwrap().data, drawing.data);
        // One undo step brings the diagram back.
        e.undo().unwrap();
        let i = run(
            &mut e,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["layout"], "picture");
        // Layout thumbnails for the dialog.
        let p = run(
            &mut e,
            &json!({"cmd": "smartArtPreviews", "colors": "gray"}),
        );
        assert_eq!(p["previews"].as_array().unwrap().len(), 17);
    }

    const FILE_COLORS: &str = r#"<dgm:colorsDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" uniqueId="urn:example/colors/test1"><dgm:styleLbl name="node1"><dgm:fillClrLst meth="repeat"><a:srgbClr val="AA1122"/><a:srgbClr val="11AA22"/></dgm:fillClrLst></dgm:styleLbl></dgm:colorsDef>"#;

    // source: ECMA-376 Part 1, §21.4 colorsDef — the file's own colours on a sheet.
    #[test]
    fn the_files_own_colours_on_a_sheet() {
        let mut e = SheetEngine::new();
        e.new_workbook("Book").unwrap();
        run(
            &mut e,
            &json!({"cmd": "insertSmartArt", "sheet": 0, "layout": "process", "from": [1, 1], "items": items(&[("Một", 0), ("Hai", 0), ("Ba", 0)])}),
        );
        // As another application writes it: its own colours part, named on the document point.
        let bytes = e.save("xlsx", 0).unwrap();
        let mut pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
        pkg.put_part("/xl/diagrams/colors1.xml", FILE_COLORS.as_bytes().to_vec());
        let d =
            String::from_utf8(pkg.part("/xl/diagrams/data1.xml").unwrap().data.clone()).unwrap();
        let at = d.find("csTypeId=\"").unwrap() + 10;
        let end = at + d[at..].find('"').unwrap();
        pkg.put_part(
            "/xl/diagrams/data1.xml",
            format!("{}urn:example/colors/test1{}", &d[..at], &d[end..]).into_bytes(),
        );
        let mut f = SheetEngine::new();
        f.open(&pkg.write().unwrap(), "file.xlsx").unwrap();
        let i = run(
            &mut f,
            &json!({"cmd": "smartArtInfo", "sheet": 0, "index": 0}),
        );
        assert_eq!(i["colors"], "file");
        let r = run(
            &mut f,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "items": items(&[("Một", 0), ("Hai", 0), ("Ba", 0), ("Bốn", 0)])}),
        );
        assert_eq!(r["smartArt"]["colors"], "file");
        let saved = f.save("xlsx", 0).unwrap();
        let out = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
        assert_eq!(
            out.part("/xl/diagrams/colors1.xml").unwrap().data,
            FILE_COLORS.as_bytes()
        );
        let drawing = String::from_utf8_lossy(&out.part("/xl/diagrams/drawing1.xml").unwrap().data)
            .into_owned();
        assert_eq!(drawing.matches(r#"<a:srgbClr val="AA1122"/>"#).count(), 2);
        assert_eq!(drawing.matches(r#"<a:srgbClr val="11AA22"/>"#).count(), 2);
        assert!(!drawing.contains("1F6FB2"));
        let r = run(
            &mut f,
            &json!({"cmd": "setSmartArt", "sheet": 0, "index": 0, "colors": "blue"}),
        );
        assert_eq!(r["smartArt"]["colors"], "blue");
    }
}
