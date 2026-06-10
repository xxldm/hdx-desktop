# Desktop 端

本目录是 HDX Desktop 客户端工程，采用 Tauri + Rust + Vite + TypeScript。

Desktop 第一阶段设计已由根仓库 `docs/adr/0008-desktop-tauri-windows-linux-flavors.md` 记录：

- 技术栈采用 Tauri + Rust，第一阶段 Windows + Linux 并列。
- `apps/desktop` 只维护一套代码，不拆成 Local/Online 两套项目。
- Local/Online 通过构建 flavor、Tauri 配置变体和安装包内容区分。
- `HDX Desktop Local` 包含 `backend-all-in-one` sidecar/native exe，仅离线本地模式。
- `HDX Desktop Online` 不包含 all-in-one，仅在线远程模式。
- 自启动、通知、deep link、托盘、配置目录和导入导出应抽象为 Windows/Linux 通用 desktop capability。
- 类似壁纸软件的桌面窗口嵌入是 Windows-only wallpaper mode，必须单独做 Win32 spike。

## 命令

```powershell
pnpm install
pnpm run dev:local
pnpm run dev:online
pnpm run typecheck
```

当前骨架只提供只读状态面板和 capability 空壳。`dev:local` 与 `dev:online` 使用同一套代码，通过 Tauri 配置变体和 Rust feature 区分。

Windows NSIS 安装包已配置简体中文和英文，并显示安装器语言选择器。Windows 安装包默认当前用户安装，并通过 WebView2 bootstrapper 检查和引导安装 WebView2 Runtime。

第一版正式 Release 需要同时提供 Online 和 Full。Windows 发布 NSIS 安装包和绿色 zip 包，Release asset 文件名统一使用无空格命名；Linux 第一版优先发布 AppImage。首版允许未签名，但需要在 release notes 中提示 Windows SmartScreen 或系统安全提示风险。

Desktop 当前没有 Web 端那种部署配置模板。客户端运行配置建议由应用首启/设置页写入用户级 app config，并由 Rust 侧做 schema 校验；绿色包也使用同一用户级配置位置，不在 zip 根目录维护另一套配置。

## 当前边界

- Local flavor 已在 Rust 状态中标记包含 all-in-one，但本轮尚未打包或启动真实 `backend-all-in-one`。
- Online flavor 已在 Rust 状态中标记需要远端地址，但本轮尚未实现远端地址填写和持久化。
- WebView 只调用 `desktop_status` 和 `capability_status` 两个只读 Tauri command。
- 本机 token 当前不存在，也不得进入 WebView 浏览器代码。
- Windows-only wallpaper mode 只保留 capability 位置，尚未调用 Win32 API。

本目录会作为独立 Git 仓库管理。后续增加 sidecar、安装器、签名、自动更新或 Win32 API 前，必须同步根仓库计划、ADR 和 `docs/ARCHITECTURE.md`。
