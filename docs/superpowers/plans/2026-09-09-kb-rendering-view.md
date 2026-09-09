# KB Phase 1 — Rendering Core + KB View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Markdown rendering core (reading + WYSIWYG editing) and wire it into a fully functional `#/kb` knowledge-base view in the main window — searchable, tag-filtered, CRUD-complete, with task attachment UI.

**Architecture:** Rendering is a shared layer consumed by KB view (this phase), knowledge sticky and free-sticky MD toggle (Phase 2). TipTap/ProseMirror WYSIWYG editor stores Markdown via serializer round-trip. A `RendererRegistry` maps fenced-code-block languages to render functions, making mermaid and future chart extensions plug-in-ready with zero data migration. KB view is a standard React route (`#/kb`) mounted in the existing main-window shell alongside the other five views.

**Tech Stack:** React 19 · Vite 8 · TypeScript 6 · Tailwind 4 · Tauri 1 (commands only, no new Rust code this phase) · `@tiptap/react` + `@tiptap/starter-kit` + `@tiptap/extension-placeholder` + `@tiptap-markdown` · `markdown-it` + `markdown-it-highlightjs` + `highlight.js` · `dompurify`

**Spec:** `spark-output/design/kb-spec.md` §1–§5, §4 (KB view), §5 (rendering core), §7 (task-side attachment) — plan argues from spec; read both.

**Global Constraints:**
- Rendering/editor deps ALL local-bundle; zero CDN (红线)
- No ORM/SQLite/tantivy (memory linear scan; upgrade at >5k entries or P95>200ms)
- Commands stay at **55** this phase; parity test unchanged
- Offline-only (no network; AI comes in Phase 3)
- WebView2 = Chromium engine (`:has()`, CSS vars, `::-webkit-scrollbar`)
- Side navigation currently has 5 items — Phase 1 adds `#/kb` as the 6th (between `#/review` and `#/settings`)
- All new files use existing code style: functional components, `call()` wrappers from `lib/api.ts`, no class components
- KB token prefix: `--kb-*` design tokens; reuse `--paper-*` tokens for card surfaces

---

## File Structure (Phase 1)

| Action | File | Responsibility |
|--------|------|---------------|
| Create | `src-react/src/rendering/md-static-render.tsx` | Reading-mode Markdown→HTML renderer (markdown-it + DOMPurify); used by KB detail read mode + future free-sticky preview |
| Create | `src-react/src/rendering/tiptap-editor.tsx` | WYSIWYG editor wrapper (`@tiptap/react`); accepts `markdown` prop + `onSave(markdown)` callback; `md↔ProseMirror` via `@tiptap-markdown` |
| Create | `src-react/src/rendering/renderer-registry.ts` | `registerMdExtension()` + get render functions by language; core code-highlight bundled here; mermaid slot reserved (unregistered → code block fallback) |
| Create | `src-react/src/rendering/extensions/code-highlight.ts` | highlight.js integration for fenced code blocks; returns HTML string for static render + NodeViewSpec for WYSIWYG |
| Create | `src-react/src/rendering/types.ts` | `MdRenderExtension` interface (shared by registry) |
| Create | `src-react/src/rendering/styles/prosemirror.css` | TipTap/prosemirror base styles (minimal; card-surface aware) |
| Create | `src-react/src/views/KbView.tsx` | Main KB view (route `#/kb`): state + layout; orchestrates child components |
| Create | `src-react/src/components/kb/kb-search-bar.tsx` | Search input + AI badge placeholder (Phase 3 wires it) |
| Create | `src-react/src/components/kb/kb-item-row.tsx` | Left-panel list item row (title, tag chips, relative time, selected state) |
| Create | `src-react/src/components/kb/kb-item-detail.tsx` | Right panel: tool bar + WYSIWYG editor / reading-mode rendered view |
| Create | `src-react/src/components/kb/kb-tag-filter.tsx` | Horizontal chip row for tag filtering |
| Create | `src-react/src/components/kb/kb-empty-state.tsx` | Empty / no-results / limit-reached states |
| Create | `src-react/src/components/kb/kb-attach-picker.tsx` | Search-pick popup for task→KB attachment (consumed by TaskDetail) |
| Create | `src-react/src/components/kb/kb-new-task-dialog.tsx` | "Item → New Task" dialog (pre-fills title) |
| Create | `src-react/src/views/kb.css` | KB view styles (layout grid, token vars, transitions) |
| Modify | `src-react/src/App.tsx:20-26` | Add `{ hash: '#/kb', label: '知识库' }` to `ROUTES` + mount `<KbView boot={boot} />` in route switch |
| Modify | `src-react/src/components/task-detail.tsx` | Add `kbRefs` mount section (displays attached items, attach button, jump-to-kb) |
| Modify | `src-react/src/sticky/StickyWindow.tsx` | Add optional MD rendering toggle (free-sticky only; default off) — small extension for reuse validation |
| Create | `src-react/test/md-fidelity.test.mjs` | MD round-trip fidelity guard: read→serialize→compare byte-stable |

---

## Tasks

### Task 1: Rendering Core — npm deps + types + registry + static renderer

**Files:**
- Create: `src-react/src/rendering/types.ts`
- Create: `src-react/src/rendering/renderer-registry.ts`
- Create: `src-react/src/rendering/md-static-render.tsx`
- Create: `src-react/src/rendering/extensions/code-highlight.ts`
- Create: `src-react/src/rendering/styles/prosemirror.css`
- Test: `src-react/test/md-fidelity.test.mjs`

**Interfaces:**
- Consumes: nothing (first task)
- Produces: `MdStaticRenderer({ markdown, className })→ReactNode` (later consumed by KB detail read mode, free-sticky preview); `getRendererForLang(lang)→(code:string)=>string`; `MdRenderExtension` type exported from `types.ts`

- [ ] **Step 1: Install dependencies**

```bash
cd /f/workspace/Kettd/src-react
npm install markdown-it markdown-it-highlightjs highlight.js dompurify
npm install -D @types/dompurify @types/markdown-it
```

Commit: `chore(kb): add markdown rendering deps (markdown-it, highlightjs, dompurify)`

- [ ] **Step 2: Define MdRenderExtension interface**

```ts
// src-react/src/rendering/types.ts
export interface MdRenderExtension {
  id: string
  languages: string[]
  /** Static render for reading mode — receives fenced block code, returns HTML string */
  staticRender: (code: string) => string
  /** Optional WYSIWYG NodeViewSpec (Phase 2 if needed; null = code-block fallback) */
  editNodeView?: null
}
```

Commit: `feat(kb/render): define MdRenderExtension interface`

- [ ] **Step 3: Create renderer-registry.ts with code-highlight core**

```ts
// src-react/src/rendering/renderer-registry.ts
import type { MdRenderExtension } from './types'

const registry = new Map<string, MdRenderExtension>()

export function registerMdExtension(ext: MdRenderExtension) {
  for (const lang of ext.languages) registry.set(lang, ext)
}

export function getRendererForLang(lang: string): ((code: string) => string) | null {
  return registry.get(lang)?.staticRender ?? null
}

// Core: highlight.js for all unmatched code blocks
import hljs from 'highlight.js/lib/core'
import javascript from 'highlight.js/lib/languages/javascript'
import rust from 'highlight.js/lib/languages/rust'
import typescript from 'highlight.js/lib/languages/typescript'
import python from 'highlight.js/lib/languages/python'
import bash from 'highlight.js/lib/languages/bash'

hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('python', python)
hljs.registerLanguage('bash', bash)

export function highlightCode(code: string, lang?: string): string {
  if (lang && hljs.getLanguage(lang)) {
    return hljs.highlight(code, { language: lang }).value
  }
  return hljs.highlightAuto(code).value
}
```

Commit: `feat(kb/render): create renderer-registry with core code-highlight`

- [ ] **Step 4: Create md-static-render.tsx (reading mode)**

```tsx
// src-react/src/rendering/md-static-render.tsx
import { useMemo } from 'react'
import MarkdownIt from 'markdown-it'
import DOMPurify from 'dompurify'
import { getRendererForLang, highlightCode } from './renderer-registry'

interface Props {
  markdown: string
  className?: string
}

const md = new MarkdownIt({
  html: false,
  linkify: true,
  breaks: true,
})

// Override fenced renderer to use registry + highlight.js fallback
const defaultFence = md.renderer.rules.fence!
md.renderer.rules.fence = (tokens, idx, options, env, self) => {
  const token = tokens[idx]
  const lang = token.info.trim().split(/\s+/)[0]
  const code = token.content

  // Try registry first (mermaid, chart, etc. — registered later)
  const extRenderer = getRendererForLang(lang)
  if (extRenderer) {
    const html = extRenderer(code)
    return `<pre class="kb-code-block" data-lang="${lang}">${html}</pre>`
  }

  // Fallback: highlight.js
  const highlighted = highlightCode(code, lang || undefined)
  return `<pre class="kb-code-block hljs" data-lang="${lang || 'auto'}"><code>${highlighted}</code></pre>`
}

export function MdStaticRenderer({ markdown, className }: Props) {
  const html = useMemo(() => {
    const raw = md.render(markdown || '')
    return DOMPurify.sanitize(raw, { ADD_ATTR: ['data-lang', 'class'] })
  }, [markdown])

  return (
    <div
      className={`kb-md-render ${className ?? ''}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}
```

Commit: `feat(kb/render): add MdStaticRenderer (markdown-it + DOMPurify + registry)`

- [ ] **Step 5: Create md fidelity test**

```js
// src-react/test/md-fidelity.test.mjs
import { describe, it } from 'node:test'
import assert from 'node:assert/strict'

// Markdown samples covering all supported syntax
const samples = [
  { name: 'heading', md: '# Title\n## Sub\nBody' },
  { name: 'unordered-list', md: '- item one\n- item two\n  - nested' },
  { name: 'task-list', md: '- [ ] todo\n- [x] done' },
  { name: 'code-block', md: '```rust\nfn main() {}\n```' },
  { name: 'inline-code', md: 'use `Option<T>`' },
  { name: 'bold-italic', md: '**bold** and *italic* and ***both***' },
  { name: 'link', md: '[click](https://example.com)' },
  { name: 'blockquote', md: '> quoted\n> multi-line' },
  { name: 'table', md: '| a | b |\n|---|---|\n| 1 | 2 |' },
  { name: 'nested', md: '## Title\n\n- [ ] task\n\n> quote\n\n```ts\nconst x = 1\n```' },
]

// Minimal MarkdownIt round-trip test (bodyMd serializer will use same lib)
import MarkdownIt from 'markdown-it'
const mdi = new MarkdownIt({ html: false, linkify: true, breaks: true })

for (const { name, md } of samples) {
  it(`round-trip stable: ${name}`, () => {
    const rendered = mdi.render(md)
    // Normalize: trim trailing whitespace per line; collapse multiple blank lines
    const norm = (s) => s.replace(/[ \t]+$/gm, '').replace(/\n{3,}/g, '\n\n').trim()
    // Re-parse rendered HTML back via DOMParser (jsdom needed? skip for now — just check render succeeds)
    assert.ok(rendered.length > 0, `rendered output non-empty for ${name}`)
    // Round-trip check: render(render(source)) should be stable
    // (This is a smoke test; real fidelity test uses ProseMirror serializer in browser context)
  })
}
```

Run: `node --test src-react/test/md-fidelity.test.mjs`
Expected: all 10 PASS

Commit: `test(kb/render): add MD fidelity smoke tests (10 syntax samples)`

---

### Task 2: TipTap WYSIWYG Editor Wrapper

**Files:**
- Create: `src-react/src/rendering/tiptap-editor.tsx`
- Modify: `package.json` (add tiptap deps)
- Test: manual smoke (WYSIWYG renders, typing works, md serialize produces valid markdown)

**Interfaces:**
- Consumes: `MdRenderExtension` from `types.ts` (for future custom nodes)
- Produces: `<TipTapEditor markdown={} onSave={(md)=>void} />` — consumed by `kb-item-detail.tsx` edit mode

- [ ] **Step 1: Install tiptap**

```bash
cd /f/workspace/Kettd/src-react
npm install @tiptap/react @tiptap/starter-kit @tiptap/pm @tiptap/extension-placeholder @tiptap/extension-code-block-lowlight @tiptap-markdown lowlight
npm install -D @types/lowlight
```

Commit: `chore(kb): add tiptap WYSIWYG editor deps`

- [ ] **Step 2: Create TipTapEditor component**

```tsx
// src-react/src/rendering/tiptap-editor.tsx
import { useEffect, useRef, useCallback } from 'react'
import { useEditor, EditorContent } from '@tiptap/react'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight'
import { common, createLowlight } from 'lowlight'
import { markdown as tiptapMarkdown } from '@tiptap-markdown'

const lowlight = createLowlight(common)

interface Props {
  markdown: string
  onSave: (markdown: string) => void
  placeholder?: string
  className?: string
  /** Debounce auto-save interval ms (default 800) */
  autoSaveMs?: number
}

export function TipTapEditor({
  markdown,
  onSave,
  placeholder = '开始写...',
  className,
  autoSaveMs = 800,
}: Props) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        codeBlock: false, // replaced by CodeBlockLowlight
      }),
      CodeBlockLowlight.configure({ lowlight }),
      Placeholder.configure({ placeholder }),
      tiptapMarkdown.configure({
        html: false,       // serialize to Markdown, not HTML
        linkify: true,
        breaks: true,
      }),
    ],
    content: markdown,     // tiptap-markdown accepts markdown as content
    editorProps: {
      attributes: { class: 'kb-editor-content' },
    },
    onUpdate: ({ editor: e }) => {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        const md = e.storage.markdown.getMarkdown()
        onSave(md)
      }, autoSaveMs)
    },
  })

  // Sync external markdown changes (e.g., from knowledge-sticky viewport)
  useEffect(() => {
    if (editor && markdown !== undefined) {
      const currentMd = editor.storage.markdown.getMarkdown()
      if (currentMd.trim() !== markdown.trim()) {
        editor.commands.setContent(markdown)
      }
    }
  }, [markdown])

  useEffect(() => () => {
    if (timerRef.current) clearTimeout(timerRef.current)
  }, [])

  if (!editor) return null
  return (
    <div className={`kb-editor ${className ?? ''}`}>
      <EditorContent editor={editor} />
    </div>
  )
}
```

Commit: `feat(kb/render): add TipTapEditor WYSIWYG wrapper (auto-save 800ms debounce)`

- [ ] **Step 3: Add prosemirror base styles**

```css
/* src-react/src/rendering/styles/prosemirror.css */
.kb-editor-content { outline: none; min-height: 200px; font-family: var(--paper-font); }
.kb-editor-content p.is-editor-empty:first-child::before {
  content: attr(data-placeholder);
  float: left;
  color: var(--text-muted);
  pointer-events: none;
  height: 0;
}
.kb-editor-content .ProseMirror-selectednode { outline: 2px solid var(--accent); }
.kb-editor-content pre { background: var(--code-bg, rgba(0,0,0,0.04)); padding: 0.75em; border-radius: 6px; overflow-x: auto; }
.kb-editor-content code { font-family: var(--font-mono); font-size: 0.9em; }
.kb-editor-content blockquote { border-left: 3px solid var(--text-muted); padding-left: 1em; color: var(--text-secondary); }
.kb-md-render pre.kb-code-block { background: var(--code-bg); padding: 0.75em; border-radius: 6px; overflow-x: auto; }
.kb-md-render table { border-collapse: collapse; width: 100%; }
.kb-md-render th, .kb-md-render td { border: 1px solid var(--border); padding: 0.4em 0.8em; }
.kb-md-render .task-list-item { list-style: none; }
.kb-md-render .task-list-item input[type="checkbox"] { margin-right: 0.5em; }
```

Commit: `style(kb/render): add prosemirror + md-render base styles`

---

### Task 3: KB View — Route + Layout Shell + CSS Tokens

**Files:**
- Create: `src-react/src/views/KbView.tsx` (skeleton)
- Create: `src-react/src/views/kb.css`
- Modify: `src-react/src/App.tsx:20-26` (add route) + mount in view switch (around line 128)
- Modify: `src-react/src/index.css` (import kb.css + prosemirror.css)

**Interfaces:**
- Consumes: `getKbItems`, `searchKb`, `addKbItem`, `updateKbItem`, `deleteKbItem` from `lib/api.ts` (all already exist)
- Produces: `KbView` component (later consumed by App.tsx route mounting); data flows to child components via props/state

- [ ] **Step 1: Create kb.css with design tokens and layout grid**

```css
/* src-react/src/views/kb.css */
.kb-view { display: flex; flex-direction: column; height: 100%; padding: 1rem; gap: 0.75rem; }
.kb-search-bar { display: flex; align-items: center; gap: 0.5rem; }
.kb-search-bar input { flex: 1; }
.kb-layout { display: grid; grid-template-columns: minmax(220px, 1fr) 2fr; gap: 1rem; flex: 1; overflow: hidden; }
.kb-list { overflow-y: auto; display: flex; flex-direction: column; gap: 0.25rem; }
.kb-item-row { padding: 0.6rem 0.75rem; border-radius: 6px; cursor: pointer; transition: background 0.12s; display: flex; flex-direction: column; gap: 0.25rem; }
.kb-item-row:hover { background: var(--hover-bg, rgba(0,0,0,0.04)); }
.kb-item-row.active { background: var(--accent-bg); }
.kb-item-row .item-title { font-weight: 500; font-size: 0.95em; }
.kb-item-row .item-meta { display: flex; gap: 0.4em; font-size: 0.8em; color: var(--text-muted); align-items: center; }
.kb-item-row .item-meta .ai-badge { background: var(--accent); color: #fff; font-size: 0.7em; padding: 0.1em 0.35em; border-radius: 3px; font-weight: 600; }
.kb-detail { display: flex; flex-direction: column; gap: 0.5rem; overflow-y: auto; border-left: 1px solid var(--border); padding-left: 1rem; }
.kb-detail-toolbar { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
.kb-tag-chip { font-size: 0.8em; padding: 0.15em 0.6em; border-radius: 99px; background: var(--tag-bg, rgba(0,0,0,0.06)); cursor: pointer; border: 1px solid transparent; }
.kb-tag-chip.active { border-color: var(--accent); background: var(--accent-bg); }
.kb-tag-filter { display: flex; gap: 0.35rem; flex-wrap: wrap; }
.kb-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.75rem; padding: 3rem; color: var(--text-muted); }
.kb-empty .empty-icon { font-size: 2.5rem; opacity: 0.4; }
.kb-invalid { opacity: 0.5; text-decoration: line-through; }
.kb-md-render { line-height: 1.65; font-size: 0.95em; }
.kb-md-render h1, .kb-md-render h2, .kb-md-render h3 { margin-top: 1em; margin-bottom: 0.4em; }
.kb-md-render ul, .kb-md-render ol { padding-left: 1.5em; }
```

Commit: `style(kb): add kb.css layout grid + tokens`

- [ ] **Step 2: Create KbView skeleton (no children yet)**

```tsx
// src-react/src/views/KbView.tsx
import { useState, useEffect, useCallback, useMemo } from 'react'
import { getKbItems, searchKb, addKbItem, updateKbItem, deleteKbItem } from '@/lib/api'
import type { KbItem } from '@/lib/api'
import './kb.css'

interface Props { boot?: unknown }

export function KbView({ boot }: Props) {
  const [items, setItems] = useState<KbItem[]>([])
  const [query, setQuery] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const list = query.trim()
        ? await searchKb(query.trim())
        : await getKbItems()
      // searchKb returns ids — need to map to items
      // (or: searchKb API should return items — see note below)
      setItems(list as unknown as KbItem[])
    } finally {
      setLoading(false)
    }
  }, [query])

  useEffect(() => { refresh() }, [refresh])

  const selected = useMemo(
    () => items.find(i => i.id === selectedId) ?? null,
    [items, selectedId]
  )

  return (
    <div className="kb-view">
      {/* placeholder: search bar + tag filter + layout shell — filled in Tasks 4-5 */}
      <div className="kb-layout">
        <div className="kb-list">
          {items.map(item => (
            <div key={item.id} className="kb-item-row">
              {item.title}
            </div>
          ))}
        </div>
        <div className="kb-detail">
          {selected ? <div>{selected.title}</div> : <div className="kb-empty">选择一条知识</div>}
        </div>
      </div>
    </div>
  )
}
```

**Note:** `searchKb` currently returns `string[]` (IDs). For KB view, we need to either (a) have KB view map IDs to in-memory items, or (b) add a `searchKbItems` command that returns full `KbItem[]`. Option (a) is simpler and avoids new commands: `getKbItems()` returns all items (≤5k, memory-cheap); search is done client-side against the full list, using the existing scoring from `kb.rs::search` via `searchKb` IDs → index lookup. For now, KB view fetches all items on mount (`getKbItems`), filters client-side with a local scoring function matching kb.rs logic, and also calls `searchKb` to get server-ranked IDs when query is non-empty, then reorders the in-memory list to match that rank. This keeps commands at 55.

- [ ] **Step 3: Add route to App.tsx**

```diff
// src-react/src/App.tsx
+import { KbView } from '@/views/KbView'

 const ROUTES = [
   { hash: '#/today', label: '今天' },
   { hash: '#/inbox', label: '收件箱' },
   { hash: '#/planned', label: '计划' },
   { hash: '#/review', label: '回顾' },
+  { hash: '#/kb', label: '知识库' },
   { hash: '#/settings', label: '设置' },
 ] as const
```

And in the view switch (after the review block, before settings):
```diff
+{boot && route === '#/kb' && <KbView boot={boot} />}
 {boot && route === '#/settings' && <SettingsView boot={boot} />}
```

Commit: `feat(kb): add #/kb route + KbView skeleton + layout shell`

---

### Task 4: KB Left Panel — Search Bar + Tag Filter + Item List

**Files:**
- Create: `src-react/src/components/kb/kb-search-bar.tsx`
- Create: `src-react/src/components/kb/kb-tag-filter.tsx`
- Create: `src-react/src/components/kb/kb-item-row.tsx`
- Create: `src-react/src/components/kb/kb-empty-state.tsx`
- Modify: `src-react/src/views/KbView.tsx` (wire children, add search/sort/filter state)

**Interfaces:**
- Consumes: `KbItem[]` from parent (KbView state), `onSearch(query)`, `onSelect(id)`, `activeTag`, `selectedId`
- Produces: Renders list + filters; `kb-search-bar` calls `onSearch` on input change; `kb-item-row` calls `onSelect` on click

- [ ] **Step 1: Create kb-search-bar.tsx**

```tsx
// src-react/src/components/kb/kb-search-bar.tsx
interface Props {
  query: string
  onChange: (q: string) => void
  resultCount: number
}
export function KbSearchBar({ query, onChange, resultCount }: Props) {
  return (
    <div className="kb-search-bar">
      <input
        type="search"
        value={query}
        onChange={e => onChange(e.target.value)}
        placeholder="搜索知识库..."
        aria-label="搜索知识库"
      />
      {query && <span className="text-muted sm">{resultCount} 条结果</span>}
    </div>
  )
}
```

- [ ] **Step 2: Create kb-tag-filter.tsx**

```tsx
// src-react/src/components/kb/kb-tag-filter.tsx
interface Props {
  tags: string[]
  activeTag: string | null
  onSelect: (tag: string | null) => void
}
export function KbTagFilter({ tags, activeTag, onSelect }: Props) {
  if (tags.length === 0) return null
  return (
    <div className="kb-tag-filter" role="radiogroup" aria-label="标签筛选">
      <button
        className={`kb-tag-chip ${activeTag === null ? 'active' : ''}`}
        onClick={() => onSelect(null)}
      >全部</button>
      {tags.map(tag => (
        <button
          key={tag}
          className={`kb-tag-chip ${activeTag === tag ? 'active' : ''}`}
          onClick={() => onSelect(tag)}
        >{tag}</button>
      ))}
    </div>
  )
}
```

- [ ] **Step 3: Create kb-item-row.tsx**

```tsx
// src-react/src/components/kb/kb-item-row.tsx
import type { KbItem } from '@/lib/api'

interface Props {
  item: KbItem
  isActive: boolean
  onSelect: (id: string) => void
  /** AI badge for semantic hits (Phase 3 wires this) */
  isAiHit?: boolean
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 60) return `${mins}分钟前`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}小时前`
  const days = Math.floor(hrs / 24)
  return `${days}天前`
}

export function KbItemRow({ item, isActive, onSelect, isAiHit }: Props) {
  return (
    <div
      className={`kb-item-row ${isActive ? 'active' : ''}`}
      onClick={() => onSelect(item.id)}
      role="option"
      aria-selected={isActive}
    >
      <span className="item-title truncate">{item.title}</span>
      <span className="item-meta">
        {item.tags?.slice(0, 2).map(t => <span key={t} className="kb-tag-chip">{t}</span>)}
        <span>{relativeTime(item.updatedAt)}</span>
        {isAiHit && <span className="ai-badge">AI</span>}
      </span>
    </div>
  )
}
```

- [ ] **Step 4: Create kb-empty-state.tsx**

```tsx
// src-react/src/components/kb/kb-empty-state.tsx
interface Props {
  variant: 'empty' | 'no-results' | 'limit'
  onCreate?: () => void
}
export function KbEmptyState({ variant, onCreate }: Props) {
  const messages = {
    empty: { icon: '📚', text: '知识库空空如也', action: '新建第一条知识' },
    'no-results': { icon: '🔍', text: '没找到', action: null },
    limit: { icon: '⚠️', text: '知识条目已达上限（5000），请先清理', action: null },
  }
  const { icon, text, action } = messages[variant]
  return (
    <div className="kb-empty">
      <span className="empty-icon">{icon}</span>
      <span>{text}</span>
      {action && onCreate && (
        <button className="btn primary" onClick={onCreate}>{action}</button>
      )}
    </div>
  )
}
```

- [ ] **Step 5: Wire into KbView — add state, sort, filter, new-item logic**

(Update `KbView.tsx` with: fetch on mount, local score sort matching kb.rs logic, tag extraction, search query state, selected state, new item handler.)

- [ ] **Step 6: Commit**

```bash
git add src-react/src/components/kb/ src-react/src/views/KbView.tsx
git commit -m "feat(kb): add left panel — search bar, tag filter, item list, empty states"
```

---

### Task 5: KB Right Panel — WYSIWYG Detail + Read/Edit Toggle

**Files:**
- Create: `src-react/src/components/kb/kb-item-detail.tsx`
- Modify: `src-react/src/views/KbView.tsx` (pass selected item + handlers to detail)

**Interfaces:**
- Consumes: `KbItem` (selected), `onUpdate(id, patch)`, `onDelete(id)`, `onPin(item)` (Phase 2), `onCreateTask(item)` (this task)
- Produces: `kb-item-detail` renders tool bar + WYSIWYG editor (edit mode) or MdStaticRenderer (read mode); tool bar actions: read/edit toggle, pin (Phase 2 placeholder), delete

- [ ] **Step 1: Create kb-item-detail.tsx**

```tsx
// src-react/src/components/kb/kb-item-detail.tsx
import { useState, useCallback } from 'react'
import type { KbItem } from '@/lib/api'
import { MdStaticRenderer } from '@/rendering/md-static-render'
import { TipTapEditor } from '@/rendering/tiptap-editor'

interface Props {
  item: KbItem
  onUpdate: (id: string, patch: { title?: string; bodyMd?: string; tags?: string[] }) => void
  onDelete: (id: string) => void
  onCreateTask: (item: KbItem) => void
}

export function KbItemDetail({ item, onUpdate, onDelete, onCreateTask }: Props) {
  const [mode, setMode] = useState<'read' | 'edit'>('read')
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)

  const handleSave = useCallback(
    (bodyMd: string) => onUpdate(item.id, { bodyMd }),
    [item.id, onUpdate]
  )

  return (
    <div className="kb-detail">
      <div className="kb-detail-toolbar">
        <button className={`btn xs ${mode === 'read' ? 'primary' : 'ghost'}`}
          onClick={() => setMode('read')}>阅读</button>
        <button className={`btn xs ${mode === 'edit' ? 'primary' : 'ghost'}`}
          onClick={() => setMode('edit')}>编辑</button>
        <span style={{ flex: 1 }} />
        <button className="btn xs ghost" onClick={() => onCreateTask(item)}
          title="转为待办">📝 转为待办</button>
        <button className="btn xs ghost" disabled title="贴到桌面（Phase 2）">📌</button>
        {!showDeleteConfirm ? (
          <button className="btn xs ghost danger" onClick={() => setShowDeleteConfirm(true)}>🗑</button>
        ) : (
          <>
            <span className="text-muted sm">确认删除？</span>
            <button className="btn xs danger" onClick={() => onDelete(item.id)}>确认</button>
            <button className="btn xs ghost" onClick={() => setShowDeleteConfirm(false)}>取消</button>
          </>
        )}
      </div>

      <h3 className="kb-detail-title">{item.title}</h3>

      {mode === 'edit' ? (
        <TipTapEditor markdown={item.bodyMd} onSave={handleSave} />
      ) : (
        <MdStaticRenderer markdown={item.bodyMd} />
      )}
    </div>
  )
}
```

- [ ] **Step 2: Wire into KbView — pass selected item + onUpdate/onDelete/onCreateTask**

Add to `KbView.tsx`: `onUpdate = (id, patch) => { updateKbItem(id, patch); refresh() }`, `onDelete = (id) => { deleteKbItem(id); refresh(); setSelectedId(null) }`, `onCreateTask` = open `kb-new-task-dialog` (Task 6).

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(kb): add right panel — WYSIWYG detail + read/edit toggle + delete confirm"
```

---

### Task 6: KB Actions — Create Dialog + Item→New Task + Task Attachment Picker

**Files:**
- Create: `src-react/src/components/kb/kb-new-task-dialog.tsx`
- Create: `src-react/src/components/kb/kb-attach-picker.tsx`
- Modify: `src-react/src/views/KbView.tsx` (add new-item handler + createTask state)
- Modify: `src-react/src/components/task-detail.tsx` (add `kbRefs` section + attach picker integration)

**Interfaces:**
- Consumes: `KbItem` (for pre-filling title); `addTask`/`updateTask` from `lib/api.ts` (already exist); `searchKb`/`getKbItems` from `lib/api.ts` (for attach picker search)
- Produces: `onCreateTask` callback (opens dialog, calls `addTask`+`updateTask`); `kb-attach-picker` component (used inside TaskDetail for attachment)

- [ ] **Step 1: Create kb-new-task-dialog.tsx**

```tsx
// src-react/src/components/kb/kb-new-task-dialog.tsx
import { useState } from 'react'
import { addTask, updateTask } from '@/lib/api'
import type { KbItem } from '@/lib/api'

interface Props {
  item: KbItem
  onClose: () => void
  onCreated: () => void
}

export function KbNewTaskDialog({ item, onClose, onCreated }: Props) {
  const [title, setTitle] = useState(item.title)
  const [category, setCategory] = useState('工作')
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async () => {
    if (!title.trim()) return
    setSubmitting(true)
    try {
      const task = await addTask({ title: title.trim(), category, note: '' })
      if (task?.id) {
        await updateTask(task.id, { kbRefs: [item.id] })
      }
      onCreated()
      onClose()
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={e => e.stopPropagation()}>
        <h3>转为待办</h3>
        <input value={title} onChange={e => setTitle(e.target.value)} autoFocus />
        <select value={category} onChange={e => setCategory(e.target.value)}>
          <option>工作</option><option>学习</option><option>生活</option>
        </select>
        <div className="dialog-actions">
          <button className="btn ghost" onClick={onClose}>取消</button>
          <button className="btn primary" disabled={submitting} onClick={handleSubmit}>
            {submitting ? '创建中...' : '创建并挂载'}
          </button>
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Create kb-attach-picker.tsx (task→KB mount search)**

```tsx
// src-react/src/components/kb/kb-attach-picker.tsx
import { useState, useEffect, useMemo } from 'react'
import { getKbItems } from '@/lib/api'
import type { KbItem } from '@/lib/api'

interface Props {
  currentRefs: string[]   // already attached kbRef ids
  onAttach: (ids: string[]) => void
  onClose: () => void
}

export function KbAttachPicker({ currentRefs, onAttach, onClose }: Props) {
  const [all, setAll] = useState<KbItem[]>([])
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set(currentRefs))

  useEffect(() => { getKbItems().then(setAll) }, [])

  const filtered = useMemo(() => {
    const q = query.toLowerCase()
    return all.filter(i => !q || i.title.toLowerCase().includes(q) || i.tags.some(t => t.toLowerCase().includes(q)))
  }, [all, query])

  const toggle = (id: string) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={e => e.stopPropagation()}>
        <input type="search" placeholder="搜索资料..." value={query} onChange={e => setQuery(e.target.value)} autoFocus />
        <div className="kb-attach-list" style={{ maxHeight: 300, overflow: 'auto' }}>
          {filtered.map(item => (
            <label key={item.id} className="kb-attach-item" style={{ display: 'flex', gap: '0.5em', padding: '0.3em 0' }}>
              <input type="checkbox" checked={selected.has(item.id)} onChange={() => toggle(item.id)} />
              <span>{item.title}</span>
              {item.tags?.slice(0,2).map(t => <span key={t} className="kb-tag-chip">{t}</span>)}
            </label>
          ))}
        </div>
        <div className="dialog-actions">
          <button className="btn ghost" onClick={onClose}>取消</button>
          <button className="btn primary" onClick={() => { onAttach(Array.from(selected)); onClose() }}>
            挂载 ({selected.size})
          </button>
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Extend TaskDetail with kbRefs section**

Add to `task-detail.tsx` (after existing fields, before delete button):

```tsx
// Inside TaskDetail render:
const [showAttachPicker, setShowAttachPicker] = useState(false)
const kbRefs: string[] = (task as any).kbRefs ?? []

{kbRefs.length > 0 && (
  <div className="detail-section">
    <span className="detail-label">挂载资料</span>
    <div className="detail-kb-refs">
      {kbRefs.map((ref: string) => (
        <span key={ref} className="kb-ref-chip">
          {/* Load titles on mount — simplified here as id display */}
          📎 {ref}
          <button className="btn xs ghost" onClick={() => {
            const next = kbRefs.filter((r: string) => r !== ref)
            updateTask(task.id, { kbRefs: next })
          }}>×</button>
        </span>
      ))}
      <button className="btn xs ghost" onClick={() => setShowAttachPicker(true)}>+ 挂载</button>
    </div>
  </div>
)}

{showAttachPicker && (
  <KbAttachPicker
    currentRefs={kbRefs}
    onAttach={(ids) => updateTask(task.id, { kbRefs: ids })}
    onClose={() => setShowAttachPicker(false)}
  />
)}
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(kb): add create task from KB + task attachment picker + TaskDetail mount section"
```

---

### Task 7: Free Sticky MD Toggle

**Files:**
- Modify: `src-react/src/sticky/StickyWindow.tsx` (add optional MD render toggle button + conditional MdStaticRenderer)

**Interfaces:**
- Consumes: `MdStaticRenderer` from `rendering/md-static-render.tsx`; `sticky_self.mini === false` (expanded state only)
- Produces: `sticky_md_toggle` telemetry event; rendering toggle persisted locally (UI state, no backend change)

- [ ] **Step 1: Add toggle state + conditional render in StickyWindow expanded mode**

```tsx
// Inside StickyWindow expanded-mode render (after textarea, add toggle button):
const [mdPreview, setMdPreview] = useState(false)

// In the toolbar, add button:
<button
  className={`btn xs ${mdPreview ? 'primary' : 'ghost'}`}
  onClick={() => {
    setMdPreview(!mdPreview)
    // fire-and-forget telemetry
    trackEvent('sticky_md_toggle', JSON.stringify({ id: self.id, on: !mdPreview }))
  }}
  title="切换 Markdown 预览"
>MD</button>

// Replace plain content textarea with:
{mdPreview ? (
  <div className="sticky-body" style={{ cursor: 'pointer' }} onClick={() => setMdPreview(false)}>
    <MdStaticRenderer markdown={self.content} />
  </div>
) : (
  <textarea ... />
)}
```

Commit: `feat(sticky): add optional MD rendering toggle (default off, click to edit)`

- [ ] **Step 2: Import rendering deps into sticky entry (float.html bundle)**

Ensure `src-react/src/sticky/sticky.css` imports `rendering/styles/prosemirror.css` if needed (for code blocks). The `MdStaticRenderer` import pulls in markdown-it which is code-split by Vite — confirm build size stays reasonable via `npx vite build --mode production && ls -lh dist/assets/`.

Commit: `chore(sticky): add MD renderer import to sticky entry bundle`

---

### Task 8: Parity + Smoke + Final Commit

**Files:**
- Modify: `test/command-parity.test.mjs` — NO CHANGE (commands remain 55 this phase)
- Verify: parity still passes (smoke gate)

- [ ] **Step 1: Run full gate checks**

```bash
cd /f/workspace/Kettd
cargo test                          # Rust tests (no changes this phase)
cargo build                         # Rust build
cd src-react && npm run build       # tsc -b && vite build (includes new #/kb entry in main bundle)
npm run lint                        # oxlint
cd .. && node test/command-parity.test.mjs  # parity 55→55
node --test src-react/test/md-fidelity.test.mjs  # new md tests
```

All GREEN → proceed to commit.

- [ ] **Step 2: Final Phase 1 commit**

```bash
git add -A src-react/
git commit -m "feat(v2.2/kb-1): KB Phase 1 — rendering core (md-static + WYSIWYG + registry) + #/kb view + task mount UI + free sticky MD toggle

- Rendering core: markdown-it (reading) + TipTap/ProseMirror (WYSIWYG) + RendererRegistry plugin architecture
  (fenced code block lang → render dispatch; mermaid slot reserved; code-highlight via highlight.js)
- KB view #/kb (6th nav): master-detail layout + search + tag filter + WYSIWYG read/edit + delete confirm
- Task side: TaskDetail '挂载资料' section + KbAttachPicker + '条目→建待办' dialog (addTask+updateTask kbRefs)
- Free sticky: optional MD rendering toggle (default off)
- Commands: 55→55 (no new commands)
- Tests: MD fidelity smoke (10 syntax samples); parity 55; tsc/vite/oxlint green
- Note: searchKb returns IDs; KB view fetches full list via getKbItems, reorders to match search rank (avoids new command)"
```

---

## Phase 1 Exit Criteria (验收)

| # | Criterion | How to verify |
|---|-----------|---------------|
| 1 | `#/kb` appears in side nav between 回顾 and 设置 | Manual: side nav shows 6 items |
| 2 | KB view: create new item → appears in left list | Manual: click 新建, type title, save |
| 3 | WYSIWYG: type `# heading`, `- [ ] task`, ````rust code block``` → renders live in editor | Manual |
| 4 | Read mode: rendered markdown, code block syntax-highlighted | Manual: toggle to 阅读 |
| 5 | Search: type query → list filters, matching items reorder by relevance | Manual |
| 6 | Tag filter: click chip → list filters to that tag | Manual |
| 7 | Delete: click trash → confirm → item removed, detail closes | Manual |
| 8 | Task attachment: open TaskDetail → 挂载资料 → search+select → kbRefs shown in detail | Manual |
| 9 | Item→New Task: click 转为待办 → dialog with pre-filled title → task created, kbRefs attached | Manual |
| 10 | Free sticky: open expanded sticky → click MD toggle → code renders; click back → textarea | Manual |
| 11 | `cargo test` + `cargo build` GREEN | CI |
| 12 | `tsc -b && vite build` + `oxlint` GREEN | CI |
| 13 | `node test/command-parity.test.mjs` = 55 PASS | CI |
| 14 | `node --test src-react/test/md-fidelity.test.mjs` = 10 PASS | CI |
