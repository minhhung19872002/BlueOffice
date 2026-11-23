//! Insert › 3D Models for BlueDoc (round 17): a glTF 2.0 model (`.glb`, or `.gltf` with its
//! buffers as `data:` URIs; Khronos glTF 2.0 specification) drawn by BlueOffice's own small
//! software renderer — flat-shaded triangles with a depth buffer, seen from the default
//! angle (turned 30° and tilted 20°) — into a PNG picture that is inserted like any picture.
//! Every length read from the file is checked; a model that cannot be read gives `None`.

use serde_json::Value;

/// Largest picture side and number of triangles drawn (bounds work on hostile files).
const SIDE: usize = 640;
const MAX_TRIANGLES: usize = 2_000_000;

/// A 4 × 4 matrix, column-major as glTF writes it.
type Mat = [f32; 16];

const IDENTITY: Mat = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            o[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    o
}

fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

fn floats(v: &Value, n: usize) -> Option<Vec<f32>> {
    let a = v.as_array()?;
    (a.len() == n).then(|| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect())
}

/// The local matrix of a node (`matrix`, else translation · rotation · scale).
fn node_matrix(n: &Value) -> Mat {
    if let Some(m) = floats(&n["matrix"], 16) {
        let mut o = [0.0; 16];
        o.copy_from_slice(&m);
        return o;
    }
    let t = floats(&n["translation"], 3).unwrap_or_else(|| vec![0.0; 3]);
    let q = floats(&n["rotation"], 4).unwrap_or_else(|| vec![0.0, 0.0, 0.0, 1.0]);
    let s = floats(&n["scale"], 3).unwrap_or_else(|| vec![1.0; 3]);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let r = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        0.0,
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        0.0,
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ];
    let mut m = r;
    for c in 0..3 {
        for k in 0..3 {
            m[c * 4 + k] *= s[c];
        }
    }
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m
}

/// The JSON and the binary buffers of a `.glb` or `.gltf` file.
fn parse(bytes: &[u8]) -> Option<(Value, Vec<Vec<u8>>)> {
    let u32_at = |i: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bytes.get(i..i + 4)?.try_into().ok()?) as usize)
    };
    if bytes.starts_with(b"glTF") {
        let total = u32_at(8)?.min(bytes.len());
        let mut at = 12;
        let (mut json, mut bin) = (None, None);
        while at + 8 <= total {
            let len = u32_at(at)?;
            let kind = u32_at(at + 4)?;
            let data = bytes.get(at + 8..at.checked_add(8 + len)?)?;
            match kind {
                0x4E4F_534A => json = serde_json::from_slice::<Value>(data).ok(),
                0x004E_4942 => bin = Some(data.to_vec()),
                _ => {}
            }
            at += 8 + len.div_ceil(4) * 4;
        }
        let json = json?;
        let mut buffers = Vec::new();
        for (i, b) in json["buffers"].as_array()?.iter().enumerate() {
            buffers.push(match b["uri"].as_str() {
                Some(u) => data_uri(u)?,
                None if i == 0 => bin.clone()?,
                None => return None,
            });
        }
        return Some((json, buffers));
    }
    let json: Value = serde_json::from_slice(bytes).ok()?;
    let buffers = json["buffers"]
        .as_array()?
        .iter()
        .map(|b| data_uri(b["uri"].as_str()?))
        .collect::<Option<Vec<_>>>()?;
    Some((json, buffers))
}

/// Bytes of a base64 `data:` URI (external files are not read).
fn data_uri(u: &str) -> Option<Vec<u8>> {
    let (_, b64) = u.strip_prefix("data:")?.split_once(";base64,")?;
    bo_engine::merge::base64_decode(b64)
}

/// Accessor `i` read as `n`-component values (floats, or integers for indices).
fn accessor(json: &Value, buffers: &[Vec<u8>], i: usize, n: usize) -> Option<Vec<f32>> {
    let a = &json["accessors"][i];
    let count = a["count"].as_u64()? as usize;
    let comp = a["componentType"].as_u64()?;
    let size = match comp {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return None,
    };
    let v = &json["bufferViews"][a["bufferView"].as_u64()? as usize];
    let buf = buffers.get(v["buffer"].as_u64()? as usize)?;
    let start = v["byteOffset"].as_u64().unwrap_or(0) as usize
        + a["byteOffset"].as_u64().unwrap_or(0) as usize;
    let stride = v["byteStride"].as_u64().map_or(size * n, |s| s as usize);
    if count > MAX_TRIANGLES * 3 || stride == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(count * n);
    for e in 0..count {
        for k in 0..n {
            let at = start + e * stride + k * size;
            let b = buf.get(at..at + size)?;
            out.push(match comp {
                5126 => f32::from_le_bytes(b.try_into().ok()?),
                5125 => u32::from_le_bytes(b.try_into().ok()?) as f32,
                5123 => f32::from(u16::from_le_bytes(b.try_into().ok()?)),
                5122 => f32::from(i16::from_le_bytes(b.try_into().ok()?)),
                5121 => f32::from(b[0]),
                _ => f32::from(b[0] as i8),
            });
        }
    }
    Some(out)
}

/// A triangle in model space with its colour.
struct Tri {
    p: [[f32; 3]; 3],
    rgb: [f32; 3],
}

fn collect(
    json: &Value,
    buffers: &[Vec<u8>],
    node: usize,
    parent: &Mat,
    out: &mut Vec<Tri>,
    depth: u32,
) {
    let n = &json["nodes"][node];
    if depth > 64 || n.is_null() {
        return;
    }
    let m = mul(parent, &node_matrix(n));
    if let Some(mesh) = n["mesh"].as_u64() {
        for prim in json["meshes"][mesh as usize]["primitives"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if prim["mode"].as_u64().unwrap_or(4) != 4 {
                continue;
            }
            let Some(pos) = prim["attributes"]["POSITION"]
                .as_u64()
                .and_then(|i| accessor(json, buffers, i as usize, 3))
            else {
                continue;
            };
            let verts = pos.len() / 3;
            let idx: Vec<usize> = match prim["indices"].as_u64() {
                Some(i) => accessor(json, buffers, i as usize, 1)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|f| f as usize)
                    .collect(),
                None => (0..verts).collect(),
            };
            let rgb = prim["material"]
                .as_u64()
                .and_then(|mi| {
                    floats(
                        &json["materials"][mi as usize]["pbrMetallicRoughness"]["baseColorFactor"],
                        4,
                    )
                })
                .map_or([0.62, 0.66, 0.72], |c| [c[0], c[1], c[2]]);
            for t in idx.chunks_exact(3) {
                if out.len() >= MAX_TRIANGLES || t.iter().any(|i| *i >= verts) {
                    break;
                }
                let v = |i: usize| apply(&m, [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]]);
                out.push(Tri {
                    p: [v(t[0]), v(t[1]), v(t[2])],
                    rgb,
                });
            }
        }
    }
    for c in n["children"].as_array().into_iter().flatten() {
        if let Some(c) = c.as_u64() {
            collect(json, buffers, c as usize, &m, out, depth + 1);
        }
    }
}

/// The model's picture (PNG with a transparent background), `None` when it has no
/// triangles BlueOffice can read.
pub(crate) fn render(bytes: &[u8]) -> Option<Vec<u8>> {
    let (json, buffers) = parse(bytes)?;
    let mut tris = Vec::new();
    let scene = json["scene"].as_u64().unwrap_or(0) as usize;
    let roots: Vec<usize> = match json["scenes"][scene]["nodes"].as_array() {
        Some(a) => a
            .iter()
            .filter_map(|n| n.as_u64().map(|n| n as usize))
            .collect(),
        None => (0..json["nodes"].as_array().map_or(0, Vec::len)).collect(),
    };
    for r in roots {
        collect(&json, &buffers, r, &IDENTITY, &mut tris, 0);
    }
    if tris.is_empty() {
        return None;
    }
    // Default view: turned 30° about the vertical axis, tilted 20° towards the viewer.
    let (ya, pa) = (30f32.to_radians(), 20f32.to_radians());
    let view = |p: [f32; 3]| {
        let (x, z) = (
            p[0] * ya.cos() + p[2] * ya.sin(),
            -p[0] * ya.sin() + p[2] * ya.cos(),
        );
        let (y, z) = (
            p[1] * pa.cos() - z * pa.sin(),
            p[1] * pa.sin() + z * pa.cos(),
        );
        [x, y, z]
    };
    let vt: Vec<[[f32; 3]; 3]> = tris.iter().map(|t| t.p.map(view)).collect();
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in vt.iter().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let (w, h) = ((hi[0] - lo[0]).max(1e-6), (hi[1] - lo[1]).max(1e-6));
    if !w.is_finite() || !h.is_finite() {
        return None;
    }
    let k = (SIDE as f32 - 16.0) / w.max(h);
    let (pw, ph) = (
        ((w * k) as usize + 16).max(8),
        ((h * k) as usize + 16).max(8),
    );
    let mut depth = vec![f32::MIN; pw * ph];
    let mut px = vec![0u8; pw * ph * 4];
    let light = {
        let l = [-0.35f32, 0.55, 0.75];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        [l[0] / n, l[1] / n, l[2] / n]
    };
    for (t, src) in vt.iter().zip(&tris) {
        let s = t.map(|p| [(p[0] - lo[0]) * k + 8.0, (hi[1] - p[1]) * k + 8.0, p[2]]);
        // Flat shading from the face normal (either side faces the light).
        let (a, b) = (
            [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]],
            [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]],
        );
        let nrm = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let nl = (nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2]).sqrt();
        if nl <= 0.0 || !nl.is_finite() {
            continue;
        }
        let d = ((nrm[0] * light[0] + nrm[1] * light[1] + nrm[2] * light[2]) / nl).abs();
        let shade = 0.35 + 0.65 * d;
        let rgb = src.rgb.map(|c| (c.clamp(0.0, 1.0) * shade * 255.0) as u8);
        let (x0, x1) = (
            s.iter().map(|p| p[0]).fold(f32::MAX, f32::min),
            s.iter().map(|p| p[0]).fold(f32::MIN, f32::max),
        );
        let (y0, y1) = (
            s.iter().map(|p| p[1]).fold(f32::MAX, f32::min),
            s.iter().map(|p| p[1]).fold(f32::MIN, f32::max),
        );
        let area =
            (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[2][0] - s[0][0]) * (s[1][1] - s[0][1]);
        if area.abs() < 1e-9 {
            continue;
        }
        for y in (y0.floor().max(0.0) as usize)..=(y1.ceil() as usize).min(ph - 1) {
            for x in (x0.floor().max(0.0) as usize)..=(x1.ceil() as usize).min(pw - 1) {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((s[1][0] - fx) * (s[2][1] - fy) - (s[2][0] - fx) * (s[1][1] - fy)) / area;
                let w1 = ((s[2][0] - fx) * (s[0][1] - fy) - (s[0][0] - fx) * (s[2][1] - fy)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * s[0][2] + w1 * s[1][2] + w2 * s[2][2];
                let i = y * pw + x;
                if z > depth[i] {
                    depth[i] = z;
                    px[i * 4..i * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                }
            }
        }
    }
    bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: pw as u32,
        height: ph as u32,
        pixels: px,
    })
}

#[cfg(test)]
mod tests {
    /// A one-triangle `.gltf` with its buffer as a data URI.
    fn triangle_gltf() -> Vec<u8> {
        use base64::Engine as _;
        let mut bin = Vec::new();
        for v in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bin);
        serde_json::json!({
            "asset": { "version": "2.0" },
            "scenes": [{ "nodes": [0] }],
            "nodes": [{ "mesh": 0, "rotation": [0.0, 0.0, 0.0, 1.0] }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
            "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" }],
            "bufferViews": [{ "buffer": 0, "byteLength": 36 }],
            "buffers": [{ "byteLength": 36, "uri": format!("data:application/octet-stream;base64,{b64}") }],
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn a_triangle_model_renders_and_bad_files_do_not() {
        let png = super::render(&triangle_gltf()).expect("png");
        assert!(png.starts_with(b"\x89PNG"));
        // The same JSON in a .glb container.
        let json = triangle_gltf();
        let pad = json.len().div_ceil(4) * 4;
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&((12 + 8 + pad) as u32).to_le_bytes());
        glb.extend_from_slice(&(pad as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.resize(12 + 8 + pad, b' ');
        assert!(super::render(&glb).is_some());
        assert!(super::render(b"glTF\x02\0\0\0\xff\xff\xff\xff").is_none());
        assert!(super::render(b"not a model").is_none());
    }
}
