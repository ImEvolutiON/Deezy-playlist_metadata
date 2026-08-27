use super::*;

#[tauri::command]
pub async fn search_tracks(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.search_tracks(&query, 20).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_track_by_id(
    trackId: String,
    state: tauri::State<'_, AppState>,
) -> Result<SearchResult, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.get_track_by_id(&trackId).await
}

#[tauri::command]
pub async fn search_albums(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AlbumResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.search_albums(&query, 20).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_album_tracks(
    albumId: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.get_album_tracks(&albumId).await
}

#[tauri::command]
pub async fn search_artists(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ArtistResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.search_artists(&query, 20).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_artist_albums(
    artistId: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AlbumResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.get_artist_albums(&artistId).await
}

#[tauri::command]
pub async fn search_playlists(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<PlaylistResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.search_playlists(&query, 20).await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_playlist_title(
    playlistId: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
            .as_ref()
            .cloned()
            .ok_or("Not logged in. Set your ARL token in Settings.")?
    };

    client.get_playlist_title(&playlistId).await
}
#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_playlist_tracks(
    playlistId: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let client = {
        let lock = state.client.lock().await;
        lock
        .as_ref()
        .cloned()
        .ok_or("Not logged in. Set your ARL token in Settings.")?
    };
    client.get_playlist_tracks(&playlistId).await
}

