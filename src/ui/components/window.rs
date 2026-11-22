use crate::ui::icons::{paint_window_controls, WindowControlKind};
use crate::ui::metrics::{TITLE_BAR_HEIGHT, WINDOW_CORNER_RADIUS};
use crate::ui::theme::ThemeTokens;
use egui::{
    pos2, Align, Color32, Context, FontId, Id, Layout, RichText, Rounding, Sense, Stroke, Ui, Vec2,
    ViewportCommand,
};

pub struct WindscribeWindow;

impl WindscribeWindow {
    pub fn show(
        ctx: &Context,
        tokens: &ThemeTokens,
        title: &str,
        add_contents: impl FnOnce(&mut Ui),
    ) {
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let rounding = if is_maximized {
            Rounding::ZERO
        } else {
            Rounding::same(WINDOW_CORNER_RADIUS)
        };

        let frame = egui::Frame::none()
            .fill(tokens.bg_base)
            .stroke(if is_maximized {
                Stroke::NONE
            } else {
                Stroke::new(1.0_f32, tokens.border)
            })
            .rounding(rounding);

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            CustomTitleBar::show(ui, ctx, tokens, title, is_maximized);
            add_contents(ui);
        });
    }
}

pub struct CustomTitleBar;

impl CustomTitleBar {
    pub fn show(ui: &mut Ui, ctx: &Context, tokens: &ThemeTokens, title: &str, is_maximized: bool) {
        let width = ui.available_width();
        let (bar_rect, _) =
            ui.allocate_exact_size(Vec2::new(width, TITLE_BAR_HEIGHT), Sense::hover());

        let rounding = if is_maximized {
            Rounding::ZERO
        } else {
            Rounding {
                nw: WINDOW_CORNER_RADIUS,
                ne: WINDOW_CORNER_RADIUS,
                sw: 0.0,
                se: 0.0,
            }
        };

        ui.painter()
            .rect_filled(bar_rect, rounding, tokens.bg_header);

        let drag_rect =
            egui::Rect::from_min_max(bar_rect.min, pos2(bar_rect.max.x - 108.0, bar_rect.max.y));
        let drag_resp = ui.interact(
            drag_rect,
            Id::new("custom_title_drag"),
            Sense::click_and_drag(),
        );
        if drag_resp.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag_resp.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
        }

        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(bar_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        child_ui.add_space(12.0);
        child_ui.label(
            RichText::new(title)
                .font(FontId::proportional(10.5))
                .strong()
                .color(tokens.text_muted),
        );

        child_ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if window_ctrl_btn(ui, tokens, WindowControlKind::Close, true).clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if window_ctrl_btn(ui, tokens, WindowControlKind::Maximize, false).clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
            }
            if window_ctrl_btn(ui, tokens, WindowControlKind::Minimize, false).clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
            }
        });
    }
}

fn window_ctrl_btn(
    ui: &mut Ui,
    tokens: &ThemeTokens,
    kind: WindowControlKind,
    is_close: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(34.0, TITLE_BAR_HEIGHT), Sense::click());
    if response.hovered() {
        let bg = if is_close {
            tokens.red
        } else {
            tokens.panel_hover
        };
        ui.painter().rect_filled(rect, 0.0, bg);
    }
    let fg = if response.hovered() && is_close {
        Color32::WHITE
    } else if response.hovered() {
        tokens.text_primary
    } else {
        tokens.text_muted
    };
    paint_window_controls(ui.painter(), rect, kind, fg);
    response
}
