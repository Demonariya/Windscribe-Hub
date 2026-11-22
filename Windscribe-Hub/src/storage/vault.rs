use crate::models::WindscribeAccount;
use crate::utils::time::unix_seconds;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VaultError {
    Io(String),
    Encryption(String),
    Corrupted(String),
    InvalidPath(String),
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "Vault I/O error: {msg}"),
            Self::Encryption(msg) => write!(f, "Vault encryption error: {msg}"),
            Self::Corrupted(msg) => write!(f, "Vault data is corrupted: {msg}"),
            Self::InvalidPath(msg) => write!(f, "Invalid vault path: {msg}"),
        }
    }
}

impl std::error::Error for VaultError {}

pub trait VaultCipher: Send + Sync {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError>;
    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, VaultError>;
}

pub fn default_vault_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("WindscribeHub")
        .join("accounts.vault")
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsDpapiCipher;

#[cfg(windows)]
mod dpapi_win {
    use super::VaultError;
    use std::io;
    use std::path::Path;

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

    pub fn protect_bytes(input: &[u8], protect: bool) -> Result<Vec<u8>, VaultError> {
        let mut source = DataBlob {
            cb_data: u32::try_from(input.len())
                .map_err(|_| VaultError::Encryption("Vault payload exceeds 4 GiB".to_owned()))?,
            pb_data: input.as_ptr() as *mut u8,
        };
        let mut output = DataBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };

        let ok = unsafe {
            if protect {
                let description: Vec<u16> =
                    "Windscribe Hub account vault\0".encode_utf16().collect();
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

        if ok == 0 {
            let err = io::Error::last_os_error().to_string();
            return Err(if protect {
                VaultError::Encryption(err)
            } else {
                VaultError::Corrupted(err)
            });
        }

        let slice = unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize) };
        let result = slice.to_vec();
        if !output.pb_data.is_null() {
            unsafe { LocalFree(output.pb_data.cast()) };
        }
        Ok(result)
    }

    pub fn replace_atomically(source: &Path, destination: &Path) -> Result<(), VaultError> {
        use std::os::windows::ffi::OsStrExt;
        let src_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst_wide: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // MOVEFILE_REPLACE_EXISTING (0x1) | MOVEFILE_WRITE_THROUGH (0x8)
        let ok = unsafe { MoveFileExW(src_wide.as_ptr(), dst_wide.as_ptr(), 0x1 | 0x8) };
        if ok == 0 {
            return Err(VaultError::Io(io::Error::last_os_error().to_string()));
        }
        Ok(())
    }
}

impl VaultCipher for WindowsDpapiCipher {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        #[cfg(windows)]
        {
            dpapi_win::protect_bytes(plaintext, true)
        }
        #[cfg(not(windows))]
        {
            let _ = plaintext;
            Err(VaultError::Encryption(
                "Windows DPAPI encryption is only available on Windows hosts.".to_owned(),
            ))
        }
    }

    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
        #[cfg(windows)]
        {
            dpapi_win::protect_bytes(ciphertext, false)
        }
        #[cfg(not(windows))]
        {
            let _ = ciphertext;
            Err(VaultError::Encryption(
                "Windows DPAPI decryption is only available on Windows hosts.".to_owned(),
            ))
        }
    }
}

pub type SystemVaultCipher = WindowsDpapiCipher;

pub fn replace_file_atomically(source: &Path, destination: &Path) -> Result<(), VaultError> {
    #[cfg(windows)]
    {
        dpapi_win::replace_atomically(source, destination)
    }
    #[cfg(not(windows))]
    {
        fs::rename(source, destination).map_err(|err| VaultError::Io(err.to_string()))
    }
}

pub struct VaultStore<C: VaultCipher = SystemVaultCipher> {
    path: PathBuf,
    cipher: C,
}

impl VaultStore<SystemVaultCipher> {
    pub fn system() -> Self {
        Self::new(default_vault_path(), WindowsDpapiCipher)
    }
}

impl<C: VaultCipher> VaultStore<C> {
    pub fn new(path: PathBuf, cipher: C) -> Self {
        Self { path, cipher }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Vec<WindscribeAccount>, VaultError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let encrypted = fs::read(&self.path).map_err(|err| VaultError::Io(err.to_string()))?;
        if encrypted.is_empty() {
            return Err(VaultError::Corrupted("Vault file is empty.".to_owned()));
        }
        let plain = self.cipher.unprotect(&encrypted)?;
        let accounts = serde_json::from_slice::<Vec<WindscribeAccount>>(&plain)
            .map_err(|err| VaultError::Corrupted(err.to_string()))?;

        Ok(accounts
            .into_iter()
            .filter(|acc| !acc.username.trim().is_empty())
            .collect())
    }

    pub fn save(&self, accounts: &[WindscribeAccount]) -> Result<(), VaultError> {
        let parent = self.path.parent().ok_or_else(|| {
            VaultError::InvalidPath("Vault directory could not be resolved.".to_owned())
        })?;
        fs::create_dir_all(parent).map_err(|err| VaultError::Io(err.to_string()))?;

        let plain =
            serde_json::to_vec(accounts).map_err(|err| VaultError::Encryption(err.to_string()))?;
        let encrypted = self.cipher.protect(&plain)?;

        let temp = self.path.with_extension("vault.tmp");
        fs::write(&temp, encrypted).map_err(|err| VaultError::Io(err.to_string()))?;
        if let Err(err) = replace_file_atomically(&temp, &self.path) {
            let _ = fs::remove_file(&temp);
            return Err(err);
        }
        Ok(())
    }

    pub fn backup_corrupted(&self) -> Result<Option<PathBuf>, VaultError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let stamp = unix_seconds();
        let backup = self
            .path
            .with_file_name(format!("accounts.vault.corrupted.{stamp}"));
        fs::rename(&self.path, &backup).map_err(|err| VaultError::Io(err.to_string()))?;
        Ok(Some(backup))
    }
}
