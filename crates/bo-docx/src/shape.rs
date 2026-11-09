//! Shapes and text boxes in WordprocessingML.
//!
//! * DrawingML: `wps:wsp` inside `w:drawing` ([MS-ODRAWXML] §2.3.10 wsp; shape
//!   properties ECMA-376 Part 1 §20.1.2.2.35 spPr, §20.1.2.2.37 style, body
//!   properties §21.1.2.1.1 bodyPr, text box content §17.17.1 txbxContent).
//! * VML fallback: `v:rect`, `v:roundrect`, `v:oval`, `v:shape` inside `w:pict`
//!   (ECMA-376 Part 4, §19.1) with `v:textbox` (§19.1.2.22) and `w10:wrap` (§19.3.2.3).
//!
//! Shapes are read into [`bo_model::Shape`] with colours resolved against the
//! document theme. An unchanged shape is written back from its source XML; an edited
//! or new one is written as a DrawingML `wps:wsp` drawing.

use bo_drawingml::color::{resolve, ColorContext};
use bo_drawingml::props::{parse_sp_pr, parse_style};
use bo_model::slide::{Fill, Geometry, Theme, XmlElement};
use bo_model::{
    fingerprint, Anchor, Color, Emu, Image, Shape, ShapeArt, ShapeLine, SignatureLine, TextAnchor,
    TextPath, Wrap, WrapText,
};

/// [MS-ODRAWXML] wordprocessingShape namespace (`wps`).
pub(crate) const WPS_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";

/// Fingerprint of what the writer generates for a shape (unchanged ⇒ source XML kept).
pub(crate) fn shape_fingerprint(img: &Image) -> u64 {
    fingerprint(&(img.width, img.height, &img.anchor, img.rotation, &img.shape))
}

/// First descendant (or `el` itself) with local name `local`.
pub(crate) fn find_local<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    if el.local() == local {
        return Some(el);
    }
    el.elements().find_map(|c| find_local(c, local))
}

fn child_local<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    el.elements().find(|c| c.local() == local)
}

fn rgb(c: bo_drawingml::Rgba) -> Color {
    let [r, g, b, _] = c.to_u8();
    Color { r, g, b }
}

/// Solid colour of a fill (first stop of a gradient); `None` for no fill.
fn fill_color(f: &Fill, ctx: &ColorContext<'_>) -> Option<Color> {
    match f {
        Fill::Solid(c) => Some(rgb(resolve(c, ctx))),
        Fill::Gradient(g) => g.stops.first().map(|s| rgb(resolve(&s.color, ctx))),
        Fill::Pattern(p) => p.fg.as_ref().map(|c| rgb(resolve(c, ctx))),
        Fill::None | Fill::Picture(_) | Fill::Group => None,
    }
}

/// `wps:wsp` → shape (geometry, fill, line, flips, text body properties).
pub(crate) fn shape_from_wsp(wsp: &XmlElement, theme: Option<&Theme>) -> Shape {
    let ctx = ColorContext {
        theme,
        ..ColorContext::default()
    };
    let sp = child_local(wsp, "spPr")
        .map(parse_sp_pr)
        .unwrap_or_default();
    let style = child_local(wsp, "style").map(parse_style);
    let mut s = Shape {
        preset: "rect".into(),
        insets: Shape::INSETS,
        ..Shape::default()
    };
    // §20.1.9.18 prstGeom; custom geometry (§20.1.9.8) is shown as its bounding rectangle.
    if let Some(Geometry::Preset { name, av }) = &sp.geometry {
        s.preset.clone_from(name);
        s.adjust = av
            .iter()
            .filter_map(|g| {
                let v = g.fmla.strip_prefix("val ")?.trim().parse::<i64>().ok()?;
                Some((g.name.clone(), v))
            })
            .collect();
    }
    if let Some(x) = &sp.xfrm {
        s.flip_h = x.flip_h.unwrap_or(false);
        s.flip_v = x.flip_v.unwrap_or(false);
    }
    // §20.1.4.2.10 fillRef / §20.1.4.2.19 lnRef: theme style matrix with the reference colour.
    let ref_color = |r: &bo_model::slide::StyleRef| -> Option<Color> {
        if r.idx == 0 {
            return None;
        }
        r.color.as_ref().map(|c| rgb(resolve(c, &ctx)))
    };
    s.fill = match &sp.fill {
        Some(f) => fill_color(f, &ctx),
        None => style.as_ref().and_then(|st| ref_color(&st.fill_ref)),
    };
    let style_line_w = |idx: u32| {
        theme
            .and_then(|t| t.formats.line_styles.get((idx as usize).checked_sub(1)?))
            .and_then(|l| l.w)
    };
    s.line = match &sp.line {
        Some(l) => {
            let color = match &l.fill {
                Some(f) => fill_color(f, &ctx),
                None => style.as_ref().and_then(|st| ref_color(&st.ln_ref)),
            };
            let w =
                l.w.or_else(|| style.as_ref().and_then(|st| style_line_w(st.ln_ref.idx)));
            color.map(|color| ShapeLine {
                width: Emu(w.unwrap_or(9525)),
                color,
                dash: l.dash.clone().filter(|d| d != "solid"),
            })
        }
        None => style.as_ref().and_then(|st| {
            ref_color(&st.ln_ref).map(|color| ShapeLine {
                width: Emu(style_line_w(st.ln_ref.idx).unwrap_or(12_700)),
                color,
                dash: None,
            })
        }),
    };
    // §21.1.2.1.1 bodyPr: insets and vertical anchoring.
    if let Some(b) = child_local(wsp, "bodyPr") {
        for (i, k) in ["lIns", "tIns", "rIns", "bIns"].iter().enumerate() {
            if let Some(v) = b.attr(k).and_then(|v| v.parse::<i64>().ok()) {
                s.insets[i] = Emu(v.max(0));
            }
        }
        s.anchor = match b.attr("anchor") {
            Some("ctr") => TextAnchor::Center,
            Some("b") => TextAnchor::Bottom,
            _ => TextAnchor::Top,
        };
    }
    s
}

/// A CSS length of a VML `style` attribute in EMU (ECMA-376 Part 4, §19.1.2.19:
/// pt, in, cm, mm, pc, px; no unit = px).
pub(crate) fn css_len(v: &str) -> Option<i64> {
    let v = v.trim();
    let split = v
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(v.len());
    let (num, unit) = v.split_at(split);
    let n: f64 = num.trim().parse().ok()?;
    let k = match unit {
        "pt" => 12_700.0,
        "in" => 914_400.0,
        "cm" => 360_000.0,
        "mm" => 36_000.0,
        "pc" => 152_400.0,
        "px" | "" => 9_525.0,
        "emu" => 1.0,
        _ => return None,
    };
    Some((n * k).round() as i64)
}

/// VML colour (`#RRGGBB`, `#RGB` or a colour name; a trailing `[n]` index is ignored).
fn vml_color(v: &str) -> Option<Color> {
    let v = v.split_whitespace().next()?;
    if let Some(h) = v.strip_prefix('#') {
        if h.len() == 3 {
            let e: String = h.chars().flat_map(|c| [c, c]).collect();
            return Color::from_hex(&e);
        }
        return Color::from_hex(h);
    }
    let x = bo_drawingml::color::preset_color(v)?;
    Some(Color {
        r: (x >> 16) as u8,
        g: (x >> 8) as u8,
        b: x as u8,
    })
}

fn vml_bool(v: Option<&str>, default: bool) -> bool {
    match v {
        Some("f" | "false" | "0") => false,
        Some(_) => true,
        None => default,
    }
}

/// Attribute by local name whatever its prefix (`r:id`, `o:title`).
fn attr_local<'a>(el: &'a XmlElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some(local) && k.contains(':'))
        .map(|(_, v)| v.as_str())
}

/// Attribute `local` with or without a namespace prefix (`o:signatureline`
/// qualifies some attributes and not others).
fn any_attr<'a>(el: &'a XmlElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some(local))
        .map(|(_, v)| v.as_str())
}

/// ECMA-376 Part 4, §19.2.2.30 signatureline (Office VML, `CT_SignatureLine`):
/// `id`, `provid`, `issignatureline`, `o:suggestedsigner`, `o:suggestedsigner2`,
/// `o:suggestedsigneremail`, `signinginstructions`, `allowcomments`, `showsigndate`.
pub(crate) fn signature_line(el: &XmlElement) -> SignatureLine {
    let text = |k: &str| any_attr(el, k).unwrap_or_default().to_owned();
    SignatureLine {
        id: text("id"),
        provider_id: text("provid"),
        is_signature_line: vml_bool(any_attr(el, "issignatureline"), true),
        suggested_signer: text("suggestedsigner"),
        suggested_signer2: text("suggestedsigner2"),
        suggested_signer_email: text("suggestedsigneremail"),
        signing_instructions: text("signinginstructions"),
        allow_comments: vml_bool(any_attr(el, "allowcomments"), false),
        show_sign_date: vml_bool(any_attr(el, "showsigndate"), true),
        signed: None,
    }
}

/// A VML fraction (ECMA-376 Part 4, §19.1.2.5 fill@opacity, §19.1.2.11 imagedata
/// gain / blacklevel): `0.5`, `.5`, `50%` or a 16.16 fixed value `32768f`.
fn vml_fraction(v: &str) -> Option<f64> {
    let v = v.trim();
    if let Some(f) = v.strip_suffix('f') {
        return f.trim().parse::<f64>().ok().map(|n| n / 65_536.0);
    }
    if let Some(p) = v.strip_suffix('%') {
        return p.trim().parse::<f64>().ok().map(|n| n / 100.0);
    }
    v.parse::<f64>().ok()
}

/// Text path of a VML shape (ECMA-376 Part 4, §19.1.2.23 textpath): its `string`
/// and the font of its CSS `style`.
fn vml_text_path(el: &XmlElement) -> Option<TextPath> {
    let tp = child_local(el, "textpath")?;
    if !vml_bool(tp.attr("on"), true) {
        return None;
    }
    let text = tp.attr("string")?.to_owned();
    let mut t = TextPath {
        text,
        ..TextPath::default()
    };
    for kv in tp.attr("style").unwrap_or("").split(';') {
        let Some((k, v)) = kv.split_once(':') else {
            continue;
        };
        let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
        match k.trim().to_ascii_lowercase().as_str() {
            "font-family" => v.clone_into(&mut t.font),
            "font-weight" => t.bold = v == "bold" || v.parse::<u32>().is_ok_and(|w| w >= 600),
            "font-style" => t.italic = v == "italic",
            _ => {}
        }
    }
    Some(t)
}

/// A VML shape inside `w:pict` → (shape, width, height, anchor, rotation in
/// 60 000ths of a degree). A picture fill (`v:imagedata`) is returned in
/// [`ShapeArt::picture`] as its relationship id; the caller resolves it.
pub(crate) fn vml_shape(pict: &XmlElement) -> Option<(Shape, Emu, Emu, Option<Anchor>, i32)> {
    let el = pict
        .elements()
        .find(|e| matches!(e.local(), "rect" | "roundrect" | "oval" | "shape"))?;
    let style: Vec<(String, String)> = el
        .attr("style")
        .unwrap_or("")
        .split(';')
        .filter_map(|kv| {
            let (k, v) = kv.split_once(':')?;
            Some((k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        })
        .collect();
    let get = |k: &str| style.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let w = get("width").and_then(css_len).unwrap_or(0);
    let h = get("height").and_then(css_len).unwrap_or(0);
    if w <= 0 || h <= 0 {
        return None;
    }
    let preset = match el.local() {
        "roundrect" => "roundRect",
        "oval" => "ellipse",
        _ => "rect",
    };
    let mut s = Shape {
        preset: preset.into(),
        insets: [Emu(91_440), Emu(45_720), Emu(91_440), Emu(45_720)],
        vml: true,
        ..Shape::default()
    };
    // §19.1.2.19 shape attributes: filled/fillcolor, stroked/strokecolor/strokeweight.
    s.fill = vml_bool(el.attr("filled"), true).then(|| {
        el.attr("fillcolor")
            .and_then(vml_color)
            .unwrap_or(Color::WHITE)
    });
    s.line = vml_bool(el.attr("stroked"), true).then(|| ShapeLine {
        width: Emu(el.attr("strokeweight").and_then(css_len).unwrap_or(9525)),
        color: el
            .attr("strokecolor")
            .and_then(vml_color)
            .unwrap_or(Color::BLACK),
        dash: None,
    });
    if let Some(tb) = child_local(el, "textbox") {
        // §19.1.2.22 textbox@inset: left, top, right, bottom.
        if let Some(ins) = tb.attr("inset") {
            for (i, part) in ins.split(',').take(4).enumerate() {
                if let Some(v) = css_len(part) {
                    s.insets[i] = Emu(v.max(0));
                }
            }
        }
        if let Some(st) = tb.attr("style") {
            if st.contains("v-text-anchor:middle") {
                s.anchor = TextAnchor::Center;
            } else if st.contains("v-text-anchor:bottom") {
                s.anchor = TextAnchor::Bottom;
            }
        }
    }
    let anchor = (get("position") == Some("absolute")).then(|| {
        let rel = |v: Option<&str>, text: &str| match v {
            Some("page") => "page".to_owned(),
            Some("margin") => "margin".to_owned(),
            _ => text.to_owned(),
        };
        // §19.3.2.3 w10:wrap type
        let wrap = child_local(el, "wrap").map_or(Wrap::None, |w| match w.attr("type") {
            Some("square") => Wrap::Square,
            Some("tight" | "through") => Wrap::Tight,
            Some("topAndBottom") => Wrap::TopAndBottom,
            _ => Wrap::None,
        });
        // Office VML extension: mso-position-horizontal / -vertical alignment
        // (left, center, right, inside, outside / top, center, bottom, …).
        let align = |v: Option<&str>| {
            v.filter(|a| *a != "absolute")
                .map(|a| a.trim().to_owned())
                .filter(|a| !a.is_empty())
        };
        Anchor {
            h_relative: rel(get("mso-position-horizontal-relative"), "column"),
            v_relative: rel(get("mso-position-vertical-relative"), "paragraph"),
            x: Emu(get("margin-left").and_then(css_len).unwrap_or(0)),
            y: Emu(get("margin-top").and_then(css_len).unwrap_or(0)),
            wrap,
            behind: get("z-index").is_some_and(|z| z.trim_start().starts_with('-')),
            wrap_text: WrapText::BothSides,
            h_align: align(get("mso-position-horizontal")),
            v_align: align(get("mso-position-vertical")),
            ..Anchor::default()
        }
    });
    // §19.1.2.19 shape style `rotation` (degrees clockwise; `fd` = 1/65 536 degree).
    let rotation = get("rotation")
        .and_then(|r| {
            let r = r.trim();
            match r.strip_suffix("fd") {
                Some(f) => f.trim().parse::<f64>().ok().map(|v| v / 65_536.0),
                None => r.parse::<f64>().ok(),
            }
        })
        .map_or(0, |deg| (deg.rem_euclid(360.0) * 60_000.0).round() as i32);
    // Text path (WordArt, text watermarks) and picture fill (picture watermarks).
    let text = vml_text_path(el);
    let image = child_local(el, "imagedata");
    if text.is_some() || image.is_some() {
        let id = el.attr("id").unwrap_or("").to_ascii_lowercase();
        let opacity = child_local(el, "fill")
            .and_then(|f| f.attr("opacity"))
            .and_then(vml_fraction)
            .map_or(1000, |o| (o.clamp(0.0, 1.0) * 1000.0).round() as u16);
        let mut art = ShapeArt {
            text,
            opacity,
            watermark: id.contains("watermark"),
            ..ShapeArt::default()
        };
        if let Some(img) = image {
            // §19.1.2.11 imagedata: a lowered gain with a raised black level washes
            // the picture out.
            let gain = img.attr("gain").and_then(vml_fraction).unwrap_or(1.0);
            let black = img.attr("blacklevel").and_then(vml_fraction).unwrap_or(0.0);
            art.washout = gain < 0.99 && black > 0.01;
            art.picture = attr_local(img, "id")
                .or_else(|| attr_local(img, "relid"))
                .map(str::to_owned);
            // The picture is the content: no box fill or outline.
            s.fill = None;
            s.line = None;
            art.signature_line = child_local(el, "signatureline")
                .map(signature_line)
                .map(Box::new);
        }
        if art.text.is_some() {
            // The fill paints the letters; there is no box.
            s.line = None;
        }
        s.insets = [Emu(0); 4];
        s.art = Some(Box::new(art));
    }
    Some((s, Emu(w), Emu(h), anchor, rotation))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `a:graphic` of a shape (`wps:wsp`) with the text box content `txbx` (already
/// serialized blocks; `None` = no text box).
pub(crate) fn shape_graphic(img: &Image, s: &Shape, id: u32, txbx: Option<&str>) -> String {
    const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    let (cx, cy) = (img.width.0, img.height.0);
    let mut x = String::new();
    // [MS-ODRAWXML] §2.3.10 wsp: cNvPr?, cNvSpPr, spPr, style?, txbx?, bodyPr.
    x.push_str(&format!(
        "<a:graphic xmlns:a=\"{A_NS}\"><a:graphicData uri=\"{WPS_NS}\"><wps:wsp xmlns:wps=\"{WPS_NS}\">"
    ));
    x.push_str(if txbx.is_some() {
        "<wps:cNvSpPr txBox=\"1\"/>"
    } else {
        "<wps:cNvSpPr/>"
    });
    // §20.1.7.6 xfrm
    let mut xf = String::from("<a:xfrm");
    if img.rotation != 0 {
        xf.push_str(&format!(" rot=\"{}\"", img.rotation));
    }
    if s.flip_h {
        xf.push_str(" flipH=\"1\"");
    }
    if s.flip_v {
        xf.push_str(" flipV=\"1\"");
    }
    xf.push_str(&format!(
        "><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>"
    ));
    // §20.1.9.18 prstGeom with §20.1.9.5 avLst
    let mut av = String::new();
    for (n, v) in &s.adjust {
        av.push_str(&format!(
            "<a:gd name=\"{}\" fmla=\"val {v}\"/>",
            xml_escape(n)
        ));
    }
    let fill = match s.fill {
        // §20.1.8.54 solidFill / §20.1.8.44 noFill
        Some(c) => format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            c.to_hex()
        ),
        None => "<a:noFill/>".to_owned(),
    };
    let line = match &s.line {
        // §20.1.2.2.24 ln, §20.1.8.48 prstDash
        Some(l) => format!(
            "<a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>{}</a:ln>",
            l.width.0,
            l.color.to_hex(),
            l.dash.as_ref().map_or(String::new(), |d| format!(
                "<a:prstDash val=\"{}\"/>",
                xml_escape(d)
            ))
        ),
        None => "<a:ln><a:noFill/></a:ln>".to_owned(),
    };
    x.push_str(&format!(
        "<wps:spPr>{xf}<a:prstGeom prst=\"{}\"><a:avLst>{av}</a:avLst></a:prstGeom>{fill}{line}</wps:spPr>",
        xml_escape(&s.preset)
    ));
    if let Some(t) = txbx {
        x.push_str(&format!(
            "<wps:txbx><w:txbxContent>{t}</w:txbxContent></wps:txbx>"
        ));
    }
    let anchor = match s.anchor {
        TextAnchor::Top => "t",
        TextAnchor::Center => "ctr",
        TextAnchor::Bottom => "b",
    };
    let [l, t, r, b] = s.insets.map(|e| e.0);
    x.push_str(&format!(
        "<wps:bodyPr rot=\"0\" vert=\"horz\" wrap=\"square\" lIns=\"{l}\" tIns=\"{t}\" rIns=\"{r}\" bIns=\"{b}\" anchor=\"{anchor}\" anchorCtr=\"0\"><a:noAutofit/></wps:bodyPr>"
    ));
    let _ = id;
    x.push_str("</wps:wsp></a:graphicData></a:graphic>");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_lengths_and_colors() {
        assert_eq!(css_len("12pt"), Some(152_400));
        assert_eq!(css_len("1in"), Some(914_400));
        assert_eq!(css_len("2.5cm"), Some(900_000));
        assert_eq!(css_len("10"), Some(95_250));
        assert_eq!(css_len("auto"), None);
        assert_eq!(vml_color("#f00"), Color::from_hex("FF0000"));
        assert_eq!(vml_color("#4472c4 [3204]"), Color::from_hex("4472C4"));
        assert_eq!(vml_color("black"), Some(Color::BLACK));
    }

    #[test]
    fn reads_vml_text_box() {
        let xml = r##"<w:pict xmlns:w="w" xmlns:v="v" xmlns:w10="w10"><v:rect style="position:absolute;margin-left:10pt;margin-top:20pt;width:100pt;height:50pt;z-index:-1;mso-position-horizontal-relative:page" fillcolor="#ffff00" strokecolor="red" strokeweight="2pt"><v:textbox inset="1pt,2pt,3pt,4pt" style="v-text-anchor:middle"/><w10:wrap type="square"/></v:rect></w:pict>"##;
        let el = bo_drawingml::dom::parse(xml.as_bytes(), "/t").unwrap();
        let (s, w, h, a, _) = vml_shape(&el).unwrap();
        assert_eq!((w.0, h.0), (1_270_000, 635_000));
        assert_eq!(s.fill, Color::from_hex("FFFF00"));
        assert_eq!(s.line.as_ref().map(|l| l.width.0), Some(25_400));
        assert_eq!(s.insets[1].0, 25_400);
        assert_eq!(s.anchor, TextAnchor::Center);
        let a = a.unwrap();
        assert_eq!((a.h_relative.as_str(), a.x.0), ("page", 127_000));
        assert!(a.behind);
        assert_eq!(a.wrap, Wrap::Square);
    }
}
