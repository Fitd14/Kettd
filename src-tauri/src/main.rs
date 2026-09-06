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

mod app;
mod commands;
mod infra;
mod export;
mod models;
mod ports;
mod runtime;
mod scheduler;
mod store;
mod telemetry;

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
    // 状态必须在窗口创建前注册：float 窗口随 build() 立即加载页面并 invoke，
    // 而 setup 在窗口创建后才执行，在 setup 里 manage 会撞上启动竞态直接 panic
    .manage(Mutex::new(boot))
    .manage(runtime::HotkeyLock(Mutex::new(
      runtime::HotkeyRegistry::default(),
    )))
    .invoke_handler(tauri::generate_handler![
      commands::get_bootstrap,
      commands::get_tasks,
      commands::get_task,
      commands::get_settings,
      commands::get_reminders,
      commands::get_data_health,
      commands::get_hotkey_status,
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
      commands::capture_start_drag,
      commands::register_capture_hotkey,
      commands::register_main_hotkey,
      commands::open_data_folder,
      commands::restore_backup,
      commands::clear_migration_report,
      commands::export_weekly
    ])
    .system_tray(runtime::build_tray(&form))
    .setup(|app| {
      let handle = app.handle().clone();
      let state = app.state::<Mutex<store::Store>>();
      let combo = match state.lock() {
        Ok(guard) => (
          guard.data.settings.capture_hotkey.clone(),
          guard.data.settings.main_hotkey.clone(),
        ),
        Err(poisoned) => {
          let guard = poisoned.into_inner();
          (
            guard.data.settings.capture_hotkey.clone(),
            guard.data.settings.main_hotkey.clone(),
          )
        }
      };
      // 本地度量埋点：起后台写线程，开关随设置，记一次 app_launch（纯本机、不出网）
      telemetry::init(store::data_dir());
      telemetry::set_enabled(
        state
          .lock()
          .map(|g| g.data.settings.telemetry_enabled)
          .unwrap_or(true),
      );
      telemetry::record(
        "app_launch",
        serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }),
      );
      // 捕获热键：窗口全部就绪后注册一次；失败重试 3 次（失败时前端可仍用悬浮面板输入框）
      let hotkey_app = handle.clone();
      std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        let mut cap_failed = false;
        for attempt in 0..4 {
          match runtime::register_capture_hotkey(&hotkey_app, &combo.0) {
            Ok(()) => {
              cap_failed = false;
              break;
            }
            Err(_) => {
              cap_failed = true;
              std::thread::sleep(std::time::Duration::from_millis(400 * (attempt + 1)));
            }
          }
        }
        // 打开主界面热键：默认 Alt+Shift+O，用户解绑（None）则不注册
        let main_failed = runtime::register_main_hotkey(&hotkey_app, &combo.1).is_err();
        // qa-1：注册失败不再静默 —— 提示一次「哪个键没绑上」，再把实际注册快照广播给前端标「未生效」
        if cap_failed || main_failed {
          let mut dead: Vec<String> = Vec::new();
          if cap_failed {
            dead.push(format!("快速记录 {}", combo.0));
          }
          if main_failed {
            if let Some(main) = &combo.1 {
              dead.push(format!("打开主界面 {}", main));
            }
          }
          runtime::notify_title(
            &hotkey_app,
            "快捷键没绑上",
            &format!("{} 可能被其他程序占用了；可在设置里换一个，或先用托盘菜单", dead.join("、")),
          );
        }
        runtime::publish_hotkey_state(&hotkey_app);
      });
      // 提醒调度线程（托盘常驻，A3 批复）
      scheduler::start(handle.clone());
      // 形态跟随设置
      let state = handle.state::<Mutex<store::Store>>();
      let want = match state.lock() {
        Ok(guard) => guard.data.settings.float_form.clone(),
        Err(poisoned) => poisoned.into_inner().data.settings.float_form.clone(),
      };
      let _ = runtime::set_float_form(&handle, &want);
      if let Some(tray) = handle.tray_handle_by_id(runtime::TRAY_ID) {
        let _ = tray.set_tooltip("待办列表 · 常驻托盘");
      }
      Ok(())
    })
    .on_system_tray_event(|app, event| match event {
      tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
        if runtime::handle_tray_click(app, &id) {
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
        // 快速记录条失焦即收起（PRD 6.1 的边缘态）；收起前记忆位置
        tauri::WindowEvent::Focused(false) if label == runtime::CAPTURE_LABEL => {
          runtime::save_capture_pos_window(event.window());
          let _ = event.window().hide();
        }
        _ => {}
      }
    })
    .build(tauri::generate_context!())
    .expect("待办列表启动失败，请检查数据目录权限后重试");

  app.run(|_app_handle, _event| {});
}
