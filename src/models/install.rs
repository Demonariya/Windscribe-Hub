use std::path::PathBuf;
use std::time::SystemTime;

pub const WINDSCRIBE_PACKAGE_ID: &str = "Windscribe.Windscribe";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InstallSource {
    WinGet,
    LocalRegistry,
    LocalPath,
    #[default]
    None,
    Unknown,
}

impl InstallSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::WinGet => "WinGet",
            Self::LocalRegistry => "Windows Registry",
            Self::LocalPath => "Local Directory",
            Self::None => "Not installed",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InstallStateKind {
    #[default]
    Checking,
    NotInstalled,
    InstalledUpToDate,
    InstalledUpdateAvailable,
    InstalledUnmanaged,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProcessState {
    Running {
        pid: Option<u32>,
    },
    Stopped,
    #[default]
    Unknown,
}

impl ProcessState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Running { .. } => "Running",
            Self::Stopped => "Stopped",
            Self::Unknown => "Unable to determine",
        }
    }

    pub fn is_running(self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    RefreshStatus,
    RefreshProcess,
    Install,
    Update,
    Launch,
    ScanRegistry,
    CleanRegistry,
}

impl OperationKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::RefreshStatus => "Checking Windscribe status…",
            Self::RefreshProcess => "Checking Windscribe.exe process…",
            Self::Install => "Installing Windscribe via WinGet…",
            Self::Update => "Updating Windscribe via WinGet…",
            Self::Launch => "Launching Windscribe…",
            Self::ScanRegistry => "Scanning Windscribe registry keys…",
            Self::CleanRegistry => "Backing up & cleaning registry key…",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstallStatus {
    pub installed: bool,
    pub managed_by_winget: bool,
    pub winget_available: bool,
    pub update_available: bool,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub install_path: Option<PathBuf>,
    pub source: InstallSource,
    pub process: ProcessState,
    pub state_kind: InstallStateKind,
    pub summary: String,
    pub error: Option<String>,
    pub last_checked: Option<SystemTime>,
}

impl InstallStatus {
    pub fn primary_action(&self) -> PrimaryActionKind {
        if let Some(err) = &self.error {
            if !self.installed && !self.winget_available && err.contains("WinGet") {
                return PrimaryActionKind::RetryCheck;
            }
        }
        if !self.installed {
            if self.winget_available {
                PrimaryActionKind::Install
            } else {
                PrimaryActionKind::RetryCheck
            }
        } else if self.update_available && self.managed_by_winget {
            PrimaryActionKind::Update
        } else {
            PrimaryActionKind::Open
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryActionKind {
    Install,
    Update,
    Open,
    RetryCheck,
}

impl PrimaryActionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Install => "INSTALL WINDSCRIBE",
            Self::Update => "UPDATE WINDSCRIBE",
            Self::Open => "OPEN WINDSCRIBE",
            Self::RetryCheck => "RETRY CHECK",
        }
    }
}
