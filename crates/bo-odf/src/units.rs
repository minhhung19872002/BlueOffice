//! Length values (ODF 1.3 Part 3, §18.3.18 length: number + unit).

use bo_model::Twips;

/// Parse a length to twips. Supports `cm`, `mm`, `in`, `pt`, `pc`, `px`
/// (96 dpi). Returns `None` for percentages or junk.
pub fn parse_length(s: &str) -> Option<Twips> {
    let s = s.trim();
    let split = s
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let v: f64 = num.trim().parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    let pt = match unit.trim() {
        "pt" => v,
        "cm" => v * 72.0 / 2.54,
        "mm" => v * 72.0 / 25.4,
        "in" | "inch" => v * 72.0,
        "pc" => v * 12.0,
        "px" => v * 0.75,
        _ => return None,
    };
    let tw = (pt * 20.0).round();
    if tw.abs() > f64::from(i32::MAX) {
        return None;
    }
    Some(Twips(tw as i32))
}

/// Parse a percentage like `150%`.
pub fn parse_percent(s: &str) -> Option<f64> {
    let v: f64 = s.trim().strip_suffix('%')?.trim().parse().ok()?;
    v.is_finite().then_some(v)
}

/// Format a number with at most `decimals` decimals, trailing zeros trimmed.
pub fn fmt_num(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// Twips as a `pt` length (exact: one twip is 0.05 pt).
pub fn fmt_pt(t: Twips) -> String {
    format!("{}pt", fmt_num(f64::from(t.0) / 20.0, 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(parse_length("1in"), Some(Twips(1440)));
        assert_eq!(parse_length("2.54cm"), Some(Twips(1440)));
        assert_eq!(parse_length("12pt"), Some(Twips(240)));
        assert_eq!(parse_length("-0.5in"), Some(Twips(-720)));
        assert_eq!(parse_length("10%"), None);
        assert_eq!(parse_length("abc"), None);
        assert_eq!(parse_length("1e400cm"), None);
        assert_eq!(fmt_pt(Twips(1134)), "56.7pt");
        assert_eq!(fmt_pt(Twips(-15)), "-0.75pt");
        assert_eq!(parse_length(&fmt_pt(Twips(1701))), Some(Twips(1701)));
        assert_eq!(parse_percent("115%"), Some(115.0));
    }
}
