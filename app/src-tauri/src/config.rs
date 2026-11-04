//! 应用配置的持久化与 schema 迁移。
//!
//! 设计依据 PRD 7.9.3：配置文件带版本号，否则老用户升级后配置错乱。
//!
//! | 情况 | 处理 |
//! |---|---|
//! | 文件不存在 | 写入默认配置 |
//! | 版本 < 程序期望 | 链式迁移，迁移前备份 `config.v{n}.bak` |
//! | 版本 == 程序期望 | 正常加载 |
//! | 版本 > 程序期望 | 拒绝启动（防降级把配置写坏） |
//! | 无法解析 | 改名 `config.corrupt.<时间戳>.json`，用默认配置继续 |

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 程序当前期望的配置版本。新增字段或改变字段语义时递增，并在 [`migrate`] 里补一步。
pub const SCHEMA_VERSION: u32 = 1;

/// 数据目录名，与 PRD 的 `%APPDATA%\QingqiuhuSiliconForge\` 一致。
pub const DATA_DIR_NAME: &str = "QingqiuhuSiliconForge";

const FILE_NAME: &str = "config.json";

/// 更新通道（PRD F8-8 灰度）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub schema_version: u32,
    /// 关闭按钮行为：true = 最小化到托盘（默认）
    pub close_to_tray: bool,
    /// 更新通道
    pub update_channel: UpdateChannel,
    /// 更新源地址；None 表示使用内置默认值
    pub update_endpoint: Option<String>,
    /// 启动时静默检查更新（PRD F8-3）
    pub check_update_on_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            close_to_tray: true,
            update_channel: UpdateChannel::Stable,
            update_endpoint: None,
            check_update_on_start: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadOutcome {
    /// 首次运行，已写入默认配置
    Fresh,
    /// 版本一致，正常加载
    Loaded,
    /// 从旧版本迁移而来，`from` 为原版本号
    Migrated { from: u32 },
    /// 文件损坏，已改名隔离
    Recovered { quarantined: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("配置由更新版本的软件创建（v{found}），当前仅支持到 v{supported}。请升级软件后再运行。")]
    FutureSchema { found: u32, supported: u32 },
    #[error("配置读写失败：{0}")]
    Io(String),
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e.to_string())
    }
}

/// 配置所在目录。测试里传入临时目录，运行期传入 `data_dir()`。
pub fn default_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DATA_DIR_NAME)
}

fn file_path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// 读取配置。所有异常路径都会落到「能启动」或「明确拒绝」两种结果之一，
/// 不存在静默使用错乱配置的情况。
pub fn load(dir: &Path) -> Result<(AppConfig, LoadOutcome), ConfigError> {
    let path = file_path(dir);
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let cfg = AppConfig::default();
            save(dir, &cfg)?;
            return Ok((cfg, LoadOutcome::Fresh));
        }
        Err(e) => return Err(ConfigError::Io(e.to_string())),
    };

    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return recover(dir, &path);
    };

    let found = value
        .get("schemaVersion")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    if found > SCHEMA_VERSION {
        return Err(ConfigError::FutureSchema {
            found,
            supported: SCHEMA_VERSION,
        });
    }

    if found < SCHEMA_VERSION {
        // 迁移前先留档，回滚时人手有据可查（PRD 7.9.4）
        let backup = dir.join(format!("config.v{found}.bak"));
        if let Err(e) = fs::copy(&path, &backup) {
            return Err(ConfigError::Io(format!("迁移前备份失败：{e}")));
        }
        migrate(&mut value, found)?;
        let cfg = match serde_json::from_value::<AppConfig>(value) {
            Ok(c) => c,
            Err(e) => return Err(ConfigError::Io(format!("迁移后配置不合法：{e}"))),
        };
        save(dir, &cfg)?;
        return Ok((cfg, LoadOutcome::Migrated { from: found }));
    }

    match serde_json::from_value::<AppConfig>(value) {
        Ok(cfg) => Ok((cfg, LoadOutcome::Loaded)),
        // 版本号对但字段类型错，说明文件被手改坏了，走隔离流程
        Err(_) => recover(dir, &path),
    }
}

/// 损坏文件隔离后用默认配置继续，而不是让程序起不来（PRD 7.9.3 最后一行）。
fn recover(dir: &Path, path: &Path) -> Result<(AppConfig, LoadOutcome), ConfigError> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let quarantined = dir.join(format!("config.corrupt.{ts}.json"));
    fs::rename(path, &quarantined)?;
    let cfg = AppConfig::default();
    save(dir, &cfg)?;
    Ok((cfg, LoadOutcome::Recovered { quarantined }))
}

/// 链式迁移：每一步只处理相邻的一级，且必须单向、可重入。
///
/// 新增版本时在末尾追加 `if v == N { ...; v = N + 1; }`，不要改已有分支。
fn migrate(value: &mut serde_json::Value, from: u32) -> Result<(), ConfigError> {
    let mut v = from;

    // v0：早期版本无配置文件（关闭行为只存内存）。若用户机器上残留无版本号的
    // 配置，按 0 处理并补齐默认值即可，没有语义变化需要搬。
    if v == 0 {
        let obj = value
            .as_object_mut()
            .ok_or_else(|| ConfigError::Io("配置根节点不是对象".into()))?;
        obj.insert("schemaVersion".into(), serde_json::json!(1));
        obj.entry("closeToTray")
            .or_insert_with(|| serde_json::json!(true));
        obj.entry("updateChannel")
            .or_insert_with(|| serde_json::json!("stable"));
        obj.entry("updateEndpoint").or_insert(serde_json::Value::Null);
        v = 1;
    }

    debug_assert_eq!(v, SCHEMA_VERSION, "迁移链没有走到当前版本");
    Ok(())
}

/// 原子写入：先写临时文件再改名，避免断电/崩溃留下半截 JSON。
pub fn save(dir: &Path, cfg: &AppConfig) -> Result<(), ConfigError> {
    fs::create_dir_all(dir)?;
    let path = file_path(dir);
    let tmp = dir.join("config.json.tmp");
    let text = serde_json::to_string_pretty(cfg)
        .map_err(|e| ConfigError::Io(format!("配置序列化失败：{e}")))?;
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Tauri 托管状态。`fatal` 非空表示配置拒绝启动，前端据此展示阻断页。
pub struct ConfigState {
    pub config: Mutex<AppConfig>,
    pub fatal: Option<String>,
}

impl ConfigState {
    pub fn from_load(result: Result<(AppConfig, LoadOutcome), ConfigError>) -> Self {
        match result {
            Ok((cfg, outcome)) => {
                match &outcome {
                    LoadOutcome::Fresh => eprintln!("[config] 首次运行，已写入默认配置"),
                    LoadOutcome::Loaded => {}
                    LoadOutcome::Migrated { from } => {
                        eprintln!("[config] 配置已从 v{from} 迁移到 v{SCHEMA_VERSION}")
                    }
                    LoadOutcome::Recovered { quarantined } => eprintln!(
                        "[config] 配置文件损坏，已隔离到 {}，使用默认配置",
                        quarantined.display()
                    ),
                }
                Self {
                    config: Mutex::new(cfg),
                    fatal: None,
                }
            }
            Err(e) => Self {
                config: Mutex::new(AppConfig::default()),
                fatal: Some(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每次用独立目录，避免并行测试互相踩。
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "qqh-cfg-test-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn fresh_dir_writes_default_config() {
        let dir = temp_dir("fresh");
        let (cfg, outcome) = load(&dir).unwrap();
        assert_eq!(outcome, LoadOutcome::Fresh);
        assert_eq!(cfg, AppConfig::default());
        assert!(file_path(&dir).exists(), "首次加载应把默认配置落盘");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_load_roundtrip_keeps_settings() {
        let dir = temp_dir("roundtrip");
        let want = AppConfig {
            close_to_tray: false,
            update_channel: UpdateChannel::Beta,
            update_endpoint: Some("https://updates.example.com/beta".into()),
            ..AppConfig::default()
        };
        save(&dir, &want).unwrap();
        let (got, outcome) = load(&dir).unwrap();
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(got, want);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_config_without_version_is_migrated_and_backed_up() {
        let dir = temp_dir("legacy");
        // v0：只有关闭行为，没有 schemaVersion
        fs::write(file_path(&dir), r#"{"closeToTray": false}"#).unwrap();

        let (cfg, outcome) = load(&dir).unwrap();
        assert_eq!(outcome, LoadOutcome::Migrated { from: 0 });
        assert_eq!(cfg.schema_version, SCHEMA_VERSION);
        // 用户原有设置必须保留，不能被默认值覆盖
        assert!(!cfg.close_to_tray);
        // 其余字段补默认
        assert_eq!(cfg.update_channel, UpdateChannel::Stable);
        assert!(dir.join("config.v0.bak").exists(), "迁移前必须备份");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_config_is_quarantined_not_fatal() {
        let dir = temp_dir("corrupt");
        fs::write(file_path(&dir), "{ this is not json").unwrap();

        let (cfg, outcome) = load(&dir).unwrap();
        assert!(matches!(outcome, LoadOutcome::Recovered { .. }));
        assert_eq!(cfg, AppConfig::default());
        let quarantined: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("config.corrupt."))
            .collect();
        assert_eq!(quarantined.len(), 1, "损坏文件应改名留档");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn future_schema_refuses_to_load() {
        let dir = temp_dir("future");
        let text = format!(
            r#"{{"schemaVersion": {}, "closeToTray": true}}"#,
            SCHEMA_VERSION + 5
        );
        fs::write(file_path(&dir), text).unwrap();

        let err = load(&dir).unwrap_err();
        assert!(matches!(err, ConfigError::FutureSchema { .. }));
        // 拒绝时不得改写用户文件
        let raw = fs::read_to_string(file_path(&dir)).unwrap();
        assert!(raw.contains(&(SCHEMA_VERSION + 5).to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrong_field_type_counts_as_corrupt() {
        let dir = temp_dir("badtype");
        // 版本号对，但 closeToTray 类型不对——手改坏了
        let text = format!(r#"{{"schemaVersion": {SCHEMA_VERSION}, "closeToTray": "yes"}}"#);
        fs::write(file_path(&dir), text).unwrap();

        let (cfg, outcome) = load(&dir).unwrap();
        assert!(matches!(outcome, LoadOutcome::Recovered { .. }));
        assert!(cfg.close_to_tray);
        let _ = fs::remove_dir_all(&dir);
    }
}
