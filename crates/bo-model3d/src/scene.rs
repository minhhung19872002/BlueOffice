//! A flattened scene: triangle primitives in model space (node transforms applied),
//! materials and decoded textures.

/// A decoded texture (straight RGBA, 8 bits per channel).
#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height × 4` bytes.
    pub rgba: Vec<u8>,
    /// The encoded picture as read and its MIME type (written back to GLB unchanged).
    pub source: Option<(Vec<u8>, String)>,
}

impl Texture {
    /// Colour at texture coordinates (u, v) (glTF: origin top left, repeat wrapping), bilinear.
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        if self.width == 0 || self.height == 0 {
            return [1.0; 4];
        }
        let (w, h) = (self.width as f32, self.height as f32);
        let x = (u - u.floor()) * w - 0.5;
        let y = (v - v.floor()) * h - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let px = |xi: f32, yi: f32| -> [f32; 4] {
            let xi = (xi as i64).rem_euclid(self.width as i64) as usize;
            let yi = (yi as i64).rem_euclid(self.height as i64) as usize;
            let i = (yi * self.width as usize + xi) * 4;
            let p = self.rgba.get(i..i + 4).unwrap_or(&[255, 255, 255, 255]);
            [
                f32::from(p[0]) / 255.0,
                f32::from(p[1]) / 255.0,
                f32::from(p[2]) / 255.0,
                f32::from(p[3]) / 255.0,
            ]
        };
        let (a, b, c, d) = (
            px(x0, y0),
            px(x0 + 1.0, y0),
            px(x0, y0 + 1.0),
            px(x0 + 1.0, y0 + 1.0),
        );
        let mut o = [0.0; 4];
        for k in 0..4 {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bot = c[k] + (d[k] - c[k]) * fx;
            o[k] = top + (bot - top) * fy;
        }
        o
    }
}

/// glTF 2.0 §3.9.2 metallic-roughness material (the parts the renderer uses).
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// `baseColorFactor` (linear RGBA).
    pub base: [f32; 4],
    /// Index into [`Scene::textures`] of `baseColorTexture`.
    pub texture: Option<usize>,
    /// `metallicFactor`.
    pub metallic: f32,
    /// `roughnessFactor`.
    pub roughness: f32,
}

impl Default for Material {
    fn default() -> Self {
        // glTF 2.0 §3.9.6 default material.
        Material {
            base: [1.0, 1.0, 1.0, 1.0],
            texture: None,
            metallic: 1.0,
            roughness: 1.0,
        }
    }
}

/// Triangles sharing a material.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prim {
    /// Vertex positions (model units).
    pub positions: Vec<[f32; 3]>,
    /// Unit normals, one per vertex.
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates (empty, or one per vertex).
    pub uvs: Vec<[f32; 2]>,
    /// Vertex colours (empty, or one per vertex; linear RGBA).
    pub colors: Vec<[f32; 4]>,
    /// Three vertex indices per triangle.
    pub indices: Vec<u32>,
    /// Index into [`Scene::materials`].
    pub material: usize,
}

/// A flattened model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    /// Primitives.
    pub prims: Vec<Prim>,
    /// Materials (at least one when there are primitives).
    pub materials: Vec<Material>,
    /// Decoded textures.
    pub textures: Vec<Texture>,
}

impl Scene {
    /// Bounding box `(min, max)` of all vertices used by triangles.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        let mut any = false;
        for p in &self.prims {
            for &i in &p.indices {
                if let Some(v) = p.positions.get(i as usize) {
                    for k in 0..3 {
                        lo[k] = lo[k].min(v[k]);
                        hi[k] = hi[k].max(v[k]);
                    }
                    any = true;
                }
            }
        }
        any.then_some((lo, hi))
    }

    /// Number of triangles.
    pub fn triangles(&self) -> usize {
        self.prims.iter().map(|p| p.indices.len() / 3).sum()
    }

    /// Material of a primitive (the default material when out of range).
    pub fn material(&self, p: &Prim) -> Material {
        self.materials.get(p.material).cloned().unwrap_or_default()
    }

    /// Fill missing normals with face normals (accumulated per vertex) and drop indices
    /// out of range.
    pub fn finish(&mut self) {
        for p in &mut self.prims {
            let n = p.positions.len() as u32;
            let mut ok = Vec::with_capacity(p.indices.len());
            for t in p.indices.chunks_exact(3) {
                if t.iter().all(|&i| i < n) {
                    ok.extend_from_slice(t);
                }
            }
            p.indices = ok;
            if p.normals.len() == p.positions.len() {
                for v in &mut p.normals {
                    *v = normalize(*v);
                }
            } else {
                p.normals = vec![[0.0; 3]; p.positions.len()];
                for t in p.indices.chunks_exact(3) {
                    let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
                    let fnorm = cross(
                        sub(p.positions[b], p.positions[a]),
                        sub(p.positions[c], p.positions[a]),
                    );
                    for &i in &[a, b, c] {
                        for (n, f) in p.normals[i].iter_mut().zip(fnorm) {
                            *n += f;
                        }
                    }
                }
                for v in &mut p.normals {
                    *v = normalize(*v);
                }
            }
            if !p.uvs.is_empty() && p.uvs.len() != p.positions.len() {
                p.uvs.clear();
            }
            if !p.colors.is_empty() && p.colors.len() != p.positions.len() {
                p.colors.clear();
            }
        }
        self.prims.retain(|p| !p.indices.is_empty());
        if self.materials.is_empty() {
            self.materials.push(Material::default());
        }
    }
}

pub(crate) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(crate) fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-20 && l.is_finite() {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Decode a PNG or JPEG picture to straight RGBA.
pub fn decode_picture(bytes: &[u8]) -> Option<Texture> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        buf.truncate(info.buffer_size());
        let n = match info.color_type {
            png::ColorType::Grayscale => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
            png::ColorType::Indexed => return None,
        };
        let mut rgba = Vec::with_capacity(buf.len() / n * 4);
        for px in buf.chunks_exact(n) {
            match n {
                1 => rgba.extend_from_slice(&[px[0], px[0], px[0], 255]),
                2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
                3 => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
                _ => rgba.extend_from_slice(px),
            }
        }
        return Some(Texture {
            width: info.width,
            height: info.height,
            rgba,
            source: Some((bytes.to_vec(), "image/png".into())),
        });
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let opts = zune_jpeg::zune_core::options::DecoderOptions::default()
            .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA);
        let mut dec = zune_jpeg::JpegDecoder::new_with_options(bytes, opts);
        let rgba = dec.decode().ok()?;
        let (w, h) = dec.dimensions()?;
        if rgba.len() != w * h * 4 {
            return None;
        }
        return Some(Texture {
            width: w as u32,
            height: h as u32,
            rgba,
            source: Some((bytes.to_vec(), "image/jpeg".into())),
        });
    }
    None
}

/// Encode straight RGBA as PNG.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width.max(1), height.max(1));
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let Ok(mut w) = enc.write_header() else {
            return Vec::new();
        };
        if w.write_image_data(rgba).is_err() {
            return Vec::new();
        }
    }
    out
}
