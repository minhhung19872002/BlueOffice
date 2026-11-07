//! Text of shapes: plain-text editing that keeps run formatting, and
//! shape-level character/paragraph formatting (ECMA-376 Part 1, §21.1.2).
//!
//! Editing text format: paragraphs are separated by `\n`, line breaks inside a
//! paragraph (§21.1.2.2.1 br) are `\u{b}`.

use bo_model::slide::{Bullet, Color, Fill, ParaProps, Paragraph, RunProps, TextBody, TextRun};
use serde::{Deserialize, Serialize};

/// Line-break character of the editing format.
pub const BR: char = '\u{b}';

/// Paragraph text in the editing format.
pub fn para_text(p: &Paragraph) -> String {
    let mut s = String::new();
    for r in &p.runs {
        match r {
            TextRun::Run { text, .. } | TextRun::Field { text, .. } => s.push_str(text),
            TextRun::Break { .. } => s.push(BR),
            TextRun::Other(_) => {}
        }
    }
    s
}

/// Text body in the editing format.
pub fn body_text(t: &TextBody) -> String {
    t.paragraphs
        .iter()
        .map(para_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

fn slice_chars(s: &str, from: usize, to: usize) -> String {
    s.chars().skip(from).take(to.saturating_sub(from)).collect()
}

/// Length in characters of a run in the editing format.
fn run_len(r: &TextRun) -> usize {
    match r {
        TextRun::Run { text, .. } | TextRun::Field { text, .. } => char_len(text),
        TextRun::Break { .. } => 1,
        TextRun::Other(_) => 0,
    }
}

fn run_props(r: &TextRun) -> Option<&RunProps> {
    match r {
        TextRun::Run { props, .. } | TextRun::Break { props } | TextRun::Field { props, .. } => {
            Some(props)
        }
        TextRun::Other(_) => None,
    }
}

/// Runs covering characters `[from, to)`; zero-length runs at position `q`
/// are kept when `keep_zero(q)`.
fn keep_range(
    runs: &[TextRun],
    from: usize,
    to: usize,
    keep_zero: impl Fn(usize) -> bool,
) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut pos = 0;
    for r in runs {
        let len = run_len(r);
        let (a, b) = (pos, pos + len);
        pos = b;
        if len == 0 {
            if keep_zero(a) {
                out.push(r.clone());
            }
            continue;
        }
        let (s, e) = (a.max(from), b.min(to));
        if s >= e {
            continue;
        }
        if s == a && e == b {
            out.push(r.clone());
            continue;
        }
        // Partially kept: text runs are cut, a cut field becomes plain text.
        match r {
            TextRun::Run { props, text } | TextRun::Field { props, text, .. } => {
                out.push(TextRun::Run {
                    props: props.clone(),
                    text: slice_chars(text, s - a, e - a),
                });
            }
            _ => out.push(r.clone()),
        }
    }
    out
}

/// Props of the character at `pos` (or the one before when at the end).
fn props_at(p: &Paragraph, pos: usize) -> RunProps {
    // Typing continues the run of the character before the caret (the first
    // run at the very start).
    let mut at = 0;
    let mut last: Option<&RunProps> = None;
    for r in &p.runs {
        let len = run_len(r);
        if let (Some(rp), true) = (run_props(r), len > 0) {
            if (pos > at && pos <= at + len) || (pos == 0 && at == 0) {
                return rp.clone();
            }
            last = Some(rp);
        }
        at += len;
    }
    last.cloned()
        .or_else(|| p.end_props.clone())
        .unwrap_or_default()
}

/// Runs for inserted plain text.
fn text_runs(text: &str, props: &RunProps) -> Vec<TextRun> {
    let mut out = Vec::new();
    for (i, part) in text.split(BR).enumerate() {
        if i > 0 {
            out.push(TextRun::Break {
                props: props.clone(),
            });
        }
        if !part.is_empty() {
            out.push(TextRun::Run {
                props: props.clone(),
                text: part.to_owned(),
            });
        }
    }
    out
}

/// Merge neighbouring text runs with equal properties.
fn merge_runs(runs: Vec<TextRun>) -> Vec<TextRun> {
    let mut out: Vec<TextRun> = Vec::with_capacity(runs.len());
    for r in runs {
        if let (
            Some(TextRun::Run {
                props: pa,
                text: ta,
            }),
            TextRun::Run { props, text },
        ) = (out.last_mut(), &r)
        {
            if pa == props {
                ta.push_str(text);
                continue;
            }
        }
        out.push(r);
    }
    out
}

/// Change the text of a paragraph keeping the runs around the edit.
fn edit_paragraph(p: &Paragraph, new: &str) -> Paragraph {
    let old = para_text(p);
    if old == new {
        return p.clone();
    }
    let oc: Vec<char> = old.chars().collect();
    let nc: Vec<char> = new.chars().collect();
    let prefix = oc.iter().zip(&nc).take_while(|(a, b)| a == b).count();
    let max_suffix = oc.len().min(nc.len()) - prefix;
    let suffix = oc
        .iter()
        .rev()
        .zip(nc.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    let old_tail = oc.len() - suffix;
    let middle: String = nc[prefix..nc.len() - suffix].iter().collect();
    let props = props_at(p, prefix);
    let mut runs = keep_range(&p.runs, 0, prefix, |q| q <= prefix);
    runs.extend(text_runs(&middle, &props));
    runs.extend(keep_range(&p.runs, old_tail, oc.len(), |q| {
        q > prefix && q >= old_tail
    }));
    Paragraph {
        props: p.props.clone(),
        runs: merge_runs(runs),
        end_props: p.end_props.clone(),
    }
}

/// New paragraph formatted like `base` (its paragraph and last run properties).
fn paragraph_like(base: Option<&Paragraph>, text: &str) -> Paragraph {
    let props = base
        .map(|b| props_at(b, char_len(&para_text(b))))
        .unwrap_or_default();
    Paragraph {
        props: base.and_then(|b| b.props.clone()),
        runs: text_runs(text, &props),
        end_props: base
            .and_then(|b| b.end_props.clone())
            .or_else(|| Some(props.clone())),
    }
}

/// Replace the text of a body; unchanged paragraphs stay as they are and the
/// formatting of edited ones is kept around the change.
pub fn set_text(tb: &mut TextBody, text: &str) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let old = std::mem::take(&mut tb.paragraphs);
    let old_text: Vec<String> = old.iter().map(para_text).collect();
    let pre = old_text
        .iter()
        .zip(&lines)
        .take_while(|(a, b)| a.as_str() == **b)
        .count();
    let max_suf = old.len().min(lines.len()) - pre;
    let suf = old_text
        .iter()
        .rev()
        .zip(lines.iter().rev())
        .take(max_suf)
        .take_while(|(a, b)| a.as_str() == **b)
        .count();
    let old_mid = &old[pre..old.len() - suf];
    let new_mid = &lines[pre..lines.len() - suf];
    let mut out: Vec<Paragraph> = old[..pre].to_vec();
    for (j, line) in new_mid.iter().enumerate() {
        if let Some(p) = old_mid.get(j) {
            out.push(edit_paragraph(p, line));
        } else {
            let base = old_mid.last().or(out.last()).or(old.first());
            out.push(paragraph_like(base, line));
        }
    }
    out.extend_from_slice(&old[old.len() - suf..]);
    if out.is_empty() {
        out.push(paragraph_like(old.first(), ""));
    }
    tb.paragraphs = out;
}

/// Shape-level formatting command.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TextFormat {
    /// Toggle when `value` is absent.
    Bold {
        value: Option<bool>,
    },
    Italic {
        value: Option<bool>,
    },
    Underline {
        value: Option<bool>,
    },
    /// Font size in points.
    Size {
        points: f64,
    },
    /// Grow/shrink by `points`.
    SizeBy {
        points: f64,
    },
    /// `RRGGBB`; `None` = inherit.
    Color {
        hex: Option<String>,
    },
    Align {
        align: Align,
    },
    Bullets {
        value: Option<bool>,
    },
}

/// Paragraph alignment (§21.1.10.59 ST_TextAlignType).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
}

impl Align {
    fn code(self) -> &'static str {
        match self {
            Align::Left => "l",
            Align::Center => "ctr",
            Align::Right => "r",
            Align::Justify => "just",
        }
    }
    pub fn of(code: Option<&str>) -> Align {
        match code {
            Some("ctr") => Align::Center,
            Some("r") => Align::Right,
            Some("just" | "dist" | "justLow" | "thaiDist") => Align::Justify,
            _ => Align::Left,
        }
    }
}

/// Effective formatting of the start of a text body (ribbon state).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FormatState {
    pub has_text: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Points.
    pub size: Option<f64>,
    pub color: Option<String>,
    pub align: Option<Align>,
    pub bullets: bool,
}

/// Parse `RRGGBB` (with or without `#`).
pub fn parse_hex(hex: &str) -> Option<u32> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    u32::from_str_radix(h, 16).ok()
}

type RunChange = Box<dyn Fn(&mut RunProps)>;

/// Apply a resolved formatting change to every run of a body.
/// `eff` gives effective (paragraph, run) properties used for toggles.
pub fn apply_format(tb: &mut TextBody, f: &TextFormat, state: &FormatState, default_size: f64) {
    let toggle = |v: Option<bool>, cur: bool| v.unwrap_or(!cur);
    let rpr_change: Option<RunChange> = match f {
        TextFormat::Bold { value } => {
            let v = toggle(*value, state.bold);
            Some(Box::new(move |r: &mut RunProps| r.b = Some(v)))
        }
        TextFormat::Italic { value } => {
            let v = toggle(*value, state.italic);
            Some(Box::new(move |r: &mut RunProps| r.i = Some(v)))
        }
        TextFormat::Underline { value } => {
            let v = toggle(*value, state.underline);
            // §21.1.10.82 ST_TextUnderlineType: `sng` / `none`.
            Some(Box::new(move |r: &mut RunProps| {
                r.u = Some(if v { "sng" } else { "none" }.to_owned());
            }))
        }
        TextFormat::Size { points } => {
            let sz = (points.clamp(1.0, 4000.0) * 100.0).round() as i32;
            Some(Box::new(move |r: &mut RunProps| r.sz = Some(sz)))
        }
        TextFormat::SizeBy { points } => {
            let delta = *points;
            let base = state.size.unwrap_or(default_size);
            Some(Box::new(move |r: &mut RunProps| {
                let cur = r.sz.map_or(base, |s| f64::from(s) / 100.0);
                r.sz = Some(((cur + delta).clamp(1.0, 4000.0) * 100.0).round() as i32);
            }))
        }
        TextFormat::Color { hex } => {
            let c = hex.as_deref().and_then(parse_hex);
            Some(Box::new(move |r: &mut RunProps| {
                r.fill = c.map(|v| Fill::Solid(Color::srgb(v)));
            }))
        }
        TextFormat::Align { .. } | TextFormat::Bullets { .. } => None,
    };
    for p in &mut tb.paragraphs {
        if let Some(ch) = &rpr_change {
            for r in &mut p.runs {
                match r {
                    TextRun::Run { props, .. }
                    | TextRun::Break { props }
                    | TextRun::Field { props, .. } => ch(props),
                    TextRun::Other(_) => {}
                }
            }
            ch(p.end_props.get_or_insert_with(RunProps::default));
        }
        match f {
            TextFormat::Align { align } => {
                p.props.get_or_insert_with(ParaProps::default).algn = Some(align.code().into());
            }
            TextFormat::Bullets { value } => {
                let on = toggle(*value, state.bullets);
                let pp = p.props.get_or_insert_with(ParaProps::default);
                if on {
                    // §21.1.2.4.3 buChar with a hanging indent of 0.375 in.
                    pp.bullet = Some(Bullet::Char("•".into()));
                    if pp.mar_l.is_none() {
                        pp.mar_l = Some(342_900);
                    }
                    if pp.indent.is_none() {
                        pp.indent = Some(-342_900);
                    }
                } else {
                    // §21.1.2.4.8 buNone.
                    pp.bullet = Some(Bullet::None);
                    if pp.indent.is_some_and(|i| i < 0) {
                        pp.indent = Some(0);
                    }
                    if pp.mar_l == Some(342_900) {
                        pp.mar_l = Some(0);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, b: bool) -> TextRun {
        TextRun::Run {
            props: RunProps {
                b: Some(b),
                ..RunProps::default()
            },
            text: text.into(),
        }
    }

    fn body(paras: Vec<Vec<TextRun>>) -> TextBody {
        TextBody {
            paragraphs: paras
                .into_iter()
                .map(|runs| Paragraph {
                    runs,
                    ..Paragraph::default()
                })
                .collect(),
            ..TextBody::default()
        }
    }

    #[test]
    fn edit_keeps_formatting_of_untouched_runs() {
        let mut tb = body(vec![
            vec![run("Xin ", false), run("chào", true)],
            vec![run("Hai", false)],
        ]);
        set_text(&mut tb, "Xin chào bạn\nHai");
        assert_eq!(body_text(&tb), "Xin chào bạn\nHai");
        // " bạn" continues the bold run before it.
        assert_eq!(
            tb.paragraphs[0].runs,
            vec![run("Xin ", false), run("chào bạn", true)]
        );
        // Insert a word inside the first (plain) run.
        set_text(&mut tb, "Xin mời chào bạn\nHai");
        assert_eq!(
            tb.paragraphs[0].runs,
            vec![run("Xin mời ", false), run("chào bạn", true)]
        );
        // New paragraph in the middle, then delete everything.
        set_text(&mut tb, "Xin mời chào bạn\nMới\nHai");
        assert_eq!(tb.paragraphs.len(), 3);
        assert_eq!(body_text(&tb), "Xin mời chào bạn\nMới\nHai");
        set_text(&mut tb, "");
        assert_eq!(tb.paragraphs.len(), 1);
        assert_eq!(body_text(&tb), "");
    }

    #[test]
    fn line_breaks_and_fields() {
        let mut tb = body(vec![vec![
            run("a", false),
            TextRun::Break {
                props: RunProps::default(),
            },
            TextRun::Field {
                id: "{X}".into(),
                kind: Some("slidenum".into()),
                props: RunProps::default(),
                ppr: None,
                text: "3".into(),
            },
        ]]);
        assert_eq!(body_text(&tb), "a\u{b}3");
        set_text(&mut tb, "ab\u{b}3");
        assert_eq!(tb.paragraphs[0].runs[0], run("ab", false));
        assert!(matches!(tb.paragraphs[0].runs[1], TextRun::Break { .. }));
        assert!(matches!(tb.paragraphs[0].runs[2], TextRun::Field { .. }));
    }

    #[test]
    fn formatting_toggles_and_bullets() {
        let mut tb = body(vec![vec![run("x", false)]]);
        let st = FormatState::default();
        apply_format(&mut tb, &TextFormat::Bold { value: None }, &st, 18.0);
        assert_eq!(tb.paragraphs[0].runs, vec![run("x", true)]);
        apply_format(&mut tb, &TextFormat::SizeBy { points: 2.0 }, &st, 18.0);
        let TextRun::Run { props, .. } = &tb.paragraphs[0].runs[0] else {
            panic!()
        };
        assert_eq!(props.sz, Some(2000));
        apply_format(&mut tb, &TextFormat::Bullets { value: None }, &st, 18.0);
        let pp = tb.paragraphs[0].props.as_ref().unwrap();
        assert_eq!(pp.bullet, Some(Bullet::Char("•".into())));
        apply_format(
            &mut tb,
            &TextFormat::Align {
                align: Align::Center,
            },
            &st,
            18.0,
        );
        assert_eq!(
            tb.paragraphs[0].props.as_ref().unwrap().algn.as_deref(),
            Some("ctr")
        );
        assert_eq!(parse_hex("#FF0000"), Some(0x00FF_0000));
    }
}
