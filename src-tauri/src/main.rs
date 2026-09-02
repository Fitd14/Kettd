//! 待办列表 v2 · Wave-1 后端
//!
//! 模块划分：
//! - `models`    领域模型 + 本地时间工具（camelCase 序列化，禁 UTC 换算）
//! - `store`     存储安全层（原子写 / 写前轮转备份 / corrupt 通道 / v1 懒迁移 / 软删）
//! - `runtime`   窗口三形态、快速记录浮条、全局热键、托盘、系统通知
//! - `scheduler` 提醒调度（1s tick + 每小时上限 + 免打扰 + 错过 24h 失效 + 启动补发）
//! - `export`    周汇总导出（md / csv）
//! - `commands`  对前端的 invoke 命令（全部 Result<_, String>，错误为中文短句）

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;
mod export;
mod models;
mod runtime;
mod scheduler;
mod store;

use std::sync::Mutex;
use tauri::Manager;

fn main() {
  let mut boot = store::Store::load();
  // 启动即清理过期软删（回收站 30 天承诺）
  let purged = boot.purge_expired(models::TRASH_RETENTION_DAYS);
  if purged > 0 {
    let _ = boot.save();
  }
  let form = boot.data.settings.float_form.clone();

  let app = tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
      commands::get_bootstrap,
      commands::get_tasks,
      commands::get_task,
      commands::get_settings,
      commands::get_reminders,
      commands::get_data_health,
      commands::get_backups,
      commands::get_form_hints,
      commands::add_task,
      commands::update_task,
      commands::toggle_task,
      commands::delete_task,
      commands::undo_delete,
      commands::restore_task,
      commands::purge_task,
      commands::add_subtask,
      commands::toggle_subtask,
      commands::delete_subtask,
      commands::add_note,
      commands::delete_note,
      commands::add_reminder,
      commands::update_reminder,
      commands::delete_reminder,
      commands::toggle_reminder,
      commands::snooze_reminder,
      commands::set_settings,
      commands::open_main_window,
      commands::show_float,
      commands::hide_float,
      commands::set_float_form,
      commands::open_capture_overlay,
      commands::close_capture_overlay,
      commands::register_capture_hotkey,
      commands::open_data_folder,
      commands::restore_backup,
      commands::clear_migration_report,
      commands::export_weekly
    ])
    .system_tray(runtime::build_tray(&form))
    .setup(move |app| {
      let handle = app.handle().clone();
      app.manage(boot);
      app.manage(runtime::HotkeyLock(Mutex::new(None)));
      let combo = {
        let state = app.state::<Mutex<store::Store>>();
        match state.lock() {
          Ok(guard) => guard.data.settings.capture_hotkey.clone(),
          Err(poisoned) => poisoned.into_inner().data.settings.capture_hotkey.clone(),
        }
      };
      // 捕获热键：窗口全部就绪后注册一次；失败重试 3 次（失败时前端可仍用悬浮面板输入框）
      let hotkey_app = handle.clone();
      std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        for attempt in 0..4 {
          if runtime::register_capture_hotkey(&hotkey_app, &combo).is_ok() {
            break;
          }
          std::thread::sleep(std::time::Duration::from_millis(400 * (attempt + 1)));
        }
      });
      // 提醒调度线程（托盘常驻，A3 批复）
      scheduler::start(handle.clone());
      // 形态跟随设置
      let want = {
        let state = handle.state::<Mutex<store::Store>>();
        match state.lock() {
          Ok(guard) => guard.data.settings.float_form.clone(),
          Err(poisoned) => poisoned.into_inner().data.settings.float_form.clone(),
        }
      };
      let _ = runtime::set_float_form(&handle, &want);
      if let Some(tray) = handle.tray_handle_by_id(runtime::TRAY_ID) {
        let _ = tray.set_tooltip("待办列表 · 常驻托盘");
      }
      Ok(())
    })
    .on_system_tray_event(|app, event| match event {
      tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
        if runtime::handle_tray_click(app, id) {
          app.exit(0);
        }
      }
      _ => {}
    })
    .on_window_event(|event| {
      let label = event.window().label().to_string();
      match event.event() {
        // 主窗口关闭 = 收进托盘（A3：托盘常驻），不退出进程
        tauri::WindowEvent::CloseRequested { api, .. } if label == runtime::MAIN_LABEL => {
          api.prevent_close();
          let _ = event.window().hide();
        }
        // 快速记录条失焦即收起（PRD 6.1 的边缘态）
        tauri::WindowEvent::Focused(false) if label == runtime::CAPTURE_LABEL => {
          let _ = event.window().hide();
        }
        _ => {}
      }
    })
    .build(tauri::generate_context!())
    .expect("待办列表启动失败，请检查数据目录权限后重试");

  app.run(|_app_handle, _event| {});
}
