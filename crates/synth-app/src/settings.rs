//! Small settings kept between runs, as `key=value` lines in
//! `%APPDATA%\Synth\settings.txt` (`~/.config/synth` elsewhere).

use std::collections::BTreeMap;
use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("Synth").join("settings.txt"))
}

#[derive(Default)]
pub struct Settings(BTreeMap<String, String>);

impl Settings {
    /// Missing or unreadable file gives empty settings.
    pub fn load() -> Self {
        let text = path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        // Windows editors (and PowerShell) may start the file with a byte order mark.
        let text = text.trim_start_matches('\u{feff}');
        Self(
            text.lines()
                .filter_map(|l| l.split_once('='))
                .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
                .collect(),
        )
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Stores and writes at once; a failed write only loses the setting.
    pub fn set(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_string(), value.to_string());
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text: String = self.0.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("ayarlar yazılamadı ({}): {e}", path.display());
        }
    }
}
