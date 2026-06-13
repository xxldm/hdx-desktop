use serde::Serialize;

use crate::capabilities::{self, CapabilityStatus};
use crate::flavor::{self, DesktopFlavor};
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
    pub capabilities: Vec<CapabilityStatus>,
    pub boundary_notes: Vec<&'static str>,
}

impl DesktopStatus {
    pub fn current(backend_sidecar: BackendSidecarStatus) -> Self {
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
            capabilities: capabilities::collect(flavor),
            boundary_notes: vec![
                "Full/Online 是构建 flavor，不是两套代码。",
                "Desktop 静态 UI 只通过白名单 Tauri command 访问 Rust BFF。",
                "本机 token 只允许在 Rust 主进程和 Rust BFF 边界内流转。",
                "Windows-only wallpaper mode 不承诺 macOS/Linux 等价能力。",
            ],
        }
    }
}

#[tauri::command]
pub fn desktop_status(backend_sidecar: tauri::State<'_, BackendSidecar>) -> DesktopStatus {
    DesktopStatus::current(backend_sidecar.snapshot())
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
        let status = DesktopStatus::current(BackendSidecarStatus::not_applicable());

        assert!(!status.local_token_exposed_to_webview);
        assert!(!status.backend_sidecar.local_token_exposed_to_webview);
    }
}
