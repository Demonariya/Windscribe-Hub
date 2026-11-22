use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use windscribe_hub::models::{
    AccountDraft, AppSettings, InstallSource, InstallStateKind, ProcessState, RegistryKeyState,
    RegistryTarget, ThemeMode, WindscribeAccount,
};
use windscribe_hub::services::accounts::{
    apply_draft, filter_accounts, generate_strong_password, remove_account, suggest_username,
    SaveResult,
};
use windscribe_hub::services::process::detect_windscribe_process;
use windscribe_hub::services::registry::{
    backup_and_delete_registry, find_existing_backups, scan_registry_key,
};
use windscribe_hub::services::winget::{
    detect_windscribe, run_winget_package_action, LocalInstallInfo, LocalInstallProbe,
};
use windscribe_hub::storage::settings::{load_settings, save_settings};
use windscribe_hub::storage::vault::{VaultCipher, VaultError, VaultStore};
use windscribe_hub::utils::command::{CommandOutput, CommandRunner};

struct MockCommandRunner {
    responses: Mutex<VecDeque<(String, io::Result<CommandOutput>)>>,
}

impl MockCommandRunner {
    fn with_responses(items: Vec<(&str, io::Result<CommandOutput>)>) -> Self {
        Self {
            responses: Mutex::new(items.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()),
        }
    }
}

impl CommandRunner for MockCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        let mut queue = self.responses.lock().unwrap();
        let (expected, res) = queue
            .pop_front()
            .unwrap_or_else(|| panic!("Unexpected command: {program} {args:?}"));
        assert_eq!(program, expected, "Unexpected program invoked");
        // Side-effect simulation when testing reg.exe export
        if program == "reg.exe" && args.first().copied() == Some("export") {
            if let Ok(ref out) = res {
                if out.success {
                    let backup_path = PathBuf::from(args[2]);
                    if out.stdout != "EMPTY_BACKUP" {
                        fs::write(backup_path, "Windows Registry Editor Version 5.00\r\n")
                            .expect("mock write backup");
                    } else {
                        fs::write(backup_path, "").expect("mock write empty backup");
                    }
                }
            }
        }
        res
    }
}

struct MockProbe(LocalInstallInfo);
impl LocalInstallProbe for MockProbe {
    fn probe(&self) -> LocalInstallInfo {
        self.0.clone()
    }
}

fn ok_out(stdout: &str) -> io::Result<CommandOutput> {
    Ok(CommandOutput {
        success: true,
        exit_code: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    })
}

fn err_out(exit_code: i32, stderr: &str) -> io::Result<CommandOutput> {
    Ok(CommandOutput {
        success: false,
        exit_code: Some(exit_code),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    })
}

struct XorMockCipher(u8);
impl VaultCipher for XorMockCipher {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let mut out = vec![b'D', b'P', b'A', b'P'];
        out.extend(plaintext.iter().map(|b| b ^ self.0));
        Ok(out)
    }

    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
        if ciphertext.len() < 4 || &ciphertext[..4] != b"DPAP" {
            return Err(VaultError::Corrupted(
                "Invalid DPAPI magic header".to_owned(),
            ));
        }
        Ok(ciphertext[4..].iter().map(|b| b ^ self.0).collect())
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "windscribe_hub_test_{name}_{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn winget_managed_up_to_date_detection() {
    let runner = MockCommandRunner::with_responses(vec![
        (
            "winget",
            ok_out("Name       Id                    Version Source\nWindscribe Windscribe.Windscribe 2.12.7  winget\n"),
        ),
        (
            "winget",
            ok_out("No applicable upgrade found."),
        ),
    ]);
    let probe = MockProbe(LocalInstallInfo {
        exe_path: Some(PathBuf::from(r"C:\Program Files\Windscribe\Windscribe.exe")),
        version: Some("2.12.7".to_owned()),
        from_registry: true,
    });

    let status = detect_windscribe(&runner, &probe, ProcessState::Running { pid: Some(4120) });
    assert!(status.installed);
    assert!(status.managed_by_winget);
    assert!(!status.update_available);
    assert_eq!(status.installed_version.as_deref(), Some("2.12.7"));
    assert_eq!(status.source, InstallSource::WinGet);
    assert_eq!(status.state_kind, InstallStateKind::InstalledUpToDate);
    assert_eq!(status.process, ProcessState::Running { pid: Some(4120) });
}

#[test]
fn winget_update_available_detection() {
    let runner = MockCommandRunner::with_responses(vec![
        (
            "winget",
            ok_out("Windscribe Windscribe.Windscribe 2.11.0 winget\n"),
        ),
        (
            "winget",
            ok_out("Windscribe Windscribe.Windscribe 2.11.0 2.12.7 winget\n"),
        ),
    ]);
    let probe = MockProbe(LocalInstallInfo::default());

    let status = detect_windscribe(&runner, &probe, ProcessState::Stopped);
    assert!(status.installed);
    assert!(status.managed_by_winget);
    assert!(status.update_available);
    assert_eq!(status.installed_version.as_deref(), Some("2.11.0"));
    assert_eq!(status.latest_version.as_deref(), Some("2.12.7"));
    assert_eq!(
        status.state_kind,
        InstallStateKind::InstalledUpdateAvailable
    );
}

#[test]
fn local_unmanaged_install_is_not_reported_missing() {
    let runner = MockCommandRunner::with_responses(vec![(
        "winget",
        err_out(1, "No installed package found matching input criteria."),
    )]);
    let probe = MockProbe(LocalInstallInfo {
        exe_path: Some(PathBuf::from(r"C:\Program Files\Windscribe\Windscribe.exe")),
        version: Some("2.10.26".to_owned()),
        from_registry: true,
    });

    let status = detect_windscribe(&runner, &probe, ProcessState::Stopped);
    assert!(status.installed);
    assert!(!status.managed_by_winget);
    assert!(!status.update_available);
    assert_eq!(status.source, InstallSource::LocalRegistry);
    assert_eq!(status.state_kind, InstallStateKind::InstalledUnmanaged);
    assert_eq!(status.installed_version.as_deref(), Some("2.10.26"));
}

#[test]
fn winget_unavailable_error_is_propagated() {
    let runner = MockCommandRunner::with_responses(vec![(
        "winget",
        Err(io::Error::new(io::ErrorKind::NotFound, "winget missing")),
    )]);
    let probe = MockProbe(LocalInstallInfo::default());

    let status = detect_windscribe(&runner, &probe, ProcessState::Unknown);
    assert!(!status.installed);
    assert!(!status.winget_available);
    assert_eq!(status.state_kind, InstallStateKind::Error);
    assert!(status.error.unwrap().contains("WinGet is unavailable"));
}

#[test]
fn background_winget_action_handles_failures() {
    let runner = MockCommandRunner::with_responses(vec![(
        "winget",
        err_out(-1978335189, "Package install aborted"),
    )]);
    let res = run_winget_package_action(&runner, false);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("failed"));
}

#[test]
fn process_detection_parses_running_stopped_and_unknown() {
    let running = MockCommandRunner::with_responses(vec![(
        "tasklist",
        ok_out("\"Windscribe.exe\",\"8452\",\"Console\",\"1\",\"64,120 K\"\r\n"),
    )]);
    assert_eq!(
        detect_windscribe_process(&running),
        ProcessState::Running { pid: Some(8452) }
    );

    let stopped = MockCommandRunner::with_responses(vec![(
        "tasklist",
        ok_out("INFO: No tasks are running which match the specified criteria.\r\n"),
    )]);
    assert_eq!(detect_windscribe_process(&stopped), ProcessState::Stopped);

    let failed = MockCommandRunner::with_responses(vec![(
        "tasklist",
        Err(io::Error::new(io::ErrorKind::NotFound, "tasklist missing")),
    )]);
    assert_eq!(detect_windscribe_process(&failed), ProcessState::Unknown);
}

#[test]
fn account_crud_validation_search_and_generators() {
    let mut accounts: Vec<WindscribeAccount> = Vec::new();
    let mut draft = AccountDraft::default();
    assert!(apply_draft(&mut accounts, &draft).is_err());

    draft.username = suggest_username();
    assert!(draft.username.starts_with("ws_"));
    draft.password = generate_strong_password(20);
    assert_eq!(draft.password.len(), 20);
    assert!(draft.password.chars().any(|c| c.is_ascii_lowercase()));
    assert!(draft.password.chars().any(|c| c.is_ascii_uppercase()));
    assert!(draft.password.chars().any(|c| c.is_ascii_digit()));
    draft.note = "Primary".to_owned();

    let created = apply_draft(&mut accounts, &draft).expect("should create");
    let created_id = match created {
        SaveResult::Created { id, .. } => id,
        _ => panic!("expected created"),
    };
    assert_eq!(accounts.len(), 1);

    // Edit existing
    let mut edit_draft = AccountDraft::default();
    edit_draft.load_from(&accounts[0]);
    edit_draft.note = "Updated note".to_owned();
    let updated = apply_draft(&mut accounts, &edit_draft).expect("should update");
    assert!(matches!(updated, SaveResult::Updated { .. }));
    assert_eq!(accounts[0].note, "Updated note");

    // Search
    assert_eq!(filter_accounts(&accounts, "updated").len(), 1);
    assert_eq!(filter_accounts(&accounts, "nonexistent").len(), 0);

    // Delete
    let removed = remove_account(&mut accounts, &created_id);
    assert!(removed.is_some());
    assert!(accounts.is_empty());
}

#[test]
fn vault_encryption_roundtrip_and_corrupted_file_handling() {
    let dir = temp_dir("vault");
    let vault_path = dir.join("accounts.vault");
    let store = VaultStore::new(vault_path.clone(), XorMockCipher(0x5A));

    let accounts = vec![WindscribeAccount {
        id: "id-1".to_owned(),
        username: "ws_testuser".to_owned(),
        password: "SuperSecretPassword!99".to_owned(),
        note: "Personal".to_owned(),
        updated_at: 1700000000,
    }];

    store.save(&accounts).expect("save vault");
    let raw = fs::read(&vault_path).unwrap();
    // Ensure plaintext password never appears on disk
    assert!(!String::from_utf8_lossy(&raw).contains("SuperSecretPassword!99"));

    let loaded = store.load().expect("load vault");
    assert_eq!(loaded, accounts);

    // Corrupt the vault and ensure load fails cleanly without deleting it
    fs::write(&vault_path, b"CORRUPTED_BYTES").unwrap();
    assert!(matches!(store.load(), Err(VaultError::Corrupted(_))));
    assert!(vault_path.exists());

    // Backup corrupted vault safely
    let backup = store.backup_corrupted().unwrap().unwrap();
    assert!(backup.exists());
    assert!(!vault_path.exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn registry_scan_backup_and_delete_enforces_verification() {
    let backup_dir = temp_dir("reg_backup");

    // 1. Scan reports Exists & counts backups
    let scan_runner = MockCommandRunner::with_responses(vec![(
        "reg.exe",
        ok_out("HKEY_CURRENT_USER\\Software\\Windscribe\\Windscribe2\r\n"),
    )]);
    let info = scan_registry_key(&scan_runner, RegistryTarget::Windscribe2, &backup_dir);
    assert_eq!(info.state, RegistryKeyState::Exists);

    // 2. Full 6-step backup & delete succeeds
    let clean_runner = MockCommandRunner::with_responses(vec![
        ("reg.exe", ok_out("EXISTS")),
        ("reg.exe", ok_out("EXPORT_OK")),
        ("reg.exe", ok_out("DELETE_OK")),
        (
            "reg.exe",
            err_out(
                1,
                "ERROR: The system was unable to find the specified registry key.",
            ),
        ),
    ]);
    let outcome =
        backup_and_delete_registry(&clean_runner, RegistryTarget::Windscribe2, &backup_dir)
            .expect("cleanup should succeed");
    assert!(outcome.backup_path.exists());
    let (count, latest) = find_existing_backups(&backup_dir, "Windscribe2");
    assert_eq!(count, 1);
    assert_eq!(latest.as_ref(), Some(&outcome.backup_path));

    // 3. Aborts deletion if backup file is empty
    let empty_backup_runner = MockCommandRunner::with_responses(vec![
        ("reg.exe", ok_out("EXISTS")),
        ("reg.exe", ok_out("EMPTY_BACKUP")),
    ]);
    let err = backup_and_delete_registry(
        &empty_backup_runner,
        RegistryTarget::NetControlPort,
        &backup_dir,
    )
    .unwrap_err();
    assert!(err.contains("empty"));

    // 4. Errors if key does not exist
    let missing_runner =
        MockCommandRunner::with_responses(vec![("reg.exe", err_out(1, "Unable to find key"))]);
    let err2 =
        backup_and_delete_registry(&missing_runner, RegistryTarget::NetControlPort, &backup_dir)
            .unwrap_err();
    assert!(err2.contains("does not exist"));

    let _ = fs::remove_dir_all(backup_dir);
}

#[test]
fn settings_persistence_roundtrip_and_invalid_path_handling() {
    let dir = temp_dir("settings");
    let path = dir.join("settings.json");

    let settings = AppSettings {
        theme: ThemeMode::Light,
        animations_enabled: false,
        refresh_interval_secs: 60,
        ..Default::default()
    };

    save_settings(&path, &settings).expect("save settings");
    let loaded = load_settings(&path).expect("load settings");
    assert_eq!(loaded, settings);

    // Invalid path error handling
    let invalid_store = VaultStore::new(PathBuf::from(""), XorMockCipher(0x11));
    assert!(matches!(
        invalid_store.save(&[]),
        Err(VaultError::InvalidPath(_))
    ));

    let _ = fs::remove_dir_all(dir);
}
