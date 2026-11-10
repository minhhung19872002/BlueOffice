//! Wavefront OBJ reader: `v` (with optional vertex colour), `vt`, `vn`, `f` (polygons
//! fan-triangulated, `v`, `v/vt`, `v//vn`, `v/vt/vn`, negative = relative indices).
//! Materials (`mtllib` / `usemtl`) live in separate files and are not read: faces get a
//! neutral grey.

use crate::scene::{Material, Prim, Scene};
use crate::{ModelError, Result, MAX_TRIANGLES};
use std::collections::HashMap;

fn resolve(s: &str, len: usize) -> Option<usize> {
    let i: i64 = s.parse().ok()?;
    let len = len as i64;
    let r = if i < 0 { len + i } else { i - 1 };
    (0..len).contains(&r).then_some(r as usize)
}

/// Parse an OBJ file.
pub fn parse(bytes: &[u8]) -> Result<Scene> {
    let text = String::from_utf8_lossy(bytes);
    let mut v: Vec<[f32; 3]> = Vec::new();
    let mut vc: Vec<Option<[f32; 4]>> = Vec::new();
    let mut vt: Vec<[f32; 2]> = Vec::new();
    let mut vn: Vec<[f32; 3]> = Vec::new();
    let mut p = Prim::default();
    let mut map: HashMap<(usize, Option<usize>, Option<usize>), u32> = HashMap::new();
    let mut any_uv = false;
    let mut any_n = true;
    let mut any_color = false;
    let mut corners: Vec<(usize, Option<usize>, Option<usize>)> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let mut it = line.split_whitespace();
        let Some(tag) = it.next() else { continue };
        let nums = |it: std::str::SplitWhitespace<'_>| -> Vec<f32> {
            it.filter_map(|x| x.parse::<f32>().ok())
                .filter(|x| x.is_finite())
                .collect()
        };
        match tag {
            "v" => {
                let n = nums(it);
                if n.len() < 3 {
                    return Err(ModelError::Format("obj vertex"));
                }
                v.push([n[0], n[1], n[2]]);
                vc.push((n.len() >= 6).then(|| [n[3], n[4], n[5], 1.0]));
                any_color |= n.len() >= 6;
            }
            "vt" => {
                let n = nums(it);
                // OBJ v grows upwards, glTF v downwards.
                vt.push([
                    n.first().copied().unwrap_or(0.0),
                    1.0 - n.get(1).copied().unwrap_or(0.0),
                ]);
            }
            "vn" => {
                let n = nums(it);
                if n.len() >= 3 {
                    vn.push([n[0], n[1], n[2]]);
                }
            }
            "f" => {
                corners.clear();
                for c in it {
                    let mut parts = c.split('/');
                    let vi = parts.next().and_then(|s| resolve(s, v.len()));
                    let ti = parts
                        .next()
                        .filter(|s| !s.is_empty())
                        .and_then(|s| resolve(s, vt.len()));
                    let ni = parts
                        .next()
                        .filter(|s| !s.is_empty())
                        .and_then(|s| resolve(s, vn.len()));
                    let Some(vi) = vi else {
                        return Err(ModelError::Format("obj face index"));
                    };
                    corners.push((vi, ti, ni));
                }
                if corners.len() < 3 {
                    continue;
                }
                let mut ids = Vec::with_capacity(corners.len());
                for &key in &corners {
                    let id = *map.entry(key).or_insert_with(|| {
                        p.positions.push(v[key.0]);
                        p.uvs.push(key.1.map_or([0.0, 0.0], |t| vt[t]));
                        p.normals.push(key.2.map_or([0.0, 0.0, 0.0], |n| vn[n]));
                        p.colors.push(vc[key.0].unwrap_or([0.8, 0.8, 0.8, 1.0]));
                        (p.positions.len() - 1) as u32
                    });
                    any_uv |= key.1.is_some();
                    any_n &= key.2.is_some();
                    ids.push(id);
                }
                for k in 1..ids.len() - 1 {
                    p.indices.extend_from_slice(&[ids[0], ids[k], ids[k + 1]]);
                }
                if p.indices.len() / 3 > MAX_TRIANGLES {
                    return Err(ModelError::TooLarge);
                }
            }
            _ => {}
        }
    }
    if p.indices.is_empty() {
        return Err(ModelError::Format("obj without faces"));
    }
    if !any_uv {
        p.uvs.clear();
    }
    if !any_n {
        p.normals.clear();
    }
    if !any_color {
        p.colors.clear();
    }
    let mut scene = Scene {
        prims: vec![p],
        materials: vec![Material {
            base: if any_color {
                [1.0; 4]
            } else {
                [0.8, 0.8, 0.8, 1.0]
            },
            texture: None,
            metallic: 0.0,
            roughness: 0.6,
        }],
        textures: Vec::new(),
    };
    scene.finish();
    Ok(scene)
}
