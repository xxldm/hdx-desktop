use serde::Serialize;

use crate::capabilities::{self, CapabilityStatus};
use crate::flavor::{self, DesktopFlavor};
use crate::local_web::{LocalWebServer, LocalWebServerStatus};
use crate::platform;
use crate::sidecar::{BackendSidecar, BackendSidecarStatus};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatus {
    pub flavor: DesktopFlavor,
    pub flavor_label: &'static str,
    pub product_name: &'static str,
    pub platform: &'static str,
    pub includes_full_backend: bool,
    pub remote_endpoint_required: bool,
    pub local_actor: Option<&'static str>,
    pub local_token_exposed_to_webview: bool,
    pub backend_sidecar: BackendSidecarStatus,
    pub local_web_server: LocalWebServerStatus,
    pub capabilities: Vec<CapabilityStatus>,
    pub boundary_notes: Vec<&'static str>,
}

impl DesktopStatus {
    pub fn current(
        backend_sidecar: BackendSidecarStatus,
        local_web_server: LocalWebServerStatus,
    ) -> Self {
        let flavor = flavor::active_flavor();

        Self {
            flavor,
            flavor_label: flavor.label(),
            product_name: flavor.product_name(),
            platform: platform::current_platform(),
            includes_full_backend: flavor.includes_full_backend(),
            remote_endpoint_required: flavor.remote_endpoint_required(),
            local_actor: flavor.local_actor(),
            local_token_exposed_to_webview: false,
            backend_sidecar,
            local_web_server,
            capabilities: capabilities::collect(flavor),
            boundary_notes: vec![
                "Full/Online 是构建 flavor，不是两套代码。",
                "本机 token 只允许在 Rust 主进程和受控 Nuxt server 边界内流转。",
                "WebView 只通过白名单 Tauri command 读取只读状态。",
                "Windows-only wallpaper mode 不承诺 macOS/Linux 等价能力。",
            ],
        }
    }
}

#[tauri::command]
pub fn desktop_status(
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    local_web_server: tauri::State<'_, LocalWebServer>,
) -> DesktopStatus {
    DesktopStatus::current(backend_sidecar.snapshot(), local_web_server.snapshot())
}

#[tauri::command]
pub fn capability_status() -> Vec<CapabilityStatus> {
    capabilities::collect(flavor::active_flavor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_status_never_exposes_local_token_to_webview() {
        let status = DesktopStatus::current(
            BackendSidecarStatus::not_applicable(),
            LocalWebServerStatus::not_applicable(),
        );

        assert!(!status.local_token_exposed_to_webview);
        assert!(!status.backend_sidecar.local_token_exposed_to_webview);
        assert!(!status.local_web_server.local_token_exposed_to_webview);
    }
}
