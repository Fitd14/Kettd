# Spec Delta

## MODIFIED Requirements

### Requirement: 字体族只多一族

界面默认字族 SHALL 为 `--font-ui`（系统黑体栈）；自绘字体 SHALL 只有一个报头衬线子集（`Kettd Serif`，61 字 / 约 12 KB），MUST 只用于 `.view-title` 与侧栏品牌字两处；样式表 MUST NOT 出现裸字族名（如 `"Noto Sans SC"`），字族一律经 `--font-*` 令牌取值。

知识库文档标题 SHALL 使用系统衬线，并经独立令牌 `--font-reading-title` 取值；该令牌 MUST NOT 包含 `Kettd Serif`，以免同一标题里部分字走 61 字子集、其余字回落到系统衬线而出现两种衬线混排。

#### Scenario: 新增文案落到正文

- **WHEN** 新功能在列表行、按钮或副行里新增中文
- **THEN** 这些字由 `--font-ui` 渲染，不出现同一行里部分宋体、部分黑体的混排

#### Scenario: 报头出现子集外的字

- **WHEN** 报头渲染了子集字表之外的字符
- **THEN** 该字符回落到 `"Noto Serif SC"` → `"Songti SC"` → `SimSun` → `serif`，仍是衬线，不出现黑体断档

#### Scenario: 文档标题不混用子集

- **WHEN** 用户打开标题含子集外字符的知识条目
- **THEN** 整条标题由 `--font-reading-title` 一族的字体渲染，同一条标题内不出现两种衬线

#### Scenario: 授权可追溯

- **WHEN** 检查 `src-react/src/assets/fonts/`
- **THEN** 同时存在子集字体、来源说明 `NOTICE.md` 与 OFL 1.1 全文 `OFL-1.1.txt`