# Desktop 端

本目录是 HDX Desktop 客户端工程，采用 Tauri + Rust + Vite + TypeScript。

Desktop 第一阶段设计已由根仓库 `docs/adr/0008-desktop-tauri-windows-linux-flavors.md` 记录：

- 技术栈采用 Tauri + Rust，第一阶段 Windows + Linux 并列。
- `apps/desktop` 只维护一套代码，不拆成 Full/Online 两套项目。
- Full/Online 通过构建 flavor、Tauri 配置变体和安装包内容区分。
- `HDX Desktop Full` 包含本机后端 sidecar/native exe，仅离线本地模式。
- `HDX Desktop Online` 不包含 all-in-one，仅在线远程模式。
- 自启动、通知、deep link、托盘、配置目录和导入导出应抽象为 Windows/Linux 通用 desktop capability。
- 用户数据持久化与跨端同步边界见根仓库 `docs/adr/0016-user-data-persistence-and-sync-boundary.md`。
- 类似壁纸软件的桌面窗口嵌入是 Windows-only wallpaper mode，必须单独做 Win32 spike。

## 命令

```powershell
pnpm install
pnpm run dev:full
pnpm run dev:online
pnpm run typecheck
```

当前骨架提供只读状态面板、capability 状态、Full 本机后端 sidecar 最小启动闭环，以及 Desktop Rust BFF command。`dev:full` 与 `dev:online` 使用同一套代码，通过 Tauri 配置变体和 Rust feature 区分。

Windows NSIS 安装包已配置简体中文和英文，并显示安装器语言选择器。Windows 安装包默认当前用户安装，并通过 WebView2 bootstrapper 检查和引导安装 WebView2 Runtime。

Tauri `productName` 使用 `.` 连接并保留大小写，避免安装包默认文件名前缀包含空格。Windows 裸 EXE 的 `mainBinaryName` 允许使用空格，例如 `HDX Desktop Online.exe`；最终 Release asset 仍由发布脚本重命名为无空格文件名。

第一版正式 Release 需要同时提供 Online 和 Full。Windows 发布 NSIS 安装包和绿色 zip 包，Release asset 文件名统一使用无空格命名；Linux 第一版优先发布 AppImage。首版允许未签名，但需要在 release notes 中提示 Windows SmartScreen 或系统安全提示风险。

Desktop 当前没有 Web 端那种部署配置模板。客户端运行配置建议由应用首启/设置页写入用户级 app config，并由 Rust 侧做 schema 校验；绿色包也使用同一用户级配置位置，不在 zip 根目录维护另一套配置。

用户级 app config 只保存纯客户端配置，例如开机自启、远端地址、窗口偏好、托盘偏好和本机 capability 开关。Full flavor 的业务数据、工作台布局、组件配置和模块数据进入本机数据库；Online flavor 的登录用户数据以远端后端为事实源。

## 当前边界

- Full flavor 会从 Tauri resource 的 `backend/` 目录复制已解压 `backend-full` 到用户数据目录，启动本机 `backend-all-in-one`，轮询 `/actuator/health`，再读取 `/local/session`。
- Full flavor 的本机用户数据由 sidecar 后端和本机数据库管理，不写入 Tauri app config。
- `backend-build.json` 仍记录原始 `backend-full` Release archive 的文件名、sha256、后端 commit 和 entrypoint；运行时不解析 zip/tar archive。
- Desktop 发布包使用 `apps/web` 的 `desktop-static` 静态输出作为 Tauri frontend，不内置 Node/Nitro 运行时。
- Desktop 静态 UI 通过白名单 Tauri command 调用 Rust BFF。Full flavor 的 Rust BFF 使用 sidecar `/local/session` token 访问本机后端，但 token 不返回 WebView。
- Online flavor 已支持远端地址填写、持久化、连接检查和 Rust BFF 认证转发；access/refresh token 只保存在 Rust 主进程内存中，不返回 WebView。
- 计时器运行状态属于设备级状态，不跨设备同步；计时器预设和组件配置后续按用户数据事实源处理。
- 本地 `dev:full` / `dev:online` 仍使用本目录 Vite 状态面板，主要用于检查 Tauri flavor、capability 和 sidecar 状态。
- Windows-only wallpaper mode 只保留 capability 位置，尚未调用 Win32 API。

本目录会作为独立 Git 仓库管理。后续增加 sidecar、安装器、签名、自动更新或 Win32 API 前，必须同步根仓库计划、ADR 和 `docs/ARCHITECTURE.md`。
