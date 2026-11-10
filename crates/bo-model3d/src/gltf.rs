//! glTF 2.0 / GLB reader (Khronos glTF 2.0 specification).
//!
//! Reads the default scene (§3.5), node transforms (§3.5.2 `matrix` or `translation` /
//! `rotation` / `scale`), mesh primitives in triangle modes (§3.7.2.1: TRIANGLES,
//! TRIANGLE_STRIP, TRIANGLE_FAN) through accessors (§3.6.2, `byteStride`, normalized
//! integers), metallic-roughness materials (§3.9.2 `baseColorFactor`, `baseColorTexture`,
//! `metallicFactor`, `roughnessFactor`) and PNG / JPEG images (§3.8.2) from buffer views or
//! `data:` URIs. External files (§3.6.1.1 relative URIs) are not read: a document embeds one
//! file. The GLB container is §4.4 (12-byte header, JSON chunk, optional BIN chunk).

use crate::scene::{decode_picture, Material, Prim, Scene, Texture};
use crate::{ModelError, Result, MAX_TRIANGLES};
use serde_json::Value;

const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Split a GLB into its JSON chunk and BIN chunk (§4.4.3).
pub fn split_glb(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>)> {
    if bytes.get(0..4) != Some(b"glTF") {
        return Err(ModelError::Format("glb magic"));
    }
    if u32_at(bytes, 4) != Some(2) {
        return Err(ModelError::Unsupported("glb version"));
    }
    let total =
        (u32_at(bytes, 8).ok_or(ModelError::Format("glb header"))? as usize).min(bytes.len());
    let mut at = 12;
    let mut json = None;
    let mut bin = None;
    while at + 8 <= total {
        let len = u32_at(bytes, at).ok_or(ModelError::Format("chunk"))? as usize;
        let ty = u32_at(bytes, at + 4).ok_or(ModelError::Format("chunk"))?;
        let data = bytes
            .get(at + 8..at + 8 + len)
            .ok_or(ModelError::Format("chunk length"))?;
        match ty {
            CHUNK_JSON if json.is_none() => json = Some(data),
            CHUNK_BIN if bin.is_none() => bin = Some(data),
            _ => {}
        }
        at += 8 + len;
    }
    Ok((json.ok_or(ModelError::Format("no json chunk"))?, bin))
}

/// Parse a GLB file.
pub fn parse_glb(bytes: &[u8]) -> Result<Scene> {
    let (json, bin) = split_glb(bytes)?;
    parse_gltf(json, bin)
}

fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let rest = uri.strip_prefix("data:")?;
    let (_, data) = rest.split_once(";base64,")?;
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .ok()
}

fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn idx(v: &Value, key: &str) -> Option<usize> {
    v.get(key).and_then(Value::as_u64).map(|n| n as usize)
}

fn floats<const N: usize>(v: Option<&Value>) -> Option<[f64; N]> {
    let a = v?.as_array()?;
    if a.len() != N {
        return None;
    }
    let mut o = [0.0; N];
    for (k, x) in a.iter().enumerate() {
        o[k] = x.as_f64()?;
    }
    Some(o)
}

/// Parse glTF JSON (`bin`: the GLB BIN chunk, buffer 0 without `uri`).
pub fn parse_gltf(json: &[u8], bin: Option<&[u8]>) -> Result<Scene> {
    let doc: Value = serde_json::from_slice(json).map_err(|_| ModelError::Format("json"))?;
    if let Some(req) = doc.get("extensionsRequired").and_then(Value::as_array) {
        if !req.is_empty() {
            return Err(ModelError::Unsupported("required extension"));
        }
    }
    // §3.6.1 buffers.
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for (i, b) in doc
        .get("buffers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let data = match b.get("uri").and_then(Value::as_str) {
            Some(uri) => decode_data_uri(uri).ok_or(ModelError::Unsupported("external buffer"))?,
            None if i == 0 => bin.ok_or(ModelError::Format("no bin chunk"))?.to_vec(),
            None => Vec::new(),
        };
        buffers.push(data);
    }
    let r = Reader { doc: &doc, buffers };
    let mut scene = Scene::default();
    // Images → textures (§3.8.2, §3.8.4 textures reference images by `source`).
    let images: Vec<Option<Texture>> = doc
        .get("images")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|im| {
            let bytes = if let Some(bv) = idx(im, "bufferView") {
                r.view(bv).map(<[u8]>::to_vec)
            } else {
                im.get("uri")
                    .and_then(Value::as_str)
                    .and_then(decode_data_uri)
            };
            bytes.and_then(|b| decode_picture(&b))
        })
        .collect();
    let tex_image: Vec<Option<usize>> = doc
        .get("textures")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|t| idx(t, "source"))
        .collect();
    let mut image_slot: Vec<Option<usize>> = vec![None; images.len()];
    for m in doc
        .get("materials")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let pbr = m.get("pbrMetallicRoughness");
        let mut mat = Material::default();
        if let Some(c) = floats::<4>(pbr.and_then(|p| p.get("baseColorFactor"))) {
            mat.base = [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32];
        }
        if let Some(p) = pbr {
            if let Some(x) = num(p, "metallicFactor") {
                mat.metallic = x as f32;
            }
            if let Some(x) = num(p, "roughnessFactor") {
                mat.roughness = x as f32;
            }
            let image = p
                .get("baseColorTexture")
                .and_then(|t| idx(t, "index"))
                .and_then(|t| tex_image.get(t).copied().flatten());
            if let Some(im) = image {
                if let Some(Some(tex)) = images.get(im) {
                    let slot = if let Some(s) = image_slot[im] {
                        s
                    } else {
                        scene.textures.push(tex.clone());
                        image_slot[im] = Some(scene.textures.len() - 1);
                        scene.textures.len() - 1
                    };
                    mat.texture = Some(slot);
                }
            }
        }
        scene.materials.push(mat);
    }
    let default_material = scene.materials.len();
    scene.materials.push(Material::default());
    // §3.5 scenes: the default scene, else the first, else every root node.
    let nodes = doc
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let roots: Vec<usize> = {
        let sc = idx(&doc, "scene").unwrap_or(0);
        if let Some(list) = doc
            .get("scenes")
            .and_then(Value::as_array)
            .and_then(|s| s.get(sc))
            .and_then(|s| s.get("nodes"))
            .and_then(Value::as_array)
        {
            list.iter()
                .filter_map(Value::as_u64)
                .map(|n| n as usize)
                .collect()
        } else {
            {
                let mut child = vec![false; nodes.len()];
                for n in &nodes {
                    for c in n
                        .get("children")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(c) = c.as_u64().and_then(|c| child.get_mut(c as usize)) {
                            *c = true;
                        }
                    }
                }
                (0..nodes.len()).filter(|&i| !child[i]).collect()
            }
        }
    };
    let mut stack: Vec<(usize, [f64; 16], usize)> =
        roots.into_iter().map(|n| (n, IDENTITY, 0)).collect();
    let mut tris = 0usize;
    let mut visits = 0usize;
    while let Some((n, parent, depth)) = stack.pop() {
        visits += 1;
        if depth > 64 || visits > 100_000 {
            return Err(ModelError::Format("node graph"));
        }
        let Some(node) = nodes.get(n) else { continue };
        let m = mat_mul(&parent, &node_matrix(node));
        if let Some(mesh) = idx(node, "mesh") {
            for p in r.mesh(mesh, default_material)? {
                tris += p.indices.len() / 3;
                if tris > MAX_TRIANGLES {
                    return Err(ModelError::TooLarge);
                }
                scene.prims.push(transform_prim(p, &m));
            }
        }
        for c in node
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(c) = c.as_u64() {
                stack.push((c as usize, m, depth + 1));
            }
        }
    }
    scene.finish();
    Ok(scene)
}

const IDENTITY: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Column-major 4×4 product `a · b`.
fn mat_mul(a: &[f64; 16], b: &[f64; 16]) -> [f64; 16] {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            o[c * 4 + r] = (0..4).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    o
}

/// §3.5.2 node transform: `matrix`, else T · R · S.
fn node_matrix(node: &Value) -> [f64; 16] {
    if let Some(m) = floats::<16>(node.get("matrix")) {
        return m;
    }
    let t = floats::<3>(node.get("translation")).unwrap_or([0.0; 3]);
    let q = floats::<4>(node.get("rotation")).unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let s = floats::<3>(node.get("scale")).unwrap_or([1.0; 3]);
    let [x, y, z, w] = q;
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
    let mut o = r;
    for c in 0..3 {
        for k in 0..3 {
            o[c * 4 + k] *= s[c];
        }
    }
    o[12] = t[0];
    o[13] = t[1];
    o[14] = t[2];
    o
}

fn transform_prim(mut p: Prim, m: &[f64; 16]) -> Prim {
    let apply = |v: [f32; 3], w: f64| -> [f64; 3] {
        let (x, y, z) = (f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
        [
            m[0] * x + m[4] * y + m[8] * z + m[12] * w,
            m[1] * x + m[5] * y + m[9] * z + m[13] * w,
            m[2] * x + m[6] * y + m[10] * z + m[14] * w,
        ]
    };
    // Normals use the inverse transpose of the linear part (cofactor matrix).
    let a = [[m[0], m[4], m[8]], [m[1], m[5], m[9]], [m[2], m[6], m[10]]];
    let cof = |r: usize, c: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (c1, c2) = ((c + 1) % 3, (c + 2) % 3);
        a[r1][c1] * a[r2][c2] - a[r1][c2] * a[r2][c1]
    };
    let det = a[0][0] * cof(0, 0) + a[0][1] * cof(0, 1) + a[0][2] * cof(0, 2);
    for v in &mut p.positions {
        let o = apply(*v, 1.0);
        *v = [o[0] as f32, o[1] as f32, o[2] as f32];
    }
    for n in &mut p.normals {
        let (x, y, z) = (f64::from(n[0]), f64::from(n[1]), f64::from(n[2]));
        let o = [
            cof(0, 0) * x + cof(0, 1) * y + cof(0, 2) * z,
            cof(1, 0) * x + cof(1, 1) * y + cof(1, 2) * z,
            cof(2, 0) * x + cof(2, 1) * y + cof(2, 2) * z,
        ];
        let s = if det < 0.0 { -1.0 } else { 1.0 };
        *n = crate::scene::normalize([(o[0] * s) as f32, (o[1] * s) as f32, (o[2] * s) as f32]);
    }
    if det < 0.0 {
        // A mirroring transform reverses the winding (§3.7.2.1).
        for t in p.indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
    }
    p
}

struct Reader<'a> {
    doc: &'a Value,
    buffers: Vec<Vec<u8>>,
}

impl Reader<'_> {
    fn list(&self, key: &str) -> &[Value] {
        self.doc
            .get(key)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    /// Bytes of a buffer view (§3.6.2.1).
    fn view(&self, i: usize) -> Option<&[u8]> {
        let v = self.list("bufferViews").get(i)?;
        let buf = self.buffers.get(idx(v, "buffer")?)?;
        let off = idx(v, "byteOffset").unwrap_or(0);
        let len = idx(v, "byteLength")?;
        buf.get(off..off.checked_add(len)?)
    }

    /// Accessor `i` (§3.6.2) as numbers, `comps` per element.
    fn accessor(&self, i: usize) -> Result<(Vec<f64>, usize)> {
        let a = self
            .list("accessors")
            .get(i)
            .ok_or(ModelError::Format("accessor"))?;
        let count = idx(a, "count").ok_or(ModelError::Format("accessor count"))?;
        if count > MAX_TRIANGLES * 3 {
            return Err(ModelError::TooLarge);
        }
        let comps = match a.get("type").and_then(Value::as_str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4" | "MAT2") => 4,
            Some("MAT3") => 9,
            Some("MAT4") => 16,
            _ => return Err(ModelError::Format("accessor type")),
        };
        let ct = idx(a, "componentType").ok_or(ModelError::Format("component type"))?;
        let size = match ct {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => return Err(ModelError::Format("component type")),
        };
        let normalized = a
            .get("normalized")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut out = vec![0.0; count * comps];
        // §3.6.2.3: an accessor without a buffer view is all zeros.
        let Some(bv) = idx(a, "bufferView") else {
            return Ok((out, comps));
        };
        let data = self.view(bv).ok_or(ModelError::Format("buffer view"))?;
        let stride = self
            .list("bufferViews")
            .get(bv)
            .and_then(|v| idx(v, "byteStride"))
            .filter(|&s| s > 0)
            .unwrap_or(size * comps);
        let base = idx(a, "byteOffset").unwrap_or(0);
        for e in 0..count {
            for c in 0..comps {
                let at = base + e * stride + c * size;
                let b = data
                    .get(at..at + size)
                    .ok_or(ModelError::Format("accessor range"))?;
                let v = match ct {
                    5120 => {
                        let x = f64::from(b[0] as i8);
                        if normalized {
                            (x / 127.0).max(-1.0)
                        } else {
                            x
                        }
                    }
                    5121 => {
                        let x = f64::from(b[0]);
                        if normalized {
                            x / 255.0
                        } else {
                            x
                        }
                    }
                    5122 => {
                        let x = f64::from(i16::from_le_bytes([b[0], b[1]]));
                        if normalized {
                            (x / 32767.0).max(-1.0)
                        } else {
                            x
                        }
                    }
                    5123 => {
                        let x = f64::from(u16::from_le_bytes([b[0], b[1]]));
                        if normalized {
                            x / 65535.0
                        } else {
                            x
                        }
                    }
                    5125 => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                    _ => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                };
                out[e * comps + c] = if v.is_finite() { v } else { 0.0 };
            }
        }
        Ok((out, comps))
    }

    fn vec3(&self, i: usize) -> Result<Vec<[f32; 3]>> {
        let (v, n) = self.accessor(i)?;
        if n != 3 {
            return Err(ModelError::Format("vec3"));
        }
        Ok(v.chunks_exact(3)
            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
            .collect())
    }

    /// Mesh `i` (§3.7.2): its triangle primitives.
    fn mesh(&self, i: usize, default_material: usize) -> Result<Vec<Prim>> {
        let Some(mesh) = self.list("meshes").get(i) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for p in mesh
            .get("primitives")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mode = idx(p, "mode").unwrap_or(4);
            if !matches!(mode, 4..=6) {
                continue;
            }
            if p.get("extensions")
                .and_then(|e| e.get("KHR_draco_mesh_compression"))
                .is_some()
            {
                return Err(ModelError::Unsupported("mesh compression"));
            }
            let Some(attrs) = p.get("attributes") else {
                continue;
            };
            let Some(pos) = idx(attrs, "POSITION") else {
                continue;
            };
            let positions = self.vec3(pos)?;
            let normals = match idx(attrs, "NORMAL") {
                Some(n) => self.vec3(n)?,
                None => Vec::new(),
            };
            let uvs = match idx(attrs, "TEXCOORD_0") {
                Some(t) => {
                    let (v, n) = self.accessor(t)?;
                    if n == 2 {
                        v.chunks_exact(2)
                            .map(|c| [c[0] as f32, c[1] as f32])
                            .collect()
                    } else {
                        Vec::new()
                    }
                }
                None => Vec::new(),
            };
            let colors = match idx(attrs, "COLOR_0") {
                Some(t) => {
                    let (v, n) = self.accessor(t)?;
                    match n {
                        3 => v
                            .chunks_exact(3)
                            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, 1.0])
                            .collect(),
                        4 => v
                            .chunks_exact(4)
                            .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32])
                            .collect(),
                        _ => Vec::new(),
                    }
                }
                None => Vec::new(),
            };
            let list: Vec<u32> = match idx(p, "indices") {
                Some(ix) => self.accessor(ix)?.0.iter().map(|&x| x as u32).collect(),
                None => (0..positions.len() as u32).collect(),
            };
            let indices = match mode {
                4 => list,
                5 => {
                    let mut o = Vec::new();
                    for k in 2..list.len() {
                        if k % 2 == 0 {
                            o.extend_from_slice(&[list[k - 2], list[k - 1], list[k]]);
                        } else {
                            o.extend_from_slice(&[list[k - 1], list[k - 2], list[k]]);
                        }
                    }
                    o
                }
                _ => {
                    let mut o = Vec::new();
                    for k in 2..list.len() {
                        o.extend_from_slice(&[list[0], list[k - 1], list[k]]);
                    }
                    o
                }
            };
            out.push(Prim {
                positions,
                normals,
                uvs,
                colors,
                indices,
                material: idx(p, "material").unwrap_or(default_material),
            });
        }
        Ok(out)
    }
}
