//! Region map charts (CORE-CHART-006): the chartex `regionMap` series layout of [MS-ODRAWXML]
//! (namespace `http://schemas.microsoft.com/office/drawing/2014/chartex`). A map series keeps its
//! locations in a `cx:strDim type="cat"` and its values in a `cx:numDim type="colorVal"` (or, for a
//! map of categories, a `cx:strDim type="colorStr"`); the series' `cx:valueColors` and
//! `cx:valueColorPositions` give the colour scale, and its `cx:layoutPr` the `cx:regionLabelLayout`
//! and the `cx:geography` (projection, area level, culture, attribution and the cached geography
//! data `cx:geoCache`).
//!
//! The geography cache is an opaque binary of the application that made the file; it is kept
//! verbatim (the inner XML of `cx:geography`) and never decoded — maps are drawn from the boundary
//! sets of `bo_sheet::geo`.

use crate::chart::Node;
use bo_model::sheet::{ChartColor, MapColorScale, MapOptions};
use bo_opc::xml::escape;
use std::fmt::Write;

/// Attribution written for a new map (its boundaries come from Natural Earth, public domain).
pub const ATTRIBUTION: &str = "Natural Earth";

/// The projections of `ST_GeoProjectionType`.
pub const PROJECTIONS: [&str; 4] = ["mercator", "miller", "robinson", "albers"];
/// The area levels of `ST_GeoMappingLevel`.
pub const LEVELS: [&str; 7] = [
    "dataOnly",
    "postalCode",
    "county",
    "state",
    "countryRegion",
    "countryRegionList",
    "world",
];
/// The values of `ST_RegionLabelLayout`.
pub const LABEL_LAYOUTS: [&str; 3] = ["none", "bestFitOnly", "showAll"];

fn color_child(n: &Node, name: &str) -> Option<ChartColor> {
    n.child(name).and_then(crate::chart_format::color_of)
}

/// `cx:number@val` / `cx:percent@val` of a colour position.
fn position(n: Option<&Node>, name: &str) -> Option<f64> {
    n?.child(name)?.attr("val")?.trim().parse().ok()
}

/// Map options of a `regionMap` series `s`; `raw` is the text of the whole chartex part (for the
/// verbatim geography cache).
pub(crate) fn parse(s: &Node, raw: Option<&str>) -> MapOptions {
    let mut m = MapOptions::default();
    // [MS-ODRAWXML] chartex CT_ValueColors: minColor, midColor, maxColor (CT_SolidColorFillProperties).
    if let Some(vc) = s.child("valueColors") {
        m.scale.min = color_child(vc, "minColor");
        m.scale.mid = color_child(vc, "midColor");
        m.scale.max = color_child(vc, "maxColor");
    }
    // CT_ValueColorPositions: @count (2 or 3), min / mid / max positions.
    if let Some(vp) = s.child("valueColorPositions") {
        m.scale.diverging = vp.attr("count").map(str::trim) == Some("3");
        m.scale.min_number = position(vp.child("min"), "number");
        m.scale.max_number = position(vp.child("max"), "number");
        m.scale.mid_number = position(vp.child("mid"), "number");
        m.scale.mid_percent = position(vp.child("mid"), "percent");
    } else if m.scale.mid.is_some() {
        m.scale.diverging = true;
    }
    if let Some(lp) = s.child("layoutPr") {
        m.labels = lp
            .child("regionLabelLayout")
            .and_then(|r| r.attr("val"))
            .map(str::to_owned);
        if let Some(g) = lp.child("geography") {
            let a = |k: &str| g.attr(k).map(str::to_owned);
            m.projection = a("projectionType");
            m.level = a("viewedRegionType");
            m.culture_language = a("cultureLanguage").unwrap_or_default();
            m.culture_region = a("cultureRegion").unwrap_or_default();
            m.attribution = a("attribution").unwrap_or_default();
            m.geo_raw = raw
                .and_then(geography_inner)
                .filter(|x| !x.trim().is_empty());
        }
    }
    m
}

/// The inner XML of the part's `cx:geography` element when it only uses the `cx`, `a` and `r`
/// prefixes the writer declares (otherwise the cache is dropped: the application rebuilds it).
fn geography_inner(xml: &str) -> Option<String> {
    let start = xml.find("<cx:geography")?;
    let rest = &xml[start..];
    // End of the start tag (attribute values may hold `>`).
    let mut quote: Option<char> = None;
    let mut end = None;
    for (i, ch) in rest.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(ch),
            (None, '>') => {
                end = Some(i);
                break;
            }
            _ => {}
        }
    }
    let end = end?;
    if rest[..end].ends_with('/') {
        return None;
    }
    let body = &rest[end + 1..];
    let close = body.find("</cx:geography>")?;
    let inner = &body[..close];
    // Every prefixed element must be one the writer declares.
    let ok = inner.match_indices('<').all(|(i, _)| {
        let tail = &inner[i + 1..];
        let tail = tail.strip_prefix('/').unwrap_or(tail);
        if tail.starts_with('!') || tail.starts_with('?') {
            return true;
        }
        let name: String = tail
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, '>' | '/'))
            .collect();
        match name.split_once(':') {
            Some((p, _)) => matches!(p, "cx" | "a" | "r"),
            None => false,
        }
    });
    (ok && !inner.contains("<!")).then(|| inner.to_owned())
}

fn solid(out: &mut String, tag: &str, c: &ChartColor) {
    let _ = write!(
        out,
        "<cx:{tag}>{}</cx:{tag}>",
        crate::chart_format::color_el(c, "")
    );
}

/// `cx:valueColors` and `cx:valueColorPositions` of a numeric map series (they follow `cx:tx` /
/// `cx:spPr` in CT_Series).
pub(crate) fn write_scale(out: &mut String, sc: &MapColorScale) {
    let (dmin, dmid, dmax) = MapColorScale::default_colors(sc.diverging);
    out.push_str("<cx:valueColors>");
    solid(out, "minColor", sc.min.as_ref().unwrap_or(&dmin));
    if sc.diverging {
        if let Some(mid) = sc.mid.as_ref().or(dmid.as_ref()) {
            solid(out, "midColor", mid);
        }
    }
    solid(out, "maxColor", sc.max.as_ref().unwrap_or(&dmax));
    out.push_str("</cx:valueColors>");
    let _ = write!(
        out,
        "<cx:valueColorPositions count=\"{}\">",
        if sc.diverging { 3 } else { 2 }
    );
    let end = |out: &mut String, tag: &str, v: Option<f64>| match v {
        Some(v) if v.is_finite() => {
            let _ = write!(out, "<cx:{tag}><cx:number val=\"{v}\"/></cx:{tag}>");
        }
        _ => {
            let _ = write!(out, "<cx:{tag}><cx:extremeValue/></cx:{tag}>");
        }
    };
    end(out, "min", sc.min_number);
    if sc.diverging {
        match (sc.mid_number, sc.mid_percent) {
            (Some(v), _) if v.is_finite() => {
                let _ = write!(out, "<cx:mid><cx:number val=\"{v}\"/></cx:mid>");
            }
            (_, p) => {
                let p = p
                    .filter(|p| p.is_finite())
                    .unwrap_or(50.0)
                    .clamp(0.0, 100.0);
                let _ = write!(out, "<cx:mid><cx:percent val=\"{p}\"/></cx:mid>");
            }
        }
    }
    end(out, "max", sc.max_number);
    out.push_str("</cx:valueColorPositions>");
}

/// `cx:dataLabels` showing the region names when the map has region labels.
pub(crate) fn write_labels(out: &mut String, m: &MapOptions) {
    if matches!(m.labels.as_deref(), Some("bestFitOnly" | "showAll")) {
        out.push_str(
            "<cx:dataLabels><cx:visibility seriesName=\"0\" categoryName=\"1\" value=\"0\"/></cx:dataLabels>",
        );
    }
}

/// `cx:layoutPr` of a map series: `cx:regionLabelLayout` then `cx:geography` (CT_SeriesLayoutProperties
/// order) with the file's cache kept verbatim.
pub(crate) fn write_layout(out: &mut String, m: &MapOptions) {
    out.push_str("<cx:layoutPr>");
    if let Some(l) = m.labels.as_deref().filter(|l| LABEL_LAYOUTS.contains(l)) {
        let _ = write!(out, "<cx:regionLabelLayout val=\"{l}\"/>");
    }
    out.push_str("<cx:geography");
    if let Some(p) = m.projection.as_deref().filter(|p| PROJECTIONS.contains(p)) {
        let _ = write!(out, " projectionType=\"{p}\"");
    }
    if let Some(l) = m.level.as_deref().filter(|l| LEVELS.contains(l)) {
        let _ = write!(out, " viewedRegionType=\"{l}\"");
    }
    let or = |v: &str, d: &'static str| {
        if v.trim().is_empty() {
            d.to_owned()
        } else {
            v.to_owned()
        }
    };
    let _ = write!(
        out,
        " cultureLanguage=\"{}\" cultureRegion=\"{}\" attribution=\"{}\"",
        escape(&or(&m.culture_language, "vi-VN")),
        escape(&or(&m.culture_region, "VN")),
        escape(&or(&m.attribution, ATTRIBUTION)),
    );
    match m.geo_raw.as_deref().filter(|r| !r.trim().is_empty()) {
        Some(raw) => {
            out.push('>');
            out.push_str(raw);
            out.push_str("</cx:geography>");
        }
        None => out.push_str("/>"),
    }
    out.push_str("</cx:layoutPr>");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geography_inner_keeps_known_prefixes_only() {
        let xml = r#"<cx:chartSpace><cx:geography attribution="a > b" cultureLanguage="en-US"><cx:geoCache provider="{X}"><cx:binary>QUJD</cx:binary></cx:geoCache></cx:geography></cx:chartSpace>"#;
        assert_eq!(
            geography_inner(xml).as_deref(),
            Some(r#"<cx:geoCache provider="{X}"><cx:binary>QUJD</cx:binary></cx:geoCache>"#)
        );
        let foreign = "<cx:geography><x:foo/></cx:geography>";
        assert_eq!(geography_inner(foreign), None);
        assert_eq!(geography_inner("<cx:geography a=\"1\"/>"), None);
    }
}
