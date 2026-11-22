use crate::app::AppState;
use crate::models::{InstallStateKind, ToastLevel};
use crate::ui::components::action_button::MainActionButton;
use crate::ui::components::status::{ApplicationStatus, InstallationInformation, LoadingIndicator};
use crate::ui::metrics::{CONTENT_PAD_X, HERO_HEIGHT, HERO_SLANT_DROP, SECTION_GAP};
use crate::ui::theme::ThemeTokens;
use egui::{pos2, Context, FontId, RichText, Rounding, Shape, Stroke, Ui, Vec2};

pub fn show(ui: &mut Ui, ctx: &Context, state: &mut AppState, tokens: &ThemeTokens) {
    let busy = state.is_busy();
    let animations = state.settings.animations_enabled;
    let action = state.status.primary_action();

    // Windscribe-inspired layered slanted hero surface
    let hero_width = ui.available_width();
    let (hero_rect, _) =
        ui.allocate_exact_size(Vec2::new(hero_width, HERO_HEIGHT), egui::Sense::hover());
    let poly = vec![
        hero_rect.min,
        pos2(hero_rect.max.x, hero_rect.min.y),
        pos2(hero_rect.max.x, hero_rect.max.y - HERO_SLANT_DROP),
        pos2(hero_rect.min.x, hero_rect.max.y),
    ];
    ui.painter()
        .add(Shape::convex_polygon(poly, tokens.bg_hero, Stroke::NONE));
    ui.painter().line_segment(
        [
            pos2(hero_rect.min.x, hero_rect.max.y),
            pos2(hero_rect.max.x, hero_rect.max.y - HERO_SLANT_DROP),
        ],
        Stroke::new(1.0_f32, tokens.border_subtle),
    );

    let mut hero_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(hero_rect.shrink2(Vec2::new(CONTENT_PAD_X, 12.0)))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );

    ApplicationStatus::show_badge(
        &mut hero_ui,
        ctx,
        tokens,
        &state.status,
        state.active_operation,
        animations,
    );
    hero_ui.add_space(6.0);

    let headline = match state.status.state_kind {
        InstallStateKind::Checking => "Checking Device",
        InstallStateKind::NotInstalled => "Windscribe Missing",
        InstallStateKind::InstalledUpToDate => "Windscribe Ready",
        InstallStateKind::InstalledUpdateAvailable => "Update Available",
        InstallStateKind::InstalledUnmanaged => "Windscribe Detected",
        InstallStateKind::Error => "Action Required",
    };
    hero_ui.label(
        RichText::new(headline)
            .font(FontId::proportional(17.5))
            .strong()
            .color(tokens.text_primary),
    );
    hero_ui.label(
        RichText::new(&state.status.summary)
            .font(FontId::proportional(10.5))
            .color(tokens.text_secondary),
    );
    hero_ui.add_space(8.0);

    if MainActionButton::show(
        &mut hero_ui,
        ctx,
        tokens,
        state.status.state_kind,
        action,
        busy,
        animations,
    ) {
        state.trigger_primary_action();
    }

    // Main content body below hero
    egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(CONTENT_PAD_X, 10.0))
        .show(ui, |ui| {
            if let Some(op) = state.active_operation {
                LoadingIndicator::show(ui, ctx, tokens, op.label(), animations);
                ui.add_space(SECTION_GAP);
            } else if let Some(err) = &state.status.error {
                egui::Frame::none()
                    .fill(tokens.red_soft)
                    .stroke(Stroke::new(1.0_f32, tokens.red.linear_multiply(0.55)))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(egui::Margin::symmetric(12.0, 9.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new(err)
                                .font(FontId::proportional(10.5))
                                .color(tokens.text_primary),
                        );
                    });
                ui.add_space(SECTION_GAP);
            }

            ApplicationStatus::show_status_tiles(ui, tokens, &state.status);
            ui.add_space(SECTION_GAP);

            if let Some(copied) = InstallationInformation::show(ui, ctx, tokens, &state.status) {
                state.push_toast(ToastLevel::Info, copied, None);
            }
        });
}
