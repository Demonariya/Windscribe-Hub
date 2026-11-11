#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Stroke, Vec2};
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
use winreg::{
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    RegKey,
};

const APP_TITLE: &str = "Windscribe Pro";
const APP_VERSION: &str = "0.0.1";
const PACKAGE_ID: &str = "Windscribe.Windscribe";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const BG: Color32 = Color32::from_rgb(7, 17, 31);
const SIDEBAR: Color32 = Color32::from_rgb(9, 24, 43);
const PANEL: Color32 = Color32::from_rgb(13, 32, 56);
const CARD: Color32 = Color32::from_rgb(16, 36, 61);
const CARD_ALT: Color32 = Color32::from_rgb(12, 29, 49);
const BORDER: Color32 = Color32::from_rgb(32, 60, 96);
const BLUE: Color32 = Color32::from_rgb(8, 124, 255);
const PURPLE: Color32 = Color32::from_rgb(112, 74, 232);
const GREEN: Color32 = Color32::from_rgb(25, 196, 125);
const AMBER: Color32 = Color32::from_rgb(242, 185, 75);
const RED: Color32 = Color32::from_rgb(228, 93, 115);
const TEXT: Color32 = Color32::from_rgb(244, 247, 253);
const MUTED: Color32 = Color32::from_rgb(165, 181, 206);
const QUIET: Color32 = Color32::from_rgb(113, 133, 165);

#[derive(Clone, Debug, Default)]
struct InstallStatus {
    installed: bool,
    managed_by_winget: bool,
    update_available: bool,
    running: bool,
    installed_version: String,
    detail: String,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WindscribeAccount {
    id: String,
    username: String,
    password: String,
}

#[derive(Debug)]
enum WorkerEvent {
    Log(String),
    Status(InstallStatus),
    Finished,
}

struct WindscribeApp {
    tx: Sender<WorkerEvent>,
    rx: Receiver<WorkerEvent>,
    busy: bool,
    status: InstallStatus,
    username_input: String,
    password_input: String,
    password_visible: bool,
    accounts: Vec<WindscribeAccount>,
    selected_account: Option<String>,
    revealed_accounts: HashSet<String>,
    logs: Vec<String>,
    confirm_registry: Option<&'static str>,
    pending_delete_account: Option<String>,
    last_status_check: Option<SystemTime>,
}

impl WindscribeApp {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            tx,
            rx,
            busy: false,
            status: InstallStatus::default(),
            username_input: String::new(),
            password_input: String::new(),
            password_visible: false,
            accounts: Vec::new(),
            selected_account: None,
            revealed_accounts: HashSet::new(),
            logs: Vec::new(),
            confirm_registry: None,
            pending_delete_account: None,
            last_status_check: None,
        };
        app.load_vault();
        app.push_log("Windscribe Pro initialized.");
        app.refresh_status();
        app
    }

    fn push_log(&mut self, message: impl Into<String>) {
        let now = chrono_time();
        self.logs.push(format!("[{now}]  {}", message.into()));
        if self.logs.len() > 150 {
            let excess = self.logs.len() - 150;
            self.logs.drain(0..excess);
        }
    }

    fn spawn_worker<F>(&mut self, task: F)
    where
        F: FnOnce(Sender<WorkerEvent>) + Send + 'static,
    {
        if self.busy {
            return;
        }
        self.busy = true;
        let sender = self.tx.clone();
        thread::spawn(move || {
            task(sender.clone());
            let _ = sender.send(WorkerEvent::Finished);
        });
    }

    fn refresh_status(&mut self) {
        let sender = self.tx.clone();
        self.push_log("Checking Windscribe installation, update availability and process state…");
        self.spawn_worker(move |tx| {
            let status = detect_windscribe();
            let _ = tx.send(WorkerEvent::Status(status));
            let _ = sender.send(WorkerEvent::Log("Status check completed.".to_owned()));
        });
    }

    fn poll_worker_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                WorkerEvent::Log(line) => self.push_log(line),
                WorkerEvent::Status(status) => {
                    self.status = status;
                    self.last_status_check = Some(SystemTime::now());
                }
                WorkerEvent::Finished => self.busy = false,
            }
        }
    }

    fn install_or_update(&mut self) {
        if self.busy {
            return;
        }
        let is_update = self.status.installed && self.status.managed_by_winget;
        let action = if is_update { "upgrade" } else { "install" };
        self.push_log(format!("Starting WinGet {action} for {PACKAGE_ID}…"));
        self.spawn_worker(move |tx| {
            let result = run_hidden(
                "winget",
                &[
                    action,
                    "--id",
                    PACKAGE_ID,
                    "--exact",
                    "--source",
                    "winget",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                    "--disable-interactivity",
                ],
            );
            match result {
                Ok(output) => {
                    let combined = combined_output(&output.stdout, &output.stderr);
                    if !combined.trim().is_empty() {
                        let _ = tx.send(WorkerEvent::Log(combined));
                    }
                    if output.status.success() {
                        let _ = tx.send(WorkerEvent::Log(format!("WinGet {action} completed successfully.")));
                    } else {
                        let _ = tx.send(WorkerEvent::Log(format!(
                            "WinGet {action} failed with exit code {:?}.",
                            output.status.code()
                        )));
                    }
                }
                Err(error) => {
                    let _ = tx.send(WorkerEvent::Log(format!("WinGet could not start: {error}")));
                }
            }
            let status = detect_windscribe();
            let _ = tx.send(WorkerEvent::Status(status));
        });
    }

    fn open_windscribe(&mut self) {
        if self.busy || !self.status.installed {
            return;
        }
        self.push_log("Opening Windscribe…");
        self.spawn_worker(|tx| match launch_windscribe() {
            Ok(()) => {
                let _ = tx.send(WorkerEvent::Log("Windscribe launch request sent.".to_owned()));
                thread::sleep(Duration::from_secs(2));
                let _ = tx.send(WorkerEvent::Status(detect_windscribe()));
            }
            Err(error) => {
                let _ = tx.send(WorkerEvent::Log(format!("Could not open Windscribe: {error}")));
            }
        });
    }

    fn run_registry_cleanup(&mut self, key_name: &'static str) {
        let key_path = registry_key_path(key_name);
        self.push_log(format!("Preparing registry cleanup for {key_path}…"));
        self.spawn_worker(move |tx| match backup_and_delete_registry(key_name) {
            Ok(backup) => {
                let _ = tx.send(WorkerEvent::Log(format!(
                    "Registry key deleted. Backup saved to {}",
                    backup.display()
                )));
            }
            Err(error) => {
                let _ = tx.send(WorkerEvent::Log(format!(
                    "Registry cleanup failed for {key_path}: {error}"
                )));
            }
        });
    }

    fn vault_path() -> PathBuf {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("WindscribeHub")
            .join("accounts.vault")
    }

    fn load_vault(&mut self) {
        let path = Self::vault_path();
        if !path.exists() {
            return;
        }
        match fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|encrypted| dpapi_unprotect(&encrypted).map_err(|error| error.to_string()))
            .and_then(|plain| serde_json::from_slice::<Vec<WindscribeAccount>>(&plain).map_err(|error| error.to_string()))
        {
            Ok(accounts) => {
                self.accounts = accounts
                    .into_iter()
                    .filter(|account| !account.username.trim().is_empty())
                    .collect();
                self.push_log(format!(
                    "Loaded {} encrypted Windscribe account(s).",
                    self.accounts.len()
                ));
            }
            Err(error) => {
                self.push_log(format!("Could not open the saved account vault: {error}"));
            }
        }
    }

    fn save_vault(&self) -> Result<(), String> {
        let path = Self::vault_path();
        let parent = path
            .parent()
            .ok_or_else(|| "The vault directory could not be resolved.".to_owned())?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let plain = serde_json::to_vec(&self.accounts).map_err(|error| error.to_string())?;
        let encrypted = dpapi_protect(&plain).map_err(|error| error.to_string())?;
        let temp = path.with_extension("vault.tmp");
        fs::write(&temp, encrypted).map_err(|error| error.to_string())?;
        replace_file_atomically(&temp, &path)?;
        Ok(())
    }

    fn save_account(&mut self) {
        let username = self.username_input.trim().to_owned();
        let password = self.password_input.clone();
        if username.is_empty() || password.is_empty() {
            self.push_log("Enter both a Windscribe username and password before saving.");
            return;
        }

        if let Some(existing) = self
            .accounts
            .iter_mut()
            .find(|account| account.username.eq_ignore_ascii_case(&username))
        {
            existing.password = password;
            self.selected_account = Some(existing.id.clone());
            self.push_log(format!("Updated saved Windscribe credentials for {username}."));
        } else {
            let account = WindscribeAccount {
                id: Uuid::new_v4().to_string(),
                username: username.clone(),
                password,
            };
            self.selected_account = Some(account.id.clone());
            self.accounts.insert(0, account);
            self.push_log(format!("Saved Windscribe account {username}."));
        }

        match self.save_vault() {
            Ok(()) => {
                self.password_input.clear();
                self.password_visible = false;
                self.push_log("Encrypted account vault updated.");
            }
            Err(error) => self.push_log(format!("Could not save the account vault: {error}")),
        }
    }

    fn load_selected_account(&mut self) {
        let Some(id) = self.selected_account.as_deref() else {
            self.push_log("Select an account in the list first.");
            return;
        };
        if let Some(account) = self.accounts.iter().find(|item| item.id == id) {
            self.username_input = account.username.clone();
            self.password_input = account.password.clone();
            self.password_visible = true;
            self.push_log(format!("Loaded {} into the account editor.", account.username));
        }
    }

    fn delete_account(&mut self, id: &str) {
        let Some(index) = self.accounts.iter().position(|account| account.id == id) else {
            return;
        };
        let username = self.accounts[index].username.clone();
        self.accounts.remove(index);
        self.revealed_accounts.remove(id);
        if self.selected_account.as_deref() == Some(id) {
            self.selected_account = None;
        }
        match self.save_vault() {
            Ok(()) => self.push_log(format!("Removed saved Windscribe account {username}.")),
            Err(error) => self.push_log(format!("Could not update the account vault: {error}")),
        }
    }

    fn generate_username(&mut self) {
        let mut rng = rand::thread_rng();
        let suffix: String = (0..9)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect::<String>()
            .to_ascii_lowercase();
        self.username_input = format!("ws_{suffix}");
        self.push_log("Generated a username suggestion. Register it with Windscribe before using it.");
    }

    fn generate_password(&mut self) {
        let mut rng = rand::thread_rng();
        let groups = [
            "abcdefghijkmnopqrstuvwxyz",
            "ABCDEFGHJKLMNPQRSTUVWXYZ",
            "23456789",
            "!@#$%*-_=+?",
        ];
        let mut chars = Vec::with_capacity(20);
        for group in groups {
            let index = rng.gen_range(0..group.len());
            chars.push(group.as_bytes()[index] as char);
        }
        let alphabet = groups.join("");
        for _ in 0..16 {
            let index = rng.gen_range(0..alphabet.len());
            chars.push(alphabet.as_bytes()[index] as char);
        }
        chars.shuffle(&mut rng);
        self.password_input = chars.into_iter().collect();
        self.password_visible = true;
        self.push_log("Generated a 20-character password.");
    }

    fn selected_account(&self) -> Option<&WindscribeAccount> {
        let id = self.selected_account.as_deref()?;
        self.accounts.iter().find(|account| account.id == id)
    }

    fn copy_username(&self, ctx: &egui::Context) {
        if let Some(account) = self.selected_account() {
            ctx.copy_text(account.username.clone());
        }
    }

    fn copy_password(&self, ctx: &egui::Context) {
        if let Some(account) = self.selected_account() {
            ctx.copy_text(account.password.clone());
        }
    }

    fn render_status_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let (title, color) = if !self.status.installed {
                        ("Windscribe is not installed", AMBER)
                    } else if self.status.update_available {
                        ("Windscribe update available", AMBER)
                    } else {
                        ("Windscribe is installed", GREEN)
                    };
                    ui.label(RichText::new(title).size(19.0).strong().color(color));
                    ui.add_space(3.0);
                    ui.label(RichText::new(&self.status.detail).size(11.5).color(MUTED));
                    ui.add_space(5.0);
                    let process_text = if self.status.running {
                        "●  APP PROCESS RUNNING"
                    } else {
                        "○  APP PROCESS NOT RUNNING"
                    };
                    ui.label(
                        RichText::new(process_text)
                            .size(10.5)
                            .strong()
                            .color(if self.status.running { GREEN } else { QUIET }),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if action_button(ui, "Refresh Status", Color32::from_rgb(38, 62, 96), false).clicked()
                        && !self.busy
                    {
                        self.refresh_status();
                    }
                });
            });
        });
    }

    fn render_setup_cards(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                ui.label(RichText::new("↓  WINDSCRIBE SETUP").size(14.0).strong().color(TEXT));
                ui.add_space(5.0);
                ui.label(RichText::new("Install, update or launch the official desktop app.").size(11.0).color(MUTED));
                ui.add_space(14.0);

                let (label, color, enabled) = if self.busy {
                    ("Working…", QUIET, false)
                } else if !self.status.installed && self.status.error.as_deref().is_some_and(|error| error.starts_with("WinGet is unavailable")) {
                    ("WinGet is unavailable", QUIET, false)
                } else if !self.status.installed {
                    ("Install Windscribe", BLUE, true)
                } else if self.status.update_available {
                    ("Update Windscribe", PURPLE, true)
                } else if !self.status.managed_by_winget {
                    ("Update check unavailable", QUIET, false)
                } else {
                    ("Windscribe is up to date", Color32::from_rgb(35, 85, 63), false)
                };
                if action_button(ui, label, color, enabled).clicked() {
                    self.install_or_update();
                }
                ui.add_space(7.0);
                if action_button(
                    ui,
                    "Open Windscribe",
                    GREEN,
                    self.status.installed && !self.busy,
                )
                .clicked()
                {
                    self.open_windscribe();
                }
                if self.status.installed && !self.status.managed_by_winget {
                    ui.add_space(5.0);
                    ui.label(RichText::new("This installation was detected locally; WinGet cannot manage its updates.").size(10.0).color(QUIET));
                }
            });

            card(&mut columns[1], |ui| {
                ui.label(RichText::new("⚙  REGISTRY CLEANUP").size(14.0).strong().color(TEXT));
                ui.add_space(5.0);
                ui.label(RichText::new("A .reg backup is exported before each deletion.").size(11.0).color(MUTED));
                ui.add_space(14.0);
                if action_button(ui, "Run Command 1", PURPLE, !self.busy).clicked() {
                    self.confirm_registry = Some("one");
                }
                ui.add_space(7.0);
                if action_button(ui, "Run Command 2", PURPLE, !self.busy).clicked() {
                    self.confirm_registry = Some("two");
                }
                ui.add_space(8.0);
                ui.label(RichText::new("1  NetControlPort").size(10.0).color(QUIET));
                ui.label(RichText::new("2  Windscribe2").size(10.0).color(QUIET));
            });
        });
    }

    fn render_accounts(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔐  WINDSCRIBE ACCOUNT").size(14.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} SAVED", self.accounts.len())).size(9.5).strong().color(MUTED));
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Manage saved Windscribe logins. Passwords are encrypted for this Windows user.").size(10.5).color(MUTED));
            ui.add_space(13.0);

            ui.columns(2, |columns| {
                let left = &mut columns[0];
                left.label(RichText::new("ACCOUNT DETAILS").size(10.0).strong().color(QUIET));
                left.add_space(5.0);
                left.label(RichText::new("Username").size(11.0).strong().color(TEXT));
                left.add(egui::TextEdit::singleline(&mut self.username_input).hint_text("Windscribe username").desired_width(f32::INFINITY));
                left.add_space(7.0);
                left.label(RichText::new("Password").size(11.0).strong().color(TEXT));
                left.horizontal(|ui| {
                    let mut password = egui::TextEdit::singleline(&mut self.password_input)
                        .hint_text("Windscribe password")
                        .desired_width(ui.available_width() - 72.0);
                    password = password.password(!self.password_visible);
                    ui.add(password);
                    let show_label = if self.password_visible { "Hide" } else { "Show" };
                    if ui.add(egui::Button::new(show_label).fill(Color32::from_rgb(38, 62, 96))).clicked() {
                        self.password_visible = !self.password_visible;
                    }
                });
                left.add_space(8.0);
                left.horizontal_wrapped(|ui| {
                    if small_button(ui, "Suggest Username", Color32::from_rgb(30, 85, 144)).clicked() {
                        self.generate_username();
                    }
                    if small_button(ui, "Generate Password", PURPLE).clicked() {
                        self.generate_password();
                    }
                });
                left.add_space(7.0);
                if action_button(ui, "Save Windscribe Account", GREEN, true).clicked() {
                    self.save_account();
                }
                left.add_space(6.0);
                left.label(RichText::new("A generated username is only a suggestion; it is not registered with Windscribe.").size(9.5).color(QUIET));

                let right = &mut columns[1];
                right.label(RichText::new("SAVED WINDSCRIBE ACCOUNTS").size(10.0).strong().color(QUIET));
                right.add_space(5.0);
                egui::Frame::none()
                    .fill(CARD_ALT)
                    .rounding(egui::Rounding::same(8.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(right, |ui| {
                        egui::ScrollArea::vertical().max_height(202.0).show(ui, |ui| {
                            if self.accounts.is_empty() {
                                ui.add_space(23.0);
                                ui.centered_and_justified(|ui| {
                                    ui.label(RichText::new("No Windscribe accounts saved yet.").size(11.0).color(QUIET));
                                });
                                ui.add_space(23.0);
                            }
                            let mut delete_after: Option<String> = None;
                            let mut select_after: Option<String> = None;
                            for account in &self.accounts {
                                let selected = self.selected_account.as_deref() == Some(account.id.as_str());
                                let is_revealed = self.revealed_accounts.contains(&account.id);
                                let row_fill = if selected { Color32::from_rgb(22, 54, 88) } else { CARD_ALT };
                                egui::Frame::none()
                                    .fill(row_fill)
                                    .rounding(egui::Rounding::same(6.0))
                                    .inner_margin(egui::Margin::symmetric(7.0, 6.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(&account.username).size(11.0).strong().color(if selected { Color32::from_rgb(32, 185, 244) } else { TEXT }));
                                                let hidden = "•".repeat(account.password.chars().count().clamp(8, 16));
                                                let password = if is_revealed { account.password.as_str() } else { hidden.as_str() };
                                                ui.label(RichText::new(password).size(10.0).color(MUTED));
                                            });
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                if ui.add(egui::Button::new("×").fill(Color32::from_rgb(92, 39, 57))).clicked() {
                                                    delete_after = Some(account.id.clone());
                                                }
                                                let reveal_text = if is_revealed { "Hide" } else { "Show" };
                                                if ui.add(egui::Button::new(reveal_text).fill(Color32::from_rgb(38, 62, 96))).clicked() {
                                                    if is_revealed {
                                                        self.revealed_accounts.remove(&account.id);
                                                    } else {
                                                        self.revealed_accounts.insert(account.id.clone());
                                                    }
                                                }
                                                if ui.add(egui::Button::new(if selected { "Selected" } else { "Select" }).fill(if selected { BLUE } else { Color32::from_rgb(38, 62, 96) })).clicked() {
                                                    select_after = Some(account.id.clone());
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(4.0);
                            }
                            if let Some(id) = select_after {
                                self.selected_account = Some(id);
                            }
                            if let Some(id) = delete_after {
                                self.pending_delete_account = Some(id);
                            }
                        });
                    });

                right.add_space(8.0);
                right.horizontal_wrapped(|ui| {
                    let has_selection = self.selected_account.is_some();
                    if ui.add_enabled(has_selection, egui::Button::new("Load").fill(Color32::from_rgb(38, 62, 96))).clicked() {
                        self.load_selected_account();
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Copy User").fill(Color32::from_rgb(38, 62, 96))).clicked() {
                        self.copy_username(ctx);
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Copy Pass").fill(Color32::from_rgb(38, 62, 96))).clicked() {
                        self.copy_password(ctx);
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Delete").fill(Color32::from_rgb(92, 39, 57))).clicked() {
                        self.pending_delete_account = self.selected_account.clone();
                    }
                });
                right.add_space(3.0);
                right.label(RichText::new("Select an account to load or copy its credentials.").size(9.5).color(QUIET));
            });
        });
    }

    fn render_activity(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("▤  ACTIVITY LOG").size(13.0).strong().color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if small_button(ui, "Clear", Color32::from_rgb(38, 62, 96)).clicked() {
                        self.logs.clear();
                    }
                });
            });
            ui.add_space(7.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(5, 13, 23))
                .rounding(egui::Rounding::same(7.0))
                .inner_margin(egui::Margin::same(10.0))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(135.0).stick_to_bottom(true).show(ui, |ui| {
                        if self.logs.is_empty() {
                            ui.label(RichText::new("No activity yet.").size(10.0).color(QUIET));
                        }
                        for line in &self.logs {
                            ui.label(RichText::new(line).font(FontId::monospace(10.0)).color(MUTED));
                        }
                    });
                });
        });
    }

    fn render_registry_confirmation(&mut self, ctx: &egui::Context) {
        let Some(which) = self.confirm_registry else {
            return;
        };
        let path = registry_key_path(which);
        egui::Window::new("Confirm registry cleanup")
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(430.0, 170.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(RichText::new("This operation changes the Windows registry.").strong().color(AMBER));
                ui.add_space(6.0);
                ui.label(RichText::new(path).monospace().color(TEXT));
                ui.add_space(8.0);
                ui.label(RichText::new("A .reg backup must succeed before the key is deleted.").size(10.5).color(MUTED));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if action_button(ui, "Back up and delete", PURPLE, !self.busy).clicked() {
                        self.confirm_registry = None;
                        self.run_registry_cleanup(which);
                    }
                    if action_button(ui, "Cancel", Color32::from_rgb(38, 62, 96), true).clicked() {
                        self.confirm_registry = None;
                    }
                });
            });
    }

    fn render_account_delete_confirmation(&mut self, ctx: &egui::Context) {
        let Some(id) = self.pending_delete_account.clone() else {
            return;
        };
        let username = self.accounts.iter().find(|account| account.id == id)
            .map(|account| account.username.clone()).unwrap_or_default();
        egui::Window::new("Delete saved account")
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(360.0, 130.0))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("Delete the saved Windscribe login for {username}?"));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if action_button(ui, "Delete account", RED, true).clicked() {
                        self.pending_delete_account = None;
                        self.delete_account(&id);
                    }
                    if action_button(ui, "Cancel", Color32::from_rgb(38, 62, 96), true).clicked() {
                        self.pending_delete_account = None;
                    }
                });
            });
    }
}

impl eframe::App for WindscribeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_worker_events();
        ctx.request_repaint_after(Duration::from_millis(120));

        egui::SidePanel::left("navigation")
            .exact_width(228.0)
            .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(egui::Margin::same(18.0)))
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 13.0, Color32::from_rgb(18, 58, 103));
                    ui.painter().rect_stroke(rect, 13.0, Stroke::new(1.0, BLUE));
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "W",
                                      FontId::proportional(28.0), Color32::from_rgb(32, 185, 244));
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("WINDSCRIBE").size(15.0).strong().color(TEXT));
                        ui.label(RichText::new("PRO  •  DESKTOP HUB").size(9.0).strong().color(Color32::from_rgb(32, 185, 244)));
                    });
                });
                ui.add_space(25.0);
                nav_item(ui, "⌂", "Overview", true);
                nav_item(ui, "◈", "Windscribe Account", false);
                nav_item(ui, "⚙", "Registry Tools", false);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(RichText::new(format!("v{APP_VERSION}  •  Demonariya")).size(9.0).color(QUIET));
                    ui.add_space(14.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(13, 32, 56))
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("●  LOCAL & ENCRYPTED").size(9.5).strong().color(GREEN));
                            ui.add_space(4.0);
                            ui.label(RichText::new("Credentials are protected with Windows DPAPI for this Windows user.").size(9.5).color(MUTED));
                        });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::same(20.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new("Windscribe Pro").size(29.0).strong().color(TEXT));
                            ui.label(RichText::new("VPN setup, account access and maintenance in one place").size(11.5).color(MUTED));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (dot, label, color) = if self.busy {
                                ("●", "WORKING", AMBER)
                            } else if self.status.installed {
                                ("●", "READY", GREEN)
                            } else {
                                ("●", "NOT INSTALLED", AMBER)
                            };
                            ui.label(RichText::new(format!("{dot}  {label}")).size(10.0).strong().color(color));
                        });
                    });
                    ui.add_space(18.0);
                    self.render_status_card(ui);
                    ui.add_space(12.0);
                    self.render_setup_cards(ui);
                    ui.add_space(12.0);
                    self.render_accounts(ui, ctx);
                    ui.add_space(12.0);
                    self.render_activity(ui);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("WINDscribe PRO HUB").size(9.0).strong().color(QUIET));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some(time) = self.last_status_check {
                                let seconds = time.elapsed().unwrap_or_default().as_secs();
                                ui.label(RichText::new(format!("Status checked {seconds}s ago")).size(9.0).color(QUIET));
                            }
                        });
                    });
                });
            });

        self.render_registry_confirmation(ctx);
        self.render_account_delete_confirmation(ctx);
    }
}

fn card<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .rounding(egui::Rounding::same(13.0))
        .inner_margin(egui::Margin::same(15.0))
        .show(ui, add_contents)
        .inner
}

fn action_button(ui: &mut egui::Ui, text: &str, color: Color32, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).size(11.5).strong().color(Color32::WHITE))
            .fill(color)
            .min_size(Vec2::new(150.0, 35.0)),
    )
}

fn small_button(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(10.0).strong().color(Color32::WHITE))
            .fill(color)
            .min_size(Vec2::new(100.0, 28.0)),
    )
}

fn nav_item(ui: &mut egui::Ui, icon: &str, title: &str, active: bool) {
    let fill = if active { Color32::from_rgb(16, 69, 140) } else { Color32::TRANSPARENT };
    let text_color = if active { TEXT } else { MUTED };
    ui.add_sized(
        [192.0, 39.0],
        egui::Button::new(
            RichText::new(format!("{icon}    {title}"))
                .size(11.5)
                .strong()
                .color(text_color),
        )
        .fill(fill),
    );
    ui.add_space(3.0);
}

fn chrono_time() -> String {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let total = elapsed.as_secs();
    let hours = (total / 3600) % 24;
    let minutes = (total / 60) % 60;
    let seconds = total % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::from_utf8_lossy(stdout).trim().to_owned();
    let error = String::from_utf8_lossy(stderr).trim().to_owned();
    if !error.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&error);
    }
    text
}

fn run_hidden(program: &str, args: &[&str]) -> io::Result<std::process::Output> {
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.output()
}

fn detect_windscribe() -> InstallStatus {
    let running = is_windscribe_running();
    let local_exe = find_windscribe_exe();
    let listed = run_hidden(
        "winget",
        &["list", "--id", PACKAGE_ID, "--exact", "--accept-source-agreements", "--disable-interactivity"],
    );

    let (managed, version, winget_error) = match listed {
        Ok(output) => {
            let combined = combined_output(&output.stdout, &output.stderr);
            let found = output.status.success() && combined.to_ascii_lowercase().contains(&PACKAGE_ID.to_ascii_lowercase());
            let version = combined
                .lines()
                .find(|line| line.to_ascii_lowercase().contains(&PACKAGE_ID.to_ascii_lowercase()))
                .and_then(|line| line.split_whitespace().nth(2))
                .unwrap_or("")
                .to_owned();
            (found, version, None)
        }
        Err(error) => (false, String::new(), Some(format!("WinGet is unavailable: {error}"))),
    };

    let installed = managed || local_exe.is_some();
    let mut update_available = false;
    let mut detail = String::new();
    let mut error = winget_error;

    if !installed {
        detail = format!("Package ID: {PACKAGE_ID}  •  Install with WinGet");
        if error.is_none() {
            error = Some("Windscribe was not found in the installed package list or standard installation paths.".to_owned());
        }
    } else if !managed {
        detail = "Detected on this PC • This installation is not managed by WinGet".to_owned();
        if error.is_none() {
            error = Some("Update detection requires a WinGet-managed installation.".to_owned());
        }
    } else {
        match run_hidden(
            "winget",
            &[
                "upgrade",
                "--id",
                PACKAGE_ID,
                "--exact",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
        ) {
            Ok(output) => {
                let text = combined_output(&output.stdout, &output.stderr);
                let lower = text.to_ascii_lowercase();
                let no_update = [
                    "no applicable upgrade found",
                    "no installed package found",
                    "no available upgrade",
                    "all applicable upgrades are excluded",
                    "no package found matching input criteria",
                ]
                .iter()
                .any(|phrase| lower.contains(phrase));
                update_available = output.status.success()
                    && !no_update
                    && text.lines().any(|line| {
                        line.to_ascii_lowercase().contains(&PACKAGE_ID.to_ascii_lowercase())
                            && !line.to_ascii_lowercase().contains("id version")
                    });
                if update_available {
                    detail = format!("Installed version: {}  •  Update available", non_empty(&version, "Detected"));
                } else {
                    detail = format!("Installed version: {}  •  Up to date", non_empty(&version, "Detected"));
                }
            }
            Err(err) => {
                detail = format!("Installed version: {}  •  Update check unavailable", non_empty(&version, "Detected"));
                error = Some(format!("Could not check for updates: {err}"));
            }
        }
    }

    InstallStatus {
        installed,
        managed_by_winget: managed,
        update_available,
        running,
        installed_version: version,
        detail,
        error,
    }
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() { fallback } else { value }
}

fn is_windscribe_running() -> bool {
    run_hidden("tasklist", &["/FI", "IMAGENAME eq Windscribe.exe", "/FO", "CSV", "/NH"])
        .map(|output| String::from_utf8_lossy(&output.stdout).to_ascii_lowercase().contains("windscribe.exe"))
        .unwrap_or(false)
}

fn find_windscribe_exe() -> Option<PathBuf> {
    let mut candidates = Vec::new();
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
                let icon: String = key.get_value("DisplayIcon").unwrap_or_default();
                if !icon.is_empty() {
                    let path = icon.trim().trim_matches('"').split(',').next().unwrap_or("").trim_matches('"');
                    candidates.push(PathBuf::from(expand_env(path)));
                }
                let location: String = key.get_value("InstallLocation").unwrap_or_default();
                if !location.is_empty() {
                    let folder = PathBuf::from(expand_env(&location));
                    candidates.push(folder.join("Windscribe.exe"));
                    candidates.push(folder.join("windscribe.exe"));
                }
            }
        }
    }

    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable).map(PathBuf::from) {
            candidates.push(root.join("Windscribe").join("Windscribe.exe"));
            candidates.push(root.join("Programs").join("Windscribe").join("Windscribe.exe"));
        }
    }

    candidates.into_iter().find(|path| path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe")))
}

fn expand_env(value: &str) -> String {
    let mut result = value.to_owned();
    for (key, val) in std::env::vars() {
        result = result.replace(&format!("%{key}%"), &val);
    }
    result
}

fn launch_windscribe() -> Result<(), String> {
    if let Some(exe) = find_windscribe_exe() {
        let mut command = Command::new(exe);
        command.spawn().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let mut shortcuts = Vec::new();
    for root in [
        std::env::var_os("ProgramData").map(PathBuf::from).map(|p| p.join(r"Microsoft\Windows\Start Menu\Programs")),
        std::env::var_os("APPDATA").map(PathBuf::from).map(|p| p.join(r"Microsoft\Windows\Start Menu\Programs")),
    ]
    .into_iter()
    .flatten()
    {
        find_shortcuts(&root, &mut shortcuts, 5);
    }

    if let Some(shortcut) = shortcuts.into_iter().next() {
        let mut command = Command::new("cmd.exe");
        command.args(["/C", "start", "", &shortcut.to_string_lossy()]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().map_err(|error| error.to_string())?;
        return Ok(());
    }

    Err("Windscribe is installed, but its desktop launcher could not be located.".to_owned())
}

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
        } else if path.file_name().is_some_and(|name| name.to_string_lossy().to_ascii_lowercase().contains("windscribe"))
            && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("lnk"))
        {
            output.push(path);
        }
    }
}

fn registry_key_path(which: &str) -> &'static str {
    match which {
        "one" => r"HKCU\Software\Microsoft\Windows\CurrentVersion\NetControlPort",
        _ => r"HKCU\Software\Windscribe\Windscribe2",
    }
}

fn backup_and_delete_registry(which: &str) -> Result<PathBuf, String> {
    let key = registry_key_path(which);
    let query = run_hidden("reg.exe", &["query", key]).map_err(|error| error.to_string())?;
    if !query.status.success() {
        return Err("The registry key does not exist or cannot be read.".to_owned());
    }

    let documents = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("Windscribe Hub")
        .join("Backups");
    fs::create_dir_all(&documents).map_err(|error| error.to_string())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let file_name = if which == "one" { "NetControlPort" } else { "Windscribe2" };
    let backup = documents.join(format!("{file_name}_{stamp}.reg"));
    let backup_text = backup.to_string_lossy().into_owned();
    let export = run_hidden("reg.exe", &["export", key, &backup_text, "/y"])
        .map_err(|error| error.to_string())?;
    if !export.status.success() {
        return Err(format!("Registry backup failed: {}", combined_output(&export.stdout, &export.stderr)));
    }

    let delete = run_hidden("reg.exe", &["delete", key, "/f"]).map_err(|error| error.to_string())?;
    if !delete.status.success() {
        return Err(format!("Backup was created, but deletion failed: {}", combined_output(&delete.stdout, &delete.stderr)));
    }
    Ok(backup)
}

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[link(name = "Crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        data_in: *const DataBlob,
        description: *const u16,
        optional_entropy: *const DataBlob,
        reserved: *mut std::ffi::c_void,
        prompt: *const std::ffi::c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        data_in: *const DataBlob,
        description: *mut *mut u16,
        optional_entropy: *const DataBlob,
        reserved: *mut std::ffi::c_void,
        prompt: *const std::ffi::c_void,
        flags: u32,
        data_out: *mut DataBlob,
    ) -> i32;
}

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
}

fn replace_file_atomically(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination.as_os_str().encode_wide().chain(Some(0)).collect();
    // MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
    let success = unsafe { MoveFileExW(source_wide.as_ptr(), destination_wide.as_ptr(), 0x1 | 0x8) };
    if success == 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(())
}

fn protect_data(input: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    let mut source = DataBlob {
        cb_data: input.len() as u32,
        pb_data: input.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    let success = unsafe {
        if protect {
            let description: Vec<u16> = "Windscribe Pro account vault\0".encode_utf16().collect();
            CryptProtectData(
                &mut source,
                description.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0x1,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &mut source,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                0x1,
                &mut output,
            )
        }
    };
    if success == 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    let result = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec() };
    if !output.pb_data.is_null() {
        unsafe { LocalFree(output.pb_data.cast()) };
    }
    Ok(result)
}

fn dpapi_protect(input: &[u8]) -> Result<Vec<u8>, String> {
    protect_data(input, true)
}

fn dpapi_unprotect(input: &[u8]) -> Result<Vec<u8>, String> {
    protect_data(input, false)
}

fn main() -> eframe::Result<()> {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = Color32::from_rgb(5, 13, 23);
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(27, 47, 73);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(37, 78, 124);
    visuals.widgets.active.bg_fill = BLUE;
    visuals.selection.bg_fill = Color32::from_rgb(23, 88, 154);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 900.0])
            .with_min_inner_size([1080.0, 760.0])
            .with_title(format!("{APP_TITLE}  •  {APP_VERSION}")),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |creation_context| {
            creation_context.egui_ctx.set_visuals(visuals);
            Ok(Box::new(WindscribeApp::new()))
        }),
    )
}
