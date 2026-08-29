fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&[
                "run_command",
                "start_recording",
                "stop_recording",
                "speak",
                "list_activities",
                "list_models",
                "get_model",
                "set_model",
                "list_skills",
                "get_greeting_name",
                "get_credential_status",
                "set_credential",
            ])),
    )
    .expect("failed to run tauri-build");
}
