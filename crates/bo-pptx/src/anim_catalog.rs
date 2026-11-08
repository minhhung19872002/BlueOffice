//! The animation effects BlueSlide creates (ECMA-376 Part 1, §19.5): entrance, exit, emphasis and
//! motion-path effects, each written as standard behaviours (§19.5.66 set, §19.5.1 anim,
//! §19.5.3 animEffect filters, §19.5.4 animMotion, §19.5.5 animRot, §19.5.6 animScale,
//! §19.5.2 animClr) so any reader can play them. `presetID` / `presetSubtype` carry the preset
//! numbers producers use to name effects (presetClass + presetID); the behaviours are BlueSlide's own.

use crate::anim::{anim_effect, anim_pos, attr_names, tgt, Ids};
use bo_drawingml::dom::El;
use bo_model::slide::XmlElement;

/// Class of an effect (§19.5.29 cTn@presetClass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Brings the shape in (`entr`); the same effect run backwards takes it out (`exit`).
    EntranceExit,
    /// `emph`.
    Emphasis,
    /// `path`.
    Path,
}

/// One effect of the catalog.
#[derive(Debug, Clone, Copy)]
pub struct EffectDef {
    /// Stable id (`zoom`, `spin`, `lineDown`, …).
    pub id: &'static str,
    /// Class.
    pub class: Class,
    /// presetID.
    pub preset: u32,
    /// Variants (directions, …) with their presetSubtype (the first is the default).
    pub dirs: &'static [(&'static str, Option<u32>)],
}

const SIDES: &[(&str, Option<u32>)] = &[
    ("b", Some(4)),
    ("t", Some(1)),
    ("l", Some(8)),
    ("r", Some(2)),
];
const HV: &[(&str, Option<u32>)] = &[("horz", Some(10)), ("vert", Some(5))];
const INOUT: &[(&str, Option<u32>)] = &[("in", Some(16)), ("out", Some(32))];
const NONE: &[(&str, Option<u32>)] = &[];

const fn d(
    id: &'static str,
    class: Class,
    preset: u32,
    dirs: &'static [(&'static str, Option<u32>)],
) -> EffectDef {
    EffectDef {
        id,
        class,
        preset,
        dirs,
    }
}

use Class::{Emphasis as EM, EntranceExit as EE, Path as PA};

/// Every effect BlueSlide creates.
pub const EFFECTS: &[EffectDef] = &[
    d("appear", EE, 1, NONE),
    d("flyIn", EE, 2, SIDES),
    d("blinds", EE, 3, HV),
    d("box", EE, 4, INOUT),
    d(
        "checkerboard",
        EE,
        5,
        &[("across", Some(10)), ("down", Some(5))],
    ),
    d("circle", EE, 6, INOUT),
    d("crawl", EE, 7, SIDES),
    d("diamond", EE, 8, INOUT),
    d("dissolve", EE, 9, NONE),
    d("fade", EE, 10, NONE),
    d("peek", EE, 12, SIDES),
    d("plus", EE, 13, INOUT),
    d("randomBars", EE, 14, HV),
    d(
        "split",
        EE,
        16,
        &[
            ("horzIn", Some(26)),
            ("horzOut", Some(42)),
            ("vertIn", Some(21)),
            ("vertOut", Some(37)),
        ],
    ),
    d(
        "strips",
        EE,
        18,
        &[
            ("ld", Some(12)),
            ("lu", Some(9)),
            ("rd", Some(6)),
            ("ru", Some(3)),
        ],
    ),
    d("swivel", EE, 19, &[("horz", Some(10)), ("vert", Some(5))]),
    d("wedge", EE, 20, NONE),
    d(
        "wheel",
        EE,
        21,
        &[
            ("1", Some(1)),
            ("2", Some(2)),
            ("3", Some(3)),
            ("4", Some(4)),
            ("8", Some(8)),
        ],
    ),
    d("wipe", EE, 22, SIDES),
    d("zoom", EE, 23, &[("in", Some(16)), ("out", Some(32))]),
    d("bounce", EE, 26, NONE),
    d("float", EE, 30, &[("up", None), ("down", None)]),
    d("growTurn", EE, 31, NONE),
    d("spinner", EE, 49, NONE),
    d("expand", EE, 55, NONE),
    d(
        "growShrink",
        EM,
        6,
        &[("150", None), ("50", None), ("200", None)],
    ),
    d("spin", EM, 8, &[("cw", None), ("ccw", None)]),
    d(
        "transparency",
        EM,
        9,
        &[("50", None), ("25", None), ("75", None)],
    ),
    d("pulse", EM, 26, NONE),
    d("teeter", EM, 32, NONE),
    d("blink", EM, 35, NONE),
    d(
        "fillColor",
        EM,
        1,
        &[
            ("FF0000", None),
            ("FFC000", None),
            ("00B050", None),
            ("0070C0", None),
        ],
    ),
    d("lineDown", PA, 42, NONE),
    d("lineUp", PA, 42, NONE),
    d("lineLeft", PA, 42, NONE),
    d("lineRight", PA, 42, NONE),
    d("arcDown", PA, 37, NONE),
    d("arcUp", PA, 37, NONE),
    d("turnDown", PA, 39, NONE),
    d("circlePath", PA, 3, NONE),
    d("squarePath", PA, 6, NONE),
    d("diamondPath", PA, 4, NONE),
    d("triangle", PA, 7, NONE),
    d("loop", PA, 35, NONE),
    d("wave", PA, 62, NONE),
];

/// The effect of id `id`.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static EffectDef> {
    EFFECTS.iter().find(|e| e.id == id)
}

/// The effect a file names by presetClass and presetID (entrance and exit share their ids).
#[must_use]
pub fn recognise(class: &str, preset: u32) -> Option<&'static EffectDef> {
    let want = match class {
        "entr" | "exit" => EE,
        "emph" => EM,
        "path" => PA,
        _ => return None,
    };
    // Motion paths share preset numbers; the path decides: leave them unnamed.
    (want != PA)
        .then(|| {
            EFFECTS
                .iter()
                .find(|e| e.class == want && e.preset == preset)
        })
        .flatten()
}

/// The variant of presetSubtype `sub`.
#[must_use]
pub fn dir_of(def: &EffectDef, sub: Option<u32>) -> Option<&'static str> {
    def.dirs
        .iter()
        .find(|(_, s)| s.is_some() && *s == sub)
        .or_else(|| def.dirs.first())
        .map(|(v, _)| *v)
}

/// presetSubtype of variant `dir`.
#[must_use]
pub fn subtype(def: &EffectDef, dir: Option<&str>) -> Option<u32> {
    def.dirs
        .iter()
        .find(|(v, _)| Some(*v) == dir)
        .or_else(|| def.dirs.first())
        .and_then(|(_, s)| *s)
        .or(Some(0))
}

/// Keyframes of one attribute (§19.5.79 tav: time in 1000ths of a percent, value as a formula).
fn anim_keys(ids: &mut Ids, spid: u32, dur: u32, attr: &str, keys: &[(u32, &str)]) -> XmlElement {
    let mut list = El::new("p:tavLst");
    for (tm, v) in keys {
        list = list.child(
            El::new("p:tav")
                .a("tm", tm)
                .child(
                    El::new("p:val")
                        .child(El::new("p:strVal").a("val", v).done())
                        .done(),
                )
                .done(),
        );
    }
    El::new("p:anim")
        .a("calcmode", "lin")
        .a("valueType", "num")
        .child(
            El::new("p:cBhvr")
                .child(
                    El::new("p:cTn")
                        .a("id", ids.next())
                        .a("dur", dur)
                        .a("fill", "hold")
                        .done(),
                )
                .child(tgt(spid))
                .child(attr_names(attr))
                .done(),
        )
        .child(list.done())
        .done()
}

fn bhvr(ids: &mut Ids, spid: u32, dur: u32, extra: &[(&str, String)]) -> XmlElement {
    let mut ctn = El::new("p:cTn")
        .a("id", ids.next())
        .a("dur", dur)
        .a("fill", "hold");
    for (k, v) in extra {
        ctn = ctn.a(k, v);
    }
    El::new("p:cBhvr").child(ctn.done()).child(tgt(spid)).done()
}

/// The behaviours of effect `def` (variant `dir`, `dur` ms) on shape `spid`; `exit` runs an
/// entrance effect backwards. Visibility sets come from the caller.
#[must_use]
#[allow(clippy::too_many_lines)]
pub(crate) fn behaviours(
    ids: &mut Ids,
    def: &EffectDef,
    dir: Option<&str>,
    spid: u32,
    dur: u32,
    exit: bool,
) -> Vec<XmlElement> {
    let dir = dir
        .filter(|d| def.dirs.iter().any(|(v, _)| v == d))
        .or_else(|| def.dirs.first().map(|(v, _)| *v))
        .unwrap_or("");
    let tr = if exit { "out" } else { "in" };
    let filter = |ids: &mut Ids, f: &str| anim_effect(ids, spid, dur, f, tr);
    // An entrance animates from → to; an exit to → from.
    let pos = |ids: &mut Ids, attr: &str, from: &str, to: &str| {
        if exit {
            anim_pos(ids, spid, dur, attr, to, from)
        } else {
            anim_pos(ids, spid, dur, attr, from, to)
        }
    };
    let keys = |ids: &mut Ids, attr: &str, k: &[(u32, &str)]| {
        if exit {
            let rev: Vec<(u32, &str)> = k.iter().rev().map(|(t, v)| (100_000 - t, *v)).collect();
            anim_keys(ids, spid, dur, attr, &rev)
        } else {
            anim_keys(ids, spid, dur, attr, k)
        }
    };
    let side = |d: &str| match d {
        "t" => ("ppt_y", "0-#ppt_h/2"),
        "l" => ("ppt_x", "0-#ppt_w/2"),
        "r" => ("ppt_x", "1+#ppt_w/2"),
        _ => ("ppt_y", "1+#ppt_h/2"),
    };
    let fade = |ids: &mut Ids| filter(ids, "fade");
    let motion = |ids: &mut Ids, path: &str| {
        El::new("p:animMotion")
            .a("origin", "layout")
            .a("path", path)
            .a("pathEditMode", "relative")
            .child(bhvr(ids, spid, dur, &[]))
            .done()
    };
    match def.id {
        // Crawl is a fly-in at its own (slower) default pace.
        "flyIn" | "crawl" => {
            let (attr, from) = side(dir);
            vec![pos(ids, attr, from, &format!("#{attr}"))]
        }
        "blinds" => vec![filter(ids, if dir == "vert" { "blinds(vertical)" } else { "blinds(horizontal)" })],
        "box" | "circle" | "diamond" | "plus" => vec![filter(ids, &format!("{}({dir})", def.id))],
        "checkerboard" => vec![filter(ids, &format!("checkerboard({dir})"))],
        "dissolve" => vec![filter(ids, "dissolve")],
        "fade" => vec![fade(ids)],
        "peek" => {
            let f = match dir {
                "t" => "slide(fromTop)",
                "l" => "slide(fromLeft)",
                "r" => "slide(fromRight)",
                _ => "slide(fromBottom)",
            };
            vec![filter(ids, f)]
        }
        "randomBars" => vec![filter(ids, if dir == "vert" { "randombar(vertical)" } else { "randombar(horizontal)" })],
        "split" => {
            let f = match dir {
                "horzOut" => "barn(outHorizontal)",
                "vertIn" => "barn(inVertical)",
                "vertOut" => "barn(outVertical)",
                _ => "barn(inHorizontal)",
            };
            vec![filter(ids, f)]
        }
        "strips" => {
            let f = match dir {
                "lu" => "strips(upLeft)",
                "rd" => "strips(downRight)",
                "ru" => "strips(upRight)",
                _ => "strips(downLeft)",
            };
            vec![filter(ids, f)]
        }
        "wedge" => vec![filter(ids, "wedge")],
        "wheel" => vec![filter(ids, &format!("wheel({dir})"))],
        "wipe" => {
            let f = match dir {
                "t" => "wipe(down)",
                "l" => "wipe(right)",
                "r" => "wipe(left)",
                _ => "wipe(up)",
            };
            vec![filter(ids, f)]
        }
        "zoom" => {
            let s = if dir == "out" { "#ppt_w*1.5" } else { "0" };
            let h = if dir == "out" { "#ppt_h*1.5" } else { "0" };
            vec![pos(ids, "ppt_w", s, "#ppt_w"), pos(ids, "ppt_h", h, "#ppt_h"), fade(ids)]
        }
        "swivel" => {
            let attr = if dir == "vert" { "ppt_h" } else { "ppt_w" };
            let full = format!("#{attr}");
            vec![
                keys(
                    ids,
                    attr,
                    &[(0, "0"), (25_000, &full), (50_000, "0"), (75_000, &full), (100_000, &full)],
                ),
                fade(ids),
            ]
        }
        "bounce" => vec![
            keys(
                ids,
                "ppt_y",
                &[
                    (0, "0-#ppt_h/2"),
                    (40_000, "#ppt_y"),
                    (55_000, "#ppt_y-0.06"),
                    (70_000, "#ppt_y"),
                    (82_000, "#ppt_y-0.02"),
                    (100_000, "#ppt_y"),
                ],
            ),
            fade(ids),
        ],
        "float" => {
            let from = if dir == "down" { "#ppt_y-0.1" } else { "#ppt_y+0.1" };
            vec![pos(ids, "ppt_y", from, "#ppt_y"), fade(ids)]
        }
        "growTurn" => vec![
            pos(ids, "ppt_w", "0", "#ppt_w"),
            pos(ids, "ppt_h", "0", "#ppt_h"),
            pos(ids, "style.rotation", "90", "0"),
            fade(ids),
        ],
        "spinner" => vec![
            pos(ids, "ppt_w", "#ppt_w*0.7", "#ppt_w"),
            pos(ids, "ppt_h", "#ppt_h*0.7", "#ppt_h"),
            pos(ids, "style.rotation", "-90", "0"),
            fade(ids),
        ],
        "expand" => vec![pos(ids, "ppt_w", "#ppt_w*0.7", "#ppt_w"), fade(ids)],
        // Emphasis.
        "growShrink" => {
            let k = dir.parse::<u32>().unwrap_or(150) * 1000;
            vec![El::new("p:animScale")
                .child(bhvr(ids, spid, dur, &[]))
                .child(El::new("p:by").a("x", k).a("y", k).done())
                .done()]
        }
        "pulse" => vec![El::new("p:animScale")
            .child(bhvr(ids, spid, dur / 2, &[("autoRev", "1".into())]))
            .child(El::new("p:by").a("x", 105_000).a("y", 105_000).done())
            .done()],
        "spin" => {
            let by = if dir == "ccw" { -21_600_000 } else { 21_600_000 };
            vec![El::new("p:animRot").a("by", by).child(bhvr(ids, spid, dur, &[])).done()]
        }
        "teeter" => vec![anim_keys(
            ids,
            spid,
            dur,
            "style.rotation",
            &[(0, "0"), (20_000, "4"), (40_000, "-4"), (60_000, "4"), (80_000, "-4"), (100_000, "0")],
        )],
        "transparency" => {
            let o = 1.0 - f64::from(dir.parse::<u32>().unwrap_or(50)) / 100.0;
            vec![El::new("p:set")
                .child(
                    El::new("p:cBhvr")
                        .child(El::new("p:cTn").a("id", ids.next()).a("dur", dur).a("fill", "hold").done())
                        .child(tgt(spid))
                        .child(attr_names("style.opacity"))
                        .done(),
                )
                .child(El::new("p:to").child(El::new("p:strVal").a("val", o).done()).done())
                .done()]
        }
        "blink" => vec![El::new("p:anim")
            .a("calcmode", "discrete")
            .a("valueType", "num")
            .child(
                El::new("p:cBhvr")
                    .child(El::new("p:cTn").a("id", ids.next()).a("dur", dur).a("repeatCount", 3000).done())
                    .child(tgt(spid))
                    .child(attr_names("style.opacity"))
                    .done(),
            )
            .child(
                El::new("p:tavLst")
                    .child(
                        El::new("p:tav")
                            .a("tm", 0)
                            .child(El::new("p:val").child(El::new("p:fltVal").a("val", 0).done()).done())
                            .done(),
                    )
                    .child(
                        El::new("p:tav")
                            .a("tm", 50_000)
                            .child(El::new("p:val").child(El::new("p:fltVal").a("val", 1).done()).done())
                            .done(),
                    )
                    .done(),
            )
            .done()],
        "fillColor" => vec![El::new("p:animClr")
            .a("clrSpc", "rgb")
            .child(
                El::new("p:cBhvr")
                    .child(El::new("p:cTn").a("id", ids.next()).a("dur", dur).a("fill", "hold").done())
                    .child(tgt(spid))
                    .child(attr_names("fillcolor"))
                    .done(),
            )
            .child(El::new("p:to").child(El::new("a:srgbClr").a("val", dir).done()).done())
            .done()],
        // Motion paths (§19.5.4 path in slide widths / heights from the shape's position).
        "lineDown" => vec![motion(ids, "M 0 0 L 0 0.25 E")],
        "lineUp" => vec![motion(ids, "M 0 0 L 0 -0.25 E")],
        "lineLeft" => vec![motion(ids, "M 0 0 L -0.25 0 E")],
        "lineRight" => vec![motion(ids, "M 0 0 L 0.25 0 E")],
        "arcDown" => vec![motion(ids, "M 0 0 C 0.05 -0.2 0.2 -0.2 0.25 0 E")],
        "arcUp" => vec![motion(ids, "M 0 0 C 0.05 0.2 0.2 0.2 0.25 0 E")],
        "turnDown" => vec![motion(ids, "M 0 0 L 0.125 0 C 0.2 0 0.25 0.05 0.25 0.125 L 0.25 0.25 E")],
        "circlePath" => vec![motion(
            ids,
            "M 0 0 C 0.055 0 0.1 0.045 0.1 0.1 C 0.1 0.155 0.055 0.2 0 0.2 C -0.055 0.2 -0.1 0.155 -0.1 0.1 C -0.1 0.045 -0.055 0 0 0 Z",
        )],
        "squarePath" => vec![motion(ids, "M 0 0 L 0.15 0 L 0.15 0.2 L 0 0.2 L 0 0 Z")],
        "diamondPath" => vec![motion(ids, "M 0 0 L 0.1 0.12 L 0 0.24 L -0.1 0.12 L 0 0 Z")],
        "triangle" => vec![motion(ids, "M 0 0 L 0.1 0.2 L -0.1 0.2 L 0 0 Z")],
        "loop" => vec![motion(
            ids,
            "M 0 0 C 0.1 -0.15 0.2 0.15 0.1 0.15 C 0 0.15 0.1 -0.15 0.2 0 E",
        )],
        "wave" => vec![motion(
            ids,
            "M 0 0 C 0.05 -0.1 0.1 -0.1 0.125 0 C 0.15 0.1 0.2 0.1 0.25 0 C 0.3 -0.1 0.35 -0.1 0.375 0 E",
        )],
        // appear: visibility only.
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim_play::{effect_play, ShapeBase, TrackKind};

    fn wrap(b: Vec<XmlElement>) -> XmlElement {
        let mut kids = El::new("p:childTnLst");
        for x in b {
            kids = kids.child(x);
        }
        El::new("p:par")
            .child(El::new("p:cTn").a("id", 1).child(kids.done()).done())
            .done()
    }

    const BASE: ShapeBase = ShapeBase {
        x: 0.5,
        y: 0.5,
        w: 0.2,
        h: 0.2,
    };

    // source: ECMA-376 Part 1, §19.5 — every effect of the catalog, with each variant, writes
    // behaviours that the interpreter reads back into tracks lasting the effect's duration.
    #[test]
    fn every_effect_writes_playable_behaviours() {
        let mut seen = std::collections::HashSet::new();
        for e in EFFECTS {
            assert!(seen.insert(e.id), "{}", e.id);
            let dirs: Vec<Option<&str>> = if e.dirs.is_empty() {
                vec![None]
            } else {
                e.dirs.iter().map(|(v, _)| Some(*v)).collect()
            };
            for d in dirs {
                for exit in [false, true] {
                    if exit && e.class != Class::EntranceExit {
                        continue;
                    }
                    let mut ids = Ids(10);
                    let b = behaviours(&mut ids, e, d, 4, 800, exit);
                    if e.id == "appear" {
                        assert_eq!(b.len(), 0);
                        continue;
                    }
                    let p = effect_play(&wrap(b), &BASE);
                    assert!(!p.tracks.is_empty(), "{} {d:?}", e.id);
                    assert!(
                        (p.dur_ms - 800.0).abs() < 1e-9 || e.id == "blink",
                        "{} {d:?}: {}",
                        e.id,
                        p.dur_ms
                    );
                }
            }
        }
        assert!(EFFECTS.len() >= 45);
    }

    #[test]
    fn effects_do_what_they_say() {
        let play = |id: &str, d: Option<&str>, exit: bool| {
            let mut ids = Ids(10);
            effect_play(
                &wrap(behaviours(&mut ids, by_id(id).unwrap(), d, 4, 1000, exit)),
                &BASE,
            )
        };
        // Fly in from the left: x from just off the left edge to the shape's centre; out reversed.
        let p = play("flyIn", Some("l"), false);
        let TrackKind::Anim { attr, keys, .. } = &p.tracks[0].kind else {
            panic!()
        };
        assert_eq!(attr, "ppt_x");
        assert!((keys[0].1 + 0.1).abs() < 1e-12 && (keys[1].1 - 0.5).abs() < 1e-12);
        let p = play("flyIn", Some("l"), true);
        let TrackKind::Anim { keys, .. } = &p.tracks[0].kind else {
            panic!()
        };
        assert!((keys[1].1 + 0.1).abs() < 1e-12);
        // A counter-clockwise spin turns −360°.
        assert!(
            matches!(play("spin", Some("ccw"), false).tracks[0].kind, TrackKind::Rotate { by } if (by + 360.0).abs() < 1e-9)
        );
        // Grow 200 %.
        assert!(
            matches!(play("growShrink", Some("200"), false).tracks[0].kind, TrackKind::Scale { to, .. } if (to.0 - 2.0).abs() < 1e-12)
        );
        // A line down a quarter of the slide.
        let TrackKind::Motion { points } = &play("lineDown", None, false).tracks[0].kind else {
            panic!()
        };
        assert_eq!(points.last(), Some(&(0.0, 0.25)));
        // Exit filters take the shape out.
        assert!(
            matches!(&play("circle", Some("out"), true).tracks[0].kind, TrackKind::Filter { filter, entering: false } if filter == "circle(out)")
        );
        assert_eq!(recognise("entr", 23).map(|d| d.id), Some("zoom"));
        assert_eq!(recognise("emph", 8).map(|d| d.id), Some("spin"));
        assert_eq!(dir_of(by_id("wheel").unwrap(), Some(8)), Some("8"));
        assert_eq!(subtype(by_id("flyIn").unwrap(), Some("r")), Some(2));
    }
}
