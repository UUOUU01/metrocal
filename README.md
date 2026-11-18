# Metro Calculator · Metro Calculator

A desktop app inspired by **Windows 8 Start Screen (Metro design language)**: full-screen colorful tile grid, grouped, minimalist white vector icons, large sans-serif typography, progressive entrance animations. Core feature is a calculator.

**Rendering**: Uses **Google Skia** (GPU-accelerated priority, automatic fallback to Skia CPU raster). Cross-platform on Windows / macOS / Linux.

**Memory Safe**: Crate compiles with `#![deny(unsafe_code)]`. The only unsafe in the project is two FFI-wrapped lines in `src/gl_context.rs` (GL context/surface creation, Skia GPU backend initialization required), which are audited; CPU fallback rendering path and all other code are 100% safe Rust.

---

## ✨ Features / Features

- **Windows 8 Start Screen**: App launches with a full-screen Metro tile grid—three groups of colorful tiles (Apps / Personalization / Data), sharp corners with large color blocks, minimalist white vector icons, large sans-serif typography, accent-colored group headers, horizontal wheel scrolling, off-screen groups rendered on demand; the "Calculator" large tile flips dynamically every 5 seconds to show a tagline on the back.
- **Progressive Launch**: Splash screen with three colored squares → large title and framework appear → tiles slide in and fade in batch by batch (with animations).
- **Large-Block Calculator**: Number keys are classic Metro solid-color tiles, edge-aligned grid; pressing triggers Win8-style "sinking tilt" with bounce and ripple; result number scrolls + accent-colored sweep; error shakes.
- **Animation & Smoothness**: GPU backend with vertical sync; animations redraw only when needed, idle CPU ~0%; automatic fallback to Skia CPU raster when no GL environment (tested to run smoothly).
- **More animations + multithreading**: theme/accent cross-fade, start-screen flick inertia scrolling, staggered history-panel and settings-section entrances, top-bar title crossfade, ripples on every button; a dedicated animation driver thread wakes rendering at ~120 Hz while animating, and a background writer thread owns all settings disk I/O so the UI thread never blocks.
- **Multi-language**: One-click toggle between Simplified Chinese / English / Français (via language tile on Start screen or top bar button); number formatting adapts to language (French uses comma as decimal, non-breaking spaces for thousands).
- **Help Documentation**: Built-in multi-language help page (F1): usage, shortcuts, settings, safety & privacy, about.
- **Dark / Light Theme** + **8 Metro accent colors**, instant and persisted.
- **History**: Last 50 entries, click to refill, persisted with settings.
- **Full Keyboard Support (Accessibility)**: Digits, operators, Enter, Backspace, Delete (CE), Esc (C), Tab/Shift+Tab focus cycling, arrow keys for focus movement, space to activate focused key (visible white focus ring), H (history), Home (start screen), F1 (help).
- **Cross-platform**: Windows / macOS / Linux; auto-detects system CJK fonts (Microsoft YaHei / PingFang SC / Noto Sans CJK), prefers Segoe UI Light (Windows).
- **Quality Assurance**: Complete unit tests for calculator engine + **font smoke test** (off-screen rendering to verify text rasterization, `cargo test` / `--selftest`).

## 🚀 Build & Run / Build & Run

Requires Rust 1.80+.

```sh
cargo build --release
target/release/metro-calculator        # Windows: metro-calculator.exe
```

Debug / self-test:

```sh
cargo test                    # engine unit tests
cargo run -- --selftest       # no GUI self-test (engine + i18n + number formatting + font smoke test)
cargo run --release           # launch app
```

> **Offline Build Hint**: `skia-bindings` downloads precompiled Skia libraries from GitHub Releases. This repository's `skia-bin/skia-binaries.tar.gz` caches a matching set; `.cargo/config.toml` points to it via `SKIA_BINARIES_URL=file://…`. If build fails on another machine, delete `.cargo/config.toml` to let it download normally.

## 🎨 Rendering Architecture / Rendering

```
winit (window/events) → Frame building (layout+animation+hit regions, off-screen content skipped on demand)
                  → paint() (Skia commands: rect/circle/path/text)
    GPU path: glutin(WGL/EGL) → Skia GrDirectContext → vertical sync swap
    CPU fallback: skia surfaces::raster → softbuffer(GDI/X11/Wayland) present
```

- Animations redraw only when needed (`ControlFlow::Wait`): static frames zero redraw; dynamic tile flipping only refreshes during animation windows.
- Hit regions built together with drawing in the same frame; input hit testing matches what you see.

## ⌨️ Shortcuts / Shortcuts

| Key | Function |
| --- | --- |
| `0-9` | Digits (calculator page) |
| `.` / `,` | Decimal point |
| `+` `-` `*` `/` | Operators |
| `%` | Percent |
| `Enter` / `=` | Equals (repeat on press) |
| `Backspace` | Backspace |
| `Delete` | CE clear current entry |
| `Esc` | C clear all (returns to start screen on other pages) |
| `Tab` / `Shift+Tab` | Cycle keyboard focus between keys |
| `←` `→` `↑` `↓` | Move focus in grid |
| `Space` | Activate focused key |
| `Home` | Start screen |
| `H` | Toggle history panel |
| `F1` | Help |

## 🧮 Operation / Operation

- `%`: "percentage of existing result" when adding/subtracting (200 + 10% → 20); literal percentage when multiplying/dividing (200 × 10% → 0.1).
- `1/x` `x²` `√`: Operate on current display value.
- Division by zero, square root of negative number, overflow all produce localized error messages.

## 🔒 Safety & Privacy / Safety & Privacy

- `#![deny(unsafe_code)]`; only `src/gl_context.rs` (two GL FFI wrappers, with SAFETY comments) is an exception.
- No network access, no clipboard reading; only reads/writes its own settings file.
- Settings and history stored locally:
  - Windows: `%APPDATA%\metro-calculator\settings.json`
  - macOS: `~/Library/Application Support/metro-calculator/settings.json`
  - Linux: `$XDG_CONFIG_HOME/metro-calculator/settings.json` (default `~/.config/...`)

## 📁 Structure / Structure

```
src/
  main.rs       entry, CLI args (--selftest / --version), winit launch
  app.rs        UI layer: start screen tiles, animations, pages, font loading, persistence, rendering backend (GPU/CPU)
  gl_context.rs only unsafe module (two GL FFI wrappers, with safety notes)
  engine.rs     calculator state machine (pure logic, includes unit tests)
  i18n.rs       three-language strings, help content, number localization
  theme.rs      Metro color scheme, color utilities
```

## License / License

MIT

Generated by AI.
