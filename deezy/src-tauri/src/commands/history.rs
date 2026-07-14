use super::*;

#[tauri::command]
pub async fn save_download_history(history: Vec<serde_json::Value>, app: AppHandle) -> Result<(), String> {
    run_blocking(move || {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("download_history.json");
        let data = serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?;
        std::fs::write(&path, data).map_err(|e| e.to_string())
    }).await
}

#[tauri::command]
pub async fn load_download_history(app: AppHandle) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let path = dir.join("download_history.json");
        if path.exists() {
            let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&data).map_err(|e| e.to_string())
        } else {
            Ok(vec![])
        }
    }).await
}

#[tauri::command]
pub async fn export_download_history(
    history: Vec<serde_json::Value>,
    format: String,
    app: AppHandle,
) -> Result<String, String> {
    let extension = match format.as_str() {
        "csv" => "csv",
        "json" => "json",
        _ => return Err("Invalid format. Use 'csv' or 'json'.".to_string()),
    };

    let (tx, rx) = tokio::sync::oneshot::channel();

    app.dialog()
        .file()
        .set_title("Export Download History")
        .add_filter(format.to_uppercase(), &[extension])
        .set_file_name(format!("deezy_download_history.{}", extension))
        .save_file(move |file_path| {
            let _ = tx.send(
                file_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())),
            );
        });

    let file_path = match rx.await {
        Ok(Some(path)) => path,
        Ok(None) => return Err("Export cancelled".to_string()),
        Err(_) => return Err("Failed to get file path".to_string()),
    };

    let output_path = file_path.clone();
    run_blocking(move || {
        let content = if format == "csv" {
            generate_csv(&history)?
        } else {
            serde_json::to_string_pretty(&history).map_err(|e| e.to_string())?
        };
        std::fs::write(&output_path, content).map_err(|e| e.to_string())
    }).await?;
    Ok(file_path)
}

fn generate_csv(history: &[serde_json::Value]) -> Result<String, String> {
    let mut csv = String::from("Title,Artist,Album,Status,Progress,Timestamp,File Path,Error Message\n");

    for item in history {
        let title = sanitize_csv_field(item["title"].as_str().unwrap_or(""));
        let artist = sanitize_csv_field(item["artist"].as_str().unwrap_or(""));
        let album = sanitize_csv_field(item["album"].as_str().unwrap_or(""));
        let status = sanitize_csv_field(item["status"].as_str().unwrap_or(""));
        let percent = format!("{:.1}%", item["percent"].as_f64().unwrap_or(0.0));
        let timestamp = sanitize_csv_field(item["timestamp"].as_str().unwrap_or(""));
        let file_path = sanitize_csv_field(item["filePath"].as_str().unwrap_or(""));
        let error_msg = sanitize_csv_field(item["errorMsg"].as_str().unwrap_or(""));

        csv.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
            title, artist, album, status, percent, timestamp, file_path, error_msg
        ));
    }

    Ok(csv)
}

fn sanitize_csv_field(value: &str) -> String {
    let escaped = value.replace("\"", "\"\"");
    if let Some(first) = escaped.chars().next() {
        if matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r') {
            return format!("'{}", escaped);
        }
    }
    escaped
}

