//! Locale data used by the number-format engine.

/// Separators and names used when formatting values.
///
/// Separators always come from the locale passed by the caller (the user's
/// regional settings); a locale tag inside a format code (`[$-409]`) only
/// switches month/day names and AM/PM designators, as described for
/// `numFmt` locale tags in ECMA-376 Part 1, §18.8.31.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locale {
    /// Windows LCID (e.g. `0x42A` vi-VN, `0x409` en-US).
    pub lcid: u32,
    pub decimal: char,
    pub thousands: char,
    /// Full month names (`mmmm`).
    pub months: [&'static str; 12],
    /// Abbreviated month names (`mmm`).
    pub months_short: [&'static str; 12],
    /// Full day names, Sunday first (`dddd`).
    pub days: [&'static str; 7],
    /// Abbreviated day names, Sunday first (`ddd`).
    pub days_short: [&'static str; 7],
    pub am: &'static str,
    pub pm: &'static str,
    /// Format code used for built-in id 14 (short date).
    pub short_date: &'static str,
    /// Format code used for `[$-F800]` (system long date).
    pub long_date: &'static str,
    /// Format code used for `[$-F400]` (system long time).
    pub long_time: &'static str,
    /// Currency symbol used by built-in currency formats 5–8.
    pub currency: &'static str,
    /// Currency symbol placed after the number (vi-VN) or before (en-US).
    pub currency_after: bool,
    pub true_text: &'static str,
    pub false_text: &'static str,
}

impl Locale {
    /// Vietnamese (Vietnam): `.` thousands, `,` decimal, `dd/mm/yyyy`, `₫`.
    pub const VI_VN: Locale = Locale {
        lcid: 0x42A,
        decimal: ',',
        thousands: '.',
        months: [
            "Tháng Một",
            "Tháng Hai",
            "Tháng Ba",
            "Tháng Tư",
            "Tháng Năm",
            "Tháng Sáu",
            "Tháng Bảy",
            "Tháng Tám",
            "Tháng Chín",
            "Tháng Mười",
            "Tháng Mười Một",
            "Tháng Mười Hai",
        ],
        months_short: [
            "Thg 1", "Thg 2", "Thg 3", "Thg 4", "Thg 5", "Thg 6", "Thg 7", "Thg 8", "Thg 9",
            "Thg 10", "Thg 11", "Thg 12",
        ],
        days: [
            "Chủ Nhật",
            "Thứ Hai",
            "Thứ Ba",
            "Thứ Tư",
            "Thứ Năm",
            "Thứ Sáu",
            "Thứ Bảy",
        ],
        days_short: ["CN", "T2", "T3", "T4", "T5", "T6", "T7"],
        am: "SA",
        pm: "CH",
        short_date: "dd/mm/yyyy",
        long_date: "dddd, dd mmmm yyyy",
        long_time: "hh:mm:ss",
        currency: "₫",
        currency_after: true,
        true_text: "TRUE",
        false_text: "FALSE",
    };

    /// English (United States): `,` thousands, `.` decimal, `m/d/yyyy`, `$`.
    pub const EN_US: Locale = Locale {
        lcid: 0x409,
        decimal: '.',
        thousands: ',',
        months: [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ],
        months_short: [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ],
        days: [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ],
        days_short: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        am: "AM",
        pm: "PM",
        short_date: "m/d/yyyy",
        long_date: "dddd, mmmm d, yyyy",
        long_time: "h:mm:ss AM/PM",
        currency: "$",
        currency_after: false,
        true_text: "TRUE",
        false_text: "FALSE",
    };

    /// Locale for a Windows LCID, if known. Only the language part is compared.
    pub fn from_lcid(lcid: u32) -> Option<Locale> {
        match lcid & 0xFFFF {
            0x42A => Some(Locale::VI_VN),
            0x409 | 0x809 | 0x0C09 | 0x1009 => Some(Locale::EN_US),
            _ => None,
        }
    }
}

impl Default for Locale {
    /// BlueOffice defaults to Vietnamese (product brief: vi-VN first).
    fn default() -> Self {
        Locale::VI_VN
    }
}
