use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::env;
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub transcription: Transcription,
    pub audio: Audio,
    pub shortcuts: Shortcuts,
    pub app: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Transcription {
    pub groq_api_key: String,
    pub model: String,
    pub language: String,
    pub api_timeout: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    pub input_device: String,
    pub minimum_duration: f64,
    pub silence_threshold: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    pub push_to_talk: String,
    pub handsfree_enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub autostart: bool,
    pub save_history: bool,
    pub clipboard_restore_delay: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transcription: Transcription::default(),
            audio: Audio::default(),
            shortcuts: Shortcuts::default(),
            app: AppSettings::default(),
        }
    }
}
impl Default for Transcription {
    fn default() -> Self {
        Self {
            groq_api_key: String::new(),
            model: "whisper-large-v3-turbo".into(),
            language: "en".into(),
            api_timeout: 45.0,
        }
    }
}
impl Default for Audio {
    fn default() -> Self {
        Self {
            input_device: String::new(),
            minimum_duration: 0.35,
            silence_threshold: 220.0,
        }
    }
}
impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            push_to_talk: "ctrl+super".into(),
            handsfree_enabled: true,
        }
    }
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            autostart: true,
            save_history: false,
            clipboard_restore_delay: 0.6,
        }
    }
}

pub fn config_path() -> PathBuf {
    #[cfg(not(target_os = "linux"))]
    return dirs::config_dir()
        .expect("User configuration directory unavailable")
        .join("flow-linux/config.toml");
    #[cfg(target_os = "linux")]
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        .join("flow-linux/config.toml")
}
pub fn data_dir() -> PathBuf {
    #[cfg(not(target_os = "linux"))]
    return dirs::data_local_dir()
        .expect("User data directory unavailable")
        .join("flow-linux");
    #[cfg(target_os = "linux")]
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/share")
        })
        .join("flow-linux")
}

pub fn load() -> Result<Config, String> {
    let path = config_path();
    if path.exists() {
        let text =
            fs::read_to_string(&path).map_err(|e| format!("Could not read settings: {e}"))?;
        let config: Config =
            toml::from_str(&text).map_err(|e| format!("Invalid config.toml: {e}"))?;
        validate(&config)?;
        secure_config(&path)?;
        return Ok(config);
    }
    let mut config = Config::default();
    // Read the old .env without printing or deleting it; this preserves users who
    // upgrade directly from an older install that predates config.toml.
    let legacy = path.with_file_name(".env");
    if let Ok(content) = fs::read_to_string(legacy) {
        for line in content.lines() {
            if let Some((key, value)) = line.trim().split_once('=') {
                let value = value.trim().trim_matches(['"', '\'']);
                match key.trim() {
                    "GROQ_API_KEY" => config.transcription.groq_api_key = value.to_string(),
                    "FLOW_MODEL" => config.transcription.model = value.to_string(),
                    "FLOW_LANGUAGE" => config.transcription.language = value.to_string(),
                    _ => {}
                }
            }
        }
    }
    save(&config)?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<(), String> {
    validate(config)?;
    let path = config_path();
    let parent = path.parent().ok_or("Invalid config location")?;
    crate::storage::secure_directory(parent)?;
    let body = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    use std::io::Write;
    temporary
        .write_all(body.as_bytes())
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary
        .persist(&path)
        .map_err(|e| format!("Could not save settings: {e}"))?;
    secure_config(&path)
}
fn secure_config(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        crate::storage::secure_directory(parent)?;
    }
    crate::storage::secure_file(path)
}

fn validate(c: &Config) -> Result<(), String> {
    if c.transcription.model.trim().is_empty() || c.transcription.language.trim().is_empty() {
        return Err("Model and language cannot be empty".into());
    }
    if c.transcription.api_timeout <= 0.0
        || c.audio.minimum_duration < 0.0
        || c.audio.silence_threshold < 0.0
        || c.app.clipboard_restore_delay < 0.0
    {
        return Err("Timeout and audio settings must be non-negative".into());
    }
    let shortcut = c.shortcuts.push_to_talk.to_lowercase();
    let keys: Vec<_> = shortcut.split('+').map(str::trim).collect();
    if keys.len() != 2
        || keys[0] == keys[1]
        || !keys
            .iter()
            .all(|k| ["ctrl", "super", "alt", "shift"].contains(k))
    {
        return Err("Shortcut must be a pair such as ctrl+super".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_config_defaults_missing_fields() {
        let c: Config = toml::from_str("[transcription]\nlanguage='en'\n").unwrap();
        assert_eq!(c.transcription.model, "whisper-large-v3-turbo");
        assert!(c.shortcuts.handsfree_enabled);
    }
    #[test]
    fn rejects_invalid_shortcut() {
        let mut c = Config::default();
        c.shortcuts.push_to_talk = "ctrl+ctrl".into();
        assert!(validate(&c).is_err());
    }
}
