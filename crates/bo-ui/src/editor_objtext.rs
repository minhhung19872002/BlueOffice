//! The Format pane on text objects selected with Edit Object, as the reference editor's: the
//! Text Style section shows the selected object's font, size, colour, bold / italic, scale and
//! character spacing (the controls the reference offers there; the rest are grey) and a change
//! applies to the whole of every selected text object; the Split section's four commands
//! (Merge Text, Split Text, Remove Kerning, Convert to Shapes); the Image section when one
//! image is selected (replace, rotate, flip, opacity).
use crate::editor_edit::Item;
use crate::editor_window::{EditTool, EditorWindow};
use crate::reader_icons::{self, Ink};
use crate::Painter;
use bo_pdfread::{Align, ObjectKind, RunStyle, StyleChange};
use bo_render::Rgba;

/// Enabled and grey ink of the pane's controls (reference: #333333 / #B1B1B1).
pub(crate) const ON: Rgba = Rgba(51, 51, 51, 255);
pub(crate) const OFF: Rgba = Rgba(177, 177, 177, 255);

/// Format commands the reference offers for a text object selected with Edit Object.
pub(crate) const OBJECT_TEXT_CMDS: [&str; 7] = [
    "fmt.font",
    "fmt.size",
    "fmt.color",
    "fmt.bold",
    "fmt.italic",
    "fmt.scale",
    "fmt.charSp",
];

/// The Split section: command, icon and the label key of its tooltip.
pub(crate) const SPLIT_CMDS: [(&str, &str, &str); 4] = [
    ("obj.mergeText", "splitBlock", "pdfEditor.mergeText"),
    ("obj.splitText", "splitLines", "pdfEditor.splitText"),
    ("obj.removeKerning", "joinBlocks", "pdfEditor.removeKerning"),
    ("obj.toShapes", "textArea", "pdfEditor.toShapes"),
];

/// Whether the selected text runs have kerning: (page, runs, edit count) and the answer.
pub(crate) type KerningCache = Option<((usize, Vec<usize>, u32), bool)>;

/// Cached style of the selected text object: (page, run ids, edit count) and what it showed.
pub(crate) type ObjStyleCache = Option<((usize, Vec<usize>, u32), Option<(RunStyle, Align)>)>;

impl EditorWindow {
    /// Edit Object with at least one text object selected and no typing.
    pub(crate) fn object_text_active(&self) -> bool {
        self.edit_tool() == Some(EditTool::Object)
            && self.edit.typing.is_none()
            && self.edit.selected_kinds().contains(&ObjectKind::Text)
    }

    /// The selected items of the page being edited.
    fn selected_items(&mut self) -> (Option<usize>, Vec<Item>) {
        let Some(page) = self.edit.page else {
            return (None, Vec::new());
        };
        let sel = self.edit.selected.clone();
        let Some(v) = self.r.view_mut() else {
            return (None, Vec::new());
        };
        let list = self.edit.items(v, page);
        (
            Some(page),
            sel.iter().filter_map(|&k| list.get(k).cloned()).collect(),
        )
    }

    /// Style of the first selected text object (read once per selection and edit).
    pub(crate) fn object_text_style(&mut self) -> Option<(RunStyle, Align)> {
        if !self.object_text_active() {
            return None;
        }
        let (page, items) = self.selected_items();
        let page = page?;
        let block = items.iter().find_map(|it| it.block.clone())?;
        let v = self.r.view_mut()?;
        let key = (page, block.ids.clone(), v.doc().edit_count());
        if let Some((k, s)) = &self.pane.obj_style {
            if *k == key {
                return s.clone();
            }
        }
        // Reading the block's styles opens and closes an empty edit (no undo step).
        let doc = v.doc_mut();
        let style = doc.begin_block_edit(page, &block).ok().and_then(|e| {
            doc.end_block_edit(&e);
            Some((e.style_at(0)?.clone(), block.align))
        });
        self.pane.obj_style = Some((key, style.clone()));
        style
    }

    /// Apply a style change to the whole of every selected text object (one undo step).
    pub(crate) fn object_text_change(&mut self, change: &StyleChange) {
        let (Some(page), items) = self.selected_items() else {
            return;
        };
        let blocks: Vec<_> = items.iter().filter_map(|it| it.block.clone()).collect();
        let expect: Vec<([f32; 4], ObjectKind)> =
            items.iter().map(|it| (it.bbox, it.kind)).collect();
        let Some(v) = self.r.view_mut() else { return };
        let mut done = 0;
        let mut err = None;
        // Later blocks first: rewriting one leaves the indices of earlier runs alone.
        for b in blocks.iter().rev() {
            let doc = v.doc_mut();
            let objs = match doc.page_objects(page) {
                Ok(o) => o,
                Err(e) => {
                    err = Some(e);
                    break;
                }
            };
            let Some(now) = bo_pdfread::text_blocks(&objs)
                .into_iter()
                .find(|n| n.bbox == b.bbox)
            else {
                continue;
            };
            match doc.begin_block_edit(page, &now) {
                Ok(mut e) => {
                    let r = doc.block_edit_style(&mut e, 0..0, change);
                    doc.end_block_edit(&e);
                    match r {
                        Ok(()) => done += 1,
                        Err(x) => err = Some(x),
                    }
                }
                Err(x) => err = Some(x),
            }
        }
        if done > 1 {
            v.doc_mut().merge_last_steps(done);
        }
        v.invalidate_page(page);
        // Restyled text keeps its left edge and baseline but not its box: select again the
        // text object at the same left edge overlapping the old box.
        let list = self.edit.items(v, page).to_vec();
        let mut sel = Vec::new();
        for (b, _) in expect.iter().filter(|e| e.1 == ObjectKind::Text) {
            let best = list
                .iter()
                .enumerate()
                .filter(|(k, it)| {
                    it.kind == ObjectKind::Text
                        && !sel.contains(k)
                        && it.bbox[1] <= b[3]
                        && it.bbox[3] >= b[1]
                })
                .map(|(k, it)| (k, (it.bbox[0] - b[0]).abs()))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((k, d)) = best {
                if d < 3.0 {
                    sel.push(k);
                }
            }
        }
        self.edit.selected = sel;
        if let Some(e) = err {
            self.r.set_status(&e);
        }
        self.after_doc_edit();
    }

    /// Whether Split command `cmd` applies to the selection.
    pub(crate) fn split_enabled(&mut self, cmd: &str) -> bool {
        let (Some(page), items) = self.selected_items() else {
            return false;
        };
        let texts: Vec<&Item> = items
            .iter()
            .filter(|i| i.kind == ObjectKind::Text)
            .collect();
        match cmd {
            "obj.mergeText" => texts.len() >= 2,
            "obj.splitText" => {
                texts.len() == 1 && texts[0].block.as_ref().is_some_and(|b| b.lines > 1)
            }
            "obj.removeKerning" => {
                let ids: Vec<usize> = texts.iter().flat_map(|i| i.ids.clone()).collect();
                let edits = self.r.view().map_or(0, |v| v.doc().edit_count());
                if let Some((k, on)) = &self.pane.kerning {
                    if k.0 == page && k.1 == ids && k.2 == edits {
                        return *on;
                    }
                }
                let on = !ids.is_empty()
                    && self
                        .r
                        .view_mut()
                        .is_some_and(|v| v.doc_mut().has_kerning(page, &ids));
                self.pane.kerning = Some(((page, ids, edits), on));
                on
            }
            "obj.toShapes" => !texts.is_empty(),
            _ => false,
        }
    }

    /// Run a Split command on the selection; false when `cmd` is not one.
    pub(crate) fn split_command(&mut self, cmd: &str) -> bool {
        if !SPLIT_CMDS.iter().any(|c| c.0 == cmd) {
            return false;
        }
        if !self.split_enabled(cmd) {
            return true;
        }
        let (Some(page), items) = self.selected_items() else {
            return true;
        };
        let mut texts: Vec<Item> = items
            .into_iter()
            .filter(|i| i.kind == ObjectKind::Text)
            .collect();
        // Reading order: top to bottom, then left to right.
        texts.sort_by(|a, b| {
            a.bbox[1]
                .total_cmp(&b.bbox[1])
                .then(a.bbox[0].total_cmp(&b.bbox[0]))
        });
        let ids: Vec<usize> = texts.iter().flat_map(|i| i.ids.clone()).collect();
        let first = texts.first().map(|t| t.bbox);
        let r: Result<(), String> = match cmd {
            "obj.mergeText" => self.doc_edit(|d| merge_text(d, page, &texts)),
            "obj.splitText" => {
                let b = texts[0].block.clone();
                self.doc_edit(|d| d.split_text_lines(page, &b.ok_or("no text")?).map(|_| ()))
            }
            "obj.removeKerning" => self.doc_edit(|d| d.remove_kerning(page, &ids).map(|_| ())),
            _ => self.doc_edit(|d| d.text_to_shapes(page, &ids)),
        };
        if let Some(v) = self.r.view_mut() {
            v.invalidate_page(page);
            self.edit.selected.clear();
            if let (Ok(()), Some(b), "obj.mergeText") = (&r, first, cmd) {
                self.edit.reselect(v, page, &[(b, ObjectKind::Text)]);
            }
        }
        if let Err(e) = r {
            self.r.set_status(&e);
        }
        self.after_doc_edit();
        true
    }

    /// The Split section: the reference's four buttons, grey when they do not apply.
    pub(crate) fn paint_split_live(&mut self, p: &mut dyn Painter, x0: f32, ty: f32) {
        let k = self.ui_scale();
        let px = |v: f32| (v * k).round();
        self.section_heading(p, x0, ty, "pdfEditor.split");
        // 125 %: icons 50 px under the heading, 63 px apart.
        for (j, (cmd, icon, _)) in SPLIT_CMDS.iter().enumerate() {
            let on = self.split_enabled(cmd);
            let cx = x0 + px(42.0 + 50.0 * j as f32);
            let cy = ty + px(40.0);
            let ink = if on { ON } else { OFF };
            reader_icons::draw_weight(
                p,
                icon,
                cx - px(10.0),
                cy - px(10.0),
                px(20.0),
                Ink { ink, accent: ink },
                0.9,
            );
            if on {
                let s = px(28.0);
                self.pane
                    .hits
                    .push(([cx - s / 2.0, cy - s / 2.0, s, s], cmd));
            }
        }
    }
}

impl EditorWindow {
    /// Page and run ids of the selected objects other than text.
    fn selected_graphics(&mut self) -> Option<(usize, Vec<usize>, [f32; 4])> {
        let (page, items) = self.selected_items();
        let page = page?;
        let g: Vec<&Item> = items
            .iter()
            .filter(|i| i.kind != ObjectKind::Text)
            .collect();
        let bbox = crate::editor_arrange::union_box(&g.iter().map(|i| i.bbox).collect::<Vec<_>>())?;
        Some((page, g.iter().flat_map(|i| i.ids.clone()).collect(), bbox))
    }

    /// Effect section commands (`obj.opacity…`, `obj.shear…`, `obj.replace`); false when `cmd`
    /// is not one.
    pub(crate) fn image_command(&mut self, cmd: &str) -> bool {
        match cmd {
            "obj.opacity" => {
                let cur = self
                    .selected_graphics()
                    .and_then(|(page, ids, _)| {
                        let id = *ids.first()?;
                        Some(self.r.view_mut()?.doc_mut().object_opacity(page, id))
                    })
                    .unwrap_or(1.0);
                let items = [100, 90, 75, 50, 25, 10]
                    .iter()
                    .map(|v| {
                        (
                            format!("obj.opacity.{v}"),
                            format!("{v}%"),
                            (*v as f32 / 100.0 - cur).abs() < 0.01,
                        )
                    })
                    .collect();
                self.pane_menu(items, cmd);
                true
            }
            "obj.shear" => {
                let items = [-45, -30, -15, 15, 30, 45]
                    .iter()
                    .map(|v| (format!("obj.shear.{v}"), format!("{v}°"), false))
                    .collect();
                self.pane_menu(items, cmd);
                true
            }
            "obj.replace" => {
                self.requests.push(crate::ReaderRequest::PickFile {
                    purpose: "replaceImage".into(),
                    filters: crate::editor_window::image_filters(),
                });
                true
            }
            _ => {
                let num = |pre: &str| cmd.strip_prefix(pre).and_then(|s| s.parse::<f32>().ok());
                let (op, sh) = (num("obj.opacity."), num("obj.shear."));
                if op.is_none() && sh.is_none() {
                    return false;
                }
                let Some((page, ids, bbox)) = self.selected_graphics() else {
                    return true;
                };
                let expect: Vec<([f32; 4], ObjectKind)> = self
                    .selected_items()
                    .1
                    .iter()
                    .map(|i| (i.bbox, i.kind))
                    .collect();
                let r = match (op, sh) {
                    (Some(a), _) => self.doc_edit(|d| d.set_object_opacity(page, &ids, a / 100.0)),
                    (_, Some(deg)) => self.doc_edit(|d| d.shear_objects(page, &ids, bbox, deg)),
                    _ => Ok(()),
                };
                if let Some(v) = self.r.view_mut() {
                    v.invalidate_page(page);
                    if op.is_some() {
                        self.edit.reselect(v, page, &expect);
                    } else {
                        self.edit.selected.clear();
                    }
                }
                if let Err(e) = r {
                    self.r.set_status(&e);
                }
                self.after_doc_edit();
                true
            }
        }
    }

    /// The Effect section for images and other graphics, its heading at `ty`: Opacity and
    /// Shear as the reference's, and Replace for a single image (the reference's "Edit using
    /// other applications" row).
    pub(crate) fn paint_effect(&mut self, p: &mut dyn Painter, x0: f32, ty: f32) {
        let k = self.ui_scale();
        let px = |v: f32| (v * k).round();
        let style = crate::TextStyle {
            size: 16.0 / 1.375 * k,
            bold: false,
        };
        let sub = Rgba(118, 118, 118, 255);
        // Grey with nothing selected, as the reference.
        let has = self.selected_graphics().is_some();
        let live = if has { ON } else { OFF };
        self.section_heading(p, x0, ty, "pdfEditor.effect");
        // 125 %: Opacity / Shear labels +179 under the heading, controls +212 (pane-relative
        // offsets of the reference less its Clipping rows, which BlueEditor does not offer).
        let ly = ty + px(32.0);
        p.text(&self.t("pdfEditor.opacity"), x0 + px(27.0), ly, style, sub);
        p.text(&self.t("pdfEditor.shear"), x0 + px(107.0), ly, style, sub);
        let cy = ly + px(34.0);
        // Opacity: a checker swatch with a drop-down arrow.
        let sw = [x0 + px(30.0), cy - px(8.0), px(16.0), px(16.0)];
        p.fill_rect(sw[0], sw[1], sw[2], sw[3], Rgba(255, 255, 255, 255));
        p.fill_rect(
            sw[0],
            sw[1],
            sw[2] / 2.0,
            sw[3] / 2.0,
            Rgba(200, 200, 200, 255),
        );
        p.fill_rect(
            sw[0] + sw[2] / 2.0,
            sw[1] + sw[3] / 2.0,
            sw[2] / 2.0,
            sw[3] / 2.0,
            Rgba(200, 200, 200, 255),
        );
        frame_rect(p, sw, Rgba(150, 150, 150, 255));
        tri(p, sw[0] + px(23.0), cy - px(2.0), px(8.0), live);
        if has {
            self.pane.hits.push((
                [sw[0] - px(4.0), sw[1] - px(4.0), px(44.0), px(24.0)],
                "obj.opacity",
            ));
        }
        // Shear: icon, value box and "degrees".
        reader_icons::draw_weight(
            p,
            "objShear",
            x0 + px(107.0),
            cy - px(10.0),
            px(20.0),
            Ink {
                ink: live,
                accent: live,
            },
            0.9,
        );
        let b = [x0 + px(140.0), cy - px(12.0), px(72.0), px(24.0)];
        frame_rect(p, b, Rgba(204, 204, 204, 255));
        p.text(
            "0",
            b[0] + px(6.0),
            b[1] + px(4.0),
            style,
            if has {
                Rgba(0, 0, 0, 255)
            } else {
                Rgba(109, 109, 109, 255)
            },
        );
        p.text(
            &self.t("pdfEditor.degrees"),
            b[0] + b[2] + px(8.0),
            b[1] + px(4.0),
            style,
            sub,
        );
        if has {
            self.pane.hits.push((b, "obj.shear"));
        }
        // Replace (one image selected).
        let one_image = self
            .r
            .view_mut()
            .is_some_and(|v| self.edit.selected_image(v).is_some());
        let ry = cy + px(36.0);
        p.text(
            &self.t("pdfEditor.replaceImageRow"),
            x0 + px(27.0),
            ry,
            style,
            sub,
        );
        let by = ry + px(30.0);
        let ink = if one_image { ON } else { OFF };
        reader_icons::draw_weight(
            p,
            "imgReplace",
            x0 + px(30.0),
            by - px(10.0),
            px(20.0),
            Ink { ink, accent: ink },
            0.9,
        );
        p.text(
            &self.t("pdfEditor.replaceImage"),
            x0 + px(64.0),
            by - px(8.0),
            style,
            ink,
        );
        if one_image {
            self.pane.hits.push((
                [x0 + px(24.0), by - px(14.0), px(130.0), px(28.0)],
                "obj.replace",
            ));
        }
    }
}

fn frame_rect(p: &mut dyn Painter, r: [f32; 4], c: Rgba) {
    p.fill_rect(r[0], r[1], r[2], 1.0, c);
    p.fill_rect(r[0], r[1] + r[3] - 1.0, r[2], 1.0, c);
    p.fill_rect(r[0], r[1], 1.0, r[3], c);
    p.fill_rect(r[0] + r[2] - 1.0, r[1], 1.0, r[3], c);
}

fn tri(p: &mut dyn Painter, x: f32, y: f32, w: f32, c: Rgba) {
    p.path(
        &[
            bo_render::PathSeg::M(x, y),
            bo_render::PathSeg::L(x + w, y),
            bo_render::PathSeg::L(x + w / 2.0, y + w * 0.57),
            bo_render::PathSeg::Z,
        ],
        Some(c),
        None,
    );
}

/// Join text objects `texts` (reading order) into the first: its text gains the others' text
/// after a space, in its own style; the others are removed (one undo step).
fn merge_text(d: &mut bo_pdfread::PdfDocument, page: usize, texts: &[Item]) -> Result<(), String> {
    let blocks: Vec<bo_pdfread::TextBlock> = texts.iter().filter_map(|t| t.block.clone()).collect();
    let (first, rest) = blocks.split_first().ok_or("no text")?;
    let mut ids: Vec<usize> = rest.iter().flat_map(|b| b.ids.clone()).collect();
    ids.sort_unstable();
    ids.dedup();
    d.begin_group("merge text");
    let r = (|| {
        d.delete_objects(page, &ids)?;
        let objs = d.page_objects(page)?;
        let b = bo_pdfread::text_blocks(&objs)
            .into_iter()
            .find(|b| b.bbox == first.bbox)
            .ok_or("the text changed")?;
        let mut e = d.begin_block_edit(page, &b)?;
        let st = e.style_of.last().copied().unwrap_or(0);
        for t in rest {
            for c in std::iter::once(' ').chain(t.text.chars()) {
                e.text.push(c);
                e.style_of.push(st);
            }
        }
        let r = d.block_edit_update(&mut e);
        d.end_block_edit(&e);
        r
    })();
    d.end_group();
    r
}
