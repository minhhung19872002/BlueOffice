//! Preset shape definitions — ECMA-376 Part 1, §20.1.9.18 prstGeom and the
//! presetShapeDefinitions annex (§20.1.10.56 ST_ShapeType).
//!
//! All 187 definitions live in `presets.dsl`, converted mechanically from the
//! annex XML into a compact line format that maps 1:1 onto it (`avLst`, `gdLst`, `rect`, `pathLst`):
//!
//! ```text
//! @name                      start of a shape
//! av <name> <fmla>           adjust value (default)
//! gd <name> <fmla>           guide
//! rect <l> <t> <r> <b>       text rectangle
//! path [w=] [h=] [fill=] [stroke=0] [ext=0]
//! M x y; L x y; A wR hR stAng swAng; Q x1 y1 x y; C x1 y1 x2 y2 x y; Z
//! ```

use bo_model::slide::{CustomGeometry, GeomPath, Guide, PathCommand};
use std::collections::HashMap;
use std::sync::OnceLock;

const DEFS: &str = include_str!("presets.dsl");

/// Parse definitions in the line format above (also used by [`crate::textwarp`]).
pub(crate) fn parse_defs(defs: &'static str) -> HashMap<&'static str, CustomGeometry> {
    let mut out = HashMap::new();
    let mut name: Option<&'static str> = None;
    let mut cur = CustomGeometry::default();
    let s = |v: &str| v.to_owned();
    for line in defs
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        if let Some(n) = line.strip_prefix('@') {
            if let Some(prev) = name.take() {
                out.insert(prev, std::mem::take(&mut cur));
            }
            name = Some(n);
            continue;
        }
        let mut words = line.split_whitespace();
        let head = words.next().unwrap_or_default();
        match head {
            "av" | "gd" => {
                let gname = words.next().unwrap_or_default().to_owned();
                let fmla = words.collect::<Vec<_>>().join(" ");
                let g = Guide { name: gname, fmla };
                if head == "av" {
                    cur.av.push(g);
                } else {
                    cur.gd.push(g);
                }
            }
            "rect" => {
                let v: Vec<&str> = words.collect();
                if v.len() == 4 {
                    cur.rect = Some([s(v[0]), s(v[1]), s(v[2]), s(v[3])]);
                }
            }
            "path" => {
                let mut p = GeomPath::default();
                for kv in words {
                    match kv.split_once('=') {
                        Some(("w", v)) => p.w = v.parse().ok(),
                        Some(("h", v)) => p.h = v.parse().ok(),
                        Some(("fill", v)) => p.fill = Some(v.to_owned()),
                        Some(("stroke", v)) => p.stroke = Some(v != "0"),
                        Some(("ext", v)) => p.extrusion_ok = Some(v != "0"),
                        _ => {}
                    }
                }
                cur.paths.push(p);
            }
            _ => {
                let Some(p) = cur.paths.last_mut() else {
                    continue;
                };
                for cmd in line.split(';').map(str::trim).filter(|c| !c.is_empty()) {
                    let a: Vec<&str> = cmd.split_whitespace().collect();
                    let c = match (a[0], a.len()) {
                        ("M", 3) => PathCommand::MoveTo(s(a[1]), s(a[2])),
                        ("L", 3) => PathCommand::LineTo(s(a[1]), s(a[2])),
                        ("A", 5) => PathCommand::ArcTo {
                            w_r: s(a[1]),
                            h_r: s(a[2]),
                            st_ang: s(a[3]),
                            sw_ang: s(a[4]),
                        },
                        ("Q", 5) => PathCommand::QuadTo([s(a[1]), s(a[2]), s(a[3]), s(a[4])]),
                        ("C", 7) => PathCommand::CubicTo([
                            s(a[1]),
                            s(a[2]),
                            s(a[3]),
                            s(a[4]),
                            s(a[5]),
                            s(a[6]),
                        ]),
                        ("Z", 1) => PathCommand::Close,
                        _ => continue,
                    };
                    p.commands.push(c);
                }
            }
        }
    }
    if let Some(prev) = name {
        out.insert(prev, cur);
    }
    out
}

fn table() -> &'static HashMap<&'static str, CustomGeometry> {
    static T: OnceLock<HashMap<&'static str, CustomGeometry>> = OnceLock::new();
    T.get_or_init(|| parse_defs(DEFS))
}

/// Definition of a preset shape (`None` when not implemented).
pub fn preset(name: &str) -> Option<CustomGeometry> {
    table().get(name).cloned()
}

/// Names of all implemented presets, sorted.
pub fn preset_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = table().keys().copied().collect();
    v.sort_unstable();
    v
}
