# kb 知识库规格（kb-spec）

> 2026-09-09 头脑风暴六轮拍板定稿（brainstorming/architectural 路径，superpowers）
> 拍板：① 知识便签=KB 条目**实时视口** ② 编辑=**所见即所得**（ProseMirror 级）③ AI 一期=**配置＋混合检索**
> ④ 便签字数**分线**（自由≤500 / 知识不限）⑤ **「条目→建待办」**反向动线 ⑥ 检索**混合单列表＋AI 徽标**

## 1. 定位与红线演进

kb = 「待办的上下文资料」层（roadmap v2 转正核心地基；非第二大脑工程）。愿景兑现：待办与知识一张互链的网，
桌面便签/知识便签 = 贴到手边的文档视口。

红线（相对 roadmap v2 的演进要显式声明）：

| 红线 | 原文 | 演进后 |
|---|---|---|
| 不出网 | 数据不出本机 | **默认不出网不变**；AI opt-in——用户配置厂商 KEY 后才允许**仅向所配厂商 endpoint** 出网（embedding/模型调用）；任何其他域名一律禁止 |
| 反功能广度 | 不做平台/第三方组件 | 渲染/编辑器依赖全部**本地 bundle（禁 CDN）**；双链插件生态仍不做 |
| 数据在你手里 | 本机 JSON | secrets.json DPAPI 加密（Windows per-user），可删可迁移 |

不做什么：双向链接/活引用（H3 再评估）、云同步/账号、AI 对话问答（二期）、全局搜索（后续增量）。

## 2. 现状锚点（后端最小地基已落地，设计照此长 UI）

- `models.rs` `KbItem{id,title,body_md,tags,created_at,updated_at}` + `KbPayload`；`Task.kb_refs: Vec<String>`
  单向（ACL：KB 永不持有 Task；注释即此契约）；`TaskPayload.kb_refs` + `store.rs apply_task_patch` 整体替换已通。
- `store.rs` `Store.kb` + notes.json 物理隔离（读损坏→静默空库；save_notes 原子写）。
- `kb.rs`：create（标题非空/5k 上限）/update/delete（返回「曾被引用」）/search（内存扫，title×2/tag×1.5/body×1，
  空查=全量按更新时间倒序）+ 4 单测。**search 签名是混合检索接缝的锚**：`search(&[KbItem], &str) -> Vec<String>`。
- 命令已注册并计入 55 对账：`get_kb_items/add_kb_item/update_kb_item/delete_kb_item/search_kb`；
  api.ts 类型与包装齐全（`getKbItems/addKbItem/updateKbItem/deleteKbItem/searchKb`）。
- **UI 层为零**：无 #/kb 路由、无视图、TaskDetail 不展示 kbRefs、无挂载入口、无 kb 埋点、无渲染依赖
  （package.json 零 markdown 库——本规格引入的全新依赖全部本地 bundle）。

## 3. 数据契约增量（零迁移原则）

| 项 | 契约 |
|---|---|
| `StickyNote.kb_ref: Option<String>` | serde `skip_serializing_if is_none`；旧数据无此字段=自由便签，零迁移。知识便签正文**不落便签** |
| `kb_index.json` | AI 向量索引旁路文件（衍生数据）：chunk 表 `{itemId,chunkId,text,vector}`；可随时全量重建；读损坏→静默降级纯本地并重建 |
| `secrets.json` | DPAPI per-user 加密存 AI 密钥/端点；损坏或换用户→静默视为未配置（同 notes.json 降级哲学） |
| add_task | **行为扩展**：TaskPayload.kb_refs 已存在但被硬编码忽略 → 放开入参（新建即挂）。命令数不变 |
| `create_sticky` | 加可选 `kb_ref` 参数（知识便签入口）。命令数不变 |
| 命令面净增 3 | `set_ai_config / get_ai_status / test_ai_connection` → 对账 **55→58**（parity 三处：generate_handler/单测断言/V2-API §10） |

分池上限：自由便签 ≤6（现状 cap 不变）；知识便签 ≤4（视口轻量，正文不落便签故独立计数）。
「贴到桌面」= `create_sticky({kb_ref})` → 建 note:* 动态窗；关闭 = `delete_sticky`（只拆窗，KB 数据不删）。

## 4. 知识库视图（#/kb 第六视图，侧栏置「回顾」与「设置」之间）

```
┌ 今天│收件箱│计划│回顾│知识库│设置 ┐  （ROUTES 插入 {hash:'#/kb', label:'知识库'}）
┌────────────────────────────────────┐
│ 搜索框  [?]          [新建条目]     │ ← 置顶：键入即过滤（≥2 字）；配 KEY 后自动混合检索
│ 标签筛选行 chips…（全部/工作/生活…） │
│ ┌──────────────┬──────────────────┐ │
│ │ 左清单         │ 右详情            │ │
│ │ · 周报模板  #工作│ 工具栏：阅读│编辑│贴到桌面│转为待办│删除
│ │ · 例会纪要  AI  │ ──────────────   │ │
│ │ · 项目复盘      │ 正文：所见即所得渲染 │
│ │               │ （编辑态=WYSIWYG）  │ │
│ │ ↑新建(空态引导) │ （阅读态=全宽成品）  │ │
│ └──────────────┴──────────────────┘ │
└────────────────────────────────────┘
```

- **搜索**：本地命中字面高亮；AI 语义命中无字面→条目行尾「AI」小徽标（混排单列表，按相关度合并排序）。
  空查询=最近更新全量。未配 KEY 时搜索框右侧不出现语义标记，行为与现状零差异。
- **左清单行**：标题（截断）＋标签 chips＋相对更新时间；语义命中行 AI 徽标；选中高亮。
- **右详情**：阅读态（全宽渲染）⇄ 编辑态（WYSIWYG，见 §5）双态；工具栏四动作＋删除（二次确认）。
- **「贴到桌面」**：见 §6 知识便签。「**转为待办**」：预填任务标题（条目标题），add_task＋update_task 补挂 kbRefs，
  完成后跳「今天」并提示任务已建（收件箱理念延伸；触发埋点 kb_item_to_task）。
- **空态**（零条目）：引导文案 +「新建第一条知识」主钮；**搜索无结果**：「没找到——换个词试试 / 语义检索也没命中」；
  5k 上限报错沿用 kb.rs 文案；**失效条目**（任务/便签引用后删除）：在挂载处显示，不进入清单。

## 5. Markdown 渲染内核（一处实现、三面复用）

**编辑器**：TipTap（ProseMirror 级所见即所得，本地 bundle）；Markdown 输入规则（`# ` 标题、`- ` 列表、
``` fenced 代码照打即成型）；表格节点原生支持。轻依赖声明：不引 CDN、不引 BlockNote/Lexical。

**存储往返**：`bodyMd` 仍是唯一真相。ProseMirror↔Markdown serializer（tiptap-markdown 或自维护）。
保真守卫测试：给定 markdown 样本集（标题/列表/任务列表/表格/代码块/引用/链接/粗斜体/行内代码/多级嵌套），
读→编辑零改动→存，与原文字节级稳定（除行尾空白规范化）；样本集进 `test/md-fidelity.test.*`。

**RendererRegistry（插件架构，兑现「mermaid 等后续图表」预留）**：

```ts
interface MdRenderExtension {
  id: string
  languages: string[]          // fenced code block 的语言标记：['mermaid']…
  editNode?: NodeViewSpec      // WYSIWYG 编辑期内联渲染的节点视图（聚焦回落源码）
  staticRender: (code: string) => ReactNode   // 阅读态渲染（含错误态）
}
registerMdExtension(ext)       // v1 注册表内置：核心渲染 + 代码高亮（未注册 lang 回落）
```

- 扩展内容即 markdown fenced block —— **存量数据零迁移**，未来注册扩展即可渲染旧条目。
- 错误态：渲染器 throw/返回失败 → 显示源码块＋错误行（绝不白屏，绝不吞编辑内容）。
- **三面复用与体积纪律**：主窗 KB 视图注册全量扩展；便签/知识便签窗共享渲染 chunk 但**只注册核心渲染器**
  （mermaid 等重扩展不进便签窗，保 380×456 秒开）。vite 手动分包锁定共享 chunk。

## 6. 便签 × KB（双轨）

**自由便签**（sticky-separation 后的纯自由便签 + Markdown）：
- ≤500 字上限**保持**（纯文本心智不变，500 是「便签仍是便签」的产品线护栏）；渲染开关默认**关**；
  开启后正文走渲染内核实时显示；渲染态点击正文回编辑态。触发埋点 sticky_md_toggle。
- 数据零变化（content 恒序列化不变）；渲染是显示层能力。

**知识便签**（本轮核心增量，roadmap H2「知识便签」提前兑现）：
- 创建：KB 详情「贴到桌面」→ `create_sticky({kb_ref})`，桌面出现 note:* 窗（复用多便签全链路：
  拖拽记位 notePos、缩小 mini=置顶条（显示条目标题、点击展开）、关闭=拆窗）。
- **实时视口语义（拍板①）**：便签窗=KB 条目视口。窗内编辑防抖（≤800ms）写回 `update_kb_item` →
  store-changed 广播 → 主窗/其他知识便签同步。**无分叉可能**（正文唯一真相在 KB）。
- 不受 500 字限（正文存 KB）；便签内不存正文副本，只存 `kb_ref`。
- 失效处理：KB 条目被删除 → 知识便签检测引用失效 → 内容区「已失效」置灰 +「拆除便签」钮（delete_sticky）；
  不自动关闭（用户可能想先看内容快照）。
- 上限：知识便签 ≤4 张（分池，§3）。

## 7. 任务侧

- **TaskDetail「挂载资料」区**（只读弹层内新增段）：已挂条目卡列表——条目名＋标签；点击 → 主窗跳 #/kb 并定位该条；
  悬空引用显示「已失效」置灰；「挂载资料」钮 → 搜索弹层（searchKb）选择 → updateTask({kbRefs}) 整体提交（现状 patch 语义）；可逐条卸载。
- **任务行资料小图标**（P1）：挂载了资料的任务行尾小图标，hover 提示「有 N 条资料」，点击开 TaskDetail。
- add_task 放开 kb_refs（§3）：捕获条/快速新建维持不填（3 秒记不变重）；「转为待办」路径才用。
- 删除 KB 条目时若被任务引用：delete_kb_item 现有返回值语义保留 → 前端提示「N 个任务/便签正在引用」。

## 8. AI 配置与密钥安全

**设置页「AI 与检索」段**：

| 控件 | 规格 |
|---|---|
| 厂商预设 | OpenAI / DeepSeek / 智谱 / 通义 / Ollama(本地) / 自定义——预设只填 BaseURL+默认模型名，可改 |
| BaseURL / 模型名 / Embedding 模型名 | 明文编辑（非密钥）；Ollama 预设默认 `http://127.0.0.1:11434`（明文 HTTP 唯一豁免） |
| API KEY | 密码框；回显仅掩码 `sk-***<末4位>`；提交走 `set_ai_secret` 直交 Rust，**永不进 WebView/DOM** |
| 测试连接 | `test_ai_connection(track)`：Rust 侧对指定轨（chat/embedding）发最小请求，成功/失败/超时三态文案 |
| 总开关 | 关=完全纯本地（等效未配置）；清除=删 secrets.json 中本项 |

**密钥安全**（全部 Rust 侧）：
- `set_ai_secret` 命令收 KEY → DPAPI `CryptProtectData`（per-user）→ 写 secrets.json。读取解密仅发生在 Rust 内存，用后即弃。
- 前端此后只持有 `aiConfigured: bool` + 掩码尾号（`get_ai_status`）。
- 传输：reqwest＋rustls 证书校验；非 HTTPS 一律拒绝，**唯一豁免 http://127.0.0.1**（本地 Ollama）；每请求超时上限（embedding 30s）。
- 日志红线：events.jsonl 只记无内容事件——ai_configured / ai_call_ok / ai_call_fail(kind)；**永不记 KEY/提示词/条目内容/响应内容**。
- 密钥安全测试：DPAPI 走可注入端口（trait SecretsKeeper：dpapi 实现 + 测试替身）；损坏文件→未配置降级；掩码逻辑单测。

**厂商预设表**（规格内定默认，实施可调）：OpenAI `https://api.openai.com/v1`(gpt-4o-mini / text-embedding-3-small)、
DeepSeek `https://api.deepseek.com`(deepseek-chat)、智谱 `https://open.bigmodel.cn/api/paas/v4`(glm-4-flash / embedding-2)、
通义 `https://dashscope.aliyuncs.com/compatible-mode/v1`(qwen-turbo / text-embedding-v3)、Ollama `http://127.0.0.1:11434`(llama3.2 / nomic-embed-text)。

**双轨独立配置（消除厂商 embedding 缺位）**：chat 模型与 embedding 模型是**两条独立轨**（各自 BaseURL/模型名/KEY，
均可从任意预设或自定义取）——例如 chat 用 DeepSeek、embedding 用 OpenAI 兼容源（siliconflow 等）。
`get_ai_status` 返回 `chat: 已配/未配` + `embedding: 已配/未配`。**语义检索只在 embedding 轨已配时启用**；
embedding 未配（如用户只配了 DeepSeek）→ 搜索保持纯字面，AI 徽标永不出现，设置页给一行解释文案（不报错、不强制）。

## 9. 混合检索管线（RAG 雏形）

配 KEY（embedding 轨已配）且总开关开时，搜索升级为：

```
query(≥2字)
 ├─ 本地全文扫（现成 kb.rs 打分，结果带「字面命中」位 → 可高亮）
 └─ 向量召回：chunk 化 kb_index.json → query embedding → 余弦 top-k（≥0.25 阈值可调）
合并重排（字面得分 ⊕ 0.4×向量分 归一化）→ 单列表；纯语义命中行加「AI」徽标（拍板⑥）
```

- **分块**：按标题/二级标题/段落切，~500-800 token/块，块记 `{itemId, chunkId, text, vector}`。
- **维护**：条目保存（update_kb_item/add_kb_item）后防抖重建该条目 chunk（仅当已配 KEY；未配=零开销跳过）；
  kb_index.json 损坏→全量重建触发点=下次搜索；条目删除→同步删其 chunk。
- 未配 KEY / 关总开关 / embedding 调用失败：全链路**零差异回落纯本地**（失败降级一次并记 ai_call_fail，不阻塞 UI）。
- 二期预留：对话问答（上下文组装=命中条目 top-k 注入）、厂商扩展。语义检索开关 UI 本期不做——配了即用（拍板③最小惊讶）。

## 10. 埋点与验证门

旧 H1 验证门判据（「钉」动作）已被 sticky-separation 退役；KB 用新信号回答「存了有没有搜、有没有从任务打开、贴了有没有看」：

`kb_create / kb_update / kb_delete / kb_search(带 hits/aiHits) / kb_open_from_task / kb_item_to_task /
task_kb_attach / kb_pin_desktop / kb_unpin_desktop / sticky_md_toggle / ai_configured / ai_call_ok / ai_call_fail`

## 11. 测试与门禁、真机复验清单

**门禁**：cargo test（kb.rs 4 现有 + 新增：kb_ref 生命周期/分池上限/失效检测/DPAPI 端口/掩码）+ cargo build；
parity 55→58 三处同步；`tsc -b && vite build`（分包锁定共享 chunk）；oxlint；md 往返保真守卫（§5）。

**真机复验清单**：
① #/kb 进视图、新建条目→WYSIWYG 打 `# 标题`/`- 列表`/```代码即时成样式；改完主窗搜索/便签同步
② 搜索字面命中高亮；配 DeepSeek 等 KEY 后同词出现 AI 徽标条目；删 KEY/关开关后回落零差异
③ 条目「贴到桌面」→ 知识便签出现；便签内编辑→主窗 KB 原文同步（反向亦然）；关闭便签→数据仍在
④ 知识便签缩小 mini=标题条、展开恢复；重启回位；自由便签渲染开关默认关、≤500 照拦
⑤ KB 条目删除→任务挂载与知识便签均显「已失效」；「转为待办」建任务并挂链
⑥ 设置页 KEY 掩码回显、测试连接三态、重开应用仍已配置（DPAPI 解密）；换 Windows 用户→视为未配置
⑦ secrets.json 手工损坏→启动正常、AI 段显示未配置；kb_index.json 删除→下次搜索自动重建
⑧ 命令对账 58、设置页四热键/恢复默认不受影响

## 12. 后续增量（防膨胀，不做进一期）

捕获条片段模式（动「3 秒记」，需单独立项）；自由便签一键沉淀为 KB 条目；mermaid 扩展落地（注册表首个扩展，本地 bundle）；
AI 对话问答（二期）；全局搜索（任务+KB）；活引用/双向（H3 评估）；KB 视图响应式窄宽降级。
