//! PDF functions (ISO 32000-1, §7.10): sampled (type 0), exponential (2), stitching (3) and
//! PostScript calculator (4) functions, used by shadings and Separation / DeviceN colours.
use crate::filters;
use lopdf::{Dictionary, Document, Object};

/// A parsed function.
#[derive(Debug, Clone)]
pub(crate) enum Function {
    Sampled {
        domain: Vec<f32>,
        range: Vec<f32>,
        size: Vec<usize>,
        bps: u32,
        encode: Vec<f32>,
        decode: Vec<f32>,
        samples: Vec<u8>,
    },
    Exponential {
        domain: [f32; 2],
        c0: Vec<f32>,
        c1: Vec<f32>,
        n: f32,
    },
    Stitching {
        domain: [f32; 2],
        functions: Vec<Function>,
        bounds: Vec<f32>,
        encode: Vec<f32>,
    },
    PostScript {
        domain: Vec<f32>,
        range: Vec<f32>,
        code: Vec<Ps>,
    },
    /// Several 1-in functions whose outputs are concatenated (shading `/Function` arrays).
    Array(Vec<Function>),
}

/// PostScript calculator instruction.
#[derive(Debug, Clone)]
pub(crate) enum Ps {
    Num(f32),
    Op(String),
    If(Vec<Ps>),
    IfElse(Vec<Ps>, Vec<Ps>),
}

fn nums(doc: &Document, o: Option<&Object>) -> Vec<f32> {
    let Some(o) = o else { return Vec::new() };
    let o = doc.dereference(o).map_or(o, |(_, o)| o);
    match o {
        Object::Array(a) => a
            .iter()
            .filter_map(|v| {
                let v = doc.dereference(v).map_or(v, |(_, o)| o);
                v.as_float().ok()
            })
            .collect(),
        _ => Vec::new(),
    }
}

impl Function {
    /// Parse a function object (dictionary or stream), or an array of them.
    pub(crate) fn parse(doc: &Document, o: &Object, depth: u32) -> Option<Self> {
        if depth > 8 {
            return None;
        }
        let o = doc.dereference(o).map_or(o, |(_, o)| o);
        if let Object::Array(a) = o {
            let fs: Option<Vec<Function>> =
                a.iter().map(|f| Self::parse(doc, f, depth + 1)).collect();
            return fs.map(Function::Array);
        }
        let (dict, data): (&Dictionary, Option<Vec<u8>>) = match o {
            Object::Dictionary(d) => (d, None),
            Object::Stream(s) => (&s.dict, filters::stream_bytes(doc, s).ok()),
            _ => return None,
        };
        let kind = dict.get(b"FunctionType").ok()?.as_i64().ok()?;
        let domain = nums(doc, dict.get(b"Domain").ok());
        let d2 = [
            domain.first().copied().unwrap_or(0.0),
            domain.get(1).copied().unwrap_or(1.0),
        ];
        match kind {
            0 => {
                let size: Vec<usize> = nums(doc, dict.get(b"Size").ok())
                    .iter()
                    .map(|v| (*v as usize).clamp(1, 65536))
                    .collect();
                let range = nums(doc, dict.get(b"Range").ok());
                let mut encode = nums(doc, dict.get(b"Encode").ok());
                if encode.len() < size.len() * 2 {
                    encode = size.iter().flat_map(|s| [0.0, (*s as f32 - 1.0)]).collect();
                }
                let mut decode = nums(doc, dict.get(b"Decode").ok());
                if decode.len() < range.len() {
                    decode.clone_from(&range);
                }
                let bps = dict
                    .get(b"BitsPerSample")
                    .ok()
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(8)
                    .clamp(1, 32) as u32;
                if size.is_empty() || range.len() < 2 {
                    return None;
                }
                let samples = data?;
                // §7.10.2: the stream holds Size[0] × … × Size[m-1] × n samples; a short table
                // makes the function unusable (the reference then rejects the colour space).
                let needed = size
                    .iter()
                    .try_fold(range.len() / 2 * bps as usize, |acc, s| acc.checked_mul(*s));
                if needed.is_none_or(|bits| samples.len().saturating_mul(8) < bits) {
                    return None;
                }
                Some(Function::Sampled {
                    domain,
                    range,
                    size,
                    bps,
                    encode,
                    decode,
                    samples,
                })
            }
            2 => {
                let c0 = nums(doc, dict.get(b"C0").ok());
                let c1 = nums(doc, dict.get(b"C1").ok());
                Some(Function::Exponential {
                    domain: d2,
                    c0: if c0.is_empty() { vec![0.0] } else { c0 },
                    c1: if c1.is_empty() { vec![1.0] } else { c1 },
                    n: dict
                        .get(b"N")
                        .ok()
                        .and_then(|o| o.as_float().ok())
                        .unwrap_or(1.0),
                })
            }
            3 => {
                let fs = dict.get(b"Functions").ok()?;
                let fs = doc.dereference(fs).map_or(fs, |(_, o)| o).as_array().ok()?;
                let functions: Option<Vec<Function>> =
                    fs.iter().map(|f| Self::parse(doc, f, depth + 1)).collect();
                Some(Function::Stitching {
                    domain: d2,
                    functions: functions?,
                    bounds: nums(doc, dict.get(b"Bounds").ok()),
                    encode: nums(doc, dict.get(b"Encode").ok()),
                })
            }
            4 => {
                let code = parse_ps(&data?);
                Some(Function::PostScript {
                    domain,
                    range: nums(doc, dict.get(b"Range").ok()),
                    code,
                })
            }
            _ => None,
        }
    }

    /// Number of outputs, when known from the dictionary (`Range`, `C0`, the first stitched
    /// function); `None` for an empty stitching function.
    pub(crate) fn outputs(&self) -> Option<usize> {
        match self {
            Function::Sampled { range, .. } | Function::PostScript { range, .. } => {
                Some(range.len() / 2)
            }
            Function::Exponential { c0, .. } => Some(c0.len()),
            Function::Stitching { functions, .. } => functions.first()?.outputs(),
            Function::Array(fs) => fs.iter().map(Function::outputs).sum(),
        }
    }

    /// Evaluate at `x`.
    pub(crate) fn eval(&self, x: &[f32]) -> Vec<f32> {
        match self {
            Function::Array(fs) => fs.iter().flat_map(|f| f.eval(x)).collect(),
            Function::Exponential { domain, c0, c1, n } => {
                let t = x
                    .first()
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(domain[0], domain[1]);
                let tn = if (*n - 1.0).abs() < f32::EPSILON {
                    t
                } else {
                    t.max(0.0).powf(*n)
                };
                c0.iter()
                    .zip(c1.iter())
                    .map(|(a, b)| a + tn * (b - a))
                    .collect()
            }
            Function::Stitching {
                domain,
                functions,
                bounds,
                encode,
            } => {
                let t = x
                    .first()
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(domain[0], domain[1]);
                let k = bounds.iter().take_while(|b| t >= **b).count();
                let k = k.min(functions.len().saturating_sub(1));
                let lo = if k == 0 { domain[0] } else { bounds[k - 1] };
                let hi = bounds.get(k).copied().unwrap_or(domain[1]);
                let (e0, e1) = (
                    encode.get(2 * k).copied().unwrap_or(0.0),
                    encode.get(2 * k + 1).copied().unwrap_or(1.0),
                );
                let u = if (hi - lo).abs() < 1e-9 {
                    e0
                } else {
                    e0 + (t - lo) / (hi - lo) * (e1 - e0)
                };
                functions.get(k).map_or_else(Vec::new, |f| f.eval(&[u]))
            }
            Function::Sampled {
                domain,
                range,
                size,
                bps,
                encode,
                decode,
                samples,
            } => {
                // Nearest sample along each input (multilinear interpolation is not needed
                // for colour tints at screen resolution).
                let outputs = range.len() / 2;
                let mut index = 0_usize;
                let mut stride = 1_usize;
                for (i, s) in size.iter().enumerate() {
                    let d0 = domain.get(2 * i).copied().unwrap_or(0.0);
                    let d1 = domain.get(2 * i + 1).copied().unwrap_or(1.0);
                    let xi = x
                        .get(i)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(d0.min(d1), d0.max(d1));
                    let e = if (d1 - d0).abs() < 1e-9 {
                        encode[2 * i]
                    } else {
                        encode[2 * i] + (xi - d0) / (d1 - d0) * (encode[2 * i + 1] - encode[2 * i])
                    };
                    let e = (e.round().max(0.0) as usize).min(s - 1);
                    index += e * stride;
                    stride *= s;
                }
                let max = if *bps >= 32 {
                    u32::MAX as f32
                } else {
                    ((1_u64 << bps) - 1) as f32
                };
                (0..outputs)
                    .map(|j| {
                        let bit = (index * outputs + j) * (*bps as usize);
                        let v = read_bits(samples, bit, *bps) as f32 / max;
                        let (d0, d1) = (decode[2 * j], decode[2 * j + 1]);
                        (d0 + v * (d1 - d0)).clamp(
                            range[2 * j].min(range[2 * j + 1]),
                            range[2 * j].max(range[2 * j + 1]),
                        )
                    })
                    .collect()
            }
            Function::PostScript {
                domain,
                range,
                code,
            } => {
                let mut stack: Vec<f32> = x
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let (d0, d1) = (
                            domain.get(2 * i).copied().unwrap_or(0.0),
                            domain.get(2 * i + 1).copied().unwrap_or(1.0),
                        );
                        v.clamp(d0.min(d1), d0.max(d1))
                    })
                    .collect();
                let mut budget = 100_000_u32;
                run_ps(code, &mut stack, &mut budget, 0);
                let n = range.len() / 2;
                // §7.10.5: the program leaves n results. Measured on the reference reader:
                // fewer paint all components 0 (white in CMYK, black in RGB); of more, the
                // top n count.
                if stack.len() < n {
                    return vec![0.0; n];
                }
                stack[stack.len() - n..]
                    .iter()
                    .enumerate()
                    .map(|(i, v)| v.clamp(range[2 * i], range[2 * i + 1]))
                    .collect()
            }
        }
    }
}

fn read_bits(data: &[u8], bit: usize, n: u32) -> u32 {
    let mut v = 0_u32;
    for k in 0..n as usize {
        let b = bit + k;
        let byte = data.get(b / 8).copied().unwrap_or(0);
        v = v << 1 | u32::from(byte >> (7 - (b % 8)) & 1);
    }
    v
}

fn parse_ps(data: &[u8]) -> Vec<Ps> {
    let toks: Vec<String> = {
        let mut out = Vec::new();
        let mut cur = String::new();
        for &c in data {
            match c {
                b'{' | b'}' => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    out.push((c as char).to_string());
                }
                c if c.is_ascii_whitespace() => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c as char),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    };
    let mut pos = 0;
    // The program is one outer procedure.
    if toks.first().map(String::as_str) == Some("{") {
        pos = 1;
    }
    parse_block(&toks, &mut pos, 0)
}

fn parse_block(t: &[String], pos: &mut usize, depth: u32) -> Vec<Ps> {
    let mut out = Vec::new();
    while *pos < t.len() {
        let tok = &t[*pos];
        *pos += 1;
        match tok.as_str() {
            "}" => break,
            "{" if depth < 16 => {
                let a = parse_block(t, pos, depth + 1);
                if t.get(*pos).map(String::as_str) == Some("{") {
                    *pos += 1;
                    let b = parse_block(t, pos, depth + 1);
                    if t.get(*pos).map(String::as_str) == Some("ifelse") {
                        *pos += 1;
                    }
                    out.push(Ps::IfElse(a, b));
                } else {
                    if t.get(*pos).map(String::as_str) == Some("if") {
                        *pos += 1;
                    }
                    out.push(Ps::If(a));
                }
            }
            s => match s.parse::<f32>() {
                Ok(v) => out.push(Ps::Num(v)),
                Err(_) => out.push(Ps::Op(s.to_owned())),
            },
        }
    }
    out
}

#[allow(clippy::too_many_lines)]
fn run_ps(code: &[Ps], st: &mut Vec<f32>, budget: &mut u32, depth: u32) {
    if depth > 16 {
        return;
    }
    for ins in code {
        if *budget == 0 || st.len() > 1000 {
            return;
        }
        *budget -= 1;
        match ins {
            Ps::Num(v) => st.push(*v),
            Ps::If(a) => {
                if st.pop().unwrap_or(0.0) != 0.0 {
                    run_ps(a, st, budget, depth + 1);
                }
            }
            Ps::IfElse(a, b) => {
                if st.pop().unwrap_or(0.0) == 0.0 {
                    run_ps(b, st, budget, depth + 1);
                } else {
                    run_ps(a, st, budget, depth + 1);
                }
            }
            Ps::Op(op) => {
                let b = |v: bool| if v { 1.0 } else { 0.0 };
                macro_rules! un {
                    ($f:expr) => {{
                        let a = st.pop().unwrap_or(0.0);
                        st.push($f(a));
                    }};
                }
                macro_rules! bin {
                    ($f:expr) => {{
                        let y = st.pop().unwrap_or(0.0);
                        let x = st.pop().unwrap_or(0.0);
                        st.push($f(x, y));
                    }};
                }
                match op.as_str() {
                    "add" => bin!(|x, y| x + y),
                    "sub" => bin!(|x, y| x - y),
                    "mul" => bin!(|x, y| x * y),
                    "div" => bin!(|x: f32, y: f32| if y == 0.0 { 0.0 } else { x / y }),
                    "idiv" => bin!(|x: f32, y: f32| if y as i64 == 0 {
                        0.0
                    } else {
                        (x as i64 / y as i64) as f32
                    }),
                    "mod" => bin!(|x: f32, y: f32| if y as i64 == 0 {
                        0.0
                    } else {
                        (x as i64 % y as i64) as f32
                    }),
                    "exp" => bin!(|x: f32, y: f32| x.powf(y)),
                    "atan" => bin!(|x: f32, y: f32| {
                        let d = x.atan2(y).to_degrees();
                        if d < 0.0 {
                            d + 360.0
                        } else {
                            d
                        }
                    }),
                    "eq" => bin!(|x, y| b(x == y)),
                    "ne" => bin!(|x, y| b(x != y)),
                    "gt" => bin!(|x, y| b(x > y)),
                    "ge" => bin!(|x, y| b(x >= y)),
                    "lt" => bin!(|x, y| b(x < y)),
                    "le" => bin!(|x, y| b(x <= y)),
                    "and" => bin!(|x: f32, y: f32| ((x as i64) & (y as i64)) as f32),
                    "or" => bin!(|x: f32, y: f32| ((x as i64) | (y as i64)) as f32),
                    "xor" => bin!(|x: f32, y: f32| ((x as i64) ^ (y as i64)) as f32),
                    "bitshift" => bin!(|x: f32, y: f32| {
                        let s = y as i64;
                        if s >= 0 {
                            ((x as i64) << s.min(31)) as f32
                        } else {
                            ((x as i64) >> (-s).min(31)) as f32
                        }
                    }),
                    "abs" => un!(f32::abs),
                    "neg" => un!(|a: f32| -a),
                    "ceiling" => un!(f32::ceil),
                    "floor" => un!(f32::floor),
                    "round" => un!(f32::round),
                    "truncate" => un!(f32::trunc),
                    "sqrt" => un!(|a: f32| a.max(0.0).sqrt()),
                    "sin" => un!(|a: f32| a.to_radians().sin()),
                    "cos" => un!(|a: f32| a.to_radians().cos()),
                    "ln" => un!(|a: f32| if a > 0.0 { a.ln() } else { 0.0 }),
                    "log" => un!(|a: f32| if a > 0.0 { a.log10() } else { 0.0 }),
                    "cvi" => un!(f32::trunc),
                    "cvr" => {}
                    "not" => un!(|a: f32| if a == 0.0 { 1.0 } else { 0.0 }),
                    "true" => st.push(1.0),
                    "false" => st.push(0.0),
                    "pop" => {
                        st.pop();
                    }
                    "dup" => {
                        let a = st.last().copied().unwrap_or(0.0);
                        st.push(a);
                    }
                    "exch" => {
                        let n = st.len();
                        if n >= 2 {
                            st.swap(n - 1, n - 2);
                        }
                    }
                    "copy" => {
                        let n = st.pop().unwrap_or(0.0).max(0.0) as usize;
                        let len = st.len();
                        if n <= len {
                            let tail: Vec<f32> = st[len - n..].to_vec();
                            st.extend(tail);
                        }
                    }
                    "index" => {
                        let n = st.pop().unwrap_or(0.0).max(0.0) as usize;
                        let len = st.len();
                        let v = if n < len { st[len - 1 - n] } else { 0.0 };
                        st.push(v);
                    }
                    "roll" => {
                        let j = st.pop().unwrap_or(0.0) as i64;
                        let n = st.pop().unwrap_or(0.0).max(0.0) as usize;
                        let len = st.len();
                        if n > 0 && n <= len {
                            let s = &mut st[len - n..];
                            let k = j.rem_euclid(n as i64) as usize;
                            s.rotate_right(k);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_stitching_and_postscript() {
        let doc = Document::new();
        let mut d = Dictionary::new();
        d.set("FunctionType", 2);
        d.set("Domain", vec![0.into(), 1.into()]);
        d.set("C0", vec![0.into(), 0.into()]);
        d.set("C1", vec![1.into(), 0.5.into()]);
        d.set("N", 1);
        let f = Function::parse(&doc, &Object::Dictionary(d), 0).expect("type 2");
        assert_eq!(f.eval(&[0.5]), vec![0.5, 0.25]);

        let ps = Function::PostScript {
            domain: vec![0.0, 1.0],
            range: vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
            code: parse_ps(b"{ dup 0.5 gt { 1 exch sub } { } ifelse 0 0 3 -1 roll 0 }"),
        };
        assert_eq!(ps.eval(&[0.75]), vec![0.0, 0.0, 0.25, 0.0]);
        assert_eq!(ps.eval(&[0.25]), vec![0.0, 0.0, 0.25, 0.0]);
    }
}
