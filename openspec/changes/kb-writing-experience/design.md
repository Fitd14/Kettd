# Design

## Context

写作面全部落在 `.kb-detail` 里：`src-react/src/views/kb.css:222-263` 定义 `--kb-measure: 78ch` 与 `--kb-gutter: clamp(20px, 4vw, 64px)`，`.kb-detail-inner`（`kb.css:281-289`）统一限宽，标题/标签/工具栏/正文同列。字号与字距全走令牌：`--fs-reading: 16px`（`src-react/src/index.css:50`），正文唯一 `font-size` 在 `prosemirror.css:151`，H1–H6 用 `calc(var(--fs-reading) * k)`（`prosemirror.css:152-161`，渲染态 `:320-325` 同构）。

两条约束决定了实现形态：

1. `prosemirror.css:159-161` 的 H4–H6 规则**没有 `.kb-detail` 前缀**（知识库与便签窗共用同一套标题样式）。任何在 `documentElement` 上覆写 `--fs-reading` 的做法都会把便签窗标题一起放大，与 「便签窗不受影响」 冲突。
2. 链接由 StarterKit 3.31.3 内建的 Link 扩展提供，`openOnClick` 默认 `true`，其实现是 clickHandler 里直接 `window.open(href, target)`；而 `.kb-editor-content a` 在全库没有任何样式规则，链接颜色被 `node_modules/tailwindcss/preflight.css:92-96` 的 `a { color: inherit; text-decoration: inherit }` 重置成正文色。这两件事叠加，就是"链接看起来不像链接"的全部原因。

外链不需要新命令：`src-tauri/tauri.conf.json` 已开 `build.withGlobalTauri` 与 `allowlist.all`，`window.__TAURI__.shell.open` 现成可用，命令面四方同步与 parity 门禁不参与本次变更。

设置链路已有成熟范式可抄：`models.rs` 的 `Settings` + `store.rs::apply_settings_patch` 的区间校验（`sticky_fade_opacity` 在 `store.rs:2078-2082`）+ `coerce()` 兜底 + `App.tsx:88-101` 在 `refresh()` 里重放。

表格网格选择器见 `tiptap-editor.tsx:1059-1079`：`gridOpen` 只由按钮自身取反，面板 `top: 30px` 与 24px 高的按钮之间留了 6px 死区（`prosemirror.css:19-22`、`:126`）。

## Goals / Non-Goals

**Goals:**

- 三个阅读偏好（列宽上限 / 字号 / 字距）各有唯一存储字段、唯一 CSS 入口、单一应用函数；改档即时生效，不重启、不重载。
- 链接在编辑态、阅读态、保存后的 Markdown 里表现一致，不产生漂移。
- 移除弹窗之后仍能修改已有链接的地址。
- 便签窗的排版、标题层级不受知识库阅读档位影响。

**Non-Goals:**

- 不动 Tauri 命令面，不新增命令，不改 `docs/ARCHITECTURE.md` 附录 §2。
- 不做分栏，不做"满宽无上限"档（两档即本次范围）。
- 不为新增设置字段写数据迁移脚本（旧数据靠 serde default 直接读）。
- 不修 `md-roundtrip` 门禁缺 `jsdom` 的老问题（另案），但本次要在验收里用人工往返补证。

## Decisions

### D1 列宽用 `min(100%, 上限)`，不加比例项、不用容器查询

上一版设想是 `min(100% − 2×gutter, max(78ch, 62cqw), 96ch)`。把它代进真实面板宽度就会发现 `62cqw` 项只在约 1000–1280px 面板区间可观测：低于 1000px 被下限压住，高于 1280px 被上限压住，两端行为与"直接取上限"完全一致。`100%` 本身已经跟着详情面板走（树折叠、侧栏收起都会改变它），容器查询在这里不提供任何额外信息。

**备选**：容器查询 + 比例项（否决：复杂度换来一段几乎不可观测的动态区）；媒体查询阶梯（否决：只看窗口宽度，树折叠时刻度失效）；JS 监听 `resize` 写内联宽度（否决：多一次重渲染，首帧会闪）。

### D2 两档上限 96ch / 112ch，属性驱动，默认舒适

```css
:root { --kb-measure-cap: 96ch; }
:root[data-reading-measure='wide'] { --kb-measure-cap: 112ch; }
.kb-detail-inner { width: min(100%, var(--kb-measure-cap)); margin-inline: auto; }
```

`kb.css:293-298` 那条 `@media (max-width: 720px)` 只保留 `--kb-gutter: 20px` 的意图，`--kb-measure: 100%` 一行删掉——窄窗本来就由 `100%` 兜住。

**备选**：把 78ch 保留为第三档（否决：78ch 正是被抱怨的那一档，留着只会让默认值失去意义）。

### D3 字号用属性 + 局部规则，不覆写全局变量

```css
:root { --fs-reading: 16px; }
:root[data-reading-size='18'] .kb-detail { --fs-reading: 18px; }
```

七档七条规则，全部作用在 `.kb-detail` 子树内。`.kb-detail-title`（`kb.css:326-330`）用 `var(--fs-reading)`，自动跟随；H1–H6 的 `calc()` 也自动等比。便签窗因为不在 `.kb-detail` 里，完全不受影响。

**备选**：`documentElement.style.setProperty('--fs-reading', …)`（否决：见 Context 约束 1，会污染便签窗）；组件内联 `style`（否决：`KbView.tsx` 拿不到 settings，全文件 grep 无 bootstrap 访问）。

### D4 字距用 `em` 令牌 + 属性

```css
:root { --ls-reading: 0; }
:root[data-reading-tracking='3'] { --ls-reading: 0.045em; }
.kb-detail .kb-editor-content { letter-spacing: var(--ls-reading); }
```

`em` 相对元素自身字号解析，字号档位一变字距自动等比，不需要 JS 参与。标题自带的 `letter-spacing: .4px / .3px`（`prosemirror.css:152`、`:157`）写在更具体的选择器上，不受影响。

**备选**：px 档（否决：8px 档和 20px 档用同一个 px 值，比例会失控）。

### D5 三个偏好合并成 `src-react/src/lib/reading-prefs.ts` 一个模块

一个 `applyReadingPrefs(settings)`，一次写 `data-reading-measure` / `data-reading-size` / `data-reading-tracking` 三个属性，幂等。`App.tsx` 在 `applyReadingFont` 旁边调用，`store-changed` 时随整条 `refresh()` 重放。

**理由**：三个独立 `useEffect` 会互相覆盖属性写入时机，且单测要起三个；一个函数一次断言三个属性更省。

### D6 三个设置字段，白名单 + 双保险兜底

| 字段 | 类型 | 取值 | 默认 |
| --- | --- | --- | --- |
| `reading_size` | 整数 | `8/10/12/14/16/18/20` | `16` |
| `reading_tracking` | 整数 | `0..=6`（档位索引） | `0` |
| `reading_measure` | 字符串 | `comfortable` / `wide` | `comfortable` |

后端 `normalize_reading_*` 做白名单，`coerce()` 再做一次（仿 `models.rs:368-369` 的 `sticky_fade_opacity` 写法），`apply_settings_patch` 里挂上。前端 `api.ts` 的类型同步加三个可选字段。

**备选**：存 px / em 字面量（否决：前端要解析字符串，浮点序列化还会漂移；索引/枚举更稳）。

### D7 链接语法用 InputRule + handlePaste，`@` 在转换瞬间剥离

- InputRule 正则 `\[@([^\]\n]+)\]\(([^)\s]+)\)$`，在键入 `)` 时触发：把整段替换成一个普通文本节点，挂 `link` mark，可见文字为捕获组 1（不含 `@`）。
- 转换前先过协议白名单（`http` / `https` / `mailto`），不通过就保持原文，绝不生成 mark——对应「危险协议不成为链接」。
- 粘贴走 `editorProps.handlePaste`，同一正则，但要求整段剪贴板内容匹配（避免把含语法的长文误转）。
- 序列化天然是 `[文本](url)`：`target` / `rel` 属于 `HTMLAttributes`，不进 mark attrs，不产生漂移。

**备选**：在文档里保留 `[@]()` 原文、渲染期再解析（否决：编辑器是富文本模型，正文得是真链接）；自定义 Node 类型（否决：序列化与块级规则都要重写，收益为零）。

### D8 「保存前归一化」的边界（补查漏项 4）

文档里若残留未被 InputRule 命中的 `[@文本](url)`，保存前按链接归一化，落盘不得出现 `[@`。边界写死三条：

1. 只处理段落、标题、列表项等文本块里的**完整**匹配，且 url 通过协议白名单；
2. 行内代码与代码块内一律不处理（这就是"想字面展示语法"的逃生口）；
3. 归一化等同于"插入一个链接"，不改变其余文本。

**代价**：既有文档里如果真有字面量 `[@x](y)`，会被改写。可接受——这种写法在编辑器里本来就没有显示价值；需要展示时用行内代码。

### D9 内联输入条替代 `LinkDialog`（BREAKING）

新组件 `src-react/src/rendering/link-inline-editor.tsx`：点击已有链接时，用 `view.coordsAtPos` 定位到链接下方一条 `contenteditable=false` 的输入条，预填当前 href，Enter 确认 / Esc 取消 / 清空确认即移除链接。非模态、不遮正文、点条外关闭。

删除面：`src-react/src/rendering/link-dialog.tsx`、`prosemirror.css:439-534` 的 `.kb-link-dialog-*` 样式、`tiptap-editor.tsx` 里的 `linkOpen/linkUrl/linkCanApply` 状态与 `openLinkDialog/closeLinkDialog/applyLink/removeLink`、`:1203-1210` 的挂载点。

连带更新：`StickyWindow.tsx:47` 的 `OVERLAY_SELECTOR` 把 `.kb-link-dialog-overlay` 换成 `.kb-link-inline`（补查漏项 5）；`kb-folder-dialog.tsx:21`、`:85` 两处「沿用 LinkDialog」的注释改指内联条的焦点循环写法。

**备选**：保留弹窗但改成非阻塞浮层（否决：弹窗要求用户离开正文填地址，与"语法优先"的定位冲突）。

### D10 `Ctrl+K` 与工具栏按钮改为插入语法模板（补查漏项 6）

插入 `[@文本](url)`，并把「文本」三段选中，用户直接键入替换。斜杠菜单 `/link` 走同一条命令（`SlashItem` 加可选 `dialog?: 'link'` 分派，因为 `applySlashItem` 是模块级函数、拿不到 React 状态）。

### D11 外链点击：`openOnClick: false` + `handleClick` 判修饰键

```ts
StarterKit.configure({ codeBlock: false, link: { openOnClick: false } })
editorProps: { handleClick(view, pos, event) { /* ctrlKey||metaKey 且协议白名单 → shell.open */ } }
```

普通点击交还 ProseMirror，光标落进链接；只有修饰键才调 `window.__TAURI__.shell.open`。`autolink` 与 `linkOnPaste` 保持默认（粘贴裸 URL 仍自动成链，不影响语法入口）。

### D12 网格选择器：外部点击 + `Esc` 关闭，顺手补 6px 死区

`gridOpen` 为真时挂 `document` 的 `mousedown`（capture）与 `keydown`（Esc），命中 `.kb-editor-gridwrap` 之外就 `setGridOpen(false)`；写法照抄 `kb-context-menu.tsx:298` 的 `closeIfOutside`。同时把 `.kb-grid-picker` 的 `top: 30px` 收到 `26px`（或给 wrap 加 4px padding-bottom），消掉按钮到面板的死区——否则"从图标移到面板"会先被判成外部点击。

**理由**：用户原话是"移出消失"，实际诉求是"不要逼我回到图标再点一次"。

### D13 系统字体白名单 4 → 6

`models.rs:498-503` 加 `("Noto Serif SC", "思源宋体")`、`("Noto Sans SC", "思源黑体")`；`reading-font.ts:13` 的 `READING_FONT_SYSTEMS` 同步（文件里已注明两处必须一致）；`normalize_reading_font` 的测试补正向一条。

**注意**：白名单校验的是 key，前端把 key 直接当 CSS 族名用，所以族名必须与系统注册名逐字一致。本机字体文件是 `NotoSerifSC-VF.ttf`，注册族名是 `Noto Serif SC`——验收时必须实拍选中后的字形，不能只看"保存成功"。

### D14 文档同步（补查漏项 8–10）

`docs/capability/06-知识库.md` 五处（`:42` 的 78ch 表格、`:49` 宽窗封顶、`:50` 标准窗量算、`:53` 量算口径、`:103` 已实现清单）、`README.md:138`（"字号五档 … `--fs-reading` 16px"）、`AGENTS.md:34`（"编辑器正文只许 16px；禁止新增字号值"）。不改这三处，下一个 agent 会照着把七档改回去。

`refine-frontend-visual-language/design.md:68`、`:84` 属历史档案，不回改，在交付说明里注明"已过时"。

## Risks / Trade-offs

- **[默认变宽]** 默认从 78ch 提到 96ch，老用户首次升级就会看到更宽的行，且没有回到 78ch 的档位 → 若反馈强烈，补一档只是多一条属性映射，不动数据模型与校验。
- **[8 / 10px 档层级倒挂]** 正文会小于 chrome 的 11 / 12.5px 标签 → 有意接受（档位是用户点选的），spec 已写明"这是档位两端的有意行为"，不做静默钳制。
- **[字面量被改写]** D8 的归一化会把真正想展示 `[@x](y)` 的段落变成链接 → 逃生口是行内代码；验收时真机试一次。
- **[序列化一侧缺自动门禁]** `md-roundtrip` 长期 SKIP（缺 `jsdom`）→ 本次改动正好压在序列化上，必须手工跑一次"存盘 → 重开 → 比对"的往返，并把结果写进交付说明，不能用 `npm run check` 的 16 项冒充覆盖。
- **[便签窗被污染]** H4–H6 无前缀，任何全局字号覆盖都会放大便签标题 → D3 的属性 + 局部选择器就是为此；验收要实拍便签标题。
- **[ch 与字距的关系]** `ch` 按元素自身 0 号字宽解析，字距加在 `.kb-editor-content` 上、`96ch` 算在 `.kb-detail-inner` 上，两者不在同一元素 → 预期列宽像素不变；验收实测一次，若发现 ch 含字距，就把上限改用 px 常量并记进 spec。
- **[内联条定位]** `coordsAtPos` 给的是视口坐标，滚动容器里会偏 → 用相对 `.kb-detail` 的 `getBoundingClientRect` 差值定位，滚动时重算或直接关闭。

## Migration Plan

1. **后端先行**：三个字段 + 白名单 + `coerce` + 单测。旧 `data.json` 无字段走 serde `default`，读旧数据无损，无需迁移脚本。
2. **前端偏好层**：`reading-prefs.ts` + 七档 CSS + 设置页控件；默认值与后端保持一致。
3. **链接三步走**：先上 `--link` 令牌与链接样式（可独立验收），再上语法扩展与 `Ctrl+K`/`/link`，最后删 `LinkDialog`（BREAKING，放最后，回滚只需 revert 该提交）。
4. **网格选择器关闭**：独立提交，与链接无耦合。
5. **字体白名单**：前后端同一提交 + 单测。
6. **文档同步**：随最后一提交，按 AGENTS 4.6 先给内容待人工审核。

**回滚**：每步一个提交；删弹窗那步的 revert 同时恢复 `link-dialog.tsx` 与 `prosemirror.css:439-534`；设置字段回滚只影响读旧数据（多余字段被 serde 忽略）。

## Open Questions

- 主 specs 目录为空，三份 MODIFIED delta 在归档时会被拒（`only ADDED requirements are allowed for new specs`）。两条出路：等 `kb-file-tree` / `fix-known-defects-iteration` / `refine-frontend-visual-language` 归档后再归档本 change，或先 `openspec sync-specs` 把上游 delta 刷进主 specs。不影响实现，只影响归档时机，可留到 tasks 收尾前再定。