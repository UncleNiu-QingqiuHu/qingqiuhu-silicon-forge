mod config;
mod excel;
mod features;
mod imagetool;
mod markdown;
mod tray;
mod updater;

use tauri::{Emitter, Manager, WindowEvent};

// 依赖清单在构建期从 Cargo.toml 生成，关于页直接展示，避免手写清单与真实依赖脱节。
include!(concat!(env!("OUT_DIR"), "/deps_generated.rs"));

#[derive(serde::Serialize)]
pub struct BuildInfo {
    pub version: String,
    pub build_time: &'static str,
    /// (依赖名, 版本要求)
    pub deps: &'static [(&'static str, &'static str)],
}

#[tauri::command]
fn app_build_info(app: tauri::AppHandle) -> BuildInfo {
    BuildInfo {
        version: app.package_info().version.to_string(),
        build_time: env!("BUILD_TIME"),
        deps: DEPS,
    }
}

#[tauri::command]
fn confirm_exit(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_app_config(
    state: tauri::State<config::ConfigState>,
) -> Result<config::AppConfig, String> {
    state.config.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_close_behavior(state: tauri::State<config::ConfigState>, to_tray: bool) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.close_to_tray = to_tray;
    config::save(&config::default_dir(), &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_close_behavior(state: tauri::State<config::ConfigState>) -> bool {
    state.config.lock().map(|c| c.close_to_tray).unwrap_or(true)
}

/// 配置拒绝启动时的阻断信息（PRD 7.9.3：文件版本高于程序版本）。
#[tauri::command]
fn get_config_fatal(state: tauri::State<config::ConfigState>) -> Option<String> {
    state.fatal.clone()
}

#[tauri::command]
fn hide_to_tray(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

/// 应用版本号，从构建元信息读取，避免在界面里硬编码。
#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// 读取文本文件（仅 UTF-8）。用于 Markdown 打开。
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path)
        .map_err(|e| format!("无法读取文件（仅支持 UTF-8 编码）：{e}"))
}

/// 写入文本文件。用于 Markdown 保存。
#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    if let Some(dir) = std::path::Path::new(&path).parent() {
        if !dir.exists() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(&path, content).map_err(|e| format!("无法写入文件：{e}"))
}

/// 开机自启：写在 HKCU 的 Run 键下，不需要管理员权限（PRD F6-9）。
const AUTOSTART_NAME: &str = "QingqiuhuSiliconForge";
const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
fn autostart_cmd() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    // 安装路径含空格（Program Files），必须加引号
    Ok(format!("\"{}\"", exe.to_string_lossy()))
}

#[tauri::command]
fn get_autostart() -> Result<bool, String> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(AUTOSTART_KEY, KEY_READ)
            .map_err(|e| e.to_string())?;
        let expected = autostart_cmd()?;
        match key.get_value::<String, _>(AUTOSTART_NAME) {
            // 记录的是其他路径（例如旧安装位置或开发版）时不认为已启用
            Ok(v) => Ok(v == expected),
            Err(_) => Ok(false),
        }
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

#[tauri::command]
fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(AUTOSTART_KEY, KEY_SET_VALUE)
            .map_err(|e| e.to_string())?;
        if enable {
            key.set_value(AUTOSTART_NAME, &autostart_cmd()?)
                .map_err(|e| e.to_string())?;
        } else {
            let _ = key.delete_value(AUTOSTART_NAME);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = enable;
        Ok(())
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            features::get_features,
            excel::excel_scan,
            excel::excel_split,
            imagetool::image_probe,
            imagetool::image_convert,
            markdown::md_preview,
            markdown::md_export,
            confirm_exit,
            get_app_config,
            app_build_info,
            updater::check_update,
            updater::install_update,
            updater::set_update_channel,
            updater::set_update_endpoint,
            updater::set_check_on_start,
            set_close_behavior,
            get_close_behavior,
            get_config_fatal,
            hide_to_tray,
            get_autostart,
            set_autostart,
            read_text_file,
            write_text_file,
            app_version,
        ])
        .setup(|app| {
            let state = config::ConfigState::from_load(config::load(&config::default_dir()));
            app.manage(state);
            tray::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 关闭永不直接退进程（F6-1 / F6-7）：
                // 默认隐藏到托盘，若用户设置了「直接退出」则转交前端确认。
                api.prevent_close();
                let to_tray = window
                    .app_handle()
                    .state::<config::ConfigState>()
                    .config
                    .lock()
                    .map(|c| c.close_to_tray)
                    .unwrap_or(true);
                if to_tray {
                    let _ = window.hide();
                } else {
                    let _ = window.app_handle().emit("forge://request-exit", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
