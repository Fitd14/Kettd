# Audit — kb 知识库 · Markdown 编辑器体验走查

- **生成时间**：2026-09-10
- **走查对象**：src-react/src/components/kb/kb-item-detail.tsx + rendering/tiptap-editor.tsx + md-static-render.tsx（代码模式）
- **走查范围**：KB 详情编辑/阅读全流程（单屏聚焦）
- **走查模式**：模式 A 自动走查（代码）+ 竞品对照（Typora / Notion / Obsidian / 语雀）
- **竞品参照**：Typora 即时渲染被公认为"写即所见"标杆（Vditor 三模式并存亦以其为参照）；Notion 用 slash 命令面板做块插入；表格创建业界三路径=快捷键网格选择器（Typora Ctrl+T）/ 工具栏按钮 / slash 命令；Obsidian 实时预览（带语法痕迹）被部分用户吐槽"体验尴尬"——印证纯语法态不可取

## owner 原始问题 → findings 映射

| owner 反馈 | 对应 findings |
| --- | --- |
| ① 阅读和编辑分离 | N01、N09、N11、N12 |
| ② 编辑器体验差 | N03、N04、N06、N10 |
| ③ 阅读体验差 | N07 |
| ④ 缺少提示 | N06、N10 |
| ⑤ 无法创建表格 | N02 |

## 总览

| 严重度 | 数量 |
| --- | --- |
| 🔴 Blocker | 2 |
| 🟠 Major | 5 |
| 🟡 Minor | 5 |

## 改版机会点（按优先级）

### 🥇 机会点 1：单一即时渲染画布（去模式化）
- 关联 findings：N01、N09、N11、N12
- 现状：阅读/编辑双模式按钮切换，阅读态点击任意处进编辑（误触），切换丢滚动位置且编辑器全量重挂载
- 竞品对照：Typora 证明"无模式"才是即时渲染的正确形态；Obsidian 的模式残留被吐槽
- 方向：TipTap 画布常驻即成品样式（所见即所得本义），去掉模式切换；阅读感由排版体系承担而非模式承担
- effort：major-rework ｜ priority：**high**（owner 问题①的根）

### 🥇 机会点 2：表格全能力
- 关联 findings：N02
- 现状：@tiptap/extension-table 未安装（grep 0 命中），编辑器造不出表格；阅读态 markdown-it 反而能渲染表格——"看得见吃不着"
- 竞品对照：Typora Ctrl+T 网格选择器 / Notion slash 命令 / 语雀工具栏按钮，三路径业界并行
- 方向：装 Table 扩展族（Table/Row/Cell/Header）+ 工具栏表格钮 + Cmd/Ctrl+T 网格选择器 + 表内气泡菜单（增删行列）+ Tab 跳格
- effort：medium ｜ priority：**high**（功能缺口）

### 🥈 机会点 3：编辑提示体系
- 关联 findings：N06、N10
- 现状：9 钮字符工具栏无分组无快捷键提示；无 slash 命令、无选区浮动工具栏、无语法引导
- 竞品对照：Notion slash 面板（常用优先+键盘导航）；Typora 快捷键优先+极简栏
- 方向：slash 命令面板（/表格 /列表 /引用 /代码…）+ 选中文字弹浮动格式条 + placeholder 引导语 + 工具栏 title 补 kbd
- effort：medium ｜ priority：**high**（owner 问题②④）

### 🥈 机会点 4：阅读排版体系
- 关联 findings：N07
- 现状：渲染区与编辑区同字号层级，标题/正文对比弱，无阅读宽度上限、无目录
- 方向：标题阶梯（h1-h3 字号/字重/间距体系）+ 长文阅读宽度上限 + 代码块/引用密度优化；目录视长文再议
- effort：medium ｜ priority：medium（owner 问题③）

### 🥉 机会点 5：元数据就地编辑 + 保存可见
- 关联 findings：N03、N04、N05、N08
- 现状：标题 h3 静态不可改名；标签 chips 只读不可增删；自动保存无"已保存"指示；工具栏字符图标与 lucide 语言不一致
- 方向：标题点击就地编辑、标签 chips 带加号/删除、编辑器角落"已保存/编辑中"状态、工具栏换 lucide 图标
- effort：medium（quick-win 集合）｜ priority：medium（owner 问题②）

## 完整 Findings 清单

### flexibility（灵活与效率）
1. **[blocker] N01** 阅读/编辑双模式割裂：即时渲染被人为切断，违背"所见即所得"拍板本义
   - 位置：kb-item-detail.tsx:60,121（两处 onClick setMode('edit')）
   - 建议：去模式化，常驻即时画布 ｜ 成本：major-rework
2. **[blocker] N02** 表格无法创建：TipTap 无 Table 扩展（阅读态 markdown-it 却支持渲染，错位）
   - 位置：package.json（extension-table 0 命中）
   - 建议：Table 扩展族 + 三种创建路径 ｜ 成本：medium
3. **[minor] N10** 无快捷键提示与补充快捷键（Cmd+K 链接、Cmd+T 表格等）
   - 建议：工具栏 kbd 提示 + 快捷键 ｜ 成本：quick-win

### user-control（用户控制）
4. **[major] N03** 标题不可就地改名（kb-item-detail.tsx:99 静态 h3）
   - 建议：点击就地编辑 ｜ 成本：medium
5. **[major] N04** 标签不可增删（chips 只读）
   - 建议：就地增删 chips ｜ 成本：medium

### visibility（状态可见）
6. **[major] N05** 自动保存无反馈（便签窗有"已保存"、KB 详情没有）
   - 建议：保存状态指示 ｜ 成本：quick-win
7. **[minor] N09** 模式按钮当前态辨识弱（与 N01 一并消灭）
   - 成本：随 N01 消失

### help-docs / recognition（提示与识别）
8. **[major] N06** 提示体系缺失：无 slash 命令/浮动工具栏/语法引导，工具栏 9 钮无分组
   - 建议：见机会点 3 ｜ 成本：medium

### aesthetic（阅读美学）
9. **[major] N07** 阅读排版弱：层级对比不足、无宽度上限、代码块密度高
   - 建议：排版阶梯 + 宽度上限 ｜ 成本：medium

### consistency（一致性）
10. **[minor] N08** 工具栏字符图标（B/I/❝/</>）与全 app lucide 图标语言不一致
    - 建议：换 lucide（Bold/Italic/List/Quote/Code…）｜ 成本：quick-win

### error-prevention（防错）
11. **[minor] N11** 阅读态任意点击进编辑，误触率高（并入 N01 处理）

### performance（性能感知）
12. **[minor] N12** 模式切换重挂载编辑器全量重解析，长文卡顿隐患（去模式后自然消失）

## 下一步建议

- 按 5 个机会点出**编辑器打磨设计稿**（交互细节 + 竞品模式选型落位），再进实施
- 机会点 1 是架构级（去模式化），建议设计先行拍板后与机会点 2/3 一起实施（同文件族）
