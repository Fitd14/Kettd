/**
 * 粘贴截图 · 纯逻辑单测（node:test）
 *
 * 跑法：node src-react/test/image-paste.test.mjs
 * （不要用 node --test：沙箱内 spawn 会 EPERM；直接 node 跑本文件时 node:test 在进程内执行。）
 *
 * 覆盖：extractImageFiles —— 从伪 ClipboardData 提取图片文件的纯函数。
 * 不覆盖：canvas 压缩 / 文档总量 / 插入链路 —— 它们依赖真实浏览器能力
 * （createImageBitmap、canvas.toDataURL、ProseMirror EditorView），无头 Node 环境不可用，
 * 这里跳过；真机冒烟见 openspec/changes/fix-known-defects-iteration/tasks.md 6.3。
 */
import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  extractImageFiles,
  MAX_IMAGE_DATA_URL_BYTES,
  MAX_IMAGES_PER_PASTE,
} from '../src/rendering/paste-image.ts'

/** 伪文件对象：extractImageFiles 只读 type，不依赖 File 实例 */
function fakeFile(type, name = 'clip.png') {
  return { type, name, size: 1 }
}

/** 伪 DataTransferItem（kind === 'file'） */
function fileItem(file, type = file ? file.type : '') {
  return { kind: 'file', type, getAsFile: () => file }
}

/** 伪文本 DataTransferItem（粘贴纯文本时出现） */
function textItem() {
  return { kind: 'string', type: 'text/plain', getAsFile: () => null }
}

/** 伪 ClipboardData（items / files 均可省略） */
function clipboard({ items, files } = {}) {
  return { items: items ?? [], files: files ?? [] }
}

test('extractImageFiles：空剪贴板返回空数组', () => {
  assert.deepEqual(extractImageFiles(null), [])
  assert.deepEqual(extractImageFiles(undefined), [])
  assert.deepEqual(extractImageFiles({}), [])
  assert.deepEqual(extractImageFiles(clipboard()), [])
})

test('extractImageFiles：从 items 提取图片，忽略文本与非图片文件', () => {
  const png = fakeFile('image/png', 'shot.png')
  const txt = fakeFile('text/plain', 'note.txt')
  const data = clipboard({ items: [textItem(), fileItem(txt), fileItem(png)] })
  assert.deepEqual(extractImageFiles(data), [png])
})

test('extractImageFiles：只认 kind === "file" 的条目（string 项即使 type 是 image/* 也忽略）', () => {
  const item = { kind: 'string', type: 'image/png', getAsFile: () => fakeFile('image/png') }
  assert.deepEqual(extractImageFiles(clipboard({ items: [item] })), [])
})

test('extractImageFiles：getAsFile 返回 null 的条目跳过', () => {
  const data = clipboard({ items: [fileItem(null), fileItem(fakeFile('image/webp'))] })
  assert.deepEqual(
    extractImageFiles(data).map((file) => file.type),
    ['image/webp'],
  )
})

test('extractImageFiles：items 没有图片时回退 files', () => {
  const jpg = fakeFile('image/jpeg', 'a.jpg')
  const data = clipboard({ items: [textItem()], files: [jpg, fakeFile('text/plain')] })
  assert.deepEqual(extractImageFiles(data), [jpg])
})

test('extractImageFiles：items 已命中图片时不再从 files 取（避免同图重复）', () => {
  const png = fakeFile('image/png')
  const data = clipboard({ items: [fileItem(png)], files: [png] })
  assert.deepEqual(extractImageFiles(data), [png])
})

test('extractImageFiles：item.type 为空时用 file.type 判断', () => {
  const png = fakeFile('image/png')
  const data = clipboard({ items: [{ kind: 'file', type: '', getAsFile: () => png }] })
  assert.deepEqual(extractImageFiles(data), [png])
})

test('extractImageFiles：file.type 为空时用 item.type 判断', () => {
  const shot = fakeFile('', 'shot')
  const data = clipboard({ items: [{ kind: 'file', type: 'image/png', getAsFile: () => shot }] })
  assert.deepEqual(extractImageFiles(data), [shot])
})

test('extractImageFiles：MIME 大小写不敏感（IMAGE/PNG 也接受）', () => {
  const shot = fakeFile('IMAGE/PNG')
  assert.deepEqual(extractImageFiles(clipboard({ items: [fileItem(shot)] })), [shot])
})

test('extractImageFiles：多图保持剪贴板原有顺序', () => {
  const first = fakeFile('image/png', '1.png')
  const second = fakeFile('image/jpeg', '2.jpg')
  const data = clipboard({ items: [fileItem(first), textItem(), fileItem(second)] })
  assert.deepEqual(extractImageFiles(data), [first, second])
})

test('压缩/上限参数与 design.md D5 一致（单图 300KB、一次最多 3 张）', () => {
  assert.equal(MAX_IMAGE_DATA_URL_BYTES, 300 * 1024)
  assert.equal(MAX_IMAGES_PER_PASTE, 3)
})