# Matriz herramienta × capacidad (observada)

| Capacidad | REA+Ghidra | rea-dotnet | artifact-graph | CLI (`file`/`objdump`) | Lectores repo |
|---|---|---|---|---|---|
| PE x86/x64 desensamblado | sí (PASS) | no | no | parcial (cabeceras/strings) | no |
| Decompilación pseudocódigo | sí (Ghidra) | no (nunca C#) | no | no | transcripciones `handlers.rs` |
| Xrefs/callers/callees | sí | no | no | no | no |
| .NET administrado | n/a (sin objetivo) | sí (metadatos) | no | no | n/a |
| CASC/BLTE/MPQ/DCL | no | no | parcial (`extract_artifact`) | no | sí (`retail.rs`) |
| DAT/TBL/CHK/iscript/AI | no | no | no | no | sí |
| GRP/tileset/WAV/Ogg/DDS | no | no | no | no | sí (`render.rs`, `audio.rs`, `dds.rs`) |
| Dinámico/trazas | no (REA estático) | no | no | no | Proton + observación |

Externa requerida: solo Ghidra 12.1.4 + JDK 21 (presentes). Nada más por
instalar para la ruta crítica.
