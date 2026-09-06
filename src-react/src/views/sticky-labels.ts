/** 便签纸色预设（便签规格 §12.1）——key 与后端 Settings.stickyPaper 契约一致。 */
export const STICKY_PAPERS = [
  { key: 'warm', label: '暖白纸' },
  { key: 'kraft', label: '牛皮纸' },
  { key: 'cyan', label: '淡青' },
  { key: 'ink', label: '暗墨' },
] as const

export type StickyPaperKey = (typeof STICKY_PAPERS)[number]['key']
