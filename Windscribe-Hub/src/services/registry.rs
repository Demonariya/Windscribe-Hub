use crate::models::{CleanupOutcome, RegistryKeyInfo, RegistryKeyState, RegistryTarget};
use crate::utils::command::CommandRunner;
use crate::utils::time::unix_seconds;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn default_backup_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("Windscribe Hub")
        .join("Backups")
}

pub fn scan_registry_key(
    runner: &dyn CommandRunner,
    target: RegistryTarget,
    backup_dir: &Path,
) -> RegistryKeyInfo {
    let key_path = target.registry_path();
    let state = match runner.run("reg.exe", &["query", key_path]) {
        Ok(output) if output.success => RegistryKeyState::Exists,
        Ok(output) => {
            let combined = output.combined().to_ascii_lowercase();
            if combined.contains("unable to find")
                || combined.contains("cannot find")
                || output.exit_code == Some(1)
            {
                RegistryKeyState::NotFound
            } else {
                RegistryKeyState::Error
            }
        }
        Err(_) => RegistryKeyState::Error,
    };

    let (backup_count, latest_backup) = find_existing_backups(backup_dir, target.backup_prefix());

    RegistryKeyInfo {
        target,
        state,
        last_checked: Some(SystemTime::now()),
        backup_count,
        latest_backup,
        last_error: if state == RegistryKeyState::Error {
            Some("Unable to query Windows registry key.".to_owned())
        } else {
            None
        },
    }
}

pub fn scan_all_registry_keys(
    runner: &dyn CommandRunner,
    backup_dir: &Path,
) -> Vec<RegistryKeyInfo> {
    RegistryTarget::ALL
        .iter()
        .map(|&target| scan_registry_key(runner, target, backup_dir))
        .collect()
}

pub fn find_existing_backups(backup_dir: &Path, prefix: &str) -> (usize, Option<PathBuf>) {
    let Ok(entries) = fs::read_dir(backup_dir) else {
        return (0, None);
    };

    let mut matches: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("reg"))
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name.starts_with(&format!("{prefix}_")))
        })
        .collect();

    matches.sort();
    let count = matches.len();
    let latest = matches.pop();
    (count, latest)
}

pub fn backup_and_delete_registry(
    runner: &dyn CommandRunner,
    target: RegistryTarget,
    backup_dir: &Path,
) -> Result<CleanupOutcome, String> {
    let key = target.registry_path();

    // Step 1: Verify the registry key exists
    let query = runner
        .run("reg.exe", &["query", key])
        .map_err(|err| format!("Failed to query registry key {key}: {err}"))?;
    if !query.success {
        return Err(format!(
            "Registry key does not exist or cannot be read: {key}"
        ));
    }

    // Step 2: Create backup directory
    fs::create_dir_all(backup_dir)
        .map_err(|err| format!("Could not create backup directory: {err}"))?;

    // Step 3: Export timestamped .reg backup
    let stamp = unix_seconds();
    let prefix = target.backup_prefix();
    let backup_path = backup_dir.join(format!("{prefix}_{stamp}.reg"));
    let backup_str = backup_path.to_string_lossy().into_owned();

    let export = runner
        .run("reg.exe", &["export", key, &backup_str, "/y"])
        .map_err(|err| format!("Failed to launch reg.exe export: {err}"))?;
    if !export.success {
        return Err(format!(
            "Registry backup export failed: {}",
            export.combined()
        ));
    }

    // Step 4: Verify backup creation and non-empty file size
    let metadata = fs::metadata(&backup_path).map_err(|err| {
        format!(
            "Backup file verification failed at {}: {err}",
            backup_path.display()
        )
    })?;
    if metadata.len() == 0 {
        return Err(format!(
            "Backup file {} is empty; deletion aborted.",
            backup_path.display()
        ));
    }

    // Step 5: Delete the requested registry key
    let delete = runner
        .run("reg.exe", &["delete", key, "/f"])
        .map_err(|err| format!("Failed to execute reg.exe delete: {err}"))?;
    if !delete.success {
        return Err(format!(
            "Backup was saved to {}, but deletion failed: {}",
            backup_path.display(),
            delete.combined()
        ));
    }

    // Step 6: Verify deletion succeeded
    let verify = runner
        .run("reg.exe", &["query", key])
        .map_err(|err| format!("Could not verify deletion of {key}: {err}"))?;
    if verify.success {
        return Err(format!(
            "Backup was saved to {}, but key {key} still exists after deletion.",
            backup_path.display()
        ));
    }

    Ok(CleanupOutcome {
        target,
        backup_path,
    })
}
