use crate::models::install::PrimaryActionKind;
use crate::models::{
    AccountDraft, AppSettings, CleanupOutcome, InstallStateKind, InstallStatus, NavSection,
    OperationKind, ProcessState, RegistryKeyInfo, RegistryTarget, ToastLevel, ToastMessage,
    VaultLoadState, WindscribeAccount, WINDSCRIBE_PACKAGE_ID,
};
use crate::services::accounts as account_service;
use crate::services::accounts::SaveResult;
use crate::services::process::detect_windscribe_process;
use crate::services::registry::{
    backup_and_delete_registry, default_backup_dir, scan_all_registry_keys,
};
use crate::services::startup::set_launch_on_startup;
use crate::services::winget::{
    detect_windscribe, launch_windscribe, run_winget_package_action, WindowsLocalInstallProbe,
};
use crate::storage::{
    default_settings_path, default_vault_path, load_settings, save_settings, VaultStore,
};
use crate::utils::command::SystemCommandRunner;
use crate::utils::time::format_now_hms;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum WorkerEvent {
    Log(String),
    Status(InstallStatus),
    Process(ProcessState),
    RegistryScanned(Vec<RegistryKeyInfo>),
    RegistryCleaned(Result<CleanupOutcome, String>),
    OperationFinished {
        kind: OperationKind,
        message: Result<String, String>,
    },
}

pub struct AppState {
    runtime: tokio::runtime::Runtime,
    tx: Sender<WorkerEvent>,
    rx: Receiver<WorkerEvent>,
    pub active_operation: Option<OperationKind>,
    pub active_section: NavSection,
    pub drawer_open: bool,
    pub status: InstallStatus,
    pub accounts: Vec<WindscribeAccount>,
    pub vault_state: VaultLoadState,
    pub account_draft: AccountDraft,
    pub editor_open: bool,
    pub account_search: String,
    pub revealed_accounts: HashSet<String>,
    pub pending_delete_account: Option<String>,
    pub registry_keys: Vec<RegistryKeyInfo>,
    pub pending_registry_clean: Option<RegistryTarget>,
    pub settings: AppSettings,
    pub tools_logs_expanded: bool,
    pub logs: Vec<String>,
    pub toasts: Vec<ToastMessage>,
    next_toast_id: u64,
    last_auto_refresh: Instant,
    pub vault_path: PathBuf,
    pub settings_path: PathBuf,
    pub backup_dir: PathBuf,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create Tokio background runtime");

        let (tx, rx) = mpsc::channel();
        let vault_path = default_vault_path();
        let settings_path = default_settings_path();
        let backup_dir = default_backup_dir();
        let settings = load_settings(&settings_path).unwrap_or_default();

        let registry_keys = RegistryTarget::ALL
            .iter()
            .map(|&t| RegistryKeyInfo::new(t))
            .collect();

        let mut state = Self {
            runtime,
            tx,
            rx,
            active_operation: None,
            active_section: NavSection::Home,
            drawer_open: false,
            status: InstallStatus {
                state_kind: InstallStateKind::Checking,
                summary: "Checking Windscribe installation & process state…".to_owned(),
                ..Default::default()
            },
            accounts: Vec::new(),
            vault_state: VaultLoadState::Loaded,
            account_draft: AccountDraft::default(),
            editor_open: false,
            account_search: String::new(),
            revealed_accounts: HashSet::new(),
            pending_delete_account: None,
            registry_keys,
            pending_registry_clean: None,
            settings,
            tools_logs_expanded: false,
            logs: Vec::new(),
            toasts: Vec::new(),
            next_toast_id: 1,
            last_auto_refresh: Instant::now(),
            vault_path,
            settings_path,
            backup_dir,
        };

        state.push_log("Windscribe Hub initialized.");
        state.load_vault();
        state.refresh_all_status();
        state.scan_registry();
        state
    }

    pub fn is_busy(&self) -> bool {
        self.active_operation.is_some()
    }

    pub fn push_log(&mut self, msg: impl Into<String>) {
        let now = format_now_hms();
        self.logs.push(format!("[{now}] {}", msg.into()));
        if self.logs.len() > 200 {
            let excess = self.logs.len() - 200;
            self.logs.drain(0..excess);
        }
    }

    pub fn push_toast(
        &mut self,
        level: ToastLevel,
        title: impl Into<String>,
        detail: Option<String>,
    ) {
        if !self.settings.show_toasts {
            return;
        }
        let id = self.next_toast_id;
        self.next_toast_id = self.next_toast_id.saturating_add(1);
        self.toasts.push(ToastMessage {
            id,
            level,
            title: title.into(),
            detail,
            created_at: Instant::now(),
            duration_secs: 4.0,
        });
    }

    pub fn poll_background_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                WorkerEvent::Log(line) => self.push_log(line),
                WorkerEvent::Status(status) => {
                    self.status = status;
                }
                WorkerEvent::Process(proc_state) => {
                    self.status.process = proc_state;
                }
                WorkerEvent::RegistryScanned(keys) => {
                    self.registry_keys = keys;
                }
                WorkerEvent::RegistryCleaned(result) => match result {
                    Ok(outcome) => {
                        let msg = format!(
                            "Backed up {} to {} and removed registry key.",
                            outcome.target.display_name(),
                            outcome.backup_path.display()
                        );
                        self.push_log(&msg);
                        self.push_toast(
                            ToastLevel::Success,
                            format!("Cleaned {}", outcome.target.display_name()),
                            Some(format!("Backup saved: {}", outcome.backup_path.display())),
                        );
                        self.scan_registry_after_cleanup();
                    }
                    Err(err) => {
                        self.push_log(format!("Registry cleanup error: {err}"));
                        self.push_toast(ToastLevel::Error, "Registry cleanup failed", Some(err));
                    }
                },
                WorkerEvent::OperationFinished { kind, message } => {
                    self.active_operation = None;
                    self.last_auto_refresh = Instant::now();
                    match message {
                        Ok(info) => {
                            if !info.is_empty() {
                                self.push_log(&info);
                            }
                            match kind {
                                OperationKind::Install => self.push_toast(
                                    ToastLevel::Success,
                                    "Windscribe installed",
                                    None,
                                ),
                                OperationKind::Update => {
                                    self.push_toast(ToastLevel::Success, "Windscribe updated", None)
                                }
                                OperationKind::Launch => self.push_toast(
                                    ToastLevel::Success,
                                    "Windscribe launched",
                                    None,
                                ),
                                _ => {}
                            }
                        }
                        Err(err) => {
                            self.push_log(format!("{} failed: {err}", kind.label()));
                            self.push_toast(ToastLevel::Error, "Operation failed", Some(err));
                        }
                    }
                }
            }
        }

        if self.settings.auto_refresh
            && !self.is_busy()
            && self.last_auto_refresh.elapsed()
                >= Duration::from_secs(self.settings.refresh_interval_secs)
        {
            self.refresh_process_only();
        }
    }

    fn spawn_task<F>(&mut self, kind: OperationKind, job: F)
    where
        F: FnOnce(Sender<WorkerEvent>) -> Result<String, String> + Send + 'static,
    {
        if self.is_busy() {
            return;
        }
        self.active_operation = Some(kind);
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let outcome = job(tx.clone());
            let _ = tx.send(WorkerEvent::OperationFinished {
                kind,
                message: outcome,
            });
        });
    }

    pub fn refresh_all_status(&mut self) {
        self.push_log("Checking WinGet, Windows registry, install paths, and Windscribe.exe…");
        self.spawn_task(OperationKind::RefreshStatus, |tx| {
            let runner = SystemCommandRunner;
            let probe = WindowsLocalInstallProbe;
            let process = detect_windscribe_process(&runner);
            let status = detect_windscribe(&runner, &probe, process);
            let _ = tx.send(WorkerEvent::Status(status));
            Ok("Status check completed.".to_owned())
        });
    }

    pub fn refresh_process_only(&mut self) {
        self.last_auto_refresh = Instant::now();
        self.spawn_task(OperationKind::RefreshProcess, |tx| {
            let runner = SystemCommandRunner;
            let proc_state = detect_windscribe_process(&runner);
            let _ = tx.send(WorkerEvent::Process(proc_state));
            Ok(String::new())
        });
    }

    pub fn trigger_primary_action(&mut self) {
        match self.status.primary_action() {
            PrimaryActionKind::Install => self.install_windscribe(),
            PrimaryActionKind::Update => self.update_windscribe(),
            PrimaryActionKind::Open => self.launch_windscribe(),
            PrimaryActionKind::RetryCheck => self.refresh_all_status(),
        }
    }

    pub fn install_windscribe(&mut self) {
        self.push_log(format!(
            "Starting WinGet install for {WINDSCRIBE_PACKAGE_ID}…"
        ));
        self.spawn_task(OperationKind::Install, |tx| {
            let runner = SystemCommandRunner;
            let probe = WindowsLocalInstallProbe;
            let res = run_winget_package_action(&runner, false);
            let proc_state = detect_windscribe_process(&runner);
            let _ = tx.send(WorkerEvent::Status(detect_windscribe(
                &runner, &probe, proc_state,
            )));
            res
        });
    }

    pub fn update_windscribe(&mut self) {
        self.push_log(format!(
            "Starting WinGet upgrade for {WINDSCRIBE_PACKAGE_ID}…"
        ));
        self.spawn_task(OperationKind::Update, |tx| {
            let runner = SystemCommandRunner;
            let probe = WindowsLocalInstallProbe;
            let res = run_winget_package_action(&runner, true);
            let proc_state = detect_windscribe_process(&runner);
            let _ = tx.send(WorkerEvent::Status(detect_windscribe(
                &runner, &probe, proc_state,
            )));
            res
        });
    }

    pub fn launch_windscribe(&mut self) {
        self.push_log("Launching Windscribe desktop client…");
        self.spawn_task(OperationKind::Launch, |tx| {
            let runner = SystemCommandRunner;
            let probe = WindowsLocalInstallProbe;
            launch_windscribe(&probe)?;
            std::thread::sleep(Duration::from_millis(1500));
            let proc_state = detect_windscribe_process(&runner);
            let _ = tx.send(WorkerEvent::Process(proc_state));
            Ok("Windscribe launch command sent.".to_owned())
        });
    }

    pub fn scan_registry(&mut self) {
        let backup_dir = self.backup_dir.clone();
        self.spawn_task(OperationKind::ScanRegistry, move |tx| {
            let runner = SystemCommandRunner;
            let scanned = scan_all_registry_keys(&runner, &backup_dir);
            let _ = tx.send(WorkerEvent::RegistryScanned(scanned));
            Ok("Registry scan completed.".to_owned())
        });
    }

    fn scan_registry_after_cleanup(&mut self) {
        let backup_dir = self.backup_dir.clone();
        let tx = self.tx.clone();
        self.runtime.spawn_blocking(move || {
            let runner = SystemCommandRunner;
            let scanned = scan_all_registry_keys(&runner, &backup_dir);
            let _ = tx.send(WorkerEvent::RegistryScanned(scanned));
        });
    }

    pub fn execute_registry_cleanup(&mut self, target: RegistryTarget) {
        let backup_dir = self.backup_dir.clone();
        self.push_log(format!(
            "Starting verified backup & cleanup for {}…",
            target.registry_path()
        ));
        self.spawn_task(OperationKind::CleanRegistry, move |tx| {
            let runner = SystemCommandRunner;
            let res = backup_and_delete_registry(&runner, target, &backup_dir);
            let _ = tx.send(WorkerEvent::RegistryCleaned(res.clone()));
            res.map(|_| String::new())
        });
    }

    // --- Account Vault Actions ---

    pub fn load_vault(&mut self) {
        let store = VaultStore::system();
        match store.load() {
            Ok(accounts) => {
                self.accounts = accounts;
                self.vault_state = VaultLoadState::Loaded;
                self.push_log(format!(
                    "Loaded {} encrypted account(s) from DPAPI vault.",
                    self.accounts.len()
                ));
            }
            Err(err) => {
                self.vault_state = VaultLoadState::Corrupted {
                    error: err.to_string(),
                };
                self.push_log(format!("Account vault could not be unlocked: {err}"));
            }
        }
    }

    pub fn backup_and_reset_corrupted_vault(&mut self) {
        let store = VaultStore::system();
        match store.backup_corrupted() {
            Ok(backup) => {
                self.accounts.clear();
                self.vault_state = VaultLoadState::Loaded;
                let detail = backup
                    .map(|p| format!("Corrupted file preserved at {}", p.display()))
                    .unwrap_or_else(|| "Vault reset.".to_owned());
                self.push_log(&detail);
                self.push_toast(ToastLevel::Warning, "Vault backed up & reset", Some(detail));
            }
            Err(err) => {
                self.push_toast(
                    ToastLevel::Error,
                    "Vault reset failed",
                    Some(err.to_string()),
                );
            }
        }
    }

    pub fn save_account_draft(&mut self) {
        match account_service::apply_draft(&mut self.accounts, &self.account_draft) {
            Ok(outcome) => {
                let store = VaultStore::system();
                match store.save(&self.accounts) {
                    Ok(()) => {
                        let (verb, username) = match outcome {
                            SaveResult::Created { username, .. } => ("Saved", username),
                            SaveResult::Updated { username, .. } => ("Updated", username),
                        };
                        self.account_draft.clear();
                        self.editor_open = false;
                        self.push_log(format!("{verb} encrypted account for {username}."));
                        self.push_toast(
                            ToastLevel::Success,
                            format!("{verb} {username}"),
                            Some("Protected with Windows DPAPI".to_owned()),
                        );
                    }
                    Err(err) => {
                        self.account_draft.validation_error = Some(err.to_string());
                        self.push_toast(
                            ToastLevel::Error,
                            "Could not write vault",
                            Some(err.to_string()),
                        );
                    }
                }
            }
            Err(msg) => {
                self.account_draft.validation_error = Some(msg);
            }
        }
    }

    pub fn confirm_delete_account(&mut self, id: &str) {
        if let Some(removed) = account_service::remove_account(&mut self.accounts, id) {
            self.revealed_accounts.remove(id);
            let store = VaultStore::system();
            match store.save(&self.accounts) {
                Ok(()) => {
                    self.push_log(format!("Removed saved account {}.", removed.username));
                    self.push_toast(
                        ToastLevel::Info,
                        format!("Deleted {}", removed.username),
                        None,
                    );
                }
                Err(err) => {
                    self.push_toast(
                        ToastLevel::Error,
                        "Failed to update vault",
                        Some(err.to_string()),
                    );
                }
            }
        }
    }

    // --- Settings Actions ---

    pub fn persist_settings(&mut self) {
        if let Err(err) = save_settings(&self.settings_path, &self.settings) {
            self.push_log(format!("Failed to save settings: {err}"));
            self.push_toast(ToastLevel::Error, "Settings save failed", Some(err));
        }
    }

    pub fn toggle_startup(&mut self, enabled: bool) {
        match set_launch_on_startup(enabled) {
            Ok(()) => {
                self.settings.launch_on_startup = enabled;
                self.persist_settings();
                self.push_toast(
                    ToastLevel::Success,
                    if enabled {
                        "Windows startup enabled"
                    } else {
                        "Windows startup disabled"
                    },
                    None,
                );
            }
            Err(err) => {
                self.settings.launch_on_startup = !enabled;
                self.push_toast(
                    ToastLevel::Warning,
                    "Startup setting unavailable",
                    Some(err),
                );
            }
        }
    }

    pub fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                if self.pending_registry_clean.is_some() {
                    self.pending_registry_clean = None;
                } else if self.pending_delete_account.is_some() {
                    self.pending_delete_account = None;
                } else if self.drawer_open {
                    self.drawer_open = false;
                }
            }
            if i.key_pressed(egui::Key::F5) && !self.is_busy() {
                self.refresh_all_status();
            }
            if i.modifiers.alt {
                let keys = [
                    (egui::Key::Num1, NavSection::Home),
                    (egui::Key::Num2, NavSection::Accounts),
                    (egui::Key::Num3, NavSection::Tools),
                    (egui::Key::Num4, NavSection::Registry),
                    (egui::Key::Num5, NavSection::Settings),
                ];
                for (key, sec) in keys {
                    if i.key_pressed(key) {
                        self.active_section = sec;
                        self.drawer_open = false;
                    }
                }
            }
        });
    }
}
