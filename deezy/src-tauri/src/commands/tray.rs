use super::*;

#[tauri::command]
pub async fn update_tray_status(
    downloads_active: bool,
    downloads_paused: bool,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Update tray state
    *state.tray_state.downloads_active.lock().await = downloads_active;
    *state.tray_state.downloads_paused.lock().await = downloads_paused;

    // Update tray menu
    app_tray::update_tray_menu(&app, downloads_active, downloads_paused)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_tray_tooltip(
    tooltip: String,
    app: AppHandle,
) -> Result<(), String> {
    app_tray::set_tray_tooltip(&app, &tooltip)
        .map_err(|e| e.to_string())
}
