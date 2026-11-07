//! Serial date-time conversion (ECMA-376 Part 1, §18.17.4 "Dates and Times").
//!
//! * 1900 date system: serial 1 = 1900-01-01. For compatibility the system
//!   includes the non-existent date 1900-02-29 as serial 60 (§18.17.4.1), so
//!   serial 61 = 1900-03-01. Serial 0 is displayed as "1900-01-00".
//! * 1904 date system: serial 0 = 1904-01-01 (§18.17.4.1).
//! * The fractional part is the time of day.

/// Largest serial accepted (9999-12-31 in the 1900 system, §18.17.4.1).
pub const MAX_SERIAL_1900: f64 = 2_958_465.999_999;
/// Largest serial accepted in the 1904 system (9999-12-31).
pub const MAX_SERIAL_1904: f64 = 2_957_003.999_999;

/// A broken-down date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    /// 1–12.
    pub month: u32,
    /// 0–31 (0 only for serial 0 in the 1900 system).
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    /// Fraction of a second in units of `10^-subsec_digits` (see [`serial_to_datetime`]).
    pub subsec: u32,
    /// Day of week, 0 = Sunday.
    pub weekday: u32,
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((y + i64::from(m <= 2)) as i32, m, d)
}

/// Days since 1970-01-01 of 1899-12-30 (serial 0 of the 1900 system after the leap-bug).
const EPOCH_1900: i64 = -25_569;
/// Days since 1970-01-01 of 1904-01-01.
const EPOCH_1904: i64 = -24_107;

/// Convert a serial value to a date-time, rounding the time to `subsec_digits`
/// decimal places of a second (0 = whole seconds). Returns `None` for negative
/// or out-of-range serials (displayed as `#####`).
pub fn serial_to_datetime(serial: f64, date1904: bool, subsec_digits: u32) -> Option<DateTime> {
    let max = if date1904 {
        MAX_SERIAL_1904
    } else {
        MAX_SERIAL_1900
    };
    if !serial.is_finite() || serial < 0.0 || serial > max + 1.0 {
        return None;
    }
    let scale = 10u64.pow(subsec_digits.min(3));
    let units_per_day = 86_400 * scale;
    let total = (serial * units_per_day as f64).round() as u64;
    let day = (total / units_per_day) as i64;
    let rem = total % units_per_day;
    let secs = rem / scale;
    let subsec = (rem % scale) as u32;
    let (year, month, d, weekday) = if date1904 {
        let days = EPOCH_1904 + day;
        let (y, m, d) = civil_from_days(days);
        (y, m, d, (days + 4).rem_euclid(7) as u32)
    } else {
        // Serials below 61 count from 1900-01-00 and include the fictitious 1900-02-29.
        let weekday = ((day + 6) % 7) as u32;
        match day {
            0 => (1900, 1, 0, weekday),
            1..=59 => {
                let (y, m, d) = civil_from_days(days_from_civil(1900, 1, 1) + day - 1);
                (y, m, d, weekday)
            }
            60 => (1900, 2, 29, weekday),
            _ => {
                let (y, m, d) = civil_from_days(EPOCH_1900 + day);
                (y, m, d, weekday)
            }
        }
    };
    if year > 9999 {
        return None;
    }
    Some(DateTime {
        year,
        month,
        day: d,
        hour: (secs / 3600) as u32,
        minute: ((secs / 60) % 60) as u32,
        second: (secs % 60) as u32,
        subsec,
        weekday,
    })
}

/// Convert a date and time to a serial value. `None` before the epoch.
pub fn datetime_to_serial(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: f64,
    date1904: bool,
) -> Option<f64> {
    let time = (f64::from(hour) * 3600.0 + f64::from(minute) * 60.0 + second) / 86_400.0;
    let days = days_from_civil(year, month, day);
    let serial = if date1904 {
        days - EPOCH_1904
    } else {
        let s = days - EPOCH_1900;
        // Dates before 1900-03-01 sit one day lower (no fictitious leap day yet).
        if s < 61 {
            s - 1
        } else {
            s
        }
    };
    (serial >= 0).then_some(serial as f64 + time)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(serial: f64, d1904: bool) -> (i32, u32, u32) {
        let d = serial_to_datetime(serial, d1904, 0).unwrap();
        (d.year, d.month, d.day)
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (date systems, 1900 leap-year rule)
    #[test]
    fn system_1900() {
        assert_eq!(ymd(0.0, false), (1900, 1, 0));
        assert_eq!(ymd(1.0, false), (1900, 1, 1));
        assert_eq!(ymd(59.0, false), (1900, 2, 28));
        assert_eq!(ymd(60.0, false), (1900, 2, 29));
        assert_eq!(ymd(61.0, false), (1900, 3, 1));
        assert_eq!(ymd(36526.0, false), (2000, 1, 1));
        assert_eq!(ymd(45351.0, false), (2024, 2, 29));
        assert_eq!(ymd(2_958_465.0, false), (9999, 12, 31));
        assert!(serial_to_datetime(-1.0, false, 0).is_none());
        assert!(serial_to_datetime(2_958_467.0, false, 0).is_none());
        // Weekday: serial 1 is reported as a Sunday, serial 61 (1900-03-01) a Thursday.
        assert_eq!(serial_to_datetime(1.0, false, 0).unwrap().weekday, 0);
        assert_eq!(serial_to_datetime(61.0, false, 0).unwrap().weekday, 4);
        assert_eq!(serial_to_datetime(45351.0, false, 0).unwrap().weekday, 4);
    }

    // source: ECMA-376 Part 1, §18.17.4.1 (1904 date system base 1904-01-01)
    #[test]
    fn system_1904() {
        assert_eq!(ymd(0.0, true), (1904, 1, 1));
        assert_eq!(ymd(1462.0, true), (1908, 1, 2));
        assert_eq!(serial_to_datetime(0.0, true, 0).unwrap().weekday, 5);
        assert_eq!(datetime_to_serial(1904, 1, 1, 0, 0, 0.0, true), Some(0.0));
    }

    #[test]
    fn time_of_day_and_rounding() {
        let d = serial_to_datetime(0.5, false, 0).unwrap();
        assert_eq!((d.hour, d.minute, d.second), (12, 0, 0));
        // 23:59:59.6 rounds to the next day at whole-second precision.
        let s = 1.0 + (86_399.6 / 86_400.0);
        let d = serial_to_datetime(s, false, 0).unwrap();
        assert_eq!((d.day, d.hour, d.minute, d.second), (2, 0, 0, 0));
        let d = serial_to_datetime(s, false, 1).unwrap();
        assert_eq!((d.day, d.hour, d.second, d.subsec), (1, 23, 59, 6));
    }

    #[test]
    fn serial_roundtrip() {
        for s in [1.0, 59.0, 60.0, 61.0, 100.0, 36526.0, 45351.0, 2_958_465.0] {
            if s == 60.0 {
                continue; // 1900-02-29 does not exist in the proleptic calendar.
            }
            let d = serial_to_datetime(s, false, 0).unwrap();
            assert_eq!(
                datetime_to_serial(d.year, d.month, d.day, 0, 0, 0.0, false),
                Some(s),
                "{s}"
            );
        }
        assert_eq!(
            datetime_to_serial(2024, 2, 29, 18, 0, 0.0, false),
            Some(45351.75)
        );
    }
}
