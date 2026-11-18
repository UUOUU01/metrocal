//! Metro palette. (Render-layer agnostic; consumed by the Skia drawing code.)
//!
//! Drawing code converts `Color32` to `skia_safe::Color` via `skc(color, alpha)`.

/// Simple RGB color (no alpha; alpha is supplied at draw time from animation state).
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

/// Classic Metro / Windows Phone solid color palette.
pub const ACCENTS: [Color32; 8] = [
    Color32::from_rgb(0x00, 0x78, 0xD4), // Cobalt
    Color32::from_rgb(0x00, 0xAB, 0xA9), // Teal
    Color32::from_rgb(0x10, 0x7C, 0x10), // Green
    Color32::from_rgb(0xC2, 0x39, 0xB3), // Purple
    Color32::from_rgb(0xFF, 0x00, 0x97), // Magenta
    Color32::from_rgb(0xF7, 0x63, 0x0C), // Orange
    Color32::from_rgb(0xE8, 0x11, 0x23), // Red
    Color32::from_rgb(0x6B, 0x69, 0xD6), // Indigo
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
            bg: Color32::from_rgb(0x1A, 0x1A, 0x1A),
            panel: Color32::from_rgb(0x25, 0x25, 0x25),
            text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            weak: Color32::from_rgb(0xB0, 0xB0, 0xB0),
            util_bg: Color32::from_rgb(0x3A, 0x3A, 0x3A),
            util_text: Color32::from_rgb(0xEE, 0xEE, 0xEE),
            digit: accent,
            op: shade(accent, 0.70),
            equals: Color32::from_rgb(0xFF, 0x82, 0x2D),
            error: Color32::from_rgb(0xFF, 0x8A, 0x8A),
            accent,
            ripple_dark: false,
        },
        Theme::Light => Palette {
            bg: Color32::from_rgb(0xF8, 0xF8, 0xF8),
            panel: Color32::from_rgb(0xE8, 0xE8, 0xE8),
            text: Color32::from_rgb(0x2A, 0x2A, 0x2A),
            weak: Color32::from_rgb(0x90, 0x90, 0x90),
            util_bg: Color32::from_rgb(0xD0, 0xD0, 0xD0),
            util_text: Color32::from_rgb(0x2A, 0x2A, 0x2A),
            digit: accent,
            op: shade(accent, 0.75),
            equals: Color32::from_rgb(0xFF, 0x95, 0x3D),
            error: Color32::from_rgb(0xFF, 0x9E, 0x9E),
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
    Sci,
}

impl TileKind {
    pub fn base(self, p: &Palette) -> Color32 {
        match self {
            TileKind::Digit => p.digit,
            TileKind::Op => p.op,
            TileKind::Util => p.util_bg,
            TileKind::Equals => p.equals,
            TileKind::Sci => p.digit, // use same as digit for consistency
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
