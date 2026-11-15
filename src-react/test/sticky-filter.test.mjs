// 便签搜索匹配单测（OpenSpec sticky-shell-polish tasks 2.6）。
// 直接 `node src-react/test/sticky-filter.test.mjs` 运行（Windows 沙箱下 node --test
// 的子进程管道会 EPERM，沿用 kb-tree.test.mjs 的极简 runner）。
import assert from 'node:assert/strict'
import { stickyMatches } from '../src/lib/sticky-filter.ts'

let passed = 0
const failures = []

function test(name, fn) {
  try {
    fn()
    passed += 1
    console.log(`✓ ${name}`)
  } catch (error) {
    failures.push({ name, error })
    console.error(`✗ ${name}\n    ${error.message}`)
  }
}

test('空查询 / 纯空白查询视为全命中', () => {
  assert.equal(stickyMatches('', '任意正文'), true)
  assert.equal(stickyMatches('   ', '任意正文'), true)
  assert.equal(stickyMatches('', null), true)
})

test('正文命中：大小写不敏感，查询两侧空白裁剪', () => {
  assert.equal(stickyMatches('world', 'Hello WORLD'), true)
  assert.equal(stickyMatches('  WORLD  ', 'hello world'), true)
  assert.equal(stickyMatches('不存在的词', 'Hello WORLD'), false)
})

test('正文按剥掉 Markdown 后的纯文本匹配', () => {
  assert.equal(stickyMatches('周报', '- [ ] 写 **周报**'), true)
  assert.equal(stickyMatches('git push', '- `git push`'), true)
  assert.equal(stickyMatches('标题 正文', '## 标题\n\n正文'), true)
  assert.equal(stickyMatches('标题正文', '## 标题\n\n正文'), false, '跨行保留空格，不把两段粘死')
})

test('知识标题命中：大小写不敏感', () => {
  assert.equal(stickyMatches('react', '', 'React 入门指南'), true)
  assert.equal(stickyMatches(' REACT ', null, 'react 入门指南'), true)
  assert.equal(stickyMatches('vue', '', 'React 入门指南'), false)
})

test('body 为空串 / null 时仍可只靠知识标题命中', () => {
  assert.equal(stickyMatches('知识', null, '知识库条目'), true)
  assert.equal(stickyMatches('golang', '', 'Go 并发笔记'), false)
})

test('正文与标题都不命中 → false', () => {
  assert.equal(stickyMatches('测试', '随便写点', '另一个标题'), false)
})

test('kbTitle 缺省（null / undefined）时只搜正文', () => {
  assert.equal(stickyMatches('正文', '正文内容', null), true)
  assert.equal(stickyMatches('标题', '正文内容', undefined), false)
})

// ─── 汇总 ────────────────────────────────────────────────────────────────

console.log('')
if (failures.length > 0) {
  console.error(`失败 ${failures.length} 项 / 通过 ${passed} 项`)
  process.exit(1)
}
console.log(`全部通过：${passed} 项`)