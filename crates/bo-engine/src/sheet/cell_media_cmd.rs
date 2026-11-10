//! BlueSheet pictures in cells, `IMAGE` pictures and checkbox cell controls
//! (SHEET-CELL-001):
//!
//! * `insertCheckbox`, `removeCheckbox`, `toggleCheckbox`, `deleteCheckboxes` —
//!   checkbox controls carried by the cell format ([MS-XLSX] §2.3.9.4); inserting
//!   writes FALSE into empty cells (the `default` 0 of §2.3.9.4);
//! * `placeImageInCell`, `cellImageAlt`, `cellImageInfo` — pictures stored in a
//!   cell ([MS-XLSX] §2.3.6.1.10 `_localImage`);
//! * `imageRequests`, `imageLoaded`, `cellMedia` — the asynchronous fetch of
//!   `IMAGE` pictures: the engine lists the URLs it needs, the host fetches them
//!   through its platform layer and hands the bytes (or the failure) back; the
//!   renderer then draws them. `cellMedia` gives the bytes of a picture id used
//!   in the display list.
//!
//! Every edit is one undo step. Commands come as JSON (`{"cmd": "insertCheckbox", …}`)
//! and are parsed before [`super::cmd::SheetCommand`].

use super::cmd::{Recalc, Rng};
use super::SheetEngine;
use crate::picture_bg::{self as bg, MarkSpec, PriorSpec, RemovalSpec};
use crate::{EngineError, Result};
use base64::Engine as _;
use bo_model::sheet::{
    CellImage, CellImageSource, CellValue, Workbook, CALC_ORIGIN_DECORATIVE, CALC_ORIGIN_STANDALONE,
};
use bo_sheet::cell_image::{pixel_size, ImageState, ImageStates};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Largest picture accepted by `placeImageInCell` / `imageLoaded` (bytes).
pub const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;
/// Most cells one checkbox command visits (larger selections stop at the used rows).
const MAX_CELLS: u64 = 200_000;

/// Fetched `IMAGE` pictures by URL.
#[derive(Debug, Default, Clone)]
pub(crate) struct WebImages {
    bytes: BTreeMap<String, Arc<Vec<u8>>>,
    states: Arc<ImageStates>,
}

impl WebImages {
    /// States by media id, for the renderer.
    pub(crate) fn states(&self) -> Arc<ImageStates> {
        self.states.clone()
    }
}

/// A picture / checkbox command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CellMediaCommand {
    /// Give the cells of `range` the checkbox control; empty cells become FALSE.
    InsertCheckbox { sheet: usize, range: Rng },
    /// Take the checkbox control off the cells of `range` (values stay).
    RemoveCheckbox { sheet: usize, range: Rng },
    /// Flip the checkboxes of `range`: every checkbox takes the opposite of the
    /// active cell's state (`row`, `col`). Returns the new state, `null` when the
    /// active cell has no checkbox.
    ToggleCheckbox {
        sheet: usize,
        range: Rng,
        row: u32,
        col: u32,
    },
    /// Delete key on `range`: clears the checkbox values when there are any,
    /// else removes the checkbox control. `{ handled }` is false when the range
    /// has no checkbox (the host clears the cells as usual).
    DeleteCheckboxes { sheet: usize, range: Rng },
    /// Store a picture in a cell (base64 `data`, file extension `ext`).
    PlaceImageInCell {
        sheet: usize,
        row: u32,
        col: u32,
        data: String,
        ext: String,
        #[serde(default)]
        alt: Option<String>,
    },
    /// Alternative text of a cell picture; `decorative` marks it decorative.
    CellImageAlt {
        sheet: usize,
        row: u32,
        col: u32,
        #[serde(default)]
        alt: Option<String>,
        #[serde(default)]
        decorative: bool,
    },
    /// `{ kind: "local" | "web", alt, decorative, url, sizing, width, height }` or `null`.
    CellImageInfo { sheet: usize, row: u32, col: u32 },
    /// `{ checkbox, value, image }` of a cell: whether it shows a checkbox (and its
    /// boolean value) and its picture as [`CellMediaCommand::CellImageInfo`].
    CellMediaState { sheet: usize, row: u32, col: u32 },
    /// URLs of `IMAGE` pictures not fetched yet (they are then marked pending).
    ImageRequests,
    /// The host fetched `url`: base64 `data`, or `error` (`blocked` / anything else).
    ImageLoaded {
        url: String,
        #[serde(default)]
        data: Option<String>,
        #[serde(default)]
        error: Option<String>,
    },
    /// Base64 bytes of a display-list picture id (`cellimg:…` or `url:…`), `null` if unknown.
    CellMedia { id: String },
    /// Preview of removing the background of a cell picture (CORE-PIC-002): the
    /// proposed removal tinted ([`crate::picture_bg::BackgroundPreview`]); no change.
    CellImageBackgroundPreview {
        sheet: usize,
        row: u32,
        col: u32,
        #[serde(default)]
        rect: Option<[f64; 4]>,
        #[serde(default)]
        marks: Vec<MarkSpec>,
        #[serde(default)]
        prior: Option<PriorSpec>,
        #[serde(default)]
        source: bool,
    },
    /// Remove the background of a cell picture (one undo step): the cell shows a PNG
    /// with alpha. [MS-XLSX] rich values hold one picture, so the original lives in the
    /// undo history (undo restores it), not in the file.
    CellImageRemoveBackground {
        sheet: usize,
        row: u32,
        col: u32,
        #[serde(default)]
        rect: Option<[f64; 4]>,
        #[serde(default)]
        marks: Vec<MarkSpec>,
        #[serde(default)]
        prior: Option<PriorSpec>,
    },
}

impl CellMediaCommand {
    /// Parses a JSON command when it is one of these.
    pub fn parse(v: &Value) -> Option<CellMediaCommand> {
        const NAMES: &[&str] = &[
            "insertCheckbox",
            "removeCheckbox",
            "toggleCheckbox",
            "deleteCheckboxes",
            "placeImageInCell",
            "cellImageAlt",
            "cellImageInfo",
            "cellMediaState",
            "imageRequests",
            "imageLoaded",
            "cellMedia",
            "cellImageBackgroundPreview",
            "cellImageRemoveBackground",
        ];
        let name = v.get("cmd")?.as_str()?;
        if !NAMES.contains(&name) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

fn bad(msg: &str) -> EngineError {
    EngineError::Sheet(msg.to_owned())
}

fn decode(data: &str) -> Result<Vec<u8>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| bad("bad picture data"))?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(bad("picture too large or empty"));
    }
    Ok(bytes)
}

/// Cells of `r` to visit: the whole range when small, else down to the last used row.
fn cells_of(wb: &Workbook, sheet: usize, r: Rng) -> Vec<(u32, u32)> {
    let range = r.range();
    let (r0, c0, c1) = (range.first.row, range.first.col, range.last.col);
    let mut r1 = range.last.row;
    let n = u64::from(r1 - r0 + 1) * u64::from(c1 - c0 + 1);
    if n > MAX_CELLS {
        let last = wb
            .sheets
            .get(sheet)
            .and_then(|ws| ws.cells.keys().map(|k| k.0).max())
            .unwrap_or(r0);
        r1 = r1.min(last.max(r0));
    }
    (r0..=r1)
        .flat_map(|row| (c0..=c1).map(move |c| (row, c)))
        .take(usize::try_from(MAX_CELLS).unwrap_or(usize::MAX))
        .collect()
}

fn is_checkbox(wb: &Workbook, sheet: usize, row: u32, col: u32) -> bool {
    wb.sheets
        .get(sheet)
        .and_then(|ws| ws.cell(row, col))
        .and_then(|c| wb.styles.xf(c.style))
        .is_some_and(|x| x.checkbox)
}

/// Style index equal to `style` with the checkbox flag set to `on`.
fn with_checkbox(wb: &mut Workbook, cache: &mut BTreeMap<u32, u32>, style: u32, on: bool) -> u32 {
    if let Some(s) = cache.get(&style) {
        return *s;
    }
    let mut xf = wb.styles.xf(style).cloned().unwrap_or_default();
    xf.checkbox = on;
    let s = wb.styles.add_xf(xf);
    cache.insert(style, s);
    s
}

impl SheetEngine {
    /// Runs a picture / checkbox command.
    pub fn cell_media(&mut self, cmd: CellMediaCommand) -> Result<Value> {
        use CellMediaCommand as C;
        match cmd {
            C::InsertCheckbox { sheet, range } => {
                self.edit(sheet, Recalc::All, |wb, _| {
                    let cells = cells_of(wb, sheet, range);
                    let mut cache = BTreeMap::new();
                    for (r, c) in cells {
                        let style = wb.sheets[sheet].cell(r, c).map_or(0, |c| c.style);
                        let s = with_checkbox(wb, &mut cache, style, true);
                        let cell = wb.sheets[sheet].cells.entry((r, c)).or_default();
                        cell.style = s;
                        if matches!(cell.value, CellValue::Empty) && cell.formula.is_none() {
                            cell.value = CellValue::Bool(false);
                        }
                    }
                    Ok(())
                })?;
                Ok(Value::Null)
            }
            C::RemoveCheckbox { sheet, range } => {
                self.remove_checkbox(sheet, range)?;
                Ok(Value::Null)
            }
            C::ToggleCheckbox {
                sheet,
                range,
                row,
                col,
            } => {
                let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
                if !is_checkbox(wb, sheet, row, col) {
                    return Ok(Value::Null);
                }
                let on = !matches!(
                    wb.sheets[sheet].cell(row, col).map(|c| &c.value),
                    Some(CellValue::Bool(true))
                );
                self.edit(sheet, Recalc::All, |wb, _| {
                    let mut cells = cells_of(wb, sheet, range);
                    if !cells.contains(&(row, col)) {
                        cells.push((row, col));
                    }
                    for (r, c) in cells {
                        if !is_checkbox(wb, sheet, r, c) {
                            continue;
                        }
                        if let Some(cell) = wb.sheets[sheet].cells.get_mut(&(r, c)) {
                            // A formula cell shows its result; only constants toggle.
                            if cell.formula.is_none() {
                                cell.value = CellValue::Bool(on);
                            }
                        }
                    }
                    Ok(())
                })?;
                Ok(json!(on))
            }
            C::DeleteCheckboxes { sheet, range } => {
                let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
                let cells: Vec<(u32, u32)> = cells_of(wb, sheet, range)
                    .into_iter()
                    .filter(|&(r, c)| is_checkbox(wb, sheet, r, c))
                    .collect();
                if cells.is_empty() {
                    return Ok(json!({ "handled": false }));
                }
                let has_values = cells.iter().any(|&(r, c)| {
                    wb.sheets[sheet].cell(r, c).is_some_and(|x| {
                        !matches!(x.value, CellValue::Empty) || x.formula.is_some()
                    })
                });
                if has_values {
                    // First Delete: the values go, the control stays (shown unchecked).
                    self.edit(sheet, Recalc::All, |wb, _| {
                        for (r, c) in cells {
                            if let Some(cell) = wb.sheets[sheet].cells.get_mut(&(r, c)) {
                                cell.value = CellValue::Empty;
                                cell.formula = None;
                                cell.vm = None;
                                cell.image = None;
                            }
                        }
                        Ok(())
                    })?;
                    Ok(json!({ "handled": true, "removed": "values" }))
                } else {
                    // Second Delete: the control goes.
                    self.remove_checkbox(sheet, range)?;
                    Ok(json!({ "handled": true, "removed": "checkbox" }))
                }
            }
            C::PlaceImageInCell {
                sheet,
                row,
                col,
                data,
                ext,
                alt,
            } => {
                let bytes = decode(&data)?;
                if pixel_size(&bytes).is_none() {
                    return Err(bad("unsupported picture format"));
                }
                let ext = ext.trim_start_matches('.').to_ascii_lowercase();
                let alt = alt.filter(|a| !a.trim().is_empty());
                self.edit(sheet, Recalc::All, |wb, _| {
                    let cell = wb.sheets[sheet].cells.entry((row, col)).or_default();
                    cell.formula = None;
                    // [MS-XLSX] §2.3.6: the fallback value of the rich value.
                    cell.value = CellValue::Error(bo_model::sheet::CellError::Value);
                    cell.vm = None;
                    cell.image = Some(Box::new(CellImage::local(bytes, &ext, alt)));
                    Ok(())
                })?;
                Ok(Value::Null)
            }
            C::CellImageAlt {
                sheet,
                row,
                col,
                alt,
                decorative,
            } => {
                let has = self
                    .wb
                    .as_ref()
                    .and_then(|wb| wb.sheets.get(sheet))
                    .and_then(|ws| ws.cell(row, col))
                    .and_then(|c| c.image.as_deref())
                    .is_some_and(|i| matches!(i.source, CellImageSource::Local { .. }));
                if !has {
                    return Err(bad("no picture in this cell"));
                }
                self.edit(sheet, Recalc::No, |wb, _| {
                    if let Some(cell) = wb.sheets[sheet].cells.get_mut(&(row, col)) {
                        if let Some(img) = cell.image.as_deref_mut() {
                            img.alt_text = alt.filter(|a| !a.trim().is_empty());
                            img.calc_origin = if decorative {
                                CALC_ORIGIN_DECORATIVE
                            } else {
                                CALC_ORIGIN_STANDALONE
                            };
                        }
                        // A changed rich value is written as a new one.
                        cell.vm = None;
                    }
                    Ok(())
                })?;
                Ok(Value::Null)
            }
            C::CellImageInfo { sheet, row, col } => {
                let img = self
                    .wb
                    .as_ref()
                    .and_then(|wb| wb.sheets.get(sheet))
                    .and_then(|ws| ws.cell(row, col))
                    .and_then(|c| c.image.as_deref());
                Ok(img.map_or(Value::Null, |i| match &i.source {
                    CellImageSource::Local { .. } => json!({
                        "kind": "local",
                        "alt": i.alt_text,
                        "decorative": i.calc_origin == CALC_ORIGIN_DECORATIVE,
                    }),
                    CellImageSource::Web {
                        url,
                        sizing,
                        height,
                        width,
                    } => json!({
                        "kind": "web",
                        "alt": i.alt_text,
                        "url": url,
                        "sizing": sizing.code(),
                        "height": height,
                        "width": width,
                        "state": match self.web_images.states.get(&i.media_id()) {
                            Some(ImageState::Ready { .. }) => "ready",
                            Some(ImageState::Failed(_)) => "failed",
                            _ => "pending",
                        },
                    }),
                }))
            }
            C::CellMediaState { sheet, row, col } => {
                let wb = self.wb.as_ref().ok_or(EngineError::NoDocument)?;
                let checkbox = is_checkbox(wb, sheet, row, col);
                let value = wb
                    .sheets
                    .get(sheet)
                    .and_then(|ws| ws.cell(row, col))
                    .and_then(|c| match c.value {
                        CellValue::Bool(b) => Some(b),
                        _ => None,
                    });
                let image = self.cell_media(C::CellImageInfo { sheet, row, col })?;
                Ok(json!({ "checkbox": checkbox, "value": value, "image": image }))
            }
            C::ImageRequests => {
                let mut urls: Vec<String> = Vec::new();
                if let Some(wb) = self.wb.as_ref() {
                    for ws in &wb.sheets {
                        for c in ws.cells.values() {
                            if let Some(CellImageSource::Web { url, .. }) =
                                c.image.as_deref().map(|i| &i.source)
                            {
                                let id = format!("url:{url}");
                                if !self.web_images.states.contains_key(&id) && !urls.contains(url)
                                {
                                    urls.push(url.clone());
                                }
                            }
                        }
                    }
                }
                let states = Arc::make_mut(&mut self.web_images.states);
                for u in &urls {
                    states.insert(format!("url:{u}"), ImageState::Pending);
                }
                Ok(json!(urls))
            }
            C::ImageLoaded { url, data, error } => {
                let id = format!("url:{url}");
                let state = match (data, error) {
                    (Some(d), None) => match decode(&d) {
                        Ok(bytes) => match pixel_size(&bytes) {
                            Some((w, h)) => {
                                self.web_images.bytes.insert(id.clone(), Arc::new(bytes));
                                ImageState::Ready {
                                    width: f64::from(w),
                                    height: f64::from(h),
                                }
                            }
                            // Not a picture format the backends draw.
                            None => ImageState::Failed("#VALUE!".into()),
                        },
                        Err(_) => ImageState::Failed("#VALUE!".into()),
                    },
                    (_, Some(e)) if e == "blocked" => ImageState::Failed("#BLOCKED!".into()),
                    _ => ImageState::Failed("#CONNECT!".into()),
                };
                let ready = matches!(state, ImageState::Ready { .. });
                Arc::make_mut(&mut self.web_images.states).insert(id, state);
                Ok(json!({ "ready": ready }))
            }
            C::CellMedia { id } => {
                let bytes: Option<Vec<u8>> = if id.starts_with("url:") {
                    self.web_images.bytes.get(&id).map(|b| b.to_vec())
                } else {
                    self.wb.as_ref().and_then(|wb| {
                        wb.sheets
                            .iter()
                            .flat_map(|ws| ws.cells.values())
                            .filter_map(|c| c.image.as_deref())
                            .find(|i| i.media_id() == id)
                            .and_then(|i| i.data().map(<[u8]>::to_vec))
                    })
                };
                Ok(bytes.map_or(Value::Null, |b| {
                    json!(base64::engine::general_purpose::STANDARD.encode(b))
                }))
            }
            C::CellImageBackgroundPreview {
                sheet,
                row,
                col,
                rect,
                marks,
                prior,
                source,
            } => {
                let spec = RemovalSpec {
                    rect,
                    marks,
                    prior,
                    source,
                };
                let data = self.local_image(sheet, row, col)?;
                let p = bg::preview(&data, &spec, &bg::clock_ms)
                    .ok_or_else(|| bad("unsupported picture format"))?;
                Ok(serde_json::to_value(p).unwrap_or(Value::Null))
            }
            C::CellImageRemoveBackground {
                sheet,
                row,
                col,
                rect,
                marks,
                prior,
            } => {
                let spec = RemovalSpec {
                    rect,
                    marks,
                    prior,
                    source: false,
                };
                let data = self.local_image(sheet, row, col)?;
                let png =
                    bg::remove(&data, &spec).ok_or_else(|| bad("unsupported picture format"))?;
                self.edit(sheet, Recalc::No, |wb, _| {
                    if let Some(cell) = wb.sheets[sheet].cells.get_mut(&(row, col)) {
                        if let Some(img) = cell.image.as_deref_mut() {
                            let next = CellImage::local(png, "png", img.alt_text.clone());
                            img.source = next.source;
                        }
                        // A changed rich value is written as a new one.
                        cell.vm = None;
                    }
                    Ok(())
                })?;
                Ok(Value::Null)
            }
        }
    }

    /// Bytes of the stored picture of a cell, when it is one the background removal
    /// decodes.
    fn local_image(&self, sheet: usize, row: u32, col: u32) -> Result<Vec<u8>> {
        self.wb
            .as_ref()
            .and_then(|wb| wb.sheets.get(sheet))
            .and_then(|ws| ws.cell(row, col))
            .and_then(|c| c.image.as_deref())
            .and_then(CellImage::data)
            .filter(|d| bg::removable(d))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| bad("no picture in this cell"))
    }

    /// Bytes of the pictures the `Image` commands of `pages` use (PDF export).
    pub(crate) fn media_of(&self, pages: &[bo_render::PageList]) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for c in pages.iter().flat_map(|p| p.commands.iter()) {
            let bo_render::Command::Image { image, .. } = c else {
                continue;
            };
            if out.contains_key(image) {
                continue;
            }
            let bytes = if image.starts_with("url:") {
                self.web_images.bytes.get(image).map(|b| b.to_vec())
            } else {
                self.wb.as_ref().and_then(|wb| {
                    wb.sheets
                        .iter()
                        .flat_map(|ws| ws.cells.values())
                        .filter_map(|c| c.image.as_deref())
                        .find(|i| &i.media_id() == image)
                        .and_then(|i| i.data().map(<[u8]>::to_vec))
                })
            };
            if let Some(b) = bytes {
                out.insert(image.clone(), b);
            }
        }
        out
    }

    fn remove_checkbox(&mut self, sheet: usize, range: Rng) -> Result<()> {
        self.edit(sheet, Recalc::No, |wb, _| {
            let cells = cells_of(wb, sheet, range);
            let mut cache = BTreeMap::new();
            for (r, c) in cells {
                let Some(style) = wb.sheets[sheet].cell(r, c).map(|c| c.style) else {
                    continue;
                };
                if wb.styles.xf(style).is_some_and(|x| x.checkbox) {
                    let s = with_checkbox(wb, &mut cache, style, false);
                    if let Some(cell) = wb.sheets[sheet].cells.get_mut(&(r, c)) {
                        cell.style = s;
                    }
                }
            }
            Ok(())
        })
    }

    /// [`SheetEngine::cell_media`] after the read-only and protection checks
    /// (checkbox controls are a format change, ECMA-376 Part 1, §18.3.1.85
    /// `formatCells`; values of locked cells are checked by the edit itself).
    pub(crate) fn cell_media_checked(&mut self, cmd: CellMediaCommand) -> Result<Value> {
        if Self::cell_media_edits(&cmd) {
            self.writable()?;
        }
        if let CellMediaCommand::InsertCheckbox { sheet, .. }
        | CellMediaCommand::RemoveCheckbox { sheet, .. } = &cmd
        {
            if let Some(ws) = self.wb.as_ref().and_then(|wb| wb.sheets.get(*sheet)) {
                super::protect::check(ws, super::protect::Action::FormatCells)?;
            }
        }
        self.cell_media(cmd)
    }

    /// Whether the command changes the workbook (refused on read-only files).
    pub(crate) fn cell_media_edits(cmd: &CellMediaCommand) -> bool {
        !matches!(
            cmd,
            CellMediaCommand::CellImageInfo { .. }
                | CellMediaCommand::CellMediaState { .. }
                | CellMediaCommand::ImageRequests
                | CellMediaCommand::ImageLoaded { .. }
                | CellMediaCommand::CellMedia { .. }
        )
    }
}
