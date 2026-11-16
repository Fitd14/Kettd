# Markdown 保存往返（round-trip）基线 —— 批次 5 / #10 / tasks.md 6.1

## 1. 目的与判定口径

保存链路的真实契约：**markdown → 编辑器解析 → `storage.markdown.getMarkdown()` → 落盘**。
从磁盘再次打开时会重复一次「解析 → 序列化」，因此要求：

1. **幂等**：`out2 === out1`（第二次保存与第一次完全一致＝二次保存稳定，不会越存越坏）；
2. **文本不丢**：`doc1.textContent === doc2.textContent`；
3. **能力保留**：表格对齐、单元格内字面竖线、硬换行反斜杠、任务列表、autolink 等按预期断言。

门禁脚本：`src-react/test/md-roundtrip.test.mjs`，直接 `node` 执行（不用 `node --test`；Windows 沙箱 spawn 子进程会 EPERM）。
共 **21 个用例**：18 个 Markdown 文本 fixture + 3 个结构化 JSON fixture（覆盖 Markdown 语法无法表达的边界形态：单元格内硬换行、无表头表格、colspan）。

```bash
node src-react/test/md-roundtrip.test.mjs            # 门禁模式（失败 exit 1）
node src-react/test/md-roundtrip.test.mjs --report   # 额外输出逐 fixture 明细（生成本基线用）
```

测试栈**镜像** `src-react/src/rendering/tiptap-editor.tsx` 中与 Markdown 保存相关的扩展：
`StarterKit.configure({codeBlock:false})` + `CodeBlockLowlight` + `TaskList` + `TaskItem.configure({nested:true})` +
`Table` 家族 + `Placeholder` + `Markdown.configure({html:false, linkify:true, breaks:true})`。
若 `@tiptap/extension-image` 可解析安装则自动加入栈（#15 落地后图片断言自动升级），否则记为已知限制。
`src/rendering/table-alignment.ts` 存在时走「修复后」自定义 Table 序列化，不存在时回退原生 Table（即本基线）。

环境：Node v22.23.3、jsdom 24.1.3（`src-react/node_modules` 中的传递依赖，未写入 package.json、未新增依赖）、
@tiptap/* 3.31.3、tiptap-markdown 0.9.0、prosemirror-markdown 1.13.7、markdown-it 15.0.1。
归一化口径复刻 `src-tauri/src/kb.rs` 的 `normalize_body_md`（只裁首尾 `\n`/`\r`，保留行尾空格）。

> 说明：`@tiptap/extension-table` 3.31.3 的 TableCell/TableHeader **上游已内置 `align` attr**
> （从 markdown-it 渲染出的 `<th>/<td style="text-align:...">` 解析，归一化为 left/center/right/null）。
> 因此修复只需补「序列化器输出对齐分隔行」，无需新增/修改 attr（不要重复实现，避免与上游冲突）。

## 2. 修复前汇总（vanilla `@tiptap/extension-table` 默认表格序列化）

第一次复现只含 17 个 Markdown fixture（#10 原复现的 59 样例中挑出的主链路代表），记录：
**TOTAL=17、CHANGED=14、NOT_IDEMPOTENT=2、TEXT_LOST=2、FAILURES=10**。

修复过程中补充了 4 个回归用例（`table-bold-escaped-pipe` + 3 个结构化 fixture），并用同一份测试在
「临时移除 `table-alignment.ts` → 回退原生 Table」状态下重跑，得到纳入全部 21 用例的完整修复前基线：

| 指标 | 值 |
| --- | --- |
| fixture 总数 TOTAL | 21 |
| 一次保存后变化 CHANGED | 15 |
| 非幂等 NOT_IDEMPOTENT | 6 |
| 文本丢失 TEXT_LOST | 5 |
| 失败断言 FAILURES | 29 |

6 个修复前非幂等样例：

- `table-escaped-pipe`：`\|` 被默认序列化器原样写出 → 下次解析变列分隔符，内容被截断；
- `table-code-pipe`：行内代码中的 `\|` 位于 code span 内，序列化时绕过 `esc()` 直接输出裸 `|` → 下次解析列错位；
- `table-bold-escaped-pipe`：`**a \| b**` 的转义竖线在粗体 mark 外被默认序列化器输出为裸 `|` → 二次解析列错位；
- `structured-cell-hardbreak`：单元格内 hardBreak 被写成 `[hardBreak]` 占位文本 → 二次保存被转义为 `\[hardBreak\]`；
- `structured-headerless-table`：默认 serializer 不支持无表头表格 → 整表回退为 `[table]` → 二次保存 `\[table\]`，内容全丢；
- `structured-colspan-table`：默认 serializer 不支持 colspan → 整表回退为 `[table]`，内容全丢。

> 修复前 21 例数据的复现方式：临时将 `src-react/src/rendering/table-alignment.ts` 移出（改为 `.ts.bak`），
> 测试会自动回退到原生 Table 并走「修复前」分支，`node src-react/test/md-roundtrip.test.mjs --report` 采集后立即还原。
> 门禁日常运行不需要也不应做这一步——`table-alignment.ts` 存在即走修复后序列化。

## 3. 逐 fixture 基线（修复前）

- `bullet-star`：`* 苹果⏎* 香蕉` → `- 苹果⏎- 香蕉`；CHANGED、幂等 ✅、文本 ✅（接受归一化：无序标记统一 `-`）。
- `bullet-plus`：`+ 苹果⏎+ 香蕉` → `- 苹果⏎- 香蕉`；CHANGED、幂等 ✅、文本 ✅（同上）。
- `ordered-paren`：`1) 甲⏎2) 乙` → `1. 甲⏎2. 乙`；CHANGED、幂等 ✅、文本 ✅（接受归一化：`1)` → `1.`）。
- `setext-h1`：`一级标题⏎===` → `# 一级标题`；CHANGED、幂等 ✅、文本 ✅（接受归一化：setext → ATX）。
- `setext-h2`：`二级标题⏎---` → `## 二级标题`；CHANGED、幂等 ✅、文本 ✅（同上）。
- `fence-tilde`：`~~~js⏎const n = 1⏎~~~` → ` ```js⏎const n = 1⏎``` `；CHANGED、幂等 ✅、文本 ✅（接受归一化：`~~~` → ```）。
- `table-align`：三列对齐 `:---` / `:---:` / `---:` →
  `| 左对齐 | 居中 | 右对齐 |⏎| --- | --- | --- |⏎| a | b | c |⏎| 甲 | 乙 | 丙 |`。
  CHANGED、幂等 ✅、文本 ✅；doc1 的 `align` attr 三行均为 `left/center/right`（解析没问题），
  **doc2 的 align attr 全变 null（对齐信息在保存时丢失）**；分隔行断言失败。← 6.2 主修复点。
- `table-plain`：无对齐表格，原样往返（CHANGED=false、幂等 ✅、文本 ✅、align 全 null）。
- `table-escaped-pipe`：`| x \| y | z |` → `| x | y | z |` → 二次保存 `| x | y |`；
  **非幂等 ❌、文本丢失 ❌**（`x | y` 被截断，`z` 丢失）。← 6.2 主修复点。
- `table-code-pipe`：`` | `a \| b` | 含竖线 | `` → `` | `a | b` | 含竖线 | `` → 二次保存 `` | \`a | b\` | ``；
  **非幂等 ❌、文本丢失 ❌**（代码 span 内竖线未转义导致列错位）。← 6.2 主修复点。
- `table-bold-escaped-pipe`（修复中新增回归）：`| **a \| b** | c |` → `| **a | b** | c |` → 二次保存
  `| \*\*a | b\*\* |`；**非幂等 ❌、文本丢失 ❌**、doc2 对齐 attr 全 null。
- `hardbreak-two-spaces`：`第一行␠␠⏎第二行` → `第一行\⏎第二行`；CHANGED、幂等 ✅、文本 ✅
  （接受归一化：两空格硬换行 → 反斜杠硬换行；6.2 要求「统一反斜杠形式」已满足）。
- `hardbreak-backslash`：`第一行\⏎第二行` → 原样；CHANGED=false、幂等 ✅、文本 ✅。
- `bare-url`：`https://example.com` → `<https://example.com>`；CHANGED、幂等 ✅、文本 ✅（接受归一化：autolink）。
- `task-list-loose`：松散任务列表（项间空行、嵌套项）→ 原样；CHANGED=false、幂等 ✅、文本 ✅。
- `image`：`![示意图](https://example.com/a.png)` → `""`（空串）；CHANGED、幂等 ✅（空串往返）、文本 ✅（两边都无文本）。
  **图片整体丢失**，属已知限制（schema 无 image 节点，#15 负责 Image 扩展）。
- `inline-html`：`这是 <b>粗体</b> …` → `这是 &lt;b&gt;粗体&lt;/b&gt; …`（HTML 实体转义）；CHANGED、幂等 ✅、文本 ✅
  （接受归一化：行内 HTML 以实体文本保存；二次保存稳定）。
- `mixed-doc`（标题 + 列表 + 表格 + 代码块 + 链接）：一次保存后无序标记归一 `-`、表格对齐丢失（此 fixture 未单独断言对齐）；
  CHANGED、幂等 ✅、文本 ✅。
- `structured-cell-hardbreak`（JSON）：单元格内 `第一行` + hardBreak + `第二行` → `| h |⏎| --- |⏎| 第一行[hardBreak]第二行 |`
  → 二次保存 `\[hardBreak\]`；**非幂等 ❌**、占位文本污染内容。← 6.2 顺带修复点。
- `structured-headerless-table`（JSON，无表头、两行两列）：整表回退为 `[table]` → 二次保存 `\[table\]`；
  **非幂等 ❌、文本丢失 ❌（`r1c1r1c2r2c1` 全丢）**。← 自定义序列化器修复点。
- `structured-colspan-table`（JSON，表头 h1/h2 + colspan=2 的 merged 单元格）：整表回退为 `[table]` → `\[table\]`；
  **非幂等 ❌、文本丢失 ❌（`h1h2merged` 全丢）**。← 自定义序列化器修复点。

### 修复前失败断言（29 条，摘要）

1–2. `table-align`：分隔行不带对齐标记；doc2 表格 align attr 全为 null。
3–6. `table-escaped-pipe`：二次保存不幂等、文本丢失、out1 未保留 `\|`、二轮文本缺 `x | y`。
7–10. `table-code-pipe`：二次保存不幂等、文本丢失、out1 未保留 `\|`、二轮文本缺 `a | b`。
11–15. `table-bold-escaped-pipe`：二次保存不幂等、文本丢失、out1 未保留 `**a \| b**`、二轮文本缺 `a | b`、doc2 对齐 null。
16–18. `structured-cell-hardbreak`：二次保存不幂等、out1 未含 `| 第一行 第二行 |`（仍是 `[hardBreak]`）、out1 仍含 `[hardBreak]`。
19–25. `structured-headerless-table`：out1 为 `[table]` 未含合成表头行、未含 `r1c1`/`r2c1`、二次不幂等、文本丢失、二轮文本缺 r1c1/r2c1（共 7 条）。
26–29. `structured-colspan-table`：out1 为 `[table]`、未含 `merged`、二次不幂等、文本丢失、二轮文本缺 `merged`（共 4 条）。
（另：`image` 因未安装 `@tiptap/extension-image` 只记已知限制，不计失败。）

## 4. 接受的一次归一化清单（门禁允许，只要求幂等 + 文本不丢）

1. 无序列表标记 `*` / `+` → `-`；
2. 有序列表标记 `1)` → `1.`（并按序号重编号）；
3. setext 标题（`===` / `---`）→ ATX（`#` / `##`）；
4. `~~~` 代码围栏 → ``` 围栏；
5. 软换行/行尾两空格硬换行 → 反斜杠硬换行（`\` + 换行），且统一后幂等；
6. 裸 URL → `<url>` autolink 包裹；
7. 行内 HTML（`<b>`、`<span class="x">` 等）→ HTML 实体（`&lt;…&gt;`）文本化保存；
8. 表格分隔行统一为紧凑空格形式；无对齐时为 `---`；
9. 任务列表标记统一小写（`[X]` → `[x]`）；松散列表项之间补空行；
10. 相邻有序列表合并、`_x_` → `*x*`、`***` → `---` 等由文档模型/序列化器决定的归一化（#10 原 `repro/editor.md` 已列；
    本门禁 fixture 覆盖主链路，未逐一断言这些边角）；
11. 表格单元格内硬换行 → 单个空格（GFM 表格语法无换行表达；修复后按「有后续兄弟才写空格」处理，内容保留）；
12. 无表头表格 → 保存时合成一行全空表头 + 分隔行，原内容全部作为数据行（GFM 表格必须有表头）；
13. `colspan`/`rowspan` → 扁平化为普通单元格（合并格文本保留在首格，其余补空）。

## 5. 已知限制（本门禁当前不断言 / 不在 #10 范围）

- **图片**：编辑器 schema 无 image 节点，图片整体丢失（已有 Image 扩展由 #15 提供；测试会在扩展可解析时自动升级为强断言）。
- **colspan/rowspan**：GFM 表格语法无法表达，修复后按清单 13 扁平化（文本内容保留、一次归一化，二次保存稳定）。
- **表格单元格内硬换行**：修复后归一化为空格（清单 11）。信息形态（换行）无法在 GFM 表格中表达，但内容不丢且幂等。
- **极窄边角**：行内代码/普通文本中「字面反斜杠 `\` + 竖线」组合（内容本身想表达 `\|`），管道转义按反斜杠奇偶规则补齐，
  普通文本与代码 span 均可 round-trip，但这类双反斜杠组合可能多/少一个反斜杠（极低风险，见修复后「剩余风险」）。
- **GFM 输入约束**：表格行内代码 span 中若出现裸 `|`（未写 `\|`），markdown-it 本身会把它当列分隔符（不识别 code span），
  属标准解析行为而非保存 bug；正确写法是 `` `a \| b` ``（门禁 fixture `table-code-pipe` 即此形态）。

## 6. 修复后（自定义 Table 序列化 + 编辑器接入）

实施内容：

- 新增 `src-react/src/rendering/table-alignment.ts`：`TableWithAlignment = Table.extend({ addStorage(){ return { markdown: { serialize } } } })`，
  自定义表格序列化器按「每列首个非 null 的 `align`」输出 `:---` / `:---:` / `---:` 分隔行（复用上游已内置的 `align` attr，未新增 attr）；
  单元格渲染后对 `state.out` 的片段补 `\|` 转义，并**同步平移 tiptap-markdown 待 trim 的 mark 下标**
  （其 `MarkdownSerializerState.render()` 会按记录的 start/end 做延迟空白 trim，直接插 `\` 会使下标错位、破坏粗体等 mark）；
  单元格内 hardBreak 临时替换为「写空格」（有后续兄弟时），无表头表格合成空表头行，colspan/rowspan 扁平化。
- `src-react/src/rendering/tiptap-editor.tsx`：`Table` 导入替换为 `./table-alignment` 的 `TableWithAlignment`，扩展栈内改用
  `TableWithAlignment.configure({ resizable: false })`（TableRow/TableCell/TableHeader 不变）。
- `src-react/test/md-roundtrip.test.mjs`：新增上述用例（含 `table-bold-escaped-pipe` 回归 + 3 个结构化 fixture）。
- 序列化仍只发生在 800ms 防抖保存时；未引入每键重渲染。

修复后门禁结果（同一脚本、同一 21 用例）：

```text
TOTAL=21 CHANGED=11 NOT_IDEMPOTENT=0 TEXT_LOST=0 FAILURES=0
```

| 指标 | 修复前 | 修复后 |
| --- | --- | --- |
| TOTAL | 21 | 21 |
| CHANGED | 15 | 11 |
| NOT_IDEMPOTENT | 6 | **0** |
| TEXT_LOST | 5 | **0** |
| FAILURES | 29 | **0** |

CHANGED 从 15 降到 11：`table-align`、`table-escaped-pipe`、`table-code-pipe`、`table-bold-escaped-pipe`
四个表格用例现在**原样往返**（对齐分隔行、`\|` 均保留），其余 11 个变化均为第 4 节接受的一次归一化。

修复后关键 out1：

- `table-align`：原样保留 `| :--- | :---: | ---: |`；doc1/doc2 的 `align` attr 均为 `left/center/right`；
- `table-escaped-pipe`：`| a | b |⏎| --- | --- |⏎| x \| y | z |` → 原样（幂等、文本 `x | y` 保留）；
- `table-code-pipe`：`` | `a \| b` | 含竖线 | `` → 原样（代码 span 内也补转义）；
- `table-bold-escaped-pipe`：`**a \| b**` 原样往返（幂等、文本 `a | b` 保留、对齐保留）；
- `structured-cell-hardbreak`：`| h |⏎| --- |⏎| 第一行 第二行 |`（无 `[hardBreak]`、幂等）；
- `structured-headerless-table`：`|  |  |⏎| --- | --- |⏎| r1c1 | r1c2 |⏎| r2c1 |  |`（内容保留、幂等）；
- `structured-colspan-table`：`| h1 | h2 |⏎| --- | --- |⏎| merged |  |`（合并格文本保留、幂等）。

### 剩余风险

1. **图片仍丢失**：schema 无 image 节点，需 #15 的 Image 扩展落地后门禁自动升级为强断言；在此之前保存含图片文档会丢图片（本任务不实现）。
2. **列对齐冲突**：同列不同行 align 不一致时取每列首个非 null（表头优先），属一次归一化；二次保存稳定。
3. **无表头表格 / colspan / rowspan**：按第 4 节清单 12–13 归一化；表格结构与合并信息无法在 GFM 中表达。
4. **单元格内硬换行**：归一化为空格；若未来需要保留换行，需改用 HTML 表格或扩展 GFM 表达（超出本任务）。
5. **双反斜杠 + 竖线极窄边角**：管道转义按「前导反斜杠奇偶」补齐，普通文本/代码 span 常规用例已覆盖并通过；
   极端的「字面 `\|`」内容可能多/少一个反斜杠（低风险，未入 fixture）。
6. 自定义序列化器依赖 tiptap-markdown 内部状态（`state.out` / `inlines` / `nodes`）与上游 `align` attr；升级 tiptap-markdown /
   @tiptap/extension-table 时需重跑本门禁（脚本已内建，`node src-react/test/md-roundtrip.test.mjs`）。

验证命令：`node src-react/test/md-roundtrip.test.mjs`（全绿）+ `cd src-react && npx tsc --noEmit -p tsconfig.app.json`（0 error）。