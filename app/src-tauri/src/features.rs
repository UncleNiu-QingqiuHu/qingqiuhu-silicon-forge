//! 功能状态注册表。
//!
//! 状态源自 `features.toml`，由 `build.rs` 在编译期生成为静态表。
//! 任何功能是否可用都必须经由此处判断，禁止在业务代码里硬编码。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Committed,
    Conditional,
    Deferred,
    Out,
}

pub struct FeatureMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub module: &'static str,
    pub status: Status,
    pub milestone: Option<&'static str>,
    pub placeholder: Option<&'static str>,
    pub blocked_by: &'static [&'static str],
    /// 是否已在本版本交付。committed 但未交付 = 开发中，界面应显示进度而非入口。
    pub delivered: bool,
}

include!(concat!(env!("OUT_DIR"), "/features_generated.rs"));

#[derive(Debug, thiserror::Error)]
pub enum FeatureError {
    #[error("{feature} 计划于 {milestone} 提供")]
    NotPlanned { feature: String, milestone: String },
    #[error("{feature} 暂不提供")]
    Deferred { feature: String },
    #[error("未知功能标识：{0}")]
    Unknown(String),
}

impl Serialize for FeatureError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub fn find(id: &str) -> Option<&'static FeatureMeta> {
    FEATURES.iter().find(|f| f.id == id)
}

/// 供命令入口调用：未排期功能在这里统一拦截，返回可读提示而非 panic。
pub fn ensure_available(id: &str) -> Result<&'static FeatureMeta, FeatureError> {
    match find(id) {
        None => Err(FeatureError::Unknown(id.to_string())),
        Some(f) => match f.status {
            Status::Committed => Ok(f),
            Status::Conditional => Err(FeatureError::NotPlanned {
                feature: f.name.to_string(),
                milestone: f.milestone.unwrap_or("后续版本").to_string(),
            }),
            _ => Err(FeatureError::Deferred {
                feature: f.name.to_string(),
            }),
        },
    }
}

#[derive(Serialize)]
pub struct FeatureView {
    pub id: &'static str,
    pub name: &'static str,
    pub module: &'static str,
    pub status: Status,
    pub milestone: Option<&'static str>,
    pub placeholder: Option<&'static str>,
    pub blocked_by: &'static [&'static str],
    pub delivered: bool,
    pub available: bool,
}

#[tauri::command]
pub fn get_features() -> Vec<FeatureView> {
    FEATURES
        .iter()
        .map(|f| FeatureView {
            id: f.id,
            name: f.name,
            module: f.module,
            status: f.status,
            milestone: f.milestone,
            placeholder: f.placeholder,
            blocked_by: f.blocked_by,
            delivered: f.delivered,
            available: f.status == Status::Committed,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in FEATURES {
            assert!(seen.insert(f.id), "功能 ID 重复: {}", f.id);
        }
    }

    #[test]
    fn non_committed_must_have_placeholder() {
        for f in FEATURES {
            if f.status != Status::Committed {
                assert!(
                    f.placeholder.is_some(),
                    "功能 {} 未排期但缺少 placeholder 文案",
                    f.id
                );
            }
        }
    }

    #[test]
    fn conditional_must_declare_blockers() {
        for f in FEATURES {
            if f.status == Status::Conditional {
                assert!(!f.blocked_by.is_empty(), "条件性功能 {} 未声明前置", f.id);
            }
        }
    }
}
