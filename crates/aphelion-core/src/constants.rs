//! Physical and astronomical constants, plus unit conversions.
//!
//! Values follow CODATA 2018 and the IAU 2009/2015 nominal system. Where a
//! quantity is defined rather than measured (the astronomical unit, the speed
//! of light, the Julian day) the exact value is used.

/// Newtonian constant of gravitation, in m³·kg⁻¹·s⁻² (CODATA 2018).
///
/// This is the least precisely known constant in the whole simulation
/// (relative uncertainty ~2.2e-5). Products `GM` are known far more accurately,
/// which is why ephemerides publish `GM` rather than mass.
pub const G: f64 = 6.674_30e-11;

/// Speed of light in vacuum, in m·s⁻¹ (exact by SI definition).
pub const C: f64 = 299_792_458.0;

/// Astronomical unit, in metres (exact by IAU 2012 definition).
pub const AU: f64 = 1.495_978_707e11;

/// Parsec, in metres.
pub const PARSEC: f64 = 3.085_677_581_491_367e16;

/// Light-year, in metres.
pub const LIGHT_YEAR: f64 = C * YEAR;

/// Heliocentric gravitational constant `GM☉`, in m³·s⁻² (IAU 2015 nominal).
pub const GM_SUN: f64 = 1.327_124_400_18e20;

/// Geocentric gravitational constant `GM🜨`, in m³·s⁻² (IAU 2015 nominal).
pub const GM_EARTH: f64 = 3.986_004_418e14;

/// Solar mass, in kilograms, derived from [`GM_SUN`] and [`G`].
pub const SOLAR_MASS: f64 = GM_SUN / G;

/// Earth mass, in kilograms, derived from [`GM_EARTH`] and [`G`].
pub const EARTH_MASS: f64 = GM_EARTH / G;

/// Nominal solar equatorial radius, in metres (IAU 2015).
pub const SOLAR_RADIUS: f64 = 6.957e8;

/// Nominal Earth equatorial radius, in metres (IAU 2015).
pub const EARTH_RADIUS: f64 = 6.378_1e6;

/// Julian day, in seconds (exact).
pub const DAY: f64 = 86_400.0;

/// Julian year, in seconds (exact: 365.25 Julian days).
pub const YEAR: f64 = 365.25 * DAY;

/// Julian century, in seconds (exact: 36525 Julian days).
pub const CENTURY: f64 = 36_525.0 * DAY;

/// Julian date of the J2000.0 epoch (2000-01-01T12:00:00 TT).
pub const J2000_JULIAN_DATE: f64 = 2_451_545.0;

/// Radians in one arcsecond.
pub const ARCSEC: f64 = std::f64::consts::PI / (180.0 * 3600.0);

/// Converts degrees to radians.
#[inline]
pub fn deg(degrees: f64) -> f64 {
    degrees.to_radians()
}

/// Converts astronomical units to metres.
#[inline]
pub fn au(astronomical_units: f64) -> f64 {
    astronomical_units * AU
}

/// Converts metres to astronomical units.
#[inline]
pub fn to_au(metres: f64) -> f64 {
    metres / AU
}

/// Converts kilometres to metres.
#[inline]
pub fn km(kilometres: f64) -> f64 {
    kilometres * 1000.0
}

/// Converts days to seconds.
#[inline]
pub fn days(count: f64) -> f64 {
    count * DAY
}

/// Converts years to seconds.
#[inline]
pub fn years(count: f64) -> f64 {
    count * YEAR
}
