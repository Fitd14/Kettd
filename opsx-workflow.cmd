@echo off
chcp 65001 >nul
echo ==============================================
echo   OpenSpec + Superpowers 混合开发流程
echo ==============================================
echo.
echo 该脚本用于启动 OpenSpec + Superpowers 混合开发流程。
echo.
echo 流程阶段：
echo   Phase 1: 规划阶段（OpenSpec 主导）
echo     - Step 1: openspec-propose    创建变更提案
echo     - Step 2: brainstorming       查漏补缺
echo     - Step 3: openspec-explore    验证方案
echo.
echo   Phase 2: 实施阶段（Superpowers 驱动）
echo     - Step 4: writing-plans       编写实施计划
echo     - Step 5: test-driven-development  TDD 实现
echo     - Step 6: systematic-debugging     问题处理
echo     - Step 7: verification-before-completion 验证完成
echo.
echo   Phase 3: 验证和归档阶段
echo     - Step 8: 人验证              功能验收
echo     - Step 9: openspec-archive-change  归档变更
echo.
echo ==============================================
echo.
set /p change_name=请输入变更名称（kebab-case）： 
echo.
echo 开始 Phase 1: 规划阶段...
echo.
echo Step 1: 创建变更提案
call openspec new change "%change_name%"
echo.
echo Step 2: 深入分析需求（使用 brainstorming 技能）
echo 提示：使用 "Use Skill: brainstorming" 进行头脑风暴
echo.
echo Step 3: 探索代码库（使用 openspec-explore 技能）
echo 提示：使用 "Use Skill: openspec-explore" 进行探索分析
echo.
echo ==============================================
echo Phase 1 完成！
echo.
echo 接下来进入 Phase 2: 实施阶段
echo 使用以下指令继续：
echo   - "Use Skill: writing-plans"
echo   - "Use Skill: test-driven-development"
echo   - "Use Skill: systematic-debugging"
echo   - "Use Skill: verification-before-completion"
echo.
echo ==============================================
pause