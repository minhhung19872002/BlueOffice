//! 3-D properties of a shape (ECMA-376 Part 1, §20.1.5 "3-D properties"), read into the camera, solid
//! and look that [`crate::mesh3d`] draws as a real extruded and bevelled solid:
//!
//! * the camera (§20.1.5.5 `camera`, preset names of §20.1.10.47 `ST_PresetCameraType`, explicit
//!   §20.1.5.11 `rot`, `fov` for the perspective presets) becomes a view rotation (orthographic,
//!   oblique / cabinet, or perspective);
//! * the extrusion (§20.1.5.9 `sp3d@extrusionH`, `extrusionClr`), the top bevel (§20.1.5.2 `bevelT` with
//!   `w`, `h`, `prst`) and the contour (`contourW`, `contourClr`) become the solid;
//! * the light rig direction (§20.1.5.10 `lightRig@dir`) and the material (§20.1.10.51
//!   `ST_PresetMaterialType`) become a light vector and a specular term.
//!
//! The specification only names the camera presets and materials; the angles, field of view and
//! specular strengths used for them are chosen so that the family reads right (isometric = 35.26°/45°,
//! perspective ≈ 20°, fov 45°), not measured. The XML stays untouched in `extra` (lossless round trip).

use bo_model::slide::{Color, Extra, XmlElement};

use crate::mesh3d::{View, V3};

/// How a 3-D shape is drawn (local shape coordinates, points).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Solid {
    /// Camera rotation (view space: x right, y down, z towards the viewer).
    pub view: View,
    /// Field of view of a perspective camera (degrees).
    pub fov: Option<f64>,
    /// `extrusionH` (points).
    pub depth: f64,
    /// `contourW` (points).
    pub contour: f64,
    /// `bevelT` width, height (points) and preset.
    pub bevel: Option<(f64, f64, String)>,
    /// `bevelB` width, height (points) and preset.
    pub bevel_bottom: Option<(f64, f64, String)>,
    /// `extrusionClr`.
    pub extrusion_color: Option<Color>,
    /// `contourClr`.
    pub contour_color: Option<Color>,
    /// Direction towards the light (view space, unit).
    pub light: V3,
    /// Specular strength and exponent of the material (0: matte).
    pub specular: (f64, f64),
}

impl Solid {
    /// Linear map `[a, b, c, d]` of the front face, (x, y) → (a·x + c·y, b·x + d·y) about the centre
    /// (the orthographic projection of the plane z = 0).
    pub fn face(&self) -> [f64; 4] {
        let m = &self.view.m;
        [m[0][0], m[1][0], m[0][1], m[1][1]]
    }

    /// The view for a `w` × `h` shape: a perspective camera stands where its field of view spans the
    /// larger side.
    pub fn view_for(&self, w: f64, h: f64) -> View {
        let mut v = self.view;
        if let Some(fov) = self.fov.filter(|f| *f > 1.0 && *f < 179.0) {
            v.eye = Some(
                (w.max(h) / 2.0 / (fov / 2.0).to_radians().tan()).max(1.0) + self.depth.max(0.0),
            );
        }
        v
    }
}

/// Screen direction (y down) of a §20.1.10.29 ST_LightRigDirection.
fn light_dir(dir: &str) -> (f64, f64) {
    let (x, y): (f64, f64) = match dir {
        "tl" => (-1.0, -1.0),
        "tr" => (1.0, -1.0),
        "l" => (-1.0, 0.0),
        "r" => (1.0, 0.0),
        "bl" => (-1.0, 1.0),
        "b" => (0.0, 1.0),
        "br" => (1.0, 1.0),
        _ => (0.0, -1.0),
    };
    let n = (x * x + y * y).sqrt();
    (x / n, y / n)
}

const EMU_PT: f64 = 12_700.0;

fn find<'a>(extra: &'a [Extra], name: &str) -> Option<&'a XmlElement> {
    extra.iter().map(|e| &e.xml).find(|x| x.name == name)
}

fn emu(el: &XmlElement, attr: &str, default: f64) -> f64 {
    el.attr(attr)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map_or(default, |v| v / EMU_PT)
}

fn color_in(el: Option<&XmlElement>) -> Option<Color> {
    el?.elements()
        .find(|c| bo_drawingml::color::is_color_element(&c.name))
        .and_then(bo_drawingml::color::parse_color)
}

/// Camera of a §20.1.10.47 preset: `Oblique(dx, dy)` (front face unturned, depth direction) or
/// `Turned(lat, lon)` in degrees (positive lat: seen from above; positive lon: seen from the left).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Camera {
    Front,
    Oblique(f64, f64),
    Turned(f64, f64),
}

fn preset(name: &str) -> Camera {
    let n = name.to_ascii_lowercase();
    let side = |up: &str, down: &str| -> f64 {
        if n.contains(up) {
            1.0
        } else if n.contains(down) {
            -1.0
        } else {
            0.0
        }
    };
    let vertical = side("top", "bottom") + side("above", "below");
    let horizontal = side("left", "right");
    if n.contains("oblique") {
        if vertical == 0.0 && horizontal == 0.0 {
            return Camera::Front;
        }
        // The camera sees the sides it stands at: the back shows on that side of the face.
        return Camera::Oblique(-horizontal, -vertical);
    }
    if n.contains("isometric") {
        if n.contains("offaxis") {
            return Camera::Turned(
                18.0 * if vertical == 0.0 { 1.0 } else { vertical },
                20.0 * if horizontal == 0.0 { 1.0 } else { horizontal },
            );
        }
        // The isometric angle: arctan(1/√2).
        let lat = 35.264_389_682_754_654 * if vertical == 0.0 { 1.0 } else { vertical };
        let lon = 45.0 * if horizontal == 0.0 { 1.0 } else { horizontal };
        return Camera::Turned(lat, lon);
    }
    if n.contains("perspective") && (vertical != 0.0 || horizontal != 0.0) {
        let tilt = if n.contains("relaxed") { 30.0 } else { 20.0 };
        return Camera::Turned(tilt * vertical, 20.0 * horizontal);
    }
    Camera::Front
}

/// Towards the light for a §20.1.10.29 ST_LightRigDirection: from that side, 30° off the front.
fn light_vector(dir: &str) -> V3 {
    let (x, y) = light_dir(dir);
    let (x, y, z) = (x * 0.5, y * 0.5, 0.866);
    let n = (x * x + y * y + z * z).sqrt();
    [x / n, y / n, z / n]
}

/// Specular strength and exponent of a §20.1.10.51 ST_PresetMaterialType.
fn specular(material: &str) -> (f64, f64) {
    match material {
        "matte" | "warmMatte" | "flat" | "powder" | "translucentPowder" | "legacyMatte"
        | "legacyWireframe" => (0.0, 1.0),
        "metal" | "legacyMetal" => (0.55, 12.0),
        "softmetal" => (0.35, 8.0),
        "plastic" | "legacyPlastic" => (0.4, 24.0),
        _ => (0.25, 16.0),
    }
}

/// The 3-D drawing of a shape whose `spPr` children are `extra`, or `None` when it would look flat
/// (no turned or oblique camera, no extrusion, bevel or contour).
pub(crate) fn solid_of(extra: &[Extra]) -> Option<Solid> {
    let scene = find(extra, "a:scene3d");
    let sp3d = find(extra, "a:sp3d");
    let camera = scene.and_then(|s| s.child("a:camera"));
    let prst = camera
        .and_then(|c| c.attr("prst"))
        .unwrap_or("orthographicFront");
    let mut cam = preset(prst);
    // §20.1.5.11 rot: explicit camera rotation (60000ths of a degree).
    if let Some(rot) = camera.and_then(|c| c.child("a:rot")) {
        let deg = |a: &str| {
            rot.attr(a)
                .and_then(|v| v.trim().parse::<f64>().ok())
                .map_or(0.0, |v| v / 60_000.0)
        };
        // Angles of a full turn map to ±180°.
        let wrap = |d: f64| if d > 180.0 { d - 360.0 } else { d };
        cam = Camera::Turned(-wrap(deg("lat")), -wrap(deg("lon")));
    }
    let depth = sp3d.map_or(0.0, |s| emu(s, "extrusionH", 0.0));
    let contour = sp3d.map_or(0.0, |s| emu(s, "contourW", 0.0));
    // §20.1.5.2 bevelT: default 76200 EMU (6 pt) wide and high, circle profile, when present.
    let bevel_of = |name: &str| {
        sp3d.and_then(|s| s.child(name)).map(|b| {
            (
                emu(b, "w", 6.0),
                emu(b, "h", 6.0),
                b.attr("prst").unwrap_or("circle").to_owned(),
            )
        })
    };
    let bevel = bevel_of("a:bevelT");
    // §20.1.5.1 bevelB: the same on the back.
    let bevel_bottom = bevel_of("a:bevelB");
    if cam == Camera::Front && depth == 0.0 && contour == 0.0 && bevel.is_none() {
        return None;
    }
    let view = match cam {
        Camera::Front => View::turned(0.0, 0.0, 0.0),
        Camera::Oblique(dx, dy) => {
            // Cabinet projection: depth at 45°, half length.
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            View::oblique(0.5 * dx / len, 0.5 * dy / len)
        }
        Camera::Turned(lat, lon) => View::turned(lat, lon, 0.0),
    };
    // §20.1.5.5 camera@fov (60000ths of a degree) for the perspective presets.
    let fov = prst.to_ascii_lowercase().contains("perspective").then(|| {
        camera
            .and_then(|c| c.attr("fov"))
            .and_then(|v| v.trim().parse::<f64>().ok())
            .map_or(45.0, |v| v / 60_000.0)
    });
    let rig = scene.and_then(|s| s.child("a:lightRig"));
    let material = sp3d
        .and_then(|s| s.attr("prstMaterial"))
        .unwrap_or("warmMatte");
    Some(Solid {
        view,
        fov,
        depth,
        contour,
        bevel,
        bevel_bottom,
        extrusion_color: color_in(sp3d.and_then(|s| s.child("a:extrusionClr"))),
        contour_color: color_in(sp3d.and_then(|s| s.child("a:contourClr"))),
        light: light_vector(rig.and_then(|r| r.attr("dir")).unwrap_or("t")),
        specular: specular(material),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extra(xml: &str) -> Vec<Extra> {
        let el = bo_drawingml::dom::parse(xml.as_bytes(), "/t.xml").unwrap();
        el.elements()
            .map(|e| Extra {
                key: 0,
                xml: e.clone(),
            })
            .collect()
    }

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

    #[test]
    fn a_flat_front_camera_is_not_3d() {
        let e = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="orthographicFront"/><a:lightRig rig="threePt" dir="t"/></a:scene3d></p:spPr>"#
        ));
        assert_eq!(solid_of(&e), None);
    }

    #[test]
    fn oblique_extrusion_goes_back_at_45_degrees() {
        let e = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="obliqueTopLeft"/></a:scene3d><a:sp3d extrusionH="254000"><a:extrusionClr><a:srgbClr val="203040"/></a:extrusionClr></a:sp3d></p:spPr>"#
        ));
        let s = solid_of(&e).unwrap();
        assert_eq!(s.face(), [1.0, 0.0, 0.0, 1.0]);
        assert!((s.depth - 20.0).abs() < 1e-9);
        // A point 20 pt back is drawn half length towards the upper left (the camera sees the top and
        // left sides): 20 · 0.5 · (√½, √½).
        let m = s.view.m;
        let back = (m[0][2] * -20.0, m[1][2] * -20.0);
        assert!(
            (back.0 + 7.071).abs() < 0.01 && (back.1 + 7.071).abs() < 0.01,
            "{back:?}"
        );
        assert!(s.extrusion_color.is_some());
    }

    #[test]
    fn isometric_turns_the_face_and_sends_the_back_up() {
        let e = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="isometricLeftDown"/></a:scene3d><a:sp3d extrusionH="127000"><a:bevelT w="38100" h="25400" prst="angle"/></a:sp3d></p:spPr>"#
        ));
        let s = solid_of(&e).unwrap();
        let f = s.face();
        // cos 45° wide, the vertical squeezed by cos 35.26° = √(2/3).
        assert!(
            (f[0] - 0.5f64.sqrt()).abs() < 1e-9 && (f[3] - (2.0f64 / 3.0).sqrt()).abs() < 1e-9,
            "{f:?}"
        );
        // Seen from the left and from above: a point behind the face shows to the left and higher.
        let m = s.view.m;
        assert!(-m[0][2] < 0.0 && -m[1][2] < 0.0, "{m:?}");
        assert_eq!(s.bevel, Some((3.0, 2.0, "angle".to_owned())));
        assert_eq!(s.fov, None);
    }

    #[test]
    fn light_material_and_perspective() {
        let flat = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="obliqueTop"/><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:sp3d extrusionH="12700"/></p:spPr>"#
        ));
        let s = solid_of(&flat).unwrap();
        // Light from the top and the front; the default material is matte.
        assert!(s.specular.0.abs() < f64::EPSILON);
        assert!(s.light[1] < 0.0 && s.light[2] > 0.0 && s.light[0].abs() < 1e-12);
        let p = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="perspectiveLeft" fov="3600000"/><a:lightRig rig="threePt" dir="b"/></a:scene3d><a:sp3d prstMaterial="metal"/></p:spPr>"#
        ));
        let s = solid_of(&p).unwrap();
        assert!(s.specular.0 > 0.0 && s.light[1] > 0.0, "{s:?}");
        assert_eq!(s.fov, Some(60.0));
        // Eye distance where a 60° field of view spans 100 pt: 50 / tan 30°.
        let eye = s.view_for(100.0, 40.0).eye.unwrap();
        assert!(
            (eye - 50.0 / 30f64.to_radians().tan()).abs() < 1e-9,
            "{eye}"
        );
    }
}
