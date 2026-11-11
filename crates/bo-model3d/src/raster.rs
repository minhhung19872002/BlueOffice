//! Software z-buffer renderer: a scene seen through a [`View3d`] into a frame of
//! `w × h` EMU, as straight RGBA pixels with a transparent background. Deterministic
//! (no threads, fixed lighting), so its PNG output can be snapshot-tested.
//!
//! Per pixel: perspective-correct interpolation of normal, texture coordinates and vertex
//! colour; base colour = `baseColorFactor` × vertex colour × texture (glTF 2.0 §3.9.2,
//! textures decoded from sRGB); Lambert lighting from a key and a fill light fixed to the
//! camera plus ambient, a Blinn specular term for smooth materials, then linear → sRGB.
//! 2 × 2 supersampling smooths the edges.

use crate::scene::Scene;
use crate::view::{dot, unit, View3d};

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::powf((c + 0.055) / 1.055, 2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    }
}

struct Vert {
    /// Pixel position.
    x: f64,
    y: f64,
    /// Depth ahead of the camera (perspective) or distance (orthographic).
    z: f64,
    /// 1 / z for perspective-correct interpolation (1 for orthographic).
    w: f64,
    n: [f64; 3],
    uv: [f32; 2],
    c: [f32; 4],
}

/// Render `scene` into `px_w × px_h` pixels for a frame of `frame_w × frame_h` EMU.
pub fn render(
    scene: &Scene,
    view: &View3d,
    frame_w: f64,
    frame_h: f64,
    px_w: u32,
    px_h: u32,
) -> Vec<u8> {
    const SS: u32 = 2;
    let (w, h) = ((px_w.max(1) * SS) as usize, (px_h.max(1) * SS) as usize);
    let kx = w as f64 / frame_w.max(1.0);
    let ky = h as f64 / frame_h.max(1.0);
    let mut color = vec![[0.0f32; 4]; w * h];
    let mut depth = vec![f64::INFINITY; w * h];
    let cam = view.camera();
    let r = view.rotation();
    let perspective = cam.tan_half.is_some();
    // Lights fixed to the camera (directions towards the light).
    let key = unit(
        [
            -0.45 * cam.right[0] + 0.55 * cam.up[0] - 0.70 * cam.fwd[0],
            -0.45 * cam.right[1] + 0.55 * cam.up[1] - 0.70 * cam.fwd[1],
            -0.45 * cam.right[2] + 0.55 * cam.up[2] - 0.70 * cam.fwd[2],
        ],
        [0.0, 0.0, 1.0],
    );
    let fill = unit(
        [
            0.60 * cam.right[0] - 0.25 * cam.up[0] - 0.75 * cam.fwd[0],
            0.60 * cam.right[1] - 0.25 * cam.up[1] - 0.75 * cam.fwd[1],
            0.60 * cam.right[2] - 0.25 * cam.up[2] - 0.75 * cam.fwd[2],
        ],
        [0.0, 0.0, 1.0],
    );
    let eye = [-cam.fwd[0], -cam.fwd[1], -cam.fwd[2]];
    let half = unit([key[0] + eye[0], key[1] + eye[1], key[2] + eye[2]], eye);
    for prim in &scene.prims {
        let mat = scene.material(prim);
        let tex = mat.texture.and_then(|t| scene.textures.get(t));
        let verts: Vec<Option<Vert>> = (0..prim.positions.len())
            .map(|i| {
                let c = cam.to_camera(view.to_scene(&r, prim.positions[i]));
                let (nx, ny) = cam.ndc(c)?;
                let (fx, fy) = view.to_frame((nx, ny), frame_w, frame_h);
                Some(Vert {
                    x: fx * kx,
                    y: fy * ky,
                    z: c[2],
                    w: if perspective { 1.0 / c[2] } else { 1.0 },
                    n: prim
                        .normals
                        .get(i)
                        .map_or([0.0, 0.0, 1.0], |n| view.normal_to_scene(&r, *n)),
                    uv: prim.uvs.get(i).copied().unwrap_or([0.0, 0.0]),
                    c: prim.colors.get(i).copied().unwrap_or([1.0; 4]),
                })
            })
            .collect();
        let shade = |n: [f64; 3], uv: [f32; 2], vc: [f32; 4]| -> Option<[f32; 4]> {
            let mut base = [
                mat.base[0] * vc[0],
                mat.base[1] * vc[1],
                mat.base[2] * vc[2],
                mat.base[3] * vc[3],
            ];
            if let Some(t) = tex {
                let s = t.sample(uv[0], uv[1]);
                for k in 0..3 {
                    base[k] *= srgb_to_linear(s[k]);
                }
                base[3] *= s[3];
            }
            if base[3] < 0.5 {
                return None;
            }
            // Two-sided: the side facing the eye.
            let n = if dot(n, eye) < 0.0 {
                [-n[0], -n[1], -n[2]]
            } else {
                n
            };
            let lambert = 0.30 + 0.62 * dot(n, key).max(0.0) + 0.22 * dot(n, fill).max(0.0);
            let smooth = (1.0 - mat.roughness).clamp(0.0, 1.0);
            let spec = if smooth > 0.0 {
                f64::from(smooth)
                    * 0.35
                    * libm::pow(dot(n, half).max(0.0), 8.0 + 56.0 * f64::from(smooth))
            } else {
                0.0
            };
            let mut o = [0.0; 4];
            for k in 0..3 {
                o[k] = linear_to_srgb((f64::from(base[k]) * lambert + spec) as f32);
            }
            o[3] = 1.0;
            Some(o)
        };
        for t in prim.indices.chunks_exact(3) {
            let (Some(Some(a)), Some(Some(b)), Some(Some(c))) = (
                verts.get(t[0] as usize),
                verts.get(t[1] as usize),
                verts.get(t[2] as usize),
            ) else {
                continue;
            };
            let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            if area.abs() < 1e-12 || !area.is_finite() {
                continue;
            }
            let minx = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
            let maxx = (a.x.max(b.x).max(c.x).ceil().min(w as f64)) as usize;
            let miny = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
            let maxy = (a.y.max(b.y).max(c.y).ceil().min(h as f64)) as usize;
            for py in miny..maxy {
                let sy = py as f64 + 0.5;
                for px in minx..maxx {
                    let sx = px as f64 + 0.5;
                    let w0 = ((b.x - sx) * (c.y - sy) - (b.y - sy) * (c.x - sx)) / area;
                    let w1 = ((c.x - sx) * (a.y - sy) - (c.y - sy) * (a.x - sx)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    // Perspective-correct weights.
                    let q0 = w0 * a.w;
                    let q1 = w1 * b.w;
                    let q2 = w2 * c.w;
                    let qs = q0 + q1 + q2;
                    if qs <= 0.0 {
                        continue;
                    }
                    let (q0, q1, q2) = (q0 / qs, q1 / qs, q2 / qs);
                    let z = q0 * a.z + q1 * b.z + q2 * c.z;
                    let i = py * w + px;
                    if z >= depth[i] {
                        continue;
                    }
                    let n = unit(
                        [
                            q0 * a.n[0] + q1 * b.n[0] + q2 * c.n[0],
                            q0 * a.n[1] + q1 * b.n[1] + q2 * c.n[1],
                            q0 * a.n[2] + q1 * b.n[2] + q2 * c.n[2],
                        ],
                        a.n,
                    );
                    let (f0, f1, f2) = (q0 as f32, q1 as f32, q2 as f32);
                    let uv = [
                        f0 * a.uv[0] + f1 * b.uv[0] + f2 * c.uv[0],
                        f0 * a.uv[1] + f1 * b.uv[1] + f2 * c.uv[1],
                    ];
                    let vc: [f32; 4] =
                        std::array::from_fn(|k| f0 * a.c[k] + f1 * b.c[k] + f2 * c.c[k]);
                    if let Some(px_color) = shade(n, uv, vc) {
                        depth[i] = z;
                        color[i] = px_color;
                    }
                }
            }
        }
    }
    // Box filter down to the output size (premultiplied average).
    let (ow, oh) = (px_w.max(1) as usize, px_h.max(1) as usize);
    let mut out = vec![0u8; ow * oh * 4];
    let ss = SS as usize;
    for y in 0..oh {
        for x in 0..ow {
            let mut acc = [0.0f32; 4];
            for dy in 0..ss {
                for dx in 0..ss {
                    let c = color[(y * ss + dy) * w + x * ss + dx];
                    for k in 0..3 {
                        acc[k] += c[k] * c[3];
                    }
                    acc[3] += c[3];
                }
            }
            let n = (ss * ss) as f32;
            let o = (y * ow + x) * 4;
            if acc[3] > 0.0 {
                for k in 0..3 {
                    out[o + k] = ((acc[k] / acc[3]) * 255.0).round().clamp(0.0, 255.0) as u8;
                }
                out[o + 3] = ((acc[3] / n) * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Pixel size of a picture for a frame `w × h` EMU at `px_per_inch`, the longer side at
/// most `max_side`.
pub fn picture_size(w: f64, h: f64, px_per_inch: f64, max_side: u32) -> (u32, u32) {
    let k = px_per_inch / 914_400.0;
    let (mut pw, mut ph) = ((w * k).max(1.0), (h * k).max(1.0));
    let longest = pw.max(ph);
    if longest > f64::from(max_side) {
        let s = f64::from(max_side) / longest;
        pw *= s;
        ph *= s;
    }
    (pw.round().max(1.0) as u32, ph.round().max(1.0) as u32)
}

/// [`render`] as a PNG.
pub fn render_png(
    scene: &Scene,
    view: &View3d,
    frame_w: f64,
    frame_h: f64,
    px_w: u32,
    px_h: u32,
) -> Vec<u8> {
    let rgba = render(scene, view, frame_w, frame_h, px_w, px_h);
    crate::scene::encode_png(px_w.max(1), px_h.max(1), &rgba)
}
