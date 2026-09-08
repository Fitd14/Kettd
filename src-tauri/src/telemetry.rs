//! 本地度量埋点（Telemetry-Minimal · v2.1，见 PRD-本地度量埋点）
//!
//! 铁律：**绝不出网**。事件写本机 `data_dir()/events.jsonl`，与 `data.json` 物理隔离。
//! 设计：全局 mpsc 通道 + 单写线程，热路径 `record()` 只做一次非阻塞 send；
//! 写盘失败/通道满一律静默丢弃（埋点是尽力而为，绝不影响主流程与真库）。
//! 开关由 `Settings.telemetry_enabled` 控制（默认开），关则 `record()` 直接返回。

use chrono::{DateTime, Local};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// 单文件上限：超过则轮转（本地个人用，90 天窗绰绰有余）
const MAX_BYTES: u64 = 1_048_576; // 1 MB

static ENABLED: AtomicBool = AtomicBool::new(true);
static SINK: OnceLock<Option<SyncSender<String>>> = OnceLock::new();
static WRITER: OnceLock<Mutex<()>> = OnceLock::new();
static DIR: OnceLock<PathBuf> = OnceLock::new();

/// 启动写线程（setup 里调一次）。之后 `record` 走这条通道。
pub fn init(dir: PathBuf) {
  let _ = WRITER.get_or_init(|| Mutex::new(()));
  let _ = DIR.set(dir.clone());
  let (tx, rx) = mpsc::sync_channel::<String>(512);
  std::thread::spawn(move || writer_loop(rx, dir));
  let _ = SINK.set(Some(tx));
}

pub fn set_enabled(on: bool) {
  ENABLED.store(on, Ordering::Relaxed);
}

/// 清空本地统计（planned-settings-ui-spec B.5★）：events.jsonl 重置为空文件。
/// 尽力而为语义：写线程队列里可能还有一批在途事件（≤64 条）随后落盘，属可接受残留。
/// 只清事件流，不碰 data.json；随后记一条 events_cleared 便于对账。
pub fn clear_events() -> Result<(), String> {
  let dir = DIR.get().ok_or_else(|| "埋点尚未初始化".to_string())?;
  let path = events_path(dir);
  std::fs::File::create(&path).map_err(|e| format!("清空失败：{}", e))?;
  record_str("events_cleared", &[]);
  Ok(())
}

/// 记一条事件。绝不 panic、绝不出网、失败静默。
pub fn record(event: &str, props: Value) {
  if !ENABLED.load(Ordering::Relaxed) {
    return;
  }
  let Some(Some(tx)) = SINK.get() else { return };
  let line = json!({
    "ts": iso_now(),
    "event": event,
    "props": props,
  });
  match tx.try_send(line.to_string()) {
    // 队列满 → 丢弃本条（尽力而为）
    Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
    Ok(()) => {}
  }
}

/// 便捷：只带少量字符串字段的快捷记录
pub fn record_str(event: &str, pairs: &[(&str, &str)]) {
  let mut map = serde_json::Map::new();
  for (k, v) in pairs {
    map.insert((*k).to_string(), Value::String((*v).to_string()));
  }
  record(event, Value::Object(map));
}

fn iso_now() -> String {
  Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn events_path(dir: &PathBuf) -> PathBuf {
  dir.join("events.jsonl")
}

fn writer_loop(rx: mpsc::Receiver<String>, dir: PathBuf) {
  loop {
    // 收一批再写，摊薄开文件开销
    let first = match rx.recv() {
      Ok(line) => line,
      Err(_) => return,
    };
    let mut batch = Vec::with_capacity(32);
    batch.push(first);
    while let Ok(line) = rx.try_recv() {
      batch.push(line);
      if batch.len() >= 64 {
        break;
      }
    }
    let _ = write_batch(&dir, &batch);
  }
}

fn write_batch(dir: &PathBuf, lines: &[String]) -> std::io::Result<()> {
  let path = events_path(dir);
  rotate_if_needed(&path);
  let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
  for line in lines {
    writeln!(f, "{}", line)?;
  }
  let _ = f.flush();
  Ok(())
}

/// 超过上限则改名为带时间戳的历史分片，重开新文件
fn rotate_if_needed(path: &PathBuf) {
  let meta = match std::fs::metadata(path) {
    Ok(m) => m,
    Err(_) => return,
  };
  if meta.len() < MAX_BYTES {
    return;
  }
  let stamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
  let archived = path.with_file_name(format!("events.{}.jsonl", stamp));
  let _ = std::fs::rename(path, archived);
}

// ============================================================ 度量口径纯函数
// 只喂内存数据做单测；不碰真库、不碰 events.jsonl。

/// 中位数（P50）。输入自定秒/毫秒均可（这里按整数毫秒时延）。空 → None。
pub fn p50_ms(samples: &[i64]) -> Option<i64> {
  if samples.is_empty() {
    return None;
  }
  let mut s = samples.to_vec();
  s.sort_unstable();
  let n = s.len();
  Some(if n % 2 == 1 {
    s[n / 2]
  } else {
    (s[n / 2 - 1] + s[n / 2]) / 2
  })
}

/// 落在给定 ISO 周（年*100+周）的事件条数。ts 为 RFC3339 字符串。
pub fn count_in_week(events: &[(String, String)], year_week: i32) -> usize {
  events
    .iter()
    .filter(|(name, _)| name == "capture_commit")
    .filter(|(_, ts)| parse_week(ts) == Some(year_week))
    .count()
}

/// 提醒触达率（0..=1）：分母排除 app_not_running（未运行态不计入承诺）。
/// 返回 None 表示无可评估样本。
pub fn reach_rate(shown: u32, missed_running: u32, missed_not_running: u32) -> Option<f64> {
  let denom = shown + missed_running;
  if denom == 0 {
    return None;
  }
  let _ = missed_not_running; // 明确不计入分母
  Some(shown as f64 / denom as f64)
}

fn parse_week(ts: &str) -> Option<i32> {
  let dt: DateTime<Local> = DateTime::parse_from_rfc3339(ts).ok()?.into();
  use chrono::Datelike;
  let iso = dt.iso_week();
  Some(iso.year() * 100 + iso.week() as i32)
}

// ============================================================ 捕获会话 / 时延
// open→commit 用单调钟配对；commit 事件自带 latency_ms，分析端不必再配对。

static LAST_OPEN: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();

fn last_open() -> &'static Mutex<Option<Instant>> {
  LAST_OPEN.get_or_init(|| Mutex::new(None))
}

/// 快速记录条唤起：记 capture_open，并暂存时刻供 commit 算时延
pub fn capture_open() {
  if let Ok(mut g) = last_open().lock() {
    *g = Some(Instant::now());
  }
  record("capture_open", json!({}));
}

/// 快速记录落库（source=capture）：算并记 capture_commit(latency_ms)，清暂存
pub fn capture_commit() {
  let mut latency = serde_json::Map::new();
  if let Ok(mut g) = last_open().lock() {
    if let Some(t0) = g.take() {
      latency.insert("latency_ms".into(), json!(t0.elapsed().as_millis() as i64));
    }
  }
  record("capture_commit", Value::Object(latency));
}

// ============================================================ 快照（读 events.jsonl → 度量）

#[derive(Debug, Clone, PartialEq)]
pub struct Ev {
  pub event: String,
  pub ts: String,
  pub latency_ms: Option<i64>,
  pub reason: Option<String>,
}

fn parse_line(line: &str) -> Option<Ev> {
  let v: Value = serde_json::from_str(line).ok()?;
  Some(Ev {
    event: v.get("event")?.as_str()?.to_string(),
    ts: v.get("ts")?.as_str()?.to_string(),
    latency_ms: v
      .pointer("/props/latency_ms")
      .and_then(|x| x.as_i64()),
    reason: v
      .pointer("/props/reason")
      .and_then(|x| x.as_str())
      .map(|s| s.to_string()),
  })
}

/// 读最近 N 行（按字节尾读近似：全读后取尾）。文件缺失返回空。
pub fn load_recent(dir: &PathBuf, max: usize) -> Vec<Ev> {
  let text = match std::fs::read_to_string(events_path(dir)) {
    Ok(t) => t,
    Err(_) => return Vec::new(),
  };
  let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
  let start = lines.len().saturating_sub(max);
  lines[start..].iter().filter_map(|l| parse_line(l)).collect()
}

/// 由事件集算四项指标并渲染 markdown。纯函数，喂内存数据即可测。
pub fn render_snapshot(events: &[Ev]) -> String {
  let lat: Vec<i64> = events
    .iter()
    .filter(|e| e.event == "capture_commit")
    .filter_map(|e| e.latency_ms)
    .collect();
  let p50 = p50_ms(&lat);
  // 本周快录条数（含无 latency 的老事件也计数）
  use chrono::{DateTime, Datelike, Local};
  let this_week = Local::now().iso_week();
  let this_week_key = this_week.year() * 100 + this_week.week() as i32;
  let weekly_commits = events
    .iter()
    .filter(|e| e.event == "capture_commit")
    .filter(|e| {
      DateTime::parse_from_rfc3339(&e.ts)
        .map(|d| {
          let l: DateTime<Local> = d.into();
          let w = l.iso_week();
          w.year() * 100 + w.week() as i32 == this_week_key
        })
        .unwrap_or(false)
    })
    .count();
  let shown = events.iter().filter(|e| e.event == "reminder_shown").count() as u32;
  let missed_running = events
    .iter()
    .filter(|e| e.event == "reminder_missed")
    .filter(|e| !matches!(e.reason.as_deref(), Some("app_not_running")))
    .count() as u32;
  let missed_not_running = events
    .iter()
    .filter(|e| e.event == "reminder_missed")
    .filter(|e| matches!(e.reason.as_deref(), Some("app_not_running")))
    .count() as u32;
  let reach = reach_rate(shown, missed_running, missed_not_running);
  let weekly_used = events.iter().any(|e| e.event == "weekly_export" || e.event == "weekly_open");

  let mut out = String::from("\n\n---\n## 度量快照（本地·不出网）\n\n");
  out.push_str(&format!(
    "- 捕获时延 P50：{}\n",
    p50.map(|m| format!("{} ms", m)).unwrap_or_else(|| "无样本".into())
  ));
  out.push_str(&format!("- 本周快录条数：{}（目标 ≥25/周）\n", weekly_commits));
  out.push_str(&format!(
    "- 提醒触达率：{}（shown {} / 运行中 missed {}；未运行 {} 不计）\n",
    reach.map(|r| format!("{:.0}%", r * 100.0)).unwrap_or_else(|| "无样本".into()),
    shown,
    missed_running,
    missed_not_running
  ));
  out.push_str(&format!(
    "- 周汇总本周是否用过：{}\n",
    if weekly_used { "是" } else { "否" }
  ));
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn p50_odd_and_even() {
    assert_eq!(p50_ms(&[300, 100, 200]), Some(200)); // 奇数取中
    assert_eq!(p50_ms(&[100, 200, 300, 400]), Some(250)); // 偶数取均值
    assert_eq!(p50_ms(&[]), None);
    assert_eq!(p50_ms(&[42]), Some(42));
  }

  #[test]
  fn reach_rate_excludes_not_running() {
    // 8 shown, 1 missed(运行中), 5 missed(未运行) → 8/9
    let r = reach_rate(8, 1, 5).unwrap();
    assert!((r - 8.0 / 9.0).abs() < 1e-9);
    // 未运行为主，分母仍只看运行态
    assert_eq!(reach_rate(0, 0, 10), None);
    assert_eq!(reach_rate(3, 0, 100), Some(1.0));
  }

  #[test]
  fn count_in_week_filters_name_and_week() {
    // 构造两个不同周的合法 RFC3339（本机时区可能不同，只验同周计数与名称过滤）
    let same = iso_now();
    let ev = vec![
      ("capture_commit".to_string(), same.clone()),
      ("capture_commit".to_string(), same.clone()),
      ("app_launch".to_string(), same.clone()),
    ];
    let wk = parse_week(&same).unwrap();
    assert_eq!(count_in_week(&ev, wk), 2); // 只数 capture_commit，忽略 app_launch
    assert_eq!(count_in_week(&ev, 199901), 0);
  }

  fn ev(event: &str, latency: Option<i64>, reason: Option<&str>) -> Ev {
    Ev {
      event: event.to_string(),
      ts: iso_now(),
      latency_ms: latency,
      reason: reason.map(|s| s.to_string()),
    }
  }

  #[test]
  fn snapshot_computes_four_metrics() {
    let events = vec![
      ev("capture_commit", Some(1000), None),
      ev("capture_commit", Some(2000), None),
      ev("reminder_shown", None, None),
      ev("reminder_shown", None, None),
      ev("reminder_missed", None, Some("notify_fail")),
      ev("reminder_missed", None, Some("app_not_running")), // 不计入分母
      ev("weekly_export", None, None),
    ];
    let md = render_snapshot(&events);
    assert!(md.contains("1500 ms"), "P50 应为 1500：{}", md); // (1000+2000)/2
    assert!(md.contains("本周快录条数：2"), "周计数：{}", md);
    assert!(md.contains("67%"), "触达率 2/3：{}", md); // 排除 app_not_running
    assert!(md.contains("周汇总本周是否用过：是"), "周汇总用过：{}", md);
  }

  #[test]
  fn parse_line_reads_nested_props() {
    let line = r#"{"ts":"2026-09-04T10:00:00+08:00","event":"capture_commit","props":{"latency_ms":812}}"#;
    let e = parse_line(line).unwrap();
    assert_eq!(e.event, "capture_commit");
    assert_eq!(e.latency_ms, Some(812));
    assert_eq!(e.reason, None);

    let miss = r#"{"ts":"2026-09-04T10:00:00+08:00","event":"reminder_missed","props":{"reason":"dnd"}}"#;
    assert_eq!(parse_line(miss).unwrap().reason.as_deref(), Some("dnd"));
  }
}
