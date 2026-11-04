//! 系统托盘：常驻、菜单、双击唤出。
//!
//! 对应 PRD F6-1 ~ F6-4。退出不走系统托盘默认行为，
//! 而是发事件给前端做确认（F6-7），因为需要在有任务时
//! 把「最小化并继续」作为首选按钮。

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
    let progress = MenuItem::with_id(app, "progress", "任务进度", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &progress, &settings, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("青丘湖硅基工坊")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "progress" => {
                show_main(app);
                let _ = app.emit("forge://nav", "progress");
            }
            "settings" => {
                show_main(app);
                let _ = app.emit("forge://nav", "settings");
            }
            "quit" => {
                // 不在后端直接退出，交给前端弹确认（F6-7）
                let _ = app.emit("forge://request-exit", ());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}
