//! The transitions BlueSlide writes and recognises: the effect children of ECMA-376 Part 1, §19.3.1.50
//! transition (§19.3.1.2 blinds … §19.3.1.54 zoom), the PowerPoint 2010 effects of [MS-PPTX] 2.3 (`p14:`
//! namespace), the preset transitions of [MS-PPTX] (`p15:prstTrans`) and Morph (`p159:morph`).
//!
//! Each entry has a stable id (what the engine and the UI use), the element, the attributes that
//! identify it (a `p15:prstTrans` preset, the [MS-PPTX] `p14:prism` flags) and its variants: the
//! values of its direction-like attributes (`dir`, `orient`, `thruBlk`, `spokes`, `pattern`, `invX`).

/// One transition of the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionDef {
    /// Stable id (`fade`, `vortex`, `fallOver`, …).
    pub id: &'static str,
    /// Qualified element name (`p:fade`, `p14:vortex`, `p15:prstTrans`).
    pub element: &'static str,
    /// Attributes that identify the entry among those sharing its element.
    pub fixed: &'static [(&'static str, &'static str)],
    /// Variants: id and the attributes written for it (the first is the default).
    pub variants: &'static [(&'static str, &'static [(&'static str, &'static str)])],
    /// Values of attributes left out (schema defaults), for recognising a variant.
    pub defaults: &'static [(&'static str, &'static str)],
}

type V = (&'static str, &'static [(&'static str, &'static str)]);

const SIDES: &[V] = &[
    ("l", &[("dir", "l")]),
    ("u", &[("dir", "u")]),
    ("r", &[("dir", "r")]),
    ("d", &[("dir", "d")]),
];
const EIGHT: &[V] = &[
    ("l", &[("dir", "l")]),
    ("u", &[("dir", "u")]),
    ("r", &[("dir", "r")]),
    ("d", &[("dir", "d")]),
    ("lu", &[("dir", "lu")]),
    ("ru", &[("dir", "ru")]),
    ("ld", &[("dir", "ld")]),
    ("rd", &[("dir", "rd")]),
];
const LEFT_RIGHT: &[V] = &[("l", &[("dir", "l")]), ("r", &[("dir", "r")])];
const HORZ_VERT: &[V] = &[("horz", &[("dir", "horz")]), ("vert", &[("dir", "vert")])];
const IN_OUT: &[V] = &[("in", &[("dir", "in")]), ("out", &[("dir", "out")])];
const CORNERS: &[V] = &[
    ("lu", &[("dir", "lu")]),
    ("ru", &[("dir", "ru")]),
    ("ld", &[("dir", "ld")]),
    ("rd", &[("dir", "rd")]),
];
const BLACK: &[V] = &[("smooth", &[]), ("black", &[("thruBlk", "1")])];
const SPOKES: &[V] = &[
    ("1", &[("spokes", "1")]),
    ("2", &[("spokes", "2")]),
    ("3", &[("spokes", "3")]),
    ("4", &[("spokes", "4")]),
    ("8", &[("spokes", "8")]),
];
/// [MS-PPTX] p15:prstTrans: `invX` mirrors the preset (from the right instead of the left).
const MIRROR: &[V] = &[("l", &[]), ("r", &[("invX", "1")])];
const NONE: &[V] = &[];

const fn def(
    id: &'static str,
    element: &'static str,
    fixed: &'static [(&'static str, &'static str)],
    variants: &'static [V],
    defaults: &'static [(&'static str, &'static str)],
) -> TransitionDef {
    TransitionDef {
        id,
        element,
        fixed,
        variants,
        defaults,
    }
}

const fn preset(id: &'static str, prst: &'static [(&'static str, &'static str)]) -> TransitionDef {
    def(id, "p15:prstTrans", prst, MIRROR, &[("invX", "0")])
}

/// Every transition, ECMA-376 first, then [MS-PPTX] p14 and p15.
pub const CATALOG: &[TransitionDef] = &[
    // ECMA-376 Part 1, §19.3.1.
    def("cut", "p:cut", &[], BLACK, &[("thruBlk", "0")]),
    def("fade", "p:fade", &[], BLACK, &[("thruBlk", "0")]),
    def("push", "p:push", &[], SIDES, &[("dir", "l")]),
    def("wipe", "p:wipe", &[], SIDES, &[("dir", "l")]),
    def(
        "split",
        "p:split",
        &[],
        &[
            ("horz", &[("orient", "horz"), ("dir", "out")]),
            ("vert", &[("orient", "vert"), ("dir", "out")]),
            ("horzIn", &[("orient", "horz"), ("dir", "in")]),
            ("vertIn", &[("orient", "vert"), ("dir", "in")]),
        ],
        &[("orient", "horz"), ("dir", "out")],
    ),
    def("cover", "p:cover", &[], EIGHT, &[("dir", "l")]),
    def("pull", "p:pull", &[], EIGHT, &[("dir", "l")]),
    def("random", "p:random", &[], NONE, &[]),
    def(
        "randomBar",
        "p:randomBar",
        &[],
        HORZ_VERT,
        &[("dir", "horz")],
    ),
    def("blinds", "p:blinds", &[], HORZ_VERT, &[("dir", "horz")]),
    def("checker", "p:checker", &[], HORZ_VERT, &[("dir", "horz")]),
    def("comb", "p:comb", &[], HORZ_VERT, &[("dir", "horz")]),
    def("circle", "p:circle", &[], NONE, &[]),
    def("diamond", "p:diamond", &[], NONE, &[]),
    def("plus", "p:plus", &[], NONE, &[]),
    def("wedge", "p:wedge", &[], NONE, &[]),
    def("dissolve", "p:dissolve", &[], NONE, &[]),
    def("newsflash", "p:newsflash", &[], NONE, &[]),
    def("strips", "p:strips", &[], CORNERS, &[("dir", "lu")]),
    def("wheel", "p:wheel", &[], SPOKES, &[("spokes", "4")]),
    def("zoom", "p:zoom", &[], IN_OUT, &[("dir", "out")]),
    // [MS-PPTX] 2.3 (PowerPoint 2010).
    def(
        "reveal",
        "p14:reveal",
        &[],
        &[
            ("l", &[("dir", "l")]),
            ("r", &[("dir", "r")]),
            ("lBlack", &[("thruBlk", "1"), ("dir", "l")]),
            ("rBlack", &[("thruBlk", "1"), ("dir", "r")]),
        ],
        &[("thruBlk", "0"), ("dir", "l")],
    ),
    def("flash", "p14:flash", &[], NONE, &[]),
    def("vortex", "p14:vortex", &[], SIDES, &[("dir", "l")]),
    def("switch", "p14:switch", &[], LEFT_RIGHT, &[("dir", "r")]),
    def("flip", "p14:flip", &[], LEFT_RIGHT, &[("dir", "r")]),
    def(
        "ripple",
        "p14:ripple",
        &[],
        &[
            ("center", &[("dir", "center")]),
            ("lu", &[("dir", "lu")]),
            ("ru", &[("dir", "ru")]),
            ("ld", &[("dir", "ld")]),
            ("rd", &[("dir", "rd")]),
        ],
        &[("dir", "center")],
    ),
    def("honeycomb", "p14:honeycomb", &[], NONE, &[]),
    def(
        "glitter",
        "p14:glitter",
        &[],
        &[
            ("l", &[("dir", "l"), ("pattern", "diamond")]),
            ("u", &[("dir", "u"), ("pattern", "diamond")]),
            ("r", &[("dir", "r"), ("pattern", "diamond")]),
            ("d", &[("dir", "d"), ("pattern", "diamond")]),
            ("lHex", &[("dir", "l"), ("pattern", "hexagon")]),
            ("rHex", &[("dir", "r"), ("pattern", "hexagon")]),
        ],
        &[("dir", "l"), ("pattern", "diamond")],
    ),
    def(
        "shred",
        "p14:shred",
        &[],
        &[
            ("stripIn", &[("pattern", "strip"), ("dir", "in")]),
            ("stripOut", &[("pattern", "strip"), ("dir", "out")]),
            ("rectangleIn", &[("pattern", "rectangle"), ("dir", "in")]),
            ("rectangleOut", &[("pattern", "rectangle"), ("dir", "out")]),
        ],
        &[("pattern", "strip"), ("dir", "in")],
    ),
    def("gallery", "p14:gallery", &[], LEFT_RIGHT, &[("dir", "l")]),
    // p14:prism: a cube; inverted, a box; turning the content only, rotate / orbit.
    def(
        "cube",
        "p14:prism",
        &[("isContent", "0"), ("isInverted", "0")],
        SIDES,
        &[("dir", "l"), ("isContent", "0"), ("isInverted", "0")],
    ),
    def(
        "box",
        "p14:prism",
        &[("isContent", "0"), ("isInverted", "1")],
        SIDES,
        &[("dir", "l"), ("isContent", "0"), ("isInverted", "0")],
    ),
    def(
        "rotate",
        "p14:prism",
        &[("isContent", "1"), ("isInverted", "0")],
        SIDES,
        &[("dir", "l"), ("isContent", "0"), ("isInverted", "0")],
    ),
    def(
        "orbit",
        "p14:prism",
        &[("isContent", "1"), ("isInverted", "1")],
        SIDES,
        &[("dir", "l"), ("isContent", "0"), ("isInverted", "0")],
    ),
    def("doors", "p14:doors", &[], HORZ_VERT, &[("dir", "vert")]),
    def("window", "p14:window", &[], HORZ_VERT, &[("dir", "vert")]),
    def("ferris", "p14:ferris", &[], LEFT_RIGHT, &[("dir", "l")]),
    def("conveyor", "p14:conveyor", &[], LEFT_RIGHT, &[("dir", "l")]),
    def("pan", "p14:pan", &[], SIDES, &[("dir", "d")]),
    def("warp", "p14:warp", &[], IN_OUT, &[("dir", "in")]),
    def(
        "flythrough",
        "p14:flythrough",
        &[],
        &[
            ("in", &[("dir", "in")]),
            ("out", &[("dir", "out")]),
            ("inBounce", &[("dir", "in"), ("hasBounce", "1")]),
            ("outBounce", &[("dir", "out"), ("hasBounce", "1")]),
        ],
        &[("dir", "in"), ("hasBounce", "0")],
    ),
    def(
        "wheelReverse",
        "p14:wheelReverse",
        &[],
        SPOKES,
        &[("spokes", "4")],
    ),
    // [MS-PPTX] p159:morph (PowerPoint 2016+): by object, word or character.
    def(
        "morph",
        "p159:morph",
        &[],
        &[
            ("byObject", &[("option", "byObject")]),
            ("byWord", &[("option", "byWord")]),
            ("byChar", &[("option", "byChar")]),
        ],
        &[("option", "byObject")],
    ),
    // [MS-PPTX] p15:prstTrans presets.
    preset("fallOver", &[("prst", "fallOver")]),
    preset("drape", &[("prst", "drape")]),
    preset("curtains", &[("prst", "curtains")]),
    preset("wind", &[("prst", "wind")]),
    preset("prestige", &[("prst", "prestige")]),
    preset("fracture", &[("prst", "fracture")]),
    preset("crush", &[("prst", "crush")]),
    preset("peelOff", &[("prst", "peelOff")]),
    preset("pageCurlDouble", &[("prst", "pageCurlDouble")]),
    preset("pageCurlSingle", &[("prst", "pageCurlSingle")]),
    preset("airplane", &[("prst", "airplane")]),
    preset("origami", &[("prst", "origami")]),
];

/// The entry of id `id`.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static TransitionDef> {
    CATALOG.iter().find(|d| d.id == id)
}

fn value<'a>(attrs: &'a [(String, String)], def: &'a TransitionDef, name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .or_else(|| {
            def.defaults
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| *v)
        })
}

/// Boolean attribute values (`1`/`true`, `0`/`false`) compare by meaning.
fn same(a: &str, b: &str) -> bool {
    fn norm(v: &str) -> &str {
        match v {
            "true" => "1",
            "false" => "0",
            v => v,
        }
    }
    norm(a) == norm(b)
}

/// The entry of effect element `element` with attributes `attrs` (the [MS-PPTX] `p15:prstTrans` preset
/// defaults to `fallOver`).
#[must_use]
pub fn recognise(element: &str, attrs: &[(String, String)]) -> Option<&'static TransitionDef> {
    CATALOG.iter().find(|d| {
        d.element == element
            && d.fixed.iter().all(|(k, v)| {
                let got = attrs
                    .iter()
                    .find(|(a, _)| a == k)
                    .map(|(_, x)| x.as_str())
                    .or_else(|| (*k == "prst").then_some("fallOver"))
                    .or_else(|| d.defaults.iter().find(|(a, _)| a == k).map(|(_, x)| *x))
                    .unwrap_or("0");
                same(got, v)
            })
    })
}

/// The variant of `def` that `attrs` select (`None` when no variant matches, or the entry has none).
#[must_use]
pub fn variant_of(def: &TransitionDef, attrs: &[(String, String)]) -> Option<&'static str> {
    def.variants
        .iter()
        .find(|(_, va)| {
            va.iter()
                .all(|(k, v)| value(attrs, def, k).is_some_and(|x| same(x, v)))
                // A variant without attributes (`smooth`) needs the others' attributes unset.
                && def.variants.iter().flat_map(|(_, o)| o.iter()).all(|(k, _)| {
                    va.iter().any(|(vk, _)| vk == k)
                        || value(attrs, def, k).is_none_or(|x| {
                            def.defaults.iter().any(|(dk, dv)| dk == k && same(dv, x))
                        })
                })
        })
        .map(|(id, _)| *id)
}

/// The attributes written for `def` with variant `variant` (unknown or absent: the default variant).
#[must_use]
pub fn attrs_for(def: &TransitionDef, variant: Option<&str>) -> Vec<(String, String)> {
    let v = def
        .variants
        .iter()
        .find(|(id, _)| Some(*id) == variant)
        .or_else(|| def.variants.first());
    let mut out: Vec<(String, String)> = Vec::new();
    for (k, x) in def.fixed.iter().chain(v.map_or(&[][..], |(_, a)| *a)) {
        if !out.iter().any(|(a, _)| a == k) {
            out.push(((*k).to_owned(), (*x).to_owned()));
        }
    }
    out
}

/// The variant ids of `def`.
#[must_use]
pub fn variants(def: &TransitionDef) -> Vec<&'static str> {
    def.variants.iter().map(|(id, _)| *id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    // source: ECMA-376 Part 1, §19.3.1 (transition effects) and [MS-PPTX] 2.3 — every entry written
    // with each variant is recognised as itself and that variant.
    #[test]
    fn every_entry_and_variant_round_trips() {
        let mut ids = std::collections::HashSet::new();
        for d in CATALOG {
            assert!(ids.insert(d.id), "duplicate {}", d.id);
            let vs = variants(d);
            if vs.is_empty() {
                let w = attrs_for(d, None);
                assert_eq!(recognise(d.element, &w).map(|x| x.id), Some(d.id));
            }
            for v in vs {
                let w = attrs_for(d, Some(v));
                assert_eq!(
                    recognise(d.element, &w).map(|x| x.id),
                    Some(d.id),
                    "{} {v}",
                    d.id
                );
                assert_eq!(variant_of(d, &w), Some(v), "{} {v}: {w:?}", d.id);
            }
        }
        assert!(CATALOG.len() >= 56, "{}", CATALOG.len());
    }

    #[test]
    fn defaults_select_variants() {
        // §19.3.1.47 split: orient horz, dir out by default.
        let split = by_id("split").unwrap();
        assert_eq!(variant_of(split, &a(&[])), Some("horz"));
        assert_eq!(variant_of(split, &a(&[("orient", "vert")])), Some("vert"));
        assert_eq!(variant_of(split, &a(&[("dir", "in")])), Some("horzIn"));
        // §19.3.1.20 fade@thruBlk.
        let fade = by_id("fade").unwrap();
        assert_eq!(variant_of(fade, &a(&[])), Some("smooth"));
        assert_eq!(variant_of(fade, &a(&[("thruBlk", "true")])), Some("black"));
        // [MS-PPTX] p14:prism flags pick cube / box / rotate / orbit.
        assert_eq!(recognise("p14:prism", &a(&[])).map(|d| d.id), Some("cube"));
        assert_eq!(
            recognise("p14:prism", &a(&[("isInverted", "1")])).map(|d| d.id),
            Some("box")
        );
        assert_eq!(
            recognise(
                "p14:prism",
                &a(&[("isContent", "true"), ("isInverted", "true")])
            )
            .map(|d| d.id),
            Some("orbit")
        );
        assert_eq!(
            recognise("p15:prstTrans", &a(&[("prst", "airplane")])).map(|d| d.id),
            Some("airplane")
        );
        assert_eq!(
            recognise("p15:prstTrans", &a(&[])).map(|d| d.id),
            Some("fallOver")
        );
        assert_eq!(recognise("p:unknown", &a(&[])), None);
    }
}
