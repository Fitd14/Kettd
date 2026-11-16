# Spec Delta

## Purpose

定义知识库在拆分存储后的行为契约：正文与元数据索引分离、图片资源内容寻址外置、列表不再传输全文、旧数据一次性幂等迁移，以及损坏与一致性边界。目标是让单条文章的内容量不再拖累整库读写，同时保持现有用户可见行为可用。

## ADDED Requirements

### Requirement: 元数据索引与正文分离

知识库持久化 MUST 将条目元数据（`id`、`title`、`tags`、`folderId`、`order`、`pinned`、`createdAt`、`updatedAt`、`contentHash`）存入 `notes.json` 索引，正文 MUST 存入独立的 `notes/<id>.md` 文件；`notes.json` MUST NOT 内联正文或图片数据。

#### Scenario: 新建条目拆分落盘

- **WHEN** 用户新建一条含正文的知识库条目
- **THEN** `notes.json` 中该条目只包含元数据与 `contentHash`，不含 `bodyMd` 字段
- **AND** `notes/<id>.md` 的内容等于规范化后的正文
- **AND** `contentHash` 等于该正文的 SHA-256

#### Scenario: 编辑正文保持一致

- **WHEN** 用户修改某条目的正文并保存
- **THEN** 系统先原子写入 `notes/<id>.md`，再原子写入 `notes.json` 索引
- **AND** 索引中的 `updatedAt` 与 `contentHash` 随正文更新
- **AND** 若写索引失败，旧正文文件与旧索引 MUST 仍可读取，不出现「索引更新但正文丢失」的半套状态

#### Scenario: 删除条目清理正文

- **WHEN** 用户删除一条知识库条目
- **THEN** 系统删除该条目的索引记录与 `notes/<id>.md` 文件
- **AND** 其余条目与其正文不受影响

### Requirement: 列表只返回元数据、正文按需读取

知识库列表读取 MUST 只返回条目元数据，MUST NOT 随列表传输正文；正文 MUST 在打开详情、知识便签等需要展示的时机按条目 id 读取。

#### Scenario: 打开知识库列表

- **WHEN** 用户进入知识库目录
- **THEN** 列表数据只包含元数据，正文为空
- **AND** 点击某条条目打开详情时，系统按 id 读取并展示完整正文

#### Scenario: 批量取正文

- **WHEN** 前端一次需要展示或备份多个条目的正文
- **THEN** 系统 MUST 支持按一组 id 一次取回正文，避免逐条往返
- **AND** 不存在的 id MUST 被忽略，不导致整次请求失败

### Requirement: 图片内容寻址外置与去重

保存正文时，正文中的 `data:image/*;base64` 图片 MUST 被抽取为独立资源文件 `assets/<sha256>.<ext>`（扩展名白名单：`png`、`jpg`、`webp`、`gif`），正文 MUST 改写为相对引用 `assets/<sha256>.<ext>` 并保留 alt 文案。

#### Scenario: 粘贴图片后保存

- **WHEN** 用户粘贴一张压缩后的图片并保存条目
- **THEN** 正文中不再出现 `data:image` 内联数据
- **AND** `assets/` 下存在以该图片内容 SHA-256 命名的资源文件
- **AND** 阅读视图能正常显示该图片

#### Scenario: 相同图片去重

- **WHEN** 两条条目（或同一篇内两次）引用内容完全相同的图片
- **THEN** `assets/` 中 MUST 只保留一份该图片文件
- **AND** 两处引用 MUST 都指向同一相对路径

#### Scenario: 资源缺失不崩溃

- **WHEN** 渲染正文时某 `assets/<sha256>.<ext>` 文件缺失或无法解析
- **THEN** 渲染 MUST 显示占位或跳过该图片，MUST NOT 导致整页/整窗崩溃或白屏

### Requirement: 旧数据一次性幂等迁移

应用载入知识库时若检测到旧格式（`notes.json` 条目内联 `bodyMd`），MUST 执行一次性迁移：先轮转 `notes.json` 备份，再写正文文件与资源，最后写新索引；迁移 MUST 幂等，可安全重试。

#### Scenario: 首次升级启动

- **WHEN** 用户升级后第一次启动，`notes.json` 仍为内联正文格式
- **THEN** 系统在写任何新索引前轮转现有 `notes.json` 备份（保留最近 5 份）
- **AND** 为每条条目生成 `notes/<id>.md`、抽取图片到 `assets/`、写出元数据索引
- **AND** 迁移完成后知识库条目数量、标题、标签、文件夹归属与迁移前一致

#### Scenario: 迁移中断后重试

- **WHEN** 迁移过程中应用崩溃或被关闭，下次启动再次进入迁移
- **THEN** 迁移 MUST 可重复执行且结果相同（已写正文/资源可覆盖，`assets/` 按内容去重）
- **AND** MUST NOT 产生重复条目、丢失正文或重复图片

#### Scenario: 迁移失败不毁数据

- **WHEN** 迁移写入正文或索引失败（磁盘只读、空间不足等）
- **THEN** 旧 `notes.json` MUST 保持原样可读
- **AND** 应用 MUST 给出中文错误提示，并在下次启动重试
- **AND** MUST NOT 把知识库清空或渲染成假空态

### Requirement: 迁移后的兼容与损坏语义

迁移 MUST 保持旧备份可读：旧格式 `notes.json.bak-N` 仍 MUST 出现在备份列表并可按既有恢复流程恢复；恢复旧格式备份后 MUST 自动再走一次迁移。

#### Scenario: 恢复旧格式备份

- **WHEN** 用户在设置页恢复一个旧格式（内联正文）的知识库备份
- **THEN** 系统先轮转当前 `notes.json`，再按旧格式内容替换
- **AND** 重载时自动迁移为分文件结构，条目内容与备份一致

#### Scenario: 索引损坏

- **WHEN** `notes.json` 或 `folders.json` 无法解析
- **THEN** 知识库 MUST 进入既有损坏错误通道并展示中文错误信息
- **AND** MUST NOT 展示「你还没有笔记」等假空态，MUST NOT 用空数据覆盖磁盘文件

#### Scenario: 文件夹引用失效

- **WHEN** 某条目的 `folderId` 指向不存在的文件夹
- **THEN** 条目 MUST 回退显示为未分类，且 MUST NOT 被丢弃

### Requirement: 容量边界保持

知识库条目数 MUST 仍受现有上限（5000 条）约束；达到上限时新增 MUST 被拒绝并返回中文错误。

#### Scenario: 达到条目上限

- **WHEN** 知识库已有 5000 条且用户继续新增
- **THEN** 新增被拒绝并提示条数已达上限
- **AND** 已有条目、正文与资源不受影响