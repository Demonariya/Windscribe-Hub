use crate::models::{AccountDraft, WindscribeAccount};
use crate::ui::animations::{animate_fast, lerp_color};
use crate::ui::components::controls::{windscribe_button, CustomTooltip};
use crate::ui::icons::{paint_copy_icon, paint_edit_icon, paint_eye_icon, paint_trash_icon};
use crate::ui::metrics::CARD_RADIUS;
use crate::ui::theme::ThemeTokens;
use egui::{
    Align, Color32, Context, FontId, Id, Layout, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};

pub struct PasswordInput;

impl PasswordInput {
    pub fn show(
        ui: &mut Ui,
        tokens: &ThemeTokens,
        value: &mut String,
        visible: &mut bool,
        hint: &str,
    ) {
        ui.horizontal(|ui| {
            let input_w = (ui.available_width() - 36.0).max(120.0);
            ui.add(
                egui::TextEdit::singleline(value)
                    .hint_text(hint)
                    .password(!*visible)
                    .desired_width(input_w),
            );
            let (eye_rect, eye_resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
            ui.painter().rect_filled(
                eye_rect,
                6.0,
                if eye_resp.hovered() {
                    tokens.panel_hover
                } else {
                    tokens.panel_alt
                },
            );
            paint_eye_icon(
                ui.painter(),
                eye_rect.center(),
                7.2,
                *visible,
                if *visible {
                    tokens.accent
                } else {
                    tokens.text_secondary
                },
            );
            if eye_resp.clicked() {
                *visible = !*visible;
            }
            CustomTooltip::show(
                &eye_resp,
                tokens,
                if *visible {
                    "Hide password"
                } else {
                    "Show password"
                },
            );
        });
    }
}

pub enum AccountEditorEvent {
    None,
    Save,
    Cancel,
    SuggestUsername,
    GeneratePassword,
}

pub struct AccountEditor;

impl AccountEditor {
    pub fn show(ui: &mut Ui, tokens: &ThemeTokens, draft: &mut AccountDraft) -> AccountEditorEvent {
        let mut event = AccountEditorEvent::None;
        let is_edit = draft.editing_id.is_some();

        egui::Frame::none()
            .fill(tokens.panel)
            .stroke(Stroke::new(1.0_f32, tokens.accent.linear_multiply(0.5)))
            .rounding(Rounding::same(CARD_RADIUS))
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if is_edit {
                            "EDIT ACCOUNT"
                        } else {
                            "NEW ACCOUNT"
                        })
                        .font(FontId::proportional(10.0))
                        .strong()
                        .color(tokens.accent),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Cancel")
                                        .font(FontId::proportional(10.0))
                                        .color(tokens.text_muted),
                                )
                                .fill(Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            event = AccountEditorEvent::Cancel;
                        }
                    });
                });

                ui.add_space(8.0);
                ui.label(
                    RichText::new("Username")
                        .font(FontId::proportional(11.0))
                        .color(tokens.text_secondary),
                );
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    let w = (ui.available_width() - 78.0).max(120.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.username)
                            .hint_text("Windscribe username")
                            .desired_width(w),
                    );
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Suggest")
                                    .font(FontId::proportional(10.5))
                                    .color(tokens.text_primary),
                            )
                            .fill(tokens.panel_alt)
                            .min_size(Vec2::new(70.0, 26.0)),
                        )
                        .clicked()
                    {
                        event = AccountEditorEvent::SuggestUsername;
                    }
                });

                ui.add_space(8.0);
                ui.label(
                    RichText::new("Password")
                        .font(FontId::proportional(11.0))
                        .color(tokens.text_secondary),
                );
                ui.add_space(3.0);
                PasswordInput::show(
                    ui,
                    tokens,
                    &mut draft.password,
                    &mut draft.password_visible,
                    "Enter or generate password",
                );

                ui.add_space(8.0);
                ui.label(
                    RichText::new("Label / Note (optional)")
                        .font(FontId::proportional(11.0))
                        .color(tokens.text_secondary),
                );
                ui.add_space(3.0);
                ui.add(
                    egui::TextEdit::singleline(&mut draft.note)
                        .hint_text("e.g. Pro personal, backup")
                        .desired_width(ui.available_width()),
                );

                if let Some(err) = &draft.validation_error {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(err)
                            .font(FontId::proportional(10.5))
                            .color(tokens.red),
                    );
                }

                ui.add_space(10.0);
                ui.columns(2, |cols| {
                    if windscribe_button(&mut cols[0], tokens, "Generate Password", false, true, true)
                        .clicked()
                    {
                        event = AccountEditorEvent::GeneratePassword;
                    }
                    if windscribe_button(
                        &mut cols[1],
                        tokens,
                        if is_edit { "Save Changes" } else { "Save Account" },
                        true,
                        true,
                        true,
                    )
                    .clicked()
                    {
                        event = AccountEditorEvent::Save;
                    }
                });

                ui.add_space(6.0);
                ui.label(
                    RichText::new("Credentials are encrypted locally with Windows DPAPI and never sent externally.")
                        .font(FontId::proportional(9.5))
                        .color(tokens.text_muted),
                );
            });

        event
    }
}

pub enum AccountCardAction {
    None,
    ToggleReveal,
    CopyUsername,
    CopyPassword,
    Edit,
    Delete,
}

pub struct AccountCard;

impl AccountCard {
    pub fn show(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        account: &WindscribeAccount,
        revealed: bool,
        animations_enabled: bool,
    ) -> AccountCardAction {
        let mut action = AccountCardAction::None;

        let hover_id = Id::new(("acc_card_hover", &account.id));
        let hover_prev = ctx.data(|d| d.get_temp::<bool>(hover_id).unwrap_or(false));
        let hover_t = animate_fast(
            ctx,
            Id::new(("acc_card_anim", &account.id)),
            hover_prev,
            animations_enabled,
        );

        let fill = lerp_color(tokens.panel, tokens.panel_alt, hover_t * 0.65);
        let stroke_col = lerp_color(tokens.border_subtle, tokens.border, hover_t);

        let frame_resp = egui::Frame::none()
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, stroke_col))
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&account.username)
                                .font(FontId::proportional(12.5))
                                .strong()
                                .color(tokens.text_primary),
                        );
                        if !account.note.trim().is_empty() {
                            ui.label(
                                RichText::new(&account.note)
                                    .font(FontId::proportional(10.0))
                                    .color(tokens.text_muted),
                            );
                        }
                        let mask = "•".repeat(account.password.chars().count().clamp(8, 16));
                        let pw_display = if revealed {
                            account.password.as_str()
                        } else {
                            mask.as_str()
                        };
                        ui.label(
                            RichText::new(pw_display)
                                .font(FontId::monospace(10.5))
                                .color(if revealed {
                                    tokens.accent
                                } else {
                                    tokens.text_secondary
                                }),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_btn(ui, tokens, "Delete account", |p, c| {
                            paint_trash_icon(p, c, 6.2, tokens.red);
                        }) {
                            action = AccountCardAction::Delete;
                        }
                        if icon_btn(ui, tokens, "Edit account", |p, c| {
                            paint_edit_icon(p, c, 6.0, tokens.text_secondary);
                        }) {
                            action = AccountCardAction::Edit;
                        }
                        if icon_btn(ui, tokens, "Copy password", |p, c| {
                            paint_copy_icon(p, c, 6.0, tokens.accent);
                        }) {
                            action = AccountCardAction::CopyPassword;
                        }
                        if icon_btn(ui, tokens, "Copy username", |p, c| {
                            paint_copy_icon(p, c, 6.0, tokens.text_secondary);
                        }) {
                            action = AccountCardAction::CopyUsername;
                        }
                        if icon_btn(
                            ui,
                            tokens,
                            if revealed {
                                "Hide password"
                            } else {
                                "Reveal password"
                            },
                            |p, c| {
                                paint_eye_icon(
                                    p,
                                    c,
                                    6.2,
                                    revealed,
                                    if revealed {
                                        tokens.accent
                                    } else {
                                        tokens.text_secondary
                                    },
                                );
                            },
                        ) {
                            action = AccountCardAction::ToggleReveal;
                        }
                    });
                });
            });

        let is_hovered = frame_resp.response.hovered();
        ctx.data_mut(|d| d.insert_temp(hover_id, is_hovered));

        action
    }
}

fn icon_btn(
    ui: &mut Ui,
    tokens: &ThemeTokens,
    tooltip: &str,
    paint: impl FnOnce(&egui::Painter, egui::Pos2),
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 6.0, tokens.panel_hover);
    }
    paint(ui.painter(), rect.center());
    CustomTooltip::show(&resp, tokens, tooltip);
    resp.clicked()
}
