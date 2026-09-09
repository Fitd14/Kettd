# KB Phase 2 — Knowledge Sticky + Task Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add knowledge-sticky (KB item pinned to desktop as a movable viewport), improve task↔KB attachment UI, and implement the "Item→New Task" reverse flow — all wiring into the rendering core and KB view from Phase 1.

**Architecture:** `StickyNote.kb_ref` is the data seam: a new optional field pointing to a `KbItem.id`. When present, the sticky window renders the KB item's body_md via `TipTapEditor`/`MdStaticRenderer` (the same rendering core from Phase 1), with edits debounced back to `update_kb_item`. Deleting the KB item triggers a "已失效" state in the sticky. The sticky cap is split: free-sticky ≤6 (unchanged), knowledge-sticky ≤4 (separate counter). Task attachment UI gains a richer attach picker and inline KB item preview chips.

**Tech Stack:** (Same as Phase 1 — no new deps this phase.) Rust: `models.rs`, `store.rs`, `commands.rs` changes (small: `StickyNote.kb_ref` field + `create_sticky` param + `add_task` kb_refs acceptance). Frontend: `StickyWindow.tsx` extension, `task-detail.tsx` enrichment.

**Spec:** `spark-output/design/kb-spec.md` §2 (data contract), §3 (knowledge sticky dual-track), §6 (task-side), §7 (sticky×KB behavior), §9 (validation gate items)

**Global Constraints:**
- Commands stay at **55** this phase (StickyNote.kb_ref is a field extension, not new commands; `create_sticky` param addition doesn't change command count)
- Parity test remains 55→55
- Knowledge sticky cap: ≤4 (split-pool with free-sticky ≤6)
- `StickyNote.kb_ref` uses `skip_serializing_if = "is_none"` → old data zero-migration
- Knowledge-sticky edits write back to `update_kb_item` (debounced 800ms); closing sticky = `delete_sticky` (only destroys window, KB data intact)
- When KB item deleted → knowledge-sticky shows "已失效" + destroy button (no auto-close)

---

## File Structure (Phase 2)

| Action | File | Responsibility |
|--------|------|---------------|
| Modify | `src-tauri/src/models.rs` | Add `kb_ref: Option<String>` to `StickyNote` (serde skip_if_none) |
| Modify | `src-tauri/src/store.rs` | `normalize_stickies_v4`: ensure kb_ref field exists (optional migration; skip_if_none handles gracefully) |
| Modify | `src-tauri/src/commands.rs` | `create_sticky`: accept optional `kb_ref` param; `update_sticky`: if kb_ref present, delegate to `update_kb_item` instead of updating content; add `STICKY_KB_CAP = 4` constant; split pool counting logic |
| Modify | `src-tauri/src/runtime.rs` | `create_note`: pass `kb_ref` through; knowledge-sticky title from KbItem on build |
| Modify | `src-react/src/lib/api.ts` | `StickyNoteItem.kbRef?: string`; `createSticky({kbRef?})`; `StickySelf.kbRef?: string` |
| Modify | `src-react/src/sticky/StickyWindow.tsx` | Knowledge-sticky mode: render `TipTapEditor`/`MdStaticRenderer` from KB data; on delete show "已失效"; debounce edits to `update_kb_item` |
| Modify | `src-react/src/components/task-detail.tsx` | Richer kbRefs display: load KbItem titles via `getKbItems`, show chips with titles, jump-to-kb button |
| Modify | `src-react/src/views/SettingsView.tsx` | Display knowledge-sticky count in 便签 management section |

---

## Tasks

### Task 1: Rust — StickyNote.kb_ref field + migration + split-pool cap

**Files:**
- Modify: `src-tauri/src/models.rs:408-430` (StickyNote struct)
- Modify: `src-tauri/src/store.rs` (normalize_stickies_v4 — optional, for safety)
- Modify: `src-tauri/src/commands.rs` (create_sticky param, split-pool counter)

**Interfaces:**
- Consumes: existing `StickyNote` struct; existing `create_sticky` command
- Produces: `StickyNote.kb_ref: Option<String>` (serde skip_if_none); `create_sticky({kb_ref?})` command; split pool: `count_stickies_by_type()` helper; `STICKY_KB_CAP = 4`

- [ ] **Step 1: Add kb_ref to StickyNote in models.rs**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StickyNote {
  pub id: String,
  pub content: String,          // free-sticky ≤500; knowledge-sticky: unused (KB holds body)
  pub mini: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kb_ref: Option<String>,   // Phase 2: knowledge-sticky viewport pointer
  pub created_at: String,
  pub updated_at: String,
}
```

Also update Default impl and any place `StickyNote` is constructed (commands.rs create_sticky).

- [ ] **Step 2: Add normalize_stickies_v4 in store.rs (defensive)**

```rust
/// v4 migration: ensure kb_ref field exists (serde default handles missing; this is belt-and-suspenders)
fn normalize_stickies_v4(data: &mut Value) -> bool {
  if let Some(stickies) = data.get_mut("stickies").and_then(Value::as_array_mut) {
    let mut changed = false;
    for note in stickies.iter_mut() {
      if note.get("kb_ref").is_none() {
        note["kb_ref"] = Value::Null; // serde default → None on deserialization
        changed = true;
      }
    }
    changed
  } else {
    false
  }
}
```

Call in `load_with` after v3 migration, before `from_value`.

- [ ] **Step 3: Add split-pool cap in commands.rs**

```rust
const STICKY_KB_CAP: usize = 4;

// In create_sticky command, before building window:
let kb_count = store.data.stickies.iter().filter(|s| s.kb_ref.is_some()).count();
if kb_ref.is_some() && kb_count >= STICKY_KB_CAP {
  return Err(format!("知识便签已达上限（{}张），请先拆掉不需要的", STICKY_KB_CAP));
}
// (Free-sticky cap check remains at existing STICKY_CAP minus kb_count — or: keep separate caps)
```

- [ ] **Step 4: Pass kb_ref through create_sticky command**

Update `create_sticky` command signature to accept `kb_ref: Option<String>`:

```rust
#[tauri::command]
pub async fn create_sticky(
  state: tauri::State<'_, crate::runtime::RuntimeState>,
  store: tauri::State<'_, crate::store::Store>,
  kb_ref: Option<String>,
) -> Result<String, String> {
  // ... existing logic, pass kb_ref to runtime::create_note(app, kb_ref)
}
```

Update `main.rs` generate_handler if needed (parameter already optional, no change needed if Rust matches).

- [ ] **Step 5: Rust tests — split pool + kb_ref construction**

```rust
#[test]
fn create_sticky_with_kb_ref_respects_knowledge_cap() {
  // ... fill 4 knowledge stickies → 5th fails
}

#[test]
fn create_sticky_without_kb_ref_uses_free_cap() {
  // ... fill 6 free stickies → 7th fails; kb_ref ones don't count toward free cap
}
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/store.rs src-tauri/src/commands.rs src-tauri/src/runtime.rs
git commit -m "feat(kb/sticky): add StickyNote.kb_ref + split-pool cap (free≤6, knowledge≤4)"
```

---

### Task 2: Frontend API + StickyWindow knowledge-sticky mode

**Files:**
- Modify: `src-react/src/lib/api.ts` (StickyNoteItem.kbRef, createSticky param, StickySelf.kbRef)
- Modify: `src-react/src/sticky/StickyWindow.tsx` (knowledge-sticky rendering branch)

**Interfaces:**
- Consumes: `getKbItems()` / `updateKbItem()` from api.ts; `TipTapEditor`/`MdStaticRenderer` from rendering core; `kb_ref` from StickyNote
- Produces: Knowledge-sticky renders KB item content; edits debounced to update_kb_item; deletion triggers "已失效" state

- [ ] **Step 1: Update api.ts types**

```ts
// In StickyNoteItem:
export interface StickyNoteItem {
  id: string; content: string; mini: boolean;
  kbRef?: string | null;   // Phase 2
  createdAt: string; updatedAt: string;
}

// In StickySelf:
export interface StickySelf {
  id: string; content: string; mini: boolean;
  kbRef?: string | null;   // Phase 2
}

// Update createSticky:
export const createSticky = (args?: { kbRef?: string }) =>
  call<string>('create_sticky', { args: args ?? {} })
```

- [ ] **Step 2: Extend StickyWindow for knowledge-sticky mode**

In `StickyWindow.tsx`, detect `self.kbRef`:

```tsx
const [kbItem, setKbItem] = useState<KbItem | null>(null)
const [kbInvalid, setKbInvalid] = useState(false)

useEffect(() => {
  if (!self.kbRef) return
  getKbItems().then(items => {
    const found = items.find(i => i.id === self.kbRef)
    if (found) setKbItem(found)
    else setKbInvalid(true)
  }).catch(() => setKbInvalid(true))
}, [self.kbRef])

// In render — knowledge-sticky branch:
if (self.kbRef) {
  if (kbInvalid) {
    return (
      <div className="sticky-note is-invalid">
        <span>已失效（原资料已删除）</span>
        <button onClick={() => deleteSticky(self.id)}>拆除便签</button>
      </div>
    )
  }
  if (!kbItem) return <div className="sticky-note">加载中...</div>

  return (
    <div className="sticky-note knowledge-sticky">
      {/* Header: KB item title + close button */}
      <div className="sticky-header" data-tauri-drag-region>
        <span className="sticky-title">{kbItem.title}</span>
        <button className="btn xs ghost" onClick={() => deleteSticky(self.id)}>×</button>
      </div>
      {/* Body: WYSIWYG editor editing KB item bodyMd */}
      <div className="sticky-body">
        <TipTapEditor
          markdown={kbItem.bodyMd}
          onSave={(md) => updateKbItem(self.kbRef!, { bodyMd: md })}
          autoSaveMs={800}
        />
      </div>
    </div>
  )
}
```

Free-sticky path remains unchanged (≤500 + optional MD toggle from Phase 1).

- [ ] **Step 3: Wire "贴到桌面" button in KB view detail**

In `kb-item-detail.tsx`, enable the pin button (was disabled Phase 1):

```tsx
<button className="btn xs ghost" onClick={async () => {
  await createSticky({ kbRef: item.id })
  // emit telemetry
  trackEvent('kb_pin_desktop', JSON.stringify({ itemId: item.id }))
}} title="贴到桌面">📌 贴到桌面</button>
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(kb/sticky): knowledge-sticky viewport mode + KB '贴到桌面' button + api types"
```

---

### Task 3: TaskDetail enrichment — KB item title display + jump-to-kb

**Files:**
- Modify: `src-react/src/components/task-detail.tsx`

**Interfaces:**
- Consumes: `getKbItems()` to load title map for kbRefs; `KbItem.id → title` lookup
- Produces: Richer display: chip showing item title, click jumps to `#/kb?id={itemId}`

- [ ] **Step 1: Load KB item titles on mount, render rich chips**

```tsx
// In TaskDetail, add state:
const [kbItems, setKbItems] = useState<Map<string, KbItem>>(new Map())
const kbRefs: string[] = (task as any).kbRefs ?? []

useEffect(() => {
  if (kbRefs.length > 0) {
    getKbItems().then(items => {
      setKbItems(new Map(items.map(i => [i.id, i])))
    })
  }
}, [kbRefs.join(',')])

// Render kbRefs section:
{kbRefs.length > 0 && (
  <div className="detail-section">
    <span className="detail-label">挂载资料 ({kbRefs.length})</span>
    <div className="detail-kb-refs">
      {kbRefs.map(ref => {
        const item = kbItems.get(ref)
        return (
          <span key={ref} className={`kb-ref-chip ${item ? '' : 'invalid'}`}>
            📎 {item?.title ?? '已失效'}
            {item && (
              <button className="btn xs ghost" onClick={() => {
                window.location.hash = `#/kb?id=${ref}`
              }}>↗</button>
            )}
            <button className="btn xs ghost" onClick={() => {
              const next = kbRefs.filter(r => r !== ref)
              updateTask(task.id, { kbRefs: next })
            }}>×</button>
          </span>
        )
      })}
    </div>
  </div>
)}
```

- [ ] **Step 2: Handle `#/kb?id=xxx` deep-link in KbView**

Add to `KbView.tsx`: on mount, read `window.location.hash` for `?id=` param, auto-select that item:

```tsx
useEffect(() => {
  const hash = window.location.hash
  const idMatch = hash.match(/[?&]id=([^&]+)/)
  if (idMatch) setSelectedId(decodeURIComponent(idMatch[1]))
}, [])
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(kb/task): enrich TaskDetail with KB item titles + deep-link jump to KB"
```

---

### Task 4: SettingsView knowledge-sticky count + parity gate

**Files:**
- Modify: `src-react/src/views/SettingsView.tsx` (show knowledge-sticky count in 便签 section)

- [ ] **Step 1: Display split-pool counts in SettingsView**

In the 便签管理 section, add: `自由便签 {freeCount} / 知识便签 {kbCount}（上限 {STICKY_KB_CAP}）` — read from `stickleList` in boot data (filter by `kbRef` presence).

- [ ] **Step 2: Full gate checks**

```bash
cargo test && cargo build
cd src-react && npm run build && npm run lint
cd .. && node test/command-parity.test.mjs   # still 55
```

All GREEN → commit.

- [ ] **Step 3: Final Phase 2 commit**

```bash
git add -A src-tauri/ src-react/
git commit -m "feat(v2.2/kb-2): KB Phase 2 — knowledge-sticky viewport + split-pool cap + task KB enrichment

- StickyNote.kb_ref field (serde skip_if_none, zero-migration)
- Knowledge-sticky: WYSIWYG edits debounced to update_kb_item; '已失效' state on KB deletion
- Split pool: free ≤6 / knowledge ≤4
- '贴到桌面' button in KB view detail toolbar
- TaskDetail: KB item title chips + deep-link jump to /#/kb?id=
- Parity: 55→55 (no new commands)
- Gates: cargo test/build + tsc/vite/oxlint + parity"
```

---

## Phase 2 Exit Criteria

| # | Criterion | Verify |
|---|-----------|--------|
| 1 | Click "贴到桌面" on KB item → knowledge-sticky window opens showing KB content | Manual |
| 2 | Edit in knowledge-sticky → main KB view shows updated content (sync) | Manual |
| 3 | Close knowledge-sticky → KB data still exists (window destroyed only) | Manual |
| 4 | Delete KB item → knowledge-sticky shows "已失效" + destroy button | Manual |
| 5 | Knowledge-sticky 5th created → blocked with error message | Manual |
| 6 | Free sticky still works independently (≤6 cap) | Manual |
| 7 | TaskDetail shows KB item titles + can jump to KB | Manual |
| 8 | "条目→建待办" creates task with kbRefs attached | Manual |
| 9 | Parity still 55 | CI |
