//! `iso_time` — canonical RFC-3339 / ISO-8601 UTC timestamp
//! formatter for the substrate.
//!
//! Single source of truth. Anywhere the substrate stamps a
//! moment-of-build (manifest `updated_at`, fingerprint registry
//! `timestamp`, substrate-state `captured_at`, session-context
//! capture time, build-report `started`, etc.) flows through
//! [`format_rfc3339_utc`].
//!
//! Per `[[iso-standards]]` doctrine — default to ISO/IEC 8601
//! for dates + 25010 for quality + 40500 WCAG. RFC 3339 is the
//! IETF profile of ISO 8601 that closes ambiguity (always a `Z`
//! / offset, fixed-width seconds, lowercase `T`).
//!
//! Why pure stdlib (not `chrono` / `time` crate): forge-core's
//! invariant is "compiles in <2s cold-cache". Adding a date
//! crate for a fixed-width formatter that fits in 25 lines
//! would bloat the dependency graph for every consumer.
//!
//! AVP-2 INVARIANTS
//! ----------------
//! * `forbid(unsafe_code)` inherited.
//! * No I/O; pure transformation epoch → string.
//! * Howard Hinnant's "civil from days" algorithm — public
//!   domain, deterministic, works for any reasonable date range.
//!
//! Wire-shape contract — output is ALWAYS:
//!
//! ```text
//! YYYY-MM-DDTHH:MM:SSZ   (20 chars, no fractional, no offset)
//! ```
//!
//! Downstream parsers (forge-core::reference_capture readers,
//! the fingerprint registry verifier, the crawler-side mirror)
//! rely on this exact width. Adding fractional seconds or
//! offset suffix is a spec change requiring a CaptureSpec /
//! report-format bump.

use std::time::{SystemTime, UNIX_EPOCH};

/// Format an epoch second timestamp as RFC-3339 UTC.
///
/// Output is fixed-width `YYYY-MM-DDTHH:MM:SSZ` (20 chars).
/// Always uppercase `T` + `Z` per RFC 3339 § 5.6 (a profile of
/// ISO 8601 that rejects lowercase variants).
#[must_use]
pub fn format_rfc3339_utc(epoch: u64) -> String {
    let days = epoch / 86400;
    let secs_in_day = epoch % 86400;
    let hour = secs_in_day / 3600;
    let minute = (secs_in_day % 3600) / 60;
    let second = secs_in_day % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Current wall-clock as RFC-3339 UTC. Falls back to the epoch
/// (`1970-01-01T00:00:00Z`) if the system clock pre-dates the
/// Unix epoch — substrate stamps must never panic, even on a
/// host with a corrupt clock.
#[must_use]
pub fn current_rfc3339_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_rfc3339_utc(secs)
}

/// Howard Hinnant's "civil from days" — converts day-count since
/// 1970-01-01 to (year, month, day). Public-domain algorithm.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = (y + i64::from(m <= 2)) as i32;
    (year, m, d)
}

/// Check whether a string is a valid RFC-3339 UTC timestamp in
/// the substrate's canonical fixed-width form. Substrate readers
/// (manifest loaders, registry verifiers) should consult this
/// before trusting a timestamp field.
#[must_use]
pub fn is_canonical_rfc3339_utc(s: &str) -> bool {
    if s.len() != 20 {
        return false;
    }
    let b = s.as_bytes();
    // Layout: YYYY-MM-DDTHH:MM:SSZ
    // Indices: 0123456789012345678901
    let digits_at = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    let ch_at = |i: usize, c: u8| b.get(i) == Some(&c);
    digits_at(0)
        && digits_at(1)
        && digits_at(2)
        && digits_at(3)
        && ch_at(4, b'-')
        && digits_at(5)
        && digits_at(6)
        && ch_at(7, b'-')
        && digits_at(8)
        && digits_at(9)
        && ch_at(10, b'T')
        && digits_at(11)
        && digits_at(12)
        && ch_at(13, b':')
        && digits_at(14)
        && digits_at(15)
        && ch_at(16, b':')
        && digits_at(17)
        && digits_at(18)
        && ch_at(19, b'Z')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_unix_epoch() {
        assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_date_2026_05_20() {
        // 2026-05-20T00:00:00Z = 1779235200 epoch
        assert_eq!(format_rfc3339_utc(1779235200), "2026-05-20T00:00:00Z");
    }

    #[test]
    fn known_date_2000_01_01() {
        // 2000-01-01T00:00:00Z = 946684800 epoch
        assert_eq!(format_rfc3339_utc(946684800), "2000-01-01T00:00:00Z");
    }

    #[test]
    fn output_is_always_20_chars() {
        for epoch in [0, 1, 86399, 86400, 946684800, 1779235200, 4102444800] {
            assert_eq!(format_rfc3339_utc(epoch).len(), 20);
        }
    }

    #[test]
    fn hms_round_trips_within_day() {
        // 13:45:09 on 2026-05-20
        let epoch = 1779235200 + 13 * 3600 + 45 * 60 + 9;
        assert_eq!(format_rfc3339_utc(epoch), "2026-05-20T13:45:09Z");
    }

    #[test]
    fn current_does_not_panic() {
        let s = current_rfc3339_utc();
        assert!(is_canonical_rfc3339_utc(&s), "got {s}");
    }

    #[test]
    fn validator_accepts_canonical() {
        assert!(is_canonical_rfc3339_utc("1970-01-01T00:00:00Z"));
        assert!(is_canonical_rfc3339_utc("2026-05-20T13:45:09Z"));
    }

    #[test]
    fn validator_rejects_off_shape() {
        // Wrong length.
        assert!(!is_canonical_rfc3339_utc(""));
        assert!(!is_canonical_rfc3339_utc("2026-05-20T13:45:09"));
        // Lowercase t (RFC 3339 § 5.6 requires uppercase).
        assert!(!is_canonical_rfc3339_utc("2026-05-20t13:45:09Z"));
        // Fractional seconds (substrate canonical excludes).
        assert!(!is_canonical_rfc3339_utc("2026-05-20T13:45:09.1Z"));
        // Wrong separator.
        assert!(!is_canonical_rfc3339_utc("2026/05/20T13:45:09Z"));
        // Non-digit in year.
        assert!(!is_canonical_rfc3339_utc("20a6-05-20T13:45:09Z"));
        // Trailing offset (we only accept Z).
        assert!(!is_canonical_rfc3339_utc("2026-05-20T13:45:09+"));
    }
}
