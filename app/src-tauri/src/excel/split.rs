//! Excel 工作表拆分。
//!
//! 策略是「复制原文件 → 删掉不要的表 → 另存」，不是「读出数据 → 重新生成」。
//! 后者会丢失样式、合并单元格、条件格式、图表锚点等，拆出来的文件不能直接用。
//!
//! 读取只做一次，之后靠 `Workbook: Clone` 派生，避免 N 个表读 N 次文件
//! （PRD 5.2 要求 20 个工作表拆分 ≤ 8 秒）。

use std::path::{Path, PathBuf};

use serde::Serialize;
use umya_spreadsheet::reader::xlsx;
use umya_spreadsheet::writer::xlsx as xlsx_writer;
use umya_spreadsheet::{Workbook, Worksheet};

use super::refs::referenced_sheets;

/// 单条跨表引用的明细，用于让用户看见「哪个单元格引用了哪张表」。
#[derive(Debug, Serialize, Clone)]
pub struct CrossRef {
    pub cell: String,
    pub formula: String,
    pub targets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SheetInfo {
    pub name: String,
    /// visible / hidden / veryHidden
    pub state: String,
    pub rows: u32,
    pub cols: u32,
    pub has_chart: bool,
    /// 本表引用了其他表的位置
    pub cross_refs: Vec<CrossRef>,
    /// cross_refs 是否因数量过多被截断
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkbookScan {
    pub path: String,
    pub sheet_count: usize,
    pub sheets: Vec<SheetInfo>,
    /// 定义名称（Named Range）中的跨表引用
    pub defined_name_refs: Vec<CrossRef>,
}

#[derive(Debug, Serialize)]
pub struct SplitOutcome {
    pub sheet: String,
    pub path: String,
    pub bytes: u64,
}

/// 遇到跨表引用时的处理策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossRefPolicy {
    /// 保留公式，拆分后可能变成 #REF!
    Keep,
    /// 用缓存值替换公式（文件需曾被 Excel/WPS 保存过才有缓存值）
    ToValue,
    /// 中止，不产出任何文件
    Abort,
}

impl CrossRefPolicy {
    pub fn parse(s: &str) -> Self {
        match s {
            "to_value" => Self::ToValue,
            "abort" => Self::Abort,
            _ => Self::Keep,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExcelError {
    #[error("无法读取工作簿（可能受密码保护、文件已损坏，或不是有效的 xlsx）：{0}")]
    Read(String),
    #[error("无法写入工作簿：{0}")]
    Write(String),
    #[error("工作簿中没有名为「{0}」的工作表")]
    SheetNotFound(String),
    #[error("工作表「{0}」存在跨表引用，已按策略中止")]
    Aborted(String),
    #[error("文件系统错误：{0}")]
    Io(String),
    #[error("该文件不是 xlsx 格式（.xls 需先另存为 .xlsx）")]
    NotXlsx,
}

impl Serialize for ExcelError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

const MAX_REFS_PER_SHEET: usize = 200;

fn is_xlsx(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xlsx") || e.eq_ignore_ascii_case("xlsm"))
        .unwrap_or(false)
}

/// 扫描工作簿：列出工作表及各自的跨表引用情况。
pub fn scan(path: &Path) -> Result<WorkbookScan, ExcelError> {
    if !is_xlsx(path) {
        return Err(ExcelError::NotXlsx);
    }
    let book = xlsx::read(path).map_err(|e| ExcelError::Read(e.to_string()))?;

    let names: Vec<String> = book
        .sheet_collection()
        .iter()
        .map(|s| s.name().to_string())
        .collect();

    let mut sheets = Vec::with_capacity(names.len());
    for ws in book.sheet_collection() {
        let own = ws.name().to_string();
        let mut cross_refs = Vec::new();
        let mut truncated = false;

        for cell in ws.cells() {
            if !cell.is_formula() {
                continue;
            }
            let formula = cell.formula();
            let targets = external_targets(formula, &own, &names);
            if targets.is_empty() {
                continue;
            }
            if cross_refs.len() >= MAX_REFS_PER_SHEET {
                truncated = true;
                break;
            }
            cross_refs.push(CrossRef {
                cell: cell.coordinate().get_coordinate(),
                formula: formula.to_string(),
                targets,
            });
        }

        sheets.push(SheetInfo {
            name: own,
            state: ws.sheet_state().to_string(),
            rows: ws.highest_row(),
            cols: ws.highest_column(),
            has_chart: !ws.chart_collection().is_empty(),
            cross_refs,
            truncated,
        });
    }

    let mut defined_name_refs = Vec::new();
    for dn in book.defined_names() {
        let addr = dn.address();
        let targets: Vec<String> = referenced_sheets(&addr)
            .into_iter()
            .filter(|t| names.iter().any(|n| n == t))
            .collect();
        if !targets.is_empty() {
            defined_name_refs.push(CrossRef {
                cell: dn.name().to_string(),
                formula: addr,
                targets,
            });
        }
    }

    Ok(WorkbookScan {
        path: path.to_string_lossy().to_string(),
        sheet_count: names.len(),
        sheets,
        defined_name_refs,
    })
}

/// 公式中指向「其他」工作表的引用，只保留确实存在于本工作簿的表名。
fn external_targets(formula: &str, own: &str, names: &[String]) -> Vec<String> {
    referenced_sheets(formula)
        .into_iter()
        .filter(|t| t != own)
        .filter(|t| names.iter().any(|n| n == t))
        .collect()
}

/// 把跨表引用的公式替换为当前缓存值。
///
/// 缓存值来自文件里已有的计算结果；若文件从未被 Excel/WPS 保存过，
/// 缓存为空，替换后单元格会变空——这一点由调用方在 UI 上提示用户。
fn freeze_cross_refs(ws: &mut Worksheet, names: &[String], own: &str) {
    let coords: Vec<String> = ws
        .cells()
        .iter()
        .filter(|c| c.is_formula())
        .filter(|c| !external_targets(c.formula(), own, names).is_empty())
        .map(|c| c.coordinate().get_coordinate())
        .collect();

    for coord in coords {
        let cached = match ws.cell(coord.as_str()) {
            Some(c) => c.value().to_string(),
            None => continue,
        };
        let cell = ws.cell_mut(coord.as_str());
        cell.set_formula("");
        cell.set_value(cached);
    }
}

/// 执行拆分。
pub fn split(
    src: &Path,
    targets: &[String],
    out_dir: Option<&Path>,
    policy: CrossRefPolicy,
) -> Result<Vec<SplitOutcome>, ExcelError> {
    if !is_xlsx(src) {
        return Err(ExcelError::NotXlsx);
    }
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    let base = xlsx::read(src).map_err(|e| ExcelError::Read(e.to_string()))?;
    let names: Vec<String> = base
        .sheet_collection()
        .iter()
        .map(|s| s.name().to_string())
        .collect();

    for t in targets {
        if !names.iter().any(|n| n == t) {
            return Err(ExcelError::SheetNotFound(t.clone()));
        }
    }

    // 中止策略：任何一个目标表存在跨表引用就先拒绝，不产出半成品
    if policy == CrossRefPolicy::Abort {
        for t in targets {
            let has = base
                .sheet_by_name(t)
                .map(|ws| {
                    ws.cells()
                        .iter()
                        .any(|c| c.is_formula() && !external_targets(c.formula(), t, &names).is_empty())
                })
                .unwrap_or(false);
            if has {
                return Err(ExcelError::Aborted(t.clone()));
            }
        }
    }

    let dir: PathBuf = match out_dir {
        Some(d) => d.to_path_buf(),
        None => src.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };
    std::fs::create_dir_all(&dir).map_err(|e| ExcelError::Io(e.to_string()))?;

    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "workbook".to_string());

    let mut results = Vec::with_capacity(targets.len());

    for target in targets {
        let mut book: Workbook = base.clone();

        for name in &names {
            if name != target {
                let _ = book.remove_sheet_by_name(name);
            }
        }

        if let Ok(ws) = book.sheet_by_name_mut(target) {
            // 拆出来的表必须可见，否则用户打开文件看不到内容
            if ws.sheet_state() != "visible" {
                ws.set_sheet_state("visible".to_string());
            }
            if policy == CrossRefPolicy::ToValue {
                let own = target.clone();
                freeze_cross_refs(ws, &names, &own);
            }
        }

        let _ = book.set_active_sheet(0);

        let file_name = format!("{}_{}.xlsx", sanitize(&stem), sanitize(target));
        let out_path = dir.join(&file_name);
        xlsx_writer::write(&book, &out_path).map_err(|e| ExcelError::Write(e.to_string()))?;

        let bytes = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        results.push(SplitOutcome {
            sheet: target.clone(),
            path: out_path.to_string_lossy().to_string(),
            bytes,
        });
    }

    Ok(results)
}

/// 清理文件名中的非法字符，并限制长度。
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '[' | ']' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    let limited: String = trimmed.chars().take(80).collect();
    if limited.is_empty() {
        "sheet".to_string()
    } else {
        limited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_illegal_chars() {
        assert_eq!(sanitize("a/b:c*d?e"), "a_b_c_d_e");
        assert_eq!(sanitize("[汇总]"), "_汇总_");
        assert_eq!(sanitize("  "), "sheet");
        assert_eq!(sanitize("正常名称"), "正常名称");
    }

    #[test]
    fn external_targets_filters_self_and_unknown() {
        let names = vec!["Sheet1".to_string(), "Sheet2".to_string()];
        assert_eq!(
            external_targets("Sheet2!A1+Sheet1!B1+Other!C1", "Sheet1", &names),
            vec!["Sheet2"]
        );
        assert!(external_targets("A1+B1", "Sheet1", &names).is_empty());
    }

    #[test]
    fn policy_parsing() {
        assert_eq!(CrossRefPolicy::parse("abort"), CrossRefPolicy::Abort);
        assert_eq!(CrossRefPolicy::parse("to_value"), CrossRefPolicy::ToValue);
        assert_eq!(CrossRefPolicy::parse("keep"), CrossRefPolicy::Keep);
        assert_eq!(CrossRefPolicy::parse("whatever"), CrossRefPolicy::Keep);
    }

    /// 端到端：构造一个含跨表引用的工作簿，扫描 → 拆分 → 回读校验。
    #[test]
    fn end_to_end_scan_and_split() {
        use umya_spreadsheet::new_file;

        let dir = std::env::temp_dir().join(format!("forge_e2e_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("book.xlsx");

        let mut book = new_file();
        if let Ok(ws) = book.get_sheet_mut(&0) {
            ws.set_name("汇总");
            ws.cell_mut("A1").set_formula("明细1!A1");
            ws.cell_mut("B1").set_value("备注");
        }
        book.new_sheet("明细1")
            .unwrap()
            .cell_mut("A1")
            .set_value_number(100.0);
        book.new_sheet("明细2")
            .unwrap()
            .cell_mut("A1")
            .set_value_number(200.0);
        xlsx_writer::write(&book, &src).unwrap();

        let s = scan(&src).unwrap();
        assert_eq!(s.sheet_count, 3);
        let hz = s.sheets.iter().find(|x| x.name == "汇总").unwrap();
        assert_eq!(hz.cross_refs.len(), 1, "汇总表应检测到 1 处跨表引用");
        assert_eq!(hz.cross_refs[0].cell, "A1");
        assert_eq!(hz.cross_refs[0].targets, vec!["明细1"]);

        let detail = s.sheets.iter().find(|x| x.name == "明细1").unwrap();
        assert!(detail.cross_refs.is_empty(), "明细表不应有跨表引用");

        let out = split(
            &src,
            &["汇总".to_string(), "明细2".to_string()],
            Some(&dir),
            CrossRefPolicy::Keep,
        )
        .unwrap();
        assert_eq!(out.len(), 2);

        for o in &out {
            assert!(Path::new(&o.path).exists());
            let b = xlsx::read(&o.path).unwrap();
            assert_eq!(
                b.sheet_collection().len(),
                1,
                "拆出的文件应只含一个工作表：{}",
                o.sheet
            );
            assert_eq!(b.sheet_collection()[0].name(), o.sheet);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 中止策略：存在跨表引用时不产出任何文件。
    #[test]
    fn abort_policy_rejects_cross_ref_sheet() {
        use umya_spreadsheet::new_file;

        let dir = std::env::temp_dir().join(format!("forge_abort_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("book.xlsx");

        let mut book = new_file();
        if let Ok(ws) = book.get_sheet_mut(&0) {
            ws.set_name("汇总");
            ws.cell_mut("A1").set_formula("明细!A1");
        }
        book.new_sheet("明细")
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1.0);
        xlsx_writer::write(&book, &src).unwrap();

        let res = split(&src, &["汇总".to_string()], Some(&dir), CrossRefPolicy::Abort);
        assert!(res.is_err(), "中止策略应拒绝含跨表引用的表");

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("book_"))
            .collect();
        assert!(leftovers.is_empty(), "中止时不应产出半成品文件");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_sheet_reports_clearly() {
        use umya_spreadsheet::new_file;

        let dir = std::env::temp_dir().join(format!("forge_missing_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("book.xlsx");
        let book = new_file();
        xlsx_writer::write(&book, &src).unwrap();

        let res = split(&src, &["不存在".to_string()], Some(&dir), CrossRefPolicy::Keep);
        match res {
            Err(ExcelError::SheetNotFound(n)) => assert_eq!(n, "不存在"),
            other => panic!("期望 SheetNotFound，实际 {:?}", other.map(|v| v.len())),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
