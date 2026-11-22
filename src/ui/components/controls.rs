use crate::ui::animations::{animate_fast, ease_out_cubic, lerp_color, lerp_f32};
use crate::ui::metrics::{CARD_PADDING, CARD_RADIUS, TOGGLE_HEIGHT, TOGGLE_WIDTH};
use crate::ui::theme::ThemeTokens;
use egui::{
    pos2, Color32, Context, FontId, Id, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};

pub struct CustomToggle;

impl CustomToggle {
    pub fn show(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        id_salt: &str,
        checked: &mut bool,
        animations_enabled: bool,
    ) -> bool {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(TOGGLE_WIDTH, TOGGLE_HEIGHT), Sense::click());
        let mut changed = false;
        if response.clicked() {
            *checked = !*checked;
            changed = true;
        }

        let t = animate_fast(
            ctx,
            Id::new(("toggle", id_salt)),
            *checked,
            animations_enabled,
        );
        let track_color = lerp_color(tokens.panel_hover, tokens.accent, t);
        let border_color = lerp_color(tokens.border, tokens.accent, t);
        ui.painter()
            .rect_filled(rect, TOGGLE_HEIGHT * 0.5, track_color);
        ui.painter().rect_stroke(
            rect,
            TOGGLE_HEIGHT * 0.5,
            Stroke::new(1.0_f32, border_color),
        );

        let knob_r = (TOGGLE_HEIGHT - 6.0) * 0.5;
        let x = lerp_f32(
            rect.min.x + knob_r + 3.0,
            rect.max.x - knob_r - 3.0,
            ease_out_cubic(t),
        );
        let knob_color = if *checked {
            tokens.accent_text
        } else {
            tokens.text_secondary
        };
        ui.painter()
            .circle_filled(pos2(x, rect.center().y), knob_r, knob_color);

        changed
    }
}

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn show(
        ui: &mut Ui,
        tokens: &ThemeTokens,
        title: &str,
        add_contents: impl FnOnce(&mut Ui),
    ) {
        egui::Frame::none()
            .fill(tokens.panel)
            .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
            .rounding(Rounding::same(CARD_RADIUS))
            .inner_margin(egui::Margin::same(CARD_PADDING))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    RichText::new(title)
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(tokens.text_muted),
                );
                ui.add_space(8.0);
                add_contents(ui);
            });
    }
}

pub struct CustomTooltip;

impl CustomTooltip {
    pub fn show(response: &Response, tokens: &ThemeTokens, text: &str) {
        if !response.hovered() || text.trim().is_empty() {
            return;
        }
        response.clone().on_hover_ui_at_pointer(|ui| {
            egui::Frame::none()
                .fill(tokens.bg_header)
                .stroke(Stroke::new(1.0_f32, tokens.border))
                .rounding(Rounding::same(6.0))
                .inner_margin(egui::Margin::symmetric(8.0, 5.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(text)
                            .font(FontId::proportional(10.5))
                            .color(tokens.text_primary),
                    );
                });
        });
    }
}

pub struct AnimatedPanel;

impl AnimatedPanel {
    #[allow(clippy::too_many_arguments)]
    pub fn collapsible(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        id_salt: &str,
        title: &str,
        expanded: &mut bool,
        animations_enabled: bool,
        add_contents: impl FnOnce(&mut Ui),
    ) {
        egui::Frame::none()
            .fill(tokens.panel)
            .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
            .rounding(Rounding::same(CARD_RADIUS))
            .inner_margin(egui::Margin::same(CARD_PADDING))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let (hdr_rect, hdr_resp) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
                if hdr_resp.clicked() {
                    *expanded = !*expanded;
                }
                ui.painter().text(
                    pos2(hdr_rect.min.x, hdr_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    title,
                    FontId::proportional(10.5),
                    tokens.text_secondary,
                );
                let chevron = if *expanded { "▾" } else { "▸" };
                ui.painter().text(
                    pos2(hdr_rect.max.x - 4.0, hdr_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    chevron,
                    FontId::proportional(12.0),
                    tokens.text_muted,
                );

                let t = animate_fast(
                    ctx,
                    Id::new(("panel_open", id_salt)),
                    *expanded,
                    animations_enabled,
                );
                if t > 0.05 {
                    ui.add_space(8.0 * t);
                    add_contents(ui);
                }
            });
    }
}

pub fn windscribe_button(
    ui: &mut Ui,
    tokens: &ThemeTokens,
    label: &str,
    accent: bool,
    enabled: bool,
    full_width: bool,
) -> Response {
    let min_w = if full_width {
        ui.available_width()
    } else {
        86.0
    };
    let fill = if !enabled {
        tokens.panel_alt
    } else if accent {
        tokens.accent
    } else {
        tokens.panel_alt
    };
    let text_color = if !enabled {
        tokens.text_muted
    } else if accent {
        tokens.accent_text
    } else {
        tokens.text_primary
    };
    let stroke = if !enabled {
        Stroke::new(1.0_f32, tokens.border_subtle)
    } else if accent {
        Stroke::NONE
    } else {
        Stroke::new(1.0_f32, tokens.border)
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(11.5))
                .strong()
                .color(text_color),
        )
        .fill(fill)
        .stroke(stroke)
        .rounding(Rounding::same(8.0))
        .min_size(Vec2::new(min_w, 32.0)),
    )
}

pub fn danger_button(ui: &mut Ui, tokens: &ThemeTokens, label: &str, enabled: bool) -> Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(
            RichText::new(label)
                .font(FontId::proportional(11.5))
                .strong()
                .color(if enabled {
                    Color32::WHITE
                } else {
                    tokens.text_muted
                }),
        )
        .fill(if enabled {
            tokens.red
        } else {
            tokens.panel_alt
        })
        .rounding(Rounding::same(8.0))
        .min_size(Vec2::new(96.0, 32.0)),
    )
}
