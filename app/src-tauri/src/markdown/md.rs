//! Markdown → Typst 标记转换。
//!
//! 走 comrak 的 AST 而不是正则替换：正则处理嵌套列表、表格、行内代码
//! 这类结构时会出错，而 AST 本身已经解析好了结构。
//!
//! 页眉与页码在 preamble 里一次性设定（PRD F3-7 / F3-8）。

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::{parse_document, Arena, Options};

pub const BRAND: &str = "青丘湖硅基工坊 www.qingqiuhu.com";

#[derive(Debug, Clone)]
pub struct DocOptions {
    /// 文档标题（写入 PDF 元信息，不显示在正文）
    pub title: String,
    /// 是否显示页码
    pub page_numbers: bool,
}

impl Default for DocOptions {
    fn default() -> Self {
        Self {
            title: "文档".into(),
            page_numbers: true,
        }
    }
}

/// 生成完整的 typst 源文件。
pub fn to_typst(markdown: &str, opt: &DocOptions) -> String {
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.footnotes = true;
    opts.extension.autolink = true;
    opts.render.hardbreaks = false;

    let arena = Arena::new();
    let root = parse_document(&arena, markdown, &opts);

    let mut out = String::with_capacity(markdown.len() * 2);
    out.push_str(&preamble(opt));

    for node in root.children() {
        render(node, &mut out, 0);
    }

    out
}

fn preamble(opt: &DocOptions) -> String {
    let numbering = if opt.page_numbers { "\"1\"" } else { "none" };
    let title = escape(&opt.title);

    format!(
        r##"#set document(title: "{title}")
#set page(
  paper: "a4",
  margin: (x: 2.2cm, top: 2.5cm, bottom: 2.2cm),
  header: align(center, text(size: 8.5pt, fill: luma(120))[{brand}]),
  header-ascent: 1.2em,
  numbering: {numbering},
  number-align: center,
)
#set text(font: ("Microsoft YaHei", "SimSun", "Arial"), size: 10.5pt, lang: "zh", region: "cn")
#set par(leading: 0.78em, spacing: 1.05em, justify: true)
#set heading(numbering: none)
#show heading.where(level: 1): it => {{ v(0.6em); it; v(0.4em) }}
#show heading.where(level: 2): it => {{ v(0.5em); it; v(0.3em) }}
#show link: it => text(fill: rgb("#0d6e66"), it)
#set table(stroke: 0.5pt + luma(180), inset: 6pt)

"##,
        title = title,
        brand = BRAND,
        numbering = numbering
    )
}

fn render<'a>(node: &'a AstNode<'a>, out: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);

    match &node.data.borrow().value {
        NodeValue::Document => {
            for child in node.children() {
                render(child, out, depth);
            }
        }

        NodeValue::Heading(h) => {
            let marks = "=".repeat(h.level.clamp(1, 6) as usize);
            // typst 标题附带的空格会被吞掉，显式保留
            out.push_str(&format!("{} {} ", marks, inline(node)));
            out.push_str("\n\n");
        }

        NodeValue::Paragraph => {
            out.push_str(&indent);
            out.push_str(&inline(node));
            out.push_str("\n\n");
        }

        NodeValue::BlockQuote => {
            out.push_str("#quote(block: true, attribution: none)[\n");
            for child in node.children() {
                let mut inner = String::new();
                render(child, &mut inner, 0);
                out.push_str(&inner);
            }
            out.push_str("]\n\n");
        }

        NodeValue::List(list) => {
            let ordered = matches!(list.list_type, ListType::Ordered);
            for item in node.children() {
                out.push_str(&indent);
                out.push_str(if ordered { "+ " } else { "- " });
                render_item(item, out, depth + 1);
            }
            if depth == 0 {
                out.push('\n');
            }
        }

        NodeValue::CodeBlock(cb) => {
            let lang = cb.info.split_whitespace().next().unwrap_or("");
            let fence = "```";
            out.push_str(&format!("{}{}\n", fence, lang));
            out.push_str(&cb.literal);
            if !cb.literal.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("{}\n\n", fence));
        }

        NodeValue::Table(_) => render_table(node, out),

        NodeValue::ThematicBreak => {
            out.push_str("#line(length: 100%, stroke: 0.5pt + luma(200))\n\n");
        }

        NodeValue::HtmlBlock(_) => { /* HTML 块在 PDF 中忽略 */ }

        NodeValue::FootnoteDefinition(_) => { /* M2 暂不支持脚注正文 */ }

        _ => {
            // 其余节点按行内内容处理
            let text = inline(node);
            if !text.trim().is_empty() {
                out.push_str(&indent);
                out.push_str(&text);
                out.push_str("\n\n");
            }
        }
    }
}

fn render_item<'a>(item: &'a AstNode<'a>, out: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);
    let mut first = true;

    for child in item.children() {
        match &child.data.borrow().value {
            NodeValue::Paragraph => {
                if !first {
                    out.push('\n');
                    out.push_str(&indent);
                }
                out.push_str(&inline(child));
                out.push('\n');
                first = false;
            }
            NodeValue::List(_) => {
                render(child, out, depth);
            }
            _ => {
                let mut extra = String::new();
                render(child, &mut extra, depth);
                out.push_str(&extra);
            }
        }
    }

    if first {
        out.push('\n');
    }
}

fn render_table<'a>(node: &'a AstNode<'a>, out: &mut String) {
    let mut rows: Vec<Vec<String>> = Vec::new();

    for row in node.children() {
        let mut cells = Vec::new();
        for cell in row.children() {
            cells.push(inline(cell));
        }
        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return;
    }

    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    out.push_str(&format!("#table(\n  columns: {},\n", cols));
    for (i, row) in rows.iter().enumerate() {
        out.push_str("  ");
        for c in 0..cols {
            let content = row.get(c).cloned().unwrap_or_default();
            if i == 0 {
                out.push_str(&format!("[*{}*], ", content));
            } else {
                out.push_str(&format!("[{}], ", content));
            }
        }
        out.push('\n');
    }
    out.push_str(")\n\n");
}

/// 渲染一个节点的行内内容（不含块级分隔）。
fn inline<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    inline_into(node, &mut out);
    out
}

fn inline_into<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Text(t) => out.push_str(&escape(t)),
            NodeValue::Code(c) => {
                out.push('`');
                out.push_str(&c.literal);
                out.push('`');
            }
            NodeValue::Strong => {
                out.push('*');
                inline_into(child, out);
                out.push('*');
            }
            NodeValue::Emph => {
                out.push('_');
                inline_into(child, out);
                out.push('_');
            }
            NodeValue::Strikethrough => {
                out.push_str("#strike[");
                inline_into(child, out);
                out.push(']');
            }
            NodeValue::Link(link) => {
                out.push_str(&format!("#link(\"{}\")[", escape_url(&link.url)));
                inline_into(child, out);
                out.push(']');
            }
            NodeValue::Image(img) => {
                let path = escape_url(&img.url);
                out.push_str(&format!("#image(\"{}\", width: 100%)", path));
            }
            NodeValue::SoftBreak => out.push(' '),
            NodeValue::LineBreak => out.push_str(" \\\n"),
            NodeValue::Escaped | NodeValue::EscapedTag(_) => {}
            NodeValue::HtmlInline(_) => {}
            NodeValue::FootnoteReference(r) => {
                out.push_str(&format!("[{}]", escape(&r.name)));
            }
            _ => inline_into(child, out),
        }
    }
}

/// 转义 Typst 标记里有特殊含义的字符。
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '#' | '*' | '_' | '`' | '$' | '@' | '<' | '>' | '\\' | '~' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn escape_url(s: &str) -> String {
    s.replace('\\', "/").replace('"', "%22")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typst(md: &str) -> String {
        to_typst(md, &DocOptions::default())
    }

    #[test]
    fn header_contains_brand() {
        let out = typst("# 标题");
        assert!(out.contains("青丘湖硅基工坊 www.qingqiuhu.com"));
    }

    #[test]
    fn headings_map_to_equals() {
        let out = typst("# 一级\n\n## 二级\n\n### 三级");
        assert!(out.contains("= 一级"));
        assert!(out.contains("== 二级"));
        assert!(out.contains("=== 三级"));
    }

    #[test]
    fn emphasis_and_strong() {
        let out = typst("这是**粗体**和*斜体*");
        assert!(out.contains("*粗体*"));
        assert!(out.contains("_斜体_"));
    }

    #[test]
    fn escapes_special_chars() {
        let out = typst("价格是 $100 #标签");
        assert!(out.contains(r"\$100"));
        assert!(out.contains(r"\#标签"));
    }

    #[test]
    fn code_block_preserved() {
        let out = typst("```rust\nfn main() {}\n```");
        assert!(out.contains("```rust"));
        assert!(out.contains("fn main() {}"));
    }

    #[test]
    fn inline_code_not_escaped() {
        let out = typst("使用 `a_b` 变量");
        assert!(out.contains("`a_b`"));
    }

    #[test]
    fn unordered_and_ordered_lists() {
        let a = typst("- 甲\n- 乙");
        assert!(a.contains("- 甲"));
        assert!(a.contains("- 乙"));
        let b = typst("1. 一\n2. 二");
        assert!(b.contains("+ 一"));
    }

    #[test]
    fn link_and_image() {
        let out = typst("[站点](https://example.com) ![图](img/a.png)");
        assert!(out.contains("#link(\"https://example.com\")"));
        assert!(out.contains("#image(\"img/a.png\""));
    }

    #[test]
    fn table_generates_typst_table() {
        let out = typst("| 名称 | 数量 |\n|---|---|\n| 甲 | 3 |");
        assert!(out.contains("#table("));
        assert!(out.contains("columns: 2"));
    }

    #[test]
    fn page_numbers_toggle() {
        let mut opt = DocOptions::default();
        opt.page_numbers = false;
        let out = to_typst("文本", &opt);
        assert!(out.contains("numbering: none"));
    }
}
