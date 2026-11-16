# sticky-notes/fade-guard Specification

## Purpose
定义便签「移出淡化」在什么条件下才允许触发，确保中文输入法组合输入、窗口焦点切换与临时浮层不会让便签在用户仍在操作时变淡。

## Requirements

### Requirement: 淡化触发条件

便签 SHALL 且仅在以下条件全部满足时淡化：指针实际位于便签窗口之外、窗口没有焦点、不处于输入法组合输入中、窗口内没有打开的浮层。任一条件不满足时 MUST NOT 淡化。指针位置 MUST 结合鼠标事件坐标与窗口边界判定，MUST NOT 只依赖 `mouseleave` 事件。

#### Scenario: 指针移出且窗口失焦

- **WHEN** 指针移出便签窗口、窗口失去焦点，且没有输入法组合与浮层
- **THEN** 便签在去抖时间后淡化到设置的不透明度

#### Scenario: 指针移出但窗口仍有焦点

- **WHEN** 指针移出便签窗口但焦点仍在便签窗口内
- **THEN** 便签保持完全不透明

#### Scenario: 输入法组合输入中

- **WHEN** 用户正在用中文输入法在便签内组合输入（候选窗存在、指针仍在便签内）
- **THEN** 便签保持完全不透明，不因输入法引起的鼠标事件而淡化

### Requirement: 输入法组合守卫

便签窗口 MUST 监听 `compositionstart` 与 `compositionend`：组合输入期间 MUST NOT 淡化，且 MUST 取消任何已排期的淡化；`compositionend` 后 SHALL 按当前指针与焦点状态重新判定，并仍经过去抖。

#### Scenario: 组合开始取消淡化

- **WHEN** 便签正在组合输入且此前已有一个待执行的淡化
- **THEN** 淡化被取消，便签保持完全不透明

#### Scenario: 组合结束后重新判定

- **WHEN** `compositionend` 触发时指针已在便签窗外且窗口无焦点
- **THEN** 在去抖时间后淡化，而不是立即淡化

#### Scenario: 中文输入法打字不误淡化

- **WHEN** 用户用中文输入法在便签内连续输入，指针始终在便签内
- **THEN** 整个输入过程中便签不淡化

### Requirement: 焦点与文档可见性守卫

淡化判定 MUST 结合 `window` 的 `blur` / `focus` 事件与 `document.hasFocus()`。窗口重新获得焦点时 MUST 立即取消淡化与待执行的淡化；窗口失焦后 MUST 经过去抖才允许淡化。

#### Scenario: 切换到其它应用

- **WHEN** 用户把焦点切到其它应用且指针不在便签内
- **THEN** 便签在去抖时间后淡化

#### Scenario: 点回便签窗口

- **WHEN** 用户点击便签窗口使其重新获得焦点
- **THEN** 便签立即恢复到完全不透明，不等待去抖

### Requirement: 去抖与立即取消

允许淡化前 MUST 等待约 200–300ms 的去抖窗口。窗口内发生任意按键、鼠标进入、指针按下或焦点恢复时 MUST 立即取消待执行的淡化。关闭「移出淡化」设置时 MUST 永不淡化。

#### Scenario: 快速移出又移回

- **WHEN** 指针移出便签后在去抖时间内又移回
- **THEN** 便签不产生可见的淡化

#### Scenario: 键击取消淡化

- **WHEN** 指针在窗外且出现待执行的淡化时，便签窗口收到键盘事件
- **THEN** 淡化被取消，便签保持完全不透明

#### Scenario: 关闭移出淡化

- **WHEN** 设置中的「移出淡化」为关闭状态且指针移出便签
- **THEN** 便签始终不发生淡化

### Requirement: 保留既有淡化外观

淡化不透明度 MUST 继续取设置值 `stickyFadeOpacity`（范围 10–100，默认 38），并通过现有 `opacity` 过渡呈现。缩小（mini）态便签 MUST NOT 淡化。`prefers-reduced-motion: reduce` 时 MUST 不使用过渡动画。

#### Scenario: 自定义不透明度生效

- **WHEN** 用户把移出淡化不透明度设为 60 且指针移出便签
- **THEN** 便签淡化到 60% 不透明度

#### Scenario: mini 条不淡化

- **WHEN** 便签处于缩小的 40px 悬浮条形态且指针移出
- **THEN** 悬浮条不发生淡化

#### Scenario: 减少动态效果

- **WHEN** 操作系统开启「减少动态效果」
- **THEN** 淡化状态切换不带过渡动画
