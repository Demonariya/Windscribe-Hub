use crate::models::{
    InstallSource, InstallStateKind, InstallStatus, ProcessState, WINDSCRIBE_PACKAGE_ID,
};
use crate::utils::command::CommandRunner;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalInstallInfo {
    pub exe_path: Option<PathBuf>,
    pub version: Option<String>,
    pub from_registry: bool,
}

pub trait LocalInstallProbe: Send + Sync {
    fn probe(&self) -> LocalInstallInfo;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsLocalInstallProbe;

impl LocalInstallProbe for WindowsLocalInstallProbe {
    fn probe(&self) -> LocalInstallInfo {
        #[cfg(windows)]
        {
            probe_windows_installation()
        }
        #[cfg(not(windows))]
        {
            LocalInstallInfo::default()
        }
    }
}

#[cfg(windows)]
fn probe_windows_installation() -> LocalInstallInfo {
    use crate::utils::time::expand_env_vars;
    use winreg::{
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
        RegKey,
    };

    let mut candidates = Vec::new();
    let mut reg_version: Option<String> = None;
    let mut from_registry = false;

    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let parent = RegKey::predef(hive);
        for root in [
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ] {
            let Ok(uninstall) = parent.open_subkey(root) else {
                continue;
            };
            for subkey_name in uninstall.enum_keys().flatten() {
                let Ok(key) = uninstall.open_subkey(&subkey_name) else {
                    continue;
                };
                let display_name: String = key.get_value("DisplayName").unwrap_or_default();
                if !display_name.to_ascii_lowercase().contains("windscribe") {
                    continue;
                }
                from_registry = true;
                if reg_version.is_none() {
                    let ver: String = key.get_value("DisplayVersion").unwrap_or_default();
                    if !ver.trim().is_empty() {
                        reg_version = Some(ver.trim().to_owned());
                    }
                }
                let icon: String = key.get_value("DisplayIcon").unwrap_or_default();
                if !icon.is_empty() {
                    let path = icon
                        .trim()
                        .trim_matches('"')
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim_matches('"');
                    if !path.is_empty() {
                        candidates.push(PathBuf::from(expand_env_vars(path)));
                    }
                }
                let location: String = key.get_value("InstallLocation").unwrap_or_default();
                if !location.is_empty() {
                    let folder = PathBuf::from(expand_env_vars(&location));
                    candidates.push(folder.join("Windscribe.exe"));
                    candidates.push(folder.join("windscribe.exe"));
                }
            }
        }
    }

    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable).map(PathBuf::from) {
            candidates.push(root.join("Windscribe").join("Windscribe.exe"));
            candidates.push(
                root.join("Programs")
                    .join("Windscribe")
                    .join("Windscribe.exe"),
            );
        }
    }

    let exe_path = candidates.into_iter().find(|path| {
        path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    });

    LocalInstallInfo {
        exe_path,
        version: reg_version,
        from_registry,
    }
}

pub fn parse_winget_list_version(output_text: &str) -> (bool, Option<String>) {
    let lower_pkg = WINDSCRIBE_PACKAGE_ID.to_ascii_lowercase();
    for line in output_text.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.contains(&lower_pkg) || lower.contains("id version") {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if let Some(id_pos) = tokens
            .iter()
            .position(|t| t.to_ascii_lowercase() == lower_pkg)
        {
            let version = tokens
                .get(id_pos + 1)
                .map(|s| (*s).to_owned())
                .filter(|s| !s.is_empty() && *s != "winget");
            return (true, version);
        }
        return (true, None);
    }
    (false, None)
}

pub fn parse_winget_upgrade(output_text: &str, success: bool) -> (bool, Option<String>) {
    let lower = output_text.to_ascii_lowercase();
    let no_update = [
        "no applicable upgrade found",
        "no installed package found",
        "no available upgrade",
        "all applicable upgrades are excluded",
        "no package found matching input criteria",
    ]
    .iter()
    .any(|phrase| lower.contains(phrase));

    if !success || no_update {
        return (false, None);
    }

    let lower_pkg = WINDSCRIBE_PACKAGE_ID.to_ascii_lowercase();
    for line in output_text.lines() {
        let line_lower = line.to_ascii_lowercase();
        if line_lower.contains(&lower_pkg) && !line_lower.contains("id version") {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if let Some(id_pos) = tokens
                .iter()
                .position(|t| t.to_ascii_lowercase() == lower_pkg)
            {
                let available = tokens
                    .get(id_pos + 2)
                    .map(|s| (*s).to_owned())
                    .filter(|s| !s.is_empty() && *s != "winget");
                return (true, available);
            }
            return (true, None);
        }
    }
    (false, None)
}

pub fn detect_windscribe(
    runner: &dyn CommandRunner,
    probe: &dyn LocalInstallProbe,
    process: ProcessState,
) -> InstallStatus {
    let local = probe.probe();
    let listed = runner.run(
        "winget",
        &[
            "list",
            "--id",
            WINDSCRIBE_PACKAGE_ID,
            "--exact",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
    );

    let (winget_available, managed_by_winget, winget_version, winget_error) = match listed {
        Ok(output) => {
            let combined = output.combined();
            let (found, ver) = if output.success {
                parse_winget_list_version(&combined)
            } else {
                (false, None)
            };
            (true, found, ver, None)
        }
        Err(err) => (
            false,
            false,
            None,
            Some(format!("WinGet is unavailable: {err}")),
        ),
    };

    let installed = managed_by_winget || local.exe_path.is_some();
    let installed_version = winget_version.or(local.version);
    let source = if managed_by_winget {
        InstallSource::WinGet
    } else if local.exe_path.is_some() {
        if local.from_registry {
            InstallSource::LocalRegistry
        } else {
            InstallSource::LocalPath
        }
    } else if !winget_available {
        InstallSource::Unknown
    } else {
        InstallSource::None
    };

    let mut update_available = false;
    let mut latest_version = None;
    let mut error = winget_error;
    let state_kind;
    let summary;

    if !installed {
        if winget_available {
            state_kind = InstallStateKind::NotInstalled;
            summary = format!(
                "Windscribe was not detected. Ready to install {WINDSCRIBE_PACKAGE_ID} via WinGet."
            );
        } else {
            state_kind = InstallStateKind::Error;
            summary = "Windscribe was not detected and WinGet is unavailable.".to_owned();
        }
    } else if !managed_by_winget {
        state_kind = InstallStateKind::InstalledUnmanaged;
        summary = "Installed locally (not managed by WinGet). Ready to launch.".to_owned();
    } else {
        match runner.run(
            "winget",
            &[
                "upgrade",
                "--id",
                WINDSCRIBE_PACKAGE_ID,
                "--exact",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        ) {
            Ok(output) => {
                let text = output.combined();
                let (has_update, avail) = parse_winget_upgrade(&text, output.success);
                update_available = has_update;
                latest_version = avail.or_else(|| {
                    if !has_update {
                        installed_version.clone()
                    } else {
                        None
                    }
                });
                if update_available {
                    state_kind = InstallStateKind::InstalledUpdateAvailable;
                    summary = match (&installed_version, &latest_version) {
                        (Some(cur), Some(new)) => format!("Update ready: v{cur} → v{new}"),
                        _ => "A Windscribe update is available via WinGet.".to_owned(),
                    };
                } else {
                    state_kind = InstallStateKind::InstalledUpToDate;
                    summary = match &installed_version {
                        Some(ver) => format!("Windscribe v{ver} is installed and up to date."),
                        None => "Windscribe is installed via WinGet and up to date.".to_owned(),
                    };
                }
            }
            Err(err) => {
                state_kind = InstallStateKind::InstalledUnmanaged;
                summary = "Installed via WinGet; update check failed.".to_owned();
                error = Some(format!("Update check failed: {err}"));
            }
        }
    }

    InstallStatus {
        installed,
        managed_by_winget,
        winget_available,
        update_available,
        installed_version,
        latest_version,
        install_path: local.exe_path,
        source,
        process,
        state_kind,
        summary,
        error,
        last_checked: Some(SystemTime::now()),
    }
}

pub fn run_winget_package_action(
    runner: &dyn CommandRunner,
    is_update: bool,
) -> Result<String, String> {
    let action = if is_update { "upgrade" } else { "install" };
    let output = runner
        .run(
            "winget",
            &[
                action,
                "--id",
                WINDSCRIBE_PACKAGE_ID,
                "--exact",
                "--source",
                "winget",
                "--accept-package-agreements",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        )
        .map_err(|err| format!("WinGet could not start: {err}"))?;

    let combined = output.combined();
    if output.success {
        Ok(if combined.is_empty() {
            format!("WinGet {action} completed successfully.")
        } else {
            combined
        })
    } else {
        Err(format!(
            "WinGet {action} failed (exit {:?}): {}",
            output.exit_code, combined
        ))
    }
}

pub fn launch_windscribe(probe: &dyn LocalInstallProbe) -> Result<(), String> {
    if let Some(exe) = probe.probe().exe_path {
        Command::new(&exe)
            .spawn()
            .map_err(|err| format!("Failed to start {}: {err}", exe.display()))?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        let mut shortcuts = Vec::new();
        for root in [
            std::env::var_os("ProgramData")
                .map(PathBuf::from)
                .map(|p| p.join(r"Microsoft\Windows\Start Menu\Programs")),
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .map(|p| p.join(r"Microsoft\Windows\Start Menu\Programs")),
        ]
        .into_iter()
        .flatten()
        {
            find_shortcuts(&root, &mut shortcuts, 5);
        }

        if let Some(shortcut) = shortcuts.into_iter().next() {
            use std::os::windows::process::CommandExt;
            let mut command = Command::new("cmd.exe");
            command
                .args(["/C", "start", "", &shortcut.to_string_lossy()])
                .creation_flags(0x0800_0000);
            command
                .spawn()
                .map_err(|err| format!("Failed to launch shortcut: {err}"))?;
            return Ok(());
        }
    }

    Err("Windscribe is installed, but its executable or shortcut could not be located.".to_owned())
}

#[allow(dead_code)]
fn find_shortcuts(root: &Path, output: &mut Vec<PathBuf>, depth: usize) {
    if depth == 0 || !root.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_shortcuts(&path, output, depth - 1);
        } else if path.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .to_ascii_lowercase()
                .contains("windscribe")
        }) && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        {
            output.push(path);
        }
    }
}
