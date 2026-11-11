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
            | SmartArtCommand::DeleteSmartArt { sheet, .. } => *sheet,
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
    })
}

fn outline(items: &[SmartItem]) -> Vec<(String, usize)> {
    items
        .iter()
        .filter(|i| !i.text.trim().is_empty())
        .map(|i| (i.text.trim().to_owned(), i.level))
        .collect()
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

/// Data model part name, its bytes and the layout definition's bytes (if any).
type Sources<'a> = (&'a str, &'a [u8], Option<&'a [u8]>);

/// Data model (name, bytes) and layout definition of a diagram: its own parts once edited, else
/// the file's.
fn sources<'a>(wb: &'a Workbook, d: &'a SheetDiagram) -> Option<Sources<'a>> {
    if let Some(x) = &d.xml {
        return Some((
            d.data.as_str(),
            x.data.as_slice(),
            Some(x.layout.as_slice()),
        ));
    }
    Some((d.data.as_str(), opaque(wb, &d.data)?, opaque(wb, &d.layout)))
}

/// Outline, category and look of a diagram's data model.
fn read(wb: &Workbook, d: &SheetDiagram) -> Option<(Vec<dl::DiagramNode>, Layout, Look)> {
    let (name, data, layout) = sources(wb, d)?;
    let (nodes, lo) = dl::parse_data_model(data, name)?;
    Some((
        nodes,
        dl::layout_of(lo.as_deref(), layout),
        dl::look_of(data, name),
    ))
}

fn info_json(wb: &Workbook, index: usize, d: &SheetDiagram) -> Value {
    let Some((nodes, cat, lk)) = read(wb, d) else {
        return Value::Null;
    };
    let items: Vec<SmartItem> = dl::outline_of(&nodes)
        .into_iter()
        .map(|(text, level)| SmartItem { text, level })
        .collect();
    let to =
        d.to.unwrap_or(CellRef::new(d.from.row + 10, d.from.col + 6));
    json!({
        "index": index,
        "name": d.name,
        "layout": cat.name(),
        "items": items,
        "colors": lk.palette.name(),
        "style": lk.fill.name(),
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
        if !matches!(cmd, SmartArtCommand::SmartArtInfo { .. }) {
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
                let lines = outline(&items);
                let nodes = dl::nodes_from_outline(&lines);
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
                let built = dl::build_parts(&nodes, cat, lk, ext, DRAWING_REL_PLACEHOLDER);
                let index = self.edit(sheet, Recalc::No, |wb, _| {
                    let names = diagram::part_names(DIR, fresh_number(wb));
                    let ws = &mut wb.sheets[sheet];
                    ws.diagrams.push(SheetDiagram {
                        from,
                        to: Some(to),
                        part: names[4].clone(),
                        name: format!("SmartArt {}", ws.diagrams.len() + 1),
                        data: names[0].clone(),
                        layout: names[1].clone(),
                        style: names[2].clone(),
                        colors: names[3].clone(),
                        rel_dm: String::new(),
                        xml: Some(Box::new(DiagramXml {
                            data: built.data,
                            layout: built.layout,
                            style: built.style,
                            colors: built.colors,
                            drawing: built.drawing,
                        })),
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
                ..
            } => {
                check_index(index)?;
                let (d, old) = {
                    let wb = self.wb()?;
                    let d = wb.sheets[sheet].diagrams[index].clone();
                    let old = read(wb, &d).ok_or_else(|| bad("noSmartArt"))?;
                    // A file diagram keeps the drawing relationship its data model names (none:
                    // the data model names no drawing); a new one gets it when written.
                    let rel = if d.rel_dm.is_empty() {
                        DRAWING_REL_PLACEHOLDER.to_owned()
                    } else {
                        sources(wb, &d)
                            .and_then(|(_, data, _)| {
                                bo_pptx::diagram::drawing_rel_id(data, &d.data)
                            })
                            .unwrap_or_default()
                    };
                    (d, (old, rel))
                };
                let ((old_nodes, old_cat, old_look), rel) = old;
                let cat = match layout.as_deref() {
                    Some(l) => Layout::parse(l).ok_or_else(|| bad("badSmartArt"))?,
                    None => old_cat,
                };
                let lk = look(old_look, colors.as_deref(), style.as_deref())?;
                let nodes = match items.as_deref() {
                    Some(i) => dl::nodes_from_outline(&outline(i)),
                    None => old_nodes,
                };
                if nodes.is_empty() {
                    return Err(bad("badSmartArt"));
                }
                let to =
                    d.to.unwrap_or(CellRef::new(d.from.row + 10, d.from.col + 6));
                let ext = self.frame_emu(sheet, d.from, to)?;
                let built = dl::build_parts(&nodes, cat, lk, ext, &rel);
                self.edit(sheet, Recalc::No, |wb, _| {
                    let d = &mut wb.sheets[sheet].diagrams[index];
                    d.xml = Some(Box::new(DiagramXml {
                        data: built.data,
                        layout: built.layout,
                        style: built.style,
                        colors: built.colors,
                        drawing: built.drawing,
                    }));
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
                let resized = {
                    let wb = self.wb()?;
                    let d = &wb.sheets[sheet].diagrams[index];
                    read(wb, d).map(|r| (r, d.clone()))
                };
                let ext = self.frame_emu(sheet, from, to)?;
                self.edit(sheet, Recalc::No, |wb, _| {
                    let d = &mut wb.sheets[sheet].diagrams[index];
                    d.from = from;
                    d.to = Some(to);
                    d.moved = true;
                    if let (Some(x), Some(((nodes, cat, lk), _))) = (d.xml.as_mut(), resized) {
                        x.drawing = dl::drawing_xml(&nodes, cat, lk, ext.0, ext.1).into_bytes();
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
}
