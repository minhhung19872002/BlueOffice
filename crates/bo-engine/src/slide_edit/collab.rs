//! Live collaboration for BlueSlide (prompt §6 Phase 6): every slide is an
//! entry of the `slides` Y.Map (key = `p:sldId@id`, ECMA-376 Part 1
//! §19.2.1.33; value = JSON `{"pos", "slide"}` with the slide without its
//! shapes and its position in the deck), every top-level object of a slide
//! an entry of the `shapes` Y.Map (key `"<slide>/<cNvPr@id>"`, value JSON
//! `{"z", "node"}` with the z-order and the object, §19.3.1.45 spTree).
//!
//! Conflicts resolve last-writer-wins per object (and per slide for slide
//! properties and order). After each local command the slides that changed
//! are compared with the shadow of the shared maps; remote entries rebuild
//! the affected slides, applied outside the local history.

use super::ops::SlideOp;
use super::{Outcome, SlideEditResult};
use crate::collab::{Blob, CollabChange};
use crate::{Engine, EngineError};
use bo_model::slide::{Presentation, RawPart, ShapeNode, Slide};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Collaboration output of a BlueSlide command (part of [`SlideEditResult`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SlideCollabOut {
    pub change: CollabChange,
    /// Key of the selected object (`"<slide>/<shape>"`) for presence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
}

/// Where a collaborator's selected object is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeerShape {
    pub slide: usize,
    pub rect: super::geom::BoxR,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    pos: f64,
    slide: Slide,
}

#[derive(Serialize, Deserialize)]
struct ShapeEntry {
    z: usize,
    node: ShapeNode,
}

#[derive(Debug, Default)]
struct Shadow {
    meta: Option<String>,
    shapes: BTreeMap<String, String>,
    /// The model slide matching `meta` + `shapes` (when it is in the deck).
    slide: Option<Slide>,
    pos: usize,
}

/// Shadow of the shared slide maps.
#[derive(Debug, Default)]
pub struct SlideCollab {
    joining: bool,
    slides: BTreeMap<String, Shadow>,
    known_parts: BTreeSet<String>,
    client: u64,
    counter: u32,
}

/// Keys of a slide's top-level objects: `cNvPr@id`, with an occurrence
/// suffix for the duplicated ids real files contain.
fn slide_shapes(key: &str, s: &Slide) -> BTreeMap<String, String> {
    let mut seen: BTreeMap<u32, usize> = BTreeMap::new();
    s.data
        .shapes
        .iter()
        .enumerate()
        .map(|(z, n)| {
            let json =
                serde_json::to_string(&ShapeEntry { z, node: n.clone() }).unwrap_or_default();
            let k = match n.nv() {
                Some(nv) => {
                    let c = seen.entry(nv.id).or_insert(0);
                    *c += 1;
                    if *c == 1 {
                        format!("{key}/{}", nv.id)
                    } else {
                        format!("{key}/{}~{c}", nv.id)
                    }
                }
                None => format!("{key}/x{z}"),
            };
            (k, json)
        })
        .collect()
}

fn meta_json(s: &Slide, pos: usize) -> String {
    let mut bare = s.clone();
    bare.data.shapes.clear();
    serde_json::to_string(&Meta {
        pos: pos as f64,
        slide: bare,
    })
    .unwrap_or_default()
}

impl SlideCollab {
    fn new(pres: &Presentation, seed: bool, client: u64) -> SlideCollab {
        SlideCollab {
            joining: !seed,
            slides: BTreeMap::new(),
            known_parts: pres.parts.iter().map(|p| p.name.clone()).collect(),
            client,
            counter: 0,
        }
    }

    /// Gives slides created during the session (`sldId` 0 or duplicated) a
    /// fresh id so every replica keys them the same; returns whether any changed.
    fn assign_ids(&mut self, pres: &mut Presentation) -> bool {
        let mut used = BTreeSet::new();
        let mut changed = false;
        for i in 0..pres.slides.len() {
            let id = pres.slides[i].slide_id;
            if id >= 256 && used.insert(id) {
                continue;
            }
            // §19.2.1.33: 256 ≤ id < 2147483648.
            let fresh = loop {
                self.counter = self.counter.wrapping_add(1);
                let h = self
                    .client
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                    .wrapping_add(u64::from(self.counter).wrapping_mul(0x2545_F491));
                let cand = 256 + (h % (0x7FFF_FFFF - 256)) as u32;
                if !used.contains(&cand) && pres.slides.iter().all(|s| s.slide_id != cand) {
                    break cand;
                }
            };
            pres.slides[i].slide_id = fresh;
            used.insert(fresh);
            changed = true;
        }
        changed
    }

    /// Changes of the deck since the last call.
    fn local_change(&mut self, pres: &Presentation) -> CollabChange {
        let mut ch = CollabChange::default();
        if self.joining {
            return ch;
        }
        let mut present = BTreeSet::new();
        for (i, s) in pres.slides.iter().enumerate() {
            let key = s.slide_id.to_string();
            present.insert(key.clone());
            let sh = self.slides.entry(key.clone()).or_default();
            if sh.pos == i && sh.slide.as_ref() == Some(s) {
                continue;
            }
            let meta = meta_json(s, i);
            if sh.meta.as_ref() != Some(&meta) {
                ch.slides.insert(key.clone(), Some(meta.clone()));
                sh.meta = Some(meta);
            }
            let now = slide_shapes(&key, s);
            for (k, v) in &now {
                if sh.shapes.get(k) != Some(v) {
                    ch.shapes.insert(k.clone(), Some(v.clone()));
                }
            }
            for k in sh.shapes.keys() {
                if !now.contains_key(k) {
                    ch.shapes.insert(k.clone(), None);
                }
            }
            sh.shapes = now;
            sh.slide = Some(s.clone());
            sh.pos = i;
        }
        // Slides of the model that are gone (pending remote entries stay).
        let gone: Vec<String> = self
            .slides
            .iter()
            .filter(|(k, sh)| sh.slide.is_some() && !present.contains(*k))
            .map(|(k, _)| k.clone())
            .collect();
        for k in gone {
            if let Some(sh) = self.slides.remove(&k) {
                ch.slides.insert(k, None);
                for sk in sh.shapes.into_keys() {
                    ch.shapes.insert(sk, None);
                }
            }
        }
        for p in &pres.parts {
            if self.known_parts.insert(p.name.clone()) {
                ch.blobs.insert(
                    p.name.clone(),
                    Some(Blob::encode(p.content_type.as_deref(), &p.data)),
                );
            }
        }
        ch
    }

    /// Applies remote entries to the shadow; returns the new slide list when
    /// the deck changes.
    fn apply(&mut self, pres: &Presentation, change: &CollabChange) -> Option<Vec<Slide>> {
        let mut affected = BTreeSet::new();
        for (k, v) in &change.slides {
            self.slides.entry(k.clone()).or_default().meta.clone_from(v);
            affected.insert(k.clone());
        }
        for (key, v) in &change.shapes {
            let Some((slide, _)) = key.split_once('/') else {
                continue;
            };
            let sh = self.slides.entry(slide.to_owned()).or_default();
            match v {
                Some(v) => {
                    sh.shapes.insert(key.clone(), v.clone());
                }
                None => {
                    sh.shapes.remove(key);
                }
            }
            affected.insert(slide.to_owned());
        }
        if affected.is_empty() {
            return None;
        }
        if self.joining {
            if !self.slides.values().any(|s| s.meta.is_some()) {
                return None;
            }
            // First shared state: it replaces the whole deck.
            affected.extend(self.slides.keys().cloned());
            self.joining = false;
        }
        let mut order: Vec<(f64, String, Slide)> = Vec::new();
        for (k, sh) in &self.slides {
            let Some(meta) = sh.meta.as_deref() else {
                continue;
            };
            let current = pres.slides.iter().find(|s| s.slide_id.to_string() == *k);
            let Ok(m) = serde_json::from_str::<Meta>(meta) else {
                // Unreadable entry: keep what the model has.
                if let Some(s) = current {
                    order.push((sh.pos as f64, k.clone(), s.clone()));
                }
                continue;
            };
            let slide = match current {
                Some(s) if !affected.contains(k) => s.clone(),
                _ => {
                    let mut slide = m.slide;
                    let mut nodes: Vec<(usize, &String, ShapeNode)> = sh
                        .shapes
                        .iter()
                        .filter_map(|(sk, v)| {
                            serde_json::from_str::<ShapeEntry>(v)
                                .ok()
                                .map(|e| (e.z, sk, e.node))
                        })
                        .collect();
                    nodes.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
                    slide.data.shapes = nodes.into_iter().map(|n| n.2).collect();
                    slide
                }
            };
            order.push((m.pos, k.clone(), slide));
        }
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for sh in self.slides.values_mut() {
            sh.slide = None;
        }
        let mut out = Vec::with_capacity(order.len());
        for (i, (_, k, s)) in order.into_iter().enumerate() {
            if let Some(sh) = self.slides.get_mut(&k) {
                sh.slide = Some(s.clone());
                sh.pos = i;
            }
            out.push(s);
        }
        // Positions are re-announced by the next local change when they moved.
        Some(out)
    }
}

impl Engine {
    /// Bind the open presentation to a collaboration session (see
    /// [`Engine::collab_start`]); `client` is the replica's client id (used to
    /// number slides created during the session).
    pub fn slide_collab_start(&mut self, seed: bool, client: u64) -> crate::Result<CollabChange> {
        let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
        let mut c = SlideCollab::new(pres, seed, client);
        if c.assign_ids(pres) {
            self.slides.undo.clear();
            self.slides.redo.clear();
        }
        let change = c.local_change(pres);
        self.slide_collab = Some(c);
        Ok(change)
    }

    pub fn slide_collab_start_json(&mut self, seed: bool, client: u64) -> crate::Result<String> {
        Ok(serde_json::to_string(&self.slide_collab_start(seed, client)?).unwrap_or_default())
    }

    pub fn slide_collab_stop(&mut self) {
        self.slide_collab = None;
    }

    /// After a local slide command: the change to share and the selection.
    pub(crate) fn slide_collab_after_edit(&mut self) -> Option<SlideCollabOut> {
        let c = self.slide_collab.as_mut()?;
        let pres = self.pres.as_mut()?;
        if !c.joining && c.assign_ids(pres) {
            self.slides.undo.clear();
            self.slides.redo.clear();
        }
        let change = c.local_change(pres);
        let sel = &self.slides.sel;
        let shape = match (pres.slides.get(sel.slide), sel.ids.first()) {
            (Some(s), Some(id)) => Some(format!("{}/{id}", s.slide_id)),
            _ => None,
        };
        Some(SlideCollabOut { change, shape })
    }

    /// Apply remote slide entries (`WasmCollab.takeRemote`).
    pub fn slide_collab_apply(&mut self, change: &CollabChange) -> crate::Result<SlideEditResult> {
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let c = self
                .slide_collab
                .as_mut()
                .ok_or_else(|| EngineError::BadCommand("collaboration not started".into()))?;
            for (name, v) in &change.blobs {
                let Some((ct, data)) = v.as_deref().and_then(Blob::decode) else {
                    continue;
                };
                c.known_parts.insert(name.clone());
                if pres.part_data(name).is_none() {
                    let op = SlideOp::Parts {
                        index: pres.parts.len(),
                        remove: Vec::new(),
                        insert: vec![RawPart {
                            name: name.clone(),
                            content_type: ct,
                            data,
                        }],
                    };
                    if op.apply(pres).is_err() {
                        out.error = Some("parts".into());
                    }
                }
            }
            if let Some(list) = c.apply(pres, change) {
                let same_order = list.len() == pres.slides.len()
                    && list
                        .iter()
                        .zip(&pres.slides)
                        .all(|(a, b)| a.slide_id == b.slide_id);
                if same_order {
                    for (i, s) in list.into_iter().enumerate() {
                        if pres.slides[i] != s {
                            out.touched.insert(i);
                            pres.slides[i] = s;
                        }
                    }
                } else {
                    pres.slides = list;
                    out.all = true;
                }
                // The local history no longer matches the deck.
                self.slides.undo.clear();
                self.slides.redo.clear();
            }
            self.slides.clamp(pres);
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }

    pub fn slide_collab_apply_json(&mut self, json: &str) -> crate::Result<String> {
        let change: CollabChange =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        Ok(serde_json::to_string(&self.slide_collab_apply(&change)?).unwrap_or_default())
    }

    /// Boxes of collaborators' selected objects (keys `"<slide>/<shape>"`).
    pub fn slide_collab_peers(&self, keys: &[String]) -> crate::Result<Vec<Option<PeerShape>>> {
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        Ok(keys
            .iter()
            .map(|k| {
                let (slide, id) = k.split_once('/')?;
                let id: u32 = id.parse().ok()?;
                let index = pres
                    .slides
                    .iter()
                    .position(|s| s.slide_id.to_string() == slide)?;
                let placed = super::geom::place_slide(pres, index);
                let p = super::geom::find(&placed, id)?;
                Some(PeerShape {
                    slide: index,
                    rect: p.b,
                })
            })
            .collect())
    }

    pub fn slide_collab_peers_json(&self, json: &str) -> crate::Result<String> {
        let keys: Vec<String> =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        Ok(serde_json::to_string(&self.slide_collab_peers(&keys)?).unwrap_or_default())
    }
}
