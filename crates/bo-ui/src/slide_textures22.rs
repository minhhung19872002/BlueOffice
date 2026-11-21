//! Textures of BlueSlide's Format Background › Picture or texture fill (round 22): our own
//! procedural materials (papers, cloths, stones, woods), drawn from seamless periodic value
//! noise so that they tile without seams, encoded as PNG for the slide's picture fill.
//! Nothing is taken from another product's texture files.
use bo_render::Rgba;

/// Texture side, px.
pub const SIZE: usize = 128;

/// Texture names (English, for tests and tooltips).
pub const NAMES: [&str; 25] = [
    "Parchment",
    "Linen",
    "Denim",
    "Basket weave",
    "Rain",
    "Crumpled paper",
    "Sandstone",
    "Gravel",
    "Green stone",
    "White marble",
    "Brown marble",
    "Granite",
    "Newsprint",
    "Recycled paper",
    "Cream paper",
    "Ivory",
    "Blue tissue",
    "Rose tissue",
    "Violet felt",
    "Bubbles",
    "Oak",
    "Walnut",
    "Cork",
    "Slate",
    "Moss",
];

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// Value noise on a lattice of `px` × `py` cells over the tile (periodic: seamless).
fn noise(u: f32, v: f32, px: i32, py: i32, seed: u32) -> f32 {
    let (x, y) = (u * px as f32, v * py as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let at = |i: i32, j: i32| hash(i.rem_euclid(px), j.rem_euclid(py), seed);
    let a = at(ix, iy) + (at(ix + 1, iy) - at(ix, iy)) * sx;
    let b = at(ix, iy + 1) + (at(ix + 1, iy + 1) - at(ix, iy + 1)) * sx;
    a + (b - a) * sy
}

/// Fractal sum of `oct` octaves starting at `px` × `py` cells (0..1).
fn fbm(u: f32, v: f32, px: i32, py: i32, oct: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..oct {
        let k = 1 << o;
        sum += amp * noise(u, v, px * k, py * k, seed + o);
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn c(v: u32) -> [f32; 3] {
    [
        ((v >> 16) & 255) as f32,
        ((v >> 8) & 255) as f32,
        (v & 255) as f32,
    ]
}

/// Colour of texture `i` at (`u`, `v`) in 0..1.
fn sample(i: usize, u: f32, v: f32) -> [f32; 3] {
    let tau = std::f32::consts::TAU;
    let seed = i as u32 * 7 + 1;
    match i {
        // Fibrous papers: long streaks across, soft blotches.
        0 | 14 | 15 => {
            let (a, b) = match i {
                0 => (0xE9_D3A2, 0xC9_A86A),
                14 => (0xF8_EFD8, 0xE6_D4AE),
                _ => (0xFB_F6EA, 0xED_E3CF),
            };
            let streak = fbm(u, v, 2, 24, 3, seed);
            let blot = fbm(u, v, 3, 3, 3, seed + 9);
            mix(c(a), c(b), streak * 0.7 + blot * 0.5 - 0.25)
        }
        // Woven cloths: threads both ways.
        1..=3 => {
            let n = if i == 3 { 8.0 } else { 32.0 };
            let (a, b) = match i {
                1 => (0xEF_E8DA, 0xCF_C3AA),
                2 => (0x6F_8FD6, 0x2F_4F9A),
                _ => (0xD9_B26A, 0x7A_5A2A),
            };
            let shift = if i == 2 { u } else { 0.0 };
            let wx = ((v + shift) * n * tau).sin();
            let wy = (u * n * tau).sin();
            let cell = ((u * n).floor() + (v * n).floor()) as i32;
            let over = if cell % 2 == 0 { wx } else { wy };
            let grain = fbm(u, v, 16, 16, 2, seed);
            mix(c(a), c(b), 0.45 + over * 0.3 + (grain - 0.5) * 0.5)
        }
        // Rain: pale cyan with brighter rounded drops.
        4 => {
            let base = fbm(u, v, 4, 4, 3, seed);
            let drops = noise(u, v, 6, 6, seed + 3);
            let d = ((drops - 0.62) * 6.0).clamp(0.0, 1.0);
            mix(mix(c(0x9F_D8E4), c(0xC8_EEF4), base), c(0xF2_FCFF), d)
        }
        // Crumpled paper: folds as ridged noise.
        5 => {
            let r = 1.0 - (fbm(u, v, 5, 5, 4, seed) * 2.0 - 1.0).abs();
            mix(c(0x8E_6B3F), c(0xD9_B98A), r * r)
        }
        // Speckled stones.
        6 | 7 | 11 | 13 | 22 | 12 => {
            let (a, b, k, cells) = match i {
                6 => (0xD8_C3A0, 0xB8_9D70, 0.5, 24),
                7 => (0x8C_8273, 0xD0_C6B4, 1.4, 48),
                11 => (0x5A_5A5A, 0xE4_E4E4, 1.6, 40),
                13 => (0xE0_D6C2, 0xC4_B699, 0.8, 48),
                22 => (0xB0_6A30, 0xE0_A060, 1.3, 32),
                _ => (0xF4_F4F2, 0xD8_D8D4, 0.6, 32),
            };
            let s = noise(u, v, cells, cells, seed);
            let soft = fbm(u, v, 4, 4, 3, seed + 5);
            mix(c(a), c(b), soft * 0.6 + (s - 0.5) * k + 0.2)
        }
        // Marbles: veins along a turbulent sine.
        8 | 9 | 10 | 23 => {
            let (a, b) = match i {
                8 => (0x1F_5A32, 0x7F_B08A),
                9 => (0xF4_F4F4, 0x9A_9A9A),
                10 => (0x3A_2414, 0x9C_7A58),
                _ => (0x4A_5560, 0x7A_8590),
            };
            let t = fbm(u, v, 3, 3, 4, seed);
            let vein = ((u * 2.0 + v + t * 3.0) * tau).sin().abs();
            mix(c(b), c(a), vein.powf(0.35))
        }
        // Tissues and felts: soft clouds.
        16 | 17 | 18 | 19 | 24 => {
            let (a, b) = match i {
                16 => (0xDD_EBFA, 0xF7_FBFF),
                17 => (0xE8_BFC0, 0xF6_E1E1),
                18 => (0x2E_0A5E, 0x6A_3AA0),
                19 => (0xB8_C8F0, 0xE8_D0F0),
                _ => (0x4E_6B2A, 0x8A_A552),
            };
            mix(c(a), c(b), fbm(u, v, 4, 4, 4, seed))
        }
        // Woods: rings along the grain.
        _ => {
            let (a, b) = if i == 20 {
                (0xC9_9A5E, 0x9A_6A34)
            } else {
                (0x6A_4220, 0x3E_2410)
            };
            let w = fbm(u, v, 2, 6, 3, seed);
            let ring = ((u * 6.0 + w * 2.5) * tau).sin() * 0.5 + 0.5;
            mix(
                c(a),
                c(b),
                ring * 0.8 + (fbm(u, v, 1, 32, 2, seed + 4) - 0.5) * 0.4,
            )
        }
    }
}

/// Pixels of texture `i` (RGBA, `SIZE` × `SIZE`).
#[must_use]
pub fn pixels(i: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let [r, g, b] = sample(i, x as f32 / SIZE as f32, y as f32 / SIZE as f32);
            let q = |v: f32| v.round().clamp(0.0, 255.0) as u8;
            out.extend_from_slice(&[q(r), q(g), q(b), 255]);
        }
    }
    out
}

/// Texture `i` as a PNG file.
#[must_use]
pub fn png(i: usize) -> Option<Vec<u8>> {
    bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: SIZE as u32,
        height: SIZE as u32,
        pixels: pixels(i),
    })
}

/// A coarse `n` × `n` preview of texture `i` for galleries (cell colours, row by row).
#[must_use]
pub fn preview(i: usize, n: usize) -> Vec<Rgba> {
    let n = n.max(1);
    (0..n * n)
        .map(|k| {
            let (x, y) = (k % n, k / n);
            let [r, g, b] = sample(i, (x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32);
            Rgba(r as u8, g as u8, b as u8, 255)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textures_are_seamless_distinct_pngs() {
        for (i, name) in NAMES.iter().enumerate() {
            let px = pixels(i);
            assert_eq!(px.len(), SIZE * SIZE * 4);
            // Seamless: the first column continues from the last.
            let at = |x: usize, y: usize| {
                let k = (y * SIZE + x) * 4;
                i32::from(px[k]) + i32::from(px[k + 1]) + i32::from(px[k + 2])
            };
            let jump: i32 = (0..SIZE)
                .map(|y| (at(0, y) - at(SIZE - 1, y)).abs())
                .max()
                .unwrap_or(0);
            assert!(jump < 160, "{name} jump {jump}");
            assert!(png(i).is_some_and(|b| b.starts_with(b"\x89PNG")));
        }
        assert_ne!(preview(0, 4), preview(2, 4));
    }
}
