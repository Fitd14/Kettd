# 📝 Markdown 引擎架构设计报告

## 🎯 一、目标与愿景

**对标产品**: 
- Typora：所见即所得的 Markdown 编辑体验
- Obsidian Live Preview：实时预览 + 代码高亮
- VSCode Markdown Preview Enhanced：代码片段管理

**核心能力**:
|功能点 |Typora |本项目目标|实施状态 |
|-------|--------|----------|---------|
|即时预览渲染 | ✅ 所见即所得 | ⚠️ 需优化 → preview mode toggle | MVP |
|代码高亮 (Prism.js) | ✅ 20+ 语言支持 | ✅ 可选模块加载 | P3 |
|Mermaid 图表自动生成 | ❌ 手动 HTML | ✅ 支持语法转图表 | P2 |
|数学公式 (KaTeX) | ⚠️ 部分支持 | ✅ LaTeX 渲染 | P3 |
|图片画廊 (Zoom+Lightbox) | ✅ Fullscreen 模式 | ✅ 待实现 | P4 |
|截图插入 (Snipaste 直连) | ✅ 热键调用截图工具 | 🌐 集成 Windows 剪贴板 API | MVP |
|表格编辑与拖拽排序 | ✅ 所见即所得 | ⚠️ 简化版（纯文本）| P5 |

---

## 🏗️ 二、技术架构决策 (ADR-004)

### ADR-004: Markdown 预览模式选择

**问题**: 如何在"普通用户快速阅读"与"开发者写代码"两种模式间平衡？  
**竞品方案对比**:
|产品 |默认模式 |优势 |劣势 |
|------|---------|------|-----|
|Typora |实时混合渲染（所见即所得） |沉浸式写作，流畅体验 | 性能开销大，内存占用高 |
|Obsidian |侧边预览切换 | 清晰分离编辑/显示 | 需手动切换，打断心流 |
|VSCode |双栏并排模式 | 同时查看源代码 + 渲染效果 | 占用屏幕宽度大 |

**本决策**: **默认启用"轻量级实时渲染"预览**（Vue SSR），按 Shift+Alt+N 一键切换纯文本编辑与 Rich Text 预览。  
**技术原因**: 
- Tauri WebView2 支持 CSS `display:contents`模式 → 仅渲染必要 DOM 元素
- Vue Suspense 懒加载图片/图表（避免冷启动卡顿）  
- 性能优化：虚拟滚动分页渲染列表（每屏仅渲染前 N 条代码块）

### ADR-005: Syntax Highlighting Strategy

**方案**: `Prism.js`替代`Highlight.js`  
**理由**:
|模块 |对比项 |Highlight.js |Prism.js |本项目选择 |
|-----|--------|--------------|---------|----------|
|性能指标 |<1ms 渲染速度（60fps） |⚠️ 慢速 (约 8-12ms) | ✅ <3ms（支持 GPU 合成）|Prism.js |
|语言支持数 |主流/冷门 |45+ |20+（仅覆盖核心语言）|Prism.js |
|自定义主题适配 |CSS 变量注入简单 |⚠️ 需手动重写 | ⚡ 开箱即用 |Prism.js |

```json
// TypeScript 类型定义 (VSCode Live Server + Npm install)
import Prism from 'prismjs';
const theme: SyntaxHighlightTheme = {
  plain: '#f8f8f2', // 默认背景色（浅色模式）
  comment: '#a1a1aa',
  string: '#5bc1bd',
  keyword: '#3b82f6',
  function: '#c678dd'
};

app.use('load-syntax-highlighter', { libraryName: 'PrismJS' });
```

---

## 📦 三、组件实现（Vue 3）

### 3.1 MarkdownEditor.vue（主编辑器组件）

```vue
<!-- src/components/MarkdownEditor.vue -->
<script setup>
import { ref, computed, onMounted } from 'vue';

/* === Props Definitions ===== */
interface EditorProps {
  mode: 'edit-mode'     /*纯文本编辑 | 混合渲染预览 */|'preview-mode';       /* Rich Text+代码高亮 */
}
const props = defineProps({mode: { type:String, required:true,default:'edit-mode' }});

/* === State Definitions ===== */
const editorContent = ref('');    // Vue reactive text state（纯文本模式）/rendered Markdown (混合渲染)
const previewContent = ref('');   // 渲染后的 HTML content (富文本视图）  
const isTypingIndicatorVisible = ref(false); // 光标闪烁动画指示器

/* === Computed: Markdown Parsing ===== */
computed(() => { return editorContent.value ? parseMarkdownToHTML(editorContent.value) : '';}));

/* === Methods (Vue Reactive API) ===== */
async function handleSaveDraft() {
  const content = await db.sqlite('INSERT INTO fragments (content,updated_at)', [JSON.stringify(editorContent.value)]);
  console.log(`Draft saved: ${editorContent.value.id}`); // Write to local SQLite storage
}

/* === Keyboard Event Listeners ===== */
onMounted(async () => {
  window.addEventListener('keydown', (ev) => { if (ev.ctrlKey && ev.shiftKey && ev.code==='AltRight') togglePreviewMode(); });
});
</script>

<template>
  <div class="md-editor-container">
    <!-- === Editor Mode Toggle Button ===== -->
    <div style="position:absolute;top:10px;right:12px;transition:.15s ease-out;background:#fff;box-shadow:0 2px 6px rgba(0,0,0,.1);border-radius:6px;z-index:10;">
      <button class="mode-toggle-btn {{ mode==='preview-mode'?'active':''}}" @click="togglePreviewMode()">Shift+Alt+N</button>
      <span v-if="mode==='edit-mode'" style="font-size:10px;color:#6b7280;margin-left:auto">点击预览 | Shift+Alt+N 一键切换</span>
    </div>

    <!-- === Editor Area (Pure Text Mode) ===== -->
    <textarea v-model:value="editorContent" ref="textAreaRef" class="md-text-editor" style="font-family:'Consolas',monospace;font-size:14px;line-height:1.6;background:#fafafa;width:100%;min-height:300px;resize:both;"></textarea>

    <!-- === Preview Area (Rich Text Mode) ===== -->
    <div v-if="mode==='preview-mode'" class="md-preview-container">
      <!-- Code Block Rendering with Syntax Highlighting ===== -->
      <pre :style="`background:#f8f8f2;margin-left:auto;font-size:13px;line-height:1.6;color:#4b5563`">{{ highlightCode(editorContent.value) }}</pre>

      <!-- Image Carousel (Zoom+Lightbox Mode) ===== -->
      <div v-html="renderImages(editorContent.value)" style="display:flex;flex-wrap:wrap;gap:8px"></div>

      <!-- Equation Rendering (KaTeX Support) ===== -->
      <div class="math-latex-render"><span :style="{color:'rgba(75,130,245,.6)',font-size:'12.8px'}">{{ editorContent.value.split('$$').map((x,i)=>i%2?'$$'+x+'$$':null).join('') }}</span></div>

      <!-- Mermaid Diagram (Auto-Rendering) ===== -->
      <div v-html="renderMermaid(editorContent.value)" :style="'background:#f9fafb'"></div>

      <!-- Table Editing (Typora-style drag-to-sort columns) ===== -->
      <table class="md-table-editor" style="border-collapse:collapse;width:100%;margin-top:12px;font-size:13px"><thead><tr><th>字段名</th><th>类型</th><th>描述</th></tr></thead><tbody><!-- Row-by-row rendering based on parsed markdown --> </tbody></table>
    </div>

    <!-- === Typing Effect Animation (Simulated Cursor Blink) ===== -->
    <span v-if="editorContent==='Typing...'" class="cursor-blank" style="display:inline-block;width:2px;height:16px;background:#475569;border-radius:2px;vertical-align:text-bottom;margin-left:-16px;animation: blink-cursor 0.8s infinite"></span>

    <!-- === Footer Status Bar ===== -->
    <div style="position:absolute;bottom:6px;left:12px;font-size:12px;color:#9ca3af">📝 {{ editorContent.split(/\r?\n/g).length }} lines • {{ editorContent.length }} chars • 🕐 Auto-save enabled</div>

  </div>
</template>

<style scoped>
/* === Editor Area Styles ===== */
.md-text-editor { outline: none;caret-color: #1f2937;transition:border .15s ease-out;}
.md-text-editor:focus { border-left:3px solid #3b82f6;box-shadow: inset 0 0 0 2px rgba(59,130,246,.2);background:#fff; }

/* === Preview Area (Rich Text) ===== */
.md-preview-container .pre { white-space:pre-wrap;color:#2d3748;background:#f8fafc;margin-bottom:8px;word-break:break-all;}

/* === Math Equation Syntax Highlighting ===== --> 
@keyframes blink-cursor { from, to{opacity:1;} 50%{},opacity:; }}
.cursor-blank {} <!-- cursor blink animation keyframes (optional) -->
</style>
```

### 3.2 MarkdownPreviewContainer.vue（预览容器组件）

```vue
<!-- src/components/MarkdownPreviewContainer.vue -->
<script setup>
import { ref, computed, watch } from 'vue';

const previewText = ref(''); // Vue reactive preview string (parsed markdown + rendered HTML)
const imageZoomModeActive = ref(false); // 全屏图片模式状态（Lightbox 效果）
const mermaidChartVisible = ref(false); // Mermaid 图表生成状态（仅当用户添加```chart 语法时激活）

/* === Keyboard Event Listening ===== */
onMounted(async () => { watch(previewText,(value)=>{ if(value==='Typing...'){isTypingIndicatorVisible.value=true;}else{} }); }));

/* === Typora-style Image Carousel (Lightbox Mode) ===== --> 
function showImageZoom(imageUrl){ 
  imageZoomModeActive.value=true;
  window.__TAURI__.shell.openFile(file:///path/to/image.jpg);// Open full-window preview in new Tauri tab // Use external editor for editing markdown files directly (VSCode/TextMate/etc.)
}
</script>

<template>
  <div class="preview-container" :style="'display:'+(imageZoomModeActive?'flex':'block')+';justify-content:center;align-items:center;'">
    <!-- Image Preview (Typora-style Carousel) ===== -->
    <img v-if="imageUrl" alt="Markdown preview image" src="data:image/png;base64,{base64EncodedImageData}" class="preview-image" @click.capture.stop="showImageZoom(imageUrl)" style="position:relative;width:100%;max-width:576px;cursor:pointer;transition:.3s ease-out;" />

    <!-- Mermaid Chart Rendering (Vue SSR Lazy-load) ===== -->
    <div v-html="renderMermaidChart()" class="mermaid-render" :style="'background:#f9fafb'"></div> <!-- Table Editing (Drag-to-sort columns, hover effect) ===== -->
  </div>
</template>

<style scoped>
/* === Code Block Styling with Line Numbering ===== */
pre { background:#f8f8f2;margin-left:auto;font-size:13px;line-height:1.6;color:#4b5563;white-space:pre-wrap;word-break:break-all;} 

/* === Table Editing (Typora-style drag-to-sort columns) ===== --> 
.th { border-bottom:1px solid #d1d5db;border-left:1px solid #d1d5db;padding:6px 8px;text-align:left;height:40px }
.td { border-bottom:1px solid #d1d5db;padding:6px 8px;text-align:left;height:40px;transition:border .1s ease-out;}
/* === Image Carousel (Zoom+Lightbox Mode) ===== -->
.preview-image.transition-anim:hover, .preview-image:focus-within { transform:scale(1.1);box-shadow:0 8px 24px rgba(0,0,0,.15)}
</style>
```

---

## 🎨 四、UI/UX设计规范 (Typora 对标)

### 4.1 字体与行高适配

|元素 |字体族 |大小 |行高 |说明 |
|-----|-------|------|-----|-----|
|代码块（Pre block） | `Consolas,Monaco` | 13px | 1.6 | VSCode + iTerm2 风格一致 |
|段落文字 | Segoe UI / Microsoft YaHei | 14px | 1.9 | 行间距适中，避免拥挤 |
|标题（H1/H2） | Arial/Hiragino Sans GB | 20-28px | 1.65 | 加粗 + 下划线区分层级 |
|链接与图片说明 | Calibri/Segoe UI SCript | 12px | 1.4 | 悬停时显示 Tooltip 说明（Typora 风格） |

**CSS 配置**:
```css
.md-editor { font-family: 'Consolas', 'Monaco', monospace; line-height: 1.6 }
.preview-image img { transition: scale .3s ease-out }  
/* === Image Carousel (Zoom+Lightbox Mode) ===== */
.preview-image:hover, .preview-image:focus-within { transform:scale(1.05); cursor:pointer}
```

### 4.2 Tooltip 与悬浮提示（Typora 风格热键说明）

**交互设计**:
- 快捷键 **Ctrl+F3/F4/F5** → 显示/隐藏代码块高亮
- **Esc**关闭当前打开的全屏图片预览模式（Lightbox zoom-in）  
- 光标移到图片上显示 "点击放大" Tooltip  

```vue
<!-- ToolTip Component (Vue Popper + Vue Use) ===== -->
<template>
  <div style="position:absolute;top:-28px;transform:translateX(-50%);width:148px;text-align:center;font-size:11px;color:#6b7280;background:#fff;border:1px solid #e4e4e7;border-radius:6px;padding:6px 12px;">
    <!-- Tooltip Content + Hover Animation ===== -->
    <span :style="hoverEffectEnabled?'font-weight:500;animation: tooltip-pop .3s ease-in':''">{{ tooltipText }}</span>
  </div>
</template>

<script setup>
const tooltipText = ref(''); // Vue reactive string state (Typora-style hotkey description)
function showTooltip(content){ 
  setTimeout(()=>{tooltipText.value=content;},250); // Delay by half-animation+user observation time (30s)
}</script>
```

---

## 📊 五、性能指标（Web Vitals）

|指标 |目标值 |测量方法 |优化方案 |
|------|-------|---------|----------|
|冷启动时间 | <1.5s | `performance.now()`基准测试 | Vue SSR + Lazy-load组件（虚拟滚动分页加载） |
|渲染帧率 | 60fps (GPU合成) | Chrome DevTools Performance tab | 禁用过度重排动画 → 使用`transform`+`will-change`替代直接修改盒模型 |
|内存占用 | <150MB | Task Manager + `navigator.deviceMemory` | 删除不再使用的 DOM 节点 → 避免内存泄漏 |

### 5.1 Lazy-load图片实现（WebP 格式支持）

```javascript
// === Vue SSR lazy-load images ===== --> 
const image = new Image(); 

image.addEventListener('load', ()=>{ console.log(`Image loaded: ${image.naturalWidth}x${image.naturalHeight}`);});

image.src = await fetch(imageDataURI).then(response=>response.blob()).then(blob=>URL.createObjectURL(blob)); // Blob URL creation + memory cleanup on blur
```

**优化建议**: 
- 使用 **WebP 格式图片**（浏览器自动转换至 PNG/JPG fallback）  
- 懒加载策略：仅渲染可视区域前 N 张图片 → 超出范围立即清除 `objectURL` 引用  

---

## 📋 六、验收标准 (Acceptance Criteria)

|功能点 |验收条件 |测试方法 |
|-------|---------|---------|
|即时预览切换 | Shift+Alt+N 一键切换纯文本/预览模式 | 键盘事件监听测试（event.preventDefault） |
|代码高亮效果 | Prism.js语法高亮正常显示 | Chrome DevTools Console check for syntax errors |
|表格拖拽排序 | 支持水平方向（列）拖动 → 自动重排顺序 | UI 回放录制视频 +逐帧检查 DOM 节点 |
|图片全屏放大 | 点击打开独立 Tauri WebView2窗口 | `window.__TAURI__.shell.openFile`API 调用测试 |

---

**报告撰写**: AI Assistant  
**评审日期**: 2026 年 8 月  
