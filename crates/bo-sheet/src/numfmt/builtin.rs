//! Built-in number formats (ECMA-376 Part 1, §18.8.30 `numFmt`, table of
//! "predefined" format ids 0–49).

use super::Locale;

/// Format code of built-in number format `id` for `locale`, or `None` if the id
/// is not a built-in (custom ids start at 164).
///
/// Ids 5–8 (currency) and 14 (short date) are locale dependent; ids 27–36 and
/// 50–81 are reserved for East Asian locales and fall back to the short date.
pub fn builtin_format(id: u32, locale: &Locale) -> Option<String> {
    let s: &str = match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5..=8 => return Some(currency_format(id, locale)),
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 | 27..=36 | 50..=81 => locale.short_date,
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => return Some(format!("{} h:mm", locale.short_date)),
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        41 => r#"_(* #,##0_);_(* \(#,##0\);_(* "-"_);_(@_)"#,
        42 => return Some(accounting(locale, false)),
        43 => r#"_(* #,##0.00_);_(* \(#,##0.00\);_(* "-"??_);_(@_)"#,
        44 => return Some(accounting(locale, true)),
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mmss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    };
    Some(s.to_owned())
}

fn quoted(sym: &str) -> String {
    format!("\"{sym}\"")
}

fn currency_format(id: u32, l: &Locale) -> String {
    let c = quoted(l.currency);
    let num = if id >= 7 { "#,##0.00" } else { "#,##0" };
    let red = if id == 6 || id == 8 { "[Red]" } else { "" };
    if l.currency_after {
        format!("{num} {c};{red}-{num} {c}")
    } else {
        format!("{c}{num}_);{red}({c}{num})")
    }
}

fn accounting(l: &Locale, decimals: bool) -> String {
    let c = quoted(l.currency);
    let (num, zero) = if decimals {
        ("#,##0.00", "\"-\"??")
    } else {
        ("#,##0", "\"-\"")
    };
    if l.currency_after {
        format!("_-* {num} {c}_-;-* {num} {c}_-;_-* {zero} {c}_-;_-@_-")
    } else {
        format!("_({c}* {num}_);_({c}* \\({num}\\);_({c}* {zero}_);_(@_)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: ECMA-376 Part 1, §18.8.30 numFmt — predefined format ids
    #[test]
    fn predefined_ids() {
        let l = Locale::EN_US;
        let table: &[(u32, &str)] = &[
            (0, "General"),
            (1, "0"),
            (2, "0.00"),
            (3, "#,##0"),
            (4, "#,##0.00"),
            (9, "0%"),
            (10, "0.00%"),
            (11, "0.00E+00"),
            (12, "# ?/?"),
            (13, "# ??/??"),
            (14, "m/d/yyyy"),
            (15, "d-mmm-yy"),
            (16, "d-mmm"),
            (17, "mmm-yy"),
            (18, "h:mm AM/PM"),
            (19, "h:mm:ss AM/PM"),
            (20, "h:mm"),
            (21, "h:mm:ss"),
            (22, "m/d/yyyy h:mm"),
            (37, "#,##0 ;(#,##0)"),
            (38, "#,##0 ;[Red](#,##0)"),
            (39, "#,##0.00;(#,##0.00)"),
            (40, "#,##0.00;[Red](#,##0.00)"),
            (45, "mm:ss"),
            (46, "[h]:mm:ss"),
            (47, "mmss.0"),
            (48, "##0.0E+0"),
            (49, "@"),
        ];
        for (id, code) in table {
            assert_eq!(builtin_format(*id, &l).as_deref(), Some(*code), "id {id}");
        }
        assert_eq!(
            builtin_format(14, &Locale::VI_VN).as_deref(),
            Some("dd/mm/yyyy")
        );
        assert_eq!(
            builtin_format(5, &Locale::VI_VN).as_deref(),
            Some("#,##0 \"₫\";-#,##0 \"₫\"")
        );
        assert!(builtin_format(164, &l).is_none());
        assert!(builtin_format(23, &l).is_none());
    }
}
