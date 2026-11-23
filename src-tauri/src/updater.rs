use serde::Serialize;

#[derive(Serialize)]
pub struct UpdateCheckResult {
    available: bool,
}

#[tauri::command]
pub fn get_update_state() -> UpdateCheckResult {
    UpdateCheckResult { available: false }
}

#[tauri::command]
pub fn check_for_updates(force: bool) -> UpdateCheckResult {
    let _ = force;
    get_update_state()
}

#[tauri::command]
pub fn install_update() -> Result<(), String> {
    Err("Updates are unavailable in this build.".into())
}
