//! New shapes, slides and notes for the slide editor.
//!
//! Elements are written as PresentationML/DrawingML markup and parsed with the
//! regular reader, so new objects use exactly the same model (and the same
//! writer) as objects read from a file.

use super::geom::EMU_PT;
use bo_model::slide::{
    ColorMapOverride, CommonSlideData, NonVisual, Notes, PartInfo, Presentation, RawPart, Rel,
    ShapeNode, Slide, XmlElement,
};

/// Language tag of new text runs.
pub const LANG: &str = "vi-VN";

/// Escape text for XML content and attribute values.
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            // Characters not allowed in XML 1.0 (§2.2) are dropped.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => o.push(c),
        }
    }
    o
}

fn parse(xml: &str) -> Option<XmlElement> {
    bo_drawingml::dom::parse(xml.as_bytes(), "/new").ok()
}

/// Parse one shape-tree element.
pub fn node(xml: &str) -> Option<ShapeNode> {
    parse(xml).map(|e| bo_pptx::parse_node(&e))
}

fn emu(pt: f64) -> i64 {
    (pt * EMU_PT).round() as i64
}

/// §20.1.7.6 xfrm markup for a box in points.
fn xfrm(tag: &str, x: f64, y: f64, w: f64, h: f64) -> String {
    format!(
        r#"<{tag}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></{tag}>"#,
        emu(x),
        emu(y),
        emu(w.max(0.0)),
        emu(h.max(0.0))
    )
}

/// Runs of one paragraph of plain text (`\u{b}` = line break, §21.1.2.2.1 br).
fn runs(line: &str, rpr: &str) -> String {
    let mut out = String::new();
    for (i, part) in line.split('\u{b}').enumerate() {
        if i > 0 {
            out.push_str(&format!("<a:br>{rpr}</a:br>"));
        }
        if !part.is_empty() {
            out.push_str(&format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", esc(part)));
        }
    }
    out
}

/// Paragraphs of plain text (`\n` separates paragraphs, §21.1.2.2.6 p).
pub fn paragraphs(text: &str, ppr: &str, rpr_attrs: &str) -> String {
    let rpr = format!(r#"<a:rPr lang="{LANG}"{rpr_attrs} dirty="0"/>"#);
    let end = format!(r#"<a:endParaRPr lang="{LANG}"{rpr_attrs} dirty="0"/>"#);
    let mut out = String::new();
    for l in text.split('\n') {
        out.push_str("<a:p>");
        out.push_str(ppr);
        out.push_str(&runs(l, &rpr));
        out.push_str(&end);
        out.push_str("</a:p>");
    }
    out
}

/// Preset shape with the theme's default shape style (§19.3.1.46 style:
/// line = accent1 shaded, fill = accent1, text = lt1, minor font).
pub fn preset_shape(id: u32, preset: &str, x: f64, y: f64, w: f64, h: f64) -> Option<ShapeNode> {
    let name = format!("{preset} {id}");
    node(&format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr>{xfrm}<a:prstGeom prst="{preset}"><a:avLst/></a:prstGeom></p:spPr>"#,
            r#"<p:style><a:lnRef idx="2"><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:lnRef>"#,
            r#"<a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef>"#,
            r#"<a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef>"#,
            r#"<a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style>"#,
            r#"<p:txBody><a:bodyPr rtlCol="0" anchor="ctr"/><a:lstStyle/>{paras}</p:txBody></p:sp>"#
        ),
        id = id,
        name = esc(&name),
        xfrm = xfrm("a:xfrm", x, y, w, h),
        preset = esc(preset),
        paras = paragraphs("", r#"<a:pPr algn="ctr"/>"#, ""),
    ))
}

/// Text box (§19.3.1.43 sp with `cNvSpPr@txBox`), auto-fitting its height.
pub fn text_box(id: u32, text: &str, x: f64, y: f64, w: f64, h: f64) -> Option<ShapeNode> {
    node(&format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="TextBox {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>"#,
            r#"<p:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr>"#,
            r#"<p:txBody><a:bodyPr wrap="square" rtlCol="0"><a:spAutoFit/></a:bodyPr><a:lstStyle/>{paras}</p:txBody></p:sp>"#
        ),
        id = id,
        xfrm = xfrm("a:xfrm", x, y, w, h),
        paras = paragraphs(text, "", r#" sz="1800""#),
    ))
}

/// Picture (§19.3.1.37 pic) referencing image relationship `rid`.
pub fn picture(
    id: u32,
    rid: &str,
    name: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Option<ShapeNode> {
    node(&format!(
        concat!(
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Picture {id}" descr="{descr}"/>"#,
            r#"<p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr>{xfrm}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
        ),
        id = id,
        descr = esc(name),
        rid = esc(rid),
        xfrm = xfrm("a:xfrm", x, y, w, h),
    ))
}

/// Table (§21.1.3.13 tbl in a §19.3.1.21 graphicFrame) using table style
/// `style` (§21.1.3.15 tableStyleId) with a header row and banded rows.
pub fn table(
    id: u32,
    rows: usize,
    cols: usize,
    x: f64,
    y: f64,
    w: f64,
    style: &str,
) -> Option<ShapeNode> {
    let rows = rows.clamp(1, 50);
    let cols = cols.clamp(1, 30);
    let col_w = emu(w / cols as f64);
    let row_h: i64 = 370_840; // 29.2 pt
    let mut grid = String::new();
    for _ in 0..cols {
        grid.push_str(&format!(r#"<a:gridCol w="{col_w}"/>"#));
    }
    let mut trs = String::new();
    for _ in 0..rows {
        trs.push_str(&format!(r#"<a:tr h="{row_h}">"#));
        for _ in 0..cols {
            trs.push_str(&format!(
                r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="{LANG}" dirty="0"/></a:p></a:txBody><a:tcPr/></a:tc>"#
            ));
        }
        trs.push_str("</a:tr>");
    }
    let h = rows as f64 * row_h as f64 / EMU_PT;
    node(&format!(
        concat!(
            r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Table {id}"/>"#,
            r#"<p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>"#,
            r#"{xfrm}<a:graphic><a:graphicData uri="{uri}"><a:tbl><a:tblPr firstRow="1" bandRow="1"><a:tableStyleId>{style}</a:tableStyleId></a:tblPr>"#,
            r#"<a:tblGrid>{grid}</a:tblGrid>{trs}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#
        ),
        id = id,
        xfrm = xfrm("p:xfrm", x, y, col_w as f64 * cols as f64 / EMU_PT, h),
        uri = bo_pptx::URI_TABLE,
        grid = grid,
        trs = trs,
        style = esc(style),
    ))
}

/// The table styles part with built-in style `n` added when missing
/// (§20.1.4.2.27 tblStyleLst); `None` when the deck already defines it.
pub fn table_styles_with(pres: &Presentation, n: usize) -> Option<RawPart> {
    use bo_pptx::table_style::{builtin_xml, BUILTIN_IDS, TABLE_STYLES_CT, TABLE_STYLES_PART};
    let id = BUILTIN_IDS[n % 6];
    let style = parse(&format!(
        r#"<a:r xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{}</a:r>"#,
        builtin_xml(n)
    ))?
    .child("a:tblStyle")?
    .clone();
    let mut root = if let Some(d) = pres.part_data(TABLE_STYLES_PART) {
        bo_drawingml::dom::parse(d, TABLE_STYLES_PART).ok()?
    } else {
        {
            let mut r = XmlElement::new("a:tblStyleLst");
            r.attrs = vec![
                (
                    "xmlns:a".into(),
                    "http://schemas.openxmlformats.org/drawingml/2006/main".into(),
                ),
                ("def".into(), id.into()),
            ];
            r
        }
    };
    if root
        .elements()
        .any(|e| e.local() == "tblStyle" && e.attr("styleId") == Some(id))
    {
        return None;
    }
    root.children.push(bo_model::slide::XmlNode::Element(style));
    Some(RawPart {
        name: TABLE_STYLES_PART.into(),
        content_type: Some(TABLE_STYLES_CT.into()),
        data: bo_drawingml::dom::to_part_xml(&root),
    })
}

/// Connector (§19.3.1.19 cxnSp) with an arrow at its end, styled like new
/// lines (lnRef accent1); `preset` is a connector shape (`straightConnector1`,
/// `bentConnector3`, `curvedConnector3`).
pub fn connector(id: u32, preset: &str) -> Option<ShapeNode> {
    node(&format!(
        concat!(
            r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="{id}" name="Connector {id}"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>"#,
            r#"<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></a:xfrm><a:prstGeom prst="{preset}"><a:avLst/></a:prstGeom>"#,
            r#"<a:ln w="19050"><a:tailEnd type="triangle"/></a:ln></p:spPr>"#,
            r#"<p:style><a:lnRef idx="1"><a:schemeClr val="accent1"/></a:lnRef><a:fillRef idx="0"><a:schemeClr val="accent1"/></a:fillRef>"#,
            r#"<a:effectRef idx="0"><a:schemeClr val="accent1"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="tx1"/></a:fontRef></p:style></p:cxnSp>"#
        ),
        id = id,
        preset = esc(preset),
    ))
}

/// Group (§19.3.1.22 grpSp) whose child space equals its box (no scaling).
pub fn group(
    id: u32,
    off: (i64, i64),
    ext: (i64, i64),
    children: Vec<ShapeNode>,
) -> Option<ShapeNode> {
    let (x, y) = off;
    let (cx, cy) = ext;
    let ShapeNode::Group(mut g) = node(&format!(
        concat!(
            r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="{id}" name="Group {id}"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
            r#"<p:grpSpPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/>"#,
            r#"<a:chOff x="{x}" y="{y}"/><a:chExt cx="{cx}" cy="{cy}"/></a:xfrm></p:grpSpPr></p:grpSp>"#
        ),
        id = id,
        x = x,
        y = y,
        cx = cx,
        cy = cy,
    ))?
    else {
        return None;
    };
    g.children = children;
    Some(ShapeNode::Group(g))
}

/// Placeholder shape copied from a layout placeholder (§19.3.1.36 ph): only the
/// placeholder reference — position and formatting are inherited.
fn placeholder(id: u32, kind: Option<&str>, idx: Option<u32>, name: &str) -> Option<ShapeNode> {
    let mut ph = String::from("<p:ph");
    if let Some(k) = kind {
        ph.push_str(&format!(r#" type="{}""#, esc(k)));
    }
    if let Some(i) = idx {
        ph.push_str(&format!(r#" idx="{i}""#));
    }
    ph.push_str("/>");
    node(&format!(
        concat!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr/>"#,
            r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="{lang}" dirty="0"/></a:p></p:txBody></p:sp>"#
        ),
        id = id,
        name = esc(name),
        ph = ph,
        lang = LANG,
    ))
}

/// Empty spTree header (§19.3.1.45 spTree: nvGrpSpPr + grpSpPr).
fn empty_tree() -> CommonSlideData {
    CommonSlideData {
        tree_nv: NonVisual {
            id: 1,
            name: String::new(),
            ..NonVisual::default()
        },
        ..CommonSlideData::default()
    }
}

/// A unique part name `prefix{N}.xml` among slides, notes and preserved parts.
pub fn unique_part(pres: &Presentation, prefix: &str, ext: &str) -> String {
    unique_part_except(pres, prefix, ext, &[])
}

/// [`unique_part`] also avoiding the names in `also` (parts about to be added).
pub fn unique_part_except(pres: &Presentation, prefix: &str, ext: &str, also: &[String]) -> String {
    let mut used: Vec<String> = pres
        .parts
        .iter()
        .map(|p| p.name.to_ascii_lowercase())
        .collect();
    used.extend(
        pres.themes
            .iter()
            .map(|t| t.info.part_name.to_ascii_lowercase()),
    );
    used.extend(
        pres.masters
            .iter()
            .map(|m| m.info.part_name.to_ascii_lowercase()),
    );
    used.extend(
        pres.layouts
            .iter()
            .map(|l| l.info.part_name.to_ascii_lowercase()),
    );
    for s in &pres.slides {
        used.push(s.info.part_name.to_ascii_lowercase());
        if let Some(n) = &s.notes {
            used.push(n.info.part_name.to_ascii_lowercase());
        }
    }
    used.extend(also.iter().map(|a| a.to_ascii_lowercase()));
    for n in 1.. {
        let name = format!("{prefix}{n}.{ext}");
        if !used.contains(&name.to_ascii_lowercase()) {
            return name;
        }
    }
    unreachable!("unbounded counter")
}

/// New slide on `layout` with the layout's content placeholders (§19.3.1.38 sld).
pub fn slide_from_layout(pres: &Presentation, layout: usize) -> Slide {
    let mut data = empty_tree();
    let mut id = 2;
    if let Some(l) = pres.layouts.get(layout) {
        for n in &l.data.shapes {
            let ShapeNode::Shape(s) = n else { continue };
            let Some(ph) = &s.nv.placeholder else {
                continue;
            };
            // Date, footer, slide number and header are not copied (§19.3.1.36).
            if matches!(ph.kind.as_deref(), Some("dt" | "ftr" | "sldNum" | "hdr")) {
                continue;
            }
            if let Some(p) = placeholder(id, ph.kind.as_deref(), ph.idx, &s.nv.name) {
                data.shapes.push(p);
                id += 1;
            }
        }
    }
    Slide {
        info: PartInfo {
            part_name: unique_part(pres, "/ppt/slides/slide", "xml"),
            ..PartInfo::default()
        },
        data,
        layout,
        clr_map_ovr: Some(ColorMapOverride::Master),
        ..Slide::default()
    }
}

/// Copy of a slide that is written as a new part (`sldId`, relationship id and
/// part names are allocated again; notes are copied too).
pub fn duplicate_slide(pres: &Presentation, s: &Slide) -> Slide {
    let mut d = s.clone();
    d.info.part_name = unique_part(pres, "/ppt/slides/slide", "xml");
    d.slide_id = 0;
    d.pres_rel_id = String::new();
    if let Some(n) = &mut d.notes {
        n.info.part_name = unique_part(pres, "/ppt/notesSlides/notesSlide", "xml");
    }
    d
}

/// Part name of the notes master, if the package has one.
pub fn notes_master(pres: &Presentation) -> Option<String> {
    pres.parts
        .iter()
        .find(|p| {
            p.content_type.as_deref()
                == Some(
                    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml",
                )
        })
        .map(|p| p.name.clone())
}

/// What is needed to give a deck a notes master (§19.3.1.28 notesMaster):
/// the master part, its theme (a copy of the first slide theme) and their rels.
pub struct NotesMasterParts {
    pub parts: Vec<RawPart>,
    pub master: String,
    pub rel: Rel,
    pub id_list: XmlElement,
}

const NS: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);

/// Build a minimal notes master: slide image and notes body placeholders.
pub fn new_notes_master(pres: &Presentation) -> Option<NotesMasterParts> {
    let master = unique_part(pres, "/ppt/notesMasters/notesMaster", "xml");
    let theme_name = unique_part(pres, "/ppt/theme/theme", "xml");
    let theme_bytes = bo_drawingml::theme::write_theme(pres.themes.first()?);
    let (nw, nh) = if pres.notes_size.cx > 0 {
        (pres.notes_size.cx, pres.notes_size.cy)
    } else {
        (6_858_000, 9_144_000)
    };
    let (img_w, img_h) = {
        let sw = pres.slide_size.cx.max(1) as f64;
        let sh = pres.slide_size.cy.max(1) as f64;
        let w = nw as f64 * 0.8;
        (w as i64, (w * sh / sw) as i64)
    };
    let mx = nw / 10;
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<p:notesMaster {ns}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>"#,
            r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{mx}" y="{iy}"/><a:ext cx="{iw}" cy="{ih}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:ln></p:spPr></p:sp>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{mx}" y="{by}"/><a:ext cx="{bw}" cy="{bh}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="{lang}"/></a:p></p:txBody></p:sp>"#,
            r#"</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" "#,
            r#"accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#,
            r#"<p:notesStyle><a:lvl1pPr marL="0" algn="l" rtl="0"><a:defRPr sz="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill>"#,
            r#"<a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"#
        ),
        ns = NS,
        mx = mx,
        iy = nh / 12,
        iw = img_w,
        ih = img_h,
        by = nh / 12 + img_h + nh / 24,
        bw = nw - 2 * mx,
        bh = nh - (nh / 12 + img_h + nh / 24) - nh / 12,
        lang = LANG,
    );
    let rels_name = {
        let (dir, file) = master.rsplit_once('/')?;
        format!("{dir}/_rels/{file}.rels")
    };
    let theme_target = format!("../theme/{}", theme_name.rsplit('/').next()?);
    let rels = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="{ns}theme" Target="{t}"/></Relationships>"#
        ),
        ns = bo_pptx::RELATIONSHIP_NS,
        t = theme_target,
    );
    let rid = {
        let mut n = 1;
        loop {
            let id = format!("rId{n}");
            let taken = pres.info.rels.iter().any(|r| r.id == id)
                || pres.slides.iter().any(|s| s.pres_rel_id == id)
                || pres.masters.iter().any(|m| m.pres_rel_id == id);
            if !taken {
                break id;
            }
            n += 1;
        }
    };
    let id_list = parse(&format!(
        r#"<p:notesMasterIdLst><p:notesMasterId r:id="{rid}"/></p:notesMasterIdLst>"#
    ))?;
    Some(NotesMasterParts {
        parts: vec![
            RawPart {
                name: master.clone(),
                content_type: Some(
                    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"
                        .into(),
                ),
                data: xml.into_bytes(),
            },
            RawPart {
                name: rels_name,
                content_type: Some(
                    "application/vnd.openxmlformats-package.relationships+xml".into(),
                ),
                data: rels.into_bytes(),
            },
            RawPart {
                name: theme_name,
                content_type: Some(
                    "application/vnd.openxmlformats-officedocument.theme+xml".into(),
                ),
                data: theme_bytes,
            },
        ],
        rel: Rel {
            id: rid,
            rel_type: format!("{}notesMaster", bo_pptx::RELATIONSHIP_NS),
            target: master.trim_start_matches("/ppt/").to_owned(),
            external: false,
        },
        master,
        id_list,
    })
}

/// Notes slide (§19.3.1.26 notes) for a slide, related to `master`.
pub fn notes(pres: &Presentation, master: &str, text: &str) -> Option<Notes> {
    let xml = format!(
        concat!(
            r#"<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>"#,
            r#"<p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paras}</p:txBody></p:sp>"#,
            r#"</p:spTree></p:cSld>"#
        ),
        paras = paragraphs(text, "", ""),
    );
    let csld = parse(&xml)?;
    let mut data = empty_tree();
    for el in csld.child("p:spTree")?.elements() {
        if matches!(el.name.as_str(), "p:sp") {
            data.shapes.push(bo_pptx::parse_node(el));
        }
    }
    let part_name = unique_part(pres, "/ppt/notesSlides/notesSlide", "xml");
    let target = format!("../notesMasters/{}", master.rsplit('/').next()?);
    Some(Notes {
        info: PartInfo {
            part_name,
            rels: vec![Rel {
                id: "rId1".into(),
                rel_type: format!("{}notesMaster", bo_pptx::RELATIONSHIP_NS),
                target,
                external: false,
            }],
            root_attrs: Vec::new(),
        },
        data,
        clr_map_ovr: Some(ColorMapOverride::Master),
        extra: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_parse_into_model_nodes() {
        let ShapeNode::Shape(s) = preset_shape(5, "ellipse", 10.0, 20.0, 100.0, 50.0).unwrap()
        else {
            panic!("shape");
        };
        assert_eq!(s.nv.id, 5);
        assert_eq!(
            s.props.xfrm.as_ref().unwrap().ext,
            Some((1_270_000, 635_000))
        );
        assert!(s.style.is_some());
        let ShapeNode::Shape(t) =
            text_box(6, "Xin chào\nViệt\u{b}Nam & <b>", 0.0, 0.0, 100.0, 40.0).unwrap()
        else {
            panic!("text box");
        };
        let tb = t.text.unwrap();
        assert_eq!(tb.paragraphs.len(), 2);
        assert_eq!(tb.text(), "Xin chào\nViệt\nNam & <b>");
        assert!(matches!(
            table(7, 2, 3, 0.0, 0.0, 300.0, "{X}"),
            Some(ShapeNode::Frame(_))
        ));
        assert!(matches!(
            picture(8, "rId9", "a.png", 0.0, 0.0, 10.0, 10.0),
            Some(ShapeNode::Picture(_))
        ));
    }
}
