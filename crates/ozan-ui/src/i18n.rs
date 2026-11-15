//! Interface texts. English is the default; Turkish can be chosen in the header.
//! The interface thread reads the current table through [`s`].

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Tr,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Tr];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Tr => "tr",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| code.trim().eq_ignore_ascii_case(l.code()))
    }

    /// The language's name in itself, as shown in the selector.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Tr => "Türkçe",
        }
    }

    fn strings(self) -> &'static Strings {
        match self {
            Lang::En => &EN,
            Lang::Tr => &TR,
        }
    }
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn current() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) % Lang::ALL.len()]
}

pub fn select(lang: Lang) {
    CURRENT.store(Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0), Ordering::Relaxed);
}

/// Texts in the current language.
pub fn s() -> &'static Strings {
    current().strings()
}

pub fn table_name(kind: ozan_core::WavetableKind) -> &'static str {
    use ozan_core::WavetableKind::*;
    let t = s();
    match kind {
        Basic => t.table_basic,
        Pulse => t.table_pulse,
        Harmonics => t.table_harmonics,
        Vowels => t.table_vowels,
        Sync => t.table_sync,
    }
}

pub fn source_name(src: ozan_core::ModSource) -> &'static str {
    use ozan_core::ModSource::*;
    let t = s();
    match src {
        Off => t.src_off,
        Lfo => t.src_lfo,
        Lfo2 => t.src_lfo2,
        ModEnv => t.src_mod_env,
        FilterEnv => t.src_filter_env,
        AmpEnv => t.src_amp_env,
        Velocity => t.src_velocity,
        Note => t.src_note,
        Wheel => t.src_wheel,
        Aftertouch => t.src_aftertouch,
        Random => t.src_random,
    }
}

pub fn dest_name(dest: ozan_core::ModDest) -> &'static str {
    use ozan_core::ModDest::*;
    let t = s();
    match dest {
        Cutoff => t.dst_cutoff,
        Resonance => t.dst_resonance,
        Pitch => t.dst_pitch,
        Osc1Pitch => t.dst_osc1_pitch,
        Osc2Pitch => t.dst_osc2_pitch,
        Osc1Position => t.dst_osc1_position,
        Osc2Position => t.dst_osc2_position,
        Osc1Level => t.dst_osc1_level,
        Osc2Level => t.dst_osc2_level,
        Volume => t.dst_volume,
        Pan => t.dst_pan,
        UnisonDetune => t.dst_detune,
    }
}

/// Replaces each `{}` in `template` with the next value.
pub fn fill(template: &str, values: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut parts = template.split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        if let Some(v) = values.get(i) {
            out.push_str(&v.to_string());
        }
        out.push_str(part);
    }
    out
}

pub struct Strings {
    // Header and presets
    pub prev_preset: &'static str,
    pub next_preset: &'static str,
    pub unsaved: &'static str,
    pub your_presets: &'static str,
    pub save: &'static str,
    pub save_hint: &'static str,
    pub delete: &'static str,
    pub factory_locked: &'static str,
    pub save_title: &'static str,
    pub preset_name: &'static str,
    pub will_overwrite: &'static str,
    pub cancel: &'static str,
    pub delete_title: &'static str,
    /// `{}`: preset name.
    pub delete_body: &'static str,
    pub unreadable_preset: &'static str,
    pub save_failed: &'static str,
    pub delete_failed: &'static str,
    pub audio_failed: &'static str,
    pub midi_input: &'static str,
    pub no_device: &'static str,
    pub off: &'static str,
    pub rescan: &'static str,
    pub midi_led: &'static str,
    pub panic: &'static str,
    pub panic_hint: &'static str,
    pub theme: &'static str,
    pub language: &'static str,
    pub theme_studio: &'static str,
    pub theme_night: &'static str,
    // Sections
    pub osc1: &'static str,
    pub osc2: &'static str,
    pub filter: &'static str,
    pub filter_env: &'static str,
    pub amp_env: &'static str,
    pub output: &'static str,
    pub lfo: &'static str,
    pub wheel_section: &'static str,
    pub chorus: &'static str,
    pub delay: &'static str,
    pub reverb: &'static str,
    // Knobs and switches
    pub octave: &'static str,
    pub semitone: &'static str,
    pub fine: &'static str,
    pub level: &'static str,
    pub cutoff: &'static str,
    pub resonance: &'static str,
    pub env_amount: &'static str,
    pub key_track: &'static str,
    pub attack: &'static str,
    pub decay: &'static str,
    pub sustain: &'static str,
    pub release: &'static str,
    pub volume: &'static str,
    pub voices: &'static str,
    pub init: &'static str,
    pub init_hint: &'static str,
    pub lfo_led: &'static str,
    pub rate: &'static str,
    pub vibrato: &'static str,
    pub filter_knob: &'static str,
    pub tremolo: &'static str,
    pub wheel: &'static str,
    pub aftertouch: &'static str,
    pub bend: &'static str,
    pub on: &'static str,
    pub ping_pong: &'static str,
    pub depth: &'static str,
    pub mix: &'static str,
    pub time: &'static str,
    pub feedback: &'static str,
    pub tone: &'static str,
    pub size: &'static str,
    pub reverb_decay: &'static str,
    pub damping: &'static str,
    pub predelay: &'static str,
    /// Unit after semitone values.
    pub semitone_unit: &'static str,
    /// `{}`: octave of the A key.
    pub pc_keys: &'static str,
    // Waveforms and filter types (tooltips)
    pub sine: &'static str,
    pub saw: &'static str,
    pub square: &'static str,
    pub triangle: &'static str,
    pub ramp: &'static str,
    pub sample_hold: &'static str,
    pub lp12: &'static str,
    pub lp24: &'static str,
    pub band_pass: &'static str,
    pub high_pass: &'static str,
    // Wavetables, unison, matrix
    pub wavetable: &'static str,
    pub position: &'static str,
    pub table_basic: &'static str,
    pub table_pulse: &'static str,
    pub table_harmonics: &'static str,
    pub table_vowels: &'static str,
    pub table_sync: &'static str,
    pub unison: &'static str,
    pub unison_voices: &'static str,
    pub detune: &'static str,
    pub spread: &'static str,
    pub mod_matrix: &'static str,
    pub source: &'static str,
    pub destination: &'static str,
    pub amount: &'static str,
    pub src_off: &'static str,
    pub src_lfo: &'static str,
    pub src_filter_env: &'static str,
    pub src_amp_env: &'static str,
    pub src_velocity: &'static str,
    pub src_note: &'static str,
    pub src_wheel: &'static str,
    pub src_aftertouch: &'static str,
    pub src_random: &'static str,
    pub dst_cutoff: &'static str,
    pub dst_resonance: &'static str,
    pub dst_pitch: &'static str,
    pub dst_osc1_pitch: &'static str,
    pub dst_osc2_pitch: &'static str,
    pub dst_osc1_position: &'static str,
    pub dst_osc2_position: &'static str,
    pub dst_osc1_level: &'static str,
    pub dst_osc2_level: &'static str,
    pub dst_volume: &'static str,
    pub dst_pan: &'static str,
    pub dst_detune: &'static str,
    // Play section
    pub play: &'static str,
    pub poly: &'static str,
    pub mono: &'static str,
    pub legato: &'static str,
    pub poly_hint: &'static str,
    pub mono_hint: &'static str,
    pub legato_hint: &'static str,
    pub glide: &'static str,
    pub velocity: &'static str,
    pub noise: &'static str,
    // Arpeggiator
    pub arp: &'static str,
    pub hold: &'static str,
    pub arp_up: &'static str,
    pub arp_down: &'static str,
    pub arp_updown: &'static str,
    pub arp_random: &'static str,
    pub arp_played: &'static str,
    pub arp_up_hint: &'static str,
    pub arp_down_hint: &'static str,
    pub arp_updown_hint: &'static str,
    pub arp_random_hint: &'static str,
    pub arp_played_hint: &'static str,
    pub octaves: &'static str,
    pub gate: &'static str,
    pub tempo: &'static str,
    pub step: &'static str,
    // Second LFO, mod envelope
    pub lfo2: &'static str,
    pub mod_env: &'static str,
    pub src_lfo2: &'static str,
    pub src_mod_env: &'static str,
    pub lfo2_hint: &'static str,
    // Errors from devices and files
    pub no_audio_device: &'static str,
    pub sample_format: &'static str,
    pub midi_gone: &'static str,
    /// `{}`: device, error.
    pub midi_busy: &'static str,
    pub no_settings_dir: &'static str,
}

pub static EN: Strings = Strings {
    prev_preset: "Previous preset",
    next_preset: "Next preset",
    unsaved: "—  unsaved sound",
    your_presets: "Your presets",
    save: "Save",
    save_hint: "Saves the current sound as a preset",
    delete: "Delete",
    factory_locked: "Factory presets can't be deleted",
    save_title: "Save preset",
    preset_name: "Preset name",
    will_overwrite: "A preset with this name exists; it will be replaced.",
    cancel: "Cancel",
    delete_title: "Delete preset?",
    delete_body: "“{}” will be deleted for good; this can't be undone.",
    unreadable_preset: "Unreadable preset: {}",
    save_failed: "Couldn't save: {}",
    delete_failed: "Couldn't delete: {}",
    audio_failed: "Couldn't open the sound output: {}",
    midi_input: "MIDI input",
    no_device: "no device",
    off: "off",
    rescan: "Look for MIDI devices again",
    midi_led: "Lights up as MIDI messages arrive",
    panic: "All notes off",
    panic_hint: "Stops playing notes, echoes and reverb tails",
    theme: "Theme",
    language: "Language",
    theme_studio: "Studio (70s)",
    theme_night: "Night",
    osc1: "OSCILLATOR 1",
    osc2: "OSCILLATOR 2",
    filter: "FILTER",
    filter_env: "FILTER ENVELOPE",
    amp_env: "AMP ENVELOPE",
    output: "OUTPUT",
    lfo: "LFO 1",
    wheel_section: "WHEEL · AFTERTOUCH · BEND",
    chorus: "CHORUS",
    delay: "DELAY",
    reverb: "REVERB",
    octave: "Octave",
    semitone: "Semitone",
    fine: "Fine",
    level: "Level",
    cutoff: "Cutoff",
    resonance: "Reso",
    env_amount: "Env amt",
    key_track: "Key track",
    attack: "Attack",
    decay: "Decay",
    sustain: "Sustain",
    release: "Release",
    volume: "Volume",
    voices: "Voices",
    init: "Init",
    init_hint: "Resets every setting to its starting value",
    lfo_led: "The LFO's current value",
    rate: "Rate",
    vibrato: "Vibrato",
    filter_knob: "Filter",
    tremolo: "Tremolo",
    wheel: "Wheel",
    aftertouch: "Aftertouch",
    bend: "Bend",
    on: "On",
    ping_pong: "Ping-pong",
    depth: "Depth",
    mix: "Mix",
    time: "Time",
    feedback: "Feedback",
    tone: "Tone",
    size: "Size",
    reverb_decay: "Decay",
    damping: "Damping",
    predelay: "Pre-delay",
    semitone_unit: "st",
    pc_keys: "Computer keys: A W S E D F T G Y H U J K · Z/X octave (A = C{})",
    sine: "Sine",
    saw: "Saw",
    square: "Square",
    triangle: "Triangle",
    ramp: "Ramp",
    sample_hold: "Random (sample & hold)",
    lp12: "Low-pass, 12 dB/octave",
    lp24: "Low-pass, 24 dB/octave",
    band_pass: "Band-pass",
    high_pass: "High-pass",
    wavetable: "Wavetable",
    position: "Position",
    table_basic: "Basic",
    table_pulse: "Pulse width",
    table_harmonics: "Harmonics",
    table_vowels: "Vowels",
    table_sync: "Sync sweep",
    unison: "UNISON",
    unison_voices: "Copies",
    detune: "Detune",
    spread: "Spread",
    mod_matrix: "MOD MATRIX",
    source: "Source",
    destination: "Destination",
    amount: "Amount",
    src_off: "—",
    src_lfo: "LFO 1",
    src_filter_env: "Filter env",
    src_amp_env: "Amp env",
    src_velocity: "Velocity",
    src_note: "Note",
    src_wheel: "Mod wheel",
    src_aftertouch: "Aftertouch",
    src_random: "Random",
    dst_cutoff: "Cutoff",
    dst_resonance: "Resonance",
    dst_pitch: "Pitch",
    dst_osc1_pitch: "Osc 1 pitch",
    dst_osc2_pitch: "Osc 2 pitch",
    dst_osc1_position: "Osc 1 position",
    dst_osc2_position: "Osc 2 position",
    dst_osc1_level: "Osc 1 level",
    dst_osc2_level: "Osc 2 level",
    dst_volume: "Volume",
    dst_pan: "Pan",
    dst_detune: "Unison detune",
    play: "PLAY",
    poly: "Poly",
    mono: "Mono",
    legato: "Legato",
    poly_hint: "Up to 16 notes at once",
    mono_hint: "One note at a time; every key restarts the envelopes",
    legato_hint: "One note at a time; keys played while another is held only move the pitch",
    glide: "Glide",
    velocity: "Velocity",
    noise: "Noise",
    arp: "ARPEGGIATOR",
    hold: "Hold",
    arp_up: "Up",
    arp_down: "Down",
    arp_updown: "Up/Dn",
    arp_random: "Rnd",
    arp_played: "Played",
    arp_up_hint: "Lowest to highest",
    arp_down_hint: "Highest to lowest",
    arp_updown_hint: "Up, then back down",
    arp_random_hint: "A random held note each step",
    arp_played_hint: "In the order the keys were pressed",
    octaves: "Octaves",
    gate: "Gate",
    tempo: "Tempo",
    step: "Step",
    lfo2: "LFO 2",
    mod_env: "MOD ENVELOPE",
    src_lfo2: "LFO 2",
    src_mod_env: "Mod env",
    lfo2_hint: "Reaches its targets through the mod matrix",
    no_audio_device: "no sound output device",
    sample_format: "unsupported sample format {}",
    midi_gone: "“{}” is no longer connected",
    midi_busy: "Couldn't open “{}” ({}); another program may be using it",
    no_settings_dir: "settings folder not found",
};

pub static TR: Strings = Strings {
    prev_preset: "Önceki preset",
    next_preset: "Sonraki preset",
    unsaved: "—  kaydedilmemiş ses",
    your_presets: "Senin preset'lerin",
    save: "Kaydet",
    save_hint: "Şu anki sesi preset olarak kaydeder",
    delete: "Sil",
    factory_locked: "Fabrika preset'leri silinemez",
    save_title: "Preset kaydet",
    preset_name: "Preset adı",
    will_overwrite: "Bu adda bir preset var; üzerine yazılacak.",
    cancel: "Vazgeç",
    delete_title: "Preset silinsin mi?",
    delete_body: "“{}” dosyası kalıcı olarak silinir; geri alınamaz.",
    unreadable_preset: "Okunamayan preset: {}",
    save_failed: "Kaydedilemedi: {}",
    delete_failed: "Silinemedi: {}",
    audio_failed: "Ses çıkışı açılamadı: {}",
    midi_input: "MIDI giriş",
    no_device: "aygıt yok",
    off: "kapalı",
    rescan: "MIDI aygıtlarını yeniden ara",
    midi_led: "MIDI mesajı geldikçe yanar",
    panic: "Tüm notaları sustur",
    panic_hint: "Çalan notaları, yankı ve reverb kuyruklarını keser",
    theme: "Tema",
    language: "Dil",
    theme_studio: "Stüdyo (70'ler)",
    theme_night: "Gece",
    osc1: "OSİLATÖR 1",
    osc2: "OSİLATÖR 2",
    filter: "FİLTRE",
    filter_env: "FİLTRE ZARFI",
    amp_env: "AMFİ ZARFI",
    output: "ÇIKIŞ",
    lfo: "LFO 1",
    wheel_section: "TEKERLEK · AFTERTOUCH · BEND",
    chorus: "CHORUS",
    delay: "DELAY",
    reverb: "REVERB",
    octave: "Oktav",
    semitone: "Yarım ton",
    fine: "İnce",
    level: "Seviye",
    cutoff: "Cutoff",
    resonance: "Rezonans",
    env_amount: "Zarf",
    key_track: "Nota izi",
    attack: "Attack",
    decay: "Decay",
    sustain: "Sustain",
    release: "Release",
    volume: "Ses",
    voices: "Ses sayısı",
    init: "Varsayılan",
    init_hint: "Tüm ayarları başlangıç değerine döndürür",
    lfo_led: "LFO'nun o anki değeri",
    rate: "Hız",
    vibrato: "Vibrato",
    filter_knob: "Filtre",
    tremolo: "Tremolo",
    wheel: "Teker",
    aftertouch: "Aftertouch",
    bend: "Bend",
    on: "Açık",
    ping_pong: "Ping-pong",
    depth: "Derinlik",
    mix: "Karışım",
    time: "Süre",
    feedback: "Tekrar",
    tone: "Ton",
    size: "Boyut",
    reverb_decay: "Süre",
    damping: "Sönüm",
    predelay: "Gecikme",
    semitone_unit: "yt",
    pc_keys: "Bilgisayar klavyesi: A W S E D F T G Y H U J K · Z/X oktav (A = C{})",
    sine: "Sinüs",
    saw: "Testere",
    square: "Kare",
    triangle: "Üçgen",
    ramp: "Rampa",
    sample_hold: "Rastgele (sample & hold)",
    lp12: "Alçak geçiren, 12 dB/oktav",
    lp24: "Alçak geçiren, 24 dB/oktav",
    band_pass: "Bant geçiren",
    high_pass: "Yüksek geçiren",
    wavetable: "Wavetable",
    position: "Pozisyon",
    table_basic: "Temel",
    table_pulse: "Darbe genişliği",
    table_harmonics: "Harmonikler",
    table_vowels: "Ünlüler",
    table_sync: "Sync taraması",
    unison: "UNISON",
    unison_voices: "Kopya",
    detune: "Detune",
    spread: "Genişlik",
    mod_matrix: "MOD MATRİSİ",
    source: "Kaynak",
    destination: "Hedef",
    amount: "Miktar",
    src_off: "—",
    src_lfo: "LFO 1",
    src_filter_env: "Filtre zarfı",
    src_amp_env: "Amfi zarfı",
    src_velocity: "Velocity",
    src_note: "Nota",
    src_wheel: "Mod tekeri",
    src_aftertouch: "Aftertouch",
    src_random: "Rastgele",
    dst_cutoff: "Cutoff",
    dst_resonance: "Rezonans",
    dst_pitch: "Perde",
    dst_osc1_pitch: "Osc 1 perde",
    dst_osc2_pitch: "Osc 2 perde",
    dst_osc1_position: "Osc 1 pozisyon",
    dst_osc2_position: "Osc 2 pozisyon",
    dst_osc1_level: "Osc 1 seviye",
    dst_osc2_level: "Osc 2 seviye",
    dst_volume: "Ses",
    dst_pan: "Pan",
    dst_detune: "Unison detune",
    play: "ÇALMA",
    poly: "Poly",
    mono: "Mono",
    legato: "Legato",
    poly_hint: "Aynı anda 16 notaya kadar",
    mono_hint: "Tek nota; her tuş zarfları yeniden başlatır",
    legato_hint: "Tek nota; başka tuş basılıyken çalınanlar yalnızca perdeyi değiştirir",
    glide: "Kayma",
    velocity: "Velocity",
    noise: "Gürültü",
    arp: "ARPEJ",
    hold: "Tut",
    arp_up: "Yukarı",
    arp_down: "Aşağı",
    arp_updown: "Y/A",
    arp_random: "Rast.",
    arp_played: "Sıra",
    arp_up_hint: "Pesten tize",
    arp_down_hint: "Tizden pese",
    arp_updown_hint: "Yukarı, sonra aşağı",
    arp_random_hint: "Her adımda rastgele bir nota",
    arp_played_hint: "Tuşlara basılış sırasıyla",
    octaves: "Oktav",
    gate: "Gate",
    tempo: "Tempo",
    step: "Adım",
    lfo2: "LFO 2",
    mod_env: "MOD ZARFI",
    src_lfo2: "LFO 2",
    src_mod_env: "Mod zarfı",
    lfo2_hint: "Hedeflerine mod matrisi üzerinden ulaşır",
    no_audio_device: "ses çıkış aygıtı bulunamadı",
    sample_format: "desteklenmeyen örnek biçimi {}",
    midi_gone: "“{}” artık bağlı değil",
    midi_busy: "“{}” açılamadı ({}); başka bir program kullanıyor olabilir",
    no_settings_dir: "ayar klasörü bulunamadı",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_have_the_same_placeholders() {
        for (en, tr) in [
            (EN.delete_body, TR.delete_body),
            (EN.unreadable_preset, TR.unreadable_preset),
            (EN.save_failed, TR.save_failed),
            (EN.delete_failed, TR.delete_failed),
            (EN.audio_failed, TR.audio_failed),
            (EN.pc_keys, TR.pc_keys),
            (EN.sample_format, TR.sample_format),
            (EN.midi_gone, TR.midi_gone),
            (EN.midi_busy, TR.midi_busy),
        ] {
            assert_eq!(en.matches("{}").count(), tr.matches("{}").count(), "{en} / {tr}");
        }
    }

    #[test]
    fn fill_replaces_in_order() {
        assert_eq!(fill("“{}” ({})", &[&"KeyStep", &5]), "“KeyStep” (5)");
    }

    #[test]
    fn codes_round_trip() {
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
        assert_eq!(Lang::from_code("TR "), Some(Lang::Tr));
    }
}
