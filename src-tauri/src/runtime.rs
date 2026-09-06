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
#[derive(Default)]
pub struct HotkeyRegistry {
  pub capture: Option<String>,
  pub main: Option<String>,
}

pub struct HotkeyLock(pub Mutex<HotkeyRegistry>);

/// 全局快捷键用途；同一组合键不允许绑两个动作
#[derive(Clone, Copy, PartialEq)]
pub enum HotkeyAction {
  Capture,
  OpenMain,
}

impl HotkeyAction {
  fn other(self) -> Self {
    match self {
      HotkeyAction::Capture => HotkeyAction::OpenMain,
      HotkeyAction::OpenMain => HotkeyAction::Capture,
    }
  }
}

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
    .inner_size(560.0, 80.0)
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
  apply_capture_material(&window);
  place_capture(app, &window);
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
    // 用窗口实际尺寸居中，避免与 inner_size 的常量重复（改一处漏一处）
    let (w, h) = window
      .outer_size()
      .map(|s| (s.width as i32, s.height as i32))
      .unwrap_or((560, 80));
    let x = monitor.position().x + (monitor.size().width as i32 - w) / 2;
    let y = monitor.position().y + ((monitor.size().height as i32 - h) * 28) / 100;
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
  }
}

/// 打开时定位：有记忆的屏内位置就用它，否则回落默认居中（不再每次强制回中央）
fn place_capture(app: &AppHandle, window: &Window) {
  let stored = app
    .try_state::<Mutex<Store>>()
    .and_then(|s| s.lock().ok().and_then(|g| g.data.settings.capture_pos));
  if let Some([x, y]) = stored {
    if capture_pos_on_screen(window, x, y) {
      let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
      return;
    }
  }
  center_capture(window);
}

/// 离屏守卫：拔外接屏 / 分辨率变化后，位置若落在所有工作区外则视为无效
fn capture_pos_on_screen(window: &Window, x: i32, y: i32) -> bool {
  match window.available_monitors() {
    Ok(monitors) => monitors.iter().any(|m| {
      let p = m.position();
      let s = m.size();
      x >= p.x - 160 && y >= p.y - 40 && x <= p.x + s.width as i32 && y <= p.y + s.height as i32
    }),
    Err(_) => false,
  }
}

/// 收起前记住当前位置（移动即记的落点：hide / 关闭时持久化）
pub fn save_capture_pos(app: &AppHandle) {
  let Some(window) = app.get_window(CAPTURE_LABEL) else {
    return;
  };
  save_capture_pos_window(&window);
}

/// 窗口事件路径只有 &Window 时用：直接持久化其位置。
/// 位置没变不落盘；要写也走不轮转备份的轻量路径 —— 位置是高频低价值变更，
/// 全量轮转会把历史备份里真正有价值的旧档顶掉。
pub fn save_capture_pos_window(window: &Window) {
  let Ok(pos) = window.outer_position() else {
    return;
  };
  let Some(state) = window.try_state::<Mutex<Store>>() else {
    return;
  };
  let Ok(mut store) = state.lock() else {
    return;
  };
  let next = Some([pos.x, pos.y]);
  if store.data.settings.capture_pos == next {
    return;
  }
  store.data.settings.capture_pos = next;
  let _ = store.save_light();
}

/// 桌面材质：**主用 accent blur + 暖纸 tint，acrylic 退为兜底**。
/// 主次对调的依据是真机像素实测：acrylic 板底 RGB 中位是中性灰 `224,224,224`，
/// 而品牌纸白是 `252,250,248` —— `apply_acrylic` 在 Windows 上**不接受 tint**
/// （那个 color 参数只在 macOS 生效），CSS 层的白 tint 救不回色相，于是"玻璃"
/// 读成一块灰板。`apply_blur` 接受 RGBA tint，可以直接喂暖纸白。
/// alpha 取 55/255 ≈ 0.22：**AA 由 CSS 层独立保证**（等效 α 0.62 时模型实算最差 5.11:1），
/// OS 层再叠只会上加不透明度 —— 真机实测取 120 时总遮蔽高达 87%、透出率仅 13%，
/// 已经退化成实心卡片而不是玻璃。降到 0.22 后总遮蔽约 71%、透出率回到约 30%。
#[cfg(target_os = "windows")]
fn apply_capture_material(window: &Window) {
  apply_capture_rounding(window);
  const PAPER_TINT: (u8, u8, u8, u8) = (252, 250, 248, 55); // == --background 暖纸白
  unsafe {
    if window_vibrancy::apply_blur(window, Some(PAPER_TINT)).is_err() {
      let _ = window_vibrancy::apply_acrylic(window, None);
    }
  }
}

#[cfg(target_os = "windows")]
#[link(name = "dwmapi")]
extern "system" {
  fn DwmSetWindowAttribute(
    hwnd: *mut core::ffi::c_void,
    attribute: u32,
    value: *const u32,
    size: u32,
  ) -> i32;
}

/// 让 DWM 圆这个窗口本身。无边框 WS_POPUP 在 Win11 默认**不**被圆角，出来是直角灰板；
/// 而 CSS 自绘 border-radius 裁不动 OS 磨砂，会在圆角外露出一圈方形磨砂边 ——
/// 所以圆角必须由 DWM 做（DWMWA_WINDOW_CORNER_PREFERENCE = 33，DWMWCP_ROUND = 2）。
#[cfg(target_os = "windows")]
fn apply_capture_rounding(window: &Window) {
  const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
  const DWMWCP_ROUND: u32 = 2;
  if let Ok(hwnd) = window.hwnd() {
    unsafe {
      DwmSetWindowAttribute(
        hwnd.0 as *mut core::ffi::c_void,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &DWMWCP_ROUND,
        core::mem::size_of::<u32>() as u32,
      );
    }
  }
}

#[cfg(not(target_os = "windows"))]
fn apply_capture_material(_window: &Window) {}

/// 供前端「拖拽把手」mousedown 调用，让用户用鼠标移动无边框浮条
pub fn capture_start_drag(app: &AppHandle) -> Result<(), String> {
  let window = app
    .get_window(CAPTURE_LABEL)
    .ok_or_else(|| "快速记录条未就绪".to_string())?;
  window
    .start_dragging()
    .map_err(|_| "无法拖动快速记录条".to_string())
}

pub fn close_capture_overlay(app: &AppHandle) -> Result<(), String> {
  save_capture_pos(app);
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
  let result = register_global_hotkey(app, combo, HotkeyAction::Capture);
  // 成败都广播：失败时该槽位仍是旧值/None，前端据此标「未生效」
  publish_hotkey_state(app);
  result
}

/// 注册 / 换绑 / 解绑「打开主界面」热键；None 或空串 = 解绑
pub fn register_main_hotkey(app: &AppHandle, combo: &Option<String>) -> Result<(), String> {
  let result = main_hotkey_inner(app, combo);
  publish_hotkey_state(app);
  result
}

fn main_hotkey_inner(app: &AppHandle, combo: &Option<String>) -> Result<(), String> {
  match combo.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
    Some(target) => register_global_hotkey(app, target, HotkeyAction::OpenMain),
    None => {
      if let Some(old) = read_hotkey(app, HotkeyAction::OpenMain) {
        let _ = app.global_shortcut_manager().unregister(&old);
      }
      store_hotkey(app, HotkeyAction::OpenMain, None);
      Ok(())
    }
  }
}

fn register_global_hotkey(app: &AppHandle, combo: &str, action: HotkeyAction) -> Result<(), String> {
  let target = combo.trim().to_string();
  if target.is_empty() {
    return Err("快捷键不能为空".to_string());
  }
  let taken = read_hotkey(app, action.other());
  if taken.as_deref() == Some(target.as_str()) {
    return Err("这个快捷键已被快速记录或打开主界面占用，换一个组合".to_string());
  }
  let previous = read_hotkey(app, action);
  let mut manager = app.global_shortcut_manager();
  if let Some(old) = &previous {
    if old != &target {
      let _ = manager.unregister(old);
    }
  }
  let registered = bind_global_hotkey(&mut manager, app, &target, action);
  if registered {
    store_hotkey(app, action, Some(target));
    return Ok(());
  }
  // 注册失败：把旧键还回去，保证用户还能用
  if let Some(old) = previous {
    let _ = bind_global_hotkey(&mut manager, app, &old, action);
  }
  Err("快捷键被占用，请用备用入口".to_string())
}

fn bind_global_hotkey<M: GlobalShortcutManager>(
  manager: &mut M,
  app: &AppHandle,
  combo: &str,
  action: HotkeyAction,
) -> bool {
  match action {
    HotkeyAction::Capture => {
      let handle = app.clone();
      manager
        .register(combo, move || {
          let _ = open_capture_overlay(&handle);
        })
        .is_ok()
    }
    HotkeyAction::OpenMain => {
      let handle = app.clone();
      manager
        .register(combo, move || {
          let _ = show_main(&handle, None);
        })
        .is_ok()
    }
  }
}

fn read_hotkey(app: &AppHandle, action: HotkeyAction) -> Option<String> {
  match app.try_state::<HotkeyLock>() {
    None => None,
    Some(state) => match state.0.lock() {
      Ok(guard) => slot(&guard, action).clone(),
      Err(poisoned) => slot(&poisoned.into_inner(), action).clone(),
    },
  }
}

fn store_hotkey(app: &AppHandle, action: HotkeyAction, value: Option<String>) {
  if let Some(state) = app.try_state::<HotkeyLock>() {
    match state.0.lock() {
      Ok(mut guard) => *slot_mut(&mut guard, action) = value,
      Err(poisoned) => *slot_mut(&mut poisoned.into_inner(), action) = value,
    }
  }
}

/// 两个热键的**实际注册**快照（read_hotkey 是唯一事实来源，不另存一份判断）
pub fn hotkey_status(app: &AppHandle) -> crate::models::HotkeyStatus {
  crate::models::HotkeyStatus {
    capture: read_hotkey(app, HotkeyAction::Capture),
    main: read_hotkey(app, HotkeyAction::OpenMain),
  }
}

/// 广播实际注册结果给前端（设置页据此标「未生效」）。
/// 启动时主窗口是隐藏的，这条事件可能没人接收 —— 所以前端仍要在启动与进设置页时
/// 主动 get_hotkey_status 兜一次，广播只是让已打开的页面即时更新。
pub fn publish_hotkey_state(app: &AppHandle) {
  let _ = app.emit_all("hotkey-state", hotkey_status(app));
}

fn slot(registry: &HotkeyRegistry, action: HotkeyAction) -> &Option<String> {
  match action {
    HotkeyAction::Capture => &registry.capture,
    HotkeyAction::OpenMain => &registry.main,
  }
}

fn slot_mut(registry: &mut HotkeyRegistry, action: HotkeyAction) -> &mut Option<String> {
  match action {
    HotkeyAction::Capture => &mut registry.capture,
    HotkeyAction::OpenMain => &mut registry.main,
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
      if let Some(item) = handle.try_get_item(*id) {
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

/// 到点提醒的统一标题（调度器 deliver 与托盘共用）
pub const NOTIFY_TITLE: &str = "待办提醒";

pub fn notify_title(app: &AppHandle, title: &str, body: &str) -> bool {
  let identifier = app.config().tauri.bundle.identifier.clone();
  tauri::api::notification::Notification::new(identifier)
    .title(title)
    .body(body)
    .show()
    .is_ok()
}
