//! Ink content parts (CORE-INK-001): W3C InkML 1.0 documents referenced from a
//! `contentPart` element ([MS-ODRAWXML] §1.3.2 "Content Parts and Ink", §2.1.4
//! "Ink Content Part"; ECMA-376 Part 1, §19.3.1.14 p:contentPart).
//!
//! * [`InkDoc`] keeps the part as a DOM, so every element and attribute that is
//!   not modelled (annotations, `traceView`, EMMA recognition results, canvas,
//!   timestamps, …) is written back verbatim; edits only touch the `trace`
//!   elements they change.
//! * [`InkDoc::traces`] resolves each trace against its context (trace format,
//!   channel resolutions) and brush into [`Trace`]s in points, with the
//!   pressure (`F`) and time (`T`) channels when present.
//! * [`InkDoc::from_strokes`] writes new ink (BlueSlide's Draw tab).
//! * [`paint`] / [`stroke_outline`] turn traces into filled outlines whose width
//!   follows the pressure ([`pressure_width`]); a highlighter (`rasterOp`
//!   `maskPen`) is drawn translucent.

use crate::dom;
use crate::geom::Seg;
use bo_model::slide::{XmlElement, XmlNode};
use std::collections::HashMap;
use std::ops::Range;

/// InkML namespace ([MS-ODRAWXML] §2.1.4: the root `ink` MUST be in it).
pub const INKML_NS: &str = "http://www.w3.org/2003/InkML";
/// Content type of an ink part ([MS-ODRAWXML] §2.1.4).
pub const CONTENT_TYPE: &str = "application/inkml+xml";
/// Relationship type from the slide / document / drawing to an ink part
/// ([MS-ODRAWXML] §2.1.4 "Source Relationship").
pub const REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";
/// Opacity of highlighter ink (`rasterOp` `maskPen`) — BlueOffice's own
/// rendering of "colours common to both the pen and the display".
pub const HIGHLIGHTER_OPACITY: f64 = 0.5;

const PT_PER_CM: f64 = 72.0 / 2.54;
/// Resolution of the X/Y channels BlueOffice writes: 1000 per cm (0.01 mm).
const WRITE_RESOLUTION: f64 = 1000.0;
/// Range of the pressure channel BlueOffice writes (`F`, 0..=32767).
const WRITE_F_MAX: f64 = 32767.0;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Pen tip shape (brushProperty `tip`, [MS-ODRAWXML] §2.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tip {
    /// `ellipse` (default).
    #[default]
    Ellipse,
    /// `rectangle`.
    Rectangle,
}

/// InkML §3.3.2 brush with the properties of [MS-ODRAWXML] §2.1.4 (lengths in points).
#[derive(Debug, Clone, PartialEq)]
pub struct Brush {
    /// `color` (`#RRGGBB`).
    pub color: [u8; 3],
    /// `width` (default .053 cm).
    pub width: f64,
    /// `height` (default .001 cm).
    pub height: f64,
    /// `transparency` 0 (opaque) ..= 255.
    pub transparency: u8,
    /// `tip`.
    pub tip: Tip,
    /// `rasterOp` (default `copyPen`).
    pub raster_op: String,
    /// `ignorePressure`.
    pub ignore_pressure: bool,
    /// `fitToCurve` (kept; BlueOffice draws straight segments between samples).
    pub fit_to_curve: bool,
}

impl Default for Brush {
    fn default() -> Self {
        Brush {
            color: [0, 0, 0],
            width: 0.053 * PT_PER_CM,
            height: 0.001 * PT_PER_CM,
            transparency: 0,
            tip: Tip::Ellipse,
            raster_op: "copyPen".into(),
            ignore_pressure: false,
            fit_to_curve: false,
        }
    }
}

impl Brush {
    /// A highlighter: `rasterOp` `maskPen` (pen colour combined with the display).
    pub fn is_highlighter(&self) -> bool {
        self.raster_op == "maskPen"
    }
    /// Opacity of the ink (transparency, halved for a highlighter).
    pub fn opacity(&self) -> f64 {
        let a = 1.0 - f64::from(self.transparency) / 255.0;
        if self.is_highlighter() {
            a * HIGHLIGHTER_OPACITY
        } else {
            a
        }
    }
    /// `RRGGBB`.
    pub fn hex(&self) -> String {
        format!(
            "{:02X}{:02X}{:02X}",
            self.color[0], self.color[1], self.color[2]
        )
    }
}

/// One sample of a trace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// X in points (ink space).
    pub x: f64,
    /// Y in points (ink space, downwards).
    pub y: f64,
    /// Normalised pen pressure 0..=1 (channel `F`).
    pub f: Option<f64>,
    /// Time in ms (channel `T`).
    pub t: Option<f64>,
}

/// A resolved InkML §3.2 trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    /// Brush of the trace.
    pub brush: Brush,
    /// Samples in order.
    pub samples: Vec<Sample>,
}

/// A stroke to write ([`InkDoc::from_strokes`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewStroke {
    /// Points `[x, y]` in points.
    pub points: Vec<[f64; 2]>,
    /// Pressure 0..=1 per point (`None`: no `F` channel).
    pub pressure: Option<Vec<f64>>,
    /// Time in ms per point (`None`: no `T` channel).
    pub times: Option<Vec<f64>>,
    /// `RRGGBB`.
    pub color: String,
    /// Pen width in points.
    pub width: f64,
    /// Highlighter (rectangular tip, `maskPen`).
    pub highlighter: bool,
}

// ---------------------------------------------------------------------------
// Units and channels
// ---------------------------------------------------------------------------

/// Points per unit of an InkML length unit (InkML §6.4 units); `None` if unknown.
fn unit_pt(u: &str) -> Option<f64> {
    Some(match u {
        "m" => 100.0 * PT_PER_CM,
        "cm" => PT_PER_CM,
        "mm" => PT_PER_CM / 10.0,
        "in" => 72.0,
        "pt" => 1.0,
        "pc" => 12.0,
        "px" => 0.75,
        "himetric" => PT_PER_CM / 1000.0,
        _ => return None,
    })
}

/// Lengths in a brushProperty (default unit cm, [MS-ODRAWXML] §2.1.4 defaults are in cm).
fn length_pt(value: f64, units: Option<&str>) -> f64 {
    value * units.and_then(unit_pt).unwrap_or(PT_PER_CM)
}

/// InkML §3.1.2 channel `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Decimal,
    Integer,
    Boolean,
}

/// A regular channel of a trace format with the factor from raw values to
/// points (X, Y), milliseconds (T) or the raw range (F).
#[derive(Debug, Clone, PartialEq)]
struct Channel {
    name: String,
    kind: Kind,
    min: Option<f64>,
    max: Option<f64>,
    scale: f64,
}

/// Brush and channels in effect for a trace (InkML §4 context).
#[derive(Debug, Clone, Default)]
struct Scope {
    channels: Option<Vec<Channel>>,
    brush: Option<Brush>,
}

fn strip_ref(r: &str) -> &str {
    r.strip_prefix('#').unwrap_or(r)
}

fn xml_id(e: &XmlElement) -> Option<&str> {
    e.attr("xml:id").or_else(|| e.attr("id"))
}

fn num(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

/// Elements with an `xml:id`, anywhere in the document.
fn index_ids<'a>(e: &'a XmlElement, out: &mut HashMap<String, &'a XmlElement>) {
    for c in e.elements() {
        if let Some(id) = xml_id(c) {
            out.entry(id.to_owned()).or_insert(c);
        }
        index_ids(c, out);
    }
}

struct Defs<'a> {
    ids: HashMap<String, &'a XmlElement>,
}

impl<'a> Defs<'a> {
    fn get(&self, r: &str, local: &str) -> Option<&'a XmlElement> {
        self.ids
            .get(strip_ref(r))
            .copied()
            .filter(|e| e.local() == local)
    }

    /// InkML §3.3.2 brush (+ `brushRef` parent, depth-limited).
    fn brush(&self, e: &XmlElement, depth: u8) -> Brush {
        let mut b = match e.attr("brushRef").and_then(|r| self.get(r, "brush")) {
            Some(p) if depth < 8 => self.brush(p, depth + 1),
            _ => Brush::default(),
        };
        for p in e.elements().filter(|c| c.local() == "brushProperty") {
            let name = p.attr("name").unwrap_or_default();
            let value = p.attr("value").unwrap_or_default().trim();
            let units = p.attr("units");
            // [MS-ODRAWXML] §2.1.4 brushProperty names and value constraints.
            match name {
                "width" => {
                    if let Some(v) = num(Some(value)).filter(|v| *v >= 0.0) {
                        b.width = length_pt(v, units);
                    }
                }
                "height" => {
                    if let Some(v) = num(Some(value)).filter(|v| *v >= 0.0) {
                        b.height = length_pt(v, units);
                    }
                }
                "color" => {
                    let hex = value.trim_start_matches('#');
                    if hex.len() == 6 {
                        if let Ok(v) = u32::from_str_radix(hex, 16) {
                            b.color = [(v >> 16) as u8, (v >> 8) as u8, v as u8];
                        }
                    }
                }
                "transparency" => {
                    if let Ok(v) = value.parse::<i64>() {
                        b.transparency = v.clamp(0, 255) as u8;
                    }
                }
                "tip" => {
                    b.tip = if value == "rectangle" {
                        Tip::Rectangle
                    } else {
                        Tip::Ellipse
                    };
                }
                "rasterOp" => value.clone_into(&mut b.raster_op),
                "ignorePressure" => b.ignore_pressure = dom::parse_bool(value).unwrap_or(false),
                "fitToCurve" => b.fit_to_curve = dom::parse_bool(value).unwrap_or(false),
                _ => {}
            }
        }
        b
    }

    /// InkML §3.1.1 traceFormat inside `src` (an inkSource or context), with the
    /// §3.1.6 channelProperty `resolution`s of `src`.
    fn channels(tf: &XmlElement, props: Option<&XmlElement>) -> Vec<Channel> {
        let resolution = |name: &str| -> Option<(f64, Option<String>)> {
            let p = props?
                .elements()
                .filter(|c| c.local() == "channelProperty")
                .find(|c| {
                    c.attr("channel") == Some(name) && c.attr("name") == Some("resolution")
                })?;
            let v = num(p.attr("value")).filter(|v| *v > 0.0)?;
            let unit = p
                .attr("units")
                .and_then(|u| u.strip_prefix("1/"))
                .map(str::to_owned);
            Some((v, unit))
        };
        tf.elements()
            .filter(|c| c.local() == "channel")
            .map(|c| {
                let name = c.attr("name").unwrap_or_default().to_owned();
                let kind = match c.attr("type") {
                    Some("integer") => Kind::Integer,
                    Some("boolean") => Kind::Boolean,
                    _ => Kind::Decimal,
                };
                let units = c.attr("units");
                let scale = match name.as_str() {
                    "X" | "Y" => match resolution(&name) {
                        Some((r, unit)) => {
                            unit.as_deref()
                                .or(units)
                                .and_then(unit_pt)
                                .unwrap_or(PT_PER_CM)
                                / r
                        }
                        // No resolution: values in the channel's own unit
                        // (unknown or device units: 0.01 mm, BlueOffice's choice).
                        None => units.and_then(unit_pt).unwrap_or(PT_PER_CM / 1000.0),
                    },
                    // [MS-ODRAWXML] §2.1.4: T values are integer milliseconds.
                    "T" if units == Some("s") => 1000.0,
                    _ => 1.0,
                };
                Channel {
                    name,
                    kind,
                    min: num(c.attr("min")),
                    max: num(c.attr("max")),
                    scale,
                }
            })
            .collect()
    }

    /// Channels of an inkSource (§3.1.4) element.
    fn source_channels(src: &XmlElement) -> Option<Vec<Channel>> {
        let props = src.elements().find(|c| c.local() == "channelProperties");
        let tf = src.elements().find(|c| c.local() == "traceFormat")?;
        Some(Self::channels(tf, props))
    }

    /// InkML §4.1 context: inherits `contextRef`, then refs, then children.
    fn context(&self, e: &XmlElement, depth: u8) -> Scope {
        let mut s = match e.attr("contextRef").and_then(|r| self.get(r, "context")) {
            Some(p) if depth < 8 => self.context(p, depth + 1),
            _ => Scope::default(),
        };
        if let Some(src) = e
            .attr("inkSourceRef")
            .and_then(|r| self.get(r, "inkSource"))
        {
            if let Some(ch) = Self::source_channels(src) {
                s.channels = Some(ch);
            }
        }
        if let Some(tf) = e
            .attr("traceFormatRef")
            .and_then(|r| self.get(r, "traceFormat"))
        {
            s.channels = Some(Self::channels(tf, None));
        }
        if let Some(b) = e.attr("brushRef").and_then(|r| self.get(r, "brush")) {
            s.brush = Some(self.brush(b, 0));
        }
        for c in e.elements() {
            match c.local() {
                "inkSource" => {
                    if let Some(ch) = Self::source_channels(c) {
                        s.channels = Some(ch);
                    }
                }
                "traceFormat" => s.channels = Some(Self::channels(c, None)),
                "brush" => s.brush = Some(self.brush(c, 0)),
                _ => {}
            }
        }
        s
    }

    /// Scope of an element carrying `contextRef` / `brushRef` over `outer`.
    fn scoped(&self, e: &XmlElement, outer: &Scope) -> Scope {
        let mut s = outer.clone();
        if let Some(c) = e.attr("contextRef").and_then(|r| self.get(r, "context")) {
            let ctx = self.context(c, 0);
            if ctx.channels.is_some() {
                s.channels = ctx.channels;
            }
            if ctx.brush.is_some() {
                s.brush = ctx.brush;
            }
        }
        if let Some(b) = e.attr("brushRef").and_then(|r| self.get(r, "brush")) {
            s.brush = Some(self.brush(b, 0));
        }
        s
    }
}

/// InkML §3.1.1: without a trace format, a trace has decimal X and Y channels.
fn default_channels() -> Vec<Channel> {
    ["X", "Y"]
        .iter()
        .map(|n| Channel {
            name: (*n).into(),
            kind: Kind::Decimal,
            min: None,
            max: None,
            scale: PT_PER_CM / 1000.0,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Trace text (InkML §3.2.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Num(f64),
    /// `*` / `?`: the previous value is kept.
    Keep,
}

/// Values of one point: (difference qualifier, value).
fn tokenize_point(s: &str) -> Vec<(Option<char>, Tok)> {
    let mut out = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut qual = None;
    while i < b.len() {
        let c = b[i];
        match c {
            '!' | '\'' | '"' => {
                qual = Some(c);
                i += 1;
            }
            'T' | 'F' => {
                out.push((qual.take(), Tok::Num(if c == 'T' { 1.0 } else { 0.0 })));
                i += 1;
            }
            '*' | '?' => {
                out.push((qual.take(), Tok::Keep));
                i += 1;
            }
            '-' | '+' | '.' | '0'..='9' => {
                // number ::= sign? (digits ('.' digits?)? | '.' digits) exponent?
                // A sign or a second '.' starts the next number ("1-2", "1.5.5").
                let start = i;
                if matches!(c, '-' | '+') {
                    i += 1;
                }
                let mut dot = false;
                while i < b.len() {
                    match b[i] {
                        '0'..='9' => i += 1,
                        '.' if !dot => {
                            dot = true;
                            i += 1;
                        }
                        'e' | 'E'
                            if i + 1 < b.len()
                                && (b[i + 1].is_ascii_digit()
                                    || (matches!(b[i + 1], '-' | '+')
                                        && b.get(i + 2).is_some_and(char::is_ascii_digit))) =>
                        {
                            i += 2;
                            while i < b.len() && b[i].is_ascii_digit() {
                                i += 1;
                            }
                            break;
                        }
                        _ => break,
                    }
                }
                let text: String = b[start..i].iter().collect();
                match text.parse::<f64>() {
                    Ok(v) if v.is_finite() => out.push((qual.take(), Tok::Num(v))),
                    _ => {
                        if i == start {
                            i += 1;
                        }
                    }
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Decode trace content into raw channel values per point (`n` regular channels).
/// InkML §3.2.1: `!` explicit value, `'` first difference, `"` second
/// difference; a qualifier stays in effect for its channel until another one.
fn decode(text: &str, n: usize) -> Vec<Vec<f64>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Explicit,
        First,
        Second,
    }
    let mut mode = vec![Mode::Explicit; n];
    let mut last = vec![0.0f64; n];
    let mut vel = vec![0.0f64; n];
    let mut out = Vec::new();
    for p in text.split(',') {
        if p.trim().is_empty() {
            continue;
        }
        let toks = tokenize_point(p);
        if toks.is_empty() {
            continue;
        }
        let mut pt = Vec::with_capacity(n);
        for ch in 0..n {
            let (q, tok) = toks.get(ch).copied().unwrap_or((None, Tok::Keep));
            match q {
                Some('!') => mode[ch] = Mode::Explicit,
                Some('\'') => mode[ch] = Mode::First,
                Some('"') => mode[ch] = Mode::Second,
                _ => {}
            }
            let v = match tok {
                Tok::Keep => last[ch],
                Tok::Num(x) => match mode[ch] {
                    Mode::Explicit => x,
                    Mode::First => last[ch] + x,
                    Mode::Second => last[ch] + vel[ch] + x,
                },
            };
            vel[ch] = v - last[ch];
            last[ch] = v;
            pt.push(v);
        }
        out.push(pt);
    }
    out
}

/// Shortest decimal text of a value.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s == "-0" {
            "0".into()
        } else {
            s.to_owned()
        }
    }
}

/// Trace content with explicit values (InkML §3.2.1).
fn encode(raw: &[Vec<f64>], channels: &[Channel]) -> String {
    raw.iter()
        .map(|p| {
            p.iter()
                .zip(channels)
                .map(|(v, c)| match c.kind {
                    Kind::Boolean => (if *v == 0.0 { "F" } else { "T" }).to_owned(),
                    Kind::Integer => fmt_num(v.round()),
                    Kind::Decimal => fmt_num(*v),
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// An ink part (InkML 1.0 `ink` root), kept as a DOM for lossless writing.
#[derive(Debug, Clone, PartialEq)]
pub struct InkDoc {
    /// The `ink` element.
    pub root: XmlElement,
}

/// A trace element found in the document with what it resolved to.
struct Found {
    path: Vec<usize>,
    channels: Vec<Channel>,
    raw: Vec<Vec<f64>>,
    brush: Brush,
}

fn element_at<'a>(root: &'a XmlElement, path: &[usize]) -> Option<&'a XmlElement> {
    let mut e = root;
    for &i in path {
        match e.children.get(i) {
            Some(XmlNode::Element(c)) => e = c,
            _ => return None,
        }
    }
    Some(e)
}

fn element_at_mut<'a>(root: &'a mut XmlElement, path: &[usize]) -> Option<&'a mut XmlElement> {
    let mut e = root;
    for &i in path {
        match e.children.get_mut(i) {
            Some(XmlNode::Element(c)) => e = c,
            _ => return None,
        }
    }
    Some(e)
}

impl InkDoc {
    /// Parse an ink part; `None` when it is not an InkML `ink` document.
    pub fn parse(bytes: &[u8]) -> Option<InkDoc> {
        let root = dom::parse(bytes, "/ink").ok()?;
        (root.local() == "ink").then_some(InkDoc { root })
    }

    /// Serialized part.
    pub fn to_bytes(&self) -> Vec<u8> {
        dom::to_part_xml(&self.root)
    }

    /// Every `trace` that draws (direct children of `ink` or of `traceGroup`s,
    /// [MS-ODRAWXML] §2.1.4), in document order.
    fn found(&self) -> Vec<Found> {
        let mut ids = HashMap::new();
        index_ids(&self.root, &mut ids);
        let defs = Defs { ids };
        let mut out = Vec::new();
        let mut current = Scope::default();
        walk_traces(
            &defs,
            &self.root,
            &mut Vec::new(),
            &mut current,
            true,
            &mut out,
        );
        out
    }

    /// Resolved traces in document order.
    pub fn traces(&self) -> Vec<Trace> {
        self.found()
            .into_iter()
            .map(|f| Trace {
                samples: samples_of(&f.channels, &f.raw),
                brush: f.brush,
            })
            .collect()
    }

    /// Remove traces (indices of [`InkDoc::traces`]).
    pub fn remove_traces(&mut self, which: &[usize]) {
        let mut paths: Vec<Vec<usize>> = self
            .found()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| which.contains(i))
            .map(|(_, f)| f.path)
            .collect();
        // Later siblings first, so earlier paths stay valid.
        paths.sort();
        for p in paths.iter().rev() {
            let Some((&last, parent)) = p.split_last() else {
                continue;
            };
            if let Some(par) = element_at_mut(&mut self.root, parent) {
                if last < par.children.len() {
                    par.children.remove(last);
                }
            }
        }
    }

    /// Keep only the sample ranges `runs` of trace `index`: each run becomes a
    /// trace of its own with the same attributes (point eraser). Values are
    /// written back exactly as read (explicit form).
    pub fn split_trace(&mut self, index: usize, runs: &[Range<usize>]) {
        let Some(f) = self.found().into_iter().nth(index) else {
            return;
        };
        let Some((&last, parent)) = f.path.split_last() else {
            return;
        };
        let Some(orig) = element_at(&self.root, &f.path).cloned() else {
            return;
        };
        let mut news = Vec::new();
        for (k, r) in runs.iter().enumerate() {
            let Some(raw) = f.raw.get(r.clone()) else {
                continue;
            };
            if raw.is_empty() {
                continue;
            }
            let mut e = orig.clone();
            if k > 0 {
                for (key, v) in &mut e.attrs {
                    if key == "xml:id" || key == "id" {
                        *v = format!("{v}-{k}");
                    }
                }
            }
            // InkML §3.2: continuation / priorRef / duration describe the whole trace.
            e.attrs.retain(|(key, _)| {
                !matches!(key.as_str(), "continuation" | "priorRef" | "duration")
            });
            e.children = vec![XmlNode::Text(encode(raw, &f.channels))];
            news.push(XmlNode::Element(e));
        }
        if let Some(par) = element_at_mut(&mut self.root, parent) {
            if last < par.children.len() {
                par.children.splice(last..=last, news);
            }
        }
    }

    /// A new ink document holding `strokes` (X/Y in 1/1000 cm, `F` 0..=32767,
    /// `T` in ms, one context per channel set, one brush per distinct look).
    pub fn from_strokes(strokes: &[NewStroke]) -> InkDoc {
        let p = "inkml:";
        let mut defs = XmlElement::new(&format!("{p}definitions"));
        let mut contexts: Vec<(bool, bool)> = Vec::new();
        let mut brushes: Vec<(String, i64, bool)> = Vec::new();
        let mut traces = Vec::new();
        for s in strokes {
            let pressure = s
                .pressure
                .as_ref()
                .filter(|v| v.len() == s.points.len() && !v.is_empty());
            let times = s
                .times
                .as_ref()
                .filter(|v| v.len() == s.points.len() && !v.is_empty());
            let key = (pressure.is_some(), times.is_some());
            let ci = contexts.iter().position(|c| *c == key).unwrap_or_else(|| {
                contexts.push(key);
                contexts.len() - 1
            });
            let color = s.color.trim_start_matches('#').to_ascii_uppercase();
            let color = if color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit()) {
                color
            } else {
                "000000".into()
            };
            // Width kept to 1/1000 pt for the brush key.
            let bkey = (
                color,
                (s.width.clamp(0.1, 200.0) * 1000.0).round() as i64,
                s.highlighter,
            );
            let bi = brushes.iter().position(|b| *b == bkey).unwrap_or_else(|| {
                brushes.push(bkey);
                brushes.len() - 1
            });
            let mut text = Vec::new();
            for (i, pt) in s.points.iter().enumerate() {
                let k = WRITE_RESOLUTION / PT_PER_CM;
                let mut vals = vec![fmt_num((pt[0] * k).round()), fmt_num((pt[1] * k).round())];
                if let Some(f) = pressure {
                    let v = f.get(i).copied().unwrap_or(0.5).clamp(0.0, 1.0);
                    vals.push(fmt_num((v * WRITE_F_MAX).round()));
                }
                if let Some(t) = times {
                    vals.push(fmt_num(t.get(i).copied().unwrap_or(0.0).max(0.0).round()));
                }
                text.push(vals.join(" "));
            }
            let mut tr = dom::El::new(&format!("{p}trace"))
                .a("contextRef", format!("#ctx{ci}"))
                .a("brushRef", format!("#br{bi}"))
                .done();
            tr.children.push(XmlNode::Text(text.join(", ")));
            traces.push(tr);
        }
        for (i, (f, t)) in contexts.iter().enumerate() {
            // InkML §3.1.1 traceFormat / §3.1.2 channel; [MS-ODRAWXML] §2.1.4: the
            // trace format of an inkSource (xml:id required), resolution properties.
            let ch = |name: &str, units: &str, max: Option<f64>| {
                dom::El::new(&format!("{p}channel"))
                    .a("name", name)
                    .a("type", "integer")
                    .opt("max", max.map(fmt_num))
                    .a("units", units)
                    .done()
            };
            let mut tf = dom::El::new(&format!("{p}traceFormat"))
                .child(ch("X", "cm", None))
                .child(ch("Y", "cm", None));
            if *f {
                tf = tf.child(ch("F", "dev", Some(WRITE_F_MAX)));
            }
            if *t {
                tf = tf.child(ch("T", "dev", None));
            }
            let prop = |c: &str| {
                dom::El::new(&format!("{p}channelProperty"))
                    .a("channel", c)
                    .a("name", "resolution")
                    .a("value", fmt_num(WRITE_RESOLUTION))
                    .a("units", "1/cm")
                    .done()
            };
            let props = dom::El::new(&format!("{p}channelProperties"))
                .child(prop("X"))
                .child(prop("Y"))
                .done();
            let src = dom::El::new(&format!("{p}inkSource"))
                .a("xml:id", format!("inkSrc{i}"))
                .child(tf.done())
                .child(props)
                .done();
            defs.children.push(XmlNode::Element(
                dom::El::new(&format!("{p}context"))
                    .a("xml:id", format!("ctx{i}"))
                    .child(src)
                    .done(),
            ));
        }
        for (i, (color, w, hl)) in brushes.iter().enumerate() {
            let cm = |pt: f64| fmt_num((pt / PT_PER_CM * 100_000.0).round() / 100_000.0);
            let w = *w as f64 / 1000.0;
            let bp = |name: &str, value: String, units: Option<&str>| {
                dom::El::new(&format!("{p}brushProperty"))
                    .a("name", name)
                    .a("value", value)
                    .opt("units", units)
                    .done()
            };
            // InkML §3.3.2 brush, [MS-ODRAWXML] §2.1.4 brushProperty names.
            let mut b = dom::El::new(&format!("{p}brush"))
                .a("xml:id", format!("br{i}"))
                .child(bp("width", cm(w), Some("cm")))
                .child(bp("height", cm(if *hl { w * 2.0 } else { w }), Some("cm")))
                .child(bp("color", format!("#{color}"), None));
            if *hl {
                b = b.child(bp("tip", "rectangle".into(), None)).child(bp(
                    "rasterOp",
                    "maskPen".into(),
                    None,
                ));
            }
            defs.children.push(XmlNode::Element(b.done()));
        }
        let mut root = dom::El::new(&format!("{p}ink"))
            .a("xmlns:inkml", INKML_NS)
            .done();
        root.children.push(XmlNode::Element(defs));
        root.children
            .extend(traces.into_iter().map(XmlNode::Element));
        InkDoc { root }
    }
}

fn walk_traces(
    defs: &Defs<'_>,
    e: &XmlElement,
    path: &mut Vec<usize>,
    scope: &mut Scope,
    root: bool,
    out: &mut Vec<Found>,
) {
    for (i, c) in e.children.iter().enumerate() {
        let XmlNode::Element(c) = c else { continue };
        path.push(i);
        match c.local() {
            // InkML §4.1: a context element in the stream changes the current context.
            "context" if root => {
                let ctx = defs.context(c, 0);
                if ctx.channels.is_some() {
                    scope.channels = ctx.channels;
                }
                if ctx.brush.is_some() {
                    scope.brush = ctx.brush;
                }
            }
            "trace" => {
                let s = defs.scoped(c, scope);
                let channels = s.channels.unwrap_or_else(default_channels);
                let raw = decode(&c.text(), channels.len());
                out.push(Found {
                    path: path.clone(),
                    channels,
                    raw,
                    brush: s.brush.unwrap_or_default(),
                });
            }
            // InkML §3.3 traceGroup: its contextRef / brushRef apply to its traces.
            "traceGroup" => {
                let mut inner = defs.scoped(c, scope);
                walk_traces(defs, c, path, &mut inner, false, out);
            }
            _ => {}
        }
        path.pop();
    }
}

/// Samples of decoded raw values.
fn samples_of(channels: &[Channel], raw: &[Vec<f64>]) -> Vec<Sample> {
    let idx = |n: &str| channels.iter().position(|c| c.name == n);
    let (xi, yi, fi, ti) = (idx("X"), idx("Y"), idx("F"), idx("T"));
    // Pressure range: the channel's min/max, else the largest value seen.
    let frange = fi.map(|i| {
        let c = &channels[i];
        let lo = c.min.unwrap_or(0.0);
        let hi = c.max.unwrap_or_else(|| {
            let m = raw
                .iter()
                .filter_map(|p| p.get(i).copied())
                .fold(0.0f64, f64::max);
            if m <= 1.0 {
                1.0
            } else {
                m
            }
        });
        (lo, hi)
    });
    raw.iter()
        .map(|p| {
            let g = |i: Option<usize>| i.and_then(|i| p.get(i).map(|v| v * channels[i].scale));
            Sample {
                x: g(xi).unwrap_or(0.0),
                y: g(yi).unwrap_or(0.0),
                f: fi.zip(frange).and_then(|(i, (lo, hi))| {
                    let v = p.get(i)?;
                    (hi > lo).then(|| ((v - lo) / (hi - lo)).clamp(0.0, 1.0))
                }),
                t: g(ti),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Placement and rendering
// ---------------------------------------------------------------------------

/// Pen width at a sample: BlueOffice's own pressure curve, `base · (0.5 + f)`
/// (half width with no pressure, the nominal width at half pressure, 1.5× at
/// full pressure); `base` without a pressure value.
pub fn pressure_width(base: f64, f: Option<f64>) -> f64 {
    match f {
        Some(f) => base * (0.5 + f.clamp(0.0, 1.0)),
        None => base,
    }
}

/// Bounding box `[x0, y0, x1, y1]` of the samples (points), `None` when empty.
pub fn bounds(traces: &[Trace]) -> Option<[f64; 4]> {
    let mut it = traces.iter().flat_map(|t| t.samples.iter());
    let first = it.next()?;
    Some(it.fold([first.x, first.y, first.x, first.y], |b, s| {
        [b[0].min(s.x), b[1].min(s.y), b[2].max(s.x), b[3].max(s.y)]
    }))
}

/// Half the largest brush width / height (the margin around the samples).
pub fn margin(traces: &[Trace]) -> (f64, f64) {
    traces.iter().fold((0.0f64, 0.0f64), |m, t| {
        (m.0.max(t.brush.width / 2.0), m.1.max(t.brush.height / 2.0))
    })
}

/// Maps ink space (points) into a frame: `(p − min + margin) · scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Top-left sample position.
    pub min: (f64, f64),
    /// Margin added before scaling.
    pub margin: (f64, f64),
    /// Scale factors.
    pub scale: (f64, f64),
}

impl Placement {
    /// Unscaled placement and the frame size it needs (samples + margins).
    pub fn natural(traces: &[Trace]) -> Option<(Placement, (f64, f64))> {
        let b = bounds(traces)?;
        let m = margin(traces);
        Some((
            Placement {
                min: (b[0], b[1]),
                margin: m,
                scale: (1.0, 1.0),
            },
            (b[2] - b[0] + 2.0 * m.0, b[3] - b[1] + 2.0 * m.1),
        ))
    }

    /// Fit the ink (samples + margins) into a `w × h` frame — the frame of the
    /// content part (`p14:xfrm`) is the ink's box; a resized frame scales the ink.
    pub fn fit(traces: &[Trace], w: f64, h: f64) -> Option<Placement> {
        let (mut p, (nw, nh)) = Placement::natural(traces)?;
        p.scale = (
            if nw > 1e-9 { w / nw } else { 1.0 },
            if nh > 1e-9 { h / nh } else { 1.0 },
        );
        Some(p)
    }

    /// Frame coordinates of an ink-space point.
    pub fn map(&self, x: f64, y: f64) -> (f64, f64) {
        (
            (x - self.min.0 + self.margin.0) * self.scale.0,
            (y - self.min.1 + self.margin.1) * self.scale.1,
        )
    }
}

/// One stroke ready to fill (nonzero rule).
#[derive(Debug, Clone, PartialEq)]
pub struct InkPaint {
    /// Outline in frame coordinates.
    pub segs: Vec<Seg>,
    /// Ink colour.
    pub color: [u8; 3],
    /// Opacity 0..=1.
    pub opacity: f64,
}

/// A stroke sample in frame coordinates with the tip's half extents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TipAt {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Half width of the tip.
    pub rx: f64,
    /// Half height of the tip.
    pub ry: f64,
}

/// Tips of a trace in frame coordinates.
pub fn tips(t: &Trace, place: &Placement) -> Vec<TipAt> {
    t.samples
        .iter()
        .map(|s| {
            let f = if t.brush.ignore_pressure { None } else { s.f };
            let (x, y) = place.map(s.x, s.y);
            TipAt {
                x,
                y,
                rx: (pressure_width(t.brush.width, f) / 2.0 * place.scale.0).max(0.01),
                ry: (pressure_width(t.brush.height, f) / 2.0 * place.scale.1).max(0.01),
            }
        })
        .collect()
}

/// Outlines of every trace placed by `place`.
pub fn paint(traces: &[Trace], place: &Placement) -> Vec<InkPaint> {
    traces
        .iter()
        .map(|t| InkPaint {
            segs: stroke_outline(&tips(t, place), t.brush.tip),
            color: t.brush.color,
            opacity: t.brush.opacity(),
        })
        .collect()
}

/// Signed area ×2 of a polygon (positive: clockwise on a y-down page).
fn shoelace(p: &[(f64, f64)]) -> f64 {
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

/// Variable-width outline of a stroke: the tip stamped at every sample plus,
/// for each segment, the quadrilateral joining the tips' extents across the
/// segment. Every subpath turns the same way, so a nonzero fill gives their
/// union in one paint (no darker overlaps for translucent ink).
pub fn stroke_outline(pts: &[TipAt], tip: Tip) -> Vec<Seg> {
    // Circle by four cubic Béziers: control distance k = 4/3·(√2 − 1).
    const K: f64 = 0.552_284_749_830_793_6;
    let mut segs = Vec::new();
    let stamp = |segs: &mut Vec<Seg>, p: &TipAt| {
        let (cx, cy, rx, ry) = (p.x, p.y, p.rx, p.ry);
        match tip {
            Tip::Ellipse => {
                segs.push(Seg::Move(cx + rx, cy));
                segs.push(Seg::Cubic(
                    cx + rx,
                    cy + K * ry,
                    cx + K * rx,
                    cy + ry,
                    cx,
                    cy + ry,
                ));
                segs.push(Seg::Cubic(
                    cx - K * rx,
                    cy + ry,
                    cx - rx,
                    cy + K * ry,
                    cx - rx,
                    cy,
                ));
                segs.push(Seg::Cubic(
                    cx - rx,
                    cy - K * ry,
                    cx - K * rx,
                    cy - ry,
                    cx,
                    cy - ry,
                ));
                segs.push(Seg::Cubic(
                    cx + K * rx,
                    cy - ry,
                    cx + rx,
                    cy - K * ry,
                    cx + rx,
                    cy,
                ));
            }
            Tip::Rectangle => {
                segs.push(Seg::Move(cx - rx, cy - ry));
                segs.push(Seg::Line(cx + rx, cy - ry));
                segs.push(Seg::Line(cx + rx, cy + ry));
                segs.push(Seg::Line(cx - rx, cy + ry));
            }
        }
        segs.push(Seg::Close);
    };
    for p in pts {
        stamp(&mut segs, p);
    }
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = dx.hypot(dy);
        if len < 1e-9 {
            continue;
        }
        let (nx, ny) = (-dy / len, dx / len);
        // Half extent of the tip across the segment (support of the tip along n).
        let ext = |p: &TipAt| match tip {
            Tip::Ellipse => (p.rx * nx).hypot(p.ry * ny),
            Tip::Rectangle => (p.rx * nx).abs() + (p.ry * ny).abs(),
        };
        let (ea, eb) = (ext(&a), ext(&b));
        let mut q = vec![
            (a.x + nx * ea, a.y + ny * ea),
            (b.x + nx * eb, b.y + ny * eb),
            (b.x - nx * eb, b.y - ny * eb),
            (a.x - nx * ea, a.y - ny * ea),
        ];
        if shoelace(&q) < 0.0 {
            q.reverse();
        }
        segs.push(Seg::Move(q[0].0, q[0].1));
        for p in &q[1..] {
            segs.push(Seg::Line(p.0, p.1));
        }
        segs.push(Seg::Close);
    }
    segs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-written InkML document: a context with X/Y at 1000/cm, pressure
    /// 0..=1023 and time; a red pen and a yellow highlighter brush; a trace with
    /// first and second differences; a trace group; an EMMA annotation and a
    /// traceView that must survive edits.
    const FIXTURE: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<inkml:ink xmlns:inkml="http://www.w3.org/2003/InkML" xmlns:emma="http://www.w3.org/2003/04/emma">
<inkml:definitions>
<inkml:context xml:id="ctx0">
<inkml:inkSource xml:id="src0">
<inkml:traceFormat>
<inkml:channel name="X" type="integer" units="cm"/>
<inkml:channel name="Y" type="integer" units="cm"/>
<inkml:channel name="F" type="integer" max="1023" units="dev"/>
<inkml:channel name="T" type="integer" units="dev"/>
</inkml:traceFormat>
<inkml:channelProperties>
<inkml:channelProperty channel="X" name="resolution" value="1000" units="1/cm"/>
<inkml:channelProperty channel="Y" name="resolution" value="1000" units="1/cm"/>
</inkml:channelProperties>
</inkml:inkSource>
</inkml:context>
<inkml:brush xml:id="pen">
<inkml:brushProperty name="width" value="0.1" units="cm"/>
<inkml:brushProperty name="height" value="0.1" units="cm"/>
<inkml:brushProperty name="color" value="#FF0000"/>
</inkml:brush>
<inkml:brush xml:id="hl">
<inkml:brushProperty name="width" value="2" units="mm"/>
<inkml:brushProperty name="height" value="4" units="mm"/>
<inkml:brushProperty name="color" value="#FFFF00"/>
<inkml:brushProperty name="tip" value="rectangle"/>
<inkml:brushProperty name="rasterOp" value="maskPen"/>
<inkml:brushProperty name="transparency" value="51"/>
</inkml:brush>
</inkml:definitions>
<inkml:trace xml:id="t1" contextRef="#ctx0" brushRef="#pen">1000 2000 0 0, '254 '0 1023 10, "0 '508 '0 '10, 0 0 0 0</inkml:trace>
<inkml:traceGroup brushRef="#hl" contextRef="#ctx0">
<inkml:annotationXML><emma:emma version="1.0"><emma:interpretation id="{00000000-0000-0000-0000-000000000000}" emma:mode="ink"/></emma:emma></inkml:annotationXML>
<inkml:trace xml:id="t2">0 0 512 100,1000-500 512 110</inkml:trace>
</inkml:traceGroup>
<inkml:traceView traceDataRef="#t1"/>
</inkml:ink>"##;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn parses_channels_brushes_and_differences() {
        let doc = InkDoc::parse(FIXTURE.as_bytes()).unwrap();
        let t = doc.traces();
        assert_eq!(t.len(), 2);
        // 1000 raw = 1 cm = 72/2.54 pt.
        let cm = 72.0 / 2.54;
        let s = &t[0].samples;
        assert_eq!(s.len(), 4);
        // Point 1 explicit; point 2: X +254 and Y +0 as first differences, F 1023 and
        // T 10 explicit (no qualifier on them).
        assert!(close(s[0].x, cm) && close(s[0].y, 2.0 * cm));
        assert!(close(s[1].x, 1.254 * cm) && close(s[1].y, 2.0 * cm));
        assert_eq!(s[1].f, Some(1.0));
        assert_eq!(s[1].t, Some(10.0));
        // Point 3: X second difference 0 → velocity 254 kept: 1508; Y first
        // difference 508: 2508; F first difference 0: 1023; T +10: 20.
        assert!(close(s[2].x, 1.508 * cm) && close(s[2].y, 2.508 * cm));
        assert_eq!(s[2].f, Some(1.0));
        assert_eq!(s[2].t, Some(20.0));
        // Point 4: qualifiers persist: X second difference 0 → 1762; Y +0 → 2508.
        assert!(close(s[3].x, 1.762 * cm) && close(s[3].y, 2.508 * cm));
        let pen = &t[0].brush;
        assert_eq!(pen.color, [255, 0, 0]);
        assert!(close(pen.width, 0.1 * cm) && close(pen.height, 0.1 * cm));
        assert!(!pen.is_highlighter());
        assert!(close(pen.opacity(), 1.0));
        // The group's brush; "1000-500" is two numbers.
        let hl = &t[1].brush;
        assert!(hl.is_highlighter());
        assert_eq!(hl.tip, Tip::Rectangle);
        assert!(close(hl.width, 0.2 * cm) && close(hl.height, 0.4 * cm));
        // Transparency 51/255 = 0.2 → 0.8 opacity, halved for the highlighter.
        assert!(close(hl.opacity(), 0.4));
        let s = &t[1].samples;
        assert!(close(s[1].x, cm) && close(s[1].y, -0.5 * cm));
        // 512 / 1023.
        assert!(close(s[0].f.unwrap(), 512.0 / 1023.0));
    }

    #[test]
    fn edits_keep_unknown_content() {
        let mut doc = InkDoc::parse(FIXTURE.as_bytes()).unwrap();
        doc.split_trace(0, &[0..2, 3..4]);
        let t = doc.traces();
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].samples.len(), 2);
        assert_eq!(t[1].samples.len(), 1);
        // The split trace keeps its values: the last point (1762, 2508).
        let cm = 72.0 / 2.54;
        assert!(close(t[1].samples[0].x, 1.762 * cm));
        doc.remove_traces(&[2]);
        let back = String::from_utf8(doc.to_bytes()).unwrap();
        // Explicit values, xml:id made unique, annotation and traceView kept.
        assert!(back.contains(r##"<inkml:trace xml:id="t1" contextRef="#ctx0" brushRef="#pen">1000 2000 0 0, 1254 2000 1023 10</inkml:trace>"##));
        assert!(back.contains(r#"xml:id="t1-1""#));
        assert!(back.contains("emma:interpretation"));
        assert!(back.contains(r##"<inkml:traceView traceDataRef="#t1"/>"##));
        assert!(!back.contains(r#"xml:id="t2""#));
        // Parsing again gives the same model.
        let again = InkDoc::parse(&doc.to_bytes()).unwrap();
        assert_eq!(again.traces(), doc.traces());
    }

    #[test]
    fn written_strokes_read_back() {
        // 72 pt = 2.54 cm = 2540 raw; pressure 0.5 → 16384 (rounded 16383.5).
        let s = NewStroke {
            points: vec![[72.0, 36.0], [144.0, 36.0]],
            pressure: Some(vec![0.5, 1.0]),
            times: Some(vec![0.0, 16.0]),
            color: "00aa00".into(),
            width: 3.0,
            highlighter: false,
        };
        let h = NewStroke {
            points: vec![[0.0, 0.0], [10.0, 10.0]],
            color: "FFE600".into(),
            width: 12.0,
            highlighter: true,
            ..NewStroke::default()
        };
        let doc = InkDoc::from_strokes(&[s, h]);
        let text = String::from_utf8(doc.to_bytes()).unwrap();
        assert!(text.contains(">2540 1270 16384 0, 5080 1270 32767 16<"));
        let back = InkDoc::parse(text.as_bytes()).unwrap().traces();
        assert_eq!(back.len(), 2);
        let a = &back[0];
        assert!(close(a.samples[0].x, 72.0) && close(a.samples[1].x, 144.0));
        assert!((a.samples[0].f.unwrap() - 16384.0 / 32767.0).abs() < 1e-12);
        assert_eq!(a.samples[1].t, Some(16.0));
        assert_eq!(a.brush.color, [0, 0xAA, 0]);
        // 3 pt = 0.10583 cm (5 decimals) → 2.99991 pt.
        assert!((a.brush.width - 3.0).abs() < 1e-3);
        let b = &back[1];
        assert!(b.brush.is_highlighter() && b.brush.tip == Tip::Rectangle);
        assert_eq!(b.samples[0].f, None);
        assert_eq!(b.samples[0].t, None);
    }

    #[test]
    fn pressure_widths() {
        // base · (0.5 + f): 2 pt at f = 0.25 → 1.5 pt; f = 1 → 3 pt; none → 2 pt.
        assert!(close(pressure_width(2.0, Some(0.25)), 1.5));
        assert!(close(pressure_width(2.0, Some(1.0)), 3.0));
        assert!(close(pressure_width(2.0, Some(7.0)), 3.0));
        assert!(close(pressure_width(2.0, None), 2.0));
    }

    #[test]
    fn segment_outline_by_hand() {
        // Tips at (0,0) r = 1 and (10,0) r = 2 (round): normal n = (0, 1);
        // quad (0,1) (10,2) (10,−2) (0,−1) turns negatively → reversed to
        // (0,−1) (10,−2) (10,2) (0,1).
        let pts = [
            TipAt {
                x: 0.0,
                y: 0.0,
                rx: 1.0,
                ry: 1.0,
            },
            TipAt {
                x: 10.0,
                y: 0.0,
                rx: 2.0,
                ry: 2.0,
            },
        ];
        let segs = stroke_outline(&pts, Tip::Ellipse);
        // Two circles (move + 4 cubics + close) then the quad.
        assert_eq!(segs.len(), 6 + 6 + 5);
        assert_eq!(segs[0], Seg::Move(1.0, 0.0));
        assert_eq!(segs[6], Seg::Move(12.0, 0.0));
        assert_eq!(
            &segs[12..],
            &[
                Seg::Move(0.0, -1.0),
                Seg::Line(10.0, -2.0),
                Seg::Line(10.0, 2.0),
                Seg::Line(0.0, 1.0),
                Seg::Close
            ]
        );
        // Rectangle tip 2 × 1 half extents across a vertical segment (n = (−1, 0)):
        // |rx·nx| + |ry·ny| = 2; quad (−2,0) (−2,5) (2,5) (2,0) turns negatively → reversed.
        let pts = [
            TipAt {
                x: 0.0,
                y: 0.0,
                rx: 2.0,
                ry: 1.0,
            },
            TipAt {
                x: 0.0,
                y: 5.0,
                rx: 2.0,
                ry: 1.0,
            },
        ];
        let segs = stroke_outline(&pts, Tip::Rectangle);
        let quad: Vec<Seg> = segs[10..].to_vec();
        assert_eq!(
            quad,
            vec![
                Seg::Move(2.0, 0.0),
                Seg::Line(2.0, 5.0),
                Seg::Line(-2.0, 5.0),
                Seg::Line(-2.0, 0.0),
                Seg::Close
            ]
        );
    }

    #[test]
    fn placement_fits_frame() {
        // Samples (10,20)–(30,40), pen 4 pt wide → margin 2 → natural 24 × 24;
        // a 48 × 12 frame scales by (2, 0.5).
        let t = Trace {
            brush: Brush {
                width: 4.0,
                height: 4.0,
                ..Brush::default()
            },
            samples: vec![
                Sample {
                    x: 10.0,
                    y: 20.0,
                    f: Some(0.5),
                    t: None,
                },
                Sample {
                    x: 30.0,
                    y: 40.0,
                    f: None,
                    t: None,
                },
            ],
        };
        let (_, size) = Placement::natural(std::slice::from_ref(&t)).unwrap();
        assert_eq!(size, (24.0, 24.0));
        let p = Placement::fit(std::slice::from_ref(&t), 48.0, 12.0).unwrap();
        assert_eq!(p.map(10.0, 20.0), (4.0, 1.0));
        assert_eq!(p.map(30.0, 40.0), (44.0, 11.0));
        let tips = tips(&t, &p);
        // Half width 2 · 2 at f = 0.5; half height 2 · 0.5.
        assert_eq!((tips[0].rx, tips[0].ry), (4.0, 1.0));
    }

    #[test]
    fn number_splitting() {
        assert_eq!(decode("1-2 .5.5", 4), vec![vec![1.0, -2.0, 0.5, 0.5]]);
        assert_eq!(decode("3e2 T F", 3), vec![vec![300.0, 1.0, 0.0]]);
        assert_eq!(decode("1 2, * 5", 2), vec![vec![1.0, 2.0], vec![1.0, 5.0]]);
    }
}
