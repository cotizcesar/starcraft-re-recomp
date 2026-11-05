//! Reader for the retail StarCraft Remastered `.rep` container.
//!
//! The format was recovered from the shipped autosaves (see
//! `research/docs/session-log.md`, round of 2026-10-29):
//!
//! ```text
//! 0x00  u32           preamble
//! 0x04  u32           1
//! 0x08  u32           4
//! 0x0c  "seRS"        magic
//! 0x10  u32           version (0xa70d for 1.23.10)
//! 0x14  block*        framed sections
//! 0x??  chunk*        [u32 size][zlib] chunks of 8192 bytes (the CHK)
//! 0x??  block*        framed sections again (player list)
//! 0x??  tag*          named sections (SKIN, LMTS, BFIX, CCLR, GCFG)
//! ```
//!
//! A *framed* block is `[u32 checksum][u32 count][u32 size][zlib data]`.
//! The autosaves shipped with the game contain the header (players + map
//! name), the whole CHK (map) and the named settings sections, but **no
//! command stream**; a replay saved by the player through F10 → Save Replay
//! carries the actions as well, and this reader exposes them so the
//! simulation can be driven tick by tick.

use std::io::Read;

/// Magic that starts the replay payload.
pub const MAGIC: &[u8; 4] = b"seRS";

/// One player slot from the replay header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplaySlot {
    pub name: String,
    /// 0 = Zerg, 1 = Terran, 2 = Protoss, as stored in the header.
    pub race: u8,
}

/// A parsed retail replay.
#[derive(Clone, Debug, Default)]
pub struct RetailReplay {
    /// Container version word (0xa70d on 1.23.10.13515).
    pub version: u32,
    /// The map the game was played on.
    pub map_name: String,
    /// Every player slot, in order.
    pub slots: Vec<ReplaySlot>,
    /// The embedded CHK (the map itself).
    pub chk: Vec<u8>,
    /// Named trailing sections (`SKIN`, `LMTS`, `BFIX`, `CCLR`, `GCFG`…).
    pub sections: Vec<String>,
    /// Residual payload after the map: the command stream of a player-saved
    /// replay. Empty for the autosaves, which only carry map + settings.
    pub commands: Vec<u8>,
}

impl RetailReplay {
    /// Parse a `.rep` file. Returns `None` when the container is not a replay
    /// or is truncated.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0x18 || &bytes[0x0c..0x10] != MAGIC {
            return None;
        }
        let version = u32::from_le_bytes(bytes[0x10..0x14].try_into().ok()?);
        let mut out = Self {
            version,
            ..Self::default()
        };

        // Header block: players and the map name live here as C strings.
        let (header, mut off) = read_block(bytes, 0x14)?;
        let names = c_strings(&header);
        // The local player comes first, then the map name, then the roster.
        out.map_name = names
            .iter()
            .skip(1)
            .find(|n| n.contains(' ') || n.len() > 12)
            .cloned()
            .unwrap_or_default();
        for name in &names {
            if *name == out.map_name || out.slots.iter().any(|s| s.name == *name) {
                continue;
            }
            out.slots.push(ReplaySlot {
                name: name.clone(),
                race: 0,
            });
        }

        // Framed blocks until the framing stops making sense.
        while off + 12 <= bytes.len() {
            let Some((data, next)) = read_block(bytes, off) else {
                break;
            };
            if data.starts_with(b"VER ") {
                out.chk.extend_from_slice(&data);
            } else if data.len() > 16 {
                // The player-list block carries the roster.
                for name in c_strings(&data) {
                    if name == out.map_name || out.slots.iter().any(|s| s.name == name) {
                        continue;
                    }
                    out.slots.push(ReplaySlot { name, race: 0 });
                }
            }
            off = next;
        }

        // Chunked continuation of the CHK: [u32 size][zlib] blocks.
        while off + 4 <= bytes.len() {
            let size = u32::from_le_bytes(bytes[off..off + 4].try_into().ok()?) as usize;
            if size == 0 || size > bytes.len() - off - 4 {
                break;
            }
            let Some(chunk) = inflate(&bytes[off + 4..off + 4 + size]) else {
                break;
            };
            // Every chunk of this run belongs to the embedded CHK.
            out.chk.extend_from_slice(&chunk);
            off += 4 + size;
        }

        // The map continues after the first `VER ` block, alternating framed
        // 8 KiB blocks with bare chunks, and then comes the roster/action
        // stream.  The newer container revision (0x8f8e) needs both framings
        // here or the CHK ends up without MTXM and cannot rebuild the map.
        // Once the `VER ` block appears, every following payload belongs to
        // the map until the named sections start; only the first one carries
        // the `VER ` header, so a "starts with VER" test alone truncated the
        // CHK (it ended up without MTXM).
        let mut in_chk = false;
        loop {
            if let Some((data, next)) = read_block(bytes, off) {
                if data.starts_with(b"VER ") {
                    out.chk.extend_from_slice(&data);
                    in_chk = true;
                } else if in_chk && !data.starts_with(b"SKIN") && data.len() > 64 {
                    out.chk.extend_from_slice(&data);
                } else {
                    for name in c_strings(&data) {
                        if name == out.map_name || out.slots.iter().any(|s| s.name == name) {
                            continue;
                        }
                        out.slots.push(ReplaySlot { name, race: 0 });
                    }
                }
                off = next;
                continue;
            }
            if off + 4 <= bytes.len() {
                let size =
                    u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap_or([0; 4])) as usize;
                if size > 8
                    && size <= bytes.len() - off - 4
                    && let Some(chunk) = inflate(&bytes[off + 4..off + 4 + size])
                {
                    if chunk.starts_with(b"VER ") {
                        out.chk.extend_from_slice(&chunk);
                        in_chk = true;
                    } else if in_chk {
                        out.chk.extend_from_slice(&chunk);
                    }
                    off += 4 + size;
                    continue;
                }
            }
            break;
        }

        // Trailing named sections, then whatever is left (commands).
        let mut rest = &bytes[off.min(bytes.len())..];
        while rest.len() >= 8 {
            let tag = &rest[0..4];
            if !tag.iter().all(|b| b.is_ascii_uppercase()) {
                break;
            }
            let size = u32::from_le_bytes(rest[4..8].try_into().ok()?) as usize;
            if size > rest.len() - 8 {
                break;
            }
            out.sections.push(String::from_utf8_lossy(tag).to_string());
            // The section body may itself be a [count][size][zlib] block.
            let body = &rest[8..8 + size];
            if let Some((data, _)) = read_block(body, 0)
                && let Some(first) = c_strings(&data).first()
            {
                out.slots.push(ReplaySlot {
                    name: first.clone(),
                    race: 0,
                });
            }
            rest = &rest[8 + size..];
        }
        out.commands = rest.to_vec();
        Some(out)
    }

    /// True when the file carries the action stream (a player-saved replay).
    pub fn has_commands(&self) -> bool {
        self.commands.len() >= 4
    }
}

/// One action of a player-saved replay.
///
/// The retail stream is a sequence of 4-byte records; the first byte is the
/// number of frames since the previous action, the second the player slot
/// (1-based, 0 for the neutral player) and the last two the action word. This
/// decoding is a **hypothesis** backed by the container layout: it is stated
/// here so it can be falsified the moment a replay with a non-empty stream is
/// available (the shipped autosaves carry none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// Absolute simulation frame (24 per second).
    pub frame: u32,
    /// Player slot, 0 for neutral.
    pub player: u8,
    /// Raw action word (low byte action id, high byte data).
    pub action: u16,
}

/// A decoded action stream.
#[derive(Clone, Debug, Default)]
pub struct CommandStream {
    pub commands: Vec<Command>,
    /// False when more than 10% of the records carry an impossible player id,
    /// which means the 4-byte layout does not hold for this file.
    pub plausible: bool,
}

impl CommandStream {
    /// Split the raw bytes into 4-byte records and decode them.
    pub fn parse(bytes: &[u8]) -> Self {
        let mut commands = Vec::with_capacity(bytes.len() / 4);
        let mut frame = 0u32;
        let mut bad = 0usize;
        for rec in bytes.as_chunks::<4>().0 {
            frame = frame.saturating_add(rec[0] as u32);
            let player = rec[1];
            if player > 8 {
                bad += 1;
            }
            let action = u16::from_le_bytes([rec[2], rec[3]]);
            commands.push(Command {
                frame,
                player,
                action,
            });
        }
        let plausible = !commands.is_empty() && bad * 10 <= commands.len();
        Self {
            commands,
            plausible,
        }
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Last frame referenced by the stream: the tick a driver has to reach.
    pub fn last_frame(&self) -> u32 {
        self.commands.last().map(|c| c.frame).unwrap_or(0)
    }

    /// Commands that take effect at exactly `frame`.
    pub fn at_frame(&self, frame: u32) -> &[Command] {
        let start = self.commands.partition_point(|c| c.frame < frame);
        let end = self.commands.partition_point(|c| c.frame <= frame);
        &self.commands[start..end]
    }

    /// How many commands each player slot issued (index 0..=8).
    pub fn per_player(&self) -> [usize; 9] {
        let mut out = [0usize; 9];
        for c in &self.commands {
            out[(c.player as usize).min(8)] += 1;
        }
        out
    }
}

impl RetailReplay {
    /// Decode the action stream, if the file carries one.
    pub fn command_stream(&self) -> Option<CommandStream> {
        self.has_commands()
            .then(|| CommandStream::parse(&self.commands))
    }
}

/// Read one `[u32 checksum][u32 count][u32 size][zlib]` block.
fn read_block(bytes: &[u8], off: usize) -> Option<(Vec<u8>, usize)> {
    if off + 12 > bytes.len() {
        return None;
    }
    let size = u32::from_le_bytes(bytes[off + 8..off + 12].try_into().ok()?) as usize;
    if size == 0 || size > bytes.len() - off - 12 {
        return None;
    }
    let body = &bytes[off + 12..off + 12 + size];
    let data = inflate(body).unwrap_or_else(|| body.to_vec());
    Some((data, off + 12 + size))
}

/// zlib-inflate, or `None` when the payload is not compressed.
fn inflate(body: &[u8]) -> Option<Vec<u8>> {
    if body.len() < 2 || body[0] != 0x78 {
        return None;
    }
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(body)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Printable C strings of at least three characters.
fn c_strings(data: &[u8]) -> Vec<String> {
    data.split(|b| *b == 0)
        .filter(|s| {
            s.len() >= 3
                && s.iter().all(|b| (32..127).contains(b))
                && s.first() != Some(&b' ')
                && s.last() != Some(&b' ')
        })
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect()
}

/// The retail client's replay folder: `SC_REPLAY_BASE`, else
/// `<drive_c>/users/<user>/Documents/StarCraft/Maps/Replays` next to a Wine
/// or Proton `SC_INSTALL` (`<drive_c>/Program Files (x86)/StarCraft`).
pub fn default_replay_base() -> Option<std::path::PathBuf> {
    if let Some(base) = std::env::var_os("SC_REPLAY_BASE") {
        return Some(base.into());
    }
    let install = std::path::PathBuf::from(std::env::var_os("SC_INSTALL")?);
    let users = install.parent()?.parent()?.join("users");
    std::fs::read_dir(users)
        .ok()?
        .flatten()
        .map(|u| u.path().join("Documents/StarCraft/Maps/Replays"))
        .find(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn autosave() -> Option<Vec<u8>> {
        let root = std::env::var_os("SC_REPLAY_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| default_replay_base().map(|b| b.join("AutoSave/20261027")))?;
        let path = root.join("135033,(8)The Hunters.rep");
        std::fs::read(path).ok()
    }

    #[test]
    fn parses_the_shipped_autosave_container() {
        let Some(bytes) = autosave() else {
            eprintln!("skip: no autosave replay on this machine");
            return;
        };
        let rep = RetailReplay::parse(&bytes).expect("parse replay");
        assert_eq!(rep.version, 0xa70d, "1.23.10 container version");
        assert_eq!(rep.map_name, "The Hunters");
        assert!(
            rep.chk.starts_with(b"VER "),
            "embedded CHK should start with the version section"
        );
        assert!(rep.chk.len() > 100_000, "full map data expected");
        assert!(rep.sections.iter().any(|s| s == "LMTS"));
        // The autosaves carry no action stream; a player-saved replay does.
        assert!(!rep.has_commands());
    }

    #[test]
    fn decodes_a_four_byte_command_stream() {
        // Three records: +12 frames P1 action 0x0123, +6 frames P2 0x0004,
        // +0 frames P1 0x0005.
        let raw = [12u8, 1, 0x23, 0x01, 6, 2, 0x04, 0x00, 0, 1, 0x05, 0x00];
        let st = CommandStream::parse(&raw);
        assert_eq!(st.len(), 3);
        assert!(st.plausible);
        assert_eq!(st.last_frame(), 18);
        assert_eq!(
            st.commands[0],
            Command {
                frame: 12,
                player: 1,
                action: 0x0123
            }
        );
        assert_eq!(st.at_frame(18).len(), 2, "frames 18 and 18 (zero delta)");
        assert_eq!(st.per_player()[1], 2);
        assert_eq!(st.per_player()[2], 1);
    }

    #[test]
    fn flags_an_implausible_stream() {
        // Every record has an impossible player id.
        let raw = [1u8, 200, 0, 0, 1, 201, 0, 0, 1, 202, 0, 0];
        let st = CommandStream::parse(&raw);
        assert_eq!(st.len(), 3);
        assert!(!st.plausible, "player ids above 8 must falsify the layout");
    }

    #[test]
    fn rejects_foreign_payloads() {
        assert!(RetailReplay::parse(b"not a replay at all").is_none());
        assert!(RetailReplay::parse(&[0u8; 8]).is_none());
    }

    #[test]
    fn inflates_a_zlib_block_and_falls_back_to_raw() {
        // "VER " zlib-compressed.
        let raw = b"VER \x02\x00\x00\x00";
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        use std::io::Write;
        enc.write_all(raw).unwrap();
        let packed = enc.finish().unwrap();
        assert_eq!(inflate(&packed).as_deref(), Some(&raw[..]));
        assert!(inflate(b"\x00\x01\x02").is_none());
    }
}
