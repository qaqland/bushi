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

pub fn relative_time(unix_seconds: i64) -> String {
    let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(value) => value.as_secs() as i64,
        Err(_) => unix_seconds,
    };
    let delta = now - unix_seconds;
    if delta < 0 {
        return "future".to_string();
    }
    let minute = 60;
    let hour = 60 * minute;
    let day = 24 * hour;
    if delta < minute {
        "now".to_string()
    } else if delta < hour {
        format!("{}m ago", delta / minute)
    } else if delta < day {
        format!("{}h ago", delta / hour)
    } else if delta < 30 * day {
        format!("{}d ago", delta / day)
    } else if delta < 365 * day {
        format!("{}mo ago", delta / (30 * day))
    } else {
        format!("{}y ago", delta / (365 * day))
    }
}
