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
mod kb;
mod export;
mod models;
mod ports;
mod runtime;
mod scheduler;
mod store;
mod telemetry;
mod ai;

use std::sync::Mutex;
use tauri::Manager;

/// 单实例守卫：多实例并存会放大 WebView2 建窗挂起（共享浏览器进程被搅），
/// 且双实例互踩 data.json。第二个实例弹窗说明后立即退出。
#[cfg(windows)]
fn enforce_single_instance() {
  #[link(name = "kernel32")]
  extern "system" {
    fn CreateMutexW(lpMutexAttributes: *mut u8, bInitialOwner: i32, lpName: *const u16) -> *mut u8;
    fn GetLastError() -> u32;
  }
  #[link(name = "user32")]
  extern "system" {
    fn MessageBoxW(hwnd: *mut u8, text: *const u16, caption: *const u16, utype: u32) -> i32;
  }
  const ERROR_ALREADY_EXISTS: u32 = 183;
  const MB_OK_ICON_INFO: u32 = 0x40;
  let name: Vec<u16> = "Local\\Kettd.TodoList.SingleInstance\0"
    .encode_utf16()
    .collect();
  let _guard = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
  if !_guard.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
    let text: Vec<u16> = "Kettd 已在运行（托盘里找回它）。\0".encode_utf16().collect();
    let caption: Vec<u16> = "Kettd\0".encode_utf16().collect();
    unsafe {
      MessageBoxW(
        std::ptr::null_mut(),
        text.as_ptr(),
        caption.as_ptr(),
        MB_OK_ICON_INFO,
      )
    };
    std::process::exit(0);
  }
}

fn main() {
  runtime::mark_main_thread();
  #[cfg(windows)]
  enforce_single_instance();
  let mut boot = store::Store::load();
  // 启动即清理过期软删（回收站 30 天承诺）
  let purged = boot.purge_expired(models::TRASH_RETENTION_DAYS);
  if purged > 0 {
    let _ = boot.save();
  }
  // 待办悬浮窗托盘勾选初始态（manage 移交 boot 前读走，todo-float）
  let todo_visible = boot.runtime.todo_float_visible;

  let app = tauri::Builder::default()
    // 状态必须在窗口创建前注册：便签窗（note:*）与 capture 窗随 build() 立即加载页面并 invoke，
    // 而 setup 在窗口创建后才执行，在 setup 里 manage 会撞上启动竞态直接 panic
    .manage(Mutex::new(boot))
    .manage(runtime::HotkeyLock(Mutex::new(
      runtime::HotkeyRegistry::default(),
    )))
    .manage(ai::AiKeeper::new(std::sync::Arc::new(
      ai::secrets::InMemoryKeeper::new(),
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
      commands::open_capture_overlay,
      commands::close_capture_overlay,
      commands::capture_start_drag,
      commands::capture_resize,
      commands::register_capture_hotkey,
      commands::register_main_hotkey,
      commands::register_sticky_hotkey,
      commands::register_todo_hotkey,
      commands::hide_todo_float,
      commands::open_data_folder,
      commands::restore_backup,
      commands::rollback_schema_split,
      commands::get_kb_items,
      commands::add_kb_item,
      commands::update_kb_item,
      commands::delete_kb_item,
      commands::search_kb,
      commands::clear_migration_report,
      commands::export_weekly,
      commands::reorder_tasks,
      commands::clear_events,
      commands::track_event,
      commands::create_sticky,
      commands::list_stickies,
      commands::update_sticky,
      commands::delete_sticky,
      commands::sticky_self,
      ai::commands::set_ai_config,
      ai::commands::get_ai_status,
      ai::commands::test_ai_connection,
      ai::commands::search_kb_hybrid,
    ])
    .system_tray(runtime::build_tray(todo_visible))
    .setup(|app| {
      let handle = app.handle().clone();
      let state = app.state::<Mutex<store::Store>>();
      // 主窗自定义标题栏（方案B）：decorations 已关，圆角与最大化态由这里接管
      if let Some(main_window) = app.get_window(runtime::MAIN_LABEL) {
        runtime::watch_main_window_frame(&main_window);
      }
      let combo = match state.lock() {
        Ok(guard) => (
          guard.data.settings.capture_hotkey.clone(),
          guard.data.settings.main_hotkey.clone(),
          guard.data.settings.sticky_hotkey.clone(),
          guard.data.settings.todo_hotkey.clone(),
        ),
        Err(poisoned) => {
          let guard = poisoned.into_inner();
          (
            guard.data.settings.capture_hotkey.clone(),
            guard.data.settings.main_hotkey.clone(),
            guard.data.settings.sticky_hotkey.clone(),
            guard.data.settings.todo_hotkey.clone(),
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
      let sticky_count = state
        .lock()
        .map(|g| g.data.stickies.len())
        .unwrap_or(0);
      telemetry::record(
        "app_launch",
        serde_json::json!({
          "version": env!("CARGO_PKG_VERSION"),
          "sticky_count": sticky_count
        }),
      );
      // 三个全局热键：窗口全部就绪后注册一次；失败重试 3 次（失败时前端可仍用主窗输入框）
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
        // 新建便签热键：默认 Alt+Shift+S，解绑（None）则不注册（sticky-separation）
        let sticky_failed = runtime::register_sticky_hotkey(&hotkey_app, &combo.2).is_err();
        // 待办悬浮窗热键：默认 Alt+Shift+T，解绑（None）则不注册（todo-float）
        let todo_failed = runtime::register_todo_hotkey(&hotkey_app, &combo.3).is_err();
        // qa-1：注册失败不再静默 —— 提示一次「哪个键没绑上」，再把实际注册快照广播给前端标「未生效」
        if cap_failed || main_failed || sticky_failed || todo_failed {
          let mut dead: Vec<String> = Vec::new();
          if cap_failed {
            dead.push(format!("快速记录 {}", combo.0));
          }
          if main_failed {
            if let Some(main) = &combo.1 {
              dead.push(format!("打开主界面 {}", main));
            }
          }
          if sticky_failed {
            if let Some(sticky) = &combo.2 {
              dead.push(format!("新建便签 {}", sticky));
            }
          }
          if todo_failed {
            if let Some(todo) = &combo.3 {
              dead.push(format!("待办悬浮窗 {}", todo));
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
      // 多便签恢复（sticky-separation：全部为动态 note:* 窗，缩小态以悬浮条形态恢复）
      {
        let state = handle.state::<Mutex<store::Store>>();
        let (stickies, note_pos, todo_visible, todo_pinned) = state
          .lock()
          .map(|g| {
            (
              g.data.stickies.clone(),
              g.runtime.note_pos.clone(),
              g.runtime.todo_float_visible,
              g.data.settings.todo_float_pinned,
            )
          })
          .unwrap_or_default();
        for note in &stickies {
          let pos = note_pos.get(&note.id).copied();
          let _ = runtime::open_note_window(&handle, &note.id, note.mini, pos);
        }
        // 待办悬浮窗：按上次显隐恢复（todo-float）
        let _ = runtime::apply_todo_float_pinned(&handle, todo_pinned);
        if todo_visible {
          let _ = runtime::open_todo_float(&handle);
        }
      }
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
        // 待办悬浮窗关闭（✕/Alt+F4）= 隐藏收进托盘，显隐状态落盘（todo-float）
        tauri::WindowEvent::CloseRequested { api, .. } if label == runtime::TODO_LABEL => {
          api.prevent_close();
          let app_handle = event.window().app_handle().clone();
          let _ = runtime::hide_todo_float(&app_handle);
        }
        // 便签窗关闭 = 销毁（sticky-separation：关闭即消失，含 Alt+F4）；
        // 数据与窗口一起回收，堵住「Alt+F4 只关窗、数据残留」的旧漏洞
        tauri::WindowEvent::CloseRequested { api: _, .. } if label.starts_with("note:") => {
          if let Some(state) = event.window().try_state::<Mutex<store::Store>>() {
            if let Ok(mut guard) = state.lock() {
              let id = label.strip_prefix("note:").unwrap_or(&label).to_string();
              if guard.delete_sticky(&id).is_ok() {
                let _ = guard.save();
                let _ = guard.save_runtime_only();
                drop(guard);
                let _ = event
                  .window()
                  .emit_all("store-changed", crate::models::StoreEvent::changed("stickies"));
              }
            }
          }
          crate::telemetry::record_str("sticky_delete", &[("via", "alt_f4")]);
        }
        // 快速记录条失焦即收起（PRD 6.1 的边缘态）；收起前记忆位置
        tauri::WindowEvent::Focused(false) if label == runtime::CAPTURE_LABEL => {
          runtime::save_capture_pos_window(event.window());
          let _ = event.window().hide();
        }
        // 便签拖动即记位（位置归 runtime.json，不轮转；note:* 动态便签）
        tauri::WindowEvent::Moved(_) if label.starts_with("note:") => {
          runtime::save_sticky_pos_window(event.window());
        }
        _ => {}
      }
    })
    .build(tauri::generate_context!())
    .expect("待办列表启动失败，请检查数据目录权限后重试");

  app.run(|_app_handle, _event| {});
}
