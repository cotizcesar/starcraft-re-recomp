//! Streaming playback of the retail WebM cinematics.
//!
//! The retail client opens with the Blizzard / StarCraft intro movie
//! (`smk/starintr.webm`, 138 s, VP9 + audio) before the main menu.  Decoding
//! that file up front is not an option — 1920x1080 RGB would be ~8 MB per
//! frame — so playback streams it: a worker thread decodes a bounded number
//! of frames at a time with the system `ffmpeg`, the player consumes them on
//! its own clock and asks for the next chunk when the buffer runs low.
//!
//! Determinism matters only for gameplay, not for a movie, so the clock is
//! wall time.  Everything else in the client keeps using the LCG sim.
//!
//! Without `ffmpeg` (or without a retail install) [`Cinematic::open`] returns
//! `None` and the caller keeps its static poster; the client never hard-fails
//! because a decoder is missing.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::video::{PosterFrame, VideoInfo, decode_frame_at_path, decode_frames_from_info};

/// Frames decoded per ffmpeg run.  Two seconds at 12 fps is ~18 MB of RGB at
/// 640x360: small enough to buffer, large enough that seeking is amortised.
pub const CHUNK_SECONDS: f32 = 2.0;

/// Playback state of the streaming player.
pub struct Cinematic {
    /// Frames decoded but not consumed yet, in playback order.
    buffer: VecDeque<PosterFrame>,
    /// Frames popped from `buffer` so far; `current` is the last of them.
    decoded: u64,
    /// Playback clock in frames.
    clock: f32,
    rx: Receiver<Vec<PosterFrame>>,
    cancel: Arc<AtomicBool>,
    duration: f32,
    fps: f32,
    width: u32,
    path: PathBuf,
    /// True once the worker will not send anything else.
    source_done: bool,
    /// Frame on screen, so an underrun holds the picture instead of flashing
    /// black while the decoder catches up.
    current: Option<PosterFrame>,
    /// Synchronous first frame, shown until the stream buffer takes over.
    preroll: Option<PosterFrame>,
}

impl Cinematic {
    /// Start streaming `path`.  `fps` is the playback rate requested from the
    /// decoder; `width` caps the decoded frame width (the client upscales).
    pub fn open(path: &Path, info: &VideoInfo, fps: f32, width: u32) -> Option<Self> {
        Self::open_at(path, info, fps, width, 0.0)
    }

    /// Start streaming at `start_at` seconds.
    ///
    /// The first frame is decoded synchronously so the very first drawn frame
    /// is real video (this is what makes `--shot-screen intro` deterministic);
    /// the worker then continues from there.
    pub fn open_at(
        path: &Path,
        info: &VideoInfo,
        fps: f32,
        width: u32,
        start_at: f32,
    ) -> Option<Self> {
        let duration = info.duration_ms.map(|ms| ms as f32 / 1000.0)?;
        if duration <= 0.0 || info.pixel_width == 0 || !path.exists() {
            return None;
        }
        let fps = fps.clamp(1.0, 60.0);
        let width = width.clamp(160, info.pixel_width);
        let start_at = start_at.clamp(0.0, (duration - 0.05).max(0.0));
        let first = decode_frame_at_path(path, info, start_at, width)?;
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let worker_path = path.to_path_buf();
        let worker_info = info.clone();
        let chunk = (CHUNK_SECONDS * fps).round().max(1.0) as usize;
        std::thread::Builder::new()
            .name("cinematic-decode".into())
            .spawn(move || {
                let mut start = start_at;
                while start < duration {
                    if worker_cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let frames = decode_frames_from_info(
                        &worker_path,
                        &worker_info,
                        start,
                        chunk,
                        fps,
                        width,
                    );
                    if frames.is_empty() {
                        return;
                    }
                    if tx.send(frames).is_err() {
                        return;
                    }
                    start += chunk as f32 / fps;
                }
            })
            .ok()?;
        Some(Self {
            buffer: VecDeque::new(),
            decoded: 0,
            clock: start_at * fps,
            rx,
            cancel,
            duration,
            fps,
            width,
            path: path.to_path_buf(),
            source_done: false,
            // None: `advance` consumes the buffered chunk, which starts at
            // `start_at`; `preroll` keeps the synchronous first frame until
            // the buffer takes over.
            current: None,
            preroll: Some(first),
        })
    }

    /// Player over an explicit frame list (used by tests and by callers that
    /// already hold decoded frames, e.g. the briefing interstitials).
    pub fn from_frames(frames: Vec<PosterFrame>, fps: f32) -> Self {
        let fps = fps.max(1.0);
        let duration = frames.len() as f32 / fps;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = tx.send(frames.clone());
        drop(tx);
        Self {
            buffer: frames.into_iter().collect(),
            decoded: 0,
            clock: 0.0,
            rx,
            cancel: Arc::new(AtomicBool::new(false)),
            duration,
            fps,
            width: 0,
            path: PathBuf::new(),
            source_done: false,
            current: None,
            preroll: None,
        }
    }

    /// Total length in seconds.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Playback position in seconds.
    pub fn position(&self) -> f32 {
        self.clock / self.fps
    }

    /// Fraction played, 0.0..=1.0.
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0 {
            1.0
        } else {
            (self.position() / self.duration).clamp(0.0, 1.0)
        }
    }

    /// True when the clock has run past the end of the movie.
    pub fn finished(&self) -> bool {
        self.position() >= self.duration
    }

    /// Advance the clock by `dt` seconds and return the frame to show.
    ///
    /// Frame `n` is on screen during `[n / fps, (n + 1) / fps)`.  On an
    /// underrun this repeats the last frame (and returns `None` only before
    /// the first frame has arrived).
    pub fn advance(&mut self, dt: f32) -> Option<PosterFrame> {
        if dt > 0.0 {
            self.clock += dt * self.fps;
        }
        self.fill();
        if self.current.is_none()
            && let Some(f) = self.preroll.take()
        {
            self.current = Some(f);
        }
        let want = self.clock.floor().max(0.0) as u64 + 1;
        while self.decoded < want {
            match self.buffer.pop_front() {
                Some(f) => {
                    self.current = Some(f);
                    self.decoded += 1;
                }
                None => break,
            }
        }
        self.fill();
        self.current.clone()
    }

    /// Pull every chunk the worker has ready, without blocking.
    fn fill(&mut self) {
        if self.source_done {
            return;
        }
        loop {
            match self.rx.try_recv() {
                Ok(frames) => self.buffer.extend(frames),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.source_done = true;
                    return;
                }
            }
        }
    }

    /// Path the player streams (empty for the explicit-frame constructor).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Decoded frame width cap.
    pub fn width(&self) -> u32 {
        self.width
    }
}

impl Drop for Cinematic {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(tag: u8) -> PosterFrame {
        PosterFrame {
            width: 2,
            height: 1,
            rgb: vec![tag; 6],
        }
    }

    #[test]
    fn plays_every_frame_once_in_order() {
        let frames: Vec<PosterFrame> = (0..10).map(frame).collect();
        let mut c = Cinematic::from_frames(frames, 10.0);
        assert_eq!(c.duration(), 1.0);
        let mut seen = vec![c.advance(0.0).expect("first frame").rgb[0]];
        for _ in 0..9 {
            seen.push(c.advance(0.1).expect("frame").rgb[0]);
        }
        assert_eq!(seen, (0..10).collect::<Vec<u8>>());
        assert!((c.progress() - 0.9).abs() < 1e-5, "{}", c.progress());
        // The last tick reaches the end.
        let _ = c.advance(0.1);
        assert!(c.finished());
        assert_eq!(c.progress(), 1.0);
    }

    #[test]
    fn a_large_stall_consumes_what_is_buffered_and_ends() {
        let frames: Vec<PosterFrame> = (0..6).map(frame).collect();
        let mut c = Cinematic::from_frames(frames, 6.0);
        let f = c.advance(10.0).expect("frame");
        assert_eq!(f.rgb[0], 5);
        assert!(c.finished());
    }

    #[test]
    fn holds_the_last_frame_on_underrun() {
        let mut c = Cinematic::from_frames(vec![frame(7)], 10.0);
        assert_eq!(c.advance(0.05).expect("first").rgb[0], 7);
        // Nothing buffered: repeat frame 7 instead of going black, and the
        // movie reports itself finished.
        assert_eq!(c.advance(0.05).expect("held").rgb[0], 7);
        assert!(c.finished());
    }

    #[test]
    fn open_rejects_a_video_without_a_duration() {
        let info = VideoInfo {
            codec: "V_VP9".into(),
            pixel_width: 1920,
            pixel_height: 1080,
            display_width: 1920,
            display_height: 1080,
            duration_ms: None,
            timecode_scale: 1_000_000,
            has_audio: true,
            video_tracks: 1,
            audio_tracks: 1,
            clusters: 1,
            bytes: 0,
        };
        assert!(Cinematic::open(Path::new("/nonexistent.webm"), &info, 12.0, 640).is_none());
    }
}
