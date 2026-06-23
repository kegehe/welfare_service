import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  build: {
    target: 'esnext',
    outDir: '../static',
    emptyOutDir: true, // 构建时清空 outDir，避免旧 hashed 文件堆积
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-antd': ['antd', '@ant-design/icons', 'react', 'react-dom'],
          'vendor-echarts': ['echarts', 'echarts-for-react'],
        },
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
    },
  },
  server: {
    proxy: {
      '/admin': 'http://127.0.0.1:8080',
      '/v1': 'http://127.0.0.1:8080',
    },
  },
})
