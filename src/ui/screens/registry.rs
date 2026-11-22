use crate::app::AppState;
use crate::ui::components::controls::windscribe_button;
use crate::ui::components::registry::RegistryCard;
use crate::ui::metrics::{CARD_RADIUS, CONTENT_PAD_X, CONTENT_PAD_Y, SECTION_GAP};
use crate::ui::theme::ThemeTokens;
use egui::{Align, FontId, Layout, RichText, Rounding, Stroke, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState, tokens: &ThemeTokens) {
    let busy = state.is_busy();

    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(CONTENT_PAD_X, CONTENT_PAD_Y))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Registry Cleaner")
                            .font(FontId::proportional(16.0))
                            .strong()
                            .color(tokens.text_primary),
                    );
                    ui.label(
                        RichText::new("Verified .reg backup before removal")
                            .font(FontId::proportional(10.5))
                            .color(tokens.text_muted),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if windscribe_button(ui, tokens, "Re-scan Keys", false, !busy, false).clicked()
                    {
                        state.scan_registry();
                    }
                });
            });

            ui.add_space(SECTION_GAP);

            egui::Frame::none()
                .fill(tokens.amber_soft)
                .stroke(Stroke::new(1.0_f32, tokens.amber.linear_multiply(0.5)))
                .rounding(Rounding::same(CARD_RADIUS))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        RichText::new("Protected Registry Workflow")
                            .font(FontId::proportional(11.5))
                            .strong()
                            .color(tokens.amber),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(format!(
                            "Before deleting any key, Windscribe Hub exports and verifies a timestamped .reg backup inside:\n{}",
                            state.backup_dir.display()
                        ))
                        .font(FontId::proportional(10.0))
                        .color(tokens.text_secondary),
                    );
                });

            ui.add_space(SECTION_GAP);

            let keys = state.registry_keys.clone();
            for info in &keys {
                if RegistryCard::show(ui, tokens, info, busy) {
                    state.pending_registry_clean = Some(info.target);
                }
                ui.add_space(10.0);
            }
        });
}
