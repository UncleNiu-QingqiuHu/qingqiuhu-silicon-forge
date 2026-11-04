//! 更新检查与安装（PRD F8-3 / F8-4 / F8-5 / F8-8）。
//!
//! 更新源可在设置里改（F8-5），未配置时按通道取内置默认值。
//! 下载包的完整性由 minisign 校验（F8-4），公钥写在 tauri.conf.json 的
//! `plugins.updater.pubkey`，校验在插件内部完成，签名不符会直接失败。

use serde::Serialize;
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::config::{AppConfig, ConfigState, UpdateChannel};

/// 内置默认更新源。内网实际部署地址待定（PRD Q-03），
/// 未配置前这两个地址只作占位，检查更新会失败并如实提示。
const DEFAULT_STABLE: &str = "https://updates.qingqiuhu.com/forge/stable/latest.json";
const DEFAULT_BETA: &str = "https://updates.qingqiuhu.com/forge/beta/latest.json";

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
    /// 本次实际使用的更新源，便于用户核对「到底连的哪个地址」
    pub endpoint: String,
}

fn read_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    app.state::<ConfigState>()
        .config
        .lock()
        .map(|c| c.clone())
        .map_err(|e| e.to_string())
}

fn endpoint_of(cfg: &AppConfig) -> String {
    match &cfg.update_endpoint {
        Some(url) if !url.trim().is_empty() => url.trim().to_string(),
        _ => match cfg.update_channel {
            UpdateChannel::Stable => DEFAULT_STABLE.to_string(),
            UpdateChannel::Beta => DEFAULT_BETA.to_string(),
        },
    }
}

fn build_updater(
    app: &tauri::AppHandle,
    endpoint: &str,
) -> Result<tauri_plugin_updater::Updater, String> {
    let url = tauri::Url::parse(endpoint).map_err(|e| format!("更新源地址无效：{e}"))?;
    app.updater_builder()
        .endpoints(vec![url])
        .map_err(|e| format!("更新源配置有误：{e}"))?
        .build()
        .map_err(|e| format!("初始化更新器失败：{e}"))
}

/// 检查更新。返回结构里始终带 `endpoint`，失败时用户能自己核对地址。
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let cfg = read_config(&app)?;
    let endpoint = endpoint_of(&cfg);
    let updater = build_updater(&app, &endpoint)?;

    let current = app.package_info().version.to_string();
    let found = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败（{endpoint}）：{e}"))?;

    Ok(match found {
        Some(u) => UpdateInfo {
            available: true,
            current_version: u.current_version.clone(),
            version: Some(u.version.clone()),
            notes: u.body.clone(),
            date: u.date.map(|d| d.to_string()),
            endpoint,
        },
        None => UpdateInfo {
            available: false,
            current_version: current,
            version: None,
            notes: None,
            date: None,
            endpoint,
        },
    })
}

#[derive(Clone, Serialize)]
struct Progress {
    downloaded: usize,
    total: Option<u64>,
}

/// 下载并安装更新。Windows 上安装程序启动后本进程会退出，属预期行为。
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let cfg = read_config(&app)?;
    let endpoint = endpoint_of(&cfg);
    let updater = build_updater(&app, &endpoint)?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?
        .ok_or_else(|| "当前已是最新版本".to_string())?;

    let emitter = app.clone();
    update
        .download_and_install(
            move |downloaded, total| {
                let _ = emitter.emit("forge://update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| format!("安装更新失败：{e}"))?;
    Ok(())
}

/// 切换灰度通道（F8-8）。
#[tauri::command]
pub fn set_update_channel(app: tauri::AppHandle, channel: UpdateChannel) -> Result<(), String> {
    let state = app.state::<ConfigState>();
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.update_channel = channel;
    crate::config::save(&crate::config::default_dir(), &cfg).map_err(|e| e.to_string())
}

/// 设置自定义更新源；传空字符串恢复默认。
#[tauri::command]
pub fn set_update_endpoint(app: tauri::AppHandle, endpoint: String) -> Result<(), String> {
    let state = app.state::<ConfigState>();
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    let trimmed = endpoint.trim();
    cfg.update_endpoint = if trimmed.is_empty() {
        None
    } else {
        // 提前校验，避免用户填错地址后到检查更新时才报错
        tauri::Url::parse(trimmed).map_err(|e| format!("地址格式不正确：{e}"))?;
        Some(trimmed.to_string())
    };
    crate::config::save(&crate::config::default_dir(), &cfg).map_err(|e| e.to_string())
}

/// 启动时是否自动检查更新（F8-3）。
#[tauri::command]
pub fn set_check_on_start(app: tauri::AppHandle, enable: bool) -> Result<(), String> {
    let state = app.state::<ConfigState>();
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.check_update_on_start = enable;
    crate::config::save(&crate::config::default_dir(), &cfg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with(channel: UpdateChannel, endpoint: Option<&str>) -> AppConfig {
        AppConfig {
            update_channel: channel,
            update_endpoint: endpoint.map(|s| s.to_string()),
            ..AppConfig::default()
        }
    }

    #[test]
    fn endpoint_falls_back_to_channel_default() {
        assert_eq!(endpoint_of(&cfg_with(UpdateChannel::Stable, None)), DEFAULT_STABLE);
        assert_eq!(endpoint_of(&cfg_with(UpdateChannel::Beta, None)), DEFAULT_BETA);
    }

    #[test]
    fn custom_endpoint_wins_and_is_trimmed() {
        let cfg = cfg_with(UpdateChannel::Beta, Some("  https://intranet.example.com/u.json  "));
        assert_eq!(endpoint_of(&cfg), "https://intranet.example.com/u.json");
    }

    #[test]
    fn blank_endpoint_is_treated_as_unset() {
        // 用户把地址清空后又保存，不应产生空 URL 导致检查更新直接崩
        let cfg = cfg_with(UpdateChannel::Beta, Some("   "));
        assert_eq!(endpoint_of(&cfg), DEFAULT_BETA);
    }
}
