use super::*;

#[tauri::command]
pub fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn login(
    arl: String,
    state: tauri::State<'_, AppState>,
    app: AppHandle,
) -> Result<Value, String> {
    let client = DeezerClient::new(&arl).await?;
    let user = serde_json::to_value(&client.user).map_err(|e| e.to_string())?;

    *state.client.lock().await = Some(client);

    let _settings_io = state.settings_io.lock().await;
    let mut updated_settings = state.settings.lock().await.clone();
    updated_settings.arl = arl;
    let settings_to_save = updated_settings.clone();
    run_blocking(move || settings_to_save.save(&app)).await?;
    *state.settings.lock().await = updated_settings;

    Ok(user)
}

#[tauri::command]
pub async fn auto_login(
    state: tauri::State<'_, AppState>,
    app: AppHandle,
) -> Result<Option<Value>, String> {
    let settings_io = state.settings_io.lock().await;
    let settings = run_blocking(move || Settings::load(&app)).await?;
    *state.settings.lock().await = settings.clone();
    drop(settings_io);
    if settings.arl.trim().is_empty() {
        return Ok(None);
    }

    let client = DeezerClient::new(&settings.arl).await?;
    let user = serde_json::to_value(&client.user).map_err(|e| e.to_string())?;

    *state.client.lock().await = Some(client);

    Ok(Some(user))
}

