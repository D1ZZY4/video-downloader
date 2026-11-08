//! Formatting helpers for download info and paths.

pub(crate) fn filesize_fmt(num: i64, suffix: &str) -> String {
    let mut n = num as f64;
    const UNITS: [&str; 9] = ["", "k", "M", "G", "T", "P", "E", "Z", "Y"];
    let mut unit = "";
    for u in UNITS {
        if n.abs() < 1000.0 {
            unit = u;
            break;
        }
        n /= 1000.0;
        unit = u;
    }
    format!("{:.1}\u{00A0}{}{}", n, unit, suffix)
}

pub(crate) fn format_download_info(
    download_bytes: i64,
    download_bytes_total: i64,
    eta: i64,
    download_speed: i64,
) -> String {
    let time_msg = if eta >= 0 {
        let h = eta / 3600;
        let min = (eta / 60) % 60;
        let s = eta % 60;
        format!("{}\u{2236}{:02}:{:02}", h, min, s)
    } else {
        String::new()
    };
    let size_msg = if download_bytes >= 0 || download_bytes_total >= 0 {
        let a = if download_bytes >= 0 {
            filesize_fmt(download_bytes, "B")
        } else {
            "unknown".to_string()
        };
        let b = if download_bytes_total >= 0 {
            filesize_fmt(download_bytes_total, "B")
        } else {
            "unknown".to_string()
        };
        format!("{} of {}", a, b)
    } else {
        String::new()
    };
    let speed_msg = if download_speed >= 0 {
        filesize_fmt(download_speed, "B/\u{2060}s")
    } else {
        String::new()
    };
    let mut out = String::new();
    if !time_msg.is_empty() {
        out.push_str(&time_msg);
    }
    if !time_msg.is_empty() && (!size_msg.is_empty() || !speed_msg.is_empty()) {
        out.push_str("\u{00A0}- ");
    }
    out.push_str(&size_msg);
    if !size_msg.is_empty() && !speed_msg.is_empty() {
        out.push_str(&format!(" ({})", speed_msg));
    } else {
        out.push_str(&speed_msg);
    }
    out
}

pub(crate) fn format_path_for_tooltip(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    if let Ok(home) = std::env::var("HOME") {
        if path == home {
            return "~".to_string();
        }
        if path.starts_with(&format!("{}/", home)) {
            return format!("~{}", &path[home.len()..]);
        }
    }
    path.to_string()
}
