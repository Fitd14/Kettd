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

/// 启动期的严重错误必须可见：release 没有控制台，dev 的控制台也可能被管道吞掉。
/// 除 stderr 外弹一个中文短句对话框，避免用户只看到「双击没反应」。
#[cfg(windows)]
fn show_startup_error(message: &str) {
  #[link(name = "user32")]
  extern "system" {
    fn MessageBoxW(hwnd: *mut u8, text: *const u16, caption: *const u16, utype: u32) -> i32;
  }
  const MB_OK_ICON_ERROR: u32 = 0x10;
  eprintln!("{message}");
  let text: Vec<u16> = format!("{message}\0").encode_utf16().collect();
  let caption: Vec<u16> = "Kettd\0".encode_utf16().collect();
  unsafe {
    MessageBoxW(
      std::ptr::null_mut(),
      text.as_ptr(),
      caption.as_ptr(),
      MB_OK_ICON_ERROR,
    );
  }
}

#[cfg(not(windows))]
fn show_startup_error(message: &str) {
  eprintln!("{message}");
}

/// setup 的错误类型是 `Box<dyn Error>`，而建窗错误是中文 `String`：包一层 io::Error 才能上抛。
fn boot_error(message: String) -> Box<dyn std::error::Error> {
  Box::new(std::io::Error::new(std::io::ErrorKind::Other, message))
}

fn main() {
  infra::env::load();
  runtime::mark_main_thread();
  #[cfg(windows)]
  enforce_single_instance();
  // WebView2 用户数据目录必须在建窗前定下来：默认目录不可写会让 build() 直接失败，
  // 而那时既没有窗口也没有埋点，用户只能看到一条 panic（fix-known-defects 7.1）
  let webview2_dir = match runtime::resolve_webview2_dir() {
    Ok(dir) => dir,
    Err(message) => {
      show_startup_error(&message);
      std::process::exit(1);
    }
  };
  let fallback = webview2_dir != store::data_dir().join("webview2");
  runtime::set_webview2_dir(webview2_dir.clone());
  // 埋点提前到建窗之前：建窗失败也要能在 events.jsonl 里留下证据
  telemetry::init(store::data_dir());
  let dir_text = webview2_dir.to_string_lossy().to_string();
  telemetry::record_str(
    "webview2_dir",
    &[("dir", dir_text.as_str()), ("fallback", if fallback { "yes" } else { "no" })],
  );
  let mut boot = store::Store::load();
  // 启动即清理过期软删（回收站 30 天承诺）
  let purged = boot.purge_expired(models::TRASH_RETENTION_DAYS);
  if purged > 0 {
    let _ = boot.save();
  }
  // 待办悬浮窗托盘勾选初始态（manage 移交 boot 前读走，todo-float）
  let todo_visible = boot.runtime.todo_float_visible;

  let builder = tauri::Builder::default()
    // 状态必须在窗口创建前注册：便签窗（note:*）与 capture 窗随 build() 立即加载页面并 invoke，
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
      commands::hide_main_window,
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
      commands::list_folders,
      commands::create_folder,
      commands::rename_folder,
      commands::delete_folder,
      commands::list_kb_backups,
      commands::restore_kb_backup,
      commands::clear_migration_report,
      commands::export_weekly,
      commands::reorder_tasks,
      commands::clear_events,
      commands::track_event,
      commands::create_sticky,
      commands::list_stickies,
      commands::update_sticky,
      commands::open_sticky,
      commands::delete_sticky,
      commands::sticky_self,
      commands::set_todo_float_mini,
    ])
    .system_tray(runtime::build_tray(todo_visible))
    .setup(|app| {
      // 主窗与快速记录条改为代码建窗：tauri.conf.json 的 WindowConfig 没有 dataDirectory
      // 字段，只有代码建窗能显式指定 WebView2 用户数据目录（默认目录不可写时 build() 会失败）。
      // 与 Tauri 自建 conf 窗口同阶段（事件循环尚未开始），不触发「事件循环内同步建窗挂起」。
      let handle = app.handle().clone();
      if app.get_window(runtime::MAIN_LABEL).is_none() {
        runtime::create_main_window(&*app).map_err(boot_error)?;
      }
      if app.get_window(runtime::CAPTURE_LABEL).is_none() {
        runtime::create_capture_window(&*app).map_err(boot_error)?;
      }
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
      // 本地度量：写线程已在 main() 建窗之前启动（建窗失败也要留证据），这里只同步开关
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
      // WebView2 默认右键菜单（#3）：主窗与配置即建的快速记录条在 setup 前已创建，
      // 这里统一关闭；动态 note:*/todo 窗在建窗成功后各自附加（见 runtime.rs 各 builder）。
      // attach 失败只记埋点，不影响窗口生命周期。
      if let Some(main_window) = app.get_window(runtime::MAIN_LABEL) {
        runtime::disable_default_context_menu(&main_window);
      }
      if let Some(capture_window) = app.get_window(runtime::CAPTURE_LABEL) {
        runtime::disable_default_context_menu(&capture_window);
      }
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
        // 只恢复可见便签（fix-known-defects 4.1：隐藏 = 用户主动关窗，重启不弹；
        // 升级首启的旧数据全为默认可见，所以 6 张旧便签仍会恢复，不强制隐藏）
        for note in stickies.iter().filter(|note| note.visible) {
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
        // 便签窗关闭（✕/Alt+F4）= 隐藏（fix-known-defects 4.1/4.3）：只置 visible=false
        // 并藏窗，不删数据、不清 note_pos；可从便签列表 open_sticky 重新打开。
        tauri::WindowEvent::CloseRequested { api, .. } if label.starts_with("note:") => {
          api.prevent_close();
          let id = label.strip_prefix("note:").unwrap_or(&label).to_string();
          let mut changed = false;
          if let Some(state) = event.window().try_state::<Mutex<store::Store>>() {
            if let Ok(mut guard) = state.lock() {
              if let Some(note) = guard.find_sticky_mut(&id) {
                if note.visible {
                  note.visible = false;
                  note.updated_at = crate::models::now_text();
                  changed = true;
                }
              }
              if changed {
                let _ = guard.save();
              }
            }
          }
          let _ = event.window().hide();
          if changed {
            let _ = event.window().emit_all(
              "store-changed",
              crate::models::StoreEvent::changed("stickies"),
            );
          }
          crate::telemetry::record_str("sticky_hide", &[("via", "alt_f4")]);
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
    });

  let app = match builder.build(tauri::generate_context!()) {
    Ok(app) => app,
    Err(error) => {
      // 不 panic：release 无控制台，panic 等于静默退出
      eprintln!("启动失败（build）：{error:?}");
      telemetry::record_str("startup_error", &[("stage", "build"), ("err", &format!("{error}"))]);
      show_startup_error("待办列表启动失败，请稍后重试；若反复出现请检查磁盘空间与数据目录权限。");
      std::process::exit(1);
    }
  };

  app.run(|app_handle, event| {
    // 退出前显式释放 WebView2 控制器：默认退出流程直接 process::exit，
    // 不做窗口销毁，msedgewebview2.exe 可能残留（fix-known-defects 7.4 / #16）
    if let tauri::RunEvent::ExitRequested { .. } = event {
      runtime::close_all_webviews(app_handle);
    }
  });
}
