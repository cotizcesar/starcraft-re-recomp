//! Audio-layer checks: the bundled retail WAV effects decode, resample to the
//! mixer rate and are exposed by name.

use galaxycraft::audio::{AudioPlayer, SAMPLE_RATE, SOUND_NAMES, SoundBank, Wav};
use galaxycraft::render::asset_root;
use std::fs;

#[test]
fn bundled_sound_effects_decode_to_the_mixer_rate() {
    let dir = asset_root().join("sound");
    for name in SOUND_NAMES {
        let path = dir.join(format!("{name}.wav"));
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(&bytes[0..4], b"RIFF", "{name} is a RIFF file");
        let wav = Wav::parse(&bytes).unwrap_or_else(|| panic!("{name} decodes"));
        assert_eq!(wav.rate, SAMPLE_RATE, "{name} is 44.1 kHz");
        assert!((1..=2).contains(&wav.channels), "{name} channels");
        assert!(wav.seconds() > 0.05, "{name} has audible length");
        let mono = wav.to_mono_44k();
        assert!(!mono.is_empty(), "{name} has samples");
        assert!(
            mono.iter().any(|&s| s.abs() > 128),
            "{name} is not digital silence"
        );
        let expected = (wav.seconds() * SAMPLE_RATE as f32) as i64;
        assert!(
            (mono.len() as i64 - expected).abs() <= 2,
            "{name} resampled to {} frames, expected {expected}",
            mono.len()
        );
    }
}

#[test]
fn sound_bank_exposes_every_bundled_effect_by_name() {
    let bank = SoundBank::load(&asset_root().join("sound"));
    assert_eq!(bank.len(), SOUND_NAMES.len());
    for name in SOUND_NAMES {
        assert!(bank.has(name), "missing effect {name}");
        assert!(!bank.get(name).unwrap().is_empty());
    }
    assert!(bank.race_yes(galaxycraft::sim::Race::Terran).is_some());
    assert!(bank.race_yes(galaxycraft::sim::Race::Zerg).is_some());
    assert!(bank.race_yes(galaxycraft::sim::Race::Protoss).is_some());
}

#[test]
fn audio_player_degrades_to_a_no_op_without_a_device() {
    let player = AudioPlayer::new(false);
    assert!(!player.enabled());
    // Playing through a disabled device must be harmless.
    player.play(Some(&[0i16, 1, -1]));
    player.play(None);
}
