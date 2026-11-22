use crate::models::ThemeMode;
use crate::ui::metrics::CARD_RADIUS;
use egui::{Color32, Rounding, Stroke, Vec2};

#[derive(Clone, Debug)]
pub struct ThemeTokens {
    pub is_dark: bool,
    pub bg_base: Color32,
    pub bg_header: Color32,
    pub bg_hero: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub panel_hover: Color32,
    pub border: Color32,
    pub border_subtle: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_text: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub green: Color32,
    pub green_soft: Color32,
    pub amber: Color32,
    pub amber_soft: Color32,
    pub red: Color32,
    pub red_soft: Color32,
    pub gray: Color32,
    pub overlay_scrim: Color32,
}

impl ThemeTokens {
    pub fn dark() -> Self {
        Self {
            is_dark: true,
            bg_base: Color32::from_rgb(11, 17, 27), // #0B111B
            bg_header: Color32::from_rgb(16, 25, 39), // #101927
            bg_hero: Color32::from_rgb(21, 34, 52), // #152234
            panel: Color32::from_rgb(24, 37, 54),   // #182536
            panel_alt: Color32::from_rgb(29, 43, 61), // #1D2B3D
            panel_hover: Color32::from_rgb(34, 51, 72), // #223348
            border: Color32::from_rgb(40, 58, 82),  // #283A52
            border_subtle: Color32::from_rgb(32, 47, 67), // #202F43
            accent: Color32::from_rgb(0, 229, 137), // Windscribe emerald green
            accent_soft: Color32::from_rgb(16, 62, 50),
            accent_text: Color32::from_rgb(11, 17, 27),
            text_primary: Color32::from_rgb(245, 247, 250), // #F5F7FA
            text_secondary: Color32::from_rgb(187, 197, 209), // #BBC5D1
            text_muted: Color32::from_rgb(127, 140, 157),   // #7F8C9D
            green: Color32::from_rgb(46, 204, 113),
            green_soft: Color32::from_rgb(21, 59, 46),
            amber: Color32::from_rgb(245, 166, 35),
            amber_soft: Color32::from_rgb(63, 47, 24),
            red: Color32::from_rgb(232, 74, 95),
            red_soft: Color32::from_rgb(67, 29, 39),
            gray: Color32::from_rgb(100, 116, 139),
            overlay_scrim: Color32::from_rgba_unmultiplied(6, 10, 18, 175),
        }
    }

    pub fn light() -> Self {
        Self {
            is_dark: false,
            bg_base: Color32::from_rgb(238, 242, 247),
            bg_header: Color32::from_rgb(255, 255, 255),
            bg_hero: Color32::from_rgb(228, 235, 244),
            panel: Color32::from_rgb(255, 255, 255),
            panel_alt: Color32::from_rgb(244, 247, 251),
            panel_hover: Color32::from_rgb(230, 237, 246),
            border: Color32::from_rgb(203, 214, 228),
            border_subtle: Color32::from_rgb(224, 232, 241),
            accent: Color32::from_rgb(13, 166, 102),
            accent_soft: Color32::from_rgb(219, 246, 234),
            accent_text: Color32::WHITE,
            text_primary: Color32::from_rgb(18, 28, 42),
            text_secondary: Color32::from_rgb(66, 82, 104),
            text_muted: Color32::from_rgb(112, 127, 148),
            green: Color32::from_rgb(16, 163, 98),
            green_soft: Color32::from_rgb(220, 246, 233),
            amber: Color32::from_rgb(201, 126, 12),
            amber_soft: Color32::from_rgb(253, 242, 217),
            red: Color32::from_rgb(211, 52, 74),
            red_soft: Color32::from_rgb(253, 229, 232),
            gray: Color32::from_rgb(122, 134, 154),
            overlay_scrim: Color32::from_rgba_unmultiplied(15, 23, 42, 120),
        }
    }

    pub fn resolve(mode: ThemeMode, system_is_dark: bool) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
            ThemeMode::System => {
                if system_is_dark {
                    Self::dark()
                } else {
                    Self::light()
                }
            }
        }
    }
}

pub fn apply_egui_visuals(ctx: &egui::Context, tokens: &ThemeTokens) {
    let mut visuals = if tokens.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = tokens.bg_base;
    visuals.window_fill = tokens.panel;
    visuals.extreme_bg_color = tokens.bg_header;
    visuals.override_text_color = Some(tokens.text_primary);
    visuals.window_rounding = Rounding::same(CARD_RADIUS);
    visuals.window_stroke = Stroke::new(1.0_f32, tokens.border);

    visuals.widgets.noninteractive.bg_fill = tokens.panel;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, tokens.text_secondary);
    visuals.widgets.noninteractive.rounding = Rounding::same(8.0);

    visuals.widgets.inactive.bg_fill = tokens.panel_alt;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, tokens.text_primary);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, tokens.border_subtle);
    visuals.widgets.inactive.rounding = Rounding::same(8.0);

    visuals.widgets.hovered.bg_fill = tokens.panel_hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, tokens.text_primary);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, tokens.border);
    visuals.widgets.hovered.rounding = Rounding::same(8.0);

    visuals.widgets.active.bg_fill = tokens.accent_soft;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, tokens.text_primary);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, tokens.accent);
    visuals.widgets.active.rounding = Rounding::same(8.0);

    visuals.selection.bg_fill = tokens.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0_f32, tokens.accent);

    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.scroll.bar_width = 5.0;
        style.spacing.scroll.floating = true;
    });
}
