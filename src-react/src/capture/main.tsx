import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@/index.css'
import CaptureWindow from '@/capture/CaptureWindow'

createRoot(document.getElementById('cap-root')!).render(
  <StrictMode>
    <CaptureWindow />
  </StrictMode>,
)
