import assert from 'node:assert/strict'
import test from 'node:test'

import { buildPdfHtml, PdfHtmlError } from '../src/lib/pdf-html.ts'

test('空正文抛出可读错误', () => {
  assert.throws(
    () => buildPdfHtml({ title: 't', bodyHtml: '  \n ' }),
    (err) => err instanceof PdfHtmlError && err.message.includes('正文为空'),
  )
})

test('A4 页面与非彩色打印基线', () => {
  const html = buildPdfHtml({ title: '文档', bodyHtml: '<p>正文</p>' })
  assert.match(html, /@page \{ size: A4;/)
  assert.match(html, /background: #FFFFFF;/)
  assert.match(html, /color: #1A1A1A;/)
})

test('不携带任何应用外观主题的令牌', () => {
  const html = buildPdfHtml({ title: '文档', bodyHtml: '<p>正文</p>' })
  // 主题令牌名、主题纸色与主题圆角都不该出现在打印件里
  assert.doesNotMatch(html, /--paper|--ink-|--kb-/)
  assert.doesNotMatch(html, /#FAF6EE|#FCFAF6|#FAFAFC/)
  assert.doesNotMatch(html, /border-radius/)
})

test('标题按 HTML 转义', () => {
  const html = buildPdfHtml({ title: '<b>粗体</b> & "引号"', bodyHtml: '<p>x</p>' })
  assert.match(html, /<title>&lt;b&gt;粗体&lt;\/b&gt; &amp; &quot;引号&quot;<\/title>/)
})

test('图片保留 assets 引用，交给 Rust 内联', () => {
  const html = buildPdfHtml({
    title: '文档',
    bodyHtml: '<p>图</p><img src="assets/abc.png" alt="图">',
  })
  assert.match(html, /src="assets\/abc.png"/)
  assert.doesNotMatch(html, /data:image/)
})

test('正文原样嵌入，结构规则齐备', () => {
  const body =
    '<h1>标题</h1><ul><li>项</li></ul><blockquote><p>引用</p></blockquote>' +
    '<pre><code>x</code></pre><table><tr><td>1</td></tr></table>'
  const html = buildPdfHtml({ title: '文档', bodyHtml: body })
  assert.ok(html.includes(body))
  for (const rule of [/h1 \{/, /ul, ol \{/, /blockquote \{/, /pre \{/, /th, td \{/]) {
    assert.match(html, rule)
  }
})