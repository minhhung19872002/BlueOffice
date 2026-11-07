//! Collaboration core on [Yrs](https://github.com/y-crdt/y-crdt) — the Rust
//! port of Yjs — so the web (WASM), desktop (native) and the .NET relay
//! (`apps/server/BlueOffice.Api/Collab`, which only frames messages) all
//! speak the standard y-sync / awareness protocol (prompt §3.2, §6 Phase 6).
//!
//! Shared structures of one [`Session`] (one per open file):
//! * `body` — `Y.Text` of a BlueDoc document: paragraphs separated by `\n`,
//!   run formatting as text attributes, paragraph properties as attributes
//!   on the paragraph's `\n` (the same model as rich-text Yjs bindings);
//! * `cells` — `Y.Map` of BlueSheet cell inputs keyed `"<sheet>!<A1>"`
//!   (last-writer-wins per cell);
//! * `shapes` — `Y.Map` of BlueSlide objects keyed `"<slide>/<shape>"`
//!   holding the object's properties as a JSON string.
//!
//! Local edits are tagged with the [`LOCAL_ORIGIN`] so the collaborative
//! [`UndoManager`] only undoes the current user's changes. Every mutating call
//! queues the y-sync `Update` message to broadcast; [`Session::drain_outbox`]
//! hands them to the transport.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use yrs::sync::{Awareness, DefaultProtocol, Message, Protocol, SyncMessage};
use yrs::types::text::{Diff, YChange};
use yrs::types::Attrs;
use yrs::undo::Options as UndoOptions;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};
use yrs::{
    Any, Doc, GetString, Map, MapRef, Origin, Out, ReadTxn, StateVector, Text, TextRef, Transact,
    UndoManager,
};

/// Transaction origin of edits made by this replica's user.
pub const LOCAL_ORIGIN: &str = "bo-local";

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

/// A run of the body text with its attributes (Quill-style delta op).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeltaRun {
    pub insert: String,
    #[serde(default, skip_serializing_if = "JsonMap::is_empty")]
    pub attributes: JsonMap<String, Value>,
}

/// One collaborative editing session (one document, one replica).
pub struct Session {
    awareness: Awareness,
    body: TextRef,
    cells: MapRef,
    shapes: MapRef,
    undo: UndoManager,
    outbox: Vec<Vec<u8>>,
    clock: HostClock,
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

impl Session {
    /// New empty replica with a random client id.
    #[must_use]
    pub fn new() -> Self {
        Self::with_doc(Doc::new())
    }

    /// New replica with a fixed client id (tests, deterministic replays).
    #[must_use]
    pub fn with_client_id(id: u64) -> Self {
        Self::with_doc(Doc::with_client_id(id))
    }

    fn with_doc(doc: Doc) -> Self {
        let body = doc.get_or_insert_text("body");
        let cells = doc.get_or_insert_map("cells");
        let shapes = doc.get_or_insert_map("shapes");
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
        Self {
            awareness: Awareness::with_clock(doc, clock.clone()),
            body,
            cells,
            shapes,
            undo,
            outbox: Vec::new(),
            clock,
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
        let update = {
            let mut txn = self.doc().transact_mut_with(LOCAL_ORIGIN);
            f(&mut txn);
            txn.encode_update_v1()
        };
        self.queue_update(update);
    }

    fn queue_update(&mut self, update: Vec<u8>) {
        // An empty v1 update is [0, 0]: nothing changed.
        if update.len() > 2 {
            self.outbox
                .push(encode(&Message::Sync(SyncMessage::Update(update))));
        }
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

    /// Body as a list of attributed runs.
    #[must_use]
    pub fn body_delta(&self) -> Vec<DeltaRun> {
        let txn = self.doc().transact();
        self.body
            .diff(&txn, YChange::identity)
            .into_iter()
            .filter_map(|d: Diff<YChange>| {
                let insert = match d.insert {
                    Out::Any(Any::String(s)) => s.to_string(),
                    _ => return None,
                };
                let attributes = d
                    .attributes
                    .map(|a| {
                        a.iter()
                            .map(|(k, v)| (k.to_string(), any_to_json(v)))
                            .collect()
                    })
                    .unwrap_or_default();
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

    /// Undoes the last local change (remote changes are never undone).
    pub fn undo(&mut self) -> bool {
        self.undo_redo(false)
    }

    pub fn redo(&mut self) -> bool {
        self.undo_redo(true)
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
