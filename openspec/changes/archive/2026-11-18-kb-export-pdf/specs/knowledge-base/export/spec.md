# Spec Delta

## ADDED Requirements

### Requirement: 详情页可把当前文档导出为 PDF

知识库详情页 SHALL 提供「导出为 PDF」动作，把当前条目的正文导出为一个 `.pdf` 文件。该动作 MUST 与 Word 导出并列出现在文档工具条的动作区，MUST 有可读的中文名称或 `aria-label`；正文为空时该动作 MUST 处于禁用状态，MUST NOT 打开保存对话框。

#### Scenario: 从详情页导出 PDF

- **WHEN** 用户在一篇有正文的文档上点击「导出为 PDF」
- **THEN** 系统 SHALL 先弹出保存对话框，确认后在用户选定位置生成一个 `.pdf` 文件
- **AND** 导出过程中按钮 MUST 显示进行中状态，MUST NOT 被重复触发

#### Scenario: 空正文不可导出

- **WHEN** 当前条目正文为空或只有空白字符
- **THEN** 「导出为 PDF」MUST 处于禁用状态，点击不产生保存对话框，也不产生文件

#### Scenario: 非 Windows 构建不暴露该能力

- **WHEN** 应用运行在不提供 WebView2 打印能力的平台上
- **THEN** 「导出为 PDF」MUST NOT 出现在工具条上，调用同名命令 MUST 返回中文错误而不是崩溃

### Requirement: PDF 导出选择目标路径并默认落在下载目录

导出 PDF SHALL 通过系统保存对话框让用户选择目录与文件名；对话框的默认文件名 MUST 为 `<标题>.pdf`（标题经文件名清洗，非法字符替换为空格），默认目录 MUST 为系统下载目录，过滤器 MUST 只列 `pdf`。用户取消对话框时系统 MUST 什么都不做，MUST NOT 生成文件或报错。

#### Scenario: 默认落在下载目录

- **WHEN** 用户触发 PDF 导出且未改动对话框
- **THEN** 文件 MUST 写入系统下载目录，文件名为清洗后的标题加 `.pdf`

#### Scenario: 取消不产生副作用

- **WHEN** 用户在保存对话框里点取消
- **THEN** 系统 MUST 静默返回，不生成文件、不显示错误、不改变任何设置

#### Scenario: 目标目录不可写

- **WHEN** 用户选择的目录没有写入权限
- **THEN** 系统 MUST 显示中文短句说明换一个位置，MUST NOT 在目标目录留下临时文件或半截文件

### Requirement: PDF 内容与阅读视图结构一致

PDF 的内容 SHALL 由当前文档正文生成，排版 MUST 覆盖标题层级、段落与行内强调、列表、引用、代码块、表格与图片。生成 MUST 使用与阅读视图相同的 Markdown 解析配置；图片 MUST 内嵌进 HTML 而不是依赖运行时可访问的路径；页面 MUST 为 A4、使用打印默认边距，且 MUST NOT 输出页眉页脚或页码。打印件 MUST 使用中性文档基线（白纸、黑字、只表达结构），MUST NOT 套用应用外观主题的纸色、墨色、圆角、斑马纹等屏幕皮肤，MUST NOT 混入应用外壳元素（侧栏、工具条、按钮）。

#### Scenario: 复杂文档导出后结构完整

- **WHEN** 导出包含标题、列表、引用、代码块、表格与图片的文档
- **THEN** PDF MUST 页数大于 0，且逐项包含上述结构的内容
- **AND** 中文 MUST 正常显示，MUST NOT 出现方框或乱码

#### Scenario: 图片内嵌

- **WHEN** 文档含本地图片
- **THEN** 图片 MUST 出现在 PDF 中，且导出完成后 MUST NOT 依赖原图片路径仍然可读

#### Scenario: 只打印正文

- **WHEN** 文档在应用内导出
- **THEN** PDF 页面 MUST 只包含文档正文与中性排版，MUST NOT 包含工具条、侧栏、按钮或应用外观主题的装饰

### Requirement: PDF 打印有超时与失败回退

打印 SHALL 在固定上限内完成，超时 MUST 判定为失败；打印回调报告失败、WebView2 运行时不满足要求、或临时资源准备失败时，系统 MUST 返回中文短句错误，MUST NOT 崩溃，MUST NOT 留下未完成的 `.pdf`。

#### Scenario: 打印超时

- **WHEN** 打印在超时上限内没有收到完成回调
- **THEN** 系统 MUST 关闭离屏窗口、清理临时文件并返回中文超时提示

#### Scenario: 打印回调失败

- **WHEN** WebView2 的打印完成回调报告 `isSuccessful = false`
- **THEN** 系统 MUST 返回中文失败提示，且目标路径 MUST NOT 出现损坏的 PDF

#### Scenario: 连续导出

- **WHEN** 用户先后导出两次 PDF
- **THEN** 两次 MUST 各自成功，第二次 MUST NOT 因为上一次的离屏窗口或用户数据目录被占用而失败

### Requirement: PDF 导出不改变文档与设置

导出 PDF SHALL 是只读操作：MUST NOT 修改条目的正文、标题、更新时间或排序，MUST NOT 写入 `data.json`，MUST NOT 改变导出目录之外的任何用户设置。

#### Scenario: 导出后数据不变

- **WHEN** 用户导出一次 PDF 后重新打开该条目
- **THEN** 正文、标题与更新时间 MUST 与导出前一致
- **AND** 数据目录下的 `data.json` MUST 未被导出操作改写

### Requirement: 导出格式覆盖 Word 与 PDF

知识库文档导出能力 SHALL 同时覆盖 `.docx` 与 `.pdf` 两种格式：两种格式 MUST 共用同一套入口位置、标题清洗规则、默认下载目录与保存对话框交互；任一种格式失败时 MUST NOT 影响另一种格式的可用性。

#### Scenario: 两种格式共用同一套交互

- **WHEN** 用户分别导出同一篇文档的 Word 与 PDF
- **THEN** 两次的默认文件名 MUST 都是清洗后的同一标题（仅扩展名不同），默认目录 MUST 都是系统下载目录

#### Scenario: 一种格式失败不影响另一种

- **WHEN** PDF 导出因运行时不可用而失败
- **THEN** 导出 Word MUST 仍然可用并成功，且失败提示 MUST 指明是 PDF 这一种格式出了问题