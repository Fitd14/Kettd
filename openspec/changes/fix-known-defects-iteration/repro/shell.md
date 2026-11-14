# 缺陷复现证据（#3 / #13 / #16）

## 环境

| 项 | 值 |
| --- | --- |
| 仓库 | `C:\Users\simy\project\mywork\kettd`（Kettd，Tauri v1.8.3 + React 19） |
| git HEAD | `2167a48`（完整 `2167a48c238bb1a86c88cd33dd79157809ca3eea`，2026-11-13 22:38:47 +0800） |
| 调查时间 | 2026-11-13 23:05（+08:00） |
| 工作树 | `M src-react/src/rendering/styles/prosemirror.css`、`M src-react/src/rendering/tiptap-editor.tsx`、`M src-tauri/src/kb.rs`（与本 3 项无关）；`src-tauri/Cargo.toml` 仅为行尾差异（`git diff` 无内容差异）；本报告引用的其余文件均与 HEAD 一致 |
| crate 源码前缀 | 下文 `CS\` = `C:\Users\simy\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\`（Cargo.lock：tauri 1.8.3 / tauri-runtime 0.14.6 / tauri-runtime-wry 0.14.11 / tauri-utils 1.6.2 / wry 0.24.12 / webview2-com 0.19.1） |
| 构建产物 | `src-tauri\target\debug\Kettd.exe`（2026-11-13 22:34:29，17,799,680 B）；`src-tauri\target\release\Kettd.exe`（22:23:22，9,958,912 B）；`target\release\bundle\msi\Kettd_2.2.0_x64_zh-CN.msi`（22:23:24）、`target\release\bundle\nsis\Kettd_2.2.0_x64-setup.exe`（22:23:32） |
| WebView2 | 运行时版本 157.0.4341.46（注册表 `HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` 的 `pv`）；用户数据目录 `C:\Users\simy\AppData\Local\com.todo.list\EBWebView` 存在（LastWriteTime 2026-11-13 22:48:28） |
| 方法 | 纯静态阅读 + 只读命令（git/文件/进程查询）。**调查期间未启动应用、未运行 build、未改任何源码** |
| 调查时进程快照 | `Get-Process -Name Kettd` 与 `Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'"` 均无结果（未启动应用；该快照不能否定缺陷的间歇性） |

---

## #3 右键菜单仍出现「开发者工具/检查」

### 现象

应用网页区域右键时弹出 WebView2 原生上下文菜单，其中含「检查 / Inspect」（打开 DevTools 的入口）。

### 证据

1. **仓库没有任何自定义右键菜单**：`grep -n "contextmenu|onContextMenu|context-menu|devtools|inspect" src-react/src` → 0 命中；`grep -n "devtools|contextmenu|onContextMenu|SetAreDefaultContextMenus|open_devtools" src-tauri/src` → 0 命中。即该菜单完全来自 WebView2 默认设置。

2. **默认右键菜单由 wry 无条件开启**：`CS\wry-0.24.12\src\webview\webview2\mod.rs:245-258`

   ```rust
   settings
     .SetAreDefaultContextMenusEnabled(true)   // :246 无条件 true
   ...
   settings
     .SetAreDevToolsEnabled(false)             // :252 先默认关闭
   if attributes.devtools {                    // :254-258 仅 devtools=true 时打开
     settings.SetAreDevToolsEnabled(true)
   }
   ```

   即：默认菜单始终存在；「检查」项是否出现取决于 `attributes.devtools`。

3. **devtools 属性的来源（debug/feature 双条件）**：`CS\tauri-runtime-wry-0.14.11\src\lib.rs:3392-3395`

   ```rust
   #[cfg(any(debug_assertions, feature = "devtools"))]
   { webview_builder = webview_builder.with_devtools(true); }
   ```

4. **本项目未启用 devtools feature**：`src-tauri/Cargo.toml:17` 为 `tauri = { version = "1.0.0", features = [ "api-all", "system-tray" ] }`，无 `devtools`；tauri 1.8.3 默认 features 为 `["wry","compression","objc-exception"]`（`CS\tauri-1.8.3\Cargo.toml`），`devtools = ["tauri-runtime/devtools","tauri-runtime-wry/devtools"]` 不在默认。tauri 1.8.3 文档亦写明：devtools “Enabled by default on debug builds”（`CS\tauri-1.8.3\src\lib.rs:22`）。

5. **配置层无开关**：`src-tauri/tauri.conf.json:68-101` 仅声明 `main`（:68-82）与 `capture`（:83-100）两窗，无 devtools 字段；Tauri v1 的 `WindowConfig` 也没有 devtools 字段（`CS\tauri-utils-1.6.2\src\config.rs:1233` 只有 `additional_browser_args`）。

6. **dev 构建 vs release 安装包的静态差异**：

   | 构建 | 编译条件 | wry 行为 | 右键菜单预期 |
   | --- | --- | --- | --- |
   | `npm run tauri dev` / `target\debug\Kettd.exe`（debug profile） | `debug_assertions` 开 | 命中 `any(debug_assertions, ...)` → `with_devtools(true)` → `SetAreDevToolsEnabled(true)` | 原生默认菜单 **含「检查」** |
   | `npm run tauri build` / MSI/NSIS 安装包（release profile，`Cargo.toml` 无 `[profile.release]` 覆盖） | `debug_assertions` 关，且 feature 表无 `devtools` | 条件不成立 → 保持 `SetAreDevToolsEnabled(false)` | 原生默认菜单 **不含「检查」**，但仍有复制/粘贴/全选/刷新等默认项（`wry:246` 无条件开启，属独立问题） |

7. 外部依据：Microsoft Learn `CoreWebView2Settings.AreDevToolsEnabled` —— “Determines whether the user is able to use the context menu or keyboard shortcuts to open the DevTools window.”（<https://learn.microsoft.com/zh-tw/dotnet/api/microsoft.web.webview2.core.corewebview2settings.aredevtoolsenabled>）

8. **结论（推断）**：缺陷「右键菜单出现开发者工具/检查」在 **dev 构建上必然可复现**（源于 `debug_assertions`）；**release 安装包预期不出现「检查」**，但右键仍有原生默认菜单（这是另一层待修项：仓库若要完全自定义/屏蔽右键，需要在 Rust 侧对每个 WebView 调 `SetAreDefaultContextMenusEnabled(false)`，与 design.md D10 一致）。若用户报告来自 release 安装包，则需真机截图复核（见下）。现有两个可对照产物：`target\debug\Kettd.exe`（22:34:29）与 `target\release\bundle\nsis\Kettd_2.2.0_x64-setup.exe`（22:23:32）。

### 复现步骤（用户操作路径）

- **dev 构建**：`npm run build:front` 后 `npm run tauri dev`（AGENTS.md 约定 dev 需先构建前端）→ 主窗任意网页区域右键 → 菜单中出现「检查 / Inspect」。
- **release 安装包**：安装 `target\release\bundle\nsis\Kettd_2.2.0_x64-setup.exe`（或 MSI）后启动 → 同一位置右键 → 记录菜单实际项。
- 修复验证后：同一位置右键应无可打开 DevTools 的菜单项（若产品要求，默认菜单也应消失）。

### 是否可无头验证

部分可：上述静态证据（代码路径、cfg 条件、feature 表、配置）已能确定 dev/release 的菜单开关差异，无需运行应用；但菜单里实际显示的条目（及其本地化文案「检查」）只能真机查看。

### 需真机确认点

1. dev 构建右键菜单截图：确认含「检查」。
2. release 安装包右键菜单截图：确认无「检查」，并记录残留的默认菜单项。
3. 确认缺陷报告者实际运行的是哪个构建（dev 产物 vs 安装包版本），这是判定「缺陷是否落在 release」的关键。

---

## #13 侧栏与知识库折叠

### 现象

(a) **侧栏**：五个视图入口只有文字、没有图标；没有折叠按钮，也没有折叠状态持久化。窗口宽度 ≤1040px 时侧栏被压到 56px 且文字（label/count/brand）全部隐藏，变成一条无可点击标识的窄条。
(b) **知识库**：左侧清单虽有折叠钮，但折叠时列宽变 0 且 `display:none`，条目标题信息全部丢失，只能靠「⟩ 列表」浮钮恢复；未折叠时条目标题过长没有省略号/截断，长标题会换行或撑破行高。

### 证据

1. **侧栏只有文字**：`src-react/src/App.tsx:21-28` 的 `ROUTES` 仅 `{hash, label}`（今天/收件箱/计划/回顾/知识库/设置）；渲染在 `:99-112`，每个按钮只有 `<span className="label">` 与可选 `<span className="count">`（`:108-109`），`title={r.label}`（`:105`）——无 `<svg>`/icon、无折叠按钮、无 localStorage 读写。

2. **侧栏无折叠态**：全仓 `grep -n "sideCollapsed|side-collapsed|listCollapsed" src-react/src` 只命中 `KbView.tsx`（`kb.listCollapsed` 共 9 处），侧栏没有任何折叠状态键。

3. **侧栏宽度与响应式**：`src-react/src/styles/app.css:391-404`

   ```css
   .shell-side { width: 168px; flex: none; display: flex; flex-direction: column; ... }  /* :391-400 */
   .shell-side .brand { font-size: 15px; ... }                                            /* :401-405 */
   ```

   `src-react/src/styles/app.css:441-454`

   ```css
   @media (max-width: 1040px) {
     .shell-side { width: 56px; }                 /* :443-445 */
     .shell-side .brand { font-size: 0; ... }     /* :446-449 */
     .side-item .label, .side-item .count { display: none; }  /* :450-453 */
   }
   ```

   主窗 `minWidth` 为 1024（`src-tauri/tauri.conf.json:74`），窗口拖到最窄即可触发该断点：56px 内只剩 padding，无图标可点。

4. **图标库已具备、已在本项目使用**：`src-react/package.json:31` `"lucide-react": "^1.40.0"`；现有使用：`todo/TodoFloatWindow.tsx:2`、`sticky/StickyWindow.tsx:2`、`rendering/tiptap-editor.tsx:21`、`components/task-detail.tsx:2`、`components/kb/kb-item-detail.tsx:4` → 侧栏加图标无需新增依赖。

5. **知识库折叠已存在（持久化也有）**：`src-react/src/views/KbView.tsx:32-42`——`listCollapsed` 初值读 `localStorage.getItem('kb.listCollapsed') === '1'`，`toggleList` 写 `'0'/'1'`；`:163-169` 折叠时连 `KbTagFilter` 一起隐藏；`:171` 切换 class `kb-layout is-collapsed`；`:195-207` 详情面板左缘「⟨/⟩」切换钮与折叠后的「⟩ 列表」浮钮。

6. **折叠即标题全丢**：`src-react/src/views/kb.css:51-68`

   ```css
   .kb-layout { grid-template-columns: clamp(200px, 232px, 264px) 1fr; gap: 1rem; ... }  /* :51-58 */
   .kb-layout.is-collapsed { grid-template-columns: 0 1fr; gap: 0; }                      /* :60-63 */
   .kb-layout.is-collapsed .kb-list { display: none; overflow: hidden; }                  /* :65-68 */
   ```

   折叠后左列宽度为 0 且列表整体不渲染，没有任何「窄轨」保留可识别信息（设计期望 72–110px 窄轨 + 单行省略，见 design.md D10）。

7. **标题无截断**：`src-react/src/components/kb/kb-item-row.tsx:28` 直接输出 `<span className="item-title">{item.title}</span>`；`src-react/src/views/kb.css:149-152`

   ```css
   .kb-item-row .item-title { font-weight: 500; font-size: 0.95em; }
   ```

   无 `white-space: nowrap` / `overflow: hidden` / `text-overflow: ellipsis`（`kb.css` 中 ellipsis 仅出现在 :265 的保存提示 `.save-pill` 的 `white-space: nowrap`），长标题不受控。

8. **结论（推断）**：侧栏需补「图标（lucide）+ 折叠钮 + `localStorage('shell.sideCollapsed')` 持久化 + 窄轨 icon-only」；KB 折叠需从 `0 + display:none` 改为保留窄轨（或至少不整体卸载列表），并给 `.item-title` 补单行省略。与 `openspec/changes/fix-known-defects-iteration/design.md:74-76`（D10）方向一致。

### 复现步骤（用户操作路径）

- **侧栏（a）**：启动应用 → 主窗左侧 168px 侧栏观察：无图标、无「收起/展开」按钮；把主窗拖窄到约 1024px（≤1040px 断点）→ 侧栏变 56px，label 与 count 全部消失，只剩空白窄条；关闭重开应用后无任何侧栏折叠状态记忆。
- **知识库（b）**：进入「知识库」→ 点右侧详情面板左缘「⟨」→ 左侧清单整列消失（宽度 0），只能点「⟩ 列表」恢复；新建一条超长标题的知识条目并查看左侧清单 → 标题换行/撑高、无省略号。

### 是否可无头验证

部分可：`App.tsx`/`app.css`/`KbView.tsx`/`kb.css` 的 DOM 与 CSS 静态可核查（本报告即静态证据），无需运行应用；但真实窗口宽度下的视觉效果、长标题溢出与折叠持久化行为需真机查看。

### 需真机确认点

1. 侧栏在 ≤1040px 时的实际观感（56px 是否完全没有可辨识入口）。
2. KB 折叠后是否仍能识别/选择清单条目（当前必然不能，需确认修复目标形态）。
3. 超长标题在 200–264px 列宽内的实际表现（换行/溢出/行高）。
4. 侧栏折叠状态重启后是否按预期保持（修复后）。

---

## #16 退出应用后 msedgewebview2.exe 进程残留

### 现象

从托盘菜单「退出」应用后，任务管理器中仍可见 `msedgewebview2.exe` 后台进程（WebView2 Runtime 进程组），制约内存回收；再次启动应用可能叠加/复用旧进程。

### 证据（退出链路，全部真实 file:line）

1. **托盘「退出」菜单项**：`src-tauri/src/runtime.rs:887-907`（`tray_menu`），`:906` `.add_item(CustomMenuItem::new("quit", "退出"))`；点击分发 `src-tauri/src/runtime.rs:932-976`（`handle_tray_click`），`:972` `"quit" => return true`。

2. **唯一退出动作 = `app.exit(0)`**：`src-tauri/src/main.rs:269-276`

   ```rust
   .on_system_tray_event(|app, event| match event {
     tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
       if runtime::handle_tray_click(app, &id) {
         app.exit(0);          // :272
       }
     }
     _ => {}
   })
   ```

3. **`app.exit` 是硬退出**：`CS\tauri-1.8.3\src\app.rs:564-568`

   ```rust
   pub fn exit(&self, exit_code: i32) {
     self.cleanup_before_exit();      // :566
     std::process::exit(exit_code);   // :567
   }
   ```

   `cleanup_before_exit`（`:576-588`）只做两件事：`#[cfg(any(shell_execute, shell_sidecar))]` 的 `kill_children()`（杀 Tauri shell 子进程；与本缺陷的 WebView2 子进程无关）和系统托盘 `tray.destroy()`。**不销毁任何窗口、不关闭任何 WebView2 Controller**。

4. **正常的事件链被跳过**：`CS\tauri-runtime-wry-0.14.11\src\lib.rs:3081-3097`——只有收到 `WryWindowEvent::Destroyed` 并把窗口从集合移除、集合为空时才回调 `RunEvent::ExitRequested`，未 prevent 才 `ControlFlow::Exit`；`CS\tauri-1.8.3\src\app.rs:906-923` 在 `RuntimeRunEvent::Exit` 时才 `cleanup_before_exit`。而本应用 `src-tauri/src/main.rs:324` 是 `app.run(|_app_handle, _event| {});`（空回调），且 `app.exit(0)` 直接 `std::process::exit`，**不会触发 Destroyed → ExitRequested → Exit 链**。

5. **退出瞬间存活多个 WebView 窗口**：
   - 启动即恢复全部便签窗：`src-tauri/src/main.rs:254-257` 循环 `runtime::open_note_window`（`runtime.rs:123-134` `note_builder`，`WindowUrl::App("float.html")`）。
   - 按上次显隐恢复待办悬浮窗：`src-tauri/src/main.rs:258-262` → `runtime::open_todo_float`（`runtime.rs:311-321` `todo.html`）。
   - 快速记录条：`src-tauri/tauri.conf.json:83-100` 启动即建（`capture.html`，`visible:false`），另有 `runtime.rs:420-437` `create_capture_window` 兜底。
   - 主窗：`src-tauri/tauri.conf.json:68-82`（`main`，`index.html`）。

   这些 WebView 共用同一 user data folder——Tauri 在 Windows 强制 `data_directory` 为 LocalData/identifier（`CS\tauri-1.8.3\src\manager.rs:1059-1073`），本机实测即 `C:\Users\simy\AppData\Local\com.todo.list\EBWebView`。按 WebView2 进程模型，同一 user data folder 的多个 WebView 共享**一个 browser 进程**及 renderer/GPU 等子进程。

6. **进程内没有窗口级清理代码**：`grep -n "destroy|RunEvent" src-tauri/src` 无命中（无 `Window::destroy`、无 `RunEvent` 处理、无 `app.windows()` 遍历关闭）；仅便签窗的 CloseRequested 会真正关窗销毁（`src-tauri/src/main.rs:293-308`，不 `prevent_close`），主窗/待办窗的关闭是 `prevent_close()+hide()`（`:281-290`），快速记录条失焦是 `hide()`（`:310-313`）。

7. **外部依据**：
   - Microsoft Learn WebView2 进程模型：“To safely shut down associated browser and renderer processes, use the `Close` method of `CoreWebView2Controller`.”（<https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/process-model>）
   - WebView2 UWP Best Practices：“Explicitly close the WebView2 controller on app shutdown … Without an explicit `Close()`, the WebView2 instance is torn down without going through the normal browser shutdown procedure.”（<https://github.com/microsoftedge/webview2feedback/blob/2efb0bcf349c5bbb2040df78eb36e851cb6853d7/best-practices/uwp-best-practices.md>）
   - 同类残留案例：<https://learn.microsoft.com/en-ca/answers/questions/425814/c-close-webview2-problem>、<https://github.com/dotnet/maui/issues/7997>。

8. **结论（推断）**：托盘「退出」= `app.exit(0)` 硬退进程，跳过窗口销毁与 `CoreWebView2Controller.Close()`；多个窗口的 WebView2 进程组（browser 进程为根）未走正常关闭流程，因此存在 `msedgewebview2.exe` 残留的确切代码成因。属「缺少优雅关闭 WebView2 的退出路径」的确定性静态缺陷；残留是否必现、持续多久取决于 WebView2 Runtime 的回收行为，必须真机确认（调查时本机当前无残留进程，说明并非每次退出都长期残留）。

### 复现步骤（用户操作路径，真机）

1. 启动应用（dev 或 release 安装版均可），使 WebView 数量尽量多：主窗可见 + 托盘「新建便签」1–2 个 + 托盘打开待办悬浮窗 + 托盘「快速记录」唤起记录条。
2. 任务管理器 →「详细信息」，搜索 `msedgewebview2.exe`，记录 PID、父 PID、命令行（`--user-data-dir` 应含 `com.todo.list\EBWebView`）。
3. 托盘图标右键 → 「退出」（注意：直接点主窗关闭只是收进托盘，不退出，见 `main.rs:281-284`）。
4. 退出后 0s / 2s / 5s / 10s / 30s 记录残留数量与父进程是否存活。
5. 对照实验：任务管理器「结束任务」强杀 `Kettd.exe` vs 托盘「退出」，观察两者残留差异。
6. 再次启动应用，观察是否复用残留进程树、是否出现进程数叠加。

命令行取证（只读）：

```powershell
Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
  Select-Object ProcessId, ParentProcessId, CreationDate, CommandLine | Format-List
Get-Process -Name Kettd -ErrorAction SilentlyContinue | Select-Object Id, StartTime
```

### 是否可无头验证

可无头验证「退出链路没有 WebView2 关闭/窗口销毁代码」这一静态事实（本报告第 1–6 条，纯静态即可确认）；「退出后是否残留、残留多久」无法无头验证，必须在真机用任务管理器/进程查询观察。

### 需真机确认点

1. 残留是否出现、数量与内存占用、持续时长（是否 30s 后仍在）。
2. 是否每次退出必现；与打开的窗口数（便签/待办/记录条）是否正相关。
3. dev 构建与 release 安装包是否表现一致。
4. 与 #17（建窗 data_directory 相关修复）复测时的交互：修复退出路径后是否一并消失。

---

## 附：证据采集方式

- 代码/行号：仓库与 Cargo registry 源码直接阅读（read/grep）；`git rev-parse --short HEAD`、`git status --porcelain`、`git diff --numstat`。
- 产物与时间：`Get-Item` / `Get-ChildItem`。
- WebView2 运行时版本与数据目录：只读注册表查询 + 目录列举。
- 未运行应用、未运行 build、未执行 git 写命令；除本文件（及新建 `repro` 目录）外未写入任何文件。