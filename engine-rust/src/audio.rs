//! Retail sound effects and a small software mixer.
//!
//! The shipped `sound\*.wav` files are ordinary 44.1 kHz 16-bit PCM.  The
//! bank in `assets/sound` is extracted byte-for-byte from CASC; the player
//! mixes overlapping effects into one mono stream and hands it to the
//! platform output (ALSA, WinMM or CoreAudio, all loaded at run time).  When
//! no audio device exists (headless runs, CI) the player degrades to a no-op
//! so gameplay is unaffected.

use std::collections::HashMap;
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

/// A PCM output: blocking writes of mono `i16` blocks at [`SAMPLE_RATE`].
/// Every backend is loaded dynamically, so the binary starts (silent) on a
/// system without it, and tests never need an audio device.
trait PcmOut {
    /// Queue one block; `false` = the device failed and the mixer stops.
    fn write(&mut self, block: &[i16]) -> bool;
}

/// The platform's output: ALSA on Linux/BSD, WinMM on Windows, CoreAudio
/// (AudioQueue) on macOS.
fn open_output() -> Option<Box<dyn PcmOut>> {
    if cfg!(windows) {
        Some(Box::new(winmm::WaveOut::open()?))
    } else if cfg!(target_os = "macos") {
        Some(Box::new(coreaudio::Queue::open()?))
    } else {
        Some(Box::new(alsa::Pcm::open()?))
    }
}

/// ALSA (`libasound.so.2`).
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
mod alsa {
    use std::ffi::c_void;

    type PcmOpen = unsafe extern "C" fn(*mut *mut c_void, *const std::ffi::c_char, i32, i32) -> i32;
    type PcmSetParams = unsafe extern "C" fn(*mut c_void, i32, i32, u32, u32, i32, u32) -> i32;
    type PcmWritei = unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> isize;
    type PcmRecover = unsafe extern "C" fn(*mut c_void, i32, i32) -> i32;
    type PcmDrain = unsafe extern "C" fn(*mut c_void) -> i32;
    type PcmClose = unsafe extern "C" fn(*mut c_void) -> i32;

    pub struct Pcm {
        _lib: libloading::Library,
        pcm: *mut c_void,
        writei: PcmWritei,
        recover: PcmRecover,
        drain: PcmDrain,
        close: PcmClose,
    }

    impl Pcm {
        pub fn open() -> Option<Self> {
            unsafe {
                let lib = libloading::Library::new("libasound.so.2").ok()?;
                let open = *lib.get::<PcmOpen>(b"snd_pcm_open\0").ok()?;
                let set_params = *lib.get::<PcmSetParams>(b"snd_pcm_set_params\0").ok()?;
                let writei = *lib.get::<PcmWritei>(b"snd_pcm_writei\0").ok()?;
                let recover = *lib.get::<PcmRecover>(b"snd_pcm_recover\0").ok()?;
                let drain = *lib.get::<PcmDrain>(b"snd_pcm_drain\0").ok()?;
                let close = *lib.get::<PcmClose>(b"snd_pcm_close\0").ok()?;
                let mut pcm: *mut c_void = std::ptr::null_mut();
                if open(&mut pcm, c"default".as_ptr(), 0, 0) != 0 || pcm.is_null() {
                    return None;
                }
                // S16_LE = 2, RW_INTERLEAVED = 3, mono, 200 ms latency.
                if set_params(pcm, 2, 3, 1, super::SAMPLE_RATE, 1, 200_000) != 0 {
                    close(pcm);
                    return None;
                }
                Some(Self {
                    _lib: lib,
                    pcm,
                    writei,
                    recover,
                    drain,
                    close,
                })
            }
        }
    }

    impl super::PcmOut for Pcm {
        fn write(&mut self, block: &[i16]) -> bool {
            let mut written = 0usize;
            while written < block.len() {
                let rc = unsafe {
                    (self.writei)(
                        self.pcm,
                        block[written..].as_ptr() as *const c_void,
                        block.len() - written,
                    )
                };
                if rc < 0 {
                    if unsafe { (self.recover)(self.pcm, rc as i32, 1) } < 0 {
                        return false;
                    }
                } else if rc == 0 {
                    break;
                } else {
                    written += rc as usize;
                }
            }
            true
        }
    }

    impl Drop for Pcm {
        fn drop(&mut self) {
            unsafe {
                (self.drain)(self.pcm);
                (self.close)(self.pcm);
            }
        }
    }
}

/// Windows waveOut (`winmm.dll`): a ring of headers, each written when the
/// device has finished with it (`WHDR_DONE`).
#[cfg_attr(not(windows), allow(dead_code))]
mod winmm {
    use std::ffi::c_void;

    #[repr(C)]
    struct WaveFormatEx {
        format_tag: u16,
        channels: u16,
        samples_per_sec: u32,
        avg_bytes_per_sec: u32,
        block_align: u16,
        bits_per_sample: u16,
        cb_size: u16,
    }

    #[repr(C)]
    pub struct WaveHdr {
        data: *mut i8,
        buffer_length: u32,
        bytes_recorded: u32,
        user: usize,
        flags: u32,
        loops: u32,
        next: *mut WaveHdr,
        reserved: usize,
    }

    const WAVE_MAPPER: u32 = u32::MAX;
    const WHDR_DONE: u32 = 1;
    const WHDR_PREPARED: u32 = 2;
    const BUFFERS: usize = 4;

    type Open = unsafe extern "system" fn(
        *mut *mut c_void,
        u32,
        *const WaveFormatEx,
        usize,
        usize,
        u32,
    ) -> u32;
    type Hdr = unsafe extern "system" fn(*mut c_void, *mut WaveHdr, u32) -> u32;
    type Handle = unsafe extern "system" fn(*mut c_void) -> u32;

    pub struct WaveOut {
        _lib: libloading::Library,
        dev: *mut c_void,
        prepare: Hdr,
        unprepare: Hdr,
        write: Hdr,
        reset: Handle,
        close: Handle,
        /// Created once and never resized: the device holds pointers to
        /// these headers while it plays them.
        headers: Vec<WaveHdr>,
        data: Vec<Vec<i16>>,
        next: usize,
    }

    impl WaveOut {
        pub fn open() -> Option<Self> {
            unsafe {
                let lib = libloading::Library::new("winmm.dll").ok()?;
                let open = *lib.get::<Open>(b"waveOutOpen\0").ok()?;
                let prepare = *lib.get::<Hdr>(b"waveOutPrepareHeader\0").ok()?;
                let unprepare = *lib.get::<Hdr>(b"waveOutUnprepareHeader\0").ok()?;
                let write = *lib.get::<Hdr>(b"waveOutWrite\0").ok()?;
                let reset = *lib.get::<Handle>(b"waveOutReset\0").ok()?;
                let close = *lib.get::<Handle>(b"waveOutClose\0").ok()?;
                let fmt = WaveFormatEx {
                    format_tag: 1, // WAVE_FORMAT_PCM
                    channels: 1,
                    samples_per_sec: super::SAMPLE_RATE,
                    avg_bytes_per_sec: super::SAMPLE_RATE * 2,
                    block_align: 2,
                    bits_per_sample: 16,
                    cb_size: 0,
                };
                let mut dev: *mut c_void = std::ptr::null_mut();
                // CALLBACK_NULL: the mixer polls WHDR_DONE.
                if open(&mut dev, WAVE_MAPPER, &fmt, 0, 0, 0) != 0 || dev.is_null() {
                    return None;
                }
                let headers = (0..BUFFERS)
                    .map(|_| WaveHdr {
                        data: std::ptr::null_mut(),
                        buffer_length: 0,
                        bytes_recorded: 0,
                        user: 0,
                        flags: WHDR_DONE,
                        loops: 0,
                        next: std::ptr::null_mut(),
                        reserved: 0,
                    })
                    .collect();
                Some(Self {
                    _lib: lib,
                    dev,
                    prepare,
                    unprepare,
                    write,
                    reset,
                    close,
                    headers,
                    data: vec![Vec::new(); BUFFERS],
                    next: 0,
                })
            }
        }
    }

    impl super::PcmOut for WaveOut {
        fn write(&mut self, block: &[i16]) -> bool {
            let i = self.next;
            self.next = (i + 1) % BUFFERS;
            let size = std::mem::size_of::<WaveHdr>() as u32;
            // Wait until the device has played this slot (the ring is the
            // blocking point, like ALSA's writei).
            let hdr: *mut WaveHdr = &mut self.headers[i];
            while unsafe { std::ptr::read_volatile(&(*hdr).flags) } & WHDR_DONE == 0 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            unsafe {
                if (*hdr).flags & WHDR_PREPARED != 0 {
                    (self.unprepare)(self.dev, hdr, size);
                }
                self.data[i].clear();
                self.data[i].extend_from_slice(block);
                (*hdr).data = self.data[i].as_mut_ptr() as *mut i8;
                (*hdr).buffer_length = (block.len() * 2) as u32;
                (*hdr).flags = 0;
                (self.prepare)(self.dev, hdr, size) == 0 && (self.write)(self.dev, hdr, size) == 0
            }
        }
    }

    impl Drop for WaveOut {
        fn drop(&mut self) {
            let size = std::mem::size_of::<WaveHdr>() as u32;
            unsafe {
                (self.reset)(self.dev);
                for h in self.headers.iter_mut() {
                    if h.flags & WHDR_PREPARED != 0 {
                        (self.unprepare)(self.dev, h, size);
                    }
                }
                (self.close)(self.dev);
            }
        }
    }
}

/// macOS CoreAudio (`AudioToolbox` AudioQueue): buffers come back through
/// the queue's callback into a free list the mixer waits on.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod coreaudio {
    use std::ffi::c_void;
    use std::sync::{Arc, Mutex};

    #[repr(C)]
    struct StreamDescription {
        sample_rate: f64,
        format_id: u32,
        format_flags: u32,
        bytes_per_packet: u32,
        frames_per_packet: u32,
        bytes_per_frame: u32,
        channels_per_frame: u32,
        bits_per_channel: u32,
        reserved: u32,
    }

    #[repr(C)]
    pub struct QueueBuffer {
        capacity: u32,
        data: *mut c_void,
        byte_size: u32,
        user: *mut c_void,
        packet_capacity: u32,
        packets: *mut c_void,
        packet_count: u32,
    }

    type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut QueueBuffer);
    type NewOutput = unsafe extern "C" fn(
        *const StreamDescription,
        Callback,
        *mut c_void,
        *mut c_void,
        *mut c_void,
        u32,
        *mut *mut c_void,
    ) -> i32;
    type Allocate = unsafe extern "C" fn(*mut c_void, u32, *mut *mut QueueBuffer) -> i32;
    type Enqueue = unsafe extern "C" fn(*mut c_void, *mut QueueBuffer, u32, *const c_void) -> i32;
    type Start = unsafe extern "C" fn(*mut c_void, *const c_void) -> i32;
    type Dispose = unsafe extern "C" fn(*mut c_void, u8) -> i32;

    const LIB: &str = "/System/Library/Frameworks/AudioToolbox.framework/AudioToolbox";
    const LINEAR_PCM: u32 = u32::from_be_bytes(*b"lpcm");
    /// Signed integer | packed.
    const FLAGS: u32 = 4 | 8;
    const BUFFERS: usize = 4;
    const BLOCK_BYTES: u32 = 4096;

    /// Free buffers handed back by the queue thread.
    type FreeList = Mutex<Vec<usize>>;

    unsafe extern "C" fn done(user: *mut c_void, _queue: *mut c_void, buf: *mut QueueBuffer) {
        let free = &*(user as *const FreeList);
        if let Ok(mut v) = free.lock() {
            v.push(buf as usize);
        }
    }

    pub struct Queue {
        _lib: libloading::Library,
        queue: *mut c_void,
        enqueue: Enqueue,
        start: Start,
        dispose: Dispose,
        free: Arc<FreeList>,
        started: bool,
    }

    impl Queue {
        pub fn open() -> Option<Self> {
            unsafe {
                let lib = libloading::Library::new(LIB).ok()?;
                let new_output = *lib.get::<NewOutput>(b"AudioQueueNewOutput\0").ok()?;
                let allocate = *lib.get::<Allocate>(b"AudioQueueAllocateBuffer\0").ok()?;
                let enqueue = *lib.get::<Enqueue>(b"AudioQueueEnqueueBuffer\0").ok()?;
                let start = *lib.get::<Start>(b"AudioQueueStart\0").ok()?;
                let dispose = *lib.get::<Dispose>(b"AudioQueueDispose\0").ok()?;
                let fmt = StreamDescription {
                    sample_rate: super::SAMPLE_RATE as f64,
                    format_id: LINEAR_PCM,
                    format_flags: FLAGS,
                    bytes_per_packet: 2,
                    frames_per_packet: 1,
                    bytes_per_frame: 2,
                    channels_per_frame: 1,
                    bits_per_channel: 16,
                    reserved: 0,
                };
                let free: Arc<FreeList> = Arc::new(Mutex::new(Vec::new()));
                let mut queue: *mut c_void = std::ptr::null_mut();
                // No run loop: callbacks arrive on the queue's own thread.
                let user = Arc::as_ptr(&free) as *mut c_void;
                let null = std::ptr::null_mut();
                if new_output(&fmt, done, user, null, null, 0, &mut queue) != 0 || queue.is_null() {
                    return None;
                }
                for _ in 0..BUFFERS {
                    let mut buf: *mut QueueBuffer = std::ptr::null_mut();
                    if allocate(queue, BLOCK_BYTES, &mut buf) != 0 || buf.is_null() {
                        dispose(queue, 1);
                        return None;
                    }
                    free.lock().ok()?.push(buf as usize);
                }
                Some(Self {
                    _lib: lib,
                    queue,
                    enqueue,
                    start,
                    dispose,
                    free,
                    started: false,
                })
            }
        }
    }

    impl super::PcmOut for Queue {
        fn write(&mut self, block: &[i16]) -> bool {
            let buf = loop {
                if let Some(b) = self.free.lock().ok().and_then(|mut v| v.pop()) {
                    break b as *mut QueueBuffer;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            };
            unsafe {
                let bytes = (block.len() * 2).min((*buf).capacity as usize);
                std::ptr::copy_nonoverlapping(
                    block.as_ptr() as *const u8,
                    (*buf).data as *mut u8,
                    bytes,
                );
                (*buf).byte_size = bytes as u32;
                if (self.enqueue)(self.queue, buf, 0, std::ptr::null()) != 0 {
                    return false;
                }
                if !self.started {
                    self.started = (self.start)(self.queue, std::ptr::null()) == 0;
                }
            }
            true
        }
    }

    impl Drop for Queue {
        fn drop(&mut self) {
            // Immediate dispose stops the callbacks before `free` is dropped.
            unsafe {
                (self.dispose)(self.queue, 1);
            }
        }
    }
}

/// Mix every queued effect and the looping music into one mono stream.
fn mixer_thread(rx: Receiver<AudioMessage>) {
    let Some(mut out) = open_output() else {
        // Keep draining so `play` never fails; the game just runs silent.
        while rx.recv().is_ok() {}
        return;
    };
    let mut mixer = Mixer::default();
    let mut buf = vec![0i16; 1024];
    loop {
        // Drain every queued message before mixing the next block.
        loop {
            match rx.try_recv() {
                Ok(m) => mixer.apply(m),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        if mixer.is_idle() {
            match rx.recv() {
                Ok(m) => mixer.apply(m),
                Err(_) => return,
            }
            continue;
        }
        mixer.mix(&mut buf);
        if !out.write(&buf) {
            return;
        }
    }
}

/// Software mixer state: up to 8 effects plus one looping music track.
#[derive(Default)]
struct Mixer {
    active: Vec<(Vec<i16>, usize)>,
    music: Option<(std::sync::Arc<Vec<i16>>, usize)>,
}

impl Mixer {
    fn apply(&mut self, m: AudioMessage) {
        match m {
            AudioMessage::Effect(samples) => {
                if !samples.is_empty() && self.active.len() < 8 {
                    self.active.push((samples, 0));
                }
            }
            AudioMessage::Music(track) => {
                self.music = (!track.is_empty()).then_some((track, 0));
            }
            AudioMessage::StopMusic => self.music = None,
        }
    }

    fn is_idle(&self) -> bool {
        self.active.is_empty() && self.music.is_none()
    }

    fn mix(&mut self, buf: &mut [i16]) {
        buf.fill(0);
        for (samples, pos) in self.active.iter_mut() {
            for (i, dst) in buf.iter_mut().enumerate() {
                if *pos + i < samples.len() {
                    *dst = (*dst as i32 + samples[*pos + i] as i32).clamp(-32768, 32767) as i16;
                }
            }
            *pos += buf.len();
        }
        self.active.retain(|(samples, pos)| *pos < samples.len());
        // Music is looped at a lower level so effects remain audible.
        if let Some((samples, pos)) = self.music.as_mut() {
            for (i, dst) in buf.iter_mut().enumerate() {
                let src = (*pos + i) % samples.len();
                let mixed = *dst as i32 + (samples[src] as i32 * 3) / 5;
                *dst = mixed.clamp(-32768, 32767) as i16;
            }
            *pos = (*pos + buf.len()) % samples.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_sums_effects_clamps_and_loops_music_at_three_fifths() {
        let mut m = Mixer::default();
        assert!(m.is_idle());
        m.apply(AudioMessage::Effect(vec![30_000, 100, 7]));
        m.apply(AudioMessage::Effect(vec![30_000, -50]));
        m.apply(AudioMessage::Music(std::sync::Arc::new(vec![10, -10])));
        let mut buf = [0i16; 4];
        m.mix(&mut buf);
        // 30000 + 30000 clamps; music adds 10 * 3 / 5 = 6 and -6.
        assert_eq!(buf, [32_767, 44, 13, -6]);
        // Effects shorter than the block are done; the music keeps looping.
        m.mix(&mut buf);
        assert_eq!(buf, [6, -6, 6, -6]);
        m.apply(AudioMessage::StopMusic);
        assert!(m.is_idle());
        // At most eight overlapping effects.
        for _ in 0..10 {
            m.apply(AudioMessage::Effect(vec![1; 8]));
        }
        m.mix(&mut buf);
        assert_eq!(buf, [8; 4]);
    }

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
