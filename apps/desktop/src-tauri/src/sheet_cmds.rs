//! BlueSheet engine commands (native `bo_engine::SheetEngine`), mirroring
//! the WASM `WasmSheet` methods used by `RpcSheetHost` on the TS side.

use bo_engine::sheet::{SheetEngine, ViewRequest};
use serde_json::Value;
use std::sync::Mutex;
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::State;

pub struct SheetState(pub Mutex<SheetEngine>);

impl SheetState {
    pub fn new() -> Self {
        Self(Mutex::new(SheetEngine::with_bundled_fonts()))
    }
}

fn num(a: &[Value], i: usize) -> Result<f64, String> {
    a.get(i)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("argument {i}: expected a number"))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn uint(a: &[Value], i: usize) -> Result<u32, String> {
    num(a, i).map(|v| v.max(0.0) as u32)
}

fn idx(a: &[Value], i: usize) -> Result<usize, String> {
    uint(a, i).map(|v| v as usize)
}

fn text(a: &[Value], i: usize) -> Result<String, String> {
    a.get(i)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("argument {i}: expected a string"))
}

fn view(a: &[Value]) -> Result<ViewRequest, String> {
    Ok(ViewRequest {
        sheet: idx(a, 0)?,
        scroll_x: num(a, 1)?,
        scroll_y: num(a, 2)?,
        width: num(a, 3)?,
        height: num(a, 4)?,
    })
}

fn to_json<T: serde::Serialize>(v: &T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Dispatches one JSON method call (see `SheetMethod` in packages/platform).
pub fn dispatch(e: &mut SheetEngine, method: &str, a: &[Value]) -> Result<Value, String> {
    let s = |r: bo_engine::Result<()>| r.map(|()| Value::Null).map_err(|e| e.to_string());
    match method {
        "newWorkbook" => to_json(&e.new_workbook(&text(a, 0)?).map_err(|e| e.to_string())?),
        "info" => to_json(&e.info().map_err(|e| e.to_string())?),
        "render" => to_json(&e.render(&view(a)?).map_err(|e| e.to_string())?),
        "hitTest" => to_json(
            &e.hit_test(&view(a)?, num(a, 5)?, num(a, 6)?)
                .map_err(|e| e.to_string())?,
        ),
        "rangeRect" => to_json(
            &e.range_rect(
                &view(a)?,
                (uint(a, 5)?, uint(a, 6)?),
                (uint(a, 7)?, uint(a, 8)?),
            )
            .map_err(|e| e.to_string())?,
        ),
        "contentSize" => {
            let (w, h) = e.content_size(idx(a, 0)?).map_err(|e| e.to_string())?;
            to_json(&[w, h])
        }
        "cell" => to_json(
            &e.cell(idx(a, 0)?, uint(a, 1)?, uint(a, 2)?)
                .map_err(|e| e.to_string())?,
        ),
        "setInputs" => {
            let cells: Vec<(u32, u32, String)> =
                serde_json::from_str(&text(a, 1)?).map_err(|e| e.to_string())?;
            s(e.set_inputs(idx(a, 0)?, &cells))
        }
        "structural" => s(e.structural(
            idx(a, 0)?,
            a.get(1).and_then(Value::as_bool).unwrap_or(true),
            uint(a, 2)?,
            uint(a, 3)?,
            a.get(4).and_then(Value::as_bool).unwrap_or(false),
        )),
        "undo" => to_json(&e.undo().map_err(|e| e.to_string())?),
        "redo" => to_json(&e.redo().map_err(|e| e.to_string())?),
        "addSheet" => to_json(
            &e.add_sheet(a.first().and_then(Value::as_str))
                .map_err(|e| e.to_string())?,
        ),
        "renameSheet" => s(e.rename_sheet(idx(a, 0)?, &text(a, 1)?)),
        "deleteSheet" => s(e.delete_sheet(idx(a, 0)?)),
        "exec" => serde_json::from_str(&e.exec_json(&text(a, 0)?).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string()),
        // Live collaboration (bo_engine::sheet::collab), same as WasmSheet.collab*.
        "collabStart" => to_json(
            &e.collab_start(
                a.first().and_then(Value::as_bool).unwrap_or(false),
                a.get(1).and_then(Value::as_u64).unwrap_or(0),
            )
            .map_err(|e| e.to_string())?,
        ),
        "collabLocal" => to_json(&e.collab_local().map_err(|e| e.to_string())?),
        "collabApply" => serde_json::from_str(
            &e.collab_apply_json(&text(a, 0)?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string()),
        "collabStop" => {
            e.collab_stop();
            Ok(Value::Null)
        }
        "glyphs" => {
            let glyphs: Vec<u16> = a
                .get(1)
                .and_then(Value::as_array)
                .map(|v| {
                    v.iter()
                        .filter_map(Value::as_u64)
                        .filter_map(|g| u16::try_from(g).ok())
                        .collect()
                })
                .unwrap_or_default();
            to_json(&e.glyph_outlines(uint(a, 0)?, &glyphs))
        }
        other => Err(format!("unknown sheet method '{other}'")),
    }
}

#[tauri::command]
pub fn sheet_call(
    state: State<'_, SheetState>,
    method: String,
    args: Vec<Value>,
) -> Result<Value, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    dispatch(&mut e, &method, &args)
}

/// Binary results: `save` (format, sheet) and `exportPdf`.
#[tauri::command]
pub fn sheet_binary(
    state: State<'_, SheetState>,
    method: String,
    args: Vec<Value>,
) -> Result<Response, String> {
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    let bytes = match method.as_str() {
        "save" => e.save(&text(&args, 0)?, idx(&args, 1)?),
        "exportPdf" => e.export_pdf(),
        other => return Err(format!("unknown sheet method '{other}'")),
    }
    .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes))
}

/// Opens a workbook from a raw IPC body (`x-file-name` header).
#[tauri::command]
pub fn sheet_open(
    state: State<'_, SheetState>,
    fonts: State<'_, crate::FontsReady>,
    request: Request<'_>,
) -> Result<Value, String> {
    fonts.wait(crate::FONT_WAIT);
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected a binary request body".into());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map_or_else(|| "book.xlsx".into(), crate::shell::percent_decode);
    let mut e = state.0.lock().map_err(|e| e.to_string())?;
    to_json(&e.open(bytes, &name).map_err(|e| e.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dispatches_like_the_wasm_worker() {
        let mut e = SheetEngine::with_bundled_fonts();
        dispatch(&mut e, "newWorkbook", &[json!("a.xlsx")]).unwrap();
        dispatch(&mut e, "setInputs", &[json!(0), json!("[[0,0,\"5\"]]")]).unwrap();
        let c = dispatch(&mut e, "cell", &[json!(0), json!(0), json!(0)]).unwrap();
        assert_eq!(c["input"], "5");
        let hit = dispatch(
            &mut e,
            "hitTest",
            &[
                json!(0),
                json!(0),
                json!(0),
                json!(300),
                json!(200),
                json!(1),
                json!(1),
            ],
        )
        .unwrap();
        assert_eq!(hit["kind"], "corner");
        let st = dispatch(
            &mut e,
            "exec",
            &[json!(
                "{\"cmd\":\"formatState\",\"sheet\":0,\"row\":0,\"col\":0}"
            )],
        )
        .unwrap();
        assert_eq!(st["bold"], false);
        assert!(dispatch(&mut e, "nope", &[]).is_err());
        assert!(dispatch(&mut e, "cell", &[json!("x")]).is_err());
    }
}
