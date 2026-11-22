use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NavSection {
    #[default]
    Home,
    Accounts,
    Tools,
    Registry,
    Settings,
}

impl NavSection {
    pub const ALL: [Self; 5] = [
        Self::Home,
        Self::Accounts,
        Self::Tools,
        Self::Registry,
        Self::Settings,
    ];

    pub fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Accounts => 1,
            Self::Tools => 2,
            Self::Registry => 3,
            Self::Settings => 4,
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Accounts => "Accounts",
            Self::Tools => "Tools",
            Self::Registry => "Registry",
            Self::Settings => "Settings",
        }
    }

    pub fn full_title(self) -> &'static str {
        match self {
            Self::Home => "Windscribe Hub",
            Self::Accounts => "Account Vault",
            Self::Tools => "Tools & Diagnostics",
            Self::Registry => "Registry Cleaner",
            Self::Settings => "Preferences",
        }
    }

    pub fn shortcut_hint(self) -> &'static str {
        match self {
            Self::Home => "Alt+1",
            Self::Accounts => "Alt+2",
            Self::Tools => "Alt+3",
            Self::Registry => "Alt+4",
            Self::Settings => "Alt+5",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::System];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::System => "System",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub animations_enabled: bool,
    pub launch_on_startup: bool,
    pub auto_refresh: bool,
    pub refresh_interval_secs: u64,
    pub show_toasts: bool,
    pub native_notifications: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            animations_enabled: true,
            launch_on_startup: false,
            auto_refresh: true,
            refresh_interval_secs: 30,
            show_toasts: true,
            native_notifications: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct ToastMessage {
    pub id: u64,
    pub level: ToastLevel,
    pub title: String,
    pub detail: Option<String>,
    pub created_at: Instant,
    pub duration_secs: f32,
}
