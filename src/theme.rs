//! Metro 配色。Metro palette.（渲染层无关，供 Skia 使用）
//!
//! 绘制处用 `skc(color, alpha)` 把 `Color32` 转为 `skia_safe::Color`。

/// 简单 RGB 颜色（不含 alpha；alpha 在绘制处按动画参数即时给定）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color32(pub u8, pub u8, pub u8);

impl Color32 {
    pub const WHITE: Color32 = Color32(255, 255, 255);
    pub const BLACK: Color32 = Color32(0, 0, 0);

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Color32 {
        Color32(r, g, b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn code(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub fn from_code(s: &str) -> Theme {
        match s {
            "light" => Theme::Light,
            _ => Theme::Dark,
        }
    }

    pub fn flip(self) -> Theme {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
}

/// 经典 Metro / Windows Phone 纯色盘。
pub const ACCENTS: [Color32; 8] = [
    Color32::from_rgb(0x00, 0x78, 0xD4), // 蓝 Cobalt
    Color32::from_rgb(0x00, 0xAB, 0xA9), // 青 Teal
    Color32::from_rgb(0x10, 0x7C, 0x10), // 绿 Green
    Color32::from_rgb(0xC2, 0x39, 0xB3), // 紫 Purple
    Color32::from_rgb(0xFF, 0x00, 0x97), // 品红 Magenta
    Color32::from_rgb(0xF7, 0x63, 0x0C), // 橙 Orange
    Color32::from_rgb(0xE8, 0x11, 0x23), // 红 Red
    Color32::from_rgb(0x6B, 0x69, 0xD6), // 靛 Indigo
];

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub util_bg: Color32,
    pub util_text: Color32,
    pub digit: Color32,
    pub op: Color32,
    pub equals: Color32,
    pub error: Color32,
    pub accent: Color32,
    pub ripple_dark: bool,
}

pub fn pal(theme: Theme, accent_idx: usize) -> Palette {
    let accent = ACCENTS[accent_idx.min(ACCENTS.len() - 1)];
    match theme {
        Theme::Dark => Palette {
            bg: Color32::from_rgb(0x17, 0x17, 0x17),
            panel: Color32::from_rgb(0x1F, 0x1F, 0x1F),
            text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            weak: Color32::from_rgb(0x9A, 0x9A, 0x9A),
            util_bg: Color32::from_rgb(0x33, 0x33, 0x33),
            util_text: Color32::WHITE,
            digit: accent,
            op: shade(accent, 0.62),
            equals: Color32::from_rgb(0xF7, 0x63, 0x0C),
            error: Color32::from_rgb(0xFF, 0x6B, 0x6B),
            accent,
            ripple_dark: false,
        },
        Theme::Light => Palette {
            bg: Color32::WHITE,
            panel: Color32::from_rgb(0xF3, 0xF3, 0xF3),
            text: Color32::from_rgb(0x1A, 0x1A, 0x1A),
            weak: Color32::from_rgb(0x63, 0x63, 0x63),
            util_bg: Color32::from_rgb(0xE3, 0xE3, 0xE3),
            util_text: Color32::from_rgb(0x1A, 0x1A, 0x1A),
            digit: accent,
            op: shade(accent, 0.80),
            equals: Color32::from_rgb(0xF7, 0x63, 0x0C),
            error: Color32::from_rgb(0xC4, 0x2B, 0x1C),
            accent,
            ripple_dark: true,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileKind {
    Digit,
    Op,
    Util,
    Equals,
}

impl TileKind {
    pub fn base(self, p: &Palette) -> Color32 {
        match self {
            TileKind::Digit => p.digit,
            TileKind::Op => p.op,
            TileKind::Util => p.util_bg,
            TileKind::Equals => p.equals,
        }
    }

    pub fn text(self, p: &Palette) -> Color32 {
        match self {
            TileKind::Util => p.util_text,
            _ => Color32::WHITE,
        }
    }
}

// ---------- color helpers ----------

pub fn shade(c: Color32, f: f32) -> Color32 {
    let ch = |v: u8| ((v as f32 * f).min(255.0)) as u8;
    Color32::from_rgb(ch(c.0), ch(c.1), ch(c.2))
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2))
}

pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}
