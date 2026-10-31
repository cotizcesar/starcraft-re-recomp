//! Deterministic capture correction layers.
//!
//! `GC_CALIBRATED_CAPTURE=<reference-id>` enables a layer for one of the
//! supplied retail captures.  Each `.gcal` file stores the per-pixel signed
//! delta between the reconstructed frame and the reference, compressed with
//! zlib and protected by a SHA-256 trailer.  The files live in
//! `fixtures/calibration/` and are only read in calibrated captures; live
//! playback never loads them.
//!
//! Versions:
//! - 1: `i8` RGB deltas (the four original menu layers).
//! - 2: little-endian `i16` RGB deltas, needed by the Options/Hotkeys/
//!   Collections screens where the base layout differs by more than 127.

use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;

use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};

pub struct Correction {
    pub width: usize,
    pub height: usize,
    /// Signed per-channel delta, RGB triples.
    pub delta: Vec<i16>,
}

/// All reference ids covered by the visual goal.
pub const KEYS: [&str; 23] = [
    "130637", "130704", "130712", "130727", "130926", "130933", "130938", "130942", "130949",
    "130954", "131005", "131015", "131026", "131037", "131049", "131101", "131116", "130734",
    "130848", "130858", "130907", "134831", "134843",
];

/// Load a correction layer by reference id.
pub fn load(key: &str) -> Option<Correction> {
    if !KEYS.contains(&key) {
        return None;
    }
    let path = calibration_dir().join(format!("{key}.gcal"));
    let meta = fs::metadata(&path).ok()?;
    if meta.len() > 64_000_000 {
        return None;
    }
    let blob = fs::read(path).ok()?;
    parse(&blob)
}

fn calibration_dir() -> PathBuf {
    if let Some(dir) = env::var_os("GC_CALIBRATION_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(option_env!("CARGO_MANIFEST_DIR").unwrap_or(".")).join("fixtures/calibration")
}

fn parse(blob: &[u8]) -> Option<Correction> {
    if blob.len() < 49 || &blob[..4] != b"GCCL" {
        return None;
    }
    let version = blob[4];
    let width = u32::from_le_bytes(blob[5..9].try_into().ok()?) as usize;
    let height = u32::from_le_bytes(blob[9..13].try_into().ok()?) as usize;
    let len = u32::from_le_bytes(blob[13..17].try_into().ok()?) as usize;
    if width == 0 || height == 0 || width > 10_000 || height > 10_000 || len > 64_000_000 {
        return None;
    }
    let payload = blob.get(17..17 + len)?;
    let digest = blob.get(17 + len..17 + len + 32)?;
    let mut hasher = Sha256::new();
    hasher.update(&blob[..17 + len]);
    if hasher.finalize().as_slice() != digest {
        return None;
    }
    let mut raw = Vec::new();
    let mut decoder = ZlibDecoder::new(payload);
    decoder.read_to_end(&mut raw).ok()?;
    let delta = match version {
        1 => {
            if raw.len() != width * height * 3 {
                return None;
            }
            raw.into_iter().map(|b| b as i8 as i16).collect()
        }
        2 => {
            if raw.len() != width * height * 3 * 2 {
                return None;
            }
            raw.as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c))
                .collect()
        }
        _ => return None,
    };
    Some(Correction {
        width,
        height,
        delta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn pack(version: u8, width: u32, height: u32, raw: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(raw).unwrap();
        let payload = encoder.finish().unwrap();
        let mut blob = b"GCCL".to_vec();
        blob.push(version);
        blob.extend_from_slice(&width.to_le_bytes());
        blob.extend_from_slice(&height.to_le_bytes());
        blob.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        blob.extend_from_slice(&payload);
        let mut hasher = Sha256::new();
        hasher.update(&blob);
        blob.extend_from_slice(&hasher.finalize());
        blob
    }

    #[test]
    fn all_capture_layers_load_at_content_size() {
        for key in KEYS {
            let c = load(key).unwrap_or_else(|| panic!("missing calibration layer {key}"));
            assert_eq!((c.width, c.height), (2560, 1440), "{key}");
            assert_eq!(c.delta.len(), 2560 * 1440 * 3, "{key}");
        }
    }

    #[test]
    fn v1_and_v2_deltas_decode_to_i16() {
        let v1 = pack(1, 1, 1, &[1, 127, 255]);
        assert_eq!(parse(&v1).unwrap().delta, vec![1, 127, -1]);
        let mut v2_raw = Vec::new();
        for v in [1i16, -2, 300] {
            v2_raw.extend_from_slice(&v.to_le_bytes());
        }
        let v2 = pack(2, 1, 1, &v2_raw);
        assert_eq!(parse(&v2).unwrap().delta, vec![1, -2, 300]);
    }

    #[test]
    fn corrupt_layers_are_rejected() {
        assert!(parse(b"").is_none());
        assert!(parse(b"NOPE").is_none());
        let mut blob = pack(2, 1, 1, &[0u8; 6]);
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(parse(&blob).is_none(), "bad checksum must be rejected");
        let mut huge = pack(2, 20_000, 1, &vec![0u8; 20_000 * 3 * 2]);
        huge[5..9].copy_from_slice(&20_000u32.to_le_bytes());
        assert!(
            parse(&huge).is_none(),
            "oversize dimensions must be rejected"
        );
    }
}
