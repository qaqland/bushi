pub fn human_size(size: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let size = size as f64;
    if size >= MIB {
        format!("{:.1} MiB", size / MIB)
    } else if size >= KIB {
        format!("{:.1} KiB", size / KIB)
    } else {
        format!("{} B", size as u64)
    }
}

pub fn utc_time(unix_seconds: i64) -> String {
    utc_stamp(unix_seconds, "[year]-[month]-[day] [hour]:[minute] UTC")
}

/// Same timestamp without the zone label, for columns headed "Time (UTC)".
pub fn utc_time_without_zone(unix_seconds: i64) -> String {
    utc_stamp(unix_seconds, "[year]-[month]-[day] [hour]:[minute]")
}

fn utc_stamp(unix_seconds: i64, format: &'static str) -> String {
    let format =
        time::format_description::parse_borrowed::<2>(format).expect("static format description");
    time::OffsetDateTime::from_unix_timestamp(unix_seconds)
        .map(|date| date.to_offset(time::UtcOffset::UTC).format(&format))
        .ok()
        .and_then(Result::ok)
        .unwrap_or_else(|| "Unknown date".to_string())
}

#[cfg(test)]
mod tests {
    use super::{utc_time, utc_time_without_zone};

    #[test]
    fn utc_time_is_fixed_width() {
        assert_eq!(utc_time(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc_time(1_000_000_000), "2001-09-09 01:46 UTC");
        assert_eq!(utc_time_without_zone(1_000_000_000), "2001-09-09 01:46");
    }
}
