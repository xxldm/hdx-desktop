mod bff;
mod capabilities;
mod commands;
mod flavor;
mod local_http;
mod online_config;
mod platform;
mod sidecar;

use tauri::Manager;

pub fn run() {
    let backend_sidecar = sidecar::BackendSidecar::default();
    let shutdown_sidecar = backend_sidecar.clone();

    tauri::Builder::default()
        .setup(move |app| {
            app.manage(backend_sidecar.clone());
            if flavor::active_flavor().includes_full_backend() {
                let resource_dir = app.path().resource_dir().map_err(|error| {
                    std::io::Error::other(format!("无法定位 Tauri resource 目录：{error}"))
                })?;
                let resource_backend_dir = resource_dir.join("backend");
                let app_data_dir = app.path().app_data_dir().map_err(|error| {
                    std::io::Error::other(format!("无法定位应用数据目录：{error}"))
                })?;
                backend_sidecar.start_in_background(resource_backend_dir, app_data_dir);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::desktop_status,
            commands::capability_status,
            bff::hdx_auth_session,
            bff::hdx_auth_login,
            bff::hdx_auth_logout,
            online_config::hdx_online_config_get,
            online_config::hdx_online_config_save,
            online_config::hdx_online_connection_check,
            bff::hdx_runtime_info,
            bff::hdx_tools_list,
            bff::hdx_tools_create
        ])
        .build(tauri::generate_context!())
        .expect("运行 HDX Desktop 失败。")
        .run(move |_app_handle, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                shutdown_sidecar.stop();
            }
        });
}
