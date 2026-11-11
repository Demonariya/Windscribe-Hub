use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
        let icon = out.join("windscribe-pro.ico");
        write_icon(&icon).expect("could not generate the application icon");

        let mut resource = winres::WindowsResource::new();
        resource.set_icon(icon.to_str().expect("icon path is not valid UTF-8"));
        resource.set("ProductName", "Windscribe Pro");
        resource.set("FileDescription", "Windscribe Pro desktop manager");
        resource.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        resource.compile().expect("could not embed the Windows application icon");
    }
}

fn write_icon(path: &PathBuf) -> std::io::Result<()> {
    const W: u32 = 32;
    const H: u32 = 32;

    let mut pixels = Vec::with_capacity((W * H * 4) as usize);
    for y in (0..H).rev() {
        for x in 0..W {
            let color = pixel(x, y);
            pixels.extend_from_slice(&[color.2, color.1, color.0, 255]);
        }
    }

    let mask_row = ((W + 31) / 32) * 4;
    let mask = vec![0u8; (mask_row * H) as usize];
    let image_size = 40 + pixels.len() as u32 + mask.len() as u32;
    let mut bytes = Vec::with_capacity(22 + image_size as usize);

    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());

    bytes.push(W as u8);
    bytes.push(H as u8);
    bytes.push(0);
    bytes.push(0);
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&image_size.to_le_bytes());
    bytes.extend_from_slice(&22u32.to_le_bytes());

    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&(W as i32).to_le_bytes());
    bytes.extend_from_slice(&((H * 2) as i32).to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&((W * H * 4) as u32).to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&pixels);
    bytes.extend_from_slice(&mask);

    fs::write(path, bytes)
}

fn pixel(x: u32, y: u32) -> (u8, u8, u8) {
    let inside = |points: &[(f32, f32)]| {
        let mut hit = false;
        let mut j = points.len() - 1;
        for i in 0..points.len() {
            let (xi, yi) = points[i];
            let (xj, yj) = points[j];
            if ((yi > y as f32) != (yj > y as f32))
                && ((x as f32) < (xj - xi) * (y as f32 - yi) / (yj - yi + 0.0001) + xi)
            {
                hit = !hit;
            }
            j = i;
        }
        hit
    };

    let edge = x <= 1 || y <= 1 || x >= 30 || y >= 30;
    if edge {
        return (8, 124, 255);
    }

    let shield = [(16.0, 4.0), (26.0, 8.0), (25.0, 19.0), (21.0, 26.0), (16.0, 29.0),
                  (11.0, 26.0), (7.0, 19.0), (6.0, 8.0)];
    if inside(&shield) {
        let left = 8.0 + (y as f32 * 0.18);
        let right = 24.0 - (y as f32 * 0.18);
        if x as f32 <= left + 1.0 || x as f32 >= right - 1.0 {
            return (32, 185, 244);
        }
        let w_points = [
            ((10.0, 12.0), (13.0, 21.0)),
            ((13.0, 21.0), (16.0, 15.0)),
            ((16.0, 15.0), (19.0, 21.0)),
            ((19.0, 21.0), (22.0, 12.0)),
        ];
        for ((ax, ay), (bx, by)) in w_points {
            let vx = bx - ax;
            let vy = by - ay;
            let t = (((x as f32 - ax) * vx + (y as f32 - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
            let dx = x as f32 - (ax + t * vx);
            let dy = y as f32 - (ay + t * vy);
            if dx * dx + dy * dy <= 1.6 {
                return (244, 248, 255);
            }
        }
        return (8, 74, 133);
    }
    (13, 37, 67)
}
