use crate::models::install::PrimaryActionKind;
use crate::models::NavSection;
use crate::ui::theme::ThemeTokens;
use egui::{pos2, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use std::f32::consts::{FRAC_PI_2, TAU};

pub fn paint_brand_crest(painter: &Painter, rect: Rect, tokens: &ThemeTokens) {
    painter.rect_filled(rect, 8.0, tokens.panel_alt);
    painter.rect_stroke(rect, 8.0, Stroke::new(1.2_f32, tokens.accent));

    let c = rect.center();
    let w = rect.width() * 0.30;
    let h = rect.height() * 0.22;
    let pts = vec![
        pos2(c.x - w, c.y - h),
        pos2(c.x - w * 0.48, c.y + h),
        pos2(c.x, c.y - h * 0.25),
        pos2(c.x + w * 0.48, c.y + h),
        pos2(c.x + w, c.y - h),
    ];
    painter.add(Shape::line(pts, Stroke::new(2.1_f32, tokens.text_primary)));
}

pub fn paint_hamburger(painter: &Painter, center: Pos2, open_t: f32, color: Color32) {
    let half_w = 7.0;
    let spread = 4.5 * (1.0 - open_t);
    let rot = open_t * std::f32::consts::FRAC_PI_4;

    // Top bar -> diagonal 1
    let dx = half_w * rot.cos();
    let dy = half_w * rot.sin();
    painter.line_segment(
        [
            pos2(center.x - dx, center.y - spread - dy),
            pos2(center.x + dx, center.y - spread + dy),
        ],
        Stroke::new(1.8_f32, color),
    );

    // Middle bar fades out
    let mid_alpha = ((1.0 - open_t * 1.4).clamp(0.0, 1.0) * 255.0) as u8;
    if mid_alpha > 4 {
        let mid_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), mid_alpha);
        painter.line_segment(
            [
                pos2(center.x - half_w, center.y),
                pos2(center.x + half_w, center.y),
            ],
            Stroke::new(1.8_f32, mid_color),
        );
    }

    // Bottom bar -> diagonal 2
    painter.line_segment(
        [
            pos2(center.x - dx, center.y + spread + dy),
            pos2(center.x + dx, center.y + spread - dy),
        ],
        Stroke::new(1.8_f32, color),
    );
}

pub fn paint_nav_icon(
    painter: &Painter,
    center: Pos2,
    size: f32,
    section: NavSection,
    color: Color32,
) {
    let r = size * 0.42;
    let stroke = Stroke::new(1.6_f32, color);
    match section {
        NavSection::Home => {
            // Shield / Home silhouette
            let pts = vec![
                pos2(center.x, center.y - r),
                pos2(center.x + r * 0.85, center.y - r * 0.5),
                pos2(center.x + r * 0.72, center.y + r * 0.45),
                pos2(center.x, center.y + r),
                pos2(center.x - r * 0.72, center.y + r * 0.45),
                pos2(center.x - r * 0.85, center.y - r * 0.5),
                pos2(center.x, center.y - r),
            ];
            painter.add(Shape::line(pts, stroke));
            painter.circle_filled(center, 2.0, color);
        }
        NavSection::Accounts => {
            // Key / Vault icon
            let head = pos2(center.x - r * 0.35, center.y);
            painter.circle_stroke(head, r * 0.42, stroke);
            painter.line_segment(
                [
                    pos2(head.x + r * 0.42, center.y),
                    pos2(center.x + r * 0.9, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x + r * 0.6, center.y),
                    pos2(center.x + r * 0.6, center.y + r * 0.38),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x + r * 0.85, center.y),
                    pos2(center.x + r * 0.85, center.y + r * 0.32),
                ],
                stroke,
            );
        }
        NavSection::Tools => {
            // Two slider tracks (Windscribe Preferences / Tools look)
            let y1 = center.y - r * 0.42;
            let y2 = center.y + r * 0.42;
            painter.line_segment(
                [pos2(center.x - r * 0.85, y1), pos2(center.x + r * 0.85, y1)],
                stroke,
            );
            painter.line_segment(
                [pos2(center.x - r * 0.85, y2), pos2(center.x + r * 0.85, y2)],
                stroke,
            );
            painter.circle_filled(pos2(center.x - r * 0.28, y1), 2.5, color);
            painter.circle_filled(pos2(center.x + r * 0.28, y2), 2.5, color);
        }
        NavSection::Registry => {
            // Layered registry stack
            for dy in [-r * 0.52, 0.0, r * 0.52] {
                let rect = Rect::from_center_size(
                    pos2(center.x, center.y + dy),
                    Vec2::new(r * 1.65, r * 0.36),
                );
                painter.rect_stroke(rect, 2.0, Stroke::new(1.3_f32, color));
            }
        }
        NavSection::Settings => {
            // Gear ring + spokes
            painter.circle_stroke(center, r * 0.48, stroke);
            painter.circle_filled(center, r * 0.16, color);
            for i in 0..6 {
                let a = (i as f32) * (TAU / 6.0);
                let p1 = pos2(center.x + a.cos() * r * 0.52, center.y + a.sin() * r * 0.52);
                let p2 = pos2(center.x + a.cos() * r * 0.88, center.y + a.sin() * r * 0.88);
                painter.line_segment([p1, p2], stroke);
            }
        }
    }
}

pub fn paint_power_glyph(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    action: PrimaryActionKind,
    color: Color32,
) {
    let stroke = Stroke::new(2.6_f32, color);
    match action {
        PrimaryActionKind::Open => {
            // Classic Windscribe power ring with top stem
            let steps = 28;
            let start = -FRAC_PI_2 + 0.45;
            let end = -FRAC_PI_2 + TAU - 0.45;
            let pts: Vec<Pos2> = (0..=steps)
                .map(|i| {
                    let a = start + (end - start) * (i as f32 / steps as f32);
                    pos2(center.x + a.cos() * radius, center.y + a.sin() * radius)
                })
                .collect();
            painter.add(Shape::line(pts, stroke));
            painter.line_segment(
                [
                    pos2(center.x, center.y - radius * 1.15),
                    pos2(center.x, center.y - radius * 0.15),
                ],
                stroke,
            );
        }
        PrimaryActionKind::Install => {
            // Downward install arrow + tray bar
            painter.line_segment(
                [
                    pos2(center.x, center.y - radius * 0.85),
                    pos2(center.x, center.y + radius * 0.42),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x - radius * 0.5, center.y - radius * 0.02),
                    pos2(center.x, center.y + radius * 0.48),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x + radius * 0.5, center.y - radius * 0.02),
                    pos2(center.x, center.y + radius * 0.48),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x - radius * 0.72, center.y + radius * 0.82),
                    pos2(center.x + radius * 0.72, center.y + radius * 0.82),
                ],
                stroke,
            );
        }
        PrimaryActionKind::Update => {
            // Upward update arrow inside clean ring arc
            painter.line_segment(
                [
                    pos2(center.x, center.y + radius * 0.75),
                    pos2(center.x, center.y - radius * 0.55),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x - radius * 0.48, center.y - radius * 0.05),
                    pos2(center.x, center.y - radius * 0.62),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x + radius * 0.48, center.y - radius * 0.05),
                    pos2(center.x, center.y - radius * 0.62),
                ],
                stroke,
            );
        }
        PrimaryActionKind::RetryCheck => {
            // Refresh / retry arc
            paint_refresh_icon(painter, center, radius * 0.85, 0.0, color);
        }
    }
}

pub fn paint_refresh_icon(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    angle: f32,
    color: Color32,
) {
    let stroke = Stroke::new(1.8_f32, color);
    let steps = 18;
    let start = angle + 0.35;
    let end = angle + TAU - 0.75;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + (end - start) * (i as f32 / steps as f32);
            pos2(center.x + a.cos() * radius, center.y + a.sin() * radius)
        })
        .collect();
    painter.add(Shape::line(pts, stroke));

    let tip = pos2(center.x + end.cos() * radius, center.y + end.sin() * radius);
    let tang = end + FRAC_PI_2;
    painter.line_segment(
        [
            tip,
            pos2(
                tip.x - (tang - 0.55).cos() * radius * 0.45,
                tip.y - (tang - 0.55).sin() * radius * 0.45,
            ),
        ],
        stroke,
    );
    painter.line_segment(
        [
            tip,
            pos2(
                tip.x - (tang + 0.65).cos() * radius * 0.45,
                tip.y - (tang + 0.65).sin() * radius * 0.45,
            ),
        ],
        stroke,
    );
}

pub fn paint_eye_icon(painter: &Painter, center: Pos2, radius: f32, visible: bool, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let w = radius * 1.1;
    let h = radius * 0.6;
    let top_pts = vec![
        pos2(center.x - w, center.y),
        pos2(center.x - w * 0.45, center.y - h),
        pos2(center.x + w * 0.45, center.y - h),
        pos2(center.x + w, center.y),
    ];
    let bot_pts = vec![
        pos2(center.x - w, center.y),
        pos2(center.x - w * 0.45, center.y + h),
        pos2(center.x + w * 0.45, center.y + h),
        pos2(center.x + w, center.y),
    ];
    painter.add(Shape::line(top_pts, stroke));
    painter.add(Shape::line(bot_pts, stroke));
    painter.circle_filled(center, radius * 0.28, color);
    if !visible {
        painter.line_segment(
            [
                pos2(center.x - radius * 0.85, center.y + radius * 0.65),
                pos2(center.x + radius * 0.85, center.y - radius * 0.65),
            ],
            Stroke::new(1.7_f32, color),
        );
    }
}

pub fn paint_copy_icon(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    let back = Rect::from_center_size(
        pos2(center.x - radius * 0.18, center.y - radius * 0.18),
        Vec2::splat(radius * 1.05),
    );
    let front = Rect::from_center_size(
        pos2(center.x + radius * 0.22, center.y + radius * 0.22),
        Vec2::splat(radius * 1.05),
    );
    painter.rect_stroke(back, 2.0, stroke);
    painter.rect_stroke(front, 2.0, stroke);
}

pub fn paint_edit_icon(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    painter.line_segment(
        [
            pos2(center.x - radius * 0.65, center.y + radius * 0.65),
            pos2(center.x + radius * 0.65, center.y - radius * 0.65),
        ],
        stroke,
    );
    painter.circle_filled(
        pos2(center.x - radius * 0.65, center.y + radius * 0.65),
        1.5,
        color,
    );
}

pub fn paint_trash_icon(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    painter.line_segment(
        [
            pos2(center.x - radius * 0.72, center.y - radius * 0.42),
            pos2(center.x + radius * 0.72, center.y - radius * 0.42),
        ],
        stroke,
    );
    painter.line_segment(
        [
            pos2(center.x - radius * 0.28, center.y - radius * 0.68),
            pos2(center.x + radius * 0.28, center.y - radius * 0.68),
        ],
        stroke,
    );
    let body = Rect::from_min_max(
        pos2(center.x - radius * 0.52, center.y - radius * 0.35),
        pos2(center.x + radius * 0.52, center.y + radius * 0.72),
    );
    painter.rect_stroke(body, 2.0, stroke);
}

pub fn paint_search_icon(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    let stroke = Stroke::new(1.5_f32, color);
    let c = pos2(center.x - radius * 0.18, center.y - radius * 0.18);
    painter.circle_stroke(c, radius * 0.55, stroke);
    painter.line_segment(
        [
            pos2(c.x + radius * 0.39, c.y + radius * 0.39),
            pos2(center.x + radius * 0.75, center.y + radius * 0.75),
        ],
        stroke,
    );
}

pub fn paint_window_controls(
    painter: &Painter,
    rect: Rect,
    kind: WindowControlKind,
    color: Color32,
) {
    let c = rect.center();
    let stroke = Stroke::new(1.3_f32, color);
    match kind {
        WindowControlKind::Minimize => {
            painter.line_segment(
                [pos2(c.x - 5.0, c.y + 1.0), pos2(c.x + 5.0, c.y + 1.0)],
                stroke,
            );
        }
        WindowControlKind::Maximize => {
            let box_rect = Rect::from_center_size(c, Vec2::splat(9.0));
            painter.rect_stroke(box_rect, 1.5, stroke);
        }
        WindowControlKind::Close => {
            painter.line_segment(
                [pos2(c.x - 4.5, c.y - 4.5), pos2(c.x + 4.5, c.y + 4.5)],
                stroke,
            );
            painter.line_segment(
                [pos2(c.x - 4.5, c.y + 4.5), pos2(c.x + 4.5, c.y - 4.5)],
                stroke,
            );
        }
    }
    let _ = Align2::CENTER_CENTER;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowControlKind {
    Minimize,
    Maximize,
    Close,
}
