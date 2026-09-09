//! 窗口 / 托盘 / 全局快捷键 / 系统通知（不含业务写库逻辑）

use crate::store::Store;
use std::path::Path;
use std::sync::Mutex;
use tauri::{
  AppHandle, CustomMenuItem, GlobalShortcutManager, LogicalSize, Manager, PhysicalPosition,
  Position, SystemTray, SystemTrayMenu, SystemTrayMenuItem, Window, WindowBuilder, WindowEvent,
  WindowUrl,
};

pub const MAIN_LABEL: &str = "main";
pub const TODO_LABEL: &str = "todo";
pub const CAPTURE_LABEL: &str = "capture";
pub const TRAY_ID: &str = "main";

/// 当前注册成功的全局快捷键（换键时先解绑旧的）
#[derive(Default)]
pub struct HotkeyRegistry {
  pub capture: Option<String>,
  pub main: Option<String>,
  pub sticky: Option<String>,
  pub todo: Option<String>,
}

pub struct HotkeyLock(pub Mutex<HotkeyRegistry>);

/// 全局快捷键用途；同一组合键不允许绑两个动作
#[derive(Clone, Copy, PartialEq)]
pub enum HotkeyAction {
  Capture,
  OpenMain,
  NewSticky,
  ToggleTodoFloat,
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

/// 便签位置 key：动态窗用自身 id（ADR-0007；float 主便签已退役）
fn sticky_pos_key(window: &Window) -> String {
  let label = window.label();
  label.strip_prefix("note:").unwrap_or(label).to_string()
}

/// 便签位置记忆（ADR-0007 约束：位置归 runtime.json 的 note_pos map，不进用户数据）
pub fn save_sticky_pos_window(window: &Window) {
  let Ok(pos) = window.outer_position() else {
    return;
  };
  let Some(state) = window.try_state::<Mutex<Store>>() else {
    return;
  };
  let Ok(mut store) = state.lock() else {
    return;
  };
  let key = sticky_pos_key(window);
  let next = [pos.x, pos.y];
  if store.runtime.note_pos.get(&key) == Some(&next) {
    return; // 位置没变不落盘
  }
  store.runtime.note_pos.insert(key, next);
  let _ = store.save_runtime_only();
}

pub fn note_label(id: &str) -> String {
  format!("note:{id}")
}

// 主线程标记：setup（事件循环未启动）里建窗走内联路径才可靠；其余线程一律走
// 「帮手线程 + 6s 超时」。背景（2026-09-08 真机复盘）：WebView2 层建窗偶发无限挂起，
// 挂在 sync 命令里 = 事件循环线程被阻塞 = 全应用按钮假死（"点击便签后按钮全失效"根因）。
use std::sync::OnceLock;
use std::thread::ThreadId;

static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

pub fn mark_main_thread() {
  let _ = MAIN_THREAD.set(std::thread::current().id());
}

fn is_main_thread() -> bool {
  MAIN_THREAD.get() == Some(&std::thread::current().id())
}

/// 便签窗尺寸：展开态纸片 380×456；缩小态置顶悬浮文本条 380×40
pub const NOTE_EXPAND_SIZE: (f64, f64) = (380.0, 456.0);
pub const NOTE_MINI_SIZE: (f64, f64) = (380.0, 40.0);

/// 便签窗 builder 统一配置（在线程内构造：WindowBuilder 非 Send，不能跨线程携带）。
/// 常驻置顶无开关（sticky-separation）；尺寸按缩小态标记二选一。
fn note_builder<'a>(app: &'a AppHandle, label: &'a str, mini: bool) -> WindowBuilder<'a> {
  let (w, h) = if mini { NOTE_MINI_SIZE } else { NOTE_EXPAND_SIZE };
  WindowBuilder::new(app, label, WindowUrl::App("float.html".into()))
    .title("便签")
    .inner_size(w, h)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
}

/// 建窗统一超时包装：主线程内联；其余线程经帮手线程带 6s 超时。
/// 超时后顺手探测事件循环是否存活（主窗 is_visible 同样要过循环队列，
/// 3s 内回不来即循环已卡死），结果记 events.jsonl —— 区分「派发丢失」与「循环内卡死」。
/// `err_event` 是埋点事件名（便签 sticky_window_error / 待办悬浮窗 todo_window_error）。
fn build_with_timeout<F>(
  app: &AppHandle,
  label: String,
  err_event: &str,
  build: F,
) -> Result<tauri::Window, String>
where
  F: Fn(&AppHandle, &str) -> Result<tauri::Window, String> + Send + 'static,
{
  if is_main_thread() {
    return build(app, &label);
  }
  let app2 = app.clone();
  let label2 = label.clone();
  let event = err_event.to_string();
  let (tx, rx) = std::sync::mpsc::channel();
  std::thread::Builder::new()
    .name(format!("win-build:{label}"))
    .spawn(move || {
      let _ = tx.send(build(&app2, &label2));
    })
    .map_err(|_| "建窗线程启动失败".to_string())?;
  match rx.recv_timeout(std::time::Duration::from_secs(6)) {
    Ok(result) => result,
    Err(_) => {
      let app2 = app.clone();
      let (tx2, rx2) = std::sync::mpsc::channel::<bool>();
      std::thread::spawn(move || {
        let alive = app2
          .get_window(MAIN_LABEL)
          .map(|w| w.is_visible().unwrap_or(false))
          .unwrap_or(false);
        let _ = tx2.send(alive);
      });
      let loop_alive = rx2
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap_or(false);
      crate::telemetry::record_str(
        &event,
        &[
          ("label", label.as_str()),
          ("err", "timeout_6s"),
          ("loop_alive", if loop_alive { "yes" } else { "no" }),
        ],
      );
      Err("窗口创建超时，请重试；若连续出现请重启应用".to_string())
    }
  }
}

/// 记建窗失败的统一埋点格式
fn log_build_error(event: &str, label: &str, error: &str) -> String {
  crate::telemetry::record_str(event, &[("label", label), ("err", error)]);
  format!("窗口创建失败：{error}")
}

/// 便签建窗入口（走统一超时包装，埋点沿用 sticky_window_error）
fn build_note_window(app: &AppHandle, label: String, mini: bool) -> Result<tauri::Window, String> {
  build_with_timeout(app, label, "sticky_window_error", move |app, label| {
    note_builder(app, label, mini)
      .build()
      .map_err(|e| log_build_error("sticky_window_error", label, &format!("{e}")))
  })
}

/// 打开（或创建）便签窗；窗口形态（展开/缩小）由 mini 决定，创建即显示
/// （缩小态本身就是可见的悬浮条——旧的「收起=隐藏」语义已退役）。
/// 位置：stored_pos 有存档且在屏内则恢复。stored_pos 由调用方在自己的锁作用域内
/// 取好传入——本函数不碰全局 Store 锁，杜绝"命令持锁 → 这里重入同锁"的死锁（v2.2 修复）。
pub fn open_note_window(
  app: &AppHandle,
  id: &str,
  mini: bool,
  stored_pos: Option<[i32; 2]>,
) -> Result<(), String> {
  let label = note_label(id);
  if let Some(existing) = app.get_window(&label) {
    let _ = existing.show();
    return Ok(());
  }
  let window = build_note_window(app, label.clone(), mini)?;
  if let Some([x, y]) = stored_pos {
    if capture_pos_on_screen(&window, x, y) {
      let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    }
  }
  let _ = window.show();
  Ok(())
}

/// 新建一张便签（sticky-separation + Phase 2 知识便签）：设置页命令与全局热键共用的唯一入口。
/// 建窗可能被 WebView2 层挂起 → 调用方绝不能在事件循环线程上
/// （命令是 async 跑在线程池；热键回调 spawn 后台线程）。
/// 失败回滚已入库便签，不留「数据有窗没有」的幽灵行。
pub fn create_note(
  app: &AppHandle,
  kb_ref: Option<String>,
) -> Result<crate::models::StickyNote, String> {
  use crate::models::{StickyNote, StoreEvent, STICKY_CAP, STICKY_KB_CAP};
  let Some(state) = app.try_state::<Mutex<Store>>() else {
    return Err("应用还没准备好".to_string());
  };
  let mut guard = state.lock().map_err(|_| "应用状态忙，请重试".to_string())?;
  guard.ensure_writable()?;

  // 分池上限检查
  if kb_ref.is_some() {
    let kb_count = guard.data.stickies.iter().filter(|s| s.kb_ref.is_some()).count();
    if kb_count >= STICKY_KB_CAP {
      return Err(format!("知识便签已达上限（{}张），请先拆掉不需要的", STICKY_KB_CAP));
    }
  } else {
    let free_count = guard.data.stickies.iter().filter(|s| s.kb_ref.is_none()).count();
    if free_count >= STICKY_CAP {
      return Err(format!("自由便签已达上限（{}张），请先关闭一张", STICKY_CAP));
    }
  }

  let note = guard.add_sticky(StickyNote {
    id: crate::models::new_id("n"),
    kb_ref,
    ..Default::default()
  });
  if let Err(error) = guard.save() {
    return Err(error);
  }
  drop(guard);

  // 级联错位基准：任意一张已存在便签窗的实时位置（float 退役后无固定锚点）
  let anchor = app
    .windows()
    .values()
    .find(|w| w.label().starts_with("note:"))
    .and_then(|w| w.outer_position().ok())
    .map(|p| [p.x, p.y]);
  if let Err(e) = open_note_window(app, &note.id, note.mini, None) {
    if let Some(state) = app.try_state::<Mutex<Store>>() {
      if let Ok(mut guard) = state.lock() {
        let _ = guard.delete_sticky(&note.id);
        let _ = guard.save();
      }
    }
    return Err(e);
  }
  if let (Some([x, y]), Some(w)) = (anchor, app.get_window(&note_label(&note.id))) {
    let _ = w.set_position(Position::Physical(PhysicalPosition::new(x + 32, y + 32)));
    save_sticky_pos_window(&w);
  }
  crate::telemetry::record_str("sticky_create", &[("type", "free")]);
  let _ = app.emit_all("store-changed", StoreEvent::changed("stickies"));
  Ok(note)
}

/// 便签窗形态切换：缩小 = 380×40 悬浮条；展开 = 380×456 纸片并聚焦。
/// resizable(false) 只挡用户手拖，编程 set_size 不受限（capture_resize 同款）。
pub fn apply_note_shape(app: &AppHandle, id: &str, mini: bool) -> Result<(), String> {
  let window = window_of(app, &note_label(id))?;
  let (w, h) = if mini { NOTE_MINI_SIZE } else { NOTE_EXPAND_SIZE };
  window
    .set_size(LogicalSize::new(w, h))
    .map_err(|_| "便签窗口改尺寸失败".to_string())?;
  if !mini {
    let _ = window.set_focus();
  }
  Ok(())
}

// ---------------------------------------------------------------- 待办悬浮窗（todo-float）

/// 待办悬浮窗 builder：380×456 纸片（便签纸感家族），置顶按设置（窗内 Pin 可切）。
/// 在线程内构造：WindowBuilder 非 Send，不能跨线程携带。
fn todo_builder<'a>(app: &'a AppHandle, pinned: bool) -> WindowBuilder<'a> {
  WindowBuilder::new(app, TODO_LABEL, WindowUrl::App("todo.html".into()))
    .title("待办悬浮窗")
    .inner_size(NOTE_EXPAND_SIZE.0, NOTE_EXPAND_SIZE.1)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(pinned)
    .skip_taskbar(true)
    .visible(false)
}

fn todo_pinned_of(app: &AppHandle) -> bool {
  app
    .try_state::<Mutex<Store>>()
    .and_then(|s| s.lock().ok().map(|g| g.data.settings.todo_float_pinned))
    .unwrap_or(true)
}

/// 显隐状态落 runtime.json（行为状态归运行态，ADR-0005 分层）
fn set_todo_float_visible(app: &AppHandle, visible: bool) {
  if let Some(state) = app.try_state::<Mutex<Store>>() {
    if let Ok(mut guard) = state.lock() {
      if guard.runtime.todo_float_visible != visible {
        guard.runtime.todo_float_visible = visible;
        let _ = guard.save_runtime_only();
      }
    }
  }
}

fn todo_float_visible_of(app: &AppHandle) -> bool {
  app
    .try_state::<Mutex<Store>>()
    .and_then(|s| s.lock().ok().map(|g| g.runtime.todo_float_visible))
    .unwrap_or(true)
}

/// 打开（或唤起）待办悬浮窗；置顶跟随设置；位置记忆 note_pos["todo"]
pub fn open_todo_float(app: &AppHandle) -> Result<(), String> {
  let pinned = todo_pinned_of(app);
  if app.get_window(TODO_LABEL).is_none() {
    let stored = app
      .try_state::<Mutex<Store>>()
      .and_then(|s| s.lock().ok().and_then(|g| g.runtime.note_pos.get("todo").copied()));
    let window = build_with_timeout(app, TODO_LABEL.to_string(), "todo_window_error", move |app, label| {
      todo_builder(app, pinned)
        .build()
        .map_err(|e| log_build_error("todo_window_error", label, &format!("{e}")))
    })?;
    if let Some([x, y]) = stored {
      if capture_pos_on_screen(&window, x, y) {
        let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
      }
    }
  }
  show_window(app, TODO_LABEL)?;
  set_todo_float_visible(app, true);
  sync_tray_todo(app, true);
  Ok(())
}

/// 隐藏待办悬浮窗（✕ / 热键 / 托盘同款）：只藏窗，数据常驻；显隐状态落盘
pub fn hide_todo_float(app: &AppHandle) -> Result<(), String> {
  hide_window(app, TODO_LABEL)?;
  set_todo_float_visible(app, false);
  sync_tray_todo(app, false);
  Ok(())
}

/// 呼出 ⇄ 隐藏（热键/托盘共用）
pub fn toggle_todo_float(app: &AppHandle) -> Result<(), String> {
  let visible = app
    .get_window(TODO_LABEL)
    .map(|w| w.is_visible().unwrap_or(false))
    .unwrap_or(false);
  if visible {
    hide_todo_float(app)
  } else {
    open_todo_float(app)
  }
}

/// 置顶切换的窗口同步（设置来源：set_settings / todoFloatPinned）
pub fn apply_todo_float_pinned(app: &AppHandle, pinned: bool) {
  if let Ok(window) = window_of(app, TODO_LABEL) {
    let _ = window.set_always_on_top(pinned);
  }
}

// ---------------------------------------------------------------- 快速记录浮条

/// 快速记录条宽度：创建（create_capture_window）与动态改高（capture_resize）共用一个常量，
/// 避免两处字面量改一处漏一处（同 center_capture 改用 outer_size 的教训）
pub const CAPTURE_W: f64 = 560.0;
/// 动态高度安全下界：低于任何真实内容高度（空输入 ≈45），防退化成 0 高窗
pub const CAPTURE_MIN_H: f64 = 36.0;
/// 动态高度安全上界：最挤内容（输入行+chips+错误行）≈90，200 已宽裕
pub const CAPTURE_MAX_H: f64 = 200.0;

/// 前端上报的内容高度 → 合法窗口高度。
/// NaN 必须先挡掉：f64::clamp 遇 NaN 原样返回 NaN，会毒化 set_size；±Inf 由 clamp 收边界。
pub fn sanitize_capture_height(height: f64) -> f64 {
  if height.is_nan() {
    return CAPTURE_MIN_H;
  }
  height.clamp(CAPTURE_MIN_H, CAPTURE_MAX_H)
}

fn create_capture_window(app: &AppHandle) -> Result<Window, String> {
  // 初始高度只是首帧值：窗口随配置在启动即创建（隐藏），前端挂载后由 capture_resize 按内容校正
  WindowBuilder::new(app, CAPTURE_LABEL, WindowUrl::App("capture.html".into()))
    .title("快速记录")
    .inner_size(CAPTURE_W, 46.0)
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
    .and_then(|s| s.lock().ok().and_then(|g| g.runtime.capture_pos));
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
  if store.runtime.capture_pos == next {
    return;
  }
  store.runtime.capture_pos = next;
  let _ = store.save_runtime_only();
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
  set_window_corner(window, DWMWCP_ROUND);
}

// ── 主窗自定义标题栏（方案B）─────────────────────────────────────────────
// main 窗 decorations:false 后栏归前端（title-bar 组件走 Tauri window API），
// Win11 也不再自动圆角 —— 转交 DWM：还原态圆角、最大化态收直角（最大化还圆角
// 会在屏幕四角露出桌面缺口，Win11 原生窗口同款行为）。
pub const DWMWCP_ROUND: u32 = 2;
pub const DWMWCP_DONOTROUND: u32 = 1;

pub fn corner_preference_for(maximized: bool) -> u32 {
  if maximized {
    DWMWCP_DONOTROUND
  } else {
    DWMWCP_ROUND
  }
}

#[cfg(target_os = "windows")]
fn set_window_corner(window: &Window, preference: u32) {
  const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
  if let Ok(hwnd) = window.hwnd() {
    unsafe {
      DwmSetWindowAttribute(
        hwnd.0 as *mut core::ffi::c_void,
        DWMWA_WINDOW_CORNER_PREFERENCE,
        &preference,
        core::mem::size_of::<u32>() as u32,
      );
    }
  }
}

#[cfg(target_os = "windows")]
fn apply_main_rounding(window: &Window, maximized: bool) {
  set_window_corner(window, corner_preference_for(maximized));
}

#[cfg(not(target_os = "windows"))]
fn apply_main_rounding(_window: &Window, _maximized: bool) {}

/// 主窗框架维护：挂初始圆角，并在尺寸变化（最大化 ↔ 还原）时切换圆角偏好。
/// Resized 在交互缩放期间高频触发，但这里只是写一个 DWM 标志位，无需防抖。
pub fn watch_main_window_frame(window: &Window) {
  apply_main_rounding(window, false);
  let watched = window.clone();
  window.on_window_event(move |event| {
    if matches!(event, WindowEvent::Resized(_)) {
      let maximized = watched.is_maximized().unwrap_or(false);
      apply_main_rounding(&watched, maximized);
    }
  });
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

/// 动态改高（方案 B）：前端 ResizeObserver 按内容实际高度上报，窗口顶部锚定、向下伸缩。
/// resizable(false) 只挡用户手拖，编程 set_size 不受限；隐藏窗口也接受 set_size
/// （启动即创建时是隐藏的，前端挂载后先改高、首开即是正确尺寸）。
pub fn capture_resize(app: &AppHandle, height: f64) -> Result<(), String> {
  let window = app
    .get_window(CAPTURE_LABEL)
    .ok_or_else(|| "快速记录条未就绪".to_string())?;
  let h = sanitize_capture_height(height);
  window
    .set_size(LogicalSize::new(CAPTURE_W, h))
    .map_err(|_| "快速记录条改高失败".to_string())
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

/// 注册 / 换绑 / 解绑「新建便签」热键；None 或空串 = 解绑（sticky-separation）
pub fn register_sticky_hotkey(app: &AppHandle, combo: &Option<String>) -> Result<(), String> {
  let result = sticky_hotkey_inner(app, combo);
  publish_hotkey_state(app);
  result
}

/// 注册 / 换绑 / 解绑「待办悬浮窗」呼出热键；None 或空串 = 解绑（todo-float）
pub fn register_todo_hotkey(app: &AppHandle, combo: &Option<String>) -> Result<(), String> {
  let result = todo_hotkey_inner(app, combo);
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

fn sticky_hotkey_inner(app: &AppHandle, combo: &Option<String>) -> Result<(), String> {
  match combo.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
    Some(target) => register_global_hotkey(app, target, HotkeyAction::NewSticky),
    None => {
      if let Some(old) = read_hotkey(app, HotkeyAction::NewSticky) {
        let _ = app.global_shortcut_manager().unregister(&old);
      }
      store_hotkey(app, HotkeyAction::NewSticky, None);
      Ok(())
    }
  }
}

fn todo_hotkey_inner(app: &AppHandle, combo: &Option<String>) -> Result<(), String> {
  match combo.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
    Some(target) => register_global_hotkey(app, target, HotkeyAction::ToggleTodoFloat),
    None => {
      if let Some(old) = read_hotkey(app, HotkeyAction::ToggleTodoFloat) {
        let _ = app.global_shortcut_manager().unregister(&old);
      }
      store_hotkey(app, HotkeyAction::ToggleTodoFloat, None);
      Ok(())
    }
  }
}

fn register_global_hotkey(app: &AppHandle, combo: &str, action: HotkeyAction) -> Result<(), String> {
  let target = combo.trim().to_string();
  if target.is_empty() {
    return Err("快捷键不能为空".to_string());
  }
  // 四槽位互斥：任一其他功能绑了这个组合键都拒绝
  for rival in [HotkeyAction::Capture, HotkeyAction::OpenMain, HotkeyAction::NewSticky, HotkeyAction::ToggleTodoFloat] {
    if rival == action {
      continue;
    }
    if read_hotkey(app, rival).as_deref() == Some(target.as_str()) {
      return Err("这个快捷键已被其他功能占用，换一个组合".to_string());
    }
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
    HotkeyAction::NewSticky => {
      let handle = app.clone();
      manager
        .register(combo, move || {
          // 建窗可能被 WebView2 层挂起（真机复盘）：绝不能在事件循环线程上执行，
          // 甩到后台线程，6s 超时机制在 build_with_timeout 里兜底。
          // register 的闭包是 Fn（可多次触发），handle 只能克隆不能移动。
          let handle = handle.clone();
          std::thread::spawn(move || {
            let _ = create_note(&handle, None);
          });
        })
        .is_ok()
    }
    HotkeyAction::ToggleTodoFloat => {
      let handle = app.clone();
      manager
        .register(combo, move || {
          // 首次呼出会建窗（可能挂起），同 NewSticky 甩后台线程
          let handle = handle.clone();
          std::thread::spawn(move || {
            let _ = toggle_todo_float(&handle);
          });
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

/// 槽位当前真正生效的组合键（设置页「当前绑定」的事实来源）
pub fn bound_combo(app: &AppHandle, slot: crate::ports::hotkeys::HotkeySlot) -> Option<String> {
  let action = match slot {
    crate::ports::hotkeys::HotkeySlot::Capture => HotkeyAction::Capture,
    crate::ports::hotkeys::HotkeySlot::Main => HotkeyAction::OpenMain,
    crate::ports::hotkeys::HotkeySlot::Sticky => HotkeyAction::NewSticky,
    crate::ports::hotkeys::HotkeySlot::TodoFloat => HotkeyAction::ToggleTodoFloat,
  };
  read_hotkey(app, action)
}

fn store_hotkey(app: &AppHandle, action: HotkeyAction, value: Option<String>) {
  if let Some(state) = app.try_state::<HotkeyLock>() {
    match state.0.lock() {
      Ok(mut guard) => *slot_mut(&mut guard, action) = value,
      Err(poisoned) => *slot_mut(&mut poisoned.into_inner(), action) = value,
    }
  }
}

/// 四个热键的**实际注册**快照（read_hotkey 是唯一事实来源，不另存一份判断）
pub fn hotkey_status(app: &AppHandle) -> crate::models::HotkeyStatus {
  crate::models::HotkeyStatus {
    capture: read_hotkey(app, HotkeyAction::Capture),
    main: read_hotkey(app, HotkeyAction::OpenMain),
    sticky: read_hotkey(app, HotkeyAction::NewSticky),
    todo: read_hotkey(app, HotkeyAction::ToggleTodoFloat),
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
    HotkeyAction::NewSticky => &registry.sticky,
    HotkeyAction::ToggleTodoFloat => &registry.todo,
  }
}

fn slot_mut(registry: &mut HotkeyRegistry, action: HotkeyAction) -> &mut Option<String> {
  match action {
    HotkeyAction::Capture => &mut registry.capture,
    HotkeyAction::OpenMain => &mut registry.main,
    HotkeyAction::NewSticky => &mut registry.sticky,
    HotkeyAction::ToggleTodoFloat => &mut registry.todo,
  }
}

// ---------------------------------------------------------------- 托盘

pub fn tray_menu(todo_visible: bool) -> SystemTrayMenu {
  let mut todo_item = CustomMenuItem::new("todo_float", "待办悬浮窗");
  if todo_visible {
    todo_item = todo_item.selected();
  }
  SystemTrayMenu::new()
    .add_item(CustomMenuItem::new("open_main", "打开主界面"))
    .add_item(CustomMenuItem::new("capture", "快速记录"))
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(todo_item)
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(CustomMenuItem::new("sticky_new", "新建便签"))
    .add_item(CustomMenuItem::new("sticky_show_all", "显示全部便签"))
    .add_item(CustomMenuItem::new("sticky_hide_all", "隐藏全部便签"))
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(CustomMenuItem::new("reminders", "管理提醒"))
    .add_item(CustomMenuItem::new("weekly", "本周汇总导出"))
    .add_item(CustomMenuItem::new("data_dir", "打开数据文件夹"))
    .add_native_item(SystemTrayMenuItem::Separator)
    .add_item(CustomMenuItem::new("quit", "退出"))
}

pub fn build_tray(todo_visible: bool) -> SystemTray {
  SystemTray::new().with_id(TRAY_ID).with_menu(tray_menu(todo_visible))
}

/// 待办悬浮窗显隐变化后刷新托盘勾选
pub fn sync_tray_todo(app: &AppHandle, visible: bool) {
  if let Some(handle) = app.tray_handle_by_id(TRAY_ID) {
    if let Some(item) = handle.try_get_item("todo_float") {
      let _ = item.set_selected(visible);
    }
  }
}

/// 便签 id 的锁内快照（托盘/热键等后台动作先拿 id 列表，再逐个操作窗口，
/// 不把 Store 锁带进窗口操作——v2.2 死锁教训）
fn note_ids(app: &AppHandle) -> Vec<String> {
  app
    .try_state::<Mutex<Store>>()
    .and_then(|s| s.lock().ok().map(|g| g.data.stickies.iter().map(|n| n.id.clone()).collect()))
    .unwrap_or_default()
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
    "sticky_new" => {
      let handle = app.clone();
      std::thread::spawn(move || {
        let _ = create_note(&handle, None);
      });
    }
    "todo_float" => {
      let next = !todo_float_visible_of(app);
      let result = if next { open_todo_float(app) } else { hide_todo_float(app) };
      if result.is_ok() {
        sync_tray_todo(app, next);
      }
    }
    "sticky_show_all" => {
      for id in note_ids(app) {
        let _ = show_window(app, &note_label(&id));
      }
    }
    "sticky_hide_all" => {
      for id in note_ids(app) {
        let _ = hide_window(app, &note_label(&id));
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

#[cfg(test)]
mod capture_resize_tests {
  use super::*;

  #[test]
  fn height_in_range_passes_through() {
    assert_eq!(sanitize_capture_height(45.0), 45.0);
    assert_eq!(sanitize_capture_height(68.0), 68.0);
  }

  #[test]
  fn height_out_of_range_clamped_to_bounds() {
    assert_eq!(sanitize_capture_height(0.0), CAPTURE_MIN_H);
    assert_eq!(sanitize_capture_height(-5.0), CAPTURE_MIN_H);
    assert_eq!(sanitize_capture_height(10_000.0), CAPTURE_MAX_H);
  }

  #[test]
  fn height_non_finite_falls_back_to_safe_bounds() {
    assert_eq!(sanitize_capture_height(f64::NAN), CAPTURE_MIN_H);
    assert_eq!(sanitize_capture_height(f64::INFINITY), CAPTURE_MAX_H);
    assert_eq!(sanitize_capture_height(f64::NEG_INFINITY), CAPTURE_MIN_H);
  }
}

#[cfg(test)]
mod main_titlebar_tests {
  use super::*;

  #[test]
  fn restored_window_gets_round_corners() {
    assert_eq!(corner_preference_for(false), 2, "还原态 = DWMWCP_ROUND");
  }

  #[test]
  fn maximized_window_gets_square_corners() {
    assert_eq!(corner_preference_for(true), 1, "最大化态 = DWMWCP_DONOTROUND");
  }
}
