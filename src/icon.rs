//! 应用图标：蓝色方底 + 白色计算器字形（Metro 扁平），透明背景。
//! 运行时窗口图标（标题栏/任务栏）+ 多尺寸规范 .ico 生成器（32bpp BMP 条目）。

use skia_safe::{surfaces, Color, ColorType, AlphaType, ImageInfo, Paint, PaintStyle, Rect};

const BG: (u8, u8, u8) = (0x00, 0x78, 0xD4); // Metro 蓝

/// 渲染 size×size 的 RGBA（自上而下、逐像素 4 字节）图标位图。
pub fn render_rgba(size: u32) -> Vec<u8> {
    let mut surface =
        surfaces::raster_n32_premul((size as i32, size as i32)).expect("icon surface");
    {
        let canvas = surface.canvas();
        canvas.clear(Color::TRANSPARENT);
        let s = size as f32;
        let m = s * 0.04; // 透明外边
        let side = s - 2.0 * m;
        // 蓝色方底（Metro 直角）
        let mut p = Paint::default();
        p.set_color(Color::from_rgb(BG.0, BG.1, BG.2));
        canvas.draw_rect(Rect::from_xywh(m, m, side, side), &p);
        // 白色计算器字形（居中）
        let gw = side * 0.50; // 字形宽
        let gh = side * 0.64; // 字形高
        let gx = m + (side - gw) / 2.0;
        let gy = m + (side - gh) / 2.0;
        let mut w = Paint::default();
        w.set_anti_alias(true);
        w.set_color(Color::WHITE);
        w.set_style(PaintStyle::Stroke);
        w.set_stroke_width(s * 0.035);
        canvas.draw_rect(Rect::from_xywh(gx, gy, gw, gh), &w);
        // 显示条
        let mut f = Paint::default();
        f.set_anti_alias(true);
        f.set_color(Color::WHITE);
        canvas.draw_rect(
            Rect::from_xywh(gx + gw * 0.16, gy + gh * 0.12, gw * 0.68, gh * 0.16),
            &f,
        );
        // 2×3 按键点
        for r in 0..2 {
            for c in 0..3 {
                canvas.draw_circle(
                    (
                        gx + gw * 0.24 + c as f32 * gw * 0.26,
                        gy + gh * 0.50 + r as f32 * gh * 0.20,
                    ),
                    s * 0.026,
                    &f,
                );
            }
        }
    }
    let info = ImageInfo::new(
        (size as i32, size as i32),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let mut buf = vec![0u8; (size * size * 4) as usize];
    surface.read_pixels(&info, &mut buf, (size * 4) as usize, (0, 0));
    buf
}

/// 运行时窗口图标（标题栏 + 任务栏）。
pub fn window_icon() -> Option<winit::window::Icon> {
    let rgba = render_rgba(64);
    winit::window::Icon::from_rgba(rgba, 64, 64).ok()
}

/// 生成规范 .ico（16/24/32/48/64/128/256，全部 32bpp 带透明通道）。
pub fn write_ico(path: &std::path::Path) -> std::io::Result<()> {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let mut images: Vec<(u32, Vec<u8>)> = Vec::new();
    for &s in &sizes {
        let rgba = render_rgba(s);
        // 32bpp BMP：BITMAPINFOHEADER(40) + 自下而上 BGRA 像素 + AND 掩码（全 0）
        let mask_row = ((s + 31) / 32) * 4;
        let mut bmp: Vec<u8> = Vec::with_capacity(40 + (s * 4 + mask_row * s) as usize);
        let mut push32 = |v: i32, out: &mut Vec<u8>| out.extend((v as u32).to_le_bytes());
        push32(40, &mut bmp); // biSize
        push32(s as i32, &mut bmp); // biWidth
        push32((s * 2) as i32, &mut bmp); // biHeight = 图像高度 ×2（含掩码）
        bmp.extend(1u16.to_le_bytes()); // biPlanes
        bmp.extend(32u16.to_le_bytes()); // biBitCount
        push32(0, &mut bmp); // biCompression = BI_RGB
        push32(((s * 4) + mask_row * s) as i32, &mut bmp); // biSizeImage
        push32(0, &mut bmp);
        push32(0, &mut bmp);
        push32(0, &mut bmp);
        push32(0, &mut bmp);
        for y in (0..s).rev() {
            for x in 0..s {
                let i = ((y * s) + x) as usize * 4;
                bmp.push(rgba[i + 2]); // B
                bmp.push(rgba[i + 1]); // G
                bmp.push(rgba[i]); // R
                bmp.push(rgba[i + 3]); // A
            }
        }
        bmp.extend(std::iter::repeat(0u8).take((mask_row * s) as usize));
        images.push((s, bmp));
    }

    let mut out: Vec<u8> = Vec::new();
    out.extend(0u16.to_le_bytes()); // reserved
    out.extend(1u16.to_le_bytes()); // type = icon
    out.extend((images.len() as u16).to_le_bytes()); // count
    let mut offset = (6 + 16 * images.len()) as u32;
    for (s, bmp) in &images {
        let b = if *s >= 256 { 0u8 } else { *s as u8 };
        out.push(b); // width
        out.push(b); // height
        out.push(0); // palette
        out.push(0); // reserved
        out.extend(1u16.to_le_bytes()); // planes
        out.extend(32u16.to_le_bytes()); // bpp
        out.extend((bmp.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += bmp.len() as u32;
    }
    for (_, bmp) in &images {
        out.extend_from_slice(bmp);
    }
    std::fs::write(path, out)
}
