# Synth

An open-source software synthesizer written in Rust, built up step by step.
Play it with a USB MIDI keyboard, the computer keyboard or the mouse.

*Türkçe açıklama aşağıda.*

## Status — stage 1

- Polyphonic (16 voices) sine synthesizer with an ADSR envelope
- USB MIDI input (all channels): notes, velocity, sustain pedal, pitch bend (±2 semitones), all-notes-off
- MIDI keyboards are found and connected automatically
- Panel with knobs (drag up/down, Shift for fine, double-click for default, mouse wheel), envelope graph, level meter
- On-screen keyboard (C2–C7) that also shows the notes coming from MIDI
- Computer keyboard: `A W S E D F T G Y H U J K O L`, `Z`/`X` change octave

## Building

```
cargo run --release -p synth-app
```

Needs Rust 1.85 or newer. On Windows, a MIDI input can be used by one program at a
time — close your DAW (or let it release the keyboard) before connecting here.

## Layout

| Crate | What it holds |
|---|---|
| `crates/synth-core` | Sound engine: oscillators, envelopes, voices, MIDI parsing. No dependencies, no allocation while rendering — meant to run later inside a plugin too. |
| `crates/synth-app` | Standalone program: sound card (`cpal`), MIDI (`midir`), panel (`egui`). |

Events from MIDI and the panel reach the audio thread through a lock-free queue (`rtrb`).

## Roadmap

1. ~~Single sine voice, MIDI, audio~~ → done, with polyphony and ADSR
2. Classic synth: band-limited saw/square/triangle (PolyBLEP), resonant low-pass filter, second oscillator
3. Modulation: LFO, mod wheel, presets (save/load)
4. Effects: chorus, delay, reverb
5. Wavetable oscillator, unison, modulation matrix
6. VST3/CLAP plugin with `nih-plug`

---

## Türkçe

Rust ile yazılmış, adım adım büyütülen açık kaynak bir yazılım synthesizer.
USB MIDI klavye, bilgisayar klavyesi ya da fareyle çalınır.

**1. aşamada olanlar:** 16 sesli polifonik sinüs, ADSR zarfı, USB MIDI (nota, velocity,
sustain pedalı, pitch bend), otomatik MIDI bağlantısı, knob'lu panel, ekran klavyesi.

**Çalıştırma:** `cargo run --release -p synth-app`

Windows'ta bir MIDI girişini aynı anda tek program kullanabilir; DAW açıksa klavyeyi
bırakmasını sağlayın.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
