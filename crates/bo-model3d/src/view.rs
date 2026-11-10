//! The view of a 3D model as `am3d:model3d` stores it ([MS-ODRAWXML] model3d schema):
//! `am3d:trans` (CT_Model3DTransform: `meterPerModelUnit`, `preTrans`, `scale`, `rot`,
//! `postTrans`), `am3d:camera` (CT_Model3DCamera: `pos`, `up`, `lookAt`, `perspective@fov`
//! or `orthographic/sz`) and `am3d:objViewport@viewportSz` (CT_ObjectViewport) or
//! `am3d:winViewport`.
//!
//! BlueOffice's reading of these numbers (the schema gives types, not formulas):
//! * scene space is in EMU (`a:CT_Point3D` / `a:CT_Vector3D` are `ST_Coordinate`), with
//!   36 000 000 EMU per metre;
//! * a model vertex `v` (model units) goes to the scene as
//!   `R · (S ⊙ (v · meterPerModelUnit · 36e6 + preTrans)) + postTrans`, where `R` turns about
//!   z by `az`, then y by `ay`, then x by `ax` (`ST_Angle`, 60 000ths of a degree);
//! * the camera looks from `pos` at `lookAt` with `up`; its square picture (field of view
//!   `fov` across, or `sz` EMU across for an orthographic camera) is mapped onto a square
//!   `viewportSz` EMU wide centred on the frame (`winViewport`: the square covering the
//!   frame), and clipped to the frame.
//!
//! The web viewer (`packages/slide-editor/src/model3d`) mirrors these formulas.

use serde::{Deserialize, Serialize};

/// EMU per metre (914 400 EMU per inch).
pub const EMU_PER_METER: f64 = 36_000_000.0;
/// 60 000ths of a degree per degree (`ST_Angle`).
pub const ANGLE: f64 = 60_000.0;

/// View parameters of a model (units as stored: EMU, 60 000ths of a degree, ratios).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View3d {
    /// `camera/pos` (EMU).
    pub camera_pos: [f64; 3],
    /// `camera/up`.
    pub camera_up: [f64; 3],
    /// `camera/lookAt` (EMU).
    pub look_at: [f64; 3],
    /// `perspective@fov` (60 000ths of a degree); `None` = orthographic.
    pub fov: Option<f64>,
    /// `orthographic/sz` (EMU across) for an orthographic camera.
    pub ortho_size: f64,
    /// `trans/meterPerModelUnit`.
    pub meter_per_unit: f64,
    /// `trans/preTrans` (EMU).
    pub pre_trans: [f64; 3],
    /// `trans/scale` (sx, sy, sz).
    pub scale: [f64; 3],
    /// `trans/rot` (ax, ay, az; 60 000ths of a degree).
    pub rot: [f64; 3],
    /// `trans/postTrans` (EMU).
    pub post_trans: [f64; 3],
    /// `objViewport@viewportSz` (EMU); `None` = `winViewport`.
    pub viewport: Option<f64>,
}

impl Default for View3d {
    fn default() -> Self {
        View3d {
            camera_pos: [0.0, 0.0, 4.0 * EMU_PER_METER],
            camera_up: [0.0, EMU_PER_METER, 0.0],
            look_at: [0.0; 3],
            fov: Some(30.0 * ANGLE),
            ortho_size: EMU_PER_METER,
            meter_per_unit: 1.0,
            pre_trans: [0.0; 3],
            scale: [1.0; 3],
            rot: [0.0; 3],
            post_trans: [0.0; 3],
            viewport: None,
        }
    }
}

/// 3 × 3 rows.
pub type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// `m · v`.
pub fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// Dot product.
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Unit vector (`fallback` when degenerate).
pub fn unit(a: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-12 && l.is_finite() {
        scale(a, 1.0 / l)
    } else {
        fallback
    }
}

/// The camera as an orthonormal frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Eye position (EMU).
    pub pos: [f64; 3],
    /// Right.
    pub right: [f64; 3],
    /// Up.
    pub up: [f64; 3],
    /// Forward (towards `lookAt`).
    pub fwd: [f64; 3],
    /// `tan(fov / 2)` for a perspective camera.
    pub tan_half: Option<f64>,
    /// Half the orthographic square (EMU).
    pub half: f64,
    /// Distance from the eye to `lookAt`.
    pub dist: f64,
}

impl Camera {
    /// Camera coordinates (right, up, depth ahead) of a scene point.
    pub fn to_camera(&self, p: [f64; 3]) -> [f64; 3] {
        let d = sub(p, self.pos);
        [dot(d, self.right), dot(d, self.up), dot(d, self.fwd)]
    }

    /// Position in the camera square (−1..1, y up) of camera coordinates; `None` behind
    /// the eye.
    pub fn ndc(&self, c: [f64; 3]) -> Option<(f64, f64)> {
        match self.tan_half {
            Some(t) => (c[2] > self.dist * 1e-3).then(|| (c[0] / (c[2] * t), c[1] / (c[2] * t))),
            None => Some((c[0] / self.half, c[1] / self.half)),
        }
    }
}

impl View3d {
    /// The turn `R = Rx(ax) · Ry(ay) · Rz(az)`.
    pub fn rotation(&self) -> M3 {
        let [a, b, c] = self.rot.map(|r| (r / ANGLE).to_radians());
        let rx = [
            [1.0, 0.0, 0.0],
            [0.0, a.cos(), -a.sin()],
            [0.0, a.sin(), a.cos()],
        ];
        let ry = [
            [b.cos(), 0.0, b.sin()],
            [0.0, 1.0, 0.0],
            [-b.sin(), 0.0, b.cos()],
        ];
        let rz = [
            [c.cos(), -c.sin(), 0.0],
            [c.sin(), c.cos(), 0.0],
            [0.0, 0.0, 1.0],
        ];
        mul(&rx, &mul(&ry, &rz))
    }

    /// Scene position (EMU) of a model vertex.
    pub fn to_scene(&self, r: &M3, v: [f32; 3]) -> [f64; 3] {
        let k = self.meter_per_unit * EMU_PER_METER;
        let p = [
            (f64::from(v[0]) * k + self.pre_trans[0]) * self.scale[0],
            (f64::from(v[1]) * k + self.pre_trans[1]) * self.scale[1],
            (f64::from(v[2]) * k + self.pre_trans[2]) * self.scale[2],
        ];
        add(apply(r, p), self.post_trans)
    }

    /// Scene direction of a model normal (inverse transpose of `R · S`).
    pub fn normal_to_scene(&self, r: &M3, n: [f32; 3]) -> [f64; 3] {
        let s = self.scale.map(|x| if x.abs() > 1e-12 { x } else { 1e-12 });
        let m = [
            f64::from(n[0]) / s[0],
            f64::from(n[1]) / s[1],
            f64::from(n[2]) / s[2],
        ];
        unit(apply(r, m), [0.0, 0.0, 1.0])
    }

    /// The camera frame.
    pub fn camera(&self) -> Camera {
        let fwd = unit(sub(self.look_at, self.camera_pos), [0.0, 0.0, -1.0]);
        let up0 = unit(self.camera_up, [0.0, 1.0, 0.0]);
        let mut right = cross(fwd, up0);
        if dot(right, right) < 1e-12 {
            right = cross(fwd, [0.0, 0.0, 1.0]);
        }
        let right = unit(right, [1.0, 0.0, 0.0]);
        let up = cross(right, fwd);
        let dist = dot(
            sub(self.look_at, self.camera_pos),
            sub(self.look_at, self.camera_pos),
        )
        .sqrt();
        Camera {
            pos: self.camera_pos,
            right,
            up,
            fwd,
            tan_half: self
                .fov
                .map(|f| ((f / ANGLE).clamp(1.0, 179.0).to_radians() / 2.0).tan()),
            half: (self.ortho_size / 2.0).max(1.0),
            dist: dist.max(1.0),
        }
    }

    /// Side of the square the camera picture maps to (EMU) for a frame `w × h` EMU.
    pub fn square(&self, w: f64, h: f64) -> f64 {
        self.viewport.filter(|v| *v > 0.0).unwrap_or(w.max(h))
    }

    /// Frame position (EMU from its top left) of camera-square coordinates.
    pub fn to_frame(&self, ndc: (f64, f64), w: f64, h: f64) -> (f64, f64) {
        let s = self.square(w, h) / 2.0;
        (w / 2.0 + ndc.0 * s, h / 2.0 - ndc.1 * s)
    }

    /// Turn the model by `dx` degrees about the vertical axis and `dy` degrees about the
    /// horizontal one (a drag on the rotation handle).
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        let wrap = |a: f64| (a.rem_euclid(360.0 * ANGLE)).round();
        self.rot[1] = wrap(self.rot[1] + dx * ANGLE);
        self.rot[0] = wrap(self.rot[0] + dy * ANGLE);
    }

    /// Zoom by `factor` (> 1: larger): the viewport square grows (an orthographic or
    /// window viewport camera narrows instead).
    pub fn zoom(&mut self, factor: f64, frame: (f64, f64)) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let v = self.square(frame.0, frame.1);
        let max = frame.0.max(frame.1).max(1.0) * 50.0;
        self.viewport = Some((v * factor).clamp(1000.0, max).round());
    }

    /// Pan by (`dx`, `dy`) EMU on the page (y down): the model follows the pointer.
    pub fn pan(&mut self, dx: f64, dy: f64, frame: (f64, f64)) {
        let cam = self.camera();
        let square = self.square(frame.0, frame.1).max(1.0);
        // Scene units across the camera square at the look-at depth.
        let across = match cam.tan_half {
            Some(t) => 2.0 * cam.dist * t,
            None => 2.0 * cam.half,
        };
        let k = across / square;
        let shift = add(scale(cam.right, -dx * k), scale(cam.up, dy * k));
        self.camera_pos = add(self.camera_pos, shift).map(f64::round);
        self.look_at = add(self.look_at, shift).map(f64::round);
    }

    /// A view that frames a model with bounds `(lo, hi)` (model units): scaled to one
    /// metre across, centred on the origin, seen from the front by a 30° camera.
    pub fn fit(lo: [f32; 3], hi: [f32; 3]) -> View3d {
        let ext = (0..3).map(|k| f64::from(hi[k] - lo[k])).fold(0.0, f64::max);
        let mpu = if ext > 0.0 && ext.is_finite() {
            1.0 / ext
        } else {
            1.0
        };
        let k = mpu * EMU_PER_METER;
        let centre = (0..3).map(|i| -(f64::from(lo[i]) + f64::from(hi[i])) / 2.0 * k);
        let mut pre = [0.0; 3];
        for (i, c) in centre.enumerate() {
            pre[i] = c.round();
        }
        let diag = (0..3)
            .map(|i| f64::from(hi[i] - lo[i]) * k)
            .map(|d| d * d)
            .sum::<f64>()
            .sqrt();
        let fov = 30.0_f64;
        let dist = (diag / 2.0) / (fov.to_radians() / 2.0).sin();
        View3d {
            camera_pos: [0.0, 0.0, dist.round().max(1.0)],
            meter_per_unit: mpu,
            pre_trans: pre,
            fov: Some(fov * ANGLE),
            ..View3d::default()
        }
    }

    /// Rotation of a preset view (degrees ax, ay, az).
    pub fn preset(name: &str) -> Option<[f64; 3]> {
        Some(match name {
            "front" => [0.0, 0.0, 0.0],
            "back" => [0.0, 180.0, 0.0],
            "left" => [0.0, 90.0, 0.0],
            "right" => [0.0, 270.0, 0.0],
            "top" => [90.0, 0.0, 0.0],
            "bottom" => [270.0, 0.0, 0.0],
            "aboveFrontLeft" => [25.0, 35.0, 0.0],
            "aboveFrontRight" => [25.0, 325.0, 0.0],
            "belowFrontLeft" => [335.0, 35.0, 0.0],
            "belowFrontRight" => [335.0, 325.0, 0.0],
            _ => return None,
        })
    }

    /// Projected extent of the model in the camera square: `(min x, min y, max x, max y)`.
    pub fn ndc_bounds(&self, scene: &crate::Scene) -> Option<(f64, f64, f64, f64)> {
        let cam = self.camera();
        let r = self.rotation();
        let mut b = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &scene.prims {
            for &i in &p.indices {
                let Some(v) = p.positions.get(i as usize) else {
                    continue;
                };
                if let Some((x, y)) = cam.ndc(cam.to_camera(self.to_scene(&r, *v))) {
                    b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
                }
            }
        }
        (b.0 <= b.2).then_some(b)
    }

    /// Frame size (EMU) and viewport that show the whole model at most `max_w × max_h`
    /// EMU, the camera square centred on the frame.
    pub fn frame_for(&mut self, scene: &crate::Scene, max_w: f64, max_h: f64) -> (f64, f64) {
        let (x0, y0, x1, y1) = self.ndc_bounds(scene).unwrap_or((-0.5, -0.5, 0.5, 0.5));
        // Half extents about the square centre, with a small margin.
        let hx = x0.abs().max(x1.abs()).clamp(0.05, 1.0) * 1.04;
        let hy = y0.abs().max(y1.abs()).clamp(0.05, 1.0) * 1.04;
        // Frame = viewport × (hx, hy); largest viewport that fits.
        let v = (max_w / hx).min(max_h / hy);
        self.viewport = Some(v.round());
        ((v * hx).round(), (v * hy).round())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_order_and_orbit() {
        let mut v = View3d::default();
        v.orbit(90.0, 0.0);
        assert_eq!(v.rot, [0.0, 90.0 * ANGLE, 0.0]);
        let r = v.rotation();
        // +x turns to −z about y.
        let p = apply(&r, [1.0, 0.0, 0.0]);
        assert!((p[2] + 1.0).abs() < 1e-9, "{p:?}");
        v.orbit(-450.0, -30.0);
        assert_eq!(v.rot[1], 0.0);
        assert_eq!(v.rot[0], 330.0 * ANGLE);
    }

    #[test]
    fn fit_centres_the_model() {
        let v = View3d::fit([0.0, 0.0, 0.0], [2.0, 1.0, 1.0]);
        assert!((v.meter_per_unit - 0.5).abs() < 1e-12);
        let r = v.rotation();
        let c = v.to_scene(&r, [1.0, 0.5, 0.5]);
        assert!(c.iter().all(|x| x.abs() < 1.0), "{c:?}");
        let cam = v.camera();
        let (x, y) = cam.ndc(cam.to_camera(c)).unwrap();
        assert!(x.abs() < 1e-9 && y.abs() < 1e-9);
    }

    #[test]
    fn pan_follows_the_pointer() {
        let mut v = View3d::fit([-1.0; 3], [1.0; 3]);
        v.viewport = Some(1_000_000.0);
        let frame = (1_000_000.0, 1_000_000.0);
        v.pan(100_000.0, 50_000.0, frame);
        let cam = v.camera();
        let r = v.rotation();
        let o = v.to_scene(&r, [0.0, 0.0, 0.0]);
        let (fx, fy) = v.to_frame(cam.ndc(cam.to_camera(o)).unwrap(), frame.0, frame.1);
        assert!(
            (fx - 600_000.0).abs() < 50.0 && (fy - 550_000.0).abs() < 50.0,
            "{fx} {fy}"
        );
    }

    #[test]
    fn zoom_scales_the_viewport() {
        let mut v = View3d {
            viewport: Some(2_000_000.0),
            ..View3d::default()
        };
        v.zoom(1.5, (1_000_000.0, 1_000_000.0));
        assert_eq!(v.viewport, Some(3_000_000.0));
    }
}
