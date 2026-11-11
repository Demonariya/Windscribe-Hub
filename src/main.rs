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

const BG: Color32 = Color32::from_rgb(10, 14, 21);
const SIDEBAR: Color32 = Color32::from_rgb(13, 18, 27);
const PANEL: Color32 = Color32::from_rgb(17, 24, 35);
const CARD: Color32 = Color32::from_rgb(21, 29, 42);
const CARD_ALT: Color32 = Color32::from_rgb(16, 23, 34);
const BORDER: Color32 = Color32::from_rgb(43, 56, 74);
const BLUE: Color32 = Color32::from_rgb(57, 132, 230);
const PURPLE: Color32 = Color32::from_rgb(125, 103, 218);
const GREEN: Color32 = Color32::from_rgb(54, 181, 128);
const AMBER: Color32 = Color32::from_rgb(218, 168, 75);
const RED: Color32 = Color32::from_rgb(214, 91, 105);
const TEXT: Color32 = Color32::from_rgb(238, 242, 248);
const MUTED: Color32 = Color32::from_rgb(164, 176, 194);
const QUIET: Color32 = Color32::from_rgb(111, 125, 146);
const ACCENT_SOFT: Color32 = Color32::from_rgb(25, 44, 69);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Overview,
    Accounts,
    Registry,
}

#[derive(Clone, Debug, Default)]
struct InstallStatus {
    installed: bool,
    managed_by_winget: bool,
    update_available: bool,
    running: bool,
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
    active_page: Page,
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
            active_page: Page::Overview,
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
        let (title, color, status_label) = if !self.status.installed {
            ("Windscribe isn't installed", AMBER, "ACTION REQUIRED")
        } else if self.status.update_available {
            ("An update is ready", AMBER, "UPDATE AVAILABLE")
        } else {
            ("Windscribe is ready", GREEN, "SYSTEM READY")
        };

        egui::Frame::none()
            .fill(PANEL)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .rounding(egui::Rounding::same(16.0))
            .inner_margin(egui::Margin::same(20.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (accent_rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 76.0), egui::Sense::hover());
                    ui.painter().rect_filled(accent_rect, 2.0, color);
                    ui.add_space(5.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(title).size(20.0).strong().color(TEXT));
                            status_pill(ui, status_label, color);
                        });
                        ui.add_space(5.0);
                        ui.label(RichText::new(&self.status.detail).size(11.5).color(MUTED));
                        ui.add_space(9.0);
                        let process_text = if self.status.running { "●  RUNNING" } else { "○  NOT RUNNING" };
                        ui.label(
                            RichText::new(process_text)
                                .size(10.0)
                                .strong()
                                .color(if self.status.running { GREEN } else { QUIET }),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if action_button(ui, if self.busy { "Checking…" } else { "↻  Refresh status" }, ACCENT_SOFT, !self.busy).clicked() {
                            self.refresh_status();
                        }
                    });
                });
            });
    }

    fn render_quick_stats(&self, ui: &mut egui::Ui) {
        ui.columns(3, |columns| {
            metric_card(
                &mut columns[0],
                "INSTALLATION",
                if self.status.installed { "Installed" } else { "Not found" },
                if self.status.managed_by_winget { "Managed by WinGet" } else { "Local detection" },
                if self.status.installed { GREEN } else { AMBER },
            );
            metric_card(
                &mut columns[1],
                "APP PROCESS",
                if self.status.running { "Running" } else { "Stopped" },
                "Desktop process state",
                if self.status.running { GREEN } else { QUIET },
            );
            metric_card(
                &mut columns[2],
                "SAVED ACCOUNTS",
                &self.accounts.len().to_string(),
                "Encrypted on this device",
                BLUE,
            );
        });
    }

    fn render_setup_cards(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |columns| {
            card(&mut columns[0], |ui| {
                ui.horizontal(|ui| {
                    icon_tile(ui, "W", BLUE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("WINDSCRIBE DESKTOP").size(12.0).strong().color(TEXT));
                        ui.label(RichText::new("Install, maintain and launch").size(10.0).color(QUIET));
                    });
                });
                ui.add_space(13.0);
                let (label, color, enabled) = if self.busy {
                    ("Please wait…", QUIET, false)
                } else if !self.status.installed && self.status.error.as_deref().is_some_and(|error| error.starts_with("WinGet is unavailable")) {
                    ("WinGet unavailable", QUIET, false)
                } else if !self.status.installed {
                    ("Install Windscribe", BLUE, true)
                } else if self.status.update_available {
                    ("Install available update", PURPLE, true)
                } else if !self.status.managed_by_winget {
                    ("Update check unavailable", QUIET, false)
                } else {
                    ("You're up to date", Color32::from_rgb(37, 77, 61), false)
                };
                if wide_button(ui, label, color, enabled).clicked() {
                    self.install_or_update();
                }
                ui.add_space(8.0);
                if wide_button(ui, "↗  Open Windscribe", ACCENT_SOFT, self.status.installed && !self.busy).clicked() {
                    self.open_windscribe();
                }
                if self.status.installed && !self.status.managed_by_winget {
                    ui.add_space(8.0);
                    ui.label(RichText::new("This copy was detected locally. Update checks require a WinGet-managed installation.").size(9.5).color(QUIET));
                }
            });

            card(&mut columns[1], |ui| {
                ui.horizontal(|ui| {
                    icon_tile(ui, "⌘", PURPLE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("REGISTRY MAINTENANCE").size(12.0).strong().color(TEXT));
                        ui.label(RichText::new("Back up before making changes").size(10.0).color(QUIET));
                    });
                });
                ui.add_space(13.0);
                ui.label(RichText::new("Remove selected Windscribe registry entries. A .reg backup is created first; deletion only proceeds after a successful export.").size(10.5).color(MUTED));
                ui.add_space(11.0);
                ui.horizontal(|ui| {
                    if small_button(ui, "Clean key 01", ACCENT_SOFT).clicked() {
                        self.confirm_registry = Some("one");
                    }
                    if small_button(ui, "Clean key 02", ACCENT_SOFT).clicked() {
                        self.confirm_registry = Some("two");
                    }
                });
                ui.add_space(9.0);
                ui.label(RichText::new("01  NetControlPort").size(9.5).color(QUIET));
                ui.label(RichText::new("02  Windscribe2").size(9.5).color(QUIET));
            });
        });
    }

    fn render_accounts(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                icon_tile(ui, "◈", BLUE);
                ui.vertical(|ui| {
                    ui.label(RichText::new("ACCOUNT VAULT").size(13.0).strong().color(TEXT));
                    ui.label(RichText::new("Encrypted credentials stored for this Windows user").size(9.5).color(QUIET));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_pill(ui, &format!("{} SAVED", self.accounts.len()), BLUE);
                });
            });
            ui.add_space(14.0);

            ui.columns(2, |columns| {
                let left = &mut columns[0];
                egui::Frame::none()
                    .fill(CARD_ALT)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .rounding(egui::Rounding::same(11.0))
                    .inner_margin(egui::Margin::same(13.0))
                    .show(left, |ui| {
                        ui.label(RichText::new("CREDENTIAL EDITOR").size(9.0).strong().color(QUIET));
                        ui.add_space(9.0);
                        ui.label(RichText::new("Username").size(10.0).strong().color(TEXT));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.username_input)
                                .hint_text("Enter Windscribe username")
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(9.0);
                        ui.label(RichText::new("Password").size(10.0).strong().color(TEXT));
                        ui.horizontal(|ui| {
                            let mut password = egui::TextEdit::singleline(&mut self.password_input)
                                .hint_text("Enter password")
                                .desired_width((ui.available_width() - 64.0).max(100.0));
                            password = password.password(!self.password_visible);
                            ui.add(password);
                            let show_label = if self.password_visible { "Hide" } else { "Show" };
                            if ui.add(egui::Button::new(show_label).fill(ACCENT_SOFT)).clicked() {
                                self.password_visible = !self.password_visible;
                            }
                        });
                        ui.add_space(11.0);
                        ui.horizontal_wrapped(|ui| {
                            if small_button(ui, "Suggest username", ACCENT_SOFT).clicked() {
                                self.generate_username();
                            }
                            if small_button(ui, "Generate password", Color32::from_rgb(57, 47, 91)).clicked() {
                                self.generate_password();
                            }
                        });
                        ui.add_space(11.0);
                        if wide_button(ui, "Save account", GREEN, true).clicked() {
                            self.save_account();
                        }
                        ui.add_space(7.0);
                        ui.label(RichText::new("Username suggestions are not registered automatically.").size(8.5).color(QUIET));
                    });

                let right = &mut columns[1];
                ui_heading(right, "SAVED ACCOUNTS", "Select an entry to load or copy its credentials.");
                right.add_space(9.0);
                egui::Frame::none()
                    .fill(CARD_ALT)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .rounding(egui::Rounding::same(11.0))
                    .inner_margin(egui::Margin::same(8.0))
                    .show(right, |ui| {
                        egui::ScrollArea::vertical().max_height(225.0).show(ui, |ui| {
                            if self.accounts.is_empty() {
                                ui.add_space(32.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new("No saved accounts").size(11.0).strong().color(MUTED));
                                    ui.label(RichText::new("Add a login using the editor.").size(9.0).color(QUIET));
                                });
                                ui.add_space(32.0);
                            }

                            let mut delete_after: Option<String> = None;
                            let mut select_after: Option<String> = None;
                            for account in &self.accounts {
                                let selected = self.selected_account.as_deref() == Some(account.id.as_str());
                                let is_revealed = self.revealed_accounts.contains(&account.id);
                                let row_fill = if selected { Color32::from_rgb(28, 47, 71) } else { CARD_ALT };
                                let row_border = if selected { BLUE.linear_multiply(0.65) } else { BORDER.linear_multiply(0.35) };
                                egui::Frame::none()
                                    .fill(row_fill)
                                    .stroke(Stroke::new(1.0_f32, row_border))
                                    .rounding(egui::Rounding::same(8.0))
                                    .inner_margin(egui::Margin::symmetric(9.0, 8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(&account.username).size(10.5).strong().color(if selected { Color32::from_rgb(139, 190, 255) } else { TEXT }));
                                                let hidden = "•".repeat(account.password.chars().count().clamp(8, 16));
                                                let password = if is_revealed { account.password.as_str() } else { hidden.as_str() };
                                                ui.label(RichText::new(password).size(9.0).color(QUIET));
                                            });
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                if ui.add(egui::Button::new("×").fill(Color32::from_rgb(83, 39, 49))).clicked() {
                                                    delete_after = Some(account.id.clone());
                                                }
                                                let reveal_text = if is_revealed { "Hide" } else { "Show" };
                                                if ui.add(egui::Button::new(reveal_text).fill(ACCENT_SOFT)).clicked() {
                                                    if is_revealed {
                                                        self.revealed_accounts.remove(&account.id);
                                                    } else {
                                                        self.revealed_accounts.insert(account.id.clone());
                                                    }
                                                }
                                                if ui.add(egui::Button::new(if selected { "Selected" } else { "Select" }).fill(if selected { BLUE } else { ACCENT_SOFT })).clicked() {
                                                    select_after = Some(account.id.clone());
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(5.0);
                            }

                            if let Some(id) = select_after {
                                self.selected_account = Some(id);
                            }
                            if let Some(id) = delete_after {
                                self.pending_delete_account = Some(id);
                            }
                        });
                    });

                right.add_space(9.0);
                right.horizontal_wrapped(|ui| {
                    let has_selection = self.selected_account.is_some();
                    if ui.add_enabled(has_selection, egui::Button::new("Load into editor").fill(ACCENT_SOFT)).clicked() {
                        self.load_selected_account();
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Copy username").fill(ACCENT_SOFT)).clicked() {
                        self.copy_username(ctx);
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Copy password").fill(ACCENT_SOFT)).clicked() {
                        self.copy_password(ctx);
                    }
                    if ui.add_enabled(has_selection, egui::Button::new("Delete").fill(Color32::from_rgb(83, 39, 49))).clicked() {
                        self.pending_delete_account = self.selected_account.clone();
                    }
                });
            });
        });
    }

    fn render_registry_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                icon_tile(ui, "⌘", PURPLE);
                ui.vertical(|ui| {
                    ui.label(RichText::new("REGISTRY CLEANUP").size(14.0).strong().color(TEXT));
                    ui.label(RichText::new("Careful maintenance of Windscribe registry entries").size(10.0).color(QUIET));
                });
            });
            ui.add_space(12.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(57, 43, 25))
                .rounding(egui::Rounding::same(9.0))
                .inner_margin(egui::Margin::same(11.0))
                .show(ui, |ui| {
                    ui.label(RichText::new("Backup protection is enabled").size(10.5).strong().color(AMBER));
                    ui.label(RichText::new("Each key is exported to a timestamped .reg file before deletion. If export fails, the key is left untouched.").size(9.5).color(MUTED));
                });
            ui.add_space(13.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("KEY 01").size(9.0).strong().color(QUIET));
                    ui.label(RichText::new("NetControlPort").size(12.0).strong().color(TEXT));
                    ui.label(RichText::new(r#"HKCU\Software\Microsoft\Windows\CurrentVersion\NetControlPort"#).size(9.0).monospace().color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if action_button(ui, "Review & clean", PURPLE, !self.busy).clicked() {
                        self.confirm_registry = Some("one");
                    }
                });
            });
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("KEY 02").size(9.0).strong().color(QUIET));
                    ui.label(RichText::new("Windscribe2").size(12.0).strong().color(TEXT));
                    ui.label(RichText::new(r#"HKCU\Software\Windscribe\Windscribe2"#).size(9.0).monospace().color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if action_button(ui, "Review & clean", PURPLE, !self.busy).clicked() {
                        self.confirm_registry = Some("two");
                    }
                });
            });
        });
    }

    fn render_activity(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("ACTIVITY").size(12.0).strong().color(TEXT));
                    ui.label(RichText::new("Recent operations and system messages").size(9.5).color(QUIET));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_pill(ui, &format!("{} EVENTS", self.logs.len()), MUTED);
                    ui.add_space(7.0);
                    if small_button(ui, "Clear log", ACCENT_SOFT).clicked() {
                        self.logs.clear();
                    }
                });
            });
            ui.add_space(11.0);
            egui::Frame::none()
                .fill(Color32::from_rgb(11, 16, 24))
                .stroke(Stroke::new(1.0_f32, BORDER))
                .rounding(egui::Rounding::same(10.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().max_height(142.0).stick_to_bottom(true).show(ui, |ui| {
                        if self.logs.is_empty() {
                            ui.label(RichText::new("No activity recorded yet.").size(10.0).color(QUIET));
                        }
                        for line in &self.logs {
                            ui.horizontal(|ui| {
                                let (dot, _) = ui.allocate_exact_size(Vec2::splat(5.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot.center(), 2.5, BLUE);
                                ui.label(RichText::new(line).font(FontId::monospace(9.5)).color(MUTED));
                            });
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
        ctx.request_repaint_after(Duration::from_millis(180));

        egui::SidePanel::left("navigation")
            .exact_width(244.0)
            .frame(egui::Frame::none().fill(SIDEBAR).inner_margin(egui::Margin::same(18.0)))
            .show(ctx, |ui| {
                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(44.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 12.0, ACCENT_SOFT);
                    ui.painter().rect_stroke(rect, 12.0, Stroke::new(1.0_f32, BLUE));
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "W",
                        FontId::proportional(25.0),
                        Color32::from_rgb(114, 177, 248),
                    );
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("WINDSCRIBE").size(13.0).strong().color(TEXT));
                        ui.label(RichText::new("DESKTOP COMPANION").size(8.5).strong().color(QUIET));
                    });
                });

                ui.add_space(31.0);
                ui.label(RichText::new("WORKSPACE").size(8.5).strong().color(QUIET));
                ui.add_space(8.0);
                if nav_item(ui, "◫", "Overview", self.active_page == Page::Overview) {
                    self.active_page = Page::Overview;
                }
                if nav_item(ui, "◈", "Account vault", self.active_page == Page::Accounts) {
                    self.active_page = Page::Accounts;
                }
                if nav_item(ui, "⌘", "Registry tools", self.active_page == Page::Registry) {
                    self.active_page = Page::Registry;
                }

                ui.add_space(17.0);
                ui.separator();
                ui.add_space(14.0);
                ui.label(RichText::new("DEVICE").size(8.5).strong().color(QUIET));
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(CARD_ALT)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .rounding(egui::Rounding::same(11.0))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let indicator = if self.status.installed { GREEN } else { AMBER };
                            let (dot, _) = ui.allocate_exact_size(Vec2::splat(7.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot.center(), 3.5, indicator);
                            ui.label(RichText::new(if self.status.installed { "Windscribe detected" } else { "Not detected" }).size(10.0).strong().color(TEXT));
                        });
                        ui.add_space(4.0);
                        ui.label(RichText::new(if self.status.running { "Desktop process is running" } else { "Desktop process is stopped" }).size(9.0).color(QUIET));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Saved accounts").size(9.0).color(MUTED));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(self.accounts.len().to_string()).size(10.0).strong().color(TEXT));
                            });
                        });
                    });

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("VERSION {APP_VERSION}")).size(8.5).strong().color(QUIET));
                    ui.add_space(12.0);
                    egui::Frame::none()
                        .fill(Color32::from_rgb(20, 35, 31))
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(43, 76, 63)))
                        .rounding(egui::Rounding::same(10.0))
                        .inner_margin(egui::Margin::same(11.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("●  PRIVATE BY DESIGN").size(8.5).strong().color(GREEN));
                            ui.add_space(4.0);
                            ui.label(RichText::new("Account credentials are encrypted locally with Windows DPAPI.").size(9.0).color(MUTED));
                        });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(egui::Margin::symmetric(25.0, 22.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let (title, subtitle) = match self.active_page {
                        Page::Overview => ("Overview", "Your Windscribe desktop status at a glance."),
                        Page::Accounts => ("Account vault", "Manage encrypted credentials stored on this device."),
                        Page::Registry => ("Registry tools", "Review maintenance actions and protected backups."),
                    };

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(title).size(26.0).strong().color(TEXT));
                            ui.label(RichText::new(subtitle).size(10.5).color(MUTED));
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (label, color) = if self.busy {
                                ("WORKING", AMBER)
                            } else if self.status.installed {
                                ("CONNECTED TO DEVICE", GREEN)
                            } else {
                                ("DEVICE CHECK", QUIET)
                            };
                            status_pill(ui, label, color);
                        });
                    });
                    ui.add_space(19.0);

                    match self.active_page {
                        Page::Overview => {
                            self.render_status_card(ui);
                            ui.add_space(12.0);
                            self.render_quick_stats(ui);
                            ui.add_space(12.0);
                            self.render_setup_cards(ui);
                            ui.add_space(12.0);
                            self.render_accounts(ui, ctx);
                            ui.add_space(12.0);
                            self.render_activity(ui);
                        }
                        Page::Accounts => {
                            self.render_accounts(ui, ctx);
                            ui.add_space(12.0);
                            self.render_activity(ui);
                        }
                        Page::Registry => {
                            self.render_status_card(ui);
                            ui.add_space(12.0);
                            self.render_registry_page(ui);
                            ui.add_space(12.0);
                            self.render_activity(ui);
                        }
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("WINDSCRIBE DESKTOP COMPANION").size(8.5).strong().color(QUIET));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some(time) = self.last_status_check {
                                let seconds = time.elapsed().unwrap_or_default().as_secs();
                                ui.label(RichText::new(format!("Last device check · {seconds}s ago")).size(8.5).color(QUIET));
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
        .stroke(Stroke::new(1.0_f32, BORDER))
        .rounding(egui::Rounding::same(14.0))
        .inner_margin(egui::Margin::same(17.0))
        .show(ui, add_contents)
        .inner
}

fn ui_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(9.0).strong().color(QUIET));
    ui.add_space(2.0);
    ui.label(RichText::new(subtitle).size(9.0).color(MUTED));
}

fn status_pill(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.linear_multiply(0.16))
        .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.35)))
        .rounding(egui::Rounding::same(20.0))
        .inner_margin(egui::Margin::symmetric(9.0, 5.0))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(8.5).strong().color(color));
        });
}

fn icon_tile(ui: &mut egui::Ui, icon: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(37.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 10.0, color.linear_multiply(0.17));
    ui.painter().rect_stroke(rect, 10.0, Stroke::new(1.0_f32, color.linear_multiply(0.38)));
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, icon, FontId::proportional(17.0), color);
    ui.add_space(3.0);
}

fn metric_card(ui: &mut egui::Ui, label: &str, value: &str, detail: &str, color: Color32) {
    card(ui, |ui| {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(7.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 3.5, color);
            ui.label(RichText::new(label).size(8.5).strong().color(QUIET));
        });
        ui.add_space(8.0);
        ui.label(RichText::new(value).size(18.0).strong().color(TEXT));
        ui.add_space(2.0);
        ui.label(RichText::new(detail).size(9.0).color(MUTED));
    });
}

fn action_button(ui: &mut egui::Ui, text: &str, color: Color32, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).size(10.5).strong().color(Color32::WHITE))
            .fill(color)
            .min_size(Vec2::new(142.0, 36.0)),
    )
}

fn wide_button(ui: &mut egui::Ui, text: &str, color: Color32, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).size(10.5).strong().color(Color32::WHITE))
            .fill(color)
            .min_size(Vec2::new(ui.available_width(), 38.0)),
    )
}

fn small_button(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(9.0).strong().color(TEXT))
            .fill(color)
            .min_size(Vec2::new(92.0, 29.0)),
    )
}

fn nav_item(ui: &mut egui::Ui, icon: &str, title: &str, active: bool) -> bool {
    let fill = if active { ACCENT_SOFT } else { Color32::TRANSPARENT };
    let text_color = if active { TEXT } else { MUTED };
    let response = ui.add_sized(
        [ui.available_width(), 42.0],
        egui::Button::new(
            RichText::new(format!("{icon}    {title}"))
                .size(10.5)
                .strong()
                .color(text_color),
        )
        .fill(fill),
    );
    ui.add_space(4.0);
    response.clicked()
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
    let mut detail: String;
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
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(31, 43, 59);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(43, 62, 86);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(185, 207, 237));
    visuals.widgets.active.bg_fill = BLUE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.selection.bg_fill = Color32::from_rgb(35, 74, 119);
    visuals.window_rounding = egui::Rounding::same(13.0);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);

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
            creation_context.egui_ctx.style_mut(|style| {
                style.spacing.item_spacing = Vec2::new(10.0, 8.0);
                style.spacing.button_padding = Vec2::new(12.0, 8.0);
            });
            Ok(Box::new(WindscribeApp::new()))
        }),
    )
}
