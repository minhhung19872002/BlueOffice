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
//!
//! * Order: `pos` is an order key ([`crate::collab::order`]); a move rewrites
//!   only the moved slide, so concurrent moves of different slides converge
//!   with both moves kept (the same slide moved twice: last writer wins).
//! * Notes: the text of the notes body placeholder (§19.3.1.26 notes) is the
//!   nested text `n:<slide>`, merged like body text; the rest of the notes
//!   part travels with the slide.
//! * Masters, layouts and themes (§19.3.1.42, §19.3.1.39, §20.1.6): one
//!   `meta` entry per part, `sm:<key>`, `sl:<key>`, `th:<key>`, plus `pp` for
//!   the presentation relationships; last writer wins per part. The parts of
//!   the opened file are keyed by their index; parts created in the session
//!   (slides pasted from another deck) get a key of their own, so two replicas
//!   importing a design at once keep both. References between them (slide →
//!   layout, layout → master, master → theme / layouts) travel as keys
//!   (`lk` in the slide entry, `@m`, `@t`, `@l` in the part entry) and are
//!   turned back into indices on every replica.

use super::ops::{PresFields, SlideOp};
use super::{Outcome, SlideEditResult};
use crate::collab::{fingerprint, order, Blob, CollabChange, TextDelta, TextOp};
use crate::{Engine, EngineError};
use bo_model::slide::{
    Extra, Presentation, RawPart, Rel, ShapeNode, Slide, SlideLayout, SlideMaster, SlideSize, Theme,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Collaboration output of a BlueSlide command (part of [`SlideEditResult`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SlideCollabOut {
    /// Local change to broadcast to the other replicas.
    pub change: CollabChange,
    /// Key of the selected object (`"<slide>/<shape>"`) for presence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    /// Key of the current slide (`p:sldId@id`) for presence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide: Option<String>,
}

/// Where a collaborator's selected object is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PeerShape {
    /// Index of the slide holding the object.
    pub slide: usize,
    /// Rotated box of the object on that slide (points).
    pub rect: super::geom::BoxR,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    pos: f64,
    /// Key of the slide's layout (absent in entries of older clients).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lk: Option<String>,
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
    /// Shared order key.
    ord: Option<f64>,
    /// Shared notes text (`n:<slide>`).
    notes: Option<String>,
}

/// Presentation-level parts as last compared.
#[derive(Debug, Clone, PartialEq)]
struct Parts {
    layouts: Vec<SlideLayout>,
    masters: Vec<SlideMaster>,
    themes: Vec<Theme>,
    rels: Vec<Rel>,
    extra: Vec<Extra>,
    /// Slide and notes page size (§19.2.1.39 sldSz, §19.2.1.23 notesSz).
    size: (SlideSize, SlideSize),
}

impl Parts {
    fn of(p: &Presentation) -> Parts {
        Parts {
            layouts: p.layouts.clone(),
            masters: p.masters.clone(),
            themes: p.themes.clone(),
            rels: p.info.rels.clone(),
            extra: p.extra.clone(),
            size: (p.slide_size.clone(), p.notes_size.clone()),
        }
    }

    /// `meta` entries (`sm:<key>`, `sl:<key>`, `th:<key>`, `pp`).
    fn entries(&self, keys: &PartKeys) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        let mut put = |k: String, v: Result<String, serde_json::Error>| {
            if let Ok(v) = v {
                m.insert(k, v);
            }
        };
        let key = |list: &[String], i: usize| list.get(i).map(|k| Value::String(k.clone()));
        for (x, k) in self.masters.iter().zip(&keys.masters) {
            let l: Vec<Value> = x
                .layouts
                .iter()
                .map(|r| key(&keys.layouts, r.layout).unwrap_or(Value::Null))
                .collect();
            let refs = [
                ("@t", key(&keys.themes, x.theme)),
                ("@l", Some(Value::Array(l))),
            ];
            put(format!("sm:{k}"), with_refs(x, &refs));
        }
        for (x, k) in self.layouts.iter().zip(&keys.layouts) {
            put(
                format!("sl:{k}"),
                with_refs(x, &[("@m", key(&keys.masters, x.master))]),
            );
        }
        for (x, k) in self.themes.iter().zip(&keys.themes) {
            put(format!("th:{k}"), serde_json::to_string(x));
        }
        put(
            "pp".into(),
            serde_json::to_string(&(&self.rels, &self.extra)),
        );
        put("ps".into(), serde_json::to_string(&self.size));
        m
    }
}

fn is_part_key(k: &str) -> bool {
    k == "pp" || k == "ps" || k.starts_with("sm:") || k.starts_with("sl:") || k.starts_with("th:")
}

/// Shared keys of the deck's layouts, masters and themes (index-aligned).
#[derive(Debug, Clone, Default, PartialEq)]
struct PartKeys {
    layouts: Vec<String>,
    masters: Vec<String>,
    themes: Vec<String>,
}

impl PartKeys {
    /// Keys of the parts of the opened file: their index (the same on every
    /// replica, which all open the same file version).
    fn of(p: &Presentation) -> PartKeys {
        let n = |len: usize| (0..len).map(|i| i.to_string()).collect();
        PartKeys {
            layouts: n(p.layouts.len()),
            masters: n(p.masters.len()),
            themes: n(p.themes.len()),
        }
    }
}

/// Order of part keys on every replica: the file's parts (numeric keys) by
/// index, then the parts created in the session by key.
fn key_order(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

/// `x` as JSON with reference keys added (`None` values are left out).
fn with_refs<T: Serialize>(
    x: &T,
    refs: &[(&str, Option<Value>)],
) -> Result<String, serde_json::Error> {
    let mut v = serde_json::to_value(x)?;
    if let Value::Object(o) = &mut v {
        for (k, r) in refs {
            if let Some(r) = r {
                o.insert((*k).to_owned(), r.clone());
            }
        }
    }
    serde_json::to_string(&v)
}

/// One decoded part with the reference keys of its entry.
struct Decoded<T> {
    key: String,
    part: T,
    refs: Map<String, Value>,
}

/// Items of one kind from `meta` (`prefix<key>`) in [`key_order`]; the part
/// the deck has under the same key is kept where an entry is unreadable.
fn part_list<T: Clone + serde::de::DeserializeOwned>(
    meta: &BTreeMap<String, String>,
    prefix: &str,
    current: &[T],
    current_keys: &[String],
) -> Vec<Decoded<T>> {
    let mut keys: Vec<&str> = meta.keys().filter_map(|k| k.strip_prefix(prefix)).collect();
    keys.sort_by(|a, b| key_order(a, b));
    let mut out = Vec::new();
    for key in keys {
        let Some(j) = meta.get(&format!("{prefix}{key}")) else {
            continue;
        };
        let mut v = serde_json::from_str::<Value>(j).unwrap_or(Value::Null);
        let mut refs = Map::new();
        if let Value::Object(o) = &mut v {
            for r in ["@m", "@t", "@l"] {
                if let Some(x) = o.remove(r) {
                    refs.insert(r.to_owned(), x);
                }
            }
        }
        let part = match serde_json::from_value::<T>(v) {
            Ok(x) => x,
            Err(_) => match current_keys
                .iter()
                .position(|k| k == key)
                .and_then(|i| current.get(i))
            {
                Some(x) => x.clone(),
                None => continue,
            },
        };
        out.push(Decoded {
            key: key.to_owned(),
            part,
            refs,
        });
    }
    out
}

type PartLists = (Vec<SlideLayout>, Vec<SlideMaster>, Vec<Theme>, PartKeys);

fn decode_lists(meta: &BTreeMap<String, String>, cur: &PresFields, keys: &PartKeys) -> PartLists {
    let layouts = part_list(meta, "sl:", &cur.layouts, &keys.layouts);
    let masters = part_list(meta, "sm:", &cur.masters, &keys.masters);
    let themes = part_list(meta, "th:", &cur.themes, &keys.themes);
    let new_keys = PartKeys {
        layouts: layouts.iter().map(|d| d.key.clone()).collect(),
        masters: masters.iter().map(|d| d.key.clone()).collect(),
        themes: themes.iter().map(|d| d.key.clone()).collect(),
    };
    // A reference key → index in the new lists (entries without one keep their index).
    let find = |list: &[String], v: Option<&Value>| {
        v.and_then(Value::as_str)
            .and_then(|k| list.iter().position(|x| x == k))
    };
    let layouts_out = layouts
        .into_iter()
        .map(|d| {
            let mut l = d.part;
            if let Some(i) = find(&new_keys.masters, d.refs.get("@m")) {
                l.master = i;
            }
            l
        })
        .collect();
    let masters_out = masters
        .into_iter()
        .map(|d| {
            let mut m = d.part;
            if let Some(i) = find(&new_keys.themes, d.refs.get("@t")) {
                m.theme = i;
            }
            if let Some(Value::Array(ls)) = d.refs.get("@l") {
                for (r, k) in m.layouts.iter_mut().zip(ls) {
                    if let Some(i) = find(&new_keys.layouts, Some(k)) {
                        r.layout = i;
                    }
                }
            }
            m
        })
        .collect();
    let themes_out = themes.into_iter().map(|d| d.part).collect();
    (layouts_out, masters_out, themes_out, new_keys)
}

/// Layouts, masters and themes from `meta`. A master's text styles are tens
/// of kilobytes of nested values that the derived deserializer keeps on the
/// stack: native builds decode them on a thread with a large stack (the
/// default 1 MB of a Windows main thread is not enough in debug builds).
fn decode_parts(meta: &BTreeMap<String, String>, cur: &PresFields, keys: &PartKeys) -> PartLists {
    #[cfg(not(target_family = "wasm"))]
    {
        let big = std::thread::scope(|s| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(s, || decode_lists(meta, cur, keys))
                .ok()
                .and_then(|h| h.join().ok())
        });
        if let Some(r) = big {
            return r;
        }
    }
    decode_lists(meta, cur, keys)
}

/// Shadow of the shared slide maps.
#[derive(Debug, Default)]
pub struct SlideCollab {
    joining: bool,
    slides: BTreeMap<String, Shadow>,
    /// Package parts every replica has, with the fingerprint last shared: a
    /// part rewritten in place (comments, SmartArt, an embedded workbook) is
    /// shared again.
    known_parts: BTreeMap<String, (usize, u64)>,
    client: u64,
    counter: u32,
    /// Shared presentation parts (`meta`) and what the deck had at the last comparison.
    meta: BTreeMap<String, String>,
    seen: Option<Parts>,
    /// Shared keys of the deck's layouts, masters and themes.
    keys: PartKeys,
}

/// Minimal delta turning plain text `old` into `new` (UTF-16 lengths).
pub(crate) fn plain_delta(old: &str, new: &str) -> Vec<TextOp> {
    let (a, b): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let n = a.len().min(b.len());
    let mut p = 0;
    while p < n && a[p] == b[p] {
        p += 1;
    }
    let mut s = 0;
    while s < n - p && a[a.len() - 1 - s] == b[b.len() - 1 - s] {
        s += 1;
    }
    let u16s = |c: &[char]| c.iter().map(|c| c.len_utf16() as u32).sum::<u32>();
    let mut ops = Vec::new();
    if p > 0 {
        ops.push(TextOp::Retain {
            retain: u16s(&a[..p]),
            attributes: Map::new(),
        });
    }
    let ins: String = b[p..b.len() - s].iter().collect();
    if !ins.is_empty() {
        ops.push(TextOp::Insert {
            insert: Value::String(ins),
            attributes: Map::new(),
        });
    }
    let del = u16s(&a[p..a.len() - s]);
    if del > 0 {
        ops.push(TextOp::Delete { delete: del });
    }
    ops
}

/// Applies a delta to plain text (embeds count one position and are dropped).
pub(crate) fn apply_plain(text: &str, delta: &[TextOp]) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut out: Vec<u16> = Vec::with_capacity(units.len());
    let mut i = 0usize;
    for op in delta {
        match op {
            TextOp::Retain { retain, .. } => {
                let e = (i + *retain as usize).min(units.len());
                out.extend_from_slice(&units[i..e]);
                i = e;
            }
            TextOp::Delete { delete } => i = (i + *delete as usize).min(units.len()),
            TextOp::Insert { insert, .. } => {
                if let Value::String(t) = insert {
                    out.extend(t.encode_utf16());
                }
            }
        }
    }
    out.extend_from_slice(&units[i..]);
    String::from_utf16_lossy(&out)
}

/// Notes body text of a slide (§19.3.1.36 ph type="body").
fn notes_of(s: &Slide) -> Option<String> {
    let n = s.notes.as_ref()?;
    let i = super::notes_body_index(n)?;
    match n.data.shapes.get(i) {
        Some(ShapeNode::Shape(sh)) => Some(
            sh.text
                .as_ref()
                .map(super::text::body_text)
                .unwrap_or_default(),
        ),
        _ => None,
    }
}

/// Sets the notes body text of a slide (as the SetNotes command does).
fn set_notes(s: &mut Slide, text: &str) {
    let Some(n) = s.notes.as_mut() else { return };
    let Some(i) = super::notes_body_index(n) else {
        return;
    };
    if let Some(ShapeNode::Shape(sh)) = n.data.shapes.get_mut(i) {
        let tb = sh
            .text
            .get_or_insert_with(bo_model::slide::TextBody::default);
        super::text::set_text(tb, text);
    }
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

fn meta_json(s: &Slide, pos: f64, lk: Option<String>) -> String {
    let mut bare = s.clone();
    bare.data.shapes.clear();
    serde_json::to_string(&Meta {
        pos,
        lk,
        slide: bare,
    })
    .unwrap_or_default()
}

impl SlideCollab {
    fn new(pres: &Presentation, seed: bool, client: u64) -> SlideCollab {
        SlideCollab {
            joining: !seed,
            slides: BTreeMap::new(),
            known_parts: pres
                .parts
                .iter()
                .map(|p| (p.name.clone(), fingerprint(&p.data)))
                .collect(),
            client,
            counter: 0,
            meta: BTreeMap::new(),
            seen: None,
            keys: PartKeys::of(pres),
        }
    }

    /// Client id of the replica.
    pub(crate) fn client_id(&self) -> u64 {
        self.client
    }

    /// Keys for parts the deck gained since the last comparison (lists only
    /// grow at the end locally); a part created in this session gets a key
    /// that is never numeric and unique per replica.
    fn align_keys(&mut self, pres: &Presentation) {
        let (client, counter) = (self.client, &mut self.counter);
        let k = &mut self.keys;
        for (keys, len) in [
            (&mut k.layouts, pres.layouts.len()),
            (&mut k.masters, pres.masters.len()),
            (&mut k.themes, pres.themes.len()),
        ] {
            keys.truncate(len);
            while keys.len() < len {
                *counter = counter.wrapping_add(1);
                keys.push(format!("{client:x}-{counter}"));
            }
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
        self.align_keys(pres);
        let mut present = BTreeSet::new();
        // Order keys: slides already in shared order keep theirs.
        let keys: Vec<String> = pres.slides.iter().map(|s| s.slide_id.to_string()).collect();
        let known: Vec<Option<f64>> = keys
            .iter()
            .map(|k| self.slides.get(k).and_then(|sh| sh.ord))
            .collect();
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let ords = order::assign(&known, &refs);
        for (i, s) in pres.slides.iter().enumerate() {
            let key = s.slide_id.to_string();
            present.insert(key.clone());
            let sh = self.slides.entry(key.clone()).or_default();
            if let Some(t) = notes_of(s) {
                if sh.notes.as_deref() != Some(t.as_str()) {
                    let d = match &sh.notes {
                        Some(old) => TextDelta {
                            reset: false,
                            deltas: vec![plain_delta(old, &t)],
                        },
                        None => TextDelta {
                            reset: true,
                            deltas: vec![plain_delta("", &t)],
                        },
                    };
                    ch.texts.insert(format!("n:{key}"), Some(d));
                    sh.notes = Some(t);
                }
            }
            if sh.pos == i && sh.ord == Some(ords[i]) && sh.slide.as_ref() == Some(s) {
                continue;
            }
            let meta = meta_json(s, ords[i], self.keys.layouts.get(s.layout).cloned());
            sh.ord = Some(ords[i]);
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
                if sh.notes.is_some() {
                    ch.texts.insert(format!("n:{k}"), None);
                }
                ch.slides.insert(k, None);
                for sk in sh.shapes.into_keys() {
                    ch.shapes.insert(sk, None);
                }
            }
        }
        // Masters, layouts, themes.
        let parts = Parts::of(pres);
        if self.seen.as_ref() != Some(&parts) {
            let now = parts.entries(&self.keys);
            for (k, v) in &now {
                if self.meta.get(k) != Some(v) {
                    ch.meta.insert(k.clone(), Some(v.clone()));
                }
            }
            for k in self.meta.keys() {
                if !now.contains_key(k) {
                    ch.meta.insert(k.clone(), None);
                }
            }
            self.meta = now;
            self.seen = Some(parts);
        }
        for p in &pres.parts {
            let f = fingerprint(&p.data);
            if self.known_parts.insert(p.name.clone(), f) != Some(f) {
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
            let sh = self.slides.entry(k.clone()).or_default();
            sh.meta.clone_from(v);
            sh.ord = v
                .as_deref()
                .and_then(|j| serde_json::from_str::<Value>(j).ok())
                .and_then(|j| j.get("pos").and_then(Value::as_f64));
            affected.insert(k.clone());
        }
        for (k, v) in &change.texts {
            let Some(slide) = k.strip_prefix("n:") else {
                continue;
            };
            let sh = self.slides.entry(slide.to_owned()).or_default();
            sh.notes = match v {
                None => None,
                Some(tc) => {
                    let mut t = if tc.reset {
                        String::new()
                    } else {
                        sh.notes.clone().unwrap_or_default()
                    };
                    for d in &tc.deltas {
                        t = apply_plain(&t, d);
                    }
                    Some(t)
                }
            };
            affected.insert(slide.to_owned());
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
                    if let Some(i) =
                        m.lk.as_ref()
                            .and_then(|k| self.keys.layouts.iter().position(|x| x == k))
                    {
                        slide.layout = i;
                    }
                    if let Some(t) = &sh.notes {
                        if notes_of(&slide).as_ref() != Some(t) {
                            set_notes(&mut slide, t);
                        }
                    }
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

    /// [`Self::slide_collab_start`] with the change serialized as JSON (WASM bridge).
    pub fn slide_collab_start_json(&mut self, seed: bool, client: u64) -> crate::Result<String> {
        Ok(serde_json::to_string(&self.slide_collab_start(seed, client)?).unwrap_or_default())
    }

    /// Leave the collaboration session; the presentation stays open locally.
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
        let cur = pres.slides.get(sel.slide);
        let shape = match (cur, sel.ids.first()) {
            (Some(s), Some(id)) => Some(format!("{}/{id}", s.slide_id)),
            _ => None,
        };
        let slide = cur.map(|s| s.slide_id.to_string());
        Some(SlideCollabOut {
            change,
            shape,
            slide,
        })
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
                c.known_parts.insert(name.clone(), fingerprint(&data));
                let at = pres.parts.iter().position(|p| p.name == *name);
                if at.is_some_and(|i| pres.parts[i].data == data) {
                    continue;
                }
                let part = RawPart {
                    name: name.clone(),
                    content_type: ct,
                    data,
                };
                // A part rewritten in place keeps its slot (and its content type).
                let op = match at {
                    Some(i) => SlideOp::Parts {
                        index: i,
                        remove: vec![pres.parts[i].clone()],
                        insert: vec![RawPart {
                            content_type: pres.parts[i].content_type.clone().or(part.content_type),
                            ..part
                        }],
                    },
                    None => SlideOp::Parts {
                        index: pres.parts.len(),
                        remove: Vec::new(),
                        insert: vec![part],
                    },
                };
                if op.apply(pres).is_err() {
                    out.error = Some("parts".into());
                }
                if at.is_some() {
                    // What the slides draw from it (SmartArt, comments) changed.
                    out.all = true;
                }
            }
            // Masters, layouts, themes (before the slides that use them).
            let joining = c.joining;
            let mut hit = false;
            for (k, v) in &change.meta {
                if !is_part_key(k) {
                    continue;
                }
                hit = true;
                match v {
                    Some(j) => c.meta.insert(k.clone(), j.clone()),
                    None => c.meta.remove(k),
                };
            }
            if hit || (joining && !c.meta.is_empty() && !change.slides.is_empty()) {
                let before = PresFields::of(pres);
                let (rels, extra) = c
                    .meta
                    .get("pp")
                    .and_then(|j| serde_json::from_str::<(Vec<Rel>, Vec<Extra>)>(j).ok())
                    .unwrap_or_else(|| (before.rels.clone(), before.extra.clone()));
                let (layouts, masters, themes, keys) = decode_parts(&c.meta, &before, &c.keys);
                let (slide_size, notes_size) = c
                    .meta
                    .get("ps")
                    .and_then(|j| serde_json::from_str::<(SlideSize, SlideSize)>(j).ok())
                    .unwrap_or_else(|| (before.slide_size.clone(), before.notes_size.clone()));
                let after = PresFields {
                    rels,
                    extra,
                    layouts,
                    masters,
                    themes,
                    slide_size,
                    notes_size,
                    default_text_style: before.default_text_style.clone(),
                };
                if after != before {
                    let op = SlideOp::Pres {
                        before: Box::new(before),
                        after: Box::new(after),
                    };
                    if op.apply(pres).is_err() {
                        out.error = Some("presentation parts".into());
                    }
                    out.all = true;
                }
                // Slides keep their layout when the list order changed.
                if keys.layouts != c.keys.layouts {
                    let shadows = c.slides.values_mut().filter_map(|sh| sh.slide.as_mut());
                    for s in pres.slides.iter_mut().chain(shadows) {
                        if let Some(i) = c
                            .keys
                            .layouts
                            .get(s.layout)
                            .and_then(|k| keys.layouts.iter().position(|x| x == k))
                        {
                            s.layout = i;
                        }
                    }
                    out.all = true;
                    self.slides.undo.clear();
                    self.slides.redo.clear();
                }
                c.keys = keys;
                c.seen = Some(Parts::of(pres));
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

    /// [`Self::slide_collab_apply`] taking the remote change as JSON and returning
    /// the edit result as JSON (WASM bridge).
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
                if id.is_empty() {
                    // `"<slide>/"`: only the slide (no object selected).
                    let index = pres
                        .slides
                        .iter()
                        .position(|s| s.slide_id.to_string() == slide)?;
                    return Some(PeerShape {
                        slide: index,
                        rect: super::geom::BoxR::default(),
                    });
                }
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

    /// [`Self::slide_collab_peers`] taking the keys as a JSON array and returning
    /// the boxes as JSON (WASM bridge).
    pub fn slide_collab_peers_json(&self, json: &str) -> crate::Result<String> {
        let keys: Vec<String> =
            serde_json::from_str(json).map_err(|e| EngineError::BadCommand(e.to_string()))?;
        Ok(serde_json::to_string(&self.slide_collab_peers(&keys)?).unwrap_or_default())
    }
}
