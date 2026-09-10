# kb 知识库 · 设计验收报告（QA）

- **验收目标**：kb 知识库 Phase 1-3 实现 + 真机复验六问题修复（HEAD = 1b7782e）
- **验收时间**：2026-09-10
- **验收模式**：模式 A 自动比对（设计源 = `spark-output/design/kb-spec.md` + `kb-phase2-supplement.md` + `kb-phase3-supplement.md`；实现 = src-react/src/{views/KbView,components/kb/*,rendering/*,sticky/StickyWindow,components/settings/ai-section,components/task-detail} + src-tauri/src/ai/*）
- **方法**：静态比对 + 6 组 grep 取证（证据见各条 location）

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 2 |
| 🟠 Major | 3 |
| 🟡 Minor | 9 |

**通过维度**（零偏差）：间距 · 字体 · 圆角阴影 · 图标与图片 · 响应式
**Check finding 核对**：上游 check.json 属多便签范围（09-07），与 kb 无对应 finding → N/A（0/0）

## Deviations

### 🔴 Blocker

**qa-1 ｜状态完整性｜混合检索向量召回未接线——AI 徽标永不出现**
- 设计源：kb-spec §9「配 KEY 后 query → 本地扫 ⊕ 向量召回 → 合并重排，纯语义命中标 AI 徽标」；验收项「配 embedding 后搜索出现 AI 徽标」
- 实现：`src-tauri/src/ai/hybrid.rs:51-54` ai_map **恒为空**（注释「暂时返回空」）；`embed_texts` 零生产调用点（仅定义与测试）；`kb_index.json` 无任何读写实现（仅注释提及）
- 差异：embedding 轨即使完整配置，搜索行为与纯本地**完全相同**；Phase 3 核心承诺未兑现
- 修复建议：hybrid_search 改 async 接 query embedding → 余弦 top-k → 合并；add/update_kb_item 后防抖重建该条 chunk；store 增加 kb_index 原子读写（损坏→清空重建）
- 位置：src-tauri/src/ai/hybrid.rs、src-tauri/src/ai/embedding.rs:51、src-tauri/src/store.rs:608

**qa-2 ｜状态完整性｜AI 配置注入 InMemoryKeeper——重启即丢 + 安全文案失实**
- 设计源：kb-spec §8「KEY 经 DPAPI per-user 加密落 secrets.json；重启仍已配置；设置页显示掩码」
- 实现：`main.rs:86-88` 生产注入 **InMemoryKeeper**（内存 HashMap）；DpapiKeeper 存在但内部是 **base64 占位**（`secrets.rs` 内 TODO 注释，未接 CryptProtectData）；设置页文案却宣称「🔒 KEY 经 DPAPI 加密存储本机」
- 差异：① 配置重启全丢（验收项「重启仍已配置」必挂）② secrets.json 永不落盘 ③ 加密承诺与实现不符
- 修复建议：main.rs 按 Windows 注入 DpapiKeeper 并真正接入 winapi CryptProtectData/CryptUnprotectData（去 base64 占位）；文案在落地前先改为「本机会话内存储」
- 位置：src-tauri/src/main.rs:86、src-tauri/src/ai/secrets.rs（DpapiKeeper impl）

### 🟠 Major

**qa-3 ｜颜色｜`--accent` 裸引用产生无效 CSS——AI 徽标不可见**
- 设计源：index.css 令牌体系 `--accent: 36 100% 95%`（HSL 三元组，须 `hsl(var(--accent))` 使用）
- 实现：kb.css 三处裸 `var(--accent, hsl(210 100% 50%))`——var 命中后整值为 `36 100% 95%`，作为 color/background **非法**，声明被丢弃
- 差异：① `.ai-badge{background:var(--accent)…}` → 徽标透明底+白字 = **看不见** ② `.kb-item-row.active` 的 `border-left:3px solid var(--accent)` 被丢弃 → 选中指示减弱 ③ `.kb-editor-tool.on` 的 `color:var(--accent)` 被丢弃
- 修复建议：统一改 `hsl(var(--accent) / α)` 写法（或换 `--primary`）；AI 徽标建议直接用 `--primary` 反白
- 位置：src-react/src/views/kb.css（.ai-badge / .kb-item-row.active / .kb-list-toggle 相邻规则）、同文件 .kb-editor-tool.on 在 prosemirror.css

**qa-4 ｜交互态｜埋点 13 缺 10——新验证门信号断裂**
- 设计源：kb-spec §10 列 13 事件（旧「钉」判据退役后的替代验证信号）
- 实现：仅 kb_pin_desktop / kb_unpin_desktop / sticky_md_toggle 已接（grep 证实）；kb_create / kb_update / kb_delete / kb_search / kb_open_from_task / kb_item_to_task / task_kb_attach / task_kb_detach / ai_configured / ai_call_ok / ai_call_fail **均未埋**
- 差异：kb 有没有被用（spec 定义的上线后判据）将无法回答
- 修复建议：前端五处 trackEvent（视图 CRUD/搜索/deep-link/转待办/挂载卸载）；ai_* 三事件埋在 Rust set_ai_config 与 embedding 调用成败分支
- 位置：src-react/src/views/KbView.tsx、components/kb/*、components/task-detail.tsx、src-tauri/src/ai/{config,embedding}.rs

**qa-5 ｜可访问性｜知识条目行不可键盘操作**
- 设计源：QA 9.3/9.4（Tab 可达、Enter/Space 触发）；kb-item-row 已标 `role="option"`
- 实现：`KbItemRow` 为 div+onClick，无 tabIndex、无 onKeyDown；listbox 容器无键盘导航
- 差异：键盘用户无法选中/打开任何条目（详情面板从此不可达）
- 修复建议：行改 button 或加 tabIndex={0}+onKeyDown(Enter/Space)；容器补 aria-activedescendant 或改 listbox 罗斯特式
- 位置：src-react/src/components/kb/kb-item-row.tsx

### 🟡 Minor

**qa-6 ｜契约｜add_task 未放开 kbRefs 入参**
- spec §3 要求放开；实现仍 `kb_refs: Vec::new()`（store.rs:314）。「条目→建待办」走两步调用功能等价，无用户可见差异。建议补一行改动+测试对齐 spec。

**qa-7 ｜状态完整性｜错误静默：上限/失败无提示**
- 5k 条目上限（addKbItem Err）、知识便签 4 张上限（createSticky Err）、贴桌面失败均被 `if (result.data)` / `if (!result.err)` 吞掉；KbEmptyState 的 `limit` 变体永不可达。建议统一 inline-err 呈现。

**qa-8 ｜状态完整性｜删除 KB 条目无「N 个任务/便签正在引用」提示**
- spec §7 要求利用 delete_kb_item 返回值提示；前端未读返回值（kb-item-detail.tsx handleDelete）。

**qa-9 ｜状态完整性｜设置页便签清单知识便签显示占位符**
- 补充设计 §4.2 要求显示条目标题；实现显示「📎（知识便签）」。建议批量拉 KB 标题映射。

**qa-10 ｜契约｜V2-API `create_sticky` 行过期**
- 仍写「新建一张自由便签…只有这一种…≥6 Err」，未含 `kbRef?` 参数、分池（自由≤6/知识≤4）与知识便签语义（kb-spec §3）。parity 只对命令名，参数文档需手同步。

**qa-11 ｜状态完整性｜任务行资料小图标（P1）未实现**
- spec §7 标 P1 可选；当前仅 TaskDetail 内可见挂载。建议延后至有真实使用数据后再做（与 spec 一致），或在 QA 台账明确销账。

**qa-12 ｜交互态｜AI 总开关初始加载时「关」侧误高亮**
- `status?.enabled !== true` 在 status=null（加载中）时为 true → 「关」显示 on，加载完成才跳变。建议 status 未载时两钮均不高亮。

**qa-13 ｜交互态｜保存配置时空字段覆写另一轨已存值**
- save() 对 baseUrl/model **总是**传字符串（空串=Some("")=覆盖）；getAiStatus 加载失败时直接保存会清空既有配置。建议空串字段不下发（与 KEY 同策略）。

**qa-14 ｜状态完整性｜知识便签失效态布局类缺失**
- 失效态复用 `.kb-empty`，但 kb.css 只随主窗 KbView 加载，便签窗 bundle（index.css+sticky.css）无此类 → 居中排版失效（内容仍可读、按钮可用）。建议把失效态样式内联进 sticky.css。

## 通过维度明细（零偏差）

| 维度 | 结论 |
| --- | --- |
| 间距 | ✓ rem 阶梯与全库一致；同类间距统一（kb-view/track/grid） |
| 字体 | ✓ 全继承令牌字体族；无裸 font-size 混断（工具栏 px 尺寸属控件尺寸非字号） |
| 圆角阴影 | ✓ 沿用 var(--radius)/既有 6px 阶，与 app.css 卡片语言一致 |
| 图标与图片 | ✓ lucide SVG（currentColor）+ 字符图标，无位图 |
| 响应式 | ✓ 桌面 App（minWidth 1024）；左列固定窄带 + 折叠兜底；移动断点 N/A |

## 修复优先级建议

- **必须修复**（阻断验收）：qa-1、qa-2 + 用户可见 bug qa-3 —— 3 项
- **建议修复**：qa-4（验证门信号）、qa-5（键盘可达）—— 2 项
- **可延后**：qa-6..14（其中 qa-10 文档行可随手修）—— 9 项
