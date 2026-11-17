// Markdown → 导出块模型测试（tasks 1.2）。
// 直接 `node src-react/test/docx-blocks.test.mjs` 运行（node --test 在 Windows 沙箱会 EPERM）。
import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import {
  DOCX_MARKDOWN_OPTIONS,
  sanitizeDocxName,
  toDocxBlocks,
} from '../src/lib/docx-blocks.ts'

const here = dirname(fileURLToPath(import.meta.url))

describe('块模型：配置漂移守卫', () => {
  it('与 rendering/markdown.ts 的 MarkdownIt 开关一致', () => {
    const source = readFileSync(join(here, '../src/rendering/markdown.ts'), 'utf8')
    for (const [key, value] of Object.entries(DOCX_MARKDOWN_OPTIONS)) {
      assert.match(
        source,
        new RegExp(`${key}:\\s*${String(value)}`),
        `rendering/markdown.ts 里应有 ${key}: ${String(value)}`,
      )
    }
  })
})

describe('块模型：标题', () => {
  it('h1–h3 映射标题层级，h4 起降级为加粗段落', () => {
    const blocks = toDocxBlocks('# 一级\n\n## 二级\n\n### 三级\n\n#### 四级')
    const headings = blocks.filter((b) => b.kind === 'heading')
    assert.deepEqual(headings.map((b) => b.level), [1, 2, 3])
    assert.deepEqual(headings.map((b) => b.runs[0].text), ['一级', '二级', '三级'])

    const fallback = blocks.find((b) => b.kind === 'paragraph')
    assert.ok(fallback)
    assert.equal(fallback.runs[0].text, '四级')
    assert.equal(fallback.runs[0].bold, true)
  })
})

describe('块模型：行内样式', () => {
  it('加粗/斜体/行内代码/链接各就各位', () => {
    const blocks = toDocxBlocks('**粗** *斜* `码` [链](https://example.com) 普通')
    assert.equal(blocks.length, 1)
    const runs = blocks[0].runs
    assert.equal(runs.find((r) => r.text === '粗')?.bold, true)
    assert.equal(runs.find((r) => r.text === '斜')?.italic, true)
    assert.equal(runs.find((r) => r.text === '码')?.code, true)
    assert.equal(runs.find((r) => r.text === '链')?.link, 'https://example.com')
    assert.equal(runs.find((r) => r.text.startsWith('普通'))?.bold, undefined)
  })

  it('行内图片降级成占位文字，不丢位置', () => {
    const blocks = toDocxBlocks('说明 ![](assets/a.png) 结束')
    const text = blocks[0].runs.map((r) => r.text).join('')
    assert.ok(text.includes('[图片]'))
  })
})

describe('块模型：列表', () => {
  it('无序列表保留缩进深度', () => {
    const blocks = toDocxBlocks('- a\n  - b\n- c')
    assert.equal(blocks[0].kind, 'bullets')
    assert.deepEqual(blocks[0].items.map((i) => i.depth), [0, 1, 0])
    assert.deepEqual(blocks[0].items.map((i) => i.runs[0].text), ['a', 'b', 'c'])
  })

  it('有序列表用 numbers', () => {
    const blocks = toDocxBlocks('1. 一\n2. 二')
    assert.equal(blocks[0].kind, 'numbers')
    assert.equal(blocks[0].items.length, 2)
  })

  it('任务列表退化为字面方括号（spec 已声明的已知不保真）', () => {
    const blocks = toDocxBlocks('- [ ] todo\n- [x] done')
    assert.equal(blocks[0].kind, 'bullets')
    const texts = blocks[0].items.map((i) => i.runs.map((r) => r.text).join(''))
    assert.deepEqual(texts, ['[ ] todo', '[x] done'])
  })
})

describe('块模型：引用', () => {
  it('一层引用装块', () => {
    const blocks = toDocxBlocks('> 引用一行')
    assert.equal(blocks[0].kind, 'quote')
    assert.equal(blocks[0].blocks[0].kind, 'paragraph')
  })

  it('三层引用拍平到第二层', () => {
    const blocks = toDocxBlocks('> > > 三层')
    const count = (list) =>
      list.reduce(
        (sum, b) => sum + (b.kind === 'quote' ? 1 + count(b.blocks) : 0),
        0,
      )
    assert.equal(count(blocks), 2, '只保留两层 quote')

    const inner = blocks[0].blocks.find((b) => b.kind === 'quote')
    assert.ok(inner, '第二层应仍是 quote')
    assert.equal(inner.blocks[0].kind, 'paragraph')
    assert.equal(inner.blocks[0].runs[0].text, '三层')
  })

  it('同级的嵌套引用各成一块（markdown-it 的兄弟结构不被压平）', () => {
    const blocks = toDocxBlocks('> 一\n>\n> > 二\n>\n> > > 三')
    const quotes = blocks[0].blocks.filter((b) => b.kind === 'quote')
    assert.equal(quotes.length, 2, '第二层与第三层的兄弟引用各一块')
  })
})

describe('块模型：代码与表格', () => {
  it('围栏代码块保留语言与原文', () => {
    const blocks = toDocxBlocks('```ts\nconst x = 1\n\nconst y = 2\n```')
    assert.equal(blocks[0].kind, 'code')
    assert.equal(blocks[0].lang, 'ts')
    assert.equal(blocks[0].text, 'const x = 1\n\nconst y = 2')
  })

  it('表格识别表头与行列', () => {
    const blocks = toDocxBlocks('| 材质 | 底色 |\n| --- | --- |\n| 宣纸 | 米白 |\n| 青瓷 | 青白 |')
    assert.equal(blocks[0].kind, 'table')
    assert.equal(blocks[0].header, true)
    assert.deepEqual(blocks[0].rows, [
      ['材质', '底色'],
      ['宣纸', '米白'],
      ['青瓷', '青白'],
    ])
  })

  it('单元格内的强调只保留文字', () => {
    const blocks = toDocxBlocks('| a |\n| --- |\n| **粗** |')
    assert.deepEqual(blocks[0].rows, [['a'], ['粗']])
  })
})

describe('块模型：图片与空输入', () => {
  it('独占一段的 assets 图片变成 image 块', () => {
    const blocks = toDocxBlocks('![](assets/deadbeef.png)')
    assert.equal(blocks.length, 1)
    assert.equal(blocks[0].kind, 'image')
    assert.equal(blocks[0].asset, 'assets/deadbeef.png')
  })

  it('空正文产出空数组', () => {
    assert.deepEqual(toDocxBlocks(''), [])
    assert.deepEqual(toDocxBlocks('   \n\n  '), [])
  })
})

describe('文件名清洗', () => {
  it('去掉 Windows 非法字符并折叠空白', () => {
    assert.equal(sanitizeDocxName('周报/2026: 第三季度'), '周报 2026 第三季度')
    assert.equal(sanitizeDocxName('  a   b  '), 'a b')
  })

  it('空标题与纯符号回落到占位名', () => {
    assert.equal(sanitizeDocxName(''), '未命名文档')
    assert.equal(sanitizeDocxName('///'), '未命名文档')
  })

  it('保留中文与长度上限', () => {
    const long = '字'.repeat(120)
    assert.equal(sanitizeDocxName(long).length, 80)
    assert.equal(sanitizeDocxName('读书笔记'), '读书笔记')
  })
})