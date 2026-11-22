use crate::ui::metrics::{ANIM_FAST_SECS, ANIM_NORMAL_SECS};
use egui::{Color32, Context, Id};

pub fn animate_bool(ctx: &Context, id: Id, value: bool, enabled: bool) -> f32 {
    if !enabled {
        return if value { 1.0 } else { 0.0 };
    }
    ctx.animate_bool_with_time(id, value, ANIM_NORMAL_SECS)
}

pub fn animate_fast(ctx: &Context, id: Id, value: bool, enabled: bool) -> f32 {
    if !enabled {
        return if value { 1.0 } else { 0.0 };
    }
    ctx.animate_bool_with_time(id, value, ANIM_FAST_SECS)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let x = 1.0 - t.clamp(0.0, 1.0);
    1.0 - x * x * x
}

pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |ca: u8, cb: u8| -> u8 {
        ((ca as f32) + ((cb as f32) - (ca as f32)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        mix(a.a(), b.a()),
    )
}

pub fn pulse_factor(ctx: &Context, enabled: bool, speed_hz: f32) -> f32 {
    if !enabled {
        return 1.0;
    }
    let t = ctx.input(|i| i.time) as f32;
    0.5 + 0.5 * (t * speed_hz * std::f32::consts::TAU).sin()
}
