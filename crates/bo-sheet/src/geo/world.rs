//! The bundled world boundaries: Natural Earth 1:50m Admin-0 countries (public domain), simplified
//! and quantised by `scripts/build-world-map.mjs` into `assets/maps/world/countries.bin` (format in
//! the script's header; source, version and terms in `assets/maps/README.md`).

use super::{Boundaries, BoundaryProvider, MapLevel, Region};
use std::sync::{Arc, OnceLock};

/// The bundled file.
pub const COUNTRIES_BIN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/maps/world/countries.bin"
));

const MAGIC: &[u8; 8] = b"BOGEO1\0\0";

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n)?)?;
        self.p += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.bytes(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.bytes(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.bytes(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// Zig-zag LEB128 varint.
    fn varint(&mut self) -> Option<i64> {
        let mut v: u64 = 0;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= u64::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                let z = (v >> 1) as i64;
                return Some(if v & 1 == 0 { z } else { -z - 1 });
            }
        }
        None
    }
}

/// Decodes a `BOGEO1` file into regions (codes: alpha-2, alpha-3; names: English, Vietnamese, …).
#[must_use]
pub fn decode(b: &[u8]) -> Option<Vec<Region>> {
    let mut r = Reader { b, p: 0 };
    if r.bytes(8)? != MAGIC {
        return None;
    }
    let n = r.u32()? as usize;
    let mut out = Vec::with_capacity(n.min(1000));
    for _ in 0..n {
        let k = r.u8()?;
        let mut strings = Vec::with_capacity(k as usize);
        for _ in 0..k {
            let len = r.u16()? as usize;
            strings.push(String::from_utf8_lossy(r.bytes(len)?).into_owned());
        }
        let codes: Vec<String> = strings
            .iter()
            .take(2)
            .filter(|s| !s.is_empty())
            .cloned()
            .collect();
        let names: Vec<String> = strings.into_iter().skip(2).collect();
        let rings_n = r.u16()?;
        let mut rings = Vec::with_capacity(rings_n as usize);
        for _ in 0..rings_n {
            let pts = r.u32()? as usize;
            let mut ring = Vec::with_capacity(pts.min(100_000));
            let (mut x, mut y) = (0i64, 0i64);
            for _ in 0..pts {
                x += r.varint()?;
                y += r.varint()?;
                ring.push((x as f64 / 100.0, y as f64 / 100.0));
            }
            rings.push(ring);
        }
        out.push(Region {
            codes,
            names,
            rings,
        });
    }
    Some(out)
}

/// The world's countries (decoded once).
#[must_use]
pub fn world() -> Arc<dyn BoundaryProvider> {
    static WORLD: OnceLock<Arc<Boundaries>> = OnceLock::new();
    WORLD
        .get_or_init(|| {
            Arc::new(Boundaries::new(
                "world",
                MapLevel::Country,
                None,
                decode(COUNTRIES_BIN).unwrap_or_default(),
                &[],
            ))
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_world_decodes_and_matches_vi_and_en_names() {
        let w = world();
        assert!(w.regions().len() > 200, "{}", w.regions().len());
        let vn = w.find("Việt Nam").expect("Việt Nam");
        assert_eq!(w.find("Vietnam"), Some(vn));
        assert_eq!(w.find("viet nam"), Some(vn));
        assert_eq!(w.find("VN"), Some(vn));
        assert_eq!(w.find("VNM"), Some(vn));
        // Hà Nội (105.85° E, 21.03° N) lies inside Viet Nam's bounding box.
        let b = w.regions()[vn].bbox().unwrap();
        assert!(
            b[0] < 105.85 && 105.85 < b[2] && b[1] < 21.03 && 21.03 < b[3],
            "{b:?}"
        );
        for n in [
            "France",
            "Pháp",
            "Japan",
            "Nhật Bản",
            "Brazil",
            "Singapore",
            "Germany",
        ] {
            assert!(w.find(n).is_some(), "{n}");
        }
        assert_eq!(w.find("Atlantis"), None);
    }

    #[test]
    fn decode_rejects_bad_data() {
        assert!(decode(b"nope").is_none());
        assert!(decode(b"BOGEO1\0\0\x05\0\0\0").is_none());
    }
}
