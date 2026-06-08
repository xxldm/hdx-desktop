# Desktop 端

本目录用于未来 desktop 客户端实现。当前仍未创建 Tauri 工程或引入运行时代码。

Desktop 第一阶段设计已由根仓库 `docs/adr/0008-desktop-tauri-windows-flavors.md` 记录：

- 技术栈采用 Tauri + Rust，首版 Windows first。
- `apps/desktop` 只维护一套代码，不拆成 Local/Online 两套项目。
- Local/Online 通过构建 flavor、Tauri 配置变体和安装包内容区分。
- `HDX Desktop Local` 包含 `backend-all-in-one` sidecar/native exe，仅离线本地模式。
- `HDX Desktop Online` 不包含 all-in-one，仅在线远程模式。
- 自启动、通知、deep link、托盘、配置目录和导入导出应抽象为可跨平台 capability。
- 类似壁纸软件的桌面窗口嵌入是 Windows-only wallpaper mode，必须单独做 Win32 spike。

本目录会作为独立 Git 仓库管理。后续创建 Tauri 工程、引入依赖、增加打包配置或实现 Win32 API 前，必须同步根仓库计划、ADR 和 `docs/ARCHITECTURE.md`。
