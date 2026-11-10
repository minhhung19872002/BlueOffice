//! Self-made sample models (corpus files and tests): a cube with coloured faces, a
//! square pyramid, a ring (torus) and a cube with a generated checker texture.

use crate::scene::{encode_png, Material, Prim, Scene, Texture};

fn mat(rgb: [f32; 3], roughness: f32) -> Material {
    Material {
        base: [rgb[0], rgb[1], rgb[2], 1.0],
        texture: None,
        metallic: 0.0,
        roughness,
    }
}

/// Faces of the unit cube centred on the origin: (normal, four corners counter-clockwise).
fn cube_faces() -> [([f32; 3], [[f32; 3]; 4]); 6] {
    let h = 0.5;
    [
        (
            [0.0, 0.0, 1.0],
            [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]],
        ),
        (
            [1.0, 0.0, 0.0],
            [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]],
        ),
    ]
}

/// A unit cube, each face its own colour (one primitive per face).
pub fn cube() -> Scene {
    let colors = [
        [0.10, 0.35, 0.80],
        [0.85, 0.45, 0.10],
        [0.15, 0.65, 0.30],
        [0.80, 0.15, 0.20],
        [0.95, 0.80, 0.20],
        [0.45, 0.25, 0.70],
    ];
    let mut s = Scene::default();
    for (i, (n, q)) in cube_faces().iter().enumerate() {
        s.materials.push(mat(colors[i], 0.5));
        s.prims.push(Prim {
            positions: q.to_vec(),
            normals: vec![*n; 4],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            colors: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3],
            material: i,
        });
    }
    s
}

/// A cube whose faces show a generated 8 × 8 checker texture.
pub fn textured_cube() -> Scene {
    let mut s = cube();
    let size = 64u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            rgba.extend_from_slice(if on {
                &[240, 240, 235, 255]
            } else {
                &[30, 90, 160, 255]
            });
        }
    }
    let png = encode_png(size, size, &rgba);
    s.textures.push(Texture {
        width: size,
        height: size,
        rgba,
        source: Some((png, "image/png".into())),
    });
    s.materials = vec![Material {
        base: [1.0; 4],
        texture: Some(0),
        metallic: 0.0,
        roughness: 0.8,
    }];
    for p in &mut s.prims {
        p.material = 0;
    }
    s
}

/// A square pyramid (base 1 × 1, height 1), flat-shaded.
pub fn pyramid() -> Scene {
    let apex = [0.0, 0.5, 0.0];
    let b = [
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, -0.5],
    ];
    let mut p = Prim::default();
    let tri = |a: [f32; 3], b: [f32; 3], c: [f32; 3], p: &mut Prim| {
        let base = p.positions.len() as u32;
        p.positions.extend_from_slice(&[a, b, c]);
        p.indices.extend_from_slice(&[base, base + 1, base + 2]);
    };
    for i in 0..4 {
        tri(b[i], b[(i + 1) % 4], apex, &mut p);
    }
    tri(b[0], b[3], b[2], &mut p);
    tri(b[0], b[2], b[1], &mut p);
    let mut s = Scene {
        prims: vec![p],
        materials: vec![mat([0.90, 0.62, 0.18], 0.35)],
        textures: Vec::new(),
    };
    s.finish();
    s
}

/// A ring (torus): major radius 0.5, minor 0.18, smooth normals.
pub fn ring() -> Scene {
    let (n_major, n_minor) = (48usize, 20usize);
    let (big, small) = (0.5f32, 0.18f32);
    let mut p = Prim::default();
    for i in 0..n_major {
        let u = i as f32 / n_major as f32 * std::f32::consts::TAU;
        for j in 0..n_minor {
            let v = j as f32 / n_minor as f32 * std::f32::consts::TAU;
            // Portable libm: the GLB (and so the corpus) is the same on every platform.
            let (cu, su, cv, sv) = (libm::cosf(u), libm::sinf(u), libm::cosf(v), libm::sinf(v));
            p.positions
                .push([(big + small * cv) * cu, small * sv, (big + small * cv) * su]);
            p.normals.push([cv * cu, sv, cv * su]);
        }
    }
    for i in 0..n_major {
        for j in 0..n_minor {
            let a = (i * n_minor + j) as u32;
            let b = (((i + 1) % n_major) * n_minor + j) as u32;
            let c = (((i + 1) % n_major) * n_minor + (j + 1) % n_minor) as u32;
            let d = (i * n_minor + (j + 1) % n_minor) as u32;
            p.indices.extend_from_slice(&[a, d, c, a, c, b]);
        }
    }
    Scene {
        prims: vec![p],
        materials: vec![mat([0.20, 0.55, 0.75], 0.25)],
        textures: Vec::new(),
    }
}

/// A sample by name (`cube`, `textured-cube`, `pyramid`, `ring`).
pub fn by_name(name: &str) -> Option<Scene> {
    Some(match name {
        "cube" => cube(),
        "textured-cube" => textured_cube(),
        "pyramid" => pyramid(),
        "ring" => ring(),
        _ => return None,
    })
}

/// The pyramid as OBJ text.
pub fn pyramid_obj() -> String {
    "# BlueOffice sample\nv -0.5 -0.5 0.5\nv 0.5 -0.5 0.5\nv 0.5 -0.5 -0.5\nv -0.5 -0.5 -0.5\nv 0 0.5 0\n\
     f 1 2 5\nf 2 3 5\nf 3 4 5\nf 4 1 5\nf 1 4 3 2\n"
        .to_owned()
}

/// A tetrahedron as binary STL (Z-up).
pub fn tetra_stl_binary() -> Vec<u8> {
    let v = [
        [0.0f32, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let faces = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let mut out = vec![0u8; 80];
    out.extend_from_slice(&(faces.len() as u32).to_le_bytes());
    for f in faces {
        out.extend_from_slice(&[0u8; 12]);
        for i in f {
            for c in v[i] {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0u8; 2]);
    }
    out
}

/// A tetrahedron as ASCII STL.
pub fn tetra_stl_ascii() -> String {
    "solid t\nfacet normal 0 0 -1\nouter loop\nvertex 0 0 0\nvertex 0 1 0\nvertex 1 0 0\nendloop\nendfacet\n\
     facet normal 0 -1 0\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 0 1\nendloop\nendfacet\n\
     facet normal -1 0 0\nouter loop\nvertex 0 0 0\nvertex 0 0 1\nvertex 0 1 0\nendloop\nendfacet\n\
     facet normal 1 1 1\nouter loop\nvertex 1 0 0\nvertex 0 1 0\nvertex 0 0 1\nendloop\nendfacet\nendsolid t\n"
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::View3d;

    #[test]
    fn glb_round_trip() {
        for name in ["cube", "textured-cube", "pyramid", "ring"] {
            let s = by_name(name).unwrap();
            let glb = crate::glb::write(&s);
            assert_eq!(&glb[0..4], b"glTF");
            assert_eq!(glb.len() % 4, 0);
            let back = crate::gltf::parse_glb(&glb).unwrap();
            assert_eq!(back.triangles(), s.triangles(), "{name}");
            assert_eq!(back.bounds(), s.bounds());
            assert_eq!(back.textures.len(), s.textures.len());
            if let Some(t) = back.textures.first() {
                assert_eq!((t.width, t.height), (64, 64));
            }
        }
    }

    #[test]
    fn gltf_json_with_data_uri() {
        use base64::Engine;
        let glb = crate::glb::write(&pyramid());
        let (json, bin) = crate::gltf::split_glb(&glb).unwrap();
        let mut doc: serde_json::Value = serde_json::from_slice(json).unwrap();
        let uri = format!(
            "data:application/octet-stream;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bin.unwrap())
        );
        doc["buffers"][0]["uri"] = serde_json::json!(uri);
        let text = serde_json::to_vec(&doc).unwrap();
        let (scene, stored) = crate::load(&text, "p.gltf").unwrap();
        assert_eq!(scene.triangles(), 6);
        assert_eq!(&stored[0..4], b"glTF");
        // External buffers are refused.
        doc["buffers"][0]["uri"] = serde_json::json!("p.bin");
        assert!(crate::load(&serde_json::to_vec(&doc).unwrap(), "p.gltf").is_err());
    }

    #[test]
    fn obj_and_stl() {
        let (s, glb) = crate::load(pyramid_obj().as_bytes(), "p.OBJ").unwrap();
        // Four sides + the quad base fanned into two triangles.
        assert_eq!(s.triangles(), 6);
        assert_eq!(crate::gltf::parse_glb(&glb).unwrap().triangles(), 6);
        let (b, _) = crate::load(&tetra_stl_binary(), "t.stl").unwrap();
        let (a, _) = crate::load(tetra_stl_ascii().as_bytes(), "t.stl").unwrap();
        assert_eq!(b.triangles(), 4);
        assert_eq!(a.triangles(), 4);
        // Z-up → Y-up: the apex (0, 0, 1) is at y = 1.
        assert_eq!(b.bounds().unwrap().1[1], 1.0);
        assert_eq!(b.bounds(), a.bounds());
        assert!(crate::load(b"garbage", "x.stl").is_err());
        assert!(crate::load(b"v 1 2\n", "x.obj").is_err());
        assert!(crate::load(b"glTFxxxx", "x.glb").is_err());
    }

    #[test]
    fn node_transforms_apply() {
        // A GLB whose node translates by (10, 0, 0) and scales by 2.
        let glb = crate::glb::write(&pyramid());
        let (json, bin) = crate::gltf::split_glb(&glb).unwrap();
        let mut doc: serde_json::Value = serde_json::from_slice(json).unwrap();
        doc["nodes"] = serde_json::json!([{ "children": [1], "translation": [10, 0, 0] }, { "mesh": 0, "scale": [2, 2, 2] }]);
        let s = crate::gltf::parse_gltf(&serde_json::to_vec(&doc).unwrap(), bin).unwrap();
        let (lo, hi) = s.bounds().unwrap();
        assert_eq!((lo[0], hi[0]), (9.0, 11.0));
        assert_eq!((lo[1], hi[1]), (-1.0, 1.0));
    }

    fn view_for(s: &Scene) -> View3d {
        let (lo, hi) = s.bounds().unwrap();
        let mut v = View3d::fit(lo, hi);
        v.rot = [20.0 * 60_000.0, 30.0 * 60_000.0, 0.0];
        v
    }

    #[test]
    fn render_is_deterministic_and_covers_the_model() {
        let s = cube();
        let mut v = view_for(&s);
        let (w, h) = v.frame_for(&s, 2_000_000.0, 2_000_000.0);
        let a = crate::raster::render(&s, &v, w, h, 64, 64);
        let b = crate::raster::render(&s, &v, w, h, 64, 64);
        assert_eq!(a, b);
        // Centre opaque, corners transparent-ish, several face colours visible.
        let at = |x: usize, y: usize| &a[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(32, 32)[3], 255);
        assert_eq!(at(0, 0)[3], 0);
        let mut colours: Vec<[u8; 3]> = a
            .chunks_exact(4)
            .filter(|p| p[3] == 255)
            .map(|p| [p[0] / 32, p[1] / 32, p[2] / 32])
            .collect();
        colours.sort_unstable();
        colours.dedup();
        assert!(colours.len() >= 3, "{colours:?}");
    }

    #[test]
    fn render_png_snapshot() {
        // Deterministic picture: the checksum of the pixels pins the renderer. Update the
        // constant (and look at the PNG written to the target dir) after a deliberate change.
        let s = textured_cube();
        let mut v = view_for(&s);
        let (w, h) = v.frame_for(&s, 1_800_000.0, 1_800_000.0);
        let rgba = crate::raster::render(&s, &v, w, h, 96, 96);
        let png = crate::scene::encode_png(96, 96, &rgba);
        let out = std::env::temp_dir().join("bo-model3d-textured-cube.png");
        let _ = std::fs::write(&out, &png);
        let sum = rgba.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        let opaque = rgba.chunks_exact(4).filter(|p| p[3] == 255).count();
        assert!(opaque > 96 * 96 / 4, "{opaque}");
        assert_eq!(sum, SNAPSHOT, "pixels changed; see {}", out.display());
    }

    /// FNV-1a of the pixels of `render_png_snapshot`.
    const SNAPSHOT: u64 = 505_937_847_043_923_585;
}
