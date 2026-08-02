use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

const KEYRING_SERVICE: &str = "com.pierr.deezy";
const KEYRING_USER: &str = "arl_token";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FolderStructure {
    #[default]
    Flat,
    ArtistTrack,
    ArtistAlbumTrack,
    AlbumTrack,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub arl: String,
    pub output_dir: String,
    pub quality: String,
    #[serde(default)]
    pub folder_structure: FolderStructure,
    #[serde(default = "default_custom_folder_template")]
    pub custom_folder_template: String,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub custom_theme: Option<String>,
    #[serde(default)]
    pub search_history: Vec<String>,
    #[serde(default = "default_true")]
    pub enable_search_history: bool,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
}

fn default_true() -> bool {
    true
}

fn default_locale() -> String {
    "en".to_string()
}

fn default_custom_folder_template() -> String {
    "{artist}/{release_date} - {album}/{track_number} - {title}".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());

        let default_dir = PathBuf::from(home)
            .join("Music")
            .join("Deezy");

        Self {
            arl: String::new(),
            output_dir: default_dir.to_string_lossy().to_string(),
            quality: "MP3_320".into(),
            folder_structure: FolderStructure::default(),
            custom_folder_template: default_custom_folder_template(),
            theme: Some("system".to_string()),
            custom_theme: None,
            search_history: Vec::new(),
            enable_search_history: true,
            notifications_enabled: true,
            locale: "en".to_string(),
            close_to_tray: true,
        }
    }
}

fn keyring_enabled() -> bool {
    !matches!(
        std::env::var("DEEZY_NO_KEYRING").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

fn save_arl_to_keyring(arl: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| format!("Keyring error: {}", e))?;
    entry
        .set_password(arl)
        .map_err(|e| format!("Failed to save ARL to credential store: {}", e))
}

fn load_arl_from_keyring() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
    entry.get_password().ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArlStorage {
    Keyring,
    PlainFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArlStorageStatus {
    pub storage: ArlStorage,
    pub reason: Option<String>,
}

impl ArlStorageStatus {
    fn secure() -> Self {
        Self { storage: ArlStorage::Keyring, reason: None }
    }

    fn insecure(reason: impl Into<String>) -> Self {
        Self { storage: ArlStorage::PlainFile, reason: Some(reason.into()) }
    }
}

fn keyring_probe() -> Result<(), String> {
    if !keyring_enabled() {
        return Err("Disabled by DEEZY_NO_KEYRING".to_string());
    }

    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;

    // NoEntry means the credential store answered but holds nothing yet.
    match entry.get_password() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn disk_arl(app: &tauri::AppHandle) -> Option<String> {
    let path = Settings::path(app).ok()?;
    let data = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    value.get("arl")?.as_str().map(|s| s.to_string())
}

pub fn arl_storage_status(app: &tauri::AppHandle) -> ArlStorageStatus {
    let probe = keyring_probe();

    if disk_arl(app).is_some_and(|arl| !arl.trim().is_empty()) {
        return ArlStorageStatus { storage: ArlStorage::PlainFile, reason: probe.err() };
    }

    match probe {
        Ok(()) => ArlStorageStatus::secure(),
        Err(e) => ArlStorageStatus::insecure(e),
    }
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn restrict_to_owner(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn write_private(path: &Path, data: &[u8]) -> Result<(), String> {
    // mode() below only applies on create, so existing files need this too.
    if path.exists() {
        restrict_to_owner(path)?;
    }

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(path).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, data).map_err(|e| e.to_string())
}

impl Settings {
    fn path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir.join("settings.json"))
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validate ARL
        if self.arl.trim().is_empty() {
            return Err("ARL token is required".to_string());
        }

        if self.arl.trim().len() < 100 {
            return Err("ARL token appears to be invalid (too short)".to_string());
        }

        // Validate output directory
        if self.output_dir.trim().is_empty() {
            return Err("Output directory is required".to_string());
        }

        // Try to create the directory if it doesn't exist
        let output_path = PathBuf::from(&self.output_dir);
        if !output_path.exists() {
            std::fs::create_dir_all(&output_path)
                .map_err(|e| format!("Cannot create output directory: {}", e))?;
        }

        // Check if directory is writable
        if !output_path.is_dir() {
            return Err("Output path is not a directory".to_string());
        }

        // Validate quality
        let valid_qualities = ["MP3_128", "MP3_320", "FLAC"];
        if !valid_qualities.contains(&self.quality.as_str()) {
            return Err(format!("Invalid quality '{}'. Must be one of: MP3_128, MP3_320, FLAC", self.quality));
        }

        if self.folder_structure == FolderStructure::Custom && self.custom_folder_template.trim().is_empty() {
            return Err("Custom folder template is required".to_string());
        }

        Ok(())
    }

    pub fn load(app: &tauri::AppHandle) -> Result<Self, String> {
        let path = Self::path(app)?;
        let mut settings: Self = if path.exists() {
            if let Err(e) = restrict_to_owner(&path) {
                eprintln!("Warning: could not restrict permissions on {:?}: {}", path, e);
            }

            let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&data).map_err(|e| e.to_string())?
        } else {
            Self::default()
        };

        if keyring_probe().is_err() {
            return Ok(settings);
        }

        // Migrate: if ARL was stored in the JSON file, move it to keyring
        if !settings.arl.is_empty() {
            if save_arl_to_keyring(&settings.arl).is_err() {
                return Ok(settings);
            }

            // Re-save settings without the ARL in the file
            let mut clean = settings.clone();
            clean.arl = String::new();
            let data = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
            write_private(&path, data.as_bytes())?;
        }

        // Load ARL from OS credential store
        if let Some(arl) = load_arl_from_keyring() {
            if !arl.is_empty() {
                settings.arl = arl;
            }
        }

        Ok(settings)
    }

    pub fn save(&self, app: &tauri::AppHandle) -> Result<(), String> {
        // Validate before saving
        self.validate()?;

        let mut settings_for_disk = self.clone();
        if keyring_enabled() {
            match save_arl_to_keyring(&self.arl) {
                Ok(()) => settings_for_disk.arl = String::new(),
                Err(e) => eprintln!(
                    "Warning: secure credential storage unavailable ({}). Storing ARL in settings.json",
                    e
                ),
            }
        }

        let path = Self::path(app)?;
        let data = serde_json::to_string_pretty(&settings_for_disk).map_err(|e| e.to_string())?;
        write_private(&path, data.as_bytes())?;

        Ok(())
    }
}
