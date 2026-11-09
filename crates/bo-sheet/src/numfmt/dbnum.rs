//! East Asian numerals for the `[DBNum1]`…`[DBNum4]` format-code tags
//! (ECMA-376 Part 1, §18.8.31: bracketed tags in a format code).
//!
//! The tags only name a numeral style; the exact rendering is this
//! implementation's own choice, based on the CJK numeral conventions described
//! in the Unicode Standard (§18.1 "Han", CJK ideographic numerals):
//!
//! | tag      | style                                              | 1234         |
//! |----------|----------------------------------------------------|--------------|
//! | DBNum1   | ideographic numerals with place units (十 百 千 万 亿) | 一千二百三十四 |
//! | DBNum2   | formal ("financial") numerals with units           | 壹仟贰佰叁拾肆 |
//! | DBNum3   | full-width digits, digit by digit                  | １２３４      |
//! | DBNum4   | ideographic digits, digit by digit                 | 一二三四      |
//!
//! Japanese (LCID 0x411) DBNum1 omits the leading 一 before 十/百/千 and writes no
//! 零 for skipped places (千五 for 1005); Traditional Chinese locales (0x404,
//! 0xC04, 0x1404) use traditional formal numerals (貳 參 萬 億) for DBNum2.
//!
//! Digit runs are converted as whole integers for the unit styles; runs after a
//! decimal separator, and date runs of three or more digits (years), are written
//! digit by digit.

/// Numeral variant derived from the section's locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variant {
    Chinese,
    Traditional,
    Japanese,
}

fn variant(lcid: u32) -> Variant {
    match lcid & 0xFFFF {
        0x411 => Variant::Japanese,
        0x404 | 0xC04 | 0x1404 => Variant::Traditional,
        _ => Variant::Chinese,
    }
}

const PLAIN: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
const FORMAL_SC: [char; 10] = ['零', '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖'];
const FORMAL_TC: [char; 10] = ['零', '壹', '貳', '參', '肆', '伍', '陸', '柒', '捌', '玖'];

fn digit(d: u32) -> usize {
    d as usize % 10
}

/// Digit-by-digit conversion.
fn per_digit(digits: &str, style: u8, var: Variant) -> String {
    digits
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| match style {
            2 => match var {
                Variant::Traditional => FORMAL_TC[digit(d)],
                _ => FORMAL_SC[digit(d)],
            },
            3 => char::from_u32(0xFF10 + d).unwrap_or('?'),
            _ => PLAIN[digit(d)],
        })
        .collect()
}

/// An integer written with place units (DBNum1 / DBNum2).
fn with_units(digits: &str, formal: bool, var: Variant) -> String {
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        return if formal { "零" } else { "〇" }.to_owned();
    }
    let nums: Vec<u32> = trimmed.chars().filter_map(|c| c.to_digit(10)).collect();
    let (digits_tab, small, big): (&[char; 10], [&str; 3], [&str; 3]) = match (formal, var) {
        (true, Variant::Traditional) => (&FORMAL_TC, ["拾", "佰", "仟"], ["萬", "億", "兆"]),
        (true, _) => (&FORMAL_SC, ["拾", "佰", "仟"], ["万", "亿", "兆"]),
        (false, Variant::Traditional) => (&PLAIN, ["十", "百", "千"], ["萬", "億", "兆"]),
        (false, _) => (&PLAIN, ["十", "百", "千"], ["万", "亿", "兆"]),
    };
    let japanese = !formal && var == Variant::Japanese;
    let n = nums.len();
    let mut out = String::new();
    let mut zero_pending = false;
    for (i, &d) in nums.iter().enumerate() {
        let pos = n - 1 - i; // power of ten
        let place = pos % 4; // 0 ones, 1 tens, 2 hundreds, 3 thousands
        let big_idx = pos / 4; // 1 = 万, 2 = 亿, 3 = 兆
        if d == 0 {
            zero_pending = true;
        } else {
            // A skipped place inside the number is read as one 零.
            if zero_pending && !japanese && !out.is_empty() {
                out.push('零');
            }
            zero_pending = false;
            let omit_one =
                d == 1 && place > 0 && (japanese || (!formal && place == 1 && out.is_empty()));
            if !omit_one {
                out.push(digits_tab[digit(d)]);
            }
            if place > 0 {
                out.push_str(small[place - 1]);
            }
        }
        if place == 0 && big_idx > 0 {
            let group = &nums[i.saturating_sub(3)..=i];
            if group.iter().any(|&g| g != 0) {
                out.push_str(big[(big_idx - 1).min(2)]);
            }
        }
    }
    out
}

/// Rewrite the ASCII digits of `text` in the `[DBNum<style>]` numeral style.
/// `date` marks date sections (year runs digit by digit); `decimal` and
/// `thousands` are the separators of the formatted text.
pub fn apply(
    text: &str,
    style: u8,
    lcid: u32,
    date: bool,
    decimal: char,
    thousands: char,
) -> String {
    let var = variant(lcid);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() * 3);
    let mut i = 0;
    let mut after_decimal = false;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            // Only a separator directly between digits starts a fraction.
            after_decimal = !date
                && chars[i] == decimal
                && i > 0
                && chars[i - 1].is_ascii_digit()
                && chars.get(i + 1).is_some_and(char::is_ascii_digit);
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let mut run = String::new();
        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                run.push(chars[i]);
                i += 1;
            } else if !date
                && !after_decimal
                && (style == 1 || style == 2)
                && chars[i] == thousands
                && chars
                    .get(i + 1..i + 4)
                    .is_some_and(|s| s.iter().all(char::is_ascii_digit))
            {
                // Grouping separators have no meaning with place units.
                i += 1;
            } else {
                break;
            }
        }
        let per = after_decimal || style >= 3 || (date && run.len() >= 3);
        if per {
            out.push_str(&per_digit(&run, style, var));
        } else {
            out.push_str(&with_units(&run, style == 2, var));
        }
        after_decimal = false;
    }
    out
}
