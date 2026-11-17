//! Metro 计算器 · Metro Calculator
//!
//! Memory safety: the crate is compiled with `#![deny(unsafe_code)]`. The
//! single exception is `gl_context.rs`, which contains two audited FFI
//! wrappers (GL context/window-surface creation for Skia's GPU backend);
//! the CPU fallback path and all other code are 100% safe Rust.
//! Rendering: Google Skia, GPU (OpenGL) preferred.
//! Windows / macOS / Linux via winit + glutin.

#![deny(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod engine;
mod gl_context;
mod i18n;
mod icon;
mod theme;

use std::error::Error;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("Metro Calculator 1.0.0 — Skia rendering, 100% safe Rust, zero unsafe blocks");
        return;
    }

    if args.iter().any(|a| a == "--selftest") {
        match app::selftest::run() {
            Ok(()) => println!("SELFTEST OK — engine fuzz, i18n, formatting and font checks passed"),
            Err(e) => {
                eprintln!("SELFTEST FAILED: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // 生成多尺寸透明 .ico（发布打包用）：metro-calculator.exe --gen-icon [路径]
    if let Some(pos) = args.iter().position(|a| a == "--gen-icon") {
        let path = args.get(pos + 1).cloned().unwrap_or_else(|| "metro.ico".to_owned());
        match icon::write_ico(std::path::Path::new(&path)) {
            Ok(()) => println!("icon written: {path}"),
            Err(e) => {
                eprintln!("gen-icon failed: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let event_loop = winit::event_loop::EventLoop::builder().build()?;
    let mut app = app::MetroCalcApp::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
