//! Lazy object store: the PDF's cross-reference information is read at open, objects are
//! parsed on first use and kept.
//!
//! - ISO 32000-1 §7.5.4 cross-reference table, §7.5.5 trailer, §7.5.6 incremental updates
//!   (`/Prev`), §7.5.8 cross-reference streams and hybrid files (`/XRefStm`);
//! - §7.3 objects (own parser over [`crate::lexer`] tokens, indirect references, streams with a
//!   direct or indirect `/Length`);
//! - §7.5.7 object streams, decoded on the first use of one of their objects and cached.
//!
//! Opening the 1 202-page manual reads one table and ~1 300 page objects instead of every
//! object of the file. Anything that needs the whole object graph (editing, saving, lopdf's own
//! helpers) goes through `Deref` / `DerefMut`, which load the full `lopdf::Document` once
//! exactly as before (`Document::load_mem`), so the write path keeps its behaviour.
//! Encrypted files and files whose cross-reference information cannot be read are loaded in
//! full at open.
use crate::lexer::{Lexer, Tok};
use lopdf::xref::{Xref, XrefEntry, XrefType};
use lopdf::{Dictionary, Object, ObjectId, ObjectStream, Stream};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

/// Longest chain of references followed (as lopdf).
const DEREF_LIMIT: usize = 128;
/// Deepest nesting of arrays / dictionaries parsed.
const MAX_DEPTH: u32 = 100;
/// Deepest chain of indirect `/Length` values (and object stream containers) followed.
const MAX_CHAIN: u32 = 16;

type Res<T> = lopdf::Result<T>;

/// A PDF document whose objects load on demand (see the module documentation).
pub(crate) struct Document {
    /// The whole document, once something needed it.
    full: OnceLock<lopdf::Document>,
    /// The on-demand reader; `None` once the full document replaced it for writing.
    lazy: Option<Arc<Lazy>>,
}

struct Lazy {
    bytes: Arc<Vec<u8>>,
    /// Where `%PDF-` starts (offsets in the file count from there, as lopdf).
    base: usize,
    version: String,
    trailer: Dictionary,
    xref: Xref,
    /// Parsed objects by object number, with the id their header gave.
    slots: Vec<OnceLock<Option<(ObjectId, Object)>>>,
    /// Decoded object streams by container object number.
    containers: Mutex<HashMap<u32, Arc<BTreeMap<ObjectId, Object>>>>,
    /// Positions of every `n g obj` header, for entries whose offset is wrong.
    headers: OnceLock<HashMap<ObjectId, usize>>,
}

impl Clone for Document {
    fn clone(&self) -> Self {
        match self.full.get() {
            Some(d) => Self::from(d.clone()),
            None => Self {
                full: OnceLock::new(),
                lazy: self.lazy.clone(),
            },
        }
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("materialized", &self.is_materialized())
            .finish_non_exhaustive()
    }
}

impl From<lopdf::Document> for Document {
    fn from(d: lopdf::Document) -> Self {
        Self {
            full: OnceLock::from(d),
            lazy: None,
        }
    }
}

impl std::ops::Deref for Document {
    type Target = lopdf::Document;

    fn deref(&self) -> &lopdf::Document {
        self.full.get_or_init(|| self.materialize())
    }
}

impl std::ops::DerefMut for Document {
    fn deref_mut(&mut self) -> &mut lopdf::Document {
        if self.full.get().is_none() {
            let d = self.materialize();
            let _ = self.full.set(d);
        }
        // Edits change objects: the on-demand reader of the original bytes is stale from now.
        self.lazy = None;
        match self.full.get_mut() {
            Some(d) => d,
            None => unreachable!("set above"),
        }
    }
}

impl Document {
    /// Read the cross-reference information of `bytes[base..]` (from `%PDF-`). `None` when it
    /// cannot be read or the file is encrypted: the caller loads the whole file instead.
    pub(crate) fn open_lazy(bytes: &Arc<Vec<u8>>, base: usize) -> Option<Self> {
        let lazy = Lazy::open(bytes, base)?;
        Some(Self {
            full: OnceLock::new(),
            lazy: Some(Arc::new(lazy)),
        })
    }

    /// Whether the whole document has been loaded.
    pub(crate) fn is_materialized(&self) -> bool {
        self.full.get().is_some()
    }

    fn lazy(&self) -> Option<&Lazy> {
        if self.full.get().is_some() {
            return None;
        }
        self.lazy.as_deref()
    }

    fn materialize(&self) -> lopdf::Document {
        let Some(l) = self.lazy.as_deref() else {
            return lopdf::Document::new();
        };
        // `BO_PDF_LOAD_TRACE=<file>` records what needed the whole document (diagnostics).
        if let Some(p) = std::env::var_os("BO_PDF_LOAD_TRACE") {
            use std::io::Write as _;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
            {
                let _ = writeln!(f, "{}", std::backtrace::Backtrace::force_capture());
            }
        }
        lopdf::Document::load_mem(l.buf()).unwrap_or_else(|_| l.everything())
    }

    /// The trailer dictionary (the newest one).
    pub(crate) fn trailer_ref(&self) -> &Dictionary {
        match self.lazy() {
            Some(l) => &l.trailer,
            None => &self.trailer,
        }
    }

    /// PDF version from the header.
    pub(crate) fn pdf_version(&self) -> &str {
        match self.lazy() {
            Some(l) => &l.version,
            None => &self.version,
        }
    }

    /// Whether the file is encrypted (files read on demand never are).
    pub(crate) fn is_encrypted(&self) -> bool {
        match self.lazy() {
            Some(_) => false,
            None => (**self).is_encrypted(),
        }
    }

    /// Object `id`, references followed.
    pub(crate) fn get_object(&self, id: ObjectId) -> Res<&Object> {
        match self.lazy() {
            Some(l) => {
                let o = l.get(id, 0).ok_or(lopdf::Error::ObjectNotFound(id))?;
                self.dereference(o).map(|(_, o)| o)
            }
            None => (**self).get_object(id),
        }
    }

    /// Follow references from `object` (at most 128).
    pub(crate) fn dereference<'a>(
        &'a self,
        mut object: &'a Object,
    ) -> Res<(Option<ObjectId>, &'a Object)> {
        let Some(l) = self.lazy() else {
            return (**self).dereference(object);
        };
        let mut id = None;
        let mut n = 0;
        while let Ok(r) = object.as_reference() {
            id = Some(r);
            object = l.get(r, 0).ok_or(lopdf::Error::ObjectNotFound(r))?;
            n += 1;
            if n > DEREF_LIMIT {
                return Err(lopdf::Error::ReferenceLimit);
            }
        }
        Ok((id, object))
    }

    /// Dictionary object `id`.
    pub(crate) fn get_dictionary(&self, id: ObjectId) -> Res<&Dictionary> {
        self.get_object(id).and_then(Object::as_dict)
    }

    /// The document catalog (`/Root`).
    pub(crate) fn catalog(&self) -> Res<&Dictionary> {
        self.trailer_ref()
            .get(b"Root")
            .and_then(Object::as_reference)
            .and_then(|id| self.get_dictionary(id))
    }

    /// Page numbers (from 1) and their page objects, in page tree order (§7.7.3.2), walked
    /// like lopdf's `get_pages`.
    pub(crate) fn get_pages(&self) -> BTreeMap<u32, ObjectId> {
        let Some(l) = self.lazy() else {
            return (**self).get_pages();
        };
        let mut out = BTreeMap::new();
        let kids = |id: ObjectId| -> Option<&[Object]> {
            let d = self.get_dictionary(id).ok()?;
            let k = self.dereference(d.get(b"Kids").ok()?).ok()?.1;
            k.as_array().ok().map(Vec::as_slice)
        };
        let Some(root) = self
            .catalog()
            .and_then(|c| c.get(b"Pages"))
            .and_then(Object::as_reference)
            .ok()
        else {
            return out;
        };
        let mut limit = l.xref.entries.len();
        let mut stack: Vec<&[Object]> = Vec::new();
        let mut cur = kids(root);
        loop {
            while let Some((kid, rest)) = cur.and_then(|k| k.split_first()) {
                if limit == 0 {
                    return out;
                }
                limit -= 1;
                cur = Some(rest);
                let Ok(id) = kid.as_reference() else { continue };
                match self.get_dictionary(id).and_then(Dictionary::get_type) {
                    Ok(b"Page") => {
                        out.insert(out.len() as u32 + 1, id);
                    }
                    Ok(b"Pages") if stack.len() < 256 => {
                        if !rest.is_empty() {
                            stack.push(rest);
                        }
                        cur = kids(id);
                    }
                    _ => {}
                }
            }
            match stack.pop() {
                Some(k) => cur = Some(k),
                None => return out,
            }
        }
    }
}

impl Lazy {
    fn buf(&self) -> &[u8] {
        self.bytes.get(self.base..).unwrap_or_default()
    }

    fn open(bytes: &Arc<Vec<u8>>, base: usize) -> Option<Self> {
        let buf = bytes.get(base..)?;
        let version = header_version(buf)?;
        let start = startxref(buf)?;
        let mut me = Self {
            bytes: bytes.clone(),
            base,
            version,
            trailer: Dictionary::new(),
            xref: Xref::new(0, XrefType::CrossReferenceTable),
            slots: Vec::new(),
            containers: Mutex::new(HashMap::new()),
            headers: OnceLock::new(),
        };
        let (xref, mut trailer) = me.section(start)?;
        let mut merged = xref;
        // Hybrid file: the table's own entries win over its cross-reference stream (§7.5.8.4).
        let mut pending: Vec<i64> = trailer
            .get(b"XRefStm")
            .and_then(Object::as_i64)
            .into_iter()
            .collect();
        let mut prev = trailer.remove(b"Prev").and_then(|o| o.as_i64().ok());
        let mut seen = HashSet::new();
        loop {
            for at in pending.drain(..) {
                let at = usize::try_from(at).ok()?;
                if seen.insert(at) {
                    let (x, _) = me.section(at)?;
                    merged.merge(x);
                }
            }
            let Some(p) = prev.take() else { break };
            let at = usize::try_from(p).ok()?;
            if !seen.insert(at) {
                break;
            }
            let (x, t) = me.section(at)?;
            merged.merge(x);
            pending.extend(t.get(b"XRefStm").and_then(Object::as_i64));
            prev = t.get(b"Prev").and_then(Object::as_i64).ok();
        }
        if trailer.has(b"Encrypt") || !trailer.has(b"Root") {
            return None;
        }
        merged.size = merged.max_id().checked_add(1)?;
        me.slots = (0..merged.size).map(|_| OnceLock::new()).collect();
        me.xref = merged;
        me.trailer = trailer;
        Some(me)
    }

    /// One cross-reference section at `at`: a table with its trailer, or a stream.
    fn section(&self, at: usize) -> Option<(Xref, Dictionary)> {
        let buf = self.buf();
        let mut lx = Lexer::at(buf, at);
        lx.skip_white();
        if buf.get(lx.pos()..)?.starts_with(b"xref") {
            lx.seek(lx.pos() + 4);
            return table(&mut lx);
        }
        let (_, obj) = self.parse_at(at, 0)?;
        match obj {
            Object::Stream(s) => lopdf::xref::decode_xref_stream(s).ok(),
            _ => None,
        }
    }

    /// Object `id` (not followed when it is a reference).
    fn get(&self, id: ObjectId, chain: u32) -> Option<&Object> {
        let slot = self.slots.get(id.0 as usize)?;
        if slot.get().is_none() {
            if chain > MAX_CHAIN {
                return None;
            }
            let v = self.load(id.0, chain);
            let _ = slot.set(v);
        }
        match slot.get()? {
            Some((i, o)) if *i == id => Some(o),
            _ => None,
        }
    }

    fn load(&self, n: u32, chain: u32) -> Option<(ObjectId, Object)> {
        match *self.xref.get(n)? {
            XrefEntry::Normal { offset, generation } => {
                let id = (n, generation);
                self.parse_at(offset as usize, chain)
                    .filter(|(i, _)| *i == id)
                    .or_else(|| {
                        let at = *self.headers().get(&id)?;
                        self.parse_at(at, chain)
                    })
            }
            XrefEntry::Compressed { container, .. } => {
                let objs = self.container(container, chain)?;
                let id = (n, 0);
                objs.get(&id).map(|o| (id, o.clone()))
            }
            XrefEntry::Free | XrefEntry::UnusableFree => None,
        }
    }

    /// The objects of object stream `n` (§7.5.7).
    fn container(&self, n: u32, chain: u32) -> Option<Arc<BTreeMap<ObjectId, Object>>> {
        if let Some(m) = self.containers.lock().ok()?.get(&n) {
            return Some(m.clone());
        }
        let Object::Stream(s) = self.get((n, 0), chain + 1)? else {
            return None;
        };
        if !s.dict.has_type(b"ObjStm") {
            return None;
        }
        let mut s = s.clone();
        let objs = Arc::new(ObjectStream::new(&mut s).ok()?.objects);
        let mut m = self.containers.lock().ok()?;
        Some(m.entry(n).or_insert(objs).clone())
    }

    /// Every `n g obj` header in the file.
    fn headers(&self) -> &HashMap<ObjectId, usize> {
        self.headers.get_or_init(|| {
            let buf = self.buf();
            let mut out = HashMap::new();
            let mut i = 0;
            while let Some(p) = find(buf, b"obj", i) {
                i = p + 3;
                if let Some((id, start)) = header_before(buf, p) {
                    out.insert(id, start);
                }
            }
            out
        })
    }

    /// The indirect object whose header starts at `at` (§7.3.10).
    fn parse_at(&self, at: usize, chain: u32) -> Option<(ObjectId, Object)> {
        let buf = self.buf();
        let mut lx = Lexer::at(buf, at);
        let Some(Tok::Obj(Object::Integer(n))) = lx.token() else {
            return None;
        };
        let Some(Tok::Obj(Object::Integer(g))) = lx.token() else {
            return None;
        };
        if !matches!(lx.token(), Some(Tok::Op(w)) if w == b"obj") {
            return None;
        }
        let id = (u32::try_from(n).ok()?, u16::try_from(g).ok()?);
        let first = lx.token()?;
        let obj = value(&mut lx, first, 0)?;
        let Object::Dictionary(dict) = obj else {
            return Some((id, obj));
        };
        // A stream follows its dictionary (§7.3.8).
        let after = lx.pos();
        match lx.token() {
            Some(Tok::Op(w)) if w == b"stream" => {}
            _ => {
                lx.seek(after);
                return Some((id, Object::Dictionary(dict)));
            }
        }
        let mut start = lx.pos();
        while matches!(buf.get(start), Some(b' ' | b'\t')) {
            start += 1;
        }
        if buf.get(start..start + 2) == Some(b"\r\n") {
            start += 2;
        } else if matches!(buf.get(start), Some(b'\n' | b'\r')) {
            start += 1;
        }
        let length = match dict.get(b"Length") {
            Ok(Object::Reference(r)) => self.get(*r, chain + 1).and_then(|o| o.as_i64().ok()),
            Ok(o) => o.as_i64().ok(),
            Err(_) => None,
        };
        let ends_at = |end: usize| {
            let mut e = end;
            while buf.get(e).is_some_and(u8::is_ascii_whitespace) {
                e += 1;
            }
            buf.get(e..).is_some_and(|r| r.starts_with(b"endstream"))
        };
        let end = length
            .and_then(|l| usize::try_from(l).ok())
            .and_then(|l| start.checked_add(l))
            .filter(|&e| e <= buf.len() && ends_at(e))
            .or_else(|| {
                // Wrong /Length: the data ends at `endstream`, less its end-of-line marker.
                let mut e = find(buf, b"endstream", start)?;
                if buf.get(e.wrapping_sub(1)) == Some(&b'\n') {
                    e -= 1;
                }
                if e > start && buf.get(e - 1) == Some(&b'\r') {
                    e -= 1;
                }
                Some(e.max(start))
            })?;
        let data = buf.get(start..end)?.to_vec();
        Some((id, Object::Stream(Stream::new(dict, data))))
    }

    /// Every object, for a file `lopdf` cannot load itself.
    fn everything(&self) -> lopdf::Document {
        let mut d = lopdf::Document::new();
        d.version.clone_from(&self.version);
        d.trailer = self.trailer.clone();
        d.reference_table = self.xref.clone();
        d.max_id = self.xref.size.saturating_sub(1);
        for &n in self.xref.entries.keys() {
            if let Some(Some((id, o))) = self
                .slots
                .get(n as usize)
                .map(|s| s.get().cloned().unwrap_or_else(|| self.load(n, 0)))
            {
                d.objects.insert(id, o);
            }
        }
        d
    }
}

/// An array / dictionary / reference-aware value starting with token `first` (§7.3).
fn value(lx: &mut Lexer, first: Tok, depth: u32) -> Option<Object> {
    if depth > MAX_DEPTH {
        return None;
    }
    match first {
        Tok::Obj(Object::Integer(n)) if n >= 0 => {
            // `n g R` is a reference (§7.3.10).
            let back = lx.pos();
            if let Some(Tok::Obj(Object::Integer(g))) = lx.token() {
                if let Some(Tok::Op(w)) = lx.token() {
                    if w == b"R" {
                        if let (Ok(n), Ok(g)) = (u32::try_from(n), u16::try_from(g)) {
                            return Some(Object::Reference((n, g)));
                        }
                    }
                }
            }
            lx.seek(back);
            Some(Object::Integer(n))
        }
        Tok::Obj(o) => Some(o),
        Tok::ArrayStart => {
            let mut items = Vec::new();
            loop {
                match lx.token()? {
                    Tok::ArrayEnd => break,
                    Tok::DictEnd | Tok::Op(_) => {}
                    t => items.extend(value(lx, t, depth + 1)),
                }
            }
            Some(Object::Array(items))
        }
        Tok::DictStart => {
            let mut d = Dictionary::new();
            let mut key: Option<Vec<u8>> = None;
            loop {
                match lx.token()? {
                    Tok::DictEnd => break,
                    Tok::ArrayEnd | Tok::Op(_) => {}
                    t => {
                        let Some(o) = value(lx, t, depth + 1) else {
                            continue;
                        };
                        match key.take() {
                            None => {
                                if let Object::Name(n) = o {
                                    key = Some(n);
                                }
                            }
                            Some(k) => d.set(k, o),
                        }
                    }
                }
            }
            Some(Object::Dictionary(d))
        }
        Tok::ArrayEnd | Tok::DictEnd | Tok::Op(_) => None,
    }
}

/// A cross-reference table after its `xref` keyword, then its trailer (§7.5.4, §7.5.5).
/// Free entries are not recorded (as lopdf), so they never hide an older entry.
fn table(lx: &mut Lexer) -> Option<(Xref, Dictionary)> {
    let mut x = Xref::new(0, XrefType::CrossReferenceTable);
    loop {
        match lx.token()? {
            Tok::Op(w) if w == b"trailer" => break,
            Tok::Obj(Object::Integer(start)) => {
                let Some(Tok::Obj(Object::Integer(count))) = lx.token() else {
                    return None;
                };
                let start = u32::try_from(start).ok()?;
                for i in 0..u32::try_from(count).ok()? {
                    let Some(Tok::Obj(Object::Integer(off))) = lx.token() else {
                        return None;
                    };
                    let Some(Tok::Obj(Object::Integer(gen))) = lx.token() else {
                        return None;
                    };
                    match lx.token()? {
                        Tok::Op(k) if k == b"n" => {
                            if let (Ok(offset), Ok(generation)) =
                                (u32::try_from(off), u16::try_from(gen))
                            {
                                x.insert(
                                    start.checked_add(i)?,
                                    XrefEntry::Normal { offset, generation },
                                );
                            }
                        }
                        Tok::Op(k) if k == b"f" => {}
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
    let first = lx.token()?;
    let Object::Dictionary(t) = value(lx, first, 0)? else {
        return None;
    };
    x.size = u32::try_from(t.get(b"Size").and_then(Object::as_i64).ok()?).ok()?;
    Some((x, t))
}

/// `1.7` from a `%PDF-1.7` header.
fn header_version(buf: &[u8]) -> Option<String> {
    let rest = buf.strip_prefix(b"%PDF-")?;
    let v: Vec<u8> = rest
        .iter()
        .take(8)
        .take_while(|c| c.is_ascii_digit() || **c == b'.')
        .copied()
        .collect();
    (!v.is_empty()).then(|| String::from_utf8_lossy(&v).into_owned())
}

/// Offset after `startxref` near the end of the file (§7.5.5).
fn startxref(buf: &[u8]) -> Option<usize> {
    let tail = buf.len().saturating_sub(1024);
    let at = rfind(buf, b"startxref", tail)?;
    let mut lx = Lexer::at(buf, at + 9);
    match lx.token()? {
        Tok::Obj(Object::Integer(v)) => usize::try_from(v).ok().filter(|&v| v < buf.len()),
        _ => None,
    }
}

fn find(buf: &[u8], pat: &[u8], from: usize) -> Option<usize> {
    let hay = buf.get(from..)?;
    hay.windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

fn rfind(buf: &[u8], pat: &[u8], from: usize) -> Option<usize> {
    let hay = buf.get(from..)?;
    hay.windows(pat.len())
        .rposition(|w| w == pat)
        .map(|p| p + from)
}

/// The `n g` before an `obj` keyword at `p`, with where `n` starts.
fn header_before(buf: &[u8], p: usize) -> Option<(ObjectId, usize)> {
    if buf.get(p + 3).is_some_and(u8::is_ascii_alphanumeric) {
        return None;
    }
    let digits_back = |mut e: usize| -> Option<(u64, usize)> {
        while e > 0 && buf[e - 1].is_ascii_whitespace() {
            e -= 1;
        }
        let end = e;
        while e > 0 && buf[e - 1].is_ascii_digit() && end - e < 10 {
            e -= 1;
        }
        let s = std::str::from_utf8(buf.get(e..end)?).ok()?;
        Some((s.parse().ok()?, e))
    };
    let (g, gs) = digits_back(p)?;
    let (n, ns) = digits_back(gs)?;
    if ns > 0 && buf[ns - 1].is_ascii_alphanumeric() {
        return None;
    }
    Some(((u32::try_from(n).ok()?, u16::try_from(g).ok()?), ns))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with a classic table, then an incremental update with a cross-reference stream
    /// and an object stream.
    fn sample() -> Vec<u8> {
        let mut f = b"%PDF-1.5\n%\xe2\xe3\n".to_vec();
        let mut offs = [0usize; 6];
        let mut obj = |f: &mut Vec<u8>, n: usize, body: &[u8]| {
            offs[n] = f.len();
            f.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
            f.extend_from_slice(body);
            f.extend_from_slice(b"\nendobj\n");
        };
        obj(&mut f, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
        obj(&mut f, 2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
        obj(
            &mut f,
            3,
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>",
        );
        obj(&mut f, 4, b"<< /Length 5 0 R >>\nstream\nBT ET\nendstream");
        obj(&mut f, 5, b"5");
        let x = f.len();
        f.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
        for o in &offs[1..] {
            f.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        f.extend_from_slice(b"trailer\n<< /Size 6 /Root 1 0 R >>\n");
        f.extend_from_slice(format!("startxref\n{x}\n%%EOF\n").as_bytes());
        // Update: object 3 replaced by one inside object stream 6, a new title in 7.
        let inner =
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Contents 4 0 R >> (Updated)";
        let head = format!("3 0 7 {} ", inner.len() - 9);
        let mut content = head.clone().into_bytes();
        content.extend_from_slice(inner);
        let o6 = f.len();
        f.extend_from_slice(
            format!(
                "6 0 obj\n<< /Type /ObjStm /N 2 /First {} /Length {} >>\nstream\n",
                head.len(),
                content.len()
            )
            .as_bytes(),
        );
        f.extend_from_slice(&content);
        f.extend_from_slice(b"\nendstream\nendobj\n");
        let o8 = f.len();
        // Cross-reference stream: W [1 4 2], entries for 3, 6, 7, 8.
        let mut rows = Vec::new();
        let mut row = |t: u8, a: u32, b: u16| {
            rows.push(t);
            rows.extend_from_slice(&a.to_be_bytes());
            rows.extend_from_slice(&b.to_be_bytes());
        };
        row(2, 6, 0);
        row(1, o6 as u32, 0);
        row(2, 6, 1);
        row(1, o8 as u32, 0);
        f.extend_from_slice(
            format!(
                "8 0 obj\n<< /Type /XRef /Size 9 /W [1 4 2] /Index [3 1 6 3] /Root 1 0 R /Info 9 0 R /Prev {x} /Length {} >>\nstream\n",
                rows.len()
            )
            .as_bytes(),
        );
        f.extend_from_slice(&rows);
        f.extend_from_slice(b"\nendstream\nendobj\n");
        f.extend_from_slice(format!("startxref\n{o8}\n%%EOF\n").as_bytes());
        f
    }

    #[test]
    fn reads_tables_streams_and_object_streams_on_demand() {
        let bytes = Arc::new(sample());
        let doc = Document::open_lazy(&bytes, 0).expect("lazy");
        assert_eq!(doc.pdf_version(), "1.5");
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 1);
        let page = doc.get_dictionary(pages[&1]).expect("page");
        // The newest page (from the object stream) wins over the table's.
        let mb = page
            .get(b"MediaBox")
            .and_then(Object::as_array)
            .expect("box");
        assert_eq!(mb[2].as_i64().ok(), Some(300));
        // Indirect /Length.
        let c = doc
            .get_object((4, 0))
            .and_then(Object::as_stream)
            .expect("stream");
        assert_eq!(c.content, b"BT ET");
        assert_eq!(
            doc.get_object((7, 0)).and_then(Object::as_str).ok(),
            Some(&b"Updated"[..])
        );
        assert!(doc.get_object((9, 0)).is_err());
        assert!(!doc.is_materialized());
        // The whole document matches lopdf's own loader.
        let full = lopdf::Document::load_mem(&bytes).expect("lopdf");
        for (id, o) in &full.objects {
            if *id == (8, 0) || *id == (6, 0) {
                continue;
            }
            assert_eq!(doc.get_object(*id).ok(), Some(o), "{id:?}");
        }
        assert_eq!(doc.objects.len(), full.objects.len());
        assert!(doc.is_materialized());
    }

    #[test]
    fn wrong_offsets_and_lengths_still_read() {
        let mut f = sample();
        // Break the table offset of object 2 and the stream length of object 4 (object 5).
        let five = find(
            &f,
            b"5 0 obj
5",
            0,
        )
        .expect("five")
            + 8;
        f[five] = b'9';
        let x = find(
            &f,
            b"xref
0 6
",
            0,
        )
        .expect("xref")
            + 9
            + 20 * 2;
        f[x..x + 10].copy_from_slice(b"0000000017");
        let doc = Document::open_lazy(&Arc::new(f), 0).expect("lazy");
        assert!(doc.get_dictionary((2, 0)).is_ok());
        let c = doc
            .get_object((4, 0))
            .and_then(Object::as_stream)
            .expect("stream");
        assert_eq!(c.content, b"BT ET");
    }

    #[test]
    fn garbage_does_not_panic() {
        let base = sample();
        for i in (0..base.len()).step_by(7) {
            let mut f = base.clone();
            f[i] = f[i].wrapping_add(0x55);
            f.truncate(base.len() - i % 13);
            if let Some(doc) = Document::open_lazy(&Arc::new(f), 0) {
                let _ = doc.get_pages();
                for n in 0..10 {
                    let _ = doc.get_object((n, 0));
                }
            }
        }
    }

    #[test]
    fn writes_go_to_the_full_document() {
        let doc = Document::open_lazy(&Arc::new(sample()), 0).expect("lazy");
        let mut copy = doc.clone();
        copy.objects.insert((20, 0), Object::Integer(1));
        assert!(copy.is_materialized() && copy.get_object((20, 0)).is_ok());
        assert!(!doc.is_materialized() && doc.get_object((20, 0)).is_err());
    }
}
