//! Simulation time.
//!
//! Time is counted in seconds from the J2000.0 epoch — 2000-01-01T12:00:00 TT,
//! Julian date 2451545.0 — which is the epoch essentially every modern
//! ephemeris is published against.
//!
//! Aphelion does not model the difference between time scales (TT, TAI, UTC) or
//! leap seconds. Over the spans it is used for, that difference is well under a
//! minute and invisible next to the modelling error of the orbits themselves.

use std::fmt;

use crate::constants::{DAY, J2000_JULIAN_DATE, YEAR};

/// An instant, as seconds since J2000.0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Epoch(pub f64);

impl Epoch {
    /// The J2000.0 epoch itself.
    pub const J2000: Epoch = Epoch(0.0);

    /// Seconds since J2000.0.
    #[inline]
    pub fn seconds(self) -> f64 {
        self.0
    }

    /// Days since J2000.0.
    #[inline]
    pub fn days(self) -> f64 {
        self.0 / DAY
    }

    /// Julian years since J2000.0.
    #[inline]
    pub fn years(self) -> f64 {
        self.0 / YEAR
    }

    /// The corresponding Julian date.
    #[inline]
    pub fn julian_date(self) -> f64 {
        J2000_JULIAN_DATE + self.days()
    }

    /// Builds an epoch from a Julian date.
    #[inline]
    pub fn from_julian_date(julian_date: f64) -> Self {
        Epoch((julian_date - J2000_JULIAN_DATE) * DAY)
    }

    /// Builds an epoch from a proleptic Gregorian calendar date and time.
    ///
    /// `month` is 1-based. Fractional seconds are allowed.
    pub fn from_gregorian(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: f64,
    ) -> Self {
        // Subtract the J2000 Julian day number in integer arithmetic, so the
        // 2.45e6-day offset never touches an f64 and the clock time stays exact.
        let days_from_j2000 = gregorian_to_julian_day(year, month, day) - 2_451_545;
        #[allow(clippy::cast_precision_loss)]
        let seconds = days_from_j2000 as f64 * 86_400.0
            + (f64::from(hour) - 12.0) * 3600.0
            + f64::from(minute) * 60.0
            + second;
        Epoch(seconds)
    }

    /// Decomposes into a proleptic Gregorian calendar date and time.
    ///
    /// Returns `(year, month, day, hour, minute, second)` with `month` 1-based.
    pub fn to_gregorian(self) -> (i32, u32, u32, u32, u32, f64) {
        // Split into a whole day count and an offset inside that day *before*
        // going anywhere near a Julian date. A JD is ~2.45e6, which leaves f64
        // only about 40 µs of resolution — enough to turn 20:17:00 into
        // 20:16:59.99999. Counting from J2000 keeps three more digits.
        let day_index = (self.0 / 86_400.0).floor();
        let seconds_since_noon = {
            let raw = self.0 - day_index * 86_400.0;
            // Round to the microsecond so exact clock times stay exact.
            (raw * 1e6).round() / 1e6
        };

        // Civil days start at midnight, i.e. half a day before the J2000-style
        // noon boundary we just cut on.
        let (seconds_of_day, day_carry) = if seconds_since_noon >= 43_200.0 {
            (seconds_since_noon - 43_200.0, 1)
        } else {
            (seconds_since_noon + 43_200.0, 0)
        };

        #[allow(clippy::cast_possible_truncation)]
        let julian_day_number = 2_451_545_i64 + day_index as i64 + day_carry;

        // Fliegel & Van Flandern, run backwards from the Julian day number.
        let mut l = julian_day_number + 68_569;
        let n = 4 * l / 146_097;
        l -= (146_097 * n + 3) / 4;
        let i = 4000 * (l + 1) / 1_461_001;
        l -= 1461 * i / 4 - 31;
        let j = 80 * l / 2447;
        let day = l - 2447 * j / 80;
        l = j / 11;
        let month = j + 2 - 12 * l;
        let year = 100 * (n - 49) + i + l;

        let hour = (seconds_of_day / 3600.0).floor();
        let minute = ((seconds_of_day - hour * 3600.0) / 60.0).floor();
        let second = seconds_of_day - hour * 3600.0 - minute * 60.0;

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (
            year as i32,
            month as u32,
            day as u32,
            hour as u32,
            minute as u32,
            second,
        )
    }

    /// Formats as an ISO-8601-like calendar date, to the second.
    pub fn to_iso8601(self) -> String {
        let (year, month, day, hour, minute, second) = self.to_gregorian();
        // `second` comes out of to_gregorian in [0, 60), so the cast is sound.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let whole_seconds = second.floor() as u32;
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{whole_seconds:02}")
    }
}

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iso8601())
    }
}

impl std::ops::Add<f64> for Epoch {
    type Output = Epoch;
    fn add(self, seconds: f64) -> Epoch {
        Epoch(self.0 + seconds)
    }
}

impl std::ops::AddAssign<f64> for Epoch {
    fn add_assign(&mut self, seconds: f64) {
        self.0 += seconds;
    }
}

impl std::ops::Sub for Epoch {
    type Output = f64;
    /// The interval between two epochs, in seconds.
    fn sub(self, other: Epoch) -> f64 {
        self.0 - other.0
    }
}

/// Julian day number of noon on a proleptic Gregorian date.
fn gregorian_to_julian_day(year: i32, month: u32, day: u32) -> i64 {
    let (year, month) = (i64::from(year), i64::from(month));
    let a = (14 - month) / 12;
    let y = year + 4800 - a;
    let m = month + 12 * a - 3;
    i64::from(day) + (153 * m + 2) / 5 + 365 * y + y / 4 - y / 100 + y / 400 - 32_045
}

/// Formats a duration in seconds using the largest natural astronomical unit.
///
/// Used by the UI to say "3.2 years" rather than "100 000 000 s".
pub fn format_duration(seconds: f64) -> String {
    let magnitude = seconds.abs();
    if magnitude < 120.0 {
        format!("{seconds:.1} s")
    } else if magnitude < 7200.0 {
        format!("{:.1} min", seconds / 60.0)
    } else if magnitude < 2.0 * DAY {
        format!("{:.1} h", seconds / 3600.0)
    } else if magnitude < 2.0 * YEAR {
        format!("{:.1} d", seconds / DAY)
    } else {
        format!("{:.2} yr", seconds / YEAR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2000_is_the_reference_julian_date() {
        assert!((Epoch::J2000.julian_date() - J2000_JULIAN_DATE).abs() < 1e-9);
        assert_eq!(Epoch::J2000.to_iso8601(), "2000-01-01T12:00:00");
    }

    #[test]
    fn calendar_conversion_round_trips() {
        for &(y, mo, d, h, mi) in &[
            (2000, 1, 1, 12, 0),
            (1969, 7, 20, 20, 17),
            (2026, 8, 13, 6, 30),
            (2149, 12, 31, 23, 59),
        ] {
            let epoch = Epoch::from_gregorian(y, mo, d, h, mi, 0.0);
            let (ry, rmo, rd, rh, rmi, rs) = epoch.to_gregorian();
            assert_eq!(
                (y, mo, d, h, mi),
                (ry, rmo, rd, rh, rmi),
                "round trip failed, seconds = {rs}"
            );
        }
    }

    #[test]
    fn one_julian_year_after_j2000() {
        let epoch = Epoch::J2000 + YEAR;
        assert!((epoch.years() - 1.0).abs() < 1e-12);
        assert!((epoch.days() - 365.25).abs() < 1e-9);
    }
}
