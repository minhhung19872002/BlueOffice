//! The placeholder picture of a picture content control (ECMA-376 Part 1, §17.5.2.26 picture):
//! BlueOffice's own drawing — a light frame with a sun and two hills — made here as a PNG so the
//! saved file holds an ordinary picture (§17.5.2.39 showingPlcHdr marks it as the placeholder).

/// Width and height of the placeholder in the document (one inch).
pub(crate) const SIZE_EMU: i64 = 914_400;

/// Pixels per side of the picture.
const PX: u32 = 96;

/// The placeholder picture as PNG bytes.
pub(crate) fn placeholder_png() -> Vec<u8> {
    let n = PX as usize;
    let mut rgba = vec![0u8; n * n * 4];
    let pf = f64::from(PX);
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            // Paper, a grey frame, a sun at the top right and two hills at the bottom.
            let border = x < 2 || y < 2 || x >= n - 2 || y >= n - 2;
            let sun = (fx - pf * 0.68).hypot(fy - pf * 0.3) < pf * 0.1;
            let hill1 = fy > pf * 0.78 - (pf * 0.32 - (fx - pf * 0.35).abs()).max(0.0) * 0.9;
            let hill2 = fy > pf * 0.8 - (pf * 0.25 - (fx - pf * 0.7).abs()).max(0.0) * 0.8;
            let inside = fx > 10.0 && fx < pf - 10.0 && fy > 10.0 && fy < pf - 10.0;
            let c: [u8; 3] = if border {
                [0x9A, 0xA4, 0xB1]
            } else if inside && hill1 {
                [0x7F, 0x8C, 0x9C]
            } else if inside && hill2 {
                [0xA3, 0xAE, 0xBB]
            } else if inside && sun {
                [0xB8, 0xC2, 0xCE]
            } else {
                [0xF1, 0xF3, 0xF6]
            };
            let i = (y * n + x) * 4;
            rgba[i..i + 3].copy_from_slice(&c);
            rgba[i + 3] = 0xFF;
        }
    }
    bo_model3d::scene::encode_png(PX, PX, &rgba)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_is_a_png() {
        let png = super::placeholder_png();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }
}
