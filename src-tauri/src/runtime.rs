//! 窗口 / 托盘 / 全局快捷键 / 系统通知（不含业务写库逻辑）

use crate::store::Store;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;
use tauri::{
  AppHandle, CustomMenuItem, GlobalShortcutManager, LogicalSize, Manager, PhysicalPosition,
  Position, SystemTray, SystemTrayMenu, SystemTrayMenuItem, Window, WindowBuilder, WindowEvent,
  WindowUrl,
};

pub const MAIN_LABEL: &str = "main";
pub const TODO_LABEL: &str = "todo";
pub const CAPTURE_LABEL: &str = "capture";
pub const TRAY_ID: &str = "main";

/// 本次启动实际使用的 WebView2 用户数据目录。
/// 由 main 在 `Builder::build()` 之前探测并注入；所有建窗路径共用同一份，
/// 避免「有的窗口用默认目录、有的用回退目录」把浏览器进程拆成两套。
static WEBVIEW2_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 启动早期调用一次：注入已探测过的可写目录（幂等，重复调用以首次为准）。
pub fn set_webview2_dir(dir: PathBuf) {
  let _ = WEBVIEW2_DIR.set(dir);
}

/// 给建窗器挂显式 `data_directory`：conf 的 WindowConfig 没有这个字段，
/// 只有代码建窗能指定，所以主窗/快速记录条都改成 setup 里建（见 main.rs）。
fn with_webview2_dir(builder: WindowBuilder<'_>) -> WindowBuilder<'_> {
  match WEBVIEW2_DIR.get() {
    Some(dir) => builder.data_directory(dir.clone()),
    None => builder,
  }
}

/// 启动早期探测 WebView2 用户数据目录（fix-known-defects 7.1）。
///
/// WebView2 创建环境对目录权限极敏感：目录不可写/被占用时
/// `CreateCoreWebView2EnvironmentWithOptions` 直接失败（0x800700AA / 拒绝访问 / E_UNEXPECTED），
/// 而失败点在 `Builder::build()` 里、早于 setup 与埋点，用户只能看到一条 panic。
/// 所以在建窗之前先把目录定下来，按「用户数据目录 → Tauri 旧默认 → 临时目录」降级：
/// 用户数据目录与 data.json 同盘同权限，是最可能可写、也最便于用户自查的位置。
pub fn resolve_webview2_dir() -> Result<PathBuf, String> {
  // 沙箱/受限上下文是这套目录探查唯一真正会让「所有候选都不可写」的场景，
  // 而 WebView2 在沙箱里另有一道硬墙（浏览器进程走命名管道 IPC，被禁即 E_UNEXPECTED），
  // 所以先把运行上下文打印出来，别让用户以为是磁盘或权限配置问题。
  let dsh_shell = std::env::var("DSH_SHELL").ok();
  let dsh_session = std::env::var("DSH_SESSION_ID").ok();
  // 目录全不可写有两种可能：进程被沙箱降权，或进程根本不是当前登录用户（例如被提权器/服务拉起）。
  // 把「谁在跑、可写区域指向哪」一并打出来，免得只凭「不可写」三个字在权限策略里乱翻。
  eprintln!(
    "webview2 启动环境：user={:?} userprofile={:?} appdata={:?} temp={} dsh_shell={:?} dsh_session={:?}",
    std::env::var("USERNAME").ok(),
    std::env::var("USERPROFILE").ok(),
    std::env::var("APPDATA").ok(),
    std::env::temp_dir().display(),
    dsh_shell,
    dsh_session
  );

  let mut candidates: Vec<PathBuf> = Vec::new();
  // 1) %APPDATA%\todo-list\webview2：与 data.json 同目录，权限随数据目录走
  candidates.push(crate::store::data_dir().join("webview2"));
  // 2) Tauri 1.x 的默认位置（%LOCALAPPDATA%\<identifier>），保留给老用户继续复用既有缓存
  if let Some(local) = dirs::data_local_dir() {
    candidates.push(local.join("com.todo.list"));
  }
  // 3) 最后兜底：临时目录（磁盘满/漫游配置异常时仍能启动，代价是缓存不持久）
  candidates.push(std::env::temp_dir().join("kettd-webview2"));

  for dir in candidates {
    match probe_writable(&dir) {
      Ok(()) => {
        eprintln!("webview2 候选目录 {} 可写=true", dir.display());
        return Ok(dir);
      }
      // 启动失败时这是唯一能留下线索的地方（此时埋点还没初始化、也没有窗口）：
      // 必须带上系统错误原文，否则「不可写」三个字分不清是权限、路径还是磁盘。
      Err(reason) => eprintln!("webview2 候选目录 {} 不可写：{reason}", dir.display()),
    }
  }

  if dsh_shell.is_some() || dsh_session.is_some() {
    return Err(
      "检测到当前进程跑在 DSH 沙箱里（DSH_SHELL/DSH_SESSION_ID），沙箱禁止 WebView2 所需的命名管道与用户目录写入；请在 DSH 之外的终端（Windows Terminal / VS Code 等）运行 npm run tauri dev"
        .to_string(),
    );
  }
  Err("找不到可写的 WebView2 数据目录，请检查磁盘空间或权限后重试".to_string())
}

/// 真写一次再删：只 `create_dir_all` 不够 —— WebView2 会在目录里建 SQLite/本地存储，
/// 目录存在但文件不可写时同样起不来。失败原因原样带回，供启动日志定位。
fn probe_writable(dir: &Path) -> Result<(), String> {
  std::fs::create_dir_all(dir).map_err(|error| format!("建目录失败 {error}"))?;
  let probe = dir.join(".kettd-write-probe");
  std::fs::write(&probe, b"ok").map_err(|error| format!("写探针失败 {error}"))?;
  let _ = std::fs::remove_file(&probe);
  Ok(())
}

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

/// 主窗 builder：与 `tauri.conf.json` 的 main 节同参（1100×720 / min 1024×640 /
/// decorations:false / 初始隐藏），仅供缺窗重建复用，不参与首启（首启仍由配置建窗）
fn main_builder<'a>(app: &'a AppHandle, label: &'a str) -> WindowBuilder<'a> {
  WindowBuilder::new(app, label, WindowUrl::App("index.html".into()))
    .title("Kettd")
    .inner_size(1100.0, 720.0)
    .min_inner_size(1024.0, 640.0)
    .resizable(true)
    .decorations(false)
    .center()
    .visible(false)
}

/// 打开主界面；route 为前端内部路由（如 /planned/reminders）
pub fn show_main(app: &AppHandle, route: Option<&str>) -> Result<(), String> {
  let window = match app.get_window(MAIN_LABEL) {
    Some(window) => window,
    None => return restore_main_window(app, route),
  };
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

/// 主窗已不在（历史 ✕ 走 appWindow.close() = 销毁，绕不过 prevent_close）时的恢复通道。
/// 建窗丢给帮手线程且立刻返回：托盘分发跑在事件循环线程上，在这里等建窗回执会和新窗口
/// 互相卡死（回执只有循环空出来才处理得了）。恢复后的深链 route 尽力送达，页面尚未
/// 监听时落到默认视图。
fn restore_main_window(app: &AppHandle, route: Option<&str>) -> Result<(), String> {
  crate::telemetry::record_str("main_window_restore", &[("from", "tray_or_hotkey")]);
  let app2 = app.clone();
  let route2 = route.map(|path| path.to_string());
  std::thread::Builder::new()
    .name("main-window-restore".into())
    .spawn(move || {
      let rebuilt = build_with_timeout(
        &app2,
        MAIN_LABEL.to_string(),
        "main_window_restore",
        |app, label| {
          main_builder(app, label)
            .build()
            .map_err(|e| log_build_error("main_window_restore", label, &format!("{e}")))
        },
      );
      match rebuilt {
        Ok(window) => {
          let _ = window.show();
          let _ = window.set_focus();
          if let Some(path) = route2 {
            let _ = window.emit("navigate", path);
          }
        }
        Err(error) => {
          notify_title(&app2, "Kettd", &error);
        }
      }
    })
    .map_err(|_| "主界面恢复失败，请重启应用".to_string())?;
  Ok(())
}

/// 窗口呼出的失败上报：错误本身就是用户可见文案（中文短句，不含路径与堆栈）
fn report_window_error(app: &AppHandle, result: Result<(), String>) {
  if let Err(error) = result {
    notify_title(app, "Kettd", &error);
  }
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
  with_webview2_dir(
    WindowBuilder::new(app, label, WindowUrl::App("float.html".into()))
      .title("便签")
      .inner_size(w, h)
      .resizable(false)
      .decorations(false)
      .transparent(true)
      .always_on_top(true)
      .skip_taskbar(true)
      .visible(false),
  )
}

/// 主窗：从 tauri.conf.json 移到代码建窗，唯一目的是能传 `data_directory`
/// （WindowConfig 没有该字段，而 WebView2 默认目录不可写时整个 `build()` 会失败）。
/// 参数保持 conf 原值：1100×720 / 最小 1024×640 / 无边框 / 启动隐藏（由托盘或热键唤起）。
///
/// 必须传 `&App`（setup 里的 `app`）而不是 `AppHandle`：前者走 Runtime 直建，
/// 后者是 Dispatch 消息、要等事件循环，而 setup 阶段事件循环还没开始 —— 会死锁。
pub fn create_main_window<M: tauri::Manager<tauri::Wry>>(manager: &M) -> Result<Window, String> {
  with_webview2_dir(
    WindowBuilder::new(manager, MAIN_LABEL, WindowUrl::App("index.html".into()))
      .title("Kettd")
      .inner_size(1100.0, 720.0)
      .min_inner_size(1024.0, 640.0)
      .resizable(true)
      .decorations(false)
      .visible(false)
      .center(),
  )
  .build()
  .map_err(|error| {
    eprintln!("主窗建窗失败：{error:?}");
    crate::telemetry::record_str("main_window_error", &[("err", &format!("{error}"))]);
    "主窗口创建失败，请重启应用；若反复出现请检查数据目录权限".to_string()
  })
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
    let window = note_builder(app, label, mini)
      .build()
      .map_err(|e| log_build_error("sticky_window_error", label, &format!("{e}")))?;
    // 建窗成功即关 WebView2 默认右键菜单（#3）；失败只记埋点，不影响便签窗
    disable_default_context_menu(&window);
    Ok(window)
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
  use crate::models::{visible_sticky_count, StickyNote, StoreEvent, STICKY_CAP};
  let Some(state) = app.try_state::<Mutex<Store>>() else {
    return Err("应用还没准备好".to_string());
  };
  let mut guard = state.lock().map_err(|_| "应用状态忙，请重试".to_string())?;
  guard.ensure_writable()?;

  // 可见便签合计上限（fix-known-defects 4.2：分池取消，自由 + 知识合起来算；
  // 隐藏便签不计入）。升级首启已有 6 张旧数据不强制隐藏，只挡新建/重新打开。
  if visible_sticky_count(&guard.data.stickies) >= STICKY_CAP {
    return Err(format!("便签最多同时打开 {} 张，请先关闭一张", STICKY_CAP));
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

/// 隐藏便签窗（✕ / Alt+F4 / 列表关闭同一语义）：只藏窗，不销毁、不删数据、不清位置。
/// 窗口不存在视为已隐藏（幂等）。
pub fn hide_note_window(app: &AppHandle, id: &str) {
  if let Some(window) = app.get_window(&note_label(id)) {
    let _ = window.hide();
  }
}

/// 重新打开一张已隐藏（或已可见）的便签（fix-known-defects 4.4，open_sticky 命令入口）。
/// 可见上限只算「可见」便签：隐藏便签重新打开时若已达上限则拒绝，不悄悄挤掉别人。
/// 建窗失败把 visible 回滚为 false（不留「数据可见但没窗」的幽灵态）。
/// 返回最新 StickyNote。锁纪律：open_note_window 在锁外调用（其建窗路径不碰 Store 锁，
/// 但这里仍按铁律 drop 后再建窗，失败回滚时重新加锁）。
pub fn open_note(app: &AppHandle, id: &str) -> Result<crate::models::StickyNote, String> {
  use crate::models::{visible_sticky_count, StoreEvent, STICKY_CAP};
  let Some(state) = app.try_state::<Mutex<Store>>() else {
    return Err("应用还没准备好".to_string());
  };
  let (note, stored_pos, became_visible) = {
    let mut guard = state.lock().map_err(|_| "应用状态忙，请重试".to_string())?;
    let current = guard
      .find_sticky(id)
      .ok_or_else(|| "找不到这张便签".to_string())?;
    let became_visible = !current.visible;
    if became_visible {
      guard.ensure_writable()?;
      if visible_sticky_count(&guard.data.stickies) >= STICKY_CAP {
        return Err(format!("便签最多同时打开 {} 张，请先关闭一张", STICKY_CAP));
      }
      if let Some(note) = guard.find_sticky_mut(id) {
        note.visible = true;
        note.updated_at = crate::models::now_text();
      }
      if let Err(error) = guard.save() {
        if let Some(note) = guard.find_sticky_mut(id) {
          note.visible = false;
        }
        return Err(error);
      }
    }
    let note = guard
      .find_sticky(id)
      .ok_or_else(|| "找不到这张便签".to_string())?;
    let stored_pos = guard.runtime.note_pos.get(id).copied();
    (note, stored_pos, became_visible)
  };

  if let Err(error) = open_note_window(app, &note.id, note.mini, stored_pos) {
    if became_visible {
      // 回滚可见标记：窗口没建成，不能让便签停在「可见」状态（否则重启会再试开）
      if let Ok(mut guard) = state.lock() {
        if let Some(note) = guard.find_sticky_mut(id) {
          note.visible = false;
        }
        let _ = guard.save();
      }
    }
    return Err(error);
  }
  if let Some(window) = app.get_window(&note_label(&note.id)) {
    let _ = window.show();
    let _ = window.set_focus();
  }
  if became_visible {
    let _ = app.emit_all("store-changed", StoreEvent::changed("stickies"));
  }
  Ok(note)
}

// ---------------------------------------------------------------- 待办悬浮窗（todo-float）

/// 待办悬浮窗 builder：完整面板 380×456 / 最小化单行条 380×40（便签纸感家族）；
/// 置顶按设置（窗内 Pin 可切）。在线程内构造：WindowBuilder 非 Send，不能跨线程携带。
fn todo_builder<'a>(app: &'a AppHandle, pinned: bool, mini: bool) -> WindowBuilder<'a> {
  let (w, h) = if mini { NOTE_MINI_SIZE } else { NOTE_EXPAND_SIZE };
  with_webview2_dir(
    WindowBuilder::new(app, TODO_LABEL, WindowUrl::App("todo.html".into()))
      .title("待办悬浮窗")
      .inner_size(w, h)
      .resizable(false)
      .decorations(false)
      .transparent(true)
      .always_on_top(pinned)
      .skip_taskbar(true)
      .visible(false),
  )
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

fn todo_float_mini_of(app: &AppHandle) -> bool {
  app
    .try_state::<Mutex<Store>>()
    .and_then(|s| s.lock().ok().map(|g| g.runtime.todo_float_mini))
    .unwrap_or(false)
}

/// 待办窗形态切换（fix-known-defects 4.7）：最小化 = 380×40 单行条；完整 = 380×456。
pub fn apply_todo_shape(app: &AppHandle, mini: bool) -> Result<(), String> {
  let window = window_of(app, TODO_LABEL)?;
  let (w, h) = if mini { NOTE_MINI_SIZE } else { NOTE_EXPAND_SIZE };
  window
    .set_size(LogicalSize::new(w, h))
    .map_err(|_| "待办悬浮窗改尺寸失败".to_string())?;
  // 只在窗口可见时聚焦：窗口隐藏时 set_focus 无意义，避免把「设置页改形态」变成抢焦点
  if !mini && window.is_visible().unwrap_or(false) {
    let _ = window.set_focus();
  }
  Ok(())
}

/// 最小化态持久化：写 runtime.json（不轮转）后再切窗尺寸；窗口未建时只落状态。
pub fn set_todo_float_mini(app: &AppHandle, mini: bool) -> Result<(), String> {
  if let Some(state) = app.try_state::<Mutex<Store>>() {
    if let Ok(mut guard) = state.lock() {
      if guard.runtime.todo_float_mini != mini {
        guard.runtime.todo_float_mini = mini;
        let _ = guard.save_runtime_only();
      }
    }
  }
  if app.get_window(TODO_LABEL).is_some() {
    apply_todo_shape(app, mini)?;
  }
  Ok(())
}

/// 打开（或唤起）待办悬浮窗；置顶跟随设置；位置记忆 note_pos["todo"]；
/// 初始形态按 runtime.todo_float_mini（重启恢复也走这里，天然按最小化态建窗）。
pub fn open_todo_float(app: &AppHandle) -> Result<(), String> {
  let pinned = todo_pinned_of(app);
  let mini = todo_float_mini_of(app);
  if app.get_window(TODO_LABEL).is_none() {
    let stored = app
      .try_state::<Mutex<Store>>()
      .and_then(|s| s.lock().ok().and_then(|g| g.runtime.note_pos.get("todo").copied()));
    let window = build_with_timeout(app, TODO_LABEL.to_string(), "todo_window_error", move |app, label| {
      todo_builder(app, pinned, mini)
        .build()
        .map_err(|e| log_build_error("todo_window_error", label, &format!("{e}")))
    })?;
    // 建窗成功即关 WebView2 默认右键菜单（#3）；失败只记埋点，不影响待办窗
    disable_default_context_menu(&window);
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

/// 快速记录条：启动时由 setup 建一次（隐藏），命令路径下作为兜底重建入口。
pub fn create_capture_window<M: tauri::Manager<tauri::Wry>>(manager: &M) -> Result<Window, String> {
  // 初始高度只是首帧值：窗口随后由 capture_resize 按内容校正
  let window = with_webview2_dir(
    WindowBuilder::new(manager, CAPTURE_LABEL, WindowUrl::App("capture.html".into()))
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
      .center(),
  )
  .build()
  .map_err(|error| {
    crate::telemetry::record_str("capture_window_error", &[("err", &format!("{error}"))]);
    "快速记录条创建失败，请用悬浮面板底部输入框".to_string()
  })?;
  // 垫底建窗路径同样关 WebView2 默认右键菜单（#3）
  disable_default_context_menu(&window);
  Ok(window)
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

// ---------------------------------------------------------------- WebView2 右键菜单（#3）

/// 关闭 WebView2 原生右键菜单（含「检查 / Inspect」等默认项；#3）。
///
/// wry 建窗时无条件 `SetAreDefaultContextMenusEnabled(true)`（wry-0.24.12
/// `webview/webview2/mod.rs:246`），`WindowConfig` 没有对应开关，只能建窗成功后经
/// `with_webview` 拿到 `ICoreWebView2Controller` 再关。只关右键菜单，不碰
/// `SetAreDevToolsEnabled`：debug 构建的 F12 / Ctrl+Shift+I 仍可开 DevTools
/// （design D10「dev 构建保留 devtools 热键」）；文本区 Ctrl+C / Ctrl+V 是键盘快捷键，
/// 不经过右键菜单，不受影响。
///
/// `with_webview` 的闭包在事件循环线程执行，COM 接口不跨线程保存。attach 失败
/// （派发失败 / `CoreWebView2` 尚未就绪）只记埋点，绝不打断建窗与窗口生命周期。
#[cfg(target_os = "windows")]
pub fn disable_default_context_menu(window: &Window) {
  let label = window.label().to_string();
  let closure_label = label.clone();
  let attached = window.with_webview(move |webview| {
    let disabled = unsafe {
      webview
        .controller()
        .CoreWebView2()
        .and_then(|core| core.Settings())
        .and_then(|settings| settings.SetAreDefaultContextMenusEnabled(false))
    };
    if let Err(error) = disabled {
      crate::telemetry::record_str(
        "webview_contextmenu_error",
        &[
          ("label", closure_label.as_str()),
          ("err", &format!("{error}")),
        ],
      );
    }
  });
  if let Err(error) = attached {
    crate::telemetry::record_str(
      "webview_contextmenu_error",
      &[("label", label.as_str()), ("err", &format!("{error}"))],
    );
  }
}

/// 非 Windows 平台没有 WebView2 原生右键菜单；保留同名 no-op，四处建窗点无需 cfg 分叉。
#[cfg(not(target_os = "windows"))]
pub fn disable_default_context_menu(_window: &Window) {}

// ---------------------------------------------------------------- 退出时释放 WebView2（#16）

/// 退出前逐个关闭所有窗口的 WebView2 控制器（#16 退出残留 msedgewebview2.exe）。
///
/// 托盘「退出」走 `app.exit(0)`：tauri 1.8.3 的默认退出流程直接 `cleanup_before_exit()`
/// 再 `process::exit`（`tauri-1.8.3/src/app.rs:564-568`、`:576-588`），不经过窗口
/// `Destroyed` 链路，也不释放 `ICoreWebView2Controller`。浏览器进程于是等不到归属窗口
/// 销毁，主进程退出后 `msedgewebview2.exe` 可能继续残留。
///
/// 这里在 `RunEvent::ExitRequested`（默认退出流程开始处）对每个窗口调
/// `ICoreWebView2Controller::Close()`：它同步关闭 WebView 并触发浏览器进程回收；
/// 再留一小段时间给浏览器进程退出，避免随后 `process::exit` 抢跑。
/// `with_webview` 在主线程内联执行（tauri-runtime-wry `send_user_message` 对主线程
/// 直接 handle，不发事件等回包），所以从 run 回调里调用不会死锁。
#[cfg(target_os = "windows")]
pub fn close_all_webviews(app: &AppHandle) {
  let windows = app.windows();
  let window_count = windows.len();
  let mut closed = 0usize;
  for (_label, window) in windows {
    let attached = window.with_webview(|webview| unsafe {
      let _ = webview.controller().Close();
    });
    if attached.is_ok() {
      closed += 1;
    }
  }
  crate::telemetry::record_str(
    "app_shutdown",
    &[
      ("windows", &window_count.to_string()),
      ("webviews_closed", &closed.to_string()),
    ],
  );
  // WebView2 浏览器进程收尾是异步的；给 300ms 再让主进程退出，避免 Windows 上被强杀
  std::thread::sleep(std::time::Duration::from_millis(300));
}

/// 非 Windows 平台没有 WebView2 控制器需要显式关闭。
#[cfg(not(target_os = "windows"))]
pub fn close_all_webviews(_app: &AppHandle) {}

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
          report_window_error(&handle, show_main(&handle, None));
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

/// 便签的锁内快照（id / visible / mini / 位置）：托盘与启动恢复先拿快照，
/// 再逐个操作窗口，不把 Store 锁带进窗口操作——v2.2 死锁教训。
fn note_snapshots(app: &AppHandle) -> Vec<(String, bool, bool, Option<[i32; 2]>)> {
  app
    .try_state::<Mutex<Store>>()
    .and_then(|s| {
      s.lock().ok().map(|g| {
        g.data
          .stickies
          .iter()
          .map(|n| {
            (
              n.id.clone(),
              n.visible,
              n.mini,
              g.runtime.note_pos.get(&n.id).copied(),
            )
          })
          .collect()
      })
    })
    .unwrap_or_default()
}

/// 托盘菜单点击分发；返回 true 表示要退出应用
pub fn handle_tray_click(app: &AppHandle, id: &str) -> bool {
  match id {
    "open_main" => {
      report_window_error(app, show_main(app, None));
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
      // 「显示全部便签」= 恢复所有「可见」便签的窗口。隐藏是用户主动关闭的语义
      // （✕/Alt+F4 只隐藏），这里绝不擅自把隐藏项改回可见（fix-known-defects 4.3）。
      for (id, visible, mini, pos) in note_snapshots(app) {
        if visible {
          let _ = open_note_window(app, &id, mini, pos);
        }
      }
    }
    "sticky_hide_all" => {
      // 「隐藏全部便签」= 全部置 visible=false 并落盘，再藏窗；数据与位置都保留。
      let mut hidden_any = false;
      if let Some(state) = app.try_state::<Mutex<Store>>() {
        if let Ok(mut guard) = state.lock() {
          for note in guard.data.stickies.iter_mut() {
            if note.visible {
              note.visible = false;
              note.updated_at = crate::models::now_text();
              hidden_any = true;
            }
          }
          if hidden_any {
            let _ = guard.save();
          }
        }
      }
      if hidden_any {
        let _ = app.emit_all(
          "store-changed",
          crate::models::StoreEvent::changed("stickies"),
        );
      }
      for (id, ..) in note_snapshots(app) {
        hide_note_window(app, &id);
      }
    }
    "reminders" => {
      report_window_error(app, show_main(app, Some("/planned/reminders")));
    }
    "weekly" => {
      report_window_error(app, show_main(app, Some("/review/weekly")));
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
