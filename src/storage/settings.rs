use crate::models::AppSettings;
use crate::storage::vault::replace_file_atomically;
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_settings_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("WindscribeHub")
        .join("settings.json")
}

pub fn load_settings(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let data = fs::read(path).map_err(|err| format!("Failed to read settings: {err}"))?;
    let mut settings = serde_json::from_slice::<AppSettings>(&data)
        .map_err(|err| format!("Invalid settings format: {err}"))?;
    settings.refresh_interval_secs = settings.refresh_interval_secs.clamp(10, 600);
    Ok(settings)
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Settings directory could not be resolved.".to_owned())?;
    fs::create_dir_all(parent).map_err(|err| format!("Failed to create settings folder: {err}"))?;

    let payload = serde_json::to_vec_pretty(settings)
        .map_err(|err| format!("Failed to serialize settings: {err}"))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, payload).map_err(|err| format!("Failed to write settings: {err}"))?;
    replace_file_atomically(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        err.to_string()
    })?;
    Ok(())
}
