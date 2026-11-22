use crate::app::AppState;
use crate::models::{InstallSource, WINDSCRIBE_PACKAGE_ID};
use crate::ui::components::controls::{windscribe_button, AnimatedPanel, SettingsPanel};
use crate::ui::components::status::LoadingIndicator;
use crate::ui::metrics::{CONTENT_PAD_X, CONTENT_PAD_Y, SECTION_GAP};
use crate::ui::theme::ThemeTokens;
use crate::utils::time::format_hms;
use egui::{Align, Context, FontId, Layout, RichText, Rounding, Stroke, Ui};

pub fn show(ui: &mut Ui, ctx: &Context, state: &mut AppState, tokens: &ThemeTokens) {
    let busy = state.is_busy();
    let animations = state.settings.animations_enabled;

    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(CONTENT_PAD_X, CONTENT_PAD_Y))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Tools & Diagnostics")
                    .font(FontId::proportional(16.0))
                    .strong()
                    .color(tokens.text_primary),
            );
            ui.label(
                RichText::new("Manage Windscribe WinGet packages and inspect Windscribe.exe.")
                    .font(FontId::proportional(10.5))
                    .color(tokens.text_muted),
            );
            ui.add_space(SECTION_GAP);

            if let Some(op) = state.active_operation {
                LoadingIndicator::show(ui, ctx, tokens, op.label(), animations);
                ui.add_space(SECTION_GAP);
            }

            // Section 1: WinGet Installation & Lifecycle
            SettingsPanel::show(ui, tokens, "WINDSCRIBE INSTALLATION", |ui| {
                let status_line = if !state.status.installed {
                    "Not installed on this Windows PC"
                } else if state.status.managed_by_winget && state.status.update_available {
                    "Installed via WinGet — update available"
                } else if state.status.managed_by_winget {
                    "Installed via WinGet — up to date"
                } else {
                    "Detected locally (unmanaged by WinGet)"
                };

                ui.label(
                    RichText::new(status_line)
                        .font(FontId::proportional(12.5))
                        .strong()
                        .color(tokens.text_primary),
                );
                if !state.status.winget_available {
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("WinGet CLI was not found or could not start.")
                            .font(FontId::proportional(10.5))
                            .color(tokens.amber),
                    );
                } else if state.status.installed && !state.status.managed_by_winget {
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(
                            "Update queries only run when installed through WinGet (Windscribe.Windscribe).",
                        )
                        .font(FontId::proportional(10.0))
                        .color(tokens.text_muted),
                    );
                }

                ui.add_space(10.0);
                ui.columns(2, |cols| {
                    if windscribe_button(&mut cols[0], tokens, "Detect / Refresh", false, !busy, true)
                        .clicked()
                    {
                        state.refresh_all_status();
                    }
                    if !state.status.installed {
                        if windscribe_button(
                            &mut cols[1],
                            tokens,
                            "Install via WinGet",
                            true,
                            !busy && state.status.winget_available,
                            true,
                        )
                        .clicked()
                        {
                            state.install_windscribe();
                        }
                    } else if state.status.update_available && state.status.managed_by_winget {
                        if windscribe_button(
                            &mut cols[1],
                            tokens,
                            "Update via WinGet",
                            true,
                            !busy,
                            true,
                        )
                        .clicked()
                        {
                            state.update_windscribe();
                        }
                    } else if windscribe_button(&mut cols[1], tokens, "Open Windscribe", true, !busy, true)
                        .clicked()
                    {
                        state.launch_windscribe();
                    }
                });

                if state.status.installed && state.status.update_available {
                    ui.add_space(8.0);
                    if windscribe_button(ui, tokens, "Open Existing Installation", false, !busy, true)
                        .clicked()
                    {
                        state.launch_windscribe();
                    }
                }
            });

            ui.add_space(SECTION_GAP);

            // Section 2: Process Information
            SettingsPanel::show(ui, tokens, "PROCESS INFORMATION", |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Windscribe.exe")
                                .font(FontId::monospace(12.5))
                                .strong()
                                .color(tokens.text_primary),
                        );
                        ui.label(
                            RichText::new(format!("State: {}", state.status.process.label()))
                                .font(FontId::proportional(11.0))
                                .color(if state.status.process.is_running() {
                                    tokens.green
                                } else {
                                    tokens.text_secondary
                                }),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if windscribe_button(ui, tokens, "Check Process", false, !busy, false)
                            .clicked()
                        {
                            state.refresh_process_only();
                        }
                    });
                });
            });

            ui.add_space(SECTION_GAP);

            // Section 3: Installation Details
            SettingsPanel::show(ui, tokens, "INSTALLATION DETAILS", |ui| {
                detail_item(ui, tokens, "Package ID", WINDSCRIBE_PACKAGE_ID);
                detail_item(
                    ui,
                    tokens,
                    "Install Source",
                    match state.status.source {
                        InstallSource::WinGet => "WinGet Package Manager",
                        InstallSource::LocalRegistry => "Windows Uninstall Registry",
                        InstallSource::LocalPath => "Standard Install Directory",
                        InstallSource::None => "Not installed",
                        InstallSource::Unknown => "Unknown",
                    },
                );
                detail_item(
                    ui,
                    tokens,
                    "Installed Version",
                    state.status.installed_version.as_deref().unwrap_or("—"),
                );
                detail_item(
                    ui,
                    tokens,
                    "Install Location",
                    &state
                        .status
                        .install_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Not located".to_owned()),
                );
                detail_item(
                    ui,
                    tokens,
                    "Last Checked",
                    &state
                        .status
                        .last_checked
                        .map(format_hms)
                        .unwrap_or_else(|| "Pending".to_owned()),
                );
            });

            ui.add_space(SECTION_GAP);

            // Section 4: Collapsible Diagnostics Log
            let mut expanded = state.tools_logs_expanded;
            let mut clear_logs = false;
            let logs_snapshot = state.logs.clone();
            AnimatedPanel::collapsible(
                ui,
                ctx,
                tokens,
                "tools_op_logs",
                &format!("OPERATION LOG ({})", logs_snapshot.len()),
                &mut expanded,
                animations,
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Recent background checks & WinGet output")
                                .font(FontId::proportional(10.0))
                                .color(tokens.text_muted),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Clear").clicked() {
                                clear_logs = true;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    egui::Frame::none()
                        .fill(tokens.bg_base)
                        .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(130.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    if logs_snapshot.is_empty() {
                                        ui.label(
                                            RichText::new("No operation messages yet.")
                                                .font(FontId::proportional(10.0))
                                                .color(tokens.text_muted),
                                        );
                                    }
                                    for line in &logs_snapshot {
                                        ui.label(
                                            RichText::new(line)
                                                .font(FontId::monospace(9.5))
                                                .color(tokens.text_secondary),
                                        );
                                    }
                                });
                        });
                },
            );
            state.tools_logs_expanded = expanded;
            if clear_logs {
                state.logs.clear();
            }
        });
}

fn detail_item(ui: &mut Ui, tokens: &ThemeTokens, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(k)
                .font(FontId::proportional(11.0))
                .color(tokens.text_muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(v)
                    .font(FontId::proportional(11.0))
                    .color(tokens.text_primary),
            );
        });
    });
    ui.add_space(3.0);
}
