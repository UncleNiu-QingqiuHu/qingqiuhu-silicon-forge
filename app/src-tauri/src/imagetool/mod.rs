//! 图片格式转换与基础处理。
//!
//! 逐文件调用由前端队列驱动，方便显示进度与单条重试（PRD F5-2/F5-3）。
//!
//! 几个必须显式处理的坑：
//! - PNG/JPG 互转时透明区域会变黑，必须主动铺底（F4-6）
//! - 手机照片带 EXIF 方向，不矫正会躺倒（F4-7）
//! - SVG 必须按目标尺寸渲染，否则按原始尺寸出图（F4-8）
//! - ICO 有 256px 上限，大图需自动缩（F4-9）

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ExtendedColorType, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ConvertOptions {
    /// png / jpg / webp / bmp / ico
    pub target_format: String,
    /// 1-100，仅对 jpg 生效；webp 当前为无损编码
    pub quality: u8,
    /// 长边限制（像素）
    pub max_edge: Option<u32>,
    /// 按百分比缩放，与 max_edge 同时存在时长边优先
    pub scale_percent: Option<f32>,
    /// PNG → JPG 时的铺底色，如 "#FFFFFF"
    pub background: Option<String>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            target_format: "png".into(),
            quality: 85,
            max_edge: None,
            scale_percent: None,
            background: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConvertOutcome {
    pub source: String,
    pub output: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub note: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("不支持的文件格式：{0}")]
    Unsupported(String),
    #[error("无法读取图片：{0}")]
    Read(String),
    #[error("无法写入图片：{0}")]
    Write(String),
    #[error("SVG 处理失败：{0}")]
    Svg(String),
    #[error("图片过大，超过处理上限")]
    TooLarge,
    #[error("文件系统错误：{0}")]
    Io(String),
}

impl Serialize for ImageError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

const MAX_PIXELS: u64 = 200_000_000;

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_svg(path: &Path) -> bool {
    ext_of(path) == "svg"
}

/// 转换单个文件。
pub fn convert(src: &Path, out_dir: Option<&Path>, opt: &ConvertOptions) -> Result<ConvertOutcome, ImageError> {
    let target = opt.target_format.to_ascii_lowercase();
    let (target_fmt, note_quality) = match target.as_str() {
        "png" => ("png", None),
        "jpg" | "jpeg" => ("jpg", None),
        "webp" => ("webp", Some("WebP 当前为无损编码，质量参数未生效".to_string())),
        "bmp" => ("bmp", None),
        "ico" => ("ico", None),
        other => return Err(ImageError::Unsupported(other.to_string())),
    };

    let mut img = if is_svg(src) {
        render_svg(src, opt.max_edge)?
    } else {
        let decoded = image::ImageReader::open(src)
            .map_err(|e| ImageError::Read(e.to_string()))?
            .with_guessed_format()
            .map_err(|e| ImageError::Read(e.to_string()))?
            .decode()
            .map_err(|e| ImageError::Read(e.to_string()))?;
        let (w, h) = (decoded.width() as u64, decoded.height() as u64);
        if w * h > MAX_PIXELS {
            return Err(ImageError::TooLarge);
        }
        apply_exif_orientation(decoded, src)
    };

    img = resize_if_needed(img, opt);

    // 目标格式不支持透明时必须铺底，否则透明区会变黑
    if target_fmt != "png" && target_fmt != "webp" {
        img = flatten(img, opt.background.as_deref());
    }

    let dir: PathBuf = match out_dir {
        Some(d) => d.to_path_buf(),
        None => src.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    std::fs::create_dir_all(&dir).map_err(|e| ImageError::Io(e.to_string()))?;

    let out_path = unique_output_path(src, &dir, target_fmt);
    encode(&img, &out_path, target_fmt, opt.quality)?;

    let bytes = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);

    Ok(ConvertOutcome {
        source: src.to_string_lossy().to_string(),
        output: out_path.to_string_lossy().to_string(),
        bytes,
        width: img.width(),
        height: img.height(),
        note: note_quality,
    })
}

fn resize_if_needed(img: DynamicImage, opt: &ConvertOptions) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return img;
    }

    if let Some(max_edge) = opt.max_edge {
        let long = w.max(h);
        if long > max_edge {
            let ratio = max_edge as f32 / long as f32;
            let nw = ((w as f32 * ratio).round() as u32).max(1);
            let nh = ((h as f32 * ratio).round() as u32).max(1);
            return img.resize_exact(nw, nh, image::imageops::FilterType::Lanczos3);
        }
        return img;
    }

    if let Some(pct) = opt.scale_percent {
        if pct > 0.0 && (pct - 100.0).abs() > f32::EPSILON {
            let ratio = pct / 100.0;
            let nw = ((w as f32 * ratio).round() as u32).max(1);
            let nh = ((h as f32 * ratio).round() as u32).max(1);
            return img.resize_exact(nw, nh, image::imageops::FilterType::Lanczos3);
        }
    }

    img
}

/// 根据 EXIF Orientation 矫正方向（手机照片常见）。
fn apply_exif_orientation(img: DynamicImage, src: &Path) -> DynamicImage {
    let file = match File::open(src) {
        Ok(f) => f,
        Err(_) => return img,
    };
    let mut reader = BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(e) => e,
        Err(_) => return img,
    };
    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1);

    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn parse_hex_color(s: &str) -> image::Rgba<u8> {
    let t = s.trim().trim_start_matches('#');
    if t.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&t[0..2], 16),
            u8::from_str_radix(&t[2..4], 16),
            u8::from_str_radix(&t[4..6], 16),
        ) {
            return image::Rgba([r, g, b, 255]);
        }
    }
    image::Rgba([255, 255, 255, 255])
}

/// 把带 alpha 的图铺到纯色背景上。不这样做的话转 JPG 会出现黑底。
fn flatten(img: DynamicImage, background: Option<&str>) -> DynamicImage {
    if !img.color().has_alpha() {
        return img;
    }
    let rgba = img.to_rgba8();
    let color = parse_hex_color(background.unwrap_or("#FFFFFF"));
    let mut canvas = RgbaImage::from_pixel(rgba.width(), rgba.height(), color);
    image::imageops::overlay(&mut canvas, &rgba, 0, 0);
    DynamicImage::ImageRgba8(canvas)
}

fn render_svg(src: &Path, max_edge: Option<u32>) -> Result<DynamicImage, ImageError> {
    let data = std::fs::read(src).map_err(|e| ImageError::Read(e.to_string()))?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())
        .map_err(|e| ImageError::Svg(e.to_string()))?;

    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if sw <= 0.0 || sh <= 0.0 {
        return Err(ImageError::Svg("SVG 尺寸无效".into()));
    }

    let scale = match max_edge {
        Some(m) => (m as f32 / sw.max(sh)).max(0.01),
        None => 1.0,
    };
    let w = (sw * scale).round().max(1.0) as u32;
    let h = (sh * scale).round().max(1.0) as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| ImageError::Svg("无法创建渲染画布".into()))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(w as f32 / sw, h as f32 / sh),
        &mut pixmap.as_mut(),
    );

    let buffer = pixmap.data().to_vec();
    let img = RgbaImage::from_raw(w, h, buffer)
        .ok_or_else(|| ImageError::Svg("渲染结果尺寸不匹配".into()))?;
    Ok(DynamicImage::ImageRgba8(img))
}

fn unique_output_path(src: &Path, dir: &Path, ext: &str) -> PathBuf {
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "image".into());
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut i = 1;
    while candidate.exists() {
        candidate = dir.join(format!("{stem}_{i}.{ext}"));
        i += 1;
    }
    candidate
}

fn encode(img: &DynamicImage, path: &Path, fmt: &str, quality: u8) -> Result<(), ImageError> {
    let q = quality.clamp(1, 100);

    match fmt {
        "jpg" => {
            let mut file = File::create(path).map_err(|e| ImageError::Write(e.to_string()))?;
            let rgb = img.to_rgb8();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, q);
            enc.encode_image(&rgb)
                .map_err(|e| ImageError::Write(e.to_string()))?;
        }
        "png" => {
            img.save_with_format(path, ImageFormat::Png)
                .map_err(|e| ImageError::Write(e.to_string()))?;
        }
        "webp" => {
            let mut file = File::create(path).map_err(|e| ImageError::Write(e.to_string()))?;
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            image::codecs::webp::WebPEncoder::new_lossless(&mut file)
                .encode(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(|e| ImageError::Write(e.to_string()))?;
        }
        "bmp" => {
            img.save_with_format(path, ImageFormat::Bmp)
                .map_err(|e| ImageError::Write(e.to_string()))?;
        }
        "ico" => {
            // ICO 单帧上限 256px，需要先缩
            let limited = if img.width() > 256 || img.height() > 256 {
                img.resize(256, 256, image::imageops::FilterType::Lanczos3)
            } else {
                img.clone()
            };
            limited
                .save_with_format(path, ImageFormat::Ico)
                .map_err(|e| ImageError::Write(e.to_string()))?;
        }
        _ => return Err(ImageError::Unsupported(fmt.to_string())),
    }

    Ok(())
}

/// 只读文件头，用于前端在转换前展示尺寸与格式，不解码整图。
#[derive(Debug, Serialize)]
pub struct ImageProbe {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
}

#[tauri::command]
pub fn image_probe(src: String) -> Result<ImageProbe, String> {
    use crate::features;
    features::ensure_available("F4-1").map_err(|e| e.to_string())?;

    let path = Path::new(&src);
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let format = reader
        .format()
        .map(|f| format!("{:?}", f).to_ascii_lowercase())
        .unwrap_or_else(|| ext_of(path));
    let (width, height) = reader.into_dimensions().map_err(|e| e.to_string())?;

    Ok(ImageProbe {
        width,
        height,
        format,
        bytes,
    })
}

#[tauri::command]
pub fn image_convert(
    src: String,
    out_dir: Option<String>,
    options: ConvertOptions,
) -> Result<ConvertOutcome, String> {
    use crate::features;
    features::ensure_available("F4-2").map_err(|e| e.to_string())?;

    let dir = out_dir.map(PathBuf::from);
    convert(Path::new(&src), dir.as_deref(), &options).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_color_parsing() {
        assert_eq!(parse_hex_color("#FF8000"), image::Rgba([255, 128, 0, 255]));
        assert_eq!(parse_hex_color("ff8000"), image::Rgba([255, 128, 0, 255]));
        // 非法值回落为白色，避免产生意外黑底
        assert_eq!(parse_hex_color("nope"), image::Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn resize_respects_max_edge() {
        let img = DynamicImage::new_rgb8(1000, 500);
        let opts = ConvertOptions {
            max_edge: Some(500),
            ..Default::default()
        };
        let out = resize_if_needed(img, &opts);
        assert_eq!((out.width(), out.height()), (500, 250));
    }

    #[test]
    fn resize_percent() {
        let img = DynamicImage::new_rgb8(200, 100);
        let opts = ConvertOptions {
            scale_percent: Some(50.0),
            ..Default::default()
        };
        let out = resize_if_needed(img, &opts);
        assert_eq!((out.width(), out.height()), (100, 50));
    }

    #[test]
    fn no_resize_when_under_limit() {
        let img = DynamicImage::new_rgb8(100, 100);
        let opts = ConvertOptions {
            max_edge: Some(500),
            ..Default::default()
        };
        let out = resize_if_needed(img, &opts);
        assert_eq!((out.width(), out.height()), (100, 100));
    }
}
