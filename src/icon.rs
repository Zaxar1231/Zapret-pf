//! Программно нарисованная иконка «кнопка питания на круге» (окно и трей).

/// RGBA-пиксели иконки n×n с заданным цветом круга.
pub fn power_icon_rgba(n: u32, rgb: (u8, u8, u8)) -> Vec<u8> {
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    let s = n as f32 / 64.0; // рисуем в координатах 64×64
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = ((x as f32 + 0.5 - c) / s, (y as f32 + 0.5 - c) / s);
            let d = (dx * dx + dy * dy).sqrt();
            let a = ((32.0 - d) * s).clamp(0.0, 1.0);
            if a <= 0.0 {
                continue;
            }
            let ang = dy.atan2(dx);
            let ring = (d - 17.0).abs() < 3.5 && !(ang < -1.0 && ang > -2.14);
            let bar = dx.abs() < 3.5 && dy < -2.0 && dy > -23.0;
            let (r, g, b) = if ring || bar { (255, 255, 255) } else { rgb };
            let i = ((y * n + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[r, g, b, (a * 255.0) as u8]);
        }
    }
    rgba
}

pub const COLOR_ON: (u8, u8, u8) = (0x3F, 0x5F, 0xA9);
#[cfg_attr(not(windows), allow(dead_code))]
pub const COLOR_OFF: (u8, u8, u8) = (0x75, 0x77, 0x80);
