# Changelog

## 1.0.0 — 2026-11-18

First release with a Windows installer. Tested as a VST3 plugin in Waveform Free 14
(tempo sync, sequencer, saving with the project, host parameters).

**Sound**
- Two oscillators (sine, saw, square, triangle, wavetable), up to 7 unison copies each,
  noise, FM and ring modulation (oscillator 2 → oscillator 1)
- Two filters (LP12, LP24, BP, HP, notch, with drive), the second serial or parallel
- Three envelopes, two LFOs (tempo-synced if wanted), a pin-board modulation matrix with
  15 destinations
- Arpeggiator and a 16-step sequencer (note, rest, accent and tie per step; the key you
  hold moves the pattern)
- Effects: distortion (soft, hard, fold), phaser, chorus, delay (tempo-synced if wanted),
  reverb, 3-band EQ
- 62 factory presets in seven categories

**Playing and editing**
- Standalone program with USB MIDI, pitch and mod wheels, computer keyboard
- CLAP and VST3 plugin with the same panel; presets shared between both
- Preset browser with categories, search and favourites
- Undo and redo, a dice for random sounds, MIDI learn (standalone)
- Studio and Night looks; English and Turkish

**Installer**
- `OZAN-Synth-1.0.0-Setup.exe`: program, VST3 and CLAP plugins, Start menu entry,
  uninstaller; English and Turkish
