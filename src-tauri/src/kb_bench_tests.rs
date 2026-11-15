//! 知识库性能基准（批次 S1 · tasks.md 2.8），以及 2.10 的真实文件迁移演练。
//!
//! 只走生产真实路径：临时目录里构造 5000 条知识库（`notes.json` 纯索引 +
//! 每条 `notes/<id>.md` 正文），用 `FsBackend` + `Store::load_with` 加载并计时。
//! 输出中文表格与环境说明，`println!` 仅在 `-- --nocapture` 时可见。
//!
//! 运行基准（发布态才有参考价值）：
//!
//! ```text
//! cargo test --release kb_bench_5000 -- --ignored --nocapture
//! ```
//!
//! 数据目录一律 `std::env::temp_dir()` 下带唯一后缀的子目录，`TempDir` 的 Drop
//! 负责 `remove_dir_all`；绝不触碰 `%APPDATA%\todo-list` 生产数据。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::infra::fs_store::FsBackend;
use crate::models::{KbItem, KbItemMeta};
use crate::ports::clock::SystemClock;
use crate::ports::store_backend::StoreBackend;
use crate::store::Store;
use crate::store_notes;

/// 基准条目数（tasks.md 2.8 指定 5000）。
const BENCH_ITEMS: usize = 5000;

/// 冷加载预算（design.md Risks：5000 条启动读取 5000 个正文文件，预算 1.5s）。
const COLD_LOAD_BUDGET: Duration = Duration::from_millis(1500);

/// 一份可正常加载的 v2 `data.json`（与 store.rs 测试同形状：settings 对象 + tokens）。
const DATA_JSON: &str = r#"{"tasks":[],"reminders":[],"settings":{"theme":"float"}}"#;

/// 正文填充语料（只用于把每篇正文凑到 200–500 字符）。
const BODY_FILLER: &str = "本地优先的笔记正文，用于测量索引与正文分文件后的真实加载开销。";

// ---------------------------------------------------------------- 临时目录

/// 临时目录 RAII：Drop 时递归删除，保证基准不残留文件。
struct TempDir(PathBuf);

impl TempDir {
  fn new(tag: &str) -> Self {
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map(|value| value.as_nanos())
      .unwrap_or(0);
    let name = format!(
      "kettd-kb-bench-{tag}-{}-{nanos}",
      std::process::id()
    );
    // 先试系统 temp；受限令牌 / 沙箱下可能不可写（门禁实测 Os error 5），
    // 再退到构建目录（一定可写且被 gitignore）。
    let candidates = [
      std::env::temp_dir().join(&name),
      Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(&name),
    ];
    let mut last_error = None;
    for candidate in candidates {
      match std::fs::create_dir_all(&candidate) {
        Ok(()) => return TempDir(candidate),
        Err(error) => last_error = Some((candidate, error)),
      }
    }
    let (path, error) = last_error.expect("至少一个候选临时目录");
    panic!(
      "创建基准临时目录失败（只写临时目录，不碰生产数据）：{} — {error}",
      path.display()
    );
  }

  fn path(&self) -> &Path {
    &self.0
  }
}

impl Drop for TempDir {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.0);
  }
}

// ---------------------------------------------------------------- 确定性随机

/// 固定种子的 LCG：基准数据可复现，不引入 rand 依赖。
struct Lcg(u64);

impl Lcg {
  fn new(seed: u64) -> Self {
    Lcg(seed)
  }

  fn next(&mut self) -> u64 {
    self.0 = self
      .0
      .wrapping_mul(6_364_136_223_846_793_005)
      .wrapping_add(1_442_695_040_888_963_407);
    self.0
  }

  /// 闭区间 [low, high] 内的伪随机整数。
  fn range(&mut self, low: usize, high: usize) -> usize {
    debug_assert!(low <= high);
    low + (self.next() % (high - low + 1) as u64) as usize
  }
}

// ---------------------------------------------------------------- fixture

/// 一份完整的 5000 条知识库夹具及其体积统计（只用于打印，不参与计时）。
struct Fixture {
  metas: Vec<KbItemMeta>,
  index_bytes: usize,
  body_files: usize,
  body_bytes: u64,
  body_min_chars: usize,
  body_max_chars: usize,
  asset_files: usize,
}

/// 时间戳：合法本地语义字符串 `YYYY-MM-DDTHH:mm`（AGENTS 2.1，禁止 UTC）。
fn stamp(index: usize) -> String {
  format!(
    "2026-09-{:02}T{:02}:{:02}",
    (index % 28) + 1,
    index % 24,
    index % 60
  )
}

/// 生成一篇 200–500 字符的正文：开头点明序号 + 1 个 `assets/<sha>.png` 图片引用，
/// 其余用填充语料补足到随机目标长度（沿用 tasks.md 2.8 的夹具要求）。
fn make_body(lcg: &mut Lcg, index: usize, asset_sha: &str) -> String {
  let target = lcg.range(200, 500);
  let prefix = format!(
    "第 {} 条基准正文。\n\n![配图](assets/{}.png)\n\n",
    index + 1,
    asset_sha
  );
  let prefix_chars = prefix.chars().count();
  assert!(
    prefix_chars < target,
    "正文前缀 {prefix_chars} 字符必须短于目标 {target} 字符"
  );
  let mut body = prefix;
  let mut length = prefix_chars;
  for filler in BODY_FILLER.chars().cycle().take(target - length) {
    body.push(filler);
    length += 1;
  }
  assert_eq!(length, target);
  body
}

fn format_bytes(bytes: u64) -> String {
  if bytes >= 1024 * 1024 {
    format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
  } else if bytes >= 1024 {
    format!("{:.1} KiB", bytes as f64 / 1024.0)
  } else {
    format!("{bytes} B")
  }
}

/// 在临时目录里铺出 5000 条知识库：
/// `data.json` + `notes.json`（生产同款索引：`store_notes::index_json`）+
/// `notes/<id>.md` 5000 个（200–500 字符，含 1 个 assets 图片引用）+
/// `folders.json` + 10 个假资源文件（冷加载不读 assets，只为目录形态真实）。
fn build_fixture(root: &Path) -> Fixture {
  let notes_dir = root.join("notes");
  let assets_dir = root.join("assets");
  std::fs::create_dir_all(&notes_dir).expect("创建 notes/ 失败");
  std::fs::create_dir_all(&assets_dir).expect("创建 assets/ 失败");

  let asset_shas: Vec<String> = (0..10)
    .map(|index| store_notes::content_hash(&format!("bench-asset-{index}")))
    .collect();
  for sha in &asset_shas {
    std::fs::write(
      assets_dir.join(format!("{sha}.png")),
      [0x89u8, b'P', b'N', b'G', 0, 0, 0, 0],
    )
    .expect("写假资源文件失败");
  }

  let mut lcg = Lcg::new(0x5EED_5000);
  let mut metas: Vec<KbItemMeta> = Vec::with_capacity(BENCH_ITEMS);
  let mut bodies: Vec<String> = Vec::with_capacity(BENCH_ITEMS);
  let mut body_bytes: u64 = 0;
  let mut body_min_chars = usize::MAX;
  let mut body_max_chars = 0usize;
  for index in 0..BENCH_ITEMS {
    let id = format!("k{:04}", index + 1);
    let body = make_body(&mut lcg, index, &asset_shas[index % asset_shas.len()]);
    let chars = body.chars().count();
    body_min_chars = body_min_chars.min(chars);
    body_max_chars = body_max_chars.max(chars);
    body_bytes += body.len() as u64;
    std::fs::write(notes_dir.join(format!("{id}.md")), &body).expect("写正文文件失败");
    metas.push(KbItemMeta {
      id,
      title: format!("基准条目 {:04}", index + 1),
      tags: vec!["基准".to_string(), format!("组{}", index % 8)],
      folder_id: None,
      order: Some(index as i64),
      pinned: index % 17 == 0,
      created_at: stamp(index),
      updated_at: stamp(index),
      content_hash: store_notes::content_hash(&body),
    });
    bodies.push(body);
  }

  // notes.json 与生产 save_notes 完全同款：store_notes::index_json（pretty 裸数组、无正文）
  let items: Vec<KbItem> = metas
    .iter()
    .cloned()
    .zip(bodies.into_iter())
    .map(|(meta, body)| store_notes::item_with_body(meta, body))
    .collect();
  let index = store_notes::index_json(&items).expect("索引序列化失败");
  std::fs::write(root.join("notes.json"), &index).expect("写 notes.json 失败");
  std::fs::write(root.join("folders.json"), "[]").expect("写 folders.json 失败");
  std::fs::write(root.join("data.json"), DATA_JSON).expect("写 data.json 失败");

  Fixture {
    metas,
    index_bytes: index.len(),
    body_files: BENCH_ITEMS,
    body_bytes,
    body_min_chars,
    body_max_chars,
    asset_files: asset_shas.len(),
  }
}

// ---------------------------------------------------------------- 计时辅助

fn ms(duration: Duration) -> f64 {
  duration.as_secs_f64() * 1000.0
}

/// 序列化 `rounds` 次取平均，返回（平均耗时，最后一次的字节数）。
fn bench_serialize<T: serde::Serialize>(value: &T, rounds: u32) -> (Duration, usize) {
  let mut total = Duration::ZERO;
  let mut bytes = 0usize;
  for _ in 0..rounds {
    let started = Instant::now();
    let text = serde_json::to_string(value).expect("序列化失败");
    total += started.elapsed();
    bytes = text.len();
  }
  (total / rounds, bytes)
}

fn percentile(sorted: &[Duration], percent: usize) -> Duration {
  if sorted.is_empty() {
    return Duration::ZERO;
  }
  sorted[((sorted.len() - 1) * percent / 100).min(sorted.len() - 1)]
}

fn print_environment(dir: &TempDir, fixture: &Fixture) {
  let profile = if cfg!(debug_assertions) {
    "debug（调试构建，耗时仅供功能自检；性能结论以 --release 为准）"
  } else {
    "release"
  };
  let cpus = std::thread::available_parallelism()
    .map(|value| value.get())
    .unwrap_or(0);
  println!(
    "\n=== Kettd 知识库性能基准：{} 条（批次 S1 · tasks.md 2.8）===",
    BENCH_ITEMS
  );
  println!(
    "环境：{} / {}，{} 个逻辑 CPU，构建配置：{}",
    std::env::consts::OS,
    std::env::consts::ARCH,
    cpus,
    profile
  );
  println!(
    "数据目录：{}（std::env::temp_dir 下唯一子目录，Drop 自动删除）",
    dir.path().display()
  );
  println!(
    "数据集：索引 notes.json {} 条 / {}；正文 notes/*.md {} 个 / {}（每篇 {}–{} 字符）；assets/*.png {} 个（冷加载不读资源，仅保持目录形态）",
    fixture.metas.len(),
    format_bytes(fixture.index_bytes as u64),
    fixture.body_files,
    format_bytes(fixture.body_bytes),
    fixture.body_min_chars,
    fixture.body_max_chars,
    fixture.asset_files
  );
  println!("加载路径：Store::load_with(FsBackend + SystemClock)，与生产 Store::load() 同一条路径。");
  println!(
    "说明：夹具刚刚写盘，第 1 次加载时 OS 页缓存偏热；真机冷启动（重启后首次）通常更慢，本表是下限估计。"
  );
}

// ---------------------------------------------------------------- 2.8 性能基准

/// tasks.md 2.8：5000 条知识库的冷加载 / 列表 IPC 序列化 / 单条正文读取基准。
///
/// 默认门禁不跑（`#[ignore]`），按需执行：
/// `cargo test --release kb_bench_5000 -- --ignored --nocapture`
#[test]
#[ignore = "性能基准，按需运行"]
fn kb_bench_5000_cold_load_ipc_and_body_reads() {
  let dir = TempDir::new("5000");
  let fixture = build_fixture(dir.path());
  print_environment(&dir, &fixture);

  // ① 冷加载总耗时：生产启动同路径（data.json → notes.json 索引 → 5000 条正文 → hash）
  println!("\n[1] 冷加载（Store::load_with + FsBackend，生产启动同路径）");
  let started = Instant::now();
  let store = Store::load_with(
    Box::new(FsBackend::new(dir.path().to_path_buf())),
    Box::new(SystemClock),
  );
  let cold_load = started.elapsed();
  assert_eq!(
    store.kb.len(),
    BENCH_ITEMS,
    "冷加载条目数不符：索引或正文缺失"
  );
  assert!(
    store.ensure_kb_ok().is_ok(),
    "冷加载进入知识库错误态，基准数据不可用"
  );
  assert_eq!(
    store.kb[0].content_hash.as_deref(),
    Some(fixture.metas[0].content_hash.as_str()),
    "首条正文未按真实路径加载（contentHash 与索引不一致）"
  );
  println!("  第 1 次（冷）：{:>9.2} ms", ms(cold_load));

  // 重复加载：证明数字稳定，也顺带量出「页缓存已热」后的底噪
  let warm_rounds = 2u32;
  let mut warm_total = Duration::ZERO;
  for round in 1..=warm_rounds {
    let started = Instant::now();
    let reloaded = Store::load_with(
      Box::new(FsBackend::new(dir.path().to_path_buf())),
      Box::new(SystemClock),
    );
    let elapsed = started.elapsed();
    warm_total += elapsed;
    assert_eq!(reloaded.kb.len(), BENCH_ITEMS, "重复加载条目数不符");
    println!("  第 {} 次（页缓存热）：{:>9.2} ms", round + 1, ms(elapsed));
  }
  let warm_mean = warm_total / warm_rounds;
  println!(
    "  重复加载均值：{:>9.2} ms（{} 次，页缓存已热）",
    ms(warm_mean),
    warm_rounds
  );
  println!(
    "  预算 {} ms：{}（{}）",
    COLD_LOAD_BUDGET.as_millis(),
    if cold_load <= COLD_LOAD_BUDGET {
      "通过"
    } else {
      "实测超预算，需要后续优化"
    },
    format!("冷加载 {:.2} ms", ms(cold_load))
  );

  // ② 列表 IPC 序列化：索引数组（KbItemMeta）+ get_kb_items(None) 实际返回的 Vec<KbItem>（bodyMd 空）
  println!("\n[2] 列表 IPC / 索引序列化（serde_json::to_string，5 次均值）");
  let (index_ser, index_len) = bench_serialize(&fixture.metas, 5);
  println!(
    "  索引数组 Vec<KbItemMeta>（notes.json 元数据形状）：{:>8.2} ms，{}",
    ms(index_ser),
    format_bytes(index_len as u64)
  );
  let list_shape: Vec<KbItem> = store
    .kb
    .iter()
    .map(|item| KbItem {
      body_md: String::new(),
      ..item.clone()
    })
    .collect();
  let (list_ser, list_len) = bench_serialize(&list_shape, 5);
  println!(
    "  列表 IPC Vec<KbItem>（bodyMd 空串，get_kb_items(None) 分支）：{:>8.2} ms，{}",
    ms(list_ser),
    format_bytes(list_len as u64)
  );

  // ③ 单条正文读取：随机抽样真实 FsBackend::read_note_body
  println!("\n[3] 单条正文读取（FsBackend::read_note_body，真实磁盘）");
  let backend = FsBackend::new(dir.path().to_path_buf());
  let sample_count = 300usize;
  let mut sampler = Lcg::new(0xB0D1_E5);
  let mut samples: Vec<Duration> = Vec::with_capacity(sample_count);
  let mut bytes_read = 0usize;
  for _ in 0..sample_count {
    let index = sampler.range(0, BENCH_ITEMS - 1);
    let id = &fixture.metas[index].id;
    let started = Instant::now();
    let body = backend
      .read_note_body(id)
      .expect("正文读取失败（读写路径异常）")
      .expect("正文缺失（夹具不完整）");
    samples.push(started.elapsed());
    bytes_read += body.len();
  }
  samples.sort();
  let mean = samples.iter().sum::<Duration>() / sample_count as u32;
  println!(
    "  抽样 {} 次：均值 {:.3} ms，P50 {:.3} ms，P95 {:.3} ms，最大 {:.3} ms（共读回 {}）",
    sample_count,
    ms(mean),
    ms(percentile(&samples, 50)),
    ms(percentile(&samples, 95)),
    ms(samples[samples.len() - 1]),
    format_bytes(bytes_read as u64)
  );

  // 参考量：全量顺序读一遍正文，解释冷加载的成本构成
  let started = Instant::now();
  let mut full_bytes = 0usize;
  for meta in &fixture.metas {
    let body = backend
      .read_note_body(&meta.id)
      .expect("全量正文读取失败")
      .expect("全量正文缺失");
    full_bytes += body.len();
  }
  let full_scan = started.elapsed();
  println!(
    "  [参考] 全量 {} 条正文顺序读：{:>8.2} ms，共 {}（冷加载的主要成本来自逐条正文读取）",
    BENCH_ITEMS,
    ms(full_scan),
    format_bytes(full_bytes as u64)
  );

  println!("\n结论：");
  println!(
    "  ① 冷加载 {:.2} ms / 预算 {} ms → {}",
    ms(cold_load),
    COLD_LOAD_BUDGET.as_millis(),
    if cold_load <= COLD_LOAD_BUDGET {
      "通过"
    } else {
      "实测超预算，需要后续优化（design.md Risks 已记录降级方案：启动只读索引、正文按需）"
    }
  );
  println!(
    "  ② 列表 IPC 序列化 {:.2} ms（{}），单次调用开销在可接受范围",
    ms(list_ser),
    format_bytes(list_len as u64)
  );
  println!("  ③ 单条正文读取均值 {:.3} ms", ms(mean));
}

// ---------------------------------------------------------------- 2.10 迁移演练

fn count_md_files(dir: &Path) -> usize {
  match std::fs::read_dir(dir) {
    Ok(entries) => entries
      .filter_map(|entry| entry.ok())
      .filter(|entry| {
        entry
          .path()
          .extension()
          .map(|extension| extension == "md")
          .unwrap_or(false)
      })
      .count(),
    Err(_) => 0,
  }
}

/// tasks.md 2.10 手工演练的自动化版本（真实文件系统 + 临时目录，毫秒级，默认门禁也跑）：
/// 旧格式 `notes.json`（2 条内联 `bodyMd`）→ `Store::load_with` 生产启动路径 →
/// 打印迁移前后条目数、`notes/` 下 `.md` 数、`notes.v1.json` 是否存在、索引是否仍含 bodyMd。
#[test]
fn kb_migration_drill_temp_dir() {
  let dir = TempDir::new("migrate-drill");
  let legacy = json!([
    {
      "id": "k1",
      "title": "演练旧条目一",
      "bodyMd": "第一条正文\n\n![图](assets/aa.png)",
      "tags": ["演练"],
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00"
    },
    {
      "id": "k2",
      "title": "演练旧条目二",
      "bodyMd": "第二条正文",
      "tags": [],
      "createdAt": "2026-09-02T10:00",
      "updatedAt": "2026-09-02T10:00"
    }
  ]);
  let legacy_text = serde_json::to_string_pretty(&legacy).expect("旧格式序列化失败");
  std::fs::write(dir.path().join("data.json"), DATA_JSON).expect("写 data.json 失败");
  std::fs::write(dir.path().join("notes.json"), &legacy_text).expect("写旧 notes.json 失败");
  std::fs::write(dir.path().join("folders.json"), "[]").expect("写 folders.json 失败");

  let entries_before = legacy.as_array().map(|list| list.len()).unwrap_or(0);
  let md_before = count_md_files(&dir.path().join("notes"));

  let store = Store::load_with(
    Box::new(FsBackend::new(dir.path().to_path_buf())),
    Box::new(SystemClock),
  );

  let md_after = count_md_files(&dir.path().join("notes"));
  let index_text = std::fs::read_to_string(dir.path().join("notes.json")).expect("读新索引失败");
  let index: serde_json::Value = serde_json::from_str(&index_text).expect("新索引不是合法 JSON");
  let body_still_inline = index
    .as_array()
    .map(|list| {
      list
        .iter()
        .any(|item| item.get("bodyMd").is_some() || item.get("body_md").is_some())
    })
    .unwrap_or(true);
  let v1_exists = dir.path().join("notes.v1.json").exists();
  let body_k1 = std::fs::read_to_string(dir.path().join("notes").join("k1.md")).unwrap_or_default();

  println!("\n=== 2.10 迁移演练：旧格式 notes.json → 索引 + notes/<id>.md（真实文件系统）===");
  println!(
    "临时目录：{}（Drop 自动删除）",
    dir.path().display()
  );
  println!(
    "迁移前：notes.json 内联条目 {} 条；notes/ 下 .md 文件 {} 个",
    entries_before, md_before
  );
  println!(
    "迁移后：内存知识条目 {} 条；notes/ 下 .md 文件 {} 个",
    store.kb.len(),
    md_after
  );
  println!("notes.v1.json（迁移前存档）存在：{}", v1_exists);
  println!("新索引仍含 bodyMd：{}（false = 纯索引）", body_still_inline);
  println!("k1.md 正文回读：{:?}", body_k1);

  assert!(store.ensure_kb_ok().is_ok(), "迁移成功，不得进入错误态");
  assert_eq!(store.kb.len(), 2, "迁移后条目数不变");
  assert_eq!(store.kb[0].id, "k1");
  assert_eq!(store.kb[0].title, "演练旧条目一");
  assert_eq!(
    store.kb[0].body_md, "第一条正文\n\n![图](assets/aa.png)",
    "内存条目正文 = 旧 bodyMd"
  );
  assert_eq!(md_before, 0, "迁移前 notes/ 下不应有 .md");
  assert_eq!(md_after, 2, "每条旧条目落一个正文文件");
  assert!(v1_exists, "notes.v1.json = 旧格式永久存档，回滚依赖它");
  assert!(!body_still_inline, "notes.json 不得内联正文");
  assert_eq!(
    body_k1, "第一条正文\n\n![图](assets/aa.png)",
    "正文文件内容 = 旧 bodyMd"
  );
  assert_eq!(
    store_notes::content_hash(&body_k1),
    store.kb[0].content_hash.clone().unwrap_or_default(),
    "索引 contentHash = sha256(正文)"
  );
}

/// tasks.md 6.5 降级演练（真实文件系统 + 临时目录，默认门禁也跑）：
/// 迁移完成后把 `notes.v1.json` 覆盖回 `notes.json`、删掉 `notes/` 与 `assets/`，
/// 再走一次生产加载路径；断言旧格式仍可被当前兼容逻辑完整解析、正文/图片重新落盘，
/// 且 `notes.v1.json` 存档不被二次迁移覆盖（回滚链不断）。
///
/// 限制：本机没有旧版安装包/旧二进制，无法验证「旧版本程序读新数据」方向；
/// 这里验证的是旧格式入口的解析与迁移幂等性（回滚依赖的存档仍在）。
#[test]
fn kb_downgrade_drill_temp_dir() {
  let dir = TempDir::new("downgrade-drill");
  let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
  let legacy = json!([
    {
      "id": "d1",
      "title": "降级演练条目",
      "bodyMd": format!("正文一段\n\n![截图]({png})"),
      "tags": ["演练"],
      "createdAt": "2026-09-03T09:00",
      "updatedAt": "2026-09-03T09:00"
    }
  ]);
  let legacy_text = serde_json::to_string_pretty(&legacy).expect("旧格式序列化失败");
  std::fs::write(dir.path().join("data.json"), DATA_JSON).expect("写 data.json 失败");
  std::fs::write(dir.path().join("notes.json"), &legacy_text).expect("写旧 notes.json 失败");
  std::fs::write(dir.path().join("folders.json"), "[]").expect("写 folders.json 失败");

  // 第一次加载 = 升级迁移
  let first = Store::load_with(
    Box::new(FsBackend::new(dir.path().to_path_buf())),
    Box::new(SystemClock),
  );
  assert!(first.ensure_kb_ok().is_ok(), "升级迁移成功");
  assert_eq!(first.kb.len(), 1, "升级后条目数");
  let archive_path = dir.path().join("notes.v1.json");
  let archive_before = std::fs::read(&archive_path).expect("notes.v1.json 应存在");

  // 模拟降级：存档覆盖回旧入口、删掉拆分出来的正文与图片目录
  std::fs::write(dir.path().join("notes.json"), &archive_before).expect("覆盖回旧格式失败");
  std::fs::remove_dir_all(dir.path().join("notes")).ok();
  std::fs::remove_dir_all(dir.path().join("assets")).ok();

  // 第二次加载 = 旧格式兼容路径重新解析 + 迁移
  let second = Store::load_with(
    Box::new(FsBackend::new(dir.path().to_path_buf())),
    Box::new(SystemClock),
  );
  let md_after = count_md_files(&dir.path().join("notes"));
  let asset_files = std::fs::read_dir(dir.path().join("assets"))
    .map(|entries| entries.filter_map(|entry| entry.ok()).count())
    .unwrap_or(0);
  let body_after =
    std::fs::read_to_string(dir.path().join("notes").join("d1.md")).unwrap_or_default();
  let archive_after = std::fs::read(&archive_path).expect("notes.v1.json 仍应存在");

  println!("\n=== 6.5 降级演练：notes.v1.json 覆盖回 notes.json → 旧格式重新解析 ===");
  println!("临时目录：{}（Drop 自动删除）", dir.path().display());
  println!("升级后：{} 条；降级覆盖后重新加载：{} 条", first.kb.len(), second.kb.len());
  println!("notes/ 下 .md：{} 个；assets/ 下资源：{} 个", md_after, asset_files);
  println!("正文已外置图片引用：{}", body_after.contains("assets/"));

  assert!(second.ensure_kb_ok().is_ok(), "旧格式重新解析成功，不得进入错误态");
  assert_eq!(second.kb.len(), 1, "降级后再升级条目数不变");
  assert_eq!(second.kb[0].id, "d1");
  assert_eq!(second.kb[0].title, "降级演练条目");
  assert_eq!(md_after, 1, "正文文件重新落盘");
  assert_eq!(asset_files, 1, "内联图片重新抽成 assets 文件");
  assert!(
    body_after.contains("正文一段") && body_after.contains("assets/"),
    "正文保留且图片引用已改写为 assets/ 相对路径，实际：{body_after:?}"
  );
  assert!(
    !body_after.contains("data:image"),
    "正文文件不得再内联 data URL"
  );
  assert_eq!(
    archive_before, archive_after,
    "notes.v1.json 存档不得被二次迁移覆盖（回滚依赖）"
  );
}