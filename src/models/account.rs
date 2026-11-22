use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindscribeAccount {
    pub id: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountDraft {
    pub editing_id: Option<String>,
    pub username: String,
    pub password: String,
    pub note: String,
    pub password_visible: bool,
    pub validation_error: Option<String>,
}

impl AccountDraft {
    pub fn clear(&mut self) {
        self.editing_id = None;
        self.username.clear();
        self.password.clear();
        self.note.clear();
        self.password_visible = false;
        self.validation_error = None;
    }

    pub fn load_from(&mut self, account: &WindscribeAccount) {
        self.editing_id = Some(account.id.clone());
        self.username = account.username.clone();
        self.password = account.password.clone();
        self.note = account.note.clone();
        self.password_visible = false;
        self.validation_error = None;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum VaultLoadState {
    #[default]
    Loaded,
    Corrupted {
        error: String,
    },
}
