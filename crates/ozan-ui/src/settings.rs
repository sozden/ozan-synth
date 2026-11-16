//! Small settings kept between runs, as `key=value` lines in
//! `%APPDATA%\Ozan\settings.txt` (`~/.config/Ozan` elsewhere).

use std::collections::BTreeMap;
use std::path::PathBuf;

fn base() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// Folder of the program's own files.
pub fn dir() -> Option<PathBuf> {
    Some(base()?.join("Ozan"))
}

/// Before it was named Ozan the program kept its files in a "Synth" folder; it is
/// renamed once so settings and presets carry over.
pub fn migrate_old_folder() {
    let (Some(old), Some(new)) = (base().map(|b| b.join("Synth")), dir()) else { return };
    if old.join("settings.txt").is_file() && !new.exists() {
        if let Err(e) = std::fs::rename(&old, &new) {
            eprintln!("could not move {} to {}: {e}", old.display(), new.display());
        }
    }
}

fn path() -> Option<PathBuf> {
    Some(dir()?.join("settings.txt"))
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
        // Others (the preset browser, the plugin) write this file too: start from
        // what is on disk, so their settings are not lost.
        let mut fresh = Self::load();
        fresh.0.insert(key.to_string(), value.to_string());
        self.0 = fresh.0;
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text: String = self.0.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("settings not written ({}): {e}", path.display());
        }
    }
}
