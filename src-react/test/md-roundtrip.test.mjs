/**
 * Markdown 保存保真 round-trip 门禁（OpenSpec fix-known-defects-iteration #10 / tasks.md 6.1）
 *
 * 验证链路：markdown → 编辑器栈解析 → storage.markdown.getMarkdown() → 再解析 → 再 getMarkdown()。
 *   ① 第二次序列化 === 第一次（幂等；对应「二次保存稳定」）
 *   ② 文档文本内容不丢（doc1.textContent === doc2.textContent）
 *   ③ 关键能力保留（表格对齐 / 转义竖线 / 硬换行反斜杠 / 任务列表 / 自动链接等）
 *
 * 运行：
 *   node src-react/test/md-roundtrip.test.mjs            # 门禁模式
 *   node src-react/test/md-roundtrip.test.mjs --report   # 额外输出逐 fixture 报告（生成基线用）
 *
 * 环境约束（见任务说明）：
 * - 不使用 `node --test`（Windows 沙箱 spawn 子进程会 EPERM），本文件自带极简 runner，直接 node 执行。
 * - jsdom 目前只是 src-react/node_modules 里的传递依赖（未写入 package.json，禁止新增依赖）；
 *   先动态 `import('jsdom')`，缺失时打印明确 SKIP 原因并 exit 0。
 * - 必须先建好 DOM globals，再动态 import @tiptap 模块（不能在文件顶部静态导入 @tiptap）。
 * - `src/rendering/table-alignment.ts` 存在 → 走修复后扩展栈；不存在 → 回退原生 `@tiptap/extension-table`
 *   的 Table（修复前基线，--report 用于记录「修复前」差异）。
 * - 图片能力检测：优先装本仓库自研 `image-extension.ts`（#15，#10/#15 共用同一节点），
 *   回退 @tiptap/extension-image；都没有则记为已知限制（图片整体丢失）。
 */

import { strict as assert } from 'node:assert'

const REPORT = process.argv.includes('--report')

// ---------------------------------------------------------------------------
// 0. jsdom 加载（缺失 → SKIP）
// ---------------------------------------------------------------------------

let JSDOM
try {
  ;({ JSDOM } = await import('jsdom'))
} catch (error) {
  console.log('[SKIP] jsdom 不可用，跳过 Markdown round-trip 门禁。')
  console.log(`       原因：${error && error.message ? error.message.split('\n')[0] : String(error)}`)
  console.log('       jsdom 是 src-react/node_modules 中的传递依赖（未声明在 package.json，按约束不新增依赖）。')
  console.log('       需要时可在有 jsdom 的环境（例如已装依赖的开发者机器）运行本脚本。')
  process.exit(0)
}

// ---------------------------------------------------------------------------
// 1. 建立 DOM globals（必须先于任何 @tiptap 模块的动态 import）
// ---------------------------------------------------------------------------

const dom = new JSDOM('<!doctype html><html><body></body></html>', {
  url: 'https://kettd.test/',
  pretendToBeVisual: true,
})
const { window } = dom

function setGlobal(name, value) {
  try {
    Object.defineProperty(globalThis, name, { value, configurable: true, writable: true })
  } catch {
    // Node 原生只读全局（例如较新版本的 navigator）保留原实现即可
  }
}

setGlobal('window', window)
setGlobal('document', window.document)
setGlobal('navigator', window.navigator)
setGlobal('getComputedStyle', window.getComputedStyle.bind(window))
for (const name of [
  'HTMLElement',
  'HTMLDivElement',
  'HTMLBodyElement',
  'HTMLDocument',
  'HTMLSpanElement',
  'HTMLParagraphElement',
  'Element',
  'Node',
  'Text',
  'DocumentFragment',
  'DOMParser',
  'MutationObserver',
  'NodeFilter',
  'XMLSerializer',
  'Selection',
  'Range',
  'Event',
  'KeyboardEvent',
  'MouseEvent',
  'CustomEvent',
  'InputEvent',
  'ClipboardEvent',
  'requestAnimationFrame',
  'cancelAnimationFrame',
]) {
  if (window[name] !== undefined) setGlobal(name, window[name])
}
// jsdom 没有 ResizeObserver；tiptap 编辑器未挂载视图时不会真正使用它，补一个空实现兜底
if (typeof globalThis.ResizeObserver === 'undefined') {
  setGlobal('ResizeObserver', class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  })
}

// ---------------------------------------------------------------------------
// 2. 动态加载编辑器栈（镜像 tiptap-editor.tsx 中与 Markdown 保存相关的扩展）
// ---------------------------------------------------------------------------

const { Editor } = await import('@tiptap/core')
const { StarterKit } = await import('@tiptap/starter-kit')
const { CodeBlockLowlight } = await import('@tiptap/extension-code-block-lowlight')
const { TaskList } = await import('@tiptap/extension-task-list')
const { TaskItem } = await import('@tiptap/extension-task-item')
const { Placeholder } = await import('@tiptap/extension-placeholder')
const { Table, TableRow, TableCell, TableHeader } = await import('@tiptap/extension-table')
const { Markdown } = await import('tiptap-markdown')
const { common, createLowlight } = await import('lowlight')

const lowlight = createLowlight(common)

/** 图片扩展：优先用本仓库自研 Image 扩展（#15，无第三方依赖）；回退 @tiptap/extension-image。 */
let Image = null
let imageSource = 'none'
try {
  const mod = await import('../src/rendering/image-extension.ts')
  if (mod && mod.ImageExtension) {
    Image = mod.ImageExtension
    imageSource = 'custom'
  }
} catch {
  // 自研扩展不可用，尝试官方包
}
if (!Image) {
  try {
    ;({ Image } = await import('@tiptap/extension-image'))
    imageSource = 'official'
  } catch {
    Image = null
  }
}

/** 表格扩展：存在修复后的 table-alignment.ts → 自定义序列化；否则回退原生 Table（修复前基线）。 */
let TableExtension = Table
let tableMode = 'vanilla'
try {
  const mod = await import('../src/rendering/table-alignment.ts')
  if (mod && mod.TableWithAlignment) {
    TableExtension = mod.TableWithAlignment
    tableMode = 'table-alignment'
  }
} catch {
  // 修复前基线模式
}

function buildExtensions() {
  const extensions = [
    StarterKit.configure({ codeBlock: false }),
    CodeBlockLowlight.configure({ lowlight }),
    TaskList,
    TaskItem.configure({ nested: true }),
    TableExtension.configure({ resizable: false }),
    TableRow,
    TableHeader,
    TableCell,
    Placeholder.configure({ placeholder: '写点什么…' }),
    Markdown.configure({ html: false, linkify: true, breaks: true }),
  ]
  if (Image) extensions.push(Image)
  return extensions
}

let editorCounter = 0

function createEditor() {
  const element = document.createElement('div')
  element.id = `md-roundtrip-editor-${editorCounter++}`
  document.body.appendChild(element)
  return new Editor({
    element,
    extensions: buildExtensions(),
    content: '',
  })
}

// ---------------------------------------------------------------------------
// 3. Round-trip 核心
// ---------------------------------------------------------------------------

/** 复刻 Rust 侧 `normalize_body_md`（只裁首尾 \n / \r，保留行尾空格）。 */
function normalizeBodyMd(value) {
  return value.replace(/^[\n\r]+|[\n\r]+$/g, '')
}

/**
 * 用编辑器栈跑两轮：md → doc1 → out1 → doc2 → out2。
 * 同时记录每轮 doc 的 textContent 与表格对齐信息。
 */
function roundtrip(editor, markdown) {
  editor.commands.setContent(markdown, { emitUpdate: false })
  const doc1 = editor.state.doc
  const out1 = normalizeBodyMd(editor.storage.markdown.getMarkdown())

  editor.commands.setContent(out1, { emitUpdate: false })
  const doc2 = editor.state.doc
  const out2 = normalizeBodyMd(editor.storage.markdown.getMarkdown())

  return { doc1, doc2, out1, out2 }
}

/** 收集文档中第一个表格的形状与对齐 attr（行 × 列；表格序列化门禁用）。 */
function collectTableAligns(doc) {
  let rows = []
  doc.descendants((node) => {
    if (node.type.name !== 'table') return true
    node.forEach((row) => {
      const cells = []
      row.forEach((cell) => cells.push(cell.attrs && cell.attrs.align ? cell.attrs.align : null))
      rows.push(cells)
    })
    return false
  })
  return rows
}

function firstLineMatching(text, regex) {
  return text.split('\n').find((line) => regex.test(line))
}

// ---------------------------------------------------------------------------
// 4. Fixtures
// ---------------------------------------------------------------------------

const FIXTURES = [
  { name: 'bullet-star', md: '* 苹果\n* 香蕉', expect: { includes: ['- 苹果'] } },
  { name: 'bullet-plus', md: '+ 苹果\n+ 香蕉', expect: { includes: ['- 苹果'] } },
  { name: 'ordered-paren', md: '1) 甲\n2) 乙', expect: { includes: ['1. 甲'] } },
  { name: 'setext-h1', md: '一级标题\n===', expect: { startsWith: '# 一级标题' } },
  { name: 'setext-h2', md: '二级标题\n---', expect: { startsWith: '## 二级标题' } },
  { name: 'fence-tilde', md: '~~~js\nconst n = 1\n~~~', expect: { includes: ['```js'] } },
  {
    name: 'table-align',
    md: '| 左对齐 | 居中 | 右对齐 |\n| :--- | :---: | ---: |\n| a | b | c |\n| 甲 | 乙 | 丙 |',
    expect: {
      tableAligns: [
        ['left', 'center', 'right'],
        ['left', 'center', 'right'],
        ['left', 'center', 'right'],
      ],
      delimiter: /^\| :--- \| :---: \| ---: \|$/,
    },
  },
  {
    name: 'table-plain',
    md: '| 列1 | 列2 |\n| --- | --- |\n| 1 | 2 |',
    expect: { tableAligns: [[null, null], [null, null]] },
  },
  {
    name: 'table-escaped-pipe',
    md: '| a | b |\n| --- | --- |\n| x \\| y | z |',
    expect: { includes: ['\\|'], textIncludes: ['x | y'] },
  },
  {
    name: 'table-code-pipe',
    md: '| 代码 | 说明 |\n| --- | --- |\n| `a \\| b` | 含竖线 |',
    expect: { includes: ['\\|'], textIncludes: ['a | b'] },
  },
  {
    // 回归：粗体 mark 跨转义竖线。tiptap-markdown 的延迟 mark trim 下标会因补 `\` 而错位，
    // 曾输出 `**a \|** *`（非幂等且丢内容）；修复后必须原样往返。
    name: 'table-bold-escaped-pipe',
    md: '| 粗体 | 其他 |\n| :--- | ---: |\n| **a \\| b** | c |',
    expect: {
      includes: ['**a \\| b**'],
      textIncludes: ['a | b'],
      tableAligns: [
        ['left', 'right'],
        ['left', 'right'],
      ],
    },
  },
  { name: 'hardbreak-two-spaces', md: '第一行  \n第二行', expect: { includes: ['\\\n'] } },
  { name: 'hardbreak-backslash', md: '第一行\\\n第二行', expect: { includes: ['\\\n'] } },
  { name: 'bare-url', md: '访问 https://example.com 查看', expect: { includes: ['<https://example.com>'] } },
  {
    name: 'task-list-loose',
    md: '- [ ] 未完成\n\n- [x] 已完成\n\n  - [ ] 嵌套',
    expect: { includes: ['- [ ] 未完成', '- [x] 已完成', '- [ ] 嵌套'] },
  },
  { name: 'image', md: '![示意图](https://example.com/a.png)', expect: { image: true } },
  {
    // #15 截图链路：图片以 data URL 存进 bodyMd，必须能原样往返（不丢、不被二次转义）
    name: 'image-data-url',
    md: '截图：![截图](data:image/webp;base64,UklGRg==)',
    expect: { image: true, includes: ['![截图](data:image/webp;base64,UklGRg==)'] },
  },
  {
    name: 'inline-html',
    md: '这是 <b>粗体</b> 与 <span class="x">span</span> 文本',
    expect: { textIncludes: ['<b>粗体</b>', '<span class="x">span</span>'] },
  },
  {
    name: 'mixed-doc',
    md: '# 标题\n\n* 列表项\n\n| a | b |\n| :--- | ---: |\n| 1 | 2 |\n\n```ts\nconst a = 1\n```\n\n[链接](https://example.com)',
    expect: { includes: ['# 标题', '```ts', '[链接](https://example.com)'] },
  },
]

// ---------------------------------------------------------------------------
// 5. Runner（不用 node:test）
// ---------------------------------------------------------------------------

const reportRows = []
const failures = []
const knownLimitationNotes = []

function escapeForLog(value) {
  return JSON.stringify(value)
}

for (const fixture of FIXTURES) {
  const editor = createEditor()
  let result
  try {
    result = roundtrip(editor, fixture.md)
  } catch (error) {
    failures.push(`[${fixture.name}] round-trip 抛错：${error && error.stack ? error.stack : String(error)}`)
    reportRows.push({ name: fixture.name, error: String(error) })
    editor.destroy()
    continue
  } finally {
    if (editor) editor.destroy()
  }

  const { doc1, doc2, out1, out2 } = result
  const localFailures = []
  const run = (label, fn) => {
    try {
      fn()
    } catch (error) {
      localFailures.push(`[${fixture.name}] ${label}: ${error.message}`)
    }
  }

  // ---- 通用断言 ----
  if (fixture.expect.image && !Image) {
    // 图片扩展未安装：内容丢失属于当前已知限制，恢复图片列为单独检查
  } else {
    run('输出非空', () => assert.ok(out1.length > 0, `out1 为空（输入：${escapeForLog(fixture.md)}）`))
  }
  run('二次保存幂等', () => assert.strictEqual(out2, out1, `out2 !== out1\nout1=${escapeForLog(out1)}\nout2=${escapeForLog(out2)}`))
  run('文本不丢失', () =>
    assert.strictEqual(doc2.textContent, doc1.textContent, `文本变化：doc1=${escapeForLog(doc1.textContent)} doc2=${escapeForLog(doc2.textContent)}`),
  )

  // ---- 归一化接受清单（一次归一化允许，但必须幂等；这里只做记录，不做原文断言） ----

  // ---- 能力断言 ----
  const expect = fixture.expect
  if (expect.startsWith) {
    run('指定前缀', () => assert.ok(out1.startsWith(expect.startsWith), `期望以 ${escapeForLog(expect.startsWith)} 开头，实际 ${escapeForLog(out1)}`))
  }
  for (const sub of expect.includes || []) {
    run(`保留 ${escapeForLog(sub)}`, () => assert.ok(out1.includes(sub), `out1=${escapeForLog(out1)}`))
  }
  for (const sub of expect.textIncludes || []) {
    run(`文本含 ${escapeForLog(sub)}`, () => assert.ok(doc1.textContent.includes(sub), `doc1.textContent=${escapeForLog(doc1.textContent)}`))
    run(`二轮文本含 ${escapeForLog(sub)}`, () => assert.ok(doc2.textContent.includes(sub), `doc2.textContent=${escapeForLog(doc2.textContent)}`))
  }
  if (expect.delimiter) {
    run('表格分隔行保留对齐', () => {
      const line = firstLineMatching(out1, /---/)
      assert.ok(line, `out1 中没有表格分隔行：${escapeForLog(out1)}`)
      assert.match(line.trim(), expect.delimiter, `分隔行不带对齐标记：${escapeForLog(line)}`)
    })
  }
  if (expect.tableAligns) {
    run('doc1 表格对齐 attr', () => assert.deepStrictEqual(collectTableAligns(doc1), expect.tableAligns))
    run('doc2 表格对齐 attr', () => assert.deepStrictEqual(collectTableAligns(doc2), expect.tableAligns))
  }
  if (expect.image) {
    if (Image) {
      run('保留图片语法', () => assert.ok(/!\[[^\]]*\]\(\s*[^)]+\)/.test(out1), `out1 中没有图片语法：${escapeForLog(out1)}`))
    } else {
      knownLimitationNotes.push('image：Image 扩展不可用（自研 image-extension.ts 与 @tiptap/extension-image 均未解析到），图片保存会丢失，门禁暂不断言')
    }
  }

  const idempotent = out2 === out1
  const textPreserved = doc1.textContent === doc2.textContent
  const changed = out1 !== normalizeBodyMd(fixture.md)

  for (const failure of localFailures) failures.push(failure)
  reportRows.push({
    name: fixture.name,
    changed,
    idempotent,
    textPreserved,
    out1,
    out2,
    input: normalizeBodyMd(fixture.md),
    alignsDoc1: expect.tableAligns || expect.delimiter ? collectTableAligns(doc1) : undefined,
    alignsDoc2: expect.tableAligns || expect.delimiter ? collectTableAligns(doc2) : undefined,
    failures: localFailures,
  })
}

// ---------------------------------------------------------------------------
// 5b. 结构化 fixture（JSON 文档）：覆盖 Markdown 语法无法直接表达的边界形态
//     —— 单元格内硬换行、无表头表格、colspan 单元格。
// ---------------------------------------------------------------------------

const textBlock = (type, text) => ({
  type,
  content: text ? [{ type: 'paragraph', content: [{ type: 'text', text }] }] : [{ type: 'paragraph' }],
})

const STRUCTURED_FIXTURES = [
  {
    // 单元格内硬换行：Markdown 表格无法表达换行，保存时归一化为空格（旧行为写 `[hardBreak]` 占位文本，
    // 二次保存还会被转义成 `\[hardBreak\]`，属内容污染 + 非幂等）。
    name: 'structured-cell-hardbreak',
    json: {
      type: 'doc',
      content: [
        {
          type: 'table',
          content: [
            { type: 'tableRow', content: [textBlock('tableHeader', 'h')] },
            {
              type: 'tableRow',
              content: [
                {
                  type: 'tableCell',
                  content: [
                    {
                      type: 'paragraph',
                      content: [{ type: 'text', text: '第一行' }, { type: 'hardBreak' }, { type: 'text', text: '第二行' }],
                    },
                  ],
                },
              ],
            },
          ],
        },
      ],
    },
    expect: {
      out1Includes: ['| 第一行 第二行 |'],
      out1Excludes: ['[hardBreak]'],
      doc2TextIncludes: ['第一行', '第二行'],
    },
    // 一次归一化：hardBreak → 空格，文本带空格变化属预期，不做 doc 文本严格相等断言
    textPreserved: false,
  },
  {
    // 无表头表格：序列化时合成一行全空表头（GFM 必须有表头），内容全部作为数据行保留
    name: 'structured-headerless-table',
    json: {
      type: 'doc',
      content: [
        {
          type: 'table',
          content: [
            { type: 'tableRow', content: [textBlock('tableCell', 'r1c1'), textBlock('tableCell', 'r1c2')] },
            { type: 'tableRow', content: [textBlock('tableCell', 'r2c1'), textBlock('tableCell', '')] },
          ],
        },
      ],
    },
    expect: { out1Includes: ['|  |  |', 'r1c1', 'r2c1'], doc2TextIncludes: ['r1c1', 'r2c1'] },
    textPreserved: true,
  },
  {
    // colspan 单元格：Markdown 无法表达，序列化时按「首格保留 + 后续空列」扁平化，内容不丢且幂等
    name: 'structured-colspan-table',
    json: {
      type: 'doc',
      content: [
        {
          type: 'table',
          content: [
            { type: 'tableRow', content: [textBlock('tableHeader', 'h1'), textBlock('tableHeader', 'h2')] },
            { type: 'tableRow', content: [{ ...textBlock('tableCell', 'merged'), attrs: { colspan: 2 } }] },
          ],
        },
      ],
    },
    expect: { out1Includes: ['merged'], doc2TextIncludes: ['merged'] },
    textPreserved: true,
  },
]

for (const fixture of STRUCTURED_FIXTURES) {
  const editor = createEditor()
  const localFailures = []
  try {
    editor.commands.setContent(fixture.json, { emitUpdate: false })
    const doc1 = editor.state.doc
    const out1 = normalizeBodyMd(editor.storage.markdown.getMarkdown())
    editor.commands.setContent(out1, { emitUpdate: false })
    const doc2 = editor.state.doc
    const out2 = normalizeBodyMd(editor.storage.markdown.getMarkdown())
    const run = (label, fn) => {
      try {
        fn()
      } catch (error) {
        localFailures.push(`[${fixture.name}] ${label}: ${error.message}`)
      }
    }
    run('二次保存幂等', () =>
      assert.strictEqual(out2, out1, `out2 !== out1\nout1=${escapeForLog(out1)}\nout2=${escapeForLog(out2)}`),
    )
    if (fixture.textPreserved) {
      run('文本不丢失', () =>
        assert.strictEqual(
          doc2.textContent,
          doc1.textContent,
          `文本变化：doc1=${escapeForLog(doc1.textContent)} doc2=${escapeForLog(doc2.textContent)}`,
        ),
      )
    }
    for (const sub of fixture.expect.out1Includes || []) {
      run(`保留 ${escapeForLog(sub)}`, () => assert.ok(out1.includes(sub), `out1=${escapeForLog(out1)}`))
    }
    for (const sub of fixture.expect.out1Excludes || []) {
      run(`不应出现 ${escapeForLog(sub)}`, () => assert.ok(!out1.includes(sub), `out1=${escapeForLog(out1)}`))
    }
    for (const sub of fixture.expect.doc2TextIncludes || []) {
      run(`二轮文本含 ${escapeForLog(sub)}`, () =>
        assert.ok(doc2.textContent.includes(sub), `doc2.textContent=${escapeForLog(doc2.textContent)}`),
      )
    }
    for (const failure of localFailures) failures.push(failure)
    reportRows.push({
      name: fixture.name,
      changed: null,
      idempotent: out2 === out1,
      textPreserved: fixture.textPreserved ? doc1.textContent === doc2.textContent : null,
      out1,
      out2,
      input: '（结构化 JSON 文档）',
      failures: localFailures,
    })
  } catch (error) {
    failures.push(`[${fixture.name}] 结构化用例抛错：${error && error.stack ? error.stack : String(error)}`)
    reportRows.push({ name: fixture.name, error: String(error) })
  } finally {
    editor.destroy()
  }
}

// ---------------------------------------------------------------------------
// 6. 输出
// ---------------------------------------------------------------------------

const total = reportRows.length
const changedCount = reportRows.filter((row) => row.changed).length
const nonIdempotent = reportRows.filter((row) => row.idempotent === false).length
const textLost = reportRows.filter((row) => row.textPreserved === false).length

console.log(`stack: ${tableMode === 'table-alignment' ? 'table-alignment（修复后自定义 Table 序列化）' : 'vanilla @tiptap/extension-table（修复前基线）'}`)
console.log(`image: ${Image ? `已启用 ${imageSource === 'custom' ? '自研 image-extension.ts（#15）' : '@tiptap/extension-image'}` : 'Image 扩展不可用（已知限制）'}`)

if (REPORT) {
  for (const row of reportRows) {
    console.log('')
    console.log(`=== fixture: ${row.name} ===`)
    if (row.error) {
      console.log(`  error: ${row.error}`)
      continue
    }
    console.log(`  input:       ${escapeForLog(row.input)}`)
    console.log(`  out1:        ${escapeForLog(row.out1)}`)
    console.log(`  out2:        ${escapeForLog(row.out2)}`)
    console.log(`  changed: ${row.changed}  idempotent: ${row.idempotent}  textPreserved: ${row.textPreserved}`)
    if (row.alignsDoc1 !== undefined) {
      console.log(`  aligns doc1: ${escapeForLog(row.alignsDoc1)}`)
      console.log(`  aligns doc2: ${escapeForLog(row.alignsDoc2)}`)
    }
    for (const failure of row.failures) console.log(`  FAIL: ${failure}`)
  }
  console.log('')
}

for (const note of knownLimitationNotes) console.log(`[已知限制] ${note}`)

console.log(
  `TOTAL=${total} CHANGED=${changedCount} NOT_IDEMPOTENT=${nonIdempotent} TEXT_LOST=${textLost} FAILURES=${failures.length}`,
)

if (failures.length > 0) {
  console.error('')
  for (const failure of failures) console.error(`FAIL ${failure}`)
  console.error(`\nround-trip 门禁未通过：${failures.length} 项断言失败。`)
  process.exitCode = 1
} else {
  console.log('round-trip 门禁通过：所有 fixture 二次保存幂等且文本/能力保留。')
}