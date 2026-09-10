import { useEffect, useState } from 'react'
import {
  getAiStatus,
  setAiConfig,
  testAiConnection,
  type AiStatus,
} from '@/lib/api'

/** 厂商预设：选中即填 URL+模型默认值（kb-phase3-supplement §1.2） */
interface VendorPreset {
  key: string
  label: string
  baseUrl: string
  chatModel: string
  embeddingModel: string
  /** Ollama 本地免 KEY */
  noKey?: boolean
  /** DeepSeek 无独立 embedding API → embedding 轨不提供此预设 */
  noEmbedding?: boolean
}

const PRESETS: VendorPreset[] = [
  { key: 'openai', label: 'OpenAI', baseUrl: 'https://api.openai.com/v1', chatModel: 'gpt-4o-mini', embeddingModel: 'text-embedding-3-small' },
  { key: 'deepseek', label: 'DeepSeek', baseUrl: 'https://api.deepseek.com', chatModel: 'deepseek-chat', embeddingModel: '', noEmbedding: true },
  { key: 'zhipu', label: '智谱', baseUrl: 'https://open.bigmodel.cn/api/paas/v4', chatModel: 'glm-4-flash', embeddingModel: 'embedding-2' },
  { key: 'qwen', label: '通义', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1', chatModel: 'qwen-turbo', embeddingModel: 'text-embedding-v3' },
  { key: 'ollama', label: 'Ollama（本地）', baseUrl: 'http://127.0.0.1:11434', chatModel: 'llama3.2', embeddingModel: 'nomic-embed-text', noKey: true },
  { key: 'custom', label: '自定义', baseUrl: '', chatModel: '', embeddingModel: '' },
]

interface TrackForm {
  preset: string
  baseUrl: string
  model: string
  apiKey: string
}

const emptyTrack: TrackForm = { preset: 'custom', baseUrl: '', model: '', apiKey: '' }

type TestState = { ok: boolean; msg: string } | null

/**
 * 设置页「AI 与检索」段：总开关 + Chat/Embedding 双轨配置 + 每轨测试连接。
 *
 * KEY 安全：输入框只在提交时把值交给 set_ai_config（Rust DPAPI 加密落 secrets.json），
 * 回显永远只有掩码；KEY 输入为空时提交不携带该字段（防误清已存 KEY）。
 */
export function AiSection() {
  const [status, setStatus] = useState<AiStatus | null>(null)
  const [chat, setChat] = useState<TrackForm>(emptyTrack)
  const [embedding, setEmbedding] = useState<TrackForm>(emptyTrack)
  const [saving, setSaving] = useState(false)
  const [saveMsg, setSaveMsg] = useState<string | null>(null)
  const [chatTest, setChatTest] = useState<TestState>(null)
  const [embeddingTest, setEmbeddingTest] = useState<TestState>(null)

  useEffect(() => {
    void getAiStatus().then((r) => {
      if (r.err || !r.data) return
      setStatus(r.data)
      setChat((f) => ({
        ...f,
        baseUrl: r.data?.chat.baseUrl ?? '',
        model: r.data?.chat.model ?? '',
      }))
      setEmbedding((f) => ({
        ...f,
        baseUrl: r.data?.embedding.baseUrl ?? '',
        model: r.data?.embedding.model ?? '',
      }))
    })
  }, [])

  const applyPreset = (track: 'chat' | 'embedding', key: string) => {
    const preset = PRESETS.find((p) => p.key === key)
    const patch = (f: TrackForm): TrackForm => ({
      ...f,
      preset: key,
      baseUrl: preset?.baseUrl ?? '',
      model: track === 'chat' ? (preset?.chatModel ?? '') : (preset?.embeddingModel ?? ''),
    })
    if (track === 'chat') setChat(patch)
    else setEmbedding(patch)
  }

  const save = async () => {
    setSaving(true)
    setSaveMsg(null)
    try {
      const patch = {
        baseUrl: chat.baseUrl.trim(),
        model: chat.model.trim(),
        // KEY 空 = 不改动已存 KEY（Rust patch 语义 Some("") 会覆盖，这里显式跳过）
        apiKey: chat.apiKey.trim() || undefined,
        embeddingBaseUrl: embedding.baseUrl.trim(),
        embeddingModel: embedding.model.trim(),
        embeddingApiKey: embedding.apiKey.trim() || undefined,
      }
      const r = await setAiConfig(patch)
      if (!r.err && r.data) {
        setStatus(r.data)
        setSaveMsg('✓ 已保存')
        setChat((f) => ({ ...f, apiKey: '' }))
        setEmbedding((f) => ({ ...f, apiKey: '' }))
      } else {
        setSaveMsg(`✗ ${r.err ?? '保存失败'}`)
      }
    } finally {
      setSaving(false)
    }
  }

  const runTest = async (track: 'chat' | 'embedding') => {
    const set = track === 'chat' ? setChatTest : setEmbeddingTest
    set({ ok: false, msg: '测试中…' })
    const r = await testAiConnection(track)
    set({ ok: r.data?.ok ?? false, msg: r.data?.message ?? r.err ?? '未知错误' })
  }

  const segBtn = (on: boolean, label: string, onClick: () => void) => (
    <button className={`btn sm ${on ? 'on' : ''}`} onClick={onClick}>{label}</button>
  )

  const trackRow = (
    track: 'chat' | 'embedding',
    form: TrackForm,
    setForm: (f: TrackForm) => void,
    st: { configured: boolean; model?: string; baseUrl?: string },
    test: TestState,
    presets: VendorPreset[],
  ) => (
    <div className="ai-track">
      <div className="ai-track-head">
        <b className="tiny">{track === 'chat' ? 'Chat 模型' : 'Embedding 模型（语义检索）'}</b>
        <span className={`tiny ${st.configured ? '' : 'text-muted'}`}>
          {st.configured ? `✓ 已配置 ${st.model ?? ''}` : '未配置'}
        </span>
      </div>
      <div className="ai-grid">
        <label className="tiny text-muted">厂商预设</label>
        <select
          value={form.preset}
          onChange={(e) => applyPreset(track, e.target.value)}
        >
          {presets.map((p) => (
            <option key={p.key} value={p.key}>{p.label}</option>
          ))}
        </select>
        <label className="tiny text-muted">BaseURL</label>
        <input
          value={form.baseUrl}
          onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
          placeholder="https://..."
        />
        <label className="tiny text-muted">模型名</label>
        <input
          value={form.model}
          onChange={(e) => setForm({ ...form, model: e.target.value })}
          placeholder={track === 'chat' ? 'gpt-4o-mini' : 'text-embedding-3-small'}
        />
        <label className="tiny text-muted">API KEY</label>
        <input
          type="password"
          value={form.apiKey}
          onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
          placeholder={st.configured ? `已存储（${status?.keyMask ?? '掩码'}）· 留空=不改动` : '输入 API KEY'}
          autoComplete="off"
        />
      </div>
      <div className="ai-track-actions">
        <button className="btn xs outline" onClick={() => void runTest(track)}>测试连接</button>
        {test && (
          <span className={`tiny ${test.ok ? '' : 'text-muted'}`}>
            {test.ok ? '✓' : '✗'} {test.msg}
          </span>
        )}
      </div>
    </div>
  )

  return (
    <section className="section" aria-label="AI 与检索">
      <div className="section-head"><b>AI 与检索</b><span>混合检索 · 本地全文 + AI 语义</span></div>

      <div className="set-row"><span>总开关
        <div className="tiny text-muted">关闭时搜索完全本地，不调用任何外部 API</div>
      </span>
        <div className="seg" role="group" aria-label="AI 总开关">
          {segBtn(status?.enabled === true, '开', () => {
            void setAiConfig({ enabled: true }).then((r) => { if (!r.err && r.data) setStatus(r.data) })
          })}
          {segBtn(status?.enabled !== true, '关', () => {
            void setAiConfig({ enabled: false }).then((r) => { if (!r.err && r.data) setStatus(r.data) })
          })}
        </div>
      </div>

      {trackRow('chat', chat, setChat, status?.chat ?? { configured: false }, chatTest, PRESETS)}
      {trackRow(
        'embedding',
        embedding,
        setEmbedding,
        status?.embedding ?? { configured: false },
        embeddingTest,
        PRESETS.filter((p) => !p.noEmbedding),
      )}

      <div className="set-row"><span className="row-flex items-center gap-2">
        <button className="btn sm primary" disabled={saving} onClick={() => { void save() }}>
          {saving ? '保存中…' : '保存配置'}
        </button>
        {saveMsg && <span className="tiny text-muted">{saveMsg}</span>}
      </span>
        <span className="tiny text-muted" style={{ lineHeight: 1.5 }}>
          🔒 KEY 经 DPAPI 加密存储本机，日志与界面永不回显明文；未配置 Embedding 时搜索仅字面匹配；
          Ollama 本地（127.0.0.1）免 KEY。
        </span>
      </div>
    </section>
  )
}
