//! Serial date/time arithmetic.
//!
//! ECMA-376 Part 1, §18.17.4 (dates and times): a date is a serial number of days
//! since a base date, the fraction is the time of day.
//! * 1900 date system: serial 1 = 1900-01-01; serial 60 is the fictitious
//!   1900-02-29 kept for compatibility, so serials ≥ 61 are offset by one day.
//!   Serial 0 is "1900-01-00".
//! * 1904 date system: serial 0 = 1904-01-01.
//!
//! Calendar conversion uses the proleptic Gregorian days-from-civil algorithm.

/// Which base date serial numbers count from (ECMA-376 Part 1, §18.2.28 `date1904`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DateSystem {
    /// Base 1899-12-31 with the fictitious 1900-02-29.
    #[default]
    Date1900,
    /// Base 1904-01-01.
    Date1904,
}

/// Largest serial (9999-12-31) in the 1900 system.
pub const MAX_SERIAL_1900: f64 = 2_958_465.0;
/// Largest serial (9999-12-31) in the 1904 system.
pub const MAX_SERIAL_1904: f64 = 2_957_003.0;
/// Difference between the systems: serial1900 = serial1904 + 1462.
pub const OFFSET_1904: f64 = 1462.0;

impl DateSystem {
    /// Largest valid serial.
    pub fn max_serial(self) -> f64 {
        match self {
            DateSystem::Date1900 => MAX_SERIAL_1900,
            DateSystem::Date1904 => MAX_SERIAL_1904,
        }
    }

    /// Convert a serial of this system to the 1900 system (for weekday arithmetic).
    pub fn to_1900(self, serial: f64) -> f64 {
        match self {
            DateSystem::Date1900 => serial,
            DateSystem::Date1904 => serial + OFFSET_1904,
        }
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (month 1..=12).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Whether `y` is a Gregorian leap year.
pub fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Number of days in a month (Gregorian).
pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
    }
}

const BASE_1899_12_31: i64 = -25_568;
const BASE_1904_01_01: i64 = -24_107;

/// Linear (unchecked) serial of a valid calendar date.
fn serial_linear(y: i64, m: i64, d: i64, system: DateSystem) -> i64 {
    let days = days_from_civil(y, m, d);
    match system {
        DateSystem::Date1900 => {
            let n = days - BASE_1899_12_31;
            // Dates from 1900-03-01 on are shifted by the fictitious 1900-02-29.
            if n >= 60 {
                n + 1
            } else {
                n
            }
        }
        DateSystem::Date1904 => days - BASE_1904_01_01,
    }
}

/// Serial number of a calendar date; `None` if the date is invalid or out of range.
/// In the 1900 system, 1900-02-29 maps to serial 60.
pub fn ymd_to_serial(y: i64, m: u32, d: u32, system: DateSystem) -> Option<f64> {
    if system == DateSystem::Date1900 && (y, m, d) == (1900, 2, 29) {
        return Some(60.0);
    }
    if !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    let s = serial_linear(y, i64::from(m), i64::from(d), system);
    (s >= 0 && (s as f64) <= system.max_serial()).then_some(s as f64)
}

/// The `DATE(year, month, day)` construction with normalization of overflowing
/// months/days (ECMA-376 Part 1, §18.17.7 DATE). Years 0..=1899 are offset by 1900.
pub fn date_serial(year: i64, month: i64, day: i64, system: DateSystem) -> Option<f64> {
    if !(0..10_000).contains(&year) {
        return None;
    }
    let y = if year < 1900 { year + 1900 } else { year };
    let total = y * 12 + (month - 1);
    let ny = total.div_euclid(12);
    let nm = total.rem_euclid(12) + 1;
    let first = serial_linear(ny, nm, 1, system);
    let s = first + day - 1;
    (s >= 0 && (s as f64) <= system.max_serial()).then_some(s as f64)
}

/// Calendar date of a serial (fraction ignored). Serial 0 in the 1900 system is
/// `(1900, 1, 0)`; serial 60 is `(1900, 2, 29)`.
pub fn serial_to_ymd(serial: f64, system: DateSystem) -> Option<(i64, u32, u32)> {
    if !serial.is_finite() || serial < 0.0 || serial >= system.max_serial() + 1.0 {
        return None;
    }
    let s = serial.floor() as i64;
    match system {
        DateSystem::Date1900 => {
            if s == 0 {
                return Some((1900, 1, 0));
            }
            if s == 60 {
                return Some((1900, 2, 29));
            }
            let days = if s < 60 { s } else { s - 1 };
            Some(civil_from_days(BASE_1899_12_31 + days))
        }
        DateSystem::Date1904 => Some(civil_from_days(BASE_1904_01_01 + s)),
    }
}

/// Time-of-day fraction for hours, minutes, seconds.
pub fn time_fraction(h: f64, m: f64, s: f64) -> f64 {
    (h * 3600.0 + m * 60.0 + s) / 86_400.0
}

/// Split the time part of a serial into (hour, minute, second), rounded to the
/// nearest second.
pub fn serial_to_hms(serial: f64) -> (u32, u32, u32) {
    let frac = serial - serial.floor();
    let mut secs = (frac * 86_400.0).round() as u32;
    if secs >= 86_400 {
        secs = 0;
    }
    (secs / 3600, (secs / 60) % 60, secs % 60)
}

/// Day of week of a serial with Sunday = 0. In the 1900 system serial 1 is
/// treated as a Sunday (consistent with the fictitious leap day).
pub fn weekday_sun0(serial: f64, system: DateSystem) -> u32 {
    let s = system.to_1900(serial.floor()) as i64;
    ((s - 1).rem_euclid(7)) as u32
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn month_from_name(s: &str) -> Option<u32> {
    let l = s.to_ascii_lowercase();
    if l.len() < 3 {
        return None;
    }
    MONTHS
        .iter()
        .position(|m| m.starts_with(&l))
        .map(|i| i as u32 + 1)
}

/// Two-digit year expansion: 00-29 → 2000s, 30-99 → 1900s.
fn expand_year(y: i64, digits: usize) -> i64 {
    if digits <= 2 {
        if y < 30 {
            2000 + y
        } else {
            1900 + y
        }
    } else {
        y
    }
}

fn parse_time_part(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let (body, ampm) = if let Some(b) = lower.strip_suffix("am") {
        (b.trim_end().to_owned(), Some(false))
    } else if let Some(b) = lower.strip_suffix("pm") {
        (b.trim_end().to_owned(), Some(true))
    } else {
        (lower, None)
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || (parts.len() == 1 && ampm.is_none()) {
        return None;
    }
    let mut h: f64 = parts[0].trim().parse::<u32>().ok()?.into();
    let m: f64 = match parts.get(1) {
        Some(p) => p.trim().parse::<u32>().ok()?.into(),
        None => 0.0,
    };
    let s: f64 = match parts.get(2) {
        Some(p) => p.trim().parse::<f64>().ok()?,
        None => 0.0,
    };
    if m >= 60.0 || !(0.0..60.0).contains(&s) {
        return None;
    }
    if let Some(pm) = ampm {
        if !(1.0..=12.0).contains(&h) && h != 0.0 {
            return None;
        }
        if h == 12.0 {
            h = 0.0;
        }
        if pm {
            h += 12.0;
        }
    }
    // Hours beyond a day are allowed only without AM/PM (e.g. "25:00" = 1.0416..).
    if parts.len() == 1 && ampm.is_none() {
        return None;
    }
    Some(time_fraction(h, m, s))
}

fn num_with_len(s: &str) -> Option<(i64, usize)> {
    let t = s.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((t.parse().ok()?, t.len()))
}

fn parse_date_part(s: &str, system: DateSystem) -> Option<f64> {
    let t = s.trim().trim_end_matches(',');
    // Numeric forms with '-' or '/'.
    for sep in ['-', '/'] {
        let parts: Vec<&str> = t.split(sep).collect();
        if parts.len() == 3 {
            if let (Some((a, la)), Some((b, _)), Some((c, lc))) =
                (num_with_len(parts[0]), num_with_len(parts[1]), num_with_len(parts[2]))
            {
                // yyyy-mm-dd / yyyy/mm/dd
                if la == 4 {
                    return ymd_to_serial(a, u32::try_from(b).ok()?, u32::try_from(c).ok()?, system);
                }
                // m/d/yyyy (en-US default locale)
                let y = expand_year(c, lc);
                return ymd_to_serial(y, u32::try_from(a).ok()?, u32::try_from(b).ok()?, system);
            }
            // d-Mon-yyyy
            if let (Some((d, _)), Some(m), Some((y, ly))) =
                (num_with_len(parts[0]), month_from_name(parts[1]), num_with_len(parts[2]))
            {
                return ymd_to_serial(expand_year(y, ly), m, u32::try_from(d).ok()?, system);
            }
        }
    }
    // "15 January 2020", "January 15, 2020", "Jan 15 2020"
    let words: Vec<&str> = t
        .split(|c: char| c == ' ' || c == ',')
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() == 3 {
        if let (Some((d, _)), Some(m), Some((y, ly))) =
            (num_with_len(words[0]), month_from_name(words[1]), num_with_len(words[2]))
        {
            return ymd_to_serial(expand_year(y, ly), m, u32::try_from(d).ok()?, system);
        }
        if let (Some(m), Some((d, _)), Some((y, ly))) =
            (month_from_name(words[0]), num_with_len(words[1]), num_with_len(words[2]))
        {
            return ymd_to_serial(expand_year(y, ly), m, u32::try_from(d).ok()?, system);
        }
    }
    None
}

/// Parse date and/or time text into a serial (`DATEVALUE`/`TIMEVALUE` and
/// arithmetic coercion). Accepted forms: `yyyy-mm-dd`, `yyyy/mm/dd`,
/// `m/d/yyyy`, `d-Mon-yyyy`, `d Month yyyy`, `Month d, yyyy`, optionally
/// followed by a time `h:mm[:ss][ AM|PM]`, or a time alone.
pub fn parse_datetime_text(s: &str, system: DateSystem) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(time) = parse_time_part(t) {
        return Some(time);
    }
    if let Some(d) = parse_date_part(t, system) {
        return Some(d);
    }
    // Date followed by time: split at 'T' (ISO) or at the last space before a ':' group.
    let split = if let Some(i) = t.find('T').filter(|&i| i == 10) {
        Some((&t[..i], &t[i + 1..]))
    } else {
        t.find(':').and_then(|colon| t[..colon].rfind(' ').map(|sp| (&t[..sp], &t[sp + 1..])))
    };
    let (dp, tp) = split?;
    let d = parse_date_part(dp, system)?;
    let tm = parse_time_part(tp)?;
    Some(d + tm)
}

/// Current local-agnostic (UTC) date-time as a serial number.
pub fn now_serial(system: DateSystem) -> f64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    // 1970-01-01 is serial 25569 in the 1900 system.
    let s1900 = secs / 86_400.0 + 25_569.0;
    match system {
        DateSystem::Date1900 => s1900,
        DateSystem::Date1904 => s1900 - OFFSET_1904,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // source: ECMA-376 Part 1, §18.17.4.1 (1900 date system incl. serial 60 = 1900-02-29)
    #[test]
    fn serials_1900() {
        let s = DateSystem::Date1900;
        assert_eq!(ymd_to_serial(1900, 1, 1, s), Some(1.0));
        assert_eq!(ymd_to_serial(1900, 2, 28, s), Some(59.0));
        assert_eq!(ymd_to_serial(1900, 2, 29, s), Some(60.0));
        assert_eq!(ymd_to_serial(1900, 3, 1, s), Some(61.0));
        assert_eq!(ymd_to_serial(9999, 12, 31, s), Some(MAX_SERIAL_1900));
        assert_eq!(serial_to_ymd(60.0, s), Some((1900, 2, 29)));
        assert_eq!(serial_to_ymd(61.0, s), Some((1900, 3, 1)));
        assert_eq!(serial_to_ymd(0.0, s), Some((1900, 1, 0)));
        assert_eq!(ymd_to_serial(1970, 1, 1, s), Some(25_569.0));
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (1904 date system base 1904-01-01)
    #[test]
    fn serials_1904() {
        let s = DateSystem::Date1904;
        assert_eq!(ymd_to_serial(1904, 1, 1, s), Some(0.0));
        assert_eq!(ymd_to_serial(1904, 1, 2, s), Some(1.0));
        assert_eq!(
            ymd_to_serial(2000, 1, 1, s).map(|x| x + OFFSET_1904),
            ymd_to_serial(2000, 1, 1, DateSystem::Date1900)
        );
        assert_eq!(serial_to_ymd(MAX_SERIAL_1904, s), Some((9999, 12, 31)));
    }

    #[test]
    fn roundtrip_all_days() {
        for sys in [DateSystem::Date1900, DateSystem::Date1904] {
            let mut n = 1.0;
            while n <= sys.max_serial() {
                let (y, m, d) = serial_to_ymd(n, sys).unwrap();
                assert_eq!(ymd_to_serial(y, m, d, sys), Some(n));
                n += 97.0;
            }
        }
    }

    #[test]
    fn text_parsing() {
        let s = DateSystem::Date1900;
        assert_eq!(parse_datetime_text("2020-01-15", s), ymd_to_serial(2020, 1, 15, s));
        assert_eq!(parse_datetime_text("1/15/2020", s), ymd_to_serial(2020, 1, 15, s));
        assert_eq!(parse_datetime_text("15-Jan-2020", s), ymd_to_serial(2020, 1, 15, s));
        assert_eq!(parse_datetime_text("12:00", s), Some(0.5));
        assert_eq!(parse_datetime_text("6:00 PM", s), Some(0.75));
        assert_eq!(
            parse_datetime_text("2020-01-15 06:00", s),
            ymd_to_serial(2020, 1, 15, s).map(|d| d + 0.25)
        );
        assert_eq!(parse_datetime_text("hello", s), None);
    }
}
