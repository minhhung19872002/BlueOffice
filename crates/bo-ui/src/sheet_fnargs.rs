//! Formulas › Function Arguments (round 52, measured at 125 % on the reference's dialog:
//! 899 × 448 px, `show/sheet-58`): a frame titled with the function's name holding up to
//! five argument rows (bold name when required, a box with a range picker, `=` and the
//! argument's live value or its type in grey bold), the function's result under the frame,
//! its description, the focused argument's help, "Formula result =" and OK / Cancel.
//! Repeating arguments (`number1, [number2], …`) grow a box when the last one is filled.
//! Values come from the engine's `evaluateText` (nothing is stored until OK).
use super::form_impl::{Event, Form, FormKind, Kind};
use super::SheetWindow;
use crate::{Font, Painter};
use bo_render::Rgba;
use serde_json::{json, Value};

/// Dialog size (reference pixels, title bar included).
const SIZE: (f32, f32) = (899.0, 448.0);
/// Argument rows shown at once.
pub(super) const ROWS: usize = 5;
/// Box ids, their picker buttons and the first row's top; rows are 30 px apart.
pub(super) const BOXES: [&str; ROWS] = ["a0", "a1", "a2", "a3", "a4"];
pub(super) const PICKS: [&str; ROWS] = ["pick0", "pick1", "pick2", "pick3", "pick4"];
const ROW0: f32 = 73.0;
const ROW_H: f32 = 30.0;
/// Most arguments a repeating list grows to.
const MAX_ARGS: usize = 255;

/// One parameter of a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Param {
    pub name: String,
    pub optional: bool,
}

/// State of the open dialog.
#[derive(Debug, Clone, Default)]
pub(super) struct FnArgs {
    pub name: String,
    /// The fixed parameters, then the repeating group (`number1, [number2], …` repeats
    /// `number`).
    pub params: Vec<Param>,
    pub repeat: Option<Vec<String>>,
    /// Argument texts (as many as shown boxes).
    pub args: Vec<String>,
    /// First argument shown in row 0, and the focused argument.
    pub top: usize,
    pub focus: usize,
    /// The cell the formula goes into.
    pub cell: (usize, u32, u32),
    /// Live values: each argument's and the whole formula's.
    pub values: Vec<Option<String>>,
    pub result: Option<String>,
}

/// Parse a signature such as `number1, [number2], …` or `test, value_if_true,
/// [value_if_false]`: the parameters and the repeating group's base names.
pub(super) fn parse_signature(sig: &str) -> (Vec<Param>, Option<Vec<String>>) {
    let parts: Vec<&str> = sig
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let repeats = parts.last().is_some_and(|p| *p == "…" || *p == "...");
    let mut params: Vec<Param> = parts
        .iter()
        .filter(|p| **p != "…" && **p != "...")
        .map(|p| Param {
            name: p.trim_matches(['[', ']']).to_owned(),
            optional: p.starts_with('['),
        })
        .collect();
    if !repeats {
        return (params, None);
    }
    let base = |n: &str| n.trim_end_matches(|c: char| c.is_ascii_digit()).to_owned();
    // The group starts at the first `x2` whose `x1` came before (number2, test2).
    let first = params.iter().position(|p| {
        p.name.ends_with('2')
            && params
                .iter()
                .any(|q| q.name == format!("{}1", base(&p.name)))
    });
    let group = match first {
        Some(i) => {
            let g: Vec<String> = params[i..]
                .iter()
                .filter(|p| p.name.ends_with('2'))
                .map(|p| base(&p.name))
                .collect();
            params.truncate(i);
            g
        }
        None => params
            .last()
            .map(|p| vec![base(&p.name)])
            .unwrap_or_default(),
    };
    (params, Some(group))
}

/// Split the arguments of `=NAME(a, b, …)` at top-level commas (quotes, brackets and
/// braces respected); `None` when the formula is not a single call.
pub(super) fn split_call(formula: &str) -> Option<(String, Vec<String>)> {
    let f = formula.trim().strip_prefix('=')?.trim();
    let open = f.find('(')?;
    let name = f[..open].trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    {
        return None;
    }
    let body = f[open + 1..].strip_suffix(')')?;
    let mut args = vec![String::new()];
    let mut depth = 0i32;
    let mut quote = false;
    for ch in body.chars() {
        match ch {
            '"' => quote = !quote,
            '(' | '{' | '[' if !quote => depth += 1,
            ')' | '}' | ']' if !quote => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            ',' | ';' if !quote && depth == 0 => {
                args.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(last) = args.last_mut() {
            last.push(ch);
        }
    }
    if depth != 0 || quote {
        return None;
    }
    let args: Vec<String> = args.into_iter().map(|a| a.trim().to_owned()).collect();
    let args = if args.len() == 1 && args[0].is_empty() {
        Vec::new()
    } else {
        args
    };
    Some((name.to_ascii_uppercase(), args))
}

impl FnArgs {
    /// Name of argument `i` (number1, number2, … for the repeating group).
    pub(super) fn param_name(&self, i: usize) -> String {
        if let Some(p) = self.params.get(i) {
            return p.name.clone();
        }
        let Some(g) = self.repeat.as_ref().filter(|g| !g.is_empty()) else {
            return String::new();
        };
        // Position inside the repeating group, counted from its first parameter (number1).
        let start = self
            .params
            .iter()
            .position(|p| p.name == format!("{}1", g[0]))
            .unwrap_or(self.params.len());
        let k = i.saturating_sub(start);
        format!("{}{}", g[k % g.len()], k / g.len() + 1)
    }

    /// Argument `i` must be given.
    pub(super) fn required(&self, i: usize) -> bool {
        self.params.get(i).is_some_and(|p| !p.optional)
    }

    /// Boxes wanted: every argument typed plus one more for a repeating list (at least
    /// the fixed parameters).
    pub(super) fn wanted(&self) -> usize {
        let typed = self
            .args
            .iter()
            .rposition(|a| !a.is_empty())
            .map_or(0, |i| i + 1);
        let n = if self.repeat.is_some() {
            (typed + 1).max(self.params.len() + 1).min(MAX_ARGS)
        } else {
            typed
        };
        n.max(self.params.len()).max(1)
    }

    /// The formula the arguments make (trailing empty arguments left out).
    pub(super) fn formula(&self) -> String {
        let last = self
            .args
            .iter()
            .rposition(|a| !a.is_empty())
            .map_or(0, |i| i + 1);
        format!("={}({})", self.name, self.args[..last].join(","))
    }
}

/// Type shown in grey bold for an empty argument box, from its name.
pub(super) fn type_hint(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.starts_with("number") || n.contains("num") || n == "x" || n.ends_with("_num") {
        "number"
    } else if n.starts_with("logical") || n.starts_with("test") || n.contains("condition") {
        "logical"
    } else if n.starts_with("text") || n.contains("string") {
        "text"
    } else if n.contains("range") || n.contains("ref") || n.contains("array") {
        "reference"
    } else {
        "any"
    }
}

impl SheetWindow {
    /// Open Function Arguments for `name` with `args` (Insert Function's OK, or Insert
    /// Function on a cell holding a single call).
    pub(super) fn open_fn_args(&mut self, name: &str, args: Vec<String>) -> bool {
        let sig = super::dataforms_impl::FUNCTION_ARGS
            .iter()
            .find(|(n, _)| *n == name)
            .map_or("value1, [value2], …", |(_, s)| *s);
        let (params, repeat) = parse_signature(sig);
        let (row, col) = self.view.active();
        let mut st = FnArgs {
            name: name.to_owned(),
            params,
            repeat,
            args,
            cell: (self.view.sheet(), row, col),
            ..FnArgs::default()
        };
        let n = st.wanted();
        st.args.resize(n.max(st.args.len()), String::new());
        let t = |k: &str, d: &str| self.t(k, d);
        let mut f = Form::new(&t("sheetFn.argsTitle", "Function Arguments"), SIZE, vec![]);
        f.add(
            "frame",
            None,
            [18.0, 47.0, 861.0, 179.0],
            Kind::Group(name.to_owned()),
        );
        f.add("rows", None, [20.0, 62.0, 857.0, 160.0], Kind::Canvas);
        for i in 0..ROWS {
            let y = ROW0 + ROW_H * i as f32;
            f.add(
                BOXES[i],
                None,
                [217.0, y, 260.0, 27.0],
                Kind::Edit {
                    value: Vec::new(),
                    caret: 0,
                },
            );
            f.add(PICKS[i], None, [477.0, y, 27.0, 27.0], Kind::Canvas);
        }
        f.add("info", None, [18.0, 232.0, 861.0, 110.0], Kind::Canvas);
        f.add("rule", None, [18.0, 352.0, 861.0, 1.0], Kind::Canvas);
        f.add("result", None, [18.0, 366.0, 600.0, 24.0], Kind::Canvas);
        f.add(
            "helpLink",
            None,
            [18.0, 404.0, 250.0, 22.0],
            Kind::Label(t("sheetNative.fnHelpLink", "Help on this function")),
        );
        f.enable("helpLink", false);
        f.ok_cancel(&t("common.ok", "OK"), &t("common.cancel", "Cancel"));
        if let Some(c) = f.get_mut("ok") {
            c.rect = [641.0, 400.0, 111.0, 29.0];
        }
        if let Some(c) = f.get_mut("cancel") {
            c.rect = [768.0, 400.0, 111.0, 29.0];
        }
        self.form = Some(f);
        self.form_kind = FormKind::FnArgs;
        self.fnargs = Some(st);
        self.fnargs_show();
        if let Some(f) = self.form.as_mut() {
            // The first box opens with its text selected, as the reference's.
            f.select_all("a0");
        }
        true
    }

    /// Put the shown arguments into the boxes and refresh the live values.
    pub(super) fn fnargs_show(&mut self) {
        let Some(st) = self.fnargs.as_ref() else {
            return;
        };
        let shown: Vec<Option<String>> = (0..ROWS)
            .map(|i| {
                let k = st.top + i;
                (k < st.args.len()).then(|| st.args[k].clone())
            })
            .collect();
        if let Some(f) = self.form.as_mut() {
            for (i, a) in shown.iter().enumerate() {
                f.show(BOXES[i], a.is_some());
                f.show(PICKS[i], a.is_some());
                if let Some(a) = a {
                    if f.text(BOXES[i]) != *a {
                        f.set_text(BOXES[i], a);
                    }
                }
            }
        }
        self.fnargs_values();
    }

    /// Evaluate each argument and the formula.
    fn fnargs_values(&mut self) {
        let Some(st) = self.fnargs.clone() else {
            return;
        };
        let (sheet, row, col) = st.cell;
        let eval = |w: &mut Self, text: &str| -> Option<String> {
            if text.trim().is_empty() {
                return None;
            }
            let out = w.view.query(json!({
                "cmd": "evaluateText", "sheet": sheet, "row": row, "col": col, "formula": text,
            }))?;
            out.get("ok")
                .and_then(Value::as_bool)
                .filter(|ok| *ok)
                .and_then(|_| out.get("value").and_then(Value::as_str))
                .map(str::to_owned)
        };
        let values: Vec<Option<String>> = st.args.iter().map(|a| eval(self, a)).collect();
        let formula = st.formula();
        let result = eval(self, &formula);
        if let Some(s) = self.fnargs.as_mut() {
            s.values = values;
            s.result = result;
        }
    }

    /// Read the boxes back into the arguments; grow a repeating list.
    fn fnargs_take(&mut self) {
        let Some(f) = self.form.as_ref() else {
            return;
        };
        let texts: Vec<String> = BOXES.iter().map(|b| f.text(b)).collect();
        let Some(st) = self.fnargs.as_mut() else {
            return;
        };
        for (i, t) in texts.into_iter().enumerate() {
            let k = st.top + i;
            if k < st.args.len() {
                st.args[k] = t;
            }
        }
        let n = st.wanted();
        if n > st.args.len() {
            st.args.resize(n, String::new());
        }
    }

    /// Strings the canvases draw (measured at layout time).
    pub(super) fn fnargs_strings(&self) -> Vec<String> {
        let Some(st) = self.fnargs.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<String> = (st.top..(st.top + ROWS).min(st.args.len()))
            .map(|i| capitalized(&st.param_name(i)))
            .collect();
        out.push(format!("{}:", capitalized(&st.param_name(st.focus))));
        out
    }

    /// Events of Function Arguments.
    pub(super) fn fnargs_event(&mut self, ev: &Event, text: &dyn crate::TextMeasure) {
        match ev {
            Event::Cancel | Event::Clicked("cancel") => {
                self.form = None;
                self.fnargs = None;
            }
            Event::Ok | Event::Clicked("ok") => {
                self.fnargs_take();
                let Some(st) = self.fnargs.take() else {
                    return;
                };
                self.form = None;
                let (sheet, row, col) = st.cell;
                if sheet == self.view.sheet() {
                    self.view.select((row, col), (row, col), (row, col));
                }
                self.view.edit_with(&st.formula(), text);
                self.view.commit();
            }
            Event::Changed(id) => self.fnargs_changed(id),
            _ => {}
        }
    }

    /// Box `id` changed: take the texts, grow a repeating list, refresh the values.
    pub(super) fn fnargs_changed(&mut self, id: &str) {
        if let Some(i) = BOXES.iter().position(|b| *b == id) {
            if let Some(st) = self.fnargs.as_mut() {
                st.focus = st.top + i;
            }
        }
        let before = self.fnargs.as_ref().map_or(0, |s| s.args.len());
        self.fnargs_take();
        if self.fnargs.as_ref().map_or(0, |s| s.args.len()) == before {
            self.fnargs_values();
        } else {
            self.fnargs_show();
        }
    }

    /// Draw the argument rows' labels and values, the result, description and help.
    pub(super) fn paint_fnargs(&self, f: &Form, id: &str, r: [f32; 4], p: &mut dyn Painter) {
        let Some(st) = self.fnargs.as_ref() else {
            return;
        };
        let k = f.scale_k();
        let font = f.font();
        let bold = Font {
            weight: 700,
            ..font
        };
        let ink = Rgba(0, 0, 0, 255);
        let grey = Rgba(0xA0, 0xA0, 0xA0, 255);
        let line = font.size * 1.33;
        let (ox, oy) = f.origin();
        let at = |x: f32, y: f32| (ox + x * k, oy + y * k);
        match id {
            "rows" => {
                for i in 0..ROWS {
                    let a = st.top + i;
                    if a >= st.args.len() {
                        break;
                    }
                    let y = ROW0 + ROW_H * i as f32 + (27.0 - line / k) / 2.0;
                    let name = capitalized(&st.param_name(a));
                    let req = st.required(a);
                    let w = f.width(&name) * if req { 1.06 } else { 1.0 };
                    let (lx, ly) = at(205.0, y);
                    p.text_font(
                        &name,
                        (lx - w).round(),
                        ly.round(),
                        if req { &bold } else { &font },
                        ink,
                    );
                    let (ex, ey) = at(521.0, y);
                    p.text_font("=", ex.round(), ey.round(), &font, ink);
                    let (vx, vy) = at(543.0, y);
                    match st.values.get(a).cloned().flatten() {
                        Some(v) => p.text_font(&clip(&v, 52), vx.round(), vy.round(), &font, ink),
                        None => p.text_font(
                            &self.t(
                                &format!("sheetFn.type_{}", type_hint(&st.param_name(a))),
                                type_hint(&st.param_name(a)),
                            ),
                            vx.round(),
                            vy.round(),
                            &bold,
                            grey,
                        ),
                    }
                }
            }
            "info" => {
                // "= result" under the frame, the description, the focused argument's help.
                let (ex, ey) = at(521.0, 236.0);
                p.text_font("=", ex.round(), ey.round(), &font, ink);
                if let Some(v) = &st.result {
                    let (vx, _) = at(543.0, 0.0);
                    p.text_font(&clip(v, 52), vx.round(), ey.round(), &font, ink);
                }
                let desc = self.function_description(&st.name);
                let (dx, dy) = at(18.0, 257.0);
                p.text_font(&desc, dx.round(), dy.round(), &font, ink);
                let label = format!("{}:", capitalized(&st.param_name(st.focus)));
                let (hx, hy) = at(313.0, 284.0);
                p.text_font(
                    &label,
                    (hx - f.width(&label) * 1.06).round(),
                    hy.round(),
                    &bold,
                    ink,
                );
                let help = self.fnargs_help(st);
                let (tx, _) = at(326.0, 0.0);
                for (i, l) in wrap(&help, 88).iter().enumerate() {
                    p.text_font(l, tx.round(), (hy + i as f32 * line).round(), &font, ink);
                }
            }
            "rule" => p.fill_rect(r[0], r[1], r[2], 1.0, Rgba(0xDC, 0xDC, 0xDC, 255)),
            "result" => {
                let label = self.t("sheetFn.formulaResult", "Formula result =");
                let (x, y) = at(18.0, 371.0);
                p.text_font(&label, x.round(), y.round(), &font, ink);
                if let Some(v) = &st.result {
                    let (vx, _) = at(128.0, 0.0);
                    p.text_font(&clip(v, 60), vx.round(), y.round(), &font, ink);
                }
            }
            _ if id.starts_with("pick") => super::ttc_impl::paint_pick_button(p, r, k),
            _ => {}
        }
    }

    /// Help line of the focused argument (our own wording from its name and kind).
    pub(super) fn fnargs_help(&self, st: &FnArgs) -> String {
        let name = st.param_name(st.focus);
        // Our own help for the argument: of this function first, then of any argument
        // with that name (`sheetFnArg.SUM_number`, `sheetFnArg.number`).
        let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
        for key in [
            format!("sheetFnArg.{}_{base}", st.name.replace('.', "_")),
            format!("sheetFnArg.{base}"),
        ] {
            let s = self.t(&key, "");
            if !s.is_empty() {
                let optional = if st.required(st.focus) {
                    String::new()
                } else {
                    format!(" {}", self.t("sheetFn.optional", "(Optional.)"))
                };
                return format!("{s}{optional}");
            }
        }
        let kind = self.t(
            &format!("sheetFn.type_{}", type_hint(&name)),
            type_hint(&name),
        );
        let key = if st.required(st.focus) {
            "sheetFn.argRequired"
        } else {
            "sheetFn.argOptional"
        };
        let fallback = if st.required(st.focus) {
            "is required: a {kind} value, a cell reference or a formula."
        } else {
            "is optional: a {kind} value, a cell reference or a formula."
        };
        format!("{name} {}", self.t(key, fallback).replace("{kind}", &kind))
    }
}

/// `number1` → `Number1`.
fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// At most `n` characters (with an ellipsis).
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(n - 1).collect();
        out.push('…');
        out
    }
}

/// Words wrapped at `n` characters a line.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > n {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_and_calls_parse() {
        let (p, r) = parse_signature("number1, [number2], …");
        assert_eq!(p.len(), 1);
        assert_eq!(r, Some(vec!["number".to_owned()]));
        let st = FnArgs {
            params: p,
            repeat: r,
            ..FnArgs::default()
        };
        assert_eq!(
            (st.param_name(0), st.param_name(3)),
            ("number1".into(), "number4".into())
        );
        let (p, r) = parse_signature("test, value_if_true, [value_if_false]");
        assert_eq!((p.len(), r), (3, None));
        assert!(p[0].name == "test" && !p[0].optional && p[2].optional);
        assert_eq!(
            split_call("=SUM(D2:D4, IF(A1>0,1,2), \"a,b\")"),
            Some((
                "SUM".into(),
                vec!["D2:D4".into(), "IF(A1>0,1,2)".into(), "\"a,b\"".into()]
            ))
        );
        assert_eq!(split_call("=A1+SUM(B1)"), None);
        assert_eq!(split_call("=NOW()"), Some(("NOW".into(), vec![])));
    }
}
