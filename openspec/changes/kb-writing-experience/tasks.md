# Tasks

> **实施状态（2026-11-16）**：代码已全部落盘，静态与自动化证据见各项；除 1.x / 3.1 / 7.1 / 7.3 外，
> 其余条目的「验证」子句要求真机操作（截图 / 控制台 / WebView2 渲染），本轮无法执行，
> 故保持未勾选。真机验收统一在 7.4 一次过完后再逐条打勾——不要把「代码写完」当成「已验证」。

## 1. 阅读偏好：后端字段与校验

- [x] 1.1 在 `src-tauri/src/models.rs` 的 `Settings` 加 `reading_size: u8`、`reading_tracking: u8`、`reading_measure: String` 三个字段（`#[serde(default)]` 沿用结构体级 `rename_all = "camelCase"`），默认值 `16` / `0` / `comfortable`；验证：`cargo check` 通过，且用一份不含这三个字段的旧 `data.json` 副本启动后字段被补成默认值
- [x] 1.2 加 `normalize_reading_size` / `normalize_reading_tracking` / `normalize_reading_measure` 三个白名单函数（字号只认 `8/10/12/14/16/18/20`，字距索引只认 `0..=6`，列宽只认 `comfortable`/`wide`），在 `coerce()` 里兜底，并在 `src-tauri/src/store.rs::apply_settings_patch` 挂上；验证：`cargo test` 新增三条越界用例（15px、7、`wide-ish`）全部回落到默认值，旧用例不红
- [x] 1.3 同步 `SettingsPayload`（`models.rs:769-787`）与 `src-react/src/lib/api.ts` 的 Settings 类型与 patch 可选字段；验证：`npx tsc -b` 无错，设置页读到 `settings.readingSize` 不出现 `undefined`
- [x] 1.4 系统字体白名单 4→6：`models.rs:498-503` 加 `("Noto Serif SC", "思源宋体")`、`("Noto Sans SC", "思源黑体")`，`src-react/src/lib/reading-font.ts:13` 的 `READING_FONT_SYSTEMS` 同步；验证：`cargo test` 里 `normalize_reading_font("system:Noto Serif SC")` 返回 Ok，`system:Comic Sans` 仍 Err

## 2. 阅读偏好：前端属性与 CSS 档位

- [ ] 2.1 新增 `src-react/src/lib/reading-prefs.ts`，导出幂等的 `applyReadingPrefs(settings)`，一次写 `data-reading-measure` / `data-reading-size` / `data-reading-tracking` 三个属性；验证：`npx tsc -b` 通过，连续调用两次后 `document.documentElement.dataset` 上三个值不变（真机控制台确认）
- [ ] 2.2 在 `src-react/src/index.css` 落 `--link` / `--ls-reading` 令牌与 `--kb-measure-cap` 默认值，在 `src-react/src/views/kb.css` 写七条字号规则、七条字距规则、`.kb-detail-inner { width: min(100%, var(--kb-measure-cap)) }`，并删掉 `kb.css:293-298` 里的 `--kb-measure: 100%`（只留 gutter 意图）；验证：真机把字号切到 8px / 20px 各截一张，标题与正文同比缩放、便签窗标题不变
- [ ] 2.3 在 `src-react/src/views/SettingsView.tsx` 加三组控件（字号七钮、字距七钮、列宽两钮），沿用现有 `.seg` / `.btn.sm.on` 写法，样式补进 `src-react/src/views/settings-font.css`；验证：真机连点七个字号档，每一次都在不重载的情况下立即改变正文显示
- [ ] 2.4 在 `src-react/src/App.tsx:88-101` 的 `refresh()` 里，与 `applyReadingFont` 并列调用 `applyReadingPrefs`，并确认 `store-changed` 重放路径也走到；验证：开两个窗口，在 A 窗改字号，B 窗在一秒内跟随变化

## 3. 链接：可见性与令牌

- [x] 3.1 定 `--link` 令牌亮/暗两值（亮 `214 70% 40%`、暗 `210 75% 68%`），逐一算对比度；验证：对 `--paper` 背景实测 ≥ 4.5:1，把两个数值与算式写进交付说明（结果：亮 6.03:1 / 白卡 6.53:1，暗 7.50:1，算式见 `src-react/src/index.css` 注释）
- [ ] 3.2 在 `src-react/src/rendering/styles/prosemirror.css` 补 `.kb-editor-content a`（链接色 + 下划线 + `text-underline-offset`）并与 `.kb-md-render a`（`:375-382`）统一取 `--link`；验证：深浅两套主题下各截一张，编辑区与阅读态链接颜色一致且可辨，正文颜色不受影响

## 4. 链接：`[@文本](url)` 语法

- [ ] 4.1 新增语法扩展（建议 `src-react/src/rendering/extensions/link-syntax.ts`）：InputRule 在键入 `)` 时匹配 `[@文本](url)`，先过协议白名单（http/https/mailto），通过则替换为挂 `link` mark 的纯文本、可见文字剥掉 `@`；验证：真机键入 `[@示例](https://example.com)` 立刻变成链接，键入 `[@点我](javascript:alert(1))` 保持原文且不可点
- [ ] 4.2 补 `editorProps.handlePaste`，剪贴板内容整段匹配同一正则时才转换；验证：粘贴单条 `[@示例](https://a.b)` 生成链接，粘贴含该语法的三行长文不改动
- [ ] 4.3 实现保存前归一化：仅处理非代码区域的完整匹配且协议合法，行内代码与代码块内跳过；验证：造一份含 `` `[@x](y)` `` 与代码块内 `[@x](y)` 的条目，保存后重新打开，两份文本原样保留，而普通段落里的 `[@x](y)` 变成链接，落盘文件 grep 不到 `[@`
- [ ] 4.4 `Ctrl+K`、工具栏链接按钮、斜杠菜单 `/link` 统一走同一命令：插入 `[@文本](url)` 并把「文本」选中；`SlashItem` 加可选 `dialog?: 'link'` 分派（`applySlashItem` 是模块级函数拿不到 React 状态），并让便签窗编辑器也能命中该项；验证：知识库条目与知识便签各试一次 `Ctrl+K` 与 `/link`，插入后直接键入即替换占位文本

## 5. 链接：内联编辑条替换弹窗（BREAKING）

- [ ] 5.1 新增 `src-react/src/rendering/link-inline-editor.tsx`：点击已有链接时用 `coordsAtPos` 定位到链接下方，输入条预填 href，Enter 确认、Esc 取消、清空确认即移除链接、点条外关闭；验证：真机改一条链接地址，正文不发生滚动跳动，输入条不遮住被编辑行
- [ ] 5.2 删除 `src-react/src/rendering/link-dialog.tsx`、`prosemirror.css:439-534` 的 `.kb-link-dialog-*` 全部样式、`tiptap-editor.tsx` 的 `linkOpen/linkUrl/linkCanApply` 状态与四个处理函数及 `:1203-1210` 挂载点；把 `src-react/src/sticky/StickyWindow.tsx:47` 的 `.kb-link-dialog-overlay` 换成 `.kb-link-inline`，`src-react/src/components/kb/kb-folder-dialog.tsx:21`、`:85` 两处注释改指内联条；验证：全仓 `rg "link-dialog|LinkDialog"` 只剩历史 openspec 文档，且便签窗编辑时鼠标移到正文不会触发窗口淡化
- [ ] 5.3 `StarterKit.configure({ codeBlock: false, link: { openOnClick: false } })`，并加 `editorProps.handleClick`：仅 `ctrlKey || metaKey` 且协议在白名单时才 `window.__TAURI__.shell.open`；验证：普通点击链接只落光标不跳转，`Ctrl+点击` 在系统浏览器打开，应用窗口停留原页

## 6. 表格网格选择器外部关闭

- [ ] 6.1 `gridOpen` 为真时挂 `document` 的 `mousedown`（capture）与 `keydown`（Esc），命中 `.kb-editor-gridwrap` 之外即关闭（照抄 `kb-context-menu.tsx:298` 的 closeIfOutside 写法），并把 `.kb-grid-picker` 的 `top: 30px` 收到 `26px` 或给 wrap 补 4px 底部内边距；验证：点开网格后移到正文点一下，选择器关闭；从图标移到面板途中不误关；Esc 关闭后光标回原位

## 7. 文档、门禁与真机验收

- [x] 7.1 改 `docs/capability/06-知识库.md` 五处（`:42` 78ch 表格、`:49` 宽窗封顶、`:50` 标准窗量算、`:53` 量算口径、`:103` 已实现清单）、`README.md:138` 的字号五档表述、`AGENTS.md:34` 的 16px 断言；验证：`node test/docs.test.mjs` 通过（实测 `# pass 4 | # fail 0`），且按 AGENTS 4.6 把改动内容先交人工审核再提交
- [x] 7.2 跑 `npm run check`（17 项）与 `cargo test`；验证：贴出真实条数与结果，SKIP 项单独列名，不以 16/17 冒充全绿（实测：`cargo test` 214 passed / 0 failed / 1 ignored；`npm run check` 本身因 `scripts/check.ps1` 在 PowerShell 5.1 下按 ANSI 读 UTF-8 无 BOM 而解析失败，改为逐项直跑：version OK、command-parity 6、packaging 8、docs 4、md-fidelity 14、main-window-close 5、tray-menu 5、heading-select-wiring 6、tray-entry-parity 5、sticky-kbref-seam 4、sticky-text / sticky-filter / sticky-fade-guard / heading-select 均 exit 0、md-roundtrip 通过、`npx tsc -b` exit 0、`npm run lint` 0 errors / 31 warnings）
- [x] 7.3 手工补 `md-roundtrip` 的缺位：造一份含链接、表格、代码块、图片的条目，存盘后重开并逐项比对；验证：把"改前/改后"两段 Markdown 差异贴进交付说明，确认无新增漂移（实测 `node src-react/test/md-roundtrip.test.mjs` → `TOTAL=22 CHANGED=10 NOT_IDEMPOTENT=0 TEXT_LOST=0 FAILURES=0`，二次保存幂等）
- [ ] 7.4 L4 真机五条冒烟 + 本次新增路径（字号/字距/列宽三组切换、链接三种输入路径、内联条、网格选择器关闭、思源字体选中后的实际字形）；验证：截图逐条留证，思源必须看到字形变化而非只看"保存成功"

## Workflow follow-up

- 归档前先解决"主 specs 为空"：等 `kb-file-tree`、`fix-known-defects-iteration`、`refine-frontend-visual-language` 归档后再归档本 change，或先跑 `openspec sync-specs` 把上游 delta 刷进主 specs（两者择一，不影响实现）。
- 归档后回归 `openspec validate --strict` 与 `openspec list --specs`，确认三条能力已进主 specs。