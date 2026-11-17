//! OpenGL 上下文创建 —— 本 crate 中唯一包含 unsafe 代码的模块。
//!
//! OpenGL context creation — the ONLY module in this crate that contains
//! unsafe code. The crate is compiled with `#![deny(unsafe_code)]`; this
//! module deliberately opts in (`#![allow(unsafe_code)]`) for exactly two
//! audited FFI calls into glutin that create the GL context / window surface
//! used by Skia's GPU backend. Everything else in the application is safe
//! Rust; the CPU fallback path never touches this module.
//!
//! SAFETY contract: glutin requires that the raw window handle used to create
//! a surface stays valid for the surface's lifetime. We guarantee this by
//! keeping the `Arc<Window>` alive for the whole program and dropping the
//! `Backend` (surface + context) before the window field drops.
#![allow(unsafe_code)]

use glutin::config::Config;
use glutin::display::{Display, GlDisplay};
use glutin::error::Error as GlError;
use glutin::surface::{Surface as GlutinSurface, SurfaceAttributes, WindowSurface};
use glutin::context::{ContextAttributes, NotCurrentContext};

/// 创建未激活的 GL 上下文。
///
/// SAFETY: `display.create_context` is an FFI call into the platform GL/EGL/WGL
/// loader. It is sound here because `config` was enumerated by this `display`
/// and `attrs` only carries version/api preferences (glutin validates both
/// before touching FFI).
pub fn create_context(
    display: &Display,
    config: &Config,
    attrs: &ContextAttributes,
) -> Result<NotCurrentContext, String> {
    unsafe { display.create_context(config, attrs) }.map_err(|e: GlError| e.to_string())
}

/// 创建窗口表面（交换缓冲目标）。
///
/// SAFETY: `display.create_window_surface` is an FFI call; sound because the
/// raw window handle inside `attrs` belongs to a live window whose `Arc` we
/// keep for the program's lifetime.
pub fn create_window_surface(
    display: &Display,
    config: &Config,
    attrs: &SurfaceAttributes<WindowSurface>,
) -> Result<GlutinSurface<WindowSurface>, String> {
    unsafe { display.create_window_surface(config, attrs) }.map_err(|e: GlError| e.to_string())
}
