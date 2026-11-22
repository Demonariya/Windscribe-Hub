use crate::models::install::{InstallStateKind, PrimaryActionKind};
use crate::ui::animations::{animate_fast, lerp_color, lerp_f32};
use crate::ui::icons::paint_power_glyph;
use crate::ui::metrics::{ACTION_INNER_RADIUS, ACTION_OUTER_RADIUS, ACTION_RING_STROKE};
use crate::ui::theme::ThemeTokens;
use egui::{pos2, Color32, Context, FontId, Id, Pos2, RichText, Sense, Shape, Stroke, Ui, Vec2};
use std::f32::consts::{FRAC_PI_2, TAU};

pub struct MainActionButton;

impl MainActionButton {
    pub fn show(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        state_kind: InstallStateKind,
        action: PrimaryActionKind,
        busy: bool,
        animations_enabled: bool,
    ) -> bool {
        let size = Vec2::splat(ACTION_OUTER_RADIUS * 2.0 + 10.0);
        let (alloc_rect, response) = ui.allocate_exact_size(size, Sense::click());
        let center = alloc_rect.center();

        let hover_t = animate_fast(
            ctx,
            Id::new("main_action_hover"),
            response.hovered() && !busy,
            animations_enabled,
        );
        let press_t = animate_fast(
            ctx,
            Id::new("main_action_press"),
            response.is_pointer_button_down_on() && !busy,
            animations_enabled,
        );

        let ring_color = match state_kind {
            InstallStateKind::Checking => tokens.accent,
            InstallStateKind::InstalledUpToDate | InstallStateKind::InstalledUnmanaged => {
                tokens.accent
            }
            InstallStateKind::InstalledUpdateAvailable | InstallStateKind::NotInstalled => {
                tokens.amber
            }
            InstallStateKind::Error => tokens.red,
        };

        let scale = lerp_f32(1.0, 1.03, hover_t) * lerp_f32(1.0, 0.96, press_t);
        let outer_r = ACTION_OUTER_RADIUS * scale;
        let inner_r = ACTION_INNER_RADIUS * scale;

        // Subtle outer ambient halo when hovered or installed
        let halo_alpha = if busy {
            42
        } else {
            (22.0 + hover_t * 28.0) as u8
        };
        ui.painter().circle_filled(
            center,
            outer_r + 4.0,
            Color32::from_rgba_unmultiplied(
                ring_color.r(),
                ring_color.g(),
                ring_color.b(),
                halo_alpha,
            ),
        );

        // Outer track
        ui.painter().circle_stroke(
            center,
            outer_r,
            Stroke::new(ACTION_RING_STROKE, tokens.border),
        );

        if busy {
            // Honest indeterminate sweeping arc (never fake percentages)
            let time = ctx.input(|i| i.time) as f32;
            if animations_enabled {
                ctx.request_repaint();
            }
            let start = if animations_enabled {
                time * 3.8 - FRAC_PI_2
            } else {
                -FRAC_PI_2
            };
            paint_arc(
                ui.painter(),
                center,
                outer_r,
                start,
                start + 1.9,
                Stroke::new(ACTION_RING_STROKE + 0.6, ring_color),
            );
        } else {
            ui.painter().circle_stroke(
                center,
                outer_r,
                Stroke::new(ACTION_RING_STROKE, ring_color),
            );
        }

        // Inner circular Windscribe power-style button
        let base_fill = tokens.panel_alt;
        let hover_fill = tokens.panel_hover;
        let inner_fill = if busy {
            tokens.panel
        } else {
            lerp_color(base_fill, hover_fill, hover_t)
        };
        ui.painter().circle_filled(center, inner_r, inner_fill);
        ui.painter().circle_stroke(
            center,
            inner_r,
            Stroke::new(
                1.2_f32,
                lerp_color(tokens.border, ring_color, hover_t * 0.7),
            ),
        );

        let glyph_color = if busy {
            tokens.text_muted
        } else if response.hovered() {
            ring_color
        } else {
            tokens.text_primary
        };
        paint_power_glyph(ui.painter(), center, 17.5 * scale, action, glyph_color);

        ui.add_space(6.0);
        let caption = if busy { "WORKING..." } else { action.label() };
        ui.label(
            RichText::new(caption)
                .font(FontId::proportional(11.0))
                .strong()
                .color(if busy { tokens.text_muted } else { ring_color }),
        );

        response.clicked() && !busy
    }
}

fn paint_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start: f32,
    end: f32,
    stroke: Stroke,
) {
    let steps = 28;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + (end - start) * (i as f32 / steps as f32);
            pos2(center.x + a.cos() * radius, center.y + a.sin() * radius)
        })
        .collect();
    painter.add(Shape::line(pts, stroke));
    let _ = TAU;
}
