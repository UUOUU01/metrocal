//! UI layer: Metro tiles, animations, pages, persistence. 100% safe Rust.
//! Rendering: Skia GPU (OpenGL) preferred, CPU raster fallback; both paths share the same drawing code.
//!
//! Modeled on the Windows 8 start screen: full-screen colorful tile grid, groups,
//! minimal white vector icons, progressive staggered entrance, live tile flip
//! (lightweight 2D flip via Skia transform matrices).

use std::collections::HashMap;
use std::ffi::CString;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::{GlSurface, NotCurrentGlContext};
use glutin::surface::{
    Surface as GlutinSurface, SurfaceAttributesBuilder, SwapInterval, WindowSurface,
};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use skia_safe::{
    Canvas, ClipOp, ColorType, Data, Font, FontMgr, ImageInfo, Paint, PaintStyle, Path,
    PathBuilder, Point, Rect, Surface as SkSurface, Typeface,
};
use softbuffer::Surface as SbSurface;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, NamedKey};
use winit::window::{CursorIcon, Window};

use crate::engine::{Engine, ErrKey, HistKind, LastExpr, Op, Unary};
use crate::gl_context;
use crate::i18n::{self, Lang};
use crate::theme::{self, Color32, Palette, Theme, TileKind};

const TRANSITION_SECS: f32 = 0.24;
const SPLASH_SECS: f32 = 0.85;
const ROLL_SECS: f32 = 0.20;
const POP_SECS: f32 = 0.35;
const SHAKE_SECS: f32 = 0.35;
const RIPPLE_SECS: f32 = 0.28;
const HIST_W: f32 = 280.0;
const PALETTE_FADE_SECS: f32 = 0.30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Start,
    Calc,
    Help,
    Settings,
}

impl Page {
    fn index(self) -> usize {
        match self {
            Page::Start => 0,
            Page::Calc => 1,
            Page::Help => 2,
            Page::Settings => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Digit(u8),
    Dot,
    Opset(Op),
    Equals,
    Clear,
    ClearEntry,
    Back,
    Percent,
    Negate,
    Recip,
    Sqr,
    Sqrt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum BarBtn {
    Back,
    Home,
    Lang,
    Theme,
    Settings,
    Help,
    History,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum HitKey {
    Tile(u64, usize),
    StartTile(u64, usize),
    Bar(BarBtn),
    Lang(Lang),
    ThemeOpt(Theme),
    Accent(usize),
    HistItem(u64),
    HistClear,
    SetClear,
    SplashSkip,
}

#[derive(Clone, Copy, Debug)]
enum HitAct {
    Calc(Action),
    GoTo(Page),
    ToggleHist,
    OpenCalcHistory,
    CycleLang,
    FlipTheme,
    SetLang(Lang),
    SetTheme(Theme),
    SetAccent(usize),
    LoadHist(u64),
    ClearHist,
    SkipSplash,
    Exit,
}

/// Minimal white vector icons for Windows 8 start-screen tiles (vector drawn).
#[derive(Clone, Copy, Debug)]
enum Glyph {
    Calculator,
    Clock,
    Question,
    Gear,
    Moon,
    LangGlyph,
    Trash,
    Power,
}

/// Start-screen tile definition.
struct StartTileDef {
    /// Column span (1 = small tile, 2 = Win8 large tile filling 2×2)
    span: u32,
    glyph: Glyph,
    label: &'static str,
    color: Color32,
    act: HitAct,
}

#[derive(Clone, Copy)]
enum TileLabel {
    Text(&'static str),
    BackIcon,
}

#[derive(Clone, Copy, Debug)]
struct HitRegion {
    key: HitKey,
    rect: Rect,
    act: HitAct,
}

#[derive(Clone, Copy, Debug)]
struct Ripple {
    rect: Rect,
    origin: (f32, f32),
    start: Instant,
    dark: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Smooth {
    value: f32,
}

impl Smooth {
    fn update(&mut self, target: f32, dt: f32, speed: f32) {
        let k = 1.0 - (-dt * speed).exp();
        self.value += (target - self.value) * k;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Anim2 {
    hover: Smooth,
    press: Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextAlign {
    Left,
    Center,
    Right,
}

// ---------- draw-command frame (decoupled from state to avoid borrow conflicts) ----------

enum Cmd {
    FillRect(Rect, skia_safe::Color),
    StrokeRect(Rect, skia_safe::Color, f32),
    FillCircle(Point, f32, skia_safe::Color),
    StrokeCircle(Point, f32, skia_safe::Color, f32),
    Line(Point, Point, skia_safe::Color, f32),
    FillPath(Path, skia_safe::Color),
    Text {
        pos: Point,
        text: String,
        font: Font,
        color: skia_safe::Color,
    },
    PushClip(Rect),
    PushTranslate(f32, f32),
    PushScale(Point, f32, f32),
    Pop,
}

#[derive(Default)]
struct Frame {
    cmds: Vec<Cmd>,
    regions: Vec<HitRegion>,
    cursor_pointer: bool,
}

impl Frame {
    fn push_clip(&mut self, r: Rect) {
        self.cmds.push(Cmd::PushClip(r));
    }
    fn push_translate(&mut self, x: f32, y: f32) {
        self.cmds.push(Cmd::PushTranslate(x, y));
    }
    fn push_scale(&mut self, origin: (f32, f32), sx: f32, sy: f32) {
        self.cmds.push(Cmd::PushScale((origin.0, origin.1).into(), sx, sy));
    }
    fn pop(&mut self) {
        self.cmds.push(Cmd::Pop);
    }
}

fn paint(canvas: &Canvas, fr: &Frame) {
    for cmd in &fr.cmds {
        match cmd {
            Cmd::FillRect(r, c) => {
                let mut p = Paint::default();
                p.set_anti_alias(false);
                p.set_color(*c);
                canvas.draw_rect(*r, &p);
            }
            Cmd::StrokeRect(r, c, w) => {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_style(PaintStyle::Stroke);
                p.set_stroke_width(*w);
                p.set_color(*c);
                canvas.draw_rect(*r, &p);
            }
            Cmd::FillCircle(pt, rad, c) => {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_color(*c);
                canvas.draw_circle(*pt, *rad, &p);
            }
            Cmd::StrokeCircle(pt, rad, c, w) => {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_style(PaintStyle::Stroke);
                p.set_stroke_width(*w);
                p.set_color(*c);
                canvas.draw_circle(*pt, *rad, &p);
            }
            Cmd::Line(a, b, c, w) => {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_stroke_width(*w);
                p.set_color(*c);
                canvas.draw_line(*a, *b, &p);
            }
            Cmd::FillPath(path, c) => {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_color(*c);
                canvas.draw_path(path, &p);
            }
            Cmd::Text {
                pos,
                text,
                font,
                color,
            } => {
                let mut p = Paint::default();
                p.set_anti_alias(true);
                p.set_color(*color);
                canvas.draw_str(text, *pos, font, &p);
            }
            Cmd::PushClip(r) => {
                canvas.save();
                canvas.clip_rect(*r, ClipOp::Intersect, false);
            }
            Cmd::PushTranslate(x, y) => {
                canvas.save();
                canvas.translate((*x, *y));
            }
            Cmd::PushScale(o, sx, sy) => {
                canvas.save();
                canvas.translate((o.x, o.y));
                canvas.scale((*sx, *sy));
            }
            Cmd::Pop => {
                canvas.restore();
            }
        }
    }
}

fn skc(c: Color32, alpha: f32) -> skia_safe::Color {
    skia_safe::Color::from_argb((alpha.clamp(0.0, 1.0) * 255.0) as u8, c.0, c.1, c.2)
}

/// Cross-fade between two palettes (theme / accent switch animation).
fn mix_palette(a: &Palette, b: &Palette, t: f32) -> Palette {
    let m = |x: Color32, y: Color32| theme::mix(x, y, t);
    Palette {
        bg: m(a.bg, b.bg),
        panel: m(a.panel, b.panel),
        text: m(a.text, b.text),
        weak: m(a.weak, b.weak),
        util_bg: m(a.util_bg, b.util_bg),
        util_text: m(a.util_text, b.util_text),
        digit: m(a.digit, b.digit),
        op: m(a.op, b.op),
        equals: m(a.equals, b.equals),
        error: m(a.error, b.error),
        accent: m(a.accent, b.accent),
        ripple_dark: if t < 0.5 { a.ripple_dark } else { b.ripple_dark },
    }
}

fn rect_contains(r: Rect, x: f32, y: f32) -> bool {
    x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
}

fn rect_center(r: Rect) -> (f32, f32) {
    (r.center_x(), r.center_y())
}

/// Tile unit size (based on row height).
fn u_size(rect: Rect) -> f32 {
    rect.height()
}

// ---------- persistence ----------

#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    lang: String,
    theme: String,
    accent: usize,
    history: Vec<crate::engine::HistoryItem>,
}

// ---------- render backend ----------

enum Backend {
    Gpu {
        gr_context: skia_safe::gpu::DirectContext,
        gl_surface: GlutinSurface<WindowSurface>,
        gl_context: PossiblyCurrentContext,
        surface: SkSurface,
        fb_info: skia_safe::gpu::gl::FramebufferInfo,
        num_samples: usize,
        stencil_size: usize,
    },
    Cpu {
        surface: SkSurface,
        sb_surface: SbSurface<Arc<Window>, Arc<Window>>,
        pixels: Vec<u8>,
    },
}

// ---------- app ----------

pub struct MetroCalcApp {
    engine: Engine,
    lang: Lang,
    theme: Theme,
    accent_idx: usize,
    page: Page,
    page_switch: Option<(Instant, Page, Page)>,
    history_open: bool,
    ripples: Vec<Ripple>,
    t0: Instant,
    splash_done: bool,
    entrance_gen: u64,
    entrance_start: Instant,
    roll: Option<(f64, Instant)>,
    prev_value: Option<f64>,
    prev_error: Option<ErrKey>,
    result_pop: Option<Instant>,
    shake: Option<Instant>,
    settings_path: Option<PathBuf>,
    palette_fade: Option<(Instant, Palette)>,
    hist_open_at: Option<Instant>,
    page_enter: Instant,
    last_move: Instant,
    anim_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    settings_tx: Option<std::sync::mpsc::Sender<(PathBuf, String)>>,

    start_scroll_y: f32,
    start_content_h: f32,
    calc_scroll: f32,
    drag: Option<(bool, f32, f32)>,
    start_scroll_vel: f32,
    ui_scale: f32,
    quit: bool,

    anims: HashMap<HitKey, Anim2>,
    back_vis: Smooth,
    hist_w: Smooth,
    hist_scroll: f32,
    help_scroll: f32,
    set_scroll: f32,
    help_content_h: f32,
    set_content_h: f32,

    lw: f32,
    lh: f32,
    scale: f32,
    pointer: (f32, f32),
    hover: Option<HitKey>,
    pressed: Option<HitKey>,
    focus: Option<usize>,
    regions: Vec<HitRegion>,
    cursor_pointer: bool,
    cursor_icon: CursorIcon,
    shift: bool,
    last_frame: Instant,

    tf_display: Option<Typeface>,
    tf_body: Option<Typeface>,

    backend: Option<Backend>,
    surface_size: (u32, u32),
    dirty: bool,
    // keep `backend` dropped BEFORE `window`
    window: Option<Arc<Window>>,
}

impl MetroCalcApp {
    pub fn new() -> Self {
        let settings_path = config_dir().map(|d| d.join("metro-calculator").join("settings.json"));
        let (tf_display, tf_body) = load_typefaces();
        eprintln!(
            "fonts: display={} body={}",
            tf_display.is_some(),
            tf_body.is_some()
        );
        // Background settings writer: a single dedicated thread owns all
        // settings/history disk I/O so the UI thread never blocks on writes.
        let settings_tx = settings_path.clone().map(|_| {
            let (tx, rx) = std::sync::mpsc::channel::<(PathBuf, String)>();
            std::thread::spawn(move || {
                while let Ok((path, json)) = rx.recv() {
                    if let Some(dir) = path.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let _ = std::fs::write(&path, json);
                }
            });
            tx
        });
        let mut app = Self {
            engine: Engine::new(),
            lang: Lang::En,
            theme: Theme::Dark,
            accent_idx: 0,
            page: Page::Start,
            page_switch: None,
            history_open: false,
            ripples: Vec::new(),
            t0: Instant::now(),
            splash_done: false,
            entrance_gen: 1,
            entrance_start: Instant::now(),
            roll: None,
            prev_value: None,
            prev_error: None,
            result_pop: None,
            shake: None,
            settings_path: settings_path.clone(),
            palette_fade: None,
            hist_open_at: None,
            page_enter: Instant::now(),
            last_move: Instant::now(),
            anim_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            settings_tx,
            start_scroll_y: 0.0,
            start_content_h: 0.0,
            calc_scroll: 0.0,
            drag: None,
            start_scroll_vel: 0.0,
            ui_scale: 1.0,
            quit: false,
            anims: HashMap::new(),
            back_vis: Smooth::default(),
            hist_w: Smooth::default(),
            hist_scroll: 0.0,
            help_scroll: 0.0,
            set_scroll: 0.0,
            help_content_h: 0.0,
            set_content_h: 0.0,
            lw: 880.0,
            lh: 680.0,
            scale: 1.0,
            pointer: (0.0, 0.0),
            hover: None,
            pressed: None,
            focus: None,
            regions: Vec::new(),
            cursor_pointer: false,
            cursor_icon: CursorIcon::Default,
            shift: false,
            last_frame: Instant::now(),
            tf_display,
            tf_body,
            backend: None,
            surface_size: (0, 0),
            dirty: true,
            window: None,
        };
        if let Some(path) = &settings_path {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(st) = serde_json::from_str::<Settings>(&text) {
                    app.lang = Lang::from_code(&st.lang);
                    app.theme = Theme::from_code(&st.theme);
                    app.accent_idx = st.accent.min(theme::ACCENTS.len() - 1);
                    app.engine.set_history(st.history);
                }
            }
        }
        app
    }

    fn pal(&self) -> Palette {
        let target = theme::pal(self.theme, self.accent_idx);
        match self.palette_fade {
            Some((t0, from)) => {
                let t =
                    (Instant::now().duration_since(t0).as_secs_f32() / PALETTE_FADE_SECS).min(1.0);
                mix_palette(&from, &target, theme::ease_out(t))
            }
            None => target,
        }
    }

    /// Top bar height scales with the window.
    fn top_h(&self) -> f32 {
        58.0 * self.ui_scale
    }

    /// Font sizes / dimensions scale with the window (expand fullscreen, never shrink into a corner).
    fn fs(&self, v: f32) -> f32 {
        v * self.ui_scale
    }

    /// Visible height of the start-screen tile grid (below the title area).
    fn start_view_h(&self) -> f32 {
        (self.lh - self.top_h() - self.fs(150.0)).max(0.0)
    }

    fn save_settings(&self) {
        let Some(path) = &self.settings_path else { return };
        let st = Settings {
            lang: self.lang.code().to_owned(),
            theme: self.theme.code().to_owned(),
            accent: self.accent_idx,
            history: self.engine.history.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&st) {
            if let Some(tx) = &self.settings_tx {
                let _ = tx.send((path.clone(), json)); // writer thread persists off the UI thread
            }
        }
    }

    fn go_to(&mut self, now: Instant, page: Page) {
        if self.page == page && self.page_switch.is_none() {
            return;
        }
        self.focus = None; // clear keyboard focus on page switch
        self.page_switch = Some((now, self.page_current(), page));
        self.page_enter = now;
        self.dirty = true;
    }

    fn page_current(&self) -> Page {
        match self.page_switch {
            Some((_, _, to)) => to,
            None => self.page,
        }
    }

    fn cycle_lang(&mut self, now: Instant) {
        self.lang = match self.lang {
            Lang::ZhCn => Lang::En,
            Lang::En => Lang::Fr,
            Lang::Fr => Lang::ZhCn,
        };
        self.entrance_gen += 1;
        self.entrance_start = now;
        self.save_settings();
        self.dirty = true;
    }

    fn run_action(&mut self, a: Action, now: Instant) {
        match a {
            Action::Digit(d) => self.engine.digit(d),
            Action::Dot => self.engine.dot(),
            Action::Opset(o) => self.engine.set_op(o),
            Action::Equals => self.engine.equals(),
            Action::Clear => self.engine.c(),
            Action::ClearEntry => self.engine.ce(),
            Action::Back => self.engine.backspace(),
            Action::Percent => self.engine.percent(),
            Action::Negate => self.engine.negate(),
            Action::Recip => self.engine.unary(Unary::Recip),
            Action::Sqr => self.engine.unary(Unary::Sqr),
            Action::Sqrt => self.engine.unary(Unary::Sqrt),
        }

        let newv = self.engine.raw_value();
        if matches!(
            a,
            Action::Equals | Action::Recip | Action::Sqr | Action::Sqrt | Action::Percent
        ) {
            self.result_pop = Some(now);
            if let (Some(pv), Some(nv)) = (self.prev_value, newv) {
                if (pv - nv).abs() > f64::EPSILON {
                    self.roll = Some((pv, now));
                }
            }
        }
        if self.engine.error.is_some() && self.prev_error.is_none() {
            self.shake = Some(now); // visual-only shake feedback
        }
        self.prev_error = self.engine.error;
        self.prev_value = newv;
        if self.engine.take_history_dirty() {
            self.save_settings();
        }
        self.dirty = true;
    }

    fn activate(&mut self, act: HitAct, now: Instant) {
        if !self.splash_done && !matches!(act, HitAct::SkipSplash) {
            self.splash_done = true;
            self.dirty = true;
            return;
        }
        match act {
            HitAct::Calc(a) => self.run_action(a, now),
            HitAct::GoTo(p) => self.go_to(now, p),
            HitAct::ToggleHist => {
                self.history_open = !self.history_open;
                if self.history_open {
                    self.hist_open_at = Some(now); // items stagger slide-in from open time
                }
                self.dirty = true;
            }
            HitAct::OpenCalcHistory => {
                self.go_to(now, Page::Calc);
                self.history_open = true;
                self.hist_open_at = Some(now);
                self.dirty = true;
            }
            HitAct::CycleLang => self.cycle_lang(now),
            HitAct::FlipTheme => {
                let from = self.pal();
                self.theme = self.theme.flip();
                self.palette_fade = Some((now, from)); // cross-fade to the new palette
                self.save_settings();
                self.dirty = true;
            }
            HitAct::SetLang(l) => {
                if self.lang != l {
                    self.lang = l;
                    self.entrance_gen += 1;
                    self.entrance_start = now;
                    self.save_settings();
                    self.dirty = true;
                }
            }
            HitAct::SetTheme(t) => {
                if self.theme != t {
                    let from = self.pal();
                    self.theme = t;
                    self.palette_fade = Some((now, from)); // cross-fade to the new palette
                    self.save_settings();
                    self.dirty = true;
                }
            }
            HitAct::SetAccent(i) => {
                if self.accent_idx != i {
                    let from = self.pal();
                    self.accent_idx = i;
                    self.palette_fade = Some((now, from)); // cross-fade to the new accent
                    self.save_settings();
                    self.dirty = true;
                }
            }
            HitAct::LoadHist(id) => {
                if let Some(item) = self.engine.history.iter().find(|h| h.id == id) {
                    let raw = item.result.clone();
                    self.engine.load_result(&raw);
                    self.dirty = true;
                }
            }
            HitAct::ClearHist => {
                self.engine.clear_history();
                self.save_settings();
                self.dirty = true;
            }
            HitAct::SkipSplash => {
                self.splash_done = true;
                self.dirty = true;
            }
            HitAct::Exit => {
                self.quit = true;
                self.dirty = false;
            }
        }
    }

    fn handle_key(&mut self, k: &winit::event::KeyEvent) {
        let now = Instant::now();
        if !self.splash_done {
            self.splash_done = true;
            self.dirty = true;
            return;
        }
        let on_calc = self.page_current() == Page::Calc;
        // Keyboard focus navigation (accessibility): Tab/Shift+Tab cycle, arrow keys
        // move focus by grid, Space/Enter activate the focused key; focus ring always visible.
        let n_tiles = self.tile_specs().len();
        let move_focus = |focus: &mut Option<usize>, delta: isize, wrap: bool| {
            *focus = Some(match *focus {
                None => {
                    if delta < 0 {
                        n_tiles - 1
                    } else {
                        0
                    }
                }
                Some(i) => {
                    let next = i as isize + delta;
                    if wrap {
                        ((next + n_tiles as isize) % n_tiles as isize) as usize
                    } else {
                        next.clamp(0, n_tiles as isize - 1) as usize
                    }
                }
            });
        };
        match &k.logical_key {
            Key::Named(NamedKey::Tab) if on_calc => {
                let back = self.shift;
                move_focus(&mut self.focus, if back { -1 } else { 1 }, true);
                self.dirty = true;
            }
            Key::Named(NamedKey::ArrowRight) if on_calc => {
                move_focus(&mut self.focus, 1, false);
                self.dirty = true;
            }
            Key::Named(NamedKey::ArrowLeft) if on_calc => {
                move_focus(&mut self.focus, -1, false);
                self.dirty = true;
            }
            Key::Named(NamedKey::ArrowDown) if on_calc => {
                move_focus(&mut self.focus, 4, false);
                self.dirty = true;
            }
            Key::Named(NamedKey::ArrowUp) if on_calc => {
                move_focus(&mut self.focus, -4, false);
                self.dirty = true;
            }
            Key::Named(NamedKey::Space) if on_calc => {
                if let Some(i) = self.focus {
                    let act = self.tile_specs()[i].2;
                    self.run_action(act, now);
                }
            }
            Key::Named(NamedKey::Enter) if on_calc => {
                if let Some(i) = self.focus {
                    let act = self.tile_specs()[i].2;
                    self.run_action(act, now);
                } else {
                    self.run_action(Action::Equals, now);
                }
            }
            Key::Named(NamedKey::Backspace) if on_calc => self.run_action(Action::Back, now),
            Key::Named(NamedKey::Delete) if on_calc => self.run_action(Action::ClearEntry, now),
            Key::Named(NamedKey::Escape) => {
                if on_calc {
                    self.run_action(Action::Clear, now); // Esc = C
                } else if self.page != Page::Start || self.page_switch.is_some() {
                    self.go_to(now, Page::Start);
                }
            }
            Key::Named(NamedKey::Home) => self.go_to(now, Page::Start),
            Key::Named(NamedKey::F1) => self.go_to(now, Page::Help),
            Key::Character(s) => {
                for ch in s.chars() {
                    match ch {
                        '0'..='9' if on_calc => {
                            self.run_action(Action::Digit(ch as u8 - b'0'), now)
                        }
                        '.' | ',' if on_calc => self.run_action(Action::Dot, now),
                        '+' if on_calc => self.run_action(Action::Opset(Op::Add), now),
                        '-' if on_calc => self.run_action(Action::Opset(Op::Sub), now),
                        '*' | 'x' | '×' if on_calc => self.run_action(Action::Opset(Op::Mul), now),
                        '/' if on_calc => self.run_action(Action::Opset(Op::Div), now),
                        '%' if on_calc => self.run_action(Action::Percent, now),
                        '=' if on_calc => self.run_action(Action::Equals, now),
                        'h' | 'H' => {
                            self.history_open = !self.history_open;
                            if self.history_open {
                                self.hist_open_at = Some(now);
                            }
                            self.dirty = true;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn hit_test(&self, x: f32, y: f32) -> Option<&HitRegion> {
        self.regions.iter().rev().find(|r| rect_contains(r.rect, x, y))
    }

    fn anim_get(&self, key: HitKey) -> (f32, f32) {
        match self.anims.get(&key) {
            Some(a) => (a.hover.value, a.press.value),
            None => (0.0, 0.0),
        }
    }

    // ---------- fonts ----------

    fn font(&self, text: &str, size: f32) -> Font {
        let tf = if text.is_ascii() {
            self.tf_display.as_ref().or(self.tf_body.as_ref())
        } else {
            self.tf_body.as_ref().or(self.tf_display.as_ref())
        };
        match tf {
            Some(tf) => {
                let mut f = Font::from_typeface(tf.clone(), size);
                f.set_subpixel(true);
                f
            }
            None => {
                let mut f = Font::default();
                f.set_size(size);
                f
            }
        }
    }

    fn push_text(
        &self,
        fr: &mut Frame,
        text: &str,
        x: f32,
        y: f32,
        align: TextAlign,
        size: f32,
        color: skia_safe::Color,
    ) -> f32 {
        if text.is_empty() {
            return 0.0;
        }
        let font = self.font(text, size);
        let (w, _) = font.measure_str(text, None);
        let px = match align {
            TextAlign::Left => x,
            TextAlign::Right => x - w,
            TextAlign::Center => x - w * 0.5,
        };
        let (_, m) = font.metrics();
        let baseline = y - (m.ascent + m.descent) * 0.5;
        fr.cmds.push(Cmd::Text {
            pos: (px, baseline).into(),
            text: text.to_owned(),
            font,
            color,
        });
        w
    }

    fn wrap_text(&self, text: &str, size: f32, max_w: f32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            if para.is_empty() {
                out.push(String::new());
                continue;
            }
            let font = self.font(para, size);
            let spaced = para.is_ascii();
            let tokens: Vec<&str> = if spaced {
                para.split(' ').collect()
            } else {
                para.split("").filter(|s| !s.is_empty()).collect()
            };
            let mut line = String::new();
            for tok in tokens {
                let cand = if line.is_empty() {
                    tok.to_owned()
                } else if spaced {
                    format!("{line} {tok}")
                } else {
                    format!("{line}{tok}")
                };
                let (w, _) = font.measure_str(&cand, None);
                if w <= max_w || line.is_empty() {
                    line = cand;
                } else {
                    out.push(std::mem::take(&mut line));
                    line = tok.to_owned();
                }
            }
            out.push(line);
        }
        out
    }

    fn push_text_ml(
        &self,
        fr: &mut Frame,
        text: &str,
        x: f32,
        y: f32,
        align: TextAlign,
        size: f32,
        color: skia_safe::Color,
        max_w: f32,
    ) {
        let line_h = size * 1.5;
        for (i, line) in self.wrap_text(text, size, max_w).iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            self.push_text(fr, line, x, y + line_h * (i as f32 + 0.5), align, size, color);
        }
    }

    // ---------- animation tick ----------

    fn tick(&mut self, dt: f32, now: Instant) {
        let pressed_key = self.pressed;
        let hover = self.hover;
        for (k, a) in self.anims.iter_mut() {
            let ht = if hover == Some(*k) { 1.0 } else { 0.0 };
            let pt = if pressed_key == Some(*k) { 1.0 } else { 0.0 };
            a.hover.update(ht, dt, 12.0);
            a.press.update(pt, dt, 18.0);
        }
        let back_target = if self.page_current() != Page::Start { 1.0 } else { 0.0 };
        self.back_vis.update(back_target, dt, 14.0);
        let hist_target = if self.history_open { HIST_W } else { 0.0 };
        self.hist_w.update(hist_target, dt, 9.0);
        self.start_scroll_y = self
            .start_scroll_y
            .clamp(0.0, (self.start_content_h - self.start_view_h()).max(0.0));

        // Start-screen flick inertia: keep gliding after drag release, exponential decay
        if self.drag.is_none() {
            if self.start_scroll_vel.abs() > 20.0 {
                self.start_scroll_y += self.start_scroll_vel * dt;
                self.start_scroll_vel *= (-dt * 4.0).exp();
            } else {
                self.start_scroll_vel = 0.0;
            }
        }

        // clamp scrolls
        let view_h = (self.lh - self.top_h()).max(0.0);
        let hist_view = (self.lh - self.top_h() - self.fs(44.0)).max(0.0);
        let hist_total = self.engine.history.len() as f32 * 56.0;
        self.hist_scroll = self.hist_scroll.clamp(0.0, (hist_total - hist_view).max(0.0));
        self.help_scroll = self
            .help_scroll
            .clamp(0.0, (self.help_content_h - view_h).max(0.0));
        self.set_scroll = self
            .set_scroll
            .clamp(0.0, (self.set_content_h - view_h).max(0.0));
        // Calculator keypad is fixed at two columns: vertical scrolling only when 12 rows don't fit (defensive)
        let rows_n = 12.0f32;
        let grid_avail =
            (self.lh - self.top_h() - self.fs(8.0) - 120.0 - self.fs(12.0)).max(0.0);
        let grid_needed = rows_n * 40.0 + (rows_n - 1.0) * 3.0;
        self.calc_scroll = self
            .calc_scroll
            .clamp(0.0, (grid_needed - grid_avail).max(0.0));

        // one-shot cleanup
        if let Some((t0, _)) = self.palette_fade {
            if now.duration_since(t0).as_secs_f32() >= PALETTE_FADE_SECS {
                self.palette_fade = None;
            }
        }
        if let Some((_, t0)) = self.roll {
            if now.duration_since(t0).as_secs_f32() >= ROLL_SECS {
                self.roll = None;
            }
        }
        if let Some(t0) = self.result_pop {
            if now.duration_since(t0).as_secs_f32() >= POP_SECS * 1.4 {
                self.result_pop = None;
            }
        }
        if let Some(t0) = self.shake {
            if now.duration_since(t0).as_secs_f32() >= SHAKE_SECS {
                self.shake = None;
            }
        }
        self.ripples
            .retain(|r| now.duration_since(r.start).as_secs_f32() < RIPPLE_SECS);

        if let Some((start, _, to)) = self.page_switch {
            if now.duration_since(start).as_secs_f32() >= TRANSITION_SECS {
                self.page = to;
                self.page_switch = None;
            }
        }
        if !self.splash_done && now.duration_since(self.t0).as_secs_f32() >= SPLASH_SECS {
            self.splash_done = true;
        }
    }

    fn animating(&self, now: Instant) -> bool {
        if !self.splash_done {
            return true;
        }
        if self.page_switch.is_some() {
            return true;
        }
        if self.palette_fade.is_some() {
            return true; // theme / accent cross-fade
        }
        if self.start_scroll_vel.abs() > 20.0 {
            return true; // start-screen flick inertia
        }
        if self.history_open {
            if let Some(t0) = self.hist_open_at {
                if now.duration_since(t0).as_secs_f32() < 0.8 {
                    return true; // history items stagger slide-in
                }
            }
        }
        if self.page_current() == Page::Settings
            && now.duration_since(self.page_enter).as_secs_f32() < 1.0
        {
            return true; // settings sections stagger entrance
        }
        // Live tile flip on the Start screen (every 5s, flip lasts 0.6s)
        if self.page_current() == Page::Start {
            let phase = now.duration_since(self.t0).as_secs_f32() % 5.0;
            if phase < 0.75 {
                return true;
            }
        }
        // Progressive entrance (title → groups → tiles appear in batches)
        if now.duration_since(self.entrance_start) < Duration::from_secs_f32(1.8) {
            return true;
        }
        if !self.ripples.is_empty() {
            return true;
        }
        if self.roll.is_some() || self.result_pop.is_some() || self.shake.is_some() {
            return true;
        }
        let pressed_key = self.pressed;
        let hover = self.hover;
        for (k, a) in &self.anims {
            let ht = if hover == Some(*k) { 1.0 } else { 0.0 };
            let pt = if pressed_key == Some(*k) { 1.0 } else { 0.0 };
            if (a.hover.value - ht).abs() > 0.004 || (a.press.value - pt).abs() > 0.004 {
                return true;
            }
        }
        let hist_target = if self.history_open { HIST_W } else { 0.0 };
        if (self.hist_w.value - hist_target).abs() > 0.5 {
            return true;
        }
        let back_target = if self.page_current() != Page::Start { 1.0 } else { 0.0 };
        if (self.back_vis.value - back_target).abs() > 0.004 {
            return true;
        }
        false
    }

    // ---------- frame building ----------

    fn build_frame(&mut self, now: Instant) -> Frame {
        let dt = (now - self.last_frame).as_secs_f32().clamp(0.0, 0.1);
        self.last_frame = now;
        self.tick(dt, now);
        let mut fr = Frame::default();
        let pal = self.pal();
        let lw = self.lw;
        let lh = self.lh;

        // pages (clipped below top bar)
        let top = self.top_h();
        fr.push_clip(Rect::from_xywh(0.0, top, lw, (lh - top).max(0.0)));
        let sw = self.page_switch;
        match sw {
            None => self.build_page(&mut fr, &pal, self.page, lw, lh, now),
            Some((start, from, to)) => {
                let t = (now.duration_since(start).as_secs_f32() / TRANSITION_SECS).min(1.0);
                let e = theme::ease_out(t);
                let dir = if to.index() >= from.index() { 1.0 } else { -1.0 };
                fr.push_translate(-dir * e * lw * 0.5, 0.0);
                self.build_page(&mut fr, &pal, from, lw, lh, now);
                fr.pop();
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(0.0, top, lw, (lh - top).max(0.0)),
                    skc(pal.bg, (1.0 - e) * 0.9),
                ));
                fr.push_translate(dir * (1.0 - e) * lw * 0.5, 0.0);
                self.build_page(&mut fr, &pal, to, lw, lh, now);
                fr.pop();
            }
        }
        fr.pop();

        self.build_top_bar(&mut fr, &pal, lw, now);
        self.build_history(&mut fr, &pal, lw, lh, now);
        // Ripples drawn once, above every page and the top bar (each clipped to its button)
        self.build_ripples(&mut fr, now);
        self.build_splash(&mut fr, &pal, lw, lh, now);
        // Lock page hit regions during the slide transition:
        // both pages are moving at once, so clicks should only hit the top bar /
        // start overlay, avoiding invisible tiles
        if self.page_switch.is_some() {
            fr.regions
                .retain(|r| matches!(r.key, HitKey::Bar(_) | HitKey::SplashSkip));
        }
        fr.cursor_pointer = self.hover.is_some();
        fr
    }

    fn build_page(
        &mut self,
        fr: &mut Frame,
        pal: &Palette,
        page: Page,
        lw: f32,
        lh: f32,
        now: Instant,
    ) {
        match page {
            Page::Start => self.build_start(fr, pal, lw, lh, now),
            Page::Calc => self.build_calc(fr, pal, lw, lh, now),
            Page::Help => self.build_help(fr, pal, lw, lh),
            Page::Settings => self.build_settings(fr, pal, lw, lh, now),
        }
    }

    // ---------- Windows 8 start screen ----------

    fn build_start(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, lh: f32, now: Instant) {
        let s = i18n::strings(self.lang);
        let elapsed = now.duration_since(self.entrance_start).as_secs_f32();

        // Large flat title (framework appears first)
        let hp = theme::ease_out((elapsed - 0.55) / 0.40);
        if hp > 0.01 {
            let y = self.top_h() + self.fs(56.0) + (1.0 - hp) * self.fs(18.0);
            self.push_text(
                fr,
                s.app_name,
                self.fs(40.0),
                y,
                TextAlign::Left,
                self.fs(54.0),
                skc(theme::mix(pal.bg, pal.text, hp), hp),
            );
            fr.cmds.push(Cmd::FillRect(
                Rect::from_xywh(self.fs(42.0), y + self.fs(34.0), 170.0 * hp, self.fs(4.0)),
                skc(pal.accent, hp),
            ));
        }

        // Two-column tile grid (Windows Phone style): columns side by side, stacked
        // top to bottom, vertical scrolling; the calculator large tile fills a whole
        // row (spans both columns), other tiles pair up two per row
        let y0 = self.top_h() + self.fs(150.0);
        let pad = self.fs(40.0);
        let gap = self.fs(6.0);
        let content_w = (lw - pad * 2.0).max(self.fs(200.0));
        let uw = (content_w - gap) / 2.0; // single-column tile width
        let uh = uw.clamp(self.fs(70.0), self.fs(150.0)); // tile height

        enum Row {
            Header(&'static str),
            Tile(StartTileDef),
        }
        let rows: [Row; 12] = [
            Row::Header(s.g_apps),
            Row::Tile(StartTileDef { span: 2, glyph: Glyph::Calculator, label: s.t_calc, color: theme::ACCENTS[0], act: HitAct::GoTo(Page::Calc) }),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Clock, label: s.t_history, color: theme::ACCENTS[1], act: HitAct::OpenCalcHistory }),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Question, label: s.t_help, color: theme::ACCENTS[3], act: HitAct::GoTo(Page::Help) }),
            Row::Header(s.g_personal),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Moon, label: s.t_theme, color: theme::ACCENTS[7], act: HitAct::FlipTheme }),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::LangGlyph, label: s.t_lang, color: theme::ACCENTS[4], act: HitAct::CycleLang }),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Gear, label: s.t_accent, color: theme::ACCENTS[2], act: HitAct::GoTo(Page::Settings) }),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Gear, label: s.t_settings, color: theme::ACCENTS[5], act: HitAct::GoTo(Page::Settings) }),
            Row::Header(s.g_data),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Trash, label: s.t_clear, color: theme::ACCENTS[6], act: HitAct::ClearHist }),
            Row::Tile(StartTileDef { span: 1, glyph: Glyph::Power, label: s.t_exit, color: Color32::from_rgb(0x4A, 0x4A, 0x4A), act: HitAct::Exit }),
        ];

        let mut cy = y0; // unscrolled content cursor
        let mut col = 0usize; // 0 = left column, 1 = right column
        let mut idx = 0usize;
        let mut group_i = 0usize;
        for row in rows.iter() {
            match row {
                Row::Header(title) => {
                    let ghp = theme::ease_out(
                        (elapsed - (0.65 + group_i as f32 * 0.12)) / 0.30,
                    );
                    group_i += 1;
                    // Breathing room before group headers, avoiding flush/overlap with the tile row above
                    cy += self.fs(16.0);
                    let draw_y = cy - self.start_scroll_y;
                    if draw_y > -self.fs(30.0) && draw_y < lh && ghp > 0.01 {
                        self.push_text(
                            fr,
                            title,
                            pad,
                            draw_y,
                            TextAlign::Left,
                            self.fs(24.0),
                            skc(theme::mix(pal.bg, pal.accent, ghp), ghp),
                        );
                    }
                    cy += self.fs(44.0);
                    col = 0;
                }
                Row::Tile(def) => {
                    let span = def.span as usize;
                    if col + span > 2 {
                        cy += uh + gap;
                        col = 0;
                    }
                    // draw_y must be recomputed after advancing cy (group headers
                    // advance cy), otherwise tiles use a stale y value and cover the header
                    let draw_y = cy - self.start_scroll_y;
                    let tw = if span == 2 { content_w } else { uw };
                    let x = pad + col as f32 * (uw + gap);
                    let rect = Rect::from_xywh(x, draw_y, tw, uh);
                    if draw_y > -uh - self.fs(20.0) && draw_y < lh + self.fs(20.0) {
                        self.start_tile(fr, pal, idx, rect, def, now, elapsed);
                    }
                    col += span;
                    if col >= 2 {
                        cy += uh + gap;
                        col = 0;
                    }
                    idx += 1;
                }
            }
        }
        self.start_content_h = cy - y0 + self.fs(20.0);
        // Vertical panning via wheel and blank-area drag gesture (Metro convention)
    }

    fn start_tile(
        &mut self,
        fr: &mut Frame,
        pal: &Palette,
        idx: usize,
        rect: Rect,
        def: &StartTileDef,
        now: Instant,
        elapsed: f32,
    ) {
        let key = HitKey::StartTile(self.entrance_gen, idx);
        let (hov, press) = self.anim_get(key);
        let delay = 0.75 + idx as f32 * 0.07;
        let pin = theme::ease_out((elapsed - delay) / 0.32);
        if pin <= 0.01 {
            return; // staggered entrance: this tile's turn hasn't come yet
        }
        let rect = Rect::from_xywh(
            rect.left,
            rect.top + (1.0 - pin) * 24.0,
            rect.width(),
            rect.height(),
        );

        let scale = 1.0 - 0.05 * press;
        let (mut tx, mut ty2) = (0.0f32, 0.0f32);
        if press > 0.01 {
            let (cx, cy) = rect_center(rect);
            let dx = (self.pointer.0 - cx) * 0.08 * press;
            let dy = (self.pointer.1 - cy) * 0.08 * press;
            let m = (dx * dx + dy * dy).sqrt();
            if m > 5.0 {
                tx = dx * 5.0 / m;
                ty2 = dy * 5.0 / m;
            } else {
                tx = dx;
                ty2 = dy;
            }
        }

        let col = theme::mix(theme::mix(pal.bg, def.color, pin), Color32::WHITE, 0.15 * hov);
        let (cx, cy) = rect_center(rect);
        let w = rect.width() * scale;
        let h = rect.height() * scale;
        fr.cmds.push(Cmd::FillRect(
            Rect::from_xywh(cx + tx - w * 0.5, cy + ty2 - h * 0.5, w, h),
            skc(col, 1.0),
        ));

        let big = def.span == 2;
        let label_size = self.fs(if big { 26.0 } else { 19.0 });
        let label_y = rect.bottom - if big { 16.0 } else { 11.0 };

        // Calculator tile: live flip (every 5s, flip lasts 0.6s)
        // Lightweight 2D plane flip via Skia transform matrix: X-axis scaling
        // around the tile center
        if idx == 0 {
            let phase = now.duration_since(self.t0).as_secs_f32() % 5.0;
            let (sx, front) = if phase < 0.6 {
                let ft = phase / 0.6;
                ((ft * std::f32::consts::PI).cos().abs(), ft < 0.5)
            } else {
                (1.0, true)
            };
            fr.push_scale((cx + tx, cy + ty2), sx.max(0.02), 1.0);
            if front {
                let gsize = if big { u_size(rect) * 0.5 } else { u_size(rect) * 0.4 };
                self.draw_glyph(
                    fr,
                    def.glyph,
                    0.0,
                    -u_size(rect) * 0.10,
                    gsize,
                    skc(Color32::WHITE, pin),
                );
            } else {
                let s = i18n::strings(self.lang);
                self.push_text(
                    fr,
                    s.tagline,
                    0.0,
                    -u_size(rect) * 0.06,
                    TextAlign::Center,
                    (u_size(rect) * 0.16).max(12.0),
                    skc(Color32::WHITE, pin),
                );
            }
            // Label is also inside the transform (local coords = absolute coords - tile center)
            self.push_text(
                fr,
                def.label,
                rect.left + 14.0 - (cx + tx),
                label_y + ty2 - (cy + ty2),
                TextAlign::Left,
                label_size,
                skc(Color32::WHITE, pin),
            );
            fr.pop();
        } else {
            let gsize = if big { u_size(rect) * 0.5 } else { u_size(rect) * 0.42 };
            self.draw_glyph(
                fr,
                def.glyph,
                cx + tx,
                cy + ty2 - u_size(rect) * 0.08,
                gsize,
                skc(Color32::WHITE, pin),
            );
            self.push_text(
                fr,
                def.label,
                rect.left + 10.0 + tx,
                label_y + ty2,
                TextAlign::Left,
                label_size,
                skc(Color32::WHITE, pin),
            );
        }

        if pin > 0.5 {
            fr.regions.push(HitRegion {
                key,
                rect,
                act: def.act,
            });
        }
    }

    fn draw_glyph(
        &self,
        fr: &mut Frame,
        g: Glyph,
        cx: f32,
        cy: f32,
        s: f32,
        color: skia_safe::Color,
    ) {
        match g {
            Glyph::Calculator => {
                let w = s * 0.64;
                let h = s * 0.80;
                fr.cmds.push(Cmd::StrokeRect(
                    Rect::from_xywh(cx - w * 0.5, cy - h * 0.5, w, h),
                    color,
                    s * 0.055,
                ));
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(cx - w * 0.32, cy - h * 0.34, w * 0.64, h * 0.15),
                    color,
                ));
                for r in 0..2 {
                    for c in 0..3 {
                        fr.cmds.push(Cmd::FillCircle(
                            (
                                cx - w * 0.28 + c as f32 * w * 0.28,
                                cy + h * 0.06 + r as f32 * h * 0.20,
                            )
                                .into(),
                            s * 0.05,
                            color,
                        ));
                    }
                }
            }
            Glyph::Clock => {
                fr.cmds.push(Cmd::StrokeCircle((cx, cy).into(), s * 0.30, color, s * 0.055));
                fr.cmds.push(Cmd::Line((cx, cy).into(), (cx, cy - s * 0.17).into(), color, s * 0.05));
                fr.cmds.push(Cmd::Line((cx, cy).into(), (cx + s * 0.13, cy + s * 0.06).into(), color, s * 0.05));
            }
            Glyph::Question => {
                self.push_text(fr, "?", cx, cy, TextAlign::Center, s * 0.62, color);
            }
            Glyph::Gear => {
                fr.cmds.push(Cmd::StrokeCircle((cx, cy).into(), s * 0.20, color, s * 0.05));
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0 + 0.39;
                    let (dx, dy) = (a.cos(), a.sin());
                    fr.cmds.push(Cmd::Line(
                        (cx + dx * s * 0.25, cy + dy * s * 0.25).into(),
                        (cx + dx * s * 0.34, cy + dy * s * 0.34).into(),
                        color,
                        s * 0.05,
                    ));
                }
                fr.cmds.push(Cmd::FillCircle((cx, cy).into(), s * 0.06, color));
            }
            Glyph::Moon => {
                fr.cmds.push(Cmd::FillCircle((cx, cy).into(), s * 0.26, color));
                fr.push_clip(Rect::from_xywh(cx, cy - s * 0.28, s * 0.30, s * 0.56));
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(cx, cy - s * 0.28, s * 0.30, s * 0.56),
                    skc(Color32::BLACK, 0.0),
                ));
                fr.pop();
            }
            Glyph::LangGlyph => {
                self.push_text(fr, "文", cx - s * 0.10, cy, TextAlign::Center, s * 0.30, color);
                self.push_text(fr, "A", cx + s * 0.17, cy + s * 0.04, TextAlign::Center, s * 0.24, color);
            }
            Glyph::Trash => {
                let w = s * 0.44;
                fr.cmds.push(Cmd::Line(
                    (cx - w, cy - s * 0.24).into(),
                    (cx + w, cy - s * 0.24).into(),
                    color,
                    s * 0.05,
                ));
                fr.cmds.push(Cmd::Line(
                    (cx - w * 0.8, cy - s * 0.18).into(),
                    (cx - w * 0.6, cy + s * 0.26).into(),
                    color,
                    s * 0.05,
                ));
                fr.cmds.push(Cmd::Line(
                    (cx + w * 0.8, cy - s * 0.18).into(),
                    (cx + w * 0.6, cy + s * 0.26).into(),
                    color,
                    s * 0.05,
                ));
                fr.cmds.push(Cmd::Line(
                    (cx - w * 0.6, cy + s * 0.26).into(),
                    (cx + w * 0.6, cy + s * 0.26).into(),
                    color,
                    s * 0.05,
                ));
                fr.cmds.push(Cmd::Line(
                    (cx, cy - s * 0.30).into(),
                    (cx, cy - s * 0.24).into(),
                    color,
                    s * 0.05,
                ));
            }
            Glyph::Power => {
                fr.cmds.push(Cmd::StrokeCircle((cx, cy + s * 0.04).into(), s * 0.22, color, s * 0.055));
                fr.cmds.push(Cmd::Line(
                    (cx, cy - s * 0.26).into(),
                    (cx, cy - s * 0.02).into(),
                    color,
                    s * 0.06,
                ));
            }
        }
    }

    // ---------- calculator page ----------

    fn tile_specs(&self) -> Vec<(TileLabel, TileKind, Action)> {
        let dec = i18n::dec_sep(self.lang);
        vec![
            (TileLabel::Text("%"), TileKind::Util, Action::Percent),
            (TileLabel::Text("CE"), TileKind::Util, Action::ClearEntry),
            (TileLabel::Text("C"), TileKind::Util, Action::Clear),
            (TileLabel::BackIcon, TileKind::Util, Action::Back),
            (TileLabel::Text("1/x"), TileKind::Util, Action::Recip),
            (TileLabel::Text("x²"), TileKind::Util, Action::Sqr),
            (TileLabel::Text("√"), TileKind::Util, Action::Sqrt),
            (TileLabel::Text("÷"), TileKind::Op, Action::Opset(Op::Div)),
            (TileLabel::Text("7"), TileKind::Digit, Action::Digit(7)),
            (TileLabel::Text("8"), TileKind::Digit, Action::Digit(8)),
            (TileLabel::Text("9"), TileKind::Digit, Action::Digit(9)),
            (TileLabel::Text("×"), TileKind::Op, Action::Opset(Op::Mul)),
            (TileLabel::Text("4"), TileKind::Digit, Action::Digit(4)),
            (TileLabel::Text("5"), TileKind::Digit, Action::Digit(5)),
            (TileLabel::Text("6"), TileKind::Digit, Action::Digit(6)),
            (TileLabel::Text("−"), TileKind::Op, Action::Opset(Op::Sub)),
            (TileLabel::Text("1"), TileKind::Digit, Action::Digit(1)),
            (TileLabel::Text("2"), TileKind::Digit, Action::Digit(2)),
            (TileLabel::Text("3"), TileKind::Digit, Action::Digit(3)),
            (TileLabel::Text("+"), TileKind::Op, Action::Opset(Op::Add)),
            (TileLabel::Text("±"), TileKind::Util, Action::Negate),
            (TileLabel::Text("0"), TileKind::Digit, Action::Digit(0)),
            (TileLabel::Text(dec), TileKind::Digit, Action::Dot),
            (TileLabel::Text("="), TileKind::Equals, Action::Equals),
        ]
    }

    fn build_calc(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, lh: f32, now: Instant) {
        let s = self.ui_scale;
        // Content reflows live while the history panel is open (not just covered):
        // available width = window width - panel width (panel width itself is
        // animated, so reflow is smooth)
        let content_w = (lw - self.hist_w.value).max(200.0);
        let disp_h = (((lh - self.top_h()) * 0.28).clamp(120.0, 200.0 * s)).max(100.0);
        let disp = Rect::from_xywh(16.0, self.top_h() + 8.0, content_w - 32.0, disp_h);
        self.build_display(fr, pal, disp, now);

        // Keypad fixed at two columns (2 cols × 12 rows), 4px gap;
        // when rows exceed the visible height, pan with real wheel/drag —
        // tiles are never clipped or overlapped
        let gap = 4.0f32;
        let n = 24.0f32;
        let cols = 2.0f32;
        let rows_n = (n / cols).ceil();
        let min_th = 44.0;
        let grid_top = disp.bottom + 16.0;
        let avail = (lh - grid_top).max(0.0);
        let needed = rows_n * min_th + (rows_n - 1.0) * gap;
        let th = if avail >= needed {
            (avail - gap * (rows_n - 1.0)) / rows_n
        } else {
            min_th
        };
        let total_h = th * rows_n + gap * (rows_n - 1.0);
        let y0 = if avail >= total_h {
            lh - total_h
        } else {
            grid_top - self.calc_scroll
        };

        let tw = (content_w - gap * (cols - 1.0)) / cols;
        let specs = self.tile_specs();
        for idx in 0..specs.len() {
            let col = (idx as f32 % cols) * (tw + gap);
            let row = (idx as f32 / cols).floor();
            let rect = Rect::from_xywh(col, y0 + row * (th + gap), tw, th);
            let (label, kind, action) = specs[idx];
            self.build_tile(fr, pal, idx, rect, label, kind, action, now);
        }
    }

    fn build_tile(
        &mut self,
        fr: &mut Frame,
        pal: &Palette,
        idx: usize,
        rect: Rect,
        label: TileLabel,
        kind: TileKind,
        action: Action,
        now: Instant,
    ) {
        let key = HitKey::Tile(self.entrance_gen, idx);
        let (hov, press) = self.anim_get(key);
        let delay = (idx as f32) * 0.028;
        let pin = theme::ease_out(((now - self.entrance_start).as_secs_f32() - delay) / 0.30);
        let off_y = (1.0 - pin) * 16.0;
        let rect = Rect::from_xywh(rect.left, rect.top + off_y, rect.width(), rect.height());

        // Win8-style tile "sink tilt": shrink + tilt toward the press point
        let scale = 1.0 - 0.06 * press;
        let (mut tx, mut ty) = (0.0f32, 0.0f32);
        if press > 0.01 {
            let (cx, cy) = rect_center(rect);
            let dx = (self.pointer.0 - cx) * 0.10 * press;
            let dy = (self.pointer.1 - cy) * 0.10 * press;
            let m = (dx * dx + dy * dy).sqrt();
            if m > 6.0 {
                let f = 6.0 / m;
                tx = dx * f;
                ty = dy * f;
            } else {
                tx = dx;
                ty = dy;
            }
        }

        let base = kind.base(pal);
        let mut col = theme::mix(base, Color32::WHITE, 0.18 * hov);
        col = theme::mix(pal.bg, col, pin);
        let (cx, cy) = rect_center(rect);
        let dcx = cx + tx;
        let dcy = cy + ty;
        let w = rect.width() * scale;
        let h = rect.height() * scale;
        let draw_rect = Rect::from_xywh(dcx - w * 0.5, dcy - h * 0.5, w, h);
        fr.cmds.push(Cmd::FillRect(draw_rect, skc(col, 1.0)));

        let tcol = kind.text(pal);
        let tsize = (rect.height() * 0.52).clamp(22.0, 46.0);
        let px = dcx + tx * 0.5;
        let py = dcy + ty * 0.5;
        match label {
            TileLabel::Text(t) => {
                self.push_text(fr, t, px, py, TextAlign::Center, tsize, skc(tcol, 1.0));
            }
            TileLabel::BackIcon => {
                let mut pb = PathBuilder::new();
                pb.move_to((px - tsize * 0.62, py));
                pb.line_to((px + tsize * 0.55, py - tsize * 0.45));
                pb.line_to((px + tsize * 0.55, py + tsize * 0.45));
                pb.close();
                fr.cmds.push(Cmd::FillPath(pb.detach(), skc(tcol, 1.0)));
                let cxx = px + tsize * 0.18;
                let s = tsize * 0.16;
                fr.cmds.push(Cmd::Line(
                    (cxx - s, py - s).into(),
                    (cxx + s, py + s).into(),
                    skc(col, 1.0),
                    2.0,
                ));
                fr.cmds.push(Cmd::Line(
                    (cxx - s, py + s).into(),
                    (cxx + s, py - s).into(),
                    skc(col, 1.0),
                    2.0,
                ));
            }
        }

        // Keyboard focus ring (accessibility): white outline always clearly visible
        if self.focus == Some(idx) {
            fr.cmds.push(Cmd::StrokeRect(
                Rect::from_xywh(
                    draw_rect.left + 2.0,
                    draw_rect.top + 2.0,
                    (draw_rect.width() - 4.0).max(1.0),
                    (draw_rect.height() - 4.0).max(1.0),
                ),
                skc(Color32::WHITE, 1.0),
                2.0,
            ));
        }

        fr.regions.push(HitRegion {
            key,
            rect,
            act: HitAct::Calc(action),
        });
    }

    fn build_ripples(&mut self, fr: &mut Frame, now: Instant) {
        for r in &self.ripples {
            let t = now.duration_since(r.start).as_secs_f32() / RIPPLE_SECS;
            fr.push_clip(r.rect);
            let base = if r.dark { Color32::BLACK } else { Color32::WHITE };
            let dx = r.rect.width();
            let dy = r.rect.height();
            let len = (dx * dx + dy * dy).sqrt();
            fr.cmds.push(Cmd::FillCircle(
                (r.origin.0, r.origin.1).into(),
                4.0 + t * len * 0.6,
                skc(base, (1.0 - t) * 0.35),
            ));
            fr.pop();
        }
    }

    fn build_display(&mut self, fr: &mut Frame, pal: &Palette, rect: Rect, now: Instant) {
        let s = i18n::strings(self.lang);

        if let Some(expr) = self.expr_text() {
            self.push_text(
                fr,
                &expr,
                rect.right,
                rect.top + self.fs(14.0),
                TextAlign::Right,
                self.fs(16.0),
                skc(pal.weak, 1.0),
            );
        }

        let mut shake_x = 0.0;
        if let Some(t0) = self.shake {
            let t = now.duration_since(t0).as_secs_f32() / SHAKE_SECS;
            if t < 1.0 {
                shake_x = (t * 12.0).sin() * (1.0 - t) * 7.0;
            }
        }

        let pop_t = self
            .result_pop
            .map(|t0| now.duration_since(t0).as_secs_f32() / POP_SECS);

        let (text, is_err) = match self.engine.display() {
            crate::engine::Disp::Error(k) => {
                let t = match k {
                    ErrKey::DivZero => s.err_div0,
                    ErrKey::Invalid => s.err_invalid,
                    ErrKey::Overflow => s.err_overflow,
                };
                (t.to_owned(), true)
            }
            crate::engine::Disp::Value(v) => {
                let v = self.roll_value(v, now);
                (i18n::fmt_value(v, self.lang), false)
            }
            crate::engine::Disp::Entry => (
                i18n::fmt_entry(self.engine.entry_str(), self.lang),
                false,
            ),
        };

        let n = text.chars().count();
        let mut size = self.ui_scale
            * if is_err {
                42.0
            } else if n <= 9 {
                78.0
            } else if n <= 13 {
                58.0
            } else if n <= 18 {
                46.0
            } else {
                38.0
            };
        if let Some(t) = pop_t {
            if t < 1.0 {
                size *= 1.0 + 0.12 * (1.0 - t) * (1.0 - t);
            }
        }

        self.push_text(
            fr,
            &text,
            rect.right + shake_x,
            rect.bottom - 18.0,
            TextAlign::Right,
            size,
            skc(if is_err { pal.error } else { pal.text }, 1.0),
        );

        // Accent underline sweep when a result appears
        if let Some(t) = pop_t {
            let w = rect.width() * theme::ease_out((t / 1.4).min(1.0));
            if t < 1.4 && w > 1.0 {
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(rect.right - w, rect.bottom - 2.0, w, 3.0),
                    skc(pal.accent, 1.0),
                ));
            }
        }
    }

    fn roll_value(&self, target: f64, now: Instant) -> f64 {
        if let Some((from, t0)) = self.roll {
            let t = (now.duration_since(t0).as_secs_f32() / ROLL_SECS).min(1.0);
            let e = theme::ease_out(t);
            from + (target - from) * e as f64
        } else {
            target
        }
    }

    fn expr_text(&self) -> Option<String> {
        if self.engine.error.is_some() {
            return None;
        }
        if let Some(le) = &self.engine.last_expr {
            return Some(match le {
                LastExpr::Binary(a, o, b) => format!(
                    "{} {} {} =",
                    i18n::fmt_value(*a, self.lang),
                    o.sym(),
                    i18n::fmt_value(*b, self.lang)
                ),
                LastExpr::UnarySqr(x) => format!("{}² =", i18n::fmt_value(*x, self.lang)),
                LastExpr::UnarySqrt(x) => format!("√({}) =", i18n::fmt_value(*x, self.lang)),
                LastExpr::UnaryRecip(x) => format!("1/({}) =", i18n::fmt_value(*x, self.lang)),
            });
        }
        if let Some((a, o)) = self.engine.pending() {
            return Some(format!("{} {}", i18n::fmt_value(a, self.lang), o.sym()));
        }
        None
    }

    // ---------- top bar ----------

    /// Shared top-bar button part: light hover highlight (Win8 Metro convention).
    fn bar_hover(&self, fr: &mut Frame, key: HitKey, rect: Rect, bgc: Color32, tc: Color32) {
        let (hov, _) = self.anim_get(key);
        if hov > 0.01 {
            fr.cmds.push(Cmd::FillRect(
                rect,
                skc(theme::mix(bgc, tc, 0.10 * hov), 1.0),
            ));
        }
    }

    fn build_top_bar(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, now: Instant) {
        let s = i18n::strings(self.lang);
        let k = self.ui_scale;
        let top = self.top_h();
        fr.cmds.push(Cmd::FillRect(
            Rect::from_xywh(0.0, 0.0, lw, top),
            skc(pal.bg, 1.0),
        ));

        // Back button (animates in/out), always returns to the start screen
        let ap = self.back_vis.value;
        if ap > 0.02 {
            let (hov, press) = self.anim_get(HitKey::Bar(BarBtn::Back));
            if hov > 0.01 {
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(k * 8.0, k * 10.0, k * 38.0, k * 38.0),
                    skc(theme::mix(pal.bg, pal.text, 0.10 * hov), 1.0),
                ));
            }
            let size = k * 38.0 * (0.5 + 0.5 * ap);
            let kk = (1.0 - 0.06 * press) * (0.5 + 0.5 * ap);
            let (cx, cy) = (k * 27.0, k * 29.0);
            let stroke = skc(pal.text, 1.0);
            let a = 6.0 * size / (k * 38.0) * kk;
            let b = 5.0 * size / (k * 38.0) * kk;
            fr.cmds.push(Cmd::Line((cx + a, cy).into(), (cx - a, cy).into(), stroke, 2.2 * k));
            fr.cmds.push(Cmd::Line((cx - a, cy).into(), (cx - a + b, cy - b).into(), stroke, 2.2 * k));
            fr.cmds.push(Cmd::Line((cx - a, cy).into(), (cx - a + b, cy + b).into(), stroke, 2.2 * k));
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::Back),
                rect: Rect::from_xywh(k * 8.0, k * 10.0, k * 38.0, k * 38.0),
                act: HitAct::GoTo(Page::Start),
            });
        }

        let title_x = k * 14.0 + k * 44.0 * ap;
        let title_of = |page: Page| match page {
            Page::Start => s.app_name,
            Page::Calc => s.nav_calc,
            Page::Help => s.nav_help,
            Page::Settings => s.nav_settings,
        };
        // Title crossfade during page switches: old slides out left, new slides in from the right
        if let Some((t0, from, to)) = self.page_switch {
            let t = (now.duration_since(t0).as_secs_f32() / TRANSITION_SECS).min(1.0);
            let e = theme::ease_out(t);
            let slide = k * 26.0;
            self.push_text(
                fr,
                title_of(from),
                title_x - e * slide,
                k * 29.0,
                TextAlign::Left,
                k * 36.0,
                skc(theme::mix(pal.bg, pal.text, 1.0 - e), 1.0),
            );
            let tw = self.push_text(
                fr,
                title_of(to),
                title_x + (1.0 - e) * slide,
                k * 29.0,
                TextAlign::Left,
                k * 36.0,
                skc(theme::mix(pal.bg, pal.text, e), 1.0),
            );
            self.push_text(
                fr,
                s.tagline,
                title_x + (1.0 - e) * slide + tw + k * 12.0,
                k * 29.0,
                TextAlign::Left,
                k * 15.0,
                skc(theme::mix(pal.bg, pal.weak, e), 1.0),
            );
        } else {
            let title = title_of(self.page_current());
            let tw = self.push_text(fr, title, title_x, k * 29.0, TextAlign::Left, k * 36.0, skc(pal.text, 1.0));
            self.push_text(
                fr,
                s.tagline,
                title_x + tw + k * 12.0,
                k * 29.0,
                TextAlign::Left,
                k * 15.0,
                skc(pal.weak, 1.0),
            );
        }

        // Right-side buttons (left→right: home, history, help, settings, theme, lang)
        let (tc, bgc) = (pal.text, pal.bg);
        let bar_y = k * 10.0;
        let icon = k * 38.0;
        let pitch = k * 40.0;
        let home_x = lw - k * 66.0 - 5.0 * pitch;
        let history_x = lw - k * 66.0 - 4.0 * pitch;
        let help_x = lw - k * 66.0 - 3.0 * pitch;
        let settings_x = lw - k * 66.0 - 2.0 * pitch;
        let theme_x = lw - k * 66.0 - pitch;
        let lang_x = lw - k * 66.0;

        // home (four-color squares, Win8-logo style)
        {
            let rect = Rect::from_xywh(home_x, bar_y, icon, icon);
            self.bar_hover(fr, HitKey::Bar(BarBtn::Home), rect, bgc, tc);
            let c = rect_center(rect);
            let q = k * 5.5;
            let g = k * 2.5;
            let cols = [theme::ACCENTS[0], theme::ACCENTS[1], theme::ACCENTS[2], theme::ACCENTS[5]];
            for i in 0..4 {
                let dx = if i % 2 == 0 { -(q + g) / 2.0 } else { g / 2.0 };
                let dy = if i < 2 { -(q + g) / 2.0 } else { g / 2.0 };
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(c.0 + dx, c.1 + dy, q, q),
                    skc(cols[i], 1.0),
                ));
            }
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::Home),
                rect,
                act: HitAct::GoTo(Page::Start),
            });
        }
        // history clock
        {
            let rect = Rect::from_xywh(history_x, bar_y, icon, icon);
            self.bar_hover(fr, HitKey::Bar(BarBtn::History), rect, bgc, tc);
            let c: Point = rect_center(rect).into();
            fr.cmds.push(Cmd::StrokeCircle(c, k * 8.5, skc(tc, 1.0), k * 2.0));
            fr.cmds.push(Cmd::Line(c, (c.x, c.y - k * 5.0).into(), skc(tc, 1.0), k * 2.0));
            fr.cmds.push(Cmd::Line(c, (c.x + k * 4.0, c.y + k * 2.0).into(), skc(tc, 1.0), k * 2.0));
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::History),
                rect,
                act: HitAct::ToggleHist,
            });
        }
        // help "?"
        {
            let rect = Rect::from_xywh(help_x, bar_y, icon, icon);
            self.bar_hover(fr, HitKey::Bar(BarBtn::Help), rect, bgc, tc);
            let c = rect_center(rect);
            self.push_text(fr, "?", c.0, c.1, TextAlign::Center, k * 20.0, skc(tc, 1.0));
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::Help),
                rect,
                act: HitAct::GoTo(Page::Help),
            });
        }
        // settings gear
        {
            let rect = Rect::from_xywh(settings_x, bar_y, icon, icon);
            self.bar_hover(fr, HitKey::Bar(BarBtn::Settings), rect, bgc, tc);
            let c: Point = rect_center(rect).into();
            fr.cmds.push(Cmd::StrokeCircle(c, k * 6.5, skc(tc, 1.0), k * 2.0));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0 + 0.39;
                let (dx, dy) = (a.cos(), a.sin());
                fr.cmds.push(Cmd::Line(
                    (c.x + dx * k * 8.5, c.y + dy * k * 8.5).into(),
                    (c.x + dx * k * 11.5, c.y + dy * k * 11.5).into(),
                    skc(tc, 1.0),
                    k * 2.0,
                ));
            }
            fr.cmds.push(Cmd::FillCircle(c, k * 2.0, skc(tc, 1.0)));
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::Settings),
                rect,
                act: HitAct::GoTo(Page::Settings),
            });
        }
        // theme half-moon
        {
            let rect = Rect::from_xywh(theme_x, bar_y, icon, icon);
            self.bar_hover(fr, HitKey::Bar(BarBtn::Theme), rect, bgc, tc);
            let c: Point = rect_center(rect).into();
            fr.cmds.push(Cmd::FillCircle(c, k * 9.0, skc(tc, 1.0)));
            fr.push_clip(Rect::from_xywh(c.x, c.y - k * 10.0, k * 11.0, k * 20.0));
            fr.cmds.push(Cmd::FillRect(
                Rect::from_xywh(c.x, c.y - k * 10.0, k * 11.0, k * 20.0),
                skc(bgc, 1.0),
            ));
            fr.pop();
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::Theme),
                rect,
                act: HitAct::FlipTheme,
            });
        }
        // language
        {
            let rect = Rect::from_xywh(lang_x, bar_y, icon, icon);
            self.bar_hover(fr, HitKey::Bar(BarBtn::Lang), rect, bgc, tc);
            let c = rect_center(rect);
            self.push_text(fr, self.lang.short(), c.0, c.1, TextAlign::Center, k * 15.0, skc(tc, 1.0));
            fr.regions.push(HitRegion {
                key: HitKey::Bar(BarBtn::Lang),
                rect,
                act: HitAct::CycleLang,
            });
        }
    }

    // ---------- history panel ----------

    fn build_history(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, lh: f32, now: Instant) {
        let w = self.hist_w.value;
        if w <= 4.0 {
            return;
        }
        let panel = Rect::from_xywh(lw - w, self.top_h(), w, (lh - self.top_h()).max(0.0));
        fr.push_clip(panel);
        // Opaque background + Metro accent edge strip: fully separated from main content
        fr.cmds.push(Cmd::FillRect(panel, skc(pal.panel, 1.0)));
        fr.cmds.push(Cmd::FillRect(
            Rect::from_xywh(panel.left, panel.top, 3.0, panel.height()),
            skc(pal.accent, 1.0),
        ));
        fr.push_translate(w - HIST_W, 0.0);

        let s = i18n::strings(self.lang);
        let base_x = lw - HIST_W + 12.0;
        self.push_text(fr, s.history, base_x, self.top_h() + 22.0, TextAlign::Left, self.fs(26.0), skc(pal.text, 1.0));
        {
            let font = self.font(s.clear, 16.0);
            let (tw, _) = font.measure_str(s.clear, None);
            let rect = Rect::from_xywh(lw - 24.0 - (tw + 22.0), self.top_h() + 12.0, tw + 22.0, 28.0);
            let (hov, _) = self.anim_get(HitKey::HistClear);
            if hov > 0.01 {
                fr.cmds.push(Cmd::FillRect(
                    rect,
                    skc(theme::mix(pal.panel, pal.text, 0.10 * hov), 1.0),
                ));
            }
            self.push_text(
                fr,
                s.clear,
                rect.center_x(),
                rect.center_y(),
                TextAlign::Center,
                14.0,
                skc(theme::mix(pal.panel, pal.text, 0.55 + 0.45 * hov), 1.0),
            );
            fr.regions.push(HitRegion {
                key: HitKey::HistClear,
                rect,
                act: HitAct::ClearHist,
            });
        }

        let items: Vec<(u64, String, String, Instant)> = self
            .engine
            .history
            .iter()
            .map(|h| {
                (
                    h.id,
                    hist_expr_text(&h.kind, self.lang),
                    h.result.clone(),
                    h.at,
                )
            })
            .collect();

        if items.is_empty() {
            self.push_text_ml(
                fr,
                s.history_empty,
                base_x,
                self.top_h() + 80.0,
                TextAlign::Left,
                14.0,
                skc(pal.weak, 1.0),
                HIST_W - 24.0,
            );
        } else {
            for (i, (id, expr, raw, at)) in items.into_iter().enumerate() {
                let y = self.top_h() + 44.0 + i as f32 * 56.0 - self.hist_scroll;
                if y > lh + 56.0 || y + 56.0 < self.top_h() {
                    continue; // skip items outside the panel's visible range
                }
                let key = HitKey::HistItem(id);
                let (hov, _) = self.anim_get(key);
                // stagger slide-in from the right when the panel opens
                let slide = self
                    .hist_open_at
                    .map(|t0| {
                        theme::ease_out(
                            (now.duration_since(t0).as_secs_f32() - i as f32 * 0.03) / 0.28,
                        )
                        .clamp(0.0, 1.0)
                    })
                    .unwrap_or(1.0);
                let dx = (1.0 - slide) * self.fs(36.0);
                let rect = Rect::from_xywh(base_x - 12.0 + dx, y, HIST_W - 24.0, 54.0);
                let pin = theme::ease_out(at.elapsed().as_secs_f32() / 0.25).max(hov) * slide;
                if hov > 0.01 {
                    fr.cmds.push(Cmd::FillRect(
                        rect,
                        skc(theme::mix(pal.panel, pal.text, 0.08 * hov), 1.0),
                    ));
                }
                self.push_text(
                    fr,
                    &expr,
                    rect.right - 10.0,
                    rect.top + 14.0,
                    TextAlign::Right,
                    self.fs(16.0),
                    skc(theme::mix(pal.panel, pal.weak, pin), 1.0),
                );
                self.push_text(
                    fr,
                    &i18n::localize_number(&raw, self.lang),
                    rect.right - 10.0,
                    rect.bottom - 12.0,
                    TextAlign::Right,
                    self.fs(24.0),
                    skc(theme::mix(pal.panel, pal.text, pin), 1.0),
                );
                let _ = raw;
                fr.regions.push(HitRegion {
                    key,
                    rect,
                    act: HitAct::LoadHist(id),
                });
            }
        }
        fr.pop();
        fr.pop();
    }

    // ---------- help & settings ----------

    fn build_help(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, lh: f32) {
        let s = i18n::strings(self.lang);
        let pad = self.fs(24.0);
        let top = self.top_h();
        fr.push_clip(Rect::from_xywh(0.0, top, lw, (lh - top).max(0.0)));
        fr.push_translate(0.0, -self.help_scroll);
        let mut y = top + self.fs(12.0);
        self.push_text(fr, s.app_name, pad, y + self.fs(18.0), TextAlign::Left, self.fs(42.0), skc(pal.text, 1.0));
        y += self.fs(56.0);
        self.push_text(fr, s.safety_line, pad, y, TextAlign::Left, self.fs(17.0), skc(pal.weak, 1.0));
        y += self.fs(34.0);
        for sec in i18n::help_sections(self.lang) {
            y += self.fs(16.0);
            fr.cmds.push(Cmd::FillRect(
                Rect::from_xywh(pad, y + self.fs(3.0), self.fs(5.0), self.fs(26.0)),
                skc(pal.accent, 1.0),
            ));
            self.push_text(fr, sec.title, pad + self.fs(14.0), y + self.fs(16.0), TextAlign::Left, self.fs(30.0), skc(pal.text, 1.0));
            y += self.fs(40.0);
            self.push_text_ml(
                fr,
                sec.body,
                pad + self.fs(4.0),
                y,
                TextAlign::Left,
                self.fs(18.0),
                skc(pal.weak, 1.0),
                lw - pad * 2.0,
            );
            let lines = self.wrap_text(sec.body, 18.0, lw - pad * 2.0).len() as f32;
            y += lines * self.fs(26.0) + self.fs(12.0);
        }
        self.help_content_h = y + self.fs(24.0) - top;
        fr.pop();
        fr.pop();
        // Vertical panning via wheel and blank-area drag gesture
    }

    fn build_settings(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, lh: f32, now: Instant) {
        let s = i18n::strings(self.lang);
        let elapsed = now.duration_since(self.page_enter).as_secs_f32();
        let k = self.ui_scale;
        let pad = self.fs(24.0);
        let gap = self.fs(8.0);
        let top = self.top_h();
        fr.push_clip(Rect::from_xywh(0.0, top, lw, (lh - top).max(0.0)));
        fr.push_translate(0.0, -self.set_scroll);
        let mut y = top + self.fs(16.0);

        // language — sections stagger in on page entry (hit regions move with the draw)
        let mut off = (1.0 - theme::ease_out(elapsed / 0.32)) * self.fs(26.0);
        y += off;
        self.heading(fr, pal, s.language, &mut y);
        let bw = ((lw - 2.0 * pad - 2.0 * gap) / 3.0).max(k * 120.0);
        for (i, l) in Lang::ALL.iter().enumerate() {
            let rect = Rect::from_xywh(pad + i as f32 * (bw + gap), y, bw, self.fs(80.0));
            self.select_tile(fr, pal, HitKey::Lang(*l), HitAct::SetLang(*l), rect, l.native(), self.lang == *l);
        }
        y += self.fs(80.0 + 16.0);
        y -= off;

        // theme
        off = (1.0 - theme::ease_out((elapsed - 0.08) / 0.32)) * self.fs(26.0);
        y += off;
        self.heading(fr, pal, s.theme, &mut y);
        let bw2 = ((lw - 2.0 * pad - gap) / 2.0).max(k * 120.0);
        let themes = [Theme::Dark, Theme::Light];
        let labels = [s.dark, s.light];
        for (i, t) in themes.iter().enumerate() {
            let rect = Rect::from_xywh(pad + i as f32 * (bw2 + gap), y, bw2, self.fs(80.0));
            self.select_tile(fr, pal, HitKey::ThemeOpt(*t), HitAct::SetTheme(*t), rect, labels[i], self.theme == *t);
        }
        y += self.fs(80.0 + 16.0);
        y -= off;

        // accent
        off = (1.0 - theme::ease_out((elapsed - 0.16) / 0.32)) * self.fs(26.0);
        y += off;
        self.heading(fr, pal, s.accent, &mut y);
        let n = theme::ACCENTS.len();
        let sw = ((lw - 2.0 * pad - (n as f32 - 1.0) * gap) / n as f32).max(k * 28.0);
        for (i, c) in theme::ACCENTS.iter().enumerate() {
            let rect = Rect::from_xywh(pad + i as f32 * (sw + gap), y, sw, self.fs(52.0));
            let (hov, _) = self.anim_get(HitKey::Accent(i));
            fr.cmds.push(Cmd::FillRect(rect, skc(theme::mix(*c, Color32::WHITE, 0.15 * hov), 1.0)));
            if self.accent_idx == i {
                fr.cmds.push(Cmd::StrokeRect(
                    Rect::from_xywh(rect.left + 3.0, rect.top + 3.0, sw - 6.0, self.fs(38.0)),
                    skc(Color32::WHITE, 1.0),
                    2.0,
                ));
            } else if hov > 0.01 {
                fr.cmds.push(Cmd::StrokeRect(
                    Rect::from_xywh(rect.left + 3.0, rect.top + 3.0, sw - 6.0, self.fs(38.0)),
                    skc(pal.text, hov * 0.6),
                    1.5,
                ));
            }
            fr.regions.push(HitRegion {
                key: HitKey::Accent(i),
                rect,
                act: HitAct::SetAccent(i),
            });
        }
        y += self.fs(44.0 + 6.0);
        self.push_text(
            fr,
            s.accents[self.accent_idx.min(7)],
            pad,
            y,
            TextAlign::Left,
            self.fs(16.0),
            skc(pal.weak, 1.0),
        );
        y += self.fs(24.0 + 14.0);
        y -= off;

        // data
        off = (1.0 - theme::ease_out((elapsed - 0.24) / 0.32)) * self.fs(26.0);
        y += off;
        self.heading(fr, pal, s.data, &mut y);
        let count_text = s.history_count.replace("{}", &self.engine.history.len().to_string());
        self.push_text(fr, &count_text, pad, y + self.fs(16.0), TextAlign::Left, self.fs(18.0), skc(pal.weak, 1.0));
        {
            let font = self.font(s.clear, 14.0);
            let (tw, _) = font.measure_str(s.clear, None);
            let rect = Rect::from_xywh(pad + self.fs(140.0), y, tw + self.fs(22.0), self.fs(28.0));
            let (hov, _) = self.anim_get(HitKey::SetClear);
            if hov > 0.01 {
                fr.cmds.push(Cmd::FillRect(rect, skc(theme::mix(pal.bg, pal.text, 0.10 * hov), 1.0)));
            }
            self.push_text(
                fr,
                s.clear,
                rect.center_x(),
                rect.center_y(),
                TextAlign::Center,
                self.fs(14.0),
                skc(theme::mix(pal.bg, pal.text, 0.55 + 0.45 * hov), 1.0),
            );
            fr.regions.push(HitRegion {
                key: HitKey::SetClear,
                rect,
                act: HitAct::ClearHist,
            });
        }
        y += self.fs(36.0);
        let path = self
            .settings_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("(n/a)"));
        let path_text = s.settings_path.replace("{}", &path.to_string_lossy());
        self.push_text_ml(fr, &path_text, pad, y, TextAlign::Left, self.fs(12.0), skc(pal.weak, 1.0), lw - pad * 2.0);
        y += self.fs(30.0 + 12.0);
        y -= off;

        // about
        off = (1.0 - theme::ease_out((elapsed - 0.32) / 0.32)) * self.fs(26.0);
        y += off;
        self.heading(fr, pal, s.version_title, &mut y);
        self.push_text(fr, s.about_line, pad, y + self.fs(14.0), TextAlign::Left, self.fs(18.0), skc(pal.text, 1.0));
        y += self.fs(32.0);
        self.push_text(fr, s.safety_line, pad, y, TextAlign::Left, self.fs(16.0), skc(pal.weak, 1.0));
        y += self.fs(30.0);
        y -= off;

        self.set_content_h = y + self.fs(24.0) - top;
        fr.pop();
        fr.pop();
        // Vertical panning via wheel and blank-area drag gesture
    }

    fn heading(&self, fr: &mut Frame, pal: &Palette, title: &str, y: &mut f32) {
        fr.cmds.push(Cmd::FillRect(
            Rect::from_xywh(self.fs(24.0), *y, self.fs(5.0), self.fs(28.0)),
            skc(pal.accent, 1.0),
        ));
        self.push_text(fr, title, self.fs(40.0), *y + self.fs(14.0), TextAlign::Left, self.fs(28.0), skc(pal.text, 1.0));
        *y += self.fs(46.0);
    }

    fn select_tile(
        &mut self,
        fr: &mut Frame,
        pal: &Palette,
        key: HitKey,
        act: HitAct,
        rect: Rect,
        label: &str,
        selected: bool,
    ) {
        let (hov, _) = self.anim_get(key);
        let fill = if selected { pal.accent } else { theme::mix(pal.bg, pal.text, 0.06) };
        let fill = theme::mix(fill, Color32::WHITE, 0.12 * hov);
        fr.cmds.push(Cmd::FillRect(rect, skc(fill, 1.0)));
        self.push_text(
            fr,
            label,
            rect.center_x(),
            rect.center_y(),
            TextAlign::Center,
            self.fs(24.0),
            skc(if selected { Color32::WHITE } else { pal.text }, 1.0),
        );
        fr.regions.push(HitRegion { key, rect, act });
    }

    // ---------- splash ----------

    fn build_splash(&mut self, fr: &mut Frame, pal: &Palette, lw: f32, lh: f32, now: Instant) {
        if self.splash_done {
            return;
        }
        let t = now.duration_since(self.t0).as_secs_f32();
        let fade = if t < SPLASH_SECS - 0.3 {
            1.0
        } else {
            1.0 - (t - (SPLASH_SECS - 0.3)) / 0.3
        };
        fr.cmds.push(Cmd::FillRect(
            Rect::from_xywh(0.0, 0.0, lw, lh),
            skc(pal.bg, fade),
        ));
        let cols = [pal.accent, Color32::from_rgb(0x00, 0xAB, 0xA9), pal.equals];
        for (i, col) in cols.iter().enumerate() {
            let delay = i as f32 * 0.08;
            let p = theme::ease_out((t - delay) / 0.25);
            if p > 0.01 {
                let size = 34.0 * (0.3 + 0.7 * p);
                let cx = lw * 0.5 + (i as f32 - 1.0) * 46.0;
                let cy = lh * 0.5 - 26.0;
                fr.cmds.push(Cmd::FillRect(
                    Rect::from_xywh(cx - size * 0.5, cy - size * 0.5, size, size),
                    skc(*col, p * fade),
                ));
            }
        }
        let tp = theme::ease_out((t - 0.22) / 0.30);
        if tp > 0.01 {
            let s = i18n::strings(self.lang);
            self.push_text(
                fr,
                s.app_name,
                lw * 0.5,
                lh * 0.5 + 34.0 + (1.0 - tp) * 16.0,
                TextAlign::Center,
                34.0,
                skc(theme::mix(pal.bg, pal.text, tp * fade), 1.0),
            );
            self.push_text(
                fr,
                s.tagline,
                lw * 0.5,
                lh * 0.5 + 74.0,
                TextAlign::Center,
                14.0,
                skc(theme::mix(pal.bg, pal.weak, tp * fade), 1.0),
            );
        }
        fr.regions.push(HitRegion {
            key: HitKey::SplashSkip,
            rect: Rect::from_xywh(0.0, 0.0, lw, lh),
            act: HitAct::SkipSplash,
        });
    }

    // ---------- rendering ----------

    fn sync_size(&mut self) {
        let Some(window) = &self.window else { return };
        let scale = window.scale_factor() as f32;
        let size = window.inner_size();
        let (pw, ph) = (size.width.max(1), size.height.max(1));
        self.scale = scale;
        self.lw = pw as f32 / scale;
        self.lh = ph as f32 / scale;
        // Responsive: 720×560 baseline (larger fonts), keeps expanding proportionally fullscreen
        self.ui_scale = ((self.lw / 720.0).max(self.lh / 560.0)).clamp(1.0, 2.4);
        if self.surface_size != (pw, ph) {
            match self.backend.as_mut() {
                Some(Backend::Gpu {
                    surface,
                    gl_surface,
                    gl_context,
                    gr_context,
                    fb_info,
                    num_samples,
                    stencil_size,
                    ..
                }) => {
                    *surface = Self::gpu_surface(
                        (pw as i32, ph as i32),
                        gr_context,
                        fb_info,
                        *num_samples,
                        *stencil_size,
                    )
                    .expect("skia gpu surface");
                    let _ = gl_surface.resize(
                        gl_context,
                        NonZeroU32::new(pw).unwrap(),
                        NonZeroU32::new(ph).unwrap(),
                    );
                }
                Some(Backend::Cpu { surface, sb_surface, pixels }) => {
                    let _ = sb_surface.resize(NonZeroU32::new(pw).unwrap(), NonZeroU32::new(ph).unwrap());
                    *surface = surfaces_raster(pw, ph);
                    pixels.clear();
                    pixels.resize(pw as usize * ph as usize * 4, 0);
                }
                None => {}
            }
            self.surface_size = (pw, ph);
        }
    }

    fn gpu_surface(
        size: (i32, i32),
        gr_context: &mut skia_safe::gpu::DirectContext,
        fb_info: &skia_safe::gpu::gl::FramebufferInfo,
        num_samples: usize,
        stencil_size: usize,
    ) -> Option<SkSurface> {
        let target = skia_safe::gpu::backend_render_targets::make_gl(
            size,
            num_samples,
            stencil_size,
            *fb_info,
        );
        skia_safe::gpu::surfaces::wrap_backend_render_target(
            gr_context,
            &target,
            skia_safe::gpu::SurfaceOrigin::BottomLeft,
            ColorType::RGBA8888,
            None,
            None,
        )
    }

    fn render(&mut self) {
        self.sync_size();
        let now = Instant::now();
        let fr = self.build_frame(now);
        // Trim the animation state table: keep only regions visible this frame + persistent top-bar items (prevents unbounded growth)
        {
            let live: std::collections::HashSet<HitKey> =
                fr.regions.iter().map(|r| r.key).collect();
            self.anims
                .retain(|k, _| live.contains(k) || matches!(k, HitKey::Bar(_)));
        }
        for r in &fr.regions {
            self.anims.entry(r.key).or_default();
        }
        self.regions = fr.regions.clone();
        self.cursor_pointer = fr.cursor_pointer;

        // Must clear the canvas every frame: no leftover content from the previous frame (otherwise ghosting / barcode artifacts)
        let bg = skc(self.pal().bg, 1.0);

        match self.backend.as_mut() {
            Some(Backend::Gpu {
                surface,
                gr_context,
                gl_surface,
                gl_context,
                ..
            }) => {
                {
                    let canvas = surface.canvas();
                    canvas.clear(bg);
                    paint(canvas, &fr);
                }
                match gr_context.flush_and_submit() {
                    Ok(_) => {
                        let _ = gl_surface.swap_buffers(gl_context);
                    }
                    Err(e) => eprintln!("skia flush failed, skipping frame: {e}"),
                }
            }
            Some(Backend::Cpu {
                surface,
                sb_surface,
                pixels,
            }) => {
                {
                    let canvas = surface.canvas();
                    canvas.clear(bg);
                    paint(canvas, &fr);
                }
                let (w, h) = self.surface_size;
                let info = ImageInfo::new(
                    (w as i32, h as i32),
                    ColorType::RGBA8888,
                    skia_safe::AlphaType::Premul,
                    None,
                );
                if surface.read_pixels(&info, pixels, (w as usize) * 4, (0, 0)) {
                    if let Ok(mut buf) = sb_surface.buffer_mut() {
                        for (i, chunk) in pixels.chunks_exact(4).take(buf.len()).enumerate() {
                            buf[i] =
                                (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | chunk[2] as u32;
                        }
                        let _ = buf.present();
                    }
                }
            }
            None => {}
        }

        // cursor icon
        let want = if self.cursor_pointer { CursorIcon::Pointer } else { CursorIcon::Default };
        if self.cursor_icon != want {
            if let Some(w) = &self.window {
                w.set_cursor(want);
            }
            self.cursor_icon = want;
        }

        // Publish animation state to the driver thread: it wakes the event loop
        // at ~120 Hz while animations are in flight and stays silent when idle.
        self.anim_flag
            .store(self.animating(now), std::sync::atomic::Ordering::Relaxed);
    }

    // ---------- GPU init (preferred) ----------

    fn init_gpu(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let template = ConfigTemplateBuilder::new()
            .with_alpha_size(8)
            .with_transparency(false);
        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(window_attrs()))
            .build(el, template, |configs| {
                // Skia handles its own anti-aliasing; pick the fewest samples.
                configs
                    .reduce(|accum, config| {
                        if config.num_samples() < accum.num_samples() {
                            config
                        } else {
                            accum
                        }
                    })
                    .expect("no GL config available")
            })
            .map_err(|e| format!("display/config: {e}"))?;
        let window = window.ok_or("no window for GL")?;
        let raw = window
            .window_handle()
            .map_err(|e| format!("window handle: {e}"))?
            .as_raw();

        let attrs_gl = ContextAttributesBuilder::new().build(Some(raw));
        let attrs_es = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::Gles(None))
            .build(Some(raw));
        let not_current =
            gl_context::create_context(&gl_config.display(), &gl_config, &attrs_gl).or_else(|_| {
                gl_context::create_context(&gl_config.display(), &gl_config, &attrs_es)
            })
            .map_err(|e| format!("create_context: {e}"))?;

        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        let surf_attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw,
            NonZeroU32::new(w).unwrap(),
            NonZeroU32::new(h).unwrap(),
        );
        let gl_surface = gl_context::create_window_surface(
            &gl_config.display(),
            &gl_config,
            &surf_attrs,
        )
        .map_err(|e| format!("create_window_surface: {e}"))?;
        let gl_context = not_current
            .make_current(&gl_surface)
            .map_err(|e| format!("make_current: {e}"))?;

        let interface = skia_safe::gpu::gl::Interface::new_load_with(|name| {
            if name == "eglGetCurrentDisplay" {
                return std::ptr::null();
            }
            match CString::new(name) {
                Ok(c) => gl_config.display().get_proc_address(c.as_c_str()),
                Err(_) => std::ptr::null(),
            }
        })
        .ok_or("skia gl interface")?;
        let mut gr_context =
            skia_safe::gpu::direct_contexts::make_gl(interface, None).ok_or("make_gl")?;
        let fb_info = skia_safe::gpu::gl::FramebufferInfo {
            fboid: 0,
            format: skia_safe::gpu::gl::Format::RGBA8.into(),
            ..Default::default()
        };
        let num_samples = gl_config.num_samples() as usize;
        let stencil_size = gl_config.stencil_size() as usize;
        let surface = Self::gpu_surface(
            (w as i32, h as i32),
            &mut gr_context,
            &fb_info,
            num_samples,
            stencil_size,
        )
        .ok_or("wrap backend render target")?;
        let _ = gl_surface.set_swap_interval(
            &gl_context,
            SwapInterval::Wait(NonZeroU32::new(1).unwrap()),
        );

        self.window = Some(Arc::new(window));
        self.backend = Some(Backend::Gpu {
            gr_context,
            gl_surface,
            gl_context,
            surface,
            fb_info,
            num_samples,
            stencil_size,
        });
        self.sync_size();
        Ok(())
    }

    // ---------- CPU fallback (last resort: no GL available) ----------

    fn init_cpu(&mut self, el: &ActiveEventLoop) {
        let window = Arc::new(el.create_window(window_attrs()).expect("create window"));
        let ctx = softbuffer::Context::new(window.clone()).expect("softbuffer context");
        let mut sb_surface = SbSurface::new(&ctx, window.clone()).expect("softbuffer surface");
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        let _ = sb_surface.resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap());
        let surface = surfaces_raster(w, h);
        self.window = Some(window);
        self.backend = Some(Backend::Cpu {
            surface,
            sb_surface,
            pixels: vec![0u8; w as usize * h as usize * 4],
        });
        self.sync_size();
    }
}

fn surfaces_raster(w: u32, h: u32) -> SkSurface {
    skia_safe::surfaces::raster_n32_premul((w as i32, h as i32)).expect("raster surface")
}

fn window_attrs() -> winit::window::WindowAttributes {
    let mut attrs = Window::default_attributes()
        .with_title("Metro Calculator")
        .with_inner_size(LogicalSize::new(880.0f64, 680.0f64))
        .with_min_inner_size(LogicalSize::new(480.0f64, 360.0f64));
    if let Some(icon) = crate::icon::window_icon() {
        attrs = attrs.with_window_icon(Some(icon));
    }
    attrs
}

fn hist_expr_text(k: &HistKind, lang: Lang) -> String {
    match k {
        HistKind::Binary(a, o, b) => format!(
            "{} {} {}",
            i18n::fmt_value(*a, lang),
            o.sym(),
            i18n::fmt_value(*b, lang)
        ),
        HistKind::UnarySqr(x) => format!("{}²", i18n::fmt_value(*x, lang)),
        HistKind::UnarySqrt(x) => format!("√({})", i18n::fmt_value(*x, lang)),
        HistKind::UnaryRecip(x) => format!("1/({})", i18n::fmt_value(*x, lang)),
    }
}

// ---------- winit handler ----------

impl ApplicationHandler for MetroCalcApp {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        el.set_control_flow(ControlFlow::Wait);
        if self.window.is_some() {
            return;
        }
        match self.init_gpu(el) {
            Ok(()) => eprintln!("renderer: Skia GPU (OpenGL)"),
            Err(e) => {
                eprintln!(
                    "renderer: Skia GPU (OpenGL) unavailable ({e}) - falling back to CPU raster"
                );
                self.init_cpu(el);
            }
        }
        // Timing baseline starts when the window is ready: splash → title frame → staggered tiles
        self.t0 = Instant::now();
        self.entrance_start = self.t0;
        self.page_enter = self.t0;
        self.dirty = true;

        // Animation driver thread: request_redraw() is thread-safe (winit), so a
        // dedicated thread wakes the event loop at ~120 Hz whenever the render
        // path reports ongoing animation. Idle → flag false → thread just
        // sleeps, zero CPU. The thread dies with the process (never joined).
        let flag = self.anim_flag.clone();
        let win = self.window.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(8));
            if flag.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(w) = win.as_ref() {
                    w.request_redraw();
                }
            }
        });

        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(_) => {
                self.dirty = true;
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Focused(_) => {
                self.hover = None;
                self.dirty = true;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let s = self.scale.max(1.0);
                let (nx, ny) = ((position.x as f32) / s, (position.y as f32) / s);
                let now_i = Instant::now();
                let dt_e = now_i.duration_since(self.last_move).as_secs_f32().max(0.001);
                // Drag panning (Metro-style swipe gesture): press on blank space to scroll
                if let Some((horizontal, p0, s0)) = self.drag {
                    let cur = if horizontal { nx } else { ny };
                    let d = p0 - cur;
                    if horizontal {
                        self.help_scroll = s0 + d;
                    } else {
                        match self.page {
                            Page::Start => {
                                self.start_scroll_y = s0 + d;
                                // track flick velocity (EMA-smoothed) for inertia on release
                                let v = ((self.pointer.1 - ny) / dt_e).clamp(-4000.0, 4000.0);
                                self.start_scroll_vel = self.start_scroll_vel * 0.7 + v * 0.3;
                            }
                            Page::Help => self.help_scroll = s0 + d,
                            Page::Settings => self.set_scroll = s0 + d,
                            Page::Calc => self.calc_scroll = s0 + d,
                        }
                    }
                }
                self.pointer = (nx, ny);
                self.last_move = now_i;
                self.hover = self.hit_test(nx, ny).map(|r| r.key);
                self.dirty = true;
            }
            WindowEvent::CursorLeft { .. } => {
                self.hover = None;
                self.dirty = true;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => {
                    // Press only records the hit region + ripple feedback; mouse takes focus;
                    // blank-area press starts drag panning; actions fire once on Released
                    self.focus = None;
                    match self.hit_test(self.pointer.0, self.pointer.1).cloned() {
                        Some(r) => {
                            self.pressed = Some(r.key);
                            // Ripple feedback on every button (calc keys, start tiles, top bar)
                            if !matches!(r.act, HitAct::SkipSplash) {
                                let pal = self.pal();
                                self.ripples.push(Ripple {
                                    rect: r.rect,
                                    origin: self.pointer,
                                    start: Instant::now(),
                                    dark: pal.ripple_dark,
                                });
                            }
                        }
                        None => {
                            // All pages pan vertically (start-screen two-column tiles scroll vertically)
                            let sc = match self.page {
                                Page::Start => self.start_scroll_y,
                                Page::Help => self.help_scroll,
                                Page::Settings => self.set_scroll,
                                Page::Calc => self.calc_scroll,
                            };
                            self.drag = Some((false, self.pointer.1, sc));
                        }
                    }
                    self.dirty = true;
                }
                ElementState::Released => {
                    self.drag = None;
                    // cap the flick velocity that feeds start-screen inertia
                    self.start_scroll_vel = self.start_scroll_vel.clamp(-3000.0, 3000.0);
                    if let Some(k) = self.pressed.take() {
                        if let Some(r) = self.hit_test(self.pointer.0, self.pointer.1) {
                            if r.key == k {
                                let act = r.act;
                                self.activate(act, Instant::now());
                            }
                        }
                    }
                    self.dirty = true;
                }
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                let in_hist = self.history_open
                    && self.pointer.0 > self.lw - self.hist_w.value
                    && self.pointer.1 > self.top_h();
                if in_hist {
                    self.hist_scroll -= dy;
                } else {
                    match self.page {
                        Page::Start => self.start_scroll_y -= dy, // two-column tiles scroll vertically
                        Page::Help => self.help_scroll -= dy,
                        Page::Settings => self.set_scroll -= dy,
                        Page::Calc => self.calc_scroll -= dy, // vertical scroll when the window is too short
                    }
                }
                self.dirty = true;
            }
            WindowEvent::KeyboardInput { event: k, .. } => {
                if k.state == ElementState::Pressed && !k.repeat {
                    self.handle_key(&k);
                }
                self.dirty = true;
            }
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.quit {
            el.exit();
            return;
        }
        if self.dirty {
            self.dirty = false;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }
}

// ---------- fonts & config ----------

fn is_font_file(b: &[u8]) -> bool {
    b.len() > 12
        && (b.starts_with(b"ttcf")
            || b.starts_with(&[0x00, 0x01, 0x00, 0x00])
            || b.starts_with(b"OTTO")
            || b.starts_with(b"true"))
}

/// Create a typeface from font-file bytes via the system font manager
/// (Windows: DirectWrite / macOS: CoreText / Linux: FreeType). Note that
/// `FontMgr::new()` must be used —
/// `empty()` is the null-object manager and `make_from_data` always returns None.
fn load_typeface(paths: &[&str]) -> Option<Typeface> {
    let mgr = FontMgr::new();
    for p in paths {
        if let Ok(bytes) = std::fs::read(p) {
            if is_font_file(&bytes) {
                if let Some(tf) = mgr.new_from_data(Data::new_copy(&bytes), 0u32) {
                    return Some(tf);
                }
            }
        }
    }
    None
}

/// Fallback: request a family by name directly from the system font manager.
fn load_family(families: &[&str]) -> Option<Typeface> {
    let mgr = FontMgr::new();
    for name in families {
        if let Some(tf) = mgr.match_family_style(name, skia_safe::FontStyle::normal()) {
            return Some(tf);
        }
    }
    None
}

fn load_typefaces() -> (Option<Typeface>, Option<Typeface>) {
    let body = load_typeface(cjk_candidates()).or_else(|| {
        load_family(&[
            "Microsoft YaHei",
            "微软雅黑",
            "PingFang SC",
            "Noto Sans CJK SC",
            "WenQuanYi Micro Hei",
            "SimHei",
        ])
    });
    let display = load_typeface(latin_candidates())
        .or_else(|| {
            load_family(&["Segoe UI Light", "Segoe UI", "Helvetica Neue", "DejaVu Sans"])
        })
        .or_else(|| body.clone());
    (display, body)
}

fn cjk_candidates() -> &'static [&'static str] {
    #[cfg(windows)]
    let paths: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhl.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ];
    #[cfg(target_os = "macos")]
    let paths: &[&str] = &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let paths: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    ];
    #[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
    let paths: &[&str] = &[];
    paths
}

fn latin_candidates() -> &'static [&'static str] {
    #[cfg(windows)]
    let paths: &[&str] = &[
        r"C:\Windows\Fonts\segoeuil.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    #[cfg(target_os = "macos")]
    let paths: &[&str] = &[
        "/System/Library/Fonts/HelveticaNeue.ttc",
        "/System/Library/Fonts/Helvetica.ttc",
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let paths: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ];
    #[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
    let paths: &[&str] = &[];
    paths
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Library").join("Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    #[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
    let base: Option<PathBuf> = None;
    base
}

// ---------- headless self test ----------

pub mod selftest {
    pub fn run() -> Result<(), String> {
        crate::engine::self_check()?;
        crate::engine::fuzz_check()?;
        crate::i18n::parity_check()?;
        crate::i18n::format_check()?;
        font_smoke()?;
        Ok(())
    }

    /// Font smoke test: render a line of text offscreen and count non-background
    /// pixels, ensuring Skia font loading and rasterization really work
    /// (regression guard).
    fn font_smoke() -> Result<(), String> {
        let (display, body) = super::load_typefaces();
        let Some(tf) = body.as_ref().or(display.as_ref()) else {
            return Err("no typeface loaded (font smoke)".to_owned());
        };
        let mut surface =
            skia_safe::surfaces::raster_n32_premul((200, 100)).expect("smoke surface");
        {
            let canvas = surface.canvas();
            canvas.clear(skia_safe::Color::BLACK);
            let font = skia_safe::Font::from_typeface(tf.clone(), 48.0);
            let mut p = skia_safe::Paint::default();
            p.set_anti_alias(true);
            p.set_color(skia_safe::Color::WHITE);
            canvas.draw_str("123计算", (8.0, 62.0), &font, &p);
        }
        let info = skia_safe::ImageInfo::new(
            (200, 100),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        let mut buf = vec![0u8; 200 * 100 * 4];
        if !surface.read_pixels(&info, &mut buf, 200 * 4, (0, 0)) {
            return Err("font smoke: read_pixels failed".to_owned());
        }
        let lit = buf.chunks_exact(4).filter(|c| c[0] > 100).count();
        if lit < 300 {
            return Err(format!("font smoke: only {lit} lit pixels (text not rasterized)"));
        }
        eprintln!("font smoke: OK ({lit} lit pixels)");
        Ok(())
    }
}
