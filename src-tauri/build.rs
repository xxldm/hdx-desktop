fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&[
                "desktop_status",
                "capability_status",
            ])),
    )
    .expect("生成 Tauri 构建上下文失败。");
}
