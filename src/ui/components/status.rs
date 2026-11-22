use crate::models::{
    InstallStateKind, InstallStatus, OperationKind, ProcessState, WINDSCRIBE_PACKAGE_ID,
};
use crate::ui::animations::pulse_factor;
use crate::ui::components::controls::CustomTooltip;
use crate::ui::theme::ThemeTokens;
use crate::utils::time::format_hms;
use egui::{
    pos2, Align, Color32, Context, FontId, Layout, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};

pub struct ApplicationStatus;

impl ApplicationStatus {
    pub fn show_badge(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        status: &InstallStatus,
        active_op: Option<OperationKind>,
        animations_enabled: bool,
    ) {
        let (label, color, bg) = if let Some(op) = active_op {
            (op.label(), tokens.accent, tokens.accent_soft)
        } else {
            match status.state_kind {
                InstallStateKind::Checking => {
                    ("CHECKING SYSTEM", tokens.accent, tokens.accent_soft)
                }
                InstallStateKind::InstalledUpToDate => {
                    ("UP TO DATE", tokens.green, tokens.green_soft)
                }
                InstallStateKind::InstalledUnmanaged => {
                    ("LOCAL INSTALL", tokens.green, tokens.green_soft)
                }
                InstallStateKind::InstalledUpdateAvailable => {
                    ("UPDATE AVAILABLE", tokens.amber, tokens.amber_soft)
                }
                InstallStateKind::NotInstalled => {
                    ("NOT INSTALLED", tokens.amber, tokens.amber_soft)
                }
                InstallStateKind::Error => ("ATTENTION NEEDED", tokens.red, tokens.red_soft),
            }
        };

        egui::Frame::none()
            .fill(bg)
            .stroke(Stroke::new(1.0_f32, color.linear_multiply(0.5)))
            .rounding(Rounding::same(12.0))
            .inner_margin(egui::Margin::symmetric(10.0, 4.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(7.0), Sense::hover());
                    let pulse = if active_op.is_some() || status.process.is_running() {
                        if animations_enabled {
                            ctx.request_repaint();
                        }
                        0.45 + 0.55 * pulse_factor(ctx, animations_enabled, 0.8)
                    } else {
                        1.0
                    };
                    let dot_color = Color32::from_rgba_unmultiplied(
                        color.r(),
                        color.g(),
                        color.b(),
                        (255.0 * pulse) as u8,
                    );
                    ui.painter().circle_filled(dot.center(), 3.2, dot_color);
                    ui.label(
                        RichText::new(label)
                            .font(FontId::proportional(9.5))
                            .strong()
                            .color(color),
                    );
                });
            });
    }

    pub fn show_status_tiles(ui: &mut Ui, tokens: &ThemeTokens, status: &InstallStatus) {
        ui.columns(2, |cols| {
            status_tile(
                &mut cols[0],
                tokens,
                "CLIENT STATE",
                if status.installed {
                    "Installed"
                } else {
                    "Not Found"
                },
                status
                    .installed_version
                    .as_deref()
                    .map(|v| format!("v{v}"))
                    .unwrap_or_else(|| status.source.label().to_owned()),
                if status.installed {
                    tokens.green
                } else {
                    tokens.amber
                },
            );
            let proc_color = match status.process {
                ProcessState::Running { .. } => tokens.green,
                ProcessState::Stopped => tokens.gray,
                ProcessState::Unknown => tokens.amber,
            };
            let proc_sub = match status.process {
                ProcessState::Running { pid: Some(pid) } => format!("Windscribe.exe (PID {pid})"),
                ProcessState::Running { pid: None } => "Windscribe.exe active".to_owned(),
                ProcessState::Stopped => "Windscribe.exe inactive".to_owned(),
                ProcessState::Unknown => "Process check unavailable".to_owned(),
            };
            status_tile(
                &mut cols[1],
                tokens,
                "PROCESS",
                status.process.label(),
                proc_sub,
                proc_color,
            );
        });
    }
}

fn status_tile(
    ui: &mut Ui,
    tokens: &ThemeTokens,
    title: &str,
    value: &str,
    sub: String,
    accent: Color32,
) {
    egui::Frame::none()
        .fill(tokens.panel)
        .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
        .rounding(Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(6.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.0, accent);
                ui.label(
                    RichText::new(title)
                        .font(FontId::proportional(9.0))
                        .strong()
                        .color(tokens.text_muted),
                );
            });
            ui.add_space(4.0);
            ui.label(
                RichText::new(value)
                    .font(FontId::proportional(14.0))
                    .strong()
                    .color(tokens.text_primary),
            );
            ui.label(
                RichText::new(sub)
                    .font(FontId::proportional(10.0))
                    .color(tokens.text_muted),
            );
        });
}

pub struct InstallationInformation;

impl InstallationInformation {
    pub fn show(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        status: &InstallStatus,
    ) -> Option<String> {
        let mut copied_msg = None;

        egui::Frame::none()
            .fill(tokens.panel)
            .stroke(Stroke::new(1.0_f32, tokens.border_subtle))
            .rounding(Rounding::same(12.0))
            .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new("INSTALLATION DETAILS")
                        .font(FontId::proportional(9.5))
                        .strong()
                        .color(tokens.text_muted),
                );
                ui.add_space(6.0);

                let install_path = status
                    .install_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Not detected".to_owned());

                if info_row(ui, tokens, "Install Path", &install_path, true)
                    && status.install_path.is_some()
                {
                    ctx.copy_text(install_path.clone());
                    copied_msg = Some("Install path copied to clipboard.".to_owned());
                }
                divider(ui, tokens);
                info_row(ui, tokens, "Package ID", WINDSCRIBE_PACKAGE_ID, false);
                divider(ui, tokens);
                info_row(
                    ui,
                    tokens,
                    "Installed Version",
                    status.installed_version.as_deref().unwrap_or("—"),
                    false,
                );
                divider(ui, tokens);
                let latest = if status.managed_by_winget {
                    status.latest_version.as_deref().unwrap_or("Checking…")
                } else if status.installed {
                    "WinGet unmanaged"
                } else {
                    "—"
                };
                info_row(ui, tokens, "Latest Version", latest, false);
                divider(ui, tokens);
                info_row(ui, tokens, "Install Source", status.source.label(), false);
                divider(ui, tokens);
                let last_check = status
                    .last_checked
                    .map(format_hms)
                    .unwrap_or_else(|| "Pending".to_owned());
                info_row(ui, tokens, "Last Check", &last_check, false);
            });

        copied_msg
    }
}

fn info_row(ui: &mut Ui, tokens: &ThemeTokens, label: &str, value: &str, clickable: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        ui.label(
            RichText::new(label)
                .font(FontId::proportional(11.0))
                .color(tokens.text_secondary),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let text = RichText::new(truncate_middle(value, 28))
                .font(FontId::proportional(11.0))
                .strong()
                .color(if clickable {
                    tokens.text_primary
                } else {
                    tokens.text_secondary
                });
            let resp = ui.add(egui::Label::new(text).sense(if clickable {
                Sense::click()
            } else {
                Sense::hover()
            }));
            if clickable && resp.clicked() {
                clicked = true;
            }
            if value.len() > 28 || clickable {
                CustomTooltip::show(&resp, tokens, value);
            }
        });
    });
    clicked
}

fn divider(ui: &mut Ui, tokens: &ThemeTokens) {
    let rect = ui.allocate_space(Vec2::new(ui.available_width(), 1.0)).1;
    ui.painter().line_segment(
        [
            pos2(rect.min.x, rect.center().y),
            pos2(rect.max.x, rect.center().y),
        ],
        Stroke::new(1.0_f32, tokens.border_subtle),
    );
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_owned();
    }
    let keep = (max_chars.saturating_sub(1)) / 2;
    let start: String = chars[..keep].iter().collect();
    let end: String = chars[chars.len() - keep..].iter().collect();
    format!("{start}…{end}")
}

pub struct LoadingIndicator;

impl LoadingIndicator {
    pub fn show(
        ui: &mut Ui,
        ctx: &Context,
        tokens: &ThemeTokens,
        label: &str,
        animations_enabled: bool,
    ) {
        egui::Frame::none()
            .fill(tokens.panel_alt)
            .stroke(Stroke::new(1.0_f32, tokens.accent.linear_multiply(0.4)))
            .rounding(Rounding::same(10.0))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                    if animations_enabled {
                        ctx.request_repaint();
                    }
                    let t = if animations_enabled {
                        ctx.input(|i| i.time) as f32 * 5.0
                    } else {
                        0.0
                    };
                    let c = rect.center();
                    let r = 6.0;
                    ui.painter()
                        .circle_stroke(c, r, Stroke::new(2.0_f32, tokens.border));
                    let pts: Vec<_> = (0..=14)
                        .map(|i| {
                            let a = t + (i as f32 / 14.0) * 2.1;
                            pos2(c.x + a.cos() * r, c.y + a.sin() * r)
                        })
                        .collect();
                    ui.painter()
                        .add(egui::Shape::line(pts, Stroke::new(2.2_f32, tokens.accent)));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(label)
                            .font(FontId::proportional(11.0))
                            .strong()
                            .color(tokens.text_primary),
                    );
                });
            });
    }
}
