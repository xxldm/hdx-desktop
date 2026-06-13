fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "desktop_status",
            "capability_status",
            "hdx_auth_session",
            "hdx_auth_login",
            "hdx_auth_logout",
            "hdx_online_config_get",
            "hdx_online_config_save",
            "hdx_online_connection_check",
            "hdx_runtime_info",
            "hdx_tools_list",
            "hdx_tools_create",
        ]),
    ))
    .expect("生成 Tauri 构建上下文失败。");
}
