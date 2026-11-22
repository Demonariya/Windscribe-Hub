use crate::models::{ToastLevel, ToastMessage};
use crate::ui::animations::animate_fast;
use crate::ui::components::controls::{danger_button, windscribe_button};
use crate::ui::metrics::CARD_RADIUS;
use crate::ui::theme::ThemeTokens;
use egui::{
    pos2, Align, Area, Color32, Context, FontId, Id, Layout, Order, RichText, Rounding, Sense,
    Stroke, Vec2,
};

pub struct ConfirmationDialog;

pub struct ConfirmationSpec<'a> {
    pub id_salt: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub highlight: Option<&'a str>,
    pub confirm_label: &'a str,
    pub destructive: bool,
}

pub enum DialogChoice {
    Pending,
    Confirmed,
    Cancelled,
}

impl ConfirmationDialog {
    pub fn show(
        ctx: &Context,
        tokens: &ThemeTokens,
        spec: ConfirmationSpec<'_>,
        animations_enabled: bool,
    ) -> DialogChoice {
        let mut choice = DialogChoice::Pending;
        let t = animate_fast(
            ctx,
            Id::new(("dlg_open", spec.id_salt)),
            true,
            animations_enabled,
        );
        let screen = ctx.screen_rect();

        Area::new(Id::new(("dlg_scrim", spec.id_salt)))
            .order(Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                let alpha = ((tokens.overlay_scrim.a() as f32) * t).round() as u8;
                let c = tokens.overlay_scrim;
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha),
                );
                if resp.clicked() {
                    choice = DialogChoice::Cancelled;
                }
            });

        let modal_w = (screen.width() - 40.0).min(340.0);
        Area::new(Id::new(("dlg_body", spec.id_salt)))
            .order(Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(tokens.panel)
                    .stroke(Stroke::new(1.2_f32, tokens.border))
                    .rounding(Rounding::same(CARD_RADIUS))
                    .inner_margin(egui::Margin::same(16.0))
                    .show(ui, |ui| {
                        ui.set_width(modal_w - 32.0);
                        ui.label(
                            RichText::new(spec.title)
                                .font(FontId::proportional(14.5))
                                .strong()
                                .color(tokens.text_primary),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(spec.body)
                                .font(FontId::proportional(11.5))
                                .color(tokens.text_secondary),
                        );
                        if let Some(hl) = spec.highlight {
                            ui.add_space(8.0);
                            egui::Frame::none()
                                .fill(tokens.bg_header)
                                .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
                                .rounding(Rounding::same(8.0))
                                .inner_margin(egui::Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(
                                        RichText::new(hl)
                                            .font(FontId::monospace(10.0))
                                            .color(tokens.amber),
                                    );
                                });
                        }
                        ui.add_space(14.0);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if spec.destructive {
                                if danger_button(ui, tokens, spec.confirm_label, true).clicked() {
                                    choice = DialogChoice::Confirmed;
                                }
                            } else if windscribe_button(
                                ui,
                                tokens,
                                spec.confirm_label,
                                true,
                                true,
                                false,
                            )
                            .clicked()
                            {
                                choice = DialogChoice::Confirmed;
                            }
                            if windscribe_button(ui, tokens, "Cancel", false, true, false).clicked()
                            {
                                choice = DialogChoice::Cancelled;
                            }
                        });
                    });
            });

        choice
    }
}

pub struct NotificationToast;

impl NotificationToast {
    pub fn show_stack(
        ctx: &Context,
        tokens: &ThemeTokens,
        toasts: &mut Vec<ToastMessage>,
        animations_enabled: bool,
    ) {
        toasts.retain(|t| t.created_at.elapsed().as_secs_f32() < t.duration_secs);
        if toasts.is_empty() {
            return;
        }
        if animations_enabled {
            ctx.request_repaint();
        }

        let screen = ctx.screen_rect();
        let toast_w = (screen.width() - 32.0).min(360.0);
        let mut dismiss_id: Option<u64> = None;

        Area::new(Id::new("toast_stack"))
            .order(Order::Tooltip)
            .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -14.0))
            .show(ctx, |ui| {
                ui.set_width(toast_w);
                for toast in toasts.iter().rev().take(3) {
                    let accent = match toast.level {
                        ToastLevel::Info => tokens.accent,
                        ToastLevel::Success => tokens.green,
                        ToastLevel::Warning => tokens.amber,
                        ToastLevel::Error => tokens.red,
                    };
                    egui::Frame::none()
                        .fill(tokens.bg_header)
                        .stroke(Stroke::new(1.0_f32, accent.linear_multiply(0.65)))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(egui::Margin::symmetric(12.0, 9.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let (bar, _) =
                                    ui.allocate_exact_size(Vec2::new(3.0, 22.0), Sense::hover());
                                ui.painter().rect_filled(bar, 1.5, accent);
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&toast.title)
                                            .font(FontId::proportional(11.5))
                                            .strong()
                                            .color(tokens.text_primary),
                                    );
                                    if let Some(detail) = &toast.detail {
                                        ui.label(
                                            RichText::new(detail)
                                                .font(FontId::proportional(10.0))
                                                .color(tokens.text_secondary),
                                        );
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let (x_rect, x_resp) =
                                        ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                                    let c = x_rect.center();
                                    let fg = if x_resp.hovered() {
                                        tokens.text_primary
                                    } else {
                                        tokens.text_muted
                                    };
                                    ui.painter().line_segment(
                                        [pos2(c.x - 4.0, c.y - 4.0), pos2(c.x + 4.0, c.y + 4.0)],
                                        Stroke::new(1.3_f32, fg),
                                    );
                                    ui.painter().line_segment(
                                        [pos2(c.x - 4.0, c.y + 4.0), pos2(c.x + 4.0, c.y - 4.0)],
                                        Stroke::new(1.3_f32, fg),
                                    );
                                    if x_resp.clicked() {
                                        dismiss_id = Some(toast.id);
                                    }
                                });
                            });
                        });
                    ui.add_space(6.0);
                }
            });

        if let Some(id) = dismiss_id {
            toasts.retain(|t| t.id != id);
        }
    }
}
