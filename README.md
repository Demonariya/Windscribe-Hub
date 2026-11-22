# Windscribe Hub

**Version:** 0.1.0  
**Platform:** Windows 10 / Windows 11 (x64)  
**Language:** Rust (stable)  
**UI:** eframe / egui / egui_extras  
**License:** MIT

Windscribe Hub is a native Windows desktop management utility for the official **Windscribe** desktop application.  
It does **not** establish VPN connections, connect to VPN servers, change IP addresses, or implement VPN protocols.

## Capabilities

### Windscribe Installation & Process Management
- Detects the official WinGet package `Windscribe.Windscribe`.
- Inspects Windows Uninstall registry entries (`HKLM` and `HKCU`, 64-bit and WOW6432Node) and common installation paths so non-WinGet installations are recognized accurately.
- Queries WinGet upgrades only when Windscribe is managed by WinGet.
- Launches, installs, or updates Windscribe asynchronously on background Tokio workers without blocking the UI thread.
- Detects the live `Windscribe.exe` process state (`Running`, `Stopped`, or `Unable to determine`).

### Secure Account Vault
- Stores multiple Windscribe username/password credentials locally.
- Individual show/hide password toggles, search filtering, username suggestion (`ws_*`), and 20-character strong password generation.
- Encrypts the vault at `%LOCALAPPDATA%\WindscribeHub\accounts.vault` using **Windows DPAPI** (`CryptProtectData` / `CryptUnprotectData`) and atomic file replacement (`MoveFileExW`).
- Never logs passwords, never stores plaintext passwords on disk, and never transmits credentials externally.

### Registry Cleaner
Supports safe maintenance of:
1. `HKCU\Software\Microsoft\Windows\CurrentVersion\NetControlPort`
2. `HKCU\Software\Windscribe\Windscribe2`

Every cleanup requires explicit user confirmation, verifies key existence, exports a timestamped `.reg` backup to `%USERPROFILE%\Documents\Windscribe Hub\Backups`, verifies the backup file exists and is non-empty, deletes the key, and verifies deletion succeeded.

### Preferences & Themes
- Dark, Light, and System themes inspired by the official Windscribe Windows desktop interface (`420 × 760` compact window).
- Animation preferences, configurable automatic background refresh, optional Windows startup integration (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), and persistent settings at `%LOCALAPPDATA%\WindscribeHub\settings.json`.

## Build From Source (Windows x64)

Requires stable Rust and Visual Studio C++ Build Tools (MSVC) with the Windows SDK:

```powershell
cargo fmt --check
cargo check
cargo test
cargo build --release
```

Output binary: `target\release\windscribe-hub.exe`
