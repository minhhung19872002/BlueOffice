//! STL reader, binary (80-byte header, triangle count, 50 bytes per facet) and ASCII
//! (`solid` / `facet normal` / `vertex` / `endfacet`). STL files are usually Z-up (CAD);
//! they are turned to the Y-up convention of glTF (x, y, z) → (x, z, −y). Facets are
//! flat: each triangle has its own three vertices and the facet normal.

use crate::scene::{cross, normalize, sub, Material, Prim, Scene};
use crate::{ModelError, Result, MAX_TRIANGLES};

fn f32_at(b: &[u8], at: usize) -> f32 {
    b.get(at..at + 4)
        .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .filter(|x| x.is_finite())
        .unwrap_or(0.0)
}

fn up(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], -v[1]]
}

fn push(p: &mut Prim, tri: [[f32; 3]; 3]) {
    let [a, b, c] = tri.map(up);
    let n = normalize(cross(sub(b, a), sub(c, a)));
    let base = p.positions.len() as u32;
    p.positions.extend_from_slice(&[a, b, c]);
    p.normals.extend_from_slice(&[n, n, n]);
    p.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

/// Is `bytes` a binary STL (its size matches the facet count)?
pub fn is_binary(bytes: &[u8]) -> bool {
    if bytes.len() < 84 {
        return false;
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    n.checked_mul(50).and_then(|x| x.checked_add(84)) == Some(bytes.len())
}

/// Parse an STL file.
pub fn parse(bytes: &[u8]) -> Result<Scene> {
    let mut p = Prim::default();
    if is_binary(bytes) {
        let n = (bytes.len() - 84) / 50;
        if n > MAX_TRIANGLES {
            return Err(ModelError::TooLarge);
        }
        for t in 0..n {
            let at = 84 + t * 50 + 12;
            let v = |k: usize| {
                [
                    f32_at(bytes, at + k * 12),
                    f32_at(bytes, at + k * 12 + 4),
                    f32_at(bytes, at + k * 12 + 8),
                ]
            };
            push(&mut p, [v(0), v(1), v(2)]);
        }
    } else {
        let text = String::from_utf8_lossy(bytes);
        if !text.trim_start().starts_with("solid") {
            return Err(ModelError::Format("stl"));
        }
        let mut cur: Vec<[f32; 3]> = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("vertex") => {
                    let n: Vec<f32> = it
                        .filter_map(|x| x.parse().ok())
                        .filter(|x: &f32| x.is_finite())
                        .collect();
                    if n.len() < 3 {
                        return Err(ModelError::Format("stl vertex"));
                    }
                    cur.push([n[0], n[1], n[2]]);
                }
                Some("endloop") => {
                    // Polygons with more than three vertices are fanned.
                    for k in 1..cur.len().saturating_sub(1) {
                        push(&mut p, [cur[0], cur[k], cur[k + 1]]);
                    }
                    cur.clear();
                    if p.indices.len() / 3 > MAX_TRIANGLES {
                        return Err(ModelError::TooLarge);
                    }
                }
                _ => {}
            }
        }
    }
    if p.indices.is_empty() {
        return Err(ModelError::Format("stl without facets"));
    }
    let mut scene = Scene {
        prims: vec![p],
        materials: vec![Material {
            base: [0.72, 0.74, 0.78, 1.0],
            texture: None,
            metallic: 0.0,
            roughness: 0.5,
        }],
        textures: Vec::new(),
    };
    scene.finish();
    Ok(scene)
}
