use crate::app::AppState;
use crate::models::{ThemeMode, ToastLevel};
use crate::ui::components::controls::{CustomToggle, SettingsPanel};
use crate::ui::metrics::{CONTENT_PAD_X, CONTENT_PAD_Y, SECTION_GAP};
use crate::ui::theme::ThemeTokens;
use egui::{Align, Context, FontId, Layout, RichText, Rounding, Stroke, Ui};

pub fn show(ui: &mut Ui, ctx: &Context, state: &mut AppState, tokens: &ThemeTokens) {
    let anim = state.settings.animations_enabled;
    let mut settings_changed = false;

    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(CONTENT_PAD_X, CONTENT_PAD_Y))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Preferences")
                    .font(FontId::proportional(16.0))
                    .strong()
                    .color(tokens.text_primary),
            );
            ui.label(
                RichText::new("Customize Windscribe Hub appearance, automation, and storage.")
                    .font(FontId::proportional(10.5))
                    .color(tokens.text_muted),
            );
            ui.add_space(SECTION_GAP);

            // Appearance
            SettingsPanel::show(ui, tokens, "APPEARANCE", |ui| {
                ui.label(
                    RichText::new("Theme")
                        .font(FontId::proportional(11.5))
                        .color(tokens.text_secondary),
                );
                ui.add_space(6.0);
                ui.columns(3, |cols| {
                    for (idx, &mode) in ThemeMode::ALL.iter().enumerate() {
                        let active = state.settings.theme == mode;
                        let resp = cols[idx].add(
                            egui::Button::new(
                                RichText::new(mode.label())
                                    .font(FontId::proportional(11.0))
                                    .strong()
                                    .color(if active {
                                        tokens.accent_text
                                    } else {
                                        tokens.text_primary
                                    }),
                            )
                            .fill(if active {
                                tokens.accent
                            } else {
                                tokens.panel_alt
                            })
                            .stroke(if active {
                                Stroke::NONE
                            } else {
                                Stroke::new(1.0_f32, tokens.border_subtle)
                            })
                            .rounding(Rounding::same(8.0))
                            .min_size(egui::Vec2::new(cols[idx].available_width(), 30.0)),
                        );
                        if resp.clicked() && !active {
                            state.settings.theme = mode;
                            settings_changed = true;
                        }
                    }
                });

                ui.add_space(10.0);
                if toggle_row(
                    ui,
                    ctx,
                    tokens,
                    "Enable UI animations",
                    "Smooth panel slides, toggles, and status pulses",
                    "pref_animations",
                    &mut state.settings.animations_enabled,
                    anim,
                ) {
                    settings_changed = true;
                }
            });

            ui.add_space(SECTION_GAP);

            // Behavior
            SettingsPanel::show(ui, tokens, "BEHAVIOR & STARTUP", |ui| {
                let mut startup = state.settings.launch_on_startup;
                if toggle_row(
                    ui,
                    ctx,
                    tokens,
                    "Launch at Windows startup",
                    "Register Windscribe Hub in HKCU\\...\\Run",
                    "pref_startup",
                    &mut startup,
                    anim,
                ) {
                    state.toggle_startup(startup);
                }

                ui.add_space(8.0);
                if toggle_row(
                    ui,
                    ctx,
                    tokens,
                    "Auto-refresh process status",
                    "Check Windscribe.exe periodically in background",
                    "pref_autorefresh",
                    &mut state.settings.auto_refresh,
                    anim,
                ) {
                    settings_changed = true;
                }

                if state.settings.auto_refresh {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Refresh interval")
                                .font(FontId::proportional(11.0))
                                .color(tokens.text_secondary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            for secs in [60u64, 30, 15] {
                                let selected = state.settings.refresh_interval_secs == secs;
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(format!("{secs}s"))
                                                .font(FontId::proportional(10.5))
                                                .color(if selected {
                                                    tokens.accent_text
                                                } else {
                                                    tokens.text_secondary
                                                }),
                                        )
                                        .fill(if selected {
                                            tokens.accent
                                        } else {
                                            tokens.panel_alt
                                        })
                                        .rounding(Rounding::same(6.0)),
                                    )
                                    .clicked()
                                {
                                    state.settings.refresh_interval_secs = secs;
                                    settings_changed = true;
                                }
                            }
                        });
                    });
                }

                ui.add_space(8.0);
                if toggle_row(
                    ui,
                    ctx,
                    tokens,
                    "In-app toast notifications",
                    "Show feedback overlays for vault and cleanup actions",
                    "pref_toasts",
                    &mut state.settings.show_toasts,
                    anim,
                ) {
                    settings_changed = true;
                }
            });

            ui.add_space(SECTION_GAP);

            // Data & Backup Locations
            SettingsPanel::show(ui, tokens, "DATA & BACKUP PATHS", |ui| {
                let vault_str = state.vault_path.display().to_string();
                let settings_str = state.settings_path.display().to_string();
                let backup_str = state.backup_dir.display().to_string();

                if path_row(ui, ctx, tokens, "Account Vault (DPAPI)", &vault_str) {
                    state.push_toast(ToastLevel::Info, "Vault path copied", None);
                }
                ui.add_space(6.0);
                if path_row(ui, ctx, tokens, "Settings File", &settings_str) {
                    state.push_toast(ToastLevel::Info, "Settings path copied", None);
                }
                ui.add_space(6.0);
                if path_row(ui, ctx, tokens, "Registry Backups", &backup_str) {
                    state.push_toast(ToastLevel::Info, "Backup path copied", None);
                }
            });

            ui.add_space(SECTION_GAP);

            // About
            SettingsPanel::show(ui, tokens, "ABOUT WINDSCRIBE HUB", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Windscribe Hub")
                            .font(FontId::proportional(12.5))
                            .strong()
                            .color(tokens.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                                .font(FontId::monospace(10.5))
                                .color(tokens.accent),
                        );
                    });
                });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Native Windows utility for managing the official Windscribe desktop installation, DPAPI-encrypted account vault, process status, and verified registry cleanup. Does not establish VPN connections.",
                    )
                    .font(FontId::proportional(10.0))
                    .color(tokens.text_muted),
                );
            });
        });

    if settings_changed {
        state.persist_settings();
    }
}

#[allow(clippy::too_many_arguments)]
fn toggle_row(
    ui: &mut Ui,
    ctx: &Context,
    tokens: &ThemeTokens,
    title: &str,
    subtitle: &str,
    id_salt: &str,
    value: &mut bool,
    animations_enabled: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width((ui.available_width() - 52.0).max(120.0));
            ui.label(
                RichText::new(title)
                    .font(FontId::proportional(11.5))
                    .strong()
                    .color(tokens.text_primary),
            );
            ui.label(
                RichText::new(subtitle)
                    .font(FontId::proportional(9.5))
                    .color(tokens.text_muted),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = CustomToggle::show(ui, ctx, tokens, id_salt, value, animations_enabled);
        });
    });
    changed
}

fn path_row(ui: &mut Ui, ctx: &Context, tokens: &ThemeTokens, label: &str, path: &str) -> bool {
    let mut copied = false;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(label)
                    .font(FontId::proportional(10.5))
                    .color(tokens.text_secondary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Copy").clicked() {
                    ctx.copy_text(path.to_owned());
                    copied = true;
                }
            });
        });
        ui.label(
            RichText::new(path)
                .font(FontId::monospace(9.5))
                .color(tokens.text_muted),
        );
    });
    copied
}
