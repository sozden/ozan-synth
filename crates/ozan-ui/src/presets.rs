//! Preset list: the factory presets from the engine, then the user's own, saved as
//! JSON files in `%APPDATA%\Synth\presets`.

use std::path::{Path, PathBuf};

use ozan_core::Patch;

pub struct Preset {
    pub name: String,
    pub patch: Patch,
    pub category: Option<Category>,
    /// The user's file; `None` for factory presets.
    pub file: Option<PathBuf>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PresetFile {
    name: String,
    /// Missing in files saved before categories existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<Category>,
    patch: Patch,
}

/// Kind of sound, for finding presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Bass,
    Lead,
    Pad,
    Keys,
    Pluck,
    /// Arpeggios and sequences.
    Sequence,
    Fx,
}

impl Category {
    pub const ALL: [Category; 7] = [Category::Bass, Category::Lead, Category::Pad, Category::Keys, Category::Pluck, Category::Sequence, Category::Fx];

    pub fn name(self) -> &'static str {
        let t = crate::i18n::s();
        match self {
            Category::Bass => t.cat_bass,
            Category::Lead => t.cat_lead,
            Category::Pad => t.cat_pad,
            Category::Keys => t.cat_keys,
            Category::Pluck => t.cat_pluck,
            Category::Sequence => t.cat_seq,
            Category::Fx => t.cat_fx,
        }
    }
}

/// The kind of each factory preset.
pub fn factory_category(name: &str) -> Option<Category> {
    use Category::*;
    Some(match name {
        "Acid Bass" | "Fat Bass" | "Wobble Bass" | "Mono Bass" | "Gritty Bass" | "Sub Bass" | "Reese Bass" | "Pluck Bass" | "Rubber Bass"
        | "Talking Bass" | "Tempo Wobble" | "FM Bass" | "Fold Bass" => Bass,
        "Init" | "Vibrato Lead" | "Supersaw" | "Sync Lead" | "Glide Lead" | "Pure Sine" | "Hoover" | "Square Lead" | "Saw Lead"
        | "Chip Lead" | "Flute" | "Crunch Lead" => Lead,
        "Soft Pad" | "Vowel Pad" | "PWM Strings" | "Evolving Pad" | "Warm Pad" | "Glass Pad" | "Dark Strings" | "Choir" | "Gated Pad"
        | "Band Sweep" | "Phaser Pad" => Pad,
        "Brass" | "Tremolo Organ" | "Harmonic Keys" | "E-Piano" | "Clav" | "Drawbar Organ" | "Brass Stab" | "Dub Chord" | "FM E-Piano" => Keys,
        "Pluck" | "Distant Bell" | "Marimba" | "Kalimba" | "FM Bell" | "Ring Clang" => Pluck,
        "Arp Pluck" | "Arp Bass" | "Arp Sparkle" | "Seq Acid" | "Seq Pluck" => Sequence,
        "S&H Bloop" | "Wind" | "Notch Sweep" | "Laser" | "Ocean" | "Random Bells" => Fx,
        _ => return None,
    })
}

fn dir() -> Option<PathBuf> {
    Some(crate::settings::dir()?.join("presets"))
}

/// All presets, plus a note for each user file that could not be read.
pub fn load_all() -> (Vec<Preset>, Vec<String>) {
    let mut list: Vec<Preset> = ozan_core::presets::factory()
        .into_iter()
        .map(|(name, patch)| Preset { name: name.to_string(), patch, category: factory_category(name), file: None })
        .collect();
    let mut problems = Vec::new();
    let mut user = Vec::new();
    if let Some(entries) = dir().and_then(|d| std::fs::read_dir(d).ok()) {
        for path in entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")) {
            match read(&path) {
                Ok(p) => user.push(Preset { name: p.name, patch: p.patch, category: p.category, file: Some(path) }),
                Err(e) => problems.push(format!("{}: {e}", path.display())),
            }
        }
    }
    user.sort_by_key(|p| p.name.to_lowercase());
    list.extend(user);
    (list, problems)
}

fn read(path: &Path) -> Result<PresetFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

/// File name for a preset name: characters Windows does not allow become `_`.
fn file_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect();
    format!("{}.json", clean.trim().trim_end_matches('.'))
}

/// Where a preset of this name is (or would be) saved.
pub fn path_for(name: &str) -> Option<PathBuf> {
    Some(dir()?.join(file_name(name)))
}

/// Saves (or overwrites) a user preset.
pub fn save(name: &str, patch: &Patch, category: Option<Category>) -> Result<PathBuf, String> {
    let path = path_for(name).ok_or(crate::i18n::s().no_settings_dir)?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&PresetFile { name: name.to_string(), category, patch: *patch }).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn delete(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_factory_preset_has_a_category() {
        for (name, _) in ozan_core::presets::factory() {
            assert!(factory_category(name).is_some(), "{name}");
        }
    }

    #[test]
    fn unsafe_characters_are_replaced() {
        assert_eq!(file_name("Bas: 1/2?"), "Bas_ 1_2_.json");
        assert_eq!(file_name(" Pad. "), "Pad.json");
    }

    #[test]
    fn patch_round_trips_through_json() {
        for (name, patch) in ozan_core::presets::factory() {
            let json = serde_json::to_string(&PresetFile { name: name.into(), category: factory_category(name), patch }).unwrap();
            let back: PresetFile = serde_json::from_str(&json).unwrap();
            assert_eq!(back.patch, patch, "{name}");
        }
    }

    #[test]
    fn older_presets_without_new_fields_still_load() {
        // A stage-2 patch had no LFO, wheel or bend range.
        let mut value = serde_json::to_value(PresetFile { name: "eski".into(), category: None, patch: Patch::default() }).unwrap();
        let patch = value["patch"].as_object_mut().unwrap();
        patch.remove("lfo");
        patch.remove("wheel");
        patch.remove("bend_range");
        // Stage 4 had no unison, matrix or wavetable fields either.
        patch.remove("unison");
        patch.remove("mods");
        patch["filter"].as_object_mut().unwrap().remove("drive");
        for osc in patch["osc"].as_array_mut().unwrap() {
            let osc = osc.as_object_mut().unwrap();
            osc.remove("table");
            osc.remove("position");
        }
        let back: PresetFile = serde_json::from_value(value).unwrap();
        assert_eq!(back.patch, Patch::default());
    }
}
