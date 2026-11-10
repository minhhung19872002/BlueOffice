//! GLB writer (glTF 2.0 §4.4): a flattened [`Scene`] as one node holding one mesh, a
//! primitive per [`Prim`] (POSITION with `min` / `max` as §3.7.2.1 requires, NORMAL,
//! TEXCOORD_0, COLOR_0, UNSIGNED_INT indices), metallic-roughness materials and the
//! textures' encoded pictures in buffer views.

use crate::scene::{encode_png, Scene};
use serde_json::{json, Value};

fn pad4(b: &mut Vec<u8>, with: u8) {
    while !b.len().is_multiple_of(4) {
        b.push(with);
    }
}

struct Out {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Out {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        pad4(&mut self.bin, 0);
        let off = self.bin.len();
        self.bin.extend_from_slice(bytes);
        let mut v = json!({ "buffer": 0, "byteOffset": off, "byteLength": bytes.len() });
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.views.push(v);
        self.views.len() - 1
    }

    fn floats(&mut self, data: &[f32], comps: usize, ty: &str, minmax: bool) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        // §3.6.2.4 target ARRAY_BUFFER.
        let view = self.view(&bytes, Some(34962));
        let mut a = json!({
            "bufferView": view,
            "componentType": 5126,
            "count": data.len() / comps,
            "type": ty,
        });
        if minmax {
            let mut lo = vec![f32::INFINITY; comps];
            let mut hi = vec![f32::NEG_INFINITY; comps];
            for c in data.chunks_exact(comps) {
                for k in 0..comps {
                    lo[k] = lo[k].min(c[k]);
                    hi[k] = hi[k].max(c[k]);
                }
            }
            a["min"] = json!(lo);
            a["max"] = json!(hi);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }
}

/// Serialize a scene as GLB.
pub fn write(scene: &Scene) -> Vec<u8> {
    let mut o = Out {
        bin: Vec::new(),
        views: Vec::new(),
        accessors: Vec::new(),
    };
    let mut images = Vec::new();
    for t in &scene.textures {
        let (bytes, mime) = match &t.source {
            Some((b, m)) => (b.clone(), m.clone()),
            None => (
                encode_png(t.width, t.height, &t.rgba),
                "image/png".to_owned(),
            ),
        };
        let view = o.view(&bytes, None);
        images.push(json!({ "bufferView": view, "mimeType": mime }));
    }
    let textures: Vec<Value> = (0..images.len()).map(|i| json!({ "source": i })).collect();
    let materials: Vec<Value> = scene
        .materials
        .iter()
        .map(|m| {
            let mut pbr = json!({
                "baseColorFactor": m.base,
                "metallicFactor": m.metallic,
                "roughnessFactor": m.roughness,
            });
            if let Some(t) = m.texture.filter(|&t| t < textures.len()) {
                pbr["baseColorTexture"] = json!({ "index": t });
            }
            let mut v = json!({ "pbrMetallicRoughness": pbr });
            if m.base[3] < 1.0 {
                v["alphaMode"] = json!("BLEND");
            }
            v
        })
        .collect();
    let mut prims = Vec::new();
    for p in &scene.prims {
        let pos: Vec<f32> = p.positions.iter().flatten().copied().collect();
        let mut attrs = json!({ "POSITION": o.floats(&pos, 3, "VEC3", true) });
        if p.normals.len() == p.positions.len() {
            let n: Vec<f32> = p.normals.iter().flatten().copied().collect();
            attrs["NORMAL"] = json!(o.floats(&n, 3, "VEC3", false));
        }
        if !p.uvs.is_empty() && p.uvs.len() == p.positions.len() {
            let n: Vec<f32> = p.uvs.iter().flatten().copied().collect();
            attrs["TEXCOORD_0"] = json!(o.floats(&n, 2, "VEC2", false));
        }
        if !p.colors.is_empty() && p.colors.len() == p.positions.len() {
            let n: Vec<f32> = p.colors.iter().flatten().copied().collect();
            attrs["COLOR_0"] = json!(o.floats(&n, 4, "VEC4", false));
        }
        let ib: Vec<u8> = p.indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        // §3.6.2.4 target ELEMENT_ARRAY_BUFFER.
        let view = o.view(&ib, Some(34963));
        o.accessors.push(json!({
            "bufferView": view,
            "componentType": 5125,
            "count": p.indices.len(),
            "type": "SCALAR",
        }));
        let indices = o.accessors.len() - 1;
        prims.push(json!({
            "attributes": attrs,
            "indices": indices,
            "material": p.material.min(scene.materials.len().saturating_sub(1)),
            "mode": 4,
        }));
    }
    pad4(&mut o.bin, 0);
    let mut doc = json!({
        "asset": { "version": "2.0", "generator": "BlueOffice" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": prims }],
        "materials": materials,
        "accessors": o.accessors,
        "bufferViews": o.views,
        "buffers": [{ "byteLength": o.bin.len() }],
    });
    if !images.is_empty() {
        doc["images"] = json!(images);
        doc["textures"] = json!(textures);
    }
    let mut js = serde_json::to_vec(&doc).unwrap_or_default();
    pad4(&mut js, b' ');
    let total = 12 + 8 + js.len() + 8 + o.bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    out.extend_from_slice(&js);
    out.extend_from_slice(&(o.bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    out.extend_from_slice(&o.bin);
    out
}
