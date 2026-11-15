// 标题下拉纯逻辑单测（OpenSpec sticky-shell-polish tasks 3.1 / 3.5）。
// 直接 `node src-react/test/heading-select.test.mjs` 运行（刻意不用 node:test / --test：
// Windows 沙箱下 node --test 的子进程管道会 EPERM）。Node 22 默认开启 type stripping，
// 可以直接 import 源码 .ts；heading-options.ts 里只有 `import type`，不会真的加载 @tiptap/core。
import assert from 'node:assert/strict'
import {
  HEADING_OPTIONS,
  applyHeadingCommand,
  headingIndex,
  headingLabel,
  headingValueForLevel,
  nextIndex,
  toHeadingLevel,
} from '../src/lib/heading-options.ts'

// ─── 极简测试运行器（与 sticky-text.test.mjs 同款）─────────────────────────

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

// ─── 选项集合：正文 + H1–H6 ───────────────────────────────────────────────

test('七项固定顺序：正文 + H1–H6（value = p / 1..6，level = 0 / 1..6）', () => {
  assert.equal(HEADING_OPTIONS.length, 7)
  assert.deepEqual(
    HEADING_OPTIONS.map((option) => option.value),
    ['p', '1', '2', '3', '4', '5', '6'],
  )
  assert.deepEqual(
    HEADING_OPTIONS.map((option) => option.label),
    ['正文', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6'],
  )
  assert.deepEqual(
    HEADING_OPTIONS.map((option) => option.level),
    [0, 1, 2, 3, 4, 5, 6],
  )
})

test('选项表自洽：value 与 label / level 一一对应（不会出现 H6 映射到 level 5）', () => {
  for (const option of HEADING_OPTIONS) {
    assert.equal(headingIndex(option.value), option.level)
    assert.equal(toHeadingLevel(option.value), option.level)
    assert.equal(headingValueForLevel(option.level), option.value)
    assert.equal(headingLabel(option.value), option.label)
  }
})

// ─── value → 级别 / 下标，未知值兜底 ───────────────────────────────────────

test("value → 下标：'p' = 0，'1'..'6' = 1..6", () => {
  assert.equal(headingIndex('p'), 0)
  assert.equal(headingIndex('1'), 1)
  assert.equal(headingIndex('2'), 2)
  assert.equal(headingIndex('3'), 3)
  assert.equal(headingIndex('4'), 4)
  assert.equal(headingIndex('5'), 5)
  assert.equal(headingIndex('6'), 6)
})

test('未知 / 非法 value 一律兜底「正文」（下标 0、级别 0、标签正文）', () => {
  for (const value of ['', '0', '7', '-1', 'h2', 'H2', 'heading', ' 2', null, undefined]) {
    assert.equal(headingIndex(value), 0)
    assert.equal(toHeadingLevel(value), 0)
    assert.equal(headingLabel(value), '正文')
  }
})

test("value → 级别：'p' → 0，'1'..'6' → 1..6", () => {
  assert.equal(toHeadingLevel('p'), 0)
  for (let level = 1; level <= 6; level += 1) {
    assert.equal(toHeadingLevel(String(level)), level)
  }
})

// ─── 级别 → value（回显兜底）───────────────────────────────────────────────

test('级别 → value：0 = 正文，1..6 = H1–H6', () => {
  assert.equal(headingValueForLevel(0), 'p')
  for (let level = 1; level <= 6; level += 1) {
    assert.equal(headingValueForLevel(level), String(level))
  }
})

test('级别 → value 边界：越界 / 非整数 / 非数（NaN、Infinity、null）→ 正文', () => {
  for (const level of [7, 100, -1, -6, NaN, Infinity, -Infinity, null, undefined, '2']) {
    assert.equal(headingValueForLevel(level), 'p')
  }
  // 小数按整数部分取（Tiptap 级别永远是整数，这里只做防御）
  assert.equal(headingValueForLevel(2.9), '2')
  assert.equal(headingValueForLevel(0.9), 'p')
})

// ─── nextIndex：↑↓ 在七项里首尾循环 ───────────────────────────────────────

test('nextIndex 普通移动：0→1、3→5、6→5（不越界）', () => {
  assert.equal(nextIndex(0, 1), 1)
  assert.equal(nextIndex(3, 2), 5)
  assert.equal(nextIndex(6, -1), 5)
})

test('nextIndex 首尾循环：下标 0 上移回绕到最后一项、最后一项下移回到 0', () => {
  assert.equal(nextIndex(0, -1), 6)
  assert.equal(nextIndex(6, 1), 0)
  // 连续按到底依旧循环：5 → 6 → 0
  assert.equal(nextIndex(nextIndex(5, 1), 1), 0)
  assert.equal(nextIndex(nextIndex(1, -1), -1), 6)
})

test('nextIndex 一圈以上 / 负 delta 取模后仍在 0..6', () => {
  assert.equal(nextIndex(2, 7), 2)
  assert.equal(nextIndex(2, -7), 2)
  assert.equal(nextIndex(2, 14), 2)
  assert.equal(nextIndex(2, 15), 3)
  assert.equal(nextIndex(2, -15), 1)
  assert.equal(nextIndex(0, 999), 999 % 7)
})

test('nextIndex 边界：current 越界 / 负数 / 非有限数先归一到合法下标', () => {
  assert.equal(nextIndex(-1, 1), 0)
  assert.equal(nextIndex(-1, 0), 6)
  assert.equal(nextIndex(7, 0), 0)
  assert.equal(nextIndex(13, 0), 6)
  assert.equal(nextIndex(NaN, 1), 1)
  assert.equal(nextIndex(NaN, -1), 6)
  assert.equal(nextIndex(Infinity, 0), 0)
  assert.equal(nextIndex(3, NaN), 3)
  assert.equal(nextIndex(3, Infinity), 3)
  // 任意结果都落在合法下标内
  for (let delta = -20; delta <= 20; delta += 1) {
    const index = nextIndex(4, delta)
    assert.ok(Number.isInteger(index) && index >= 0 && index <= 6, `delta=${delta} 越界：${index}`)
  }
})

// ─── value → 编辑命令映射（正文 setParagraph / H1–H6 toggleHeading）────────

/** 记录调用的假命令链：链式返回自身，便于断言".run() 之前不改文档"的契约 */
function fakeChain() {
  const calls = []
  const chain = {
    calls,
    setParagraph() {
      calls.push(['setParagraph'])
      return chain
    },
    toggleHeading(attributes) {
      calls.push(['toggleHeading', attributes])
      return chain
    },
  }
  return chain
}

test("'p' → setParagraph（不调用 toggleHeading）", () => {
  const chain = fakeChain()
  const returned = applyHeadingCommand(chain, 'p')
  assert.equal(returned, chain)
  assert.deepEqual(chain.calls, [['setParagraph']])
})

test("'1'..'6' → toggleHeading({ level: n })（不调用 setParagraph）", () => {
  for (let level = 1; level <= 6; level += 1) {
    const chain = fakeChain()
    const returned = applyHeadingCommand(chain, String(level))
    assert.equal(returned, chain)
    assert.deepEqual(chain.calls, [['toggleHeading', { level }]])
  }
})

test('未知 / 空 value 按「正文」兜底：走 setParagraph', () => {
  for (const value of ['', '0', '7', 'h2', null, undefined]) {
    const chain = fakeChain()
    applyHeadingCommand(chain, value)
    assert.deepEqual(chain.calls, [['setParagraph']], `value=${String(value)} 未兜底`)
  }
})

test('命令映射与选项表一致：七项各自只产生一次命令', () => {
  for (const option of HEADING_OPTIONS) {
    const chain = fakeChain()
    applyHeadingCommand(chain, option.value)
    assert.equal(chain.calls.length, 1)
    const [command, attributes] = chain.calls[0]
    if (option.level === 0) {
      assert.equal(command, 'setParagraph')
    } else {
      assert.equal(command, 'toggleHeading')
      assert.deepEqual(attributes, { level: option.level })
    }
  }
})

// ─── 汇总 ────────────────────────────────────────────────────────────────

console.log('')
if (failures.length > 0) {
  console.error(`失败 ${failures.length} 项 / 通过 ${passed} 项`)
  process.exit(1)
}
console.log(`全部通过：${passed} 项`)