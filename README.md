# Metro 计算器 · Metro Calculator

一款以 **Windows 8 开始屏幕（Metro 设计语言）** 为蓝本的桌面应用：全屏彩色磁贴网格、分组、
白色极简矢量图标、大字号扁平排版、渐进式启动动画。核心功能为计算器。
使用 **Google Skia** 渲染（GPU 加速优先，无 GL 环境时自动回退 Skia CPU 光栅），跨 Windows / macOS / Linux。

**内存安全**：crate 以 `#![deny(unsafe_code)]` 编译。全项目唯一的 unsafe 是
`src/gl_context.rs` 中两行经过注释审计的 FFI 封装（GL 上下文/表面创建，Skia GPU 后端初始化必需）；
CPU 兜底渲染路径与其余全部代码均为 100% 安全 Rust。

---

## ✨ 特性 / Features

- **Windows 8 开始屏幕**：应用以全屏 Metro 磁贴网格开启——三组彩色磁贴（应用 / 个性化 / 数据）、
  方正无圆角大色块、白色极简矢量图标、大字号扁平排版、强调色分组标题、横向滚轮滚动、
  屏外分组按需渲染；「计算器」大磁贴每 5 秒动态翻转显示背面标语。
- **渐进式启动**：splash 三色方块 → 大标题与框架浮现 → 磁贴分批滑入淡入（一批一批，带动画）。
- **大色块计算器**：数字键为经典 Metro 纯色大磁贴，贴边网格；按压带 Win8 风格“下沉倾斜”回弹与涟漪；
  结果数字滚动 + 强调色扫光；错误抖动提示。
- **极致动画与流畅兼顾**：GPU 后端垂直同步；动画按需重绘，空闲 CPU 0%；
  无 GL 环境自动回退 Skia CPU 光栅（已实测可流畅运行）。
- **多语言**：简体中文 / English / Français 一键切换（开始屏幕语言磁贴或顶栏按钮）；
  数字格式随语言（法语逗号小数点、不间断空格分组）。
- **帮助文档**：内置多语言帮助页（F1）：使用方法、快捷键、设置、安全与隐私、关于。
- **深色 / 浅色主题** + **8 种 Metro 强调色**，即时生效并持久化。
- **历史记录**：最近 50 条，点击回填，随设置持久化。
- **完整键盘支持（可访问性）**：数字、运算符、Enter、Backspace、Delete（CE）、Esc（C）、
  Tab/Shift+Tab 焦点循环、方向键焦点移动、空格激活焦点键（白色焦点环可见）、H（历史）、Home（开始屏幕）、F1（帮助）。
- **跨平台**：Windows / macOS / Linux；自动探测系统中文字体（微软雅黑 / 苹方 / Noto Sans CJK），
  西文优先 Segoe UI Light（Windows）。
- **质量保障**：计算引擎完整单元测试 + **字体烟雾测试**（离屏渲染验证文字光栅化，`cargo test` / `--selftest`）。

## 🚀 构建与运行 / Build & Run

需要 Rust 1.80+。

```sh
cargo build --release
target/release/metro-calculator        # Windows: metro-calculator.exe
```

调试运行 / 自检：

```sh
cargo test                    # 引擎单元测试
cargo run -- --selftest       # 无 GUI 自检（引擎 + i18n + 数字格式化 + 字体烟雾测试）
cargo run --release           # 启动应用
```

> **离线构建提示**：`skia-bindings` 会从 GitHub Releases 下载预编译 Skia 库。
> 本仓库 `skia-bin/skia-binaries.tar.gz` 保存了一份与本机配对的缓存，
> `.cargo/config.toml` 已通过 `SKIA_BINARIES_URL=file://…` 指向它；
> 若在其他机器构建失败，删除 `.cargo/config.toml` 让它正常联网下载即可。

## 🎨 渲染架构 / Rendering

```
winit（窗口/事件） → Frame 构建（布局+动画+命中区域，屏外内容按需跳过）
                  → paint()（Skia 命令：矩形/圆/路径/文本）
    GPU 路径：glutin(WGL/EGL) → Skia GrDirectContext → 垂直同步交换
    CPU 兜底：skia surfaces::raster → softbuffer(GDI/X11/Wayland) 呈现
```

- 动画按需重绘（`ControlFlow::Wait`）：静态画面零重绘；动态磁贴翻转等仅在动画窗口内刷新。
- 命中区域与绘制同帧构建，输入命中测试与所见即所得一致。

## ⌨️ 快捷键 / Shortcuts

| 按键 | 功能 |
| --- | --- |
| `0-9` | 数字（计算器页） |
| `.` / `,` | 小数点 |
| `+` `-` `*` `/` | 运算符 |
| `%` | 百分比 |
| `Enter` / `=` | 等于（重复按重复上次运算） |
| `Backspace` | 退格 |
| `Delete` | CE 清除当前输入 |
| `Esc` | C 全部清除（其他页面返回开始屏幕） |
| `Tab` / `Shift+Tab` | 在按键之间循环键盘焦点 |
| `←` `→` `↑` `↓` | 按网格移动焦点 |
| `空格` | 激活焦点所在按键 |
| `Home` | 开始屏幕 |
| `H` | 开关历史面板 |
| `F1` | 帮助 |

## 🧮 运算说明

- `%`：加减时按“已有结果的百分比”（200 + 10% → 20）；乘除时按字面百分比（200 × 10% → 0.1）。
- `1/x` `x²` `√`：作用于当前显示值。
- 除以零、负数开方、溢出均给出本地化错误提示。

## 🔒 安全与隐私 / Safety & Privacy

- `#![deny(unsafe_code)]`；仅 `src/gl_context.rs`（GPU 初始化的两个 FFI 封装，含 SAFETY 说明）例外。
- 不访问网络、不读取剪贴板；仅读写自身设置文件。
- 设置与历史保存在本地：
  - Windows：`%APPDATA%\metro-calculator\settings.json`
  - macOS：`~/Library/Application Support/metro-calculator/settings.json`
  - Linux：`$XDG_CONFIG_HOME/metro-calculator/settings.json`（默认 `~/.config/...`）

## 📁 结构 / Structure

```
src/
  main.rs       入口、命令行参数（--selftest / --version）、winit 启动
  app.rs        UI 层：开始屏幕磁贴、动画、页面、字体加载、持久化、渲染后端（GPU/CPU）
  gl_context.rs 唯一的 unsafe 模块（两个 GL FFI 封装，含安全说明）
  engine.rs     计算器状态机（纯逻辑，含单元测试）
  i18n.rs       三语言字符串、帮助内容、数字本地化
  theme.rs      Metro 配色、颜色工具
```

## 许可 / License

MIT
