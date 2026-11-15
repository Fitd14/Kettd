// 便签纯文本摘要单测（OpenSpec sticky-shell-polish tasks 2.2）。
// 直接 `node src-react/test/sticky-text.test.mjs` 运行（刻意不用 node:test / --test：
// Windows 沙箱下 node --test 的子进程管道会 EPERM）。Node 22 默认开启 type stripping，
// 可以直接 import 源码 .ts。
import assert from 'node:assert/strict'
import { DEFAULT_PREVIEW_MAX, plainPreview } from '../src/lib/sticky-text.ts'

// ─── 极简测试运行器（与 kb-tree.test.mjs 同款）─────────────────────────────

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

// ─── Markdown 剥离 ───────────────────────────────────────────────────────

test('剥 Markdown：标题 / 列表 / 引用 / 粗体 / 斜体 / 行内码', () => {
  const md = '## 今日待办\n\n- **写周报**\n- `git push`\n> 引用一句\n*斜体*'
  assert.equal(plainPreview(md), '今日待办 写周报 git push 引用一句 斜体')
})

test('链接保留文字、图片保留 alt、内联 HTML 标签剥掉', () => {
  assert.equal(
    plainPreview('[文档](https://a.b/c) ![图](x.png) <b>加粗</b>'),
    '文档 图 加粗',
  )
})

test('任务清单复选框标记剥离', () => {
  assert.equal(plainPreview('- [x] 已完成的待办\n- [ ] 未完成'), '已完成的待办 未完成')
})

test('代码围栏行剥掉、围栏内正文保留', () => {
  assert.equal(plainPreview('```js\nconst x = 1\n```'), 'const x = 1')
})

test('表格：分隔行剥掉，列分隔压成空格', () => {
  assert.equal(plainPreview('| 列一 | 列二 |\n|---|:---:|\n| 甲 | 乙 |'), '列一 列二 甲 乙')
})

test('多行正文压成单行，空行不参与', () => {
  assert.equal(plainPreview('第一行\n\n\n第二行  '), '第一行 第二行')
})

test('CRLF 换行按 LF 同口径处理', () => {
  assert.equal(plainPreview('第一行\r\n第二行'), '第一行 第二行')
})

// ─── 截断 ────────────────────────────────────────────────────────────────

test('默认 120 字：超长截断并补省略号（长度 = 120 + 1）', () => {
  const text = '字'.repeat(200)
  const preview = plainPreview(text)
  assert.equal(DEFAULT_PREVIEW_MAX, 120)
  assert.equal(preview.length, DEFAULT_PREVIEW_MAX + 1)
  assert.ok(preview.endsWith('…'))
  assert.equal(preview.slice(0, DEFAULT_PREVIEW_MAX), text.slice(0, DEFAULT_PREVIEW_MAX))
})

test('恰好 120 字不截断、不加省略号', () => {
  const text = 'a'.repeat(DEFAULT_PREVIEW_MAX)
  assert.equal(plainPreview(text), text)
})

test('自定义 maxLen：按字符截断', () => {
  assert.equal(plainPreview('一二三四五六七八九十', 5), '一二三四五…')
  assert.equal(plainPreview('一二三', 5), '一二三')
})

// ─── 空值防御 ────────────────────────────────────────────────────────────

test('空 / null / undefined / 纯空白 → 空串', () => {
  assert.equal(plainPreview(''), '')
  assert.equal(plainPreview(null), '')
  assert.equal(plainPreview(undefined), '')
  assert.equal(plainPreview('   \n\t\n  '), '')
})

test('只有 Markdown 标记（无正文）→ 空串', () => {
  assert.equal(plainPreview('###\n---\n'), '')
  assert.equal(plainPreview('> \n- \n'), '')
})

// ─── 汇总 ────────────────────────────────────────────────────────────────

console.log('')
if (failures.length > 0) {
  console.error(`失败 ${failures.length} 项 / 通过 ${passed} 项`)
  process.exit(1)
}
console.log(`全部通过：${passed} 项`)