//! Non-Gregorian calendars selected by a number-format locale tag.
//!
//! A locale tag `[$-CCLLLL]` (ECMA-376 Part 1, §18.8.31 `numFmt` format codes)
//! carries the Windows LCID in its low 16 bits and a calendar identifier in
//! bits 16–23. The calendar identifiers are the publicly documented Win32
//! `CALID` constants ("Calendar Identifiers": 1 Gregorian, 6 Hijri, 7 Thai
//! Buddhist, 14/15/17–21 East Asian lunisolar calendars, 23 Um al-Qura).
//! The `B1` / `B2` prefix of a date code selects Gregorian / Hijri explicitly.
//!
//! * Lunisolar (Vietnamese / Chinese) calendar: the astronomical method
//!   published by Hồ Ngọc Đức ("Thuật toán tính âm lịch", 2004), which follows
//!   the new-moon and solar-longitude series of J. Meeus, *Astronomical
//!   Algorithms* (1998). The calendar rules: a month starts on the day (in the
//!   calendar's time zone) of a new moon; month 11 contains the winter solstice;
//!   in a 13-month year the first month without a major solar term is the leap
//!   month. Re-implemented here from that published description. Vietnam uses
//!   UTC+7, China UTC+8, Korea / Japan UTC+9 — which is why e.g. Tết 1985 falls
//!   on 1985-01-21 in Vietnam but on 1985-02-20 in China.
//! * Hijri: the arithmetic (tabular) Islamic calendar of E. M. Reingold &
//!   N. Dershowitz, *Calendrical Calculations* (civil epoch 16 July 622 Julian,
//!   leap years 2, 5, 7, 10, 13, 16, 18, 21, 24, 26, 29 of each 30-year cycle).
//! * Thai Buddhist: Gregorian months and days, year + 543.

use std::f64::consts::PI;

/// Julian Day Number of 1970-01-01 (the epoch of [`super::days_from_civil`]).
const JDN_UNIX_EPOCH: i64 = 2_440_588;

/// Calendar used to display the date parts of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Calendar {
    /// Proleptic Gregorian (default).
    #[default]
    Gregorian,
    /// Thai Buddhist era (CALID 7).
    Thai,
    /// Arithmetic Hijri (CALID 6 / 23, `B2`).
    Hijri,
    /// East Asian lunisolar calendar computed for a time zone (hours east of UTC).
    Lunar {
        /// Time zone of the calendar's meridian.
        tz: i8,
    },
}

impl Calendar {
    /// Calendar selected by a locale tag value (`[$-CCLLLL]`); a tag without a
    /// language part uses `default_lcid` (the user's locale).
    pub fn from_tag(id: u32, default_lcid: u32) -> Calendar {
        let cal = (id >> 16) & 0xFF;
        let lang = match id & 0xFFFF {
            0 => default_lcid & 0xFFFF,
            l => l,
        };
        match cal {
            6 | 23 => Calendar::Hijri,
            7 => Calendar::Thai,
            // Japanese lunar (14), Korean lunar (20), lunar ETO Korean (18): UTC+9.
            14 | 18 | 20 => Calendar::Lunar { tz: 9 },
            // Chinese lunar (15), lunar ETO (17, 19), Taiwan lunar (21): the
            // Vietnamese locale (0x42A) uses its own meridian (UTC+7).
            15 | 17 | 19 | 21 => Calendar::Lunar {
                tz: if lang == 0x42A { 7 } else { 8 },
            },
            _ => Calendar::Gregorian,
        }
    }

    /// Convert a Gregorian date to this calendar's (year, month, day, leap month).
    pub fn convert(self, y: i32, m: u32, d: u32) -> CalDate {
        match self {
            Calendar::Gregorian => CalDate::plain(y, m, d),
            Calendar::Thai => CalDate::plain(y + 543, m, d),
            Calendar::Hijri => hijri_from_jdn(jdn(y, m, d)),
            Calendar::Lunar { tz } => lunar_from_jdn(jdn(y, m, d), f64::from(tz)),
        }
    }
}

/// A date in some calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalDate {
    /// Year in the calendar's era.
    pub year: i32,
    /// Month, 1-based.
    pub month: u32,
    /// Day of month, 1-based.
    pub day: u32,
    /// Lunisolar leap (intercalary) month.
    pub leap: bool,
}

impl CalDate {
    fn plain(year: i32, month: u32, day: u32) -> CalDate {
        CalDate {
            year,
            month,
            day,
            leap: false,
        }
    }
}

/// Julian Day Number of a proleptic Gregorian date.
pub fn jdn(y: i32, m: u32, d: u32) -> i64 {
    super::days_from_civil(y, m, d) + JDN_UNIX_EPOCH
}

// ---------------------------------------------------------------------------
// Lunisolar calendar (Hồ Ngọc Đức's description of Meeus' series)
// ---------------------------------------------------------------------------

/// Julian day (TT, fractional) of the `k`-th new moon after 1900-01-01
/// (Meeus, ch. 49, abridged series as given in the published algorithm).
fn new_moon(k: f64) -> f64 {
    let t = k / 1236.85;
    let t2 = t * t;
    let t3 = t2 * t;
    let dr = PI / 180.0;
    let mut jd1 = 2_415_020.759_33 + 29.530_588_68 * k + 0.000_117_8 * t2 - 0.000_000_155 * t3;
    jd1 += 0.000_33 * ((166.56 + 132.87 * t - 0.009_173 * t2) * dr).sin();
    // Mean anomalies of sun and moon, moon's argument of latitude.
    let m = 359.2242 + 29.105_356_08 * k - 0.000_033_3 * t2 - 0.000_003_47 * t3;
    let mpr = 306.0253 + 385.816_918_06 * k + 0.010_730_6 * t2 + 0.000_012_36 * t3;
    let f = 21.2964 + 390.670_506_46 * k - 0.001_652_8 * t2 - 0.000_002_39 * t3;
    let mut c1 = (0.1734 - 0.000_393 * t) * (m * dr).sin() + 0.0021 * (2.0 * dr * m).sin();
    c1 = c1 - 0.4068 * (mpr * dr).sin() + 0.0161 * (dr * 2.0 * mpr).sin();
    c1 -= 0.0004 * (dr * 3.0 * mpr).sin();
    c1 = c1 + 0.0104 * (dr * 2.0 * f).sin() - 0.0051 * (dr * (m + mpr)).sin();
    c1 = c1 - 0.0074 * (dr * (m - mpr)).sin() + 0.0004 * (dr * (2.0 * f + m)).sin();
    c1 = c1 - 0.0004 * (dr * (2.0 * f - m)).sin() - 0.0006 * (dr * (2.0 * f + mpr)).sin();
    c1 = c1 + 0.0010 * (dr * (2.0 * f - mpr)).sin() + 0.0005 * (dr * (2.0 * mpr + m)).sin();
    // ΔT (TT − UT) in days.
    let delta_t = if t < -11.0 {
        0.001 + 0.000_839 * t + 0.000_226_1 * t2 - 0.000_008_45 * t3 - 0.000_000_081 * t * t3
    } else {
        -0.000_278 + 0.000_265 * t + 0.000_262 * t2
    };
    jd1 + c1 - delta_t
}

/// Apparent solar longitude (radians, 0..2π) at Julian day `jd` (Meeus, ch. 25, low accuracy).
fn sun_longitude(jd: f64) -> f64 {
    let t = (jd - 2_451_545.0) / 36_525.0;
    let t2 = t * t;
    let dr = PI / 180.0;
    let m = 357.529_10 + 35_999.050_30 * t - 0.000_155_9 * t2 - 0.000_000_48 * t * t2;
    let l0 = 280.466_45 + 36_000.769_83 * t + 0.000_303_2 * t2;
    let mut dl = (1.914_600 - 0.004_817 * t - 0.000_014 * t2) * (dr * m).sin();
    dl += (0.019_993 - 0.000_101 * t) * (dr * 2.0 * m).sin() + 0.000_290 * (dr * 3.0 * m).sin();
    let l = (l0 + dl) * dr;
    l - 2.0 * PI * (l / (2.0 * PI)).floor()
}

/// Solar term index (0..11, 30° sectors) at the start of local day `day`.
fn sun_sector(day: i64, tz: f64) -> i64 {
    (sun_longitude(day as f64 - 0.5 - tz / 24.0) / PI * 6.0).floor() as i64
}

/// Local day (JDN) of the `k`-th new moon.
fn new_moon_day(k: i64, tz: f64) -> i64 {
    (new_moon(k as f64) + 0.5 + tz / 24.0).floor() as i64
}

/// First day of the lunar month 11 (the month containing the winter solstice) of year `y`.
fn month11(y: i32, tz: f64) -> i64 {
    let off = jdn(y, 12, 31) - 2_415_021;
    let k = (off as f64 / 29.530_588_853).floor() as i64;
    let nm = new_moon_day(k, tz);
    // Solar longitude ≥ 270° at the new moon → that month is already month 12.
    if sun_sector(nm, tz) >= 9 {
        new_moon_day(k - 1, tz)
    } else {
        nm
    }
}

/// Index (after month 11 starting at `a11`) of the leap month of a 13-month year:
/// the first month in which the solar sector does not change.
fn leap_month_offset(a11: i64, tz: f64) -> i64 {
    let k = ((a11 as f64 - 2_415_021.076_998_695) / 29.530_588_853 + 0.5).floor() as i64;
    let mut i = 1;
    let mut arc = sun_sector(new_moon_day(k + i, tz), tz);
    loop {
        let last = arc;
        i += 1;
        arc = sun_sector(new_moon_day(k + i, tz), tz);
        if arc == last || i >= 14 {
            break;
        }
    }
    i - 1
}

/// Lunisolar date of local day `day` (JDN) in time zone `tz`.
fn lunar_from_jdn(day: i64, tz: f64) -> CalDate {
    let k = ((day as f64 - 2_415_021.076_998_695) / 29.530_588_853).floor() as i64;
    let mut month_start = new_moon_day(k + 1, tz);
    if month_start > day {
        month_start = new_moon_day(k, tz);
    }
    let (gy, _, _) = super::civil_from_days(day - JDN_UNIX_EPOCH);
    let mut a11 = month11(gy, tz);
    let mut b11 = a11;
    let mut year;
    if a11 >= month_start {
        year = gy;
        a11 = month11(gy - 1, tz);
    } else {
        year = gy + 1;
        b11 = month11(gy + 1, tz);
    }
    let lday = (day - month_start + 1) as u32;
    let diff = (month_start - a11) / 29;
    let mut leap = false;
    let mut month = diff + 11;
    if b11 - a11 > 365 {
        let leap_diff = leap_month_offset(a11, tz);
        if diff >= leap_diff {
            month = diff + 10;
            leap = diff == leap_diff;
        }
    }
    if month > 12 {
        month -= 12;
    }
    if month >= 11 && diff < 4 {
        year -= 1;
    }
    CalDate {
        year,
        month: month as u32,
        day: lday,
        leap,
    }
}

/// Sexagenary (can chi / 干支) name of a lunisolar year.
pub fn sexagenary_year(year: i32, vietnamese: bool) -> String {
    const CAN: [&str; 10] = [
        "Giáp", "Ất", "Bính", "Đinh", "Mậu", "Kỷ", "Canh", "Tân", "Nhâm", "Quý",
    ];
    const CHI: [&str; 12] = [
        "Tý", "Sửu", "Dần", "Mão", "Thìn", "Tỵ", "Ngọ", "Mùi", "Thân", "Dậu", "Tuất", "Hợi",
    ];
    const STEMS: [&str; 10] = ["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸"];
    const BRANCHES: [&str; 12] = [
        "子", "丑", "寅", "卯", "辰", "巳", "午", "未", "申", "酉", "戌", "亥",
    ];
    // Year 4 CE was 甲子 (Giáp Tý): stem = (y + 6) mod 10, branch = (y + 8) mod 12.
    let s = (year + 6).rem_euclid(10) as usize;
    let b = (year + 8).rem_euclid(12) as usize;
    if vietnamese {
        format!("{} {}", CAN[s], CHI[b])
    } else {
        format!("{}{}", STEMS[s], BRANCHES[b])
    }
}

/// Vietnamese name of a lunar month (`mmmm` in a lunar section).
pub fn lunar_month_name_vi(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Giêng", "Hai", "Ba", "Tư", "Năm", "Sáu", "Bảy", "Tám", "Chín", "Mười", "Một", "Chạp",
    ];
    NAMES[(month.clamp(1, 12) - 1) as usize]
}

// ---------------------------------------------------------------------------
// Arithmetic Hijri calendar (Calendrical Calculations)
// ---------------------------------------------------------------------------

/// JDN of 1 Muharram 1 AH (civil epoch, 16 July 622 Julian).
const HIJRI_EPOCH: i64 = 1_948_440;

/// JDN of a Hijri date.
fn jdn_from_hijri(y: i64, m: i64, d: i64) -> i64 {
    // ceil(29.5 × (m − 1)) = (59 × (m − 1) + 1) / 2.
    d + (59 * (m - 1) + 1) / 2 + (y - 1) * 354 + (3 + 11 * y).div_euclid(30) + HIJRI_EPOCH - 1
}

fn hijri_from_jdn(day: i64) -> CalDate {
    let y = (30 * (day - HIJRI_EPOCH) + 10_646).div_euclid(10_631);
    let prior = day - jdn_from_hijri(y, 1, 1);
    // month = ⌈(prior − 29) / 29.5⌉ + 1 = ⌊2·prior / 59⌋ + 1 (the leap day 355
    // of a 30-year-cycle leap year stays in month 12).
    let m = ((2 * prior).div_euclid(59) + 1).clamp(1, 12);
    let d = day - jdn_from_hijri(y, m, 1) + 1;
    CalDate::plain(y as i32, m as u32, d as u32)
}

/// Transliterated Hijri month names (`mmmm` in a Hijri section).
pub fn hijri_month_name(month: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Muharram",
        "Safar",
        "Rabi' al-awwal",
        "Rabi' al-thani",
        "Jumada al-awwal",
        "Jumada al-thani",
        "Rajab",
        "Sha'ban",
        "Ramadan",
        "Shawwal",
        "Dhu al-Qi'dah",
        "Dhu al-Hijjah",
    ];
    NAMES[(month.clamp(1, 12) - 1) as usize]
}
