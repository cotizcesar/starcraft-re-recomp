# Platform and runtime

## Target platform

Windows PE GUI 6.00, Win32 x86 **and** Win64 x86-64. Runs on Bazzite via
Lutris/Battle.net (Proton). Dependencies: `D3DX9_42.dll`, `D3DCompiler_42.dll`,
`ClientSdk.dll` (Battle.net SDK), CEF only in the launcher.

## Executables

| Path | Bits | Size | Role |
|---|---|---:|---|
| `x86/StarCraft.exe` | 32 | 12 820 096 | classic game; RE target |
| `x86_64/StarCraft.exe` | 64 | 15 734 912 | 64-bit variant |
| `StarCraft Launcher.exe` | 32 | 5 004 152 | CEF launcher/login |
| `x86/cef/SceneCefBrowser.exe` | 32 | 2 560 640 | launcher UI renderer |
| `x86*/ClientSdk.dll` | 32/64 | ~12/17 MB | online SDK |
| `x86*/BlizzardError.exe` | 32/64 | 893 384 | error reporter |

## Data

CASC (`Data/data/data.000…005`, 48 `.idx`, 47 `Data/indices`), `Data/config`,
`Maps/` (323 `.scm`/`.scx` MPQ with CHK), `locales/enUS/`.

## Implications

Static analysis on the decrypted memory image (on-disk PE is encrypted); data via
`src/retail.rs`; equivalence against the 32-bit binary under Proton.
Historical detail: the superseded `archive/platform-and-runtime.md` was deleted (git history keeps it); this file is canonical.
