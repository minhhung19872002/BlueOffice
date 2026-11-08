//! Placeholder and text-style inheritance (ECMA-376 Part 1, §19.3.1.36 ph,
//! §19.3.1.52 txStyles, §21.1.2.4.12 lstStyle).
//!
//! A placeholder shape on a slide inherits position, geometry, fill, line,
//! body properties and list styles from the matching placeholder of its
//! layout, which in turn inherits from the master. Matching is by `idx`
//! first, then by (normalised) `type`; on the master every content type that
//! has no own placeholder falls back to `body`.

use bo_model::slide::{
    BodyProps, ColorMap, ListStyle, ParaProps, Paragraph, Placeholder, Presentation, RunProps,
    Shape, ShapeNode, ShapeProps, Slide, Theme, Xfrm,
};

/// Where a shape tree lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A slide, by index into the presentation's slides.
    Slide(usize),
    /// A slide layout, by index into the presentation's layouts.
    Layout(usize),
    /// A slide master, by index into the presentation's masters.
    Master(usize),
}

fn norm_kind(kind: Option<&str>) -> &str {
    match kind {
        Some("ctrTitle" | "title") => "title",
        Some(k) => k,
        None => "obj",
    }
}

fn master_kind(kind: &str) -> &str {
    match kind {
        "title" | "dt" | "ftr" | "sldNum" | "hdr" => kind,
        _ => "body",
    }
}

/// Find the placeholder shape matching `ph` in a shape tree.
pub fn find_placeholder<'a>(
    nodes: &'a [ShapeNode],
    ph: &Placeholder,
    on_master: bool,
) -> Option<&'a Shape> {
    let mut all: Vec<(&'a Placeholder, &'a Shape)> = Vec::new();
    collect(nodes, &mut all);
    let kind = norm_kind(ph.kind.as_deref());
    let kind = if on_master { master_kind(kind) } else { kind };
    if !on_master {
        if let Some(idx) = ph.idx {
            if let Some((_, s)) = all.iter().find(|(p, _)| p.idx == Some(idx)) {
                return Some(s);
            }
        }
    }
    if let Some((_, s)) = all
        .iter()
        .find(|(p, _)| norm_kind(p.kind.as_deref()) == kind)
    {
        return Some(s);
    }
    if on_master {
        return None;
    }
    // Layout: an untyped (obj) slide placeholder may sit on a body placeholder.
    all.iter()
        .find(|(p, _)| {
            matches!(kind, "obj" | "body") && matches!(norm_kind(p.kind.as_deref()), "obj" | "body")
        })
        .map(|(_, s)| *s)
}

fn collect<'a>(nodes: &'a [ShapeNode], out: &mut Vec<(&'a Placeholder, &'a Shape)>) {
    for n in nodes {
        match n {
            ShapeNode::Shape(s) => {
                if let Some(p) = &s.nv.placeholder {
                    out.push((p, s));
                }
            }
            ShapeNode::Group(g) => collect(&g.children, out),
            _ => {}
        }
    }
}

/// Master index of a level.
pub fn master_of(pres: &Presentation, level: Level) -> Option<usize> {
    match level {
        Level::Slide(i) => pres
            .slides
            .get(i)
            .and_then(|s| pres.layouts.get(s.layout))
            .map(|l| l.master),
        Level::Layout(i) => pres.layouts.get(i).map(|l| l.master),
        Level::Master(i) => Some(i),
    }
}

/// Theme of a level.
pub fn theme_of(pres: &Presentation, level: Level) -> Option<&Theme> {
    master_of(pres, level)
        .and_then(|m| pres.masters.get(m))
        .and_then(|m| pres.themes.get(m.theme))
}

/// Effective colour map (§19.3.1.6 clrMap with §19.3.1.7 clrMapOvr overrides).
pub fn clr_map_of(pres: &Presentation, level: Level) -> ColorMap {
    let master = master_of(pres, level)
        .and_then(|m| pres.masters.get(m))
        .map(|m| m.clr_map.clone())
        .unwrap_or_default();
    let ovr = match level {
        Level::Slide(i) => pres.slides.get(i).and_then(|s| s.clr_map_ovr.clone()),
        Level::Layout(i) => pres.layouts.get(i).and_then(|l| l.clr_map_ovr.clone()),
        Level::Master(_) => None,
    };
    match ovr {
        Some(bo_model::slide::ColorMapOverride::Override(m)) => m,
        _ => master,
    }
}

/// Inheritance chain of a shape: the shape itself first, then the matching
/// layout placeholder, then the master placeholder.
pub fn chain<'a>(pres: &'a Presentation, level: Level, shape: &'a Shape) -> Vec<&'a Shape> {
    let mut out = vec![shape];
    let Some(ph) = &shape.nv.placeholder else {
        return out;
    };
    let master = master_of(pres, level).and_then(|m| pres.masters.get(m));
    match level {
        Level::Slide(i) => {
            let layout = pres.slides.get(i).and_then(|s| pres.layouts.get(s.layout));
            let lp = layout.and_then(|l| find_placeholder(&l.data.shapes, ph, false));
            if let Some(lp) = lp {
                out.push(lp);
            }
            let mph = lp.and_then(|s| s.nv.placeholder.as_ref()).unwrap_or(ph);
            if let Some(mp) = master.and_then(|m| find_placeholder(&m.data.shapes, mph, true)) {
                out.push(mp);
            }
        }
        Level::Layout(_) => {
            if let Some(mp) = master.and_then(|m| find_placeholder(&m.data.shapes, ph, true)) {
                out.push(mp);
            }
        }
        Level::Master(_) => {}
    }
    out
}

/// Effective transform: first complete xfrm along the chain.
pub fn effective_xfrm(chain: &[&Shape]) -> Option<Xfrm> {
    chain
        .iter()
        .filter_map(|s| s.props.xfrm.as_ref())
        .find(|x| x.off.is_some() && x.ext.is_some())
        .cloned()
}

/// Effective shape properties (each property taken from the nearest level that sets it).
pub fn effective_props(chain: &[&Shape]) -> ShapeProps {
    let mut p = ShapeProps {
        xfrm: effective_xfrm(chain),
        ..ShapeProps::default()
    };
    for s in chain {
        if p.geometry.is_none() {
            p.geometry.clone_from(&s.props.geometry);
        }
        if p.fill.is_none() {
            p.fill.clone_from(&s.props.fill);
        }
        if p.line.is_none() {
            p.line.clone_from(&s.props.line);
        }
        if p.effects.is_none() {
            p.effects.clone_from(&s.props.effects);
        }
    }
    p
}

macro_rules! merge_opts {
    ($base:expr, $over:expr; $($f:ident),* $(,)?) => {
        $( if $over.$f.is_some() { $base.$f.clone_from(&$over.$f); } )*
    };
}

/// Overlay body properties (`over` wins attribute by attribute).
pub fn merge_body(base: &mut BodyProps, over: &BodyProps) {
    merge_opts!(base, over; l_ins, t_ins, r_ins, b_ins, anchor, anchor_ctr, wrap, num_col,
        spc_col, rot, vert, autofit);
}

/// Effective bodyPr: master → layout → slide.
pub fn effective_body(chain: &[&Shape]) -> BodyProps {
    let mut b = BodyProps::default();
    for s in chain.iter().rev() {
        if let Some(t) = &s.text {
            merge_body(&mut b, &t.body_pr);
        }
    }
    b
}

/// Overlay run properties.
pub fn merge_rpr(base: &mut RunProps, over: &RunProps) {
    merge_opts!(base, over; lang, sz, b, i, u, strike, baseline, spc, cap, line, fill,
        highlight, latin, ea, cs, sym);
}

/// Overlay paragraph properties (including `defRPr`).
pub fn merge_ppr(base: &mut ParaProps, over: &ParaProps) {
    merge_opts!(base, over; lvl, mar_l, mar_r, indent, algn, def_tab_sz, rtl, ln_spc,
        spc_bef, spc_aft, bu_clr, bu_sz, bu_font, bullet, tabs);
    if let Some(o) = &over.def_rpr {
        merge_rpr(base.def_rpr.get_or_insert_with(RunProps::default), o);
    }
}

fn level_of(ls: &ListStyle, lvl: usize) -> Option<&ParaProps> {
    ls.levels.get(lvl).and_then(Option::as_ref)
}

/// Stack of list styles that apply to the text of one shape, lowest priority first.
#[derive(Debug, Clone, Default)]
pub struct TextStyleStack {
    /// List styles, lowest priority first.
    pub layers: Vec<ListStyle>,
    /// Font/colour from the shape style's fontRef, applied above master styles.
    pub font_ref: Option<RunProps>,
    /// Number of master-level layers (below `font_ref`).
    pub master_layers: usize,
}

impl TextStyleStack {
    /// Build the stack for a shape at `level`.
    pub fn new(pres: &Presentation, level: Level, chain: &[&Shape]) -> Self {
        let mut layers = Vec::new();
        let master = master_of(pres, level).and_then(|m| pres.masters.get(m));
        let ph_kind = chain[0]
            .nv
            .placeholder
            .as_ref()
            .map(|p| master_kind(norm_kind(p.kind.as_deref())).to_owned());
        if let Some(d) = &pres.default_text_style {
            if ph_kind.is_none() {
                layers.push(d.clone());
            }
        }
        if let Some(m) = master {
            let ts = &m.text_styles;
            let st = match ph_kind.as_deref() {
                Some("title") => ts.title.as_ref(),
                Some("body") => ts.body.as_ref(),
                _ => ts.other.as_ref(),
            };
            if let Some(s) = st {
                layers.push(s.clone());
            }
        }
        let master_layers = layers.len();
        for s in chain.iter().rev() {
            if let Some(ls) = s.text.as_ref().and_then(|t| t.list_style.as_ref()) {
                layers.push(ls.clone());
            }
        }
        let font_ref = chain[0].style.as_ref().map(|st| {
            let face = match st.font_ref.idx.as_str() {
                "major" => Some("+mj-lt"),
                "minor" => Some("+mn-lt"),
                _ => None,
            };
            RunProps {
                latin: face.map(|f| bo_model::slide::TextFont {
                    typeface: f.into(),
                    attrs: vec![],
                }),
                fill: st.font_ref.color.clone().map(bo_model::slide::Fill::Solid),
                ..RunProps::default()
            }
        });
        TextStyleStack {
            layers,
            font_ref,
            master_layers,
        }
    }

    /// Effective paragraph properties (with merged `def_rpr`) for a paragraph.
    pub fn paragraph(&self, p: &Paragraph) -> ParaProps {
        let lvl = p.props.as_ref().and_then(|pp| pp.lvl).unwrap_or(0).min(8) as usize;
        let mut out = ParaProps::default();
        for (i, ls) in self.layers.iter().enumerate() {
            if i == self.master_layers {
                if let Some(f) = &self.font_ref {
                    merge_rpr(out.def_rpr.get_or_insert_with(RunProps::default), f);
                }
            }
            if let Some(d) = &ls.def {
                merge_ppr(&mut out, d);
            }
            if let Some(l) = level_of(ls, lvl) {
                merge_ppr(&mut out, l);
            }
        }
        if self.master_layers == self.layers.len() {
            if let Some(f) = &self.font_ref {
                merge_rpr(out.def_rpr.get_or_insert_with(RunProps::default), f);
            }
        }
        if let Some(pp) = &p.props {
            merge_ppr(&mut out, pp);
        }
        out.lvl = Some(lvl as u8);
        out
    }

    /// Effective run properties given the effective paragraph properties.
    pub fn run(ppr: &ParaProps, r: &RunProps) -> RunProps {
        let mut out = ppr.def_rpr.clone().unwrap_or_default();
        merge_rpr(&mut out, r);
        out
    }
}

/// Should master/layout non-placeholder shapes be drawn behind this slide?
/// §19.3.1.38 `showMasterSp` on the slide (root attribute) and layout.
pub fn show_master_shapes(pres: &Presentation, slide: &Slide) -> (bool, bool) {
    let slide_flag = slide
        .info
        .root_attrs
        .iter()
        .find(|(k, _)| k == "showMasterSp")
        .and_then(|(_, v)| bo_drawingml::dom::parse_bool(v))
        .unwrap_or(true);
    let layout_flag = pres
        .layouts
        .get(slide.layout)
        .and_then(|l| l.show_master_sp)
        .unwrap_or(true);
    (slide_flag, slide_flag && layout_flag)
}
