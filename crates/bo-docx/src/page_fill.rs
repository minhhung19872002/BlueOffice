//! Page background fill effects: the VML `v:background` (ECMA-376 Part 4, §19.1.2.2) with its
//! `v:fill` (§19.1.2.5) inside the page background (ECMA-376 Part 1, §17.2.1 background), as
//! desktop word processors write Design › Page Color › Fill Effects (gradient, texture, pattern,
//! picture).

use std::fmt::Write as _;

use bo_model::slide::XmlElement;
use bo_model::{Color, PageFill, PageFillKind};

use crate::shape::{vml_color, vml_fraction};

/// Attribute by local name whatever its prefix (`r:id`, `o:title`).
fn attr<'a>(el: &'a XmlElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some(local))
        .map(|(_, v)| v.as_str())
}

/// A `fill darken(n)` / `fill lighten(n)` colour formula of a one-colour gradient
/// (Part 4, §19.1.2.5 color2; n in 0..=255).
pub(crate) fn color_formula(fill: Color, expr: &str) -> Option<Color> {
    let e = expr.trim().strip_prefix("fill")?.trim();
    let (lighten, rest) = if let Some(r) = e.strip_prefix("darken(") {
        (false, r)
    } else {
        (true, e.strip_prefix("lighten(")?)
    };
    let n: f64 = rest.trim_end_matches(')').trim().parse().ok()?;
    let k = (n / 255.0).clamp(0.0, 1.0);
    let ch = |c: u8| -> u8 {
        let c = f64::from(c);
        let v = if lighten {
            255.0 - (255.0 - c) * k
        } else {
            c * k
        };
        v.round().clamp(0.0, 255.0) as u8
    };
    Some(Color {
        r: ch(fill.r),
        g: ch(fill.g),
        b: ch(fill.b),
    })
}

/// A pair of VML fractions `x,y` (`.5,.5`, `1,1`, `50%,0`) in thousandths.
fn pair(v: Option<&str>) -> (i32, i32) {
    let Some(v) = v else { return (0, 0) };
    let mut it = v.split(',').map(|s| {
        let s = s.trim();
        if s.is_empty() {
            0
        } else {
            vml_fraction(s).map_or(0, |f| (f * 1000.0).round() as i32)
        }
    });
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// The page fill of a §17.2.1 background element (parsed), with the colour of its
/// `v:background@fillcolor`; `target` resolves a relationship id to a part name.
/// `None` when it holds no fill effect (a plain page colour).
pub(crate) fn read(
    bg: &XmlElement,
    page_color: Option<Color>,
    target: impl Fn(&str) -> Option<String>,
) -> Option<(PageFill, Option<Color>)> {
    let vbg = bg.elements().find(|e| e.local() == "background")?;
    let fill = vbg.elements().find(|e| e.local() == "fill")?;
    let kind = match attr(fill, "type")? {
        "gradient" => PageFillKind::Gradient,
        "gradientRadial" | "gradientradial" => PageFillKind::GradientRadial,
        "tile" => PageFillKind::Tile,
        "pattern" => PageFillKind::Pattern,
        "frame" => PageFillKind::Frame,
        _ => return None,
    };
    let color = attr(vbg, "fillcolor").and_then(vml_color).or(page_color);
    let base = color.unwrap_or(Color::WHITE);
    let (color2, color2_expr) = match attr(fill, "color2") {
        Some(v) if v.trim_start().starts_with("fill") => {
            (color_formula(base, v), Some(v.trim().to_owned()))
        }
        Some(v) => (vml_color(v), None),
        None => (None, None),
    };
    let stops = attr(fill, "colors")
        .map(|v| {
            v.split(';')
                .filter_map(|s| {
                    let (pos, c) = s.trim().split_once(' ')?;
                    let p = vml_fraction(pos)?;
                    Some((
                        (p * 1000.0).round().clamp(0.0, 1000.0) as u32,
                        vml_color(c)?,
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let num = |k: &str| {
        attr(fill, k)
            .and_then(|v| v.trim().trim_end_matches("fd").parse::<f64>().ok())
            .map_or(0, |v| v.round() as i32)
    };
    let focus = attr(fill, "focus")
        .and_then(vml_fraction)
        .map_or(0, |f| (f * 100.0).round() as i32);
    Some((
        PageFill {
            kind,
            color2,
            color2_expr,
            angle: num("angle"),
            focus,
            focus_position: pair(attr(fill, "focusposition")),
            focus_size: pair(attr(fill, "focussize")),
            stops,
            method: attr(fill, "method").map(str::to_owned),
            image: attr(fill, "id").and_then(target),
            title: attr(fill, "title").map(str::to_owned),
            lock_aspect: attr(fill, "aspect").is_some_and(|a| a == "atLeast" || a == "atMost"),
        },
        color,
    ))
}

fn hex(c: Color) -> String {
    format!("#{}", c.to_hex().to_ascii_lowercase())
}

fn frac(v: i32) -> String {
    let f = f64::from(v) / 1000.0;
    let s = format!("{f:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.strip_prefix("0.")
            .map_or_else(|| s.to_owned(), |r| format!(".{r}"))
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The §17.2.1 background element of a fill effect over `color`; `rid` = the
/// relationship id of its picture.
pub(crate) fn write(color: Color, fill: &PageFill, rid: Option<&str>) -> String {
    const V_NS: &str = "urn:schemas-microsoft-com:vml";
    const O_NS: &str = "urn:schemas-microsoft-com:office:office";
    const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let mut f = String::from("<v:fill");
    if let Some(r) = rid {
        let _ = write!(f, " r:id=\"{r}\"");
    }
    if let Some(t) = &fill.title {
        let _ = write!(f, " o:title=\"{}\"", escape(t));
    }
    match (&fill.color2_expr, fill.color2) {
        (Some(e), _) => {
            let _ = write!(f, " color2=\"{}\"", escape(e));
        }
        (None, Some(c)) => {
            let _ = write!(f, " color2=\"{}\"", hex(c));
        }
        _ => {}
    }
    if !fill.stops.is_empty() {
        let list: Vec<String> = fill
            .stops
            .iter()
            .map(|(p, c)| {
                let p = i32::try_from(*p).unwrap_or(1000);
                format!("{} {}", frac(p), hex(*c))
            })
            .collect();
        let _ = write!(f, " colors=\"{}\"", list.join(";"));
    }
    if let Some(m) = &fill.method {
        let _ = write!(f, " method=\"{}\"", escape(m));
    }
    let gradient = matches!(
        fill.kind,
        PageFillKind::Gradient | PageFillKind::GradientRadial
    );
    if gradient {
        if fill.focus != 0 {
            let _ = write!(f, " focus=\"{}%\"", fill.focus);
        }
        if fill.angle != 0 {
            let _ = write!(f, " angle=\"{}\"", fill.angle);
        }
    }
    if fill.kind == PageFillKind::GradientRadial {
        let (x, y) = fill.focus_position;
        let _ = write!(f, " focusposition=\"{},{}\"", frac(x), frac(y));
        let (w, h) = fill.focus_size;
        if (w, h) == (0, 0) {
            f.push_str(" focussize=\"\"");
        } else {
            let _ = write!(f, " focussize=\"{},{}\"", frac(w), frac(h));
        }
    }
    if fill.lock_aspect {
        f.push_str(" aspect=\"atLeast\"");
    }
    let ty = match fill.kind {
        PageFillKind::Gradient => "gradient",
        PageFillKind::GradientRadial => "gradientRadial",
        PageFillKind::Tile => "tile",
        PageFillKind::Pattern => "pattern",
        PageFillKind::Frame => "frame",
    };
    let _ = write!(f, " type=\"{ty}\"");
    if fill.kind == PageFillKind::Frame {
        f.push_str(" recolor=\"t\"");
    }
    f.push_str("/>");
    format!(
        "<w:background w:color=\"{}\"><v:background xmlns:v=\"{V_NS}\" xmlns:o=\"{O_NS}\" xmlns:r=\"{R_NS}\" id=\"_x0000_s1025\" o:bwmode=\"white\" fillcolor=\"{}\" o:targetscreensize=\"1024,768\">{f}</v:background></w:background>",
        color.to_hex(),
        hex(color)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(xml: &str) -> XmlElement {
        bo_drawingml::dom::parse(xml.as_bytes(), "/word/document.xml").expect("xml")
    }

    #[test]
    fn reads_and_writes_gradients() {
        let x = r##"<w:background xmlns:w="w" xmlns:v="v" xmlns:o="o" xmlns:r="r" w:color="2070C0"><v:background id="_x0000_s1025" o:bwmode="white" fillcolor="#2070c0" o:targetscreensize="1024,768"><v:fill color2="fill darken(118)" method="linear sigma" focus="-50%" angle="-90" type="gradient"/></v:background></w:background>"##;
        let (f, c) = read(&parse(x), None, |_| None).expect("fill");
        assert_eq!(c, Color::from_hex("2070C0"));
        assert_eq!(f.kind, PageFillKind::Gradient);
        assert_eq!((f.angle, f.focus), (-90, -50));
        // darken(118): 0x20·118/255 = 15, 0x70·118/255 = 52, 0xC0·118/255 = 89.
        assert_eq!(
            f.color2,
            Some(Color {
                r: 15,
                g: 52,
                b: 89
            })
        );
        let w = write(Color::from_hex("2070C0").unwrap(), &f, None);
        let (g, _) = read(
            &parse(&w.replace("<w:background ", "<w:background xmlns:w=\"w\" ")),
            None,
            |_| None,
        )
        .expect("again");
        assert_eq!(g, f);
    }

    #[test]
    fn reads_radial_preset_and_picture() {
        let x = r##"<w:background xmlns:w="w" xmlns:v="v" xmlns:o="o" xmlns:r="r" w:color="FFFFFF"><v:background id="b" fillcolor="#5e9eff"><v:fill colors="0 #5e9eff;26214f #85c2ff;45875f #c4d6eb;1 #ffebfa" method="none" focus="100%" focusposition=".5,.5" focussize="" type="gradientRadial"/></v:background></w:background>"##;
        let (f, _) = read(&parse(x), None, |_| None).expect("fill");
        assert_eq!(f.kind, PageFillKind::GradientRadial);
        assert_eq!(f.focus_position, (500, 500));
        assert_eq!(f.stops.len(), 4);
        assert_eq!(f.stops[1].0, 400);
        let x = r##"<w:background xmlns:w="w" xmlns:v="v" xmlns:o="o" xmlns:r="r"><v:background id="b" fillcolor="#fff"><v:fill r:id="rId4" o:title="Canvas" type="tile"/></v:background></w:background>"##;
        let (f, _) =
            read(&parse(x), None, |id| Some(format!("/word/media/{id}.jpeg"))).expect("fill");
        assert_eq!(f.kind, PageFillKind::Tile);
        assert_eq!(f.image.as_deref(), Some("/word/media/rId4.jpeg"));
        assert_eq!(f.title.as_deref(), Some("Canvas"));
        // A plain colour has no fill effect.
        let x = r#"<w:background xmlns:w="w" w:color="FFF2CC"/>"#;
        assert!(read(&parse(x), None, |_| None).is_none());
    }

    #[test]
    fn color_formulas() {
        let c = Color::from_hex("808080").unwrap();
        assert_eq!(color_formula(c, "fill darken(255)"), Some(c));
        assert_eq!(
            color_formula(c, "fill lighten(0)"),
            Color::from_hex("FFFFFF")
        );
        assert_eq!(color_formula(c, "#123456"), None);
    }
}
