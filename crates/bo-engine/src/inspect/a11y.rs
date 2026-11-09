//! Accessibility helpers shared by the three checkers: colour contrast after the
//! WCAG 2.2 definitions of relative luminance and contrast ratio (success
//! criterion 1.4.3, minimum 4.5:1, or 3:1 for large text — at least 18 pt, or
//! 14 pt bold), and simple structure checks of tables.

/// Relative luminance of an sRGB colour (WCAG 2.2 "relative luminance": each
/// channel linearised, then weighted 0.2126 R + 0.7152 G + 0.0722 B).
pub fn luminance(rgb: (u8, u8, u8)) -> f64 {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgb.0) + 0.7152 * lin(rgb.1) + 0.0722 * lin(rgb.2)
}

/// Contrast ratio `(L1 + 0.05) / (L2 + 0.05)` with L1 the lighter colour (1 to 21).
pub fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Large text for the contrast minimum: at least 18 pt, or 14 pt when bold.
pub fn is_large(size_pt: f64, bold: bool) -> bool {
    size_pt >= 18.0 || (bold && size_pt >= 14.0)
}

/// The minimum contrast ratio text of this size needs.
pub fn minimum(size_pt: f64, bold: bool) -> f64 {
    if is_large(size_pt, bold) {
        3.0
    } else {
        4.5
    }
}

/// `Some(ratio)` (rounded to 2 decimals) when text `fg` on `bg` is below the minimum.
pub fn low_contrast(fg: (u8, u8, u8), bg: (u8, u8, u8), size_pt: f64, bold: bool) -> Option<f64> {
    let r = contrast(fg, bg);
    (r + 1e-9 < minimum(size_pt, bold)).then(|| (r * 100.0).round() / 100.0)
}

/// `RRGGBB` or `AARRGGBB` hex to a colour (alpha ignored).
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.trim().trim_start_matches('#');
    let s = if s.len() == 8 { &s[2..] } else { s };
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(rgb_of(v))
}

/// `0xRRGGBB` to a colour.
pub fn rgb_of(v: u32) -> (u8, u8, u8) {
    let [_, r, g, b] = v.to_be_bytes();
    (r, g, b)
}

/// A grid of cells (`true` = has content): the index of a row or a column that is
/// blank from end to end, when the grid is at least 2 × 2 and not entirely blank.
pub fn blank_line(grid: &[Vec<bool>]) -> bool {
    let rows = grid.len();
    let cols = grid.iter().map(Vec::len).max().unwrap_or(0);
    if rows < 2 || cols < 2 || !grid.iter().flatten().any(|c| *c) {
        return false;
    }
    let blank_row = grid.iter().any(|r| r.iter().all(|c| !c));
    let blank_col = (0..cols).any(|c| grid.iter().all(|r| !r.get(c).copied().unwrap_or(false)));
    blank_row || blank_col
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: (u8, u8, u8) = (0, 0, 0);
    const WHITE: (u8, u8, u8) = (255, 255, 255);

    #[test]
    fn wcag_reference_points() {
        assert!((contrast(BLACK, WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast(WHITE, BLACK) - 21.0).abs() < 1e-9);
        assert!((contrast(WHITE, WHITE) - 1.0).abs() < 1e-9);
        // #777777 on white is just under the 4.5:1 minimum.
        let r = contrast((0x77, 0x77, 0x77), WHITE);
        assert!((r - 4.48).abs() < 0.01, "{r}");
        // #767676 is the lightest grey that passes on white.
        assert!(contrast((0x76, 0x76, 0x76), WHITE) >= 4.5);
        assert!((luminance(WHITE) - 1.0).abs() < 1e-9);
        assert!(luminance(BLACK).abs() < 1e-12);
    }

    #[test]
    fn thresholds() {
        let grey = (0x77, 0x77, 0x77);
        assert_eq!(low_contrast(grey, WHITE, 11.0, false), Some(4.48));
        assert_eq!(low_contrast(grey, WHITE, 18.0, false), None);
        assert_eq!(low_contrast(grey, WHITE, 14.0, true), None);
        assert_eq!(low_contrast(BLACK, WHITE, 8.0, false), None);
        assert!(is_large(14.0, true) && !is_large(14.0, false));
    }

    #[test]
    fn hex() {
        assert_eq!(parse_hex("FF336699"), Some((0x33, 0x66, 0x99)));
        assert_eq!(parse_hex("#336699"), Some((0x33, 0x66, 0x99)));
        assert_eq!(parse_hex("xyz"), None);
    }

    #[test]
    fn blank_lines() {
        assert!(blank_line(&[vec![true, false], vec![true, false]]));
        assert!(blank_line(&[vec![true, true], vec![false, false]]));
        assert!(!blank_line(&[vec![true, false], vec![false, true]]));
        assert!(!blank_line(&[vec![true, true]]));
        assert!(!blank_line(&[vec![false, false], vec![false, false]]));
    }
}
