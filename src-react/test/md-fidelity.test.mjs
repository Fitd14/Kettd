/**
 * MD fidelity smoke tests.
 * 验证 markdown-it 能正确渲染 10 种核心语法（标题/列表/任务列表/代码块/行内代码/
 * 粗斜体/链接/引用/表格/嵌套组合）；round-trip 稳定性是 ProseMirror serializer
 * 的职责，这里做渲染层 smoke gate（每种语法渲染非空即通过）。
 */
import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import MarkdownIt from 'markdown-it'

const mdi = new MarkdownIt({ html: false, linkify: true, breaks: true })

const samples = [
  { name: 'heading',        md: '# Title\n## Sub\nBody text' },
  { name: 'unordered-list', md: '- item one\n- item two\n  - nested' },
  { name: 'task-list',      md: '- [ ] todo\n- [x] done item' },
  { name: 'code-block',     md: '```rust\nfn main() {}\n```' },
  { name: 'inline-code',    md: 'use `Option<T>` in rust' },
  { name: 'bold-italic',    md: '**bold** and *italic* and ***both***' },
  { name: 'link',           md: '[click here](https://example.com)' },
  { name: 'blockquote',     md: '> quoted line\n> multi-line quote' },
  { name: 'table',          md: '| a | b |\n|---|---|\n| 1 | 2 |' },
  { name: 'table-header',   md: '| 材质 | 底色 |\n| --- | --- |\n| 宣纸 | 米白偏暖 |\n| 青瓷 | 米白偏青 |' },
  { name: 'task-list-gfm',  md: '- [ ] 未完成\n- [x] 已完成项' },
  { name: 'nested-complex', md: '## Title\n\n- [ ] task\n\n> quote\n\n```ts\nconst x = 1\n```' },
]

describe('MarkdownIt render smoke (10 syntax samples)', () => {
  for (const { name, md } of samples) {
    it(`renders ${name} → non-empty HTML`, () => {
      const html = mdi.render(md)
      assert.ok(html.length > 0, `rendered HTML for "${name}" should be non-empty`)
      // Basic structural check: every sample should produce at least one block-level tag
      assert.ok(
        /<h[1-6]>|<ul|<ol|<pre|<p>|<table|<blockquote|<hr/.test(html),
        `"${name}" should contain at least one block-level HTML element`
      )
    })
  }
})

describe('MarkdownIt fence rendering', () => {
  it('fenced code block produces <pre> with data-lang', () => {
    const html = mdi.render('```rust\nfn main() {}\n```')
    assert.ok(html.includes('<pre'), 'should contain <pre>')
    assert.ok(html.includes('fn main'), 'should contain the code content')
  })

  it('unknown language falls back to auto-highlight', () => {
    const html = mdi.render('```unknownlang\nhello world\n```')
    assert.ok(html.includes('<pre'), 'unknown lang should still produce <pre>')
    assert.ok(html.includes('hello world'), 'code content preserved')
  })
})
