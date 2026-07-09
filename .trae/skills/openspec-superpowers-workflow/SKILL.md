---
name: openspec-superpowers-workflow
description: Use when starting a new development change that combines OpenSpec structured planning with Superpowers skills for implementation quality
---

# OpenSpec + Superpowers 混合开发流程

## 概述

将 OpenSpec 的结构化变更管理与 Superpowers 的质量保证技能相结合，形成一套完整的开发流程：

- **OpenSpec 负责规划**：定义"做什么"和"为什么"
- **Superpowers 负责实施**：确保"怎么做"的质量和效率
- **人负责验证**：确认功能符合预期
- **OpenSpec 负责归档**：完成变更闭环

## 流程阶段

### Phase 1: 规划阶段（OpenSpec 主导）

```
┌─────────────────────────────────────────────────────────────────┐
│                    Phase 1: 规划阶段                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Step 1: OpenSpec Propose                                      │
│   ├─ 创建 proposal.md（变更提案）                                │
│   ├─ 创建 design.md（技术设计）                                 │
│   ├─ 创建 specs/（能力规格）                                    │
│   └─ 创建 tasks.md（任务列表）                                  │
│           │                                                     │
│   Step 2: brainstorming（查漏补缺）                             │
│   ├─ 深入分析需求                                               │
│   ├─ 细化设计方案                                               │
│   └─ 补充遗漏的任务                                             │
│           │                                                     │
│   Step 3: OpenSpec Explore（验证方案）                          │
│   ├─ 探索代码库                                                 │
│   ├─ 识别集成点                                                 │
│   └─ 验证技术可行性                                             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 2: 实施阶段（Superpowers 驱动）

```
┌─────────────────────────────────────────────────────────────────┐
│                    Phase 2: 实施阶段                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Step 4: writing-plans（编写实施计划）                          │
│   └─ 为每个任务编写详细实施步骤                                  │
│           │                                                     │
│   Step 5: test-driven-development（TDD 实现）                   │
│   ├─ 编写测试用例                                               │
│   ├─ 实现代码直到测试通过                                        │
│   └─ 重构优化                                                   │
│           │                                                     │
│   Step 6: systematic-debugging（问题处理）                      │
│   └─ 处理实现过程中的 Bug 和问题                                 │
│           │                                                     │
│   Step 7: verification-before-completion（验证完成）            │
│   ├─ 运行测试套件                                               │
│   ├─ 验证代码质量                                               │
│   └─ 更新 tasks.md 标记完成                                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Phase 3: 验证和归档阶段

```
┌─────────────────────────────────────────────────────────────────┐
│                  Phase 3: 验证和归档阶段                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Step 8: 人验证（功能验收）                                     │
│   ├─ 手动测试功能                                               │
│   └─ 确认符合预期                                               │
│           │                                                     │
│   Step 9: OpenSpec Archive（归档变更）                          │
│   ├─ 同步增量规格到主规格                                       │
│   ├─ 归档变更记录                                               │
│   └─ 提交代码到远程仓库                                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## 详细步骤

### Step 1: OpenSpec Propose

**指令**: `Use Skill: openspec-propose`

创建变更的基础 artifacts：
- **proposal.md** - 描述变更的目的、范围和价值
- **design.md** - 技术设计文档
- **specs/** - 各能力的详细规格
- **tasks.md** - 可追踪的实现任务列表

### Step 2: brainstorming

**指令**: `Use Skill: brainstorming`

对 OpenSpec 创建的 artifacts 进行查漏补缺：
- 深入分析需求的完整性
- 细化技术设计方案
- 补充遗漏的实现任务
- 识别潜在的风险和问题

### Step 3: OpenSpec Explore

**指令**: `Use Skill: openspec-explore`

验证方案的可行性：
- 探索代码库，了解现有架构
- 识别集成点和依赖关系
- 验证技术选型的合理性
- 更新 artifacts 记录发现

### Step 4: writing-plans

**指令**: `Use Skill: writing-plans`

为每个任务编写详细的实施计划：
- 将大任务分解为小步骤
- 定义每个步骤的目标和验证标准
- 考虑依赖关系和顺序

### Step 5: test-driven-development

**指令**: `Use Skill: test-driven-development`

按照 TDD 方式实现每个任务：
1. 编写失败的测试用例
2. 编写最小实现代码使测试通过
3. 重构优化代码

### Step 6: systematic-debugging

**指令**: `Use Skill: systematic-debugging`

处理实现过程中的问题：
- 遇到 Bug 时立即调用
- 遵循科学调试流程（假设 → 验证 → 修复）
- 记录问题和解决方案

### Step 7: verification-before-completion

**指令**: `Use Skill: verification-before-completion`

验证每个任务的完成质量：
- 运行测试套件
- 检查代码格式
- 确认功能正常
- 更新 tasks.md 标记完成

### Step 8: 人验证

用户手动验证：
- 测试核心功能
- 确认符合需求
- 提供反馈

### Step 9: OpenSpec Archive

**指令**: `Use Skill: openspec-archive-change`

完成变更的归档：
- 同步增量规格到主规格
- 归档变更记录到 archive 目录
- 提交代码到远程仓库

## 技能调用速查表

| 阶段 | 技能 | 指令 |
|------|------|------|
| 规划 | openspec-propose | `Use Skill: openspec-propose` |
| 规划 | brainstorming | `Use Skill: brainstorming` |
| 规划 | openspec-explore | `Use Skill: openspec-explore` |
| 实施 | writing-plans | `Use Skill: writing-plans` |
| 实施 | test-driven-development | `Use Skill: test-driven-development` |
| 实施 | systematic-debugging | `Use Skill: systematic-debugging` |
| 实施 | verification-before-completion | `Use Skill: verification-before-completion` |
| 归档 | openspec-archive-change | `Use Skill: openspec-archive-change` |

## 关键原则

1. **OpenSpec 定义边界** - proposal 和 specs 定义功能的边界和验收标准
2. **Superpowers 保证质量** - TDD 和 systematic-debugging 确保代码质量
3. **验证贯穿始终** - 每个阶段都有明确的验证点
4. **可追溯性** - 所有变更都有完整的 artifacts 记录
5. **灵活调整** - 实施过程中发现问题可以回到探索阶段

## 适用场景

- ✅ **新功能开发** - 从需求分析到实现的完整流程
- ✅ **架构重构** - 需要详细设计和验证的大规模变更
- ✅ **Bug 修复** - 需要系统化分析和验证的复杂问题
- ❌ **简单配置变更** - 不需要完整流程的小型修改

## 常见错误

| 错误 | 后果 | 修正 |
|------|------|------|
| 跳过规划直接编码 | 需求不清晰，返工率高 | 强制执行 Step 1-3 |
| 跳过 TDD | 代码质量差，难以维护 | 使用 test-driven-development |
| 跳过验证 | 质量问题流入生产 | 使用 verification-before-completion |
| 过早归档 | 变更未完成 | 确保所有 tasks 完成 |

## 相关技能

**REQUIRED SUB-SKILLS:**
- `openspec-propose` - 创建变更提案
- `openspec-explore` - 探索分析
- `openspec-archive-change` - 归档变更
- `brainstorming` - 头脑风暴
- `writing-plans` - 编写计划
- `test-driven-development` - 测试驱动开发
- `systematic-debugging` - 系统化调试
- `verification-before-completion` - 完成前验证
