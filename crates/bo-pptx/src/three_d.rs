//! 3-D shapes drawn in two dimensions (ECMA-376 Part 1, §20.1.5 "3-D properties").
//!
//! The renderer has no 3-D pipeline, so a shape with `a:scene3d` / `a:sp3d` is approximated:
//!
//! * the camera (§20.1.5.5 `camera`, preset names of §20.1.10.47 `ST_PresetCameraType`, explicit
//!   §20.1.5.11 `rot`) turns the front face; under an orthographic projection a turned plane is an affine
//!   map, applied around the centre of the shape;
//! * the extrusion (§20.1.5.9 `sp3d@extrusionH`) is drawn as copies of the outline stepping back along the
//!   projected depth axis, in `extrusionClr` or a darker shade of the fill; oblique presets draw the depth
//!   at 45° and half length (cabinet projection) with an unturned front face;
//! * the top bevel (§20.1.5.2 `bevelT`) becomes a light band along the upper-left and a dark band along the
//!   lower-right edge inside the outline, the contour (`contourW`, `contourClr`) an outline around the face.
//!
//! The specification only names the camera presets; the angles used for them here are chosen so that the
//! family reads right (isometric ≈ 35°/45°, perspective ≈ 20°), not measured. Lighting (`lightRig`) and
//! materials (`prstMaterial`) are not modelled. The XML stays untouched in `extra` (lossless round trip).

use bo_model::slide::{Color, Extra, XmlElement};

/// How a 3-D shape is drawn (local shape coordinates, points).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Solid {
    /// Linear map of the front face `[a, b, c, d]`: (x, y) → (a·x + c·y, b·x + d·y) about the centre.
    pub face: [f64; 4],
    /// Screen offset of the back face (points), already scaled by the extrusion depth.
    pub depth: (f64, f64),
    /// `contourW` (points).
    pub contour: f64,
    /// `bevelT@w` (points).
    pub bevel: f64,
    /// `extrusionClr`.
    pub extrusion_color: Option<Color>,
    /// `contourClr`.
    pub contour_color: Option<Color>,
    /// Lighting of the turned face relative to a face seen straight on, in −1..=1 (negative: darker;
    /// §20.1.5.10 lightRig `dir`, simple Lambert term).
    pub shade: f64,
    /// Screen direction the light comes from (unit vector, y down) — where a highlight sits.
    pub light: (f64, f64),
    /// The material shows a highlight (§20.1.10.51 ST_PresetMaterialType: not matte, flat or powder).
    pub glossy: bool,
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
        let lat = 35.26 * if vertical == 0.0 { 1.0 } else { vertical };
        let lon = 45.0 * if horizontal == 0.0 { 1.0 } else { horizontal };
        return Camera::Turned(lat, lon);
    }
    if n.contains("perspective") && (vertical != 0.0 || horizontal != 0.0) {
        let tilt = if n.contains("relaxed") { 30.0 } else { 20.0 };
        return Camera::Turned(tilt * vertical, 20.0 * horizontal);
    }
    Camera::Front
}

/// The 3-D drawing of a shape whose `spPr` children are `extra`, or `None` when it would look flat
/// (no turned or oblique camera, no extrusion, bevel or contour).
pub(crate) fn solid_of(extra: &[Extra]) -> Option<Solid> {
    let scene = find(extra, "a:scene3d");
    let sp3d = find(extra, "a:sp3d");
    let camera = scene.and_then(|s| s.child("a:camera"));
    let mut cam = camera
        .and_then(|c| c.attr("prst"))
        .map_or(Camera::Front, preset);
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
    let extrusion = sp3d.map_or(0.0, |s| emu(s, "extrusionH", 0.0));
    let contour = sp3d.map_or(0.0, |s| emu(s, "contourW", 0.0));
    // §20.1.5.2 bevelT: default 76200 EMU (6 pt) wide when present.
    let bevel = sp3d
        .and_then(|s| s.child("a:bevelT"))
        .map_or(0.0, |b| emu(b, "w", 6.0));
    if cam == Camera::Front && extrusion == 0.0 && contour == 0.0 && bevel == 0.0 {
        return None;
    }
    // Face normal on screen axes (y down, z towards the viewer) for the lighting term.
    let mut normal = (0.0, 0.0, 1.0);
    let (face, axis) = match cam {
        Camera::Front => ([1.0, 0.0, 0.0, 1.0], (0.0, 0.0)),
        Camera::Oblique(dx, dy) => {
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            ([1.0, 0.0, 0.0, 1.0], (0.5 * dx / len, 0.5 * dy / len))
        }
        Camera::Turned(lat, lon) => {
            // Rotate about y (lon) then x (lat), project orthographically (y points down):
            // X = x·cos lon + z·sin lon, Y = y·cos lat + x·sin lon·sin lat − z·cos lon·sin lat,
            // with signs chosen so that a camera above / on the left (positive) shows the back up / left.
            let (la, lo) = ((-lat).to_radians(), lon.to_radians());
            let face = [lo.cos(), lo.sin() * la.sin(), 0.0, la.cos()];
            normal = (lo.sin(), -lo.cos() * la.sin(), lo.cos() * la.cos());
            // The back face is at z = −1.
            (face, (-lo.sin(), lo.cos() * la.sin()))
        }
    };
    let rig = scene.and_then(|s| s.child("a:lightRig"));
    let light = light_dir(rig.and_then(|r| r.attr("dir")).unwrap_or("t"));
    // Light from that side and from the front (45°).
    let l = (light.0 * 0.707, light.1 * 0.707, 0.707);
    let shade = (normal.0 * l.0 + normal.1 * l.1 + normal.2 * l.2) - l.2;
    let material = sp3d
        .and_then(|s| s.attr("prstMaterial"))
        .unwrap_or("warmMatte");
    let glossy = !matches!(
        material,
        "matte"
            | "warmMatte"
            | "flat"
            | "powder"
            | "translucentPowder"
            | "legacyMatte"
            | "legacyWireframe"
    );
    Some(Solid {
        face,
        shade,
        light,
        glossy,
        depth: (axis.0 * extrusion, axis.1 * extrusion),
        contour,
        bevel,
        extrusion_color: color_in(sp3d.and_then(|s| s.child("a:extrusionClr"))),
        contour_color: color_in(sp3d.and_then(|s| s.child("a:contourClr"))),
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
        assert_eq!(s.face, [1.0, 0.0, 0.0, 1.0]);
        // 20 pt deep, half length, towards the upper left (the camera sees the top and left sides).
        assert!(
            (s.depth.0 + 7.07).abs() < 0.01 && (s.depth.1 + 7.07).abs() < 0.01,
            "{:?}",
            s.depth
        );
        assert!(s.extrusion_color.is_some());
    }

    #[test]
    fn isometric_turns_the_face_and_sends_the_back_up() {
        let e = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="isometricLeftDown"/></a:scene3d><a:sp3d extrusionH="127000"><a:bevelT w="38100" h="38100"/></a:sp3d></p:spPr>"#
        ));
        let s = solid_of(&e).unwrap();
        assert!(s.face[0] < 0.75 && s.face[3] < 0.85, "{:?}", s.face);
        // Seen from the left and from above: the back shows to the left and higher.
        assert!(s.depth.0 < 0.0 && s.depth.1 < 0.0, "{:?}", s.depth);
        assert!((s.bevel - 3.0).abs() < 1e-9);
    }

    #[test]
    fn light_and_material() {
        let flat = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="obliqueTop"/><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:sp3d extrusionH="12700"/></p:spPr>"#
        ));
        let s = solid_of(&flat).unwrap();
        // An unturned face is lit like a flat shape; the default material is matte.
        assert!(s.shade.abs() < 1e-9 && !s.glossy);
        assert_eq!(s.light, (0.0, -1.0));
        let turned = extra(&format!(
            r#"<p:spPr {NS} xmlns:p="p"><a:scene3d><a:camera prst="isometricTopUp"/><a:lightRig rig="threePt" dir="b"/></a:scene3d><a:sp3d prstMaterial="metal"/></p:spPr>"#
        ));
        let s = solid_of(&turned).unwrap();
        // Seen from above, lit from below: the face turns away from the light.
        assert!(s.shade < 0.0 && s.glossy, "{s:?}");
    }
}
