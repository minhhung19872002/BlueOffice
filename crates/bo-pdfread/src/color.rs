//! Colour spaces (ISO 32000-1, §8.6): device spaces, ICCBased (by component count / its
//! alternate), CalGray / CalRGB / Lab (approximated), Indexed, Separation / DeviceN (through
//! their tint transform), Pattern.
use crate::filters;
use crate::function::Function;
use lopdf::{Document, Object};
use std::sync::Arc;

/// A colour space.
#[derive(Debug, Clone)]
pub(crate) enum ColorSpace {
    Gray,
    Rgb,
    Cmyk,
    Lab,
    Indexed {
        base: Arc<ColorSpace>,
        hival: usize,
        lookup: Vec<u8>,
    },
    /// Separation (one input) and DeviceN; `None` tint transform draws grey.
    Tint {
        inputs: usize,
        alt: Arc<ColorSpace>,
        func: Option<Function>,
        /// `/None` separation: paints nothing.
        none: bool,
    },
    Pattern(Option<Arc<ColorSpace>>),
}

fn resolve<'a>(doc: &'a Document, o: &'a Object) -> &'a Object {
    doc.dereference(o).map_or(o, |(_, o)| o)
}

impl ColorSpace {
    /// Parse a colour space object; `named` looks up names in the resources' `/ColorSpace`.
    pub(crate) fn parse(
        doc: &Document,
        o: &Object,
        named: &dyn Fn(&[u8]) -> Option<Object>,
        depth: u32,
    ) -> Option<Self> {
        if depth > 8 {
            return None;
        }
        let o = resolve(doc, o);
        match o {
            Object::Name(n) => match n.as_slice() {
                b"DeviceGray" | b"G" | b"CalGray" => Some(Self::Gray),
                b"DeviceRGB" | b"RGB" | b"CalRGB" => Some(Self::Rgb),
                b"DeviceCMYK" | b"CMYK" => Some(Self::Cmyk),
                b"Pattern" => Some(Self::Pattern(None)),
                b"Lab" => Some(Self::Lab),
                other => {
                    let found = named(other)?;
                    Self::parse(doc, &found, named, depth + 1)
                }
            },
            Object::Array(a) => {
                let kind = resolve(doc, a.first()?).as_name().ok()?;
                match kind {
                    b"DeviceGray" | b"CalGray" | b"G" => Some(Self::Gray),
                    b"DeviceRGB" | b"CalRGB" | b"RGB" => Some(Self::Rgb),
                    b"DeviceCMYK" | b"CMYK" => Some(Self::Cmyk),
                    b"Lab" => Some(Self::Lab),
                    b"ICCBased" => {
                        let s = resolve(doc, a.get(1)?).as_stream().ok()?;
                        if let Some(alt) = s
                            .dict
                            .get(b"Alternate")
                            .ok()
                            .and_then(|alt| Self::parse(doc, alt, named, depth + 1))
                        {
                            return Some(alt);
                        }
                        let n = s.dict.get(b"N").ok().and_then(|o| o.as_i64().ok());
                        Some(match n {
                            Some(1) => Self::Gray,
                            Some(4) => Self::Cmyk,
                            _ => Self::Rgb,
                        })
                    }
                    b"Indexed" | b"I" => {
                        let base = Self::parse(doc, a.get(1)?, named, depth + 1)?;
                        let hival = resolve(doc, a.get(2)?).as_i64().ok()?.clamp(0, 255) as usize;
                        let lookup = match resolve(doc, a.get(3)?) {
                            Object::String(s, _) => s.clone(),
                            Object::Stream(s) => filters::stream_bytes(doc, s).ok()?,
                            _ => return None,
                        };
                        Some(Self::Indexed {
                            base: Arc::new(base),
                            hival,
                            lookup,
                        })
                    }
                    b"Separation" | b"DeviceN" => {
                        let names = resolve(doc, a.get(1)?);
                        let (inputs, none) = match names {
                            Object::Array(n) => (n.len().max(1), false),
                            Object::Name(n) => (1, n == b"None"),
                            _ => (1, false),
                        };
                        let func = a.get(3).and_then(|f| Function::parse(doc, f, 0));
                        // An alternate space that cannot be read paints nothing (measured on
                        // the reference reader).
                        let Some(alt) =
                            a.get(2).and_then(|o| Self::parse(doc, o, named, depth + 1))
                        else {
                            return Some(Self::Tint {
                                inputs,
                                alt: Arc::new(Self::Gray),
                                func,
                                none: true,
                            });
                        };
                        // §8.6.6.4: the tint transform maps the tints to the alternate space's
                        // components. A missing function or one with another number of outputs
                        // makes the space unusable: `parse` fails and `cs` keeps the previous
                        // space, as the reference does.
                        if !none && func.as_ref()?.outputs()? != alt.components() {
                            return None;
                        }
                        Some(Self::Tint {
                            inputs,
                            alt: Arc::new(alt),
                            func,
                            none,
                        })
                    }
                    b"Pattern" => Some(Self::Pattern(
                        a.get(1)
                            .and_then(|b| Self::parse(doc, b, named, depth + 1))
                            .map(Arc::new),
                    )),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Number of colour components.
    pub(crate) fn components(&self) -> usize {
        match self {
            Self::Gray | Self::Indexed { .. } => 1,
            Self::Rgb | Self::Lab => 3,
            Self::Cmyk => 4,
            Self::Tint { inputs, .. } => *inputs,
            Self::Pattern(base) => base.as_ref().map_or(0, |b| b.components()),
        }
    }

    /// Initial colour (§8.6.8 Table 74 `cs`).
    pub(crate) fn initial(&self) -> Vec<f32> {
        match self {
            Self::Cmyk => vec![0.0, 0.0, 0.0, 1.0],
            Self::Tint { inputs, .. } => vec![1.0; *inputs],
            Self::Lab => vec![0.0, 0.0, 0.0],
            other => vec![0.0; other.components()],
        }
    }

    /// Whether colours of this space are inks (CMYK, Separation, DeviceN): overprinting
    /// applies to them (§8.7.4.5).
    pub(crate) fn is_subtractive(&self) -> bool {
        match self {
            Self::Cmyk | Self::Tint { .. } => true,
            Self::Indexed { base, .. } => base.is_subtractive(),
            _ => false,
        }
    }

    /// Whether painting in this space draws nothing (`/Separation /None`).
    pub(crate) fn paints_nothing(&self) -> bool {
        matches!(self, Self::Tint { none: true, .. })
    }

    /// RGB in 0..=1 of a colour value.
    pub(crate) fn rgb(&self, c: &[f32]) -> [f32; 3] {
        let g = |i: usize| c.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        match self {
            Self::Gray => [g(0); 3],
            Self::Rgb => [g(0), g(1), g(2)],
            Self::Cmyk => cmyk_to_rgb(g(0), g(1), g(2), g(3)),
            Self::Lab => {
                let l = c.first().copied().unwrap_or(0.0);
                let a = c.get(1).copied().unwrap_or(0.0);
                let b = c.get(2).copied().unwrap_or(0.0);
                lab_to_rgb(l, a, b)
            }
            Self::Indexed {
                base,
                hival,
                lookup,
            } => {
                let i = (c.first().copied().unwrap_or(0.0).round().max(0.0) as usize).min(*hival);
                let n = base.components();
                let vals: Vec<f32> = (0..n)
                    .map(|k| f32::from(lookup.get(i * n + k).copied().unwrap_or(0)) / 255.0)
                    .collect();
                base.rgb(&vals)
            }
            Self::Tint { alt, func, .. } => func
                .as_ref()
                .map_or_else(|| [1.0 - g(0); 3], |f| alt.rgb(&f.eval(c))),
            Self::Pattern(base) => base.as_ref().map_or([0.0; 3], |b| b.rgb(c)),
        }
    }
}

pub(crate) use crate::cmyk::cmyk_to_rgb;

fn lab_to_rgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    // CIE L*a*b* (D50) → XYZ → sRGB.
    let fy = (l + 16.0) / 116.0;
    let fx = fy + a / 500.0;
    let fz = fy - b / 200.0;
    let f = |t: f32| {
        if t > 6.0 / 29.0 {
            t * t * t
        } else {
            3.0 * (6.0_f32 / 29.0).powi(2) * (t - 4.0 / 29.0)
        }
    };
    let (x, y, z) = (0.9642 * f(fx), f(fy), 0.8249 * f(fz));
    let r = 3.1339 * x - 1.6169 * y - 0.4906 * z;
    let g = -0.9788 * x + 1.9161 * y + 0.0335 * z;
    let bl = 0.0719 * x - 0.2290 * y + 1.4052 * z;
    let gamma = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    [gamma(r), gamma(g), gamma(bl)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_and_indexed_spaces() {
        let doc = Document::new();
        let none = |_: &[u8]| None;
        let cs = ColorSpace::parse(
            &doc,
            &Object::Array(vec![
                Object::Name(b"Indexed".to_vec()),
                Object::Name(b"DeviceRGB".to_vec()),
                1.into(),
                Object::string_literal(vec![255, 0, 0, 0, 0, 255]),
            ]),
            &none,
            0,
        )
        .expect("indexed");
        assert_eq!(cs.rgb(&[1.0]), [0.0, 0.0, 1.0]);
        // Pure K is the reference's rich-black-less ink black (36, 32, 33).
        let k = ColorSpace::Cmyk
            .rgb(&[0.0, 0.0, 0.0, 1.0])
            .map(|v| (v * 255.0).round() as u8);
        assert_eq!(k, [36, 32, 33]);
        assert_eq!(ColorSpace::Cmyk.initial(), vec![0.0, 0.0, 0.0, 1.0]);
    }
}
