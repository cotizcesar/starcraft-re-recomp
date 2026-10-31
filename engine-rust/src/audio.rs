//! Retail sound effects and a small software mixer.
//!
//! The shipped `sound\*.wav` files are ordinary 44.1 kHz 16-bit PCM.  The
//! bank in `assets/sound` is extracted byte-for-byte from CASC; the player
//! mixes overlapping effects into one mono stream and hands it to ALSA.  When
//! no audio device exists (headless runs, CI) the player degrades to a no-op
//! so gameplay is unaffected.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};

pub const SAMPLE_RATE: u32 = 44_100;

/// One decoded PCM WAV file.  `samples` is interleaved at the source rate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wav {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl Wav {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return None;
        }
        let mut fmt: Option<(u16, u16, u32, u16)> = None;
        let mut data: Option<&[u8]> = None;
        let mut pos = 12usize;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
            let start = pos + 8;
            let end = start.checked_add(size)?;
            if end > bytes.len() {
                break;
            }
            match id {
                b"fmt " if size >= 16 => {
                    let tag = u16::from_le_bytes([bytes[start], bytes[start + 1]]);
                    let channels = u16::from_le_bytes([bytes[start + 2], bytes[start + 3]]);
                    let rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().ok()?);
                    let bits = u16::from_le_bytes([bytes[start + 14], bytes[start + 15]]);
                    fmt = Some((tag, channels, rate, bits));
                }
                b"data" => data = Some(&bytes[start..end]),
                _ => {}
            }
            pos = end + (size & 1);
        }
        let (tag, channels, rate, bits) = fmt?;
        // 1 = PCM; 3 = IEEE float is not used by the shipped effects.
        if tag != 1 || channels == 0 || rate == 0 {
            return None;
        }
        let raw = data?;
        let mut samples = Vec::new();
        match bits {
            8 => {
                samples.extend(raw.iter().map(|&b| (b as i16 - 128) * 256));
            }
            16 => {
                for pair in raw.as_chunks::<2>().0 {
                    samples.push(i16::from_le_bytes([pair[0], pair[1]]));
                }
            }
            24 => {
                for triple in raw.as_chunks::<3>().0 {
                    let v =
                        ((triple[2] as i32) << 16) | ((triple[1] as i32) << 8) | triple[0] as i32;
                    samples.push((v >> 8) as i16);
                }
            }
            32 => {
                for quad in raw.as_chunks::<4>().0 {
                    samples.push(i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]) as i16);
                }
            }
            _ => return None,
        }
        Some(Self {
            rate,
            channels,
            samples,
        })
    }

    /// Duration in seconds at the source rate.
    pub fn seconds(&self) -> f32 {
        let frames = self.samples.len() as f32 / self.channels.max(1) as f32;
        frames / self.rate.max(1) as f32
    }

    /// Resample to the mixer's mono 44.1 kHz stream.
    pub fn to_mono_44k(&self) -> Vec<i16> {
        let ch = self.channels.max(1) as usize;
        let frames = self.samples.len() / ch;
        if frames == 0 {
            return Vec::new();
        }
        let mono: Vec<i16> = (0..frames)
            .map(|f| {
                let sum: i32 = (0..ch).map(|c| self.samples[f * ch + c] as i32).sum();
                (sum / ch as i32) as i16
            })
            .collect();
        if self.rate == SAMPLE_RATE {
            return mono;
        }
        let ratio = SAMPLE_RATE as f64 / self.rate as f64;
        let out_len = ((frames as f64) * ratio).round() as usize;
        let mut out = Vec::with_capacity(out_len);
        for i in 0..out_len {
            let src = i as f64 / ratio;
            let a = src.floor() as usize;
            let t = (src - a as f64) as f32;
            let s0 = mono[a.min(frames - 1)] as f32;
            let s1 = mono[(a + 1).min(frames - 1)] as f32;
            out.push((s0 + (s1 - s0) * t).round().clamp(-32768.0, 32767.0) as i16);
        }
        out
    }
}

/// The bundled effects.  Names match `assets/sound/<name>.wav`.
pub const SOUND_NAMES: &[&str] = &[
    "ui_click",
    "ui_move",
    "ui_swish",
    "transmission",
    "terran_yes",
    "zerg_yes",
    "protoss_yes",
    "victory",
    "defeat",
];

/// Decoded effect bank.  Missing files are simply absent, which keeps the
/// client usable with a partial asset directory.
#[derive(Default)]
pub struct SoundBank {
    sounds: HashMap<String, Vec<i16>>,
}

impl SoundBank {
    pub fn load(dir: &Path) -> Self {
        let mut sounds = HashMap::new();
        for name in SOUND_NAMES {
            let path = dir.join(format!("{name}.wav"));
            if let Ok(bytes) = std::fs::read(&path) {
                if let Some(wav) = Wav::parse(&bytes) {
                    sounds.insert((*name).to_string(), wav.to_mono_44k());
                }
            }
        }
        Self { sounds }
    }

    pub fn len(&self) -> usize {
        self.sounds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }

    pub fn has(&self, name: &str) -> bool {
        self.sounds.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&[i16]> {
        self.sounds.get(name).map(|v| v.as_slice())
    }

    /// Race acknowledgement used when a unit accepts an order.
    pub fn race_yes(&self, race: crate::sim::Race) -> Option<&[i16]> {
        let name = match race {
            crate::sim::Race::Zerg => "zerg_yes",
            crate::sim::Race::Protoss => "protoss_yes",
            _ => "terran_yes",
        };
        self.get(name)
    }
}

/// Decode an Ogg Vorbis track (the Remastered `music/*.ogg` set) into the
/// mixer's mono 44.1 kHz stream.
///
/// Each Ogg page is fed through `lewton`'s Vorbis decoder; the returned
/// samples are interleaved at the source rate and then resampled exactly like
/// a WAV clip.
pub fn decode_ogg(bytes: &[u8]) -> Option<Vec<i16>> {
    use std::io::Cursor;
    let mut reader = lewton::inside_ogg::OggStreamReader::new(Cursor::new(bytes.to_vec())).ok()?;
    let channels = reader.ident_hdr.audio_channels.max(1) as usize;
    let rate = reader.ident_hdr.audio_sample_rate;
    if rate == 0 {
        return None;
    }
    let mut samples: Vec<i16> = Vec::new();
    loop {
        match reader.read_dec_packet_itl() {
            Ok(Some(packet)) => samples.extend_from_slice(&packet),
            Ok(None) => break,
            Err(_) => return None,
        }
    }
    if samples.is_empty() {
        return None;
    }
    let frames = samples.len() / channels;
    let mono: Vec<i16> = (0..frames)
        .map(|f| {
            let sum: i32 = (0..channels)
                .map(|c| samples[f * channels + c] as i32)
                .sum();
            (sum / channels as i32) as i16
        })
        .collect();
    if rate == SAMPLE_RATE {
        return Some(mono);
    }
    let ratio = SAMPLE_RATE as f64 / rate as f64;
    let out_len = ((frames as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let a = src.floor() as usize;
        let t = (src - a as f64) as f32;
        let s0 = mono[a.min(frames - 1)] as f32;
        let s1 = mono[(a + 1).min(frames - 1)] as f32;
        out.push((s0 + (s1 - s0) * t).round().clamp(-32768.0, 32767.0) as i16);
    }
    Some(out)
}

/// Lazily decoded retail music.  `root` is the materialised `music/` folder.
pub struct MusicBank {
    root: std::path::PathBuf,
    cache: std::cell::RefCell<HashMap<String, Option<std::sync::Arc<Vec<i16>>>>>,
}

impl MusicBank {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn track(&self, name: &str) -> Option<std::sync::Arc<Vec<i16>>> {
        if let Some(hit) = self.cache.borrow().get(name) {
            return hit.clone();
        }
        let path = self.root.join(format!("{name}.ogg"));
        let decoded = std::fs::read(path)
            .ok()
            .and_then(|bytes| decode_ogg(&bytes))
            .filter(|samples| !samples.is_empty())
            .map(std::sync::Arc::new);
        self.cache
            .borrow_mut()
            .insert(name.to_string(), decoded.clone());
        decoded
    }

    pub fn has(&self, name: &str) -> bool {
        self.root.join(format!("{name}.ogg")).is_file()
    }

    pub fn names(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension().and_then(|s| s.to_str()) == Some("ogg"))
                    .then(|| p.file_stem()?.to_str().map(str::to_string))?
            })
            .collect();
        out.sort();
        out
    }
}

enum AudioMessage {
    Effect(Vec<i16>),
    Music(std::sync::Arc<Vec<i16>>),
    StopMusic,
}

/// Handle used by the client.  `play` is cheap and never blocks the game
/// thread; mixing happens on the audio thread.
pub struct AudioPlayer {
    tx: Option<Sender<AudioMessage>>,
}

impl AudioPlayer {
    pub fn new(enabled: bool) -> Self {
        if !enabled {
            return Self { tx: None };
        }
        let (tx, rx) = mpsc::channel::<AudioMessage>();
        match std::thread::Builder::new()
            .name("galaxycraft-audio".into())
            .spawn(move || mixer_thread(rx))
        {
            Ok(_) => Self { tx: Some(tx) },
            Err(_) => Self { tx: None },
        }
    }

    pub fn enabled(&self) -> bool {
        self.tx.is_some()
    }

    pub fn play(&self, samples: Option<&[i16]>) {
        if let (Some(tx), Some(samples)) = (&self.tx, samples) {
            let _ = tx.send(AudioMessage::Effect(samples.to_vec()));
        }
    }

    pub fn play_named(&self, bank: &SoundBank, name: &str) {
        self.play(bank.get(name));
    }

    pub fn play_music(&self, track: Option<std::sync::Arc<Vec<i16>>>) {
        if let (Some(tx), Some(track)) = (&self.tx, track) {
            let _ = tx.send(AudioMessage::Music(track));
        }
    }

    pub fn stop_music(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(AudioMessage::StopMusic);
        }
    }
}

/// ALSA entry points loaded dynamically so the binary starts on systems
/// without the library (and so tests never need an audio device).
type PcmOpen = unsafe extern "C" fn(*mut *mut c_void, *const i8, i32, i32) -> i32;
type PcmSetParams = unsafe extern "C" fn(*mut c_void, i32, i32, u32, u32, i32, u32) -> i32;
type PcmWritei = unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> isize;
type PcmRecover = unsafe extern "C" fn(*mut c_void, i32, i32) -> i32;
type PcmDrain = unsafe extern "C" fn(*mut c_void) -> i32;
type PcmClose = unsafe extern "C" fn(*mut c_void) -> i32;

struct Alsa {
    _lib: libloading::Library,
    open: PcmOpen,
    set_params: PcmSetParams,
    writei: PcmWritei,
    recover: PcmRecover,
    drain: PcmDrain,
    close: PcmClose,
}

impl Alsa {
    fn new() -> Option<Self> {
        unsafe {
            let lib = libloading::Library::new("libasound.so.2").ok()?;
            let open = *lib.get::<PcmOpen>(b"snd_pcm_open\0").ok()?;
            let set_params = *lib.get::<PcmSetParams>(b"snd_pcm_set_params\0").ok()?;
            let writei = *lib.get::<PcmWritei>(b"snd_pcm_writei\0").ok()?;
            let recover = *lib.get::<PcmRecover>(b"snd_pcm_recover\0").ok()?;
            let drain = *lib.get::<PcmDrain>(b"snd_pcm_drain\0").ok()?;
            let close = *lib.get::<PcmClose>(b"snd_pcm_close\0").ok()?;
            Some(Self {
                _lib: lib,
                open,
                set_params,
                writei,
                recover,
                drain,
                close,
            })
        }
    }
}

fn mixer_thread(rx: Receiver<AudioMessage>) {
    let Some(alsa) = Alsa::new() else {
        // Keep draining so `play` never fails; the game just runs silent.
        while rx.recv().is_ok() {}
        return;
    };
    unsafe {
        let mut pcm: *mut c_void = std::ptr::null_mut();
        let name = b"default\0";
        if (alsa.open)(&mut pcm, name.as_ptr() as *const i8, 0, 0) != 0 || pcm.is_null() {
            while rx.recv().is_ok() {}
            return;
        }
        // S16_LE = 2, RW_INTERLEAVED = 3, mono, 200 ms latency.
        let rc = (alsa.set_params)(pcm, 2, 3, 1, SAMPLE_RATE, 1, 200_000);
        if rc != 0 {
            (alsa.close)(pcm);
            while rx.recv().is_ok() {}
            return;
        }
        let mut active: Vec<(Vec<i16>, usize)> = Vec::new();
        let mut music: Option<(std::sync::Arc<Vec<i16>>, usize)> = None;
        let mut buf = vec![0i16; 1024];
        loop {
            // Drain every queued message before mixing the next block.
            loop {
                match rx.try_recv() {
                    Ok(AudioMessage::Effect(samples)) if !samples.is_empty() => {
                        if active.len() < 8 {
                            active.push((samples, 0));
                        }
                    }
                    Ok(AudioMessage::Effect(_)) => {}
                    Ok(AudioMessage::Music(track)) => {
                        music = (!track.is_empty()).then_some((track, 0));
                    }
                    Ok(AudioMessage::StopMusic) => music = None,
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        (alsa.drain)(pcm);
                        (alsa.close)(pcm);
                        return;
                    }
                }
            }
            if active.is_empty() && music.is_none() {
                match rx.recv() {
                    Ok(AudioMessage::Effect(samples)) if !samples.is_empty() => {
                        active.push((samples, 0));
                    }
                    Ok(AudioMessage::Music(track)) if !track.is_empty() => {
                        music = Some((track, 0));
                    }
                    Ok(_) => continue,
                    Err(_) => {
                        (alsa.drain)(pcm);
                        (alsa.close)(pcm);
                        return;
                    }
                }
            }
            for slot in buf.iter_mut() {
                *slot = 0;
            }
            for (samples, pos) in active.iter_mut() {
                for (i, dst) in buf.iter_mut().enumerate() {
                    if *pos + i < samples.len() {
                        *dst = (*dst as i32 + samples[*pos + i] as i32).clamp(-32768, 32767) as i16;
                    }
                }
                *pos += buf.len();
            }
            active.retain(|(samples, pos)| *pos < samples.len());
            // Music is looped at a lower level so effects remain audible.
            if let Some((samples, pos)) = music.as_mut() {
                for (i, dst) in buf.iter_mut().enumerate() {
                    let src = (*pos + i) % samples.len();
                    let mixed = *dst as i32 + (samples[src] as i32 * 3) / 5;
                    *dst = mixed.clamp(-32768, 32767) as i16;
                }
                *pos = (*pos + buf.len()) % samples.len();
            }
            let mut written = 0usize;
            while written < buf.len() {
                let rc = (alsa.writei)(
                    pcm,
                    buf[written..].as_ptr() as *const c_void,
                    buf.len() - written,
                );
                if rc < 0 {
                    if (alsa.recover)(pcm, rc as i32, 1) < 0 {
                        (alsa.close)(pcm);
                        return;
                    }
                } else if rc == 0 {
                    break;
                } else {
                    written += rc as usize;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tiny_mono_wav_decodes() {
        // 4 frames, 16-bit mono, 44.1 kHz.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36u32 + 8).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44_100u32.to_le_bytes());
        bytes.extend_from_slice(&88_200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());
        for s in [0i16, 1000, -1000, 32767] {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        let wav = Wav::parse(&bytes).expect("valid wav");
        assert_eq!(wav.rate, 44_100);
        assert_eq!(wav.channels, 1);
        assert_eq!(wav.samples, vec![0, 1000, -1000, 32767]);
        assert_eq!(wav.to_mono_44k(), wav.samples);
    }

    #[test]
    fn vorbis_music_decodes_to_the_mixer_rate() {
        let bytes = include_bytes!("../fixtures/audio/tone.ogg");
        let samples = decode_ogg(bytes).expect("vorbis decodes");
        // One second at 44.1 kHz, within the decoder's padding tolerance.
        assert!(
            (40_000..=48_000).contains(&samples.len()),
            "{} samples",
            samples.len()
        );
        assert!(samples.iter().any(|s| s.abs() > 1000), "silent decode");
    }

    #[test]
    fn music_bank_decodes_lazily_and_caches() {
        let dir = std::env::temp_dir().join(format!("gc-music-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tone.ogg"),
            include_bytes!("../fixtures/audio/tone.ogg"),
        )
        .unwrap();
        let bank = MusicBank::new(&dir);
        assert!(bank.has("tone"));
        assert_eq!(bank.names(), vec!["tone".to_string()]);
        let first = bank.track("tone").expect("track");
        let second = bank.track("tone").expect("cached track");
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert!(!first.is_empty());
        assert!(bank.track("missing").is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
