//! A self-made sRGB ICC profile (ICC.1:2001-04, profile version 2.1) for the
//! PDF/A output intent (ISO 19005-1 §6.2.2, ISO 19005-2 §6.2.3).
//!
//! Nothing is copied from an existing profile file: the colorants are computed
//! here from the sRGB primaries and D65 white point of IEC 61966-2-1 (§5.1,
//! x/y chromaticities) adapted to the ICC D50 profile connection space with the
//! Bradford transform (ICC.1 Annex E), and the tone curves sample the sRGB
//! transfer function (IEC 61966-2-1 §5.2). The bytes are generated at export
//! time, so the profile is BlueOffice's own work under the project licence.

/// ICC D50 PCS illuminant (ICC.1 §6.3.4.3; X 0.9642, Y 1.0, Z 0.8249).
const D50: [f64; 3] = [0.9642, 1.0, 0.8249];

/// Entries of each tone curve (`curv`, ICC.1 §6.5.3).
const CURVE_POINTS: usize = 1024;

type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn mul_v(a: &M3, v: [f64; 3]) -> [f64; 3] {
    [
        a[0][0] * v[0] + a[0][1] * v[1] + a[0][2] * v[2],
        a[1][0] * v[0] + a[1][1] * v[1] + a[1][2] * v[2],
        a[2][0] * v[0] + a[2][1] * v[1] + a[2][2] * v[2],
    ]
}

fn inverse(m: &M3) -> M3 {
    let [[a, b, c], [d, e, f], [g, h, i]] = *m;
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    let k = 1.0 / det;
    [
        [
            (e * i - f * h) * k,
            (c * h - b * i) * k,
            (b * f - c * e) * k,
        ],
        [
            (f * g - d * i) * k,
            (a * i - c * g) * k,
            (c * d - a * f) * k,
        ],
        [
            (d * h - e * g) * k,
            (b * g - a * h) * k,
            (a * e - b * d) * k,
        ],
    ]
}

fn xy_to_xyz(x: f64, y: f64) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

/// Red, green and blue colorants (columns of the RGB → XYZ D50 matrix).
pub(crate) fn colorants() -> [[f64; 3]; 3] {
    // IEC 61966-2-1 §5.1: R (0.64, 0.33), G (0.30, 0.60), B (0.15, 0.06), D65 (0.3127, 0.3290).
    let prim = [
        xy_to_xyz(0.64, 0.33),
        xy_to_xyz(0.30, 0.60),
        xy_to_xyz(0.15, 0.06),
    ];
    let white = xy_to_xyz(0.3127, 0.3290);
    let p: M3 = [
        [prim[0][0], prim[1][0], prim[2][0]],
        [prim[0][1], prim[1][1], prim[2][1]],
        [prim[0][2], prim[1][2], prim[2][2]],
    ];
    let s = mul_v(&inverse(&p), white);
    // Bradford cone response matrix (ICC.1:2001-04 Annex E).
    let bradford: M3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let src = mul_v(&bradford, white);
    let dst = mul_v(&bradford, D50);
    let scale: M3 = [
        [dst[0] / src[0], 0.0, 0.0],
        [0.0, dst[1] / src[1], 0.0],
        [0.0, 0.0, dst[2] / src[2]],
    ];
    let adapt = mul(&inverse(&bradford), &mul(&scale, &bradford));
    let mut out = [[0.0; 3]; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let col = [p[0][c] * s[c], p[1][c] * s[c], p[2][c] * s[c]];
        *o = mul_v(&adapt, col);
    }
    out
}

/// sRGB transfer function (encoded → linear), IEC 61966-2-1 §5.2.
fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn s15f16(v: f64) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(v: [f64; 3]) -> Vec<u8> {
    // ICC.1 §6.5.26 XYZType.
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for c in v {
        t.extend(s15f16(c));
    }
    t
}

fn text_tag(s: &str) -> Vec<u8> {
    // ICC.1 §6.5.18 textType.
    let mut t = b"text\0\0\0\0".to_vec();
    t.extend(s.bytes());
    t.push(0);
    t
}

fn desc_tag(s: &str) -> Vec<u8> {
    // ICC.1:2001-04 §6.5.17 textDescriptionType: ASCII, then empty Unicode and
    // ScriptCode descriptions.
    let mut t = b"desc\0\0\0\0".to_vec();
    t.extend(((s.len() + 1) as u32).to_be_bytes());
    t.extend(s.bytes());
    t.push(0);
    t.extend([0; 8]); // Unicode language code, Unicode count
    t.extend([0; 3]); // ScriptCode code, count
    t.extend([0; 67]); // ScriptCode description
    t
}

fn curve_tag() -> Vec<u8> {
    // ICC.1 §6.5.3 curveType with sampled values.
    let mut t = b"curv\0\0\0\0".to_vec();
    t.extend((CURVE_POINTS as u32).to_be_bytes());
    for i in 0..CURVE_POINTS {
        let v = srgb_to_linear(i as f64 / (CURVE_POINTS - 1) as f64);
        t.extend(((v * 65535.0).round().clamp(0.0, 65535.0) as u16).to_be_bytes());
    }
    t
}

/// Profile description shown by viewers.
pub const SRGB_DESCRIPTION: &str = "sRGB IEC61966-2.1 (BlueOffice)";

/// The sRGB display profile bytes (3 components, RGB → XYZ).
#[must_use]
pub fn srgb_profile() -> Vec<u8> {
    let [r, g, b] = colorants();
    let curve = curve_tag();
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc_tag(SRGB_DESCRIPTION)),
        (b"cprt", text_tag("No copyright, use freely")),
        (b"wtpt", xyz_tag(D50)),
        (b"rXYZ", xyz_tag(r)),
        (b"gXYZ", xyz_tag(g)),
        (b"bXYZ", xyz_tag(b)),
        (b"rTRC", curve),
    ];
    let table_len = 4 + 12 * (tags.len() + 2);
    let mut data = Vec::new();
    let mut entries = Vec::new();
    let mut offset = 128 + table_len;
    for (sig, body) in &tags {
        entries.push((**sig, offset, body.len()));
        data.extend(body);
        while data.len() % 4 != 0 {
            data.push(0);
        }
        offset = 128 + table_len + data.len();
    }
    // gTRC and bTRC share the red curve's data (ICC.1 §6.4: tags may share elements).
    let (_, trc_at, trc_len) = entries[entries.len() - 1];
    entries.push((*b"gTRC", trc_at, trc_len));
    entries.push((*b"bTRC", trc_at, trc_len));
    let size = 128 + table_len + data.len();

    // ICC.1 §6.1 header.
    let mut p = Vec::with_capacity(size);
    p.extend((size as u32).to_be_bytes());
    p.extend([0; 4]); // preferred CMM
    p.extend([0x02, 0x10, 0, 0]); // version 2.1.0
    p.extend(b"mntr"); // display device profile
    p.extend(b"RGB ");
    p.extend(b"XYZ ");
    for v in [2026_u16, 1, 1, 0, 0, 0] {
        p.extend(v.to_be_bytes()); // creation date
    }
    p.extend(b"acsp");
    p.extend([0; 4]); // primary platform
    p.extend([0; 4]); // flags
    p.extend([0; 4]); // device manufacturer
    p.extend([0; 4]); // device model
    p.extend([0; 8]); // device attributes
    p.extend([0; 4]); // rendering intent: perceptual
    for c in D50 {
        p.extend(s15f16(c));
    }
    p.extend([0; 4]); // creator
    p.resize(128, 0); // reserved
    p.extend((entries.len() as u32).to_be_bytes());
    for (sig, at, len) in &entries {
        p.extend(sig);
        p.extend((*at as u32).to_be_bytes());
        p.extend((*len as u32).to_be_bytes());
    }
    p.extend(data);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorants_match_published_d50_values() {
        // Rounded D50 sRGB colorants as tabulated in colour-science references:
        // R (0.4361, 0.2225, 0.0139), G (0.3851, 0.7169, 0.0971), B (0.1431, 0.0606, 0.7141).
        let want = [
            [0.4361, 0.2225, 0.0139],
            [0.3851, 0.7169, 0.0971],
            [0.1431, 0.0606, 0.7141],
        ];
        let got = colorants();
        for (g, w) in got.iter().zip(want) {
            for (a, b) in g.iter().zip(w) {
                assert!((a - b).abs() < 0.0015, "{got:?}");
            }
        }
        // The colorants add up to the D50 white point.
        for (i, w) in D50.iter().enumerate() {
            let sum: f64 = got.iter().map(|c| c[i]).sum();
            assert!((sum - w).abs() < 1e-4);
        }
    }

    #[test]
    fn header_and_tag_table() {
        let p = srgb_profile();
        assert_eq!(
            u32::from_be_bytes(p[0..4].try_into().unwrap()) as usize,
            p.len()
        );
        assert_eq!(&p[36..40], b"acsp");
        assert_eq!(&p[12..20], b"mntrRGB ");
        assert_eq!(p[8], 2);
        let n = u32::from_be_bytes(p[128..132].try_into().unwrap());
        assert_eq!(n, 9);
        // Every tag lies inside the profile and starts with its type signature.
        for i in 0..n as usize {
            let e = &p[132 + i * 12..144 + i * 12];
            let at = u32::from_be_bytes(e[4..8].try_into().unwrap()) as usize;
            let len = u32::from_be_bytes(e[8..12].try_into().unwrap()) as usize;
            assert!(at % 4 == 0 && at + len <= p.len());
            let ty = &p[at..at + 4];
            assert!([b"desc", b"text", b"XYZ ", b"curv"]
                .iter()
                .any(|t| ty == *t));
        }
        // Tone curve: 0 → 0, 1 → 65535, mid-grey 0.5 → 0.214 linear (IEC 61966-2-1).
        assert!((srgb_to_linear(0.5) - 0.214).abs() < 0.001);
    }
}
