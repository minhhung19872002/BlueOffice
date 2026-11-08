//! Collaboration core on [Yrs](https://github.com/y-crdt/y-crdt) — the Rust
//! port of Yjs — so the web (WASM), desktop (native) and the .NET relay
//! (`apps/server/BlueOffice.Api/Collab`, which only frames messages) all
//! speak the standard y-sync / awareness protocol (prompt §3.2, §6 Phase 6).
//!
//! Shared structures of one [`Session`] (one per open file):
//! * `body` — `Y.Text` of a BlueDoc document: paragraphs separated by `\n`,
//!   run formatting as text attributes, paragraph properties as attributes
//!   on the paragraph's `\n` (the same model as rich-text Yjs bindings);
//!   offsets count UTF-16 code units, like Yjs in the browser;
//! * `cells` — `Y.Map` of BlueSheet cell inputs keyed `"<sheet>!<A1>"`
//!   (last-writer-wins per cell);
//! * `shapes` — `Y.Map` of BlueSlide objects keyed `"<slide>/<shape>"`
//!   holding the object's properties as a JSON string;
//! * `slides` — `Y.Map` of BlueSlide slide properties and order keyed by
//!   slide id (JSON strings);
//! * `blobs` — `Y.Map` of binary parts added during the session (pictures)
//!   as JSON strings `{"ct","b64"}`;
//! * `meta` — `Y.Map` of other shared objects as JSON strings, last writer
//!   wins per key (BlueDoc styles, numbering, comments, table structure;
//!   BlueSheet sheets, rows, columns, formats, merges, rules; BlueSlide
//!   masters, layouts, themes);
//! * `texts` — `Y.Map` of nested `Y.Text`s ("stories" merged like the body:
//!   table cells, comments, foot/endnotes, headers/footers, slide notes).
//!
//! Editor bindings exchange [`Change`] envelopes with the replica: local
//! edits go in through [`Session::apply_local`]; everything else that changes
//! the shared state — remote updates, undo/redo — is collected and handed out
//! by [`Session::take_remote`] (body changes as Quill-style deltas).
//!
//! Local edits are tagged with the [`LOCAL_ORIGIN`] so the collaborative
//! [`UndoManager`] only undoes the current user's changes. Every mutating call
//! queues the y-sync `Update` message to broadcast; [`Session::drain_outbox`]
//! hands them to the transport.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use yrs::sync::{Awareness, DefaultProtocol, Message, Protocol, SyncMessage};
use yrs::types::text::{Diff, YChange};
use yrs::types::{Attrs, Delta, Event, PathSegment};
use yrs::undo::Options as UndoOptions;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};
use yrs::{
    Any, DeepObservable, Doc, GetString, In, Map, MapRef, Observable, OffsetKind, Options, Origin,
    Out, ReadTxn, StateVector, Subscription, Text, TextPrelim, TextRef, Transact, UndoManager,
};

/// Transaction origin of edits made by this replica's user.
pub const LOCAL_ORIGIN: &str = "bo-local";
/// Transaction origin of the initial content published by the seeder (not undoable).
pub const SEED_ORIGIN: &str = "bo-seed";

/// Milliseconds clock shared by awareness timeouts and undo grouping. On
/// native targets it follows the system clock; in the browser (where `std`
/// has no clock) the host feeds `Date.now()` through [`Session::set_time`].
#[derive(Clone, Default)]
struct HostClock(Arc<AtomicU64>);

impl yrs::sync::Clock for HostClock {
    fn now(&self) -> yrs::sync::Timestamp {
        #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            self.0.fetch_max(now, Ordering::Relaxed);
        }
        self.0.load(Ordering::Relaxed)
    }
}

/// Errors of the collaboration layer.
#[derive(Debug, thiserror::Error)]
pub enum CollabError {
    #[error("sync protocol: {0}")]
    Protocol(#[from] yrs::sync::Error),
    #[error("invalid argument: {0}")]
    Invalid(String),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CollabError>;

/// What a user broadcasts about themselves (awareness state).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    pub user_id: String,
    pub name: String,
    /// CSS colour of the user's caret/selection.
    pub color: String,
    /// Selection in the body text as UTF-16 offsets (anchor, head).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<(u32, u32)>,
    /// Selected sheet cell / range, e.g. `"Sheet1!B2:C4"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<String>,
    /// Selected slide object, e.g. `"s1/shape3"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
}

/// A run of the body text with its attributes (Quill-style delta op);
/// `insert` is a string, or an object for an embedded item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaRun {
    pub insert: Value,
    #[serde(default, skip_serializing_if = "JsonMap::is_empty")]
    pub attributes: JsonMap<String, Value>,
}

/// A change of the shared state, exchanged with editor bindings as JSON.
///
/// * `body` — deltas over the body `Y.Text`, applied in order; each op is
///   `{"insert": text | embed, "attributes"?}`, `{"delete": n}` or
///   `{"retain": n, "attributes"?}` (UTF-16 lengths, `null` attribute = remove);
/// * `slides`, `shapes`, `blobs`, `meta`, `cells` — map entries (`null` = removed);
/// * `texts` — nested texts by key (`null` = removed), see [`TextChange`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Change {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<Vec<Value>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slides: BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shapes: BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blobs: BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cells: BTreeMap<String, Option<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub texts: BTreeMap<String, Option<TextChange>>,
}

/// Change of one nested text: `reset` = the text is (re)created with the
/// content the deltas build from empty; otherwise deltas over its current
/// content (same op format as the body).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextChange {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deltas: Vec<Vec<Value>>,
}

impl Change {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.body.iter().all(Vec::is_empty)
            && self.slides.is_empty()
            && self.shapes.is_empty()
            && self.blobs.is_empty()
            && self.meta.is_empty()
            && self.cells.is_empty()
            && self.texts.is_empty()
    }
}

/// Changes observed but not yet handed to the editor.
#[derive(Default)]
struct Pending {
    body: Vec<Vec<Value>>,
    slides: BTreeSet<String>,
    shapes: BTreeSet<String>,
    blobs: BTreeSet<String>,
    meta: BTreeSet<String>,
    cells: BTreeSet<String>,
    /// Nested texts: `None` = re-read whole (created, replaced or removed).
    texts: BTreeMap<String, Option<Vec<Vec<Value>>>>,
}

/// One collaborative editing session (one document, one replica).
pub struct Session {
    awareness: Awareness,
    body: TextRef,
    cells: MapRef,
    shapes: MapRef,
    slides: MapRef,
    blobs: MapRef,
    meta: MapRef,
    texts: MapRef,
    undo: UndoManager,
    outbox: Vec<Vec<u8>>,
    clock: HostClock,
    /// Set while this replica applies changes its editor already has.
    quiet: Arc<AtomicBool>,
    pending: Arc<Mutex<Pending>>,
    _subs: Vec<Subscription>,
}

fn json_to_any(v: &Value) -> Any {
    match v {
        Value::Null => Any::Null,
        Value::Bool(b) => Any::Bool(*b),
        Value::Number(n) => Any::Number(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => Any::String(s.as_str().into()),
        Value::Array(a) => Any::Array(a.iter().map(json_to_any).collect::<Vec<_>>().into()),
        Value::Object(o) => Any::Map(Arc::new(
            o.iter().map(|(k, v)| (k.clone(), json_to_any(v))).collect(),
        )),
    }
}

fn any_to_json(a: &Any) -> Value {
    match a {
        Any::Null | Any::Undefined => Value::Null,
        Any::Bool(b) => Value::Bool(*b),
        Any::Number(n) => serde_json::Number::from_f64(*n).map_or(Value::Null, Value::Number),
        Any::BigInt(n) => Value::from(*n),
        Any::String(s) => Value::String(s.to_string()),
        Any::Buffer(b) => Value::Array(b.iter().map(|x| Value::from(*x)).collect()),
        Any::Array(v) => Value::Array(v.iter().map(any_to_json).collect()),
        Any::Map(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), any_to_json(v))).collect()),
    }
}

fn attrs(map: &JsonMap<String, Value>) -> Attrs {
    map.iter()
        .map(|(k, v)| (Arc::<str>::from(k.as_str()), json_to_any(v)))
        .collect()
}

fn attrs_to_json(a: &Attrs) -> JsonMap<String, Value> {
    a.iter()
        .map(|(k, v)| (k.to_string(), any_to_json(v)))
        .collect()
}

/// JSON delta op from a Yrs text delta.
fn delta_to_json(d: &Delta) -> Value {
    let mut o = JsonMap::new();
    let attributes = match d {
        Delta::Inserted(v, a) => {
            let insert = match v {
                Out::Any(Any::String(s)) => Value::String(s.to_string()),
                Out::Any(other) => any_to_json(other),
                // Shared types inside the body are not produced by BlueOffice;
                // they still occupy one position.
                _ => Value::Object(JsonMap::new()),
            };
            o.insert("insert".into(), insert);
            a.as_deref()
        }
        Delta::Deleted(n) => {
            o.insert("delete".into(), Value::from(*n));
            None
        }
        Delta::Retain(n, a) => {
            o.insert("retain".into(), Value::from(*n));
            a.as_deref()
        }
    };
    if let Some(a) = attributes.filter(|a| !a.is_empty()) {
        o.insert("attributes".into(), Value::Object(attrs_to_json(a)));
    }
    Value::Object(o)
}

fn op_len(o: &JsonMap<String, Value>, key: &str) -> Result<u32> {
    o.get(key)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| CollabError::Invalid(format!("delta {key}")))
}

/// Yrs delta from JSON ops.
fn json_to_delta(ops: &[Value]) -> Result<Vec<Delta<In>>> {
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let o = op
            .as_object()
            .ok_or_else(|| CollabError::Invalid("delta op".into()))?;
        let a = o
            .get("attributes")
            .and_then(Value::as_object)
            .filter(|a| !a.is_empty())
            .map(|a| Box::new(attrs(a)));
        if let Some(v) = o.get("insert") {
            let content = match v {
                Value::String(s) if !s.is_empty() => Any::String(s.as_str().into()),
                Value::Object(_) => json_to_any(v),
                _ => return Err(CollabError::Invalid("delta insert".into())),
            };
            out.push(Delta::Inserted(In::Any(content), a));
        } else if o.contains_key("delete") {
            out.push(Delta::Deleted(op_len(o, "delete")?));
        } else if o.contains_key("retain") {
            out.push(Delta::Retain(op_len(o, "retain")?, a));
        } else {
            return Err(CollabError::Invalid("delta op".into()));
        }
    }
    Ok(out)
}

/// Total length (UTF-16) a delta expects the text to have at least.
fn delta_span(ops: &[Delta<In>]) -> u64 {
    ops.iter()
        .map(|d| match d {
            Delta::Retain(n, _) | Delta::Deleted(n) => u64::from(*n),
            Delta::Inserted(..) => 0,
        })
        .sum()
}

/// Parses deltas applied in order to a text of length `len`, checking that
/// each one fits the text it applies to (before anything changes).
fn parse_deltas(ops: &[Vec<Value>], len: u32, what: &str) -> Result<Vec<Vec<Delta<In>>>> {
    let deltas = ops
        .iter()
        .map(|d| json_to_delta(d))
        .collect::<Result<Vec<_>>>()?;
    let mut len = u64::from(len);
    for d in &deltas {
        if delta_span(d) > len {
            return Err(CollabError::Invalid(format!("delta longer than {what}")));
        }
        let ins: u64 = d
            .iter()
            .map(|x| match x {
                Delta::Inserted(In::Any(Any::String(s)), _) => s.encode_utf16().count() as u64,
                Delta::Inserted(..) => 1,
                _ => 0,
            })
            .sum();
        let del: u64 = d
            .iter()
            .map(|x| match x {
                Delta::Deleted(n) => u64::from(*n),
                _ => 0,
            })
            .sum();
        len = len + ins - del;
    }
    Ok(deltas)
}

/// The content of a text as one delta of inserts.
fn text_runs(t: &TextRef, txn: &impl ReadTxn) -> Vec<Value> {
    t.diff(txn, YChange::identity)
        .into_iter()
        .filter_map(|d: Diff<YChange>| {
            let insert = match d.insert {
                Out::Any(Any::String(s)) => Value::String(s.to_string()),
                Out::Any(other) => any_to_json(&other),
                _ => return None,
            };
            let mut o = JsonMap::new();
            o.insert("insert".into(), insert);
            if let Some(a) = d.attributes.filter(|a| !a.is_empty()) {
                o.insert("attributes".into(), Value::Object(attrs_to_json(&a)));
            }
            Some(Value::Object(o))
        })
        .collect()
}

/// A nested text as a reset change (empty text → no deltas).
fn text_reset(t: &TextRef, txn: &impl ReadTxn) -> TextChange {
    let runs = text_runs(t, txn);
    TextChange {
        reset: true,
        deltas: if runs.is_empty() {
            Vec::new()
        } else {
            vec![runs]
        },
    }
}

fn encode(msg: &Message) -> Vec<u8> {
    let mut enc = EncoderV1::new();
    msg.encode(&mut enc);
    enc.to_vec()
}

fn cell_key(sheet: &str, cell: &str) -> Result<String> {
    if sheet.is_empty() || cell.is_empty() || cell.contains('!') {
        return Err(CollabError::Invalid(format!("cell {sheet}!{cell}")));
    }
    Ok(format!("{sheet}!{cell}"))
}

fn map_entry(map: &MapRef, txn: &impl ReadTxn, key: &str) -> Option<String> {
    match map.get(txn, key) {
        Some(Out::Any(Any::String(s))) => Some(s.to_string()),
        _ => None,
    }
}

fn map_all(map: &MapRef, txn: &impl ReadTxn) -> BTreeMap<String, Option<String>> {
    map.iter(txn)
        .filter_map(|(k, v)| match v {
            Out::Any(Any::String(s)) => Some((k.to_owned(), Some(s.to_string()))),
            _ => None,
        })
        .collect()
}

impl Session {
    /// New empty replica with a random client id.
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(Options::default())
    }

    /// New replica with a fixed client id (tests, deterministic replays).
    #[must_use]
    pub fn with_client_id(id: u64) -> Self {
        Self::with_options(Options::with_client_id(id))
    }

    fn with_options(mut options: Options) -> Self {
        // Offsets and lengths count UTF-16 code units, like Yjs in the browser.
        options.offset_kind = OffsetKind::Utf16;
        Self::with_doc(Doc::with_options(options))
    }

    fn with_doc(doc: Doc) -> Self {
        let body = doc.get_or_insert_text("body");
        let cells = doc.get_or_insert_map("cells");
        let shapes = doc.get_or_insert_map("shapes");
        let slides = doc.get_or_insert_map("slides");
        let blobs = doc.get_or_insert_map("blobs");
        let meta = doc.get_or_insert_map("meta");
        let texts = doc.get_or_insert_map("texts");
        let quiet = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(Mutex::new(Pending::default()));
        let mut subs = Vec::new();
        {
            let (q, p) = (quiet.clone(), pending.clone());
            subs.push(body.observe(move |txn, e| {
                if q.load(Ordering::SeqCst) {
                    return;
                }
                let ops: Vec<Value> = e.delta(txn).iter().map(delta_to_json).collect();
                if !ops.is_empty() {
                    p.lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .body
                        .push(ops);
                }
            }));
        }
        for (map, which) in [
            (&slides, 0u8),
            (&shapes, 1),
            (&blobs, 2),
            (&meta, 3),
            (&cells, 4),
        ] {
            let (q, p) = (quiet.clone(), pending.clone());
            subs.push(map.observe(move |txn, e| {
                if q.load(Ordering::SeqCst) {
                    return;
                }
                let mut p = p.lock().unwrap_or_else(PoisonError::into_inner);
                let set = match which {
                    0 => &mut p.slides,
                    1 => &mut p.shapes,
                    2 => &mut p.blobs,
                    3 => &mut p.meta,
                    _ => &mut p.cells,
                };
                set.extend(e.keys(txn).keys().map(ToString::to_string));
            }));
        }
        {
            let (q, p) = (quiet.clone(), pending.clone());
            subs.push(texts.observe_deep(move |txn, events| {
                if q.load(Ordering::SeqCst) {
                    return;
                }
                let mut p = p.lock().unwrap_or_else(PoisonError::into_inner);
                for e in events.iter() {
                    match e {
                        // Entries of `texts` itself: created, replaced or removed.
                        Event::Map(m) if m.path().is_empty() => {
                            for k in m.keys(txn).keys() {
                                p.texts.insert(k.to_string(), None);
                            }
                        }
                        Event::Text(t) => {
                            let Some(PathSegment::Key(k)) = t.path().front().cloned() else {
                                continue;
                            };
                            let ops: Vec<Value> = t.delta(txn).iter().map(delta_to_json).collect();
                            if ops.is_empty() {
                                continue;
                            }
                            // A pending re-read already covers the deltas.
                            let slot = p
                                .texts
                                .entry(k.to_string())
                                .or_insert_with(|| Some(Vec::new()));
                            if let Some(list) = slot {
                                list.push(ops);
                            }
                        }
                        _ => {}
                    }
                }
            }));
        }
        let clock = HostClock::default();
        let mut tracked = HashSet::new();
        tracked.insert(Origin::from(LOCAL_ORIGIN));
        let mut undo = UndoManager::with_scope_and_options(
            &doc,
            &body,
            UndoOptions {
                tracked_origins: tracked,
                capture_timeout_millis: 500,
                capture_transaction: None,
                timestamp: Arc::new(clock.clone()),
                init_undo_stack: Vec::new(),
                init_redo_stack: Vec::new(),
            },
        );
        undo.expand_scope(&cells);
        undo.expand_scope(&shapes);
        undo.expand_scope(&slides);
        undo.expand_scope(&meta);
        undo.expand_scope(&texts);
        Self {
            awareness: Awareness::with_clock(doc, clock.clone()),
            body,
            cells,
            shapes,
            slides,
            blobs,
            meta,
            texts,
            undo,
            outbox: Vec::new(),
            clock,
            quiet,
            pending,
            _subs: subs,
        }
    }

    /// Advances the session clock (milliseconds since the Unix epoch); needed
    /// on `wasm32-unknown-unknown`, harmless elsewhere.
    pub fn set_time(&self, now_ms: u64) {
        self.clock.0.fetch_max(now_ms, Ordering::Relaxed);
    }

    #[must_use]
    pub fn client_id(&self) -> u64 {
        self.awareness.client_id()
    }

    /// Messages to send right after connecting (sync step 1 + own awareness).
    pub fn start(&self) -> Result<Vec<u8>> {
        let mut enc = EncoderV1::new();
        DefaultProtocol.start(&self.awareness, &mut enc)?;
        Ok(enc.to_vec())
    }

    /// Handles one binary WebSocket frame; returns replies for the sender.
    pub fn receive(&mut self, frame: &[u8]) -> Result<Vec<Vec<u8>>> {
        let replies = DefaultProtocol.handle(&self.awareness, frame)?;
        Ok(replies.iter().map(encode).collect())
    }

    /// Takes the queued broadcast messages (local updates, presence).
    pub fn drain_outbox(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outbox)
    }

    /// Full document state as one Yjs v1 update (snapshot for the server).
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        self.awareness
            .doc()
            .transact()
            .encode_state_as_update_v1(&StateVector::default())
    }

    /// Applies a snapshot (or any v1 update) received out of band, e.g.
    /// the state stored by the server when the session was created.
    pub fn load_snapshot(&mut self, update: &[u8]) -> Result<()> {
        let update = yrs::Update::decode_v1(update)
            .map_err(|e| CollabError::Invalid(format!("snapshot: {e}")))?;
        self.doc()
            .transact_mut()
            .apply_update(update)
            .map_err(|e| CollabError::Invalid(format!("snapshot: {e}")))
    }

    fn doc(&self) -> &Doc {
        self.awareness.doc()
    }

    fn edit<F: FnOnce(&mut yrs::TransactionMut<'_>)>(&mut self, f: F) {
        self.edit_with(LOCAL_ORIGIN, f);
    }

    /// Runs a change the editor already has (not reported by [`Session::take_remote`]).
    fn edit_with<F: FnOnce(&mut yrs::TransactionMut<'_>)>(&mut self, origin: &str, f: F) {
        self.quiet.store(true, Ordering::SeqCst);
        let update = {
            let mut txn = self.doc().transact_mut_with(origin);
            f(&mut txn);
            txn.encode_update_v1()
        };
        self.quiet.store(false, Ordering::SeqCst);
        self.queue_update(update);
    }

    fn queue_update(&mut self, update: Vec<u8>) {
        // An empty v1 update is [0, 0]: nothing changed.
        if update.len() > 2 {
            self.outbox
                .push(encode(&Message::Sync(SyncMessage::Update(update))));
        }
    }

    // ------------------------------------------------------ editor bindings

    fn apply_change(&mut self, origin: &str, change: &Change) -> Result<()> {
        let deltas = parse_deltas(&change.body, self.body_len(), "body")?;
        // Nested texts: (key, recreate, deltas); `None` removes the text.
        let mut nested = Vec::with_capacity(change.texts.len());
        {
            let txn = self.doc().transact();
            for (k, v) in &change.texts {
                let Some(tc) = v else {
                    nested.push((k.clone(), None));
                    continue;
                };
                let current = match self.texts.get(&txn, k) {
                    Some(Out::YText(t)) if !tc.reset => Some(t.len(&txn)),
                    _ => None,
                };
                let d = parse_deltas(&tc.deltas, current.unwrap_or(0), k)?;
                nested.push((k.clone(), Some((current.is_none(), d))));
            }
        }
        let (body, slides, shapes, blobs, meta, cells, texts) = (
            self.body.clone(),
            self.slides.clone(),
            self.shapes.clone(),
            self.blobs.clone(),
            self.meta.clone(),
            self.cells.clone(),
            self.texts.clone(),
        );
        self.edit_with(origin, |txn| {
            for d in deltas {
                body.apply_delta(txn, d);
            }
            for (k, v) in nested {
                match v {
                    None => {
                        texts.remove(txn, &k);
                    }
                    Some((fresh, ds)) => {
                        let t = if fresh {
                            texts.insert(txn, k.as_str(), TextPrelim::new(""))
                        } else {
                            match texts.get(txn, &k) {
                                Some(Out::YText(t)) => t,
                                _ => texts.insert(txn, k.as_str(), TextPrelim::new("")),
                            }
                        };
                        for d in ds {
                            t.apply_delta(txn, d);
                        }
                    }
                }
            }
            for (map, entries) in [
                (&slides, &change.slides),
                (&shapes, &change.shapes),
                (&blobs, &change.blobs),
                (&meta, &change.meta),
                (&cells, &change.cells),
            ] {
                for (k, v) in entries {
                    match v {
                        Some(v) => {
                            map.insert(txn, k.as_str(), v.as_str());
                        }
                        None => {
                            map.remove(txn, k);
                        }
                    }
                }
            }
        });
        Ok(())
    }

    /// Applies a change made by this replica's editor (undoable by this user).
    /// Body deltas are relative to the current body: the editor applies every
    /// change from [`Session::take_remote`] before sending new ones.
    pub fn apply_local(&mut self, change: &Change) -> Result<()> {
        self.apply_change(LOCAL_ORIGIN, change)
    }

    /// Publishes the initial content (the seeder's file); not undoable.
    pub fn seed(&mut self, change: &Change) -> Result<()> {
        self.apply_change(SEED_ORIGIN, change)
    }

    /// Changes from other replicas and from undo/redo since the last call,
    /// to apply to the editor's model in this order.
    pub fn take_remote(&mut self) -> Change {
        let p = std::mem::take(&mut *self.pending.lock().unwrap_or_else(PoisonError::into_inner));
        let txn = self.doc().transact();
        let read = |map: &MapRef, keys: BTreeSet<String>| -> BTreeMap<String, Option<String>> {
            keys.into_iter()
                .map(|k| {
                    let v = map_entry(map, &txn, &k);
                    (k, v)
                })
                .collect()
        };
        let texts = p
            .texts
            .into_iter()
            .map(|(k, v)| {
                let tc = match v {
                    Some(deltas) => Some(TextChange {
                        reset: false,
                        deltas,
                    }),
                    None => match self.texts.get(&txn, &k) {
                        Some(Out::YText(t)) => Some(text_reset(&t, &txn)),
                        _ => None,
                    },
                };
                (k, tc)
            })
            .collect();
        Change {
            body: p.body,
            slides: read(&self.slides, p.slides),
            shapes: read(&self.shapes, p.shapes),
            blobs: read(&self.blobs, p.blobs),
            meta: read(&self.meta, p.meta),
            cells: read(&self.cells, p.cells),
            texts,
        }
    }

    /// The whole shared state as one change from an empty document (late
    /// joiners load it after the first sync); clears pending remote changes.
    pub fn full_state(&mut self) -> Change {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = Pending::default();
        let body: Vec<Value> = self
            .body_delta()
            .into_iter()
            .map(|r| {
                let mut o = JsonMap::new();
                o.insert("insert".into(), r.insert);
                if !r.attributes.is_empty() {
                    o.insert("attributes".into(), Value::Object(r.attributes));
                }
                Value::Object(o)
            })
            .collect();
        let txn = self.doc().transact();
        Change {
            body: if body.is_empty() {
                Vec::new()
            } else {
                vec![body]
            },
            slides: map_all(&self.slides, &txn),
            shapes: map_all(&self.shapes, &txn),
            blobs: map_all(&self.blobs, &txn),
            meta: map_all(&self.meta, &txn),
            cells: map_all(&self.cells, &txn),
            texts: self
                .texts
                .iter(&txn)
                .filter_map(|(k, v)| match v {
                    Out::YText(t) => Some((k.to_owned(), Some(text_reset(&t, &txn)))),
                    _ => None,
                })
                .collect(),
        }
    }

    /// Entries of the `meta` map (JSON strings).
    #[must_use]
    pub fn meta(&self) -> BTreeMap<String, String> {
        let txn = self.doc().transact();
        map_all(&self.meta, &txn)
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect()
    }

    /// Plain text of a nested text (embeds left out).
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        let txn = self.doc().transact();
        match self.texts.get(&txn, key) {
            Some(Out::YText(t)) => Some(t.get_string(&txn)),
            _ => None,
        }
    }

    /// Keys of the nested texts.
    #[must_use]
    pub fn text_keys(&self) -> Vec<String> {
        let txn = self.doc().transact();
        self.texts.keys(&txn).map(str::to_owned).collect()
    }

    // ------------------------------------------------------------ BlueDoc

    /// Body length in UTF-16 code units (Yjs/JS string indexing).
    #[must_use]
    pub fn body_len(&self) -> u32 {
        self.body.len(&self.doc().transact())
    }

    fn check_range(&self, index: u32, len: u32) -> Result<()> {
        let total = self.body_len();
        if index.checked_add(len).is_none_or(|end| end > total) {
            return Err(CollabError::Invalid(format!(
                "range {index}+{len} outside body of length {total}"
            )));
        }
        Ok(())
    }

    /// Inserts text (with run attributes) at a UTF-16 offset.
    pub fn insert_text(
        &mut self,
        index: u32,
        text: &str,
        attributes: &JsonMap<String, Value>,
    ) -> Result<()> {
        self.check_range(index, 0)?;
        let body = self.body.clone();
        let a = attrs(attributes);
        self.edit(|txn| {
            if a.is_empty() {
                body.insert(txn, index, text);
            } else {
                body.insert_with_attributes(txn, index, text, a);
            }
        });
        Ok(())
    }

    /// Deletes `len` UTF-16 code units at `index`.
    pub fn delete_text(&mut self, index: u32, len: u32) -> Result<()> {
        self.check_range(index, len)?;
        let body = self.body.clone();
        self.edit(|txn| body.remove_range(txn, index, len));
        Ok(())
    }

    /// Sets (value) or clears (`null`) attributes on a range.
    pub fn format_text(
        &mut self,
        index: u32,
        len: u32,
        attributes: &JsonMap<String, Value>,
    ) -> Result<()> {
        self.check_range(index, len)?;
        let body = self.body.clone();
        let a = attrs(attributes);
        self.edit(|txn| body.format(txn, index, len, a));
        Ok(())
    }

    #[must_use]
    pub fn body_text(&self) -> String {
        self.body.get_string(&self.doc().transact())
    }

    /// Body as a list of attributed runs (embeds as objects).
    #[must_use]
    pub fn body_delta(&self) -> Vec<DeltaRun> {
        let txn = self.doc().transact();
        self.body
            .diff(&txn, YChange::identity)
            .into_iter()
            .filter_map(|d: Diff<YChange>| {
                let insert = match d.insert {
                    Out::Any(Any::String(s)) => Value::String(s.to_string()),
                    Out::Any(other) => any_to_json(&other),
                    _ => return None,
                };
                let attributes = d.attributes.map(|a| attrs_to_json(&a)).unwrap_or_default();
                Some(DeltaRun { insert, attributes })
            })
            .collect()
    }

    // ---------------------------------------------------------- BlueSheet

    /// Sets the input (literal or `=formula`) of a cell.
    pub fn set_cell(&mut self, sheet: &str, cell: &str, input: &str) -> Result<()> {
        let key = cell_key(sheet, cell)?;
        let cells = self.cells.clone();
        let input = input.to_owned();
        self.edit(|txn| {
            cells.insert(txn, key, input);
        });
        Ok(())
    }

    pub fn clear_cell(&mut self, sheet: &str, cell: &str) -> Result<()> {
        let key = cell_key(sheet, cell)?;
        let cells = self.cells.clone();
        self.edit(|txn| {
            cells.remove(txn, &key);
        });
        Ok(())
    }

    /// All cell inputs keyed `"<sheet>!<A1>"`.
    #[must_use]
    pub fn cells(&self) -> BTreeMap<String, String> {
        let txn = self.doc().transact();
        self.cells
            .iter(&txn)
            .filter_map(|(k, v)| match v {
                Out::Any(Any::String(s)) => Some((k.to_owned(), s.to_string())),
                _ => None,
            })
            .collect()
    }

    // ---------------------------------------------------------- BlueSlide

    /// Inserts or replaces a slide object's properties.
    pub fn set_shape(&mut self, slide: &str, shape: &str, props: &Value) -> Result<()> {
        if slide.contains('/') || slide.is_empty() || shape.is_empty() {
            return Err(CollabError::Invalid(format!("shape {slide}/{shape}")));
        }
        let key = format!("{slide}/{shape}");
        let json = serde_json::to_string(props)?;
        let shapes = self.shapes.clone();
        self.edit(|txn| {
            shapes.insert(txn, key, json);
        });
        Ok(())
    }

    pub fn remove_shape(&mut self, slide: &str, shape: &str) {
        let key = format!("{slide}/{shape}");
        let shapes = self.shapes.clone();
        self.edit(|txn| {
            shapes.remove(txn, &key);
        });
    }

    /// All slide objects keyed `"<slide>/<shape>"`.
    #[must_use]
    pub fn shapes(&self) -> BTreeMap<String, Value> {
        let txn = self.doc().transact();
        self.shapes
            .iter(&txn)
            .filter_map(|(k, v)| match v {
                Out::Any(Any::String(s)) => {
                    serde_json::from_str(&s).ok().map(|j| (k.to_owned(), j))
                }
                _ => None,
            })
            .collect()
    }

    /// Slide properties keyed by slide id.
    #[must_use]
    pub fn slides(&self) -> BTreeMap<String, String> {
        let txn = self.doc().transact();
        map_all(&self.slides, &txn)
            .into_iter()
            .filter_map(|(k, v)| v.map(|v| (k, v)))
            .collect()
    }

    // ------------------------------------------------------- undo / redo

    fn undo_redo(&mut self, redo: bool) -> bool {
        let before = self.doc().transact().state_vector();
        let done = if redo {
            self.undo.redo_blocking()
        } else {
            self.undo.undo_blocking()
        };
        if done {
            let update = self.doc().transact().encode_diff_v1(&before);
            self.queue_update(update);
        }
        done
    }

    /// Undoes the last local change (remote changes are never undone). The
    /// resulting change is reported by [`Session::take_remote`].
    pub fn undo(&mut self) -> bool {
        self.undo_redo(false)
    }

    pub fn redo(&mut self) -> bool {
        self.undo_redo(true)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    /// Closes the current undo group so the next edit starts a new one.
    pub fn stop_capturing(&mut self) {
        self.undo.reset();
    }

    // ---------------------------------------------------------- presence

    /// Publishes this user's presence and queues the awareness message.
    pub fn set_presence(&mut self, p: &Presence) -> Result<()> {
        self.awareness
            .set_local_state(p)
            .map_err(|e| CollabError::Invalid(e.to_string()))?;
        let update = self
            .awareness
            .update()
            .map_err(|e| CollabError::Invalid(e.to_string()))?;
        self.outbox.push(encode(&Message::Awareness(update)));
        Ok(())
    }

    /// Presence of the other connected users.
    #[must_use]
    pub fn peers(&self) -> Vec<(u64, Presence)> {
        let me = self.client_id();
        let mut v: Vec<(u64, Presence)> = self
            .awareness
            .iter()
            .filter(|(id, _)| *id != me)
            .filter_map(|(id, state)| {
                let json = state.data?;
                serde_json::from_str(&json).ok().map(|p| (id, p))
            })
            .collect();
        v.sort_by_key(|(id, _)| *id);
        v
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn exchange(a: &mut Session, b: &mut Session) {
        for _ in 0..3 {
            for f in a.drain_outbox() {
                for r in b.receive(&f).unwrap() {
                    a.receive(&r).unwrap();
                }
            }
            for f in b.drain_outbox() {
                for r in a.receive(&f).unwrap() {
                    b.receive(&r).unwrap();
                }
            }
        }
    }

    fn change(v: serde_json::Value) -> Change {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn nested_texts_and_meta_merge() {
        let (mut a, mut b) = (Session::with_client_id(1), Session::with_client_id(2));
        a.apply_local(&change(json!({
            "texts": {"c:t:r:c": {"reset": true, "deltas": [[{"insert": "ô\n"}]]}},
            "meta": {"tb:t": "{}"}
        })))
        .unwrap();
        exchange(&mut a, &mut b);
        let r = b.take_remote();
        assert_eq!(r.meta["tb:t"].as_deref(), Some("{}"));
        let t = r.texts["c:t:r:c"].as_ref().unwrap();
        assert!(t.reset);
        assert_eq!(b.text("c:t:r:c").as_deref(), Some("ô\n"));
        // Concurrent edits of the same nested text merge.
        a.apply_local(&change(json!({
            "texts": {"c:t:r:c": {"deltas": [[{"insert": "A"}]]}}
        })))
        .unwrap();
        b.apply_local(&change(json!({
            "texts": {"c:t:r:c": {"deltas": [[{"retain": 1}, {"insert": "B"}]]}}
        })))
        .unwrap();
        exchange(&mut a, &mut b);
        assert_eq!(a.text("c:t:r:c"), b.text("c:t:r:c"));
        assert_eq!(a.text("c:t:r:c").as_deref(), Some("AôB\n"));
        let r = a.take_remote();
        let d = r.texts["c:t:r:c"].as_ref().unwrap();
        assert!(!d.reset);
        assert_eq!(d.deltas.len(), 1);
        // Undo only reverts the own nested edit.
        assert!(b.undo());
        let r = b.take_remote();
        assert!(r.texts.contains_key("c:t:r:c"));
        exchange(&mut a, &mut b);
        assert_eq!(a.text("c:t:r:c").as_deref(), Some("Aô\n"));
        // Full state carries everything for late joiners.
        let full = a.full_state();
        assert!(full.texts["c:t:r:c"].as_ref().unwrap().reset);
        assert_eq!(full.meta.len(), 1);
        // Too long a delta is refused before anything changes.
        assert!(a
            .apply_local(&change(json!({
                "texts": {"c:t:r:c": {"deltas": [[{"retain": 99}, {"insert": "x"}]]}}
            })))
            .is_err());
        // Removal.
        a.apply_local(&change(json!({"texts": {"c:t:r:c": null}})))
            .unwrap();
        exchange(&mut a, &mut b);
        assert_eq!(b.take_remote().texts.get("c:t:r:c"), Some(&None));
        assert_eq!(b.text("c:t:r:c"), None);
    }
}
