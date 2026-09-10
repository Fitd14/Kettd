# kb 编辑器打磨 · 设计验收报告（QA 第二轮）

- **验收目标**：editor-polish-spec 实施（HEAD = 23e6d31）对照 `spark-output/design/kb-editor-polish-spec.md` + `spark-output/context/board.json`（朱砂宣）+ 上轮 audit 12 findings
- **验收时间**：2026-09-10
- **验收模式**：模式 A 自动比对（7 组 grep/类型定义取证）

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 0 |
| 🟠 Major | 2 |
| 🟡 Minor | 6 |

**通过维度**（零偏差）：间距 · 颜色 · 圆角阴影 · 图标与图片 · 响应式
**Audit finding 核对**：11 ✅ 解决 + 1 ⚠️ 部分（N02 表格：创建/Tab 跳格 ✓，表内增删 UI ✗）

## Deviations

### 🟠 Major

**qaB-1 ｜交互态｜表内增删行列/表头开关 UI 未实现（audit N02 部分解决）**
- 设计源：polish-spec §2「表格内点击出现气泡菜单（+行 −行 +列 −列 表头开关）」
- 实现：shouldShow 在 `isActive('table')` 时返回 false（tiptap-editor.tsx:411）显式排除表格；全文件无 insertRow/addColumn/updateAttributes 调用
- 差异：用户能创建表格但**无法通过 UI 加行加列**（仅末格 Tab 补行）；spec 承诺的表内气泡缺席
- 修复建议：BubbleMenu 增加第二实例（shouldShow 仅在 table 内），按钮接 `deleteRow/addRow/deleteColumn/addColumn/toggleHeaderRow` 命令
- 位置：src-react/src/rendering/tiptap-editor.tsx:405-416

**qaB-2 ｜交互态｜BubbleMenu 缺 `editor` prop——选区气泡运行时不显示**
- 设计源：spec §3 选区浮动工具栏（选中文字弹 B/I/行内码/删除线）
- 实现：`<BubbleMenu options shouldShow>` 未传 `editor`（tiptap-editor.tsx:406-408）；v3 类型里 editor 可选＝期望 EditorContext 供体，应用无 Provider → 插件拿不到 editor 实例
- 差异：TS 通过但**运行时气泡大概率永不出现**（真机必挂项）
- 修复建议：补 `editor={editor}` 一行
- 位置：src-react/src/rendering/tiptap-editor.tsx:406

### 🟡 Minor

**qaB-3 ｜状态完整性｜条目切换不重挂，保存指示串台**
- `<KbItemDetail item={selected}>` 无 `key`：切换条目组件复用，saveState（已保存 HH:MM）显示上一条的时点，直至下次编辑
- 修复：`<KbItemDetail key={selected.id} …>`｜位置：KbView.tsx:262

**qaB-4 ｜可访问性｜暖墨三级字色小字对比不足**
- ink-3 `#A59089` on `#FAF6EE` ≈ 2.8:1，用于 11px 保存指示/分隔说明（< 4.5:1 标准）
- 建议：小字场景用 ink-2 `#7A655F`（≈4.9:1），ink-3 仅作装饰｜位置：kb.css save-pill/kb-detail 相关规则

**qaB-5 ｜一致性｜网格选择器/slash 面板底色用全局白 `--card`，未随纸面暖色**
- 纸面上弹出白色面板色温断裂｜建议：底色改 `var(--paper, hsl(var(--card)))`｜位置：prosemirror.css .kb-grid-picker/.kb-slash

**qaB-6 ｜字体｜指定字体未随包分发**
- board typography 指定 Noto Serif SC/LXGW WenKai，实现仅 font-family 声明无 @font-face；Windows 无安装时回退宋体，纸感打折
- 建议：思源宋体子集化 webfont（常用 3500 字 + 界面字）或接受回退并在台账销账｜位置：src-react/src/index.css

**qaB-7 ｜交互态｜链接插入用 window.prompt**
- spec 选型为内联输入（原型无 prompt）；prompt 阻塞且风格断层｜建议：二期换行内小浮层｜位置：tiptap-editor.tsx 链接按钮 onClick

**qaB-8 ｜状态完整性｜编辑器→Markdown 序列化方向无自动化保真测试**
- md-fidelity 只覆盖 markdown-it 渲染向；tiptap-markdown 的表格/任务清单序列化正确性目前只能真机验证
- 建议：引入 jsdom 级编辑器序列化测试（成本高）或真机清单固定表格往返用例｜位置：src-react/test/md-fidelity.test.mjs

## Audit Finding 核对（N01-N12）

| Audit | 状态 |
| --- | --- |
| N01 模式割裂 / N09 模式按钮 / N11 误触 / N12 重挂载 | ✅ 已解决（去模式化一并消灭） |
| N02 表格能力 | ⚠️ 部分解决（创建/Tab/高亮 ✓；表内增删 UI ✗ → qaB-1） |
| N03 标题就地编辑 / N04 标签增删 | ✅ |
| N05 保存反馈 | ✅ |
| N06 提示体系 / N10 快捷键提示 | ✅（slash+气泡+placeholder+kbd） |
| N07 阅读排版 | ✅（阶梯/76ch/代码角标/表格样式） |
| N08 lucide 图标统一 | ✅ |

## 修复优先级建议

- **必须修复**：qaB-2（一行修复、真机必挂）、qaB-1（spec 核心承诺）—— 2 项
- **建议修复**：qaB-3/4/5（各 <10min）—— 3 项
- **可延后**：qaB-6（字体子集化需决策）、qaB-7、qaB-8 —— 3 项
