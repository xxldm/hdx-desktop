use serde::Serialize;

use crate::flavor::DesktopFlavor;
use crate::platform;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityScope {
    CrossPlatform,
    WindowsOnly,
    FlavorFull,
    FlavorOnline,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityState {
    Ready,
    Stub,
    Planned,
    NotSupported,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WebviewExposure {
    CommandOnly,
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub scope: CapabilityScope,
    pub platform: &'static str,
    pub state: CapabilityState,
    pub webview_exposure: WebviewExposure,
    pub description: &'static str,
}

pub fn collect(flavor: DesktopFlavor) -> Vec<CapabilityStatus> {
    let mut capabilities = vec![
        cross_platform("autostart", "自启动", "预留跨平台自启动 capability。"),
        cross_platform("notifications", "通知", "预留跨平台通知 capability。"),
        cross_platform(
            "deep-link",
            "URL 注册",
            "预留 deep link 与 URL scheme capability。",
        ),
        cross_platform("tray", "托盘", "预留系统托盘 capability。"),
        cross_platform("config-dir", "配置目录", "预留平台配置目录 capability。"),
        cross_platform("import-export", "导入导出", "预留手动迁移数据 capability。"),
    ];

    if !matches!(flavor, DesktopFlavor::Online) {
        capabilities.push(CapabilityStatus {
            id: "local-backend-sidecar",
            name: "Full 本机后端",
            scope: CapabilityScope::FlavorFull,
            platform: "full-flavor",
            state: CapabilityState::Ready,
            webview_exposure: WebviewExposure::None,
            description: "Full flavor 会启动本机后端 sidecar，token 不暴露给 WebView。",
        });
        capabilities.push(CapabilityStatus {
            id: "desktop-rust-bff",
            name: "Desktop Rust BFF",
            scope: CapabilityScope::FlavorFull,
            platform: "full-flavor",
            state: CapabilityState::Ready,
            webview_exposure: WebviewExposure::CommandOnly,
            description: "Desktop 静态 UI 通过 Rust BFF 访问本机后端，不接触本机 token。",
        });
    }

    if !matches!(flavor, DesktopFlavor::Full) {
        capabilities.push(CapabilityStatus {
            id: "remote-endpoint",
            name: "远端地址",
            scope: CapabilityScope::FlavorOnline,
            platform: "online-flavor",
            state: CapabilityState::Planned,
            webview_exposure: WebviewExposure::CommandOnly,
            description: "仅 Online flavor 后续读取用户填写的远端服务地址。",
        });
    }

    capabilities.push(wallpaper_mode());
    capabilities
}

fn cross_platform(
    id: &'static str,
    name: &'static str,
    description: &'static str,
) -> CapabilityStatus {
    CapabilityStatus {
        id,
        name,
        scope: CapabilityScope::CrossPlatform,
        platform: "cross-platform",
        state: CapabilityState::Stub,
        webview_exposure: WebviewExposure::CommandOnly,
        description,
    }
}

fn wallpaper_mode() -> CapabilityStatus {
    let state = if platform::wallpaper_mode_state() == "not-supported" {
        CapabilityState::NotSupported
    } else {
        CapabilityState::Planned
    };

    CapabilityStatus {
        id: "wallpaper-mode",
        name: "桌面嵌入窗口",
        scope: CapabilityScope::WindowsOnly,
        platform: "windows",
        state,
        webview_exposure: WebviewExposure::None,
        description: "Windows-only wallpaper mode，后续通过 Win32 spike 验证。",
    }
}
