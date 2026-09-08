# 设计规格增量 · 多便签（Multi-Sticky · roadmap H1）

- 状态：**已定稿实施** · 2026-09-08
- 拍板记录：①范围=两类全做 ②待办多张=分页镜像（第 N 张显示第 N×7 条） ③上限 6 张
- 实施落点：models `StickyNote`/`AppData.stickies`/`STICKY_CAP=6`；store CRUD；runtime `open_note_window`+Moved 泛化+启动恢复；commands `create/list/update/delete/sticky_self`（命令总数 54）+ 事件 sticky_create/close/delete/sticky_free_edit/sticky_count；设置页新建+清单
- H1b 留账：托盘新建入口（设置页入口已覆盖高频路径）、float 窗并入全动态注册表
- 关联：ADR-0007（动态便签窗口注册表，Accepted）· `sticky-note-component-spec.md`（单张定稿）· roadmap H1 · stories sticky-m1..m5
- 边界：本增量只管「多张化」；单张的视觉/交互语言沿用便签规格 §5 定稿，不重述

---

## 0. 一句话

把便签从「一张」变成「一叠」：桌面可贴 N 张，两类——**待办便签**（镜像今天 + 钉单条）与**自由便签**（随手写一张纸）；全部走 ADR-0007 动态窗口注册表，托盘/设置可管理，埋点回答 roadmap H1 验证门。

> 数据门说明：roadmap 原建议「等 sticky_pin 数据成立再投」；owner 拍板设计先行——**验证门语义保留**，从「投入门」转为「发布后验证」（NSM/钉动作照常观测，H2 知识网投入仍受门约束）。

## 1. 架构（ADR-0007 落地，capture 模板参数化）

| 项 | 方案 |
| --- | --- |
| 窗口 label | `note:<id8>` 动态生成；`NoteRegistry { by_label: HashMap<label, NoteId> }` 住 `ui/notes.rs` |
| 创建 | 运行时 `WindowBuilder`（照 capture 窗模板）：`decorations:false` + `transparent:true` + `skip_taskbar:true` + `resizable:false` + 380×456；`alwaysOnTop` 随每张的 pinned |
| 位置 | `runtime.json note_pos` map（**字段已预留**）：noteId → [x,y]；离屏守卫沿用 |
| 事件处理 | `main.rs on_window_event` 按 `label.starts_with("note:")` 分支，不再逐 label 匹配 |
| 调度器 | blur 轮询遍历 registry，不再点名 float |
| **迁移兼容** | 现有 conf 声明的 `float` 窗保留为 **1 号待办便签**（registry 登记 id=`sticky`，note_pos 现有键不动）——H1a 最小扰动；H1b 再统一为全动态（另开 ADR） |

## 2. 两类便签

### 待办便签（现有能力多张化）
- 内容 = 镜像今天 Top7 + 各自钉单条（沿用）
- **多张语义 = 分页镜像**（拍板项①）：第 2 张显示第 8-14 条（`todayList().slice(7,14)`），第 3 张 15-21……今天清空时全部显示空态。贴 2-3 张 = 今天清单铺满桌面，而不是同内容复制

### 自由便签（新增）
- 一张空白纸：点击纸面即编辑（contentEditable 单段纯文本，≤500 字），失焦自动保存
- 不进今天列表、不参与提醒、不参与任何统计——「它就是一张贴着的纸」
- 存储：新文件 **`stickies.json`**（ADR-0005 层次：用户内容不进 runtime.json）：`[{ id, content, created_at, updated_at }]`；首启惰性建文件；零丢失口径不变
- 删除 = 二次确认后销毁（内容不可恢复，区别于待办软删）

## 3. 生命周期与入口

| 动作 | 行为 |
| --- | --- |
| 新建 | 设置便签卡「新建便签 ▾（待办/自由）」+ 托盘子菜单同名——两入口同命令 `create_sticky(type)` |
| 关闭（✕） | **收起**（hide + registry 保留）：清单可再展开；待办便签收起不影响数据 |
| 删除 | 仅清单里提供：待办便签删除=移除窗口（数据无损）；自由便签删除=二次确认后销毁 |
| 上限 | 总数 ≤ **6 张**；达上限时新建按钮就地提示「先收起或删除一张」（无系统通知） |
| 清理建议 | 自由便签 14 天未编辑且未钉 → 清单给「建议清理」标记（可忽略，仅提示） |

## 4. 可视化

```
桌面
├ note:sticky   待办便签①（conf 迁移兼容）─ 左上
├ note:ab12     待办便签②（第 8-14 条分页）─ 右上
├ note:c3d9     自由便签「his 接口字段备忘」─ 左下
└ note:e7f1     自由便签（空）──────── 右下
设置 → 便签卡：[新建便签 ▾] + 清单（4 行：类型/摘要/显示/删除）
```

## 5. 分阶段

- **H1a（本迭代）**：Registry + N 张待办便签（分页镜像）+ 自由便签 + 清单 + 上限 + 埋点
- **H1b（后续）**：float 窗并入全动态注册表、blur 轮询统一（另开 ADR 记录）

## 6. 验收标准（汇总 stories AC）

1. N 张并存、独立记位、重启各回原位；互不影响
2. 自由便签点击即写、失焦保存、重启还在；不进今天不提醒
3. 清单实时一致；删除自由便签有确认；收起可展开
4. 第 7 张被温和拦下；14 天未动自由便签出现清理建议
5. 外观（纸色/花纹/淡化）全局生效于所有便签
6. 旧单张便签升级零感知（位置、置顶、纸色、花纹全保留）

## 7. 埋点事件表（metric 蓝图 H1 门专用）

| 事件 | 触发 | 字段 | 通道 |
| --- | --- | --- | --- |
| `sticky_create` | 新建成功（Rust create path） | `type: todo/free` | Rust 直记 |
| `sticky_close` | 用户收起该张 | `type` | Rust 直记 |
| `sticky_delete` | 删除该张（自由=确认后） | `type` | Rust 直记 |
| `sticky_free_edit` | 自由便签保存且内容有变化 | `len_bucket: 0/1-50/51-200/200+` | 前端 trackEvent |
| `sticky_count` | app_launch 快照 | `count`, `free_count` | app_launch props 扩展 |
| `sticky_pin` | 钉/取下单条（已有） | `action` | update_task 分支 |

→ 回答 H1 门：「多张便签是否真被用」（create/close/count）+「钉有没有发生」（已有）+ 自由便签是否真被写（free_edit 字数桶）。

## 8. 工时估算

Registry + 窗口生命周期 ~4h ｜ 自由便签存储+编辑 ~4h ｜ 清单 UI ~2h ｜ 上限/清理 ~1.5h ｜ 埋点 ~0.5h ≈ **12h**

## 9. 待拍板

- [ ] ① 范围：两类全做 vs 先只做 N 张待办便签（自由便签下迭代）
- [ ] ② 待办便签多张语义：分页镜像（推荐）vs 同内容复制
- [ ] ③ 上限张数：6？
