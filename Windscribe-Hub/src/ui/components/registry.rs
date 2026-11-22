use crate::models::{RegistryKeyInfo, RegistryKeyState};
use crate::ui::components::controls::windscribe_button;
use crate::ui::metrics::CARD_RADIUS;
use crate::ui::theme::ThemeTokens;
use crate::utils::time::format_hms;
use egui::{Align, FontId, Layout, RichText, Rounding, Stroke, Ui};

pub struct RegistryCard;

impl RegistryCard {
    pub fn show(ui: &mut Ui, tokens: &ThemeTokens, info: &RegistryKeyInfo, busy: bool) -> bool {
        let mut clean_clicked = false;
        let (badge_text, badge_color, badge_bg) = match info.state {
            RegistryKeyState::Exists => ("EXISTS", tokens.amber, tokens.amber_soft),
            RegistryKeyState::NotFound => ("NOT FOUND", tokens.green, tokens.green_soft),
            RegistryKeyState::Unknown => ("UNSCANNED", tokens.gray, tokens.panel_alt),
            RegistryKeyState::Error => ("ERROR", tokens.red, tokens.red_soft),
        };

        egui::Frame::none()
            .fill(tokens.panel)
            .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
            .rounding(Rounding::same(CARD_RADIUS))
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(info.target.display_name())
                            .font(FontId::proportional(13.5))
                            .strong()
                            .color(tokens.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::Frame::none()
                            .fill(badge_bg)
                            .rounding(Rounding::same(10.0))
                            .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(badge_text)
                                        .font(FontId::proportional(9.0))
                                        .strong()
                                        .color(badge_color),
                                );
                            });
                    });
                });

                ui.add_space(4.0);
                ui.label(
                    RichText::new(info.target.registry_path())
                        .font(FontId::monospace(10.0))
                        .color(tokens.text_secondary),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(info.target.description())
                        .font(FontId::proportional(10.5))
                        .color(tokens.text_muted),
                );

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let checked_str = info
                        .last_checked
                        .map(format_hms)
                        .unwrap_or_else(|| "Never".to_owned());
                    ui.label(
                        RichText::new(format!("Checked: {checked_str}"))
                            .font(FontId::proportional(10.0))
                            .color(tokens.text_muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let backup_str = if info.backup_count > 0 {
                            format!("{} backup(s) available", info.backup_count)
                        } else {
                            "No backups yet".to_owned()
                        };
                        ui.label(
                            RichText::new(backup_str)
                                .font(FontId::proportional(10.0))
                                .color(if info.backup_count > 0 {
                                    tokens.accent
                                } else {
                                    tokens.text_muted
                                }),
                        );
                    });
                });

                if let Some(latest) = &info.latest_backup {
                    if let Some(name) = latest.file_name().and_then(|n| n.to_str()) {
                        ui.label(
                            RichText::new(format!("Latest backup: {name}"))
                                .font(FontId::monospace(9.5))
                                .color(tokens.text_secondary),
                        );
                    }
                }

                if let Some(err) = &info.last_error {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(err)
                            .font(FontId::proportional(10.0))
                            .color(tokens.red),
                    );
                }

                ui.add_space(10.0);
                let can_clean = !busy && info.state == RegistryKeyState::Exists;
                let btn_label = match info.state {
                    RegistryKeyState::Exists => "Back Up & Clean Key",
                    RegistryKeyState::NotFound => "Key Not Present",
                    RegistryKeyState::Unknown => "Scan Key First",
                    RegistryKeyState::Error => "Back Up & Clean Key",
                };
                let enable_btn = can_clean || (!busy && info.state == RegistryKeyState::Error);
                if windscribe_button(ui, tokens, btn_label, can_clean, enable_btn, true).clicked() {
                    clean_clicked = true;
                }
            });

        clean_clicked
    }
}
