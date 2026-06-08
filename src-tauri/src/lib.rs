mod capabilities;
mod commands;
mod flavor;
mod platform;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::desktop_status,
            commands::capability_status
        ])
        .run(tauri::generate_context!())
        .expect("运行 HDX Desktop 失败。");
}
