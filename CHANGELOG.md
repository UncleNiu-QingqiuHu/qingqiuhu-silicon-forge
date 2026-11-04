# 更新日志

本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)，记录格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## [Unreleased]

## [0.3.0] - 2026-11-04

M4 交付：分发与更新。安装包可签名、可自动更新，配置格式从此带版本号。

### 新增

- **配置持久化**：`%APPDATA%\QingqiuhuSiliconForge\config.json`，带 `schemaVersion`
- **schema 迁移机制**：低版本链式迁移，迁移前备份 `config.v{n}.bak`；版本高于程序时拒绝启动；文件损坏时隔离为 `config.corrupt.<时间戳>.json` 并用默认配置继续
- **自动更新**：启动静默检查 + 设置页手动检查，下载进度实时显示
- **灰度通道**：`stable` / `beta` 双通道，可在设置页切换
- **更新源可配置**：留空则用内置地址；填错在保存时即报错，不会拖到检查更新才失败
- **minisign 更新签名**：公钥内置于安装包，更新包被篡改会在校验阶段失败
- **自签名代码签名**：证书生成脚本与构建期签名脚本（`app/src-tauri/scripts/`），签名按环境变量开关
- **关于页补全**：构建时间与依赖清单，均在建期从真实来源生成，不手写

### 修复

- **文件选择对话框从未注册**：`tauri-plugin-dialog` 没在 Rust 侧初始化、capability 也缺 `dialog:default`，导致 Excel 拆分与 Markdown 打开/保存点了没反应
- 关闭按钮行为此前只存内存，重启即丢；现已持久化

### 变更

- 托盘「关闭行为」设置改为写入配置文件，不再依赖进程内状态

### 已知限制

- 静默安装（`setup.exe /S`、`msiexec /qn`）需管理员权限，本机未实测，仅验证了安装包生成与签名
- 更新失败依赖安装程序自身回退，未做显式的版本回退向导（F8-6，待补）
- 默认更新源 `updates.qingqiuhu.com` 为占位地址，内网实际部署位置待定（PRD Q-03）
- 安装包签名使用自签证书，未推送根证书的机器仍会提示未知发布者
- 签名不带时间戳（内网到不了公共时间戳服务器），证书到期后已分发安装包的签名会一并失效

## [0.2.1] - 2026-11-04

统一内部标识的拼音写法：品牌名「青丘湖」正确拼音为 `qingqiuhu`，此前代码与文档普遍漏写 `hu`，与已改名的仓库地址不一致。

### 变更

- 应用标识 `productName`：`QingqiuSiliconForge` → `QingqiuhuSiliconForge`（影响安装目录、开始菜单项、卸载登记名）
- 包名：`qingqiu-silicon-forge` → `qingqiuhu-silicon-forge`（`package.json`、`Cargo.toml`）
- Rust 库名：`qingqiu_forge_lib` → `qingqiuhu_forge_lib`（`main.rs` 引用同步更新）
- 可执行文件名：`qingqiu-silicon-forge.exe` → `qingqiuhu-silicon-forge.exe`
- 开机自启注册表键名：`QingqiuSiliconForge` → `QingqiuhuSiliconForge`
- 文档与 PRD 中的安装/数据/临时目录路径同步为 `QingqiuhuSiliconForge`
- `Cargo.toml` 作者字段：`Qingqiu` → `Qingqiuhu`

### 注意

- 已安装旧版本的用户需重新勾选「开机自启」——注册表键名已变，旧键不会自动迁移
- 应用标识 `identifier`（`com.qingqiuhu.siliconforge`）本就正确，未作改动

## [0.2.0] - 2026-11-04

### 新增

- Markdown 模块：GFM 编辑、实时预览、语法高亮
- 导出单文件 HTML（样式内嵌、图片 base64、顶部品牌栏）
- 导出矢量 PDF，每页顶部品牌页眉「青丘湖硅基工坊 www.qingqiuhu.com」，页码底部居中
- Excel 批量拆分：一次选择多个工作簿，逐个输出到各自子目录
- 受保护工作簿给出明确提示，不再只报读取失败

### 变更

- 技术栈整体升级至最新：React 19、Vite 8、TypeScript 7、umya-spreadsheet 3、typst 0.15、comrak 0.59
- umya-spreadsheet 全量迁移至新版接口（`Workbook`、`cells()`、`sheet_by_name()` 等），无废弃 API 残留
- 界面版本号改为从应用元信息读取，不再硬编码

### 技术

- 自动化测试从 20 项增至 36 项，新增 Markdown → Typst → PDF 的端到端验证
- MSRV 提升至 Rust 1.90，以使用各依赖的最新稳定版本
- Vite 8 改用 rolldown 打包，构建耗时从约 800ms 降至约 1s 内

### 已知问题

- 图表数据源跨表引用仍无法检测（底层库不暴露 Series 引用），界面仅提示「含图表」
- WebP 输出为无损编码，质量参数不生效
- Markdown 中的脚注正文暂不渲染

## [0.1.0] - 2026-11-04

首个可运行版本。确定产品形态与工程骨架。

### 新增

- **系统托盘常驻**：关闭按钮最小化到托盘、双击唤出窗口、右键菜单（打开/进度/设置/退出）
- **退出确认**：无任务时提示「退出后不再驻留托盘」；有任务时把「最小化并继续」设为首选按钮，退出会中断任务
- **Excel 工作表拆分**：采用「复制原文件 → 删除其他表 → 另存」策略，保留单元格样式、合并单元格、行列宽高、冻结窗格、打印区域、页面设置、页眉页脚、条件格式、图片与图表
- **跨表引用检测**：识别 `Sheet2!A1`、`'My Sheet'!A1`、`Sheet1:Sheet3!A1` 与中文表名，正确跳过字符串字面量中的叹号
- **三种处理策略**：保留公式（可能出现 `#REF!`）／转为当前值／中止拆分，交由用户选择而非静默处理
- **图片格式转换**：支持 PNG/JPG/WebP/BMP/GIF/TIFF/ICO/SVG 输入，输出 PNG/JPG/WebP/BMP/ICO
- **图片处理**：长边缩放、百分比缩放、透明通道铺底（避免 PNG 转 JPG 出现黑底）、EXIF 方向校正、SVG 按目标尺寸渲染、ICO 自动限制 256px
- **设置页**：关闭按钮行为、开机自启、功能交付状态、版本信息
- **功能状态注册表**：`features.toml` 作为单一数据源，编译期注入静态表，界面按状态渲染「待排期／开发中／可用」

### 技术

- Tauri 2 + Rust + React 18 + TypeScript + Vite
- 20 项自动化测试，含真实 xlsx 文件的端到端拆分验证（扫描 → 拆分 → 回读校验）

### 已知问题

- 文档转 PDF、引擎探测、转换自检尚未提供（规划 M3）
- `.xls` 老格式不支持，需先另存为 `.xlsx`
- 加密工作簿读取失败时提示不明确
- 一次只能处理一个工作簿
