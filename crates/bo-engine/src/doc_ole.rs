//! BlueDoc embedded and linked workbooks (CORE-EMBED-001): the [`OleCommand`]s on the
//! document. An object is a picture whose [`bo_model::Image::ole`] is set; the writer
//! turns it into a `w:object` (ECMA-376 Part 1, §17.3.3.19) with the Office VML
//! `o:OLEObject`, a linked one inside a §17.16.5.32 LINK field.

use crate::ole::{self, Built, LinkOut, OleCommand, OleInfo};
use crate::{EngineError, Result};
use bo_edit::ole::OleUpdate;
use bo_model::ole::{OleLink, OleObject, PROG_ID_XLSX};
use bo_model::Image;
use std::sync::Arc;

const EMU_PT: f64 = 12_700.0;

fn emu(pt: f64) -> i64 {
    (pt * EMU_PT).round() as i64
}

impl crate::Engine {
    /// Bytes of a part of the document: replaced ones from the model, else the source package.
    fn doc_part(&self, name: &str) -> Option<Vec<u8>> {
        let doc = self.document()?;
        if let Some(d) = doc.media.get(name) {
            return Some(d.as_ref().clone());
        }
        let src = doc.source.as_ref()?;
        let pkg = bo_opc::Package::read(&src.bytes, &bo_opc::Limits::default()).ok()?;
        pkg.part(name).map(|p| p.data.clone())
    }

    /// A part name under `/word/<dir>/` not used by the model or the source package.
    fn doc_new_part(&self, dir: &str, stem: &str, ext: &str) -> String {
        let doc = self.document();
        let source: Vec<String> = doc
            .and_then(|d| d.source.as_ref())
            .and_then(|s| bo_opc::Package::read(&s.bytes, &bo_opc::Limits::default()).ok())
            .map(|p| {
                p.parts()
                    .iter()
                    .map(|x| x.name.to_ascii_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        let used = |n: &str| {
            doc.is_some_and(|d| d.media.contains_key(n))
                || source.iter().any(|s| s == &n.to_ascii_lowercase())
                || self
                    .session
                    .as_ref()
                    .is_some_and(|s| bo_edit::picture_tools::media_in_use(s.doc(), n))
        };
        (1..=u32::MAX)
            .map(|n| format!("/word/{dir}/{stem}{n}.{ext}"))
            .find(|n| !used(n))
            .unwrap_or_default()
    }

    /// Largest object size: the text width of the first section and 9 inches.
    fn doc_max_size(&self) -> (f64, f64) {
        let page = self
            .document()
            .and_then(|d| d.sections.first())
            .map(|s| s.page);
        let w = page.map_or(468.0, |p| {
            f64::from(p.width.0 - p.margin_left.0 - p.margin_right.0) / 20.0
        });
        (w.max(72.0), 648.0)
    }

    fn doc_ole_info(&mut self, img: &Image, cells: bool, data: bool) -> OleInfo {
        let Some(o) = img.ole.as_deref() else {
            return OleInfo::default();
        };
        let mut i = OleInfo {
            prog_id: o.prog_id.clone(),
            workbook: o.is_workbook(),
            linked: o.link.is_some(),
            source: o.link.as_ref().map(|l| ole::source_path(&l.source)),
            item: o.link.as_ref().map(|l| l.item.clone()),
            auto_update: o.link.as_ref().is_some_and(|l| l.auto_update),
            width: img.width.0 as f64 / EMU_PT,
            height: img.height.0 as f64 / EMU_PT,
            ..OleInfo::default()
        };
        if o.link.is_none() && i.workbook {
            if let Some(bytes) = self.doc_part(&o.package) {
                if let Ok((sheet, area, inputs)) = ole::area_of(&mut self.fonts, &bytes) {
                    i.sheet = Some(sheet);
                    i.range = Some(area.to_string());
                    i.cells = cells.then_some(inputs);
                }
                i.data = data.then(|| ole::encode(&bytes));
            }
        }
        i
    }

    /// Insert a built object at the selection.
    fn doc_insert_built(
        &mut self,
        b: Built,
        link: Option<OleLink>,
    ) -> std::result::Result<(), String> {
        let (mw, mh) = self.doc_max_size();
        let (w, h) = ole::fit(b.size, mw, mh);
        let preview = (self.doc_new_part("media", "image", "png"), Arc::new(b.png));
        let (package, pkg_part) = if link.is_some() {
            (String::new(), None)
        } else {
            let name = self.doc_new_part("embeddings", "Workbook", "xlsx");
            (name.clone(), Some((name, Arc::new(b.xlsx))))
        };
        let ole = OleObject {
            prog_id: PROG_ID_XLSX.into(),
            package,
            link,
            icon: false,
        };
        let s = self
            .session
            .as_mut()
            .ok_or_else(|| "noDocument".to_owned())?;
        s.insert_ole(preview, pkg_part, ole, emu(w), emu(h))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Re-embed the selected object's workbook after `f` built its new version.
    fn doc_reembed(
        &mut self,
        img: &Image,
        f: impl FnOnce(&mut bo_text::FontManager, &[u8]) -> Result<Built>,
    ) -> std::result::Result<(), String> {
        let o = img.ole.as_deref().ok_or("noObject")?;
        if o.link.is_some() {
            return Err("linkedObject".into());
        }
        if !o.is_workbook() {
            return Err("notWorkbook".into());
        }
        let old = self.doc_part(&o.package).ok_or("noObject")?;
        let b = f(&mut self.fonts, &old).map_err(|e| e.to_string())?;
        let shown = (img.width.0 as f64 / EMU_PT, img.height.0 as f64 / EMU_PT);
        let (w, h) = ole::edited_size(shown, b.old_size, b.size);
        // An `.xlsx` package keeps its part; another kind (`oleObject*.bin`) becomes one.
        let package = if o.package.to_ascii_lowercase().ends_with(".xlsx") {
            o.package.clone()
        } else {
            self.doc_new_part("embeddings", "Workbook", "xlsx")
        };
        let u = OleUpdate {
            preview: Some((self.doc_new_part("media", "image", "png"), Arc::new(b.png))),
            package: Some((package, Arc::new(b.xlsx))),
            size: Some((emu(w), emu(h))),
        };
        let s = self.session.as_mut().ok_or("noDocument")?;
        s.update_ole(u).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Links of the document, one per source.
    fn doc_links(&self) -> Vec<LinkOut> {
        let mut out: Vec<LinkOut> = Vec::new();
        let objs = self
            .session
            .as_ref()
            .map(bo_edit::Session::body_objects)
            .unwrap_or_default();
        for img in objs {
            let Some(l) = img.ole.as_ref().and_then(|o| o.link.as_ref()) else {
                continue;
            };
            match out.iter_mut().find(|x| x.source == l.source) {
                Some(x) => x.count += 1,
                None => out.push(LinkOut {
                    source: l.source.clone(),
                    path: ole::source_path(&l.source),
                    item: l.item.clone(),
                    count: 1,
                }),
            }
        }
        out
    }

    /// Run an embedded-object command on the document; the result is the
    /// [`crate::edit::EditResult`] with `ole` (the object at the selection), `links`
    /// and `count` (objects updated).
    pub fn doc_ole_command(&mut self, cmd: OleCommand) -> Result<serde_json::Value> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        if self.session.is_none() {
            return Err(EngineError::NoDocument);
        }
        let edits = !matches!(cmd, OleCommand::OleInfo { .. } | OleCommand::OleLinks);
        let target = self
            .session
            .as_ref()
            .and_then(bo_edit::Session::ole_at_selection);
        let mut want = (false, false);
        let mut links = None;
        let mut count = None;
        let res: std::result::Result<(), String> = if self.sec.read_only && edits {
            Err(crate::security::READ_ONLY.into())
        } else {
            match cmd {
                OleCommand::OleInfo { cells, data } => {
                    want = (cells, data);
                    Ok(())
                }
                OleCommand::OleLinks => {
                    links = Some(self.doc_links());
                    Ok(())
                }
                OleCommand::OleInsert {
                    data,
                    range,
                    cells,
                    link,
                    auto_update,
                } => {
                    let src = data.as_deref().map(ole::decode).transpose()?;
                    let link = link.filter(|l| !l.trim().is_empty());
                    if link.is_some() && src.is_none() {
                        Err("noData".to_owned())
                    } else {
                        match ole::build(
                            &mut self.fonts,
                            src.as_deref(),
                            range.as_deref(),
                            cells.as_deref(),
                        ) {
                            Ok(b) => {
                                let item = format!("{}!{}", b.sheet, b.range);
                                let link = link.map(|source| OleLink {
                                    source,
                                    item,
                                    auto_update,
                                });
                                self.doc_insert_built(b, link)
                            }
                            Err(e) => Err(e.to_string()),
                        }
                    }
                }
                OleCommand::OleEdit { cells } => match &target {
                    Some(img) => {
                        self.doc_reembed(img, |f, old| ole::build(f, Some(old), None, Some(&cells)))
                    }
                    None => Err("noObject".into()),
                },
                OleCommand::OleSetData { data } => {
                    let bytes = ole::decode(&data)?;
                    match &target {
                        Some(img) => {
                            self.doc_reembed(img, |f, _| ole::build(f, Some(&bytes), None, None))
                        }
                        None => Err("noObject".into()),
                    }
                }
                OleCommand::OleUpdateLinks { sources } => {
                    let mut n = 0;
                    let mut err = None;
                    let known = self.doc_links();
                    for s in sources {
                        let Some(l) = known
                            .iter()
                            .find(|l| l.source == s.source || l.path == s.source)
                        else {
                            continue;
                        };
                        let bytes = ole::decode(&s.data)?;
                        let range = (!l.item.is_empty()).then(|| ole::a1_item(&l.item));
                        let b =
                            match ole::build(&mut self.fonts, Some(&bytes), range.as_deref(), None)
                            {
                                Ok(b) => b,
                                Err(e) => {
                                    err = Some(e.to_string());
                                    continue;
                                }
                            };
                        // Every object of the source keeps its width (proportions of the new area).
                        let shown = self
                            .session
                            .as_ref()
                            .map(bo_edit::Session::body_objects)
                            .unwrap_or_default()
                            .into_iter()
                            .find(|i| {
                                i.ole
                                    .as_ref()
                                    .and_then(|o| o.link.as_ref())
                                    .is_some_and(|x| x.source == l.source)
                            })
                            .map_or(b.size, |i| {
                                (i.width.0 as f64 / EMU_PT, i.height.0 as f64 / EMU_PT)
                            });
                        let (w, h) = ole::edited_size(shown, None, b.size);
                        let u = OleUpdate {
                            preview: Some((
                                self.doc_new_part("media", "image", "png"),
                                Arc::new(b.png),
                            )),
                            package: None,
                            size: Some((emu(w), emu(h))),
                        };
                        if let Some(sess) = self.session.as_mut() {
                            match sess.update_links(&l.source, &u) {
                                Ok(k) => n += k,
                                Err(e) => err = Some(e.to_string()),
                            }
                        }
                    }
                    count = Some(n);
                    err.map_or(Ok(()), Err)
                }
            }
        };
        let dirty = self.relayout()?;
        // `story_result`: an open header/footer stays open (a double-click asks `oleInfo`).
        let mut r = self.story_result(dirty, res.err())?;
        r.collab = self.collab_after_edit();
        let mut v = serde_json::to_value(&r).unwrap_or_default();
        let now = self
            .session
            .as_ref()
            .and_then(bo_edit::Session::ole_at_selection);
        let info = now.map(|img| self.doc_ole_info(&img, want.0, want.1));
        if let Some(o) = v.as_object_mut() {
            if let Some(i) = info {
                o.insert("ole".into(), serde_json::to_value(i).unwrap_or_default());
            }
            if let Some(l) = links {
                o.insert("links".into(), serde_json::to_value(l).unwrap_or_default());
            }
            if let Some(c) = count {
                o.insert("count".into(), c.into());
            }
        }
        Ok(v)
    }
}
