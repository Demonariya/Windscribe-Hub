#[cfg(windows)]
const STARTUP_VALUE_NAME: &str = "WindscribeHub";

pub fn set_launch_on_startup(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (run_key, _) = hkcu
            .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
            .map_err(|err| format!("Could not open Windows Run key: {err}"))?;

        if enabled {
            let exe = std::env::current_exe()
                .map_err(|err| format!("Could not resolve executable path: {err}"))?;
            let quoted = format!("\"{}\"", exe.display());
            run_key
                .set_value(STARTUP_VALUE_NAME, &quoted)
                .map_err(|err| format!("Could not register startup entry: {err}"))?;
        } else {
            let _ = run_key.delete_value(STARTUP_VALUE_NAME);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("Windows startup integration is only available on Windows.".to_owned())
    }
}

pub fn is_launch_on_startup_enabled() -> bool {
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(run_key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run") else {
            return false;
        };
        let val: Result<String, _> = run_key.get_value(STARTUP_VALUE_NAME);
        val.is_ok()
    }

    #[cfg(not(windows))]
    {
        false
    }
}
