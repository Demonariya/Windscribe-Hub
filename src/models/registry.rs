use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegistryTarget {
    NetControlPort,
    Windscribe2,
}

impl RegistryTarget {
    pub const ALL: [Self; 2] = [Self::NetControlPort, Self::Windscribe2];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::NetControlPort => "NetControlPort",
            Self::Windscribe2 => "Windscribe2",
        }
    }

    pub fn registry_path(self) -> &'static str {
        match self {
            Self::NetControlPort => {
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\NetControlPort"
            }
            Self::Windscribe2 => r"HKCU\Software\Windscribe\Windscribe2",
        }
    }

    pub fn backup_prefix(self) -> &'static str {
        match self {
            Self::NetControlPort => "NetControlPort",
            Self::Windscribe2 => "Windscribe2",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::NetControlPort => {
                "Stores Windows network control port configuration created by Windscribe."
            }
            Self::Windscribe2 => {
                "Stores Windscribe 2.x client preferences and local UI/runtime state."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RegistryKeyState {
    #[default]
    Unknown,
    Exists,
    NotFound,
    Error,
}

impl RegistryKeyState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Not scanned",
            Self::Exists => "Exists",
            Self::NotFound => "Not found",
            Self::Error => "Access error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryKeyInfo {
    pub target: RegistryTarget,
    pub state: RegistryKeyState,
    pub last_checked: Option<SystemTime>,
    pub backup_count: usize,
    pub latest_backup: Option<PathBuf>,
    pub last_error: Option<String>,
}

impl RegistryKeyInfo {
    pub fn new(target: RegistryTarget) -> Self {
        Self {
            target,
            state: RegistryKeyState::Unknown,
            last_checked: None,
            backup_count: 0,
            latest_backup: None,
            last_error: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupOutcome {
    pub target: RegistryTarget,
    pub backup_path: PathBuf,
}
