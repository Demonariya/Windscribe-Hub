use crate::models::{AccountDraft, WindscribeAccount};
use crate::utils::time::unix_seconds;
use rand::{seq::SliceRandom, Rng};
use uuid::Uuid;

pub fn validate_draft(draft: &AccountDraft) -> Result<(String, String, String), String> {
    let username = draft.username.trim().to_owned();
    let password = draft.password.clone();
    let note = draft.note.trim().to_owned();

    if username.is_empty() {
        return Err("Enter a Windscribe username before saving.".to_owned());
    }
    if password.is_empty() {
        return Err("Enter a password before saving.".to_owned());
    }
    Ok((username, password, note))
}

pub enum SaveResult {
    Created { id: String, username: String },
    Updated { id: String, username: String },
}

pub fn apply_draft(
    accounts: &mut Vec<WindscribeAccount>,
    draft: &AccountDraft,
) -> Result<SaveResult, String> {
    let (username, password, note) = validate_draft(draft)?;
    let now = unix_seconds();

    if let Some(editing_id) = draft.editing_id.as_deref() {
        if let Some(existing) = accounts.iter_mut().find(|acc| acc.id == editing_id) {
            existing.username = username.clone();
            existing.password = password;
            existing.note = note;
            existing.updated_at = now;
            return Ok(SaveResult::Updated {
                id: existing.id.clone(),
                username,
            });
        }
    }

    if let Some(existing) = accounts
        .iter_mut()
        .find(|acc| acc.username.eq_ignore_ascii_case(&username))
    {
        existing.password = password;
        existing.note = note;
        existing.updated_at = now;
        return Ok(SaveResult::Updated {
            id: existing.id.clone(),
            username,
        });
    }

    let id = Uuid::new_v4().to_string();
    accounts.insert(
        0,
        WindscribeAccount {
            id: id.clone(),
            username: username.clone(),
            password,
            note,
            updated_at: now,
        },
    );
    Ok(SaveResult::Created { id, username })
}

pub fn remove_account(
    accounts: &mut Vec<WindscribeAccount>,
    id: &str,
) -> Option<WindscribeAccount> {
    let idx = accounts.iter().position(|acc| acc.id == id)?;
    Some(accounts.remove(idx))
}

pub fn filter_accounts<'a>(
    accounts: &'a [WindscribeAccount],
    query: &str,
) -> Vec<&'a WindscribeAccount> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return accounts.iter().collect();
    }
    accounts
        .iter()
        .filter(|acc| {
            acc.username.to_ascii_lowercase().contains(&needle)
                || acc.note.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

pub fn suggest_username() -> String {
    let mut rng = rand::thread_rng();
    let suffix: String = (0..9)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect::<String>()
        .to_ascii_lowercase();
    format!("ws_{suffix}")
}

pub fn generate_strong_password(length: usize) -> String {
    let target_len = length.max(12);
    let mut rng = rand::thread_rng();
    let groups = [
        "abcdefghijkmnopqrstuvwxyz",
        "ABCDEFGHJKLMNPQRSTUVWXYZ",
        "23456789",
        "!@#$%*-_=+?",
    ];
    let mut chars = Vec::with_capacity(target_len);
    for group in groups {
        let idx = rng.gen_range(0..group.len());
        chars.push(group.as_bytes()[idx] as char);
    }
    let alphabet = groups.concat();
    while chars.len() < target_len {
        let idx = rng.gen_range(0..alphabet.len());
        chars.push(alphabet.as_bytes()[idx] as char);
    }
    chars.shuffle(&mut rng);
    chars.into_iter().collect()
}
