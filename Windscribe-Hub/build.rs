fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    windows_resources::embed_icon_and_metadata();
}

#[cfg(windows)]
mod windows_resources {
    use std::{env, fs, path::PathBuf};

    pub fn embed_icon_and_metadata() {
        if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
            return;
        }
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set"));
        let icon = out.join("windscribe-hub.ico");
        write_icon(&icon).expect("could not generate the Windscribe Hub application icon");

        let mut resource = winres::WindowsResource::new();
        resource.set_icon(icon.to_str().expect("icon path is not valid UTF-8"));
        resource.set("ProductName", "Windscribe Hub");
        resource.set(
            "FileDescription",
            "Windscribe Hub — Native Windows utility for Windscribe",
        );
        resource.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        resource
            .compile()
            .expect("could not embed the Windows application icon");
    }

    fn write_icon(path: &PathBuf) -> std::io::Result<()> {
        const W: u32 = 32;
        const H: u32 = 32;

        let mut pixels = Vec::with_capacity((W * H * 4) as usize);
        for y in (0..H).rev() {
            for x in 0..W {
                let color = pixel(x, y);
                pixels.extend_from_slice(&[color.2, color.1, color.0, color.3]);
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

    fn pixel(x: u32, y: u32) -> (u8, u8, u8, u8) {
        let fx = x as f32 + 0.5;
        let fy = y as f32 + 0.5;
        let cx = 16.0;
        let cy = 16.0;
        let dx = fx - cx;
        let dy = fy - cy;
        let dist = (dx * dx + dy * dy).sqrt();

        // Rounded badge background (#0B111B with #00E589 accent ring)
        let rx = dx.abs();
        let ry = dy.abs();
        let in_box = rx <= 14.5
            && ry <= 14.5
            && !((rx > 9.5 && ry > 9.5) && ((rx - 9.5).powi(2) + (ry - 9.5).powi(2) > 25.0));
        if !in_box {
            return (0, 0, 0, 0);
        }

        if (10.6..=12.4).contains(&dist) {
            return (0, 229, 137, 255);
        }

        // Stylized Windscribe Hub 'W' wave mark
        let w_segments = [
            ((8.0, 11.0), (11.5, 21.0)),
            ((11.5, 21.0), (16.0, 14.0)),
            ((16.0, 14.0), (20.5, 21.0)),
            ((20.5, 21.0), (24.0, 11.0)),
        ];
        for ((ax, ay), (bx, by)) in w_segments {
            let vx = bx - ax;
            let vy = by - ay;
            let t = (((fx - ax) * vx + (fy - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
            let px = fx - (ax + t * vx);
            let py = fy - (ay + t * vy);
            if px * px + py * py <= 2.1 {
                return (245, 247, 250, 255);
            }
        }

        (16, 25, 39, 255)
    }
}
