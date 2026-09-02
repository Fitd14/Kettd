//! 窗口 / 托盘 / 全局快捷键 / 系统通知（不含业务写库逻辑）

use crate::store::Store;
use std::path::Path;
use std::sync::Mutex;
use tauri::{
  AppHandle, CustomMenuItem, GlobalShortcutManager, LogicalSize, Manager, PhysicalPosition,
  Position, Size, SystemTray, SystemTrayMenu, SystemTrayMenuItem, SystemTraySubmenu, Window,
  WindowBuilder, WindowUrl,
};

pub const MAIN_LABEL: &str = "main";
pub const FLOAT_LABEL: &str = "float";
pub const CAPTURE_LABEL: &str = "capture";
pub const TRAY_ID: &str = "main";

/// 当前注册成功的全局快捷键（换键时先解绑旧的）
pub struct HotkeyLock(pub Mutex<Option<String>>);

// ---------------------------------------------------------------- 窗口

pub fn window_of(app: &AppHandle, label: &str) -> Result<Window, String> {
  app
    .get_window(label)
    .ok_or_else(|| "窗口还没准备好，请稍后再试".to_string())
}

/// 打开主界面；route 为前端内部路由（如 /planned/reminders）
pub fn show_main(app: &AppHandle, route: Option<&str>) -> Result<(), String> {
  let window = window_of(app, MAIN_LABEL)?;
  window.show().map_err(|_| "主界面显示失败，请重试".to_string())?;
  if window.is_maximized().unwrap_or(false) {
    let _ = window.unmaximize();
  }
  let _ = window.set_focus();
  if let Some(path) = route {
    let _ = window.emit("navigate", path.to_string());
  }
  Ok(())
}

pub fn show_window(app: &AppHandle, label: &str) -> Result<(), String> {
  let window = window_of(app, label)?;
  window.show().map_err(|_| "窗口显示失败，请重试".to_string())?;
  let _ = window.set_focus();
  Ok(())
}

pub fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
  let window = window_of(app, label)?;
  window.hide().map_err(|_| "窗口隐藏失败".to_string())
}

/// 悬浮面板三形态：置顶 / 嵌桌面 / 迷你条
pub fn set_float_form(app: &AppHandle, form: &str) -> Result<(), String> {
  if !crate::models::FLOAT_FORMS.contains(&form) {
    return Err("悬浮形态只能是 topmost / desktop / mini".to_string());
  }
  let window = window_of(app, FLOAT_LABEL)?;
  match form {
    "topmost" => {
      let _ = window.set_always_on_top(true);
      let _ = window.set_decorations(true);
      let _ = window.set_resizable(true);
      let _ = window.set_size(Size::Logical(LogicalSize::new(380.0, 520.0)));
    }
    "desktop" => {
      // 嵌入桌面：取消置顶，失焦即收起（收起逻辑在 scheduler 的 1s 巡检里）
      let _ = window.set_always_on_top(false);
      let _ = window.set_decorations(true);
      let _ = window.set_resizable(true);
      let _ = window.set_size(Size::Logical(LogicalSize::new(380.0, 520.0)));
    }
    "mini" => {
      let _ = window.set_decorations(false);
      let _ = window.set_resizable(false);
      let _ = window.set_always_on_top(true);
      let _ = window.set_size(Size::Logical(LogicalSize::new(300.0, 56.0)));
      park_mini(&window);
    }
    _ => {}
  }
  Ok(())
}

/// 迷你条贴主屏右下角，避开工作区
fn park_mini(window: &Window) {
  if let Ok(Some(monitor)) = window.current_monitor() {
    let scale = monitor.scale_factor();
    let width = (300.0 * scale) as i32;
    let height = (56.0 * scale) as i32;
    let right = monitor.position().x + monitor.size().width as i32;
    let bottom = monitor.position().y + monitor.size().height as i32;
    let x = right - width - (16.0 * scale) as i32;
    let y = bottom - height - (16.0 * scale) as i32;
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
  }
}

/// 悬浮面板当前形态；读不到时按置顶处理
pub fn float_form(app: &AppHandle) -> String {
  match app.try_state::<Mutex<Store>>() {
    None => "topmost".to_string(),
    Some(state) => match state.lock() {
      Ok(guard) => guard.data.settings.float_form.clone(),
      Err(poisoned) => poisoned.into_inner().data.settings.float_form.clone(),
    },
  }
}

/// 供 scheduler 判断桌面形态是否要因失焦收起
pub fn float_should_hide_on_blur(app: &AppHandle) -> bool {
  float_form(app) == "desktop"
}

pub fn float_is_focused(app: &AppHandle) -> bool {
  match window_of(app, FLOAT_LABEL) {
    Ok(window) => window.is_focused().unwrap_or(true),
    Err(_) => true,
  }
}

pub fn float_is_visible(app: &AppHandle) -> bool {
  match window_of(app, FLOAT_LABEL) {
    Ok(window) => window.is_visible().unwrap_or(false),
    Err(_) => false,
  }
}

// ---------------------------------------------------------------- 快速记录浮条

fn create_capture_window(app: &AppHandle) -> Result<Window, String> {
  WindowBuilder::new(app, CAPTURE_LABEL, WindowUrl::App("capture.html".into()))
    .title("快速记录")
    .inner_size(520.0, 72.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .center()
    .build()
    .map_err(|_| "快速记录条创建失败，请用悬浮面板底部输入框".to_string())
}

/// 520×72 无边框居中置顶输入条；焦点交给输入框（前端监听 capture-opened）
pub fn open_capture_overlay(app: &AppHandle) -> Result<(), String> {
  let window = match app.get_window(CAPTURE_LABEL) {
    Some(existing) => existing,
    None => match create_capture_window(app) {
      Ok(created) => created,
      Err(_) => {
        // 降级：capture.html 尚未就绪时，让前端自挂的快捷输入条接管
        let _ = app.emit_all("open-capture", ());
        let _ = show_main(app, Some("/inbox"));
        return Err("快速记录条没就绪，请用悬浮面板输入框".to_string());
      }
    },
  };
  center_capture(&window);
  let _ = window.set_always_on_top(true);
  window
    .show()
    .map_err(|_| "快速记录条显示失败，请用悬浮面板底部输入框".to_string())?;
  if window.set_focus().is_err() {
    let again = window.clone();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_millis(120));
      let _ = again.set_focus();
    });
  }
  let _ = window.emit("capture-opened", ());
  Ok(())
}

fn center_capture(window: &Window) {
  if let Ok(Some(monitor)) = window.current_monitor() {
    let scale = monitor.scale_factor();
    let width = (520.0 * scale) as i32;
    let height = (72.0 * scale) as i32;
    let x = monitor.position().x + (monitor.size().width as i32 - width) / 2;
    let y = monitor.position().y + ((monitor.size().height as i32 - height) * 28) / 100;
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
  }
}

pub fn close_capture_overlay(app: &AppHandle) -> Result<(), String> {
  hide_window(app, CAPTURE_LABEL)
}

pub fn capture_is_visible(app: &AppHandle) -> bool {
  match window_of(app, CAPTURE_LABEL) {
    Ok(window) => window.is_visible().unwrap_or(false),
    Err(_) => false,
  }
}

// ---------------------------------------------------------------- 全局快捷键

/// 注册（或换绑）捕获热键；失败回滚旧键并返回可显示的中文提示
pub fn register_capture_hotkey(app: &AppHandle, combo: &str) -> Result<(), String> {
  let target = combo.trim().to_string();
  if target.is_empty() {
    return Err("快捷键不能为空".to_string());
  }
  let previous = read_hotkey(app);
  let mut manager = app.global_shortcut_manager();
  if let Some(old) = &previous {
    if old != &target {
      let _ = manager.unregister(old);
    }
  }
  let handle = app.clone();
  let registered = manager.register(&target, move || {
    let _ = open_capture_overlay(&handle);
  });
  if registered.is_ok() {
    store_hotkey(app, Some(target));
    return Ok(());
  }
  // 注册失败：把旧键还回去，保证用户还能用
  if let Some(old) = previous {
    let restore = app.clone();
    let _ = manager.register(&old, move || {
      let _ = open_capture_overlay(&restore);
    });
  }
  Err("快捷键被占用，请用备用入口".to_string())
}

fn read_hotkey(app: &AppHandle) -> Option<String> {
  match app.try_state::<HotkeyLock>() {
    None => None,
    Some(state) => match state.0.lock() {
      Ok(guard) => guard.clone(),
      Err(poisoned) => poisoned.into_inner().clone(),
    },
  }
}

fn store_hotkey(app: &AppHandle, value: Option<String>) {
  if let Some(state) = app.try_state::<HotkeyLock>() {
    match state.0.lock() {
      Ok(mut guard) => *guard = value,
      Err(poisoned) => *poisoned.into_inner() = value,
    }
  }
}

// ---------------------------------------------------------------- 托盘

pub fn tray_menu(form: &str) -> SystemTrayMenu {
  let mut topmost = CustomMenuItem::new("form_topmost", "置于顶层");
  let mut desktop = CustomMenuItem::new("form_desktop", "嵌入桌面");
  let mut mini = CustomMenuItem::new("form_mini", "迷你条");
  if form == "topmost" {
    topmost = topmost.selected();
  }
  if form == "desktop" {
    desktop = desktop.selected();
  }
  if form == "mini" {
    mini = mini.selected();
  }
  SystemTrayMenu::new()
    .add_item(CustomMenuItem::new("open_main", "打开主界面"))
    .add_item(CustomMenuItem::new("capture", "快速记录"))
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(CustomMenuItem::new("float_show", "显示悬浮面板"))
    .add_item(CustomMenuItem::new("float_hide", "隐藏悬浮面板"))
    .add_submenu(SystemTraySubmenu::new(
      "悬浮形态",
      SystemTrayMenu::new().add_item(topmost).add_item(desktop).add_item(mini),
    ))
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(CustomMenuItem::new("reminders", "管理提醒"))
    .add_item(CustomMenuItem::new("weekly", "本周汇总导出"))
    .add_item(CustomMenuItem::new("data_dir", "打开数据文件夹"))
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(CustomMenuItem::new("quit", "退出"))
}

pub fn build_tray(form: &str) -> SystemTray {
  SystemTray::new().with_id(TRAY_ID).with_menu(tray_menu(form))
}

/// 形态变化后刷新托盘子菜单勾选
pub fn sync_tray_selection(app: &AppHandle, form: &str) {
  if let Some(handle) = app.tray_handle_by_id(TRAY_ID) {
    let pairs = [
      ("form_topmost", form == "topmost"),
      ("form_desktop", form == "desktop"),
      ("form_mini", form == "mini"),
    ];
    for (id, active) in pairs.iter() {
      if let Some(item) = handle.try_get_item(id.as_str()) {
        let _ = item.set_selected(*active);
      }
    }
  }
}

/// 托盘菜单点击分发；返回 true 表示要退出应用
pub fn handle_tray_click(app: &AppHandle, id: &str) -> bool {
  match id {
    "open_main" => {
      let _ = show_main(app, None);
    }
    "capture" => {
      let _ = open_capture_overlay(app);
    }
    "float_show" => {
      let _ = show_window(app, FLOAT_LABEL);
    }
    "float_hide" => {
      let _ = hide_window(app, FLOAT_LABEL);
    }
    "form_topmost" | "form_desktop" | "form_mini" => {
      let form = id.replace("form_", "");
      if crate::models::FLOAT_FORMS.contains(&form.as_str()) {
        let _ = set_float_form(app, &form);
        if let Some(state) = app.try_state::<Mutex<Store>>() {
          let mut guard = match state.lock() {
            Ok(value) => value,
            Err(poisoned) => poisoned.into_inner(),
          };
          if guard.data.settings.float_form != form {
            guard.data.settings.float_form = form.clone();
            if guard.save().is_ok() {
              drop(guard);
              let _ = app.emit_all(
                "store-changed",
                crate::models::StoreEvent::changed("settings"),
              );
            }
          }
        }
        sync_tray_selection(app, &form);
      }
    }
    "reminders" => {
      let _ = show_main(app, Some("/planned/reminders"));
    }
    "weekly" => {
      let _ = show_main(app, Some("/review/weekly"));
    }
    "data_dir" => {
      let _ = open_in_folder(&crate::store::data_dir());
    }
    "quit" => return true,
    _ => {}
  }
  false
}

/// 在文件管理器里打开（或选中）一个路径
pub fn open_in_folder(path: &Path) -> Result<(), String> {
  #[cfg(target_os = "windows")]
  let spawned = {
    let mut command = std::process::Command::new("explorer");
    command.arg(path);
    command.spawn()
  };
  #[cfg(target_os = "macos")]
  let spawned = {
    let mut command = std::process::Command::new("open");
    command.arg("-R").arg(path);
    command.spawn()
  };
  #[cfg(all(unix, not(target_os = "macos")))]
  let spawned = {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path);
    command.spawn()
  };
  match spawned {
    Ok(_) => Ok(()),
    Err(_) => Err("打不开这个位置，请手动前往".to_string()),
  }
}

// ---------------------------------------------------------------- 系统通知

/// 到点的系统通知；点击后的深链由前端窗口处理（不带 payload）
pub fn notify(app: &AppHandle, body: &str) -> bool {
  notify_title(app, "待办提醒", body)
}

pub fn notify_title(app: &AppHandle, title: &str, body: &str) -> bool {
  let identifier = app.config().tauri.bundle.identifier.clone();
  tauri::api::notification::Notification::new(identifier)
    .title(title)
    .body(body)
    .show()
    .is_ok()
}
