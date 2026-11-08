//! What an animation effect does, read from its time-node tree (ECMA-376 Part 1, §19.5): the
//! behaviours of the effect's `p:par` — §19.5.66 set, §19.5.1 anim (keyframes, formulas),
//! §19.5.3 animEffect (transition filters), §19.5.4 animMotion (motion paths), §19.5.5 animRot,
//! §19.5.6 animScale, §19.5.2 animClr — placed on the effect's own timeline with their delays,
//! repetitions (`repeatCount`) and reversals (`autoRev`). Values are evaluated for the animated
//! shape (`#ppt_x`, `#ppt_y`, `#ppt_w`, `#ppt_h` of §19.5.1), so the slide show can play every
//! effect of a file, whichever application wrote it.

use bo_model::slide::{XmlElement, XmlNode};
use serde::Serialize;

/// The animated shape: centre and size as fractions of the slide (`#ppt_x` … `#ppt_h`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeBase {
    /// Centre x (`#ppt_x`).
    pub x: f64,
    /// Centre y (`#ppt_y`).
    pub y: f64,
    /// Width (`#ppt_w`).
    pub w: f64,
    /// Height (`#ppt_h`).
    pub h: f64,
}

/// What one behaviour animates.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TrackKind {
    /// §19.5.66 set: `attr` takes `value` (e.g. `style.visibility` = `visible`).
    Set {
        /// Attribute (§19.5.? attrName).
        attr: String,
        /// Value set.
        value: String,
    },
    /// §19.5.1 anim: `attr` through `keys` (time 0..1 of the behaviour, value).
    ///
    /// Values: `ppt_x` / `ppt_y` in slide widths / heights (centre), `ppt_w` / `ppt_h` in slide
    /// widths / heights, `style.opacity` 0..1, `r` / `style.rotation` in degrees.
    Anim {
        /// Attribute.
        attr: String,
        /// Keyframes (time 0..1, value).
        keys: Vec<(f64, f64)>,
        /// Jumps from value to value (calcmode `discrete`).
        discrete: bool,
    },
    /// §19.5.3 animEffect: a transition filter (`wipe(down)`, `blinds(horizontal)`, …) bringing the
    /// shape in or taking it out.
    Filter {
        /// Filter and subtype.
        filter: String,
        /// Transition `in` (else `out`).
        entering: bool,
    },
    /// §19.5.4 animMotion: a path of offsets (slide widths, heights) from the shape's position.
    Motion {
        /// Offsets along the path.
        points: Vec<(f64, f64)>,
    },
    /// §19.5.5 animRot: turn by `by` degrees.
    Rotate {
        /// Degrees clockwise.
        by: f64,
    },
    /// §19.5.6 animScale: scale from `from` to `to` (ratios of the size).
    Scale {
        /// Start ratios (x, y).
        from: (f64, f64),
        /// End ratios (x, y).
        to: (f64, f64),
    },
    /// §19.5.2 animClr: `attr` (fill, line, text colour) to sRGB `to`.
    Color {
        /// Coloured attribute (`fillcolor`, `stroke.color`, `style.color`).
        attr: String,
        /// Target colour `0xRRGGBB`.
        to: Option<u32>,
    },
}

/// One behaviour on the effect's timeline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    /// Start, ms after the effect starts.
    pub begin_ms: f64,
    /// Duration of one run, ms (`indefinite`: 0).
    pub dur_ms: f64,
    /// Runs (§19.5.29 repeatCount / 1000).
    pub repeat: f64,
    /// Every run plays forwards then backwards (§19.5.29 autoRev).
    pub auto_rev: bool,
    /// The value holds after the end (§19.5.29 fill `hold` / `freeze`).
    pub hold: bool,
    /// What it animates.
    #[serde(flatten)]
    pub kind: TrackKind,
}

impl Track {
    /// End of the behaviour, ms after the effect starts.
    #[must_use]
    pub fn end_ms(&self) -> f64 {
        self.begin_ms + self.dur_ms * self.repeat * if self.auto_rev { 2.0 } else { 1.0 }
    }

    /// Progress 0..1 of the behaviour at `ms` after the effect starts (repeats and reversals
    /// applied); `None` before it starts.
    #[must_use]
    pub fn progress(&self, ms: f64) -> Option<f64> {
        if ms < self.begin_ms {
            return None;
        }
        if self.dur_ms <= 0.0 {
            return Some(1.0);
        }
        let run = if self.auto_rev {
            2.0 * self.dur_ms
        } else {
            self.dur_ms
        };
        let local = (ms - self.begin_ms).min(run * self.repeat);
        let mut p = if local >= run * self.repeat {
            // Ended: the last run's end state.
            if self.auto_rev {
                0.0
            } else {
                1.0
            }
        } else {
            (local % run) / self.dur_ms
        };
        if self.auto_rev && p > 1.0 {
            p = 2.0 - p;
        }
        Some(p.clamp(0.0, 1.0))
    }
}

/// The effect: its tracks and total duration (ms).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectPlay {
    /// Total duration, ms.
    pub dur_ms: f64,
    /// Behaviours.
    pub tracks: Vec<Track>,
}

// ---------------------------------------------------------------------------
// Formulas (§19.5.1 anim values: `#ppt_x`, operators, functions)
// ---------------------------------------------------------------------------

/// Evaluate a formula of an animation value with the shape's variables and `$` (time 0..1).
#[must_use]
pub fn eval(f: &str, base: &ShapeBase, t: f64) -> Option<f64> {
    let toks = tokens(f)?;
    let mut p = Parser {
        toks: &toks,
        i: 0,
        base,
        t,
    };
    let v = p.expr()?;
    (p.i == toks.len() && v.is_finite()).then_some(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(char),
}

fn tokens(s: &str) -> Option<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' {
            let st = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                i += 1;
            }
            // Exponent (1e-3).
            if i < c.len()
                && (c[i] == 'e' || c[i] == 'E')
                && c.get(i + 1)
                    .is_some_and(|d| d.is_ascii_digit() || *d == '-')
            {
                i += 2;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push(Tok::Num(c[st..i].iter().collect::<String>().parse().ok()?));
        } else if ch.is_alphabetic() || ch == '#' || ch == '_' || ch == '$' {
            let st = i;
            i += 1;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Id(c[st..i].iter().collect()));
        } else if "+-*/^%(),".contains(ch) {
            out.push(Tok::Op(ch));
            i += 1;
        } else {
            return None;
        }
    }
    Some(out)
}

struct Parser<'a> {
    toks: &'a [Tok],
    i: usize,
    base: &'a ShapeBase,
    t: f64,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Tok::Op(op)) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        loop {
            if self.eat('*') {
                v *= self.power()?;
            } else if self.eat('/') {
                v /= self.power()?;
            } else if self.eat('%') {
                v %= self.power()?;
            } else {
                return Some(v);
            }
        }
    }

    fn power(&mut self) -> Option<f64> {
        let b = self.unary()?;
        if self.eat('^') {
            return Some(b.powf(self.power()?));
        }
        Some(b)
    }

    fn unary(&mut self) -> Option<f64> {
        if self.eat('-') {
            return Some(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.atom()
    }

    fn atom(&mut self) -> Option<f64> {
        match self.peek()?.clone() {
            Tok::Num(n) => {
                self.i += 1;
                Some(n)
            }
            Tok::Op('(') => {
                self.i += 1;
                let v = self.expr()?;
                self.eat(')').then_some(v)
            }
            Tok::Id(id) => {
                self.i += 1;
                if self.eat('(') {
                    let mut args = vec![self.expr()?];
                    while self.eat(',') {
                        args.push(self.expr()?);
                    }
                    if !self.eat(')') {
                        return None;
                    }
                    return func(&id, &args);
                }
                let b = self.base;
                Some(match id.as_str() {
                    "#ppt_x" => b.x,
                    "#ppt_y" => b.y,
                    "#ppt_w" => b.w,
                    "#ppt_h" => b.h,
                    "$" => self.t,
                    "pi" => std::f64::consts::PI,
                    "e" => std::f64::consts::E,
                    _ => return None,
                })
            }
            Tok::Op(_) => None,
        }
    }
}

fn func(name: &str, a: &[f64]) -> Option<f64> {
    let x = *a.first()?;
    Some(match (name, a.len()) {
        ("abs", 1) => x.abs(),
        ("acos", 1) => x.acos(),
        ("asin", 1) => x.asin(),
        ("atan", 1) => x.atan(),
        ("ceil", 1) => x.ceil(),
        ("cos", 1) => x.cos(),
        ("cosh", 1) => x.cosh(),
        ("deg", 1) => x.to_degrees(),
        ("exp", 1) => x.exp(),
        ("floor", 1) => x.floor(),
        ("ln", 1) => x.ln(),
        ("rad", 1) => x.to_radians(),
        // A deterministic stand-in for `rand(x)`: half of x.
        ("rand", 1) => x / 2.0,
        ("sin", 1) => x.sin(),
        ("sinh", 1) => x.sinh(),
        ("sqrt", 1) => x.sqrt(),
        ("tan", 1) => x.tan(),
        ("tanh", 1) => x.tanh(),
        ("max", 2) => x.max(a[1]),
        ("min", 2) => x.min(a[1]),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Time nodes
// ---------------------------------------------------------------------------

fn elements(el: &XmlElement) -> impl Iterator<Item = &XmlElement> {
    el.children.iter().filter_map(|c| match c {
        XmlNode::Element(e) => Some(e),
        XmlNode::Text(_) => None,
    })
}

fn child<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    elements(el).find(|c| c.local() == local)
}

fn text_of(el: &XmlElement) -> String {
    el.children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Text(t) => Some(t.as_str()),
            XmlNode::Element(_) => None,
        })
        .collect()
}

/// §19.5.29 cTn timing: own delay (first §19.5.25 cond), duration, repeats, reversal, fill.
struct Ctn {
    delay: f64,
    dur: Option<f64>,
    repeat: f64,
    auto_rev: bool,
    hold: bool,
}

fn ctn_of(ctn: &XmlElement) -> Ctn {
    let delay = child(ctn, "stCondLst")
        .and_then(|l| elements(l).find(|c| c.local() == "cond"))
        .and_then(|c| c.attr("delay"))
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);
    let dur = ctn.attr("dur").and_then(|d| d.parse::<f64>().ok());
    let repeat = ctn
        .attr("repeatCount")
        .and_then(|r| r.parse::<f64>().ok())
        .map_or(1.0, |r| (r / 1000.0).clamp(0.001, 1000.0));
    Ctn {
        delay,
        dur,
        repeat,
        auto_rev: ctn.attr("autoRev").is_some_and(|v| v == "1" || v == "true"),
        hold: matches!(ctn.attr("fill"), Some("hold" | "freeze")),
    }
}

/// A §19.5.79 tav value: number, formula string, or a §19.5.? strVal / fltVal / intVal / boolVal.
fn value_of(val: &XmlElement, base: &ShapeBase, t: f64) -> Option<f64> {
    let v = elements(val).next()?;
    let s = v.attr("val")?;
    match v.local() {
        "fltVal" | "intVal" => s.parse().ok(),
        "boolVal" => Some(if s == "1" || s == "true" { 1.0 } else { 0.0 }),
        _ => eval(s, base, t).or_else(|| s.parse().ok()),
    }
}

/// ST_TLTimeAnimateValueTime: 1000ths of a percent (0..100000).
fn tm(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| v.parse::<f64>().ok())
        .map(|v| (v / 100_000.0).clamp(0.0, 1.0))
}

/// Normalised attribute name of a §19.5.? attrName.
fn attr_name(bhvr: &XmlElement) -> String {
    child(bhvr, "attrNameLst")
        .and_then(|l| elements(l).find(|c| c.local() == "attrName"))
        .map(|a| text_of(a).trim().to_owned())
        .unwrap_or_default()
}

fn anim_track(el: &XmlElement, base: &ShapeBase) -> Option<TrackKind> {
    let bhvr = child(el, "cBhvr")?;
    let attr = attr_name(bhvr);
    let discrete = el.attr("calcmode") == Some("discrete");
    let mut keys = Vec::new();
    if let Some(list) = child(el, "tavLst") {
        let tavs: Vec<&XmlElement> = elements(list).filter(|c| c.local() == "tav").collect();
        let n = tavs.len().max(2) - 1;
        for (k, tav) in tavs.iter().enumerate() {
            let at = tm(tav.attr("tm")).unwrap_or(k as f64 / n as f64);
            let Some(val) = child(tav, "val") else {
                continue;
            };
            // §19.5.79 tav@fmla: the value through a formula of `$` (the keyframe value).
            let v = match tav.attr("fmla") {
                Some(f) => {
                    let x = value_of(val, base, at).unwrap_or(0.0);
                    eval(f, base, x)
                }
                None => value_of(val, base, at),
            };
            if let Some(v) = v {
                keys.push((at, v));
            }
        }
    } else {
        // from / to / by on the element.
        let get = |a: &str| {
            el.attr(a)
                .and_then(|s| eval(s, base, 0.0).or_else(|| s.parse().ok()))
        };
        let from = get("from");
        let to = get("to").or_else(|| get("by").map(|b| from.unwrap_or(0.0) + b));
        if let (Some(f), Some(t)) = (from, to) {
            keys = vec![(0.0, f), (1.0, t)];
        }
    }
    (!keys.is_empty()).then_some(TrackKind::Anim {
        attr,
        keys,
        discrete,
    })
}

/// §19.5.4 animMotion@path: `M`, `L`, `C`, `Z`, `E` with coordinates in slide widths / heights
/// (relative commands in lower case), sampled into points.
#[must_use]
pub fn motion_points(path: &str) -> Vec<(f64, f64)> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    for ch in path.chars() {
        if ch.is_ascii_alphabetic() && ch != 'e' || (ch == 'e' && cur.is_empty()) {
            if !cur.trim().is_empty() {
                toks.push(cur.clone());
            }
            cur.clear();
            toks.push(ch.to_string());
        } else if ch == ' ' || ch == ',' {
            if !cur.trim().is_empty() {
                toks.push(cur.clone());
            }
            cur.clear();
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        toks.push(cur);
    }
    let mut out: Vec<(f64, f64)> = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    let (mut sx, mut sy) = (0.0, 0.0);
    let mut cmd = 'M';
    let mut i = 0;
    let num = |i: &mut usize| -> Option<f64> {
        let v = toks.get(*i)?.parse::<f64>().ok()?;
        *i += 1;
        Some(v)
    };
    while i < toks.len() {
        if let Some(c) = toks[i]
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic() && toks[i].len() == 1)
        {
            cmd = c;
            i += 1;
            if matches!(cmd, 'Z' | 'z') {
                out.push((sx, sy));
                x = sx;
                y = sy;
            }
            if matches!(cmd, 'E' | 'e') {
                break;
            }
            continue;
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' | 'L' => {
                let (Some(a), Some(b)) = (num(&mut i), num(&mut i)) else {
                    break;
                };
                x = ox + a;
                y = oy + b;
                if cmd.eq_ignore_ascii_case(&'M') {
                    sx = x;
                    sy = y;
                }
                out.push((x, y));
            }
            'C' => {
                let vals: Vec<f64> = (0..6).filter_map(|_| num(&mut i)).collect();
                if vals.len() < 6 {
                    break;
                }
                let (x0, y0) = (x, y);
                let p1 = (ox + vals[0], oy + vals[1]);
                let p2 = (ox + vals[2], oy + vals[3]);
                let p3 = (ox + vals[4], oy + vals[5]);
                for k in 1..=12 {
                    let t = f64::from(k) / 12.0;
                    let u = 1.0 - t;
                    out.push((
                        u * u * u * x0
                            + 3.0 * u * u * t * p1.0
                            + 3.0 * u * t * t * p2.0
                            + t * t * t * p3.0,
                        u * u * u * y0
                            + 3.0 * u * u * t * p1.1
                            + 3.0 * u * t * t * p2.1
                            + t * t * t * p3.1,
                    ));
                }
                (x, y) = p3;
            }
            _ => i += 1,
        }
    }
    if out.first().is_some_and(|p| p.0 != 0.0 || p.1 != 0.0)
        && !path.trim_start().starts_with(['M', 'm'])
    {
        out.insert(0, (0.0, 0.0));
    }
    out
}

fn behaviour(el: &XmlElement, base: &ShapeBase) -> Option<TrackKind> {
    match el.local() {
        "set" => {
            let bhvr = child(el, "cBhvr")?;
            let v = child(el, "to")
                .and_then(|t| elements(t).next())
                .and_then(|v| v.attr("val"))?;
            Some(TrackKind::Set {
                attr: attr_name(bhvr),
                value: v.to_owned(),
            })
        }
        "anim" => anim_track(el, base),
        "animEffect" => Some(TrackKind::Filter {
            filter: el.attr("filter").unwrap_or("fade").to_owned(),
            entering: el.attr("transition") != Some("out"),
        }),
        "animMotion" => Some(TrackKind::Motion {
            points: motion_points(el.attr("path").unwrap_or("")),
        }),
        "animRot" => {
            let a = |n: &str| {
                el.attr(n)
                    .and_then(|v| v.parse::<f64>().ok())
                    .map(|v| v / 60_000.0)
            };
            let by = a("by").or_else(|| Some(a("to")? - a("from").unwrap_or(0.0)))?;
            Some(TrackKind::Rotate { by })
        }
        "animScale" => {
            // §19.5.6: by / from / to as 1000ths of a percent (x, y).
            let pt = |n: &str| {
                let c = child(el, n)?;
                let g = |a: &str| {
                    c.attr(a)
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(|v| v / 100_000.0)
                };
                Some((g("x").unwrap_or(1.0), g("y").unwrap_or(1.0)))
            };
            let from = pt("from").unwrap_or((1.0, 1.0));
            let to = match (pt("to"), pt("by")) {
                (Some(t), _) => t,
                (None, Some(b)) => (from.0 * b.0, from.1 * b.1),
                (None, None) => from,
            };
            Some(TrackKind::Scale { from, to })
        }
        "animClr" => {
            let bhvr = child(el, "cBhvr")?;
            let to = child(el, "to")
                .and_then(|t| elements(t).find(|c| c.local() == "srgbClr"))
                .and_then(|c| c.attr("val"))
                .and_then(|v| u32::from_str_radix(v, 16).ok());
            Some(TrackKind::Color {
                attr: attr_name(bhvr),
                to,
            })
        }
        _ => None,
    }
}

/// Walk the time nodes under `el` (offset `at` ms), collecting behaviours.
fn walk(el: &XmlElement, at: f64, base: &ShapeBase, out: &mut Vec<Track>) {
    for c in elements(el) {
        match c.local() {
            "par" | "seq" | "excl" => {
                let Some(ctn) = child(c, "cTn") else { continue };
                let t = ctn_of(ctn);
                let start = at + t.delay;
                if let Some(kids) = child(ctn, "childTnLst") {
                    if c.local() == "seq" {
                        // §19.5.65 seq: children one after the other.
                        let mut cursor = start;
                        for k in elements(kids) {
                            let mut sub = Vec::new();
                            walk(
                                &XmlElement {
                                    children: vec![XmlNode::Element(k.clone())],
                                    ..XmlElement::new("w")
                                },
                                cursor,
                                base,
                                &mut sub,
                            );
                            cursor = sub.iter().map(Track::end_ms).fold(cursor, f64::max);
                            out.extend(sub);
                        }
                    } else {
                        walk(kids, start, base, out);
                    }
                }
            }
            "set" | "anim" | "animEffect" | "animMotion" | "animRot" | "animScale" | "animClr" => {
                let Some(ctn) = child(c, "cBhvr").and_then(|b| child(b, "cTn")) else {
                    continue;
                };
                let t = ctn_of(ctn);
                let Some(kind) = behaviour(c, base) else {
                    continue;
                };
                out.push(Track {
                    begin_ms: at + t.delay,
                    dur_ms: t.dur.unwrap_or(if c.local() == "set" { 1.0 } else { 0.0 }),
                    repeat: t.repeat,
                    auto_rev: t.auto_rev,
                    hold: t.hold || c.local() == "set",
                    kind,
                });
            }
            "childTnLst" | "subTnLst" => walk(c, at, base, out),
            _ => {}
        }
    }
}

/// What the effect `par` (§19.5.53, as in the main sequence) does to a shape `base`.
#[must_use]
pub fn effect_play(par: &XmlElement, base: &ShapeBase) -> EffectPlay {
    let mut tracks = Vec::new();
    // The effect node's own delay is the effect's start, not part of its timeline.
    if let Some(kids) = child(par, "cTn").and_then(|c| child(c, "childTnLst")) {
        walk(kids, 0.0, base, &mut tracks);
    }
    let dur_ms = tracks.iter().map(Track::end_ms).fold(0.0, f64::max);
    EffectPlay { dur_ms, tracks }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const BASE: ShapeBase = ShapeBase {
        x: 0.25,
        y: 0.5,
        w: 0.2,
        h: 0.1,
    };

    // source: ECMA-376 Part 1, §19.5.1 anim formulas (`#ppt_x`, `#ppt_w`, operators, functions).
    #[test]
    fn formulas_evaluate() {
        let e = |f: &str| eval(f, &BASE, 0.5).unwrap();
        assert!((e("#ppt_x") - 0.25).abs() < 1e-12);
        assert!((e("0-#ppt_w/2") + 0.1).abs() < 1e-12);
        assert!((e("1+#ppt_h/2") - 1.05).abs() < 1e-12);
        assert!((e("#ppt_y+sin(pi*$)*0.1") - 0.6).abs() < 1e-12);
        assert!((e("2^3*min(1,4)-max(2,1)") - 6.0).abs() < 1e-12);
        assert!((e("-(1+2)*3") + 9.0).abs() < 1e-12);
        assert!((e("1e-3*1000") - 1.0).abs() < 1e-12);
        assert_eq!(eval("#ppt_q", &BASE, 0.0), None);
        assert_eq!(eval("1+", &BASE, 0.0), None);
    }

    fn par(xml: &str) -> XmlElement {
        bo_drawingml::dom::parse(xml.as_bytes(), "/t.xml").unwrap()
    }

    // source: ECMA-376 Part 1, §19.5 — a fly-in from the bottom with an appear set, a fade with a
    // delay, a spin repeated twice with autoRev, a motion path.
    #[test]
    fn effects_play_on_their_timeline() {
        let x = r##"<p:par xmlns:p="p"><p:cTn id="5" presetID="2" presetClass="entr" nodeType="clickEffect" fill="hold"><p:stCondLst><p:cond delay="300"/></p:stCondLst><p:childTnLst>
<p:set><p:cBhvr><p:cTn id="6" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="4"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set>
<p:anim calcmode="lin" valueType="num"><p:cBhvr additive="base"><p:cTn id="7" dur="500" fill="hold"/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl><p:attrNameLst><p:attrName>ppt_y</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst><p:tav tm="0"><p:val><p:strVal val="1+#ppt_h/2"/></p:val></p:tav><p:tav tm="100000"><p:val><p:strVal val="#ppt_y"/></p:val></p:tav></p:tavLst></p:anim>
<p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="8" dur="400"><p:stCondLst><p:cond delay="200"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cBhvr></p:animEffect>
<p:animRot by="21600000"><p:cBhvr><p:cTn id="9" dur="250" repeatCount="2000" autoRev="1"/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cBhvr></p:animRot>
<p:animMotion origin="layout" path="M 0 0 L 0.25 0 L 0.25 0.1 E"><p:cBhvr><p:cTn id="10" dur="1000"/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cBhvr></p:animMotion>
<p:animScale><p:cBhvr><p:cTn id="11" dur="500"/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cBhvr><p:by x="150000" y="150000"/></p:animScale>
</p:childTnLst></p:cTn></p:par>"##;
        let p = effect_play(&par(x), &BASE);
        assert_eq!(p.tracks.len(), 6);
        assert!(
            matches!(&p.tracks[0].kind, TrackKind::Set { attr, value } if attr == "style.visibility" && value == "visible")
        );
        let TrackKind::Anim { attr, keys, .. } = &p.tracks[1].kind else {
            panic!()
        };
        assert_eq!(attr, "ppt_y");
        assert!((keys[0].1 - 1.05).abs() < 1e-12 && (keys[1].1 - 0.5).abs() < 1e-12);
        // The fade starts 200 ms in; the effect node's own 300 ms delay is not on its timeline.
        assert_eq!((p.tracks[2].begin_ms, p.tracks[2].end_ms()), (200.0, 600.0));
        assert!(matches!(
            p.tracks[2].kind,
            TrackKind::Filter { entering: true, .. }
        ));
        // Spin 360° (21600000 / 60000), two forward-and-back runs of 250 ms: 1000 ms.
        assert!(matches!(p.tracks[3].kind, TrackKind::Rotate { by } if (by - 360.0).abs() < 1e-9));
        assert_eq!(p.tracks[3].end_ms(), 1000.0);
        assert_eq!(p.tracks[3].progress(125.0), Some(0.5));
        assert_eq!(p.tracks[3].progress(375.0), Some(0.5));
        assert_eq!(p.tracks[3].progress(250.0), Some(1.0));
        assert_eq!(p.tracks[3].progress(2000.0), Some(0.0));
        let TrackKind::Motion { points } = &p.tracks[4].kind else {
            panic!()
        };
        assert_eq!(points, &vec![(0.0, 0.0), (0.25, 0.0), (0.25, 0.1)]);
        assert!(
            matches!(p.tracks[5].kind, TrackKind::Scale { to, .. } if (to.0 - 1.5).abs() < 1e-12)
        );
        assert_eq!(p.dur_ms, 1000.0);
        assert_eq!(p.tracks[1].progress(-1.0), None);
    }

    #[test]
    fn motion_paths_sample_curves() {
        let pts = motion_points("M 0 0 C 0.1 0 0.2 0.1 0.2 0.2 E");
        assert_eq!(pts.len(), 13);
        assert!((pts[12].0 - 0.2).abs() < 1e-12 && (pts[12].1 - 0.2).abs() < 1e-12);
        // Relative commands and a closed loop.
        let pts = motion_points("m 0 0 l 0.1 0 l 0 0.1 z");
        assert_eq!(pts, vec![(0.0, 0.0), (0.1, 0.0), (0.1, 0.1), (0.0, 0.0)]);
    }
}
