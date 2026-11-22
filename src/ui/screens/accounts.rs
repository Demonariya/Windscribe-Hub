use crate::app::AppState;
use crate::models::{ToastLevel, VaultLoadState};
use crate::services::accounts as account_service;
use crate::ui::components::accounts::{
    AccountCard, AccountCardAction, AccountEditor, AccountEditorEvent,
};
use crate::ui::components::controls::{danger_button, windscribe_button};
use crate::ui::icons::paint_search_icon;
use crate::ui::metrics::{CARD_RADIUS, CONTENT_PAD_X, CONTENT_PAD_Y, SECTION_GAP};
use crate::ui::theme::ThemeTokens;
use egui::{Align, Context, FontId, Layout, RichText, Rounding, Sense, Stroke, Ui, Vec2};

pub fn show(ui: &mut Ui, ctx: &Context, state: &mut AppState, tokens: &ThemeTokens) {
    let animations = state.settings.animations_enabled;

    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(CONTENT_PAD_X, CONTENT_PAD_Y))
        .show(ui, |ui| {
            // Header row + "+ Add" button
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Account Vault")
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(tokens.text_primary),
                    );
                    ui.label(
                        RichText::new(format!("{} encrypted login(s)", state.accounts.len()))
                            .font(FontId::proportional(10.5))
                            .color(tokens.text_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if state.editor_open {
                        "Close Editor"
                    } else {
                        "+ Add Account"
                    };
                    if windscribe_button(ui, tokens, label, !state.editor_open, true, false)
                        .clicked()
                    {
                        if state.editor_open {
                            state.editor_open = false;
                            state.account_draft.clear();
                        } else {
                            state.account_draft.clear();
                            state.editor_open = true;
                        }
                    }
                });
            });

            ui.add_space(SECTION_GAP);

            // Corrupted vault recovery notice (never silently overwrites corrupted bytes)
            if let VaultLoadState::Corrupted { error } = state.vault_state.clone() {
                egui::Frame::none()
                    .fill(tokens.red_soft)
                    .stroke(Stroke::new(1.0_f32, tokens.red))
                    .rounding(Rounding::same(CARD_RADIUS))
                    .inner_margin(egui::Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("Encrypted vault could not be unlocked")
                                .font(FontId::proportional(12.0))
                                .strong()
                                .color(tokens.text_primary),
                        );
                        ui.label(
                            RichText::new(error)
                                .font(FontId::proportional(10.5))
                                .color(tokens.text_secondary),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if windscribe_button(ui, tokens, "Retry Unlock", false, true, false)
                                .clicked()
                            {
                                state.load_vault();
                            }
                            if danger_button(ui, tokens, "Back Up & Reset", true).clicked() {
                                state.backup_and_reset_corrupted_vault();
                            }
                        });
                    });
                ui.add_space(SECTION_GAP);
            }

            if state.editor_open {
                match AccountEditor::show(ui, tokens, &mut state.account_draft) {
                    AccountEditorEvent::Save => state.save_account_draft(),
                    AccountEditorEvent::Cancel => {
                        state.editor_open = false;
                        state.account_draft.clear();
                    }
                    AccountEditorEvent::SuggestUsername => {
                        state.account_draft.username = account_service::suggest_username();
                        state.account_draft.validation_error = None;
                    }
                    AccountEditorEvent::GeneratePassword => {
                        state.account_draft.password =
                            account_service::generate_strong_password(20);
                        state.account_draft.password_visible = true;
                        state.account_draft.validation_error = None;
                    }
                    AccountEditorEvent::None => {}
                }
                ui.add_space(SECTION_GAP);
            }

            // Search bar
            egui::Frame::none()
                .fill(tokens.panel)
                .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
                .rounding(Rounding::same(10.0))
                .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (icon_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                        paint_search_icon(
                            ui.painter(),
                            icon_rect.center(),
                            6.5,
                            tokens.text_muted,
                        );
                        let clear_w = if state.account_search.is_empty() { 0.0 } else { 28.0 };
                        ui.add(
                            egui::TextEdit::singleline(&mut state.account_search)
                                .hint_text("Search saved accounts…")
                                .frame(false)
                                .desired_width((ui.available_width() - clear_w).max(100.0)),
                        );
                        if !state.account_search.is_empty()
                            && ui.small_button("×").clicked()
                        {
                            state.account_search.clear();
                        }
                    });
                });

            ui.add_space(10.0);

            let filtered: Vec<_> = account_service::filter_accounts(
                &state.accounts,
                &state.account_search,
            )
            .into_iter()
            .cloned()
            .collect();

            if filtered.is_empty() {
                egui::Frame::none()
                    .fill(tokens.panel)
                    .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
                    .rounding(Rounding::same(CARD_RADIUS))
                    .inner_margin(egui::Margin::same(22.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if state.accounts.is_empty() {
                                    "No saved Windscribe accounts"
                                } else {
                                    "No accounts match your search"
                                })
                                .font(FontId::proportional(13.0))
                                .strong()
                                .color(tokens.text_primary),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "Add credentials locally. Encrypted per Windows user via DPAPI.",
                                )
                                .font(FontId::proportional(10.5))
                                .color(tokens.text_muted),
                            );
                        });
                    });
            } else {
                for account in &filtered {
                    let revealed = state.revealed_accounts.contains(&account.id);
                    match AccountCard::show(
                        ui,
                        ctx,
                        tokens,
                        account,
                        revealed,
                        animations,
                    ) {
                        AccountCardAction::ToggleReveal => {
                            if revealed {
                                state.revealed_accounts.remove(&account.id);
                            } else {
                                state.revealed_accounts.insert(account.id.clone());
                            }
                        }
                        AccountCardAction::CopyUsername => {
                            ctx.copy_text(account.username.clone());
                            state.push_toast(
                                ToastLevel::Info,
                                "Username copied",
                                Some(account.username.clone()),
                            );
                        }
                        AccountCardAction::CopyPassword => {
                            ctx.copy_text(account.password.clone());
                            state.push_toast(
                                ToastLevel::Success,
                                "Password copied to clipboard",
                                None,
                            );
                        }
                        AccountCardAction::Edit => {
                            state.account_draft.load_from(account);
                            state.editor_open = true;
                        }
                        AccountCardAction::Delete => {
                            state.pending_delete_account = Some(account.id.clone());
                        }
                        AccountCardAction::None => {}
                    }
                    ui.add_space(8.0);
                }
            }
        });
}
