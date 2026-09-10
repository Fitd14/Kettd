# kb QA 修复设计（kb-qa-fixes）

> 2026-09-10 头脑风暴拍板 · 修复对象 = qa.json 14 deviations（2B/3Maj/9Min）
> 拍板：① 混合检索=**渐进增强**（键入即本地，停 400ms 发 embedding 合并+徽标）② 修复节奏=**两波**（必修先走）
> 本文档与 spark-output/qa/kb-验收报告.md + context/qa.json 配套，用于后续核查。

## Wave 1（必修：qa-1/2/3/4/5）

### qa-2 · DPAPI 持久化（独立 Rust，先做）

- 依赖：`[target.'cfg(windows)'.dependencies] winapi = { version = "0.3", features = ["dpapi","wtypes","impl-default"] }`
- **DpapiKeeper 真实现**（替换 base64 占位）：
  - `save`：整张 JSON `CryptProtectData`（per-user，**无附加熵**，pbDataDescr=NULL）→ base64 → secrets.json（整体密文，键名也不暴露）
  - `load`：读文件 → base64 解码 → `CryptUnprotectData` → parse；任一步失败 → None（静默未配置，零差异降级）
  - `delete`：load→删键→save；文件不存在直接 Ok
  - 原子写沿用 tmp+rename
- `main.rs`：`#[cfg(windows)]` 注入 `DpapiKeeper::new(data_dir)`（data_dir 用既有 dirs crate）；非 Windows 回退 InMemoryKeeper + 启动 log 警示（Windows-first）
- DPAPI 无法单测 → JSON 编排层抽纯函数可测；**真机复验**：配置→重启→仍在；secrets.json 打开为密文
- 设置页「DPAPI 加密」文案修完即属实（不改）

### qa-1 · 混合检索接线（渐进增强）

- `ai/hybrid.rs` 新增 **async** `hybrid_search_async(query, items, index, ai: Option<&AiConfig>) -> Vec<HybridResult>`：
  - 本地扫同步照旧（kb::search → local_map 归一化）
  - ai 已配：`embed_texts(&[query])` → 对 index 逐 chunk cosine → per-item 取**最大**分 → 阈值 ≥0.25 计入 ai_map
  - 合并 `local_norm + 0.4×ai_norm` 降序；ai 分达标且本地无字面命中 → source=Ai
- **锁纪律**：`search_kb_hybrid` 改 async command——先短锁 clone 出 kb/kb_index/config **drop guard 再 await**（不持 Mutex 跨网络调用，假死教训同源）
- **前端渐进增强**（KbView）：
  - 键入 → 立即 `searchKb` 本地渲染（现状回落路径提前为第一跳）
  - 停手 **400ms** 防抖 → `searchKbHybrid(query)` → 到达后合并重排 + setAiHits
  - 竞态：请求序号 seq，只认最新一次返回
  - 缓存：`Map<query, {ts, results}>` 60s 内同词直接复用（不重复计费）
  - 冷却：一次失败后 **60s** 内不再发起（记 failTs），期间纯本地
- **kb_index.json 持久化**：
  - Store：`save_kb_index()` 原子写（tmp+rename，同 notes.json）；启动加载，损坏→清空（视同未建）
  - `add/update/delete_kb_item` 成功后 `tokio::spawn` 后台任务：embedding 已配才做——chunk_item → embed_texts → 替换该条 chunks → save_kb_index；**不持锁做网络**（clone 后操作，写回短锁）
  - 搜索发现 index 空且已配 → 触发全量后台重建；重建期间查询只回本地（结果自然，不阻塞）
- 埋点：`ai_call_ok{ms,n}` / `ai_call_fail{kind}` 在 embed 调用成败分支

### qa-3 · accent 无效 CSS（用户可见 bug）

全部改合法 `hsl(var(--token) / α)` 形式：
- `.ai-badge` → `background: hsl(var(--primary)); color: hsl(var(--primary-foreground))`（主令牌反白，可见性优先）
- `.kb-item-row.active` 边框/底色 → 对齐 app 现有选中语言（执行时对照 `.side-item.on` 取同款令牌组合）
- `.kb-editor-tool.on` → `color/background` 同规合法化

### qa-5 · 键盘可达

`KbItemRow`：div → `<button type="button">`（CSS 补 `width:100%; text-align:left; border:none; background 继承纸面` 重置），Enter/Space 原生触发；listbox/option 语义保留

### qa-4 · 埋点（11 处，穿插改）

前端 trackEvent：`kb_create / kb_update / kb_delete / kb_search{hits,aiHits} / kb_open_from_task（TaskDetail ↗）/ kb_item_to_task（弹窗成功）/ task_kb_attach / task_kb_detach`
Rust：`ai_configured{chat,embedding}`（set_ai_config 成功）/ `ai_call_ok / ai_call_fail{kind}`（embed 处）

## Wave 2（minor 批量）

| id | 修法 |
|---|---|
| qa-6 | store.rs add_task 读 payload.kb_refs（缺省空）+1 测试 |
| qa-7 | handleCreate/onPin 错误 → inline-err state 呈现 |
| qa-8 | 删除确认读 delete 返回值，被引用时确认文案加「有任务/便签引用将悬空」 |
| qa-9 | SettingsView 拉 getKbItems 建标题映射，知识便签行显示真实标题 |
| qa-10 | V2-API create_sticky 行更新（kbRef 参数/分池/知识便签语义） |
| qa-11 | 任务行图标按 spec P1 **延后**，台账销账（不改代码） |
| qa-12 | status===null 时开关两钮均不高亮 |
| qa-13 | save() 空 URL/模型不下发（与 KEY 同策略：trim 后空 → undefined） |
| qa-14 | 失效态居中样式并入 sticky.css（.sticky-invalid-body），弃用 kb-empty |

## 验证门（每波结束跑全量，看真实 pass/fail）

cargo test（107+hybrid async 新测试）/ cargo build / tsc -b && vite build / oxlint 0 错误 / parity 59 / md fidelity 12 / 真机复验清单追加：DPAPI 重启持久+密文、AI 徽标出现、停 400ms 合并、失败冷却
