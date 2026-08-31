use chrono::{DateTime, Local};

pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_dash = false;

    for ch in input.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else {
            None
        };

        match mapped {
            Some(ch) => {
                out.push(ch);
                last_dash = false;
            }
            None if !last_dash => {
                out.push('-');
                last_dash = true;
            }
            None => {}
        }
    }

    while out.starts_with('-') {
        out.remove(0);
    }
    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        "session".to_string()
    } else {
        out
    }
}

pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

pub fn format_duration_ms(ms: u64) -> String {
    format_duration(ms / 1000)
}

pub fn normalize_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn format_local_date(date: DateTime<Local>) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn format_local_time(date: DateTime<Local>) -> String {
    date.format("%H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_collapses_separators() {
        assert_eq!(slugify("Computer Networks 101"), "computer-networks-101");
    }

    #[test]
    fn duration_formats_as_hms() {
        assert_eq!(format_duration(3723), "01:02:03");
    }
}
