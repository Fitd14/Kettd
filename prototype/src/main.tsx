import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import './index.css'
import { KettdApp } from './flows/kettd-app/kettd-app'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <KettdApp />
    </BrowserRouter>
  </StrictMode>,
)
