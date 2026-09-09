import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import '@/index.css'
import '@/sticky/sticky.css'
import TodoFloatWindow from '@/todo/TodoFloatWindow'
import './todo.css'

createRoot(document.getElementById('todo-root')!).render(
  <StrictMode>
    <TodoFloatWindow />
  </StrictMode>,
)
