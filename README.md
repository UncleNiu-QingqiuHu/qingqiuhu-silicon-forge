# 青丘湖硅基工坊

> Qingqiuhu Silicon Forge — 本地文件加工台

公司内部使用的 Windows 桌面工具。把日常重复的文件加工操作收敛到一个应用里：文档转 PDF、Excel 按工作表拆分、Markdown 写文档导正式稿、图片批量转格式。

**全程本地处理，文件不出本机。**

当前版本 **v0.2.0**，已交付 Excel 拆表、图片转换、Markdown 编辑导出、系统托盘常驻。

---

## 为什么做这个

日常办公里有大量重复劳动：交客户的 Word/Excel/PPT 要转 PDF 保版式、一份多工作表 Excel 要按表分给不同部门、技术文档习惯用 Markdown 写但对外要 PDF、产品图片要批量压缩。

现有做法要么依赖在线转换网站（文件外泄风险），要么手工另存（效率低、易漏改）。内网环境不允许文件上传外部服务，所以需要一个完全本地运行的工具。

## 核心能力

| 模块 | 能力 | 状态 |
|---|---|---|
| **Excel 拆表** | 按工作表拆成独立工作簿，保留样式、合并单元格、打印设置；跨表引用检测并给出三种处理策略 | v0.1.0 |
| **图片转换** | PNG/JPG/WebP/BMP/GIF/TIFF/ICO/SVG 互转；缩放、透明铺底、EXIF 方向校正 | v0.1.0 |
| **Markdown** | GFM 编辑与实时预览；导出单文件 HTML 与矢量 PDF，带品牌页眉 | v0.2.0 |
| **托盘常驻** | 关闭入托盘、任务后台继续、退出必须确认、开机自启 | v0.1.0 |
| **文档转 PDF** | Word/Excel/PPT → PDF，MS Office 优先、WPS 降级，附转换自检 | 规划中 M3 |
| **AI 助手** | 自然语言编排复杂任务，仅连内网或本机模型 | 规划中 M5 |

## 设计取舍

**不引入 LibreOffice 兜底。** 目标机器必须装有 MS Office 或 WPS 之一。为一个低概率场景让所有用户多下载 250MB 不划算，而且 LibreOffice 的版式保真度在「交付客户」这个前提下是风险而非方案。

**AI 不做工具已经做好的事。** 单文件转 PDF、改个图片尺寸这类步骤确定的操作用界面直连，不套 AI。AI 只负责那些步骤无法事先写死的任务，比如「把这 30 个报价单按客户分类后各自合成 PDF」。

**最小化永远允许，退出必须确认。** 用户想收窗口干活时不该被任何弹窗拦住；想退出时则应该被提醒一次。

**未交付的功能必须显式标注。** 不允许出现「看起来有、点了没用」的控件。功能状态在 `app/src-tauri/features.toml` 单点定义，编译期注入，界面据它渲染。

## 技术栈

| 层 | 选型 |
|---|---|
| 桌面框架 | Tauri 2 |
| 后端 | Rust（stable） |
| 前端 | React 19 + TypeScript 7 + Vite 8 |
| Excel | umya-spreadsheet 3 |
| 图片 | image + resvg + kamadak-exif |
| Markdown | comrak + syntect |
| PDF 生成 | Typst（进程内，矢量输出） |
| Office 转换 | C# sidecar（.NET 10，通过 COM 调用 Office/WPS） |

## 快速开始

环境要求：Rust stable、Visual Studio 2019+（含 C++ 桌面开发）、Node.js 20+、pnpm 10+。

```bash
cd app
pnpm install
pnpm approve-builds --all    # pnpm 10+ 需显式放行 esbuild 的构建脚本
pnpm tauri dev
```

首次编译约 5–10 分钟。更多开发细节见 [app/README.md](app/README.md)。

## 目录结构

```
.
├── PRD-青丘湖硅基工坊.md     产品需求文档（功能、排期、风险、技术方案）
├── CHANGELOG.md              版本更新记录
└── app/                      应用代码
    ├── src/                  前端
    └── src-tauri/
        ├── features.toml     功能状态单一数据源
        ├── scripts/          签名证书生成与构建期签名脚本
        └── src/              Rust 后端
```

## 打包与签名

安装包用自签名证书签（内部工具，没有公网代码签名证书）。

**首次准备**：生成代码签名证书，把指纹设为环境变量。

```powershell
powershell -ExecutionPolicy Bypass -File app\src-tauri\scripts\gen-signing-cert.ps1
setx QINGQIUHU_CERT_THUMBPRINT "<上一步打印的指纹>"
```

证书写入当前用户个人存储，有效期 5 年，**私钥不出证书存储**。换机器需重新生成或导入同一份 `.pfx`。

**构建**：

```bash
cd app
set TAURI_SIGNING_PRIVATE_KEY=C:\Users\<你>\.tauri\qingqiuhu.key   # 更新包签名私钥（minisign）
pnpm tauri build
```

产物在 `app/src-tauri/target/release/bundle/`：NSIS `.exe` 与 MSI `.msi`，以及用于自动更新的 `.sig`。

- **未设置 `QINGQIUHU_CERT_THUMBPRINT` 时不会签名**，构建会明确打印「跳过签名」，产物仅供本机测试
- 公共时间戳服务器在内网通常不可达，签名脚本会自动退化为「不带时间戳」并提示。不带时间戳的签名在证书有效期内有效，**证书到期后已分发的安装包签名会随之失效**，续签要提前规划
- 自签证书未推进受信任根的机器，安装时仍会提示「未知发布者」。把 `-ExportCer` 导出的 `.cer` 交给 IT 走 GPO 即可消除

## 自动更新

- 更新包用 **minisign** 签名，公钥写死在 `tauri.conf.json` 的 `plugins.updater.pubkey`
- 更新源默认 `https://updates.qingqiuhu.com/forge/{stable|beta}/latest.json`，**内网实际地址待定**，可在设置页改成内网静态文件服务地址
- 更新源的目录结构（静态文件服务即可）：

```
stable/latest.json      当前版本元信息
stable/<安装包与 .sig>   与 latest.json 里的 url 对应
```

- 启动时静默检查（可在设置页关掉），发现新版本只提示，不自动安装
- 私钥 `~/.tauri/qingqiuhu.key` **不要提交、不要共享**；丢了就无法发布更新（已装版本会因签名不匹配拒绝更新）


## 版本路线

| 里程碑 | 内容 | 状态 |
|---|---|---|
| M1 | 骨架、托盘常驻、Excel 拆表、图片转换、设置页 | 已交付 |
| M2 | Markdown 编辑导出、Excel 批量拆分 | 已交付 |
| M3 | 文档转 PDF、引擎探测、转换自检、样本库 | 暂缓（开发机引擎前置未就绪） |
| M4 | 自签名安装包、自动更新、配置迁移 | 已交付 |
| M5–M7 | AI 助手、Office 高级技能、PS 高级技能 | 条件性，待内网模型就绪 |

## 已知限制

- 文档转换（Office → PDF）尚未提供，需本机已安装 MS Office 或 WPS
- `.xls` 老格式需先另存为 `.xlsx`
- WebP 输出当前为无损编码，质量参数不生效
- 加密工作簿暂不支持直接处理
- 仅支持 Windows 10 1809+ / Windows 11 x64

## 更新记录

见 [CHANGELOG.md](CHANGELOG.md)。

---

青丘湖 · [www.qingqiuhu.com](https://www.qingqiuhu.com)
