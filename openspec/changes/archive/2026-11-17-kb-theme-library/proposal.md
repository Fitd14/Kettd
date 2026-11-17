# Proposal

## Why

知识库写作区目前只有一套固定的纸面与字族，想要换观感只能改代码；同类工具（Typora 等）的核心体验就是「一个 CSS 文件换一整套纸面与排版」。用户明确要求：能像安装主题一样导入自己的 CSS 主题包，并附带可参考的示例主题；在看过 Paperglow 与 Mdmdt 两套真实主题后，用户进一步要求「全量 Typora 化」——不只换纸色与字族，还要能改圆角、投影、引用、代码、表格斑马纹、链接与标题字距。主题落地后用户又实测提出「要求全量修改，不仅仅是知识库」：换肤必须覆盖整个应用与四个窗口，而不是只换阅读区。把主题做成「文件约定 + 导入入口 + 内置示例」后，换皮肤不再需要发版，用户也能把自己调好的样式带走。

## What Changes

- 新增**外观主题库**：一套可安装、可切换、可删除的 CSS 主题包，作用范围是**整个应用**——外壳配色（背景、卡片、弹层、主色、次级色、静音色、边框、输入、聚焦环、链接、印章色）与知识库阅读区观感（纸面、墨色、圆角、投影、纸纹、字族、行高、标题、引用、代码、表格、链接、图片）；主窗、快速捕获、便签、待办悬浮四个窗口一致生效。
- 定义主题文件约定：单个 `.css` 文件，头部注释声明主题名与作者，正文用限定的 **59 个**自定义属性给值（十一组，见下：外壳 19 + 阅读 40），外壳颜色必须写成不带 `hsl()` 的 HSL 三元组；同一文件可同时给出**亮/暗两套值**，跟随应用明暗自动切换；越界值逐条丢弃并计数，不整份拒收。
- 白名单边界：`--paper-deep` 不受理（有声明无消费者），字号、正文字距、列宽与留白令牌不受理，语义色（`--cat-*` / `--pri-*` / `--destructive`）与捕获气泡固定深底（`--bubble-*`）不受理；主题不改列宽、留白与滚动结构。
- 令牌落点与默认值：主题值写到 `document.documentElement` 的内联自定义属性；`--paper` / `--ink-*` / `--kb-paper-*` 的默认声明从 `.kb-detail` 上移到 `:root` / `.dark`，使未选主题时逐像素不变、选中后整树继承。
- 设置页新增「阅读主题」区：列出「默认（跟随现有外观）」+ 内置主题 + 已安装主题 + 导入/删除按钮，选中即预览生效；导入复用现有上传字体的文件选择、大小限制与错误文案规范。
- 内置 2 个示例主题，每个都自带亮/暗两套取值、覆盖全部 59 个变量，既是开箱可用的选择，也是主题文件写法的活文档：
  - `paperglow`：暖纸面、陶土强调、编辑部排版、16px 圆角纸面与轻投影、暖底圆角引用、16px 圆角代码块；按 cloudy-liu/Paperglow-Theme（Apache-2.0）重写为 Kettd 令牌；行高 1.7，标题倍率 1.90 / 1.65 / 1.42 / 1.24 / 1.12 / 1.00。
  - `mdmdt`：冷白纸、皇家蓝 `#3E69D7`、h1 底边、标题字距 `0.06em`、蓝底圆角行内代码、斑马表格、扁平纸面；按 cayxc/Mdmdt（Apache-2.0）重写；行高按行盒等价折算为 1.7，标题倍率 2.00 / 1.75 / 1.50 / 1.25 / 1.12 / 1.00。
  - 两者均不整份复制上游 CSS，文件头注释写明来源、许可与本地改动（行高换算、暗色强调色提亮）。
- 安全边界：主题只允许改白名单内的自定义属性，加载前校验文件大小、字符集与禁用规则（`@import`、远程 `url()`、脚本类声明）；不合格文件拒收并给出中文短句，不静默降级。
- 与既有档位的关系：主题提供配色、字族、行高、标题（倍率/字距/底线）与元素装饰，阅读字号、正文字距、列宽仍由用户档位（`reading-prefs`）决定，主题不得覆盖；标题字距与用户字距档位叠加，不替换。

## Capabilities

### New Capabilities

- `knowledge-base/reading-themes`: 外观主题库的文件约定、安装/切换/删除流程、亮暗双套取值、全应用作用域与四窗一致的边界，以及加载安全校验与失败回退行为。

### Modified Capabilities

- `visual-language`: 应用外观从「只由内置 token 决定」放宽为「默认由 token 决定，可由已安装主题以同名变量覆盖」——外壳 19 个语义令牌与阅读面 40 个变量都可被主题赋值；主题另可改标题字距与正文元素装饰；字号四档、阅读字号/正文字距/列宽的档位契约不变，语义色不可覆盖。
- `knowledge-base/writing-layout`: 划清主题与布局的边界——主题可改阅读区装饰几何（纸面圆角与投影、引用/代码/表格/链接/图片），但列宽、居中、留白与滚动结构仍归布局。

## Impact

- 前端：`src-react/src/lib/reading-theme.js`（主题解析与应用）、`src-react/src/App.tsx`（主窗统一应用与事件监听）、`src-react/src/capture/CaptureWindow.tsx` / `src-react/src/sticky/StickyWindow.tsx` / `src-react/src/todo/TodoFloatWindow.tsx`（三窗各自应用）、`src-react/src/index.css`（外壳令牌默认值）、`src-react/src/views/kb.css`（默认令牌上移 `:root`、纸面圆角/投影/纸纹）、`src-react/src/views/SettingsView.tsx`（阅读主题区）、`src-react/src/rendering/styles/prosemirror.css`（引用、代码、表格、链接、图片、行高与标题接线）、`src-react/src/assets/themes/paperglow.css` 与 `mdmdt.css`（内置主题资源，经 `?raw` 打包进前端）。
- 后端：`src-tauri/src/models.rs`（`Settings.reading_theme` 字段、`READING_THEME_BUILTINS` 与校验）、`src-tauri/src/commands.rs`（`list_reading_themes` / `save_reading_theme` / `delete_reading_theme` / `read_reading_theme` 四条命令，参照 `reading-font` 的落盘与校验模式）、`src-tauri/src/infra/themes.rs`（新，数据目录 `themes/` 的安装/列举/删除）。
- 契约：命令四条新增加入 `main.rs` 注册、`api.ts` 封装、`docs/ARCHITECTURE.md` 附录 §2 + §10 计数，parity 门禁同步。
- 数据：主题文件存于 `<数据目录>/themes/`，`settings.readingTheme` 只存 `""` / `builtin:<id>` / `file:<文件名>` 三态，不把 CSS 塞进 `data.json`。
- 文档与门禁：README 功能说明、`docs/capability/` 对应页、`npm run check` 全量门禁；主题 CSS 需过 `CSS 解析` 步；新增「内置 id ↔ 资源文件」一致性测试与「无主题时像素不变」的默认值基线核对。
- 非目标：不做在线主题商店、不做主题内脚本、不改布局（列宽/留白/滚动结构仍归 `knowledge-base/writing-layout`）；不接受任意选择器与整份第三方 CSS（只收 59 个白名单变量）；不覆盖阅读字号、正文字距与列宽档位，不改语义色与捕获气泡底。