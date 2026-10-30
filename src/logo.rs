//! The launcher's own "OMSI 2" logo: heavy slanted letters cut by horizontal stripes, with a silver 2. Drawn as an SVG so it
//! follows the theme colours.

use crate::theme::Palette;
use eframe::egui::Color32;

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

pub fn svg(p: &Palette) -> String {
    let light_bg = (u32::from(p.bg.r()) + u32::from(p.bg.g()) + u32::from(p.bg.b())) / 3 > 200;
    let (silver_top, silver_bottom) = if light_bg { ("#b9bdc7", "#6e7380") } else { ("#f4f4f8", "#9a9ca8") };
    let letters = hex(p.dark);
    // Stripes that cut the lower half of every letter, a little thicker towards the bottom.
    let stripes = [(58.0, 2.4), (65.0, 3.0), (72.5, 3.6), (80.0, 4.2)]
        .iter()
        .map(|(y, h)| format!(r#"<rect y="{y}" width="380" height="{h}" fill="black"/>"#))
        .collect::<String>();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 380 100">
  <defs>
    <linearGradient id="silver" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{silver_top}"/>
      <stop offset="1" stop-color="{silver_bottom}"/>
    </linearGradient>
    <mask id="cut" maskUnits="userSpaceOnUse" x="0" y="0" width="380" height="100">
      <rect width="380" height="100" fill="white"/>{stripes}
    </mask>
  </defs>
  <g mask="url(#cut)" transform="translate(22 0) skewX(-12)" fill="none" stroke-width="15" stroke-linejoin="round">
    <g stroke="{letters}">
      <rect x="8" y="16" width="64" height="68" rx="30"/>
      <path d="M92 86 V16 L120 56 L148 16 V86" stroke-linejoin="miter"/>
      <path d="M214 30 C210 14 164 12 164 34 C164 56 216 46 216 68 C216 90 168 88 162 70"/>
      <path d="M242 16 V86"/>
    </g>
    <path stroke="url(#silver)" d="M284 34 C286 10 332 12 332 36 C332 56 292 66 282 86 H336"/>
  </g>
</svg>"##
    )
}
