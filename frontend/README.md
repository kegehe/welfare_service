# Welfare Service Admin - React 前端

基于 React 18 + TypeScript + Ant Design 5 + Zustand 构建的管理后台前端。

## 技术栈

| 技术 | 版本 | 用途 |
|------|------|------|
| React | 18.x | UI 框架 |
| TypeScript | 5.7 | 类型安全 |
| Ant Design | 5.x | UI 组件库 |
| Zustand | 5.x | 状态管理 |
| ECharts | 5.x | 图表库 |
| Vite | 6.x | 构建工具 |

## 功能特性

### 5 个主要 Tab 页面

1. **总览** - 系统概览统计、实时活跃密钥 SSE 推送
2. **用量统计** - 请求量、Token 用量、趋势图表
3. **号池管理** - Key 的卡片/表格双视图、CRUD 操作、健康评分
4. **访问 Key** - 接入说明、Key 管理、配置模板
5. **对话测试** - OpenAI/Claude 双协议、流式 SSE、思考过程显示

### 核心功能

- ✅ 号池 Key 管理（添加、编辑、删除、启用/禁用、测试连通性）
- ✅ 访问 Key 管理（创建、编辑、删除、启用/禁用）
- ✅ 实时活跃密钥 SSE 推送
- ✅ 用量统计图表（ECharts）
- ✅ 健康评分可视化
- ✅ 流式对话测试（支持 OpenAI + Claude 协议）
- ✅ 一键复制配置模板
- ✅ 响应式设计

## 开发

```bash
# 安装依赖
npm install

# 启动开发服务器
npm run dev

# 类型检查
npm run build
```

## 构建

```bash
# 生产构建
npm run build
```

构建输出到 `../static/` 目录，会被后端服务。

## 目录结构

```
frontend/
├── src/
│   ├── main.tsx                    # 应用入口
│   ├── App.tsx                     # 根组件
│   ├── api/                        # API 调用层
│   │   ├── index.ts                # 通用 fetch 封装
│   │   ├── admin.ts                # 管理 API
│   │   └── chat.ts                 # 对话 API
│   ├── types/                      # TypeScript 类型定义
│   │   └── index.ts
│   ├── stores/                     # Zustand 状态管理
│   │   └── useAppStore.ts
│   ├── hooks/                      # 自定义 Hooks
│   │   ├── useAutoRefresh.ts       # 定时轮询
│   │   ├── useCopyText.ts          # 剪贴板复制
│   │   ├── useEffectiveBaseUrl.ts  # Base URL 计算
│   │   └── useModelPresets.ts      # 模型预设缓存
│   ├── components/                 # React 组件
│   │   ├── layout/                 # 布局组件
│   │   ├── overview/               # 总览页面
│   │   ├── usage/                  # 用量统计
│   │   ├── pool/                   # 号池管理
│   │   ├── access/                 # 访问 Key
│   │   ├── chat/                   # 对话测试
│   │   └── common/                 # 通用组件
│   └── styles/                     # 样式文件
│       ├── variables.css           # CSS 变量设计系统
│       ├── global.css              # 全局样式
│       └── components.css          # 组件样式
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## 依赖说明

- **Ant Design**: 成熟的 UI 组件库，中文生态好
- **Zustand**: 轻量级状态管理，API 简洁
- **ECharts**: 强大的图表库，支持复杂可视化
- **echarts-for-react**: ECharts 的 React 封装

## 注意事项

1. **构建产物**: 构建输出到 `../static/`，会被后端 Rust 服务
2. **API 代理**: 开发时自动代理到 `http://127.0.0.1:8080`
3. **TypeScript**: 严格模式，所有类型都需要明确定义
4. **样式**: 使用 CSS 变量系统，便于主题定制

## 后续优化

- [ ] 添加单元测试
- [ ] 实现路由懒加载
- [ ] 添加国际化支持
- [ ] 优化 ECharts 按需加载
- [ ] 添加 PWA 支持
