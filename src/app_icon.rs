//! The application icon, drawn with plain maths so the same code serves the window icon (at run time) and the .exe icon (in
//! build.rs): an orange rounded square with a white "O" ring cut by horizontal stripes, like the logo.

/// Colour of the pixel at (u, v), both in 0..1, as RGBA; alpha 0 outside the rounded square.
fn sample(u: f32, v: f32) -> [f32; 4] {
    // rounded square, 6 % margin
    let (m, r) = (0.06_f32, 0.22_f32);
    let (x, y) = ((u - 0.5).abs(), (v - 0.5).abs());
    let half = 0.5 - m;
    let (dx, dy) = ((x - (half - r)).max(0.0), (y - (half - r)).max(0.0));
    if x > half || y > half || dx * dx + dy * dy > r * r {
        return [0.0, 0.0, 0.0, 0.0];
    }
    // vertical gradient from a lighter to a darker orange
    let k = v;
    let bg = [1.0 - 0.12 * k, 0.62 - 0.22 * k, 0.10 - 0.04 * k];
    // slanted "O" ring (skew -12 degrees around the centre)
    let (cx, cy) = (0.5, 0.5);
    let sx = (u - cx) + (v - cy) * 0.21;
    let sy = v - cy;
    let d = ((sx / 0.32).powi(2) + (sy / 0.32).powi(2)).sqrt();
    let in_ring = d < 1.0 && d > 0.50;
    // horizontal cuts in the lower half
    let cuts = [(0.545, 0.018), (0.610, 0.024), (0.680, 0.030), (0.755, 0.036)];
    let cut = cuts.iter().any(|(y0, h)| v >= *y0 && v < y0 + h);
    if in_ring && !cut {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        [bg[0], bg[1], bg[2], 1.0]
    }
}

/// `size` x `size` RGBA pixels (straight alpha), 3x3 supersampled.
pub fn rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0_f32; 4];
            for sy in 0..3 {
                for sx in 0..3 {
                    let u = (px as f32 + (sx as f32 + 0.5) / 3.0) / size as f32;
                    let v = (py as f32 + (sy as f32 + 0.5) / 3.0) / size as f32;
                    let s = sample(u, v);
                    // premultiply while averaging
                    acc[0] += s[0] * s[3];
                    acc[1] += s[1] * s[3];
                    acc[2] += s[2] * s[3];
                    acc[3] += s[3];
                }
            }
            let a = acc[3] / 9.0;
            let un = |c: f32| if acc[3] > 0.0 { (c / acc[3]).clamp(0.0, 1.0) } else { 0.0 };
            out.extend([(un(acc[0]) * 255.0) as u8, (un(acc[1]) * 255.0) as u8, (un(acc[2]) * 255.0) as u8, (a * 255.0) as u8]);
        }
    }
    out
}
