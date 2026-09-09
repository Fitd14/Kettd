# KB Phase 3 补充设计 — AI 配置与混合检索细节

> 2026-09-09 · 基于 kb-spec.md §7/§8 拍板结论，头脑风暴补全安全细节与检索管线。
> 本文档与 kb-spec.md + docs/superpowers/plans/2026-09-09-kb-ai.md 配套，用于后续核查。

## 1. AI 配置 UI（设置页「AI 与检索」段）

### 1.1 整体布局
```
┌─ AI 与检索 ──────────────────────────────────┐
│ 总开关  [●]                                    │
│                                                │
│ ┌─ Chat 模型 ──────────────────────────────┐  │
│ │ 厂商  [DeepSeek ▾]                       │  │
│ │ BaseURL  https://api.deepseek.com        │  │
│ │ 模型名  deepseek-chat                    │  │
│ │ API KEY  [sk-***a1b2          ] [显示]    │  │
│ │ 状态  ✓ 已配置          [测试连接]        │  │
│ └──────────────────────────────────────────┘  │
│                                                │
│ ┌─ Embedding 模型（语义检索）──────────────┐  │
│ │ 厂商  [OpenAI ▾]                         │  │
│ │ BaseURL  https://api.openai.com/v1       │  │
│ │ 模型名  text-embedding-3-small            │  │
│ │ API KEY  [sk-***          ] [显示]        │  │
│ │ 状态  ✓ 已配置          [测试连接]        │  │
│ │ 说明  未配置时搜索仅支持字面匹配           │  │
│ └──────────────────────────────────────────┘  │
│                                                │
│ [清除所有 AI 配置]                              │
└────────────────────────────────────────────────┘
```

### 1.2 厂商预设表
| 厂商 | BaseURL | Chat 模型 | Embedding 模型 |
|------|---------|----------|---------------|
| OpenAI | `https://api.openai.com/v1` | gpt-4o-mini | text-embedding-3-small |
| DeepSeek | `https://api.deepseek.com` | deepseek-chat | _(无，仅 chat)_ |
| 智谱 | `https://open.bigmodel.cn/api/paas/v4` | glm-4-flash | embedding-2 |
| 通义 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | qwen-turbo | text-embedding-v3 |
| Ollama | `http://127.0.0.1:11434` | llama3.2 | nomic-embed-text |
| 自定义 | _(用户填写)_ | _(用户填写)_ | _(用户填写)_ |

### 1.3 KEY 显示规则
- 未配置：输入框为空，提示「输入 API KEY」
- 已配置：回显掩码 `sk-***a1b2`（仅末 4 位），右侧「显示」按钮切换明文（临时，失焦回掩码）
- 提交：KEY 只走 `set_ai_config`→Rust 侧 DPAPI 加密→前端永不持有明文

### 1.4 测试连接
- Chat 测试：发最小请求（`messages: [{role:"user", content:"hi"}], max_tokens:1`）
- Embedding 测试：发最小请求（`input: "test"`）
- 三态结果：✓ 成功（显示响应时间）/ ✗ 失败（显示错误信息）/ ⏳ 超时（30s）

### 1.5 总开关行为
- 关=完全纯本地（等效未配置，embedding 不调用，搜索框无 AI 徽标）
- 开=按配置启用（chat 轨可选，embedding 轨决定语义搜索是否可用）
- 开关切换即时生效（set_ai_config enabled 字段）

### 1.6 清除
- 「清除所有 AI 配置」按钮→二次确认→删除 secrets.json 中 ai_config→重置为未配置状态
- 清除后 kb_index.json 保留（向量数据不因配置清除而删除，可复用）

## 2. 密钥安全架构

### 2.1 存储层级
```
WebView (JS)                    Rust 进程
─────────────                   ─────────
setAiConfig({apiKey})    →     set_ai_config 命令
                                ↓
                          SecretsKeeper.save()
                                ↓
                          DPAPI CryptProtectData()
                                ↓
                          secrets.json (加密态)
                                
getAiStatus()            ←     get_ai_status 命令
  { configured: true,            ↓
    keyMask: "sk-***a1b2"   SecretsKeeper.load()
  }                               ↓
                          DPAPI CryptUnprotectData()
                                ↓
                          内存明文（用后即弃）
```

### 2.2 安全红线
- **KEY 永不进 WebView**：前端只拿到 `configured: bool` + `keyMask: string`
- **KEY 永不进日志**：events.jsonl 只记 `ai_configured` / `ai_call_ok` / `ai_call_fail(kind)`
- **KEY 永不进 store.json**：独立 secrets.json，与业务数据物理隔离
- **传输安全**：reqwest + rustls 证书校验；非 HTTPS 拒绝（唯一豁免 http://127.0.0.1 Ollama）
- **内存安全**：解密后的明文只在 reqwest 请求头中使用，函数返回后 Rust 所有权系统自动 drop

### 2.3 降级场景
| 场景 | 行为 |
|------|------|
| secrets.json 不存在 | 视为未配置（正常启动） |
| secrets.json 损坏（非 JSON） | 静默忽略，视为未配置 |
| DPAPI 解密失败（换用户/系统重装） | 静默忽略，视为未配置 |
| secrets.json 被手动删除 | 下次启动视为未配置 |

### 2.4 DPAPI 适配
- **Windows**：`CryptProtectData` / `CryptUnprotectData`（winapi crate，feature-gated）
- **跨平台预留**：`SecretsKeeper` trait 抽象；Linux 可用 libsecret/kwallet，macOS 可用 Keychain
- **测试**：`InMemoryKeeper` 测试替身（所有非 DPAPI 测试用）

## 3. 混合检索管线

### 3.1 数据流
```
用户输入 query (≥2 字)
    ↓
┌───────────────────────────────────────┐
│ 本地全文扫（kb.rs::search）            │
│ → 带字面高亮位的评分列表               │
│ → 结果 A: [{id, score, source:'local'}]│
└───────────────────────────────────────┘
    ⊕ (并行)
┌───────────────────────────────────────┐
│ 向量召回（embedding 轨已配时）          │
│ → query embedding API 调用             │
│ → kb_index.json 余弦相似度 top-k       │
│ → 结果 B: [{id, score, source:'ai'}]  │
└───────────────────────────────────────┘
    ↓
合并重排：combined = local_norm ⊕ 0.4 × ai_norm
    ↓
单列表输出：按 combined 降序；纯 AI 命中行加「AI」徽标
```

### 3.2 分块策略
- **块大小**：~500-800 字符（非 token 精算——中文字符≈1 token，英文≈4 字符/token）
- **切分规则**：
  1. 块 0：标题（高信号，单独成块）
  2. 块 1..n：按段落（`\n\n`）累积，达到 600 字符切一块
- **存储**：`kb_index.json` → `[{itemId, chunkId, text, vector: number[]}]`
- **维护**：
  - 条目保存→防抖重建该条目 chunks（仅 embedding 轨已配时）
  - 条目删除→删除其所有 chunks
  - 索引损坏→全量重建（下次搜索触发）

### 3.3 向量相似度
- **算法**：余弦相似度 `cos(a,b) = dot(a,b) / (|a| × |b|)`
- **阈值**：≥0.25 才标记为 AI 命中（低于此视为噪声）
- **归一化**：local score 和 ai score 各自归一化到 [0,1] 后加权合并

### 3.4 厂商 Embedding 兼容性
- 大多数厂商兼容 OpenAI `/v1/embeddings` 格式（智谱/通义/siliconflow 等）
- Ollama：`POST /api/embeddings`（不同格式，需适配）
- DeepSeek：**无独立 embedding API**→embedding 轨不可配 DeepSeek（设置页不提供此组合预设）

### 3.5 错误处理
| 错误 | 行为 |
|------|------|
| 网络超时（30s） | 降级纯本地搜索 + 记 `ai_call_fail('timeout')` |
| API 401/403 | 降级纯本地 + 提示「API KEY 无效」+ 记 `ai_call_fail('auth')` |
| API 429 限流 | 降级纯本地 + 记 `ai_call_fail('rate_limit')` |
| 响应格式异常 | 降级纯本地 + 记 `ai_call_fail('parse')` |
| kb_index.json 损坏 | 全量重建 + 降级纯本地（重建期间） |
| embedding 轨未配 | 跳过向量召回，纯本地（零开销） |

### 3.6 搜索框 UI 联动
- **embedding 轨已配**：搜索框右侧不额外显示标记（混排自然发生）
- **embedding 轨未配**：搜索行为与 Phase 1 完全一致（纯字面）
- **AI 命中**：条目行尾显示「AI」小徽标（蓝色圆角标签）
- **字面命中**：保留高亮（markdown-it 渲染 + highlight.js）

## 4. kb_index.json 管理

### 4.1 文件位置
- 与 notes.json 同目录（`$APPDATA/kettd/kb_index.json`）
- 物理隔离：与 data.json（任务/设置）、notes.json（KB 条目）均独立

### 4.2 格式
```json
[
  {
    "itemId": "k1",
    "chunkId": 0,
    "text": "Rust 教程",
    "vector": [0.1, -0.3, ...]
  },
  {
    "itemId": "k1",
    "chunkId": 1,
    "text": "安装 rustup 后...",
    "vector": [0.2, 0.1, ...]
  }
]
```

### 4.3 生命周期
- **创建**：首次 embedding 调用时创建（或全量重建）
- **更新**：条目保存→删除旧 chunks→写入新 chunks
- **删除**：条目删除→删除其所有 chunks
- **损坏恢复**：读取失败→清空索引→下次搜索触发全量重建
- **配置清除**：保留（向量数据不因配置清除而删除）

## 5. 埋点清单（Phase 3 新增）

| 事件 | 触发时机 | 属性 |
|------|---------|------|
| `ai_configured` | 设置页保存 AI 配置 | `{ chat: bool, embedding: bool }` |
| `ai_test_connection` | 测试连接 | `{ track: 'chat'|'embedding', ok: bool, ms: number }` |
| `ai_call_ok` | embedding 调用成功 | `{ items_embedded: number, ms: number }` |
| `ai_call_fail` | embedding 调用失败 | `{ kind: 'timeout'|'auth'|'rate_limit'|'parse' }` |
| `kb_search_hybrid` | 混合搜索执行 | `{ has_ai: bool, local_hits: number, ai_hits: number }` |

## 6. 测试清单（Phase 3 新增）

| # | 测试项 | 类型 |
|---|--------|------|
| 1 | InMemoryKeeper CRUD | Rust 单测 |
| 2 | AiConfig 序列化往返 | Rust 单测 |
| 3 | set_ai_config + get_ai_status 往返 | Rust 单测（tokio） |
| 4 | chunk_item 段落切分正确 | Rust 单测 |
| 5 | cosine 相似度计算正确 | Rust 单测 |
| 6 | hybrid_search 纯本地回落 | Rust 单测 |
| 7 | hybrid_search 本地+向量合并排序 | Rust 单测（mock embedding） |
| 8 | DPAPI encrypt/decrypt（Windows 真机） | 真机 |
| 9 | secrets.json 损坏→静默未配置 | 真机 |
| 10 | kb_index.json 损坏→全量重建 | 真机 |
| 11 | Settings AI 段：配置→保存→重启仍在 | 真机 |
| 12 | 搜索：配 embedding→AI 徽标出现 | 真机 |
| 13 | 搜索：未配 embedding→行为不变 | 真机 |
| 14 | parity 55→59 同步 | CI |
