/**
 * 纸面中式纹样（sticky-background-pattern.md 定稿）：
 * 墨竹一枝（左下竖构图）/ 远山横带（底部三层水墨）/ 朱砂小印（折角上方点睛）。
 * 全部内联 SVG：颜色取 var(--sticky-ink)（四套纸色自适应）或 --sticky-seal（朱砂），
 * 浓度为水印级——纹样永远让位于待办文字。
 */

export function BambooPattern() {
  return (
    <svg className="pattern-bamboo" viewBox="0 0 90 210" aria-hidden="true">
      <g fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round">
        {/* 竿一：直立，分两段留节 */}
        <path d="M30 210 C29 172 31 134 30 98" />
        <path d="M30 93 C29 62 31 32 30 6" />
        <path d="M26.5 95.5 h7" strokeWidth="1.6" />
        {/* 竿二：略斜偏右，更短 */}
        <path d="M57 210 C59 182 56 152 58 120" />
        <path d="M58 115 C59 92 57 70 58 48" />
        <path d="M54.5 117.5 h7" strokeWidth="1.6" />
      </g>
      <g fill="currentColor">
        {/* 竿一竹叶：左右各一簇 */}
        <path d="M30 62 Q45 50 60 52 Q46 62 30 62 Z" />
        <path d="M30 62 Q17 47 3 47 Q16 60 30 62 Z" />
        <path d="M30 26 Q42 16 54 18 Q42 27 30 26 Z" />
        {/* 竿二竹叶 */}
        <path d="M58 98 Q73 88 86 90 Q73 100 58 98 Z" />
        <path d="M58 98 Q47 83 33 83 Q46 96 58 98 Z" />
        <path d="M58 52 Q70 42 82 44 Q70 54 58 52 Z" />
      </g>
    </svg>
  )
}

export function MountainPattern() {
  return (
    <svg
      className="pattern-mountain"
      viewBox="0 0 380 96"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      {/* 远山 → 近坡：三档墨色拉纵深 */}
      <path
        d="M0 66 Q55 34 118 54 Q180 72 236 50 Q300 26 380 58 L380 96 L0 96 Z"
        fill="currentColor"
        fillOpacity="0.05"
      />
      <path
        d="M0 78 Q85 50 170 66 Q265 82 380 68 L380 96 L0 96 Z"
        fill="currentColor"
        fillOpacity="0.08"
      />
      <path
        d="M0 90 Q95 66 200 80 Q295 92 380 82 L380 96 L0 96 Z"
        fill="currentColor"
        fillOpacity="0.11"
      />
    </svg>
  )
}

/** 朱砂小印：折角上方点睛（「记」），纸面唯一彩点，与琥珀图钉互为呼应 */
export function SealBadge() {
  return (
    <svg className="pattern-seal" viewBox="0 0 16 16" aria-hidden="true">
      <rect width="16" height="16" rx="3" fill="var(--sticky-seal)" />
      <text
        x="8"
        y="12.2"
        textAnchor="middle"
        fontSize="9.5"
        fontFamily="'SimSun','NSimSun',serif"
        fill="hsl(42 34% 94%)"
      >
        记
      </text>
    </svg>
  )
}

/** 纸面花纹入口：none 不渲染；bamboo / mountain 自带朱印伴随 */
export function PaperPattern({ pattern }: { pattern: string }) {
  if (pattern !== 'bamboo' && pattern !== 'mountain') return null
  return (
    <div className="sticky-pattern" aria-hidden="true">
      {pattern === 'bamboo' && <BambooPattern />}
      {pattern === 'mountain' && <MountainPattern />}
      <SealBadge />
    </div>
  )
}
