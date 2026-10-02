use crate::cascade::CensorCategories;
use crate::cues::ScheduledCue;
use crate::paths::PathResolver;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserSettings {
    #[serde(default = "default_true")]
    pub censor_shield_enabled: bool,
    #[serde(default = "default_true")]
    pub shield_donate_enabled: bool,
    #[serde(default = "default_threshold")]
    pub nsfw_threshold: u32,
    #[serde(default)]
    pub selected_monitor: i32,
    #[serde(default = "default_hud_monitor")]
    pub hud_monitor: i32,
    #[serde(default)]
    pub operation_mode: u8,
    #[serde(default)]
    pub fps_boosted: bool,
    #[serde(default = "default_true")]
    pub ocr_enabled: bool,
    #[serde(default = "default_pre_warn")]
    pub pre_warning_seconds: f64,
    #[serde(default = "default_true")]
    pub auto_censor: bool,
    #[serde(default = "default_true")]
    pub cue_notifications: bool,
    #[serde(default = "default_true")]
    pub append_mode: bool,
    #[serde(default)]
    pub censor_categories: CensorCategories,
    #[serde(default)]
    pub hide_setup_guide: bool,
    #[serde(default = "default_close_action")]
    pub close_action: String,
    #[serde(default = "default_hotkey_scope")]
    pub hotkey_scope: String,
    #[serde(default = "default_hotkey_panic")]
    pub hotkey_panic: String,
    #[serde(default = "default_hotkey_threat")]
    pub hotkey_threat: String,
    #[serde(default = "default_hotkey_mode")]
    pub hotkey_mode: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_active_model_profile")]
    pub active_model_profile: String,
    #[serde(default)]
    pub model_tuning: crate::cascade::ModelProfile,
    #[serde(default = "default_saved_model_profiles")]
    pub saved_model_profiles: Vec<crate::cascade::ModelProfile>,
}

fn default_active_model_profile() -> String {
    "Gaming".to_string()
}

fn default_saved_model_profiles() -> Vec<crate::cascade::ModelProfile> {
    crate::cascade::ModelProfile::builtin_presets()
}

fn default_true() -> bool {
    true
}

fn default_threshold() -> u32 {
    65
}

fn default_hud_monitor() -> i32 {
    -1
}

fn default_pre_warn() -> f64 {
    20.0
}

fn default_close_action() -> String {
    "ask".to_string()
}

fn default_hotkey_scope() -> String {
    "global".to_string()
}

fn default_hotkey_panic() -> String {
    "F9".to_string()
}

fn default_hotkey_threat() -> String {
    "F8".to_string()
}

fn default_hotkey_mode() -> String {
    "Ctrl + Shift + F1".to_string()
}

fn default_language() -> String {
    "ru".to_string()
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            censor_shield_enabled: true,
            shield_donate_enabled: true,
            nsfw_threshold: 65,
            selected_monitor: 0,
            hud_monitor: -1,
            operation_mode: 0,
            fps_boosted: false,
            ocr_enabled: true,
            pre_warning_seconds: 20.0,
            auto_censor: true,
            cue_notifications: true,
            append_mode: true,
            censor_categories: CensorCategories::default(),
            hide_setup_guide: false,
            close_action: "ask".to_string(),
            hotkey_scope: default_hotkey_scope(),
            hotkey_panic: default_hotkey_panic(),
            hotkey_threat: default_hotkey_threat(),
            hotkey_mode: default_hotkey_mode(),
            language: default_language(),
            active_model_profile: default_active_model_profile(),
            model_tuning: crate::cascade::ModelProfile::default(),
            saved_model_profiles: default_saved_model_profiles(),
        }
    }
}

/// Helper that verifies directory write permissions (vital for NSIS Program Files vs LocalAppData)
fn is_dir_writable(dir: &std::path::Path) -> bool {
    if !dir.exists() {
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let probe_file = dir.join("_write_probe.tmp");
    match fs::write(&probe_file, b"blewred_ok") {
        Ok(_) => {
            let _ = fs::remove_file(&probe_file);
            true
        }
        Err(_) => false,
    }
}

/// Resolves the configuration directory for saving user preferences
pub fn get_config_dir() -> PathBuf {
    let project_root = PathResolver::find_project_root();
    let config_dir = project_root.join("config");
    if is_dir_writable(&config_dir) {
        return config_dir;
    }
    // Fallback to local appdata if project root is read-only (e.g. NSIS installed to Program Files)
    let appdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\".to_string());
    let fallback = PathBuf::from(appdata).join("blewred").join("config");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

/// Resolves the profiles directory for saving individual readable model profile .json files
pub fn get_profiles_dir() -> PathBuf {
    let base_cfg = get_config_dir();
    let profiles_dir = base_cfg.join("profiles");
    if is_dir_writable(&profiles_dir) {
        return profiles_dir;
    }
    let appdata = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\".to_string());
    let fallback = PathBuf::from(appdata).join("blewred").join("profiles");
    let _ = fs::create_dir_all(&fallback);
    fallback
}

pub fn get_settings_file_path() -> PathBuf {
    get_config_dir().join("user_settings.json")
}

pub fn get_user_stopwords_file_path() -> PathBuf {
    get_config_dir().join("user_stopwords.txt")
}

pub fn get_user_cues_file_path() -> PathBuf {
    get_config_dir().join("user_cues.json")
}

/// Load saved user settings from disk or return defaults
pub fn load_settings() -> UserSettings {
    let path = get_settings_file_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<UserSettings>(&content) {
                println!(
                    "[Settings] Successfully loaded user settings from {:?}",
                    path
                );
                return settings;
            } else {
                eprintln!(
                    "[Settings] Failed to parse user settings at {:?}, using defaults",
                    path
                );
            }
        }
    }
    let default_settings = UserSettings::default();
    // Save defaults so user_settings.json exists on disk
    let _ = save_settings(&default_settings);
    default_settings
}

static CACHED_SETTINGS: std::sync::Mutex<Option<UserSettings>> = std::sync::Mutex::new(None);

/// Get the currently cached user settings, or load from disk
pub fn get_cached_settings() -> UserSettings {
    if let Ok(guard) = CACHED_SETTINGS.lock() {
        if let Some(ref s) = *guard {
            return s.clone();
        }
    }
    let loaded = load_settings();
    if let Ok(mut guard) = CACHED_SETTINGS.lock() {
        *guard = Some(loaded.clone());
    }
    loaded
}

/// Atomically updates a field in the cached settings and immediately saves to disk
pub fn update_cached_settings<F>(modifier: F) -> UserSettings
where
    F: FnOnce(&mut UserSettings),
{
    let mut settings = get_cached_settings();
    modifier(&mut settings);
    let _ = save_settings(&settings);
    if let Ok(mut guard) = CACHED_SETTINGS.lock() {
        *guard = Some(settings.clone());
    }
    settings
}

/// Save user settings to disk
pub fn save_settings(settings: &UserSettings) -> Result<(), String> {
    let path = get_settings_file_path();
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write settings to {:?}: {}", path, e))?;
    if let Ok(mut guard) = CACHED_SETTINGS.lock() {
        *guard = Some(settings.clone());
    }
    println!("[Settings] Successfully saved user settings to {:?}", path);
    Ok(())
}

/// Load stopwords from disk: returns custom user stopwords if saved, otherwise default built-in list
pub fn load_stopwords() -> String {
    let user_path = get_user_stopwords_file_path();
    if user_path.exists() {
        if let Ok(content) = fs::read_to_string(&user_path) {
            if !content.trim().is_empty() {
                println!("[Settings] Loaded user stopwords from {:?}", user_path);
                return content;
            }
        }
    }
    include_str!("../../config/default_stopwords.txt").to_string()
}

/// Save customized stopwords to disk
pub fn save_stopwords(text: &str) -> Result<(), String> {
    let user_path = get_user_stopwords_file_path();
    fs::write(&user_path, text)
        .map_err(|e| format!("Failed to write stopwords to {:?}: {}", user_path, e))?;
    println!(
        "[Settings] Saved user stopwords ({} bytes) to {:?}",
        text.len(),
        user_path
    );
    Ok(())
}

/// Load saved scheduled cues from disk
pub fn load_saved_cues() -> Vec<ScheduledCue> {
    let path = get_user_cues_file_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(cues) = serde_json::from_str::<Vec<ScheduledCue>>(&content) {
                println!(
                    "[Settings] Loaded {} saved scheduled cues from {:?}",
                    cues.len(),
                    path
                );
                return cues;
            }
        }
    }
    Vec::new()
}

/// Save scheduled cues to disk
pub fn save_saved_cues(cues: &[ScheduledCue]) -> Result<(), String> {
    let path = get_user_cues_file_path();
    let json = serde_json::to_string_pretty(cues)
        .map_err(|e| format!("Failed to serialize cues: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write cues to {:?}: {}", path, e))?;
    println!(
        "[Settings] Saved {} scheduled cues to {:?}",
        cues.len(),
        path
    );
    Ok(())
}

/// Clear scheduled cues from disk
pub fn clear_saved_cues() -> Result<(), String> {
    let path = get_user_cues_file_path();
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    Ok(())
}

/// Load saved model profiles from the profiles folder, merging with built-in presets
pub fn load_saved_model_profiles() -> Vec<crate::cascade::ModelProfile> {
    let dir = get_profiles_dir();
    let mut profiles = crate::cascade::ModelProfile::builtin_presets();
    let mut names: std::collections::HashSet<String> = profiles
        .iter()
        .map(|p| p.name.to_lowercase())
        .collect();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(mut prof) = serde_json::from_str::<crate::cascade::ModelProfile>(&content) {
                        let lower_name = prof.name.trim().to_lowercase();
                        // Protect built-ins
                        if crate::cascade::ModelProfile::is_protected_preset(&lower_name) {
                            continue;
                        }
                        if !names.contains(&lower_name) {
                            prof.is_preset = false;
                            names.insert(lower_name);
                            profiles.push(prof);
                        }
                    }
                }
            }
        }
    }

    profiles
}

/// Save a user-defined model profile to both UserSettings and disk as an individual .json file
pub fn save_user_model_profile(
    profile: &crate::cascade::ModelProfile,
) -> Result<Vec<crate::cascade::ModelProfile>, String> {
    let clean_name = profile.name.trim();
    if clean_name.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }
    if crate::cascade::ModelProfile::is_protected_preset(clean_name) {
        return Err(format!("Cannot overwrite protected built-in preset '{}'", clean_name));
    }

    let dir = get_profiles_dir();
    let _ = fs::create_dir_all(&dir);
    let safe_filename = format!(
        "{}.json",
        clean_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
    );
    let target_file = dir.join(&safe_filename);

    let mut to_save = profile.clone();
    to_save.name = clean_name.to_string();
    to_save.is_preset = false;

    let json = serde_json::to_string_pretty(&to_save)
        .map_err(|e| format!("Failed to serialize profile: {}", e))?;
    fs::write(&target_file, json)
        .map_err(|e| format!("Failed to save profile file {:?}: {}", target_file, e))?;

    // Also update in UserSettings
    let mut settings = load_settings();
    settings.active_model_profile = to_save.name.clone();
    settings.model_tuning = to_save.clone();

    // Update in-memory saved_model_profiles
    if let Some(pos) = settings
        .saved_model_profiles
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(&to_save.name))
    {
        settings.saved_model_profiles[pos] = to_save;
    } else {
        settings.saved_model_profiles.push(to_save);
    }
    let _ = save_settings(&settings);

    Ok(load_saved_model_profiles())
}

/// Delete a user-defined model profile from disk and UserSettings
pub fn delete_user_model_profile(name: &str) -> Result<Vec<crate::cascade::ModelProfile>, String> {
    let clean_name = name.trim();
    if crate::cascade::ModelProfile::is_protected_preset(clean_name) {
        return Err(format!("Built-in preset '{}' cannot be deleted", clean_name));
    }

    let dir = get_profiles_dir();
    let safe_filename = format!(
        "{}.json",
        clean_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
    );
    let target_file = dir.join(&safe_filename);
    if target_file.exists() {
        let _ = fs::remove_file(&target_file);
    }

    // Update settings
    let mut settings = load_settings();
    settings
        .saved_model_profiles
        .retain(|p| !p.name.eq_ignore_ascii_case(clean_name));
    if settings.active_model_profile.eq_ignore_ascii_case(clean_name) {
        settings.active_model_profile = "Gaming".to_string();
        settings.model_tuning = crate::cascade::ModelProfile::gaming_preset();
    }
    let _ = save_settings(&settings);

    Ok(load_saved_model_profiles())
}

/// Opens the profiles folder in Windows Explorer
pub fn open_profiles_folder_in_explorer() -> Result<(), String> {
    let dir = get_profiles_dir();
    let _ = fs::create_dir_all(&dir);
    #[cfg(windows)]
    {
        crate::paths::create_no_window_command("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_settings_default_censor_shield_enabled() {
        let defaults = UserSettings::default();
        assert!(
            defaults.censor_shield_enabled,
            "Censor Shield MUST be enabled by default"
        );
        assert!(defaults.shield_donate_enabled);
        assert_eq!(defaults.nsfw_threshold, 65);
        assert_eq!(defaults.selected_monitor, 0);
        assert_eq!(defaults.hud_monitor, -1);
        assert_eq!(defaults.operation_mode, 0);
        assert!(!defaults.fps_boosted);
        assert!(defaults.ocr_enabled);
        assert_eq!(defaults.pre_warning_seconds, 20.0);
        assert!(defaults.auto_censor);
        assert!(defaults.cue_notifications);
        assert!(defaults.append_mode);
        assert!(defaults.censor_categories.genitalia);
        assert!(defaults.censor_categories.breasts);
        assert!(defaults.censor_categories.buttocks);
        assert!(!defaults.hide_setup_guide);
        assert_eq!(defaults.close_action, "ask");
        assert_eq!(defaults.hotkey_scope, "global");
        assert_eq!(defaults.hotkey_panic, "F9");
        assert_eq!(defaults.hotkey_threat, "F8");
        assert_eq!(defaults.hotkey_mode, "Ctrl + Shift + F1");
    }

    #[test]
    fn test_user_settings_serialization_roundtrip() {
        let mut settings = UserSettings::default();
        settings.nsfw_threshold = 80;
        settings.selected_monitor = 2;
        settings.hud_monitor = 1;
        settings.censor_shield_enabled = true;
        settings.fps_boosted = true;
        settings.cue_notifications = false;
        settings.censor_categories.underwear = true;
        settings.hide_setup_guide = true;
        settings.close_action = "tray".to_string();
        settings.hotkey_scope = "local".to_string();
        settings.hotkey_panic = "F12".to_string();
        settings.hotkey_threat = "F11".to_string();
        settings.hotkey_mode = "Ctrl + Shift + F2".to_string();

        let json = serde_json::to_string(&settings).expect("serialization failed");
        let parsed: UserSettings = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(settings, parsed);
    }

    #[test]
    fn test_user_settings_empty_json_defaults() {
        let parsed: UserSettings = serde_json::from_str("{}").expect("empty json parsing failed");
        assert!(
            parsed.censor_shield_enabled,
            "Censor shield must default to true on empty json"
        );
        assert!(parsed.shield_donate_enabled);
        assert_eq!(parsed.nsfw_threshold, 65);
        assert_eq!(parsed.selected_monitor, 0);
        assert_eq!(parsed.hud_monitor, -1);
        assert_eq!(parsed.operation_mode, 0);
        assert!(!parsed.fps_boosted);
        assert!(parsed.ocr_enabled);
        assert_eq!(parsed.pre_warning_seconds, 20.0);
        assert!(parsed.auto_censor);
        assert!(parsed.cue_notifications);
        assert!(parsed.append_mode);
        assert!(parsed.censor_categories.genitalia);
        assert!(parsed.censor_categories.breasts);
        assert!(parsed.censor_categories.buttocks);
        assert!(!parsed.censor_categories.underwear);
        assert!(!parsed.hide_setup_guide);
        assert_eq!(parsed.close_action, "ask");
        assert_eq!(parsed.hotkey_scope, "global");
        assert_eq!(parsed.hotkey_panic, "F9");
        assert_eq!(parsed.hotkey_threat, "F8");
        assert_eq!(parsed.hotkey_mode, "Ctrl + Shift + F1");
        assert_eq!(parsed.language, "ru");
    }

    #[test]
    fn test_scheduled_cues_serialization_roundtrip() {
        use crate::cues::CueStatus;
        let cues = vec![ScheduledCue {
            id: "cue-1".to_string(),
            start_sec: 10.5,
            end_sec: 25.0,
            duration_sec: 14.5,
            formatted_range: "00:10 - 00:25".to_string(),
            reason: "Test Scene".to_string(),
            raw_text: "10:30-25:00 Test".to_string(),
            status: CueStatus::Pending,
        }];
        let json = serde_json::to_string(&cues).expect("cues serialization failed");
        let parsed: Vec<ScheduledCue> =
            serde_json::from_str(&json).expect("cues deserialization failed");
        assert_eq!(cues.len(), parsed.len());
        assert_eq!(parsed[0].id, "cue-1");
        assert_eq!(parsed[0].start_sec, 10.5);
        assert_eq!(parsed[0].end_sec, 25.0);
        assert_eq!(parsed[0].reason, "Test Scene");
    }

    #[test]
    fn test_model_profiles_persistence_and_presets() {
        let defaults = UserSettings::default();
        assert_eq!(defaults.active_model_profile, "Gaming");
        assert_eq!(defaults.saved_model_profiles.len(), 3);
        assert!(defaults.model_tuning.vit_game_filter);

        let gaming = &defaults.saved_model_profiles[0];
        assert_eq!(gaming.name, "Gaming");
        assert!(gaming.is_preset);
        assert!(crate::cascade::ModelProfile::is_protected_preset("Gaming"));
        assert!(crate::cascade::ModelProfile::is_protected_preset("gaming"));
        assert!(crate::cascade::ModelProfile::is_protected_preset("Strict"));
        assert!(crate::cascade::ModelProfile::is_protected_preset("IRL"));
        assert!(!crate::cascade::ModelProfile::is_protected_preset("Cyberpunk 2077"));
    }
}
