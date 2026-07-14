use super::*;

#[tauri::command]
pub async fn list_custom_themes(app: AppHandle) -> Result<Vec<String>, String> {
    run_blocking(move || theme_store::list_custom_themes(&app)).await
}

#[tauri::command]
pub async fn load_custom_theme(theme_name: String, app: AppHandle) -> Result<theme_store::CustomTheme, String> {
    run_blocking(move || theme_store::load_custom_theme(&app, &theme_name)).await
}

#[tauri::command]
pub async fn save_custom_theme(theme: theme_store::CustomTheme, app: AppHandle) -> Result<(), String> {
    run_blocking(move || theme_store::save_custom_theme(&app, &theme)).await
}

#[tauri::command]
pub async fn delete_custom_theme(theme_name: String, app: AppHandle) -> Result<(), String> {
    run_blocking(move || theme_store::delete_custom_theme(&app, &theme_name)).await
}

#[tauri::command]
pub async fn export_current_theme(
    theme_name: String,
    author: Option<String>,
    description: Option<String>,
    is_light: bool,
) -> Result<theme_store::CustomTheme, String> {
    Ok(theme_store::export_current_theme(theme_name, author, description, is_light))
}

#[tauri::command]
pub async fn import_theme_file(app: AppHandle) -> Result<String, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    app.dialog()
        .file()
        .set_title("Import Theme File")
        .add_filter("JSON", &["json"])
        .pick_file(move |file_path| {
            let _ = tx.send(file_path.map(|p| p.to_string()));
        });

    let file_path = match rx.await {
        Ok(Some(path)) => path,
        Ok(None) => return Err("Import cancelled".to_string()),
        Err(_) => return Err("Failed to get file path".to_string()),
    };

    run_blocking(move || {
        let data = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        let theme: theme_store::CustomTheme = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        theme.validate()?;
        theme_store::save_custom_theme(&app, &theme)?;
        Ok(theme.name)
    }).await
}

#[tauri::command]
pub async fn create_example_themes(app: AppHandle) -> Result<(), String> {
    run_blocking(move || theme_store::create_example_themes(&app)).await
}
