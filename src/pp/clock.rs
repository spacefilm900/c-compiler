//! Translation phase 1's trigraph replacement (C11 5.2.1.1) and the date
//! and time of translation for `__DATE__` and `__TIME__` (C11 6.10.8.1).

use std::borrow::Cow;

use super::{error_at, PpToken, Processor};
use crate::diag::Diagnostic;

/// The largest `SOURCE_DATE_EPOCH` accepted: 9999-12-31 23:59:59 UTC, the
/// last second whose year fits the four digits of `__DATE__`.
const MAX_EPOCH: u64 = 253_402_300_799;

impl Processor {
    /// Translation phase 1 for one source file: trigraphs, when enabled.
    pub(super) fn phase_one<'a>(&self, source: &'a str) -> Cow<'a, str> {
        if self.trigraphs {
            replace_trigraphs(source)
        } else {
            Cow::Borrowed(source)
        }
    }
}

/// Replace each of the nine trigraph sequences (C11 5.2.1.1) with the
/// character it stands for. Any other `??` sequence is left alone, and a
/// replaced character never starts a new trigraph: `???=` is `?#`.
///
/// Columns in diagnostics count a replaced trigraph as one character.
pub(super) fn replace_trigraphs(source: &str) -> Cow<'_, str> {
    if !source.contains("??") {
        return Cow::Borrowed(source);
    }
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut start = 0;
    let mut index = 0;
    while index + 2 < bytes.len() {
        if bytes[index] == b'?' && bytes[index + 1] == b'?' {
            if let Some(replacement) = trigraph(bytes[index + 2]) {
                out.push_str(&source[start..index]);
                out.push(replacement);
                index += 3;
                start = index;
                continue;
            }
        }
        index += 1;
    }
    out.push_str(&source[start..]);
    Cow::Owned(out)
}

fn trigraph(third: u8) -> Option<char> {
    Some(match third {
        b'=' => '#',
        b'(' => '[',
        b'/' => '\\',
        b')' => ']',
        b'\'' => '^',
        b'<' => '{',
        b'!' => '|',
        b'>' => '}',
        b'-' => '~',
        _ => return None,
    })
}

/// The `__DATE__` and `__TIME__` replacement lists, as string literals:
/// `"Mmm dd yyyy"` (the day padded with a space) and `"hh:mm:ss"`.
///
/// The time is the current time in UTC, or the time given by the
/// `SOURCE_DATE_EPOCH` environment variable (seconds since 1970-01-01
/// 00:00:00 UTC, as the GCC manual's "Environment Variables" describes it),
/// so that builds can be reproducible. A value that is not such a number
/// (decimal digits only, no sign or spaces, at most [`MAX_EPOCH`]) is an
/// error.
pub(super) fn translation_date_time(at: &PpToken) -> Result<(String, String), Diagnostic> {
    let seconds = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(text) => parse_epoch(&text).ok_or_else(|| {
            error_at(
                at,
                format!(
                    "environment variable SOURCE_DATE_EPOCH must be a number of seconds \
                     from 0 to {MAX_EPOCH}, not '{text}'"
                ),
            )
        })?,
        Err(_) => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs().min(MAX_EPOCH)),
    };
    Ok(format_date_time(seconds))
}

fn parse_epoch(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u64>()
        .ok()
        .filter(|&seconds| seconds <= MAX_EPOCH)
}

/// `seconds` since 1970-01-01 00:00:00 UTC as the two literals.
pub(super) fn format_date_time(seconds: u64) -> (String, String) {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let days = seconds / 86_400;
    let of_day = seconds % 86_400;
    let (year, month, day) = civil_date(days);
    let date = format!("\"{} {day:>2} {year}\"", MONTHS[month as usize - 1]);
    let time = format!(
        "\"{:02}:{:02}:{:02}\"",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    );
    (date, time)
}

/// The proleptic Gregorian (year, month 1–12, day 1–31) of `days` after
/// 1970-01-01. Years are counted from March so that the leap day ends one.
fn civil_date(days: u64) -> (u64, u64, u64) {
    // Shift the epoch to 0000-03-01: 719468 days before 1970-01-01.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_nine_trigraphs() {
        assert_eq!(
            replace_trigraphs("??= ??( ??/ ??) ??' ??< ??! ??> ??-"),
            "# [ \\ ] ^ { | } ~"
        );
    }

    #[test]
    fn other_question_marks_are_kept() {
        assert_eq!(replace_trigraphs("a ?? b ??? c ?"), "a ?? b ??? c ?");
        // The replacement does not start a new trigraph.
        assert_eq!(replace_trigraphs("???="), "?#");
        assert_eq!(replace_trigraphs("????/"), "??\\");
        assert_eq!(replace_trigraphs("??"), "??");
        assert!(matches!(replace_trigraphs("no marks"), Cow::Borrowed(_)));
    }

    #[test]
    fn dates_and_times() {
        assert_eq!(
            format_date_time(0),
            ("\"Jan  1 1970\"".to_string(), "\"00:00:00\"".to_string())
        );
        // 2000-02-29 (a leap day in a century year) 12:34:56.
        assert_eq!(
            format_date_time(951_827_696),
            ("\"Feb 29 2000\"".to_string(), "\"12:34:56\"".to_string())
        );
        // 2026-11-24 19:10:07, the day padded only when it is one digit.
        assert_eq!(
            format_date_time(1_795_547_407).0,
            "\"Nov 24 2026\"".to_string()
        );
        assert_eq!(
            format_date_time(MAX_EPOCH),
            ("\"Dec 31 9999\"".to_string(), "\"23:59:59\"".to_string())
        );
        // 2100 is not a leap year: the day after Feb 28 is Mar 1.
        assert_eq!(format_date_time(4_107_542_400).0, "\"Mar  1 2100\"");
    }

    #[test]
    fn source_date_epoch_values() {
        assert_eq!(parse_epoch("0"), Some(0));
        assert_eq!(parse_epoch("1700000000"), Some(1_700_000_000));
        assert_eq!(parse_epoch("007"), Some(7));
        assert_eq!(parse_epoch("253402300799"), Some(MAX_EPOCH));
        for bad in [
            "",
            " 5",
            "5\n",
            "-1",
            "+5",
            "1.5",
            "abc",
            "253402300800",
            "99999999999999999999",
        ] {
            assert_eq!(parse_epoch(bad), None, "{bad:?}");
        }
    }
}
