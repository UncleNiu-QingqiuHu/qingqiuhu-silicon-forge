//! Markdown 模块：预览、导出 HTML、导出 PDF。
//!
//! HTML 导出追求「单文件拿走就能看」：样式内嵌、相对路径图片转 base64。
//! PDF 导出走 Typst，矢量输出且字体嵌入，避免客户机器缺字体导致版式错乱。

pub mod md;
pub mod world;

use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use comrak::{markdown_to_html, Options};
use serde::{Deserialize, Serialize};
use tauri::command;

use crate::features;
use md::DocOptions;
use world::ForgeWorld;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ExportOptions {
    /// html | pdf
    pub format: String,
    pub title: String,
    pub page_numbers: bool,
    pub out_dir: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExportOutcome {
    pub path: String,
    pub bytes: u64,
    pub format: String,
    pub note: Option<String>,
}

fn composer_options() -> Options<'static> {
    let mut o = Options::default();
    o.extension.table = true;
    o.extension.strikethrough = true;
    o.extension.tasklist = true;
    o.extension.footnotes = true;
    o.extension.autolink = true;
    o
}

/// 渲染 HTML 片段，供右侧预览使用（不带外壳样式）。
#[command]
pub fn md_preview(markdown: String) -> String {
    markdown_to_html(&markdown, &composer_options())
}

#[command]
pub fn md_export(
    markdown: String,
    source_path: Option<String>,
    options: ExportOptions,
) -> Result<ExportOutcome, String> {
    let fmt = if options.format.is_empty() {
        "html".to_string()
    } else {
        options.format.to_ascii_lowercase()
    };

    features::ensure_available(match fmt.as_str() {
        "pdf" => "F3-6",
        _ => "F3-5",
    })
    .map_err(|e| e.to_string())?;

    let (root, stem) = match source_path.as_deref() {
        Some(p) => {
            let path = Path::new(p);
            (
                path.parent().unwrap_or(Path::new(".")).to_path_buf(),
                path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "未命名文档".into()),
            )
        }
        None => (
            std::env::current_dir().map_err(|e| e.to_string())?,
            "未命名文档".to_string(),
        ),
    };

    let dir = options
        .out_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.clone());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let doc_opt = DocOptions {
        title: if options.title.is_empty() {
            stem.clone()
        } else {
            options.title.clone()
        },
        page_numbers: options.page_numbers,
    };

    let (out_path, note) = match fmt.as_str() {
        "pdf" => {
            let target = unique_path(&dir, &stem, "pdf");
            let bytes = render_pdf(&markdown, &doc_opt, &root)?;
            std::fs::write(&target, &bytes).map_err(|e| e.to_string())?;
            (target, None)
        }
        _ => {
            let target = unique_path(&dir, &stem, "html");
            let html = render_html(&markdown, &doc_opt, &root);
            std::fs::write(&target, html.as_bytes()).map_err(|e| e.to_string())?;
            (target, None)
        }
    };

    let bytes = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
    Ok(ExportOutcome {
        path: out_path.to_string_lossy().to_string(),
        bytes,
        format: fmt,
        note,
    })
}

fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut i = 1;
    while p.exists() {
        p = dir.join(format!("{stem}_{i}.{ext}"));
        i += 1;
    }
    p
}

fn render_pdf(markdown: &str, opt: &DocOptions, root: &Path) -> Result<Vec<u8>, String> {
    let source = md::to_typst(markdown, opt);
    let world = ForgeWorld::new(source, root.to_path_buf());

    let compiled = typst::compile::<typst_layout::PagedDocument>(&world);
    let document = compiled.output.map_err(|errs| {
        let msg = errs
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("；");
        format!("PDF 生成失败：{msg}")
    })?;

    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errs| {
        let msg = errs
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("；");
        format!("PDF 导出失败：{msg}")
    })
}

fn render_html(markdown: &str, opt: &DocOptions, root: &Path) -> String {
    let raw = markdown_to_html(markdown, &composer_options());
    let body = inline_images(&raw, root);
    let title = html_escape(&opt.title);

    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ --brand: #0d6e66; --border: #e5e7eb; }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; font-family: "Microsoft YaHei", "PingFang SC", system-ui, sans-serif;
        color: #1f2328; line-height: 1.75; background: #fff; }}
.brandbar {{ position: sticky; top: 0; background: #fff; border-bottom: 1px solid var(--border);
             padding: 10px 0; text-align: center; font-size: 12px; color: #6b7280;
             letter-spacing: .3px; z-index: 10; }}
.brandbar a {{ color: var(--brand); text-decoration: none; }}
.brandbar a:hover {{ text-decoration: underline; }}
.wrap {{ max-width: 860px; margin: 0 auto; padding: 34px 26px 90px; }}
h1, h2, h3, h4, h5, h6 {{ line-height: 1.35; margin: 1.6em 0 .6em; font-weight: 600; }}
h1 {{ font-size: 1.85em; border-bottom: 1px solid #eaecef; padding-bottom: .3em; }}
h2 {{ font-size: 1.42em; }}
h3 {{ font-size: 1.16em; }}
p {{ margin: .9em 0; }}
ul, ol {{ padding-left: 1.6em; }}
li {{ margin: .3em 0; }}
code {{ background: #f3f4f6; padding: .15em .38em; border-radius: 3px; font-size: .9em;
        font-family: ui-monospace, "Cascadia Mono", Consolas, monospace; }}
pre {{ background: #f6f8fa; padding: 14px 16px; border-radius: 6px; overflow: auto; line-height: 1.55; }}
pre code {{ background: none; padding: 0; font-size: .88em; }}
blockquote {{ margin: 1em 0; padding: .4em 1em; border-left: 3px solid #d0d7de; color: #57606a; }}
blockquote p {{ margin: .4em 0; }}
table {{ border-collapse: collapse; width: 100%; margin: 1.1em 0; font-size: .94em; }}
th, td {{ border: 1px solid #d0d7de; padding: 7px 11px; text-align: left; }}
th {{ background: #f6f8fa; font-weight: 600; }}
img {{ max-width: 100%; height: auto; }}
a {{ color: var(--brand); }}
hr {{ border: none; border-top: 1px solid var(--border); margin: 2em 0; }}
@page {{ margin: 18mm; @top-center {{ content: "青丘湖硅基工坊 www.qingqiuhu.com";
         font-size: 9pt; color: #888; }} }}
</style>
</head>
<body>
<div class="brandbar"><a href="https://www.qingqiuhu.com">青丘湖硅基工坊</a> · www.qingqiuhu.com</div>
<div class="wrap">
{body}
</div>
</body>
</html>
"#,
        title = title,
        body = body
    )
}

/// 把相对路径的图片转成 base64，保证导出的 HTML 是自包含的单文件。
fn inline_images(html: &str, root: &Path) -> String {
    let mut out = html.to_string();
    for src in collect_img_srcs(html) {
        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            continue;
        }
        let path = root.join(src.trim_start_matches("./"));
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let mime = guess_mime(&src);
        let data_uri = format!("data:{};base64,{}", mime, B64.encode(&bytes));
        out = out.replace(&format!("src=\"{}\"", src), &format!("src=\"{}\"", data_uri));
    }
    out
}

fn collect_img_srcs(html: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut rest = html;

    while let Some(pos) = rest.find("<img") {
        rest = &rest[pos..];
        if let Some(s) = rest.find("src=\"") {
            let after = &rest[s + 5..];
            if let Some(end) = after.find('"') {
                let src = after[..end].to_string();
                if !list.contains(&src) {
                    list.push(src);
                }
            }
        }
        rest = &rest[4..];
        if rest.is_empty() {
            break;
        }
    }
    list
}

fn guess_mime(p: &str) -> &'static str {
    let lower = p.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_img_sources() {
        let html = r#"<p><img src="a.png" alt=""><img src="https://x/b.jpg"></p>"#;
        let got = collect_img_srcs(html);
        assert_eq!(got, vec!["a.png", "https://x/b.jpg"]);
    }

    #[test]
    fn html_escapes_title() {
        assert_eq!(html_escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }

    #[test]
    fn mime_by_extension() {
        assert_eq!(guess_mime("x.PNG"), "image/png");
        assert_eq!(guess_mime("x.jpeg"), "image/jpeg");
        assert_eq!(guess_mime("x.bin"), "application/octet-stream");
    }

    #[test]
    fn unique_path_avoids_overwrite() {
        let dir = std::env::temp_dir().join(format!("forge_md_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.html"), "x").unwrap();
        let p = unique_path(&dir, "a", "html");
        assert_eq!(p.file_name().unwrap(), "a_1.html");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_renders_gfm_table() {
        let html = markdown_to_html("| a | b |\n|---|---|\n| 1 | 2 |", &composer_options());
        assert!(html.contains("<table>"));
    }

    /// 端到端：Markdown → Typst → PDF，验证字体加载与编译链路可用。
    #[test]
    fn pdf_renders_real_document() {
        let opt = DocOptions::default();
        let root = std::env::temp_dir();

        let md = "# 测试标题\n\n这是一段中文正文，用于验证字体嵌入与排版。\n\n- 列表项一\n- 列表项二\n\n| 名称 | 数量 |\n|---|---|\n| 甲 | 3 |\n";
        let bytes = render_pdf(md, &opt, &root).expect("PDF 生成应成功");

        assert!(bytes.starts_with(b"%PDF"), "输出应为合法 PDF 文件");
        assert!(bytes.len() > 2000, "PDF 体积异常，可能未渲染出内容");
    }
}
