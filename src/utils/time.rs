use std::time::{SystemTime, UNIX_EPOCH};

pub fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn format_hms(time: SystemTime) -> String {
    let total = time
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (total / 3600) % 24;
    let minutes = (total / 60) % 60;
    let seconds = total % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

pub fn format_now_hms() -> String {
    format_hms(SystemTime::now())
}

pub fn expand_env_vars(value: &str) -> String {
    let mut result = value.to_owned();
    for (key, val) in std::env::vars() {
        let token = format!("%{key}%");
        if result.contains(&token) {
            result = result.replace(&token, &val);
        }
    }
    result
}
