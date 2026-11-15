//! Mesh shadings (ISO 32000-1, §8.7.4.5.5–8.7.4.5.8: free-form and lattice-form triangle
//! meshes, Coons and tensor-product patch meshes) and function-based shadings (§8.7.4.5.2),
//! approximated by small flat-coloured triangles (patches subdivided into a grid; a tensor
//! patch is drawn as the Coons patch of its boundary).
use crate::function::Function;
use crate::interp::{apply, M};
use lopdf::{Dictionary, Document};

/// A vertex: position and colour components.
type Vertex = ((f32, f32), Vec<f32>);
/// A patch: its 12 boundary points and 4 corner colours.
type Patch = ([(f32, f32); 12], [Vec<f32>; 4]);

/// Most triangles one shading may produce.
const MAX_TRIANGLES: usize = 40_000;

/// A triangle in shading space with the colour components at its centre.
pub(crate) struct Tri {
    pub pts: [(f32, f32); 3],
    pub color: Vec<f32>,
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn read(&mut self, n: u32) -> Option<u64> {
        if n == 0 || n > 32 {
            return if n == 0 { Some(0) } else { None };
        }
        let mut v = 0_u64;
        for _ in 0..n {
            let byte = *self.data.get(self.pos / 8)?;
            let bit = (byte >> (7 - self.pos % 8)) & 1;
            v = (v << 1) | u64::from(bit);
            self.pos += 1;
        }
        Some(v)
    }

    fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }

    fn more(&self) -> bool {
        self.pos / 8 < self.data.len()
    }
}

struct Layout {
    bpc_coord: u32,
    bpc_comp: u32,
    bpf: u32,
    decode: Vec<f32>,
    /// Colour components per vertex (1 with a function).
    ncomp: usize,
}

impl Layout {
    fn coord(&self, b: &mut Bits<'_>, axis: usize) -> Option<f32> {
        let raw = b.read(self.bpc_coord)? as f32;
        let max = ((1_u64 << self.bpc_coord) - 1) as f32;
        let (d0, d1) = (*self.decode.get(axis * 2)?, *self.decode.get(axis * 2 + 1)?);
        Some(d0 + raw * (d1 - d0) / max)
    }

    fn point(&self, b: &mut Bits<'_>) -> Option<(f32, f32)> {
        Some((self.coord(b, 0)?, self.coord(b, 1)?))
    }

    fn color(&self, b: &mut Bits<'_>) -> Option<Vec<f32>> {
        let max = ((1_u64 << self.bpc_comp) - 1) as f32;
        (0..self.ncomp)
            .map(|k| {
                let raw = b.read(self.bpc_comp)? as f32;
                let (d0, d1) = (
                    self.decode.get(4 + k * 2).copied().unwrap_or(0.0),
                    self.decode.get(5 + k * 2).copied().unwrap_or(1.0),
                );
                Some(d0 + raw * (d1 - d0) / max)
            })
            .collect()
    }
}

fn mix(a: &[f32], b: &[f32], c: &[f32]) -> Vec<f32> {
    a.iter()
        .zip(b)
        .zip(c)
        .map(|((x, y), z)| (x + y + z) / 3.0)
        .collect()
}

fn num(d: &Dictionary, k: &[u8]) -> Option<i64> {
    d.get(k).ok()?.as_i64().ok()
}

/// Triangles of a mesh shading of `kind` 4–7 (`data` is its decoded stream).
pub(crate) fn mesh(
    kind: i64,
    dict: &Dictionary,
    data: &[u8],
    ncomp_space: usize,
    has_function: bool,
) -> Vec<Tri> {
    let decode: Vec<f32> = dict
        .get(b"Decode")
        .ok()
        .and_then(|o| o.as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_float().ok()).collect())
        .unwrap_or_default();
    let l = Layout {
        bpc_coord: num(dict, b"BitsPerCoordinate").unwrap_or(0).clamp(0, 32) as u32,
        bpc_comp: num(dict, b"BitsPerComponent").unwrap_or(0).clamp(0, 16) as u32,
        bpf: num(dict, b"BitsPerFlag").unwrap_or(0).clamp(0, 8) as u32,
        decode,
        ncomp: if has_function { 1 } else { ncomp_space },
    };
    if l.bpc_coord == 0 || l.bpc_comp == 0 || l.decode.len() < 4 + 2 * l.ncomp {
        return Vec::new();
    }
    let mut b = Bits { data, pos: 0 };
    match kind {
        4 => free_form(&l, &mut b),
        5 => {
            let per_row = num(dict, b"VerticesPerRow").unwrap_or(0).max(0) as usize;
            lattice(&l, &mut b, per_row)
        }
        6 | 7 => patches(&l, &mut b, kind == 7),
        _ => Vec::new(),
    }
}

fn free_form(l: &Layout, b: &mut Bits<'_>) -> Vec<Tri> {
    let mut out = Vec::new();
    let mut verts: Vec<Vertex> = Vec::new();
    while b.more() && out.len() < MAX_TRIANGLES {
        let Some(flag) = (if l.bpf > 0 { b.read(l.bpf) } else { Some(0) }) else {
            break;
        };
        let (Some(p), Some(c)) = (l.point(b), l.color(b)) else {
            break;
        };
        b.align();
        match flag {
            0 => {
                verts.clear();
                verts.push((p, c));
                // The next two vertices complete the triangle (their flags are ignored).
                for _ in 0..2 {
                    let _ = b.read(l.bpf);
                    let (Some(p), Some(c)) = (l.point(b), l.color(b)) else {
                        return out;
                    };
                    b.align();
                    verts.push((p, c));
                }
            }
            1 | 2 if verts.len() >= 3 => {
                let n = verts.len();
                let (a, bb) = if flag == 1 {
                    (verts[n - 2].clone(), verts[n - 1].clone())
                } else {
                    (verts[n - 3].clone(), verts[n - 1].clone())
                };
                verts = vec![a, bb, (p, c)];
            }
            _ => continue,
        }
        let n = verts.len();
        let (a, bb, c) = (&verts[n - 3], &verts[n - 2], &verts[n - 1]);
        out.push(Tri {
            pts: [a.0, bb.0, c.0],
            color: mix(&a.1, &bb.1, &c.1),
        });
    }
    out
}

fn lattice(l: &Layout, b: &mut Bits<'_>, per_row: usize) -> Vec<Tri> {
    if per_row < 2 {
        return Vec::new();
    }
    let mut rows: Vec<Vec<Vertex>> = Vec::new();
    'outer: while b.more() {
        let mut row = Vec::with_capacity(per_row);
        for _ in 0..per_row {
            let (Some(p), Some(c)) = (l.point(b), l.color(b)) else {
                break 'outer;
            };
            b.align();
            row.push((p, c));
        }
        rows.push(row);
        if rows.len() * per_row * 2 > MAX_TRIANGLES {
            break;
        }
    }
    let mut out = Vec::new();
    for pair in rows.windows(2) {
        let (r0, r1) = (&pair[0], &pair[1]);
        for i in 0..per_row - 1 {
            let (a, bb, c, d) = (&r0[i], &r0[i + 1], &r1[i], &r1[i + 1]);
            out.push(Tri {
                pts: [a.0, bb.0, c.0],
                color: mix(&a.1, &bb.1, &c.1),
            });
            out.push(Tri {
                pts: [bb.0, d.0, c.0],
                color: mix(&bb.1, &d.1, &c.1),
            });
        }
    }
    out
}

/// A cubic Bézier point.
fn bez(p: [(f32, f32); 4], t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    (
        a * p[0].0 + b * p[1].0 + c * p[2].0 + d * p[3].0,
        a * p[0].1 + b * p[1].1 + c * p[2].1 + d * p[3].1,
    )
}

fn patches(l: &Layout, b: &mut Bits<'_>, tensor: bool) -> Vec<Tri> {
    let mut out = Vec::new();
    // Boundary in the order of the stream: p00 p01 p02 p03 p13 p23 p33 p32 p31 p30 p20 p10.
    let mut prev: Option<Patch> = None;
    while b.more() && out.len() < MAX_TRIANGLES {
        let Some(flag) = b.read(l.bpf.clamp(1, 8)) else {
            break;
        };
        let mut pts = [(0.0, 0.0); 12];
        let mut cols: [Vec<f32>; 4] = Default::default();
        let start = if flag == 0 {
            0
        } else {
            let Some((pp, pc)) = prev.as_ref() else {
                break;
            };
            // Shared edge (§8.7.4.5.7, Table 85).
            let (edge, c): ([usize; 4], [usize; 2]) = match flag {
                1 => ([3, 4, 5, 6], [1, 2]),
                2 => ([6, 7, 8, 9], [2, 3]),
                _ => ([9, 10, 11, 0], [3, 0]),
            };
            for (k, e) in edge.iter().enumerate() {
                pts[k] = pp[*e];
            }
            cols[0] = pc[c[0]].clone();
            cols[1] = pc[c[1]].clone();
            4
        };
        let mut ok = true;
        for p in pts.iter_mut().skip(start) {
            match l.point(b) {
                Some(v) => *p = v,
                None => ok = false,
            }
        }
        if tensor {
            // Interior points p11 p12 p22 p21: not used (Coons approximation).
            for _ in 0..4 {
                ok &= l.point(b).is_some();
            }
        }
        for c in cols.iter_mut().skip(if flag == 0 { 0 } else { 2 }) {
            match l.color(b) {
                Some(v) => *c = v,
                None => ok = false,
            }
        }
        if !ok {
            break;
        }
        b.align();
        // Colours: c00 (p00), c03 (p03), c33 (p33), c30 (p30).
        let p = pts;
        let d1 = [p[0], p[1], p[2], p[3]]; // left, v from p00 to p03
        let c2 = [p[3], p[4], p[5], p[6]]; // top, u from p03 to p33
        let d2 = [p[9], p[8], p[7], p[6]]; // right, v from p30 to p33
        let c1 = [p[0], p[11], p[10], p[9]]; // bottom, u from p00 to p30
        let span = |q: [(f32, f32); 4]| {
            q.windows(2)
                .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
                .sum::<f32>()
        };
        let size = span(d1).max(span(c2)).max(span(d2)).max(span(c1));
        let n = ((size / 8.0).ceil() as usize).clamp(2, 16);
        let s = |u: f32, v: f32| -> (f32, f32) {
            let (a, bb, c, d) = (bez(c1, u), bez(c2, u), bez(d1, v), bez(d2, v));
            let corner = |i: usize| p[i];
            let x = (1.0 - v) * a.0 + v * bb.0 + (1.0 - u) * c.0 + u * d.0
                - ((1.0 - u) * (1.0 - v) * corner(0).0
                    + (1.0 - u) * v * corner(3).0
                    + u * (1.0 - v) * corner(9).0
                    + u * v * corner(6).0);
            let y = (1.0 - v) * a.1 + v * bb.1 + (1.0 - u) * c.1 + u * d.1
                - ((1.0 - u) * (1.0 - v) * corner(0).1
                    + (1.0 - u) * v * corner(3).1
                    + u * (1.0 - v) * corner(9).1
                    + u * v * corner(6).1);
            (x, y)
        };
        let (c00, c03, c33, c30) = (&cols[0], &cols[1], &cols[2], &cols[3]);
        let color = |u: f32, v: f32| -> Vec<f32> {
            (0..c00.len())
                .map(|k| {
                    let g = |c: &Vec<f32>| c.get(k).copied().unwrap_or(0.0);
                    (1.0 - u) * (1.0 - v) * g(c00)
                        + (1.0 - u) * v * g(c03)
                        + u * v * g(c33)
                        + u * (1.0 - v) * g(c30)
                })
                .collect()
        };
        let step = 1.0 / n as f32;
        for i in 0..n {
            for j in 0..n {
                let (u0, v0) = (i as f32 * step, j as f32 * step);
                let (u1, v1) = (u0 + step, v0 + step);
                let (a, bb, c, d) = (s(u0, v0), s(u1, v0), s(u1, v1), s(u0, v1));
                let col = color(u0 + step / 2.0, v0 + step / 2.0);
                out.push(Tri {
                    pts: [a, bb, c],
                    color: col.clone(),
                });
                out.push(Tri {
                    pts: [a, c, d],
                    color: col,
                });
            }
        }
        prev = Some((pts, cols));
    }
    out
}

/// Triangles of a function-based shading (type 1) over its domain, mapped by `/Matrix`.
pub(crate) fn function_based(doc: &Document, dict: &Dictionary, func: &Function) -> Vec<Tri> {
    let nums = |k: &[u8]| -> Vec<f32> {
        dict.get(k)
            .ok()
            .map(|o| doc.dereference(o).map_or(o, |(_, o)| o))
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_float().ok()).collect())
            .unwrap_or_default()
    };
    let domain = nums(b"Domain");
    let [x0, x1, y0, y1] = <[f32; 4]>::try_from(domain.as_slice()).unwrap_or([0.0, 1.0, 0.0, 1.0]);
    let m: M =
        <[f32; 6]>::try_from(nums(b"Matrix").as_slice()).unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let n = 24;
    let mut out = Vec::with_capacity(n * n * 2);
    for i in 0..n {
        for j in 0..n {
            let (u0, v0) = (
                x0 + (x1 - x0) * i as f32 / n as f32,
                y0 + (y1 - y0) * j as f32 / n as f32,
            );
            let (u1, v1) = (
                x0 + (x1 - x0) * (i + 1) as f32 / n as f32,
                y0 + (y1 - y0) * (j + 1) as f32 / n as f32,
            );
            let col = func.eval(&[f32::midpoint(u0, u1), f32::midpoint(v0, v1)]);
            let (a, b, c, d) = (
                apply(m, u0, v0),
                apply(m, u1, v0),
                apply(m, u1, v1),
                apply(m, u0, v1),
            );
            out.push(Tri {
                pts: [a, b, c],
                color: col.clone(),
            });
            out.push(Tri {
                pts: [a, c, d],
                color: col,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coons_patch_and_triangles() {
        // One Coons patch: a unit square with straight edges, 8-bit coordinates over 0..255.
        let mut d = Dictionary::new();
        d.set("BitsPerCoordinate", 8);
        d.set("BitsPerComponent", 8);
        d.set("BitsPerFlag", 8);
        d.set(
            "Decode",
            vec![
                0.into(),
                255.into(),
                0.into(),
                255.into(),
                0.into(),
                1.into(),
            ],
        );
        let pts: [(u8, u8); 12] = [
            (0, 0),
            (0, 33),
            (0, 66),
            (0, 100),
            (33, 100),
            (66, 100),
            (100, 100),
            (100, 66),
            (100, 33),
            (100, 0),
            (66, 0),
            (33, 0),
        ];
        let mut data = vec![0_u8];
        for (x, y) in pts {
            data.extend([x, y]);
        }
        data.extend([0, 255, 255, 0]);
        let tris = mesh(6, &d, &data, 1, false);
        assert!(tris.len() >= 8);
        // Corner colours: dark at p00 and p30, light at p03 and p33.
        let near = |x: f32, y: f32| {
            tris.iter()
                .min_by(|a, b| {
                    let da = (a.pts[0].0 - x).abs() + (a.pts[0].1 - y).abs();
                    let db = (b.pts[0].0 - x).abs() + (b.pts[0].1 - y).abs();
                    da.total_cmp(&db)
                })
                .map_or(-1.0, |t| t.color[0])
        };
        assert!(near(0.0, 0.0) < 0.3, "{}", near(0.0, 0.0));
        assert!(near(0.0, 90.0) > 0.7);
        // Free-form triangle.
        let mut d4 = d.clone();
        d4.set("BitsPerFlag", 8);
        let data4 = [0, 0, 0, 0, 0, 100, 0, 128, 0, 0, 100, 255];
        let t4 = mesh(4, &d4, &data4, 1, false);
        assert_eq!(t4.len(), 1);
        assert!((t4[0].color[0] - (128.0 / 255.0) / 3.0 - 1.0 / 3.0).abs() < 0.01);
    }
}
