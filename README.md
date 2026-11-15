<p align="center"><img src="assets/logo.svg" alt="OZAN Synth" width="520"></p>

# OZAN Synth

An open-source software synthesizer written in Rust, built up step by step.
Play it with a USB MIDI keyboard, the computer keyboard or the mouse.

*Ozan* (oh-ZAHN) is Turkish for a wandering poet-musician, the bard who travels with his instrument.
The logo is drawn from geometry in `assets/make_logo.py`: the Z is a saw tooth, the A an envelope's attack.

*Türkçe açıklama aşağıda.*

## Status — stage 5 + play modes

- Classic subtractive voice: 2 oscillators → resonant filter → amplifier, 16 voices, stereo output
- Band-limited sine, saw, square and triangle (PolyBLEP / PolyBLAMP — no aliasing whine on high notes)
- Oscillator octave, semitone (osc 2), fine tune and level
- Wavetable oscillator with 5 built-in tables (basic shapes, pulse width, harmonics, vowels, sync sweep), 32 frames each, one band-limited copy per octave so high notes do not alias
- Arpeggiator: up, down, up-down, random and played order; 1/4 to 1/32 with triplets, 1–4 octaves, gate, hold; follows the host's tempo in a DAW, its own tempo knob otherwise; steps land on the exact sample
- Poly, mono and legato play modes (mono keeps a stack of held keys), glide (portamento), velocity sensitivity, white noise source
- Unison: up to 7 detuned copies per oscillator, spread across the stereo field at constant loudness
- Second LFO and a third (mod) envelope, both as matrix sources
- Modulation matrix: 6 slots, sources LFO 1 and 2, filter/amp/mod envelope, velocity, note, mod wheel, aftertouch, random-per-note; destinations pitch, wavetable position, levels, cutoff, resonance, volume, pan, unison detune
- Zero-delay-feedback state-variable filter: LP 12 dB, LP 24 dB, band-pass, high-pass, notch; cutoff, resonance, envelope amount (±7 octaves), key tracking, drive (soft saturation before the filter, with a DC blocker)
- Separate ADSR envelopes for filter and amplifier
- Gentle output saturation instead of hard clipping
- LFO (sine, triangle, ramp, square, sample & hold) for vibrato, filter wobble and tremolo
- Mod wheel and aftertouch: extra vibrato and cutoff; adjustable pitch-bend range
- Effects: stereo chorus, delay (ping-pong, darkening repeats, tape-like glide when the time is turned), reverb (8-line feedback delay network: size, decay, damping, pre-delay); each fades in and out when switched
- Presets: 25 factory sounds, save/delete your own (JSON in `%APPDATA%\Ozan\presets`), step with ◀ ▶ or MIDI program change
- USB MIDI input (all channels): notes, velocity, sustain pedal, pitch bend, mod wheel, aftertouch, program change, all-notes-off
- MIDI keyboards are found and connected automatically
- MIDI learn (standalone program): right-click a knob, choose *MIDI learn*, turn a knob on your controller; bindings are remembered
- Panel with knobs (drag up/down, Shift for fine, double-click for default, mouse wheel), envelope graphs, analogue VU meter, and a live scope: triggered waveform or log-frequency spectrum of the output
- English or Turkish interface, and two looks, chosen in the header and remembered: **Studio** (1970s style — wooden end cheeks, skirted knobs, cream lettering) and **Night** (dark cards with screws)
- On-screen keyboard (C2–C7) that also shows the notes coming from MIDI
- Computer keyboard: `A W S E D F T G Y H U J K O L`, `Z`/`X` change octave

## Building

```
cargo run --release -p ozan
```

Needs Rust 1.85 or newer. On Windows, a MIDI input can be used by one program at a
time — close your DAW (or let it release the keyboard) before connecting here.

## Plugin (CLAP / VST3)

```
powershell -ExecutionPolicy Bypass -File scripts\bundle.ps1
```

builds `dist\OZAN Synth.clap` and `dist\OZAN Synth.vst3`, then loads the CLAP once in a
small test host (`ozan-lab plugin`) to check that it plays. Copy the `.vst3` folder to
`C:\Program Files\Common Files\VST3` (or point your DAW at `dist`), the `.clap` file to
`C:\Program Files\Common Files\CLAP`.

The plugin opens the same panel as the standalone program (theme and language follow
the program's settings); the edited sound is saved with the DAW project. Presets you
save in the program appear in the plugin and the other way round (same folder). For automation
the host also sees a **Preset** selector and relative controls that leave the sound alone
at their defaults: volume, cutoff, resonance, filter envelope amount, attack and release,
LFO rate, wavetable position, unison detune, glide, and chorus/delay/reverb mix.
`ozan-lab plugin-gui <file.clap>` opens the plugin's window in a small test host. Built with [nih-plug](https://github.com/robbert-vdh/nih-plug) (ISC),
pinned to a reviewed commit.

## Layout

| Crate | What it holds |
|---|---|
| `crates/ozan-core` | Sound engine: oscillators, filter, envelopes, LFO, voices, MIDI parsing, factory presets. No required dependencies (`serde` optional), no allocation while rendering — meant to run later inside a plugin too. |
| `crates/ozan` | Standalone program: sound card (`cpal`), MIDI (`midir`), panel (`egui`). |

| `crates/ozan-ui` | Interface shared by the program and the plugin (egui 0.31): theme, knobs, meters, logo, translations, settings, the sound panel. |
| `crates/ozan-plugin` | CLAP and VST3 instrument through nih-plug and nih_plug_egui: the panel in the plugin window, preset selector and relative macro parameters. |
| `crates/ozan-lab` | Listening lab (not shipped): renders every preset to WAV with spectrograms and measurements, records the real output via WASAPI loopback. |

Events from MIDI and the panel reach the audio thread through a lock-free queue (`rtrb`).

## Listening lab

```
cargo run --release -p ozan-lab -- render lab-out        # every preset: WAV + spectrogram + table
cargo run --release -p ozan-lab -- loopback 7 out.wav    # record what the computer plays, look for dropouts
cargo run --release -p ozan-lab -- analyze out.wav       # measure a recording again
```

The test phrase plays a low note, a chord, a mod-wheel sweep, a pitch-bend swing, fast
repeated notes and a high C7 (to show aliasing). The table lists peak and RMS level, DC
offset, the level after all notes end, and a click score at every note start/end.

## Roadmap

1. ~~Single sine voice, MIDI, audio~~ → done, with polyphony and ADSR
2. ~~Classic synth: band-limited saw/square/triangle (PolyBLEP), resonant filter, second oscillator~~ → done
3. ~~Modulation: LFO, mod wheel, presets (save/load)~~ → done, with aftertouch and program change
4. ~~Effects: chorus, delay, reverb~~ → done
5. ~~Wavetable oscillator, unison, modulation matrix~~ → done
6. ~~VST3/CLAP plugin with `nih-plug`~~ → done, with the full panel as the plugin window

---

## Türkçe

OZAN Synth, Rust ile yazılmış, adım adım büyütülen açık kaynak bir yazılım synthesizer.
Adı, sazıyla diyar diyar gezen halk şairinden geliyor.
USB MIDI klavye, bilgisayar klavyesi ya da fareyle çalınır.

**Şu an (5. aşama):** 16 sesli, iki osilatörlü klasik synth. Aliasing'siz sinüs, testere,
kare ve üçgen dalgalar; rezonanslı filtre (LP12, LP24, BP, HP), filtre ve amfi için ayrı
ADSR zarfları. LFO (vibrato, filtre, tremolo), mod tekeri ve aftertouch. Stereo chorus,
delay (ping-pong) ve reverb efektleri. Wavetable osilatör (5 tablo), 7 kopyaya kadar unison
ve 6 slotlu mod matrisi. Poly/mono/legato, glide, velocity hassasiyeti, gürültü, arpejiyatör (DAW temposunu izler), ikinci LFO ve mod zarfı; çıkışın canlı dalga biçimi ve spektrumu; filtrede drive ve notch; MIDI learn (knob'a sağ tıkla, kontrolcünde bir düğme çevir). 25 fabrika
preset'i; kendi preset'lerini kaydedip silebilir, MIDI program change ile seçebilirsin.
USB MIDI otomatik bağlanır; Stüdyo (70'ler) ve Gece temaları arasında seçim yapılır.
Ayrıca CLAP/VST3 eklentisi olarak DAW içinde çalışır; kendi preset'lerin uygulama ile
eklenti arasında ortaktır (`scripts\bundle.ps1`).

**Çalıştırma:** `cargo run --release -p ozan`. Arayüz varsayılan olarak İngilizce;
başlıktaki **Language** menüsünden Türkçe seçilebilir.

Windows'ta bir MIDI girişini aynı anda tek program kullanabilir; DAW açıksa klavyeyi
bırakmasını sağlayın.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
