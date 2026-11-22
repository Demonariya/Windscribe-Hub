use crate::models::{InstallStatus, NavSection};
use crate::ui::animations::{animate_fast, lerp_color};
use crate::ui::components::controls::CustomTooltip;
use crate::ui::icons::{paint_brand_crest, paint_hamburger, paint_nav_icon, paint_refresh_icon};
use crate::ui::metrics::{DRAWER_WIDTH, HEADER_HEIGHT, NAV_STRIP_HEIGHT};
use crate::ui::theme::ThemeTokens;
use egui::{
    pos2, Align, Align2, Area, Color32, Context, FontId, Id, Layout, Order, Rect, RichText,
    Rounding, Sense, Stroke, Ui, Vec2,
};

pub struct HeaderAction {
    pub toggle_drawer: bool,
    pub refresh_clicked: bool,
    pub open_settings: bool,
}

pub struct WindscribeHeader;

impl WindscribeHeader {
    pub fn show(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        drawer_open: bool,
        busy: bool,
        animations_enabled: bool,
        status: &InstallStatus,
    ) -> HeaderAction {
        let mut action = HeaderAction {
            toggle_drawer: false,
            refresh_clicked: false,
            open_settings: false,
        };

        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, HEADER_HEIGHT), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, tokens.bg_header);
        ui.painter().line_segment(
            [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)],
            Stroke::new(1.0_f32, tokens.border_subtle),
        );

        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        header_ui.add_space(12.0);

        // Hamburger menu toggle button
        let (menu_rect, menu_resp) =
            header_ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
        let menu_hover = animate_fast(
            ctx,
            Id::new("hdr_menu_hover"),
            menu_resp.hovered(),
            animations_enabled,
        );
        let open_t = animate_fast(
            ctx,
            Id::new("hdr_menu_open"),
            drawer_open,
            animations_enabled,
        );
        let menu_bg = lerp_color(tokens.panel_alt, tokens.panel_hover, menu_hover);
        header_ui.painter().rect_filled(menu_rect, 8.0, menu_bg);
        paint_hamburger(
            header_ui.painter(),
            menu_rect.center(),
            open_t,
            if drawer_open {
                tokens.accent
            } else {
                tokens.text_primary
            },
        );
        if menu_resp.clicked() {
            action.toggle_drawer = true;
        }
        CustomTooltip::show(&menu_resp, tokens, "Navigation Menu (Esc to close)");

        header_ui.add_space(6.0);

        // Brand Crest + Title
        let (crest_rect, _) = header_ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
        paint_brand_crest(header_ui.painter(), crest_rect, tokens);
        header_ui.add_space(6.0);

        header_ui.vertical(|ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("WINDSCRIBE")
                        .font(FontId::proportional(13.0))
                        .strong()
                        .color(tokens.text_primary),
                );
                ui.label(
                    RichText::new("HUB")
                        .font(FontId::proportional(13.0))
                        .strong()
                        .color(tokens.accent),
                );
            });
            let sub = if busy {
                "Background task active"
            } else if status.process.is_running() {
                "Windscribe.exe running"
            } else if status.installed {
                "Desktop client ready"
            } else {
                "Client not detected"
            };
            ui.label(
                RichText::new(sub)
                    .font(FontId::proportional(9.5))
                    .color(tokens.text_muted),
            );
        });

        header_ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);

            // Settings shortcut button
            let (cfg_rect, cfg_resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
            let cfg_hover = animate_fast(
                ctx,
                Id::new("hdr_cfg_hover"),
                cfg_resp.hovered(),
                animations_enabled,
            );
            ui.painter().rect_filled(
                cfg_rect,
                8.0,
                lerp_color(Color32::TRANSPARENT, tokens.panel_hover, cfg_hover),
            );
            paint_nav_icon(
                ui.painter(),
                cfg_rect.center(),
                17.0,
                NavSection::Settings,
                if cfg_resp.hovered() {
                    tokens.text_primary
                } else {
                    tokens.text_secondary
                },
            );
            if cfg_resp.clicked() {
                action.open_settings = true;
            }
            CustomTooltip::show(&cfg_resp, tokens, "Preferences (Alt+5)");

            // Refresh button (rotates when busy)
            let (ref_rect, ref_resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
            let ref_hover = animate_fast(
                ctx,
                Id::new("hdr_ref_hover"),
                ref_resp.hovered(),
                animations_enabled,
            );
            ui.painter().rect_filled(
                ref_rect,
                8.0,
                lerp_color(Color32::TRANSPARENT, tokens.panel_hover, ref_hover),
            );
            let angle = if busy && animations_enabled {
                ctx.request_repaint();
                (ctx.input(|i| i.time) as f32) * 4.5
            } else {
                0.0
            };
            paint_refresh_icon(
                ui.painter(),
                ref_rect.center(),
                7.2,
                angle,
                if busy {
                    tokens.accent
                } else if ref_resp.hovered() {
                    tokens.text_primary
                } else {
                    tokens.text_secondary
                },
            );
            if ref_resp.clicked() && !busy {
                action.refresh_clicked = true;
            }
            CustomTooltip::show(&ref_resp, tokens, "Refresh installation & process status");
        });

        action
    }
}

pub struct NavigationMenu;

impl NavigationMenu {
    pub fn show_compact_strip(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        active: NavSection,
        animations_enabled: bool,
    ) -> Option<NavSection> {
        let mut next = None;
        let width = ui.available_width();
        let (strip_rect, _) =
            ui.allocate_exact_size(Vec2::new(width, NAV_STRIP_HEIGHT), Sense::hover());
        ui.painter().rect_filled(strip_rect, 0.0, tokens.bg_header);
        ui.painter().line_segment(
            [
                pos2(strip_rect.min.x, strip_rect.max.y),
                pos2(strip_rect.max.x, strip_rect.max.y),
            ],
            Stroke::new(1.0_f32, tokens.border_subtle),
        );

        let count = NavSection::ALL.len() as f32;
        let item_w = (width - 16.0) / count;
        let mut active_x = strip_rect.min.x + 8.0 + (active.index() as f32) * item_w;

        if animations_enabled {
            let target_x = active_x;
            let anim_id = Id::new("nav_strip_indicator_x");
            active_x = ctx.animate_value_with_time(anim_id, target_x, 0.18);
        }

        for (idx, &section) in NavSection::ALL.iter().enumerate() {
            let item_rect = Rect::from_min_size(
                pos2(
                    strip_rect.min.x + 8.0 + (idx as f32) * item_w,
                    strip_rect.min.y + 4.0,
                ),
                Vec2::new(item_w, NAV_STRIP_HEIGHT - 8.0),
            );
            let resp = ui.interact(item_rect, Id::new(("nav_strip_btn", idx)), Sense::click());
            let is_active = section == active;
            let hover_t = animate_fast(
                ctx,
                Id::new(("nav_strip_hover", idx)),
                resp.hovered(),
                animations_enabled,
            );
            let bg = if is_active {
                tokens.panel_alt
            } else {
                lerp_color(Color32::TRANSPARENT, tokens.panel, hover_t)
            };
            ui.painter().rect_filled(item_rect, 8.0, bg);

            let fg = if is_active {
                tokens.accent
            } else if resp.hovered() {
                tokens.text_primary
            } else {
                tokens.text_muted
            };
            paint_nav_icon(
                ui.painter(),
                pos2(item_rect.center().x, item_rect.center().y - 4.0),
                14.5,
                section,
                fg,
            );
            ui.painter().text(
                pos2(item_rect.center().x, item_rect.max.y - 6.0),
                Align2::CENTER_CENTER,
                section.title(),
                FontId::proportional(9.0),
                if is_active { tokens.text_primary } else { fg },
            );
            if resp.clicked() {
                next = Some(section);
            }
            CustomTooltip::show(
                &resp,
                tokens,
                &format!("{} ({})", section.full_title(), section.shortcut_hint()),
            );
        }

        // Animated active underline pill
        let bar_rect = Rect::from_min_size(
            pos2(active_x + item_w * 0.25, strip_rect.max.y - 2.5),
            Vec2::new(item_w * 0.5, 2.5),
        );
        ui.painter().rect_filled(bar_rect, 1.5, tokens.accent);

        next
    }

    pub fn show_drawer(
        ctx: &Context,
        tokens: &ThemeTokens,
        open: &mut bool,
        active: NavSection,
        animations_enabled: bool,
        account_count: usize,
        status: &InstallStatus,
    ) -> Option<NavSection> {
        let t = animate_fast(ctx, Id::new("nav_drawer_slide"), *open, animations_enabled);
        if t <= 0.01 {
            return None;
        }

        let mut picked = None;
        let screen = ctx.screen_rect();
        let top_y = screen.min.y + 82.0; // Below TitleBar + Header
        let content_rect = Rect::from_min_max(pos2(screen.min.x, top_y), screen.max);

        // Dim scrim
        Area::new(Id::new("nav_drawer_scrim"))
            .order(Order::Middle)
            .fixed_pos(content_rect.min)
            .show(ctx, |ui| {
                let (scrim_rect, scrim_resp) =
                    ui.allocate_exact_size(content_rect.size(), Sense::click());
                let alpha = ((tokens.overlay_scrim.a() as f32) * t).round() as u8;
                let c = tokens.overlay_scrim;
                ui.painter().rect_filled(
                    scrim_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha),
                );
                if scrim_resp.clicked() {
                    *open = false;
                }
            });

        // Sliding panel
        let drawer_w = DRAWER_WIDTH.min(screen.width() - 36.0);
        let x_offset = -drawer_w * (1.0 - t);
        let drawer_rect = Rect::from_min_size(
            pos2(screen.min.x + x_offset, top_y),
            Vec2::new(drawer_w, content_rect.height()),
        );

        Area::new(Id::new("nav_drawer_panel"))
            .order(Order::Foreground)
            .fixed_pos(drawer_rect.min)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(tokens.bg_header)
                    .stroke(Stroke::new(1.0_f32, tokens.border))
                    .inner_margin(egui::Margin::same(14.0))
                    .show(ui, |ui| {
                        ui.set_min_size(Vec2::new(drawer_w - 28.0, content_rect.height() - 28.0));
                        ui.set_max_width(drawer_w - 28.0);

                        ui.label(
                            RichText::new("NAVIGATION")
                                .font(FontId::proportional(9.5))
                                .strong()
                                .color(tokens.text_muted),
                        );
                        ui.add_space(8.0);

                        for &section in &NavSection::ALL {
                            let is_active = section == active;
                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 38.0),
                                Sense::click(),
                            );
                            let hover_t = animate_fast(
                                ctx,
                                Id::new(("drawer_item", section.index())),
                                row_resp.hovered(),
                                animations_enabled,
                            );
                            let row_fill = if is_active {
                                tokens.accent_soft
                            } else {
                                lerp_color(Color32::TRANSPARENT, tokens.panel_hover, hover_t)
                            };
                            ui.painter().rect_filled(row_rect, 8.0, row_fill);
                            if is_active {
                                let bar = Rect::from_min_size(
                                    pos2(row_rect.min.x + 2.0, row_rect.min.y + 8.0),
                                    Vec2::new(3.0, row_rect.height() - 16.0),
                                );
                                ui.painter().rect_filled(bar, 2.0, tokens.accent);
                            }

                            let icon_color = if is_active {
                                tokens.accent
                            } else {
                                tokens.text_secondary
                            };
                            paint_nav_icon(
                                ui.painter(),
                                pos2(row_rect.min.x + 22.0, row_rect.center().y),
                                16.0,
                                section,
                                icon_color,
                            );
                            ui.painter().text(
                                pos2(row_rect.min.x + 42.0, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                section.full_title(),
                                FontId::proportional(12.5),
                                if is_active {
                                    tokens.text_primary
                                } else {
                                    tokens.text_secondary
                                },
                            );
                            ui.painter().text(
                                pos2(row_rect.max.x - 10.0, row_rect.center().y),
                                Align2::RIGHT_CENTER,
                                section.shortcut_hint(),
                                FontId::monospace(9.5),
                                tokens.text_muted,
                            );

                            if row_resp.clicked() {
                                picked = Some(section);
                                *open = false;
                            }
                            ui.add_space(4.0);
                        }

                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(10.0);

                        egui::Frame::none()
                            .fill(tokens.panel)
                            .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
                            .rounding(Rounding::same(10.0))
                            .inner_margin(egui::Margin::same(10.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("DEVICE SUMMARY")
                                        .font(FontId::proportional(9.0))
                                        .strong()
                                        .color(tokens.text_muted),
                                );
                                ui.add_space(6.0);
                                summary_line(
                                    ui,
                                    tokens,
                                    "Windscribe",
                                    if status.installed {
                                        "Installed"
                                    } else {
                                        "Missing"
                                    },
                                );
                                summary_line(ui, tokens, "Process", status.process.label());
                                summary_line(
                                    ui,
                                    tokens,
                                    "Vault Accounts",
                                    &account_count.to_string(),
                                );
                            });
                    });
            });

        picked
    }
}

fn summary_line(ui: &mut Ui, tokens: &ThemeTokens, k: &str, v: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(k)
                .font(FontId::proportional(10.5))
                .color(tokens.text_muted),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(v)
                    .font(FontId::proportional(10.5))
                    .strong()
                    .color(tokens.text_primary),
            );
        });
    });
}
