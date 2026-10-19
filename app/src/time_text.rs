//! Local wall-clock text for recorded instants: the schedules surface shows a stored ISO instant as
//! a local date and time, with a relative form for a next run, instead of the raw wire string.
//!
//! The Host records UTC instants (`…Z` or an explicit offset), so the arithmetic here is plain civil
//! date math — no zone database and no dependency. Only the final display step asks the platform for
//! the local zone, through the same thread-safe `localtime_r` the chat clock already uses.

/// Milliseconds since the Unix epoch for one RFC 3339 instant, or `None` when it is not one.
///
/// Accepts `YYYY-MM-DDTHH:MM:SS`, an optional `.fraction`, and a `Z` or `±HH:MM` offset. A value that
/// cannot be read is refused rather than guessed: a schedule row with an unreadable instant must
/// fall back to the stored string, never to a wrong time.
pub fn parse_instant(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = text.split_once('T')?;
    let mut date_parts = date.split('-');
    // Fixed width, as RFC 3339 writes it: a looser read would accept `2026-1-1`, which the Host
    // never sends, and a wrong time is worse than the stored text.
    let year_text = date_parts.next()?;
    let month_text = date_parts.next()?;
    let day_text = date_parts.next()?;
    if year_text.len() != 4
        || month_text.len() != 2
        || day_text.len() != 2
        || date_parts.next().is_some()
    {
        return None;
    }
    let year: i64 = year_text.parse().ok()?;
    let month: i64 = month_text.parse().ok()?;
    let day: i64 = day_text.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Split the offset off the time: `Z`, or the last `+`/`-` after the seconds.
    let (time, offset_seconds) = if let Some(time) = rest.strip_suffix('Z') {
        (time, 0i64)
    } else if let Some(index) = rest.rfind(['+', '-']) {
        let (time, offset) = rest.split_at(index);
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        let mut parts = offset[1..].split(':');
        let hours_text = parts.next()?;
        let minutes_text = parts.next()?;
        if hours_text.len() != 2 || minutes_text.len() != 2 || parts.next().is_some() {
            return None;
        }
        let hours: i64 = hours_text.parse().ok()?;
        let minutes: i64 = minutes_text.parse().ok()?;
        if hours > 23 || minutes > 59 {
            return None;
        }
        (time, sign * (hours * 3600 + minutes * 60))
    } else {
        return None;
    };
    let mut time_parts = time.split(':');
    let hour_text = time_parts.next()?;
    let minute_text = time_parts.next()?;
    if hour_text.len() != 2 || minute_text.len() != 2 {
        return None;
    }
    let hour: i64 = hour_text.parse().ok()?;
    let minute: i64 = minute_text.parse().ok()?;
    let seconds = time_parts.next()?;
    let (second, fraction) = match seconds.split_once('.') {
        Some((second, fraction)) => (second, Some(fraction)),
        None => (seconds, None),
    };
    let second: i64 = second.parse().ok()?;
    if time_parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let millis = fraction.map_or(0, |fraction| {
        let digits: String = fraction.chars().take(3).collect();
        let padded = format!("{digits:0<3}");
        padded.parse::<i64>().unwrap_or(0)
    });
    let days = days_from_civil(year, month, day);
    let seconds_of_day = hour * 3600 + minute * 60 + second;
    Some(((days * 86_400 + seconds_of_day - offset_seconds) * 1000) + millis)
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The local date and time of one instant, as `YYYY-MM-DD HH:MM`. `None` when the instant is not
/// readable or the platform cannot resolve the zone, so a caller shows the stored text instead.
pub fn local_date_time(text: &str) -> Option<String> {
    let millis = parse_instant(text)?;
    let seconds = millis.div_euclid(1000);
    let time = libc::time_t::try_from(seconds).ok()?;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `localtime_r` writes into the `tm` this frame owns and reads only `time`; it is the
    // thread-safe form and retains no pointer to either.
    let filled = unsafe { libc::localtime_r(&time, tm.as_mut_ptr()) };
    if filled.is_null() {
        return None;
    }
    let tm = unsafe { tm.assume_init() };
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min
    ))
}

/// The reference's relative next-run form, coarse and signed: `in 2 hours`, `in 5 minutes`,
/// `3 days overdue`. A whole unit is used so the label does not jitter between renders.
pub fn relative(instant_millis: i64, now_millis: i64) -> String {
    let delta = instant_millis - now_millis;
    let minutes = delta / 60_000;
    let (magnitude, unit) = if minutes.abs() >= 60 * 24 {
        (minutes / (60 * 24), "day")
    } else if minutes.abs() >= 60 {
        (minutes / 60, "hour")
    } else if minutes.abs() >= 1 {
        (minutes, "minute")
    } else {
        return if delta >= 0 {
            "now".to_owned()
        } else {
            "overdue".to_owned()
        };
    };
    let plural = if magnitude.abs() == 1 { "" } else { "s" };
    if delta >= 0 {
        format!("in {magnitude} {unit}{plural}")
    } else {
        format!("{} {unit}{plural} overdue", magnitude.abs())
    }
}

/// The schedules rows' own rendering: a stored instant as local text, falling back to the stored
/// string when it cannot be read, so nothing is ever silently blank.
pub fn local_or_stored(text: &str) -> String {
    local_date_time(text).unwrap_or_else(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    //! Civil-date arithmetic and the relative form, with vectors computed independently.
    use super::*;

    #[test]
    fn instants_parse_to_epoch_millis() {
        assert_eq!(parse_instant("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(
            parse_instant("2026-01-01T00:00:00Z"),
            Some(1_767_225_600_000)
        );
        assert_eq!(
            parse_instant("2026-10-17T09:00:00Z"),
            Some(1_792_227_600_000)
        );
        // A fraction is kept to milliseconds.
        assert_eq!(parse_instant("1970-01-01T00:00:00.500Z"), Some(500));
        // An explicit offset converts to the same instant as `Z`.
        assert_eq!(
            parse_instant("2026-10-17T11:00:00+02:00"),
            parse_instant("2026-10-17T09:00:00Z")
        );
        assert_eq!(
            parse_instant("2026-10-17T04:00:00-05:00"),
            parse_instant("2026-10-17T09:00:00Z")
        );
    }

    #[test]
    fn an_unreadable_instant_is_refused_rather_than_guessed() {
        for bad in [
            "",
            "yesterday",
            "2026-10-17",
            "2026-10-17T09:00:00",
            "2026-13-17T09:00:00Z",
            "2026-10-17T25:00:00Z",
            "2026-10-17T09:00:00+2:00",
            "2026-10-17T09:00:00Zjunk",
        ] {
            assert_eq!(parse_instant(bad), None, "{bad}");
        }
        // An unreadable instant keeps the stored text on screen.
        assert_eq!(local_or_stored("not a time"), "not a time");
    }

    #[test]
    fn the_relative_form_is_coarse_and_signed() {
        let minute = 60_000;
        let hour = 60 * minute;
        let day = 24 * hour;
        assert_eq!(relative(2 * hour, 0), "in 2 hours");
        assert_eq!(relative(hour, 0), "in 1 hour");
        assert_eq!(relative(5 * minute, 0), "in 5 minutes");
        assert_eq!(relative(3 * day, 0), "in 3 days");
        assert_eq!(relative(-2 * hour, 0), "2 hours overdue");
        assert_eq!(relative(-3 * day, 0), "3 days overdue");
        assert_eq!(relative(30_000, 0), "now");
        assert_eq!(relative(-30_000, 0), "overdue");
    }
}
