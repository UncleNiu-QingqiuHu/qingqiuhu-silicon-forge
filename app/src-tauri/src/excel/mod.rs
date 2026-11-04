//! Excel 工作表拆分模块。
//!
//! 命令层统一把错误转成可读字符串返回前端，未排期功能在这里拦截，
//! 保证用户看到的是提示而不是崩溃（PRD 8.3）。

pub mod refs;
pub mod split;

use std::path::PathBuf;
use tauri::command;

use crate::features;
use split::{CrossRefPolicy, SplitOutcome, WorkbookScan};

#[command]
pub fn excel_scan(path: String) -> Result<WorkbookScan, String> {
    features::ensure_available("F2-1").map_err(|e| e.to_string())?;
    split::scan(&PathBuf::from(path)).map_err(|e| e.to_string())
}

#[command]
pub fn excel_split(
    path: String,
    sheets: Vec<String>,
    out_dir: Option<String>,
    policy: String,
) -> Result<Vec<SplitOutcome>, String> {
    features::ensure_available("F2-2").map_err(|e| e.to_string())?;
    let dir = out_dir.map(PathBuf::from);
    split::split(
        &PathBuf::from(path),
        &sheets,
        dir.as_deref(),
        CrossRefPolicy::parse(&policy),
    )
    .map_err(|e| e.to_string())
}
