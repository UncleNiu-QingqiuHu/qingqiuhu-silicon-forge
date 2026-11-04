# 青丘湖硅基工坊 · 开发说明

本地文件加工台。当前为 M1 阶段：托盘常驻、Excel 工作表拆分、图片格式转换、设置页。

## 环境要求

| 组件 | 版本 | 说明 |
|---|---|---|
| Rust | stable ≥ 1.77 | MSVC 工具链 |
| Visual Studio | 2019+ | 需 C++ 桌面开发组件 |
| Node.js | ≥ 20 | 项目用 pnpm 管理 |
| pnpm | ≥ 10 | — |
| WebView2 | 任意 | Win11 自带，Win10 需单独装 |

## 运行

```bash
cd app
pnpm install
pnpm tauri dev
```

首次编译约 5–10 分钟（Tauri + umya-spreadsheet + image + resvg）。

## 构建

```bash
cd app
pnpm tauri build
```

产物在 `app/src-tauri/target/release/bundle/`（NSIS + MSI）。

## 目录结构

```
app/
├── src/                      前端
│   ├── App.tsx               布局、导航、退出确认
│   ├── components/
│   │   ├── ExcelPanel.tsx    工作表拆分
│   │   ├── ImagePanel.tsx    图片转换
│   │   ├── SettingsPanel.tsx 设置与功能状态
│   │   ├── Placeholder.tsx   未交付功能的统一占位
│   │   └── ExitDialog.tsx    退出确认
│   └── lib/
│       ├── api.ts            所有后端调用的唯一出口
│       └── types.ts          与 Rust 结构体对应的类型
└── src-tauri/
    ├── features.toml         ★ 功能状态单一数据源
    ├── build.rs              解析 features.toml，编译期生成静态表
    └── src/
        ├── features.rs       功能注册表与未交付拦截
        ├── tray.rs           系统托盘
        ├── excel/
        │   ├── refs.rs       跨表引用检测（纯文本解析）
        │   └── split.rs      拆分主逻辑
        └── imagetool/mod.rs  图片转换
```

## 功能状态管理（重要）

**所有功能是否可用、是否已交付，只在 `src-tauri/features.toml` 定义。**
不要在代码或界面里硬编码功能可用性判断。

```toml
[[feature]]
id = "F2-2"
name = "按工作表拆分"
module = "excel"
status = "committed"    # committed | conditional | deferred | out
milestone = "M1"
delivered = true        # 是否已在本版本交付
```

界面表现由三个字段共同决定：

| 条件 | 界面 |
|---|---|
| `status != committed` | 灰色禁用 + 「待排期」角标 + 前置条件说明 |
| `committed && !delivered` | 「开发中」角标 |
| `committed && delivered` | 真实功能入口 |

**未实现的功能禁止用 `todo!()` / `unimplemented!()`**——那会 panic 让程序崩溃。用 `FeatureError::NotPlanned`，前端会拿到可读提示。

## 测试

```bash
cd app/src-tauri
cargo test --lib
```

当前 20 个测试，含真实 xlsx 文件的端到端拆分验证（扫描 → 拆分 → 回读校验）。

## 已知问题

**编译 `window-vibrancy` 时 rustc 可能崩溃（0xc0000005）**

并行编译叠加完整调试信息会耗尽内存。已在 `Cargo.toml` 里设置 `[profile.dev] debug = 1` 规避。若仍遇到，用单线程编译：

```bash
cargo build -j 1
```

**pnpm 的 esbuild 脚本被拦截**

pnpm 10+ 默认不执行依赖的构建脚本。首次安装后需要：

```bash
cd app && pnpm approve-builds --all
```

否则 vite 构建会失败。

## 界面联调

没有 Tauri 后端时也能在浏览器里调 UI：

```bash
cd app && pnpm dev
```

- `http://localhost:1420/` — 使用开发降级数据
- `?view=settings` — 直达设置页
- Excel 面板加 `?demo=1` — 用示例数据预览列表与跨表引用告警

这些降级逻辑都在 `import.meta.env.DEV` 下，生产构建会被剔除。
