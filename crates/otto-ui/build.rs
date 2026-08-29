fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&[
                "run_command",
                "start_recording",
                "stop_recording",
                "speak",
            ])),
    )
    .expect("failed to run tauri-build");
}
