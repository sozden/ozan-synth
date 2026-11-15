//! MIDI learn: knobs bound to controller numbers (CC).
//!
//! Each learnable knob has a key that does not depend on the interface language,
//! made of its section and its own name ("filter.cutoff", "amp_env.attack"). Right-
//! clicking a knob arms learning; the next CC that arrives is bound to it. Incoming CC
//! values wait here until the bound knob is drawn, which then takes the value: the
//! knob's own range and curve apply, and the change reaches the engine like a drag.
//!
//! All of this lives on the interface thread.

use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Default)]
struct State {
    enabled: bool,
    /// Knob key → controller number.
    map: BTreeMap<String, u8>,
    /// Knob waiting for a controller.
    armed: Option<String>,
    /// Values (0..1) not yet taken by their knob.
    pending: BTreeMap<String, f32>,
    /// Section being drawn.
    section: &'static str,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Turns learning on (the standalone program) and loads saved bindings.
pub fn enable(saved: Option<&str>) {
    STATE.with_borrow_mut(|s| {
        s.enabled = true;
        s.map = parse(saved.unwrap_or(""));
    });
}

pub fn is_enabled() -> bool {
    STATE.with_borrow(|s| s.enabled)
}

/// Bindings as text for the settings file: `key:cc,key:cc`.
pub fn save() -> String {
    STATE.with_borrow(|s| s.map.iter().map(|(k, cc)| format!("{k}:{cc}")).collect::<Vec<_>>().join(","))
}

fn parse(text: &str) -> BTreeMap<String, u8> {
    text.split(',')
        .filter_map(|pair| {
            let (k, cc) = pair.split_once(':')?;
            Some((k.trim().to_string(), cc.trim().parse::<u8>().ok().filter(|&c| c < 128)?))
        })
        .collect()
}

/// A controller moved. Returns true when it created a binding (to be saved).
pub fn feed(cc: u8, value: u8) -> bool {
    STATE.with_borrow_mut(|s| {
        if let Some(key) = s.armed.take() {
            // One controller drives one knob.
            s.map.retain(|_, c| *c != cc);
            s.map.insert(key, cc);
            return true;
        }
        let v = value.min(127) as f32 / 127.0;
        let keys: Vec<String> = s.map.iter().filter(|(_, c)| **c == cc).map(|(k, _)| k.clone()).collect();
        for k in keys {
            s.pending.insert(k, v);
        }
        false
    })
}

/// Runs `f` with `key` as the section of the knobs drawn in it.
pub fn in_section<R>(key: &'static str, f: impl FnOnce() -> R) -> R {
    let before = STATE.with_borrow_mut(|s| std::mem::replace(&mut s.section, key));
    let out = f();
    STATE.with_borrow_mut(|s| s.section = before);
    out
}

/// Full key of a knob in the current section.
pub fn full_key(knob: &str) -> String {
    STATE.with_borrow(|s| if s.section.is_empty() { knob.to_string() } else { format!("{}.{knob}", s.section) })
}

/// A value waiting for this knob, 0..1.
pub fn take(key: &str) -> Option<f32> {
    STATE.with_borrow_mut(|s| s.pending.remove(key))
}

pub fn binding(key: &str) -> Option<u8> {
    STATE.with_borrow(|s| s.map.get(key).copied())
}

pub fn arm(key: &str) {
    STATE.with_borrow_mut(|s| s.armed = Some(key.to_string()));
}

pub fn is_armed(key: &str) -> bool {
    STATE.with_borrow(|s| s.armed.as_deref() == Some(key))
}

/// Stops waiting for a controller.
pub fn disarm() {
    STATE.with_borrow_mut(|s| s.armed = None);
}

/// Returns true if a binding was removed (to be saved).
pub fn forget(key: &str) -> bool {
    STATE.with_borrow_mut(|s| s.map.remove(key).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learn_bind_and_feed() {
        enable(Some("filter.cutoff:74, broken, amp_env.attack:300,lfo.rate:21"));
        assert_eq!(binding("filter.cutoff"), Some(74));
        assert_eq!(binding("amp_env.attack"), None, "out-of-range CC is dropped");
        // Learning takes the next controller and moves it off any other knob.
        arm("reverb.mix");
        assert!(feed(21, 64));
        assert_eq!(binding("reverb.mix"), Some(21));
        assert_eq!(binding("lfo.rate"), None);
        // A bound controller leaves a value for its knob.
        assert!(!feed(74, 127));
        assert_eq!(take("filter.cutoff"), Some(1.0));
        assert_eq!(take("filter.cutoff"), None);
        assert_eq!(save(), "filter.cutoff:74,reverb.mix:21");
        assert!(forget("reverb.mix"));
        assert_eq!(save(), "filter.cutoff:74");
    }

    #[test]
    fn a_bound_knob_takes_the_controller_value() {
        enable(Some("filter.cutoff:74"));
        feed(74, 127);
        let ctx = egui::Context::default();
        let mut cutoff = 800.0f32;
        let mut changed = false;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                changed = in_section("filter", || {
                    crate::knob::Knob::keyed("cutoff", "Cutoff", &mut cutoff, 20.0, 20_000.0).log().show(ui)
                });
            });
        });
        assert!(changed);
        assert!((cutoff - 20_000.0).abs() < 0.5, "{cutoff}");
        // Halfway on a log knob is the geometric middle.
        feed(74, 64);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                in_section("filter", || crate::knob::Knob::keyed("cutoff", "Cutoff", &mut cutoff, 20.0, 20_000.0).log().show(ui));
            });
        });
        let expect = 20.0 * 1000f32.powf(64.0 / 127.0);
        assert!((cutoff / expect - 1.0).abs() < 0.01, "{cutoff} vs {expect}");
    }

    #[test]
    fn keys_follow_sections() {
        assert_eq!(full_key("cutoff"), "cutoff");
        in_section("filter", || assert_eq!(full_key("cutoff"), "filter.cutoff"));
        in_section("amp_env", || in_section("inner", || assert_eq!(full_key("attack"), "inner.attack")));
        assert_eq!(full_key("attack"), "attack");
    }
}
