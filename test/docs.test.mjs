import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/** 文档职责契约：README 说事实（架构/命令/门禁/目录），AGENTS 说规范与验证。
 *  两边各写一份就会漂移——这份门禁只钉结构，不钉措辞。 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

const readme = read('README.md');
const agents = read('AGENTS.md');

const README_SECTIONS = [
  '## 技术架构',
  '### 技术栈',
  '### 分层与数据流',
  '### 命令契约',
  '### 目录结构',
  '### 测试与门禁',
  '### 发布打包',
  '### 依赖维护',
];

const AGENTS_SECTIONS = ['## 2. 硬约束', '## 3. 历史陷阱', '## 4. 验证协议', '## 5. 分支与提交'];

test('README 承载项目事实：架构 / 命令 / 门禁 / 目录 / 打包', () => {
  const missing = README_SECTIONS.filter((s) => !readme.includes(`\n${s}`) && !readme.startsWith(s));
  assert.deepEqual(missing, [], `README.md 缺章节：${missing.join(' / ')}`);
});

test('AGENTS 只承载规范：硬约束 / 陷阱 / 验证协议 / 分支提交', () => {
  const missing = AGENTS_SECTIONS.filter((s) => !agents.includes(`\n${s}`) && !agents.startsWith(s));
  assert.deepEqual(missing, [], `AGENTS.md 缺章节：${missing.join(' / ')}`);
});

test('AGENTS 不放命令清单（命令只许留在 README）', () => {
  const fences = agents.match(/```(bash|powershell|pwsh|sh)\b/g) ?? [];
  assert.deepEqual(
    fences,
    [],
    `AGENTS.md 出现 ${fences.length} 个命令代码块；命令清单应在 README，AGENTS 只写规则`,
  );
});

test('两份文档互相链接，避免各自变成孤岛', () => {
  assert.ok(/\]\(README\.md/.test(agents), 'AGENTS.md 必须链接 README.md');
  assert.ok(/\]\(AGENTS\.md/.test(readme), 'README.md 必须链接 AGENTS.md');
});
