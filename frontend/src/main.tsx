import React from 'react'
import ReactDOM from 'react-dom/client'
import { ConfigProvider } from 'antd'
import zhCN from 'antd/locale/zh_CN'
import App from './App'
import { antdTheme } from './config/theme'
import './styles/variables.css'
import './styles/global.css'
import './styles/components.css'
import './styles/chat.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ConfigProvider locale={zhCN} theme={antdTheme}>
      <App />
    </ConfigProvider>
  </React.StrictMode>
)
