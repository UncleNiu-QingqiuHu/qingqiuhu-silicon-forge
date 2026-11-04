//! 跨表引用检测。
//!
//! 拆分工作表后，引用了其他表的公式会断裂成 `#REF!`。这类问题如果静默发生，
//! 用户往往要等到把文件发给别人、对方打开发现数据是错的才会暴露，
//! 所以必须在拆分前扫描出来并让用户决定怎么处理（PRD F2-5）。
//!
//! 这里只做纯文本层面的公式解析：不依赖 Excel 引擎，也不计算公式。
//! 能覆盖 `Sheet2!A1`、`'My Sheet'!A1`、`Sheet1:Sheet3!A1` 三种写法，
//! 并跳过字符串字面量中的叹号（如 `="注意!"`）。

/// 从单条公式中提取被引用的工作表名（去重，保持出现顺序）。
pub fn referenced_sheets(formula: &str) -> Vec<String> {
    let chars: Vec<char> = formula.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    let mut in_string = false;

    while i < chars.len() {
        let c = chars[i];

        // Excel 字符串字面量用 "" 转义内部双引号
        if c == '"' {
            if in_string && chars.get(i + 1) == Some(&'"') {
                i += 2;
                continue;
            }
            in_string = !in_string;
            i += 1;
            continue;
        }

        if in_string || c != '!' {
            i += 1;
            continue;
        }

        if i > 0 {
            let names = if chars[i - 1] == '\'' {
                scan_quoted(&chars, i - 1).into_iter().collect()
            } else {
                scan_bare(&chars, i)
            };
            for n in names {
                if !n.is_empty() && !out.contains(&n) {
                    out.push(n);
                }
            }
        }
        i += 1;
    }

    out
}

/// 解析 `'name'!` 形式，`close_quote` 指向闭合引号。
fn scan_quoted(chars: &[char], close_quote: usize) -> Option<String> {
    let mut parts: Vec<char> = Vec::new();
    let mut k = close_quote;

    while k > 0 {
        k -= 1;
        if chars[k] == '\'' {
            if k > 0 && chars[k - 1] == '\'' {
                parts.push('\'');
                k -= 1;
                continue;
            }
            parts.reverse();
            return Some(parts.into_iter().collect());
        }
        parts.push(chars[k]);
    }
    None
}

/// 解析 `Sheet!`、`Sheet1:Sheet3!` 形式。冒号表示三维引用，两端都算。
fn scan_bare(chars: &[char], bang: usize) -> Vec<String> {
    let mut names = Vec::new();
    let mut k = bang;

    loop {
        let mut start = k;
        while start > 0 {
            let c = chars[start - 1];
            if c.is_alphanumeric() || c == '_' || c == '.' {
                start -= 1;
            } else {
                break;
            }
        }
        if start == k {
            break;
        }
        let name: String = chars[start..k].iter().collect();
        // 以数字开头的必然是单元格地址（如 A1!），不是表名
        let starts_with_digit = name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(true);
        if !starts_with_digit {
            names.push(name);
        }
        if start > 0 && chars[start - 1] == ':' {
            k = start - 1;
            continue;
        }
        break;
    }

    names.reverse();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(f: &str) -> Vec<String> {
        referenced_sheets(f)
    }

    #[test]
    fn plain_sheet_ref() {
        assert_eq!(refs("Sheet2!A1"), vec!["Sheet2"]);
        assert_eq!(refs("SUM(Sheet2!A1:B10)"), vec!["Sheet2"]);
        assert_eq!(refs("Sheet1!A1+Sheet2!B2"), vec!["Sheet1", "Sheet2"]);
    }

    #[test]
    fn quoted_sheet_ref() {
        assert_eq!(refs("'My Sheet'!A1"), vec!["My Sheet"]);
        assert_eq!(refs("SUM('2024 数据'!A1:A9)"), vec!["2024 数据"]);
        assert_eq!(refs("'it''s'!A1"), vec!["it's"]);
    }

    #[test]
    fn range_3d_ref() {
        assert_eq!(refs("SUM(Sheet1:Sheet3!A1)"), vec!["Sheet1", "Sheet3"]);
    }

    #[test]
    fn chinese_sheet_ref() {
        assert_eq!(refs("销售表!B2"), vec!["销售表"]);
        assert_eq!(refs("'一季度 汇总'!C3"), vec!["一季度 汇总"]);
    }

    #[test]
    fn ignores_bang_in_string_literal() {
        assert_eq!(refs(r#"="注意!"&A1"#), Vec::<String>::new());
        assert_eq!(refs(r#"="他说""好!""了"&B1"#), Vec::<String>::new());
    }

    #[test]
    fn no_ref_at_all() {
        assert_eq!(refs("A1+B1*2"), Vec::<String>::new());
        assert_eq!(refs("SUM(A1:A10)"), Vec::<String>::new());
        assert_eq!(refs(""), Vec::<String>::new());
    }

    #[test]
    fn dedups_repeated_refs() {
        assert_eq!(refs("Sheet2!A1+Sheet2!B2+Sheet2!C3"), vec!["Sheet2"]);
    }
}
