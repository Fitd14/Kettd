import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@/index.css'
import StickyWindow from '@/sticky/StickyWindow'

createRoot(document.getElementById('sticky-root')!).render(
  <StrictMode>
    <StickyWindow />
  </StrictMode>,
)
